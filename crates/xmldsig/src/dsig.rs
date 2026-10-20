//! Firma XML (XMLDSig).
//!
//! El nodo `Signature` vive en un documento propio (la plantilla) hasta que
//! se inserta en el documento firmado, o en el documento verificado tras
//! [`XmlSecurityDsig::locate_signature`]. [`XmlSecurityDsig`] conserva la
//! plantilla y recibe el documento de trabajo en cada operación.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use openssl::hash::{Hasher, MessageDigest, hash};

use crate::c14n::{self, Method};
use crate::dom::{Document, NodeId, NodeKind};
use crate::error::Error;
use crate::key::{HMAC_SHA1, RSA_SHA1, SignatureCheck, XmlSecurityKey, lenient_base64_decode};
use crate::xpath::{DOUBLE_QUOTE, EXTENDED_ALPHANUMERIC, filter_attr_name, filter_attr_value};

/// `XMLSecurityDSig::XMLDSIGNS`.
pub const XMLDSIGNS: &str = "http://www.w3.org/2000/09/xmldsig#";
/// `XMLSecurityDSig::SHA1`.
pub const SHA1: &str = "http://www.w3.org/2000/09/xmldsig#sha1";
/// `XMLSecurityDSig::SHA256`.
pub const SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
/// `XMLSecurityDSig::SHA384`.
pub const SHA384: &str = "http://www.w3.org/2001/04/xmldsig-more#sha384";
/// `XMLSecurityDSig::SHA512`.
pub const SHA512: &str = "http://www.w3.org/2001/04/xmlenc#sha512";
/// `XMLSecurityDSig::RIPEMD160`.
pub const RIPEMD160: &str = "http://www.w3.org/2001/04/xmlenc#ripemd160";
pub use crate::c14n::{C14N, C14N_COMMENTS, EXC_C14N, EXC_C14N_COMMENTS};
/// Transformación de firma envuelta.
pub const ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
/// Transformación XPath 1.0 (sin implementación en Rust).
pub const XPATH: &str = "http://www.w3.org/TR/1999/REC-xpath-19991116";

/// `XMLSecurityDSig::template`.
pub const TEMPLATE: &str = "<ds:Signature xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\">\n  <ds:SignedInfo>\n    <ds:SignatureMethod />\n  </ds:SignedInfo>\n</ds:Signature>";
/// `XMLSecurityDSig::BASE_TEMPLATE`.
pub const BASE_TEMPLATE: &str = "<Signature Id=\"CPEForgeSign\" xmlns=\"http://www.w3.org/2000/09/xmldsig#\"><SignedInfo><SignatureMethod /></SignedInfo></Signature>";

/// Transformación de una referencia al firmar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transform {
    /// Algoritmo por URI (`Transform/@Algorithm`).
    Algorithm(String),
    /// XPath con su consulta y namespaces (prefijo, URI).
    XPath {
        /// Expresión XPath.
        query: String,
        /// Namespaces declarados en `ds:XPath`.
        namespaces: Vec<(String, String)>,
    },
}

/// Opciones de `addReference` (`prefix`, `prefix_ns`, `id_name`, `overwrite`, `force_uri`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceOptions {
    /// Prefijo del atributo de identificador.
    pub prefix: Option<String>,
    /// Namespace del atributo de identificador.
    pub prefix_ns: Option<String>,
    /// Nombre del atributo de identificador (por defecto `Id`).
    pub id_name: Option<String>,
    /// Sobrescribir el identificador existente (por defecto `true`).
    pub overwrite: Option<bool>,
    /// `URI=""` al referenciar el documento completo.
    pub force_uri: bool,
}

/// Contenido de `addObject`.
#[derive(Debug, Clone, Copy)]
pub enum ObjectData<'a> {
    /// Texto.
    Text(&'a str),
    /// Elemento importado de otro documento.
    Element(&'a Document, NodeId),
}

/// Opciones de `add509Cert`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CertOptions {
    /// Agrega `X509IssuerSerial`.
    pub issuer_serial: bool,
    /// Agrega `X509SubjectName`.
    pub subject_name: bool,
}

/// Dónde está el nodo `Signature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SigLocation {
    Template(NodeId),
    Document(NodeId),
    None,
}

/// Máximo de `Reference` por firma. Cada una canonicaliza y calcula el
/// digest de su nodo: sin tope, un documento con miles de referencias al
/// mismo nodo multiplica el costo antes de evaluar la confianza. Un
/// comprobante SUNAT usa una sola.
pub const MAX_REFERENCES: usize = 32;

/// Nodo validado por `validateReference` (`getValidatedNodes()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNode {
    /// Identificador del URI (`#id`) o `None` para el documento.
    pub id: Option<String>,
    /// Nodo validado en el documento.
    pub node: NodeId,
}

/// Firma XML (`XMLSecurityDSig`).
#[derive(Debug)]
pub struct XmlSecurityDsig {
    template: Document,
    sig: SigLocation,
    prefix: String,
    canonical_method: Option<String>,
    signed_info: Option<String>,
    validated_nodes: Option<Vec<ValidatedNode>>,
    /// Atributos adicionales de identificador (`idKeys`).
    pub id_keys: Vec<String>,
    /// Prefijos de namespace para `idKeys` (`idNS`).
    pub id_ns: Vec<(String, String)>,
    /// Rechaza con [`Error::AmbiguousReferenceId`] una referencia `#id` que
    /// coincide con más de un elemento (por defecto `false`: se usa
    /// el primero en orden de documento). Activarlo en verificación cierra
    /// el vector de duplicación de identificadores de XML Signature Wrapping.
    pub reject_duplicate_ids: bool,
    /// Rechaza con [`Error::WeakAlgorithm`] un `DigestMethod` SHA-1 al
    /// validar referencias y una firma RSA-SHA1 o HMAC-SHA1 al verificar.
    /// `false` por defecto: el perfil SUNAT ([`crate::sunat::SignedXml`])
    /// exige RSA-SHA1 y SHA-1. Activarlo para firmas fuera de ese perfil.
    pub reject_sha1: bool,
}

impl Default for XmlSecurityDsig {
    fn default() -> Self {
        Self::new(Some("ds"))
    }
}

