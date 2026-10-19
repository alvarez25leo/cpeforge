//! `purposes` y `extensions` de `openssl_x509_parse`, que devuelven
//! `X509Certificate::purposes()` y `extensions()`.
//!
//! El crate `openssl` no expone `X509_check_purpose` ni las extensiones de
//! un certificado con API segura, así que:
//!
//! - los propósitos reproducen las reglas de `crypto/x509/v3_purp.c`
//!   (`ossl_x509v3_cache_extensions`, `check_ca` y las diez funciones
//!   `check_purpose_*`) sobre las extensiones leídas del DER;
//! - los valores de las extensiones salen del propio OpenSSL: `X509_print`
//!   llama a `X509V3_EXT_print` con los mismos `flag = 0` que la referencia, solo que
//!   con sangría 16 en vez de 0, y aquí se retira esa sangría. Una extensión que OpenSSL no sabe imprimir devuelve los bytes de
//!   `extnValue`, y `subjectAltName` usa el formato propio de la referencia
//!   (`openssl_x509v3_subjectAltName`).

use openssl::asn1::Asn1Object;
use openssl::nid::Nid;
use openssl::x509::{X509, X509Name, X509PurposeRef};

// ---------------------------------------------------------------------------
// DER mínimo (solo lectura, longitudes definidas, etiquetas de un byte).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Tlv<'a> {
    tag: u8,
    content: &'a [u8],
    raw: &'a [u8],
}

fn read(input: &[u8]) -> Option<(Tlv<'_>, &[u8])> {
    let (&tag, rest) = input.split_first()?;
    if tag & 0x1f == 0x1f {
        return None;
    }
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 4 {
            return None;
        }
        let (bytes, rest) = rest.split_at_checked(count)?;
        let len = bytes
            .iter()
            .fold(0usize, |acc, byte| (acc << 8) | usize::from(*byte));
        (len, rest)
    };
    let (content, after) = rest.split_at_checked(len)?;
    let raw = input.get(..input.len() - after.len())?;
    Some((Tlv { tag, content, raw }, after))
}

/// Un único TLV que ocupa todo `input`.
fn read_all(input: &[u8]) -> Option<Tlv<'_>> {
    match read(input)? {
        (tlv, []) => Some(tlv),
        _ => None,
    }
}

fn children(content: &[u8]) -> Option<Vec<Tlv<'_>>> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let (tlv, after) = read(rest)?;
        out.push(tlv);
        rest = after;
    }
    Some(out)
}

/// OID en notación con puntos.
fn oid_text(content: &[u8]) -> Option<String> {
    let mut arcs: Vec<u128> = Vec::new();
    let mut value: u128 = 0;
    for byte in content {
        value = value.checked_mul(128)? | u128::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            arcs.push(value);
            value = 0;
        }
    }
    if content.last().is_none_or(|b| b & 0x80 != 0) {
        return None;
    }
    let (&first, rest) = arcs.split_first()?;
    let (a, b) = match first {
        0..40 => (0, first),
        40..80 => (1, first - 40),
        _ => (2, first - 80),
    };
    let mut text = format!("{a}.{b}");
    for arc in rest {
        text.push('.');
        text.push_str(&arc.to_string());
    }
    Some(text)
}

/// Extensión tal como figura en el certificado.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Extension {
    oid: String,
    critical: bool,
    value: Vec<u8>,
}

/// Número de serie y extensiones del `tbsCertificate`.
fn parse_certificate(der: &[u8]) -> Option<(Vec<u8>, Vec<Extension>)> {
    let cert = read_all(der).filter(|t| t.tag == 0x30)?;
    let tbs = *children(cert.content)?.first().filter(|t| t.tag == 0x30)?;
    let items = children(tbs.content)?;
    let skip = usize::from(items.first().is_some_and(|t| t.tag == 0xa0));
    let serial = items.get(skip).filter(|t| t.tag == 0x02)?.content.to_vec();
    let mut extensions = Vec::new();
    if let Some(block) = items.iter().find(|t| t.tag == 0xa3) {
        let list = read_all(block.content).filter(|t| t.tag == 0x30)?;
        for ext in children(list.content)? {
            let parts = children(ext.content)?;
            let oid = oid_text(parts.first().filter(|t| t.tag == 0x06)?.content)?;
            let (critical, value) = match parts.as_slice() {
                [_, value] => (false, value),
                [_, flag, value] if flag.tag == 0x01 => {
                    (flag.content.first().is_some_and(|b| *b != 0), value)
                }
                _ => return None,
            };
            if value.tag != 0x04 {
                return None;
            }
            extensions.push(Extension {
                oid,
                critical,
                value: value.content.to_vec(),
            });
        }
    }
    Some((serial, extensions))
}

// ---------------------------------------------------------------------------
// Propósitos (`X509_check_purpose`).
// ---------------------------------------------------------------------------

