//! Certificados X.509 (PKCS#12) y
//! los datos de `openssl_x509_parse` que usa la firma.

use std::fmt;

use openssl::asn1::Asn1Time;
use openssl::pkcs12::Pkcs12;
use openssl::x509::{X509, X509NameRef};
use zeroize::Zeroizing;

use crate::error::{Error, openssl_detail};
pub use crate::x509_ext::Purpose;

/// `X509ContentType`: formato de [`X509Certificate::export`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum X509ContentType {
    /// `X509ContentType::PEM` (1): certificado y clave privada.
    Pem,
    /// `X509ContentType::CER` (2): solo el certificado.
    Cer,
    /// Cualquier otro valor: cadena vacía.
    Other(i64),
}

impl X509ContentType {
    /// Constante numérica del tipo de contenido (`1` PEM, `2` certificado).
    #[must_use]
    pub fn from_code(value: i64) -> Self {
        match value {
            1 => Self::Pem,
            2 => Self::Cer,
            other => Self::Other(other),
        }
    }
}

/// Valor de un campo de nombre agrupado como en `openssl_x509_parse`: un
/// texto o, si el atributo se repite, la lista de valores.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameValue {
    /// Un solo valor.
    Single(String),
    /// Atributo repetido.
    Multiple(Vec<String>),
}

/// Nombre distinguido agrupado por nombre corto, en orden de primera aparición.
pub type GroupedName = Vec<(String, NameValue)>;

/// Datos de `openssl_x509_parse` usados por la biblioteca.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CertInfo {
    /// `name`: `X509_NAME_oneline` del sujeto.
    pub name: String,
    /// `subject` en orden del certificado, sin agrupar.
    pub subject: Vec<(String, String)>,
    /// `issuer` en orden del certificado, sin agrupar.
    pub issuer: Vec<(String, String)>,
    /// `serialNumber` en decimal.
    pub serial_number: String,
    /// `validFrom_time_t`.
    pub valid_from: i64,
    /// `validTo_time_t`.
    pub valid_to: i64,
    /// `purposes`: los diez propósitos estándar de OpenSSL, por `id`.
    pub purposes: Vec<Purpose>,
    /// `extensions`: nombre corto (u OID) → valor, en orden del certificado.
    /// Los valores son bytes, como cadenas de bytes: una extensión que
    /// OpenSSL no sabe imprimir conserva su `extnValue` en bruto.
    pub extensions: Vec<(String, Vec<u8>)>,
}

impl CertInfo {
    /// Datos de `openssl_x509_parse`. `None` cuando la lectura falla
    /// (por ejemplo, con un `subjectAltName` que no se puede decodificar).
    #[must_use]
    pub(crate) fn from_x509(cert: &X509) -> Option<Self> {
        let details = crate::x509_ext::details(cert)?;
        let epoch = Asn1Time::from_unix(0).ok()?;
        let seconds = |t: &openssl::asn1::Asn1TimeRef| -> Option<i64> {
            let diff = epoch.diff(t).ok()?;
            Some(i64::from(diff.days) * 86_400 + i64::from(diff.secs))
        };
        Some(Self {
            name: oneline(cert.subject_name()),
            subject: entries(cert.subject_name()),
            issuer: entries(cert.issuer_name()),
            serial_number: cert
                .serial_number()
                .to_bn()
                .ok()?
                .to_dec_str()
                .ok()?
                .to_string(),
            valid_from: seconds(cert.not_before())?,
            valid_to: seconds(cert.not_after())?,
            purposes: details.purposes,
            extensions: details.extensions,
        })
    }

    /// Lee el cuerpo Base64 de un certificado (`X509Certificate` de `KeyInfo`).
    #[must_use]
    pub fn from_base64_body(body: &str) -> Option<Self> {
        let pem = pem_from_body(body);
        X509::from_pem(pem.as_bytes())
            .ok()
            .and_then(|c| Self::from_x509(&c))
    }

    /// Sujeto agrupado como el arreglo `subject` de `openssl_x509_parse`.
    #[must_use]
    pub fn grouped_subject(&self) -> GroupedName {
        group(&self.subject)
    }

