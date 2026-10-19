#!/usr/bin/env python3
"""Servidor HTTP local para el oráculo de greenter/gre-api.

Escucha SOLO en 127.0.0.1 (y responde a `localhost`, para redirecciones a
"otro host"). Para cada caso de tests/reference/gre-api/cases.json:

- recibe solicitudes en /c/<id>/<ruta de la API>;
- registra método, ruta, cabeceras y cuerpo;
- valida la solicitud contra el contrato OpenAPI fijado (ruta, método,
  Content-Type, campos del formulario, esquema JSON, seguridad bearer) y la
  respuesta guionada contra el esquema declarado para su estado;
- responde lo guionado (estado, frase, cabeceras, cuerpo), con demora, cierre
  de conexión o redirección (`@same/…` → 127.0.0.1, `@other/…` → localhost).

Control: GET /__take?case=<id> devuelve y vacía lo registrado del caso.
No usa código de PHP ni de Rust: es la verificación independiente del
contrato HTTP.

Uso: mock_server.py <cases.json> <openapi.json> <archivo_puerto>
"""

from __future__ import annotations

import base64
import json
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qsl, unquote

CASES: dict[str, dict] = {}
OPENAPI: dict = {}
RECORDS: dict[str, list] = {}
LOCK = threading.Lock()
PORT = 0

OPERATIONS = [
    ("getToken", "POST", re.compile(r"^/clientessol/(?P<client_id>[^/]*)/oauth2/token/$"), "/clientessol/{client_id}/oauth2/token/"),
    ("consultarEnvio", "GET", re.compile(r"^/contribuyente/gem/comprobantes/envios/(?P<numTicket>[^/]*)$"), "/contribuyente/gem/comprobantes/envios/{numTicket}"),
    ("enviarCpe", "POST", re.compile(r"^/contribuyente/gem/comprobantes/(?P<filename>[^/]*)$"), "/contribuyente/gem/comprobantes/{filename}"),
]


def resolve(schema: dict) -> dict:
    while "$ref" in schema:
        name = schema["$ref"].rsplit("/", 1)[1]
        schema = OPENAPI["components"]["schemas"][name]
    return schema


RFC3339 = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$")


def validate(value, schema: dict, path: str, out: list) -> None:
    """Subconjunto de JSON Schema que usa el OpenAPI fijado."""
    schema = resolve(schema)
    kind = schema.get("type")
    if kind == "object":
        if not isinstance(value, dict):
            out.append(f"{path}: se esperaba objeto")
            return
        for req in schema.get("required", []):
            if req not in value:
                out.append(f"{path}.{req}: requerido")
        for name, sub in schema.get("properties", {}).items():
            if name in value and value[name] is not None:
                validate(value[name], sub, f"{path}.{name}", out)
            elif name in value:
                out.append(f"{path}.{name}: null no declarado")
    elif kind == "array":
        if not isinstance(value, list):
            out.append(f"{path}: se esperaba arreglo")
            return
        for i, item in enumerate(value):
            validate(item, schema.get("items", {}), f"{path}[{i}]", out)
    elif kind == "string":
        if not isinstance(value, str):
            out.append(f"{path}: se esperaba string")
        elif schema.get("format") == "date-time" and not RFC3339.match(value):
            out.append(f"{path}: no es date-time RFC 3339")
    elif kind == "integer":
        if not isinstance(value, int) or isinstance(value, bool):
            out.append(f"{path}: se esperaba integer")
        elif schema.get("format") == "int32" and not -(2**31) <= value < 2**31:
            out.append(f"{path}: fuera de int32")


def contract_request(method: str, path: str, headers: dict, body: bytes) -> list:
    issues = []
    for op_id, op_method, regex, template in OPERATIONS:
        m = regex.match(path)
        if not m:
            continue
        spec = OPENAPI["paths"][template][op_method.lower()]
        if method != op_method:
            issues.append(f"método {method} no declarado para {template}")
        for name, raw in m.groupdict().items():
            if raw == "":
                issues.append(f"parámetro de ruta {name} vacío")
        if "security" in spec:
            auth = headers.get("authorization", "")
            if not re.match(r"^Bearer \S+$", auth):
                issues.append("falta Authorization: Bearer")
        accept = headers.get("accept")
        if accept != "application/json":
            issues.append(f"Accept {accept!r}")
        content = spec.get("requestBody", {}).get("content", {})
        if content:
            ctype = headers.get("content-type", "")
            declared = list(content)[0]
            if ctype.split(";")[0].strip().lower() != declared:
                issues.append(f"Content-Type {ctype!r} (se declara {declared})")
            schema = content[declared]["schema"]
            if declared == "application/x-www-form-urlencoded":
                try:
                    fields = dict(parse_qsl(body.decode("utf-8"), keep_blank_values=True, strict_parsing=bool(body)))
                except Exception as e:  # noqa: BLE001
                    issues.append(f"formulario inválido: {e}")
                    fields = {}
                validate(fields, schema, "form", issues)
            elif body:
                try:
                    validate(json.loads(body), schema, "body", issues)
                except ValueError:
                    issues.append("cuerpo JSON inválido")
            else:
                issues.append("cuerpo vacío")
        return [f"{op_id}: {i}" for i in issues]
    return [f"ruta no declarada: {method} {path}"]


