//! Fachada `See` para SOAP.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, PoisonError, RwLock};

use cpeforge_core::model::response::StatusResult;
use cpeforge_gre_api::{HttpTransport, Secret};
use cpeforge_ws::WsError;
use cpeforge_ws::cdr::resolve_type_and_filename;
use cpeforge_ws::services::{BillSender, ExtService, SummarySender, fault_code_digits};
use cpeforge_ws::soap::{SoapClient, SoapStatus, WsClient};
use cpeforge_ws::zip::ZipTime;
use cpeforge_xml::RenderOptions;
use cpeforge_xmldsig::sunat::SignedXml;

use crate::CpeForgeError;
use crate::factory::{FeFactory, SendResult, Sender, XmlSigner};
use crate::options::BuilderOptionsPatch;
use crate::resolver::{DocumentKind, DocumentRef, SenderKind, XmlBuilderResolver, sender_kind};

/// Firmador compartido entre `See` (que cambia el certificado) y la factory
/// (que firma). Firmar solo lee: `SignedXml::sign_xml` toma `&self` y guarda la
/// clave cargada en un `OnceLock`, así que varias firmas pueden leer a la vez.
/// Un pánico durante una firma no deja el firmador inutilizable: el estado
/// solo cambia en `set_certificate`, que reemplaza el PEM completo.
struct SharedSigner(Arc<RwLock<SignedXml>>);

impl XmlSigner for SharedSigner {
    fn sign_xml(&self, xml: &str) -> Result<String, CpeForgeError> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .sign_xml(xml)
            .map_err(Into::into)
    }
}

/// Catálogo de descripciones de códigos de error para los faults SOAP.
pub trait ErrorCodeProvider: Send + Sync {
    /// Descripción pública del código, o `None` para conservar el faultstring.
    fn get_value(&self, code: &str) -> Option<String>;
}

// El catálogo fija todos los fallos de carga en `new()`: una instancia ya
// construida ofrece consultas infalibles para los faults SOAP.
#[cfg(feature = "xcodes")]
impl ErrorCodeProvider for cpeforge_xcodes::XmlErrorCodeProvider {
    fn get_value(&self, code: &str) -> Option<String> {
        Some(self.get(code).to_owned())
    }
}

struct ProviderClient<T> {
    inner: SoapClient<T>,
    provider: Option<Arc<dyn ErrorCodeProvider>>,
}

impl<T: HttpTransport> ProviderClient<T> {
    /// `getErrorByCode` con proveedor: si el código tiene descripción, esa es
    /// el mensaje (sin `detail`); si no, `ws` arma el mensaje como sin proveedor.
    fn map_fault(&self, error: WsError) -> WsError {
        let WsError::SoapFault {
            code,
            message,
            detail,
        } = error
        else {
            return error;
        };
        let digits = fault_code_digits(&code, &message);
        let translated = digits.as_deref().and_then(|digits| {
            self.provider
                .as_ref()
                .and_then(|provider| provider.get_value(digits))
                // "" y "0" no sustituyen al faultstring (regla de la referencia).
                .filter(|text| !text.is_empty() && text != "0")
        });
        match (digits, translated) {
            (Some(digits), Some(translated)) => WsError::SoapFault {
                code: digits,
                message: translated,
                detail: None,
            },
            _ => WsError::SoapFault {
                code,
                message,
                detail,
            },
        }
    }
}