const OID_BASIC_CONSTRAINTS: &str = "2.5.29.19";
const OID_KEY_USAGE: &str = "2.5.29.15";
const OID_EXT_KEY_USAGE: &str = "2.5.29.37";
const OID_NS_CERT_TYPE: &str = "2.16.840.1.113730.1.1";
const OID_SKID: &str = "2.5.29.14";
const OID_AKID: &str = "2.5.29.35";
const OID_SAN: &str = "2.5.29.17";
const OID_IAN: &str = "2.5.29.18";
const OID_NAME_CONSTRAINTS: &str = "2.5.29.30";
const OID_CRL_DP: &str = "2.5.29.31";
const OID_PROXY_CERT_INFO: &str = "1.3.6.1.5.5.7.1.14";
const OID_IP_ADDR_BLOCK: &str = "1.3.6.1.5.5.7.1.7";
const OID_AS_ID: &str = "1.3.6.1.5.5.7.1.8";

const KU_DIGITAL_SIGNATURE: u32 = 0x0080;
const KU_NON_REPUDIATION: u32 = 0x0040;
const KU_KEY_ENCIPHERMENT: u32 = 0x0020;
const KU_KEY_AGREEMENT: u32 = 0x0008;
const KU_KEY_CERT_SIGN: u32 = 0x0004;
const KU_CRL_SIGN: u32 = 0x0002;
const KU_TLS: u32 = KU_DIGITAL_SIGNATURE | KU_KEY_ENCIPHERMENT | KU_KEY_AGREEMENT;

const XKU_SSL_SERVER: u32 = 0x1;
const XKU_SSL_CLIENT: u32 = 0x2;
const XKU_SMIME: u32 = 0x4;
const XKU_CODE_SIGN: u32 = 0x8;
const XKU_SGC: u32 = 0x10;
const XKU_OCSP_SIGN: u32 = 0x20;
const XKU_TIMESTAMP: u32 = 0x40;
const XKU_DVCS: u32 = 0x80;
const XKU_ANYEKU: u32 = 0x100;

const NS_SSL_CLIENT: u32 = 0x80;
const NS_SSL_SERVER: u32 = 0x40;
const NS_SMIME: u32 = 0x20;
const NS_SSL_CA: u32 = 0x04;
const NS_SMIME_CA: u32 = 0x02;
const NS_ANY_CA: u32 = 0x07;

/// Resultado de `X509_check_purpose` para un propósito estándar, como el
/// arreglo `[hoja, ca, nombre corto]` indexado por `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purpose {
    /// `X509_PURPOSE_get_id` (1 = `sslclient` … 10 = `codesign`).
    pub id: i32,
    /// `X509_check_purpose($cert, $id, 0)` distinto de 0.
    pub leaf: bool,
    /// `X509_check_purpose($cert, $id, 1)` distinto de 0.
    pub ca: bool,
    /// `X509_PURPOSE_get0_sname`.
    pub short_name: &'static str,
}

/// `ex_flags` y campos derivados de `ossl_x509v3_cache_extensions`.
#[derive(Default)]
struct Flags {
    v1: bool,
    bcons: bool,
    ca: bool,
    kusage: bool,
    xkusage: bool,
    nscert: bool,
    self_signed: bool,
    invalid: bool,
    ku: u32,
    xku: u32,
    ns: u32,
}

enum Lookup<'a> {
    Absent,
    One(&'a Extension),
    /// `X509_get_ext_d2i` devuelve `NULL` con `i = -2`.
    Many,
}

fn lookup<'a>(extensions: &'a [Extension], oid: &str) -> Lookup<'a> {
    let mut found = extensions.iter().filter(|e| e.oid == oid);
    match (found.next(), found.next()) {
        (None, _) => Lookup::Absent,
        (Some(one), None) => Lookup::One(one),
        (Some(_), Some(_)) => Lookup::Many,
    }
}

/// Decodifica una extensión única; `None` si falta. Una extensión repetida
/// o ilegible marca el certificado como inválido, como `EXFLAG_INVALID`.
fn decode<T>(
    extensions: &[Extension],
    oid: &str,
    flags: &mut Flags,
    parse: impl FnOnce(&[u8]) -> Option<T>,
) -> Option<T> {
    match lookup(extensions, oid) {
        Lookup::Absent => None,
        Lookup::Many => {
            flags.invalid = true;
            None
        }
        Lookup::One(ext) => {
            let value = parse(&ext.value);
            flags.invalid |= value.is_none();
            value
        }
    }
}

/// Bits de un `BIT STRING` como `ex_kusage`: primer octeto y segundo << 8,
/// con los bits no usados del último octeto a cero.
fn bit_string(value: &[u8]) -> Option<Vec<u8>> {
    let tlv = read_all(value).filter(|t| t.tag == 0x03)?;
    let (&unused, data) = tlv.content.split_first()?;
    if unused > 7 || (data.is_empty() && unused != 0) {
        return None;
    }
    let mut data = data.to_vec();
    if let Some(last) = data.last_mut() {
        *last &= 0xff << unused;
    }
    Some(data)
}

fn sequence(value: &[u8]) -> Option<Vec<Tlv<'_>>> {
    children(read_all(value).filter(|t| t.tag == 0x30)?.content)
}

