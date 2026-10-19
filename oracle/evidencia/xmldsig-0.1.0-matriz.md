# Matriz de inventario y casos — cpeforge-xmldsig 0.1.0

Generada por `oracle/cases/xmldsig/build_matrix.py` a partir del inventario por reflexión de
`thegreenter/xmldsig` v5.1.0 (`oracle/php/xmldsig/inventory.php`), los casos y fixtures de
`tests/reference/xmldsig` y `differences.json`. No editar a mano.

Fuentes PHP inventariadas: 7 archivos de `src/` — `src/Certificate/X509Certificate.php`, `src/Certificate/X509ContentType.php`, `src/Sunat/SignedXml.php`, `src/Util/XPath.php`, `src/XMLSecEnc.php`, `src/XMLSecurityDSig.php`, `src/XMLSecurityKey.php`.

## API pública

| Símbolo PHP | Rust | Estado | Cobertura |
| --- | --- | --- | --- |
| `X509Certificate::__construct` | `X509Certificate::new` | implementado | `x509` |
| `X509Certificate::createFromFile` | `X509Certificate::from_file` | implementado | `api.rs` |
| `X509Certificate::getName` | `X509Certificate::name` | implementado | `x509` |
| `X509Certificate::getSubject` | `X509Certificate::subject` | implementado | `x509` |
| `X509Certificate::getIssuer` | `X509Certificate::issuer (devuelve el sujeto, como PHP)` | implementado | `x509` |
| `X509Certificate::getValidFrom` | `X509Certificate::valid_from` | implementado | `x509` |
| `X509Certificate::getExpiration` | `X509Certificate::expiration` | implementado | `x509` |
| `X509Certificate::getPurposes` | `x509::X509Certificate::purposes` | implementado | `x509` |
| `X509Certificate::getExtensions` | `x509::X509Certificate::extensions` | implementado | `x509` |
| `X509Certificate::getPublicKey` | `X509Certificate::public_key` | implementado | `x509` |
| `X509Certificate::getPrivateKey` | `X509Certificate::private_key` | implementado | `x509` |
| `X509Certificate::getRaw` | `X509Certificate::raw` | implementado | `x509` |
| `X509Certificate::export` | `X509Certificate::export` | implementado | `x509` |
| `X509ContentType::PEM` | `X509ContentType::Pem` | implementado | `x509` |
| `X509ContentType::CER` | `X509ContentType::Cer` | implementado | `x509` |
| `SignedXml::ENVELOPED` | `dsig::ENVELOPED` | implementado | — |
| `SignedXml::EXT_NS` | `sunat::EXT_NS` | implementado | — |
| `SignedXml::signXml` | `SignedXml::sign_xml` | implementado | `sunat_sign` |
| `SignedXml::verifyXml` | `SignedXml::verify_xml` | implementado | `sunat_verify` |
| `SignedXml::setCertificate` | `SignedXml::set_certificate` | implementado | `sunat_sign`, `sunat_verify` |
| `SignedXml::setSignatureAlgorithm` | `SignedXml::set_signature_algorithm` | implementado | `sunat_sign`, `sunat_verify` |
| `SignedXml::setDigestAlgorithm` | `SignedXml::set_digest_algorithm` | implementado | `sunat_sign` |
| `SignedXml::setCertificateFromFile` | `SignedXml::set_certificate_from_file` | implementado | `sunat_sign`, `api.rs` |
| `SignedXml::getPublicKey` | `SignedXml::public_key` | implementado | `sunat_verify` |
| `SignedXml::sign` | `SignedXml::sign` | implementado | `sunat_sign` |
| `SignedXml::signFromFile` | `SignedXml::sign_from_file` | implementado | `sunat_sign`, `api.rs` |
| `SignedXml::verify` | `SignedXml::verify` | implementado | `sunat_verify` |
| `XPath::ALPHANUMERIC` | `constante `ALPHANUMERIC` del módulo` | implementado | — |
| `XPath::NUMERIC` | `constante `NUMERIC` del módulo` | implementado | — |
| `XPath::LETTERS` | `constante `LETTERS` del módulo` | implementado | — |
| `XPath::EXTENDED_ALPHANUMERIC` | `constante `EXTENDED_ALPHANUMERIC` del módulo` | implementado | — |
| `XPath::SINGLE_QUOTE` | `constante `SINGLE_QUOTE` del módulo` | implementado | — |
| `XPath::DOUBLE_QUOTE` | `constante `DOUBLE_QUOTE` del módulo` | implementado | — |
| `XPath::ALL_QUOTES` | `constante `ALL_QUOTES` del módulo` | implementado | — |
| `XPath::filterAttrValue` | `xpath::filter_attr_value` | implementado | `xpath_filter` |
| `XPath::filterAttrName` | `xpath::filter_attr_name` | implementado | `xpath_filter` |
| `XMLSecEnc::TEMPLATE` | `constante `TEMPLATE` del módulo` | implementado | — |
| `XMLSecEnc::ELEMENT` | `constante `ELEMENT` del módulo` | implementado | — |
| `XMLSecEnc::CONTENT` | `constante `CONTENT` del módulo` | implementado | — |
| `XMLSecEnc::URI` | `constante `URI` del módulo` | implementado | — |
| `XMLSecEnc::XMLENCNS` | `constante `XMLENCNS` del módulo` | implementado | — |
| `XMLSecEnc::$type` | `XmlSecEnc::kind` | implementado | `enc_encrypt` |
| `XMLSecEnc::$encKey` | `XmlSecEnc::enc_key` | implementado | `enc_encrypt` |
| `XMLSecEnc::__construct` | `XmlSecEnc::new` | implementado | `enc_encrypt` |
| `XMLSecEnc::addReference` | `XmlSecEnc::add_reference` | implementado | — |
| `XMLSecEnc::setNode` | `XmlSecEnc::set_node` | implementado | `enc_encrypt`, `enc_decrypt` |
| `XMLSecEnc::encryptNode` | `XmlSecEnc::encrypt_node` | implementado | `enc_encrypt` |
| `XMLSecEnc::encryptReferences` | `XmlSecEnc::encrypt_references` | implementado | — |
| `XMLSecEnc::getCipherValue` | `XmlSecEnc::cipher_value` | implementado | `enc_decrypt` |
| `XMLSecEnc::decryptNode` | `XmlSecEnc::decrypt_node` | implementado | `enc_decrypt` |
| `XMLSecEnc::encryptKey` | `XmlSecEnc::encrypt_key` | implementado | `enc_encrypt` |
| `XMLSecEnc::decryptKey` | `XmlSecEnc::decrypt_key` | implementado | `enc_decrypt` |
| `XMLSecEnc::locateEncryptedData` | `XmlSecEnc::locate_encrypted_data` | implementado | `enc_decrypt` |
| `XMLSecEnc::locateKey` | `XmlSecEnc::locate_key` | implementado | `enc_decrypt` |
| `XMLSecEnc::staticLocateKeyInfo` | `XmlSecEnc::static_locate_key_info` | implementado | `enc_key_info`, `dsig_verify` |
| `XMLSecEnc::locateKeyInfo` | `XmlSecEnc::locate_key_info` | implementado | `enc_decrypt` |
| `XMLSecurityDSig::XMLDSIGNS` | `constante `XMLDSIGNS` del módulo` | implementado | — |
| `XMLSecurityDSig::SHA1` | `constante `SHA1` del módulo` | implementado | — |
| `XMLSecurityDSig::SHA256` | `constante `SHA256` del módulo` | implementado | — |
| `XMLSecurityDSig::SHA384` | `constante `SHA384` del módulo` | implementado | — |
| `XMLSecurityDSig::SHA512` | `constante `SHA512` del módulo` | implementado | — |
| `XMLSecurityDSig::RIPEMD160` | `constante `RIPEMD160` del módulo` | implementado | — |
| `XMLSecurityDSig::C14N` | `constante `C14N` del módulo` | implementado | — |
| `XMLSecurityDSig::C14N_COMMENTS` | `constante `C14N_COMMENTS` del módulo` | implementado | — |
| `XMLSecurityDSig::EXC_C14N` | `constante `EXC_C14N` del módulo` | implementado | — |
| `XMLSecurityDSig::EXC_C14N_COMMENTS` | `constante `EXC_C14N_COMMENTS` del módulo` | implementado | — |
| `XMLSecurityDSig::template` | `constante `template` del módulo` | implementado | — |
| `XMLSecurityDSig::BASE_TEMPLATE` | `constante `BASE_TEMPLATE` del módulo` | implementado | — |
| `XMLSecurityDSig::$sigNode` | `XmlSecurityDsig::signature_node / signature_xml` | adaptado | `dsig_verify` |
| `XMLSecurityDSig::$idKeys` | `XmlSecurityDsig::id_keys` | implementado | `dsig_verify` |
| `XMLSecurityDSig::$idNS` | `XmlSecurityDsig::id_ns` | implementado | — |
| `XMLSecurityDSig::__construct` | `XmlSecurityDsig::new / default` | implementado | `dsig_sign` |
| `XMLSecurityDSig::generateGUID` | `XmlSecurityDsig::generate_guid` | implementado | `guid` |
| `XMLSecurityDSig::generate_GUID` | `XmlSecurityDsig::generate_guid` | alias | `guid` |
| `XMLSecurityDSig::locateSignature` | `XmlSecurityDsig::locate_signature` | implementado | `dsig_verify` |
| `XMLSecurityDSig::createNewSignNode` | `XmlSecurityDsig::create_new_sign_node` | implementado | `api.rs` |
| `XMLSecurityDSig::setCanonicalMethod` | `XmlSecurityDsig::set_canonical_method` | implementado | `dsig_sign` |
| `XMLSecurityDSig::canonicalizeSignedInfo` | `XmlSecurityDsig::canonicalize_signed_info` | implementado | `dsig_verify` |
| `XMLSecurityDSig::calculateDigest` | `XmlSecurityDsig::calculate_digest / calculate_digest_base64` | implementado | `digest` |
| `XMLSecurityDSig::validateDigest` | `XmlSecurityDsig::validate_digest` | implementado | `dsig_verify` |
| `XMLSecurityDSig::processTransforms` | `XmlSecurityDsig::process_transforms (XPath con c14n::canonicalize_xpath)` | implementado | `c14n`, `dsig_verify` |
| `XMLSecurityDSig::processRefNode` | `XmlSecurityDsig::process_reference` | implementado | `dsig_verify`, `api.rs` |
| `XMLSecurityDSig::getRefNodeID` | `XmlSecurityDsig::ref_node_id` | implementado | `dsig_verify` |
| `XMLSecurityDSig::getRefIDs` | `XmlSecurityDsig::ref_ids` | implementado | `dsig_verify` |
| `XMLSecurityDSig::validateReference` | `XmlSecurityDsig::validate_reference` | implementado | `dsig_verify`, `sunat_verify` |
| `XMLSecurityDSig::addReference` | `XmlSecurityDsig::add_reference` | implementado | `dsig_sign` |
| `XMLSecurityDSig::addReferenceList` | `XmlSecurityDsig::add_reference_list` | implementado | `dsig_sign` |
| `XMLSecurityDSig::addObject` | `XmlSecurityDsig::add_object` | implementado | `dsig_sign` |
| `XMLSecurityDSig::locateKey` | `XmlSecurityDsig::locate_key` | implementado | `dsig_verify` |
| `XMLSecurityDSig::verify` | `XmlSecurityDsig::verify` | implementado | `dsig_verify` |
| `XMLSecurityDSig::signData` | `XmlSecurityDsig::sign_data` | implementado | `dsig_sign` |
| `XMLSecurityDSig::sign` | `XmlSecurityDsig::sign` | implementado | `dsig_sign` |
| `XMLSecurityDSig::appendCert` | `XmlSecurityDsig::append_cert (vacío, como en PHP)` | implementado | `api.rs` |
| `XMLSecurityDSig::insertSignature` | `XmlSecurityDsig::insert_signature` | implementado | `dsig_sign` |
| `XMLSecurityDSig::appendSignature` | `XmlSecurityDsig::append_signature` | implementado | `dsig_sign` |
| `XMLSecurityDSig::get509XCert` | `XmlSecurityDsig::get_509_x_cert` | implementado | `x509_cert` |
| `XMLSecurityDSig::staticGet509XCerts` | `XmlSecurityDsig::static_get_509_x_certs` | implementado | `x509_certs` |
| `XMLSecurityDSig::staticAdd509Cert` | `XmlSecurityDsig::static_add_509_cert (sin $isURL)` | adaptado | `dsig_sign`, `sunat_sign` |
| `XMLSecurityDSig::add509Cert` | `XmlSecurityDsig::add_509_cert (sin $isURL)` | adaptado | `dsig_sign`, `sunat_sign` |
| `XMLSecurityDSig::appendToKeyInfo` | `XmlSecurityDsig::append_to_key_info` | implementado | `api.rs` |
| `XMLSecurityDSig::getValidatedNodes` | `XmlSecurityDsig::validated_nodes` | implementado | `dsig_verify` |
| `XMLSecurityKey::TRIPLEDES_CBC` | `constante `TRIPLEDES_CBC` del módulo` | implementado | — |
| `XMLSecurityKey::AES128_CBC` | `constante `AES128_CBC` del módulo` | implementado | — |
| `XMLSecurityKey::AES192_CBC` | `constante `AES192_CBC` del módulo` | implementado | — |
| `XMLSecurityKey::AES256_CBC` | `constante `AES256_CBC` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_1_5` | `constante `RSA_1_5` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_OAEP_MGF1P` | `constante `RSA_OAEP_MGF1P` del módulo` | implementado | — |
| `XMLSecurityKey::DSA_SHA1` | `constante `DSA_SHA1` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_SHA1` | `constante `RSA_SHA1` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_SHA256` | `constante `RSA_SHA256` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_SHA384` | `constante `RSA_SHA384` del módulo` | implementado | — |
| `XMLSecurityKey::RSA_SHA512` | `constante `RSA_SHA512` del módulo` | implementado | — |
| `XMLSecurityKey::HMAC_SHA1` | `constante `HMAC_SHA1` del módulo` | implementado | — |
| `XMLSecurityKey::$type` | `XmlSecurityKey::algorithm` | adaptado | `key_info` |
| `XMLSecurityKey::$key` | `XmlSecurityKey::raw_key / set_raw_key (material asimétrico no expuesto)` | adaptado | `session_key` |
| `XMLSecurityKey::$passphrase` | `XmlSecurityKey::passphrase` | implementado | `sign_data` |
| `XMLSecurityKey::$iv` | `XmlSecurityKey::iv` | implementado | `symmetric_roundtrip` |
| `XMLSecurityKey::$name` | `XmlSecurityKey::name` | implementado | `enc_encrypt`, `enc_key_info` |
| `XMLSecurityKey::$keyChain` | `—` | sin uso | — |
| `XMLSecurityKey::$isEncrypted` | `XmlSecurityKey::is_encrypted` | implementado | `enc_decrypt`, `enc_key_info` |
| `XMLSecurityKey::$encryptedCtx` | `XmlSecurityKey::encrypted_ctx / decrypt_encrypted_key` | implementado | `enc_decrypt` |
| `XMLSecurityKey::$guid` | `—` | sin uso | — |
| `XMLSecurityKey::__construct` | `XmlSecurityKey::new` | implementado | `key_info` |
| `XMLSecurityKey::getSymmetricKeySize` | `XmlSecurityKey::symmetric_key_size` | implementado | `key_info` |
| `XMLSecurityKey::generateSessionKey` | `XmlSecurityKey::generate_session_key` | implementado | `session_key` |
| `XMLSecurityKey::getRawThumbprint` | `key::raw_thumbprint` | implementado | `thumbprint` |
| `XMLSecurityKey::loadKey` | `XmlSecurityKey::load_key / load_key_file` | implementado | `key_info`, `sign_data`, `rsa_roundtrip`, `key_x509`, `api.rs` |
| `XMLSecurityKey::encryptData` | `XmlSecurityKey::encrypt_data` | implementado | `symmetric_roundtrip`, `rsa_roundtrip` |
| `XMLSecurityKey::decryptData` | `XmlSecurityKey::decrypt_data` | implementado | `symmetric_decrypt`, `rsa_roundtrip` |
| `XMLSecurityKey::signData` | `XmlSecurityKey::sign_data` | implementado | `sign_data` |
| `XMLSecurityKey::verifySignature` | `XmlSecurityKey::verify_signature` | implementado | `sign_data` |
| `XMLSecurityKey::getAlgorith` | `XmlSecurityKey::algorithm` | alias | `key_info` |
| `XMLSecurityKey::getAlgorithm` | `XmlSecurityKey::algorithm` | implementado | `key_info` |
| `XMLSecurityKey::makeAsnSegment` | `key::make_asn_segment` | implementado | `asn_segment` |
| `XMLSecurityKey::convertRSA` | `key::convert_rsa` | implementado | `convert_rsa` |
| `XMLSecurityKey::getX509Certificate` | `XmlSecurityKey::x509_certificate` | implementado | `key_x509` |
| `XMLSecurityKey::getX509Thumbprint` | `XmlSecurityKey::x509_thumbprint` | implementado | `key_x509` |
| `XMLSecurityKey::fromEncryptedKeyElement` | `XmlSecurityKey::from_encrypted_key_element` | implementado | `enc_key_info`, `enc_decrypt` |

