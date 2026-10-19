#!/usr/bin/env python3
"""Traduce la salida del oráculo PHP a la identidad de CPEForge.

CPEForge cambió los identificadores que la referencia (Greenter PHP) escribe
en sus resultados: el Id y la URI de la firma, el User-Agent de la API GRE y
el espacio de nombres de las clases (`Greenter\\…` → `CPEForge\\…`). Los
fixtures de tests/reference/ usan la identidad nueva, así que cada gate de
paridad pasa la salida del oráculo por este módulo antes de compararla. Es
una traducción fija de nombres: ningún resultado, importe, mensaje ni byte del
XML de negocio cambia.

Además, en los campos descriptivos de los JSON (`description`, `reason`,
`source`, …) aplica la misma redacción neutral con la que se versionan los
fixtures (sin nombrar el proyecto ni el lenguaje de la referencia).

`a_referencia` hace el camino inverso para las entradas que el oráculo debe
recibir con el nombre original (por ejemplo, la función XSLT `matches`).

Uso:
    identidad.py RUTA...             traduce en el sitio archivos o directorios
    identidad.py --inversa RUTA...   traduce a los nombres de la referencia
    identidad.py --sha256 DIR        recalcula el `sha256` de cada caso de
                                     DIR/<familia>.json con DIR/<familia>/<id>.xml
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import re
import sys
from pathlib import Path

# Identificadores observables (salida del oráculo → CPEForge).
_SALIDA = [
    (re.compile(r"GreenterSign"), "CPEForgeSign"),
    (re.compile(r"#GREENTER-SIGN"), "#CPEFORGE-SIGN"),
    (re.compile(r"Greenter-Client/1\.0\.0/PHP"), "CPEForge/0.1.0"),
    (re.compile(r"\bGreenter(?=\\)"), "CPEForge"),
    # Los mensajes de ext/xsl escriben el nombre de la función en minúsculas.
    (re.compile(r"\bgreenter(?=\\+validator\\+xml\\+xsltfunctions)"), "cpeforge"),
]

# Las mismas reglas sobre bytes, para los documentos que el oráculo entrega en
# Base64 (`{"b64": …}`) porque no son UTF-8; los patrones son ASCII.
_SALIDA_BYTES = [(re.compile(p.pattern.encode()), n.encode()) for p, n in _SALIDA]
_B64 = re.compile(r'("b64"\s*:\s*")([A-Za-z0-9+/=]*)(")')

# Entradas que la referencia necesita con su nombre original.
_ENTRADA = [
    (re.compile(r"\bCPEForge(?=\\+Validator\\+Xml\\+XsltFunctions)"), "Greenter"),
    (re.compile(r"\bcpeforge(?=\\+validator\\+xml\\+xsltfunctions)"), "greenter"),
]

_CLAVES = (
    r"(description|reason|compatibility_note|source|note|notes|motivo|comment|"
    r"summary|why|detail|explanation|justification|rationale)"
)
_CAMPO = re.compile(r'^(\s*"' + _CLAVES + r'"\s*:\s*")((?:[^"\\]|\\.)*)(".*)$')
_PROTEGIDOS = re.compile(
    r"php:functionString|php:function|xmlns:php|http://php\.net/xsl|\w+\.php\b|"
    r"ext-xsl|ext/xsl|ext-zlib|ext/dom"
)


def _prosa(texto: str) -> str:
    """Redacción neutral de un texto descriptivo."""
    r = texto
    r = re.sub(r"`?\bthegreenter/[A-Za-z0-9_.-]+`?(\s+`?[0-9a-f]{40}`?)?", "la referencia", r)
    r = re.sub(r"`?\bgreenter/[a-z-]+`?(\s+v?\d+(\.\d+|\.x)*)?", "la referencia", r)
    r = re.sub(r"\bGreenter PHP\b(\s+v?\d+(\.\d+)*)?", "PHP", r)
    r = re.sub(r"\bGreenter\b(\s+v?\d+(\.\d+)*)?", "PHP", r)
    r = re.sub(r"\bPHP\s+v?\d+(\.\d+)*(\.x)?", "PHP", r)
    r = r.replace("PHP ↔ Rust", "referencia ↔ Rust").replace("PHP↔Rust", "referencia↔Rust")
    r = r.replace("Rust ↔ PHP", "Rust ↔ referencia")
    r = re.sub(r"\b(el|del) oráculo PHP\b", r"\1 oráculo de referencia", r)
    r = re.sub(r"\boráculo PHP\b", "oráculo de referencia", r)
    r = re.sub(
        r"\b(salidas?|resultados?|fixtures?|valores?|errores?|mensajes?|casos?|ejecución|"
        r"veredictos?|verificación|lado|runtime|referencia|implementación|versión|paquete|"
        r"repositorio|código|original|motor) PHP\b",
        r"\1 de referencia",
        r,
    )
    r = re.sub(r"\bel PHP\b", "la referencia", r)
    r = re.sub(r"\bdel PHP\b", "de la referencia", r)
    r = re.sub(r"\b(de|en|a|que|como|con|para|por|sin|y|o|según|desde|contra|entre) PHP\b", r"\1 la referencia", r)
    r = re.sub(r"\b(De|En|A|Como|Con|Para|Por|Sin|Según|Desde|Contra) PHP\b", r"\1 la referencia", r)
    r = re.sub(
        r"\b(clase|propiedad|nombre|getter|setter|constantes?|tipo|booleano|excepción|mapper|"
        r"resolutor|fijado|interfaz|parámetro|array|float|cadena|aviso|despacho|semántica)"
        r"( de la excepción)? PHP\b",
        r"\1\2",
        r,
    )
    r = re.sub(r'(^|[.:;!?]\s+|//[/!]?\s*|^\s*\*\s*|"\s*)PHP\b', lambda m: m.group(1) + "La referencia", r)
    r = re.sub(r"\bPHP\b", "la referencia", r)
    r = r.replace("la la referencia", "la referencia").replace("de la la", "de la")
    r = re.sub(r"([;:]) La referencia", r"\1 la referencia", r)
    return r


def _prosa_protegida(texto: str) -> str:
    guardados: dict[str, str] = {}

    def guardar(m: re.Match[str]) -> str:
        clave = f"\x00{len(guardados)}\x00"
        guardados[clave] = m.group(0)
        return clave

    r = _prosa(_PROTEGIDOS.sub(guardar, texto))
    for clave, valor in guardados.items():
        r = r.replace(clave, valor)
    return re.sub(r"la referencia\s+[0-9a-f]{40}\s+packages/", "packages/", r)


def _campos_descriptivos(texto: str) -> str:
    lineas = []
    for linea in texto.split("\n"):
        m = _CAMPO.match(linea)
        if m:
            linea = m.group(1) + _prosa_protegida(m.group(3)) + m.group(4)
        lineas.append(linea)
    return "\n".join(lineas)


def _b64(m: re.Match[str]) -> str:
    try:
        datos = base64.b64decode(m.group(2), validate=True)
    except binascii.Error:
        return m.group(0)
    nuevos = datos
    for patron, nuevo in _SALIDA_BYTES:
        nuevos = patron.sub(nuevo, nuevos)
    if nuevos == datos:
        return m.group(0)
    return m.group(1) + base64.b64encode(nuevos).decode("ascii") + m.group(3)


def a_cpeforge(texto: str, json: bool = True) -> str:
    """Salida del oráculo → identidad CPEForge."""
    for patron, nuevo in _SALIDA:
        texto = patron.sub(nuevo, texto)
    if not json:
        return texto
    return _campos_descriptivos(_B64.sub(_b64, texto))


def a_referencia(texto: str) -> str:
    """Entrada de CPEForge → nombres que reconoce la referencia."""
    for patron, nuevo in _ENTRADA:
        texto = patron.sub(nuevo, texto)
    return texto


def recalcular_sha256(directorio: Path) -> None:
    """Actualiza el `sha256` de los resúmenes tras traducir los XML.

    Sustituye el texto del hash anterior para conservar el formato del JSON.
    """
    for resumen in sorted(directorio.glob("*.json")):
        familia = directorio / resumen.stem
        texto = resumen.read_bytes().decode("utf-8")
        datos = json.loads(texto)
        for caso, resultado in datos.get("results", {}).items():
            anterior = resultado.get("sha256") if isinstance(resultado, dict) else None
            xml = familia / f"{caso}.xml"
            if anterior and xml.is_file():
                nuevo = hashlib.sha256(xml.read_bytes()).hexdigest()
                texto = texto.replace(f'"{anterior}"', f'"{nuevo}"')
        resumen.write_bytes(texto.encode("utf-8"))


def _archivos(rutas: list[str]) -> list[Path]:
    salida: list[Path] = []
    for ruta in map(Path, rutas):
        salida.extend(sorted(p for p in ruta.rglob("*") if p.is_file()) if ruta.is_dir() else [ruta])
    return salida


def main(argv: list[str]) -> int:
    if argv[:1] == ["--sha256"]:
        for directorio in argv[1:]:
            recalcular_sha256(Path(directorio))
        return 0
    inversa = "--inversa" in argv
    for ruta in _archivos([a for a in argv if a != "--inversa"]):
        try:
            texto = ruta.read_bytes().decode("utf-8")
        except UnicodeDecodeError:
            continue  # binarios (ZIP, PNG…): no llevan identificadores de texto
        nuevo = a_referencia(texto) if inversa else a_cpeforge(texto, ruta.suffix == ".json")
        if nuevo != texto:
            ruta.write_bytes(nuevo.encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
