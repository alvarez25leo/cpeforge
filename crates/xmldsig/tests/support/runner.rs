//! Ejecutor Rust de los casos del oráculo: la misma secuencia de llamadas que
//! el oráculo externo (`xmldsig`), con la salida en el mismo formato JSON.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cpeforge_xmldsig::dom::{Document, NodeId, ParseOptions};
use cpeforge_xmldsig::dsig::{
    self, CertOptions, ObjectData, ReferenceOptions, Transform, XmlSecurityDsig,
};
use cpeforge_xmldsig::enc::{Decrypted, Encrypted, XmlSecEnc};
use cpeforge_xmldsig::key::{self, SignatureCheck, XmlSecurityKey};
use cpeforge_xmldsig::sunat::SignedXml;
use cpeforge_xmldsig::x509::{NameValue, X509Certificate, X509ContentType, format_timestamp};
use cpeforge_xmldsig::{Error, XmlError, xpath};
use serde_json::{Map, Value, json};

/// Error del arnés o de la biblioteca, con la clase equivalente.
#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) class: String,
    pub(crate) message: String,
}

type R<T> = Result<T, Failure>;

fn fail(class: &str, message: impl Into<String>) -> Failure {
    Failure {
        class: class.to_owned(),
        message: message.into(),
    }
}

/// Clase de la referencia de cada error de la biblioteca.
fn error_class(e: &Error) -> &'static str {
    match e {
        Error::SignatureNotFound => "UnexpectedValueException",
        Error::MissingPrivateKey | Error::MissingVerificationKey => "RuntimeException",
        Error::UnsupportedSignatureAlgorithm(_) | Error::UnsupportedDigestAlgorithm(_) => {
            "InvalidArgumentException"
        }
        Error::FileNotFound(m) if m.starts_with("Certificate") || m.starts_with("File to sign") => {
            "InvalidArgumentException"
        }
        Error::Xml(XmlError::Empty) => "ValueError",
        Error::Xml(_) => "Rust\\XmlError",
        Error::XPath(_) => "Error",
        _ => "Exception",
    }
}

impl From<Error> for Failure {
    fn from(e: Error) -> Self {
        let message = match &e {
            Error::Xml(XmlError::Empty) => {
                "DOMDocument::loadXML(): Argument #1 ($source) must not be empty".to_owned()
            }
            other => other.to_string(),
        };
        Failure {
            class: error_class(&e).to_owned(),
            message,
        }
    }
}

pub(crate) struct Runner {
    root: PathBuf,
    keys: PathBuf,
    inputs: Option<PathBuf>,
}

/// Texto: cadena UTF-8 o `{"b64": …}`.
fn out_bytes(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(s) => Value::String(s.to_owned()),
        Err(_) => json!({"b64": STANDARD.encode(bytes)}),
    }
}

fn s(v: &Value) -> Option<&str> {
    v.as_str()
}

impl Runner {
    pub(crate) fn new(root: &Path, keys: &Path, inputs: Option<&Path>) -> Self {
        Self {
            root: root.to_owned(),
            keys: keys.to_owned(),
            inputs: inputs.map(Path::to_owned),
        }
    }

    fn key_file(&self, name: &str, suffix: &str) -> R<Vec<u8>> {
        fs::read(self.keys.join(format!("{name}.{suffix}"))).map_err(|_| {
            fail(
                "LogicException",
                format!("material de prueba inexistente: {name}.{suffix}"),
            )
        })
    }

    fn in_bytes(&self, v: &Value) -> R<Vec<u8>> {
        if let Some(b) = v.get("b64").and_then(Value::as_str) {
            return STANDARD
                .decode(b)
                .map_err(|e| fail("LogicException", e.to_string()));
        }
        if let Some(k) = v.get("key").and_then(Value::as_str) {
            return self.key_file(k, v.get("suffix").and_then(Value::as_str).unwrap_or("pem"));
        }
        if let Some(f) = v.get("file").and_then(Value::as_str) {
            return fs::read(self.root.join(f)).map_err(|e| fail("LogicException", e.to_string()));
        }
        if let Some(k) = v.get("key_info_cert").and_then(Value::as_str) {
            let pem = String::from_utf8_lossy(&self.key_file(k, "crt")?).into_owned();
            let body: String = pem
                .lines()
                .filter(|l| !l.starts_with("-----"))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(format!(
                "<root xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\"><t><ds:KeyInfo><ds:X509Data><ds:X509Certificate>\n{}\n</ds:X509Certificate></ds:X509Data></ds:KeyInfo></t></root>",
                body.trim()
            )
            .into_bytes());
        }
        if let Some(r) = v.get("repeat") {
            let count = usize::try_from(r["count"].as_u64().unwrap_or(0)).unwrap_or(0);
            let mut out = s(&r["prefix"]).unwrap_or("").to_owned();
            out.push_str(&s(&r["unit"]).unwrap_or("").repeat(count));
            out.push_str(s(&r["suffix"]).unwrap_or(""));
            return Ok(out.into_bytes());
        }
        if let Some(f) = v.get("input_file").and_then(Value::as_str) {
            let dir = self
                .inputs
                .as_ref()
                .ok_or_else(|| fail("LogicException", "sin directorio de entradas"))?;
            return fs::read(dir.join(f)).map_err(|e| fail("LogicException", e.to_string()));
        }
        match v {
            Value::String(t) => Ok(t.as_bytes().to_vec()),
            Value::Null => Ok(Vec::new()),
            other => Ok(other.to_string().into_bytes()),
        }
    }

