#!/usr/bin/env python3
"""Normaliza un XML firmado para contrastar la referencia PHP con CPEForge.

CPEForge cambió la identidad respecto de la referencia (Id de la firma y datos
de la empresa de ejemplo), así que los XML ya no son iguales byte a byte. Se
quita el bloque `ds:Signature` (su digest depende de esos datos) y se unifican
los identificadores; todo lo demás debe coincidir exactamente.

Uso: normalizar.py archivo.xml  (escribe el resultado en la salida estándar)
"""

from __future__ import annotations

import re
import sys

SUSTITUCIONES = [
    (rb"<ds:Signature\b.*?</ds:Signature>", b"<ds:Signature/>"),
    (rb"#(GREENTER|CPEFORGE)-SIGN", b"#FIRMA"),
    (rb"admin@(greenter\.com|cpeforge\.dev)", b"correo@ejemplo"),
    (rb"GREENTER|CPEFORGE", b"EMPRESA"),
]


def normalizar(xml: bytes) -> bytes:
    for patron, reemplazo in SUSTITUCIONES:
        xml = re.sub(patron, reemplazo, xml, flags=re.S)
    return xml


if __name__ == "__main__":
    with open(sys.argv[1], "rb") as archivo:
        sys.stdout.buffer.write(normalizar(archivo.read()))
