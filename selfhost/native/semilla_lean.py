#!/usr/bin/env python3
"""Genera el runtime "delgado" de la semilla (usado por bootstrap_semilla.sh).

La VM de Rust no puede interpretar el compilador con el runtime completo (135 000 líneas) en una máquina de
4 GB: el sistema mata el proceso. El compilador no necesita SQLite, tokenize, ONNX, audio, imágenes,
PostgreSQL ni MySQL para compilarse a sí mismo; esos módulos se sustituyen por un fichero de
funciones vacías (misma firma, cuerpo `rt_fatal`) solo para las funciones que el resto del runtime
nombra. El punto fijo (titanc1 == titanc2 == titanc3) se hace con el runtime completo.

Uso: semilla_lean.py <runtime.titan> <directorio native> <salida runtime> <salida stub>
"""
import os
import re
import sys

rt_path, native, out_rt, out_stub = sys.argv[1:5]
PREFIX = ("std_sqlite", "std_tokenize", "std_onnx", "std_audio", "std_postgres", "std_mysql", "image::")

src = open(rt_path, encoding="utf-8").read().split("\n")


def stubbed(mod):
    return mod.startswith(PREFIX)


def mod_file(mod):
    return os.path.join(native, mod.replace("::", "/") + ".titan")


removed = []
kept_text = []
for line in src:
    m = re.match(r"^import (\S+)$", line)
    if m and stubbed(m.group(1)):
        removed.append(m.group(1))
        kept_text.append(None)
    else:
        kept_text.append(line)

# texto de todo lo que se conserva (runtime + módulos importados que siguen)
rest = ["\n".join(l for l in kept_text if l is not None)]
for line in kept_text:
    m = line and re.match(r"^import (\S+)$", line)
    if m:
        p = mod_file(m.group(1))
        if os.path.exists(p):
            rest.append(open(p, encoding="utf-8").read())
rest_text = "\n".join(rest)
defined = set(re.findall(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", rest_text))
words = set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(", rest_text)) - defined

sig = re.compile(r"^(pub\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\((.*)$")
out = ["// Sustituto de los módulos pesados: solo lo usa bootstrap_semilla.sh\n"]
count = 0
for mod in removed:
    p = mod_file(mod)
    if not os.path.exists(p):
        continue
    lines = open(p, encoding="utf-8").read().split("\n")
    i = 0
    while i < len(lines):
        m = sig.match(lines[i])
        if m and m.group(2) in words:
            head = lines[i]
            while not head.rstrip().endswith("{"):
                i += 1
                head += " " + lines[i].strip()
            h = re.sub(r"\s*->\s*.*$", "", head.rstrip()[:-1].rstrip()) + " -> any"
            out.append(h + " {\n    rt_fatal(\"módulo no disponible en la semilla\")\n}\n")
            count += 1
        i += 1

open(out_stub, "w", encoding="utf-8").write("\n".join(out))
# sustituir el primer import eliminado por el del sustituto
done = False
res = []
for line, orig in zip(kept_text, src):
    if line is None:
        if not done:
            res.append("import std_lean_stub")
            done = True
        continue
    res.append(line)
open(out_rt, "w", encoding="utf-8").write("\n".join(res))
print(f"semilla delgada: {len(removed)} módulos sustituidos por {count} funciones vacías")
