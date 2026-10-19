//! Material criptográfico EFÍMERO para las pruebas, equivalente a
//! `scripts/xmldsig_keys.sh`: se crea en un directorio temporal con permisos
//! 0700 y se borra al soltar [`EphemeralKeys`]. Nunca se imprime.

use std::fs;
use std::path::{Path, PathBuf};

use openssl::asn1::{Asn1Integer, Asn1Object, Asn1OctetString, Asn1Time};
use openssl::bn::BigNum;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkcs12::Pkcs12;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::symm::Cipher;
use openssl::x509::extension::{
    AuthorityKeyIdentifier, BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectKeyIdentifier,
};
use openssl::x509::{X509, X509Extension, X509NameBuilder};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Contraseña sintética del PKCS#12 y de la clave PKCS#8 cifrada.
pub(crate) const PASSWORD: &str = "clave-sintetica-de-prueba";

/// Directorio temporal con las claves; se borra al soltarlo.
pub(crate) struct EphemeralKeys {
    dir: PathBuf,
}

impl EphemeralKeys {
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for EphemeralKeys {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn write(dir: &Path, name: &str, data: &[u8]) -> Result<(), String> {
    let path = dir.join(name);
    fs::write(&path, data).map_err(|e| format!("{name}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn identity(dir: &Path, name: &str, not_before: &str, not_after: &str) -> Result<(), String> {
    let e = |err: openssl::error::ErrorStack| err.to_string();
    let rsa = Rsa::generate(2048).map_err(e)?;
    let pkey = PKey::from_rsa(rsa.clone()).map_err(e)?;
    let mut subject = X509NameBuilder::new().map_err(e)?;
    subject.append_entry_by_text("C", "PE").map_err(e)?;
    subject
        .append_entry_by_text("O", "EMPRESA SINTETICA S.A.C.")
        .map_err(e)?;
    subject
        .append_entry_by_text("CN", "20000000001")
        .map_err(e)?;
    let subject = subject.build();
    let mut builder = X509::builder().map_err(e)?;
    builder.set_version(2).map_err(e)?;
    let serial_bn = BigNum::from_u32(4660).map_err(e)?;
    let serial = Asn1Integer::from_bn(&serial_bn).map_err(e)?;
    builder.set_serial_number(&serial).map_err(e)?;
    builder.set_subject_name(&subject).map_err(e)?;
    builder.set_issuer_name(&subject).map_err(e)?;
    let nb = Asn1Time::from_str(not_before).map_err(e)?;
    builder.set_not_before(&nb).map_err(e)?;
    let na = Asn1Time::from_str(not_after).map_err(e)?;
    builder.set_not_after(&na).map_err(e)?;
    builder.set_pubkey(&pkey).map_err(e)?;
    builder
        .append_extension(BasicConstraints::new().critical().build().map_err(e)?)
        .map_err(e)?;
    builder
        .append_extension(
            KeyUsage::new()
                .critical()
                .digital_signature()
                .non_repudiation()
                .key_encipherment()
                .data_encipherment()
                .build()
                .map_err(e)?,
        )
        .map_err(e)?;
    builder
        .append_extension(
            ExtendedKeyUsage::new()
                .client_auth()
                .email_protection()
                .build()
                .map_err(e)?,
        )
        .map_err(e)?;
    let ski = SubjectKeyIdentifier::new()
        .build(&builder.x509v3_context(None, None))
        .map_err(e)?;
    builder.append_extension(ski).map_err(e)?;
    builder.sign(&pkey, MessageDigest::sha256()).map_err(e)?;
    let cert = builder.build();

    let key_pem = pkey.private_key_to_pem_pkcs8().map_err(e)?;
    let cert_pem = cert.to_pem().map_err(e)?;
    write(dir, &format!("{name}.key"), &key_pem)?;
    write(dir, &format!("{name}.crt"), &cert_pem)?;
    write(
        dir,
        &format!("{name}.pem"),
        &[key_pem.clone(), cert_pem.clone()].concat(),
    )?;
    write(
        dir,
        &format!("{name}.rsa.pem"),
        &rsa.private_key_to_pem().map_err(e)?,
    )?;
    write(
        dir,
        &format!("{name}.pub.pem"),
        &pkey.public_key_to_pem().map_err(e)?,
    )?;
    if name == "a" {
        let pfx = Pkcs12::builder()
            .pkey(&pkey)
            .cert(&cert)
            .build2(PASSWORD)
            .map_err(e)?;
        write(dir, "a.pfx", &pfx.to_der().map_err(e)?)?;
        write(
            dir,
            "a.enc.pem",
            &pkey
                .private_key_to_pem_pkcs8_passphrase(Cipher::aes_256_cbc(), PASSWORD.as_bytes())
                .map_err(e)?,
        )?;
    }
    Ok(())
}

/// Extensión de un perfil `ext-*`: DER fijo (no depende de la clave) o
/// identificador de clave calculado como en `openssl req`.
#[derive(Clone, Copy)]
enum Ext {
    Der(&'static str, bool, &'static str),
    SubjectKeyId,
    /// `authorityKeyIdentifier = keyid:always` (autofirmado).
    AuthorityKeyId,
    /// `authorityKeyIdentifier = keyid, issuer:always`.
    AuthorityKeyIdIssuer,
}

/// DER de las extensiones que genera `scripts/xmldsig_keys.sh` para cada
/// perfil, en el mismo orden (`openssl req` añade SKID por defecto).
const EXT_PROFILES: &[(&str, &[Ext])] = &[
    (
        "ca",
        &[
            Ext::Der("2.5.29.19", true, "30060101ff020101"),
            Ext::Der("2.5.29.15", true, "03020106"),
            Ext::SubjectKeyId,
            Ext::AuthorityKeyId,
        ],
    ),
    (
        "server",
        &[
            Ext::Der("2.5.29.15", false, "030205a0"),
            Ext::Der(
                "2.5.29.37",
                false,
                "302b06082b0601050507030106082b06010505070302060a2b0601040182370a030306092b0601040183b20309",
            ),
            Ext::Der(
                "2.5.29.17",
                false,
                "3082011b82107777772e656a656d706c6f2e74657374820e2a2e656a656d706c6f2e7465737481147573756172696f40656a656d706c6f2e74657374861968747470733a2f2f656a656d706c6f2e746573742f727574618704c0000201871020010db800000000000000000000000188032a0304a020060a2b060104018237140203a0120c1075706e40656a656d706c6f2e74657374a01a06092b0601040183b20303a00d0c0b646573636f6e6f6369646fa46b3069310b3009060355040613025045311d301b060355040a0c14454d50524553412c20532e412e432e3b203c783e3111300f060355040b0c08206573706163696f3112301006035504070c09612b623d635c6422653114301206035504030c0bc383c291616e64c383c2ba",
            ),
            Ext::Der(
                "2.5.29.18",
                false,
                "30158113656d69736f7240656a656d706c6f2e74657374",
            ),
            Ext::SubjectKeyId,
            Ext::AuthorityKeyIdIssuer,
            Ext::Der(
                "2.5.29.31",
                false,
                "30263024a022a020861e687474703a2f2f63726c2e656a656d706c6f2e746573742f63612e63726c",
            ),
            Ext::Der(
                "1.3.6.1.5.5.7.1.1",
                false,
                "3051302406082b060105050730018618687474703a2f2f6f6373702e656a656d706c6f2e74657374302906082b06010505073002861d687474703a2f2f63612e656a656d706c6f2e746573742f63612e636572",
            ),
            Ext::Der(
                "2.5.29.32",
                false,
                "30763008060667810c010201306a06092b0601040183b2030a305d302406082b06010505070201161868747470733a2f2f656a656d706c6f2e746573742f637073303506082b06010505070202302930161a0c4f7267616e697a6163696f6e30060201010201021a0f417669736f2073696e74657469636f",
            ),
            Ext::Der("2.16.840.1.113730.1.1", false, "030206c0"),
            Ext::Der(
                "2.16.840.1.113730.1.13",
                false,
                "1614636f6d656e746172696f2073696e74657469636f",
            ),
            Ext::Der("1.3.6.1.4.1.55555.1", false, "0c04686f6c61"),
            Ext::Der("1.3.6.1.4.1.55555.2", false, "ff00fe"),
        ],
    ),
    (
        "tsa",
        &[
            Ext::Der("2.5.29.19", false, "3000"),
            Ext::Der("2.5.29.15", true, "03020780"),
            Ext::Der("2.5.29.37", true, "300a06082b06010505070308"),
            Ext::SubjectKeyId,
        ],
    ),
    (
        "codesign",
        &[
            Ext::Der("2.5.29.15", true, "03020780"),
            Ext::Der("2.5.29.37", false, "300a06082b06010505070303"),
            Ext::SubjectKeyId,
        ],
    ),
    (
        "nsca",
        &[
            Ext::Der("2.16.840.1.113730.1.1", false, "03020007"),
            Ext::SubjectKeyId,
        ],
    ),
    (
        "kuonly",
        &[Ext::Der("2.5.29.15", false, "03020204"), Ext::SubjectKeyId],
    ),
    (
        "invalid",
        &[Ext::Der("2.5.29.15", false, "0500"), Ext::SubjectKeyId],
    ),
    (
        "saninvalid",
        &[Ext::Der("2.5.29.17", false, "0500"), Ext::SubjectKeyId],
    ),
    // Versión 1, sin extensiones.
    ("v1", &[]),
];

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .filter_map(|i| {
            text.get(i..i + 2)
                .and_then(|b| u8::from_str_radix(b, 16).ok())
        })
        .collect()
}

/// Perfil `ext-<name>` con clave EC P-256, firmado por sí mismo o por `ca`.
fn ext_identity(
    dir: &Path,
    name: &str,
    extensions: &[Ext],
    issuer: Option<(&X509, &PKey<Private>)>,
) -> Result<(X509, PKey<Private>), String> {
    let e = |err: openssl::error::ErrorStack| err.to_string();
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(e)?;
    let pkey = PKey::from_ec_key(EcKey::generate(&group).map_err(e)?).map_err(e)?;
    let mut subject = X509NameBuilder::new().map_err(e)?;
    subject.append_entry_by_text("C", "PE").map_err(e)?;
    subject
        .append_entry_by_text("O", "EMPRESA SINTETICA S.A.C.")
        .map_err(e)?;
    subject
        .append_entry_by_text("CN", "20000000001")
        .map_err(e)?;
    let subject = subject.build();
    let mut builder = X509::builder().map_err(e)?;
    builder
        .set_version(if extensions.is_empty() { 0 } else { 2 })
        .map_err(e)?;
    let serial = match (name, issuer) {
        ("v1", _) => 4662,
        (_, Some(_)) => 4661,
        _ => 4660,
    };
    let serial = BigNum::from_u32(serial).map_err(e)?;
    let serial = Asn1Integer::from_bn(&serial).map_err(e)?;
    builder.set_serial_number(&serial).map_err(e)?;
    builder.set_subject_name(&subject).map_err(e)?;
    builder
        .set_issuer_name(issuer.map_or(&*subject, |(cert, _)| cert.subject_name()))
        .map_err(e)?;
    let not_before = Asn1Time::from_str("20240101000000Z").map_err(e)?;
    builder.set_not_before(&not_before).map_err(e)?;
    let not_after = Asn1Time::from_str("20340101000000Z").map_err(e)?;
    builder.set_not_after(&not_after).map_err(e)?;
    builder.set_pubkey(&pkey).map_err(e)?;
    for ext in extensions {
        let built = match *ext {
            Ext::Der(oid, critical, hex) => {
                let oid = Asn1Object::from_str(oid).map_err(e)?;
                let value = Asn1OctetString::new_from_bytes(&unhex(hex)).map_err(e)?;
                X509Extension::new_from_der(&oid, critical, &value)
            }
            Ext::SubjectKeyId => {
                SubjectKeyIdentifier::new().build(&builder.x509v3_context(None, None))
            }
            Ext::AuthorityKeyId => AuthorityKeyIdentifier::new()
                .keyid(true)
                .build(&builder.x509v3_context(None, None)),
            Ext::AuthorityKeyIdIssuer => AuthorityKeyIdentifier::new()
                .keyid(false)
                .issuer(true)
                .build(&builder.x509v3_context(issuer.map(|(cert, _)| &**cert), None)),
        }
        .map_err(e)?;
        builder.append_extension(built).map_err(e)?;
    }
    builder
        .sign(
            issuer.map_or(&pkey, |(_, key)| key),
            MessageDigest::sha256(),
        )
        .map_err(e)?;
    let cert = builder.build();
    let pfx = Pkcs12::builder()
        .pkey(&pkey)
        .cert(&cert)
        .build2(PASSWORD)
        .map_err(e)?;
    write(dir, &format!("ext-{name}.pfx"), &pfx.to_der().map_err(e)?)?;
    Ok((cert, pkey))
}

/// Genera el juego completo en un directorio temporal nuevo.
pub(crate) fn generate() -> Result<EphemeralKeys, String> {
    let mut rand = [0u8; 8];
    openssl::rand::rand_bytes(&mut rand).map_err(|e| e.to_string())?;
    let suffix = hex(&rand);
    let dir = std::env::temp_dir().join(format!("xmldsig-keys-{suffix}"));
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let keys = EphemeralKeys { dir };
    fill(keys.dir())?;
    Ok(keys)
}

/// Escribe el juego completo en `dir` (existente), sin borrarlo al terminar.
/// Lo usa `scripts/xmldsig_keys.sh` cuando la CLI `openssl` no admite
/// `-not_before`/`-x509v1` (OpenSSL < 3.4, como en Ubuntu 24.04).
pub(crate) fn fill(dir: &Path) -> Result<(), String> {
    identity(dir, "a", "20240101000000Z", "20340101000000Z")?;
    identity(dir, "b", "20240101000000Z", "20340101000000Z")?;
    identity(dir, "expired", "20200101000000Z", "20210101000000Z")?;
    write(dir, "pfx.pass", PASSWORD.as_bytes())?;
    let mut ca = None;
    for (name, extensions) in EXT_PROFILES {
        let issuer = if *name == "server" {
            ca.as_ref()
                .map(|(cert, key): &(X509, PKey<Private>)| (cert, key))
        } else {
            None
        };
        let identity = ext_identity(dir, name, extensions, issuer)?;
        if *name == "ca" {
            ca = Some(identity);
        }
    }
    let mut hmac = [0u8; 32];
    openssl::rand::rand_bytes(&mut hmac).map_err(|e| e.to_string())?;
    let hex = hex(&hmac);
    write(dir, "hmac.key", hex.as_bytes())?;
    Ok(())
}
