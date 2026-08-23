#!/usr/bin/env python3
"""Regenera docs/REFERENCIA_API.md a partir del codigo fuente.

Lee dos fuentes de verdad, sin compilar Rust:

  1. crates/titan_stdlib/src/native.rs  -> tabla NATIVES (registro de nativas)
  2. crates/titan_typechecker/src/lib.rs -> built-ins globales + intrinsecos
     que el compilador conoce con firma propia y baja a opcodes dedicados.

Uso:
    python3 scripts/gen-api-reference.py
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "REFERENCIA_API.md"

PRIM = {
    "String": "string",
    "Int": "int",
    "Bool": "bool",
    "Float": "float",
    "Nil": "nil",
    "Char": "char",
    "Unknown": "any",
    "Bytes": "bytes",
}


def read_natives():
    src = (ROOT / "crates/titan_stdlib/src/native.rs").read_text()
    block = src[src.index("pub static NATIVES"):]
    entries = re.findall(
        r'native!\(\s*"([^"]+)"\s*,\s*\[([^\]]*)\]\s*,\s*([A-Za-z]+)\s*'
        r"(?:,\s*([A-Za-z]+)\s*)?\)",
        block,
        re.S,
    )
    table = {}
    for name, params, result, capability in entries:
        parsed = [p.strip() for p in params.split(",") if p.strip()]
        if name in table:
            sys.exit(f"nombre duplicado en NATIVES: {name}")
        table[name] = (parsed, result, capability or "None")
    return table


def split_top(text):
    parts, depth, current = [], 0, ""
    for ch in text:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(current)
            current = ""
        else:
            current += ch
    return [p.strip() for p in parts + [current] if p.strip()]


def inner(text, head):
    """Devuelve el contenido entre los parentesis balanceados que siguen a `head`."""
    start = text.index("(", len(head) - 1)
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "(":
            depth += 1
        elif text[index] == ")":
            depth -= 1
            if depth == 0:
                return text[start + 1:index]
    return text[start + 1:]


def render_type(raw):
    raw = re.sub(r"\s+", " ", raw).strip().rstrip(",").strip()
    if raw.startswith("Type::Named("):
        return re.sub(r'^"(.*)"\.into\(\)$', r"\1", inner(raw, "Type::Named").strip())
    if raw.startswith("Type::Array("):
        item = inner(raw, "Type::Array").strip()
        if item.startswith("Box::new("):
            item = inner(item, "Box::new")
        return "[" + render_type(item) + "]"
    if raw.startswith("Type::Tuple("):
        body = inner(raw, "Type::Tuple").strip()
        match = re.fullmatch(r"vec!\[(.*)\]", body, re.S)
        if match:
            body = match.group(1)
        return "(" + ", ".join(render_type(p) for p in split_top(body)) + ")"
    if raw.startswith("Type::Function("):
        parts = split_top(inner(raw, "Type::Function"))
        match = re.fullmatch(r"vec!\[(.*)\]", parts[0], re.S)
        args = split_top(match.group(1)) if match else []
        ret = parts[1] if len(parts) > 1 else "Type::Unknown"
        if ret.strip().startswith("Box::new("):
            ret = inner(ret.strip(), "Box::new")
        return "fn(%s) -> %s" % (
            ", ".join(render_type(a) for a in args),
            render_type(ret),
        )
    match = re.fullmatch(r"Type::(\w+)", raw)
    if match and match.group(1) in PRIM:
        return PRIM[match.group(1)]
    return raw


def read_intrinsics():
    src = (ROOT / "crates/titan_typechecker/src/lib.rs").read_text()
    table = {}
    for match in re.finditer(
        r'"((?:std::)?[a-z_0-9:]+)"\.into\(\),\s*FunctionSig\s*\{(.*?)\n\s*\},',
        src,
        re.S,
    ):
        name = match.group(1)
        body = re.sub(r"\s+", " ", match.group(2)).strip()
        parsed = re.search(
            r"params:\s*vec!\[(.*)\]\s*,\s*result:\s*(.*?),?\s*$", body, re.S
        )
        if not parsed:
            continue
        table[name] = (
            [render_type(p) for p in split_top(parsed.group(1))],
            render_type(parsed.group(2)),
        )
    return table


def main():
    natives = read_natives()
    intrinsics = read_intrinsics()
    namespaces = sorted({n.split("::")[1] for n in natives})

    lines = []
    add = lines.append
    add("# Referencia de la API de TITAN — generada desde el código fuente\n")
    add("> Extraída automáticamente de `crates/titan_stdlib/src/native.rs` (registro de")
    add("> nativas) y de `crates/titan_typechecker/src/lib.rs` (built-ins globales e")
    add("> intrínsecos con opcode dedicado). Regenerar con:")
    add("> `python3 scripts/gen-api-reference.py`.")
    add("> Si esta tabla y el compilador difieren, el compilador tiene razón.\n")
    add(
        f"- **{len(natives)} funciones nativas** repartidas en "
        f"**{len(namespaces)} namespaces `std::*`**."
    )
    add(
        f"- **{len(intrinsics)} firmas conocidas directamente por el compilador** "
        f"(18 built-ins globales + {len(intrinsics) - 18} intrínsecos)."
    )
    add("")
    add("**Capacidades.** `None` = función pura, sigue funcionando con")
    add("`titan run --sandbox`. `Filesystem`, `Process`, `Network`, `Environment` y")
    add("`UserInterface` requieren esa capacidad; si está denegada la llamada falla con")
    add("`native function 'f' requires capability 'C'` — nunca se ejecuta en silencio.\n")
    add("**Tipos de handle** (`Sqlite`, `TcpStream`, `Task`, `HttpRouter`, `Postgres`, …)")
    add("son opacos: se guardan en variables y se pasan a los intrínsecos que los")
    add("aceptan, pero no tienen campos ni representación visible.\n")
    add("Varios módulos devuelven `Int` como handle de recurso (`std::kv::open`,")
    add("`std::image::load`, `std::server::start`, `std::router::new`, `std::pdf::new`,")
    add("`std::onnx::load`, …). Ese entero es un descriptor: pásalo a las funciones del")
    add("mismo namespace y ciérralo cuando termines.\n")
    add("---\n")

    add("## 1. Built-ins globales\n")
    add("Sin prefijo `std::`, compilados a opcodes dedicados. **No se pueden usar como")
    add("valores**: pasarlos a otra función da")
    add("`unsupported language feature: built-in function values ('map')`.\n")
    add("| Función | Firma |")
    add("|---|---|")
    for name in sorted(k for k in intrinsics if "::" not in k):
        params, result = intrinsics[name]
        add(f"| `{name}` | `({', '.join(params)}) -> {result}` |")
    add("")

    add("## 2. Intrínsecos `std::*` (opcode dedicado, fuera del registro)\n")
    grouped = {}
    for name, sig in intrinsics.items():
        if "::" in name:
            grouped.setdefault(name.split("::")[1], []).append((name, sig))
    for namespace in sorted(grouped):
        add(f"### `std::{namespace}`\n")
        add("| Función | Firma |")
        add("|---|---|")
        for name, (params, result) in sorted(grouped[namespace]):
            short = name.split("::", 2)[2]
            add(f"| `{short}` | `({', '.join(params)}) -> {result}` |")
        add("")

    add("---\n")
    add("## 3. Registro de nativas `std::*`\n")
    by_namespace = {}
    for name, sig in natives.items():
        by_namespace.setdefault(name.split("::")[1], []).append((name, sig))
    index = " · ".join(
        "[`%s`](#std%s--%d-funciones)"
        % (ns, ns.replace("_", ""), len(by_namespace[ns]))
        for ns in sorted(by_namespace)
    )
    add("**Índice:** " + index + "\n")
    for namespace in sorted(by_namespace):
        functions = sorted(by_namespace[namespace])
        add(f"### `std::{namespace}` — {len(functions)} funciones\n")
        add("| Función | Parámetros | Devuelve | Capacidad |")
        add("|---|---|---|---|")
        for name, (params, result, capability) in functions:
            short = name.split("::", 2)[2]
            shown = ", ".join(params) if params else "—"
            add(f"| `{short}` | `{shown}` | `{result}` | {capability} |")
        add("")

    OUT.write_text("\n".join(lines) + "\n")
    print(f"escrito {OUT.relative_to(ROOT)}: {len(natives)} nativas, "
          f"{len(intrinsics)} firmas del compilador")


if __name__ == "__main__":
    main()