Resumen: 5 adaptado, 2 alias, 129 implementado, 2 sin uso.

Estados: *implementado* (misma semántica); *alias* (método PHP duplicado); *adaptado* (misma
semántica con forma Rust: el DOM se pasa explícito y el material asimétrico no se expone);
*pendiente* (sin implementar); *sin uso* (propiedad
que la referencia declara pero nunca usa). `api.rs` indica cobertura por pruebas de API sin
oráculo.

## Casos por escenario

| Escenario | Casos | Resultados iguales | Errores equivalentes | Diferencias registradas |
| --- | ---: | ---: | ---: | ---: |
| `asn_segment` | 28 | 28 | 0 | 0 |
| `c14n` | 145 | 135 | 9 | 1 |
| `convert_rsa` | 3 | 3 | 0 | 0 |
| `digest` | 35 | 25 | 10 | 0 |
| `dsig_sign` | 30 | 28 | 2 | 0 |
| `dsig_verify` | 18 | 17 | 0 | 1 |
| `enc_decrypt` | 20 | 20 | 0 | 0 |
| `enc_encrypt` | 22 | 20 | 2 | 0 |
| `enc_key_info` | 12 | 9 | 3 | 0 |
| `guid` | 3 | 3 | 0 | 0 |
| `key_info` | 52 | 32 | 20 | 0 |
| `key_x509` | 5 | 4 | 1 | 0 |
| `rsa_roundtrip` | 12 | 9 | 3 | 0 |
| `serialize` | 82 | 61 | 12 | 9 |
| `serialize_node` | 3 | 3 | 0 | 0 |
| `session_key` | 11 | 4 | 7 | 0 |
| `sign_data` | 25 | 22 | 3 | 0 |
| `sunat_sign` | 45 | 34 | 7 | 4 |
| `sunat_verify` | 28 | 20 | 4 | 4 |
| `symmetric_decrypt` | 20 | 12 | 8 | 0 |
| `symmetric_roundtrip` | 20 | 20 | 0 | 0 |
| `thumbprint` | 7 | 7 | 0 | 0 |
| `x509` | 12 | 10 | 1 | 1 |
| `x509_cert` | 7 | 7 | 0 | 0 |
| `x509_certs` | 8 | 8 | 0 | 0 |
| `xpath_filter` | 11 | 11 | 0 | 0 |
| **Total** | **664** | **552** | **92** | **20** |