    fn in_text(&self, v: &Value) -> R<String> {
        Ok(String::from_utf8_lossy(&self.in_bytes(v)?).into_owned())
    }

    fn load_doc(&self, xml: &[u8], case: &Value) -> R<Document> {
        let mut options = ParseOptions::default();
        if let Some(p) = case.get("preserve_whitespace").and_then(Value::as_bool) {
            options.preserve_whitespace = p;
        }
        // Rechazos que libxml2 también aplica al cargar (mal formado, entidad
        // no declarada, más de 256 niveles) equivalen a `loadXML` = false.
        // Una codificación que libxml2 no conoce también da `loadXML` = false.
        Document::parse_bytes_with(xml, options).map_err(|e| match e {
            XmlError::Empty => Failure::from(Error::Xml(XmlError::Empty)),
            XmlError::UnsupportedEncoding(_)
            | XmlError::Malformed(_)
            | XmlError::UndefinedEntity(_)
            | XmlError::LimitExceeded("profundidad") => {
                fail("RuntimeException", "loadXML devolvió false")
            }
            other => Failure::from(Error::Xml(other)),
        })
    }

    fn new_key(&self, spec: &Value) -> R<XmlSecurityKey> {
        let alg = s(&spec["alg"]).unwrap_or("");
        let mut key = XmlSecurityKey::new(alg, s(&spec["type"]))?;
        if let Some(p) = s(&spec["passphrase"]) {
            p.clone_into(&mut key.passphrase);
        }
        if let Some(n) = s(&spec["name"]) {
            key.name = Some(n.to_owned());
        }
        if let Some(load) = spec.get("load") {
            let data = self.in_bytes(load)?;
            key.load_key(&data, spec["is_cert"].as_bool().unwrap_or(false))?;
        }
        Ok(key)
    }

    fn mutate(xml: &str, mutations: &Value) -> R<String> {
        let mut xml = xml.to_owned();
        for m in mutations.as_array().into_iter().flatten() {
            match s(&m["op"]).unwrap_or("") {
                "replace" => {
                    let from = s(&m["from"]).unwrap_or("");
                    let to = s(&m["to"]).unwrap_or("");
                    let pos = xml.find(from).ok_or_else(|| {
                        fail(
                            "LogicException",
                            format!("mutación sin coincidencia: {from}"),
                        )
                    })?;
                    xml.replace_range(pos..pos + from.len(), to);
                }
                "element_text" => {
                    let tag = s(&m["tag"]).unwrap_or("");
                    let index = m["index"].as_u64().unwrap_or(0);
                    let to = s(&m["to"]).unwrap_or("");
                    xml = replace_element_text(&xml, tag, |n, _| {
                        if n == index {
                            Some(to.to_owned())
                        } else {
                            None
                        }
                    });
                }
                "flip_base64" => {
                    let tag = s(&m["tag"]).unwrap_or("");
                    xml = replace_element_text(&xml, tag, |n, v| {
                        (n == 0).then(|| {
                            let mut b = v.as_bytes().to_vec();
                            if b.len() > 4 {
                                b[4] = if b[4] == b'A' { b'B' } else { b'A' };
                            }
                            String::from_utf8_lossy(&b).into_owned()
                        })
                    });
                }
                other => {
                    return Err(fail(
                        "LogicException",
                        format!("mutación desconocida {other}"),
                    ));
                }
            }
        }
        Ok(xml)
    }

