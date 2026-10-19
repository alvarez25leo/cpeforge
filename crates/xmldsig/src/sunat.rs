//! Perfil de firma SUNAT.
//!
//! Comportamiento de compatibilidad que se conserva a propósito:
//!
//! - Algoritmos por defecto **RSA-SHA1 y SHA-1** (perfil de compatibilidad
//!   SUNAT); se admiten RSA-SHA256 y SHA-256 con los setters.
//! - [`SignedXml::verify_xml`] usa **siempre** el certificado incluido en la
//!   firma, aunque se haya configurado otro con [`SignedXml::set_certificate`].
//! - La verificación solo comprueba la validez matemática de la firma y los
//!   digest; no evalúa la confianza, la vigencia ni la cadena del
//!   certificado. Para eso usar [`crate::trust`].

use std::fmt;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use zeroize::Zeroizing;

use crate::dom::{Document, NodeId};
use crate::dsig::{C14N, ENVELOPED, ReferenceOptions, SHA1, SHA256, Transform, XmlSecurityDsig};
use crate::enc::XmlSecEnc;
use crate::error::Error;
use crate::key::{RSA_SHA1, RSA_SHA256, SignatureCheck, XmlSecurityKey};

/// `SignedXml::EXT_NS`.
pub const EXT_NS: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2";

/// Firma y verificación de comprobantes UBL (`SignedXml`).
///
/// `Debug` no muestra el PEM (contiene la clave privada) y el PEM se borra de
/// memoria al soltar el firmante: `set_certificate` guarda el mismo PEM como
/// clave privada y como pública, así que ambas copias se borran.
#[derive(Clone)]
pub struct SignedXml {
    private_key: Option<Zeroizing<String>>,
    public_key: Option<Zeroizing<String>>,
    key_algorithm: String,
    digest_algorithm: String,
    canonical_method: String,
    reject_duplicate_ids: bool,
    /// Clave privada ya cargada (ver [`SignedXml::private_security_key`]).
    loaded_key: OnceLock<Arc<XmlSecurityKey>>,
}

impl fmt::Debug for SignedXml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedXml")
            .field(
                "private_key",
                &self.private_key.as_ref().map(|_| "<oculta>"),
            )
            .field("has_public_key", &self.public_key.is_some())
            .field("key_algorithm", &self.key_algorithm)
            .field("digest_algorithm", &self.digest_algorithm)
            .field("canonical_method", &self.canonical_method)
            .finish_non_exhaustive()
    }
}

impl Default for SignedXml {
    fn default() -> Self {
        Self::new()
    }
}

impl SignedXml {
    /// `new SignedXml()`: RSA-SHA1, SHA-1 y C14N.
    #[must_use]
    pub fn new() -> Self {
        Self {
            private_key: None,
            public_key: None,
            key_algorithm: RSA_SHA1.to_owned(),
            digest_algorithm: SHA1.to_owned(),
            canonical_method: C14N.to_owned(),
            reject_duplicate_ids: false,
            loaded_key: OnceLock::new(),
        }
    }

    /// `signXml($xml)`: firma y devuelve el XML serializado.
    ///
    /// # Errors
    ///
    /// XML inválido (incluidos DTD y entidades), falta de clave privada o
    /// errores de firma.
    pub fn sign_xml(&self, content: &str) -> Result<String, Error> {
        let mut doc = Document::parse(content)?;
        self.sign(&mut doc)?;
        Ok(doc.to_xml())
    }

    /// `signXml($bytes)` con la entrada y la salida en bytes: respeta la
    /// codificación declarada (por ejemplo ISO-8859-1) en ambos sentidos.
    ///
    /// # Errors
    ///
    /// Los de [`Self::sign_xml`] y [`crate::XmlError::UnsupportedEncoding`].
    pub fn sign_xml_bytes(&self, content: &[u8]) -> Result<Vec<u8>, Error> {
        let mut doc = Document::parse_bytes(content)?;
        self.sign(&mut doc)?;
        Ok(doc.to_xml_bytes())
    }

