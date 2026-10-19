//! Claves y algoritmos de firma y cifrado.
//!
//! La criptografía la realiza OpenSSL (crate `openssl`); aquí solo se
//! reproducen los parámetros por algoritmo, los formatos y los errores
//! observables de la referencia.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use openssl::hash::{MessageDigest, hash};
use openssl::memcmp;
use openssl::pkey::{PKey, Private, Public};
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::Padding;
use openssl::sign::{Signer, Verifier};
use openssl::symm::{Cipher, Crypter, Mode};
use openssl::x509::X509;
use zeroize::{Zeroize, Zeroizing};

use crate::enc::XmlSecEnc;
use crate::error::{Error, openssl_detail, openssl_error};

/// `XMLSecurityKey::TRIPLEDES_CBC`.
pub const TRIPLEDES_CBC: &str = "http://www.w3.org/2001/04/xmlenc#tripledes-cbc";
/// `XMLSecurityKey::AES128_CBC`.
pub const AES128_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes128-cbc";
/// `XMLSecurityKey::AES192_CBC`.
pub const AES192_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes192-cbc";
/// `XMLSecurityKey::AES256_CBC`.
pub const AES256_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes256-cbc";
/// `XMLSecurityKey::RSA_1_5`.
pub const RSA_1_5: &str = "http://www.w3.org/2001/04/xmlenc#rsa-1_5";
/// `XMLSecurityKey::RSA_OAEP_MGF1P` (OAEP con SHA-1 y MGF1-SHA-1).
pub const RSA_OAEP_MGF1P: &str = "http://www.w3.org/2001/04/xmlenc#rsa-oaep-mgf1p";
/// `XMLSecurityKey::DSA_SHA1`: declarado pero sin soporte (`Invalid Key Type`).
pub const DSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#dsa-sha1";
/// `XMLSecurityKey::RSA_SHA1`.
pub const RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
/// `XMLSecurityKey::RSA_SHA256`.
pub const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
/// `XMLSecurityKey::RSA_SHA384`.
pub const RSA_SHA384: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha384";
/// `XMLSecurityKey::RSA_SHA512`.
pub const RSA_SHA512: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha512";
/// `XMLSecurityKey::HMAC_SHA1`.
pub const HMAC_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#hmac-sha1";

/// Uso de una clave asimétrica (`['type' => 'public'|'private']`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Clave pública o certificado.
    Public,
    /// Clave privada.
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Symmetric {
        cipher: SymCipher,
        key_size: usize,
        block_size: usize,
    },
    Asymmetric {
        key_type: KeyType,
        padding: Padding,
        digest: Option<Digest>,
    },
    Hmac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymCipher {
    TripleDes,
    Aes128,
    Aes192,
    Aes256,
}