«Iguales» se comprueba en vivo con las mismas claves efímeras (firmas y XML byte a byte) y offline
con los fixtures redactados.

## Diferencias registradas

| Familia / caso | Categoría | Motivo |
| --- | --- | --- |
| `signing/sunat-sign-malformed` | xml-invalido | XML mal formado (incluye la secuencia ']]>' en contenido): PHP emite un aviso y SignedXml::signXml devuelve un documento VACÍO sin firma; Rust devuelve error. |
| `signing/sunat-sign-special` | xml-invalido | XML mal formado (incluye la secuencia ']]>' en contenido): PHP emite un aviso y SignedXml::signXml devuelve un documento VACÍO sin firma; Rust devuelve error. |
| `signing/sunat-sign-special-sha256` | xml-invalido | XML mal formado (incluye la secuencia ']]>' en contenido): PHP emite un aviso y SignedXml::signXml devuelve un documento VACÍO sin firma; Rust devuelve error. |
| `encryption/x509-not-pfx` | detalle-openssl | Ambos rechazan la entrada; PHP lee el PKCS#12 con d2i_PKCS12_bio y Rust con d2i_PKCS12, que informan un error ASN.1 distinto. |
| `hostile/hostile-serialize-billion-laughs` | politica-entradas | Ambos rechazan: libxml2 por amplificación de entidades; Rust antes, por la presencia del DTD. |
| `hostile/hostile-serialize-dtd-internal-entity` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-serialize-dtd-only` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-serialize-external-entity` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-serialize-external-dtd` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-serialize-parameter-entity` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-sunat-sign-dtd` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-sunat-verify-dtd-added` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-sunat-verify-entity-substitution` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-sunat-verify-external-entity` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-dsig-verify-dtd-id` | politica-entradas | Rust rechaza todo XML con DTD (entidades internas o externas, DTD externo, entidades de parámetro, ATTLIST ID); PHP lo acepta sin expandir entidades ni cargar recursos externos. |
| `hostile/hostile-serialize-huge-11mib` | politica-entradas | Rust limita la entrada a 10 MiB (ParseOptions::max_bytes); PHP acepta 11 MiB. |
| `hostile/hostile-sunat-verify-huge` | politica-entradas | Rust limita la entrada a 10 MiB (ParseOptions::max_bytes); PHP acepta 11 MiB. |
| `hostile/hostile-serialize-many-nodes` | politica-entradas | Rust limita el documento a 1.000.000 de nodos (ParseOptions::max_nodes); PHP lo acepta. |
| `hostile/hostile-c14n-xpath-quadratic` | politica-entradas | Rust limita la evaluación XPath a 10 millones de nodos recorridos por los ejes (expresiones cuadráticas como following:: en cada nodo) y devuelve error; PHP la completa. |
| `encoding/encoding-cyrillic-iso-8859-5` | codificacion-fuera-de-alcance | libxml2 convierte con iconv y PHP acepta cualquier codificación que iconv conozca (ISO-8859-5 en el caso, además de griego, cirílico, asiáticas, EBCDIC…). Rust admite UTF-8, ISO-8859-1, US-ASCII, windows-1252 e ISO-8859-15 y devuelve UnsupportedEncoding con las demás. |

## Suite `.phpt` de la referencia

| Prueba | Título |
| --- | --- |
| `tests/check-sign-xml.phpt` | Check Sign xml with .pem certificate |
| `tests/encrypteddata-node-order.phpt` | Test node order of EncryptedData children. |
| `tests/export-certificate.phpt` | Check export .pfx to .pem |
| `tests/generatesessionkey-basics.phpt` | Basic tests for generateSessionKey(). |
| `tests/generatesessionkey-parity.phpt` | Check that generated triple-des keys have the correct parity. |
| `tests/getcipherdata.phpt` | Test the getCipherData() function. |
| `tests/getsymmetrickeysize.phpt` | Test getSymmetricKeySize(). |
| `tests/retrievalmethod-findkey.phpt` | Test for ds:RetrievalMethod. |
| `tests/sign-c14-comments.phpt` | C14N_COMMENTS signatures. |
| `tests/sign-empty-uri.phpt` | Signature Forcing Empty URI |
| `tests/sign-hhvm-id-wout-ns-regenerated.phpt` | Signing HHVM ID w/out ns regenerated |
| `tests/sunat-sign-default-sha1.phpt` | Sunat SignedXml keeps RSA-SHA1 by default |
| `tests/sunat-sign-sha256.phpt` | Sunat SignedXml with RSA-SHA256 signature and SHA256 digest |
| `tests/sunat-sign-unsupported-algorithm.phpt` | Sunat SignedXml rejects unsupported signature and digest algorithms |
| `tests/sunat-verify-configured-cert.phpt` | Sunat SignedXml verify() with a configured certificate uses the algorithm of the signature |
| `tests/thumbprint.phpt` | Certificate thumbprint check |
| `tests/validate_digest_sha512.phpt` | Validate Digest SHA 512 |
| `tests/withcomment-empty-uri.phpt` | WithComments with empty URI. |
| `tests/withcomment-id-uri-object.phpt` | WithComments with an ID reference. |
| `tests/withcomment-id-uri.phpt` | WithComments with an ID reference. |
| `tests/xml-sign-prefix.phpt` | Basic Signature with no namespace prefix |
| `tests/xml-sign-sha256-rsa-sha256.phpt` | Signature RSA SHA256 |
| `tests/xml-sign-subject.phpt` | Basic Signature With Subject |
| `tests/xml-sign.phpt` | Basic Signature |
| `tests/xmlsec-decrypt-content.phpt` | Basic Decryption: Content |
| `tests/xmlsec-decrypt.phpt` | Basic Decryption |
| `tests/xmlsec-encrypt-content.phpt` | Basic Encryption: Content |
| `tests/xmlsec-encrypt-noreplace.phpt` | Encryption without modifying original data |
| `tests/xmlsec-encrypt.phpt` | Basic Encryption |
| `tests/xmlsec-verify-formatted.phpt` | Verify with formatted X509Certificate |
| `tests/xmlsec-verify-rsa-sha256.phpt` | Verify RSA SHA256 |
| `tests/xmlsec-verify.phpt` | Basic Verify |
