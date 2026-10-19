#!/usr/bin/env python3
"""Inspect Greenter tags and changes without changing the local checkout."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "oracle" / "upstream" / "referencia.json"
STABLE_TAG = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")


def git(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout.strip()


def stable_tags(repo: Path) -> list[tuple[tuple[int, int, int], str]]:
    found = []
    for tag in git(repo, "tag", "--list").splitlines():
        match = STABLE_TAG.fullmatch(tag)
        if match:
            found.append((tuple(map(int, match.groups())), tag))
    return sorted(found)


def diff_files(repo: Path, old: str, new: str) -> list[tuple[str, str]]:
    output = git(repo, "diff", "--name-status", "--find-renames", old, new, "--")
    rows = []
    for line in output.splitlines():
        parts = line.split("\t")
        if len(parts) >= 2:
            rows.append((parts[0], parts[-1]))
    return rows


def print_delta(repo: Path, old: str, new: str) -> None:
    if old == new:
        print("Sin cambios de código entre estas referencias.\n")
        return

    count = git(repo, "rev-list", "--count", f"{old}..{new}")
    files = diff_files(repo, old, new)
    packages = Counter(
        path.split("/")[1] if path.startswith("packages/") and len(path.split("/")) > 1 else "(raíz)"
        for _, path in files
    )
    print(f"Commits: {count}; archivos cambiados: {len(files)}")
    print("Paquetes y archivos cambiados:")
    for package, total in sorted(packages.items()):
        print(f"- {package}: {total}")
    print("\nCommits:")
    for line in git(repo, "log", "--format=%h %cs %s", f"{old}..{new}").splitlines():
        print(f"- {line}")
    print("\nArchivos (A=agregado, M=modificado, D=eliminado, R=renombrado):")
    for status, path in files:
        print(f"- {status} {path}")
    print()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Actualiza refs y muestra cambios de Greenter, sin cambiar el checkout."
    )
    parser.add_argument(
        "--offline", action="store_true", help="Usa las refs locales sin hacer git fetch."
    )
    args = parser.parse_args()

    config = json.loads(REFERENCE.read_text(encoding="utf-8"))
    repo = ROOT / config["local_path"]
    if not (repo / ".git").exists():
        raise RuntimeError(f"Falta el clon Git: {repo}")

    remote = git(repo, "remote", "get-url", "origin")
    expected = config["repository"]
    if remote != expected:
        raise RuntimeError(f"El remoto origin no coincide: {remote!r} != {expected!r}")

    if not args.offline:
        git(repo, "fetch", "origin", "--tags")

    baseline = config["baseline_tag"]
    baseline_commit = git(repo, "rev-parse", f"{baseline}^{{commit}}")
    if baseline_commit != config["baseline_commit"]:
        raise RuntimeError("El commit del tag baseline no coincide con referencia.json")

    tags = stable_tags(repo)
    if not tags:
        raise RuntimeError("No se encontraron tags estables vMAJOR.MINOR.PATCH")
    latest = tags[-1][1]
    latest_commit = git(repo, "rev-parse", f"{latest}^{{commit}}")
    origin_head = git(repo, "symbolic-ref", "--short", "refs/remotes/origin/HEAD")
    head_commit = git(repo, "rev-parse", origin_head)
    dirty = bool(git(repo, "status", "--porcelain=v1"))

    print("# Estado de Greenter PHP upstream")
    print(f"\nConsulta UTC: {datetime.now(timezone.utc).isoformat(timespec='seconds')}")
    print(f"Repositorio: {expected}")
    print(f"Clon: {repo}")
    print(f"Checkout actual: {git(repo, 'rev-parse', '--short', 'HEAD')}; {'con cambios' if dirty else 'limpio'}")
    print(f"Referencia de paridad: {baseline} ({baseline_commit})")
    print(f"Último tag estable: {latest} ({latest_commit})")
    print(f"Rama remota: {origin_head} ({head_commit})")
    print("Modo: offline; las refs pueden estar antiguas" if args.offline else "Modo: fetch completado")

    print("\n## Cambios publicados desde la referencia\n")
    if latest == baseline:
        print("No hay un tag estable posterior a la referencia.\n")
    else:
        print_delta(repo, baseline, latest)

    print("## Cambios de desarrollo aún no incluidos en el último tag estable\n")
    print_delta(repo, latest, origin_head)
    print("Este informe lista cambios; la clasificación semántica y la paridad requieren revisión humana/agente y pruebas.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, KeyError, ValueError, RuntimeError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
