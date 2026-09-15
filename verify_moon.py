#!/usr/bin/env python3
"""Verificacion estatica de Moon (projects/moon/) sin compilar Rust.

Comprueba, contra las fuentes del runtime de Titan:

1. Cada llamada `std::mod::fn(...)` del backend existe en alguno de los dos
   registros nativos (titan_stdlib/native.rs y titan_typechecker/lib.rs) y
   recibe EXACTAMENTE la cantidad de argumentos declarada.
2. Cada ruta `/api/...` que llama el frontend (frontend/src) tiene su
   contraparte en el router del backend (projects/moon/src/main.titan).

Uso:  python3 verify_moon.py
Salida: lista de problemas, o "TODO VERIFICADO".
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent


def native_signatures() -> dict[str, tuple[int, str]]:
    """Aridad de cada nativa, tomada de los dos registros del compilador."""
    sig: dict[str, tuple[int, str]] = {}

    stdlib = (ROOT / "crates/titan_stdlib/src/native.rs").read_text()
    for name, params in re.findall(r'native!\(\s*"([^"]+)",\s*\[([^\]]*)\]', stdlib):
        argc = 0 if not params.strip() else len([p for p in params.split(",") if p.strip()])
        sig[name] = (argc, "stdlib")

    checker = (ROOT / "crates/titan_typechecker/src/lib.rs").read_text()
    for match in re.finditer(
        r'functions\.insert\(\s*"([^"]+)"\.into\(\),\s*FunctionSig\s*\{\s*params:\s*vec!\[', checker
    ):
        i, depth, commas, has = match.end(), 1, 0, False
        while i < len(checker) and depth:
            char = checker[i]
            if char == "[":
                depth += 1
            elif char == "]":
                depth -= 1
            elif depth == 1 and char == ",":
                commas += 1
            elif depth == 1 and not char.isspace():
                has = True
            i += 1
        sig.setdefault(match.group(1), (commas + (1 if has else 0), "typechecker"))

    return sig


def count_args(text: str, open_index: int) -> int:
    """Cuenta argumentos de la llamada cuyo '(' esta en open_index."""
    i, depth, commas, has = open_index, 0, 0, False
    while i < len(text):
        char = text[i]
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth == 0:
                break
        elif char == '"':
            if depth == 1:
                has = True
            i += 1
            while i < len(text) and text[i] != '"':
                if text[i] == "\\":
                    i += 1
                i += 1
        elif char == "/" and text[i : i + 2] == "//":
            while i < len(text) and text[i] != "\n":
                i += 1
        elif depth == 1 and char == ",":
            commas += 1
        elif depth == 1 and not char.isspace():
            has = True
        i += 1
    return commas + (1 if has else 0)


def check_calls() -> tuple[int, list[str]]:
    sig = native_signatures()
    problems: list[str] = []
    total = 0
    for path in sorted((ROOT / "projects/moon/src").glob("*.titan")):
        text = path.read_text()
        for match in re.finditer(r"\b(std::[a-z_0-9]+::[a-z_0-9]+)\s*\(", text):
            total += 1
            name = match.group(1)
            line = text[: match.start()].count("\n") + 1
            if name not in sig:
                problems.append(f"{path.name}:{line}: no existe en ningun registro: {name}")
                continue
            got = count_args(text, match.end() - 1)
            want = sig[name][0]
            if got != want:
                problems.append(
                    f"{path.name}:{line}: {name} espera {want} args ({sig[name][1]}), recibe {got}"
                )
    return total, problems


def route_literals(expr: str, helpers: dict[str, str]) -> str:
    expr = expr.replace("brace_open()", '"{"').replace("brace_close()", '"}"')
    expr = re.sub(r"\b(pat_\w+)\(\)", lambda m: f'"{helpers.get(m.group(1), "?")}"', expr)
    return "".join(re.findall(r'"([^"]*)"', expr))


def normalise(route: str) -> str:
    route = re.sub(r"\$\{[^}]*\}", "{}", route)  # plantillas JS primero
    route = re.sub(r"\{[^}]*\}", "{}", route)
    return route.split("?")[0]


def check_routes() -> tuple[int, int, list[str]]:
    main = (ROOT / "projects/moon/src/main.titan").read_text()

    helpers: dict[str, str] = {}
    for match in re.finditer(r"fn\s+(\w+)\s*\(\s*\)\s*->\s*string\s*\{(.*?)\n\}", main, re.S):
        literal = route_literals(match.group(2), {})
        if literal.startswith("/api"):
            helpers[match.group(1)] = literal

    backend: set[str] = set()
    pattern = r'((?:pat_\w+\(\)|"[^"]*")(?:\s*\+\s*(?:"[^"]*"|brace_open\(\)|brace_close\(\)|\w+\(\)))*)'
    for match in re.finditer(pattern, main):
        route = normalise(route_literals(match.group(1), helpers))
        if route.startswith("/api"):
            backend.add(route)

    frontend: set[str] = set()
    files = list((ROOT / "frontend/src").rglob("*.js")) + list((ROOT / "frontend/src").rglob("*.jsx"))
    for path in files:
        for route in re.findall(r'[`\'"](/api/[^`\'"]*)[`\'"]', path.read_text()):
            frontend.add(normalise(route))

    problems: list[str] = []
    for route in sorted(frontend - backend):
        # Una plantilla como /api/feed/${tab} es valida si cada valor posible
        # corresponde a una ruta estatica declarada (p. ej. /api/feed/latest).
        dynamic = re.compile("^" + re.escape(route).replace(r"\{\}", "[^/]+") + "$")
        if any(dynamic.match(declared) for declared in backend):
            continue
        problems.append(f"el frontend llama {route!r} y el backend no la declara")
    return len(backend), len(frontend), problems


def main() -> int:
    total_calls, call_problems = check_calls()
    backend, frontend, route_problems = check_routes()

    print(f"[1] llamadas std::* en el backend: {total_calls}")
    print(f"[2] rutas del router: {backend} | rutas distintas usadas por el frontend: {frontend}")

    problems = call_problems + route_problems
    if problems:
        print(f"\nPROBLEMAS: {len(problems)}")
        for problem in problems:
            print("   X", problem)
        return 1

    print("\nOK: todas las llamadas std::* existen con la aridad correcta y todas las")
    print("    rutas del frontend tienen contraparte en el backend.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
