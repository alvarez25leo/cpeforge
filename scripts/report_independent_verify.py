#!/usr/bin/env python3
"""Verificación independiente de las salidas de cpeforge-report.

No usa el código Rust ni el PHP que generaron las salidas:

- HTML: se analiza con html.parser de la biblioteca estándar (equilibrio de
  etiquetas) y se comprueban, a partir de la **entrada** neutral del caso,
  serie-correlativo, RUC, razón social escapada, cabecera/pie confiables,
  logo (los bytes del data URI son los del parámetro) y el QR;
- SVG: ElementTree comprueba el documento (tamaño 120, viewBox) y la ruta
  `d` se rasteriza aquí con la regla par-impar, sin reutilizar la matriz;
- QR: la imagen rasterizada se decodifica con rqrr (scripts/report_qr_verify)
  y el texto debe ser el contenido esperado, calculado en Python desde la
  entrada del caso (RUC|tipo|serie|…|, URL).

Uso: report_independent_verify.py <tests/reference/report> <salidas Rust>
Las salidas Rust se generan con REPORT_RUST_OUT=<dir> cargo test -p
cpeforge-report --test parity. Sin ese directorio se usa el HTML PHP de
expected/ (solo para depurar el verificador).
"""

from __future__ import annotations

import base64
import json
import re
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from datetime import datetime
from decimal import ROUND_HALF_UP, Decimal
from html.parser import HTMLParser
from pathlib import Path
from zoneinfo import ZoneInfo

ROOT = Path(__file__).resolve().parent.parent
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}
SVG_NS = "{http://www.w3.org/2000/svg}"


