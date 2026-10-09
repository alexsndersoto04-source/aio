#!/usr/bin/env python3
"""Compara bert.titan (nativo y tract bajo `zett`) con la referencia de onnxruntime.
Uso: compare_bert.py <dir con los modelos> <bert_nativo> <zett> <bert.titan>"""
import json
import os
import subprocess
import sys

d, native, zett, src = sys.argv[1:5]
TOL = 2e-5
bad = 0


def run(cmd):
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=1500)
    out = {}
    for line in p.stdout.splitlines():
        k, _, v = line.partition(" ")
        out[k] = v.strip()
    if p.returncode != 0 or "values" not in out:
        raise RuntimeError((p.stdout + p.stderr)[-300:])
    return out


def maxdiff(a, b):
    return max([abs(x - y) for x, y in zip(a, b)] + [0.0])


for name in sorted(f[:-5] for f in os.listdir(d) if f.startswith("bert_") and f.endswith(".json")):
    doc = json.load(open(f"{d}/{name}.json"))
    ref = doc["ref"]
    args = [f"{d}/{name}.onnx", f"{d}/{name}.json"]
    try:
        nat = run([native, *args])
    except RuntimeError as e:
        print(f"{name}: nativo FALLA: {e}")
        bad += 1
        continue
    problems = []
    vals = json.loads(nat["values"])
    if json.loads(nat["shape"]) != ref[0]["shape"]:
        problems.append(f"forma {nat['shape']} != ort {ref[0]['shape']}")
    else:
        e = maxdiff(vals, ref[0]["values"])
        if e > TOL:
            problems.append(f"error vs onnxruntime {e:.2e}")
    # la salida agrupada es la media de last_hidden_state ponderada por la máscara
    if "pvalues" in nat:
        b, s, h = ref[0]["shape"]
        mask = doc["inputs"][1]["data"]
        pooled = []
        for bi in range(b):
            cnt = sum(1 for si in range(s) if mask[bi * s + si] != 0)
            for hi in range(h):
                acc = sum(ref[0]["values"][(bi * s + si) * h + hi] for si in range(s) if mask[bi * s + si] != 0)
                pooled.append(acc / cnt)
        e = maxdiff(json.loads(nat["pvalues"]), pooled)
        if e > TOL:
            problems.append(f"agrupada vs onnxruntime {e:.2e}")
    info = ""
    try:
        tr = run([zett, "run", src, *args])
    except RuntimeError as e:
        tr = None
        info = " (tract no lo carga: " + str(e).strip().splitlines()[-1][:70] + ")"
    if tr is not None:
        for k in ("inputs", "outputs", "in0", "in1", "out0", "out1", "shape", "pshape"):
            if nat.get(k) != tr.get(k):
                problems.append(f"{k}: nativo {nat.get(k)} != tract {tr.get(k)}")
        for k in ("values", "pvalues"):
            if k in nat:
                e2 = maxdiff(json.loads(nat[k]), json.loads(tr[k]))
                if e2 > TOL:
                    problems.append(f"{k} vs tract {e2:.2e}")
        info = f" vs tract {maxdiff(vals, json.loads(tr['values'])):.1e}"
    if problems:
        bad += 1
        print(f"{name}: FALLA: {'; '.join(problems)}")
    else:
        print(f"{name}: OK vs onnxruntime {maxdiff(vals, ref[0]['values']):.1e}{info}")
sys.exit(1 if bad else 0)
