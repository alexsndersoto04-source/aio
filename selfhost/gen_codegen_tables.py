#!/usr/bin/env python3
"""Genera selfhost/codegen_tables.titan desde crates/titan_codegen/src/lib.rs.

La tabla lista las llamadas por nombre que el generador de Rust traduce a una
instrucción propia de la VM (`"std::net::tcp_read" if args.len() == 2 =>
self.emit(Op::TcpRead)`). Se extrae del código fuente para no copiarla a mano.
Uso: python3 selfhost/gen_codegen_tables.py
"""
import os
import re

root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src = open(os.path.join(root, "crates/titan_codegen/src/lib.rs")).read()
start = src.index('match name.as_str() {\n                "print" | "println"')
end = src.index("_ if titan_stdlib::native::contains(name)", start)
block = re.sub(r"=>\s*\{\s*self\.emit\((Op::\w+)\)\s*\}", r"=> self.emit(\1)", src[start:end])
entries = [
    (m.group(1), int(m.group(3)) if m.group(3) else 0, m.group(4))
    for m in re.finditer(
        r'"([^"]+)" if args\.(len\(\) == (\d+)|is_empty\(\)) => self\.emit\(Op::(\w+)\)', block
    )
]
arms = len(re.findall(r"self\.emit\(Op::", block))
assert len(entries) + 1 == arms, (len(entries), arms)  # +1: print/println
keys = [f"{n}#{a}" for n, a, _ in entries]
assert len(keys) == len(set(keys)), "clave repetida"
out = [
    "// GENERADO por selfhost/gen_codegen_tables.py desde crates/titan_codegen/src/lib.rs.",
    "// No editar a mano. Llamadas que el generador traduce a una instrucción propia",
    "// de la VM, con la clave \"nombre#número_de_argumentos\".",
    "fn cg_dedicated_table() -> map {",
    "    let mut m = std::map::new()",
]
out += [f'    m = std::map::insert(m, "{n}#{a}", "{op}")' for n, a, op in entries]
out += ["    m", "}"]
open(os.path.join(root, "selfhost/codegen_tables.titan"), "w").write("\n".join(out) + "\n")
print(f"{len(entries)} entradas")
