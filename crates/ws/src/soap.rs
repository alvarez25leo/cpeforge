//! Cliente SOAP 1.1 document/literal. Reutiliza el transporte de `gre-api`:
//! TLS verificado, timeouts, límite de respuesta y sin reintentos implícitos.
//!
//! Este módulo no reintenta nunca: un `sendBill` o `sendSummary` que termina
//! en timeout pudo llegar a SUNAT, y repetirlo podría duplicar el envío.

use std::future::Future;

use base64::Engine as _;
use cpeforge_gre_api::{HttpRequest, HttpResponse, HttpTransport, Method, Secret};
use cpeforge_xml_limits::{
    DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE, check_tree_markup, exceeds_depth,
};
use roxmltree::{Document, Node, ParsingOptions};

use crate::WsError;
use crate::packet;

const SOAP: &str = packet::SOAP11;
const SERVICE: &str = "http://service.sunat.gob.pe";
const WSSE: &str =
    "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
const PASSWORD_TYPE: &str = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordText";
/// Tamaño máximo del XML SOAP recibido/generado.
pub const MAX_SOAP_BYTES: usize = 16 * 1024 * 1024;
/// Profundidad máxima de anidamiento de una respuesta SOAP (el `Envelope` es
/// el nivel 1). Una respuesta más profunda se informa como documento ilegible.
/// Un envelope de SUNAT con CDR no llega a 20 niveles; con 64 el análisis
/// recursivo cabe en la pila de 2 MiB de un hilo de Tokio también en debug
/// (ver [`crate::cdr::MAX_XML_DEPTH`]).
pub const MAX_SOAP_DEPTH: usize = 64;
/// Máximo de nodos del árbol de una respuesta SOAP.
const MAX_SOAP_NODES: u32 = 100_000;

/// Endpoints de los servicios SOAP de SUNAT. Son constantes: consultarlas no
/// realiza ninguna solicitud de red.
#[derive(Debug, Clone, Copy)]
pub struct SunatEndpoints;

impl SunatEndpoints {
    /// Facturación beta.
    pub const FE_BETA: &'static str =
        "https://e-beta.sunat.gob.pe/ol-ti-itcpfegem-beta/billService";
    /// Facturación homologación.
    pub const FE_HOMOLOGACION: &'static str =
        "https://www.sunat.gob.pe/ol-ti-itcpgem-sqa/billService";
    /// Facturación producción.
    pub const FE_PRODUCCION: &'static str =
        "https://e-factura.sunat.gob.pe/ol-ti-itcpfegem/billService";
    /// Consulta de CDR.
    pub const FE_CONSULTA_CDR: &'static str =
        "https://e-factura.sunat.gob.pe/ol-it-wsconscpegem/billConsultService";
    /// Guía SOAP beta, sustituida por la API GRE.
    #[deprecated(note = "usar GRE API")]
    pub const GUIA_BETA: &'static str =
        "https://e-beta.sunat.gob.pe/ol-ti-itemision-guia-gem-beta/billService";
    /// Guía SOAP producción, sustituida por la API GRE.
    #[deprecated(note = "usar GRE API")]
    pub const GUIA_PRODUCCION: &'static str =
        "https://e-guiaremision.sunat.gob.pe/ol-ti-itemision-guia-gem/billService";
    /// Retención/percepción beta.
    pub const RETENCION_BETA: &'static str =
        "https://e-beta.sunat.gob.pe/ol-ti-itemision-otroscpe-gem-beta/billService";
    /// Retención/percepción producción.
    pub const RETENCION_PRODUCCION: &'static str =
        "https://e-factura.sunat.gob.pe/ol-ti-itemision-otroscpe-gem/billService";
    /// URL del WSDL publicado. El contrato se valida contra una copia local
    /// fijada; el cliente nunca descarga el WSDL.
    pub const WSDL_ENDPOINT: &'static str =
        "https://e-beta.sunat.gob.pe/ol-ti-itcpfegem-beta/billService?wsdl";
}

