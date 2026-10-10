#!/usr/bin/env python3
"""Contrasta los diagnósticos del LSP con `zett check` (el oráculo de Rust) sobre muchos archivos.

Uso: python3 contraste_zett.py [CLI] [ZETT]
Para cada .titan sin `import` (el LSP revisa el documento solo, `check` carga el proyecto) compara
mensaje y posición (línea:columna) de cada error de tipos, de parseo o léxico."""
import glob, json, os, re, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cliente import sesion, frame

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
CLI = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.join(RAIZ, "selfhost/titan")
ZETT = sys.argv[2] if len(sys.argv) > 2 else "zett"

def lsp_diags(path, text):
    msgs = [
        {"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": "file://" + path, "version": 1, "text": text}}},
        {"jsonrpc": "2.0", "method": "exit"},
    ]
    out, err, rc = sesion(CLI, msgs, timeout=120)
    d = json.loads(out[0])["params"]["diagnostics"]
    return [(x["range"]["start"]["line"] + 1, x["range"]["start"]["character"] + 1, x["message"]) for x in d]

def zett_diags(path):
    r = subprocess.run([ZETT, "check", path], capture_output=True, text=True, timeout=120)
    if r.returncode == 0:
        return []
    lines = (r.stdout + r.stderr).splitlines()
    out = []
    if lines and lines[0].startswith("CHECK FAILED: "):
        lines[0] = lines[0][len("CHECK FAILED: "):]
        if not re.match(r"(?:parse|lexical) errors in ", lines[0]):
            lines = lines[1:]  # la primera línea junta todos los errores; las siguientes los repiten uno por línea
    for l in lines:
        m = re.match(re.escape(path) + r":(\d+):(\d+): (.*)$", l)
        if m:
            out.append((int(m.group(1)), int(m.group(2)), m.group(3)))
            continue
        m = re.match(r"(?:parse|lexical) errors in .*?: (.*)$", l)
        if m:
            out.append(m.group(1))
    return out

archivos = sorted(glob.glob(RAIZ + "/examples/*.titan") + glob.glob(RAIZ + "/selfhost/tests/*/*.titan") + glob.glob(RAIZ + "/selfhost/tests/*.titan"))
total = igual = 0
con_error = 0
difs = []
for p in archivos:
    text = open(p, encoding="utf-8").read()
    if "/tests/pkg/" in p or "/tests/lsp/" in p:
        continue  # usan nativas que `zett` no tiene
    if re.search(r"^\s*import\s", text, re.M) or re.search(r"^\s*mod\s", text, re.M):
        continue
    z = zett_diags(p)
    l = lsp_diags(p, text)
    total += 1
    if z:
        con_error += 1
    # Errores léxicos/de parseo: `check` los junta en una línea separados por espacios; el LSP los da uno a uno.
    if z and isinstance(z[0], str):
        ok = len(l) >= 1 and " ".join(m for _, _, m in l) == z[0]
    else:
        ok = l == z
    if ok:
        igual += 1
    else:
        difs.append((p, z, l))
print("archivos comparados: %d (con errores: %d); idénticos: %d" % (total, con_error, igual))
for p, z, l in difs[:10]:
    print("DIFERENCIA", p, "\n  zett:", z[:3], "\n  lsp: ", l[:3])
sys.exit(0 if not difs else 1)
