#!/usr/bin/env python3
"""Verificación XSD 1.0 independiente: xmlschema (Python puro), sin libxml2."""
import json
from pathlib import Path
import sys

import xmlschema

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'target/reference/ubl-validator-v2.3.0/src/xsd'
CORPUS = ROOT / 'tests/reference/ubl-validator'


def main():
    if xmlschema.__version__ != '4.3.2':
        raise SystemExit(f'xmlschema 4.3.2 requerido; encontrado {xmlschema.__version__}')
    cases = json.loads((CORPUS / 'cases.json').read_text())['cases']
    cache = {}
    mismatches = []
    checked = 0
    resolution_only = {
        'empty', 'malformed', '2.1-Invoice-missing-version',
        '2.1-Invoice-unknown-version', '2.1-Mystery-root',
    }
    for case in cases:
        if case['id'] in resolution_only:
            continue
        version, rest = case['id'].split('-', 1)
        name = rest.split('-')[0]
        key = (version, name)
        if key not in cache:
            path = BASE / version / 'maindoc' / f'UBL-{name}-{version}.xsd'
            # Los 45 archivos e imports se auditan por SHA-256; allow='local'
            # bloquea URL remotas y el runner bloquea toda la red. Se usa
            # 'local' porque los imports ../common quedan fuera de maindoc.
            cache[key] = xmlschema.XMLSchema10(str(path.resolve()), allow='local', defuse='always')
        actual = cache[key].is_valid(str((CORPUS / 'xml' / case['file']).resolve()))
        checked += 1
        if actual != case['expected_valid']:
            mismatches.append(f"{case['id']}: esperado={case['expected_valid']} independiente={actual}")
    print(f'xmlschema {xmlschema.__version__}: {checked} casos XSD, {len(cache)} XSD raíz, {len(mismatches)} diferencias')
    if mismatches:
        print('\n'.join(mismatches), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