/// Respuesta de `getStatus` / `getStatusCdr`.
#[derive(Clone, PartialEq, Eq)]
pub struct SoapStatus {
    /// Código como cadena (preserva ceros a la izquierda).
    pub code: Option<String>,
    /// Mensaje opcional del servicio.
    pub message: Option<String>,
    /// ZIP CDR opcional.
    pub content: Option<Vec<u8>>,
}

impl std::fmt::Debug for SoapStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoapStatus")
            .field("code", &self.code)
            .field("message", &"<oculto>")
            .field("content", &self.content.as_ref().map(Vec::len))
            .finish()
    }
}

/// Puerto de las cuatro operaciones usadas por `ws`; permite dobles sin red.
pub trait WsClient: Send + Sync {
    /// `sendBill`.
    fn send_bill(
        &self,
        filename: &str,
        zip: &[u8],
    ) -> impl Future<Output = Result<Option<Vec<u8>>, WsError>> + Send;
    /// `sendSummary`.
    fn send_summary(
        &self,
        filename: &str,
        zip: &[u8],
    ) -> impl Future<Output = Result<Option<String>, WsError>> + Send;
    /// `getStatus` del servicio de tickets.
    fn get_status(
        &self,
        ticket: &str,
    ) -> impl Future<Output = Result<Option<SoapStatus>, WsError>> + Send;
    /// `getStatus` o `getStatusCdr` de consulta.
    fn consult(
        &self,
        cdr: bool,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> impl Future<Output = Result<Option<SoapStatus>, WsError>> + Send;
}

/// Adaptador SOAP sobre un `HttpTransport` inyectable. El transporte incluido
/// en `gre-api` verifica siempre el certificado y el nombre del servidor TLS.
#[derive(Clone)]
pub struct SoapClient<T> {
    transport: T,
    endpoint: String,
    username: Secret,
    password: Secret,
}

impl<T> std::fmt::Debug for SoapClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoapClient")
            .field("endpoint", &"<oculto>")
            .field("username", &"<oculto>")
            .field("password", &self.password)
            .finish()
    }
}

impl<T: HttpTransport> SoapClient<T> {
    /// Configura endpoint y UsernameToken. `transport` define límites y TLS.
    #[must_use]
    pub fn new(
        transport: T,
        endpoint: impl Into<String>,
        username: impl Into<Secret>,
        password: impl Into<Secret>,
    ) -> Self {
        Self {
            transport,
            endpoint: endpoint.into(),
            username: username.into(),
            password: password.into(),
        }
    }