impl SymCipher {
    fn cipher(self) -> Cipher {
        match self {
            Self::TripleDes => Cipher::des_ede3_cbc(),
            Self::Aes128 => Cipher::aes_128_cbc(),
            Self::Aes192 => Cipher::aes_192_cbc(),
            Self::Aes256 => Cipher::aes_256_cbc(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Digest {
    Sha256,
    Sha384,
    Sha512,
}

/// Material de la clave cargada.
enum Material {
    None,
    Raw(Vec<u8>),
    Public(PKey<Public>),
    Private(PKey<Private>),
    /// `openssl_get_privatekey` falló: se guarda el fallo y el error de
    /// OpenSSL aparece recién al firmar o descifrar.
    FailedPrivate(String),
}

impl Drop for Material {
    fn drop(&mut self) {
        if let Self::Raw(key) = self {
            key.zeroize();
        }
    }
}

impl fmt::Debug for Material {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nunca se muestra el material.
        f.write_str(match self {
            Self::None => "None",
            Self::Raw(_) => "Raw(<oculto>)",
            Self::Public(_) => "Public(<oculto>)",
            Self::Private(_) => "Private(<oculto>)",
            Self::FailedPrivate(_) => "FailedPrivate",
        })
    }
}

/// Resultado de una verificación de firma (`openssl_verify` o HMAC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureCheck {
    /// Firma válida (`1` o `true`).
    Valid,
    /// Firma inválida (`0` o `false`).
    Invalid,
    /// Error de OpenSSL al verificar (`-1`).
    Error(String),
}

/// Clave con su algoritmo (`XMLSecurityKey`).
///
/// `Debug` no muestra la contraseña ni el material de la clave, y al soltarla
/// se borran la contraseña y las claves simétricas.
pub struct XmlSecurityKey {
    algorithm: &'static str,
    class: Class,
    material: Material,
    /// Contraseña de la clave privada (`passphrase`).
    pub passphrase: String,
    /// Último IV usado por el cifrado simétrico (`iv`).
    pub iv: Option<Vec<u8>>,
    /// Nombre de la clave (`name`, `KeyName`).
    pub name: Option<String>,
    /// `true` si la clave está cifrada en un `EncryptedKey` (`isEncrypted`).
    pub is_encrypted: bool,
    /// Contexto de cifrado del `EncryptedKey` (`encryptedCtx`).
    pub encrypted_ctx: Option<Box<XmlSecEnc>>,
    x509_certificate: Option<String>,
    x509_thumbprint: Option<String>,
}

impl fmt::Debug for XmlSecurityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XmlSecurityKey")
            .field("algorithm", &self.algorithm)
            .field("class", &self.class)
            .field("material", &self.material)
            .field("passphrase", &"<oculta>")
            .field("name", &self.name)
            .field("is_encrypted", &self.is_encrypted)
            .field("x509_thumbprint", &self.x509_thumbprint)
            .finish_non_exhaustive()
    }
}

impl Drop for XmlSecurityKey {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

const ALGORITHMS: [&str; 12] = [
    TRIPLEDES_CBC,
    AES128_CBC,
    AES192_CBC,
    AES256_CBC,
    RSA_1_5,
    RSA_OAEP_MGF1P,
    RSA_SHA1,
    RSA_SHA256,
    RSA_SHA384,
    RSA_SHA512,
    HMAC_SHA1,
    DSA_SHA1,
];

impl XmlSecurityKey {
    /// `new XMLSecurityKey($tipo, ['type' => …])`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidKeyType`] si el algoritmo no está soportado (incluido
    /// `DSA_SHA1`) y [`Error::KeyTypeRequired`] si un algoritmo RSA no recibe
    /// `public` o `private`.
    pub fn new(algorithm: &str, key_type: Option<&str>) -> Result<Self, Error> {
        let algorithm: &'static str = ALGORITHMS
            .iter()
            .copied()
            .find(|a| *a == algorithm)
            .ok_or_else(|| Error::InvalidKeyType(algorithm.to_owned()))?;
        let asym = |padding: Padding, digest: Option<Digest>| -> Result<Class, Error> {
            let key_type = match key_type {
                Some("public") => KeyType::Public,
                Some("private") => KeyType::Private,
                _ => return Err(Error::KeyTypeRequired),
            };
            Ok(Class::Asymmetric {
                key_type,
                padding,
                digest,
            })
        };
        let class = match algorithm {
            TRIPLEDES_CBC => Class::Symmetric {
                cipher: SymCipher::TripleDes,
                key_size: 24,
                block_size: 8,
            },
            AES128_CBC => Class::Symmetric {
                cipher: SymCipher::Aes128,
                key_size: 16,
                block_size: 16,
            },
            AES192_CBC => Class::Symmetric {
                cipher: SymCipher::Aes192,
                key_size: 24,
                block_size: 16,
            },
            AES256_CBC => Class::Symmetric {
                cipher: SymCipher::Aes256,
                key_size: 32,
                block_size: 16,
            },
            RSA_1_5 | RSA_SHA1 => asym(Padding::PKCS1, None)?,
            RSA_OAEP_MGF1P => asym(Padding::PKCS1_OAEP, None)?,
            RSA_SHA256 => asym(Padding::PKCS1, Some(Digest::Sha256))?,
            RSA_SHA384 => asym(Padding::PKCS1, Some(Digest::Sha384))?,
            RSA_SHA512 => asym(Padding::PKCS1, Some(Digest::Sha512))?,
            HMAC_SHA1 => Class::Hmac,
            _ => return Err(Error::InvalidKeyType(algorithm.to_owned())),
        };
        Ok(Self {
            algorithm,
            class,
            material: Material::None,
            passphrase: String::new(),
            iv: None,
            name: None,
            is_encrypted: false,
            encrypted_ctx: None,
            x509_certificate: None,
            x509_thumbprint: None,
        })
    }