impl XmlSecurityDsig {
    /// `new XMLSecurityDSig($prefijo)`; `None` o `""` usan el namespace por defecto.
    #[must_use]
    pub fn new(prefix: Option<&str>) -> Self {
        let prefix = prefix.filter(|p| !p.is_empty() && *p != "0");
        let mut template = BASE_TEMPLATE.to_owned();
        if let Some(p) = prefix {
            template = template
                .replace("<S", &format!("<{p}:S"))
                .replace("</S", &format!("</{p}:S"))
                .replace("xmlns=", &format!("xmlns:{p}="));
        }
        // La plantilla es fija y bien formada; un prefijo inválido produce un
        // documento vacío.
        let (template, sig) = match Document::parse(&template) {
            Ok(doc) => {
                let root = doc.document_element();
                (doc, root.map_or(SigLocation::None, SigLocation::Template))
            }
            Err(_) => (Document::new(), SigLocation::None),
        };
        Self {
            template,
            sig,
            prefix: prefix.map(|p| format!("{p}:")).unwrap_or_default(),
            canonical_method: None,
            signed_info: None,
            validated_nodes: None,
            id_keys: Vec::new(),
            id_ns: Vec::new(),
            reject_duplicate_ids: false,
            reject_sha1: false,
        }
    }

    /// `generateGUID($prefijo)`: identificador aleatorio `pfxXXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX`.
    #[must_use]
    pub fn generate_guid(prefix: &str) -> String {
        let mut bytes = [0u8; 16];
        // Si el generador falla se usa la hora; el identificador no es secreto.
        if openssl::rand::rand_bytes(&mut bytes).is_err() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            bytes = nanos.to_le_bytes();
        }
        let hex = crate::key::hex_lower(&bytes);
        format!(
            "{prefix}{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }

    /// Documento y nodo `Signature` actuales.
    fn sig<'a>(&'a self, doc: &'a Document) -> Option<(&'a Document, NodeId)> {
        match self.sig {
            SigLocation::Template(n) => Some((&self.template, n)),
            SigLocation::Document(n) => Some((doc, n)),
            SigLocation::None => None,
        }
    }

    fn sig_mut<'a>(&'a mut self, doc: &'a mut Document) -> Option<(&'a mut Document, NodeId)> {
        match self.sig {
            SigLocation::Template(n) => Some((&mut self.template, n)),
            SigLocation::Document(n) => Some((doc, n)),
            SigLocation::None => None,
        }
    }

    /// Nodo `Signature` en el documento de trabajo, si está en él (`sigNode`).
    #[must_use]
    pub fn signature_node(&self) -> Option<NodeId> {
        match self.sig {
            SigLocation::Document(n) => Some(n),
            _ => None,
        }
    }

    /// XML del nodo `Signature` (de la plantilla o del documento).
    #[must_use]
    pub fn signature_xml(&self, doc: &Document) -> Option<String> {
        self.sig(doc).map(|(d, n)| d.node_to_xml(n))
    }

    /// `locateSignature($nodo, $pos)`: `pos`-ésimo `ds:Signature` descendiente.
    pub fn locate_signature(&mut self, doc: &Document, node: NodeId, pos: usize) -> Option<NodeId> {
        let found = doc
            .descendants_ns(node, Some(XMLDSIGNS), "Signature")
            .get(pos)
            .copied();
        self.sig = found.map_or(SigLocation::None, SigLocation::Document);
        found
    }

    /// `createNewSignNode($nombre, $valor)`: elemento `ds:` (con el prefijo
    /// de la firma) en el documento de la firma, sin insertar. Usar con
    /// [`Self::signature_document_mut`] y, por ejemplo, [`Self::append_to_key_info`].
    pub fn create_new_sign_node(
        &mut self,
        doc: &mut Document,
        name: &str,
        value: Option<&str>,
    ) -> Option<NodeId> {
        let qname = format!("{}{name}", self.prefix);
        let (d, _) = self.sig_mut(doc)?;
        Some(d.create_element_ns(Some(XMLDSIGNS), &qname, value))
    }

    /// `appendCert()`: vacío en la referencia; se conserva por compatibilidad.
    pub fn append_cert(&self) {}

    /// `setCanonicalMethod($uri)`.
    ///
    /// Cada llamada inserta un `CanonicalizationMethod` nuevo al
    /// inicio de `SignedInfo` (la búsqueda del existente usa una consulta
    /// mal escrita que nunca coincide).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidCanonicalMethod`] si no es uno de los cuatro métodos C14N.
    pub fn set_canonical_method(&mut self, doc: &mut Document, method: &str) -> Result<(), Error> {
        if Method::from_uri(method).is_none() {
            return Err(Error::InvalidCanonicalMethod(method.to_owned()));
        }
        self.canonical_method = Some(method.to_owned());
        let prefix = self.prefix.clone();
        if let Some((d, sig)) = self.sig_mut(doc) {
            if let Some(sinfo) = d.child_ns(sig, XMLDSIGNS, "SignedInfo") {
                let canon = d.create_element_ns(
                    Some(XMLDSIGNS),
                    &format!("{prefix}CanonicalizationMethod"),
                    None,
                );
                let first = d.children(sinfo).first().copied();
                d.insert_before(sinfo, canon, first);
                d.set_attribute(canon, "Algorithm", method);
            }
        }
        Ok(())
    }

    /// `canonicalizeSignedInfo()`: canonicaliza `SignedInfo` con su
    /// `CanonicalizationMethod` (C14N si falta o es desconocido) y lo guarda
    /// para [`Self::verify`].
    ///
    /// # Errors
    ///
    pub fn canonicalize_signed_info(&mut self, doc: &Document) -> Option<String> {
        let (d, sig) = self.sig(doc)?;
        let sinfo = d.child_ns(sig, XMLDSIGNS, "SignedInfo")?;
        let method = d
            .child_ns(sinfo, XMLDSIGNS, "CanonicalizationMethod")
            .map(|c| d.attribute(c, "Algorithm").unwrap_or("").to_owned());
        let data = canonicalize_data(d, sinfo, method.as_deref(), None);
        self.signed_info = Some(data.clone());
        Some(data)
    }

    /// `calculateDigest($algoritmo, $datos, $codificar)`.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedDigest`] si el algoritmo no es SHA-1/256/384/512 o RIPEMD-160.
    pub fn calculate_digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, Error> {
        let md = message_digest(algorithm)
            .ok_or_else(|| Error::UnsupportedDigest(algorithm.to_owned()))?;
        hash(md, data)
            .map(|d| d.to_vec())
            .map_err(|e| crate::error::openssl_error("", &e))
    }

    /// `calculateDigest` codificado en Base64.
    ///
    /// # Errors
    ///
    /// Como [`Self::calculate_digest`].
    pub fn calculate_digest_base64(algorithm: &str, data: &[u8]) -> Result<String, Error> {
        Self::calculate_digest(algorithm, data).map(|d| STANDARD.encode(d))
    }

    /// `validateDigest($referencia, $datos)`.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedDigest`] si el `DigestMethod` no está soportado.
    pub fn validate_digest(doc: &Document, reference: NodeId, data: &[u8]) -> Result<bool, Error> {
        let algorithm = doc
            .child_ns(reference, XMLDSIGNS, "DigestMethod")
            .and_then(|m| doc.attribute(m, "Algorithm"))
            .unwrap_or("");
        let computed = Self::calculate_digest(algorithm, data)?;
        let expected = doc
            .child_ns(reference, XMLDSIGNS, "DigestValue")
            .map(|v| doc.text_content(v))
            .unwrap_or_default();
        Ok(computed == lenient_base64_decode(&expected))
    }

    /// `processTransforms($referencia, $nodo, $incluirComentarios)`:
    /// canonicaliza `data` según las transformaciones de la referencia.
    /// La firma envuelta y las transformaciones desconocidas se ignoran.
    ///
    /// `reference_doc` contiene la referencia y `data_doc` los datos.
    ///
    /// # Errors
    ///
    /// [`Error::XPath`] si la consulta de la transformación XPath es inválida.
    pub fn process_transforms(
        reference_doc: &Document,
        reference: NodeId,
        data_doc: &Document,
        data: Option<NodeId>,
        include_comments: bool,
    ) -> Result<Option<String>, Error> {
        let plan = TransformPlan::read(reference_doc, reference, include_comments);
        let Some(node) = data else { return Ok(None) };
        plan.canonicalize(data_doc, node).map(Some)
    }

    /// Digest de los datos transformados, calculado por bloques sin
    /// construir la canonicalización completa en memoria. Equivale a
    /// [`Self::calculate_digest`] sobre [`Self::process_transforms`] (vacío
    /// sin datos), con los errores en el mismo orden.
    fn transformed_digest(
        reference_doc: &Document,
        reference: NodeId,
        data_doc: &Document,
        data: Option<NodeId>,
        include_comments: bool,
        algorithm: &str,
    ) -> Result<Vec<u8>, Error> {
        let Some(md) = message_digest(algorithm) else {
            // La transformación falla antes que el algoritmo de digest.
            Self::process_transforms(reference_doc, reference, data_doc, data, include_comments)?;
            return Err(Error::UnsupportedDigest(algorithm.to_owned()));
        };
        let failed = |e: &openssl::error::ErrorStack| crate::error::openssl_error("", e);
        let mut hasher = Hasher::new(md).map_err(|e| failed(&e))?;
        if let Some(node) = data {
            let plan = TransformPlan::read(reference_doc, reference, include_comments);
            let mut error = None;
            plan.canonicalize_to(data_doc, node, &mut |chunk: &str| {
                if error.is_none() {
                    error = hasher.update(chunk.as_bytes()).err();
                }
            })?;
            if let Some(e) = error {
                return Err(failed(&e));
            }
        }
        hasher.finish().map(|d| d.to_vec()).map_err(|e| failed(&e))
    }
}