    async fn call(
        &self,
        operation: &str,
        fields: &[(&str, Field<'_>)],
    ) -> Result<DocumentOwned, WsError> {
        let request = self.envelope(operation, fields)?;
        let response = self
            .transport
            .send(request)
            .await
            .map_err(|_| WsError::Transport)?;
        if response.body.len() > MAX_SOAP_BYTES {
            return Err(WsError::Soap("respuesta demasiado grande"));
        }
        if let Some(fault) = http_fault(&response) {
            return Err(fault);
        }
        // Con estado < 400, un cuerpo vacío es una respuesta sin contenido.
        if response.body.is_empty() {
            return Ok(DocumentOwned { xml: None });
        }
        // Solo UTF-8: un cuerpo en otra codificación es un documento ilegible.
        let xml = String::from_utf8(response.body).map_err(|_| packet::no_xml_document())?;
        // El texto se guarda para que roxmltree pueda prestar referencias de él.
        Ok(DocumentOwned { xml: Some(xml) })
    }

    /// Construye una solicitud SOAP sin enviarla; útil para inspección y tests.
    ///
    /// # Errors
    ///
    /// Operación no admitida, nombre de campo no alfanumérico o exceso de tamaño.
    pub fn request(
        &self,
        operation: &str,
        fields: &[(&str, String)],
    ) -> Result<HttpRequest, WsError> {
        let fields: Vec<(&str, Field<'_>)> = fields
            .iter()
            .map(|(name, value)| (*name, Field::Text(value)))
            .collect();
        self.envelope(operation, &fields)
    }

    /// Envelope con WSSE UsernameToken escrito en un único buffer de tamaño
    /// calculado de antemano: el contenido Base64 del ZIP se codifica
    /// directamente en él, sin cadenas intermedias.
    fn envelope(
        &self,
        operation: &str,
        fields: &[(&str, Field<'_>)],
    ) -> Result<HttpRequest, WsError> {
        if fields
            .iter()
            .any(|(name, _)| !name.bytes().all(|b| b.is_ascii_alphanumeric()))
        {
            return Err(WsError::Security("nombre de campo SOAP inválido"));
        }
        if !matches!(
            operation,
            "sendBill" | "sendSummary" | "getStatus" | "getStatusCdr"
        ) {
            return Err(WsError::Soap("operación no admitida"));
        }
        let username = self.username.expose();
        let password = self.password.expose();
        let content: usize = fields
            .iter()
            .map(|(name, value)| 2 * name.len() + 5 + value.len_hint())
            .sum();
        let capacity =
            ENVELOPE_OVERHEAD + 2 * operation.len() + username.len() + password.len() + content;
        // Solo es una reserva: el límite se comprueba sobre el resultado.
        let mut xml = String::with_capacity(capacity.min(MAX_SOAP_BYTES + 1));
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?><soap:Envelope xmlns:soap=\"");
        xml.push_str(SOAP);
        xml.push_str("\" xmlns:ns=\"");
        xml.push_str(SERVICE);
        xml.push_str("\" xmlns:wsse=\"");
        xml.push_str(WSSE);
        xml.push_str("\"><soap:Header><wsse:Security><wsse:UsernameToken><wsse:Username>");
        escape_into(&mut xml, username);
        xml.push_str("</wsse:Username><wsse:Password Type=\"");
        xml.push_str(PASSWORD_TYPE);
        xml.push_str("\">");
        escape_into(&mut xml, password);
        xml.push_str(
            "</wsse:Password></wsse:UsernameToken></wsse:Security></soap:Header><soap:Body><ns:",
        );
        xml.push_str(operation);
        xml.push('>');
        for (name, value) in fields {
            xml.push('<');
            xml.push_str(name);
            xml.push('>');
            match value {
                Field::Text(text) => escape_into(&mut xml, text),
                // El alfabeto Base64 estándar no contiene caracteres que
                // requieran escape XML.
                Field::Base64(bytes) => {
                    base64::engine::general_purpose::STANDARD.encode_string(bytes, &mut xml);
                }
            }
            xml.push_str("</");
            xml.push_str(name);
            xml.push('>');
        }
        xml.push_str("</ns:");
        xml.push_str(operation);
        xml.push_str("></soap:Body></soap:Envelope>");
        if xml.len() > MAX_SOAP_BYTES {
            return Err(WsError::Soap("solicitud demasiado grande"));
        }
        Ok(HttpRequest {
            method: Method::Post,
            url: self.endpoint.clone(),
            headers: vec![
                (
                    "Content-Type".to_owned(),
                    "text/xml; charset=utf-8".to_owned(),
                ),
                ("SOAPAction".to_owned(), format!("\"urn:{operation}\"")),
            ],
            body: xml.into_bytes(),
        })
    }
}

/// Con estado HTTP >= 400, un cuerpo vacío, o uno que no es XML ni por
/// `Content-Type` ni por empezar con `<?xml`, es el fault `HTTP` con la frase
/// del estado como mensaje. Así se informa, por ejemplo, el
/// `401 Unauthorized` con que SUNAT BETA limita envíos seguidos.
fn http_fault(response: &HttpResponse) -> Option<WsError> {
    if response.status < 400 {
        return None;
    }
    let content_type_xml = response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Content-Type"))
        .is_some_and(|(_, value)| {
            let value = value.trim_start_matches([' ', '\t']);
            // El tipo de medio, hasta `;`, debe ser prefijo de un tipo XML
            // (`text/` también se acepta).
            let media = value.split_once(';').map_or(value, |(media, _)| media);
            "text/xml".starts_with(media) || "application/soap+xml".starts_with(media)
        });
    let body = response.body.as_slice();
    let error = body.is_empty()
        || (!content_type_xml && {
            // Se saltan los bytes de control iniciales y los bytes >= 0x80 (un
            // BOM UTF-8, por ejemplo) antes de buscar `<?xml`.
            let start = body
                .iter()
                .position(|&b| b == 0 || (b' '..0x80).contains(&b))
                .unwrap_or(body.len());
            !body
                .get(start..)
                .is_some_and(|rest| rest.starts_with(b"<?xml"))
        });
    error.then(|| WsError::SoapFault {
        code: "HTTP".to_owned(),
        message: response.reason.clone(),
        detail: None,
    })
}

struct DocumentOwned {
    /// `None`: cuerpo vacío.
    xml: Option<String>,
}

impl DocumentOwned {
    /// `None` si el cuerpo estaba vacío. Los DTD no se procesan nunca, y un
    /// documento con más de [`MAX_SOAP_DEPTH`] niveles, o con demasiados
    /// atributos o espacios de nombres, se rechaza antes de construir el
    /// árbol.
    fn parse(&self) -> Result<Option<Document<'_>>, WsError> {
        let Some(xml) = &self.xml else {
            return Ok(None);
        };
        if exceeds_depth(xml, MAX_SOAP_DEPTH) {
            return Err(packet::no_xml_document());
        }
        // Atributos por elemento y ámbitos de espacios de nombres: `roxmltree`
        // es cuadrático en los primeros y copia los segundos por elemento.
        if check_tree_markup(xml, DEFAULT_MAX_ATTRIBUTES, DEFAULT_MAX_NAMESPACE_SCOPE).is_err() {
            return Err(WsError::Soap("límite de nodos XML"));
        }
        Document::parse_with_options(
            xml,
            ParsingOptions {
                allow_dtd: false,
                nodes_limit: MAX_SOAP_NODES,
                ..ParsingOptions::default()
            },
        )
        .map(Some)
        .map_err(|error| match error {
            roxmltree::Error::DtdDetected => packet::dtd_fault(),
            roxmltree::Error::NodesLimitReached
            | roxmltree::Error::AttributesLimitReached
            | roxmltree::Error::NamespacesLimitReached => WsError::Soap("límite de nodos XML"),
            _ => packet::no_xml_document(),
        })
    }
}

/// Bytes fijos del envelope además de operación, credenciales y campos.
const ENVELOPE_OVERHEAD: usize = 640;

/// Valor de un parámetro del cuerpo SOAP.
enum Field<'a> {
    /// Texto que se escapa como XML.
    Text(&'a str),
    /// Binario que se escribe en Base64 estándar (`xsd:base64Binary`).
    Base64(&'a [u8]),
}

impl Field<'_> {
    /// Bytes que ocupa el valor escrito: exacto para Base64 y para texto sin
    /// caracteres que escapar.
    fn len_hint(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Base64(bytes) => bytes.len().div_ceil(3).saturating_mul(4),
        }
    }
}

/// Añade `input` a `out` con los cinco escapes predefinidos de XML.
fn escape_into(out: &mut String, input: &str) {
    let mut rest = input;
    while let Some(index) = rest.find(['&', '<', '>', '"', '\'']) {
        let (plain, tail) = rest.split_at(index);
        out.push_str(plain);
        let mut chars = tail.chars();
        out.push_str(match chars.next() {
            Some('&') => "&amp;",
            Some('<') => "&lt;",
            Some('>') => "&gt;",
            Some('"') => "&quot;",
            _ => "&apos;",
        });
        rest = chars.as_str();
    }
    out.push_str(rest);
}

fn direct_child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// Primer texto del hijo `name`; `Some("")` si el hijo existe sin texto.
fn field_text<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    direct_child(node, name).map(|child| child.text().unwrap_or_default())
}

fn field(node: Node<'_, '_>, name: &str) -> Option<String> {
    field_text(node, name).map(str::to_owned)
}

/// `Body` validado y el elemento de respuesta de `operation`
/// (`<operation>Response` en el espacio de nombres del servicio). Su
/// ausencia no es un error: el llamador recibe `None`.
fn parse_reply<'a, 'i>(
    doc: &'a Document<'i>,
    operation: &str,
) -> Result<Option<Node<'a, 'i>>, WsError> {
    let body = packet::body(doc)?;
    Ok(body.children().find(|n| {
        n.is_element()
            && n.tag_name().namespace() == Some(SERVICE)
            && n.tag_name().name().strip_suffix("Response") == Some(operation)
    }))
}

/// Respuesta de una consulta: no se admite vacía ni sin su elemento.
fn status_reply<'a, 'i>(
    doc: Option<&'a Document<'i>>,
    operation: &str,
) -> Result<Node<'a, 'i>, WsError> {
    let doc = doc.ok_or(WsError::InvalidResponse("respuesta vacía"))?;
    parse_reply(doc, operation)?.ok_or(WsError::InvalidResponse("elemento de respuesta ausente"))
}

fn decode_binary(value: Option<&str>) -> Result<Option<Vec<u8>>, WsError> {
    value
        .map(|s| {
            base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .map_err(|_| WsError::Soap("Base64 inválido"))
        })
        .transpose()
}

impl<T: HttpTransport> WsClient for SoapClient<T> {
    async fn send_bill(&self, filename: &str, zip: &[u8]) -> Result<Option<Vec<u8>>, WsError> {
        let fields = [
            ("fileName", Field::Text(filename)),
            ("contentFile", Field::Base64(zip)),
        ];
        let reply = self.call("sendBill", &fields).await?;
        let Some(doc) = reply.parse()? else {
            return Ok(None);
        };
        let response = parse_reply(&doc, "sendBill")?;
        decode_binary(response.and_then(|r| field_text(r, "applicationResponse")))
    }

