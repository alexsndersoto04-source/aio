#!/usr/bin/env python3
"""Resume la salida de verificar.sh en anotaciones de GitHub Actions.

Los logs de Actions no se pueden descargar desde el sandbox, pero las
anotaciones sí se leen por la API. GitHub solo guarda 10 anotaciones de cada
tipo por paso, así que cada una agrupa varios programas.
"""
import sys

def esc(s):
    return s.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")

lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
# Preserve the first compiler/runtime diagnostics in check annotations as well
# as the per-program summary: the Actions log host is not reachable from some
# development sandboxes, and missing IR files otherwise hide the root cause.
context = [
    line for line in lines
    if line and not line.startswith(("DIFERENCIA", "TIEMPO AGOTADO", "no admitido", "programas:", "LLVM:"))
]
if context:
    print("::notice title=LLVM raw diagnostics::" + esc("\n".join(context[:25])[:3500]))
blocks = []
for l in lines:
    if l.startswith(("DIFERENCIA", "TIEMPO AGOTADO", "no admitido")):
        blocks.append([l])
    elif l.startswith("programas:") or l.startswith("LLVM:"):
        print("::notice::[llvm] " + esc(l))
    elif blocks:
        blocks[-1].append(l)
# Primero los que no son pruebas de error (suelen ser los más informativos).
blocks.sort(key=lambda b: "/error_" in b[0])
groups = [blocks[i:i + 5] for i in range(0, min(len(blocks), 45), 5)]
for g in groups:
    text = "\n".join("\n".join(b[:7]) for b in g)
    print("::error::" + esc(text[:3500]))
