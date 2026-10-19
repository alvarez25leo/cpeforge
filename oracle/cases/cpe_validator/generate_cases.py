#!/usr/bin/env python3
"""Casos sintéticos de paridad de greenter/cpe-validator v5.3.0.

Todas las hojas XSL y los XML se construyen aquí con datos ficticios. No se
copia la XSL de prueba upstream (tests/Resources/2.X/...) ni los XML de
packages/xml-parser/tests/Resources, y no se descarga ninguna regla SUNAT:
las diez rutas de XslPathResolver se rellenan con hojas sintéticas que solo
identifican el tipo, la ruta y los parámetros. Además se reutilizan, por
ruta, XML sintéticos ya versionados que generó Greenter PHP para
cpeforge-xml (tests/reference/xml/expected).

Cada caso tiene una lista de pasos (`steps`) que ejecutan, en el mismo orden
y sobre la misma instancia, el oráculo PHP (oracle/php/cpe_validator) y la
prueba Rust (crates/cpe-validator/tests/parity.rs). Los archivos de reglas
(`rules`) y los externos (`outside`) se materializan en un directorio
temporal por caso; `{ROOT}` y `{OUTSIDE}` se sustituyen por sus rutas.

`independent` describe la verificación que no usa ni PHP ni Rust
(scripts/cpe_validator_independent_verify.py): `rules` evalúa un mini DSL de
reglas con ElementTree y `re` de Python; `xsltproc` ejecuta la hoja con otro
procesador XSLT 1.0 y compara sus mensajes.

Uso: python3 -I oracle/cases/cpe_validator/generate_cases.py [salida.json]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

CBC = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
ROOT_NS = {
    "Invoice": "urn:oasis:names:specification:ubl:schema:xsd:Invoice-2",
    "CreditNote": "urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2",
    "DebitNote": "urn:oasis:names:specification:ubl:schema:xsd:DebitNote-2",
    "DespatchAdvice": "urn:oasis:names:specification:ubl:schema:xsd:DespatchAdvice-2",
    "SummaryDocuments": "urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1",
    "VoidedDocuments": "urn:sunat:names:specification:ubl:peru:schema:xsd:VoidedDocuments-1",
    "Retention": "urn:sunat:names:specification:ubl:peru:schema:xsd:Retention-1",
    "Perception": "urn:sunat:names:specification:ubl:peru:schema:xsd:Perception-1",
}
# Copia del mapa de XslPathResolver (la prueba Rust y el inventario lo
# contrastan con el código PHP).
PATHS = {
    "01": "2.X/ValidaExprRegFactura-2.0.1.xsl",
    "03": "2.X/ValidaExprRegBoleta-2.0.1.xsl",
    "07": "2.X/ValidaExprRegNC-2.0.1.xsl",
    "08": "2.X/ValidaExprRegND-2.0.1.xsl",
    "RC": "1.X/ValidaExprRegSummary-1.1.0.xsl",
    "RA": "1.X/ValidaExprRegSummaryVoided-1.0.1.xsl",
    "09": "2.X/ValidaExprRegGreRemitente-2.0.1.xsl",
    "20": "1.X/ValidaExprRegRetencion-1.0.3.xsl",
    "40": "1.X/ValidaExprRegPercepcion-1.0.1.xsl",
    "RR": "1.X/ValidaExprRegOtrosVoided-1.0.1.xsl",
}
M = "Greenter\\Validator\\Xml\\XsltFunctions::matches"
DECL = '<?xml version="1.0" encoding="UTF-8"?>\n'
EXPECTED_XML = "tests/reference/xml/expected"


def doc(root: str, body: str = "", ns: bool = True, extra: str = "") -> str:
    xmlns = f' xmlns="{ROOT_NS[root]}"' if ns and root in ROOT_NS else ""
    return f'{DECL}<{root}{xmlns} xmlns:cbc="{CBC}"{extra}>{body}</{root}>'


def invoice(body: str = "<cbc:UBLVersionID>2.1</cbc:UBLVersionID>") -> str:
    return doc("Invoice", body)


def xsl(body: str, extra_ns: str = "", params: bool = True) -> str:
    param = '<xsl:param name="nombreArchivoEnviado"/>' if params else ""
    return (
        f'{DECL}<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" '
        f'xmlns:cbc="{CBC}" xmlns:php="http://php.net/xsl"{extra_ns}>{param}{body}</xsl:stylesheet>'
    )


def template(body: str, match: str = "/") -> str:
    return f'<xsl:template match="{match}">{body}</xsl:template>'


def message(content: str, terminate: str | None = None) -> str:
    attr = f' terminate="{terminate}"' if terminate is not None else ""
    return f"<xsl:message{attr}>{content}</xsl:message>"


def value_of(select: str) -> str:
    return f'<xsl:value-of select="{select}"/>'


def call(args: str, func: str = "function") -> str:
    """`php:function('...matches', args)` como expresión XPath."""
    return f"php:{func}('{M}', {args})"


def rules_case(case_id: str, rules: list[dict], xml: str, filename: str = "20000000001-01-F001-1.xml") -> dict:
    return single_xsl_case(case_id, rule_xsl(rules), xml, filename, independent={"kind": "rules", "rules": rules})


def rule_xsl(rules: list[dict]) -> str:
    """Hoja generada a partir del mini DSL que evalúa el verificador independiente."""
    out = []
    for i, rule in enumerate(rules):
        if "param" in rule:
            out.append(message(f"{rule['level']}|{rule['code']}|{rule['message']}|" + value_of("$nombreArchivoEnviado"),
                               "yes" if rule.get("terminate") else None))
            continue
        var = f"v{i}"
        out.append(f'<xsl:variable name="{var}" select="cbc:{rule["select"]}"/>')
        test = f"{call(repr_xpath(rule['regex']) + ', string($' + var + ')')} = 1"
        if rule.get("negate", True):
            test = f"not({test})"
        content = f"concat('{rule['level']}|{rule['code']}|{rule['message']}|{rule['path']}|', ${var})"
        out.append(f'<xsl:if test="{test}">' + message(value_of(content), "yes" if rule.get("terminate") else None) + "</xsl:if>")
    return xsl(template("".join(out), "/*"))


def repr_xpath(text: str) -> str:
    if "'" in text:
        raise ValueError(text)
    return f"'{text}'"


def step_xsl(xml: str | dict, filename: str = "20000000001-01-F001-1.xml") -> dict:
    return {"op": "xsl_validate", "filename": filename, "xml": xml if isinstance(xml, dict) else {"xml": xml}}


def single_xsl_case(case_id: str, sheet: str, xml: str | dict = None, filename: str = "20000000001-01-F001-1.xml",
                    independent: dict | None = None, **extra) -> dict:
    case = {
        "id": case_id,
        "family": "xsl",
        "rules": {"main.xsl": sheet},
        "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(xml if xml is not None else invoice(), filename)],
    }
    if independent:
        case["independent"] = independent
    case.update(extra)
    return case


def parser_cases() -> list[dict]:
    raws = {
        "parser-null": None,
        "parser-empty": "",
        "parser-one-field": "xxx",
        "parser-two-fields": "a|b",
        "parser-three-fields": "a|b|c",
        "parser-upstream-three": "1|1023|INVALID",
        "parser-upstream-five": "2|2000|INVALID|/Invoice/LegalMonetaryTotal/PayableAmount|223.20",
        "parser-level-zero": "0|c|m",
        "parser-level-space": " 1|c|m",
        "parser-level-eleven": "11|c|m",
        "parser-level-letter": "E|c|m",
        "parser-four-fields": "1|c|m|p",
        "parser-five-fields": "1|c|m|p|v",
        "parser-six-fields": "1|c|m|p|v|x",
        "parser-trailing-separator": "1|c|m|p|v|",
        "parser-three-trailing": "1|c|",
        "parser-only-separators": "||",
        "parser-four-separators": "||||",
        "parser-many-separators": "|||||||",
        "parser-unicode": "1|ÑÁ|mensaje ñ € 𝄞|/Invoice/ñ|ü",
        "parser-inner-newline": "1|c|m\nx|p|v",
        "parser-spaces": " 2 | c | m | p | v ",
    }
    return [{"id": cid, "family": "parser", "steps": [{"op": "parse", "raw": raw}]} for cid, raw in raws.items()]


def path_cases() -> list[dict]:
    cases = [
        {"id": f"path-{code.lower()}", "family": "path", "steps": [{"op": "path", "base": "/reglas", "code": code}]}
        for code in PATHS
    ]
    for cid, base, code in [
        ("path-unknown-00", "/reglas", "00"),
        ("path-empty-code", "/reglas", ""),
        ("path-null-code", "/reglas", None),
        ("path-lowercase", "/reglas", "rc"),
        ("path-padded", "/reglas", " 01"),
        ("path-base-trailing-slash", "/reglas/", "01"),
        ("path-base-empty", "", "RR"),
        ("path-base-relative", "reglas/sunat", "09"),
        ("path-base-unicode", "/reglas/ñandú", "20"),
    ]:
        cases.append({"id": cid, "family": "path", "steps": [{"op": "path", "base": base, "code": code}]})
    return cases


def type_cases() -> list[dict]:
    c = f'xmlns:cbc="{CBC}"'
    inputs = {
        "type-invoice-default": invoice(),
        "type-invoice-03": doc("Invoice", "<cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode>"),
        "type-invoice-01": doc("Invoice", "<cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode>"),
        "type-invoice-3": doc("Invoice", "<cbc:InvoiceTypeCode>3</cbc:InvoiceTypeCode>"),
        "type-invoice-whitespace": doc("Invoice", "<cbc:InvoiceTypeCode> 03</cbc:InvoiceTypeCode>"),
        "type-invoice-nested-text": doc("Invoice", "<cbc:InvoiceTypeCode>0<x>3</x></cbc:InvoiceTypeCode>"),
        "type-invoice-deep": doc("Invoice", "<a><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode></a>"),
        "type-invoice-first-wins": doc("Invoice", "<cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode>"),
        "type-invoice-char-ref": doc("Invoice", "<cbc:InvoiceTypeCode>&#48;3</cbc:InvoiceTypeCode>"),
        "type-invoice-cdata": doc("Invoice", "<cbc:InvoiceTypeCode><![CDATA[03]]></cbc:InvoiceTypeCode>"),
        "type-invoice-cbc-other-uri": '<Invoice xmlns:cbc="urn:otro"><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode></Invoice>',
        "type-invoice-cbc-other-prefix": f'<Invoice xmlns:b="{CBC}"><b:InvoiceTypeCode>03</b:InvoiceTypeCode></Invoice>',
        "type-invoice-cbc-on-child": f"<Invoice><cbc:InvoiceTypeCode {c}>03</cbc:InvoiceTypeCode></Invoice>",
        "type-invoice-prefixed-root": f'<i:Invoice xmlns:i="urn:x" {c}><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode></i:Invoice>',
        "type-invoice-undeclared-root-prefix": f"<i:Invoice {c}><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode></i:Invoice>",
        "type-invoice-no-namespace": "<Invoice/>",
        "type-invoice-other-root-ns": '<Invoice xmlns="urn:otro"/>',
        "type-invoice-comment-pi": f"{DECL}<!-- c --><?pi x?><Invoice {c}><cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode></Invoice>",
        "type-invoice-dtd": f'<!DOCTYPE Invoice [<!ENTITY t "03">]><Invoice {c}><cbc:InvoiceTypeCode>&t;</cbc:InvoiceTypeCode></Invoice>',
        "type-credit-note": doc("CreditNote"),
        "type-debit-note": doc("DebitNote"),
        "type-summary": doc("SummaryDocuments"),
        "type-despatch": doc("DespatchAdvice"),
        "type-retention": doc("Retention"),
        "type-perception": doc("Perception"),
        "type-voided-ra": doc("VoidedDocuments", "<cbc:ID>RA-20240101-1</cbc:ID>"),
        "type-voided-rr": doc("VoidedDocuments", "<cbc:ID>RR-20240101-1</cbc:ID>"),
        "type-voided-rr-lowercase": doc("VoidedDocuments", "<cbc:ID>rr-20240101-1</cbc:ID>"),
        "type-voided-rr-not-prefix": doc("VoidedDocuments", "<cbc:ID>XRR-1</cbc:ID>"),
        "type-voided-rr-leading-space": doc("VoidedDocuments", "<cbc:ID> RR-1</cbc:ID>"),
        "type-voided-r-only": doc("VoidedDocuments", "<cbc:ID>R</cbc:ID>"),
        "type-voided-empty-id": doc("VoidedDocuments", "<cbc:ID/>"),
        "type-voided-no-id": doc("VoidedDocuments"),
        "type-voided-cbc-undefined": '<VoidedDocuments><cbc:ID xmlns:cbc="urn:x">RR</cbc:ID></VoidedDocuments>',
        "type-application-response": "<ApplicationResponse></ApplicationResponse>",
        "type-lowercase-root": "<invoice/>",
        "type-malformed": "<Invoice>",
        "type-not-xml": "texto plano",
    }
    cases = [{"id": cid, "family": "type", "steps": [{"op": "type_xml", "xml": {"xml": xml}}]} for cid, xml in inputs.items()]
    cases.append({"id": "type-empty", "family": "type", "steps": [{"op": "type_xml", "xml": {"xml": ""}}]})
    cases.append({"id": "type-null", "family": "type", "steps": [{"op": "type_xml", "xml": None}]})
    for cid, rel in [
        ("type-default-boleta21", "invoice/boleta21-default.xml"),
        ("type-default-voided", "voided/voided-full-default.xml"),
        ("type-default-reversion", "voided/reversion-full-default.xml"),
        ("type-default-summary", "summary/summary-full-default.xml"),
    ]:
        cases.append({"id": cid, "family": "type", "steps": [{"op": "type_xml", "xml": {"file": f"{EXPECTED_XML}/{rel}"}}]})
    return cases


def xsl_cases() -> list[dict]:
    inv_x = doc("Invoice", "<cbc:ID>x</cbc:ID>")
    cases = [
        {"id": "xsl-not-set", "family": "xsl", "steps": [step_xsl(invoice())]},
        single_xsl_case("xsl-zero-messages", xsl(template(value_of("'sin mensajes'"))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-two-messages", xsl(template(message("1|A|m1|p|v") + message("2|B|m2"))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-terminate-yes", xsl(template(message("1|A|m1|p|v", "yes") + message("2|B|m2"))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-terminate-no", xsl(template(message("1|A|m1", "no") + message("2|B|m2", "no"))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-terminate-invalid", xsl(template(message("1|A|x", "maybe") + message("1|B|y")))),
        single_xsl_case("xsl-message-trailing-newlines", xsl(template(message("1|A|m1&#10;&#10;")))),
        single_xsl_case("xsl-message-inner-newline", xsl(template(message("1|A|m1&#10;x|y")))),
        single_xsl_case("xsl-message-empty-then-text", xsl(template(message("") + message("1|A|z")))),
        single_xsl_case("xsl-message-non-error-text", xsl(template(message("aviso sin formato") + message("1|A|despues"))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-message-two-fields", xsl(template(message("1|A") + message("1|B|ok")))),
        single_xsl_case("xsl-message-markup", xsl(template(message("1|A|<b>negrita</b>&amp;&lt;|p|v")))),
        single_xsl_case("xsl-message-unicode", xsl(template(message("2|ÑÁ|Descripción € 𝄞|/Invoice/ñ|ü")))),
        single_xsl_case(
            "xsl-message-order-templates",
            xsl(template('<xsl:for-each select="*/*">' + message("2|E|" + value_of("concat(position(), local-name())")) + "</xsl:for-each>"
                         + '<xsl:call-template name="t"/>') + '<xsl:template name="t">' + message("1|T|fin") + "</xsl:template>"),
            doc("Invoice", "<cbc:ID>1</cbc:ID><cbc:Note>n</cbc:Note><cbc:Other/>"),
            independent={"kind": "xsltproc"},
        ),
        single_xsl_case(
            "xsl-many-messages",
            xsl(template('<xsl:call-template name="r"><xsl:with-param name="n" select="60"/></xsl:call-template>')
                + '<xsl:template name="r"><xsl:param name="n"/><xsl:if test="$n &gt; 0">'
                + message("2|" + value_of("$n") + "|repeticion")
                + '<xsl:call-template name="r"><xsl:with-param name="n" select="$n - 1"/></xsl:call-template></xsl:if></xsl:template>'),
            independent={"kind": "xsltproc"},
        ),
        single_xsl_case("xsl-param-filename", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), independent={"kind": "xsltproc"}),
        single_xsl_case("xsl-param-quotes", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), filename="a'b\"c.xml"),
        single_xsl_case("xsl-param-unicode", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), filename="ñandú-€.xml"),
        single_xsl_case("xsl-param-empty", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), filename=""),
        single_xsl_case("xsl-param-pipe", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), filename="a|b|c.xml"),
        single_xsl_case("xsl-param-nul", xsl(template(message("1|P|" + value_of("$nombreArchivoEnviado")))), filename="a\u0000b.xml"),
        single_xsl_case("xsl-param-undeclared", xsl(template(message("1|P|sin-param")), params=False)),
        single_xsl_case("xsl-matches-true", xsl(template(message("1|R|" + value_of(call("'^x$', string(//cbc:ID)"))))), inv_x),
        single_xsl_case("xsl-matches-false", xsl(template(message("1|R|" + value_of(call("'^y$', string(//cbc:ID)"))))), inv_x),
        single_xsl_case("xsl-matches-in-test", xsl(template('<xsl:if test="' + call("'^x$', 'x'") + ' = 1">' + message("1|R|si") + "</xsl:if>"))),
        single_xsl_case("xsl-matches-false-comparison", xsl(template('<xsl:if test="not(' + call("'(', 'a'") + ' = 1)">' + message("1|R|falla") + "</xsl:if>"))),
        single_xsl_case("xsl-matches-invalid-regex", xsl(template(message("1|R|" + value_of(call("'(', 'a'")))))),
        single_xsl_case("xsl-matches-nodeset", xsl(template(message("1|R|" + value_of(call("'x', //cbc:ID"))))), inv_x),
        single_xsl_case("xsl-matches-empty-nodeset", xsl(template(message("1|R|" + value_of(call("'x', //cbc:Nada"))))), inv_x),
        single_xsl_case("xsl-matches-nodeset-pattern", xsl(template(message("1|R|" + value_of(call("//cbc:ID, 'x'"))))), inv_x),
        single_xsl_case("xsl-matches-rtf", xsl('<xsl:variable name="v">abc</xsl:variable>' + template(message("1|R|" + value_of(call("'b', $v")))))),
        single_xsl_case("xsl-matches-bool-true", xsl(template(message("1|R|" + value_of(call("'^1$', true()")))))),
        single_xsl_case("xsl-matches-bool-false", xsl(template(message("1|R|" + value_of(call("'^$', false()")))))),
        single_xsl_case("xsl-matches-one-arg", xsl(template(message("1|R|" + value_of(call("'1'")))))),
        single_xsl_case("xsl-matches-no-args", xsl(template(message("1|R|" + value_of(f"php:function('{M}')"))))),
        single_xsl_case("xsl-matches-extra-arg", xsl(template(message("1|R|" + value_of(call("'a', 'a', 'z'")))))),
        single_xsl_case("xsl-function-string-nodeset", xsl(template(message("1|R|" + value_of(call("'^x$', //*", "functionString"))))), inv_x),
        single_xsl_case("xsl-function-string-empty", xsl(template(message("1|R|" + value_of(call("'^$', //cbc:Nada", "functionString"))))), inv_x),
        single_xsl_case("xsl-function-other-handler", xsl(template(message("1|R|" + value_of("php:function('strlen', 'abc')")) + message("1|S|despues")))),
        single_xsl_case("xsl-function-case-mismatch", xsl(template(message("1|R|" + value_of("php:function('cpeforge\\validator\\xml\\xsltfunctions::MATCHES', 'a', 'a')"))))),
        single_xsl_case("xsl-function-leading-backslash", xsl(template(message("1|R|" + value_of("php:function('\\" + M + "', 'a', 'a')"))))),
        single_xsl_case("xsl-function-no-name", xsl(template(message("1|R|" + value_of("php:function()"))))),
        single_xsl_case("xsl-function-number-name", xsl(template(message("1|R|" + value_of("php:function(1)"))))),
        single_xsl_case("xsl-matches-slash", xsl(template(message("1|R|" + value_of(call("'a/b', 'a/b'")))))),
        single_xsl_case("xsl-matches-escaped-slash", xsl(template(message("1|R|" + value_of(call("'a\\/i', 'A\\'")))))),
        single_xsl_case("xsl-matches-trailing-backslash", xsl(template(message("1|R|" + value_of(call("'a\\', 'a'")))))),
        single_xsl_case("xsl-matches-backtrack-limit", xsl(template(message("1|R|" + value_of(call("'^(a+)+$', '" + "a" * 36 + "b'")))))),
        single_xsl_case("xsl-matches-byte-mode", xsl(template(message("1|R|" + value_of(call("'^.$', 'ñ'")) + "|" + value_of(call("'^..$', 'ñ'")))))),
        single_xsl_case("xsl-matches-empty-pattern", xsl(template(message("1|R|" + value_of(call("'', 'abc'")))))),
        single_xsl_case("xsl-matches-anchors", xsl(template(message("1|R|" + value_of(call("'^2\\.1$', '2.1&#10;'")) + "|" + value_of(call("'^2\\.1$', 'x2.1'")))))),
        single_xsl_case("xsl-matches-pcre-syntax", xsl(template(message("1|R|" + value_of(call("'^(?i)abc$', 'ABC'")) + "|" + value_of(call("'^\\d{3}(?=x)', '123x'")) + "|" + value_of(call("'\\bF\\w+', 'xF001'")))))),
        single_xsl_case("xsl-matches-lookbehind-k", xsl(template(message("1|R|" + value_of(call("'(?&lt;=a\\Kb)', 'ab'")))))),
        single_xsl_case("xsl-runtime-undefined-variable", xsl(template(message("1|A|antes") + value_of("$noDeclarada") + message("1|B|despues")))),
        single_xsl_case("xsl-runtime-unknown-function", xsl(template(message("1|A|antes") + '<xsl:value-of select="foo:bar()" xmlns:foo="urn:foo"/>' + message("1|B|despues")))),
        single_xsl_case("xsl-runtime-recursion-limit", xsl(template(message("1|A|antes") + '<xsl:call-template name="loop"/>') + '<xsl:template name="loop"><xsl:call-template name="loop"/></xsl:template>')),
        single_xsl_case("xsl-runtime-bad-element-name", xsl(template(message("1|A|antes") + '<xsl:element name="{concat(\'1|2|\', \'x\')}"/>' + message("1|B|despues")))),
        single_xsl_case("xsl-exslt-strings", xsl(template(message("1|E|" + value_of("str:padding(3, 'x')"))), ' xmlns:str="http://exslt.org/strings" extension-element-prefixes="str"')),
        single_xsl_case("xsl-compile-unknown-element", xsl(template("<xsl:bogus/>" + message("1|A|x")))),
        single_xsl_case("xsl-compile-bad-xpath", xsl(template(value_of("1 +") + message("1|A|x")))),
        rules_case(
            "xsl-rules-ubl-valid",
            [{"select": "UBLVersionID", "regex": "^(2.1)$", "level": "1", "code": "2074",
                       "message": "UBLVersionID - La versión del UBL no es correcta", "path": "Invoice/UBLVersionID", "terminate": True}],
            invoice("<cbc:UBLVersionID>2.1</cbc:UBLVersionID>"),
        ),
        rules_case(
            "xsl-rules-ubl-invalid",
            [{"select": "UBLVersionID", "regex": "^(2.1)$", "level": "1", "code": "2074",
                       "message": "UBLVersionID - La versión del UBL no es correcta", "path": "Invoice/UBLVersionID", "terminate": True}],
            invoice("<cbc:UBLVersionID>2.0</cbc:UBLVersionID>"),
        ),
        rules_case(
            "xsl-rules-several",
            [
                {"select": "UBLVersionID", "regex": "^(2.1)$", "level": "2", "code": "4001", "message": "version", "path": "Invoice/UBLVersionID"},
                {"select": "CustomizationID", "regex": "^2\\.0$", "level": "2", "code": "4002", "message": "customization", "path": "Invoice/CustomizationID"},
                {"select": "ID", "regex": "^[FB][A-Z0-9]{3}-[0-9]{1,8}$", "level": "1", "code": "1001", "message": "serie-correlativo", "path": "Invoice/ID"},
                {"param": True, "level": "2", "code": "4003", "message": "archivo"},
                {"select": "Note", "regex": "^.{1,10}$", "level": "1", "code": "3027", "message": "nota", "path": "Invoice/Note", "terminate": True},
                {"select": "Nunca", "regex": "^$", "level": "1", "code": "9999", "message": "nunca", "path": "Invoice/Nunca", "negate": False},
            ],
            invoice("<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>2.0</cbc:CustomizationID>"
                    "<cbc:ID>F001-123456789</cbc:ID><cbc:Note>texto demasiado largo</cbc:Note>"),
        ),
        rules_case(
            "xsl-rules-matching-nothing",
            [
                {"select": "ID", "regex": "^[FB][A-Z0-9]{3}-[0-9]{1,8}$", "level": "1", "code": "1001", "message": "serie", "path": "Invoice/ID"},
                {"select": "IssueDate", "regex": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$", "level": "1", "code": "1002", "message": "fecha", "path": "Invoice/IssueDate"},
            ],
            invoice("<cbc:ID>F001-1</cbc:ID><cbc:IssueDate>2024-01-31</cbc:IssueDate>"),
        ),
        rules_case(
            "xsl-rules-unicode",
            [{"select": "Note", "regex": "^[A-Za-z ]+$", "level": "2", "code": "4100", "message": "nota ñ", "path": "Invoice/Note"}],
            invoice("<cbc:Note>Año ñandú</cbc:Note>"),
        ),
        single_xsl_case("xsl-xml-dtd", xsl(template(message("1|A|" + value_of("string(/*)")))),
                        {"xml": '<!DOCTYPE Invoice [<!ENTITY e "entidad">]><Invoice>&e;</Invoice>'}),
        single_xsl_case("xsl-sheet-dtd", '<!DOCTYPE xsl:stylesheet [<!ENTITY e "entidad">]>' + xsl(template(message("1|A|&e;")))[len(DECL):]),
        {
            "id": "xsl-reuse-two-documents",
            "family": "xsl",
            "rules": {"main.xsl": rule_xsl([{"select": "UBLVersionID", "regex": "^(2.1)$", "level": "1", "code": "2074",
                                             "message": "version", "path": "Invoice/UBLVersionID", "terminate": True}])},
            "steps": [
                {"op": "xsl_set", "path": "main.xsl"},
                step_xsl(invoice("<cbc:UBLVersionID>2.0</cbc:UBLVersionID>"), "a.xml"),
                step_xsl(invoice("<cbc:UBLVersionID>2.1</cbc:UBLVersionID>"), "b.xml"),
                step_xsl(invoice("<cbc:UBLVersionID>1.0</cbc:UBLVersionID>"), "c.xml"),
            ],
        },
        {
            "id": "xsl-reuse-replace-stylesheet",
            "family": "xsl",
            "rules": {"a.xsl": xsl(template(message("1|A|primera"))), "b.xsl": xsl(template(message("1|B|segunda")))},
            "steps": [{"op": "xsl_set", "path": "a.xsl"}, step_xsl(invoice()), {"op": "xsl_set", "path": "b.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-reuse-after-exception",
            "family": "xsl",
            "rules": {"bad.xsl": xsl(template(message("1|R|" + value_of(call("'x', //cbc:ID"))))),
                      "good.xsl": xsl(template(message("1|G|limpio")))},
            "steps": [{"op": "xsl_set", "path": "bad.xsl"}, step_xsl(inv_x), {"op": "xsl_set", "path": "good.xsl"}, step_xsl(inv_x)],
        },
        {
            "id": "xsl-document-inside-root",
            "family": "xsl",
            "rules": {"main.xsl": xsl(template(message("1|D|" + value_of("count(document('datos.xml')/o)")))), "datos.xml": "<o>1</o>"},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-document-outside-root",
            "family": "xsl",
            "rules": {"main.xsl": xsl(template(message("1|D|" + value_of("count(document('{OUTSIDE}/datos.xml')/o)"))))},
            "outside": {"datos.xml": "<o>1</o>"},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-document-network",
            "family": "xsl",
            "rules": {"main.xsl": xsl(template(message("1|D|" + value_of("count(document('http://127.0.0.1:9/x.xml'))"))))},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-include-inside-root",
            "family": "xsl",
            "rules": {"main.xsl": xsl('<xsl:include href="comun/inc.xsl"/>' + template('<xsl:call-template name="t"/>')),
                      "comun/inc.xsl": xsl('<xsl:template name="t">' + message("1|INC|dentro") + "</xsl:template>", params=False)},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-import-inside-root",
            "family": "xsl",
            "rules": {"main.xsl": xsl('<xsl:import href="base.xsl"/><xsl:param name="nombreArchivoEnviado"/>'
                                      + template('<xsl:apply-imports/>' + message("1|MAIN|principal")), params=False),
                      "base.xsl": xsl(template(message("2|BASE|importada")), params=False)},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-include-outside-root",
            "family": "xsl",
            "rules": {"main.xsl": xsl('<xsl:include href="../outside/inc.xsl"/>' + template('<xsl:call-template name="t"/>'))},
            "outside": {"inc.xsl": xsl('<xsl:template name="t">' + message("1|INC|fuera") + "</xsl:template>", params=False)},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-include-missing",
            "family": "xsl",
            "rules": {"main.xsl": xsl('<xsl:include href="noexiste.xsl"/>' + template(message("1|A|x")))},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-include-entity",
            "family": "xsl",
            "rules": {"main.xsl": xsl('<xsl:include href="inc.xsl"/>' + template('<xsl:call-template name="t"/>')),
                      "inc.xsl": '<!DOCTYPE xsl:stylesheet [<!ENTITY e "entidad">]>' + xsl('<xsl:template name="t">' + message("1|INC|&e;") + "</xsl:template>", params=False)[len(DECL):]},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice())],
        },
        {
            "id": "xsl-exsl-document-write",
            "family": "xsl",
            "rules": {"main.xsl": xsl(template('<exsl:document href="{ROOT}/escrito.xml"><x/></exsl:document>' + message("1|W|despues")),
                                      ' xmlns:exsl="http://exslt.org/common" extension-element-prefixes="exsl"')},
            "steps": [{"op": "xsl_set", "path": "main.xsl"}, step_xsl(invoice()), {"op": "exists", "path": "escrito.xml"}],
        },
    ]
    # Conversión PHP float → string (precision=14) de números XPath pasados a matches.
    numbers = [
        ("1", "^1$"), ("0.1", "^0\\.1$"), ("1 div 3", "^0\\.33333333333333$"), ("2 div 3", "^0\\.66666666666667$"),
        ("100000000000000", "^1\\.0E\\+14$"), ("10000000000000", "^10000000000000$"),
        ("1000000000000000", "^1\\.0E\\+15$"), ("123456789012345678", "^1\\.2345678901235E\\+17$"),
        ("0.0001", "^0\\.0001$"), ("0.00001", "^1\\.0E-5$"), ("0.000012345", "^1\\.2345E-5$"),
        ("-0", "^-0$"), ("-1.5", "^-1\\.5$"), ("0 div 0", "^NAN$"), ("1 div 0", "^INF$"), ("-1 div 0", "^-INF$"),
        ("12345678901234.5", "^12345678901234$"), ("99999999999999.5", "^1\\.0E\\+14$"), ("0.1 + 0.2", "^0\\.3$"),
    ]
    for i, (expr, regex) in enumerate(numbers):
        cases.append(single_xsl_case(
            f"xsl-number-{i:02d}",
            xsl(template(message(f"1|N|{expr}|" + value_of(call(f"'{regex}', {expr}")) + "|" + value_of(call(f"'{regex}', {expr}", "functionString"))))),
        ))
    return cases


def type_rule(code: str, rel: str) -> str:
    return xsl(template(
        message(f"1|{code}|{rel}|" + value_of("$nombreArchivoEnviado") + "|" + value_of("local-name(/*)"))
        + message(f"2|OBS-{code}|segunda|/{rel}|" + value_of("count(/*/*)"), "yes")
        + message("1|NUNCA|tras terminate")
    ))


def all_rules() -> dict:
    return {rel: type_rule(code, rel) for code, rel in PATHS.items()}


def cpe_step(xml: str | dict | None, filename: str = "20000000001-01-F001-1.xml", mode: str = "xml") -> list[dict]:
    payload = xml if isinstance(xml, dict) or xml is None else {"xml": xml}
    return [{"op": "type", "xml": payload}, {"op": "rule", "xml": payload},
            {"op": "cpe_validate", "mode": mode, "filename": filename, "xml": payload}]


def cpe_cases() -> list[dict]:
    samples = {
        "01": invoice(), "03": doc("Invoice", "<cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode>"),
        "07": doc("CreditNote", "<cbc:ID>FC01-1</cbc:ID>"), "08": doc("DebitNote", "<cbc:ID>FD01-1</cbc:ID>"),
        "RC": doc("SummaryDocuments", "<cbc:ID>RC-20240101-1</cbc:ID>"), "RA": doc("VoidedDocuments", "<cbc:ID>RA-20240101-1</cbc:ID>"),
        "09": doc("DespatchAdvice", "<cbc:ID>T001-1</cbc:ID>"), "20": doc("Retention", "<cbc:ID>R001-1</cbc:ID>"),
        "40": doc("Perception", "<cbc:ID>P001-1</cbc:ID>"), "RR": doc("VoidedDocuments", "<cbc:ID>RR-20240101-1</cbc:ID>"),
    }
    cases = []
    for code, xml in samples.items():
        cases.append({"id": f"cpe-type-{code.lower()}", "family": "cpe", "rules": all_rules(),
                      "steps": cpe_step(xml, f"20000000001-{code}-X-1.xml"), "independent": {"kind": "xsltproc"}})
    upstream_like = rule_xsl([{"select": "UBLVersionID", "regex": "^(2.1)$", "level": "1", "code": "2074",
                               "message": "UBLVersionID - La versión del UBL no es correcta", "path": "Invoice/UBLVersionID", "terminate": True}])
    factura = {PATHS["01"]: upstream_like}
    cases += [
        {"id": "cpe-upstream-valid", "family": "cpe", "rules": factura, "steps": cpe_step(invoice())},
        {"id": "cpe-upstream-errors-dom", "family": "cpe", "rules": factura,
         "steps": cpe_step(invoice("<cbc:UBLVersionID>2.0</cbc:UBLVersionID>"), mode="dom")},
        {"id": "cpe-unknown-root", "family": "cpe", "rules": all_rules(),
         "steps": cpe_step('<ApplicationResponse xmlns="urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2"/>')},
        {"id": "cpe-unmapped-type", "family": "cpe", "rules": all_rules(), "resolver": "unmapped_type",
         "steps": [{"op": "cpe_validate", "mode": "xml", "filename": "x.xml", "xml": {"xml": invoice()}}]},
        {"id": "cpe-missing-rule-file", "family": "cpe", "rules": {}, "steps": cpe_step(invoice())},
        {"id": "cpe-malformed-rule", "family": "cpe", "rules": {PATHS["01"]: "<xsl:stylesheet"}, "steps": cpe_step(invoice())},
        {"id": "cpe-empty-rule-file", "family": "cpe", "rules": {PATHS["01"]: ""}, "steps": cpe_step(invoice())},
        {"id": "cpe-not-a-stylesheet", "family": "cpe", "rules": {PATHS["01"]: "<foo/>"}, "steps": cpe_step(invoice())},
        {"id": "cpe-compile-error-rule", "family": "cpe", "rules": {PATHS["01"]: xsl(template(value_of("1 +")))}, "steps": cpe_step(invoice())},
        {"id": "cpe-malformed-xml", "family": "cpe", "rules": all_rules(), "steps": cpe_step("<Invoice")},
        {"id": "cpe-empty-xml", "family": "cpe", "rules": all_rules(), "steps": cpe_step({"xml": ""})},
        {"id": "cpe-xml-dtd", "family": "cpe", "rules": all_rules(),
         "steps": cpe_step(f'<!DOCTYPE Invoice [<!ENTITY t "03">]><Invoice xmlns:cbc="{CBC}"><cbc:InvoiceTypeCode>&t;</cbc:InvoiceTypeCode></Invoice>')},
        {"id": "cpe-voided-without-id", "family": "cpe", "rules": all_rules(), "steps": cpe_step(doc("VoidedDocuments"))},
        {"id": "cpe-xml-over-limit", "family": "cpe", "rules": all_rules(), "rust_limits": {"max_xml_bytes": 512},
         "steps": cpe_step(invoice("<cbc:Note>" + "x" * 1024 + "</cbc:Note>"))},
        {"id": "cpe-include-outside-root", "family": "cpe",
         "rules": {PATHS["01"]: xsl('<xsl:include href="../../outside/inc.xsl"/>' + template('<xsl:call-template name="t"/>'))},
         "outside": {"inc.xsl": xsl('<xsl:template name="t">' + message("1|INC|fuera") + "</xsl:template>", params=False)},
         "steps": cpe_step(invoice())},
        {"id": "cpe-include-inside-root", "family": "cpe",
         "rules": {PATHS["01"]: xsl('<xsl:include href="../comun/inc.xsl"/>' + template('<xsl:call-template name="t"/>')),
                   "comun/inc.xsl": xsl('<xsl:template name="t">' + message("1|INC|comun") + "</xsl:template>", params=False)},
         "steps": cpe_step(invoice())},
        {"id": "cpe-symlink-escape", "family": "cpe", "rules": {}, "symlinks": {PATHS["01"]: "{OUTSIDE}/regla.xsl"},
         "outside": {"regla.xsl": xsl(template(message("1|LINK|fuera")))}, "steps": cpe_step(invoice())},
        {"id": "cpe-filename-param", "family": "cpe", "rules": all_rules(), "steps": cpe_step(invoice(), "nombre con espacios 'y' comillas.xml")},
        {"id": "cpe-reuse-sequence", "family": "cpe", "rules": all_rules(),
         "steps": cpe_step(samples["01"], "a.xml") + cpe_step(samples["RR"], "b.xml") + cpe_step("<Otro/>", "c.xml") + cpe_step(samples["03"], "d.xml")},
        {"id": "cpe-default-boleta21", "family": "cpe", "rules": all_rules(), "steps": cpe_step({"file": f"{EXPECTED_XML}/invoice/boleta21-default.xml"})},
        {"id": "cpe-default-reversion", "family": "cpe", "rules": all_rules(), "steps": cpe_step({"file": f"{EXPECTED_XML}/voided/reversion-full-default.xml"})},
    ]
    return cases


def main() -> int:
    cases = parser_cases() + path_cases() + type_cases() + xsl_cases() + cpe_cases()
    ids = [c["id"] for c in cases]
    if len(ids) != len(set(ids)):
        raise SystemExit("ids duplicados")
    out = {
        "source": "thegreenter/greenter f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f packages/cpe-validator",
        "generator": "oracle/cases/cpe_validator/generate_cases.py",
        "matches": M,
        "paths": PATHS,
        "cases": cases,
    }
    text = json.dumps(out, indent=1, ensure_ascii=False) + "\n"
    if len(sys.argv) > 1:
        Path(sys.argv[1]).write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    print(f"{len(cases)} casos", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
