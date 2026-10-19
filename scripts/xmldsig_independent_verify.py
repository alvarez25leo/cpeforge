#!/usr/bin/env python3
"""Verificación criptográfica INDEPENDIENTE de firmas XMLDSig envueltas.

No usa código de cpeforge-xmldsig ni de greenter/xmldsig PHP:

- C14N: `xmllint` del sistema (libxml2 del sistema operativo, versión distinta
  de la que enlaza PHP), sobre documentos reconstruidos con la biblioteca
  estándar de Python.
- Digest: hashlib de Python.
- Firma RSA: LibreSSL (`/usr/bin/openssl`), biblioteca criptográfica distinta
  de OpenSSL 3, que usan PHP y Rust.

Alcance: firmas con una referencia `URI=""` (o sin URI) y transformación
envuelta, C14N inclusivo o exclusivo sin comentarios, RSA-SHA1/RSA-SHA256 y
digest SHA-1/SHA-256: el perfil de SignedXml (SUNAT) y las firmas envueltas de
XMLSecurityDSig. Otros documentos se informan como "fuera de alcance".

Uso: xmldsig_independent_verify.py <dir_xml> <cert_respaldo.pem> <cert_ajeno.pem> <salida.json>

Para cada documento se verifica (1) el digest de la referencia, (2) la firma
sobre SignedInfo y (3) que una alteración del contenido firmado invalida el
digest y (4) que la firma NO se acepta con el certificado de otra clave.
No imprime material criptográfico.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import xml.parsers.expat
from pathlib import Path

DSIG = "http://www.w3.org/2000/09/xmldsig#"
C14N = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315"
EXC = "http://www.w3.org/2001/10/xml-exc-c14n#"
ENVELOPED = DSIG + "enveloped-signature"
DIGESTS = {DSIG + "sha1": "sha1", "http://www.w3.org/2001/04/xmlenc#sha256": "sha256"}
SIGNATURES = {DSIG + "rsa-sha1": "sha1", "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256": "sha256"}
# En macOS /usr/bin/openssl es LibreSSL; en otros sistemas puede ser OpenSSL
# (mismo origen que la biblioteca de PHP y Rust, aunque otro binario). La
# evidencia registra cuál se usó.
LIBRESSL = os.environ.get("INDEPENDENT_OPENSSL", "/usr/bin/openssl")
XMLLINT = os.environ.get("INDEPENDENT_XMLLINT", "/usr/bin/xmllint")


class OutOfScope(Exception):
    pass


def element_spans(data: bytes):
    """Recorre el documento con expat y devuelve, por elemento, nombre,
    atributos, declaraciones en ámbito y posiciones de inicio/fin en bytes.

    expat no conoce windows-1252 ni ISO-8859-15: un documento que declara
    una codificación distinta de UTF-8 se lee como ISO-8859-1, que conserva
    la estructura y los desplazamientos (los nombres y valores de la firma
    son ASCII). xmllint recibe siempre los bytes originales."""
    head = data[:200].lower()
    declared = re.search(rb"encoding\s*=\s*[\"']([^\"']+)", head) if head.startswith(b"<?xml") else None
    override = None if declared is None or declared.group(1) in (b"utf-8", b"utf8") else "ISO-8859-1"
    parser = xml.parsers.expat.ParserCreate(override)
    parser.ordered_attributes = True
    stack = []
    out = []

    def start(name, attrs):
        pairs = list(zip(attrs[0::2], attrs[1::2]))
        scope = dict(stack[-1]["scope"]) if stack else {}
        for k, v in pairs:
            if k == "xmlns":
                scope[""] = v
            elif k.startswith("xmlns:"):
                scope[k[6:]] = v
        prefix, _, local = name.rpartition(":")
        entry = {"name": name, "local": local, "ns": scope.get(prefix, None) if prefix else scope.get(""),
                 "attrs": dict(pairs), "scope": scope, "start": parser.CurrentByteIndex, "children": []}
        if stack:
            stack[-1]["children"].append(entry)
        stack.append(entry)
        out.append(entry)

    def end(name):
        entry = stack.pop()
        # CurrentByteIndex apunta al inicio de la etiqueta de cierre (o del
        # elemento vacío); se avanza hasta su '>'.
        idx = parser.CurrentByteIndex
        entry["end"] = data.index(b">", idx) + 1

    texts = {}

    def chars(t):
        if stack:
            texts.setdefault(id(stack[-1]), []).append(t)

    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.CharacterDataHandler = chars
    parser.Parse(data, True)
    for e in out:
        e["text"] = "".join(texts.get(id(e), []))
    return data, out


def c14n(xml_bytes: bytes, exclusive: bool) -> bytes:
    """C14N con xmllint, sin comentarios (se retiran antes: xmllint los conserva)."""
    no_comments = re.sub(rb"<!--.*?-->", b"", xml_bytes, flags=re.S)
    with tempfile.NamedTemporaryFile(suffix=".xml") as f:
        f.write(no_comments)
        f.flush()
        flag = "--exc-c14n" if exclusive else "--c14n"
        return subprocess.run([XMLLINT, flag, f.name], check=True, capture_output=True).stdout


def child(entry, local):
    return next((c for c in entry["children"] if c["local"] == local and c["ns"] == DSIG), None)


def verify(document: bytes, fallback_cert: str, tamper: bool = False, force_cert: str | None = None) -> dict:
    data, elements = element_spans(document)
    sigs = [e for e in elements if e["local"] == "Signature" and e["ns"] == DSIG]
    if not sigs:
        raise OutOfScope("sin ds:Signature")
    sig = sigs[0]
    signed_info = child(sig, "SignedInfo")
    refs = [c for c in signed_info["children"] if c["local"] == "Reference"] if signed_info else []
    if len(refs) != 1 or refs[0]["attrs"].get("URI", "") != "":
        raise OutOfScope("solo una referencia URI=\"\"")
    ref = refs[0]
    transforms = child(ref, "Transforms")
    algs = [t["attrs"].get("Algorithm") for t in (transforms["children"] if transforms else [])]
    if ENVELOPED not in algs or any(a not in (ENVELOPED, C14N, EXC) for a in algs):
        raise OutOfScope(f"transformaciones {algs}")
    canon = child(signed_info, "CanonicalizationMethod")
    canon_alg = canon["attrs"].get("Algorithm") if canon else C14N
    if canon_alg not in (C14N, EXC):
        raise OutOfScope(f"canonicalización {canon_alg}")
    digest_alg = DIGESTS.get(child(ref, "DigestMethod")["attrs"].get("Algorithm"))
    sig_alg = SIGNATURES.get(child(signed_info, "SignatureMethod")["attrs"].get("Algorithm"))
    if not digest_alg or not sig_alg:
        raise OutOfScope("algoritmo no RSA-SHA1/256 o digest no SHA-1/256")

    # 1. Digest: documento sin el nodo Signature, C14N del documento.
    without_sig = data[: sig["start"]] + data[sig["end"]:]
    if tamper:
        without_sig = re.sub(rb">([^<>\s][^<>]*)</", lambda m: b">X" + m.group(1) + b"</", without_sig, count=1)
    ref_exclusive = EXC in algs
    digest = hashlib.new(digest_alg, c14n(without_sig, ref_exclusive)).digest()
    digest_ok = base64.b64encode(digest).decode() == child(ref, "DigestValue")["text"].strip()

    # 2. SignedInfo como raíz con todas las declaraciones en ámbito (lo que
    #    C14N inclusivo emite en el ápice; el exclusivo descarta las no usadas).
    si_bytes = data[signed_info["start"]: signed_info["end"]]
    own = {k for k in signed_info["attrs"] if k == "xmlns" or k.startswith("xmlns:")}
    decls = "".join(
        f' {"xmlns" if p == "" else "xmlns:" + p}="{u}"' for p, u in signed_info["scope"].items()
        if ("xmlns" if p == "" else "xmlns:" + p) not in own and u
    )
    m = re.match(rb"<[^\s>/]+", si_bytes)
    standalone = si_bytes[: m.end()] + decls.encode() + si_bytes[m.end():]
    signed_info_c14n = c14n(standalone, canon_alg == EXC)

    cert_entries = [e for e in elements if e["local"] == "X509Certificate" and e["ns"] == DSIG]
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        if force_cert:
            pem = Path(force_cert).read_text()
            source = "certificado forzado"
        elif cert_entries:
            body = re.sub(r"\s", "", cert_entries[0]["text"])
            pem = "-----BEGIN CERTIFICATE-----\n" + "\n".join(body[i:i + 64] for i in range(0, len(body), 64)) + "\n-----END CERTIFICATE-----\n"
            source = "KeyInfo"
        else:
            pem = Path(fallback_cert).read_text()
            source = "certificado de respaldo"
        (tmp / "cert.pem").write_text(pem)
        pub = subprocess.run([LIBRESSL, "x509", "-in", str(tmp / "cert.pem"), "-pubkey", "-noout"], check=True, capture_output=True).stdout
        (tmp / "pub.pem").write_bytes(pub)
        (tmp / "si.bin").write_bytes(signed_info_c14n)
        sig_value = base64.b64decode(re.sub(r"\s", "", child(sig, "SignatureValue")["text"]))
        (tmp / "sig.bin").write_bytes(sig_value)
        res = subprocess.run([LIBRESSL, "dgst", "-" + sig_alg, "-verify", str(tmp / "pub.pem"), "-signature", str(tmp / "sig.bin"), str(tmp / "si.bin")],
                             capture_output=True, text=True)
        signature_ok = res.returncode == 0 and "Verified OK" in res.stdout
    return {"digest_ok": digest_ok, "signature_ok": signature_ok, "digest": digest_alg, "signature": "rsa-" + sig_alg,
            "canonicalization": canon_alg, "key_source": source}


def main() -> int:
    directory, fallback, wrong, output = Path(sys.argv[1]), sys.argv[2], sys.argv[3], Path(sys.argv[4])
    libressl = subprocess.run([LIBRESSL, "version"], capture_output=True, text=True).stdout.strip()
    xmllint = subprocess.run([XMLLINT, "--version"], capture_output=True, text=True).stderr.splitlines()[0]
    results = {}
    for path in sorted(directory.glob("*.xml")):
        document = path.read_bytes()
        try:
            r = verify(document, fallback)
            r["tampered_digest_ok"] = verify(document, fallback, tamper=True)["digest_ok"]
            r["wrong_key_signature_ok"] = verify(document, fallback, force_cert=wrong)["signature_ok"]
        except OutOfScope as e:
            r = {"out_of_scope": str(e)}
        except Exception as e:  # noqa: BLE001 - se informa como fallo del caso
            r = {"error": f"{type(e).__name__}: {e}"}
        results[path.name] = r
    in_scope = {k: v for k, v in results.items() if "digest_ok" in v}
    summary = {
        "tools": {"c14n": xmllint, "rsa": libressl, "digest": "python hashlib"},
        "documents": len(results),
        "in_scope": len(in_scope),
        "valid": sum(1 for v in in_scope.values() if v["digest_ok"] and v["signature_ok"]),
        "tamper_detected": sum(1 for v in in_scope.values() if not v["tampered_digest_ok"]),
        "wrong_key_rejected": sum(1 for v in in_scope.values() if not v["wrong_key_signature_ok"]),
        "results": results,
    }
    output.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n")
    print(f"independiente: {summary['documents']} documentos, {summary['in_scope']} en alcance, "
          f"{summary['valid']} válidos, {summary['tamper_detected']} alteraciones detectadas, "
          f"{summary['wrong_key_rejected']} rechazos con clave ajena ({xmllint}; {libressl})")
    # Cada documento en alcance debe ser válido, su alteración detectada y su
    # firma rechazada con una clave ajena.
    failures = [k for k, v in in_scope.items()
                if not (v["digest_ok"] and v["signature_ok"]) or v["tampered_digest_ok"] or v["wrong_key_signature_ok"]]
    errors = [k for k, v in results.items() if "error" in v]
    for k in failures + errors:
        print(f"  FALLA {k}: {results[k]}")
    return 1 if failures or errors else 0


if __name__ == "__main__":
    sys.exit(main())