/// Transformaciones de una referencia: método C14N, `PrefixList` y XPath.
struct TransformPlan {
    method: String,
    prefix_list: Option<Vec<String>>,
    /// Consulta XPath y prefijos registrados.
    xpath: Option<(String, Vec<(String, String)>)>,
}

impl TransformPlan {
    fn read(reference_doc: &Document, reference: NodeId, include_comments: bool) -> Self {
        let mut method = C14N.to_owned();
        let mut prefix_list: Option<Vec<String>> = None;
        // Consulta XPath y prefijos registrados (los del eje namespace del
        // elemento ds:XPath, sin `xml`; el por defecto queda como `xmlns`).
        let mut xpath: Option<(String, Vec<(String, String)>)> = None;
        for transforms in reference_doc.children_ns(reference, XMLDSIGNS, "Transforms") {
            for transform in reference_doc.children_ns(transforms, XMLDSIGNS, "Transform") {
                let algorithm = reference_doc
                    .attribute(transform, "Algorithm")
                    .unwrap_or("");
                match algorithm {
                    EXC_C14N | EXC_C14N_COMMENTS => {
                        method = if include_comments {
                            algorithm.to_owned()
                        } else {
                            EXC_C14N.to_owned()
                        };
                        let inclusive =
                            reference_doc.children(transform).iter().copied().find(|n| {
                                reference_doc.local_name(*n) == Some("InclusiveNamespaces")
                            });
                        if let Some(node) = inclusive {
                            let list: Vec<String> = reference_doc
                                .attribute(node, "PrefixList")
                                .unwrap_or("")
                                .split(' ')
                                .map(|p| {
                                    p.trim_matches(|c| {
                                        matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B')
                                    })
                                    .to_owned()
                                })
                                .filter(|p| !p.is_empty() && p != "0")
                                .collect();
                            if !list.is_empty() {
                                prefix_list = Some(list);
                            }
                        }
                    }
                    C14N | C14N_COMMENTS => {
                        method = if include_comments {
                            algorithm.to_owned()
                        } else {
                            C14N.to_owned()
                        };
                    }
                    XPATH => {
                        let node = reference_doc
                            .children(transform)
                            .iter()
                            .copied()
                            .find(|n| reference_doc.local_name(*n) == Some("XPath"));
                        if let Some(node) = node {
                            let namespaces = reference_doc
                                .namespace_nodes(node)
                                .into_iter()
                                .filter(|d| d.prefix.as_deref() != Some("xml"))
                                .map(|d| (d.prefix.unwrap_or_else(|| "xmlns".to_owned()), d.uri))
                                .collect();
                            xpath = Some((reference_doc.text_content(node), namespaces));
                        }
                    }
                    _ => {}
                }
            }
        }
        Self {
            method,
            prefix_list,
            xpath,
        }
    }

    fn c14n_method(&self) -> Method {
        c14n_method(Some(&self.method), self.prefix_list.clone())
    }

    fn canonicalize(&self, doc: &Document, node: NodeId) -> Result<String, Error> {
        let m = self.c14n_method();
        match &self.xpath {
            Some((query, namespaces)) => c14n::canonicalize_xpath(doc, node, &m, query, namespaces),
            None => Ok(c14n::canonicalize(doc, c14n_target(doc, node, &m), &m)),
        }
    }

    fn canonicalize_to(
        &self,
        doc: &Document,
        node: NodeId,
        sink: &mut dyn FnMut(&str),
    ) -> Result<(), Error> {
        let m = self.c14n_method();
        match &self.xpath {
            Some((query, namespaces)) => {
                c14n::canonicalize_xpath_to(doc, node, &m, query, namespaces, sink)
            }
            None => {
                c14n::canonicalize_to(doc, c14n_target(doc, node, &m), &m, sink);
                Ok(())
            }
        }
    }
}

