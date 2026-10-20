//! Confianza en el certificado, separada de la validez matemática de la firma.
//!
//! `SignedXml::verify_xml` no evalúa la confianza: acepta
//! cualquier certificado incluido en la firma, incluso vencido o
//! autofirmado. Este módulo valida la cadena contra
//! raíces explícitas y la vigencia en un instante dado, para usarlo después
//! de comprobar la firma. Exige además que el certificado firmante sirva
//! para firmar (`keyUsage`) y, si se indican, sus usos extendidos.

use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::verify::{X509VerifyFlags, X509VerifyParam};
use openssl::x509::{X509, X509StoreContext};

use crate::error::{Error, openssl_detail};
use crate::x509_ext::signing_usage;

/// OID de `anyExtendedKeyUsage`.
pub const ANY_EXTENDED_KEY_USAGE: &str = "2.5.29.37.0";

/// Política de confianza: raíces aceptadas e instante de evaluación.
#[derive(Debug, Clone)]
pub struct TrustPolicy {
    roots: Vec<X509>,
    intermediates: Vec<X509>,
    /// Instante Unix para evaluar la vigencia; `None` usa la hora actual.
    pub at: Option<i64>,
    /// Rechaza un certificado cuyo `keyUsage` no incluye `digitalSignature`
    /// ni `nonRepudiation` (por ejemplo, uno solo de cifrado). `true` por
    /// defecto; sin la extensión se acepta.
    pub require_signing_key_usage: bool,
    /// OIDs de `extendedKeyUsage` aceptados; vacío acepta cualquiera. Si el
    /// certificado declara la extensión, debe incluir uno de estos o
    /// [`ANY_EXTENDED_KEY_USAGE`]; sin la extensión se acepta (RFC 5280).
    pub accepted_extended_key_usages: Vec<String>,
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            intermediates: Vec::new(),
            at: None,
            require_signing_key_usage: true,
            accepted_extended_key_usages: Vec::new(),
        }
    }
}

/// Resultado de la evaluación de confianza.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TrustResult {
    /// La cadena termina en una raíz de la política y está vigente.
    Trusted,
    /// No confiable, con el motivo de OpenSSL (`certificate has expired`,
    /// `self-signed certificate`, …).
    Untrusted(String),
}

impl TrustPolicy {
    /// Política sin raíces: ningún certificado es confiable.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Agrega una raíz confiable en PEM.
    ///
    /// # Errors
    ///
    /// [`Error::Message`] si el PEM no es un certificado.
    pub fn add_root_pem(&mut self, pem: &[u8]) -> Result<(), Error> {
        self.roots
            .push(X509::from_pem(pem).map_err(|e| Error::Message(openssl_detail(&e)))?);
        Ok(())
    }

    /// Agrega un certificado intermedio en PEM.
    ///
    /// # Errors
    ///
    /// [`Error::Message`] si el PEM no es un certificado.
    pub fn add_intermediate_pem(&mut self, pem: &[u8]) -> Result<(), Error> {
        self.intermediates
            .push(X509::from_pem(pem).map_err(|e| Error::Message(openssl_detail(&e)))?);
        Ok(())
    }

    /// Evalúa el certificado firmante (PEM, por ejemplo
    /// [`crate::key::XmlSecurityKey::x509_certificate`]).
    ///
    /// # Errors
    ///
    /// [`Error::Message`] si el PEM no es un certificado o OpenSSL falla.
    pub fn evaluate_pem(&self, cert_pem: &[u8]) -> Result<TrustResult, Error> {
        let err = |e: openssl::error::ErrorStack| Error::Message(openssl_detail(&e));
        let cert = X509::from_pem(cert_pem).map_err(err)?;
        let mut builder = X509StoreBuilder::new().map_err(err)?;
        for root in &self.roots {
            builder.add_cert(root.clone()).map_err(err)?;
        }
        if let Some(at) = self.at {
            let mut param = X509VerifyParam::new().map_err(err)?;
            #[allow(
                clippy::useless_conversion,
                reason = "time_t es i32 en algunas plataformas"
            )]
            param.set_time(
                at.try_into()
                    .map_err(|_| Error::Message("instante fuera de rango".into()))?,
            );
            param
                .set_flags(X509VerifyFlags::USE_CHECK_TIME)
                .map_err(err)?;
            builder.set_param(&param).map_err(err)?;
        }
        let store = builder.build();
        let mut chain = Stack::new().map_err(err)?;
        for c in &self.intermediates {
            chain.push(c.clone()).map_err(err)?;
        }
        let mut ctx = X509StoreContext::new().map_err(err)?;
        let result = ctx
            .init(&store, &cert, &chain, |c| {
                Ok(if c.verify_cert()? {
                    None
                } else {
                    Some(c.error().error_string().to_owned())
                })
            })
            .map_err(err)?;
        if let Some(reason) = result {
            return Ok(TrustResult::Untrusted(reason));
        }
        Ok(match self.usage_problem(&cert) {
            None => TrustResult::Trusted,
            Some(reason) => TrustResult::Untrusted(reason.to_owned()),
        })
    }

    fn usage_problem(&self, cert: &X509) -> Option<&'static str> {
        let Some(usage) = signing_usage(cert) else {
            return Some("unreadable certificate extensions");
        };
        if usage.invalid {
            return Some("invalid or duplicated certificate extensions");
        }
        if self.require_signing_key_usage && !usage.can_sign {
            return Some("keyUsage does not allow digitalSignature or nonRepudiation");
        }
        match usage.extended {
            Some(oids)
                if !self.accepted_extended_key_usages.is_empty()
                    && !oids.iter().any(|oid| {
                        oid == ANY_EXTENDED_KEY_USAGE
                            || self.accepted_extended_key_usages.contains(oid)
                    }) =>
            {
                Some("extendedKeyUsage does not include an accepted purpose")
            }
            _ => None,
        }
    }
}