    /// URI del algoritmo (`getAlgorithm()`; `type`).
    #[must_use]
    pub fn algorithm(&self) -> &'static str {
        self.algorithm
    }

    /// Uso declarado de una clave asimétrica.
    #[must_use]
    pub fn key_type(&self) -> Option<KeyType> {
        match self.class {
            Class::Asymmetric { key_type, .. } => Some(key_type),
            _ => None,
        }
    }

    /// `getSymmetricKeySize()`: tamaño de clave de los cifrados simétricos.
    #[must_use]
    pub fn symmetric_key_size(&self) -> Option<usize> {
        match self.class {
            Class::Symmetric { key_size, .. } => Some(key_size),
            _ => None,
        }
    }

    /// Clave simétrica o HMAC en bruto (`$key->key` como cadena).
    #[must_use]
    pub fn raw_key(&self) -> Option<&[u8]> {
        match &self.material {
            Material::Raw(k) => Some(k),
            _ => None,
        }
    }

    /// Asigna una clave en bruto sin validar (`$key->key = …`).
    pub fn set_raw_key(&mut self, key: Vec<u8>) {
        self.material = Material::Raw(key);
    }

    /// `generateSessionKey()`: clave aleatoria del tamaño del cifrado; para
    /// 3DES ajusta la paridad impar de cada byte.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownKeySize`] si el algoritmo no es simétrico.
    pub fn generate_session_key(&mut self) -> Result<Vec<u8>, Error> {
        let size = self
            .symmetric_key_size()
            .ok_or_else(|| Error::UnknownKeySize(self.algorithm.to_owned()))?;
        let mut key = vec![0u8; size];
        openssl::rand::rand_bytes(&mut key)
            .map_err(|e| openssl_error("Failure generating key - ", &e))?;
        if self.algorithm == TRIPLEDES_CBC {
            for byte in &mut key {
                let mut b = *byte & 0xfe;
                let mut parity = 1u8;
                for j in 1..8 {
                    parity ^= (b >> j) & 1;
                }
                b |= parity;
                *byte = b;
            }
        }
        self.material = Material::Raw(key.clone());
        Ok(key)
    }

    /// `true` si hay una clave privada cargada y lista para firmar.
    pub(crate) fn has_private_key(&self) -> bool {
        matches!(self.material, Material::Private(_))
    }

    /// `loadKey($clave, false, $esCertificado)`.
    ///
    /// # Errors
    ///
    /// [`Error::PublicKeyExtraction`] si no se puede leer una clave pública y
    /// [`Error::SymmetricKeyTooShort`] si una clave simétrica es corta. Una
    /// clave privada inválida no falla aquí, falla al usarla.
    pub fn load_key(&mut self, key: &[u8], is_cert: bool) -> Result<(), Error> {
        // Copia de trabajo: puede contener una clave privada PEM.
        let mut key = Zeroizing::new(key.to_vec());
        // Certificado ya analizado, para no volver a leer el mismo PEM.
        let mut cert: Option<X509> = None;
        if is_cert {
            // openssl_x509_read + openssl_x509_export; si falla, sigue con
            // una clave nula.
            match X509::from_pem(&key).and_then(|c| c.to_pem().map(|pem| (c, pem))) {
                Ok((parsed, pem)) => {
                    self.x509_certificate = Some(String::from_utf8_lossy(&pem).into_owned());
                    key = Zeroizing::new(pem);
                    cert = Some(parsed);
                }
                Err(_) => {
                    self.x509_certificate = None;
                    key = Zeroizing::new(Vec::new());
                }
            }
        } else {
            self.x509_certificate = None;
        }
        match self.class {
            Class::Asymmetric {
                key_type: KeyType::Public,
                ..
            } => {
                if is_cert {
                    self.x509_thumbprint = raw_thumbprint(&String::from_utf8_lossy(&key));
                }
                let public = cert
                    .or_else(|| X509::from_pem(&key).ok())
                    .and_then(|c| c.public_key().ok())
                    .or_else(|| PKey::public_key_from_pem(&key).ok())
                    .ok_or(Error::PublicKeyExtraction)?;
                self.material = Material::Public(public);
            }
            Class::Asymmetric {
                key_type: KeyType::Private,
                ..
            } => {
                self.material =
                    match PKey::private_key_from_pem_passphrase(&key, self.passphrase.as_bytes()) {
                        Ok(private) => Material::Private(private),
                        Err(e) => Material::FailedPrivate(openssl_detail(&e)),
                    };
            }
            Class::Symmetric { key_size, .. } => {
                if key.len() < key_size {
                    return Err(Error::SymmetricKeyTooShort);
                }
                self.material = Material::Raw(key.to_vec());
            }
            Class::Hmac => self.material = Material::Raw(key.to_vec()),
        }
        Ok(())
    }

