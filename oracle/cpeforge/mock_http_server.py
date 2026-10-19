#!/usr/bin/env python3
"""Servidor loopback independiente para verificar SOAP/WSDL y GRE HTTP del crate cpeforge."""

from __future__ import annotations

import base64
import hashlib
import io
import json
import sys
import urllib.parse
import xml.etree.ElementTree as ET
import zipfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

SOAP = "http://schemas.xmlsoap.org/soap/envelope/"
SUNAT = "http://service.sunat.gob.pe"
WSSE = "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd"
WSDL = "http://schemas.xmlsoap.org/wsdl/"

port_file, log_file, wsdl_file = map(Path, sys.argv[1:4])
operations = {
    node.attrib["name"]
    for node in ET.parse(wsdl_file).iter(f"{{{WSDL}}}operation")
    if "name" in node.attrib
}
assert {"sendSummary", "getStatus"}.issubset(operations)


def check_zip(data: bytes, expected_name: str) -> list[str]:
    errors = []
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            if archive.namelist() != [expected_name]:
                errors.append("nombre/entradas ZIP")
            elif archive.read(expected_name) != b"<caller-xml/>":
                errors.append("XML ZIP")
    except (zipfile.BadZipFile, KeyError, RuntimeError):
        errors.append("ZIP ilegible")
    return errors


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def reply(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def record(self, kind: str, errors: list[str]) -> None:
        with log_file.open("a", encoding="utf-8") as log:
            log.write(json.dumps({"kind": kind, "observations": errors}, ensure_ascii=False) + "\n")

    def do_POST(self):
        size = int(self.headers.get("Content-Length", "0"))
        if size > 2 * 1024 * 1024:
            self.reply(413, b"", "text/plain")
            return
        body = self.rfile.read(size)
        if self.path == "/soap":
            self.soap_post(body)
        elif self.path == "/auth/clientessol/client.test/oauth2/token/":
            self.auth_post(body)
        elif self.path == "/cpe/contribuyente/gem/comprobantes/raw":
            self.gre_post(body)
        else:
            self.record("unexpected-post", ["ruta POST desconocida"])
            self.reply(404, b"", "text/plain")

    def soap_post(self, body: bytes) -> None:
        errors = []
        try:
            root = ET.fromstring(body)
            if root.tag != f"{{{SOAP}}}Envelope":
                errors.append("Envelope SOAP 1.1")
            header = root.find(f"{{{SOAP}}}Header")
            token = header.find(f".//{{{WSSE}}}UsernameToken") if header is not None else None
            user = token.find(f"{{{WSSE}}}Username") if token is not None else None
            password = token.find(f"{{{WSSE}}}Password") if token is not None else None
            if user is None or user.text != "20000000001TEST" or password is None or password.text != "password.test":
                errors.append("WSSE")
            soap_body = root.find(f"{{{SOAP}}}Body")
            operation = next(iter(soap_body), None) if soap_body is not None else None
            name = operation.tag.split("}")[-1] if operation is not None else ""
            if name not in operations or operation.tag != f"{{{SUNAT}}}{name}":
                errors.append("operación WSDL")
            if self.headers.get("SOAPAction") != f'"urn:{name}"':
                errors.append("SOAPAction")
            if name == "sendSummary":
                filename = operation.find("fileName")
                content = operation.find("contentFile")
                if filename is None or filename.text != "20000000001-RC-20240131-1.zip" or content is None or not content.text:
                    errors.append("parámetros sendSummary")
                else:
                    errors += check_zip(base64.b64decode(content.text, validate=True), "20000000001-RC-20240131-1.xml")
                response = f'<sendSummaryResponse xmlns="{SUNAT}"><ticket>T-123</ticket></sendSummaryResponse>'
            elif name == "getStatus":
                ticket = operation.find("ticket")
                if ticket is None or ticket.text != "T-123":
                    errors.append("ticket getStatus")
                response = f'<getStatusResponse xmlns="{SUNAT}"><status><statusCode>98</statusCode></status></getStatusResponse>'
            else:
                response = ""
        except (ET.ParseError, ValueError, zipfile.BadZipFile) as exc:
            errors.append(type(exc).__name__)
            name = "invalid"
            response = ""
        self.record(f"soap-{name}", errors)
        envelope = f'<soap:Envelope xmlns:soap="{SOAP}"><soap:Body>{response}</soap:Body></soap:Envelope>'.encode()
        self.reply(200 if not errors else 400, envelope, "text/xml; charset=utf-8")

    def auth_post(self, body: bytes) -> None:
        params = urllib.parse.parse_qs(body.decode("ascii"), strict_parsing=True)
        expected = {
            "grant_type": ["password"],
            "scope": ["https://api-cpe.sunat.gob.pe"],
            "client_id": ["client.test"],
            "client_secret": ["secret.test"],
            "username": ["20000000001TEST"],
            "password": ["password.test"],
        }
        errors = [] if params == expected else ["formulario OAuth"]
        self.record("gre-auth", errors)
        body = b'{"access_token":"fake-token","expires_in":3600}'
        self.reply(200 if not errors else 400, body, "application/json")

    def gre_post(self, body: bytes) -> None:
        errors = []
        if self.headers.get("Authorization") != "Bearer fake-token":
            errors.append("Bearer")
        try:
            archive = json.loads(body)["archivo"]
            raw = base64.b64decode(archive["arcGreZip"], validate=True)
            if archive["nomArchivo"] != "raw.zip":
                errors.append("nomArchivo")
            if archive["hashZip"] != hashlib.sha256(raw).hexdigest():
                errors.append("hashZip")
            errors += check_zip(raw, "raw.xml")
        except (KeyError, ValueError, TypeError, zipfile.BadZipFile) as exc:
            errors.append(type(exc).__name__)
        self.record("gre-send", errors)
        self.reply(200 if not errors else 400, b'{"numTicket":"T-123"}', "application/json")

    def do_GET(self):
        errors = []
        if self.path != "/cpe/contribuyente/gem/comprobantes/envios/T-123":
            errors.append("ruta estado GRE")
        if self.headers.get("Authorization") != "Bearer fake-token":
            errors.append("Bearer")
        self.record("gre-status", errors)
        self.reply(200 if not errors else 400, b'{"codRespuesta":"98"}', "application/json")


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
port_file.write_text(str(server.server_address[1]))
server.serve_forever()