class Balance(HTMLParser):
    """Cuenta etiquetas abiertas y cerradas fuera de los elementos vacíos."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.stack: list[str] = []
        self.errors: list[str] = []
        self.images: list[str] = []

    def handle_starttag(self, tag, attrs):
        if tag == "img":
            self.images.extend(v for k, v in attrs if k == "src" and v)
        if tag not in VOID:
            self.stack.append(tag)

    def handle_endtag(self, tag):
        if tag in VOID:
            return
        if tag in self.stack:
            while self.stack and self.stack[-1] != tag:
                self.errors.append(f"<{self.stack.pop()}> sin cerrar antes de </{tag}>")
            self.stack.pop()
        else:
            self.errors.append(f"</{tag}> sin abrir")


def php_escape(text: str) -> str:
    return (text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;").replace("'", "&#039;"))


def decimal(node) -> Decimal | None:
    if isinstance(node, dict) and "$decimal" in node:
        return Decimal(node["$decimal"])
    return None


def ymd(node) -> str:
    instant = datetime.fromisoformat(node["$datetime"])
    tz = node["tz"]
    zone = instant.tzinfo if tz[0] in "+-" else ZoneInfo(tz)
    return instant.astimezone(zone).strftime("%Y-%m-%d")


def money(node) -> str:
    value = decimal(node) or Decimal(0)
    return str(value.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))


def sale_payload(doc: dict) -> str:
    client = doc.get("client") or {}
    parts = [doc["company"].get("ruc"), doc.get("tipoDoc"), doc.get("serie"), doc.get("correlativo"),
             money(doc.get("mtoIGV")), money(doc.get("mtoImpVenta")), ymd(doc["fechaEmision"]),
             client.get("tipoDoc"), client.get("numDoc")]
    return "".join((p or "") + "|" for p in parts)


def despatch_payload(doc: dict) -> str:
    dest = doc.get("destinatario") or {}
    parts = [doc["company"].get("ruc"), doc.get("tipoDoc"), doc.get("serie"), doc.get("correlativo"),
             ymd(doc["fechaEmision"]), dest.get("tipoDoc"), dest.get("numDoc")]
    return "".join((p or "") + "|" for p in parts)


def rasterize(svg: str) -> tuple[list[list[bool]], int]:
    """Matriz de módulos a partir del SVG (regla par-impar en el centro de cada módulo)."""
    root = ET.fromstring(svg.encode("utf-8"))
    assert root.tag == f"{SVG_NS}svg", root.tag
    assert root.get("width") == "120" and root.get("height") == "120" and root.get("viewBox") == "0 0 120 120"
    scale_g = root.find(f"{SVG_NS}g")
    scale = float(re.fullmatch(r"scale\(([0-9.]+)\)", scale_g.get("transform")).group(1))
    path = root.find(f".//{SVG_NS}path")
    assert path.get("fill-rule") == "evenodd" and path.get("fill") == "#000000"
    size = round(120 / scale)
    edges: list[tuple[float, float, float, float]] = []
    for sub in re.findall(r"M[^MZ]*Z", path.get("d")):
        points = [tuple(map(float, p.split())) for p in re.findall(r"[ML](-?[0-9.]+ -?[0-9.]+)", sub)]
        for (x1, y1), (x2, y2) in zip(points, points[1:] + points[:1]):
            edges.append((x1, y1, x2, y2))
    grid = []
    for y in range(size):
        row = []
        cy = y + 0.5
        for x in range(size):
            cx = x + 0.5
            crossings = sum(1 for x1, y1, x2, y2 in edges
                            if (y1 > cy) != (y2 > cy) and cx < x1 + (cy - y1) * (x2 - x1) / (y2 - y1))
            row.append(crossings % 2 == 1)
        grid.append(row)
    return grid, size


def decode(svgs: dict[str, str], tool: Path) -> dict[str, str]:
    """Decodifica cada SVG con rqrr a través de imágenes PGM."""
    out: dict[str, str] = {}
    with tempfile.TemporaryDirectory() as tmp:
        files = {}
        for key, svg in svgs.items():
            grid, size = rasterize(svg)
            quiet, px = 4, 4
            side = (size + 2 * quiet) * px
            data = bytearray([255]) * (side * side)
            for y, row in enumerate(grid):
                for x, dark in enumerate(row):
                    if dark:
                        for dy in range(px):
                            start = ((y + quiet) * px + dy) * side + (x + quiet) * px
                            data[start:start + px] = b"\x00" * px
            path = Path(tmp) / f"{len(files)}.pgm"
            path.write_bytes(f"P5 {side} {side} 255\n".encode() + bytes(data))
            files[str(path)] = key
        if not files:
            return out
        result = subprocess.run([str(tool), *files], capture_output=True, text=True, check=True)
        for line in result.stdout.splitlines():
            item = json.loads(line)
            out[files[item["file"]]] = item.get("content", "ERROR: " + item.get("error", "?"))
    return out


def svg_of_data_uri(uri: str) -> str:
    return base64.b64decode(uri.split(",", 1)[1]).decode("utf-8")


def main() -> int:
    ref = Path(sys.argv[1])
    rust = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    tool_dir = ROOT / "scripts/report_qr_verify"
    target = ROOT / "target/report-qr-verify"
    subprocess.run(["cargo", "build", "--release", "--quiet", "--locked", "--manifest-path", str(tool_dir / "Cargo.toml"),
                    "--target-dir", str(target)], check=True)
    tool = target / "release/report-qr-verify"

    failures: list[str] = []
    checked = {"html": 0, "qr-html": 0, "qr": 0, "logo": 0}
    cases = json.loads((ref / "cases/html.json").read_text(encoding="utf-8"))["cases"]
    expected = json.loads((ref / "expected/html.json").read_text(encoding="utf-8"))["results"]
    pending_qr: dict[str, str] = {}
    want_qr: dict[str, str] = {}
    for case in cases:
        case_id = case["id"]
        html_dir = rust / "html" if rust else ref / "expected/html"
        path = html_dir / f"{case_id}.html"
        if "error" in expected[case_id] or not path.exists():
            continue
        html = path.read_text(encoding="utf-8")
        checked["html"] += 1
        parser = Balance()
        parser.feed(html)
        parser.close()
        doc, params = case["document"], case["params"]
        template = case["template"] or "invoice.html.twig"
        builtin = case["template_dir"] is None or template in ("summary.html.twig", "voided.html.twig")
        autoescape = case["options"].get("autoescape", True)
        if builtin and not autoescape:
            # Sin escape automático los textos se insertan tal cual (como PHP):
            # el HTML puede quedar desequilibrado y eso es lo esperado.
            client = doc.get("client") or {}
            if client.get("rznSocial") and client["rznSocial"] not in html:
                failures.append(f"{case_id}: con autoescape=false la razón social debe ir sin escapar")
        elif builtin:
            if parser.errors or parser.stack:
                failures.append(f"{case_id}: HTML desequilibrado {parser.errors[:3]} abiertas={parser.stack[:5]}")
            company = doc.get("company") or {}
            if company.get("ruc") and company["ruc"] not in html:
                failures.append(f"{case_id}: falta el RUC")
            if template in ("invoice.html.twig", "despatch.html.twig", "perception.html.twig", "retention.html.twig"):
                needle = f"{doc.get('serie') or ''}-{doc.get('correlativo') or ''}"
                if needle not in html:
                    failures.append(f"{case_id}: falta {needle}")
            header = (params.get("user") or {}).get("header")
            if header and header not in html:
                failures.append(f"{case_id}: cabecera confiable alterada")
            client = doc.get("client") or doc.get("destinatario") or doc.get("proveedor") or {}
            if client.get("rznSocial") and case["options"].get("autoescape", True) and php_escape(client["rznSocial"]) not in html:
                failures.append(f"{case_id}: razón social sin escapar como htmlspecialchars")
            logo = (params.get("system") or {}).get("logo")
            if isinstance(logo, dict) and "$bytes" in logo:
                raw = base64.b64decode(logo["$bytes"])
                if not any(not src.startswith("data:image/svg+xml") and base64.b64decode(src.split(",", 1)[1]) == raw
                           for src in parser.images):
                    failures.append(f"{case_id}: el logo incrustado no coincide con el parámetro")
                checked["logo"] += 1
        qr_images = [src for src in parser.images if src.startswith("data:image/svg+xml;base64,")]
        if template.startswith("invoice") and builtin:
            want_qr[case_id] = sale_payload(doc)
        elif template == "despatch.html.twig" and (params.get("system") or {}).get("qr"):
            want_qr[case_id] = params["system"]["qr"]
        if case_id in want_qr:
            if len(qr_images) != 1:
                failures.append(f"{case_id}: se esperaba un QR y hay {len(qr_images)}")
                del want_qr[case_id]
            else:
                pending_qr[case_id] = svg_of_data_uri(qr_images[0])
        if template in ("qr_sale.html.twig", "qr_despatch.html.twig", "qr_url.html.twig"):
            want_qr[case_id] = {"qr_sale.html.twig": sale_payload, "qr_despatch.html.twig": despatch_payload}.get(
                template, lambda d: str(params["system"]["qr"]))(doc)
            pending_qr[case_id] = html

    decoded = decode(pending_qr, tool)
    for case_id, payload in want_qr.items():
        checked["qr-html"] += 1
        if decoded.get(case_id) != payload:
            failures.append(f"{case_id}: QR decodificado {decoded.get(case_id)!r} ≠ {payload!r}")

    qr_cases = json.loads((ref / "cases/qr.json").read_text(encoding="utf-8"))["cases"]
    svgs, contents = {}, {}
    for case in qr_cases:
        svg_dir = rust / "qr" if rust else ref / "expected/qr"
        path = svg_dir / f"{case['id']}.svg"
        if path.exists():
            svgs[case["id"]] = path.read_text(encoding="utf-8")
            contents[case["id"]] = base64.b64decode(case["content"]["$bytes"]).decode("utf-8")
    for case_id, text in decode(svgs, tool).items():
        checked["qr"] += 1
        if text != contents[case_id]:
            failures.append(f"qr {case_id}: decodificado {text[:40]!r}… ≠ contenido")

    print(f"verificación independiente: {checked}")
    for failure in failures:
        print("FALLO", failure)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