def contract_response(path: str, method: str, status: int, body: str) -> list:
    for op_id, op_method, regex, template in OPERATIONS:
        if regex.match(path) and method == op_method:
            responses = OPENAPI["paths"][template][op_method.lower()]["responses"]
            spec = responses.get(str(status))
            if spec is None:
                return [f"{op_id}: estado {status} no declarado"]
            issues: list = []
            schema = spec.get("content", {}).get("application/json", {}).get("schema")
            if schema is not None:
                try:
                    validate(json.loads(body), schema, "respuesta", issues)
                except ValueError:
                    issues.append("respuesta no es JSON")
            return [f"{op_id}: {i}" for i in issues]
    return []


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):  # sin registros en consola
        pass

    def _body(self) -> bytes:
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length) if length else b""

    def _send(self, status: int, reason, headers: list, body: bytes) -> None:
        if reason is None:
            self.send_response_only(status)
        else:
            self.wfile.write(f"HTTP/1.1 {status} {reason}\r\n".encode("latin-1"))
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)
        self.close_connection = True

    def handle_any(self) -> None:
        body = self._body()
        if self.path.startswith("/__take"):
            case_id = self.path.split("case=", 1)[1]
            with LOCK:
                data = RECORDS.pop(unquote(case_id), [])
            self._send(200, None, [["Content-Type", "application/json"]], json.dumps(data).encode())
            return
        m = re.match(r"^/c/([^/]+)(/.*)$", self.path)
        if not m or m.group(1) not in CASES:
            self._send(404, None, [], b"caso desconocido")
            return
        case = CASES[m.group(1)]
        api_path = m.group(2)
        headers_lower = {k.lower(): v for k, v in self.headers.items()}
        script = case.get("response") or {}
        final = api_path.endswith("/final") and "final" in script
        if final:
            script = script["final"]
        record = {
            "method": self.command,
            "path": api_path,
            "headers": [[k, v] for k, v in self.headers.items()],
            "body": body.decode("utf-8") if _utf8(body) else {"b64": base64.b64encode(body).decode()},
            "contract": [] if final else contract_request(self.command, api_path.split("?")[0], headers_lower, body),
        }
        if script.get("action") == "close":
            _store(case["id"], record)
            self.close_connection = True
            return
        if script.get("delay"):
            time.sleep(script["delay"])
        status = script.get("status", 200)
        raw = script.get("body", "")
        resp_body = raw.encode("utf-8") if isinstance(raw, str) else base64.b64decode(raw["b64"])
        out_headers = []
        for k, v in script.get("headers", []):
            if k == "Location" and v.startswith("@"):
                host = "127.0.0.1" if v.startswith("@same/") else "localhost"
                v = f"http://{host}:{PORT}/c/{case['id']}/{v.split('/', 1)[1]}"
            out_headers.append((k, v))
        if not final:
            record["contract"] += contract_response(api_path.split("?")[0], self.command, status, raw if isinstance(raw, str) else "")
        _store(case["id"], record)
        self._send(status, script.get("reason"), out_headers, resp_body)

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = handle_any


def _utf8(b: bytes) -> bool:
    try:
        b.decode("utf-8")
        return True
    except UnicodeDecodeError:
        return False


def _store(case_id: str, record: dict) -> None:
    with LOCK:
        RECORDS.setdefault(case_id, []).append(record)


def main() -> None:
    global OPENAPI, PORT
    cases = json.load(open(sys.argv[1], encoding="utf-8"))["cases"]
    CASES.update({c["id"]: c for c in cases})
    OPENAPI = json.load(open(sys.argv[2], encoding="utf-8"))
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    PORT = server.server_address[1]
    with open(sys.argv[3], "w") as f:
        f.write(str(PORT))
    server.serve_forever()


if __name__ == "__main__":
    main()