impl XmlSecurityDsig {
    /// `getRefNodeID($referencia)`: fragmento del URI de una referencia local.
    #[must_use]
    pub fn ref_node_id(doc: &Document, reference: NodeId) -> Option<String> {
        let uri = doc.attribute(reference, "URI").filter(|u| is_set(u))?;
        let parsed = parse_uri(uri);
        if parsed.path_empty {
            parsed.fragment.filter(|f| is_set(f))
        } else {
            None
        }
    }

    /// `getRefIDs()`.
    ///
    /// # Errors
    ///
    /// [`Error::ReferenceNodesNotFound`] si `SignedInfo` no tiene referencias.
    pub fn ref_ids(&self, doc: &Document) -> Result<Vec<Option<String>>, Error> {
        let (d, sig) = self.sig(doc).ok_or(Error::ReferenceNodesNotFound)?;
        let refs = references(d, sig);
        if refs.is_empty() {
            return Err(Error::ReferenceNodesNotFound);
        }
        Ok(refs.into_iter().map(|r| Self::ref_node_id(d, r)).collect())
    }

    /// `validateReference()`: **retira el nodo `Signature` del documento**
    /// (salvo que sea la raíz) y verifica el digest de cada referencia.
    ///
    /// # Errors
    ///
    /// [`Error::ReferenceNodesNotFound`], [`Error::ReferenceValidationFailed`],
    /// [`Error::UnsupportedDigest`] o [`Error::XPath`].
    pub fn validate_reference(&mut self, doc: &mut Document) -> Result<bool, Error> {
        let SigLocation::Document(sig) = self.sig else {
            return self.validate_template_reference();
        };
        if doc.document_element() != Some(sig) && doc.parent(sig).is_some() {
            doc.remove_child(sig);
        }
        let refs = references(doc, sig);
        if refs.is_empty() {
            return Err(Error::ReferenceNodesNotFound);
        }
        if refs.len() > MAX_REFERENCES {
            return Err(Error::TooManyReferences(refs.len()));
        }
        self.reject_weak_digests(doc, &refs)?;
        self.validated_nodes = Some(Vec::new());
        for reference in refs {
            match self.process_ref_node(doc, doc, reference) {
                Ok(true) => {}
                Ok(false) => {
                    self.validated_nodes = None;
                    return Err(Error::ReferenceValidationFailed);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }

    /// Con [`Self::reject_sha1`], ninguna referencia puede usar SHA-1.
    fn reject_weak_digests(&self, doc: &Document, refs: &[NodeId]) -> Result<(), Error> {
        if !self.reject_sha1 {
            return Ok(());
        }
        for &reference in refs {
            let algorithm = doc
                .child_ns(reference, XMLDSIGNS, "DigestMethod")
                .and_then(|m| doc.attribute(m, "Algorithm"))
                .unwrap_or("");
            if algorithm == SHA1 {
                return Err(Error::WeakAlgorithm(SHA1.to_owned()));
            }
        }
        Ok(())
    }

    fn validate_template_reference(&mut self) -> Result<bool, Error> {
        let SigLocation::Template(sig) = self.sig else {
            return Err(Error::ReferenceNodesNotFound);
        };
        let template = std::mem::take(&mut self.template);
        let refs = references(&template, sig);
        let result = if refs.is_empty() {
            Err(Error::ReferenceNodesNotFound)
        } else if refs.len() > MAX_REFERENCES {
            Err(Error::TooManyReferences(refs.len()))
        } else if let Err(e) = self.reject_weak_digests(&template, &refs) {
            Err(e)
        } else {
            self.validated_nodes = Some(Vec::new());
            let mut result = Ok(true);
            for reference in refs {
                // En la plantilla los URI apuntan al documento de la plantilla.
                match self.process_ref_node(&template, &template, reference) {
                    Ok(true) => {}
                    Ok(false) => {
                        self.validated_nodes = None;
                        result = Err(Error::ReferenceValidationFailed);
                        break;
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            result
        };
        self.template = template;
        result
    }

    /// `processRefNode($referencia)`: valida el digest de una referencia de
    /// la firma localizada en `doc` y registra el nodo validado.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedDigest`] o [`Error::XPath`].
    pub fn process_reference(&mut self, doc: &Document, reference: NodeId) -> Result<bool, Error> {
        if self.validated_nodes.is_none() {
            self.validated_nodes = Some(Vec::new());
        }
        self.process_ref_node(doc, doc, reference)
    }

    fn process_ref_node(
        &mut self,
        reference_doc: &Document,
        data_doc: &Document,
        reference: NodeId,
    ) -> Result<bool, Error> {
        let mut include_comments = true;
        let mut identifier: Option<String> = None;
        let data = match reference_doc
            .attribute(reference, "URI")
            .filter(|u| is_set(u))
        {
            Some(uri) => {
                let parsed = parse_uri(uri);
                if parsed.path_empty {
                    match parsed.fragment.filter(|f| is_set(f)) {
                        Some(id) => {
                            include_comments = false;
                            let found = self.find_by_id(data_doc, &id)?;
                            identifier = Some(id);
                            found
                        }
                        None => Some(data_doc.document_node()),
                    }
                } else {
                    None
                }
            }
            None => {
                include_comments = false;
                Some(data_doc.document_node())
            }
        };
        // La referencia calculaba el digest de una cadena vacía cuando el
        // destino no existe o la URI es externa: una firma con ese digest
        // «validaba» sin cubrir ningún contenido. Aquí no valida nunca (mismo
        // `Reference validation failed` que la referencia para el caso común).
        if data.is_none() {
            return Ok(false);
        }
        let algorithm = reference_doc
            .child_ns(reference, XMLDSIGNS, "DigestMethod")
            .and_then(|m| reference_doc.attribute(m, "Algorithm"))
            .unwrap_or("");
        let computed = Self::transformed_digest(
            reference_doc,
            reference,
            data_doc,
            data,
            include_comments,
            algorithm,
        )?;
        let expected = reference_doc
            .child_ns(reference, XMLDSIGNS, "DigestValue")
            .map(|v| reference_doc.text_content(v))
            .unwrap_or_default();
        if computed != lenient_base64_decode(&expected) {
            return Ok(false);
        }
        if let (Some(node), Some(list)) = (data, self.validated_nodes.as_mut()) {
            match identifier {
                Some(id) => {
                    if let Some(existing) = list
                        .iter_mut()
                        .find(|v| v.id.as_deref() == Some(id.as_str()))
                    {
                        existing.node = node;
                    } else {
                        list.push(ValidatedNode { id: Some(id), node });
                    }
                }
                None => list.push(ValidatedNode { id: None, node }),
            }
        }
        Ok(true)
    }

    /// `//*[@Id="x" or @idKey="x"]`: primer elemento en orden de documento.
    fn find_by_id(&self, doc: &Document, id: &str) -> Result<Option<NodeId>, Error> {
        let value = filter_attr_value(id, DOUBLE_QUOTE);
        let keys: Vec<String> = self
            .id_keys
            .iter()
            .map(|k| filter_attr_name(k, EXTENDED_ALPHANUMERIC))
            .collect();
        let matches = |n: NodeId| {
            if doc.attribute_ns(n, None, "Id") == Some(value.as_str()) {
                return true;
            }
            keys.iter().any(|key| match key.split_once(':') {
                Some((prefix, local)) => {
                    let ns = self
                        .id_ns
                        .iter()
                        .find(|(p, _)| p == prefix)
                        .map(|(_, u)| u.as_str());
                    ns.is_some_and(|ns| {
                        doc.attribute_ns(n, Some(ns), local) == Some(value.as_str())
                    })
                }
                None => doc.attribute_ns(n, None, key) == Some(value.as_str()),
            })
        };
        // Recorrido en preorden con parada anticipada; no materializa la
        // lista de descendientes.
        let mut found = None;
        let mut stack: Vec<NodeId> = vec![doc.document_node()];
        while let Some(n) = stack.pop() {
            if doc.element(n).is_some() && matches(n) {
                if found.is_none() {
                    found = Some(n);
                    if !self.reject_duplicate_ids {
                        return Ok(found);
                    }
                } else {
                    return Err(Error::AmbiguousReferenceId(id.to_owned()));
                }
            }
            stack.extend(doc.children(n).iter().rev().copied());
        }
        Ok(found)
    }

    /// `addReference($nodo, $digest, $transformaciones, $opciones)`.
    ///
    /// `node` pertenece a `doc`; la referencia se agrega al `SignedInfo`.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedDigest`] o [`Error::XPath`].
    pub fn add_reference(
        &mut self,
        doc: &mut Document,
        node: NodeId,
        algorithm: &str,
        transforms: Option<&[Transform]>,
        options: Option<&ReferenceOptions>,
    ) -> Result<(), Error> {
        self.add_reference_list(doc, &[node], algorithm, transforms, options)
    }

    /// `addReferenceList($nodos, …)`.
    ///
    /// # Errors
    ///
    /// Como [`Self::add_reference`].
    pub fn add_reference_list(
        &mut self,
        doc: &mut Document,
        nodes: &[NodeId],
        algorithm: &str,
        transforms: Option<&[Transform]>,
        options: Option<&ReferenceOptions>,
    ) -> Result<(), Error> {
        for node in nodes {
            self.add_ref_internal(doc, *node, algorithm, transforms, options)?;
        }
        Ok(())
    }

    fn add_ref_internal(
        &mut self,
        doc: &mut Document,
        node: NodeId,
        algorithm: &str,
        transforms: Option<&[Transform]>,
        options: Option<&ReferenceOptions>,
    ) -> Result<(), Error> {
        let in_template = matches!(self.sig, SigLocation::Template(_));
        let sig = match self.sig {
            SigLocation::Template(n) | SigLocation::Document(n) => n,
            SigLocation::None => return Ok(()),
        };
        let sinfo = {
            let d = if in_template { &self.template } else { &*doc };
            match d.child_ns(sig, XMLDSIGNS, "SignedInfo") {
                Some(s) => s,
                None => return Ok(()),
            }
        };
        let options = options.cloned().unwrap_or_default();
        let id_prefix = options.prefix.filter(|p| is_set(p));
        let prefix_ns = options.prefix_ns.filter(|p| is_set(p));
        let id_name = options
            .id_name
            .filter(|p| is_set(p))
            .unwrap_or_else(|| "Id".to_owned());
        let overwrite = options.overwrite.unwrap_or(true);
        let attname = match &id_prefix {
            Some(p) => format!("{p}:{id_name}"),
            None => id_name.clone(),
        };

        // URI e identificador en el documento de datos.
        let uri = if matches!(doc.kind(node), NodeKind::Document) {
            options.force_uri.then(String::new)
        } else {
            let mut uri = None;
            if !overwrite {
                uri = match &prefix_ns {
                    Some(ns) => doc.attribute_ns(node, Some(ns), &id_name),
                    None => doc.attribute(node, &id_name),
                }
                .filter(|u| is_set(u))
                .map(str::to_owned);
            }
            let uri = match uri {
                Some(u) => u,
                None => {
                    let guid = Self::generate_guid("pfx");
                    doc.set_attribute_ns(node, prefix_ns.as_deref(), &attname, &guid);
                    guid
                }
            };
            Some(format!("#{uri}"))
        };

        let prefix = self.prefix.clone();
        let build = |d: &mut Document| -> NodeId {
            let create = |d: &mut Document, name: &str, value: Option<&str>| {
                d.create_element_ns(Some(XMLDSIGNS), &format!("{prefix}{name}"), value)
            };
            let reference = create(d, "Reference", None);
            d.append_child(sinfo, reference);
            if let Some(uri) = &uri {
                d.set_attribute(reference, "URI", uri);
            }
            let trans_nodes = create(d, "Transforms", None);
            d.append_child(reference, trans_nodes);
            match transforms {
                Some(list) => {
                    for t in list {
                        let tn = create(d, "Transform", None);
                        d.append_child(trans_nodes, tn);
                        match t {
                            Transform::XPath { query, namespaces }
                                if !query.is_empty() && query != "0" =>
                            {
                                d.set_attribute(tn, "Algorithm", XPATH);
                                let xp = create(d, "XPath", Some(query));
                                d.append_child(tn, xp);
                                for (p, ns) in namespaces {
                                    d.set_attribute_ns(
                                        xp,
                                        Some("http://www.w3.org/2000/xmlns/"),
                                        &format!("xmlns:{p}"),
                                        ns,
                                    );
                                }
                            }
                            Transform::XPath { .. } => d.set_attribute(tn, "Algorithm", "Array"),
                            Transform::Algorithm(a) => d.set_attribute(tn, "Algorithm", a),
                        }
                    }
                }
                None => {
                    if let Some(m) = &self.canonical_method {
                        let tn = create(d, "Transform", None);
                        d.append_child(trans_nodes, tn);
                        d.set_attribute(tn, "Algorithm", m);
                    }
                }
            }
            reference
        };
        let reference = if in_template {
            build(&mut self.template)
        } else {
            build(doc)
        };

        let digest = {
            let reference_doc = if in_template { &self.template } else { &*doc };
            STANDARD.encode(Self::transformed_digest(
                reference_doc,
                reference,
                doc,
                Some(node),
                true,
                algorithm,
            )?)
        };
        let target = if in_template { &mut self.template } else { doc };
        let method = target.create_element_ns(
            Some(XMLDSIGNS),
            &format!("{}DigestMethod", self.prefix),
            None,
        );
        target.append_child(reference, method);
        target.set_attribute(method, "Algorithm", algorithm);
        let value = target.create_element_ns(
            Some(XMLDSIGNS),
            &format!("{}DigestValue", self.prefix),
            Some(&digest),
        );
        target.append_child(reference, value);
        Ok(())
    }

    /// `addObject($datos, $mime, $codificacion)`: agrega `Object` a la firma.
    pub fn add_object(
        &mut self,
        doc: &mut Document,
        data: ObjectData<'_>,
        mime_type: Option<&str>,
        encoding: Option<&str>,
    ) -> Option<NodeId> {
        let prefix = self.prefix.clone();
        let (d, sig) = self.sig_mut(doc)?;
        let object = d.create_element_ns(Some(XMLDSIGNS), &format!("{prefix}Object"), None);
        d.append_child(sig, object);
        if let Some(m) = mime_type.filter(|m| is_set(m)) {
            d.set_attribute(object, "MimeType", m);
        }
        if let Some(e) = encoding.filter(|e| is_set(e)) {
            d.set_attribute(object, "Encoding", e);
        }
        let content = match data {
            ObjectData::Text(t) => d.create_text(t),
            ObjectData::Element(source, node) => d.import_node(source, node),
        };
        d.append_child(object, content);
        Some(object)
    }

    /// `locateKey()`: clave pública vacía con el algoritmo de `SignatureMethod`.
    #[must_use]
    pub fn locate_key(&self, doc: &Document) -> Option<XmlSecurityKey> {
        let (d, sig) = self.sig(doc)?;
        let algorithm = d
            .child_ns(sig, XMLDSIGNS, "SignedInfo")
            .and_then(|s| d.child_ns(s, XMLDSIGNS, "SignatureMethod"))
            .and_then(|m| d.attribute(m, "Algorithm"))
            .unwrap_or("");
        if !is_set(algorithm) {
            return None;
        }
        XmlSecurityKey::new(algorithm, Some("public")).ok()
    }

    /// `verify($clave)`: verifica `SignatureValue` sobre el `SignedInfo`
    /// canonicalizado por [`Self::canonicalize_signed_info`].
    ///
    /// Solo comprueba la validez matemática de la firma; no valida
    /// referencias ni la confianza en el certificado.
    ///
    /// # Errors
    ///
    /// [`Error::SignatureValueNotFound`] si falta `SignatureValue` o está vacío.
    pub fn verify(&self, doc: &Document, key: &XmlSecurityKey) -> Result<SignatureCheck, Error> {
        let (d, sig) = self.sig(doc).ok_or(Error::SignatureValueNotFound)?;
        let value = d
            .child_ns(sig, XMLDSIGNS, "SignatureValue")
            .map(|v| d.text_content(v))
            .unwrap_or_default();
        if !is_set(&value) {
            return Err(Error::SignatureValueNotFound);
        }
        if self.reject_sha1 {
            let declared = d
                .child_ns(sig, XMLDSIGNS, "SignedInfo")
                .and_then(|i| d.child_ns(i, XMLDSIGNS, "SignatureMethod"))
                .and_then(|m| d.attribute(m, "Algorithm"))
                .unwrap_or("");
            for algorithm in [declared, key.algorithm()] {
                if matches!(algorithm, RSA_SHA1 | HMAC_SHA1) {
                    return Err(Error::WeakAlgorithm(algorithm.to_owned()));
                }
            }
        }
        let signed_info = self.signed_info.as_deref().unwrap_or("");
        Ok(key.verify_signature(signed_info.as_bytes(), &lenient_base64_decode(&value)))
    }

    /// `signData($clave, $datos)`.
    ///
    /// # Errors
    ///
    /// Como [`XmlSecurityKey::sign_data`].
    pub fn sign_data(key: &XmlSecurityKey, data: &[u8]) -> Result<Vec<u8>, Error> {
        key.sign_data(data)
    }

    /// `sign($clave, $nodoPadre)`: si se indica `append_to`, inserta primero
    /// la firma como último hijo de ese nodo para canonicalizar en su contexto.
    ///
    /// # Errors
    ///
    /// Los de [`XmlSecurityKey::sign_data`].
    pub fn sign(
        &mut self,
        doc: &mut Document,
        key: &XmlSecurityKey,
        append_to: Option<NodeId>,
    ) -> Result<(), Error> {
        if let Some(parent) = append_to {
            self.append_signature(doc, parent, false);
            match doc.children(parent).last().copied() {
                Some(last) => self.sig = SigLocation::Document(last),
                None => self.sig = SigLocation::None,
            }
        }
        let method = self.canonical_method.clone();
        let prefix = self.prefix.clone();
        let Some((d, sig)) = self.sig_mut(doc) else {
            return Ok(());
        };
        let Some(sinfo) = d.child_ns(sig, XMLDSIGNS, "SignedInfo") else {
            return Ok(());
        };
        if let Some(sm) = d.child_ns(sinfo, XMLDSIGNS, "SignatureMethod") {
            d.set_attribute(sm, "Algorithm", key.algorithm());
        }
        let data = canonicalize_data(d, sinfo, method.as_deref(), None);
        let value = STANDARD.encode(key.sign_data(data.as_bytes())?);
        let node = d.create_element_ns(
            Some(XMLDSIGNS),
            &format!("{prefix}SignatureValue"),
            Some(&value),
        );
        match d.next_sibling(sinfo) {
            Some(next) => {
                let parent = d.parent(next).unwrap_or(sig);
                d.insert_before(parent, node, Some(next));
            }
            None => {
                d.append_child(sig, node);
            }
        }
        Ok(())
    }

    /// `insertSignature($nodo, $antesDe)`: inserta una copia de la firma en `doc`.
    pub fn insert_signature(
        &self,
        doc: &mut Document,
        node: NodeId,
        before: Option<NodeId>,
    ) -> Option<NodeId> {
        let copy = match self.sig {
            SigLocation::Template(n) => doc.import_node(&self.template, n),
            SigLocation::Document(n) => doc.duplicate_node(n),
            SigLocation::None => return None,
        };
        Some(doc.insert_before(node, copy, before))
    }

    /// `appendSignature($padre, $alInicio)`.
    pub fn append_signature(
        &self,
        doc: &mut Document,
        parent: NodeId,
        insert_before: bool,
    ) -> Option<NodeId> {
        let before = if insert_before {
            doc.children(parent).first().copied()
        } else {
            None
        };
        self.insert_signature(doc, parent, before)
    }

    /// `get509XCert($cert, $esPem)`: primer certificado en Base64.
    #[must_use]
    pub fn get_509_x_cert(cert: &str, is_pem: bool) -> String {
        Self::static_get_509_x_certs(cert, is_pem)
            .into_iter()
            .next()
            .unwrap_or_default()
    }

    /// `staticGet509XCerts($certs, $esPem)`: cuerpos Base64 de los bloques
    /// `CERTIFICATE`, sin saltos de línea.
    #[must_use]
    pub fn static_get_509_x_certs(certs: &str, is_pem: bool) -> Vec<String> {
        if !is_pem {
            return vec![certs.to_owned()];
        }
        let mut list = Vec::new();
        let mut data = String::new();
        let mut in_data = false;
        for line in certs.split('\n') {
            if !in_data {
                if line.starts_with("-----BEGIN CERTIFICATE") {
                    in_data = true;
                }
            } else if line.starts_with("-----END CERTIFICATE") {
                in_data = false;
                list.push(std::mem::take(&mut data));
            } else {
                data.push_str(trim_ws(line));
            }
        }
        list
    }

    /// `staticAdd509Cert($padre, …)`: agrega `KeyInfo/X509Data` a `parent`.
    pub fn static_add_509_cert(
        doc: &mut Document,
        parent: NodeId,
        cert: &str,
        is_pem: bool,
        options: CertOptions,
    ) {
        let key_info = match doc.child_ns(parent, XMLDSIGNS, "KeyInfo") {
            Some(k) => k,
            None => {
                let pfx = prefix_for(doc, parent);
                let key_info =
                    doc.create_element_ns(Some(XMLDSIGNS), &format!("{pfx}KeyInfo"), None);
                match doc.child_ns(parent, XMLDSIGNS, "Object") {
                    Some(object) => {
                        let p = doc.parent(object).unwrap_or(parent);
                        doc.insert_before(p, key_info, Some(object));
                    }
                    None => {
                        doc.append_child(parent, key_info);
                    }
                }
                key_info
            }
        };
        let pfx = prefix_for(doc, key_info);
        let x509_data = doc.create_element_ns(Some(XMLDSIGNS), &format!("{pfx}X509Data"), None);
        doc.append_child(key_info, x509_data);
        for body in Self::static_get_509_x_certs(cert, is_pem) {
            if options.issuer_serial || options.subject_name {
                if let Some(info) = crate::x509::CertInfo::from_base64_body(&body) {
                    if options.subject_name && !info.subject.is_empty() {
                        let value = info.subject_name_for_key_info();
                        let node = doc.create_element_ns(
                            Some(XMLDSIGNS),
                            &format!("{pfx}X509SubjectName"),
                            Some(&value),
                        );
                        doc.append_child(x509_data, node);
                    }
                    if options.issuer_serial
                        && !info.issuer.is_empty()
                        && is_set(&info.serial_number)
                    {
                        let issuer = info.issuer_for_serial();
                        let serial = doc.create_element_ns(
                            Some(XMLDSIGNS),
                            &format!("{pfx}X509IssuerSerial"),
                            None,
                        );
                        doc.append_child(x509_data, serial);
                        let name = doc.create_element_ns(
                            Some(XMLDSIGNS),
                            &format!("{pfx}X509IssuerName"),
                            Some(&issuer),
                        );
                        doc.append_child(serial, name);
                        let number = doc.create_element_ns(
                            Some(XMLDSIGNS),
                            &format!("{pfx}X509SerialNumber"),
                            Some(&info.serial_number),
                        );
                        doc.append_child(serial, number);
                    }
                }
            }
            let node = doc.create_element_ns(
                Some(XMLDSIGNS),
                &format!("{pfx}X509Certificate"),
                Some(&body),
            );
            doc.append_child(x509_data, node);
        }
    }

    /// `add509Cert($cert, $esPem, $esUrl=false, $opciones)`. La carga por URL
    /// no se implementa: el certificado se pasa como texto.
    pub fn add_509_cert(
        &mut self,
        doc: &mut Document,
        cert: &str,
        is_pem: bool,
        options: CertOptions,
    ) {
        if let Some((d, sig)) = self.sig_mut(doc) {
            Self::static_add_509_cert(d, sig, cert, is_pem, options);
        }
    }

    /// `appendToKeyInfo($nodo)`: agrega a `KeyInfo` un nodo ya creado en el
    /// documento de la firma ([`Self::signature_document_mut`]).
    pub fn append_to_key_info(&mut self, doc: &mut Document, node: NodeId) -> Option<NodeId> {
        let (d, sig) = self.sig_mut(doc)?;
        let key_info = match d.child_ns(sig, XMLDSIGNS, "KeyInfo") {
            Some(k) => k,
            None => {
                let pfx = prefix_for(d, sig);
                let key_info = d.create_element_ns(Some(XMLDSIGNS), &format!("{pfx}KeyInfo"), None);
                match d.child_ns(sig, XMLDSIGNS, "Object") {
                    Some(object) => {
                        d.insert_before(sig, key_info, Some(object));
                    }
                    None => {
                        d.append_child(sig, key_info);
                    }
                }
                key_info
            }
        };
        d.append_child(key_info, node);
        Some(key_info)
    }

    /// Documento que contiene la firma (la plantilla o `doc`), para crear nodos.
    pub fn signature_document_mut<'a>(
        &'a mut self,
        doc: &'a mut Document,
    ) -> Option<&'a mut Document> {
        self.sig_mut(doc).map(|(d, _)| d)
    }

    /// `getValidatedNodes()`.
    #[must_use]
    pub fn validated_nodes(&self) -> Option<&[ValidatedNode]> {
        self.validated_nodes.as_deref()
    }
}

/// Referencias `./SignedInfo/Reference` de una firma.
pub(crate) fn references(doc: &Document, sig: NodeId) -> Vec<NodeId> {
    doc.children_ns(sig, XMLDSIGNS, "SignedInfo")
        .into_iter()
        .flat_map(|s| doc.children_ns(s, XMLDSIGNS, "Reference"))
        .collect()
}

/// Prefijo `ds:` en ámbito (o vacío).
fn prefix_for(doc: &Document, node: NodeId) -> String {
    match doc.lookup_prefix(node, XMLDSIGNS) {
        Some(p) if is_set(&p) => format!("{p}:"),
        _ => String::new(),
    }
}

/// Método C14N por URI (C14N inclusivo si falta o es desconocido).
fn c14n_method(method: Option<&str>, prefix_list: Option<Vec<String>>) -> Method {
    let mut m = method.and_then(Method::from_uri).unwrap_or(Method {
        exclusive: false,
        with_comments: false,
        inclusive_prefixes: Vec::new(),
    });
    if let Some(list) = prefix_list {
        m.inclusive_prefixes = list;
    }
    m
}

/// `canonicalizeData`: canonicaliza `node` (o el documento, ver [`c14n_target`]).
fn canonicalize_data(
    doc: &Document,
    node: NodeId,
    method: Option<&str>,
    prefix_list: Option<Vec<String>>,
) -> String {
    let m = c14n_method(method, prefix_list);
    c14n::canonicalize(doc, c14n_target(doc, node, &m), &m)
}

/// Nodo que se canonicaliza en lugar de `node`: el documento completo si
/// `node` es la raíz y no le precede una PI (ni un comentario cuando se
/// conservan).
fn c14n_target(doc: &Document, node: NodeId, m: &Method) -> NodeId {
    if doc.document_element() != Some(node) || !doc.is_attached(node) {
        return node;
    }
    let mut sibling = doc.previous_sibling(node);
    while let Some(s) = sibling {
        match doc.kind(s) {
            NodeKind::ProcessingInstruction { .. } => return node,
            NodeKind::Comment(_) if m.with_comments => return node,
            _ => {}
        }
        sibling = doc.previous_sibling(s);
    }
    doc.document_node()
}

/// Algoritmo de digest por URI.
fn message_digest(algorithm: &str) -> Option<MessageDigest> {
    Some(match algorithm {
        SHA1 => MessageDigest::sha1(),
        SHA256 => MessageDigest::sha256(),
        SHA384 => MessageDigest::sha384(),
        SHA512 => MessageDigest::sha512(),
        RIPEMD160 => MessageDigest::ripemd160(),
        _ => return None,
    })
}

/// Cadena con valor: ni vacía ni `"0"` (la referencia trata `"0"` como vacío).
pub(crate) fn is_set(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

/// Recorta ` \t\n\r\0\x0B` en ambos extremos.
pub(crate) fn trim_ws(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B'))
}

struct ParsedUri {
    path_empty: bool,
    fragment: Option<String>,
}

/// Lo que `processRefNode` usa de `parse_url`: si hay ruta y el fragmento.
fn parse_uri(uri: &str) -> ParsedUri {
    let (before, fragment) = match uri.split_once('#') {
        Some((b, f)) => (b, Some(f.to_owned())),
        None => (uri, None),
    };
    let before = before.split_once('?').map_or(before, |(b, _)| b);
    let path = if let Some(rest) = before
        .split_once("://")
        .map(|(_, r)| r)
        .or_else(|| before.strip_prefix("//"))
    {
        rest.find('/').map_or("", |i| &rest[i..])
    } else {
        before
    };
    ParsedUri {
        path_empty: path.is_empty(),
        fragment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_format() {
        let g = XmlSecurityDsig::generate_guid("pfx");
        assert_eq!(g.len(), 39);
        assert!(g.starts_with("pfx"));
    }

    /// El digest por bloques coincide con el de la canonicalización completa
    /// en documentos de varios bloques, con cada transformación.
    #[test]
    fn streamed_digest_matches_full_canonicalization() {
        let mut xml =
            String::from("<?pi x?><!--c--><r xmlns=\"u\" xmlns:a=\"v\" xml:lang=\"es\"><!--k-->");
        for i in 0..4000 {
            xml.push_str(&format!(
                "<a:x Id=\"i{i}\" b=\"&amp;\t\">t\u{e9}xt &lt; {i}\r<y/><!--n--></a:x>"
            ));
        }
        xml.push_str("</r>");
        let doc = Document::parse(&xml).unwrap();
        let root = doc.document_element().unwrap();
        let transforms = [
            String::new(),
            format!("<ds:Transform Algorithm=\"{C14N_COMMENTS}\"/>"),
            format!(
                "<ds:Transform Algorithm=\"{EXC_C14N}\"><ec:InclusiveNamespaces xmlns:ec=\"{EXC_C14N}\" PrefixList=\"a #default\"/></ds:Transform>"
            ),
            format!(
                "<ds:Transform Algorithm=\"{XPATH}\"><ds:XPath>not(self::comment())</ds:XPath></ds:Transform>"
            ),
        ];
        for t in transforms {
            let reference = Document::parse(&format!(
                "<ds:Reference xmlns:ds=\"{XMLDSIGNS}\"><ds:Transforms>{t}</ds:Transforms></ds:Reference>"
            ))
            .unwrap();
            let r = reference.document_element().unwrap();
            for (node, comments) in [(root, true), (root, false), (doc.document_node(), true)] {
                for alg in [SHA1, SHA256] {
                    let full = XmlSecurityDsig::process_transforms(
                        &reference,
                        r,
                        &doc,
                        Some(node),
                        comments,
                    )
                    .unwrap()
                    .unwrap();
                    assert!(full.len() > 2 * c14n::SINK_CHUNK);
                    let expected = XmlSecurityDsig::calculate_digest(alg, full.as_bytes()).unwrap();
                    let streamed = XmlSecurityDsig::transformed_digest(
                        &reference,
                        r,
                        &doc,
                        Some(node),
                        comments,
                        alg,
                    )
                    .unwrap();
                    assert_eq!(streamed, expected);
                }
            }
            let empty = XmlSecurityDsig::transformed_digest(&reference, r, &doc, None, true, SHA1);
            assert_eq!(
                empty.unwrap(),
                XmlSecurityDsig::calculate_digest(SHA1, b"").unwrap()
            );
            assert_eq!(
                XmlSecurityDsig::transformed_digest(&reference, r, &doc, Some(root), true, "x"),
                Err(Error::UnsupportedDigest("x".into()))
            );
        }
    }

    #[test]
    fn uri_parsing() {
        assert!(parse_uri("#a").path_empty);
        assert!(parse_uri("http://example.com").path_empty);
        assert!(!parse_uri("http://example.com/x.xml").path_empty);
        assert!(!parse_uri("doc.xml#a").path_empty);
    }
}