impl<T: HttpTransport> WsClient for ProviderClient<T> {
    async fn send_bill(&self, filename: &str, zip: &[u8]) -> Result<Option<Vec<u8>>, WsError> {
        self.inner
            .send_bill(filename, zip)
            .await
            .map_err(|e| self.map_fault(e))
    }
    async fn send_summary(&self, filename: &str, zip: &[u8]) -> Result<Option<String>, WsError> {
        self.inner
            .send_summary(filename, zip)
            .await
            .map_err(|e| self.map_fault(e))
    }
    async fn get_status(&self, ticket: &str) -> Result<Option<SoapStatus>, WsError> {
        self.inner.get_status(ticket).await
    }
    async fn consult(
        &self,
        cdr: bool,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<Option<SoapStatus>, WsError> {
        self.inner.consult(cdr, ruc, tipo, serie, numero).await
    }
}

struct SoapSender<T> {
    client: SoapClient<T>,
    kind: SenderKind,
    provider: Option<Arc<dyn ErrorCodeProvider>>,
    zip_time: ZipTime,
}

impl<T: HttpTransport + Clone + 'static> Sender for SoapSender<T> {
    fn send<'a>(
        &'a self,
        filename: &'a str,
        xml: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<SendResult, CpeForgeError>> + Send + 'a>> {
        Box::pin(async move {
            let client = ProviderClient {
                inner: self.client.clone(),
                provider: self.provider.clone(),
            };
            match self.kind {
                SenderKind::Bill => Ok(SendResult::Bill(
                    BillSender::new(client)
                        .with_zip_time(self.zip_time)
                        .send(filename, xml)
                        .await?,
                )),
                SenderKind::Summary => Ok(SendResult::Summary(
                    SummarySender::new(client)
                        .with_zip_time(self.zip_time)
                        .send(filename, xml)
                        .await?,
                )),
            }
        })
    }
}

/// Endpoint SOAP predeterminado (beta de SUNAT) mientras no se llame a `set_service`.
pub const DEFAULT_SOAP_ENDPOINT: &str =
    "https://e-beta.sunat.gob.pe:443/ol-ti-itcpfegem-beta/billService";

/// Fachada SOAP; transporte inyectado y sin solicitud hasta invocar envío o estado.
pub struct See<T> {
    transport: T,
    endpoint: String,
    username: Secret,
    password: Secret,
    signer: Arc<RwLock<SignedXml>>,
    factory: FeFactory,
    options: RenderOptions,
    cache_path: Option<String>,
    code_provider: Option<Arc<dyn ErrorCodeProvider>>,
    zip_time: ZipTime,
}

impl<T> std::fmt::Debug for See<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("See")
            .field("endpoint", &"<oculto>")
            .field("credentials", &"<ocultas>")
            .field("signer", &"<oculto>")
            .field("factory", &self.factory)
            .finish()
    }
}