struct AuthorityKeyId {
    key_id: Option<Vec<u8>>,
    issuer_names: Vec<Vec<u8>>,
    serial: Option<Vec<u8>>,
}

fn authority_key_id(value: &[u8]) -> Option<AuthorityKeyId> {
    let mut akid = AuthorityKeyId {
        key_id: None,
        issuer_names: Vec::new(),
        serial: None,
    };
    for item in sequence(value)? {
        match item.tag {
            0x80 => akid.key_id = Some(item.content.to_vec()),
            0xa1 => {
                for name in general_names(item.content)? {
                    if let GeneralName::Directory(der) = name {
                        akid.issuer_names.push(der.to_vec());
                    }
                }
            }
            0x82 => akid.serial = Some(item.content.to_vec()),
            _ => return None,
        }
    }
    Some(akid)
}

fn cache_extensions(cert: &X509, serial: &[u8], extensions: &[Extension]) -> Flags {
    let mut flags = Flags {
        v1: cert.version() == 0,
        ..Flags::default()
    };
    if let Some((ca, pathlen_negative)) =
        decode(extensions, OID_BASIC_CONSTRAINTS, &mut flags, |v| {
            let mut ca = false;
            let mut negative = false;
            for item in sequence(v)? {
                match item.tag {
                    0x01 => ca = item.content.first().is_some_and(|b| *b != 0),
                    0x02 => negative = item.content.first().is_some_and(|b| b & 0x80 != 0),
                    _ => return None,
                }
            }
            Some((ca, negative))
        })
    {
        flags.ca = ca;
        flags.invalid |= pathlen_negative;
        flags.bcons = true;
    }
    if decode(extensions, OID_PROXY_CERT_INFO, &mut flags, |v| {
        sequence(v).map(|_| ())
    })
    .is_some()
        && (flags.ca
            || !matches!(lookup(extensions, OID_SAN), Lookup::Absent)
            || !matches!(lookup(extensions, OID_IAN), Lookup::Absent))
    {
        flags.invalid = true;
    }
    if let Some(bits) = decode(extensions, OID_KEY_USAGE, &mut flags, bit_string) {
        flags.ku = u32::from(bits.first().copied().unwrap_or(0))
            | (u32::from(bits.get(1).copied().unwrap_or(0)) << 8);
        flags.kusage = true;
        flags.invalid |= flags.ku == 0;
    }
    if let Some(oids) = decode(extensions, OID_EXT_KEY_USAGE, &mut flags, |v| {
        sequence(v)?
            .iter()
            .map(|t| (t.tag == 0x06).then(|| oid_text(t.content)).flatten())
            .collect::<Option<Vec<_>>>()
    }) {
        flags.xkusage = true;
        for oid in oids {
            flags.xku |= match oid.as_str() {
                "1.3.6.1.5.5.7.3.1" => XKU_SSL_SERVER,
                "1.3.6.1.5.5.7.3.2" => XKU_SSL_CLIENT,
                "1.3.6.1.5.5.7.3.4" => XKU_SMIME,
                "1.3.6.1.5.5.7.3.3" => XKU_CODE_SIGN,
                "1.3.6.1.4.1.311.10.3.3" | "2.16.840.1.113730.4.1" => XKU_SGC,
                "1.3.6.1.5.5.7.3.9" => XKU_OCSP_SIGN,
                "1.3.6.1.5.5.7.3.8" => XKU_TIMESTAMP,
                "1.3.6.1.5.5.7.3.10" => XKU_DVCS,
                "2.5.29.37.0" => XKU_ANYEKU,
                _ => 0,
            };
        }
    }
    if let Some(bits) = decode(extensions, OID_NS_CERT_TYPE, &mut flags, bit_string) {
        flags.ns = u32::from(bits.first().copied().unwrap_or(0));
        flags.nscert = true;
    }
    let skid = decode(extensions, OID_SKID, &mut flags, |v| {
        Some(read_all(v).filter(|t| t.tag == 0x04)?.content.to_vec())
    });
    let akid = decode(extensions, OID_AKID, &mut flags, authority_key_id);
    flags.self_signed = cert
        .subject_name()
        .try_cmp(cert.issuer_name())
        .is_ok_and(|o| o.is_eq())
        && match &akid {
            None => true,
            Some(akid) => {
                let key_matches = match (&akid.key_id, &skid) {
                    (Some(key), Some(skid)) => key == skid,
                    _ => true,
                };
                let serial_matches = akid.serial.as_deref().is_none_or(|s| s == serial);
                let issuer_matches = akid.issuer_names.first().is_none_or(|der| {
                    X509Name::from_der(der)
                        .ok()
                        .and_then(|name| name.try_cmp(cert.issuer_name()).ok())
                        .is_some_and(|o| o.is_eq())
                });
                key_matches && serial_matches && issuer_matches
            }
        }
        && signature_matches_key(cert);
    for (oid, parse) in [
        (OID_SAN, general_names_value as fn(&[u8]) -> Option<()>),
        (OID_NAME_CONSTRAINTS, |v| sequence(v).map(|_| ())),
        (OID_CRL_DP, |v| sequence(v).map(|_| ())),
        (OID_IP_ADDR_BLOCK, |v| sequence(v).map(|_| ())),
        (OID_AS_ID, |v| sequence(v).map(|_| ())),
    ] {
        decode(extensions, oid, &mut flags, parse);
    }
    flags
}