    fn reference_node(transforms: &Value) -> (Document, NodeId) {
        let mut doc = Document::new();
        let reference = doc.create_element_ns(Some(dsig::XMLDSIGNS), "ds:Reference", None);
        let root = doc.document_node();
        doc.append_child(root, reference);
        let trs = doc.create_element_ns(Some(dsig::XMLDSIGNS), "ds:Transforms", None);
        doc.append_child(reference, trs);
        for t in transforms.as_array().into_iter().flatten() {
            let tr = doc.create_element_ns(Some(dsig::XMLDSIGNS), "ds:Transform", None);
            doc.append_child(trs, tr);
            let alg = t.as_str().or_else(|| s(&t["algorithm"])).unwrap_or("");
            doc.set_attribute(tr, "Algorithm", alg);
            if let Some(pl) = s(&t["prefix_list"]) {
                let inc = doc.create_element_ns(
                    Some("http://www.w3.org/2001/10/xml-exc-c14n#"),
                    "ec:InclusiveNamespaces",
                    None,
                );
                doc.set_attribute(inc, "PrefixList", pl);
                doc.append_child(tr, inc);
            }
            if let Some(xp) = s(&t["xpath"]) {
                let x = doc.create_element_ns(Some(dsig::XMLDSIGNS), "ds:XPath", Some(xp));
                doc.append_child(tr, x);
                for (prefix, uri) in t["xpath_ns"].as_object().into_iter().flatten() {
                    let qname = if prefix.is_empty() {
                        "xmlns".to_owned()
                    } else {
                        format!("xmlns:{prefix}")
                    };
                    doc.set_attribute_ns(
                        x,
                        Some("http://www.w3.org/2000/xmlns/"),
                        &qname,
                        uri.as_str().unwrap_or(""),
                    );
                }
            }
        }
        (doc, reference)
    }