    /// `verifyXml($xml)`: toma la clave del certificado de la firma y verifica.
    ///
    /// # Errors
    ///
    /// XML inválido, firma ausente o sin clave para verificar.
    pub fn verify_xml(&mut self, content: &str) -> Result<bool, Error> {
        let mut doc = Document::parse(content)?;
        let preloaded = self.public_key_from_node(&doc)?;
        self.verify_with(&mut doc, preloaded)
    }

    /// `verifyXml($bytes)` con la entrada en bytes (ver [`Self::sign_xml_bytes`]).
    ///
    /// # Errors
    ///
    /// Los de [`Self::verify_xml`].
    pub fn verify_xml_bytes(&mut self, content: &[u8]) -> Result<bool, Error> {
        let mut doc = Document::parse_bytes(content)?;
        let preloaded = self.public_key_from_node(&doc)?;
        self.verify_with(&mut doc, preloaded)
    }

    /// `setCertificate($pem)`: PEM con clave privada y certificado.
    pub fn set_certificate(&mut self, cert: &str) {
        self.private_key = Some(Zeroizing::new(cert.to_owned()));
        self.public_key = Some(Zeroizing::new(cert.to_owned()));
        self.loaded_key = OnceLock::new();
    }

    /// `setSignatureAlgorithm($uri)`.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedSignatureAlgorithm`] salvo RSA-SHA1 y RSA-SHA256.
    pub fn set_signature_algorithm(&mut self, algorithm: &str) -> Result<(), Error> {
        if ![RSA_SHA1, RSA_SHA256].contains(&algorithm) {
            return Err(Error::UnsupportedSignatureAlgorithm(algorithm.to_owned()));
        }
        algorithm.clone_into(&mut self.key_algorithm);
        self.loaded_key = OnceLock::new();
        Ok(())
    }

    /// `setDigestAlgorithm($uri)`.
    ///
    /// # Errors
    ///
    /// [`Error::UnsupportedDigestAlgorithm`] salvo SHA-1 y SHA-256.
    pub fn set_digest_algorithm(&mut self, algorithm: &str) -> Result<(), Error> {
        if ![SHA1, SHA256].contains(&algorithm) {
            return Err(Error::UnsupportedDigestAlgorithm(algorithm.to_owned()));
        }
        algorithm.clone_into(&mut self.digest_algorithm);
        Ok(())
    }

    /// `setCertificateFromFile($ruta)`.
    ///
    /// # Errors
    ///
    /// `Certificate File not found`.
    pub fn set_certificate_from_file(&mut self, path: &Path) -> Result<(), Error> {
        let data = Zeroizing::new(
            std::fs::read(path).map_err(|_| Error::FileNotFound("Certificate File not found"))?,
        );
        self.set_certificate(&String::from_utf8_lossy(&data));
        Ok(())
    }

    /// Verificación estricta de identificadores: una referencia `#id` que
    /// coincide con más de un elemento hace que [`Self::verify`] devuelva
    /// `false` en vez de usar el primero (defensa contra *XML Signature
    /// Wrapping*). Desactivado por defecto.
    pub fn set_reject_duplicate_ids(&mut self, reject: bool) {
        self.reject_duplicate_ids = reject;
    }

    /// Algoritmo de firma vigente.
    #[must_use]
    pub fn signature_algorithm(&self) -> &str {
        &self.key_algorithm
    }

    /// Algoritmo de digest vigente.
    #[must_use]
    pub fn digest_algorithm(&self) -> &str {
        &self.digest_algorithm
    }

    /// `getPublicKey($doc)`: con documento, reemplaza la clave pública por el
    /// certificado de su firma (si la tiene).
    ///
    /// # Errors
    ///
    /// Los de lectura de `KeyInfo`.
    pub fn public_key(&mut self, doc: Option<&Document>) -> Result<Option<&str>, Error> {
        if let Some(doc) = doc {
            self.set_public_key_from_node(doc)?;
        }
        Ok(self.public_key.as_ref().map(|k| k.as_str()))
    }

