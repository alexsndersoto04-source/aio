#!/usr/bin/env python3
"""Cross-reference the native API inventory, Titan definitions, and test sources.

This is deliberately a source-reference audit, not a claim of branch or
behavioral coverage. The differential runner executes the test programs
separately and compares VM/native results.
"""
from __future__ import annotations

import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
INVENTORY = ROOT / "selfhost" / "natives.titan"
NATIVE = ROOT / "selfhost" / "native"
TESTS = ROOT / "selfhost" / "tests" / "native"
API_RE = re.compile(r"\bstd::[a-z_0-9]+::[a-z_0-9]+\b")
DEFINITION_RE = re.compile(r"^fn (std__[a-z0-9_]+)\b", re.MULTILINE)


def main() -> int:
    inventory_text = INVENTORY.read_text(encoding="utf-8")
    registered = set(re.findall(r'"(std::[a-z_0-9]+::[a-z_0-9]+)', inventory_text))
    if not registered:
        raise SystemExit(f"no native signatures found in {INVENTORY}")

    definitions: set[str] = set()
    for path in NATIVE.rglob("*.titan"):
        definitions.update(DEFINITION_RE.findall(path.read_text(encoding="utf-8")))
    implemented = {name for name in registered if name.replace("::", "__") in definitions}

    # `try::catch` is lowered to dedicated instructions in both native
    # backends, so it has no ordinary `fn std__try__catch` definition.
    backend = (NATIVE / "backend.titan").read_text(encoding="utf-8")
    llvm = (NATIVE / "llvm.titan").read_text(encoding="utf-8")
    if 'k == "TryCall"' in backend and 'k == "TryCall"' in llvm:
        if "std::try::catch" in registered:
            implemented.add("std::try::catch")

    test_files = sorted(TESTS.glob("*.titan"))
    if not test_files:
        raise SystemExit(f"no native test programs found in {TESTS}")
    references: set[str] = set()
    for path in test_files:
        references.update(API_RE.findall(path.read_text(encoding="utf-8")))

    cited_registered = references & registered
    cited_implemented = cited_registered & implemented
    uncited_implemented = implemented - cited_registered
    missing_definitions = cited_registered - implemented
    unknown_references = references - registered

    by_module: dict[str, dict[str, int]] = defaultdict(
        lambda: {"registered": 0, "implemented": 0, "cited": 0, "cited_implemented": 0}
    )
    for name in registered:
        module = name.split("::", 2)[1]
        row = by_module[module]
        row["registered"] += 1
        row["implemented"] += name in implemented
        row["cited"] += name in cited_registered
        row["cited_implemented"] += name in cited_implemented

    print("## Cruce estático de nativas y pruebas")
    print()
    print(f"- Firmas nativas registradas: **{len(registered)}**")
    print(f"- Firmas con función Titan o intrínseco en ambos backends: **{len(implemented)}**")
    print(f"- Programas `.titan` en la suite nativa: **{len(test_files)}**")
    print(f"- Nombres de API distintos citados por los archivos de prueba: **{len(cited_registered)}**")
    print(f"- De esos nombres, con definición Titan/intrínseco: **{len(cited_implemented)}**")
    print(f"- Nombres citados sin definición Titan/intrínseco: **{len(missing_definitions)}**")
    print(f"- Nombres con definición que no aparecen citados en la suite: **{len(uncited_implemented)}**")
    if unknown_references:
        print(f"- Referencias no registradas: {', '.join(sorted(unknown_references))}")
    print()
    print("| Namespace | Firmas | Con Titan | Citadas en pruebas | Citadas y con Titan |")
    print("| --- | ---: | ---: | ---: | ---: |")
    for module in sorted(by_module):
        row = by_module[module]
        print(
            f"| `std::{module}` | {row['registered']} | {row['implemented']} "
            f"| {row['cited']} | {row['cited_implemented']} |"
        )
    print()
    print(
        "> Las columnas de pruebas cuentan nombres mencionados en el código fuente; "
        "no miden ramas ejecutadas. La prueba conductual es el verificador diferencial, "
        "que ejecuta los programas asignados y compara salida, errores y código de salida; "
        "también compara los archivos generados por las pruebas de imagen."
    )
    if missing_definitions:
        print()
        print("Nombres citados sin definición:")
        for name in sorted(missing_definitions):
            print(f"- `{name}`")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
