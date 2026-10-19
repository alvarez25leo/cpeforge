#!/usr/bin/env python3
"""SOAP HTTP en loopback con respuestas fijas por ruta: estados de error y CDR.

Reproduce lo que se observó en SUNAT BETA (octubre de 2026): el CDR llega en
un ZIP con una entrada de directorio `dummy/` y los envíos seguidos reciben
`401 Unauthorized`. Cada ruta es un caso de scripts/ws_http_probe.sh.

Uso: http_status_server.py <cases.json> <port_file> <log_file>
"""

import base64
import io
import json
import sys
import zipfile
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

cases = {c["id"]: c for c in json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))["cases"]}
port_file = Path(sys.argv[2])
log_file = Path(sys.argv[3])
log_file.write_text("")

SOAP = "http://schemas.xmlsoap.org/soap/envelope/"
SERVICE = "http://service.sunat.gob.pe"
FAULT = (f'<soap:Envelope xmlns:soap="{SOAP}"><soap:Body><soap:Fault>'
         '<faultcode>soap:Client.0156</faultcode><faultstring>El archivo esta vacio</faultstring>'
         '</soap:Fault></soap:Body></soap:Envelope>')
DECL = '<?xml version="1.0" encoding="UTF-8"?>'


def cdr_zip() -> str:
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w", compression=zipfile.ZIP_DEFLATED) as z:
        z.writestr(zipfile.ZipInfo("dummy/", date_time=(2026, 10, 17, 21, 31, 0)), b"")
        info = zipfile.ZipInfo("R-20000000001-01-F001-1.xml", date_time=(2026, 10, 17, 21, 31, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(info, cases["cdr-ok"]["xml"])
    return base64.b64encode(data.getvalue()).decode()


CDR = cdr_zip()
SOAP12 = "http://www.w3.org/2003/05/soap-envelope"
RESPONSE = f'<ns:sendBillResponse xmlns:ns="{SERVICE}"><applicationResponse>{CDR}</applicationResponse></ns:sendBillResponse>'
SEND_BILL = f'{DECL}<soap:Envelope xmlns:soap="{SOAP}"><soap:Body>{RESPONSE}</soap:Body></soap:Envelope>'


def env11(inner: str, attrs: str = "") -> bytes:
    return f'{DECL}<soap:Envelope xmlns:soap="{SOAP}"{attrs}>{inner}</soap:Envelope>'.encode()


def env12(inner: str, attrs: str = "") -> bytes:
    return f'{DECL}<env:Envelope xmlns:env="{SOAP12}"{attrs}>{inner}</env:Envelope>'.encode()


def fault11(code: str, string: str, extra: str = "") -> bytes:
    return env11(f'<soap:Body><soap:Fault><faultcode>{code}</faultcode>'
                 f'<faultstring>{string}</faultstring>{extra}</soap:Fault></soap:Body>')


def ok(body: bytes) -> tuple:
    return (200, "OK", "text/xml; charset=utf-8", body)

# ruta -> (estado, frase, Content-Type o None, cuerpo)
RESPONSES = {
    "cdr-dummy-dir": (200, "OK", "text/xml; charset=utf-8", SEND_BILL.encode()),
    "http-401-html": (401, "Unauthorized", "text/html", b"<html><body><h1>401 Unauthorized</h1></body></html>"),
    "http-401-empty": (401, "Unauthorized", None, b""),
    "http-500-xml-fault": (500, "Internal Server Error", "text/xml; charset=utf-8", (DECL + FAULT).encode()),
    "http-500-soap12-fault": (500, "Internal Server Error", "application/soap+xml", (DECL + FAULT).encode()),
    "http-500-prefix-ct": (500, "Internal Server Error", "text/", (DECL + FAULT).encode()),
    "http-500-xml-empty": (500, "Internal Server Error", "text/xml", b""),
    "http-503-plain-fault": (503, "Service Unavailable", "text/plain", FAULT.encode()),
    "http-503-plain-decl": (503, "Service Unavailable", "text/plain", (DECL + FAULT).encode()),
    "http-503-bom-decl": (503, "Service Unavailable", None, "\ufeff".encode() + (DECL + FAULT).encode()),
    # Estado < 400 con cuerpo que no es un envelope SOAP válido (php_packet_soap.c).
    "body-not-xml": (200, "OK", "text/plain", b"esto no es XML"),
    "body-empty-200": (200, "OK", "text/xml", b""),
    "body-html-200": (200, "OK", "text/html", b"<html><body>mantenimiento</body></html>"),
    "body-latin1-undeclared": ok(b'<soap:Envelope xmlns:soap="' + SOAP.encode() + b'"><soap:Body>\xe1</soap:Body></soap:Envelope>'),
    "body-dtd": ok(f'{DECL}<!DOCTYPE x [<!ENTITY a "b">]><soap:Envelope xmlns:soap="{SOAP}"><soap:Body>{RESPONSE}</soap:Body></soap:Envelope>'.encode()),
    "env-no-namespace": ok(f'{DECL}<Envelope><Body>{RESPONSE}</Body></Envelope>'.encode()),
    "env-other-namespace": ok(f'{DECL}<x:Envelope xmlns:x="urn:x"><x:Body>{RESPONSE}</x:Body></x:Envelope>'.encode()),
    "env-unqualified-attr": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body>', ' foo="1"')),
    "env-encstyle-11-bad": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body>', ' soap:encodingStyle="urn:x"')),
    "env-encstyle-11-ok": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body>', ' soap:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"')),
    "env-encstyle-12ns-on-11": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body>', f' xmlns:e12="{SOAP12}" e12:encodingStyle="urn:x"')),
    "env-other-ns-attr": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body>', ' xmlns:o="urn:o" o:a="1"')),
    "no-body": ok(env11('<soap:Header/>')),
    "empty-envelope": ok(env11('')),
    "body-unqualified": ok(env11(f'<Body>{RESPONSE}</Body>')),
    "two-headers": ok(env11(f'<soap:Header/><soap:Header/><soap:Body>{RESPONSE}</soap:Body>')),
    "header-comment-body": ok(env11(f'<soap:Header><x:h xmlns:x="urn:x">1</x:h></soap:Header><!-- c --> <soap:Body>{RESPONSE}</soap:Body>')),
    "body-then-header": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body><soap:Header/>')),
    "extra-after-body-11": ok(env11(f'<soap:Body>{RESPONSE}</soap:Body><x:extra xmlns:x="urn:x"/>')),
    "body-unqualified-attr-11": ok(env11(f'<soap:Body foo="1">{RESPONSE}</soap:Body>')),
    "body-encstyle-11-bad": ok(env11(f'<soap:Body soap:encodingStyle="urn:x">{RESPONSE}</soap:Body>')),
    "header-unqualified-attr": ok(env11(f'<soap:Header foo="1"/><soap:Body>{RESPONSE}</soap:Body>')),
    "header-encstyle-11-bad": ok(env11(f'<soap:Header soap:encodingStyle="urn:x"/><soap:Body>{RESPONSE}</soap:Body>')),
    "soap12-success": ok(env12(f'<env:Body>{RESPONSE}</env:Body>')),
    "soap12-extra-after-body": ok(env12(f'<env:Body>{RESPONSE}</env:Body><x:extra xmlns:x="urn:x"/>')),
    "soap12-body-unqualified-attr": ok(env12(f'<env:Body foo="1">{RESPONSE}</env:Body>')),
    "soap12-encstyle-env": ok(env12(f'<env:Body>{RESPONSE}</env:Body>', ' env:encodingStyle="urn:x"')),
    "soap12-fault": ok(env12('<env:Body><env:Fault><env:Code><env:Value>env:Sender</env:Value></env:Code>'
                             '<env:Reason><env:Text xml:lang="es">0156 - El archivo esta vacio</env:Text></env:Reason>'
                             '</env:Fault></env:Body>')),
    "missing-response": ok(env11('<soap:Body/>')),
    "response-without-cdr": ok(env11(f'<soap:Body><ns:sendBillResponse xmlns:ns="{SERVICE}"/></soap:Body>')),
    "response-other-element": ok(env11(f'<soap:Body><ns:otraResponse xmlns:ns="{SERVICE}"><applicationResponse>{CDR}</applicationResponse></ns:otraResponse></soap:Body>')),
    "fault-other-namespace": ok(env11('<soap:Body><x:Fault xmlns:x="urn:x"><faultcode>0156</faultcode><faultstring>x</faultstring></x:Fault></soap:Body>')),
    "fault-second-child": ok(env11('<soap:Body><x:otro xmlns:x="urn:x"/><soap:Fault><faultcode>soap:Client.0156</faultcode><faultstring>segundo</faultstring></soap:Fault></soap:Body>')),
    # Correspondencia fault -> error de Greenter (BaseSunat::getErrorFromFault).
    "fault-digits-in-string": ok(fault11("soap:Server", "0306 - No se puede leer el archivo")),
    "fault-no-digits": ok(fault11("soap:Server", "Error interno")),
    "fault-code-zero": ok(fault11("soap:Client.0", "1033 - El comprobante fue registrado")),
    "fault-empty": ok(env11('<soap:Body><soap:Fault/></soap:Body>')),
    "fault-detail-message": ok(fault11("soap:Client.0111", "No tiene el perfil", "<detail><message>Detalle del error</message></detail>")),
    "fault-detail-text": ok(fault11("soap:Client.0111", "No tiene el perfil", "<detail>texto plano</detail>")),
    "fault-detail-nested": ok(fault11("soap:Client.0111", "Sin perfil", "<detail><a>1</a><a>2</a><b><c>\u00f1/\u00e9</c></b></detail>")),
    "fault-detail-empty": ok(fault11("soap:Client.0111", "Sin perfil", "<detail/>")),
    "fault-cdata-entities": ok(fault11("soap:Client.0156", "<![CDATA[a & b]]> &amp; c")),
    "body-blank-200": (200, "OK", "text/xml", b" \n"),
    "soap12-fault-nodigits": ok(env12('<env:Body><env:Fault><env:Code><env:Value>env:Sender</env:Value></env:Code>'
                                      '<env:Reason><env:Text>Sin codigo</env:Text></env:Reason>'
                                      '<env:Detail><m>d</m></env:Detail></env:Fault></env:Body>')),
    "fault-no-code": ok(env11('<soap:Body><soap:Fault><faultstring>0156 sin codigo</faultstring></soap:Fault></soap:Body>')),
    "fault-code-element": ok(fault11("<x/>", "0156 texto")),
    "fault-no-string": ok(env11('<soap:Body><soap:Fault><faultcode>soap:Client.0156</faultcode></soap:Fault></soap:Body>')),
    "fault-cdata": ok(fault11("soap:Client.0156", "<![CDATA[a & <b>]]>")),
    "fault-cdata-ws": ok(fault11("soap:Client.0156", " <![CDATA[a & b]]> ")),
    "fault-cdata-two": ok(fault11("soap:Client.0156", "<![CDATA[a]]><![CDATA[b]]>")),
    "fault-string-element": ok(fault11("soap:Client.0156", "a<b>x</b>")),
    "fault-string-comment": ok(fault11("soap:Client.0156", "a<!-- c -->b &lt; c")),
    "fault-string-pi": ok(fault11("soap:Client.0156", "a<?pi x?>")),
    "fault-actor-element": ok(fault11("soap:Client.0156", "texto", "<faultactor>a<b/></faultactor>")),
    "fault-detail-mixed": ok(fault11("soap:Client.0111", "Sin perfil", "<detail>texto<m>1</m> <!-- c --> <n/></detail>")),
    "fault-detail-cdata-violation": ok(fault11("soap:Client.0111", "Sin perfil", "<detail><m>a<![CDATA[b]]></m></detail>")),
    "fault-detail-escapes": ok(fault11("soap:Client.0111", "Sin perfil", "<detail><m>&quot;q&quot; \\ \U0001F600\t\u00e9 &#127;</m><x:p xmlns:x='urn:x'>prefijo</x:p></detail>")),
    "fault-detail-nil": ok(fault11("soap:Client.0111", "Sin perfil", '<detail xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xsi:nil="true"/>')),
    "fault-detail-child-nil": ok(fault11("soap:Client.0111", "Sin perfil", '<detail xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema"><m xsi:nil="true"/><n>1</n></detail>')),
    "fault-detail-xsitype": ok(fault11("soap:Client.0111", "Sin perfil", '<detail xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema"><m xsi:type="xsd:int">5</m></detail>')),
    "fault-detail-repeat-split": ok(fault11("soap:Client.0111", "Sin perfil", "<detail><a>1</a><b>2</b><a>3</a></detail>")),
    # Diferencias registradas del perfil seguro (scripts/ws_http_probe.sh).
    "ns-undeclared-prefix": ok(b'<soap:Envelope><soap:Body/></soap:Envelope>'),
    "dtd-malformed": ok(f'{DECL}<!DOCTYPE x [<!ENTITY a "b">]><soap:Envelope xmlns:soap="{SOAP}"><soap:Body></soap:Envelope>'.encode()),
    "latin1-declared": ok(b'<?xml version="1.0" encoding="ISO-8859-1"?><soap:Envelope xmlns:soap="' + SOAP.encode()
                          + b'"><soap:Body><soap:Fault><faultcode>soap:Client.0156</faultcode><faultstring>Acci\xf3n</faultstring></soap:Fault></soap:Body></soap:Envelope>'),
    # sendSummary (SummarySender): mismo parser, otra lectura de la respuesta.
    "summary-ok": ok(env11(f'<soap:Body><ns:sendSummaryResponse xmlns:ns="{SERVICE}"><ticket>T-1</ticket></ns:sendSummaryResponse></soap:Body>')),
    "summary-missing-response": ok(env11('<soap:Body/>')),
    "summary-empty-200": (200, "OK", "text/xml", b""),
    "summary-fault-detail": ok(fault11("soap:Client.0151", "Nombre de archivo", "<detail><message>x</message></detail>")),
    "fault-prefixed-children": ok(env11('<soap:Body><soap:Fault><soap:faultcode>soap:Client.0156</soap:faultcode><soap:faultstring>prefijado</soap:faultstring></soap:Fault></soap:Body>')),
    # Profundidad total del documento (Envelope = 1). Rust admite 256 niveles
    # y rechaza el documento antes de parsearlo; ext/soap con libxml2 2.15
    # admite 2048 (XML_PARSE_HUGE) y la 2.9 no limita, así que 257 difiere en
    # ambos perfiles. Casos por encima de 2048 dependerían de la versión.
    "fault-detail-depth-256": ok(fault11("soap:Client.0111", "Profundo", "<detail>" + "<a>" * 252 + "x" + "</a>" * 252 + "</detail>")),
    "fault-detail-depth-257": ok(fault11("soap:Client.0111", "Profundo", "<detail>" + "<a>" * 253 + "x" + "</a>" * 253 + "</detail>")),
    "body-depth-257": ok(env11(f'<soap:Body><ns:sendBillResponse xmlns:ns="{SERVICE}"><applicationResponse>{CDR}</applicationResponse>'
                               + "<x>" * 254 + "</x>" * 254 + '</ns:sendBillResponse></soap:Body>')),
}


class Handler(BaseHTTPRequestHandler):
    # HTTP/1.1 con Content-Length explícito, como SUNAT.
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_POST(self):
        case_id = self.path.lstrip("/")
        length = int(self.headers.get("Content-Length", "0"))
        if case_id not in RESPONSES or length > 16 * 1024 * 1024:
            self.send_error(404)
            return
        self.rfile.read(length)
        with log_file.open("a", encoding="utf-8") as output:
            output.write(case_id + "\n")
        status, reason, content_type, body = RESPONSES[case_id]
        self.send_response(status, reason)
        if content_type is not None:
            self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


server = HTTPServer(("127.0.0.1", 0), Handler)
port_file.write_text(str(server.server_port))
server.serve_forever()
