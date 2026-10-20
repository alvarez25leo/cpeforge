#!/usr/bin/env python3
"""Adaptaciones de las plantillas Twig de greenter/report v5.3.0 a MiniJinja.

Las plantillas de crates/report/templates son las de la referencia con estas
sustituciones exactas y nada más. Cada una conserva la semántica de PHP donde
MiniJinja difiere (ver crates/report/README.md):

- `in` y `!=` de Twig usan la comparación laxa de PHP 8 ("1" == "01");
- una cadena "0" es falsa en PHP;
- `null ~ 'x'` concatena una cadena vacía.

Las plantillas de `CPEFORGE_TEMPLATES` (liquidación de compra) no existen
en la referencia y quedan fuera de la comparación.

Además, `despatch.html.twig` agrega los datos de la guía de remisión
transportista (tipo 31, extensión de CPEForge) en bloques que solo se
evalúan con ese tipo: la salida de la guía remitente (09) no cambia.

Uso:
  report_template_adaptations.py check <plantillas PHP> <plantillas Rust>
  report_template_adaptations.py apply <plantillas PHP> <plantillas Rust>
"""

from __future__ import annotations

import sys
from pathlib import Path

HASH = ("{% if params.system.hash is defined and params.system.hash%}",
        "{% if params.system.hash is defined and params.system.hash is twig_true %}")
HASH_SP = ("{% if params.system.hash is defined and params.system.hash %}",
           "{% if params.system.hash is defined and params.system.hash is twig_true %}")
OBS = ("{% if doc.observacion %}", "{% if doc.observacion is twig_true %}")

# GRE transportista (31): remitente, registro MTC, pagador del flete, TUC y
# subcontratista. Las etiquetas nuevas van en la columna 0 para que, con
# `trim_blocks`, la guía remitente (09) produzca el mismo HTML que PHP.
_REM_ANCHOR = """            <br>
            <div class="tabla_borde">
                {% set cl = doc.destinatario %}
                <table width="100%" border="0" cellpadding="5" cellspacing="0">
                    <tbody>
                    <tr>
                        <td colspan="2">DESTINATARIO</td>"""
_REM_BLOCK = """{% if doc.tipoDoc == '31' and doc.remitente %}
            <div class="tabla_borde">
                {% set rem = doc.remitente %}
                <table width="100%" border="0" cellpadding="5" cellspacing="0">
                    <tbody>
                    <tr>
                        <td colspan="2">REMITENTE</td>
                    </tr>
                    <tr class="border_top">
                        <td width="60%" align="left"><strong>Razón Social:</strong>  {{ rem.rznSocial }}</td>
                        <td width="40%" align="left"><strong>{{ rem.tipoDoc|catalog('06') }}:</strong>  {{ rem.numDoc }}</td>
                    </tr>
                    </tbody></table>
            </div><br>
{% endif %}
"""
_MOTIVO = """                    <tr>
                        <td width="60%" align="left"><strong>Motivo Traslado:</strong>  {{ doc.envio.codTraslado|catalog('20') }} </td>
                        <td width="40%" align="left"><strong>Modalidad de Transporte:</strong>  {{ doc.envio.modTraslado|catalog('18') }} </td>
                    </tr>
"""
_MTC = """{% if doc.tipoDoc == '31' %}
                    <tr>
                        <td width="60%" align="left"><strong>Registro MTC:</strong>  {% if doc.envio.transportista %}{{ doc.envio.transportista.nroMtc }}{% endif %} </td>
                        <td width="40%" align="left">{% if doc.pagadorFlete %}<strong>Pagador del flete:</strong>  {{ doc.pagadorFlete.numDoc }} - {{ doc.pagadorFlete.rznSocial }}{% endif %} </td>
                    </tr>
{% else %}
"""
_PLACA = """                            <strong>Vehículo principal:</strong>  {{ vehiculo.placa }}
"""
_TUC = """{% if doc.tipoDoc == '31' and vehiculo.nroCirculacion %}
                            <br>
                            <strong>TUC o certificado:</strong>  {{ vehiculo.nroCirculacion }}
{% endif %}
"""
_TRANSPORTE_END = """                        </td>
                    </tr>
                    {% endif %}
                    </tbody></table>
            </div><br>"""