    /// Clave pública configurada, sin leer documentos.
    #[must_use]
    pub fn configured_public_key(&self) -> Option<&str> {
        self.public_key.as_ref().map(|k| k.as_str())
    }

    /// `sign($doc)`: agrega la firma envuelta (`URI=""`) al primer
    /// `ext:ExtensionContent` vacío, o a la raíz si no hay.
    ///
    /// # Errors
    ///
    /// [`Error::MissingPrivateKey`] y los de firma.
    pub fn sign(&self, doc: &mut Document) -> Result<(), Error> {
        let key = self.private_security_key()?;
        let mut dsig = XmlSecurityDsig::default();
        dsig.set_canonical_method(doc, &self.canonical_method)?;
        let root = doc.document_node();
        let options = ReferenceOptions {
            force_uri: true,
            ..ReferenceOptions::default()
        };
        dsig.add_reference(
            doc,
            root,
            &self.digest_algorithm,
            Some(&[Transform::Algorithm(ENVELOPED.to_owned())]),
            Some(&options),
        )?;
        let target = node_sign(doc);
        dsig.sign(doc, key.as_ref(), target)?;
        if let Some(public) = self
            .public_key
            .as_deref()
            .filter(|p| !p.is_empty() && *p != "0")
        {
            dsig.add_509_cert(doc, public, true, crate::dsig::CertOptions::default());
        }
        Ok(())
    }

    /// Clave privada lista para firmar. La implementación de referencia la
    /// vuelve a leer del PEM en cada firma; aquí la primera carga correcta se
    /// guarda, de modo que OpenSSL reutiliza la misma clave RSA y su
    /// *blinding* (crearlo cuesta cerca de un cuarto de la firma). La firma PKCS#1 v1.5 es determinista: el
    /// resultado no cambia. Una carga fallida no se guarda, para que cada
    /// firma devuelva el mismo error.
    fn private_security_key(&self) -> Result<Arc<XmlSecurityKey>, Error> {
        if let Some(key) = self.loaded_key.get() {
            return Ok(Arc::clone(key));
        }
        let private = self.private_key.as_ref().ok_or(Error::MissingPrivateKey)?;
        let mut key = XmlSecurityKey::new(&self.key_algorithm, Some("private"))?;
        key.load_key(private.as_bytes(), false)?;
        let key = Arc::new(key);
        if key.has_private_key() {
            // Si otro hilo la guardó antes, se usa esta copia equivalente.
            let _ = self.loaded_key.set(Arc::clone(&key));
        }
        Ok(key)
    }

    /// `signFromFile($ruta)`: devuelve los bytes firmados, en la codificación
    /// declarada por el archivo.
    ///
    /// # Errors
    ///
    /// `File to sign, not found` y los de [`Self::sign_xml`].
    pub fn sign_from_file(&self, path: &Path) -> Result<Vec<u8>, Error> {
        let data =
            std::fs::read(path).map_err(|_| Error::FileNotFound("File to sign, not found"))?;
        self.sign_xml_bytes(&data)
    }

    /// `verify($doc)`: `true` si la firma y todas las referencias son válidas.
    ///
    /// Modifica `doc`: la validación de referencias retira el nodo `Signature`.
    ///
    /// # Errors
    ///
    /// [`Error::SignatureNotFound`], [`Error::MissingVerificationKey`] y los
    /// de lectura de `KeyInfo` o de la clave pública.
    pub fn verify(&mut self, doc: &mut Document) -> Result<bool, Error> {
        self.verify_with(doc, None)
    }

