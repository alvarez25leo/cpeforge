//! Cifrado XML (XMLEnc).
//!
//! La plantilla `EncryptedData` vive en un documento propio
//! hasta que se importa en el documento de trabajo, que se pasa en cada
//! operación.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::dom::{Document, NodeId, NodeKind};
use crate::dsig::{XMLDSIGNS, XmlSecurityDsig, is_set};
use crate::error::Error;
use crate::key::{XmlSecurityKey, convert_rsa, lenient_base64_decode};
use crate::xpath::{DOUBLE_QUOTE, filter_attr_value};

/// `XMLSecEnc::TEMPLATE`.
pub const TEMPLATE: &str = "<xenc:EncryptedData xmlns:xenc='http://www.w3.org/2001/04/xmlenc#'>\n   <xenc:CipherData>\n      <xenc:CipherValue></xenc:CipherValue>\n   </xenc:CipherData>\n</xenc:EncryptedData>";
/// `XMLSecEnc::ELEMENT`.
pub const ELEMENT: &str = "http://www.w3.org/2001/04/xmlenc#Element";
/// `XMLSecEnc::CONTENT`.
pub const CONTENT: &str = "http://www.w3.org/2001/04/xmlenc#Content";
/// `XMLSecEnc::URI`.
pub const URI: i32 = 3;
/// `XMLSecEnc::XMLENCNS`.
pub const XMLENCNS: &str = "http://www.w3.org/2001/04/xmlenc#";
/// Anidamiento máximo de `EncryptedKey` al resolver `KeyInfo`. Un
/// `RetrievalMethod` puede apuntar a un `EncryptedKey` cuyo `KeyInfo` apunta
/// de vuelta a sí mismo; sin tope la recursión agota la pila.
const MAX_KEY_INFO_DEPTH: usize = 8;
const ENCRYPTED_KEY_TYPE: &str = "http://www.w3.org/2001/04/xmlenc#EncryptedKey";

fn template() -> Document {
    // La plantilla es constante y bien formada.
    Document::parse(TEMPLATE).unwrap_or_default()
}

/// Resultado de [`XmlSecEnc::encrypt_node`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encrypted {
    /// `EncryptedData` importado en el documento de trabajo.
    Node(NodeId),
    /// La plantilla, sin reemplazar (`replace = false`) o porque el nodo
    /// cifrado era el documento; ver [`XmlSecEnc::encrypted_data_xml`].
    Template,
}

/// Resultado de [`XmlSecEnc::decrypt_node`].
#[derive(Debug)]
pub enum Decrypted {
    /// Elemento descifrado que reemplazó a `EncryptedData`.
    Node(NodeId),
    /// Padre del `EncryptedData` reemplazado por el contenido descifrado.
    Parent(NodeId),
    /// Documento nuevo con el elemento descifrado.
    Document(Box<Document>),
    /// Bytes descifrados, sin reemplazar.
    Data(Vec<u8>),
}

#[derive(Debug, Clone)]
struct Reference {
    name: String,
    node: NodeId,
    kind: String,
    encdoc: Document,
    refuri: String,
}

/// Cifrado XML (`XMLSecEnc`).
#[derive(Debug, Clone)]
pub struct XmlSecEnc {
    encdoc: Document,
    raw_node: Option<NodeId>,
    /// `type`: [`ELEMENT`] o [`CONTENT`].
    pub kind: Option<String>,
    /// `encKey`: `EncryptedKey` creado sin anexar (en la plantilla).
    pub enc_key: Option<NodeId>,
    references: Vec<Reference>,
}

