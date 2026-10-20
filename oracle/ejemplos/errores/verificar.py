#!/usr/bin/env python3
"""Compara XML firmado y, si se envió, código y mensaje SUNAT PHP ↔ Rust.

Los casos marcados `solo_rust` en casos.json no tienen equivalente PHP: se
validan contra el XSD y, con --enviar, se exige su código de rechazo.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent.parent
EXAMPLE = ROOT / "example"
sys.path.insert(0, str(HERE.parent))
from normalizar import normalizar  # noqa: E402


def mensaje_comparable(message: str) -> str:
    """El ticket generado por BETA es distinto en cada solicitud."""
    return re.sub(r"(?<=ticket: )\d+(?= error: )", "<ticket>", message)


def informe(lenguaje: str) -> dict[str, dict]:
    path = EXAMPLE / lenguaje / "out" / "errores" / "benchmark.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    if not data.get("errores"):
        raise ValueError(f"{path}: no procede de --errores")
    cases = data["casos"]
    names = [case["caso"] for case in cases]
    if len(names) != len(set(names)):
        raise ValueError(f"{path}: casos duplicados")
    return {case["caso"]: case for case in cases}


def xsd_problems(name: str, case: dict, xml: Path, xsd: Path, language: str) -> list[str]:
    if not xsd.is_dir():
        print(f"{name}: XSD no ejecutado (falta el directorio de esquemas)")
        return []
    root, version = case["xsd_raiz"], case["version_ubl"]
    schema = xsd / version / "maindoc" / f"UBL-{root}-{version}.xsd"
    result = subprocess.run(
        ["xmllint", "--noout", "--nonet", "--schema", str(schema), str(xml)],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        return [f"{name}: XSD inválido en {language}: {result.stderr.strip()[:300]}"]
    return []


def solo_rust(name: str, case: dict, response: dict, xsd: Path, enviar: bool) -> list[str]:
    """Caso sin equivalente PHP: XSD y, si se envió, el código de rechazo esperado."""
    problems = xsd_problems(name, case, EXAMPLE / "rust/out/errores" / response["archivo"], xsd, "Rust")
    if enviar and str(response["codigo"]) != case["codigo_esperado"]:
        problems.append(f"{name}: Rust código {response['codigo']} != {case['codigo_esperado']}")
    if not problems:
        detalle = f"rechazo {response['codigo']}" if enviar else "XSD válido"
        print(f"{name}: solo Rust, {detalle} ({response['archivo']})")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--enviar", action="store_true", help="exige respuestas de los servicios de prueba")
    parser.add_argument("casos", nargs="*", help="casos seleccionados; por defecto todos")
    args = parser.parse_args()
    manifest = json.loads((HERE / "casos.json").read_text(encoding="utf-8"))
    selected = args.casos or list(manifest)
    unknown = set(selected) - set(manifest)
    if unknown or len(selected) != len(set(selected)):
        raise ValueError(f"casos desconocidos o repetidos: {sorted(unknown)}")
    php, rust = informe("php"), informe("rust")
    shared = [name for name in selected if not manifest[name].get("solo_rust")]
    if set(php) != set(shared) or set(rust) != set(selected):
        raise ValueError("los resultados PHP/Rust no contienen exactamente los casos elegidos")

    xsd = ROOT / "target/reference/ubl-validator-v2.3.0/src/xsd"
    problems = []
    for name in selected:
        if manifest[name].get("solo_rust"):
            problems += solo_rust(name, manifest[name], rust[name], xsd, args.enviar)
            continue
        left, right = php[name], rust[name]
        filename = left["archivo"]
        if filename != right["archivo"]:
            problems.append(f"{name}: nombres XML distintos")
            continue
        php_xml = EXAMPLE / "php/out/errores" / filename
        rust_xml = EXAMPLE / "rust/out/errores" / filename
        if normalizar(php_xml.read_bytes()) != normalizar(rust_xml.read_bytes()):
            problems.append(f"{name}: XML distinto (sin firma ni identidad)")
        else:
            print(f"{name}: XML equivalente ({filename})")

        if xsd.is_dir():
            root = manifest[name]["xsd_raiz"]
            version = manifest[name]["version_ubl"]
            schema = xsd / version / "maindoc" / f"UBL-{root}-{version}.xsd"
            for language, xml in (("PHP", php_xml), ("Rust", rust_xml)):
                result = subprocess.run(
                    ["xmllint", "--noout", "--nonet", "--schema", str(schema), str(xml)],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode:
                    problems.append(f"{name}: XSD inválido en {language}: {result.stderr.strip()[:300]}")
        else:
            print(f"{name}: XSD no ejecutado (falta el directorio de esquemas)")

        if args.enviar:
            for language, response in (("PHP", left), ("Rust", right)):
                if response["resultado"] not in ("rechazada", f"error {manifest[name]['codigo_esperado']}"):
                    problems.append(f"{name}: {language} no obtuvo rechazo SUNAT: {response['resultado']}")
                if str(response["codigo"]) != manifest[name]["codigo_esperado"]:
                    problems.append(f"{name}: {language} código {response['codigo']} != {manifest[name]['codigo_esperado']}")
                if not response["mensaje"]:
                    problems.append(f"{name}: {language} sin mensaje SUNAT")
            for field in ("resultado", "codigo", "observaciones"):
                if left[field] != right[field]:
                    problems.append(f"{name}: {field} PHP={left[field]!r} Rust={right[field]!r}")
            if mensaje_comparable(left["mensaje"] or "") != mensaje_comparable(right["mensaje"] or ""):
                problems.append(f"{name}: mensaje PHP={left['mensaje']!r} Rust={right['mensaje']!r}")
            if not any(problem.startswith(f"{name}:") for problem in problems):
                suffix = " (salvo el ticket)" if left["mensaje"] != right["mensaje"] else ""
                print(f"{name}: rechazo {left['codigo']} y mensaje iguales{suffix}: {mensaje_comparable(left['mensaje'])}")

    for problem in problems:
        print(f"FALLO: {problem}", file=sys.stderr)
    if not problems:
        print(f"OK: {len(selected)} casos comparados" + (" con respuestas de prueba" if args.enviar else " sin envío"))
    return 1 if problems else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f"FALLO: {error}", file=sys.stderr)
        sys.exit(1)