_SUB = """{% if doc.tipoDoc == '31' and doc.envio.subcontratador %}
                    <tr>
                        <td colspan="2" align="left"><strong>Subcontratista:</strong>  {{ doc.envio.subcontratador.numDoc }} - {{ doc.envio.subcontratador.rznSocial }}</td>
                    </tr>
{% endif %}
"""
CARRIER = [
    (_REM_ANCHOR, _REM_ANCHOR.replace("            <br>\n", "            <br>\n" + _REM_BLOCK, 1), 1),
    (_MOTIVO, _MTC + _MOTIVO + "{% endif %}\n", 1),
    ("                    {% if transportista %}",
     "                    {% if transportista and doc.tipoDoc != '31' %}", 1),
    (_PLACA, _PLACA + _TUC, 1),
    (_TRANSPORTE_END, _TRANSPORTE_END.replace(
        "                    {% endif %}\n", "                    {% endif %}\n" + _SUB, 1), 1),
]

# Plantillas propias de CPEForge, sin equivalente en la referencia.
CPEFORGE_TEMPLATES = {"settlement.html.twig"}

ADAPTATIONS: dict[str, list[tuple[str, str, int]]] = {
    "invoice.html.twig": [
        ("{% set isInvoice = doc.tipoDoc in ['01', '03'] %}",
         "{% set isInvoice = doc.tipoDoc is twig_in(['01', '03']) %}", 1),
        ("{% set isNota = doc.tipoDoc in ['07', '08'] %}",
         "{% set isNota = doc.tipoDoc is twig_in(['07', '08']) %}", 1),
        ("{% if doc.compra is defined and doc.compra %}",
         "{% if doc.compra is defined and doc.compra is twig_true %}", 1),
        ("{% if leg.code != '1000' %}", "{% if leg.code is not twig_eq('1000') %}", 1),
        (*HASH, 1),
    ],
    "invoice2.html.twig": [
        ("{% set serieNro = doc.serie ~ '-' ~ doc.correlativo %}",
         "{% set serieNro = doc.serie|twig_string ~ '-' ~ doc.correlativo|twig_string %}", 1),
        (*HASH, 1),
    ],
    "despatch.html.twig": [(*OBS, 1), (*HASH, 1), *CARRIER],
    "perception.html.twig": [(*OBS, 1), (*HASH, 1)],
    "retention.html.twig": [(*OBS, 1), (*HASH, 1)],
    "summary.html.twig": [(*HASH, 1)],
    "voided.html.twig": [
        ("{% if 'RA' in doc.name %}", "{% if 'RA' is twig_in(doc.name) %}", 1),
        (*HASH_SP, 1),
    ],
    "assets/style.css": [],
}


def adapt(name: str, text: str) -> str:
    for old, new, count in ADAPTATIONS[name]:
        found = text.count(old)
        if found != count:
            raise SystemExit(f"{name}: se esperaban {count} apariciones de {old!r}, hay {found}")
        text = text.replace(old, new)
    return text


def main() -> int:
    if len(sys.argv) != 4 or sys.argv[1] not in ("check", "apply"):
        print(__doc__, file=sys.stderr)
        return 2
    mode, php_dir, rust_dir = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    php_files = sorted(str(p.relative_to(php_dir)) for p in php_dir.rglob("*") if p.is_file())
    rust_files = sorted(str(p.relative_to(rust_dir)) for p in rust_dir.rglob("*")
                        if p.is_file() and p.name != "README.md"
                        and str(p.relative_to(rust_dir)) not in CPEFORGE_TEMPLATES)
    if php_files != sorted(ADAPTATIONS) or rust_files != sorted(ADAPTATIONS):
        print(f"conjunto de plantillas distinto: PHP {php_files}, Rust {rust_files}", file=sys.stderr)
        return 1
    status = 0
    for name in ADAPTATIONS:
        expected = adapt(name, (php_dir / name).read_bytes().decode("utf-8"))
        target = rust_dir / name
        if mode == "apply":
            target.write_bytes(expected.encode("utf-8"))
        elif target.read_bytes().decode("utf-8") != expected:
            print(f"{name}: no coincide con la plantilla PHP + adaptaciones", file=sys.stderr)
            status = 1
    if status == 0:
        print(f"plantillas: {len(ADAPTATIONS)} archivos = referencia PHP + "
              f"{sum(len(v) for v in ADAPTATIONS.values())} adaptaciones")
    return status


if __name__ == "__main__":
    sys.exit(main())