fn general_names_value(value: &[u8]) -> Option<()> {
    let tlv = read_all(value).filter(|t| t.tag == 0x30)?;
    general_names(tlv.content).map(|_| ())
}

/// `check_sig_alg_match`: el algoritmo de firma corresponde a la clave
/// pública del propio certificado (o RSA con RSASSA-PSS).
fn signature_matches_key(cert: &X509) -> bool {
    let Some(algorithms) = cert
        .signature_algorithm()
        .object()
        .nid()
        .signature_algorithms()
    else {
        return false;
    };
    let Ok(key) = cert.public_key() else {
        return false;
    };
    let key_id = key.id().as_raw();
    key_id == algorithms.pkey.as_raw()
        || (key_id == Nid::RSAENCRYPTION.as_raw() && algorithms.pkey == Nid::RSASSAPSS)
}

impl Flags {
    fn ku_reject(&self, usage: u32) -> bool {
        self.kusage && self.ku & usage == 0
    }
    fn xku_reject(&self, usage: u32) -> bool {
        self.xkusage && self.xku & usage == 0
    }
    fn ns_reject(&self, usage: u32) -> bool {
        self.nscert && self.ns & usage == 0
    }

    fn check_ca(&self) -> i32 {
        if self.ku_reject(KU_KEY_CERT_SIGN) {
            0
        } else if self.bcons {
            i32::from(self.ca)
        } else if self.v1 && self.self_signed {
            3
        } else if self.kusage {
            4
        } else if self.nscert && self.ns & NS_ANY_CA != 0 {
            5
        } else {
            0
        }
    }

    fn check_ssl_ca(&self) -> i32 {
        match self.check_ca() {
            0 => 0,
            5 => i32::from(self.ns & NS_SSL_CA != 0),
            _ => 1,
        }
    }

    fn ssl_client(&self, non_leaf: bool) -> i32 {
        if self.xku_reject(XKU_SSL_CLIENT) {
            0
        } else if non_leaf {
            self.check_ssl_ca()
        } else {
            i32::from(
                !self.ku_reject(KU_DIGITAL_SIGNATURE | KU_KEY_AGREEMENT)
                    && !self.ns_reject(NS_SSL_CLIENT),
            )
        }
    }

    fn ssl_server(&self, non_leaf: bool) -> i32 {
        if self.xku_reject(XKU_SSL_SERVER | XKU_SGC) {
            0
        } else if non_leaf {
            self.check_ssl_ca()
        } else {
            i32::from(!self.ns_reject(NS_SSL_SERVER) && !self.ku_reject(KU_TLS))
        }
    }

    fn ns_ssl_server(&self, non_leaf: bool) -> i32 {
        let ret = self.ssl_server(non_leaf);
        if ret == 0 || non_leaf || !self.ku_reject(KU_KEY_ENCIPHERMENT) {
            ret
        } else {
            0
        }
    }

    fn smime(&self, non_leaf: bool) -> i32 {
        if self.xku_reject(XKU_SMIME) {
            return 0;
        }
        if non_leaf {
            let ca = self.check_ca();
            return if ca == 0 || ca == 5 && self.ns & NS_SMIME_CA == 0 {
                0
            } else {
                ca
            };
        }
        if self.nscert {
            if self.ns & NS_SMIME != 0 {
                1
            } else if self.ns & NS_SSL_CLIENT != 0 {
                2
            } else {
                0
            }
        } else {
            1
        }
    }

    fn smime_with(&self, non_leaf: bool, usage: u32) -> i32 {
        let ret = self.smime(non_leaf);
        if ret == 0 || non_leaf || !self.ku_reject(usage) {
            ret
        } else {
            0
        }
    }

    fn crl_sign(&self, non_leaf: bool) -> i32 {
        if non_leaf {
            match self.check_ca() {
                2 => 0,
                other => other,
            }
        } else {
            i32::from(!self.ku_reject(KU_CRL_SIGN))
        }
    }

    fn timestamp_sign(&self, non_leaf: bool, eku_critical: bool) -> i32 {
        if non_leaf {
            return self.check_ca();
        }
        let signing = KU_NON_REPUDIATION | KU_DIGITAL_SIGNATURE;
        let bad_ku = self.kusage && (self.ku & !signing != 0 || self.ku & signing == 0);
        i32::from(!bad_ku && self.xkusage && self.xku == XKU_TIMESTAMP && eku_critical)
    }

    fn code_sign(&self, non_leaf: bool, ku_critical: Option<bool>) -> i32 {
        if non_leaf {
            return self.check_ca();
        }
        i32::from(
            self.kusage
                && self.ku & KU_DIGITAL_SIGNATURE != 0
                && self.ku & (KU_KEY_CERT_SIGN | KU_CRL_SIGN) == 0
                && ku_critical == Some(true)
                && self.xkusage
                && self.xku & XKU_CODE_SIGN != 0
                && self.xku & (XKU_ANYEKU | XKU_SSL_SERVER) == 0,
        )
    }
}

