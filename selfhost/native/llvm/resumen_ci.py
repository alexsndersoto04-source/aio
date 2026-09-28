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
blocks = []
for l in lines:
    if l.startswith("DIFERENCIA") or l.startswith("no admitido"):
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
