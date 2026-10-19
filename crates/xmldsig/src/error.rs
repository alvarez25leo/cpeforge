//! Errores del crate.
//!
//! Los mensajes de [`Error`] conservan el texto de la implementación de
//! referencia para poder compararlos; la variante indica la causa sin
//! depender del texto.

use std::fmt;

/// Error de análisis XML.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum XmlError {
    /// Entrada vacía.
    Empty,
    /// XML mal formado.
    Malformed(String),
    /// El documento declara un DTD (`<!DOCTYPE …>`); se rechaza siempre.
    Dtd,
    /// Referencia a una entidad que no es predefinida.
    UndefinedEntity(String),
    /// Codificación declarada no admitida (se admiten UTF-8, ISO-8859-1,
    /// US-ASCII, windows-1252 e ISO-8859-15 con sus alias de libiconv).
    UnsupportedEncoding(String),
    /// Se superó un límite de tamaño, profundidad o nodos.
    LimitExceeded(&'static str),
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("XML vacío"),
            Self::Malformed(m) => write!(f, "XML mal formado: {m}"),
            Self::Dtd => f.write_str("XML con DTD no permitido"),
            Self::UndefinedEntity(e) => write!(f, "entidad no definida: {e}"),
            Self::UnsupportedEncoding(e) => write!(f, "codificación no admitida: {e}"),
            Self::LimitExceeded(what) => write!(f, "límite de {what} superado"),
        }
    }
}

impl std::error::Error for XmlError {}

/// Error de firma, verificación, claves o cifrado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// XML no analizable.
    Xml(XmlError),
    /// `Invalid Canonical Method`.
    InvalidCanonicalMethod(String),
    /// `Cannot validate digest: Unsupported Algorithm <…>`.
    UnsupportedDigest(String),
    /// La consulta de la transformación XPath no es válida o no devuelve un
    /// conjunto de nodos (la canonicalización falla). El texto indica la causa.
    XPath(String),
    /// `Invalid Key Type`.
    InvalidKeyType(String),
    /// `Certificate "type" (private/public) must be passed via parameters`.
    KeyTypeRequired,
    /// `Unknown key size for type "…".`
    UnknownKeySize(String),
    /// `Key must contain at least 25 characters for this cipher`.
    SymmetricKeyTooShort,
    /// `Unable to extract public key`.
    PublicKeyExtraction,
    /// Fallo de OpenSSL con el prefijo de la operación
    /// (`Failure Signing Data: …`, `Failure decrypting Data (openssl private) - …`).
    OpenSsl {
        /// Prefijo del mensaje.
        operation: String,
        /// Error de OpenSSL (`error:XXXXXXXX:biblioteca::motivo`).
        detail: String,
    },
    /// `Reference nodes not found`.
    ReferenceNodesNotFound,
    /// `Reference validation failed`.
    ReferenceValidationFailed,
    /// Más de un elemento usa el mismo identificador referenciado y la
    /// verificación estricta de identificadores únicos está activa
    /// (defensa contra XML Signature Wrapping). No existe en la referencia.
    AmbiguousReferenceId(String),
    /// `Unable to locate SignatureValue`.
    SignatureValueNotFound,
    /// `Signature DOM element not found.`
    SignatureNotFound,
    /// `Missing private key. Use setPrivateKey to set one.`
    MissingPrivateKey,
    /// `There is no set either private key or public key for signature verification.`
    MissingVerificationKey,
    /// `Unsupported signature algorithm: …`
    UnsupportedSignatureAlgorithm(String),
    /// `Unsupported digest algorithm: …`
    UnsupportedDigestAlgorithm(String),
    /// `Certificate File not found` / `File to sign, not found`.
    FileNotFound(&'static str),
    /// Otro error con el mensaje de la referencia.
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(e) => write!(f, "{e}"),
            Self::InvalidCanonicalMethod(_) => f.write_str("Invalid Canonical Method"),
            Self::UnsupportedDigest(alg) => {
                write!(f, "Cannot validate digest: Unsupported Algorithm <{alg}>")
            }
            Self::XPath(_) => f.write_str("XPath query did not return a nodeset"),
            Self::InvalidKeyType(_) => f.write_str("Invalid Key Type"),
            Self::KeyTypeRequired => {
                f.write_str("Certificate \"type\" (private/public) must be passed via parameters")
            }
            Self::UnknownKeySize(t) => write!(f, "Unknown key size for type \"{t}\"."),
            Self::SymmetricKeyTooShort => {
                f.write_str("Key must contain at least 25 characters for this cipher")
            }
            Self::PublicKeyExtraction => f.write_str("Unable to extract public key"),
            Self::OpenSsl { operation, detail } => write!(f, "{operation}{detail}"),
            Self::ReferenceNodesNotFound => f.write_str("Reference nodes not found"),
            Self::ReferenceValidationFailed => f.write_str("Reference validation failed"),
            Self::AmbiguousReferenceId(id) => {
                write!(f, "Reference validation failed: duplicate Id \"{id}\"")
            }
            Self::SignatureValueNotFound => f.write_str("Unable to locate SignatureValue"),
            Self::SignatureNotFound => f.write_str("Signature DOM element not found."),
            Self::MissingPrivateKey => {
                f.write_str("Missing private key. Use setPrivateKey to set one.")
            }
            Self::MissingVerificationKey => f.write_str(
                "There is no set either private key or public key for signature verification.",
            ),
            Self::UnsupportedSignatureAlgorithm(a) => {
                write!(f, "Unsupported signature algorithm: {a}")
            }
            Self::UnsupportedDigestAlgorithm(a) => write!(f, "Unsupported digest algorithm: {a}"),
            Self::FileNotFound(m) => f.write_str(m),
            Self::Message(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<XmlError> for Error {
    fn from(value: XmlError) -> Self {
        Self::Xml(value)
    }
}

/// Formatea el primer error de OpenSSL como `openssl_error_string()`.
pub(crate) fn openssl_detail(stack: &openssl::error::ErrorStack) -> String {
    match stack.errors().first() {
        Some(e) => format!(
            "error:{:08X}:{}::{}",
            e.code(),
            e.library().unwrap_or(""),
            e.reason().unwrap_or("")
        ),
        None => String::new(),
    }
}

pub(crate) fn openssl_error(operation: &str, stack: &openssl::error::ErrorStack) -> Error {
    Error::OpenSsl {
        operation: operation.to_owned(),
        detail: openssl_detail(stack),
    }
}