    /// [`Self::verify`] reutilizando, si corresponde, la clave que
    /// [`Self::public_key_from_node`] ya cargó del mismo certificado.
    fn verify_with(
        &mut self,
        doc: &mut Document,
        preloaded: Option<XmlSecurityKey>,
    ) -> Result<bool, Error> {
        let mut dsig = XmlSecurityDsig::default();
        dsig.reject_duplicate_ids = self.reject_duplicate_ids;
        let root = doc.document_node();
        let Some(sig) = dsig.locate_signature(doc, root, 0) else {
            return Err(Error::SignatureNotFound);
        };
        dsig.canonicalize_signed_info(doc);
        let mut key: Option<XmlSecurityKey> = None;
        if !self.has_public_key() {
            let located = dsig.locate_key(doc).ok_or(Error::MissingVerificationKey)?;
            let located = XmlSecEnc::static_locate_key_info(doc, Some(located), sig)?;
            if let Some(k) = located {
                self.public_key = k.x509_certificate().map(|c| Zeroizing::new(c.to_owned()));
                k.algorithm().clone_into(&mut self.key_algorithm);
                key = Some(k);
            }
        }
        let key = match key {
            Some(k) => k,
            None => {
                let located = dsig.locate_key(doc);
                // La clave ya cargada del certificado de la firma es la misma
                // que se obtendría al leer de nuevo `public_key`: mismo
                // algoritmo (`SignatureMethod`) y mismo PEM.
                let configured = self.public_key.as_ref().map(|k| k.as_str());
                if let (Some(l), Some(p)) = (&located, preloaded) {
                    if l.algorithm() == p.algorithm()
                        && p.key_type() == l.key_type()
                        && p.x509_certificate().is_some()
                        && p.x509_certificate() == configured
                    {
                        return Self::check(&mut dsig, doc, &p);
                    }
                }
                let mut k = match located {
                    Some(k) => k,
                    None => XmlSecurityKey::new(&self.key_algorithm, Some("public"))?,
                };
                k.load_key(
                    self.public_key
                        .as_ref()
                        .map(|k| k.as_str())
                        .unwrap_or("")
                        .as_bytes(),
                    false,
                )?;
                k
            }
        };
        Self::check(&mut dsig, doc, &key)
    }

    /// Firma (`SignatureValue`) y después referencias.
    fn check(
        dsig: &mut XmlSecurityDsig,
        doc: &mut Document,
        key: &XmlSecurityKey,
    ) -> Result<bool, Error> {
        let valid = match dsig.verify(doc, key)? {
            SignatureCheck::Valid => !key.is_hmac(),
            SignatureCheck::Invalid | SignatureCheck::Error(_) => false,
        };
        if !valid {
            return Ok(false);
        }
        Ok(dsig.validate_reference(doc).is_ok())
    }

    fn has_public_key(&self) -> bool {
        self.public_key
            .as_deref()
            .is_some_and(|p| !p.is_empty() && p != "0")
    }

    fn set_public_key_from_node(&mut self, doc: &Document) -> Result<(), Error> {
        self.public_key_from_node(doc).map(|_| ())
    }

    /// Toma la clave pública del certificado de la firma de `doc` y
    /// devuelve la clave cargada (para no volver a leerla al verificar).
    fn public_key_from_node(&mut self, doc: &Document) -> Result<Option<XmlSecurityKey>, Error> {
        let mut dsig = XmlSecurityDsig::default();
        let Some(sig) = dsig.locate_signature(doc, doc.document_node(), 0) else {
            return Ok(None);
        };
        let Some(key) = dsig.locate_key(doc) else {
            return Ok(None);
        };
        let key = XmlSecEnc::static_locate_key_info(doc, Some(key), sig)?;
        if let Some(k) = &key {
            self.public_key = k.x509_certificate().map(|c| Zeroizing::new(c.to_owned()));
            k.algorithm().clone_into(&mut self.key_algorithm);
        }
        Ok(key)
    }
}

/// `getNodeSign`: primer `ext:ExtensionContent` cuyo texto recortado es vacío.
fn node_sign(doc: &Document) -> Option<NodeId> {
    doc.descendants_ns(doc.document_node(), Some(EXT_NS), "ExtensionContent")
        .into_iter()
        .find(|n| crate::dsig::trim_ws(&doc.text_content(*n)).is_empty())
        .or_else(|| doc.document_element())
}
