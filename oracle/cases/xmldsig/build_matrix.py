#!/usr/bin/env python3
"""Genera oracle/evidencia/xmldsig-0.1.0-matriz.md.

Combina el inventario por reflexión de la API PHP (oracle/php/xmldsig/
inventory.php), la correspondencia con la API Rust (tabla RUST de este
archivo), los casos y fixtures de tests/reference/xmldsig y el registro de
diferencias.

Uso: php -n oracle/php/xmldsig/inventory.php > inv.json
     python3 oracle/cases/xmldsig/build_matrix.py inv.json > oracle/evidencia/xmldsig-0.1.0-matriz.md
"""

from __future__ import annotations

import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REF = ROOT / "tests/reference/xmldsig"

# Símbolo PHP → (símbolo Rust, estado, escenarios del oráculo que lo ejercitan).
# Estados: implementado | alias | adaptado (misma semántica, forma Rust) |
# parcial | pendiente | omitido (decisión de seguridad) | sin uso (la referencia
# no lo usa).
RUST = {
    # SignedXml
    "SignedXml::ENVELOPED": ("dsig::ENVELOPED", "implementado", []),
    "SignedXml::EXT_NS": ("sunat::EXT_NS", "implementado", []),
    "SignedXml::signXml": ("SignedXml::sign_xml", "implementado", ["sunat_sign"]),
    "SignedXml::verifyXml": ("SignedXml::verify_xml", "implementado", ["sunat_verify"]),
    "SignedXml::setCertificate": ("SignedXml::set_certificate", "implementado", ["sunat_sign", "sunat_verify"]),
    "SignedXml::setSignatureAlgorithm": ("SignedXml::set_signature_algorithm", "implementado", ["sunat_sign", "sunat_verify"]),
    "SignedXml::setDigestAlgorithm": ("SignedXml::set_digest_algorithm", "implementado", ["sunat_sign"]),
    "SignedXml::setCertificateFromFile": ("SignedXml::set_certificate_from_file", "implementado", ["sunat_sign", "api.rs"]),
    "SignedXml::getPublicKey": ("SignedXml::public_key", "implementado", ["sunat_verify"]),
    "SignedXml::sign": ("SignedXml::sign", "implementado", ["sunat_sign"]),
    "SignedXml::signFromFile": ("SignedXml::sign_from_file", "implementado", ["sunat_sign", "api.rs"]),
    "SignedXml::verify": ("SignedXml::verify", "implementado", ["sunat_verify"]),
    # XMLSecurityDSig
    "XMLSecurityDSig::__construct": ("XmlSecurityDsig::new / default", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::generateGUID": ("XmlSecurityDsig::generate_guid", "implementado", ["guid"]),
    "XMLSecurityDSig::generate_GUID": ("XmlSecurityDsig::generate_guid", "alias", ["guid"]),
    "XMLSecurityDSig::locateSignature": ("XmlSecurityDsig::locate_signature", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::createNewSignNode": ("XmlSecurityDsig::create_new_sign_node", "implementado", ["api.rs"]),
    "XMLSecurityDSig::setCanonicalMethod": ("XmlSecurityDsig::set_canonical_method", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::canonicalizeSignedInfo": ("XmlSecurityDsig::canonicalize_signed_info", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::calculateDigest": ("XmlSecurityDsig::calculate_digest / calculate_digest_base64", "implementado", ["digest"]),
    "XMLSecurityDSig::validateDigest": ("XmlSecurityDsig::validate_digest", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::processTransforms": ("XmlSecurityDsig::process_transforms (XPath con c14n::canonicalize_xpath)", "implementado", ["c14n", "dsig_verify"]),
    "XMLSecurityDSig::processRefNode": ("XmlSecurityDsig::process_reference", "implementado", ["dsig_verify", "api.rs"]),
    "XMLSecurityDSig::getRefNodeID": ("XmlSecurityDsig::ref_node_id", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::getRefIDs": ("XmlSecurityDsig::ref_ids", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::validateReference": ("XmlSecurityDsig::validate_reference", "implementado", ["dsig_verify", "sunat_verify"]),
    "XMLSecurityDSig::addReference": ("XmlSecurityDsig::add_reference", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::addReferenceList": ("XmlSecurityDsig::add_reference_list", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::addObject": ("XmlSecurityDsig::add_object", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::locateKey": ("XmlSecurityDsig::locate_key", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::verify": ("XmlSecurityDsig::verify", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::signData": ("XmlSecurityDsig::sign_data", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::sign": ("XmlSecurityDsig::sign", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::appendCert": ("XmlSecurityDsig::append_cert (vacío, como en PHP)", "implementado", ["api.rs"]),
    "XMLSecurityDSig::insertSignature": ("XmlSecurityDsig::insert_signature", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::appendSignature": ("XmlSecurityDsig::append_signature", "implementado", ["dsig_sign"]),
    "XMLSecurityDSig::get509XCert": ("XmlSecurityDsig::get_509_x_cert", "implementado", ["x509_cert"]),
    "XMLSecurityDSig::staticGet509XCerts": ("XmlSecurityDsig::static_get_509_x_certs", "implementado", ["x509_certs"]),
    "XMLSecurityDSig::staticAdd509Cert": ("XmlSecurityDsig::static_add_509_cert (sin $isURL)", "adaptado", ["dsig_sign", "sunat_sign"]),
    "XMLSecurityDSig::add509Cert": ("XmlSecurityDsig::add_509_cert (sin $isURL)", "adaptado", ["dsig_sign", "sunat_sign"]),
    "XMLSecurityDSig::appendToKeyInfo": ("XmlSecurityDsig::append_to_key_info", "implementado", ["api.rs"]),
    "XMLSecurityDSig::getValidatedNodes": ("XmlSecurityDsig::validated_nodes", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::$sigNode": ("XmlSecurityDsig::signature_node / signature_xml", "adaptado", ["dsig_verify"]),
    "XMLSecurityDSig::$idKeys": ("XmlSecurityDsig::id_keys", "implementado", ["dsig_verify"]),
    "XMLSecurityDSig::$idNS": ("XmlSecurityDsig::id_ns", "implementado", []),
    # XMLSecurityKey
    "XMLSecurityKey::__construct": ("XmlSecurityKey::new", "implementado", ["key_info"]),
    "XMLSecurityKey::getSymmetricKeySize": ("XmlSecurityKey::symmetric_key_size", "implementado", ["key_info"]),
    "XMLSecurityKey::generateSessionKey": ("XmlSecurityKey::generate_session_key", "implementado", ["session_key"]),
    "XMLSecurityKey::getRawThumbprint": ("key::raw_thumbprint", "implementado", ["thumbprint"]),
    "XMLSecurityKey::loadKey": ("XmlSecurityKey::load_key / load_key_file", "implementado", ["key_info", "sign_data", "rsa_roundtrip", "key_x509", "api.rs"]),
    "XMLSecurityKey::encryptData": ("XmlSecurityKey::encrypt_data", "implementado", ["symmetric_roundtrip", "rsa_roundtrip"]),
    "XMLSecurityKey::decryptData": ("XmlSecurityKey::decrypt_data", "implementado", ["symmetric_decrypt", "rsa_roundtrip"]),
    "XMLSecurityKey::signData": ("XmlSecurityKey::sign_data", "implementado", ["sign_data"]),
    "XMLSecurityKey::verifySignature": ("XmlSecurityKey::verify_signature", "implementado", ["sign_data"]),
    "XMLSecurityKey::getAlgorith": ("XmlSecurityKey::algorithm", "alias", ["key_info"]),
    "XMLSecurityKey::getAlgorithm": ("XmlSecurityKey::algorithm", "implementado", ["key_info"]),
    "XMLSecurityKey::makeAsnSegment": ("key::make_asn_segment", "implementado", ["asn_segment"]),
    "XMLSecurityKey::convertRSA": ("key::convert_rsa", "implementado", ["convert_rsa"]),
    "XMLSecurityKey::getX509Certificate": ("XmlSecurityKey::x509_certificate", "implementado", ["key_x509"]),
    "XMLSecurityKey::getX509Thumbprint": ("XmlSecurityKey::x509_thumbprint", "implementado", ["key_x509"]),
    "XMLSecurityKey::fromEncryptedKeyElement": ("XmlSecurityKey::from_encrypted_key_element", "implementado", ["enc_key_info", "enc_decrypt"]),
    "XMLSecurityKey::$type": ("XmlSecurityKey::algorithm", "adaptado", ["key_info"]),
    "XMLSecurityKey::$key": ("XmlSecurityKey::raw_key / set_raw_key (material asimétrico no expuesto)", "adaptado", ["session_key"]),
    "XMLSecurityKey::$passphrase": ("XmlSecurityKey::passphrase", "implementado", ["sign_data"]),
    "XMLSecurityKey::$iv": ("XmlSecurityKey::iv", "implementado", ["symmetric_roundtrip"]),
    "XMLSecurityKey::$name": ("XmlSecurityKey::name", "implementado", ["enc_encrypt", "enc_key_info"]),
    "XMLSecurityKey::$keyChain": ("—", "sin uso", []),
    "XMLSecurityKey::$isEncrypted": ("XmlSecurityKey::is_encrypted", "implementado", ["enc_decrypt", "enc_key_info"]),
    "XMLSecurityKey::$encryptedCtx": ("XmlSecurityKey::encrypted_ctx / decrypt_encrypted_key", "implementado", ["enc_decrypt"]),
    "XMLSecurityKey::$guid": ("—", "sin uso", []),
    # XMLSecEnc
    "XMLSecEnc::__construct": ("XmlSecEnc::new", "implementado", ["enc_encrypt"]),
    "XMLSecEnc::addReference": ("XmlSecEnc::add_reference", "implementado", []),
    "XMLSecEnc::setNode": ("XmlSecEnc::set_node", "implementado", ["enc_encrypt", "enc_decrypt"]),
    "XMLSecEnc::encryptNode": ("XmlSecEnc::encrypt_node", "implementado", ["enc_encrypt"]),
    "XMLSecEnc::encryptReferences": ("XmlSecEnc::encrypt_references", "implementado", []),
    "XMLSecEnc::getCipherValue": ("XmlSecEnc::cipher_value", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::decryptNode": ("XmlSecEnc::decrypt_node", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::encryptKey": ("XmlSecEnc::encrypt_key", "implementado", ["enc_encrypt"]),
    "XMLSecEnc::decryptKey": ("XmlSecEnc::decrypt_key", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::locateEncryptedData": ("XmlSecEnc::locate_encrypted_data", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::locateKey": ("XmlSecEnc::locate_key", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::staticLocateKeyInfo": ("XmlSecEnc::static_locate_key_info", "implementado", ["enc_key_info", "dsig_verify"]),
    "XMLSecEnc::locateKeyInfo": ("XmlSecEnc::locate_key_info", "implementado", ["enc_decrypt"]),
    "XMLSecEnc::$type": ("XmlSecEnc::kind", "implementado", ["enc_encrypt"]),
    "XMLSecEnc::$encKey": ("XmlSecEnc::enc_key", "implementado", ["enc_encrypt"]),
    # X509Certificate
    "X509Certificate::__construct": ("X509Certificate::new", "implementado", ["x509"]),
    "X509Certificate::createFromFile": ("X509Certificate::from_file", "implementado", ["api.rs"]),
    "X509Certificate::getName": ("X509Certificate::name", "implementado", ["x509"]),
    "X509Certificate::getSubject": ("X509Certificate::subject", "implementado", ["x509"]),
    "X509Certificate::getIssuer": ("X509Certificate::issuer (devuelve el sujeto, como PHP)", "implementado", ["x509"]),
    "X509Certificate::getValidFrom": ("X509Certificate::valid_from", "implementado", ["x509"]),
    "X509Certificate::getExpiration": ("X509Certificate::expiration", "implementado", ["x509"]),
    "X509Certificate::getPurposes": ("x509::X509Certificate::purposes", "implementado", ["x509"]),
    "X509Certificate::getExtensions": ("x509::X509Certificate::extensions", "implementado", ["x509"]),
    "X509Certificate::getPublicKey": ("X509Certificate::public_key", "implementado", ["x509"]),
    "X509Certificate::getPrivateKey": ("X509Certificate::private_key", "implementado", ["x509"]),
    "X509Certificate::getRaw": ("X509Certificate::raw", "implementado", ["x509"]),
    "X509Certificate::export": ("X509Certificate::export", "implementado", ["x509"]),
    "X509ContentType::PEM": ("X509ContentType::Pem", "implementado", ["x509"]),
    "X509ContentType::CER": ("X509ContentType::Cer", "implementado", ["x509"]),
    # XPath
    "XPath::filterAttrValue": ("xpath::filter_attr_value", "implementado", ["xpath_filter"]),
    "XPath::filterAttrName": ("xpath::filter_attr_name", "implementado", ["xpath_filter"]),
}


def short(fqcn: str) -> str:
    return fqcn.rsplit("\\", 1)[1]


def main() -> None:
    inv = json.load(open(sys.argv[1]))
    registry = json.load(open(REF / "differences.json"))["differences"]
    reg_ids = {(d["family"], d["id"]) for d in registry}
    families = ["basics", "keys", "signing", "encryption", "hostile", "encoding"]
    per_scenario: dict[str, Counter] = defaultdict(Counter)
    for fam in families:
        cases = json.load(open(REF / f"cases/{fam}.json"))["cases"]
        expected = json.load(open(REF / f"expected/{fam}.json"))["results"]
        for c in cases:
            sc = per_scenario[c["scenario"]]
            sc["casos"] += 1
            if (fam, c["id"]) in reg_ids:
                sc["diferencias registradas"] += 1
            elif "error" in expected[c["id"]]:
                sc["errores equivalentes"] += 1
            else:
                sc["resultados iguales"] += 1

    out = []
    w = out.append
    w("# Matriz de inventario y casos — cpeforge-xmldsig 0.1.0")
    w("")
    w("Generada por `oracle/cases/xmldsig/build_matrix.py` a partir del inventario por reflexión de")
    w("`thegreenter/xmldsig` v5.1.0 (`oracle/php/xmldsig/inventory.php`), los casos y fixtures de")
    w("`tests/reference/xmldsig` y `differences.json`. No editar a mano.")
    w("")
    w(f"Fuentes PHP inventariadas: {len(inv['source_files'])} archivos de `src/` — " + ", ".join(f"`{f}`" for f in inv["source_files"]) + ".")
    w("")
    w("## API pública")
    w("")
    w("| Símbolo PHP | Rust | Estado | Cobertura |")
    w("| --- | --- | --- | --- |")
    status = Counter()
    missing = []
    for cl in inv["classes"]:
        name = short(cl["class"])
        symbols = [f"{name}::{c}" for c in cl["constants"]]
        symbols += [f"{name}::{p.replace('static ', '')}" for p in cl["properties"]]
        symbols += [f"{name}::{m.replace('static ', '').split('(')[0]}" for m in cl["methods"]]
        for sym in symbols:
            if sym in RUST:
                rust, st, cov = RUST[sym]
            elif "::" in sym and sym.split("::")[1].isupper() or sym.split("::")[1] in ("template",):
                rust, st, cov = (f"constante `{sym.split('::')[1]}` del módulo", "implementado", [])
            else:
                rust, st, cov = ("—", "SIN MAPEAR", [])
                missing.append(sym)
            status[st] += 1
            w(f"| `{sym}` | `{rust}` | {st} | {', '.join(f'`{c}`' for c in cov) or '—'} |")
    w("")
    w("Resumen: " + ", ".join(f"{v} {k}" for k, v in sorted(status.items())) + ".")
    if missing:
        w("")
        w("**Símbolos sin mapear:** " + ", ".join(missing))
    w("")
    w("Estados: *implementado* (misma semántica); *alias* (método PHP duplicado); *adaptado* (misma")
    w("semántica con forma Rust: el DOM se pasa explícito y el material asimétrico no se expone);")
    w("*pendiente* (sin implementar); *sin uso* (propiedad")
    w("que la referencia declara pero nunca usa). `api.rs` indica cobertura por pruebas de API sin")
    w("oráculo.")
    w("")
    w("## Casos por escenario")
    w("")
    w("| Escenario | Casos | Resultados iguales | Errores equivalentes | Diferencias registradas |")
    w("| --- | ---: | ---: | ---: | ---: |")
    tot = Counter()
    for sc in sorted(per_scenario):
        c = per_scenario[sc]
        tot.update(c)
        w(f"| `{sc}` | {c['casos']} | {c['resultados iguales']} | {c['errores equivalentes']} | {c['diferencias registradas']} |")
    w(f"| **Total** | **{tot['casos']}** | **{tot['resultados iguales']}** | **{tot['errores equivalentes']}** | **{tot['diferencias registradas']}** |")
    w("")
    w("«Iguales» se comprueba en vivo con las mismas claves efímeras (firmas y XML byte a byte) y offline")
    w("con los fixtures redactados.")
    w("")
    w("## Diferencias registradas")
    w("")
    w("| Familia / caso | Categoría | Motivo |")
    w("| --- | --- | --- |")
    for d in registry:
        w(f"| `{d['family']}/{d['id']}` | {d['category']} | {d['reason']} |")
    w("")
    w("## Suite `.phpt` de la referencia")
    w("")
    w("| Prueba | Título |")
    w("| --- | --- |")
    for t in inv["tests"]:
        w(f"| `{t['file']}` | {t['title']} |")
    print("\n".join(out))
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
