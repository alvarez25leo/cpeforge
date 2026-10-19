#!/usr/bin/env python3
"""Servidor SOAP local y verificador WSDL/XSD independiente del cliente Rust."""

import base64
import io
import json
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zipfile
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

cases = {c["id"]: c for c in json.loads(Path(sys.argv[1]).read_text())["cases"]}
wsdl = Path(sys.argv[2])
port_file = Path(sys.argv[3])
log_file = Path(sys.argv[4])
log_file.write_text("")
SOAP = "http://schemas.xmlsoap.org/soap/envelope/"
SERVICE = "http://service.sunat.gob.pe"
cdr_xml = cases["cdr-ok"]["xml"]
archive_bytes = io.BytesIO()
with zipfile.ZipFile(archive_bytes, "w", compression=zipfile.ZIP_DEFLATED) as z:
    z.writestr("R-20000000001-01-F001-1.xml", cdr_xml)
cdr_zip64 = base64.b64encode(archive_bytes.getvalue()).decode()


def soap(body):
    return (f'<?xml version="1.0" encoding="UTF-8"?>'
            f'<soap:Envelope xmlns:soap="{SOAP}"><soap:Body>{body}</soap:Body></soap:Envelope>').encode()


def response(case_id):
    if case_id == "soap-send-bill":
        return 200, soap(f'<ns:sendBillResponse xmlns:ns="{SERVICE}"><applicationResponse>{cdr_zip64}</applicationResponse></ns:sendBillResponse>')
    if case_id == "soap-send-summary":
        return 200, soap(f'<ns:sendSummaryResponse xmlns:ns="{SERVICE}"><ticket>TICKET-FICTICIO</ticket></ns:sendSummaryResponse>')
    if case_id == "soap-get-status":
        return 200, soap(f'<ns:getStatusResponse xmlns:ns="{SERVICE}"><status><statusCode>98</statusCode></status></ns:getStatusResponse>')
    if case_id == "soap-get-status-cdr":
        return 200, soap(f'<ns:getStatusCdrResponse xmlns:ns="{SERVICE}"><statusCdr><statusCode>0</statusCode><statusMessage>ACEPTADA</statusMessage></statusCdr></ns:getStatusCdrResponse>')
    return 500, soap('<soap:Fault><faultcode>0156</faultcode><faultstring>ERROR TEST</faultstring></soap:Fault>')


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        case_id = self.path.lstrip("/")
        if case_id not in cases or cases[case_id]["kind"] != "soap":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(min(length, 16 * 1024 * 1024 + 1))
        observations = []
        if length > 16 * 1024 * 1024 or b"<!DOCTYPE" in raw.upper():
            observations.append("SOAP: tamaño o DTD no permitido")
        params = {}
        op_name = None
        try:
            tree = ET.fromstring(raw)
            body = tree.find(f"{{{SOAP}}}Body")
            op = next(iter(body)) if body is not None else None
            if op is None:
                observations.append("SOAP: Body vacío")
            else:
                op_name = op.tag.rsplit("}", 1)[-1]
                expected = cases[case_id]["operation"]
                if op.tag != f"{{{SERVICE}}}{expected}":
                    observations.append(f"SOAP: operación {op.tag} distinta de {expected}")
                action = self.headers.get("SOAPAction", "").strip('"')
                if action != f"urn:{expected}":
                    observations.append(f"SOAPAction: {action} distinto de urn:{expected}")
                xsd = wsdl / ("billConsultService.xsd" if expected == "getStatusCdr" else "billService.xsd2.xsd")
                with tempfile.NamedTemporaryFile(suffix=".xml") as tmp:
                    tmp.write(ET.tostring(op, encoding="utf-8"))
                    tmp.flush()
                    result = subprocess.run(["xmllint", "--nonet", "--noout", "--schema", str(xsd), tmp.name], capture_output=True, text=True)
                    if result.returncode:
                        observations.append("WSDL/XSD: cuerpo no valida")
                for child in op:
                    name = child.tag.rsplit("}", 1)[-1]
                    if name == "contentFile":
                        try:
                            content = base64.b64decode(child.text or "", validate=True)
                            with zipfile.ZipFile(io.BytesIO(content)) as z:
                                params[name] = [{"filename": n, "content": z.read(n).decode("utf-8")} for n in z.namelist()]
                        except (ValueError, zipfile.BadZipFile, UnicodeDecodeError):
                            observations.append("ZIP enviado no legible")
                    else:
                        params[name] = child.text
                header = tree.find(f"{{{SOAP}}}Header")
                text = ET.tostring(header, encoding="unicode") if header is not None else ""
                if "UsernameToken" not in text or "Password" not in text or "USUARIO-FICTICIO" not in text:
                    observations.append("WSSE UsernameToken incompleto")
                if "CLAVE-FICTICIA" not in text:
                    observations.append("WSSE PasswordText incompleto")
        except (ET.ParseError, TypeError, StopIteration) as exc:
            observations.append(f"SOAP: XML inválido ({type(exc).__name__})")
        record = {"id": case_id, "method": "POST", "path": self.path,
                  "content_type": self.headers.get("Content-Type"),
                  "soap_action": self.headers.get("SOAPAction"),
                  "operation": op_name, "params": params, "observations": observations}
        with log_file.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record, ensure_ascii=False) + "\n")
        status, payload = response(case_id)
        self.send_response(status)
        self.send_header("Content-Type", "text/xml; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


server = HTTPServer(("127.0.0.1", 0), Handler)
port_file.write_text(str(server.server_port))
server.serve_forever()