    /// `encryptData($datos)`.
    ///
    /// # Errors
    ///
    /// Los fallos de OpenSSL con el mensaje de la referencia.
    pub fn encrypt_data(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        match self.class {
            Class::Symmetric {
                cipher,
                block_size,
                key_size,
            } => {
                let cipher = cipher.cipher();
                let mut iv = vec![0u8; cipher.iv_len().unwrap_or(0)];
                openssl::rand::rand_bytes(&mut iv).map_err(|e| {
                    openssl_error("Failure encrypting Data (openssl symmetric) - ", &e)
                })?;
                let padded = pad_iso10126(data, block_size)?;
                let key = Zeroizing::new(
                    self.raw_key()
                        .map(|k| k[..key_size.min(k.len())].to_vec())
                        .unwrap_or_default(),
                );
                let encrypted = crypt(cipher, Mode::Encrypt, &key, &iv, &padded).map_err(|e| {
                    openssl_error("Failure encrypting Data (openssl symmetric) - ", &e)
                })?;
                self.iv = Some(iv.clone());
                Ok([iv, encrypted].concat())
            }
            Class::Asymmetric {
                key_type, padding, ..
            } => {
                let (operation, result) = match (&self.material, key_type) {
                    (Material::Public(k), KeyType::Public) => (
                        "Failure encrypting Data (openssl public) - ",
                        rsa_public(k, data, padding, true),
                    ),
                    (Material::Private(k), KeyType::Private) => (
                        "Failure encrypting Data (openssl private) - ",
                        rsa_private(k, data, padding, true),
                    ),
                    (Material::FailedPrivate(detail), _) => {
                        return Err(Error::OpenSsl {
                            operation: "Failure encrypting Data (openssl private) - ".into(),
                            detail: detail.clone(),
                        });
                    }
                    _ => {
                        return Err(Error::Message(
                            "Failure encrypting Data - clave no cargada".into(),
                        ));
                    }
                };
                result.map_err(|e| openssl_error(operation, &e))
            }
            Class::Hmac => Ok(Vec::new()),
        }
    }

    /// `decryptData($datos)`.
    ///
    /// # Errors
    ///
    /// Los fallos de OpenSSL con el mensaje de la referencia.
    pub fn decrypt_data(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        match self.class {
            Class::Symmetric {
                cipher, key_size, ..
            } => {
                let cipher = cipher.cipher();
                let iv_len = cipher.iv_len().unwrap_or(0).min(data.len());
                let (iv, body) = data.split_at(iv_len);
                self.iv = Some(iv.to_vec());
                let mut iv = iv.to_vec();
                iv.resize(cipher.iv_len().unwrap_or(0), 0);
                let key = Zeroizing::new(
                    self.raw_key()
                        .map(|k| k[..key_size.min(k.len())].to_vec())
                        .unwrap_or_default(),
                );
                let decrypted = crypt(cipher, Mode::Decrypt, &key, &iv, body).map_err(|e| {
                    openssl_error("Failure decrypting Data (openssl symmetric) - ", &e)
                })?;
                Ok(unpad_iso10126(&decrypted))
            }
            Class::Asymmetric {
                key_type, padding, ..
            } => {
                let (operation, result) = match (&self.material, key_type) {
                    (Material::Public(k), KeyType::Public) => (
                        "Failure decrypting Data (openssl public) - ",
                        rsa_public(k, data, padding, false),
                    ),
                    (Material::Private(k), KeyType::Private) => (
                        "Failure decrypting Data (openssl private) - ",
                        rsa_private(k, data, padding, false),
                    ),
                    (Material::FailedPrivate(detail), _) => {
                        return Err(Error::OpenSsl {
                            operation: "Failure decrypting Data (openssl private) - ".into(),
                            detail: detail.clone(),
                        });
                    }
                    _ => {
                        return Err(Error::Message(
                            "Failure decrypting Data - clave no cargada".into(),
                        ));
                    }
                };
                result.map_err(|e| openssl_error(operation, &e))
            }
            Class::Hmac => Ok(Vec::new()),
        }
    }

