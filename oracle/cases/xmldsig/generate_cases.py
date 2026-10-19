#!/usr/bin/env python3
"""Genera los casos sintéticos de paridad de greenter/xmldsig v5.1.0.

Los casos describen escenarios de la API pública PHP; las claves se nombran
("a", "b", "expired") y se crean de forma efímera en cada ejecución con
scripts/xmldsig_keys.sh. Ningún caso contiene material criptográfico.

Uso: python3 oracle/cases/xmldsig/generate_cases.py [directorio_salida]
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

DSIG = "http://www.w3.org/2000/09/xmldsig#"
SHA1 = DSIG + "sha1"
SHA256 = "http://www.w3.org/2001/04/xmlenc#sha256"
SHA384 = "http://www.w3.org/2001/04/xmldsig-more#sha384"
SHA512 = "http://www.w3.org/2001/04/xmlenc#sha512"
RIPEMD160 = "http://www.w3.org/2001/04/xmlenc#ripemd160"
C14N = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315"
C14N_COM = C14N + "#WithComments"
EXC = "http://www.w3.org/2001/10/xml-exc-c14n#"
EXC_COM = EXC + "WithComments"
XPATH = "http://www.w3.org/TR/1999/REC-xpath-19991116"
ENVELOPED = DSIG + "enveloped-signature"
BASE64_T = DSIG + "base64"
RSA_SHA1 = DSIG + "rsa-sha1"
RSA_SHA256 = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"
RSA_SHA384 = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha384"
RSA_SHA512 = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha512"
HMAC_SHA1 = DSIG + "hmac-sha1"
DSA_SHA1 = DSIG + "dsa-sha1"
TDES = "http://www.w3.org/2001/04/xmlenc#tripledes-cbc"
AES128 = "http://www.w3.org/2001/04/xmlenc#aes128-cbc"
AES192 = "http://www.w3.org/2001/04/xmlenc#aes192-cbc"
AES256 = "http://www.w3.org/2001/04/xmlenc#aes256-cbc"
RSA15 = "http://www.w3.org/2001/04/xmlenc#rsa-1_5"
OAEP = "http://www.w3.org/2001/04/xmlenc#rsa-oaep-mgf1p"
ELEMENT = "http://www.w3.org/2001/04/xmlenc#Element"
CONTENT = "http://www.w3.org/2001/04/xmlenc#Content"
UBL = "tests/reference/xml/expected"


def b64(data: bytes) -> dict:
    return {"b64": base64.b64encode(data).decode()}


def key(name: str, suffix: str = "pem") -> dict:
    return {"key": name, "suffix": suffix}


def case(cid: str, scenario: str, **params) -> dict:
    return {"id": cid, "scenario": scenario, **params}


# --------------------------------------------------------------------------
# XML de entrada
# --------------------------------------------------------------------------

SIMPLE = '<?xml version="1.0" encoding="utf-8"?><root xmlns="urn:r" xmlns:a="urn:a"><a:item Id="i1" a:x="1">uno</a:item><item Id="i2">dos</item></root>'

SERIALIZE = {
    "simple": SIMPLE,
    "no-declaration": "<root><a>x</a></root>",
    "declaration-upper": '<?xml version="1.0" encoding="UTF-8"?><root/>',
    "declaration-standalone": '<?xml version="1.0" encoding="utf-8" standalone="yes"?><root/>',
    "declaration-no-encoding": '<?xml version="1.0"?><root/>',
    "non-ascii-no-decl": "<root>ÑANDÚ 😀 ü</root>",
    "non-ascii-utf8": '<?xml version="1.0" encoding="utf-8"?><root a="ñ">ÑANDÚ 😀</root>',
    "empty-elements": '<?xml version="1.0" encoding="utf-8"?><root><a></a><b/><c> </c><d><e/></d></root>',
    "attribute-escapes": '<root a="&lt;&amp;&gt;&quot;&apos;" b=\'say "hi"\' c="tab\there" d="line\nbreak" e="&#9;&#10;&#13;" f="&#x41;&#66;"/>',
    "text-escapes": "<root>&lt;a&gt; &amp; &quot;q&quot; &apos;s&apos; &#13; &#x1F600; ]]&gt; done</root>",
    "cdata": "<root><![CDATA[ <x> & ]]><![CDATA[second]]>text</root>",
    "comments-pis": '<?xml version="1.0" encoding="utf-8"?><!-- antes --><?pi data?><root><!-- dentro --><?inner x="1"?></root><!-- despues -->\n',
    "whitespace-between": '<?xml version="1.0" encoding="utf-8"?>\n\n<root>\n  <a>1</a>\n\t<b>2</b>\n</root>\n\n',
    "crlf": "<root>\r\n<a>x\ry</a>\r\n</root>",
    "namespaces": '<p:root xmlns:p="urn:p" xmlns="urn:d" xmlns:unused="urn:u"><child xmlns=""><p:in xmlns:p="urn:p"/></child><x:y xmlns:x="urn:x" x:a="1" b="2"/></p:root>',
    "attribute-order": '<root z="1" a="2" xmlns:b="urn:b" b:c="3" xmlns="urn:d" m="4"/>',
    "xml-attributes": '<root xml:lang="es" xml:space="preserve"><a xml:lang="en">x</a></root>',
    "deep": "<r>" + "<d>" * 50 + "x" + "</d>" * 50 + "</r>",
}
for name in ("invoice/inv21-full-default", "invoice/inv20-full-default", "note/cr21-full-default", "despatch/desp2022-full-default",
             "summary/summary-full-default", "voided/reversion-full-default", "retention-perception/perception-full-default",
             "invoice/inv21-special-default"):
    SERIALIZE["ubl-" + name.split("/")[1]] = {"file": f"{UBL}/{name}.xml"}

C14N_DOC = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    '<!-- inicio --><?pi antes?>\n'
    '<doc xmlns="urn:d" xmlns:a="urn:a" xmlns:b="urn:b" xmlns:unused="urn:u" xml:lang="es">\n'
    '  <!-- comentario --><a:e2 z="1" a="2" b:at="3" xmlns:c="urn:c">texto &amp; &lt;x&gt; "q"</a:e2>\n'
    '  <e3 Id="target" attr="tab\t &#9;&#10;&#13; fin" xml:space="preserve"><b:inner/><c:x xmlns:c="urn:c2">y</c:x><![CDATA[c<d]]></e3>\n'
    '  <e4 xmlns=""><f xmlns:a="urn:a">sin default</f></e4>\n'
    '  <e5 xmlns:a="urn:a2" a:k="v" Id="second"><a:e6/></e5>\n'
    '</doc>\n<!-- final -->'
)

C14N_TARGETS = ["document", "root", "id:target", "id:second", "tag:|e4"]
C14N_METHODS = {
    "c14n": [C14N],
    "c14n-comments": [C14N_COM],
    "exc": [EXC],
    "exc-comments": [EXC_COM],
    "default": [],
    "enveloped-only": [ENVELOPED],
    "unknown-transform": [BASE64_T],
    "exc-prefixlist": [{"algorithm": EXC, "prefix_list": "a b #default unused"}],
    "exc-empty-prefixlist": [{"algorithm": EXC, "prefix_list": "  "}],
    "two-transforms": [ENVELOPED, EXC],
    "xpath": [{"algorithm": XPATH, "xpath": "not(self::comment())"}],
}


def basics() -> list[dict]:
    cases = []
    for name, xml in SERIALIZE.items():
        cases.append(case(f"serialize-{name}", "serialize", xml=xml))
    cases.append(case("serialize-no-preserve-whitespace", "serialize", xml=SERIALIZE["whitespace-between"], preserve_whitespace=False))
    cases.append(case("serialize-node-element", "serialize_node", xml=C14N_DOC, target="id:target"))
    cases.append(case("serialize-node-ns-child", "serialize_node", xml=SERIALIZE["namespaces"], target="tag:urn:x|y"))
    cases.append(case("serialize-malformed", "serialize", xml="<root><a></root>"))
    cases.append(case("serialize-empty", "serialize", xml=""))
    for mname, transforms in C14N_METHODS.items():
        for target in C14N_TARGETS:
            tid = target.replace("tag:|", "tag-").replace(":", "-")
            cases.append(case(f"c14n-{mname}-{tid}", "c14n", xml=C14N_DOC, transforms=transforms, target=target))
    # Transformación XPath: subconjuntos de nodos (elementos, texto, atributos y
    # nodos de namespace), prefijos registrados en ds:XPath, posición, errores.
    xp = lambda expr, ns=None: {"algorithm": XPATH, "xpath": expr, **({"xpath_ns": ns} if ns else {})}
    xpath_cases = {
        "no-element": [xp("not(self::a:e2)", {"a": "urn:a"})],
        "no-text": [xp("not(self::text())")],
        "no-subtree": [xp("not(ancestor-or-self::*[@Id='target'])")],
        "only-elements-text": [xp("self::* or self::text()")],
        "no-attributes": [xp("count(. | ../@*) != count(../@*)")],
        "subtree-ns": [xp("ancestor-or-self::d:e3", {"d": "urn:d"})],
        "position": [xp("position() > 3 and position() mod 2 = 0")],
        "no-attr-name": [xp("not(name() = 'b:at' or name() = 'attr')")],
        "no-ns-node": [xp("not(local-name() = 'unused' and namespace-uri() = '' and not(self::*))")],
        "text-functions": [xp("not(self::text()[contains(., '&') or starts-with(normalize-space(.), 'y')])")],
        "string-functions": [xp("string-length(local-name()) < 3 or substring-before(concat(name(), ':'), ':') = 'a' or translate(name(), 'E', 'e') = 'e5'")],
        "numbers": [xp("floor(1.5) + ceiling(1.2) + round(2.5) - sum(//@z) = 4 or number('x') != number('x') and boolean(0)")],
        "lang": [xp("lang('es') or self::comment()")],
        "following-preceding": [xp("not(preceding-sibling::*[1][self::a:e2] or following::d:e5)", {"a": "urn:a", "d": "urn:d"})],
        "count-last": [xp("count(ancestor::*) < 2 and position() != last()")],
        "union-filter": [xp("(ancestor::*|self::*)[last()][self::*] or self::text()")],
        "with-exc": [xp("not(self::a:e2)", {"a": "urn:a"}), EXC],
        "with-exc-prefixlist": [xp("not(self::a:e2)", {"a": "urn:a"}), {"algorithm": EXC, "prefix_list": "a b #default"}],
        "with-comments": [C14N_COM, xp("not(self::text())")],
        "syntax-error": [xp("not(")],
        "undefined-prefix": [xp("self::zz:x")],
        "unknown-function": [xp("desconocida(.)")],
        "default-ns-registered": [xp("not(self::*[local-name()='f'])", {"": "urn:d"})],
    }
    for xname, transforms in xpath_cases.items():
        for target in ("document", "root", "id:target"):
            tid = target.replace(":", "-")
            cases.append(case(f"c14n-xpath-{xname}-{tid}", "c14n", xml=C14N_DOC, transforms=transforms, target=target))
    for mname in ("c14n-comments", "exc-comments"):
        cases.append(case(f"c14n-{mname}-no-comments-flag", "c14n", xml=C14N_DOC, transforms=C14N_METHODS[mname], target="document", include_comments=False))
    for name in ("namespaces", "attribute-escapes", "text-escapes", "cdata", "crlf", "xml-attributes", "ubl-inv21-full-default", "ubl-inv20-full-default"):
        for mname in ("c14n", "exc"):
            cases.append(case(f"c14n-{mname}-doc-{name}", "c14n", xml=SERIALIZE[name], transforms=C14N_METHODS[mname], target="document"))
    data = {"empty": b"", "ascii": b"hola mundo", "unicode": "ñandú 😀".encode(), "binary": bytes(range(256))}
    for alg_name, alg in {"sha1": SHA1, "sha256": SHA256, "sha384": SHA384, "sha512": SHA512, "ripemd160": RIPEMD160, "md5": "http://www.w3.org/2001/04/xmldsig-more#md5", "empty": ""}.items():
        for dname, d in data.items():
            cases.append(case(f"digest-{alg_name}-{dname}", "digest", alg=alg, data=b64(d)))
        cases.append(case(f"digest-{alg_name}-raw", "digest", alg=alg, data="abc", encode=False))
    for i, (value, extra) in enumerate([
        ("a\"b'c", {}), ("a\"b'c", {"quotes": '"'}), ("a\"b'c", {"quotes": "'"}), ("ñ\"x", {}),
    ]):
        cases.append(case(f"xpath-value-{i}", "xpath_filter", fn="value", value=value, **extra))
    for i, (value, extra) in enumerate([
        ("xml:id", {}), ("bad name!@#", {}), ("a-b_c.d:e f", {}), ("ñame", {}), ("abc123", {"allow": "\\d"}), ("abc123", {"allow": "\\w"}), ("a\tb\nc", {}),
    ]):
        cases.append(case(f"xpath-name-{i}", "xpath_filter", fn="name", value=value, **extra))
    cases.append(case("guid-default", "guid"))
    cases.append(case("guid-prefix", "guid", prefix="id"))
    cases.append(case("guid-empty-prefix", "guid", prefix=""))
    for t, sizes in ((0x02, (0, 1, 127, 128, 255, 256)), (0x03, (0, 5, 300)), (0x30, (10, 127, 128, 65535, 65536))):
        for size in sizes:
            for first in (0x01, 0x80):
                data_bytes = bytes([first]) + bytes((i * 7) % 256 for i in range(max(size - 1, 0))) if size else b""
                cases.append(case(f"asn-{t:02x}-{size}-{first:02x}", "asn_segment", type=t, data=b64(data_bytes)))
    cases.append(case("convert-rsa-small", "convert_rsa", modulus=b64(bytes([0xC3]) + bytes(range(1, 128))), exponent=b64(b"\x01\x00\x01")))
    cases.append(case("convert-rsa-low-bit", "convert_rsa", modulus=b64(bytes([0x43]) + bytes(255)), exponent=b64(b"\x03")))
    cases.append(case("convert-rsa-large", "convert_rsa", modulus=b64(bytes([0xFF]) * 512), exponent=b64(b"\x01\x00\x01")))
    pem1 = "-----BEGIN CERTIFICATE-----\nQUJD\nREVG\n-----END CERTIFICATE-----\n"
    pem2 = "-----BEGIN CERTIFICATE-----\r\n  SEk=  \r\n-----END CERTIFICATE-----\r\n"
    texts = {
        "one": pem1, "two": pem1 + "garbage\n" + pem2, "crlf": pem2, "no-end": "-----BEGIN CERTIFICATE-----\nQUJD\n",
        "none": "no hay certificado", "private-key": "-----BEGIN PRIVATE KEY-----\nQUJD\n-----END PRIVATE KEY-----\n",
        "begin-variant": "-----BEGIN CERTIFICATE REQUEST-----\nQUJD\n-----END CERTIFICATE REQUEST-----\n",
    }
    for name, text in texts.items():
        cases.append(case(f"x509-certs-{name}", "x509_certs", text=text))
        cases.append(case(f"x509-cert-{name}", "x509_cert", text=text))
        cases.append(case(f"thumbprint-{name}", "thumbprint", text=text))
    cases.append(case("x509-certs-not-pem", "x509_certs", text="DER-BYTES", pem=False))
    return cases


# --------------------------------------------------------------------------
# Claves
# --------------------------------------------------------------------------

ALL_ALGS = {"3des": TDES, "aes128": AES128, "aes192": AES192, "aes256": AES256, "rsa15": RSA15, "oaep": OAEP, "rsa-sha1": RSA_SHA1,
            "rsa-sha256": RSA_SHA256, "rsa-sha384": RSA_SHA384, "rsa-sha512": RSA_SHA512, "hmac": HMAC_SHA1, "dsa": DSA_SHA1,
            "unknown": "urn:desconocido"}
SYM = {"3des": (TDES, 24), "aes128": (AES128, 16), "aes192": (AES192, 24), "aes256": (AES256, 32)}


def keys() -> list[dict]:
    cases = []
    for name, alg in ALL_ALGS.items():
        cases.append(case(f"key-{name}-no-type", "key_info", key={"alg": alg}))
        for t in ("public", "private", "bogus"):
            cases.append(case(f"key-{name}-{t}", "key_info", key={"alg": alg, "type": t}))
    for name, alg in ALL_ALGS.items():
        if name in ("dsa", "unknown"):
            continue
        cases.append(case(f"session-{name}", "session_key", key={"alg": alg, "type": "private"}))
    for name, (alg, size) in SYM.items():
        k = bytes((i * 13 + 7) % 256 for i in range(size))
        block = 8 if name == "3des" else 16
        for dname, d in {
            "zero-blocks": bytes(block * 3),
            "pattern": bytes((i * 31) % 256 for i in range(block * 4)),
        }.items():
            cases.append(case(f"sym-decrypt-{name}-{dname}", "symmetric_decrypt", key={"alg": alg, "load": b64(k)}, data=b64(d)))
        cases.append(case(f"sym-decrypt-{name}-not-block", "symmetric_decrypt", key={"alg": alg, "load": b64(k)}, data=b64(bytes(block + 3))))
        cases.append(case(f"sym-decrypt-{name}-iv-only", "symmetric_decrypt", key={"alg": alg, "load": b64(k)}, data=b64(bytes(block))))
        cases.append(case(f"sym-short-key-{name}", "symmetric_decrypt", key={"alg": alg, "load": b64(k[:-1])}, data=b64(bytes(block * 2))))
        cases.append(case(f"sym-long-key-{name}", "symmetric_roundtrip", key={"alg": alg, "load": b64(k + b"extra")}, data="texto"))
        for dname, d in {"empty": "", "block-minus": "x" * (block - 1), "block": "x" * block, "unicode": "ñandú 😀 <xml/>"}.items():
            cases.append(case(f"sym-roundtrip-{name}-{dname}", "symmetric_roundtrip", key={"alg": alg, "load": b64(k)}, data=d))
    hk = {"alg": HMAC_SHA1, "load": "clave-hmac"}
    cases.append(case("hmac-sign", "sign_data", key=hk, data="datos firmados"))
    cases.append(case("hmac-sign-empty", "sign_data", key=hk, data=""))
    cases.append(case("hmac-verify-other-data", "sign_data", key=hk, data="datos", verify_data="otros"))
    cases.append(case("hmac-verify-other-key", "sign_data", key=hk, data="datos", verify_key={"alg": HMAC_SHA1, "load": "otra"}))
    for name, alg in {"rsa-sha1": RSA_SHA1, "rsa-sha256": RSA_SHA256, "rsa-sha384": RSA_SHA384, "rsa-sha512": RSA_SHA512}.items():
        priv = {"alg": alg, "type": "private", "load": key("a")}
        pub_cert = {"alg": alg, "type": "public", "load": key("a", "crt"), "is_cert": True}
        cases.append(case(f"rsa-sign-{name}", "sign_data", key=priv, verify_key=pub_cert, data="datos firmados"))
        cases.append(case(f"rsa-sign-{name}-pubkey", "sign_data", key=priv, verify_key={"alg": alg, "type": "public", "load": key("a", "pub.pem")}, data="d"))
        cases.append(case(f"rsa-sign-{name}-wrong-key", "sign_data", key=priv, verify_key={"alg": alg, "type": "public", "load": key("b", "crt"), "is_cert": True}, data="d"))
        cases.append(case(f"rsa-sign-{name}-other-data", "sign_data", key=priv, verify_key=pub_cert, data="d", verify_data="e"))
    cases.append(case("rsa-sign-pkcs1-key", "sign_data", key={"alg": RSA_SHA1, "type": "private", "load": key("a", "rsa.pem")}, verify_key={"alg": RSA_SHA1, "type": "public", "load": key("a", "crt"), "is_cert": True}, data="d"))
    cases.append(case("rsa-sign-encrypted-key", "sign_data", key={"alg": RSA_SHA1, "type": "private", "load": key("a", "enc.pem"), "passphrase": "clave-sintetica-de-prueba"}, verify_key={"alg": RSA_SHA1, "type": "public", "load": key("a", "crt"), "is_cert": True}, data="d"))
    # Con contraseña incorrecta, 1 de cada 256 sales produce un relleno
    # válido y OpenSSL informa otro error: se compara el mensaje sin el detalle.
    cases.append(case("rsa-sign-encrypted-key-wrong-pass", "sign_data", key={"alg": RSA_SHA1, "type": "private", "load": key("a", "enc.pem"), "passphrase": "otra"}, data="d", openssl_detail_random=True))
    cases.append(case("rsa-sign-invalid-key", "sign_data", key={"alg": RSA_SHA1, "type": "private", "load": "no es una clave"}, data="d"))
    cases.append(case("rsa-public-invalid", "key_x509", key={"alg": RSA_SHA1, "type": "public", "load": "no es una clave"}))
    cases.append(case("rsa-sign-with-public-key", "sign_data", key={"alg": RSA_SHA1, "type": "public", "load": key("a", "pub.pem")}, data="d"))
    for name, alg in {"rsa15": RSA15, "oaep": OAEP}.items():
        pub = {"alg": alg, "type": "public", "load": key("a", "crt"), "is_cert": True}
        priv = {"alg": alg, "type": "private", "load": key("a")}
        for dname, d in {"short": "secreto", "session": b64(bytes(range(32))), "empty": ""}.items():
            cases.append(case(f"rsa-{name}-pub-priv-{dname}", "rsa_roundtrip", encrypt_key=pub, decrypt_key=priv, data=d))
        cases.append(case(f"rsa-{name}-priv-pub", "rsa_roundtrip", encrypt_key=priv, decrypt_key=pub, data="firmado en bruto"))
        # Con clave equivocada el resultado depende del relleno aleatorio
        # (dato mayor que el módulo, fallo de decodificación o rechazo
        # implícito): solo se compara si se recupera el texto original.
        cases.append(case(f"rsa-{name}-wrong-key", "rsa_roundtrip", encrypt_key=pub, decrypt_key={"alg": alg, "type": "private", "load": key("b")}, data="x", outcome_only=True))
        cases.append(case(f"rsa-{name}-too-long", "rsa_roundtrip", encrypt_key=pub, decrypt_key=priv, data="x" * 300))
    cases.append(case("key-x509-cert", "key_x509", key={"alg": RSA_SHA1, "type": "public", "load": key("a", "crt"), "is_cert": True}))
    cases.append(case("key-x509-combined-pem", "key_x509", key={"alg": RSA_SHA1, "type": "public", "load": key("a"), "is_cert": True}))
    cases.append(case("key-x509-not-cert", "key_x509", key={"alg": RSA_SHA1, "type": "public", "load": key("a", "pub.pem")}))
    cases.append(case("key-x509-private", "key_x509", key={"alg": RSA_SHA1, "type": "private", "load": key("a")}))
    return cases


# --------------------------------------------------------------------------
# Firma y verificación
# --------------------------------------------------------------------------

UBL_SIGN = {
    "inv21": f"{UBL}/invoice/inv21-full-default.xml",
    "inv20": f"{UBL}/invoice/inv20-full-default.xml",
    "boleta21": f"{UBL}/invoice/boleta21-default.xml",
    "cr21": f"{UBL}/note/cr21-full-default.xml",
    "db20": f"{UBL}/note/db20-full-default.xml",
    "desp2022": f"{UBL}/despatch/desp2022-full-default.xml",
    "desp": f"{UBL}/despatch/desp-full-default.xml",
    "summary": f"{UBL}/summary/summary-full-default.xml",
    "voided": f"{UBL}/voided/voided-full-default.xml",
    "reversion": f"{UBL}/voided/reversion-full-default.xml",
    "retention": f"{UBL}/retention-perception/retention-full-default.xml",
    "perception": f"{UBL}/retention-perception/perception-full-default.xml",
    "special": f"{UBL}/invoice/inv21-special-default.xml",
}

FORMATTED = """<?xml version="1.0" encoding="UTF-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
    <ext:UBLExtensions>
        <ext:UBLExtension>
            <ext:ExtensionContent>
            </ext:ExtensionContent>
        </ext:UBLExtension>
    </ext:UBLExtensions>
    <cbc:ID>F001-1</cbc:ID>
    <!-- comentario -->
    <cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount>
