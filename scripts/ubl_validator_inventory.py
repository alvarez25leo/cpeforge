#!/usr/bin/env python3
"""Inventario reproducible de los XSD de la referencia fija ubl-validator v2.3.0.

Escribe el mismo JSON que crates/ubl-validator/schema-manifest.json."""
import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / 'target/reference/ubl-validator-v2.3.0'
BASE = UPSTREAM / 'src/xsd'
OUTPUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / 'target/evidence/ubl-validator-inventory.json'
XSD = '{http://www.w3.org/2001/XMLSchema}'


def rights(text):
    lower = text.lower()
    head = (text[:8000] + text[-8000:]).lower()
    owners = [name for name in ('oasis', 'w3c', 'etsi', 'un/cefact', 'sunat') if name in head]
    if 'w3c software license' in head:
        basis = 'W3C Software License indicado en cabecera'
    elif 'this document and translations of it may be copied' in lower and 'un/cefact' in head:
        basis = 'permiso UN/CEFACT indicado en archivo; exige conservar aviso'
    elif 'this document and translations of it may be copied' in lower and 'oasis' in head:
        basis = 'permiso OASIS indicado en archivo; exige conservar aviso y no modificar el XSD'
    elif 'modified' in head and 'etsi' in head:
        basis = 'derivado ETSI; permiso de redistribución por verificar'
    elif 'copyright (c) oasis open' in head or 'copyright (c) oasis' in head:
        basis = 'aviso OASIS sin permiso explícito en este archivo; verificar contra release oficial'
    else:
        basis = 'sin licencia individual explícita; atribución y permiso por verificar'
    return owners, basis


def main():
    rows = []
    for path in sorted(BASE.rglob('*.xsd')):
        content = path.read_bytes()
        root = ET.fromstring(content)
        references = []
        for node in root.iter():
            if node.tag not in (XSD + 'import', XSD + 'include', XSD + 'redefine'):
                continue
            location = node.get('schemaLocation')
            references.append({
                'kind': node.tag[len(XSD):],
                'namespace': node.get('namespace'),
                'location': location,
                'exists': location is None or (path.parent / location).is_file(),
            })
        text = content.decode('utf-8', 'replace')
        owners, basis = rights(text)
        notices = re.findall(r'(?im)^\s*[^\n]{0,120}(?:copyright|license|copied|modified)[^\n]{0,180}', text[:8000] + '\n' + text[-8000:])
        rows.append({
            'path': path.relative_to(BASE).as_posix(),
            'sha256': hashlib.sha256(content).hexdigest(),
            'bytes': len(content),
            'target_namespace': root.get('targetNamespace'),
            'owners_in_header': owners,
            'rights_basis': basis,
            'notices': [n.strip() for n in notices[:12]],
            'references': references,
        })
    php = sorted((UPSTREAM / 'src/Ubl').rglob('*.php'))
    assert len(rows) == 45 and len(php) == 9
    assert sum('/maindoc/' in ('/' + r['path']) for r in rows) == 16
    result = {'xsd': rows}
    OUTPUT.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(f'{len(php)} PHP, {len(rows)} XSD, {sum(len(r["references"]) for r in rows)} referencias; {sum(not x["exists"] for r in rows for x in r["references"])} ausentes')


if __name__ == '__main__':
    main()