    fn digest_name(&self) -> (MessageDigest, &'static str) {
        match self.class {
            Class::Asymmetric {
                digest: Some(Digest::Sha256),
                ..
            } => (MessageDigest::sha256(), "SHA256"),
            Class::Asymmetric {
                digest: Some(Digest::Sha384),
                ..
            } => (MessageDigest::sha384(), "SHA384"),
            Class::Asymmetric {
                digest: Some(Digest::Sha512),
                ..
            } => (MessageDigest::sha512(), "SHA512"),
            _ => (MessageDigest::sha1(), "1"),
        }
    }

    /// `signData($datos)`: RSA PKCS#1 v1.5 con el digest del algoritmo, o HMAC-SHA1.
    ///
    /// # Errors
    ///
    /// `Failure Signing Data: …` si la clave no es privada o OpenSSL falla.
    pub fn sign_data(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let (md, name) = self.digest_name();
        let failure = |detail: String| Error::OpenSsl {
            operation: "Failure Signing Data: ".into(),
            detail: format!("{detail} - {name}"),
        };
        match (&self.class, &self.material) {
            (Class::Hmac, Material::Raw(key)) => hmac_sha1(key, data),
            (Class::Hmac, _) => hmac_sha1(&[], data),
            (Class::Asymmetric { .. }, Material::Private(key)) => {
                let mut signer = Signer::new(md, key).map_err(|e| failure(openssl_detail(&e)))?;
                signer
                    .update(data)
                    .map_err(|e| failure(openssl_detail(&e)))?;
                signer
                    .sign_to_vec()
                    .map_err(|e| failure(openssl_detail(&e)))
            }
            (Class::Asymmetric { .. }, Material::FailedPrivate(detail)) => {
                Err(failure(detail.clone()))
            }
            // Se rechaza la clave pública sin error de OpenSSL en la cola.
            (Class::Asymmetric { .. }, Material::Public(_) | Material::None) => {
                Err(failure(String::new()))
            }
            _ => Ok(Vec::new()),
        }
    }

    /// `verifySignature($datos, $firma)`.
    #[must_use]
    pub fn verify_signature(&self, data: &[u8], signature: &[u8]) -> SignatureCheck {
        let (md, _) = self.digest_name();
        match (&self.class, &self.material) {
            (Class::Hmac, material) => {
                let key = match material {
                    Material::Raw(k) => k.as_slice(),
                    _ => &[],
                };
                match hmac_sha1(key, data) {
                    Ok(expected)
                        if expected.len() == signature.len()
                            && memcmp::eq(&expected, signature) =>
                    {
                        SignatureCheck::Valid
                    }
                    _ => SignatureCheck::Invalid,
                }
            }
            (Class::Asymmetric { .. }, Material::Public(key)) => {
                verify_with(md, key, data, signature)
            }
            (Class::Asymmetric { .. }, Material::Private(key)) => {
                verify_with(md, key, data, signature)
            }
            _ => SignatureCheck::Error("clave no cargada".into()),
        }
    }

    /// `loadKey($ruta, true, $esCertificado)`: lee la clave de un archivo.
    ///
    /// # Errors
    ///
    /// [`Error::FileNotFound`] si no se puede leer, o los de [`Self::load_key`].
    pub fn load_key_file(&mut self, path: &std::path::Path, is_cert: bool) -> Result<(), Error> {
        let data = std::fs::read(path).map_err(|_| Error::FileNotFound("Key file not found"))?;
        self.load_key(&data, is_cert)
    }