    /// Nombre del emisor como lo arma `staticAdd509Cert`: los campos en
    /// orden inverso separados por comas; un atributo repetido se imprime
    /// `Array`, como la conversión a cadena de un arreglo.
    #[must_use]
    pub fn issuer_for_serial(&self) -> String {
        group(&self.issuer)
            .into_iter()
            .rev()
            .map(|(k, v)| match v {
                NameValue::Single(s) => format!("{k}={s}"),
                NameValue::Multiple(_) => format!("{k}=Array"),
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `X509SubjectName` de `staticAdd509Cert`: cada valor antepuesto en el
    /// orden del arreglo agrupado.
    #[must_use]
    pub fn subject_name_for_key_info(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (k, v) in group(&self.subject) {
            match v {
                NameValue::Single(s) => parts.insert(0, format!("{k}={s}")),
                NameValue::Multiple(list) => {
                    for s in list {
                        parts.insert(0, format!("{k}={s}"));
                    }
                }
            }
        }
        parts.join(",")
    }
}

/// PEM con el cuerpo Base64 cortado en líneas de 64 (`chunk_split`).
#[must_use]
pub fn pem_from_body(body: &str) -> String {
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in body.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(chunk));
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

fn group(entries: &[(String, String)]) -> GroupedName {
    let mut out: GroupedName = Vec::new();
    for (k, v) in entries {
        if let Some((_, existing)) = out.iter_mut().find(|(key, _)| key == k) {
            match existing {
                NameValue::Single(s) => {
                    *existing = NameValue::Multiple(vec![std::mem::take(s), v.clone()])
                }
                NameValue::Multiple(list) => list.push(v.clone()),
            }
        } else {
            out.push((k.clone(), NameValue::Single(v.clone())));
        }
    }
    out
}

fn short_name(entry: &openssl::x509::X509NameEntryRef) -> String {
    let nid = entry.object().nid();
    match nid.short_name() {
        Ok(sn) => sn.to_owned(),
        Err(_) => entry.object().to_string(),
    }
}

fn entries(name: &X509NameRef) -> Vec<(String, String)> {
    name.entries()
        .map(|e| {
            let value = e.data().to_string().unwrap_or_default();
            (short_name(e), value)
        })
        .collect()
}

/// `X509_NAME_oneline`: `/C=PE/O=…/CN=…`; los bytes fuera de ASCII imprimible
/// se escriben `\xHH`.
fn oneline(name: &X509NameRef) -> String {
    let mut out = String::new();
    for e in name.entries() {
        out.push('/');
        out.push_str(&short_name(e));
        out.push('=');
        for b in e.data().as_slice() {
            if (0x20..0x7f).contains(b) {
                out.push(char::from(*b));
            } else {
                out.push_str(&format!("\\x{b:02X}"));
            }
        }
    }
    out
}

/// Certificado PKCS#12 con su clave privada (`X509Certificate`).
///
/// La clave privada en PEM y el PKCS#12 original se borran al soltar el valor.
pub struct X509Certificate {
    raw: Zeroizing<Vec<u8>>,
    cert_pem: Option<String>,
    key_pem: Option<Zeroizing<String>>,
    info: Option<CertInfo>,
}

impl fmt::Debug for X509Certificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("X509Certificate")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl X509Certificate {
    /// `new X509Certificate($pfx, $clave)`.
    ///
    /// # Errors
    ///
    /// [`Error::Message`] con el primer error de OpenSSL (por ejemplo
    /// `mac verify failure` con una contraseña incorrecta).
    pub fn new(pfx: &[u8], password: &str) -> Result<Self, Error> {
        let parsed = Pkcs12::from_der(pfx)
            .and_then(|p| p.parse2(password))
            .map_err(|e| Error::Message(openssl_detail(&e)))?;
        let cert_pem = match &parsed.cert {
            Some(c) => Some(
                String::from_utf8_lossy(
                    &c.to_pem().map_err(|e| Error::Message(openssl_detail(&e)))?,
                )
                .into_owned(),
            ),
            None => None,
        };
        let key_pem = match &parsed.pkey {
            Some(k) => {
                let pem = Zeroizing::new(
                    k.private_key_to_pem_pkcs8()
                        .map_err(|e| Error::Message(openssl_detail(&e)))?,
                );
                Some(Zeroizing::new(String::from_utf8_lossy(&pem).into_owned()))
            }
            None => None,
        };
        let info = parsed.cert.as_ref().and_then(CertInfo::from_x509);
        Ok(Self {
            raw: Zeroizing::new(pfx.to_vec()),
            cert_pem,
            key_pem,
            info,
        })
    }

    /// `createFromFile($ruta, $clave)`.
    ///
    /// # Errors
    ///
    /// [`Error::FileNotFound`] si no existe, o los de [`Self::new`].
    pub fn from_file(path: &std::path::Path, password: &str) -> Result<Self, Error> {
        let data = Zeroizing::new(
            std::fs::read(path).map_err(|_| Error::FileNotFound("Certificate File not found"))?,
        );
        Self::new(&data, password)
    }

    /// `getName()`.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.info.as_ref().map(|i| i.name.as_str())
    }

    /// `getSubject()`.
    #[must_use]
    pub fn subject(&self) -> Option<GroupedName> {
        self.info.as_ref().map(CertInfo::grouped_subject)
    }

    /// `getIssuer()`: **devuelve el sujeto**, por compatibilidad.
    /// Para el emisor real usar [`Self::real_issuer`].
    #[must_use]
    pub fn issuer(&self) -> Option<GroupedName> {
        self.subject()
    }

    /// Emisor real del certificado (sin equivalente en la referencia).
    #[must_use]
    pub fn real_issuer(&self) -> Option<GroupedName> {
        self.info.as_ref().map(|i| group(&i.issuer))
    }

    /// `getValidFrom()` como segundos Unix.
    #[must_use]
    pub fn valid_from(&self) -> Option<i64> {
        self.info.as_ref().map(|i| i.valid_from).filter(|v| *v != 0)
    }

    /// `getExpiration()` como segundos Unix.
    #[must_use]
    pub fn expiration(&self) -> Option<i64> {
        self.info.as_ref().map(|i| i.valid_to).filter(|v| *v != 0)
    }

    /// `getPurposes()`: `[hoja, ca, nombre]` de `X509_check_purpose` para
    /// los diez propósitos estándar de OpenSSL.
    #[must_use]
    pub fn purposes(&self) -> Option<&[Purpose]> {
        self.info.as_ref().map(|i| i.purposes.as_slice())
    }

    /// `getExtensions()`: nombre corto (u OID) → valor, como
    /// `X509V3_EXT_print` (y el formato propio de la referencia para
    /// `subjectAltName`).
    #[must_use]
    pub fn extensions(&self) -> Option<&[(String, Vec<u8>)]> {
        self.info.as_ref().map(|i| i.extensions.as_slice())
    }

    /// `getPublicKey()`: certificado PEM.
    #[must_use]
    pub fn public_key(&self) -> Option<&str> {
        self.cert_pem.as_deref()
    }

    /// `getPrivateKey()`: clave privada PEM (PKCS#8 sin cifrar).
    #[must_use]
    pub fn private_key(&self) -> Option<&str> {
        self.key_pem.as_ref().map(|k| k.as_str())
    }

    /// `getRaw()`: bytes PKCS#12 originales.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// `export($tipo)`.
    #[must_use]
    pub fn export(&self, kind: X509ContentType) -> String {
        match kind {
            X509ContentType::Pem => format!(
                "{}{}",
                self.public_key().unwrap_or(""),
                self.private_key().unwrap_or("")
            ),
            X509ContentType::Cer => self.public_key().unwrap_or("").to_owned(),
            X509ContentType::Other(_) => String::new(),
        }
    }
}

/// Fecha Unix en formato `Y-m-d\TH:i:sP` (UTC).
#[must_use]
pub fn format_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let secs = seconds.rem_euclid(86_400);
    // Algoritmo civil de días a fecha (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(format_timestamp(1_704_067_200), "2024-01-01T00:00:00+00:00");
    }
}