    async fn send_summary(&self, filename: &str, zip: &[u8]) -> Result<Option<String>, WsError> {
        let fields = [
            ("fileName", Field::Text(filename)),
            ("contentFile", Field::Base64(zip)),
        ];
        let reply = self.call("sendSummary", &fields).await?;
        let Some(doc) = reply.parse()? else {
            return Ok(None);
        };
        Ok(parse_reply(&doc, "sendSummary")?.and_then(|r| field(r, "ticket")))
    }

    async fn get_status(&self, ticket: &str) -> Result<Option<SoapStatus>, WsError> {
        let reply = self
            .call("getStatus", &[("ticket", Field::Text(ticket))])
            .await?;
        let doc = reply.parse()?;
        parse_status(status_reply(doc.as_ref(), "getStatus")?, "status")
    }

    async fn consult(
        &self,
        cdr: bool,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<Option<SoapStatus>, WsError> {
        let operation = if cdr { "getStatusCdr" } else { "getStatus" };
        let numero = numero.to_string();
        let fields = [
            ("rucComprobante", Field::Text(ruc)),
            ("tipoComprobante", Field::Text(tipo)),
            ("serieComprobante", Field::Text(serie)),
            ("numeroComprobante", Field::Text(&numero)),
        ];
        let reply = self.call(operation, &fields).await?;
        let doc = reply.parse()?;
        parse_status(
            status_reply(doc.as_ref(), operation)?,
            if cdr { "statusCdr" } else { "status" },
        )
    }
}

fn parse_status(parent: Node<'_, '_>, name: &str) -> Result<Option<SoapStatus>, WsError> {
    let Some(node) = direct_child(parent, name) else {
        return Ok(None);
    };
    Ok(Some(SoapStatus {
        code: field(node, "statusCode"),
        message: field(node, "statusMessage"),
        content: decode_binary(field_text(node, "content"))?,
    }))
}