    /// `true` para HMAC-SHA1 (`verifySignature` devuelve `bool`, no `int`).
    #[must_use]
    pub fn is_hmac(&self) -> bool {
        matches!(self.class, Class::Hmac)
    }

    /// `!empty($clave->key)`: hay material cargado utilizable.
    #[must_use]
    pub fn has_usable_material(&self) -> bool {
        match &self.material {
            Material::Raw(k) => !k.is_empty() && k.as_slice() != b"0",
            Material::Public(_) | Material::Private(_) => true,
            Material::None | Material::FailedPrivate(_) => false,
        }
    }

    /// `getX509Certificate()`: certificado PEM si la clave se cargó desde uno.
    #[must_use]
    pub fn x509_certificate(&self) -> Option<&str> {
        self.x509_certificate.as_deref()
    }

    /// `getX509Thumbprint()`: SHA-1 en hexadecimal del certificado público cargado.
    #[must_use]
    pub fn x509_thumbprint(&self) -> Option<&str> {
        self.x509_thumbprint.as_deref()
    }
}

fn verify_with<T: openssl::pkey::HasPublic>(
    md: MessageDigest,
    key: &PKey<T>,
    data: &[u8],
    signature: &[u8],
) -> SignatureCheck {
    let result = Verifier::new(md, key).and_then(|mut v| {
        v.update(data)?;
        v.verify(signature)
    });
    match result {
        Ok(true) => SignatureCheck::Valid,
        Ok(false) => SignatureCheck::Invalid,
        // openssl_verify devuelve -1 cuando OpenSSL informa un error.
        Err(e) => SignatureCheck::Error(openssl_detail(&e)),
    }
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> Result<Vec<u8>, Error> {
    let key = PKey::hmac(key).map_err(|e| openssl_error("Failure Signing Data: ", &e))?;
    let mut signer = Signer::new(MessageDigest::sha1(), &key)
        .map_err(|e| openssl_error("Failure Signing Data: ", &e))?;
    signer
        .update(data)
        .map_err(|e| openssl_error("Failure Signing Data: ", &e))?;
    signer
        .sign_to_vec()
        .map_err(|e| openssl_error("Failure Signing Data: ", &e))
}

fn crypt(
    cipher: Cipher,
    mode: Mode,
    key: &[u8],
    iv: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    let mut crypter = Crypter::new(cipher, mode, key, Some(iv))?;
    crypter.pad(false);
    let mut out = vec![0u8; data.len() + cipher.block_size()];
    let mut count = crypter.update(data, &mut out)?;
    count += crypter.finalize(&mut out[count..])?;
    out.truncate(count);
    Ok(out)
}

/// Operaciones RSA en bruto por la interfaz EVP, como `openssl_*_encrypt` y
/// `openssl_*_decrypt` de la referencia: cifrar (`EVP_PKEY_encrypt`), descifrar
/// (`EVP_PKEY_decrypt`, con el rechazo implícito de OpenSSL ≥ 3.2 para
/// PKCS#1 v1.5), firmar en bruto (`EVP_PKEY_sign`) y recuperar
/// (`EVP_PKEY_verify_recover`).
fn rsa_public(
    key: &PKey<Public>,
    data: &[u8],
    padding: Padding,
    encrypt: bool,
) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    let mut ctx = PkeyCtx::new(key)?;
    let mut out = Vec::new();
    if encrypt {
        ctx.encrypt_init()?;
        ctx.set_rsa_padding(padding)?;
        ctx.encrypt_to_vec(data, &mut out)?;
    } else {
        ctx.verify_recover_init()?;
        ctx.set_rsa_padding(padding)?;
        let len = ctx.verify_recover(data, None)?;
        out.resize(len, 0);
        let len = ctx.verify_recover(data, Some(&mut out))?;
        out.truncate(len);
    }
    Ok(out)
}

fn rsa_private(
    key: &PKey<Private>,
    data: &[u8],
    padding: Padding,
    encrypt: bool,
) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    let mut ctx = PkeyCtx::new(key)?;
    let mut out = Vec::new();
    if encrypt {
        ctx.sign_init()?;
        ctx.set_rsa_padding(padding)?;
        ctx.sign_to_vec(data, &mut out)?;
    } else {
        ctx.decrypt_init()?;
        ctx.set_rsa_padding(padding)?;
        ctx.decrypt_to_vec(data, &mut out)?;
    }
    Ok(out)
}