</Invoice>
"""
NO_EXTENSION = '<?xml version="1.0" encoding="utf-8"?><Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cbc:ID>F001-1</cbc:ID></Invoice>'
OCCUPIED = '<?xml version="1.0" encoding="utf-8"?><Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><x>ocupado</x></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions><ID>1</ID></Invoice>'
NO_DS_NS = '<Root xmlns="urn:r" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"><ext:ExtensionContent/><Amount>10.00</Amount></Root>'

PRIV = {"alg": RSA_SHA1, "type": "private", "load": key("a")}


def signing() -> list[dict]:
    cases = []
    for name, path in UBL_SIGN.items():
        cases.append(case(f"sunat-sign-{name}", "sunat_sign", xml={"file": path}, cert=key("a")))
        cases.append(case(f"sunat-sign-{name}-sha256", "sunat_sign", xml={"file": path}, cert=key("a"), signature_alg=RSA_SHA256, digest_alg=SHA256))
    for name, xml in {"formatted": FORMATTED, "no-extension": NO_EXTENSION, "occupied": OCCUPIED, "no-ds-ns": NO_DS_NS}.items():
        cases.append(case(f"sunat-sign-{name}", "sunat_sign", xml=xml, cert=key("a")))
    cases.append(case("sunat-sign-mixed-sha256-sha1", "sunat_sign", xml={"file": UBL_SIGN["inv21"]}, cert=key("a"), signature_alg=RSA_SHA256))
    cases.append(case("sunat-sign-mixed-sha1-sha256", "sunat_sign", xml={"file": UBL_SIGN["inv21"]}, cert=key("a"), digest_alg=SHA256))
    cases.append(case("sunat-sign-unsupported-signature", "sunat_sign", xml=NO_EXTENSION, cert=key("a"), signature_alg=DSA_SHA1))
    cases.append(case("sunat-sign-unsupported-sha512", "sunat_sign", xml=NO_EXTENSION, cert=key("a"), signature_alg=RSA_SHA512))
    cases.append(case("sunat-sign-unsupported-digest", "sunat_sign", xml=NO_EXTENSION, cert=key("a"), digest_alg=SHA384))
    cases.append(case("sunat-sign-no-key", "sunat_sign", xml=NO_EXTENSION))
    cases.append(case("sunat-sign-invalid-key", "sunat_sign", xml=NO_EXTENSION, cert="no es un certificado"))
    cases.append(case("sunat-sign-key-without-cert", "sunat_sign", xml=NO_EXTENSION, cert=key("a", "key")))
    cases.append(case("sunat-sign-cert-file-missing", "sunat_sign", xml=NO_EXTENSION, cert_file="/no/existe.pem"))
    cases.append(case("sunat-sign-file-missing", "sunat_sign", sign_file="/no/existe.xml", cert=key("a")))
    cases.append(case("sunat-sign-malformed", "sunat_sign", xml="<Invoice><a></Invoice>", cert=key("a")))

    base = {"xml": {"file": UBL_SIGN["inv21"]}, "cert": key("a")}
    base20 = {"xml": {"file": UBL_SIGN["inv20"]}, "cert": key("a")}
    variants = {
        "valid": {},
        "valid-sha256": {"sign": {**base, "signature_alg": RSA_SHA256, "digest_alg": SHA256}},
        "valid-20": {"sign": base20},
        "tampered-amount": {"mutations": [{"op": "replace", "from": ">237.59<", "to": ">337.59<"}]},
        "tampered-id": {"mutations": [{"op": "replace", "from": "<cbc:ID>F001-123</cbc:ID>", "to": "<cbc:ID>F001-124</cbc:ID>"}]},
        "tampered-digest": {"mutations": [{"op": "flip_base64", "tag": "ds:DigestValue"}]},
        "tampered-signature": {"mutations": [{"op": "flip_base64", "tag": "ds:SignatureValue"}]},
        "tampered-signedinfo-alg": {"mutations": [{"op": "replace", "from": 'Algorithm="http://www.w3.org/2000/09/xmldsig#rsa-sha1"', "to": 'Algorithm="http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"'}]},
        "tampered-cert": {"mutations": [{"op": "flip_base64", "tag": "ds:X509Certificate"}]},
        "whitespace-added": {"mutations": [{"op": "replace", "from": "<cbc:Note", "to": "\n<cbc:Note"}]},
        "configured-wrong-cert": {"configured_cert": key("b")},
        "configured-right-cert": {"configured_cert": key("a")},
        "dom-configured-wrong-cert": {"configured_cert": key("b"), "dom": True},
        "dom-configured-right-cert": {"configured_cert": key("a"), "dom": True},
        "dom-no-cert": {"dom": True},
        "signed-by-b-cert-a": {"sign": {**base, "cert": key("b")}, "mutations": []},
        "expired-cert": {"sign": {**base, "cert": key("expired")}},
        "no-keyinfo": {"mutations": [{"op": "regex_remove_keyinfo"}]},
        "external-uri": {"mutations": [{"op": "replace", "from": '<ds:Reference URI="">', "to": '<ds:Reference URI="http://127.0.0.1:9/doc.xml">'}]},
        "missing-signature-value": {"mutations": [{"op": "element_text", "tag": "ds:SignatureValue", "to": ""}]},
        "unsigned": {"input": {"file": UBL_SIGN["inv21"]}},
        "second-signature-wrapping": {"mutations": [{"op": "replace", "from": "<ext:UBLExtension><ext:ExtensionContent>", "to": '<ext:UBLExtension><ext:ExtensionContent><ds:Signature xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:SignedInfo/></ds:Signature>'}]},
    }
    for name, extra in variants.items():
        params = {"sign": base}
        params.update(extra)
        if params.get("mutations") and params["mutations"][0]["op"] == "regex_remove_keyinfo":
            params["mutations"] = [{"op": "replace", "from": "<ds:KeyInfo>", "to": "<ds:Removed>"}, {"op": "replace", "from": "</ds:KeyInfo>", "to": "</ds:Removed>"}]
        cases.append(case(f"sunat-verify-{name}", "sunat_verify", **params))

    doc = '<?xml version="1.0" encoding="utf-8"?><root xmlns="urn:r"><data Id="d1">importe 10.00</data><other>otro</other><!-- c --></root>'
    hmac = {"alg": HMAC_SHA1, "load": "clave-hmac"}
    sign_variants = {
        "enveloped-c14n": {"canonical": C14N, "references": [{"target": "document", "transforms": [ENVELOPED], "options": {"force_uri": True}}], "append_to": "root"},
        "enveloped-c14n-comments": {"canonical": C14N_COM, "references": [{"target": "document", "transforms": [ENVELOPED, C14N_COM]}], "append_to": "root"},
        "exc-c14n": {"canonical": EXC, "references": [{"target": "document", "transforms": [ENVELOPED, EXC]}], "append_to": "root"},
        "no-force-uri": {"canonical": C14N, "references": [{"target": "document", "transforms": [ENVELOPED]}], "append_to": "root"},
        "id-generated": {"canonical": EXC, "references": [{"target": "tag:urn:r|other", "transforms": [EXC]}], "append_to": "root"},
        "id-existing-overwrite": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}], "append_to": "root"},
        "id-custom-name": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"id_name": "ID", "overwrite": False}}], "append_to": "root"},
        "id-prefixed": {"canonical": EXC, "references": [{"target": "tag:urn:r|other", "transforms": [EXC], "options": {"prefix": "wsu", "prefix_ns": "urn:wsu"}}], "append_to": "root"},
        "default-transform": {"canonical": EXC, "references": [{"target": "id:d1", "options": {"overwrite": False}}], "append_to": "root"},
        "multiple-refs": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}, {"target": "tag:urn:r|other", "transforms": [EXC], "digest": SHA256}], "append_to": "root"},
        "digest-sha384": {"canonical": C14N, "references": [{"target": "document", "transforms": [ENVELOPED], "digest": SHA384}], "append_to": "root"},
        "digest-sha512": {"canonical": C14N, "references": [{"target": "document", "transforms": [ENVELOPED], "digest": SHA512}], "append_to": "root"},
        "digest-ripemd160": {"canonical": C14N, "references": [{"target": "document", "transforms": [ENVELOPED], "digest": RIPEMD160}], "append_to": "root"},
        "prefix-none": {"prefix": "", "canonical": EXC, "references": [{"target": "document", "transforms": [ENVELOPED, EXC]}], "append_to": "root"},
        "prefix-custom": {"prefix": "sig", "canonical": EXC, "references": [{"target": "document", "transforms": [ENVELOPED, EXC]}], "append_to": "root"},
        "object": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}], "object": "datos del objeto", "object_mime": "text/plain", "object_encoding": "utf-8", "append_to": "root"},
        "cert-options": {"canonical": EXC, "references": [{"target": "document", "transforms": [ENVELOPED, EXC]}], "append_to": "root", "cert": key("a"), "cert_options": {"issuerSerial": True, "subjectName": True}},
        "append-signature-before": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}], "append_signature": "root", "insert_before": True},
        "not-appended": {"canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}]},
        "xpath-transform": {"canonical": C14N, "references": [{"target": "document", "transforms": [{"http://www.w3.org/TR/1999/REC-xpath-19991116": {"query": "not(ancestor-or-self::secdsig:Signature)", "namespaces": {"secdsig": DSIG}}}]}], "append_to": "root"},
        "no-canonical-method": {"references": [{"target": "id:d1", "options": {"overwrite": False}}], "append_to": "root"},
        "invalid-canonical": {"canonical": "urn:no-valido", "references": [{"target": "document"}], "append_to": "root"},
        "unsupported-digest": {"canonical": C14N, "references": [{"target": "document", "digest": "urn:no-valido"}], "append_to": "root"},
    }
    for name, params in sign_variants.items():
        for kname, k in {"rsa": PRIV, "hmac": hmac}.items():
            if kname == "hmac" and name not in ("enveloped-c14n", "id-existing-overwrite", "multiple-refs", "object"):
                continue
            cases.append(case(f"dsig-sign-{name}-{kname}", "dsig_sign", xml=doc, key=k, **params))
    for name in ("rsa-sha256", "rsa-sha384", "rsa-sha512"):
        alg = {"rsa-sha256": RSA_SHA256, "rsa-sha384": RSA_SHA384, "rsa-sha512": RSA_SHA512}[name]
        cases.append(case(f"dsig-sign-{name}", "dsig_sign", xml=doc, key={"alg": alg, "type": "private", "load": key("a")}, **sign_variants["enveloped-c14n"]))

    signed = {"xml": doc, "key": PRIV, "cert": key("a"), **sign_variants["enveloped-c14n"]}
    by_id = {"xml": doc, "key": PRIV, "cert": key("a"), **sign_variants["multiple-refs"]}
    cert_a = key("a", "crt")
    verify_variants = {
        "enveloped": {"sign": signed},
        "enveloped-wrong-key": {"sign": signed, "verify_with": key("b", "crt")},
        "enveloped-tampered": {"sign": signed, "mutations": [{"op": "replace", "from": "importe 10.00", "to": "importe 99.00"}]},
        "enveloped-comment-added": {"sign": signed, "mutations": [{"op": "replace", "from": "<other>", "to": "<!-- nuevo --><other>"}]},
        "by-id": {"sign": by_id, "verify_with": cert_a},
        "by-id-tampered-unsigned-part": {"sign": by_id, "verify_with": cert_a, "mutations": [{"op": "replace", "from": "<!-- c -->", "to": "<extra>no firmado</extra>"}]},
        "by-id-duplicate-before": {"sign": by_id, "verify_with": cert_a, "mutations": [{"op": "replace", "from": '<data Id="d1">', "to": '<data Id="d1">falso</data><data Id="d1">'}]},
        "by-id-duplicate-after": {"sign": by_id, "verify_with": cert_a, "mutations": [{"op": "replace", "from": "<!-- c -->", "to": '<data Id="d1">importe 99.00</data>'}]},
        "by-id-missing": {"sign": by_id, "verify_with": cert_a, "mutations": [{"op": "replace", "from": 'Id="d1"', "to": 'Id="d2"'}]},
        "id-keys": {"sign": {**signed, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"id_name": "ID", "overwrite": False}}]}, "verify_with": cert_a, "id_keys": ["ID"]},
        "external-uri": {"sign": signed, "verify_with": cert_a, "mutations": [{"op": "replace", "from": 'URI=""', "to": 'URI="http://127.0.0.1:9/x"'}]},
        "keyinfo-cert": {"sign": signed},
        "hmac": {"sign": {**signed, "key": hmac, "cert": None}, "verify_with": "clave-hmac", "verify_is_cert": False},
        "position-1": {"sign": signed, "position": 1},
        "xpath": {"sign": {**signed, **sign_variants["xpath-transform"]}, "verify_with": cert_a},
        "xpath-tampered": {"sign": {**signed, **sign_variants["xpath-transform"]}, "verify_with": cert_a,
                           "mutations": [{"op": "replace", "from": "importe 10.00", "to": "importe 99.00"}]},
        "xpath-unsigned-part": {"sign": {**signed, **sign_variants["xpath-transform"], "references": [{"target": "document", "transforms": [{XPATH: {"query": "not(ancestor-or-self::secdsig:Signature or ancestor-or-self::r:other)", "namespaces": {"secdsig": DSIG, "r": "urn:r"}}}]}]},
                                "verify_with": cert_a, "mutations": [{"op": "replace", "from": "<other>otro</other>", "to": "<other>cambiado</other>"}]},
    }
    for name, params in verify_variants.items():
        if params.get("sign", {}).get("cert", 1) is None:
            params["sign"] = {k: v for k, v in params["sign"].items() if k != "cert"}
        cases.append(case(f"dsig-verify-{name}", "dsig_verify", **params))
    return cases


# --------------------------------------------------------------------------
# Cifrado y X.509
# --------------------------------------------------------------------------

ENC_DOC = '<?xml version="1.0" encoding="utf-8"?><root xmlns="urn:r"><secret Id="s1">dato <b>sensible</b> &amp; ñ</secret><public>visible</public></root>'
ENC_KEY_INFO = {
    "keyname": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:KeyName>clave-1</ds:KeyName></ds:KeyInfo></t></root>',
    "rsakeyvalue": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:KeyValue><ds:RSAKeyValue><ds:Modulus>' + base64.b64encode(bytes([0xC3]) + bytes(range(1, 128))).decode() + '</ds:Modulus><ds:Exponent>AQAB</ds:Exponent></ds:RSAKeyValue></ds:KeyValue></ds:KeyInfo></t></root>',
    "rsakeyvalue-missing": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:KeyValue><ds:RSAKeyValue><ds:Modulus>AQAB</ds:Modulus></ds:RSAKeyValue></ds:KeyValue></ds:KeyInfo></t></root>',
    "dsakeyvalue": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:KeyValue><ds:DSAKeyValue/></ds:KeyValue></ds:KeyInfo></t></root>',
    "retrieval": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:xenc="http://www.w3.org/2001/04/xmlenc#"><t><ds:KeyInfo><ds:RetrievalMethod Type="http://www.w3.org/2001/04/xmlenc#EncryptedKey" URI="#k1"/></ds:KeyInfo></t><xenc:EncryptedKey Id="k1"><xenc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#rsa-1_5"/><xenc:CipherData><xenc:CipherValue>AAAA</xenc:CipherValue></xenc:CipherData></xenc:EncryptedKey></root>',
    "retrieval-missing": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:RetrievalMethod Type="http://www.w3.org/2001/04/xmlenc#EncryptedKey" URI="#nada"/></ds:KeyInfo></t></root>',
    "retrieval-other-type": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:RetrievalMethod Type="urn:otro" URI="#k1"/><ds:KeyName>n</ds:KeyName></ds:KeyInfo></t></root>',
    "retrieval-not-local": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:RetrievalMethod Type="http://www.w3.org/2001/04/xmlenc#EncryptedKey" URI="http://x/k"/></ds:KeyInfo></t></root>',
    "encryptedkey": '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:xenc="http://www.w3.org/2001/04/xmlenc#"><t><ds:KeyInfo><xenc:EncryptedKey><xenc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#rsa-oaep-mgf1p"/><ds:KeyInfo><ds:KeyName>interna</ds:KeyName></ds:KeyInfo></xenc:EncryptedKey></ds:KeyInfo></t></root>',
    "no-keyinfo": '<root><t/></root>',
}


def encryption() -> list[dict]:
    cases = []
    for sname, alg in {"aes128": AES128, "aes192": AES192, "aes256": AES256, "3des": TDES}.items():
        for tname, t in {"element": ELEMENT, "content": CONTENT}.items():
            for kname, kalg in {"rsa15": RSA15, "oaep": OAEP}.items():
                enc = {"xml": ENC_DOC, "session": {"alg": alg}, "target": "id:s1", "type": t, "key_transport": {"alg": kalg, "type": "public", "load": key("a", "crt"), "is_cert": True}}
                cases.append(case(f"enc-{sname}-{tname}-{kname}", "enc_encrypt", **enc))
                cases.append(case(f"dec-{sname}-{tname}-{kname}", "enc_decrypt", encrypt=enc, private_key=key("a")))
    base = {"xml": ENC_DOC, "session": {"alg": AES128}, "target": "id:s1", "type": ELEMENT}
    cases.append(case("enc-no-replace", "enc_encrypt", **base, replace=False))
    cases.append(case("enc-key-not-appended", "enc_encrypt", **base, key_transport={"alg": RSA15, "type": "public", "load": key("a", "crt"), "is_cert": True}, append_key=False))
    cases.append(case("enc-key-named", "enc_encrypt", **base, key_transport={"alg": RSA15, "type": "public", "load": key("a", "crt"), "is_cert": True, "name": "clave-sitio"}))
    cases.append(case("enc-document-element", "enc_encrypt", **{**base, "target": "root"}))
    cases.append(case("enc-invalid-type", "enc_encrypt", **{**base, "type": "urn:otro"}))
    cases.append(case("enc-unknown-session", "enc_encrypt", **{**base, "session": {"alg": RSA15, "type": "public"}}))
    cases.append(case("dec-no-key-transport-session", "enc_decrypt", encrypt={**base, "session": {"alg": AES128, "load": b64(bytes(16))}, "generate": False}, session_key=b64(bytes(16))))
    cases.append(case("dec-wrong-private-key", "enc_decrypt", encrypt={**base, "key_transport": {"alg": OAEP, "type": "public", "load": key("a", "crt"), "is_cert": True}}, private_key=key("b"), outcome_only=True))
    cases.append(case("dec-no-encrypted-data", "enc_decrypt", input=ENC_DOC))
    cases.append(case("dec-no-replace", "enc_decrypt", encrypt={**base, "session": {"alg": AES128, "load": b64(bytes(16))}, "generate": False}, session_key=b64(bytes(16)), replace=False))
    for name, xml in ENC_KEY_INFO.items():
        cases.append(case(f"keyinfo-{name}", "enc_key_info", xml=xml, target="tag:|t", base_key={"alg": RSA15, "type": "public"}))
    cases.append(case("keyinfo-x509", "enc_key_info", xml={"key_info_cert": "a"}, target="tag:|t", base_key={"alg": RSA_SHA1, "type": "public"}))
    cases.append(case("keyinfo-no-base", "enc_key_info", xml=ENC_KEY_INFO["keyname"], target="tag:|t"))
    cases.append(case("x509-pfx", "x509", pfx=key("a", "pfx"), password=key("pfx", "pass")))
    cases.append(case("x509-pfx-wrong-password", "x509", pfx=key("a", "pfx"), password="incorrecta"))
    # getPurposes()/getExtensions() con extensiones variadas (scripts/xmldsig_keys.sh).
    for profile in ("ca", "server", "tsa", "codesign", "nsca", "kuonly", "invalid", "saninvalid", "v1"):
        cases.append(case(f"x509-ext-{profile}", "x509", pfx=key(f"ext-{profile}", "pfx"), password=key("pfx", "pass")))
    cases.append(case("x509-not-pfx", "x509", pfx="no es un pfx", password="x"))
    return cases


# --------------------------------------------------------------------------
# Entradas hostiles
# --------------------------------------------------------------------------

LOL = '<!DOCTYPE r [<!ENTITY a "lol">' + "".join(f'<!ENTITY {chr(98 + i)} "{("&" + chr(97 + i) + ";") * 10}">' for i in range(9)) + "]><r>&j;</r>"


def deep(levels: int) -> str:
    return "<d>" * levels + "x" + "</d>" * levels


def hostile() -> list[dict]:
    cases = []
    xml = {
        "dtd-internal-entity": '<?xml version="1.0" encoding="utf-8"?><!DOCTYPE r [<!ENTITY e "valor">]><r>&e;</r>',
        "dtd-only": '<!DOCTYPE r><r>x</r>',
        "external-entity": '<!DOCTYPE r [<!ENTITY ext SYSTEM "file:///no/existe/secreto">]><r>&ext;</r>',
        "external-dtd": '<!DOCTYPE r SYSTEM "http://127.0.0.1:9/x.dtd"><r>x</r>',
        "parameter-entity": '<!DOCTYPE r [<!ENTITY % p SYSTEM "file:///no/existe"> %p;]><r/>',
        "billion-laughs": LOL,
        "undefined-entity": "<r>&nada;</r>",
        "deep-255": deep(255),
        "deep-256": deep(256),
        "deep-300": deep(300),
        "non-utf8-declared": '<?xml version="1.0" encoding="ISO-8859-1"?><r>x</r>',
    }
    for name, x in xml.items():
        cases.append(case(f"hostile-serialize-{name}", "serialize", xml=x))
    # 11 MiB en muchos nodos pequeños: supera el límite de 10 MiB de Rust.
    huge = {"repeat": {"prefix": "<r>", "unit": "<a>" + "x" * 1000 + "</a>", "count": 11 * 1024, "suffix": "</r>"}}
    near = {"repeat": {"prefix": "<r>", "unit": "<a>" + "x" * 1000 + "</a>", "count": 9 * 1024, "suffix": "</r>"}}
    cases.append(case("hostile-serialize-huge-11mib", "serialize", xml=huge, summary=True))
    cases.append(case("hostile-serialize-large-9mib", "serialize", xml=near, summary=True))
    nodes = {"repeat": {"prefix": "<r>", "unit": "<a/>", "count": 1_100_000, "suffix": "</r>"}}
    # Expresión XPath cuadrática: límite de coste deliberado en Rust.
    quadratic = {"repeat": {"prefix": "<r>", "unit": "<a/>", "count": 4_000, "suffix": "</r>"}}
    cases.append(case("hostile-c14n-xpath-quadratic", "c14n", xml=quadratic, target="document",
                      transforms=[{"algorithm": XPATH, "xpath": "count(following::*) >= 0"}], summary=True))
    cases.append(case("hostile-serialize-many-nodes", "serialize", xml=nodes, summary=True))
    base = {"xml": {"file": UBL_SIGN["inv21"]}, "cert": key("a")}
    dtd = '<!DOCTYPE Invoice [<!ENTITY e "F001-123">]>\n<Invoice'
    cases.append(case("hostile-sunat-sign-dtd", "sunat_sign", xml='<?xml version="1.0" encoding="utf-8"?><!DOCTYPE Invoice [<!ENTITY e "x">]><Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions><ID>&e;</ID></Invoice>', cert=key("a")))
    cases.append(case("hostile-sunat-verify-dtd-added", "sunat_verify", sign=base, mutations=[{"op": "replace", "from": "<Invoice", "to": dtd}]))
    cases.append(case("hostile-sunat-verify-entity-substitution", "sunat_verify", sign=base, mutations=[
        {"op": "replace", "from": "<Invoice", "to": dtd},
        {"op": "replace", "from": "<cbc:ID>F001-123</cbc:ID>", "to": "<cbc:ID>&e;</cbc:ID>"},
    ]))
    cases.append(case("hostile-sunat-verify-external-entity", "sunat_verify", sign=base, mutations=[
        {"op": "replace", "from": "<Invoice", "to": '<!DOCTYPE Invoice [<!ENTITY ext SYSTEM "file:///no/existe">]>\n<Invoice'},
        {"op": "replace", "from": "<cbc:ID>F001-123</cbc:ID>", "to": "<cbc:ID>&ext;</cbc:ID>"},
    ]))
    cases.append(case("hostile-sunat-verify-huge", "sunat_verify", input=huge))
    cases.append(case("hostile-dsig-verify-dtd-id", "dsig_verify", sign={"xml": '<?xml version="1.0" encoding="utf-8"?><root xmlns="urn:r"><data Id="d1">importe 10.00</data><other>otro</other></root>', "key": PRIV, "canonical": EXC, "references": [{"target": "id:d1", "transforms": [EXC], "options": {"overwrite": False}}], "append_to": "root"},
                      verify_with=key("a", "crt"), mutations=[{"op": "replace", "from": "<root", "to": '<!DOCTYPE root [<!ATTLIST data Id ID #IMPLIED>]><root'}]))
    return cases


def encoding() -> list[dict]:
    """XML declarado en codificaciones de un byte (bytes reales, en Base64)."""
    def b64(data: bytes) -> dict:
        return {"b64": base64.b64encode(data).decode("ascii")}

    def decl(name: str, body: bytes) -> bytes:
        return f'<?xml version="1.0" encoding="{name}"?>'.encode("ascii") + body

    cases = []
    rich = decl("ISO-8859-1", b'\n<r a="caf\xe9" b="&#8364; &#x10000;"><!-- coment\xe1rio -->'
                b'<![CDATA[\xe1 <x>]]><?pi d\xe1ta?>ni\xf1o &#8364; &#233; &#x10000;<e\xf1e x="\xbf"/></r>')
    cases.append(case("encoding-latin1-serialize", "serialize", xml=b64(rich)))
    cases.append(case("encoding-latin1-serialize-node", "serialize_node", xml=b64(rich), target="root"))
    for mname in ("c14n", "exc"):
        cases.append(case(f"encoding-latin1-{mname}", "c14n", xml=b64(rich), transforms=C14N_METHODS[mname], target="document"))
    simple = b"<r>\xe9 &#8364;</r>"
    for name in ("iso-8859-1", "ISO_8859-1", "ISO_8859-1:1987", "ISO8859-1", "ISO-IR-100", "ISO-LATIN-1",
                 "latin1", "L1", "CP819", "IBM819", "csISOLatin1", "ISO LATIN 1"):
        cases.append(case(f"encoding-alias-{name.replace(' ', '_').replace(':', '_')}", "serialize", xml=b64(decl(name, simple))))
    ascii_body = b'<r a="&#233;">x &#8364;</r>'
    for name in ("US-ASCII", "ascii", "ANSI_X3.4-1968", "ISO646-US", "US", "csASCII"):
        cases.append(case(f"encoding-ascii-{name}", "serialize", xml=b64(decl(name, ascii_body))))
    cases.append(case("encoding-ascii-invalid-byte", "serialize", xml=b64(decl("US-ASCII", b"<r>\xe9</r>"))))
    cp1252 = b"<r>\x80 \x9f \xe9 &#164; &#8218; &#937;</r>"
    for name in ("windows-1252", "CP1252", "MS-ANSI"):
        cases.append(case(f"encoding-cp1252-{name}", "serialize", xml=b64(decl(name, cp1252))))
    for byte in (0x81, 0x8D, 0x8F, 0x90, 0x9D):
        cases.append(case(f"encoding-cp1252-undefined-{byte:02x}", "serialize", xml=b64(decl("windows-1252", b"<r>" + bytes([byte]) + b"</r>"))))
    latin9 = b"<r>\xa4 \xbc \xe9 &#164; &#189; &#8364;</r>"
    for name in ("ISO-8859-15", "LATIN9", "latin-9", "ISO_8859-15"):
        cases.append(case(f"encoding-latin9-{name}", "serialize", xml=b64(decl(name, latin9))))
    utf8 = "<r>é €</r>".encode("utf-8")
    for name in ("UTF8", "utf-8"):
        cases.append(case(f"encoding-utf8-{name}", "serialize", xml=b64(decl(name, utf8))))
    cases.append(case("encoding-unknown-name", "serialize", xml=b64(decl("foo", b"<r>x</r>"))))
    cases.append(case("encoding-utf16-declared", "serialize", xml=b64(decl("UTF-16", b"<r>x</r>"))))
    cases.append(case("encoding-cyrillic-iso-8859-5", "serialize", xml=b64(decl("ISO-8859-5", b"<r>\xd0\xb0</r>"))))
    cases.append(case("encoding-undeclared-latin1-bytes", "serialize", xml=b64(b"<r>\xe9</r>")))
    cases.append(case("encoding-latin1-declared-as-utf8-text", "serialize", xml='<?xml version="1.0" encoding="ISO-8859-1"?><r>é</r>'))
    invoice = decl("ISO-8859-1", (
        b'<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" '
        b'xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" '
        b'xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">'
        b"<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent/></ext:UBLExtension></ext:UBLExtensions>"
        b"<cbc:ID>F001-1</cbc:ID><cbc:Note>Pe\xf1a S.A.C. &#8364; a\xf1o</cbc:Note></Invoice>"))
    sign = {"xml": b64(invoice), "cert": key("a")}
    cases.append(case("encoding-sunat-sign-latin1", "sunat_sign", **sign))
    cases.append(case("encoding-sunat-sign-latin1-sha256", "sunat_sign", **sign, signature_alg=RSA_SHA256, digest_alg=SHA256))
    cases.append(case("encoding-sunat-verify-latin1", "sunat_verify", sign=sign))
    cases.append(case("encoding-sunat-verify-latin1-dom", "sunat_verify", sign=sign, dom=True))
    cases.append(case("encoding-sunat-sign-cp1252", "sunat_sign", xml=b64(invoice.replace(b"ISO-8859-1", b"windows-1252", 1).replace(b"&#8364;", b"\x80")), cert=key("a")))
    return cases


FAMILIES = {"basics": basics, "keys": keys, "signing": signing, "encryption": encryption, "hostile": hostile, "encoding": encoding}


def main() -> None:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "tests/reference/xmldsig/cases"
    out.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    for family, build in FAMILIES.items():
        cases = build()
        for c in cases:
            if c["id"] in seen:
                raise SystemExit(f"id duplicado: {c['id']}")
            seen.add(c["id"])
        (out / f"{family}.json").write_text(json.dumps({"family": family, "cases": cases}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