    fn dsig_sign(&self, c: &Value) -> R<Document> {
        let mut doc = self.load_doc(&self.in_bytes(&c["xml"])?, c)?;
        let mut d = match c.get("prefix") {
            Some(p) => XmlSecurityDsig::new(Some(p.as_str().unwrap_or(""))),
            None => XmlSecurityDsig::default(),
        };
        if let Some(m) = s(&c["canonical"]) {
            d.set_canonical_method(&mut doc, m)?;
        }
        let default_refs = json!([{"target": "document"}]);
        for r in c
            .get("references")
            .unwrap_or(&default_refs)
            .as_array()
            .into_iter()
            .flatten()
        {
            let node = target(&doc, s(&r["target"]).unwrap_or(""))?;
            let transforms: Option<Vec<Transform>> =
                r.get("transforms").and_then(Value::as_array).map(|list| {
                    list.iter()
                        .map(|t| match t {
                            Value::String(a) => Transform::Algorithm(a.clone()),
                            other => {
                                let x = &other[dsig::XPATH];
                                Transform::XPath {
                                    query: s(&x["query"]).unwrap_or("").to_owned(),
                                    namespaces: x["namespaces"]
                                        .as_object()
                                        .map(|m| {
                                            m.iter()
                                                .map(|(k, v)| {
                                                    (k.clone(), v.as_str().unwrap_or("").to_owned())
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                }
                            }
                        })
                        .collect()
                });
            let options = r.get("options").map(|o| ReferenceOptions {
                prefix: s(&o["prefix"]).map(str::to_owned),
                prefix_ns: s(&o["prefix_ns"]).map(str::to_owned),
                id_name: s(&o["id_name"]).map(str::to_owned),
                overwrite: o.get("overwrite").and_then(Value::as_bool),
                force_uri: o["force_uri"].as_bool().unwrap_or(false),
            });
            d.add_reference(
                &mut doc,
                node,
                s(&r["digest"]).unwrap_or(dsig::SHA1),
                transforms.as_deref(),
                options.as_ref(),
            )?;
        }
        if let Some(o) = s(&c["object"]) {
            d.add_object(
                &mut doc,
                ObjectData::Text(o),
                s(&c["object_mime"]),
                s(&c["object_encoding"]),
            );
        }
        let key = self.new_key(&c["key"])?;
        let append_to = match s(&c["append_to"]) {
            Some(t) => Some(target(&doc, t)?),
            None => None,
        };
        d.sign(&mut doc, &key, append_to)?;
        if let Some(cert) = c.get("cert") {
            let opts = &c["cert_options"];
            let options = CertOptions {
                issuer_serial: opts["issuerSerial"].as_bool().unwrap_or(false),
                subject_name: opts["subjectName"].as_bool().unwrap_or(false),
            };
            d.add_509_cert(&mut doc, &self.in_text(cert)?, true, options);
        }
        if let Some(t) = s(&c["append_signature"]) {
            let node = target(&doc, t)?;
            d.append_signature(
                &mut doc,
                node,
                c["insert_before"].as_bool().unwrap_or(false),
            );
        }
        Ok(doc)
    }

    fn sunat_sign(&self, c: &Value) -> R<Vec<u8>> {
        let mut signer = SignedXml::new();
        if let Some(a) = s(&c["signature_alg"]) {
            signer.set_signature_algorithm(a)?;
        }
        if let Some(a) = s(&c["digest_alg"]) {
            signer.set_digest_algorithm(a)?;
        }
        if let Some(cert) = c.get("cert") {
            signer.set_certificate(&self.in_text(cert)?);
        }
        if let Some(f) = s(&c["cert_file"]) {
            signer.set_certificate_from_file(Path::new(f))?;
        }
        if let Some(f) = s(&c["sign_file"]) {
            return Ok(signer.sign_from_file(Path::new(f))?);
        }
        Ok(signer.sign_xml_bytes(&self.in_bytes(&c["xml"])?)?)
    }

    fn enc_encrypt(&self, c: &Value) -> R<Value> {
        let mut doc = self.load_doc(&self.in_bytes(&c["xml"])?, c)?;
        let mut sym = self.new_key(&c["session"])?;
        if c["generate"].as_bool().unwrap_or(true) {
            sym.generate_session_key()?;
        }
        let mut enc = XmlSecEnc::new();
        enc.set_node(target(&doc, s(&c["target"]).unwrap_or(""))?);
        enc.kind = s(&c["type"]).map(str::to_owned);
        if let Some(t) = c.get("key_transport") {
            let mut transport = self.new_key(t)?;
            enc.encrypt_key(
                &mut transport,
                &sym,
                c["append_key"].as_bool().unwrap_or(true),
            )?;
        }
        let node = enc.encrypt_node(&mut doc, &mut sym, c["replace"].as_bool().unwrap_or(true))?;
        let (document, encrypted) = match node {
            Encrypted::Node(n) => (doc.to_xml(), doc.node_to_xml(n)),
            Encrypted::Template => (doc.to_xml(), enc.encrypted_data_xml()),
        };
        Ok(
            json!({"document": document, "encrypted": encrypted, "session_key": out_bytes(sym.raw_key().unwrap_or_default())}),
        )
    }

    fn enc_decrypt(&self, c: &Value) -> R<Value> {
        let xml = match c.get("input") {
            Some(i) => self.in_text(i)?,
            None => {
                let mut e = c["encrypt"].clone();
                e["scenario"] = json!("enc_encrypt");
                self.enc_encrypt(&e)?["document"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned()
            }
        };
        let mut doc = self.load_doc(xml.as_bytes(), &json!({}))?;
        let Some(data) = XmlSecEnc::locate_encrypted_data(&doc) else {
            return Ok(json!({"located": false}));
        };
        let mut enc = XmlSecEnc::new();
        enc.set_node(data);
        enc.kind = Some(doc.attribute(data, "Type").unwrap_or("").to_owned());
        let key = enc.locate_key(&doc, None);
        let mut r = Map::new();
        r.insert("located".into(), json!(true));
        r.insert("type".into(), json!(enc.kind));
        r.insert(
            "key_algorithm".into(),
            json!(key.as_ref().map(XmlSecurityKey::algorithm)),
        );
        let Some(key) = key else {
            return Ok(Value::Object(r));
        };
        let algorithm = key.algorithm();
        let info = enc.locate_key_info(&doc, Some(key), None)?;
        r.insert(
            "key_info_encrypted".into(),
            json!(info.as_ref().map(|k| k.is_encrypted)),
        );
        let mut key = match info {
            Some(mut info) if info.is_encrypted => {
                let pk = self.in_bytes(&c["private_key"])?;
                info.load_key(&pk, false)?;
                let session = info.decrypt_encrypted_key(&mut doc)?;
                let mut k = XmlSecurityKey::new(algorithm, None)?;
                k.load_key(&session, false)?;
                r.insert("session_key_length".into(), json!(session.len()));
                k
            }
            Some(mut k) => {
                if let Some(sk) = c.get("session_key") {
                    k.load_key(&self.in_bytes(sk)?, false)?;
                }
                k
            }
            None => return Ok(Value::Object(r)),
        };
        let out = enc.decrypt_node(&mut doc, &mut key, c["replace"].as_bool().unwrap_or(true))?;
        let result = match out {
            Decrypted::Document(d) => json!(d.to_xml()),
            Decrypted::Node(_) | Decrypted::Parent(_) => json!(doc.to_xml()),
            Decrypted::Data(d) => out_bytes(&d),
        };
        r.insert("result".into(), result);
        Ok(Value::Object(r))
    }

    fn dsig_verify(&self, c: &Value) -> R<Value> {
        let xml = match c.get("input") {
            Some(i) => self.in_text(i)?,
            None => self.dsig_sign(&c["sign"])?.to_xml(),
        };
        let xml = Self::mutate(&xml, &c["mutations"])?;
        let mut doc = self.load_doc(xml.as_bytes(), &json!({}))?;
        let mut d = XmlSecurityDsig::default();
        if let Some(keys) = c["id_keys"].as_array() {
            d.id_keys = keys
                .iter()
                .filter_map(|k| k.as_str().map(str::to_owned))
                .collect();
        }
        let root = doc.document_node();
        let pos = usize::try_from(c["position"].as_u64().unwrap_or(0)).unwrap_or(0);
        let sig = d.locate_signature(&doc, root, pos);
        let mut r = Map::new();
        r.insert("located".into(), json!(sig.is_some()));
        let Some(sig) = sig else {
            return Ok(Value::Object(r));
        };
        let si = d.canonicalize_signed_info(&doc);
        r.insert(
            "signed_info".into(),
            si.map_or(Value::Null, |s| out_bytes(s.as_bytes())),
        );
        r.insert("ref_ids".into(), json!(d.ref_ids(&doc)?));
        let key = d.locate_key(&doc);
        r.insert(
            "key_algorithm".into(),
            json!(key.as_ref().map(XmlSecurityKey::algorithm)),
        );
        if let Some(mut key) = key {
            let loaded: R<XmlSecurityKey> = (|| {
                if let Some(v) = c.get("verify_with") {
                    key.load_key(
                        &self.in_bytes(v)?,
                        c["verify_is_cert"].as_bool().unwrap_or(true),
                    )?;
                    Ok(key)
                } else {
                    Ok(XmlSecEnc::static_locate_key_info(&doc, Some(key), sig)?
                        .ok_or_else(|| fail("Error", "sin clave"))?)
                }
            })();
            let key = loaded?;
            match d.verify(&doc, &key) {
                Ok(check) => {
                    r.insert("verify".into(), check_value(&key, &check));
                }
                Err(e) => {
                    r.insert("verify_error".into(), error_value(&Failure::from(e)));
                }
            }
        }
        match d.validate_reference(&mut doc) {
            Ok(v) => {
                r.insert("references".into(), json!(v));
                let nodes: Vec<Value> = d
                    .validated_nodes()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let name = match doc.element(v.node) {
                            Some(e) => e.qname(),
                            None => "#document".to_owned(),
                        };
                        let text: String = doc.text_content(v.node).chars().take(40).collect();
                        let text = truncate_bytes(&doc.text_content(v.node), 40).unwrap_or(text);
                        json!({"key": v.id.clone().map_or(json!(i), Value::String), "node": name, "text": text})
                    })
                    .collect();
                r.insert("validated_nodes".into(), Value::Array(nodes));
            }
            Err(e) => {
                r.insert("references_error".into(), error_value(&Failure::from(e)));
            }
        }
        Ok(Value::Object(r))
    }

    pub(crate) fn run_scenario(&self, c: &Value) -> R<Value> {
        let scenario = s(&c["scenario"]).unwrap_or("");
        match scenario {
            "serialize" => {
                let out = self.load_doc(&self.in_bytes(&c["xml"])?, c)?.to_xml_bytes();
                if c["summary"].as_bool().unwrap_or(false) {
                    let digest = openssl::sha::sha256(&out);
                    let hex = digest.iter().fold(String::new(), |mut out, b| {
                        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("{b:02x}"));
                        out
                    });
                    return Ok(json!({"length": out.len(), "sha256": hex}));
                }
                Ok(out_bytes(&out))
            }
            "serialize_node" => {
                let doc = self.load_doc(&self.in_bytes(&c["xml"])?, c)?;
                Ok(json!(
                    doc.node_to_xml(target(&doc, s(&c["target"]).unwrap_or(""))?)
                ))
            }
            "c14n" => {
                let doc = self.load_doc(&self.in_bytes(&c["xml"])?, c)?;
                let (refdoc, reference) = Self::reference_node(&c["transforms"]);
                let node = target(&doc, s(&c["target"]).unwrap_or(""))?;
                let out = XmlSecurityDsig::process_transforms(
                    &refdoc,
                    reference,
                    &doc,
                    Some(node),
                    c["include_comments"].as_bool().unwrap_or(true),
                )?;
                if c["summary"].as_bool().unwrap_or(false) {
                    if let Some(o) = &out {
                        let digest = openssl::sha::sha256(o.as_bytes());
                        let hex = digest.iter().fold(String::new(), |mut acc, b| {
                            let _ = std::fmt::Write::write_fmt(&mut acc, format_args!("{b:02x}"));
                            acc
                        });
                        return Ok(json!({"length": o.len(), "sha256": hex}));
                    }
                }
                Ok(out.map_or(Value::Null, |o| out_bytes(o.as_bytes())))
            }
            "digest" => {
                let data = self.in_bytes(&c["data"])?;
                let alg = s(&c["alg"]).unwrap_or("");
                if c["encode"].as_bool().unwrap_or(true) {
                    Ok(json!(XmlSecurityDsig::calculate_digest_base64(alg, &data)?))
                } else {
                    Ok(out_bytes(&XmlSecurityDsig::calculate_digest(alg, &data)?))
                }
            }
            "xpath_filter" => {
                let value = s(&c["value"]).unwrap_or("");
                Ok(json!(if s(&c["fn"]) == Some("value") {
                    xpath::filter_attr_value(value, s(&c["quotes"]).unwrap_or(xpath::ALL_QUOTES))
                } else {
                    xpath::filter_attr_name(
                        value,
                        s(&c["allow"]).unwrap_or(xpath::EXTENDED_ALPHANUMERIC),
                    )
                }))
            }
            "guid" => {
                let prefix = s(&c["prefix"]).unwrap_or("pfx");
                let g = XmlSecurityDsig::generate_guid(prefix);
                let rest = g.strip_prefix(prefix).unwrap_or("");
                let parts: Vec<&str> = rest.split('-').collect();
                let ok = parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
                    && parts.iter().all(|p| {
                        p.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    });
                Ok(json!({"matches": ok, "length": g.len()}))
            }
            "asn_segment" => {
                let t = u8::try_from(c["type"].as_u64().unwrap_or(0)).unwrap_or(0);
                Ok(key::make_asn_segment(t, &self.in_bytes(&c["data"])?)
                    .map_or(Value::Null, |b| out_bytes(&b)))
            }
            "convert_rsa" => Ok(json!(key::convert_rsa(
                &self.in_bytes(&c["modulus"])?,
                &self.in_bytes(&c["exponent"])?
            ))),
            "x509_certs" => Ok(json!(XmlSecurityDsig::static_get_509_x_certs(
                &self.in_text(&c["text"])?,
                c["pem"].as_bool().unwrap_or(true)
            ))),
            "x509_cert" => Ok(json!(XmlSecurityDsig::get_509_x_cert(
                &self.in_text(&c["text"])?,
                c["pem"].as_bool().unwrap_or(true)
            ))),
            "thumbprint" => Ok(json!(key::raw_thumbprint(&self.in_text(&c["text"])?))),
            "key_info" => {
                let k = self.new_key(&c["key"])?;
                Ok(
                    json!({"algorithm": k.algorithm(), "algorith": k.algorithm(), "type": k.algorithm(), "keysize": k.symmetric_key_size()}),
                )
            }
            "session_key" => {
                let mut k = self.new_key(&c["key"])?;
                let key = k.generate_session_key()?;
                let parity = key.iter().all(|b| b.count_ones() % 2 == 1);
                Ok(
                    json!({"length": key.len(), "odd_parity": parity, "stored": k.raw_key() == Some(key.as_slice())}),
                )
            }
            "symmetric_decrypt" => {
                let mut k = self.new_key(&c["key"])?;
                Ok(out_bytes(&k.decrypt_data(&self.in_bytes(&c["data"])?)?))
            }
            "symmetric_roundtrip" => {
                let mut k = self.new_key(&c["key"])?;
                let ct = k.encrypt_data(&self.in_bytes(&c["data"])?)?;
                let iv = k.iv.clone().unwrap_or_default();
                let prefix = ct.starts_with(&iv);
                Ok(
                    json!({"ciphertext_length": ct.len(), "iv_prefix": prefix, "plaintext": out_bytes(&k.decrypt_data(&ct)?)}),
                )
            }
            "sign_data" => {
                let k = self.new_key(&c["key"])?;
                let sig = k.sign_data(&self.in_bytes(&c["data"])?)?;
                let v = self.new_key(c.get("verify_key").unwrap_or(&c["key"]))?;
                let data = self.in_bytes(c.get("verify_data").unwrap_or(&c["data"]))?;
                let check = v.verify_signature(&data, &sig);
                Ok(json!({"signature": out_bytes(&sig), "verify": check_value(&v, &check)}))
            }
            "rsa_roundtrip" => {
                let mut e = self.new_key(&c["encrypt_key"])?;
                let ct = e.encrypt_data(&self.in_bytes(&c["data"])?)?;
                let mut d = self.new_key(&c["decrypt_key"])?;
                let pt = d.decrypt_data(&ct)?;
                let matches = pt == self.in_bytes(&c["data"])?;
                Ok(
                    json!({"ciphertext_length": ct.len(), "plaintext_matches": matches, "plaintext": if matches { out_bytes(&pt) } else { Value::Null }}),
                )
            }
            "key_x509" => {
                let k = self.new_key(&c["key"])?;
                Ok(json!({"certificate": k.x509_certificate(), "thumbprint": k.x509_thumbprint()}))
            }
            "dsig_sign" => Ok(json!(self.dsig_sign(c)?.to_xml())),
            "dsig_verify" => self.dsig_verify(c),
            "sunat_sign" => Ok(out_bytes(&self.sunat_sign(c)?)),
            "sunat_verify" => {
                let xml = match c.get("input") {
                    Some(i) => self.in_bytes(i)?,
                    None => {
                        let mut sign = c["sign"].clone();
                        sign["scenario"] = json!("sunat_sign");
                        self.sunat_sign(&sign)?
                    }
                };
                // Las mutaciones solo se aplican a documentos UTF-8.
                let xml = if c["mutations"].as_array().is_some_and(|m| !m.is_empty()) {
                    let text = String::from_utf8(xml)
                        .map_err(|_| fail("LogicException", "mutación sobre XML no UTF-8"))?;
                    Self::mutate(&text, &c["mutations"])?.into_bytes()
                } else {
                    xml
                };
                let mut v = SignedXml::new();
                let configured = match c.get("configured_cert") {
                    Some(cc) => {
                        let text = self.in_text(cc)?;
                        v.set_certificate(&text);
                        Some(text)
                    }
                    None => None,
                };
                if let Some(a) = s(&c["signature_alg"]) {
                    v.set_signature_algorithm(a)?;
                }
                let result = if c["dom"].as_bool().unwrap_or(false) {
                    let mut doc = self.load_doc(&xml, &json!({}))?;
                    v.verify(&mut doc)?
                } else {
                    v.verify_xml_bytes(&xml)?
                };
                let is_configured =
                    configured.is_some_and(|t| v.configured_public_key() == Some(t.as_str()));
                Ok(json!({"result": result, "public_key_is_configured": is_configured}))
            }
            "enc_encrypt" => self.enc_encrypt(c),
            "enc_decrypt" => self.enc_decrypt(c),
            "enc_key_info" => {
                let doc = self.load_doc(&self.in_bytes(&c["xml"])?, c)?;
                let node = target(&doc, s(&c["target"]).unwrap_or(""))?;
                let base = match c.get("base_key") {
                    Some(b) => Some(self.new_key(b)?),
                    None => None,
                };
                let k = XmlSecEnc::static_locate_key_info(&doc, base, node)?;
                Ok(k.map_or(Value::Null, |k| {
                    json!({"algorithm": k.algorithm(), "name": k.name, "encrypted": k.is_encrypted, "certificate": k.x509_certificate(), "thumbprint": k.x509_thumbprint()})
                }))
            }
            "x509" => {
                let pfx = self.in_bytes(&c["pfx"])?;
                let password = self.in_text(&c["password"])?;
                let cert = X509Certificate::new(&pfx, &password)?;
                let name_obj = |n: Option<Vec<(String, NameValue)>>| -> Value {
                    n.map_or(Value::Null, |list| {
                        let mut m = Map::new();
                        for (k, v) in list {
                            m.insert(
                                k,
                                match v {
                                    NameValue::Single(s) => json!(s),
                                    NameValue::Multiple(l) => json!(l),
                                },
                            );
                        }
                        Value::Object(m)
                    })
                };
                Ok(json!({
                    "name": cert.name(),
                    "subject": name_obj(cert.subject()),
                    "issuer": name_obj(cert.issuer()),
                    "valid_from": cert.valid_from().map(format_timestamp),
                    "expiration": cert.expiration().map(format_timestamp),
                    // La referencia: arreglo con claves 1..10 (objeto JSON) y arreglo
                    // asociativo de extensiones (`[]` si está vacío).
                    "purposes": cert.purposes().map(|list| {
                        Value::Object(
                            list.iter()
                                .map(|p| (p.id.to_string(), json!([p.leaf, p.ca, p.short_name])))
                                .collect(),
                        )
                    }),
                    "extensions": cert.extensions().map(|list| {
                        if list.is_empty() {
                            json!([])
                        } else {
                            Value::Object(
                                list.iter().map(|(k, v)| (k.clone(), out_bytes(v))).collect(),
                            )
                        }
                    }),
                    "extension_order": cert
                        .extensions()
                        .map(|list| list.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()),
                    "public_key": cert.public_key(),
                    "private_key": cert.private_key(),
                    "raw_equals_input": cert.raw() == pfx.as_slice(),
                    "export_pem": cert.export(X509ContentType::Pem),
                    "export_cer": cert.export(X509ContentType::Cer),
                    "export_other": cert.export(X509ContentType::from_code(99)),
                }))
            }
            other => Err(fail(
                "LogicException",
                format!("escenario desconocido {other}"),
            )),
        }
    }

    /// Ejecuta todos los casos y devuelve `{id: {result|error}}`.
    pub(crate) fn run_all(&self, cases: &Value) -> Map<String, Value> {
        let mut results = Map::new();
        for case in cases["cases"].as_array().into_iter().flatten() {
            let id = s(&case["id"]).unwrap_or("").to_owned();
            let mut out = match self.run_scenario(case) {
                Ok(v) => json!({"result": v}),
                Err(e) => json!({"error": error_value(&e)}),
            };
            if case["openssl_detail_random"].as_bool().unwrap_or(false) {
                if let Some(Value::String(m)) =
                    out.get_mut("error").and_then(|e| e.get_mut("message"))
                {
                    if let Some(start) = m.find("error:") {
                        let end = m[start..].find('-').map_or(m.len(), |i| start + i);
                        m.replace_range(start..end, "<detalle-openssl> ");
                    }
                }
            }
            if case["outcome_only"].as_bool().unwrap_or(false) {
                let recovered = out
                    .get("result")
                    .is_some_and(|r| r.get("plaintext_matches").is_none_or(|m| m == &json!(true)));
                out = json!({"result": {"original_recovered": recovered}});
            }
            results.insert(id, out);
        }
        results
    }
}

fn truncate_bytes(text: &str, n: usize) -> Option<String> {
    // substr de la referencia opera en bytes; se conserva solo si el corte es UTF-8 válido.
    let bytes = text.as_bytes();
    let cut = &bytes[..n.min(bytes.len())];
    std::str::from_utf8(cut).ok().map(str::to_owned)
}

fn check_value(key: &XmlSecurityKey, check: &SignatureCheck) -> Value {
    if key.is_hmac() {
        json!(matches!(check, SignatureCheck::Valid))
    } else {
        json!(match check {
            SignatureCheck::Valid => 1,
            SignatureCheck::Invalid => 0,
            SignatureCheck::Error(_) => -1,
        })
    }
}

pub(crate) fn error_value(e: &Failure) -> Value {
    json!({"class": e.class, "message": e.message})
}

/// Destino: `document`, `root`, `id:VALOR` o `tag:ns|local[n]`.
fn target(doc: &Document, spec: &str) -> R<NodeId> {
    if spec == "document" {
        return Ok(doc.document_node());
    }
    if spec == "root" {
        return doc
            .document_element()
            .ok_or_else(|| fail("LogicException", "sin raíz"));
    }
    if let Some(id) = spec.strip_prefix("id:") {
        return doc
            .descendants(doc.document_node())
            .into_iter()
            .find(|n| doc.attribute_ns(*n, None, "Id") == Some(id))
            .ok_or_else(|| fail("LogicException", format!("destino inexistente {spec}")));
    }
    if let Some(rest) = spec.strip_prefix("tag:") {
        let (ns, rest) = rest
            .split_once('|')
            .ok_or_else(|| fail("LogicException", "tag sin |"))?;
        let (local, index) = match rest.split_once('[') {
            Some((l, i)) => (l, i.trim_end_matches(']').parse::<usize>().unwrap_or(0)),
            None => (rest, 0),
        };
        let ns = if ns.is_empty() { None } else { Some(ns) };
        return doc
            .descendants_ns(doc.document_node(), ns, local)
            .get(index)
            .copied()
            .ok_or_else(|| fail("LogicException", format!("destino inexistente {spec}")));
    }
    Err(fail(
        "LogicException",
        format!("destino desconocido {spec}"),
    ))
}

/// Reemplaza el texto del n-ésimo `<tag …>texto</tag>` (sin hijos).
fn replace_element_text(
    xml: &str,
    tag: &str,
    mut f: impl FnMut(u64, &str) -> Option<String>,
) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::new();
    let mut rest = xml;
    let mut n = 0u64;
    while let Some(pos) = rest.find(&open) {
        let after = &rest[pos + open.len()..];
        let boundary = after.chars().next();
        if !matches!(boundary, Some('>' | ' ' | '\t' | '\n' | '\r')) {
            out.push_str(&rest[..pos + open.len()]);
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        let content_start = pos + open.len() + gt + 1;
        let content = &rest[content_start..];
        let Some(end) = content.find(&close) else {
            break;
        };
        let text = &content[..end];
        if text.contains('<') {
            out.push_str(&rest[..content_start]);
            rest = content;
            continue;
        }
        out.push_str(&rest[..content_start]);
        match f(n, text) {
            Some(new) => out.push_str(&new),
            None => out.push_str(text),
        }
        n += 1;
        rest = &content[end..];
    }
    out.push_str(rest);
    out
}