/// Relleno de `padISO10126` en la referencia: `n` bytes de valor `n` (como PKCS#7).
fn pad_iso10126(data: &[u8], block_size: usize) -> Result<Vec<u8>, Error> {
    if block_size > 256 {
        return Err(Error::Message(
            "Block size higher than 256 not allowed".into(),
        ));
    }
    let pad = block_size - data.len() % block_size;
    let mut out = data.to_vec();
    out.extend(std::iter::repeat_n(u8::try_from(pad).unwrap_or(0), pad));
    Ok(out)
}

/// `unpadISO10126`: quita tantos bytes como indica el último, **sin validar**
/// el relleno; con `0` o un valor mayor que la longitud devuelve vacío, como
/// `substr($d, 0, -$n)` en la referencia.
fn unpad_iso10126(data: &[u8]) -> Vec<u8> {
    let Some(&last) = data.last() else {
        return Vec::new();
    };
    let n = usize::from(last);
    if n == 0 || n > data.len() {
        return Vec::new();
    }
    data[..data.len() - n].to_vec()
}

/// `getRawThumbprint($pem)`: SHA-1 en minúsculas del primer certificado PEM.
#[must_use]
pub fn raw_thumbprint(cert: &str) -> Option<String> {
    let mut data = String::new();
    let mut in_data = false;
    for line in cert.split('\n') {
        if !in_data {
            if line.starts_with("-----BEGIN CERTIFICATE") {
                in_data = true;
            }
        } else {
            if line.starts_with("-----END CERTIFICATE") {
                break;
            }
            data.push_str(
                line.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B')),
            );
        }
    }
    if data.is_empty() {
        return None;
    }
    let der = lenient_base64_decode(&data);
    let digest = hash(MessageDigest::sha1(), &der).ok()?;
    Some(hex_lower(&digest))
}

/// Hexadecimal en minúsculas.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// Decodificación Base64 sin modo estricto: ignora los caracteres que no
/// pertenecen al alfabeto.
#[must_use]
pub fn lenient_base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).unwrap_or(0));
        }
    }
    out
}

/// `makeAsnSegment($tipo, $cadena)`: segmento DER simple; `None` para
/// longitudes de 64 KiB o más.
#[must_use]
pub fn make_asn_segment(kind: u8, data: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::with_capacity(data.len() + 1);
    match kind {
        0x02 if data.first().is_some_and(|b| *b > 0x7f) => body.push(0),
        0x03 => body.push(0),
        _ => {}
    }
    body.extend_from_slice(data);
    let len = body.len();
    let mut out = vec![kind];
    if len < 128 {
        out.push(u8::try_from(len).ok()?);
    } else if len < 0x100 {
        out.extend([0x81, u8::try_from(len).ok()?]);
    } else if len < 0x10000 {
        out.extend([
            0x82,
            u8::try_from(len / 0x100).ok()?,
            u8::try_from(len % 0x100).ok()?,
        ]);
    } else {
        return None;
    }
    out.extend(body);
    Some(out)
}

/// `convertRSA($modulo, $exponente)`: PEM `PUBLIC KEY` construido a mano.
#[must_use]
pub fn convert_rsa(modulus: &[u8], exponent: &[u8]) -> String {
    let exp = make_asn_segment(0x02, exponent).unwrap_or_default();
    let modu = make_asn_segment(0x02, modulus).unwrap_or_default();
    let seq = make_asn_segment(0x30, &[modu, exp].concat()).unwrap_or_default();
    let bits = make_asn_segment(0x03, &seq).unwrap_or_default();
    let algorithm: [u8; 15] = [
        0x30, 0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];
    let info = make_asn_segment(0x30, &[algorithm.to_vec(), bits].concat()).unwrap_or_default();
    let b64 = STANDARD.encode(info);
    let mut pem = String::from("-----BEGIN PUBLIC KEY-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(chunk));
        pem.push('\n');
    }
    pem.push_str("-----END PUBLIC KEY-----\n");
    pem
}
