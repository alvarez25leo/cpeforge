#!/usr/bin/env python3
"""SOAP HTTPS en loopback con certificado efímero no confiable."""

import ssl
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

cert, key, port_file, log_file = map(Path, sys.argv[1:5])
log_file.write_text("")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        if length > 1024 * 1024:
            self.send_error(413)
            return
        body = self.rfile.read(length)
        if b"getStatus" not in body:
            self.send_error(400)
            return
        with log_file.open("a", encoding="utf-8") as output:
            output.write("POST getStatus\n")
        payload = b'''<?xml version="1.0" encoding="UTF-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><ns:getStatusResponse xmlns:ns="http://service.sunat.gob.pe"><status><statusCode>98</statusCode></status></ns:getStatusResponse></soap:Body></soap:Envelope>'''
        self.send_response(200)
        self.send_header("Content-Type", "text/xml; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


server = HTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(str(cert), str(key))
server.socket = context.wrap_socket(server.socket, server_side=True)
port_file.write_text(str(server.server_port))
server.serve_forever()