impl Default for XmlSecEnc {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlSecEnc {
    /// `new XMLSecEnc()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            encdoc: template(),
            raw_node: None,
            kind: None,
            enc_key: None,
            references: Vec::new(),
        }
    }

    /// Documento de la plantilla `EncryptedData`.
    #[must_use]
    pub fn template_document(&self) -> &Document {
        &self.encdoc
    }

    /// XML del `EncryptedData` de la plantilla.
    #[must_use]
    pub fn encrypted_data_xml(&self) -> String {
        self.encdoc
            .document_element()
            .map(|r| self.encdoc.node_to_xml(r))
            .unwrap_or_default()
    }

    /// `addReference($nombre, $nodo, $tipo)`.
    pub fn add_reference(&mut self, name: &str, node: NodeId, kind: &str) {
        let mut encdoc = template();
        let refuri = XmlSecurityDsig::generate_guid("pfx");
        if let Some(root) = encdoc.document_element() {
            encdoc.set_attribute(root, "Id", &refuri);
        }
        let reference = Reference {
            name: name.to_owned(),
            node,
            kind: kind.to_owned(),
            encdoc,
            refuri,
        };
        match self.references.iter_mut().find(|r| r.name == name) {
            Some(existing) => *existing = reference,
            None => self.references.push(reference),
        }
    }

    /// `setNode($nodo)`.
    pub fn set_node(&mut self, node: NodeId) {
        self.raw_node = Some(node);
    }

    /// Nodo de trabajo (`rawNode`).
    #[must_use]
    pub fn node(&self) -> Option<NodeId> {
        self.raw_node
    }

    /// `encryptNode($clave, $reemplazar)`.
    ///
    /// # Errors
    ///
    /// `Node to encrypt has not been set`, `Type is currently not supported`
    /// y los errores de cifrado de la clave.
    pub fn encrypt_node(
        &mut self,
        doc: &mut Document,
        key: &mut XmlSecurityKey,
        replace: bool,
    ) -> Result<Encrypted, Error> {
        let raw = self
            .raw_node
            .ok_or_else(|| Error::Message("Node to encrypt has not been set".into()))?;
        let root = self.encdoc.document_element().ok_or_else(|| {
            Error::Message("Error locating CipherValue element within template".into())
        })?;
        let cipher_value = self
            .encdoc
            .child_ns(root, XMLENCNS, "CipherData")
            .and_then(|c| self.encdoc.child_ns(c, XMLENCNS, "CipherValue"))
            .ok_or_else(|| {
                Error::Message("Error locating CipherValue element within template".into())
            })?;
        let kind = self.kind.clone().unwrap_or_default();
        let data = match kind.as_str() {
            ELEMENT => {
                self.encdoc.set_attribute(root, "Type", ELEMENT);
                doc.node_to_xml(raw)
            }
            CONTENT => {
                self.encdoc.set_attribute(root, "Type", CONTENT);
                doc.children(raw)
                    .iter()
                    .map(|c| doc.node_to_xml(*c))
                    .collect()
            }
            _ => return Err(Error::Message("Type is currently not supported".into())),
        };
        let method = self
            .encdoc
            .create_element_ns(Some(XMLENCNS), "xenc:EncryptionMethod", None);
        self.encdoc.append_child(root, method);
        self.encdoc
            .set_attribute(method, "Algorithm", key.algorithm());
        let first = self.encdoc.children(root).first().copied();
        self.encdoc.insert_before(root, method, first);
        let encrypted = STANDARD.encode(key.encrypt_data(data.as_bytes())?);
        let text = self.encdoc.create_text(&encrypted);
        self.encdoc.append_child(cipher_value, text);
        if !replace {
            return Ok(Encrypted::Template);
        }
        if matches!(doc.kind(raw), NodeKind::Document) {
            return Ok(Encrypted::Template);
        }
        let imported = doc.import_node(&self.encdoc, root);
        if kind == ELEMENT {
            doc.replace_child(imported, raw);
        } else {
            while let Some(&child) = doc.children(raw).first() {
                doc.remove_child(child);
            }
            doc.append_child(raw, imported);
        }
        Ok(Encrypted::Node(imported))
    }

    /// `encryptReferences($clave)`: cifra cada referencia con su propia plantilla.
    ///
    /// # Errors
    ///
    /// El primer error de [`Self::encrypt_node`].
    pub fn encrypt_references(
        &mut self,
        doc: &mut Document,
        key: &mut XmlSecurityKey,
    ) -> Result<(), Error> {
        let saved = (
            std::mem::take(&mut self.encdoc),
            self.raw_node,
            self.kind.clone(),
        );
        let mut result = Ok(());
        for i in 0..self.references.len() {
            self.encdoc = self.references[i].encdoc.clone();
            self.raw_node = Some(self.references[i].node);
            self.kind = Some(self.references[i].kind.clone());
            if let Err(e) = self.encrypt_node(doc, key, true) {
                result = Err(e);
                break;
            }
            self.references[i].encdoc = self.encdoc.clone();
        }
        self.encdoc = saved.0;
        self.raw_node = saved.1;
        self.kind = saved.2;
        result
    }

    /// `getCipherValue()`: `CipherData/CipherValue` decodificado.
    ///
    /// # Errors
    ///
    /// `Node to decrypt has not been set`.
    pub fn cipher_value(&self, doc: &Document) -> Result<Option<Vec<u8>>, Error> {
        let raw = self
            .raw_node
            .ok_or_else(|| Error::Message("Node to decrypt has not been set".into()))?;
        Ok(doc
            .child_ns(raw, XMLENCNS, "CipherData")
            .and_then(|c| doc.child_ns(c, XMLENCNS, "CipherValue"))
            .map(|v| lenient_base64_decode(&doc.text_content(v))))
    }

    /// `decryptNode($clave, $reemplazar)`.
    ///
    /// # Errors
    ///
    /// `Cannot locate encrypted data`, los de descifrado y los de análisis del
    /// XML descifrado.
    pub fn decrypt_node(
        &self,
        doc: &mut Document,
        key: &mut XmlSecurityKey,
        replace: bool,
    ) -> Result<Decrypted, Error> {
        let data = self
            .cipher_value(doc)?
            .filter(|d| !d.is_empty() && d.as_slice() != b"0");
        let Some(data) = data else {
            return Err(Error::Message("Cannot locate encrypted data".into()));
        };
        let decrypted = key.decrypt_data(&data)?;
        if !replace {
            return Ok(Decrypted::Data(decrypted));
        }
        let raw = self
            .raw_node
            .ok_or_else(|| Error::Message("Node to decrypt has not been set".into()))?;
        let text = String::from_utf8(decrypted.clone()).map_err(|_| {
            Error::Xml(crate::XmlError::Malformed(
                "contenido descifrado no es UTF-8".into(),
            ))
        })?;
        match self.kind.as_deref() {
            Some(ELEMENT) => {
                let newdoc = Document::parse(&text)?;
                if matches!(doc.kind(raw), NodeKind::Document) {
                    return Ok(Decrypted::Document(Box::new(newdoc)));
                }
                let root = newdoc
                    .document_element()
                    .ok_or(Error::Xml(crate::XmlError::Empty))?;
                let imported = doc.import_node(&newdoc, root);
                doc.replace_child(imported, raw);
                Ok(Decrypted::Node(imported))
            }
            Some(CONTENT) => {
                let parent = doc
                    .parent(raw)
                    .ok_or_else(|| Error::Message("EncryptedData sin padre".into()))?;
                let nodes = doc.parse_fragment(parent, &text)?;
                for n in nodes {
                    doc.insert_before(parent, n, Some(raw));
                }
                doc.remove_child(raw);
                Ok(Decrypted::Parent(parent))
            }
            _ => Ok(Decrypted::Data(decrypted)),
        }
    }

    /// `encryptKey($claveTransporte, $claveSesion, $anexar)`.
    ///
    /// # Errors
    ///
    /// Los de cifrado de la clave de transporte.
    pub fn encrypt_key(
        &mut self,
        transport: &mut XmlSecurityKey,
        session: &XmlSecurityKey,
        append: bool,
    ) -> Result<(), Error> {
        let raw = session.raw_key().unwrap_or_default().to_vec();
        let encrypted = STANDARD.encode(transport.encrypt_data(&raw)?);
        let Some(root) = self.encdoc.document_element() else {
            return Ok(());
        };
        let d = &mut self.encdoc;
        let enc_key = d.create_element_ns(Some(XMLENCNS), "xenc:EncryptedKey", None);
        if append {
            let key_info = d.create_element_ns(Some(XMLDSIGNS), "dsig:KeyInfo", None);
            let first = d.children(root).first().copied();
            d.insert_before(root, key_info, first);
            d.append_child(key_info, enc_key);
        } else {
            self.enc_key = Some(enc_key);
        }
        let method = d.create_element_ns(Some(XMLENCNS), "xenc:EncryptionMethod", None);
        d.append_child(enc_key, method);
        d.set_attribute(method, "Algorithm", transport.algorithm());
        if let Some(name) = transport.name.as_deref().filter(|n| is_set(n)) {
            let key_info = d.create_element_ns(Some(XMLDSIGNS), "dsig:KeyInfo", None);
            d.append_child(enc_key, key_info);
            let key_name = d.create_element_ns(Some(XMLDSIGNS), "dsig:KeyName", Some(name));
            d.append_child(key_info, key_name);
        }
        let cipher_data = d.create_element_ns(Some(XMLENCNS), "xenc:CipherData", None);
        d.append_child(enc_key, cipher_data);
        let cipher_value =
            d.create_element_ns(Some(XMLENCNS), "xenc:CipherValue", Some(&encrypted));
        d.append_child(cipher_data, cipher_value);
        if !self.references.is_empty() {
            let list = d.create_element_ns(Some(XMLENCNS), "xenc:ReferenceList", None);
            d.append_child(enc_key, list);
            for r in &self.references {
                let data_ref = d.create_element_ns(Some(XMLENCNS), "xenc:DataReference", None);
                d.append_child(list, data_ref);
                d.set_attribute(data_ref, "URI", &format!("#{}", r.refuri));
            }
        }
        Ok(())
    }

    /// `decryptKey($claveCifrada)`: descifra la clave de sesión del
    /// `EncryptedKey` de este contexto con la clave privada `key`.
    ///
    /// # Errors
    ///
    /// `Key is not Encrypted`, `Key is missing data to perform the decryption`
    /// o los de descifrado.
    pub fn decrypt_key(
        &self,
        doc: &mut Document,
        key: &mut XmlSecurityKey,
    ) -> Result<Vec<u8>, Error> {
        if !key.is_encrypted {
            return Err(Error::Message("Key is not Encrypted".into()));
        }
        if !key.has_usable_material() {
            return Err(Error::Message(
                "Key is missing data to perform the decryption".into(),
            ));
        }
        match self.decrypt_node(doc, key, false)? {
            Decrypted::Data(d) => Ok(d),
            _ => Ok(Vec::new()),
        }
    }

    /// `locateEncryptedData($nodo)`: primer `xenc:EncryptedData` del documento.
    #[must_use]
    pub fn locate_encrypted_data(doc: &Document) -> Option<NodeId> {
        doc.descendants_ns(doc.document_node(), Some(XMLENCNS), "EncryptedData")
            .into_iter()
            .next()
    }

    /// `locateKey($nodo)`: clave privada vacía con el algoritmo del primer
    /// `EncryptionMethod` descendiente.
    #[must_use]
    pub fn locate_key(&self, doc: &Document, node: Option<NodeId>) -> Option<XmlSecurityKey> {
        let node = node.or(self.raw_node)?;
        let method = doc
            .descendants_ns(node, Some(XMLENCNS), "EncryptionMethod")
            .into_iter()
            .next()?;
        let algorithm = doc.attribute(method, "Algorithm").unwrap_or("");
        XmlSecurityKey::new(algorithm, Some("private")).ok()
    }

    /// `staticLocateKeyInfo($claveBase, $nodo)`: aplica `KeyInfo` (nombre,
    /// RSAKeyValue, X509Data) a la clave base, o devuelve la clave de un
    /// `EncryptedKey` referenciado.
    ///
    /// # Errors
    ///
    /// `DSAKeyValue currently not supported`, `Missing Modulus or Exponent`,
    /// `Unable to locate EncryptedKey with @Id='…'.` y los de carga de clave.
    pub fn static_locate_key_info(
        doc: &Document,
        base: Option<XmlSecurityKey>,
        node: NodeId,
    ) -> Result<Option<XmlSecurityKey>, Error> {
        Self::locate_key_info_at(doc, base, node, 0)
    }

    fn locate_key_info_at(
        doc: &Document,
        base: Option<XmlSecurityKey>,
        node: NodeId,
        depth: usize,
    ) -> Result<Option<XmlSecurityKey>, Error> {
        if depth > MAX_KEY_INFO_DEPTH {
            return Err(Error::Xml(crate::XmlError::LimitExceeded(
                "anidamiento de EncryptedKey",
            )));
        }
        let Some(key_info) = doc.child_ns(node, XMLDSIGNS, "KeyInfo") else {
            return Ok(base);
        };
        let mut base = base;
        for &child in doc.children(key_info) {
            match doc.local_name(child) {
                Some("KeyName") => {
                    if let Some(k) = base.as_mut() {
                        k.name = Some(doc.text_content(child));
                    }
                }
                Some("KeyValue") => {
                    for &value in doc.children(child) {
                        match doc.local_name(value) {
                            Some("DSAKeyValue") => {
                                return Err(Error::Message(
                                    "DSAKeyValue currently not supported".into(),
                                ));
                            }
                            Some("RSAKeyValue") => {
                                let modulus = doc
                                    .elements_by_tag_name(value, "Modulus")
                                    .first()
                                    .map(|n| lenient_base64_decode(&doc.text_content(*n)));
                                let exponent = doc
                                    .elements_by_tag_name(value, "Exponent")
                                    .first()
                                    .map(|n| lenient_base64_decode(&doc.text_content(*n)));
                                let (Some(m), Some(e)) = (
                                    modulus.filter(|m| !is_blank_bytes(m)),
                                    exponent.filter(|e| !is_blank_bytes(e)),
                                ) else {
                                    return Err(Error::Message(
                                        "Missing Modulus or Exponent".into(),
                                    ));
                                };
                                let pem = convert_rsa(&m, &e);
                                match base.as_mut() {
                                    Some(k) => k.load_key(pem.as_bytes(), false)?,
                                    None => {
                                        return Err(Error::Message(
                                            "Call to a member function loadKey() on null".into(),
                                        ));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some("RetrievalMethod") => {
                    if doc.attribute(child, "Type").unwrap_or("") != ENCRYPTED_KEY_TYPE {
                        continue;
                    }
                    let uri = doc.attribute(child, "URI").unwrap_or("");
                    let Some(id) = uri.strip_prefix('#') else {
                        continue;
                    };
                    let id_filtered = filter_attr_value(id, DOUBLE_QUOTE);
                    let found = doc
                        .descendants_ns(doc.document_node(), Some(XMLENCNS), "EncryptedKey")
                        .into_iter()
                        .find(|n| doc.attribute_ns(*n, None, "Id") == Some(id_filtered.as_str()));
                    let Some(element) = found else {
                        return Err(Error::Message(format!(
                            "Unable to locate EncryptedKey with @Id='{id}'."
                        )));
                    };
                    return XmlSecurityKey::encrypted_key_at(doc, element, depth + 1).map(Some);
                }
                Some("EncryptedKey") => {
                    return XmlSecurityKey::encrypted_key_at(doc, child, depth + 1).map(Some);
                }
                Some("X509Data") => {
                    if let Some(cert) = doc.elements_by_tag_name(child, "X509Certificate").first() {
                        let body: String = doc
                            .text_content(*cert)
                            .chars()
                            .filter(|c| !matches!(c, '\r' | '\n' | ' '))
                            .collect();
                        let pem = crate::x509::pem_from_body(&body);
                        match base.as_mut() {
                            Some(k) => k.load_key(pem.as_bytes(), true)?,
                            None => {
                                return Err(Error::Message(
                                    "Call to a member function loadKey() on null".into(),
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(base)
    }

    /// `locateKeyInfo($claveBase, $nodo)`.
    ///
    /// # Errors
    ///
    /// Como [`Self::static_locate_key_info`].
    pub fn locate_key_info(
        &self,
        doc: &Document,
        base: Option<XmlSecurityKey>,
        node: Option<NodeId>,
    ) -> Result<Option<XmlSecurityKey>, Error> {
        match node.or(self.raw_node) {
            Some(n) => Self::static_locate_key_info(doc, base, n),
            None => Ok(None),
        }
    }
}

fn is_blank_bytes(b: &[u8]) -> bool {
    b.is_empty() || b == b"0"
}

impl XmlSecurityKey {
    /// `fromEncryptedKeyElement($elemento)`.
    ///
    /// # Errors
    ///
    /// `Unable to locate algorithm for this Encrypted Key` y los de `KeyInfo`.
    pub fn from_encrypted_key_element(doc: &Document, element: NodeId) -> Result<Self, Error> {
        Self::encrypted_key_at(doc, element, 0)
    }

    fn encrypted_key_at(doc: &Document, element: NodeId, depth: usize) -> Result<Self, Error> {
        let mut ctx = XmlSecEnc::new();
        ctx.set_node(element);
        let Some(mut key) = ctx.locate_key(doc, None) else {
            return Err(Error::Message(
                "Unable to locate algorithm for this Encrypted Key".into(),
            ));
        };
        key.is_encrypted = true;
        key.encrypted_ctx = Some(Box::new(ctx));
        // El valor devuelto se ignora; solo se aplica KeyInfo a la clave.
        let located = XmlSecEnc::locate_key_info_at(doc, Some(key), element, depth)?;
        located.ok_or_else(|| {
            Error::Message("Unable to locate algorithm for this Encrypted Key".into())
        })
    }

    /// Descifra la clave de sesión con el contexto `EncryptedKey` de esta
    /// clave (`$clave->encryptedCtx->decryptKey($clave)`).
    ///
    /// # Errors
    ///
    /// Como [`XmlSecEnc::decrypt_key`].
    pub fn decrypt_encrypted_key(&mut self, doc: &mut Document) -> Result<Vec<u8>, Error> {
        let ctx = self
            .encrypted_ctx
            .take()
            .ok_or_else(|| Error::Message("Key is not Encrypted".into()))?;
        let result = ctx.decrypt_key(doc, self);
        self.encrypted_ctx = Some(ctx);
        result
    }
}
