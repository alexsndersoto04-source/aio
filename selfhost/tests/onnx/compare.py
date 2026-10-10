#!/usr/bin/env python3
"""Compara, para cada modelo de <dir>, la salida de std::onnx del ejecutable
nativo (motor propio) con la de onnxruntime (referencia de gen_models.py) y con
la de tract (api.titan bajo `zett`). Uso: compare.py <dir> <api_nativo> <zett>"""
import json
import os
import subprocess
import sys

d, native, zett = sys.argv[1:4]
here = os.path.dirname(os.path.abspath(__file__))
TOL = 1e-5
bad = 0


def run(cmd):
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    lines = [l for l in p.stdout.strip().splitlines() if l.startswith("{")]
    if p.returncode != 0 or not lines:
        raise RuntimeError((p.stdout + p.stderr)[-400:])
    return json.loads(lines[-1])


def rel(a, b):
    return max([abs(x - y) / max(1.0, abs(y)) for x, y in zip(a, b)] + [0.0])


for name in sorted(f[:-5] for f in os.listdir(d) if f.endswith(".json")):
    ref = json.load(open(f"{d}/{name}.json"))["ref"]
    for pin in (False, True):
        tag = f"{name}{' [load_shape]' if pin else ''}"
        extra = ["pin"] if pin else []
        try:
            nat = run([native, *extra, f"{d}/{name}"])
        except RuntimeError as e:
            print(f"{tag}: nativo FALLA: {e}")
            bad += 1
            continue
        try:
            tr = run([zett, "run", f"{here}/api.titan", *extra, f"{d}/{name}"])
        except RuntimeError as e:
            tr = None  # tract no soporta ese modelo (p. ej. opset antiguo)
            tract_err = str(e).strip().splitlines()[-1][:80]
        problems = []
        if nat["shape"] != ref["shape"]:
            problems.append(f"forma {nat['shape']} != ort {ref['shape']}")
        else:
            e = rel(nat["values"], ref["values"])
            if e > TOL:
                problems.append(f"error vs onnxruntime {e:.2e}")
        info = ""
        if tr is not None:
            for k in ("inputs", "outputs", "in_shape", "out_shape", "shape"):
                if nat[k] != tr[k]:
                    problems.append(f"{k}: nativo {nat[k]} != tract {tr[k]}")
            e2 = rel(nat["values"], tr["values"])
            if e2 > TOL:
                problems.append(f"error vs tract {e2:.2e}")
            info = f" vs tract {e2:.1e}"
        else:
            info = f" (tract no lo ejecuta: {tract_err})"
        e1 = rel(nat["values"], ref["values"]) if nat["shape"] == ref["shape"] else float("nan")
        if problems:
            bad += 1
            print(f"{tag}: FALLA " + "; ".join(problems))
        else:
            print(f"{tag}: OK vs onnxruntime {e1:.1e}{info}")
sys.exit(1 if bad else 0)