impl<T: HttpTransport + Clone + 'static> See<T> {
    /// Construye la fachada sin escape automático y con el endpoint SOAP beta.
    #[must_use]
    pub fn new(transport: T) -> Self {
        let signer = Arc::new(RwLock::new(SignedXml::new()));
        let mut factory = FeFactory::new();
        factory.set_signer(Some(Box::new(SharedSigner(signer.clone()))));
        Self {
            transport,
            endpoint: DEFAULT_SOAP_ENDPOINT.to_owned(),
            username: Secret::new(""),
            password: Secret::new(""),
            signer,
            factory,
            options: RenderOptions::cpeforge(),
            cache_path: None,
            code_provider: None,
            zip_time: ZipTime::default(),
        }
    }

    /// Factory mutable.
    pub fn factory(&mut self) -> &mut FeFactory {
        &mut self.factory
    }
    /// Referencia de solo lectura a la factory.
    #[must_use]
    pub fn factory_ref(&self) -> &FeFactory {
        &self.factory
    }
    /// Mezcla las opciones indicadas con las actuales.
    pub fn set_builder_options(&mut self, options: BuilderOptionsPatch) -> &mut Self {
        options.apply(&mut self.options);
        self
    }
    /// Guarda la ruta de caché solicitada; el render no usa caché en disco.
    pub fn set_cache_path(&mut self, path: Option<&str>) -> &mut Self {
        self.cache_path = path.filter(|p| !p.is_empty()).map(str::to_owned);
        self
    }
    /// Ruta de cache solicitada (sin efecto sobre el renderer Rust).
    #[must_use]
    pub fn cache_path(&self) -> Option<&str> {
        self.cache_path.as_deref()
    }
    /// Configura PEM con clave y certificado efímeros. El error PEM se detecta al firmar.
    pub fn set_certificate(&mut self, pem: &str) -> Result<&mut Self, CpeForgeError> {
        self.signer
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .set_certificate(pem);
        Ok(self)
    }
    /// Configura usuario WSSE completo y contraseña.
    pub fn set_credentials(&mut self, user: &str, password: &str) -> &mut Self {
        self.username = Secret::new(user);
        self.password = Secret::new(password);
        self
    }
    /// Configura RUC+usuario secundario como username WSSE.
    pub fn set_clave_sol(&mut self, ruc: &str, user: &str, password: &str) -> &mut Self {
        self.set_credentials(&format!("{ruc}{user}"), password)
    }
    /// Selecciona el endpoint SOAP. El transporte impone TLS y límites.
    pub fn set_service(&mut self, endpoint: impl Into<String>) -> &mut Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Traduce errores de `sendBill` y `sendSummary`; `get_status` usa
    /// `ExtService` sin este proveedor.
    pub fn set_code_provider(&mut self, provider: Option<Arc<dyn ErrorCodeProvider>>) -> &mut Self {
        self.code_provider = provider;
        self
    }

    /// Hora del ZIP enviado; por defecto, la hora actual en UTC.
    pub fn set_zip_time(&mut self, zip_time: ZipTime) -> &mut Self {
        self.zip_time = zip_time;
        self
    }

    fn client(&self) -> SoapClient<T> {
        SoapClient::new(
            self.transport.clone(),
            &self.endpoint,
            self.username.clone(),
            self.password.clone(),
        )
    }

    fn configure_builder(&mut self, kind: DocumentKind) {
        self.factory.set_builder(Some(Box::new(
            XmlBuilderResolver::new(self.options.clone()).find(kind),
        )));
    }

    fn configure_factory(&mut self, kind: DocumentKind) {
        self.configure_builder(kind);
        self.factory.set_sender(Some(Box::new(SoapSender {
            client: self.client(),
            kind: sender_kind(kind),
            provider: self.code_provider.clone(),
            zip_time: self.zip_time,
        })));
    }

    /// Construye y firma sin enviar; no sustituye el sender actual.
    pub fn get_xml_signed(&mut self, document: DocumentRef<'_>) -> Result<&str, CpeForgeError> {
        self.configure_builder(document.kind());
        self.factory.get_xml_signed(document)
    }

    /// Construye, firma y envía por `sendBill` o `sendSummary`.
    pub async fn send(&mut self, document: DocumentRef<'_>) -> Result<SendResult, CpeForgeError> {
        self.configure_factory(document.kind());
        self.factory.send(document).await
    }

    /// Resuelve el builder aunque solo envíe el XML del llamador.
    /// No vuelve a firmar ni cambia `lastXml`.
    pub async fn send_xml(
        &mut self,
        kind: DocumentKind,
        name: &str,
        xml: &[u8],
    ) -> Result<SendResult, CpeForgeError> {
        self.configure_factory(kind);
        self.factory.send_xml(name, xml).await
    }

    /// Obtiene tipo y nombre del XML con lectores de `ws` y lo envía tal cual.
    /// El XML se analiza una sola vez; los errores son los de leer primero el
    /// tipo y después el nombre.
    pub async fn send_xml_file(&mut self, xml: &str) -> Result<SendResult, CpeForgeError> {
        let (class, name) = resolve_type_and_filename(xml)?;
        let kind = DocumentKind::from_class_name(class)?;
        let name = name?;
        self.send_xml(kind, &name, xml.as_bytes()).await
    }

    /// Consulta el ticket por SOAP `getStatus`.
    pub async fn get_status(&self, ticket: &str) -> Result<StatusResult, CpeForgeError> {
        Ok(ExtService::new(self.client()).get_status(ticket).await?)
    }
}
