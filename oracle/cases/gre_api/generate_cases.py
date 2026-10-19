#!/usr/bin/env python3
"""Casos sintéticos de paridad de greenter/gre-api v1.0.2.

Cada caso describe una llamada al cliente (operación, variante PHP,
configuración y argumentos) y la respuesta que dará el servidor local
(oracle/gre_api/mock_server.py). Todas las credenciales y tokens son
ficticios. Ningún caso usa hosts reales salvo las variantes «request», que
solo construyen la solicitud y no la envían.

Uso: python3 oracle/cases/gre_api/generate_cases.py [salida.json]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

SECRET = "secreto-ficticio"
PASSWORD = "clave-ficticia"
TOKEN = "token-ficticio-0123456789"
CLIENT = "cliente-ficticio"
USER = "20000000001USUARIOX"
JSON_H = [["Content-Type", "application/json"]]

TOKEN_ARGS = {
    "grant_type": "password",
    "scope": "https://api-cpe.sunat.gob.pe",
    "client_id": CLIENT,
    "client_secret": SECRET,
    "username": USER,
    "password": PASSWORD,
}
TOKEN_OK = {"access_token": TOKEN, "token_type": "Bearer", "expires_in": 3600}
ZIP_B64 = "UEsDBBQACAgIAAAAIQAAAAAAAAAAAAAAAAAMAAAA+/ab/cd+ef=="
DOC = {"archivo": {"nom_archivo": "20000000001-09-T001-1.zip", "arc_gre_zip": ZIP_B64, "hash_zip": "a" * 64}}
TICKET = "1a2b3c4d-0000-4000-8000-0123456789ab"
CPE_OK = {"numTicket": TICKET, "fecRecepcion": "2024-01-02T10:20:30.123-05:00"}


def resp(status=200, body=None, headers=None, reason=None, raw=None, **extra):
    r = {"status": status, "headers": headers if headers is not None else JSON_H}
    if raw is not None:
        r["body"] = raw
    elif body is not None:
        r["body"] = json.dumps(body, ensure_ascii=False, separators=(",", ":"))
    else:
        r["body"] = ""
    if reason is not None:
        r["reason"] = reason
    r.update(extra)
    return r


def case(cid, operation, variant="sync", response=None, config=None, **kw):
    c = {"id": cid, "operation": operation, "variant": variant, "config": config or {"host": "mock"}}
    if response is not None:
        c["response"] = response
    c.update(kw)
    return c


def token(cid, response=None, variant="sync", args=None, **kw):
    return case(cid, "getToken", variant, response, args=args or TOKEN_ARGS, **kw)


def cpe_config(**extra):
    return {"host": "mock", "access_token": TOKEN, **extra}


def envio(cid, response=None, variant="sync", filename="20000000001-09-T001-1", document=DOC, config=None, **kw):
    return case(cid, "enviarCpe", variant, response, config=config or cpe_config(),
                args={"filename": filename, "document": document}, **kw)


def consulta(cid, response=None, variant="sync", ticket=TICKET, config=None, **kw):
    return case(cid, "consultarEnvio", variant, response, config=config or cpe_config(), args={"ticket": ticket}, **kw)


def cases() -> list[dict]:
    out = []
    # ------------------------------------------------------------------ token
    for v in ("sync", "http_info", "async", "async_http_info"):
        out.append(token(f"token-ok-{v}", resp(body=TOKEN_OK, headers=JSON_H + [["X-Traza", "a"], ["X-Traza", "b"]]), v))
    out.append(token("token-reserved-chars", resp(body=TOKEN_OK), "http_info", args={
        "grant_type": "password", "scope": "https://api-cpe.sunat.gob.pe", "client_id": "id/ä ?&=#%",
        "client_secret": "s&e=c+r%t ñ", "username": "20000000001 USU", "password": "p@ss w/ord ~*'()!"}))
    out.append(token("token-content-type-json", resp(body=TOKEN_OK), "http_info", content_type="application/json"))
    out.append(token("token-content-type-empty", resp(body=TOKEN_OK), "http_info", content_type=""))
    out.append(token("token-content-type-text", resp(body=TOKEN_OK), "http_info", content_type="text/plain"))
    out.append(token("token-empty-strings", resp(body=TOKEN_OK), "http_info", args={k: "" for k in TOKEN_ARGS}))
    for field in ("client_id", "grant_type", "password"):
        out.append(token(f"token-null-{field}", None, "sync", args={**TOKEN_ARGS, field: None}, rust="no_aplica"))
    values = {
        "expires-string": {"access_token": TOKEN, "expires_in": "3600"},
        "expires-float": {"access_token": TOKEN, "expires_in": 3600.9},
        "expires-text": {"access_token": TOKEN, "expires_in": "abc"},
        "expires-prefix": {"access_token": TOKEN, "expires_in": "120s"},
        "expires-bool": {"access_token": TOKEN, "expires_in": True},
        "expires-null": {"access_token": TOKEN, "expires_in": None},
        "expires-array": {"access_token": TOKEN, "expires_in": [1]},
        "token-number": {"access_token": 123, "token_type": 1.5},
        "token-bool": {"access_token": True, "token_type": False},
        "token-array": {"access_token": ["a"]},
        "token-object": {"access_token": {"a": 1}},
        "token-unicode": {"access_token": "tökén-ñ-😀", "token_type": "Bearer"},
        "extra-fields": {**TOKEN_OK, "scope": "x", "otro": {"a": 1}},
        "big-int": {"access_token": TOKEN, "expires_in": 9223372036854775807},
    }
    for name, body in values.items():
        out.append(token(f"token-values-{name}", resp(body=body), "http_info"))
    bodies = {
        "malformed": "{bad", "empty": "", "json-null": "null", "json-array": "[]", "json-list": "[1,2]",
        "json-number": "123", "json-string": json.dumps(json.dumps(TOKEN_OK)), "json-bom": "﻿" + json.dumps(TOKEN_OK),
        "whitespace": "  \n" + json.dumps(TOKEN_OK) + "\n ",
    }
    for name, raw in bodies.items():
        for v in ("sync", "http_info"):
            out.append(token(f"token-body-{name}-{v}", resp(raw=raw), v))
    errors = {
        "401": resp(401, {"error": "invalid_client", "error_description": "Cliente no autorizado"}),
        "403-empty": resp(403, raw=""),
        "400-long": resp(400, raw="x" * 130),
        "400-120": resp(400, raw="y" * 120),
        "500-control": resp(500, raw="bad\u0001ctl"),
        "500-cut-utf8": resp(500, raw="a" + "ñ" * 61),
        "404-html": resp(404, raw="<html><body>No encontrado</body></html>", headers=[["Content-Type", "text/html"]]),
        "401-reason": resp(401, {"error": "x"}, reason="Credenciales Invalidas"),
        "418": resp(418, raw="tea"),
        "599-no-reason": resp(599, raw="z", reason=""),
    }
    for name, r in errors.items():
        for v in ("sync", "http_info"):
            out.append(token(f"token-error-{name}-{v}", r, v))
    out.append(token("token-async-401", errors["401"], "async"))
    out.append(token("token-async-http-info-500", resp(500, raw="error"), "async_http_info"))
    out.append(token("token-201", resp(201, TOKEN_OK), "http_info"))
    out.append(token("token-204-sync", resp(204, raw="", headers=[]), "sync"))
    out.append(token("token-204-http-info", resp(204, raw="", headers=[]), "http_info"))
    out.append(token("token-300", resp(300, raw="multiple"), "http_info"))
    out.append(token("token-redirect-same-host", resp(302, raw="", headers=[["Location", "@same/final"]],
                                                       final=resp(body=TOKEN_OK)), "http_info"))
    out.append(token("token-timeout", resp(body=TOKEN_OK, delay=3), "sync", client={"timeout": 1}))
    out.append(token("token-connection-refused", None, "sync", config={"host": "http://127.0.0.1:1"}))
    out.append(token("token-async-connection-refused", None, "async", config={"host": "http://127.0.0.1:1"}))
    out.append(token("token-connection-closed", {"action": "close"}, "sync"))
    out.append(token("token-user-agent-custom", resp(body=TOKEN_OK), "sync", config={"host": "mock", "user_agent": "MiApp/2.0"}))
    out.append(token("token-user-agent-empty", resp(body=TOKEN_OK), "sync", config={"host": "mock", "user_agent": ""}))
    out.append(token("token-request-default", None, "request", config={}))
    # ------------------------------------------------------------------ envío
    for v in ("sync", "http_info", "async", "async_http_info"):
        out.append(envio(f"envio-ok-{v}", resp(body=CPE_OK), v))
    out.append(envio("envio-filename-reserved", resp(body=CPE_OK), "http_info", filename="20000000001-09-T001-1 ñ/x?#&"))
    out.append(envio("envio-filename-dotdot", resp(body=CPE_OK), "http_info", filename=".."))
    out.append(envio("envio-filename-empty", resp(body=CPE_OK), "http_info", filename=""))
    out.append(envio("envio-no-document", resp(body=CPE_OK), "http_info", document=None))
    out.append(envio("envio-document-empty", resp(body=CPE_OK), "http_info", document={"archivo": None}))
    out.append(envio("envio-archivo-empty", resp(body=CPE_OK), "http_info", document={"archivo": {}}))
    out.append(envio("envio-json-escaping", resp(body=CPE_OK), "http_info", document={"archivo": {
        "nom_archivo": "ñandú-\"q\"\\/<>&'.zip", "arc_gre_zip": "ab/+cd==\u0001", "hash_zip": "😀"}}))
    out.append(envio("envio-content-type-text", resp(body=CPE_OK), "http_info", content_type="text/plain"))
    out.append(envio("envio-content-type-json-charset", resp(body=CPE_OK), "http_info", content_type="Application/JSON; charset=utf-8"))
    out.append(envio("envio-no-token", resp(401, raw=""), "http_info", config={"host": "mock"}))
    out.append(envio("envio-token-zero", resp(body=CPE_OK), "http_info", config={"host": "mock", "access_token": "0"}))
    out.append(envio("envio-large", resp(body=CPE_OK), "http_info", document={"archivo": {
        "nom_archivo": "grande.zip", "arc_gre_zip": {"repeat": {"unit": "QUJD", "count": 400000}}, "hash_zip": "b" * 64}}))
    e422 = {"cod": "422", "msg": "Unprocessable Entity", "errors": [{"cod": "2001", "msg": "El nombre no coincide"}, {"cod": 2002, "msg": "Hash"}]}
    errs = {
        "422": resp(422, e422),
        "422-null-item": resp(422, {"cod": "422", "errors": [None, {"cod": "1"}]}),
        "422-errors-object": resp(422, {"cod": "422", "errors": {"cod": "1"}}),
        "422-errors-string": resp(422, {"cod": "422", "errors": json.dumps([{"cod": "9"}])}),
        "422-malformed": resp(422, raw="{no json"),
        "422-empty": resp(422, raw=""),
        "422-msg-object": resp(422, {"cod": "422", "msg": {"x": 1}, "errors": {"y": 1}}),
        "500": resp(500, {"cod": "500", "msg": "Error interno"}),
        "500-numeric": resp(500, {"cod": 500, "msg": None}),
        "401": resp(401, {"cod": "401", "msg": "No autorizado"}),
        "503-text": resp(503, raw="Servicio no disponible", headers=[["Content-Type", "text/plain"], ["Retry-After", "30"]]),
    }
    for name, r in errs.items():
        for v in ("sync", "http_info"):
            out.append(envio(f"envio-error-{name}-{v}", r, v))
    out.append(envio("envio-async-422", errs["422"], "async"))
    fechas = {
        "z": "2024-01-02T10:20:30Z", "micro7": "2024-01-02T10:20:30.1234567-05:00", "compact-offset": "2024-01-02T10:20:30+0500",
        "no-offset": "2024-01-02T10:20:30", "date-only": "2024-01-02", "space": "2024-01-02 10:20:30-05:00",
        "empty": "", "zero": "0", "invalid": "abc", "relative": "first day of January 2024", "overflow": "2024-02-30T00:00:00Z",
        "number": 1704208830, "array": ["2024"],
    }
    for name, f in fechas.items():
        out.append(envio(f"envio-fecha-{name}", resp(body={"numTicket": TICKET, "fecRecepcion": f}), "http_info"))
    out.append(envio("envio-ticket-number", resp(body={"numTicket": 123}), "http_info"))
    out.append(envio("envio-malformed-sync", resp(raw="<html>"), "sync"))
    out.append(envio("envio-request-default", None, "request", config={"access_token": TOKEN}))
    # ---------------------------------------------------------------- consulta
    for v in ("sync", "http_info", "async", "async_http_info"):
        out.append(consulta(f"consulta-98-{v}", resp(body={"codRespuesta": "98"}), v))
    out.append(consulta("consulta-99", resp(body={"codRespuesta": "99", "error": {"numError": "1033", "desError": "El comprobante fue registrado previamente"}, "indCdrGenerado": "0"}), "http_info"))
    out.append(consulta("consulta-0", resp(body={"codRespuesta": "0", "arcCdr": "UEsDBBQAAAAIAA==", "indCdrGenerado": "1"}), "http_info"))
    numeric = {
        "cod-int-0": {"codRespuesta": 0}, "cod-int-98": {"codRespuesta": 98}, "cod-float": {"codRespuesta": 0.0},
        "cod-bool": {"codRespuesta": True}, "ind-int": {"codRespuesta": "0", "indCdrGenerado": 1},
        "num-error-int": {"codRespuesta": "99", "error": {"numError": 1033}},
        "error-json-string": {"codRespuesta": "99", "error": json.dumps({"numError": "7", "desError": "x"})},
        "error-list": {"codRespuesta": "99", "error": []}, "error-text": {"codRespuesta": "99", "error": "x"},
        "error-null": {"codRespuesta": "99", "error": None}, "empty-object": {},
    }
    for name, body in numeric.items():
        out.append(consulta(f"consulta-{name}", resp(body=body), "http_info"))
    out.append(consulta("consulta-ticket-reserved", resp(body={"codRespuesta": "98"}), "http_info", ticket="a b/c?d#e"))
    out.append(consulta("consulta-empty-body-sync", resp(raw=""), "sync"))
    out.append(consulta("consulta-content-type-text", resp(body={"codRespuesta": "98"}), "http_info", content_type="text/plain"))
    for name in ("422", "500", "422-errors-object"):
        out.append(consulta(f"consulta-error-{name}", errs[name], "http_info"))
    out.append(consulta("consulta-redirect-cross-host", resp(302, raw="", headers=[["Location", "@other/final"]],
                                                             final=resp(body={"codRespuesta": "0"})), "http_info"))
    out.append(consulta("consulta-request-default-config", None, "request", config={"access_token": TOKEN}))
    # ----------------------------------------------------------- configuración
    out.append(case("config-defaults", "config", "config", config={}))
    return out


def main() -> None:
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "tests/reference/gre-api/cases.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    data = cases()
    ids = [c["id"] for c in data]
    if len(ids) != len(set(ids)):
        raise SystemExit("ids duplicados")
    target.write_text(json.dumps({"cases": data}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