/// Los propósitos estándar (diez en OpenSSL ≥ 3.2) en el orden de
/// `X509_PURPOSE_get0`, limitados a los que conoce la OpenSSL enlazada.
fn compute_purposes(cert: &X509, serial: &[u8], extensions: &[Extension]) -> Vec<Purpose> {
    let flags = cache_extensions(cert, serial, extensions);
    // `X509_get_ext_by_NID` toma la primera aparición.
    let critical = |oid: &str| extensions.iter().find(|e| e.oid == oid).map(|e| e.critical);
    let eku_critical = critical(OID_EXT_KEY_USAGE).unwrap_or(true);
    let ku_critical = critical(OID_KEY_USAGE);
    let check = |id: i32, non_leaf: bool| -> i32 {
        if flags.invalid {
            // `ossl_x509v3_cache_extensions` falla: X509_check_purpose = -1.
            return -1;
        }
        match id {
            1 => flags.ssl_client(non_leaf),
            2 => flags.ssl_server(non_leaf),
            3 => flags.ns_ssl_server(non_leaf),
            4 => flags.smime_with(non_leaf, KU_DIGITAL_SIGNATURE | KU_NON_REPUDIATION),
            5 => flags.smime_with(non_leaf, KU_KEY_ENCIPHERMENT),
            6 => flags.crl_sign(non_leaf),
            8 if non_leaf => flags.check_ca(),
            9 => flags.timestamp_sign(non_leaf, eku_critical),
            10 => flags.code_sign(non_leaf, ku_critical),
            _ => 1,
        }
    };
    [
        "sslclient",
        "sslserver",
        "nssslserver",
        "smimesign",
        "smimeencrypt",
        "crlsign",
        "any",
        "ocsphelper",
        "timestampsign",
        "codesign",
    ]
    .into_iter()
    .zip(1..)
    // La referencia recorre la tabla de la OpenSSL enlazada: `codesign` no existe en
    // versiones anteriores a 3.2 (por ejemplo, la 3.0 de Ubuntu 24.04).
    .filter(|(short_name, _)| X509PurposeRef::get_by_sname(short_name).is_ok())
    .map(|(short_name, id)| Purpose {
        id,
        leaf: check(id, false) != 0,
        ca: check(id, true) != 0,
        short_name,
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Extensiones (`X509V3_EXT_print` y `openssl_x509v3_subjectAltName`).
// ---------------------------------------------------------------------------

enum GeneralName<'a> {
    Other { oid: String, value: Tlv<'a> },
    Email(&'a [u8]),
    Dns(&'a [u8]),
    X400,
    Directory(&'a [u8]),
    EdiParty,
    Uri(&'a [u8]),
    Ip(&'a [u8]),
    Rid(String),
}

fn general_names(content: &[u8]) -> Option<Vec<GeneralName<'_>>> {
    children(content)?
        .into_iter()
        .map(|t| {
            Some(match t.tag {
                0xa0 => {
                    let parts = children(t.content)?;
                    let oid = oid_text(parts.first().filter(|p| p.tag == 0x06)?.content)?;
                    let explicit = parts.get(1).filter(|p| p.tag == 0xa0 && parts.len() == 2)?;
                    GeneralName::Other {
                        oid,
                        value: read_all(explicit.content)?,
                    }
                }
                0x81 => GeneralName::Email(t.content),
                0x82 => GeneralName::Dns(t.content),
                0xa3 => GeneralName::X400,
                0xa4 => GeneralName::Directory(read_all(t.content).filter(|n| n.tag == 0x30)?.raw),
                0xa5 => GeneralName::EdiParty,
                0x86 => GeneralName::Uri(t.content),
                0x87 => GeneralName::Ip(t.content),
                0x88 => GeneralName::Rid(oid_text(t.content)?),
                _ => return None,
            })
        })
        .collect()
}

/// `i2t_ASN1_OBJECT`: nombre largo si OpenSSL lo conoce, si no el OID.
fn object_text(oid: &str) -> String {
    Asn1Object::from_str(oid).map_or_else(|_| oid.to_owned(), |o| o.to_string())
}

/// Clave del arreglo: `OBJ_nid2sn` o el OID si no hay NID.
fn short_name(oid: &str) -> String {
    Asn1Object::from_str(oid)
        .ok()
        .map(|o| o.nid())
        .filter(|nid| *nid != Nid::UNDEF)
        .and_then(|nid| nid.short_name().ok().map(str::to_owned))
        .unwrap_or_else(|| oid.to_owned())
}

/// `%.*s` de `BIO_printf`: corta en el primer NUL.
fn until_nul(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .position(|b| *b == 0)
        .map_or(bytes, |end| bytes.get(..end).unwrap_or(bytes))
}

/// `openssl_x509v3_subjectAltName`: correo, DNS y URI en bruto; el
/// resto con `GENERAL_NAME_print`.
fn subject_alt_name(value: &[u8]) -> Option<Vec<u8>> {
    let tlv = read_all(value).filter(|t| t.tag == 0x30)?;
    let mut out = Vec::new();
    for (index, name) in general_names(tlv.content)?.into_iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(b", ");
        }
        match name {
            GeneralName::Email(raw) => {
                out.extend_from_slice(b"email:");
                out.extend_from_slice(raw);
            }
            GeneralName::Dns(raw) => {
                out.extend_from_slice(b"DNS:");
                out.extend_from_slice(raw);
            }
            GeneralName::Uri(raw) => {
                out.extend_from_slice(b"URI:");
                out.extend_from_slice(raw);
            }
            GeneralName::Other { oid, value } => {
                let srv = oid == "1.3.6.1.5.5.7.8.7";
                let label = match oid.as_str() {
                    "1.3.6.1.5.5.7.8.9" => Some("SmtpUTF8Mailbox"),
                    "1.3.6.1.5.5.7.8.5" => Some("XmppAddr"),
                    "1.3.6.1.5.5.7.8.7" => Some("SRVName"),
                    "1.3.6.1.4.1.311.20.2.3" => Some("UPN"),
                    "1.3.6.1.5.5.7.8.8" => Some("NAIRealm"),
                    _ => None,
                };
                let expected_tag = if srv { 0x16 } else { 0x0c };
                match label.filter(|_| value.tag == expected_tag) {
                    Some(label) => {
                        out.extend_from_slice(format!("othername:{label}:").as_bytes());
                        out.extend_from_slice(until_nul(value.content));
                    }
                    None => out.extend_from_slice(b"othername:<unsupported>"),
                }
            }
            GeneralName::X400 => out.extend_from_slice(b"X400Name:<unsupported>"),
            GeneralName::EdiParty => out.extend_from_slice(b"EdiPartyName:<unsupported>"),
            GeneralName::Directory(der) => {
                out.extend_from_slice(b"DirName:");
                out.extend_from_slice(&name_oneline(der));
            }
            GeneralName::Ip(raw) => {
                out.extend_from_slice(b"IP Address:");
                out.extend_from_slice(ip_text(raw).as_bytes());
            }
            GeneralName::Rid(oid) => {
                out.extend_from_slice(b"Registered ID:");
                out.extend_from_slice(object_text(&oid).as_bytes());
            }
        }
    }
    Some(out)
}

/// `ossl_ipaddr_to_asc`.
fn ip_text(raw: &[u8]) -> String {
    match raw {
        [a, b, c, d] => format!("{a}.{b}.{c}.{d}"),
        _ if raw.len() == 16 => raw
            .chunks(2)
            .map(|pair| match pair {
                [hi, lo] => format!("{:X}", (u16::from(*hi) << 8) | u16::from(*lo)),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(":"),
        _ => format!("<invalid length={}>", raw.len()),
    }
}

/// `char_type` de `crypto/asn1/charmap.h` para ASCII.
const CHAR_TYPE: [u16; 128] = [
    1026, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 120, 0, 1, 40, 0, 0, 0, 16, 1040, 1040, 33792, 25, 25, 16400, 8208, 16, 4112, 4112, 4112,
    4112, 4112, 4112, 4112, 4112, 4112, 4112, 16, 9, 9, 16, 9, 16, 0, 4112, 4112, 4112, 4112, 4112,
    4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112,
    4112, 4112, 4112, 4112, 4112, 0, 1025, 0, 0, 0, 0, 4112, 4112, 4112, 4112, 4112, 4112, 4112,
    4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112, 4112,
    4112, 4112, 4112, 0, 0, 0, 0, 2,
];
const ESC_2253: u16 = 1;
const ESC_CTRL: u16 = 2;
const ESC_MSB: u16 = 4;
const ESC_QUOTE: u16 = 8;
const FIRST_ESC_2253: u16 = 0x20;
const LAST_ESC_2253: u16 = 0x40;
const ESC_2254: u16 = 0x400;
/// Escapes de `XN_FLAG_ONELINE`.
const ONELINE_ESC: u16 = ESC_2253 | ESC_CTRL | ESC_MSB | ESC_QUOTE;
const BS_ESC: u16 = ESC_2253 | FIRST_ESC_2253 | LAST_ESC_2253;

/// `do_esc_char` para un octeto; marca `quotes` si hay que entrecomillar.
fn escape_byte(byte: u8, flags: u16, quotes: &mut bool, out: &mut Vec<u8>) {
    let kind = if byte > 0x7f {
        flags & ESC_MSB
    } else {
        CHAR_TYPE.get(usize::from(byte)).copied().unwrap_or(0) & flags
    };
    if kind & BS_ESC != 0 {
        if kind & ESC_QUOTE != 0 {
            *quotes = true;
            out.push(byte);
        } else {
            out.push(b'\\');
            out.push(byte);
        }
    } else if kind & (ESC_CTRL | ESC_MSB | ESC_2254) != 0 {
        out.extend_from_slice(format!("\\{byte:02X}").as_bytes());
    } else if byte == b'\\' {
        out.extend_from_slice(b"\\\\");
    } else {
        out.push(byte);
    }
}

/// `UTF8_putc`: codifica cualquier valor hasta 0x7FFFFFFF (también
/// sustitutos), como OpenSSL.
fn utf8_putc(code: u32) -> Vec<u8> {
    let tail = |shift: u32| 0x80 | ((code >> shift) & 0x3f) as u8;
    match code {
        0..0x80 => vec![code as u8],
        0x80..0x800 => vec![0xc0 | (code >> 6) as u8, tail(0)],
        0x800..0x1_0000 => vec![0xe0 | (code >> 12) as u8, tail(6), tail(0)],
        0x1_0000..0x20_0000 => vec![0xf0 | (code >> 18) as u8, tail(12), tail(6), tail(0)],
        0x20_0000..0x400_0000 => vec![
            0xf8 | (code >> 24) as u8,
            tail(18),
            tail(12),
            tail(6),
            tail(0),
        ],
        _ => vec![
            0xfc | ((code >> 30) & 0x1) as u8,
            tail(24),
            tail(18),
            tail(12),
            tail(6),
            tail(0),
        ],
    }
}

/// Valor de un atributo con `do_print_ex` y `XN_FLAG_ONELINE`. `false`
/// cuando OpenSSL falla (longitud BMP/Universal inválida) y deja de
/// imprimir el nombre.
fn print_value(value: Tlv<'_>, out: &mut Vec<u8>) -> bool {
    // `tag2nbyte`: UTF8String se recorre octeto a octeto sin convertir; el
    // resto de tipos de texto se convierte a UTF-8.
    let (width, convert) = match value.tag {
        12 => (1, false),
        18 | 19 | 20 | 22 | 23 | 24 | 26 => (1, true),
        28 => (4, true),
        30 => (2, true),
        _ => {
            // Tipo no textual: volcado DER «#…» (ASN1_STRFLGS_DUMP_DER).
            out.push(b'#');
            for byte in value.raw {
                out.extend_from_slice(format!("{byte:02X}").as_bytes());
            }
            return true;
        }
    };
    if value.content.len() % width != 0 {
        return false;
    }
    let chars: Vec<u32> = value
        .content
        .chunks(width)
        .map(|c| c.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)))
        .collect();
    let mut body = Vec::new();
    let mut quotes = false;
    let last = chars.len().saturating_sub(1);
    for (index, &code) in chars.iter().enumerate() {
        let flags = if index == last {
            ONELINE_ESC | LAST_ESC_2253
        } else if index == 0 {
            ONELINE_ESC | FIRST_ESC_2253
        } else {
            ONELINE_ESC
        };
        let bytes = if convert {
            utf8_putc(code)
        } else {
            vec![u8::try_from(code).unwrap_or(0)]
        };
        for byte in bytes {
            escape_byte(byte, flags, &mut quotes, &mut body);
        }
    }
    if quotes {
        out.push(b'"');
        out.extend_from_slice(&body);
        out.push(b'"');
    } else {
        out.extend_from_slice(&body);
    }
    true
}

/// `X509_NAME_print_ex(..., 0, XN_FLAG_ONELINE)`.
fn name_oneline(der: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let Some(rdns) = read_all(der).and_then(|n| children(n.content)) else {
        return out;
    };
    let mut first = true;
    for rdn in rdns {
        for (position, attribute) in children(rdn.content)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            let parts = children(attribute.content).unwrap_or_default();
            let (Some(oid), Some(value)) = (
                parts.first().and_then(|t| oid_text(t.content)),
                parts.get(1).copied(),
            ) else {
                continue;
            };
            if !first {
                out.extend_from_slice(if position > 0 { b" + " } else { b", " });
            }
            first = false;
            out.extend_from_slice(short_name(&oid).as_bytes());
            out.extend_from_slice(b" = ");
            if !print_value(value, &mut out) {
                return out;
            }
        }
    }
    out
}

/// `ASN1_STRING_print`: no imprimibles como `.`, salvo `\n` y `\r`.
fn asn1_string_print(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| {
            if b > b'~' || (b < b' ' && b != b'\n' && b != b'\r') {
                b'.'
            } else {
                b
            }
        })
        .collect()
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

const VALUE_INDENT: &[u8] = b"                ";

/// Valores de `X509V3_EXT_print(bio, ext, 0, 0)`, sacados de `X509_print`
/// (que usa sangría 16). `None` en una posición indica que OpenSSL no la
/// imprime y la referencia usa los bytes en bruto.
fn printed_values(cert: &X509, extensions: &[Extension]) -> Vec<Option<Vec<u8>>> {
    let Ok(text) = cert.to_text() else {
        return vec![None; extensions.len()];
    };
    let headers: Vec<Vec<u8>> = extensions
        .iter()
        .map(|e| {
            format!(
                "            {}: {}\n",
                object_text(&e.oid),
                if e.critical { "critical" } else { "" }
            )
            .into_bytes()
        })
        .collect();
    const SECTION: &[u8] = b"        X509v3 extensions:\n";
    let mut cursor = find(&text, SECTION, 0).map(|p| p + SECTION.len());
    let mut out = Vec::with_capacity(extensions.len());
    for (index, ext) in extensions.iter().enumerate() {
        let header = headers.get(index).map_or(&[][..], Vec::as_slice);
        let start = cursor
            .filter(|&c| text.get(c..c + header.len()) == Some(header))
            .map(|c| c + header.len());
        let end = start.and_then(|s| match headers.get(index + 1) {
            Some(next) => find(&text, &[b"\n", next.as_slice()].concat(), s),
            None => find(&text, b"\n    Signature Algorithm: ", s),
        });
        cursor = end.map(|e| e + 1);
        let value = start
            .zip(end)
            .and_then(|(s, e)| text.get(s..e))
            .filter(|printed| *printed != [VALUE_INDENT, &asn1_string_print(&ext.value)].concat())
            .map(dedent);
        out.push(value);
    }
    out
}

/// Retira la sangría de 16 espacios al inicio de cada línea.
fn dedent(printed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(printed.len());
    for (index, line) in printed.split(|b| *b == b'\n').enumerate() {
        if index > 0 {
            out.push(b'\n');
        }
        out.extend_from_slice(line.strip_prefix(VALUE_INDENT).unwrap_or(line));
    }
    out
}

/// Propósitos y extensiones del arreglo de `openssl_x509_parse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Details {
    pub(crate) purposes: Vec<Purpose>,
    pub(crate) extensions: Vec<(String, Vec<u8>)>,
}

/// `None` cuando la lectura falla para todo el certificado: un
/// `subjectAltName` que no se puede decodificar.
pub(crate) fn details(cert: &X509) -> Option<Details> {
    let der = cert.to_der().ok()?;
    let (serial, extensions) = parse_certificate(&der)?;
    let printed = printed_values(cert, &extensions);
    let mut values: Vec<(String, Vec<u8>)> = Vec::with_capacity(extensions.len());
    for (ext, printed) in extensions.iter().zip(printed) {
        let value = if ext.oid == OID_SAN {
            subject_alt_name(&ext.value)?
        } else {
            printed.unwrap_or_else(|| ext.value.clone())
        };
        let key = short_name(&ext.oid);
        // `add_assoc_stringl`: una clave repetida conserva su posición.
        match values.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => values.push((key, value)),
        }
    }
    Some(Details {
        purposes: compute_purposes(cert, &serial, &extensions),
        extensions: values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_and_oid() {
        assert_eq!(oid_text(&[0x55, 0x1d, 0x13]).as_deref(), Some("2.5.29.19"));
        assert_eq!(
            oid_text(&[0x2b, 0x06, 0x01, 0x04, 0x01, 0x82, 0x37, 0x14, 0x02, 0x03]).as_deref(),
            Some("1.3.6.1.4.1.311.20.2.3")
        );
        assert_eq!(oid_text(&[0x55, 0x9d]), None);
        assert!(read_all(&[0x30, 0x03, 0x01, 0x01]).is_none());
        assert!(read_all(&[0x30, 0x82, 0x00]).is_none());
    }

    #[test]
    fn ip_formats() {
        assert_eq!(ip_text(&[192, 0, 2, 1]), "192.0.2.1");
        let mut v6 = [0u8; 16];
        v6[0] = 0x20;
        v6[1] = 0x01;
        v6[2] = 0x0d;
        v6[3] = 0xb8;
        v6[15] = 1;
        assert_eq!(ip_text(&v6), "2001:DB8:0:0:0:0:0:1");
        assert_eq!(ip_text(&[1, 2]), "<invalid length=2>");
    }

    #[test]
    fn oneline_escaping() {
        let mut out = Vec::new();
        assert!(print_value(
            Tlv {
                tag: 12,
                content: b"A, B",
                raw: b"",
            },
            &mut out,
        ));
        assert_eq!(out, b"\"A, B\"");
        out.clear();
        assert!(print_value(
            Tlv {
                tag: 12,
                content: "Ñ\"\\".as_bytes(),
                raw: b"",
            },
            &mut out,
        ));
        assert_eq!(out, b"\\C3\\91\\\"\\\\");
        out.clear();
        assert!(print_value(
            Tlv {
                tag: 19,
                content: b" x ",
                raw: b"",
            },
            &mut out,
        ));
        assert_eq!(out, b"\" x \"");
        out.clear();
        // BMPString «Ñ» se convierte a UTF-8 y se escapa por octeto.
        assert!(print_value(
            Tlv {
                tag: 30,
                content: &[0x00, 0xd1],
                raw: b"",
            },
            &mut out,
        ));
        assert_eq!(out, b"\\C3\\91");
        assert!(!print_value(
            Tlv {
                tag: 30,
                content: &[0x00],
                raw: b"",
            },
            &mut out,
        ));
        // Un fallo no añade nada.
        assert_eq!(out, b"\\C3\\91");
    }

    #[test]
    fn dedent_keeps_unindented_lines() {
        assert_eq!(
            dedent(b"                Full Name:\n                  URI:x\n"),
            b"Full Name:\n  URI:x\n"
        );
        assert_eq!(dedent(b"                a\nb"), b"a\nb");
    }
}
