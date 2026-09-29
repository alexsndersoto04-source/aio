#!/usr/bin/env python3
"""Fuzzer diferencial del COMPRESOR de std::compress.

  python3 fuzz_enc.py DIR SEMILLA N [BIN_NATIVO]

Genera N casos "G|Z|D NIVEL HEX", los pasa por la VM de Rust (zett run
prueba.titan) y, si se da, por el binario nativo compilado desde Titan.
Compara todo contra el modelo deflate_ref.py (port literal de miniz_oxide).
"""
import os
import random
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, AQUI)
import deflate_ref as R  # noqa: E402

R.tablas(os.environ.get("CORE_RS", "/tmp/cr/miniz_oxide-0.8.9/src/deflate/core.rs"))


def datos(rng):
    tam = rng.choice([0, 1, 2, 3, 4, 5, 8, 20, 47, 48, 49, 100, 257, 258, 259, 1000, 4095,
                      4096, 4097, rng.randrange(0, 3000), rng.randrange(0, 20000),
                      rng.randrange(30000, 70000), rng.randrange(60000, 140000)])
    tipo = rng.randrange(7)
    if tipo == 0:
        return bytes(rng.randrange(256) for _ in range(tam))
    if tipo == 1:
        alf = bytes(rng.sample(range(256), rng.randrange(1, 6)))
        return bytes(rng.choice(alf) for _ in range(tam))
    if tipo == 2:
        return bytes([rng.randrange(256)]) * tam
    if tipo == 3:
        pal = [bytes(rng.randrange(97, 123) for _ in range(rng.randrange(1, 9)))
               for _ in range(rng.randrange(1, 40))]
        out = bytearray()
        while len(out) < tam:
            out += rng.choice(pal) + b" "
        return bytes(out[:tam])
    if tipo == 4:
        # repeticiones lejanas: bloques copiados desde muy atrás
        out = bytearray(rng.randrange(256) for _ in range(min(tam, 500)))
        while len(out) < tam:
            if rng.random() < 0.5 and len(out) > 10:
                a = rng.randrange(len(out))
                n = rng.randrange(3, 300)
                out += out[a:a + n]
            else:
                out += bytes(rng.randrange(256) for _ in range(rng.randrange(1, 40)))
        return bytes(out[:tam])
    if tipo == 5:
        # mezcla de corridas y basura
        out = bytearray()
        while len(out) < tam:
            if rng.random() < 0.3:
                out += bytes([rng.randrange(256)]) * rng.randrange(1, 600)
            else:
                out += bytes(rng.randrange(4) for _ in range(rng.randrange(1, 60)))
        return bytes(out[:tam])
    # periodo fijo (distancias exactas, patrones que envuelven el diccionario)
    per = rng.choice([1, 2, 3, 7, 255, 256, 257, 4096, 8191, 8192, 8193, 32767, 32768])
    base = bytes(rng.randrange(256) for _ in range(per))
    return (base * (tam // max(per, 1) + 1))[:tam]


def main():
    d, semilla, n = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    nativo = sys.argv[4] if len(sys.argv) > 4 else None
    os.makedirs(d, exist_ok=True)
    rng = random.Random(semilla)
    casos = []
    for _ in range(n):
        modo = rng.choice("GZD")
        nivel = rng.randrange(10)
        casos.append((modo, nivel, datos(rng)))
    ruta = os.path.join(d, "casos_enc_%d.txt" % semilla)
    with open(ruta, "w") as f:
        for m, lv, b in casos:
            f.write("%s %d %s\n" % (m, lv, b.hex()))
    esperado = []
    for m, lv, b in casos:
        fmt = {"G": "gzip", "Z": "zlib", "D": "deflate"}[m]
        esperado.append("ok " + R.encode(b, lv, fmt).hex())
    salidas = {}
    env = dict(os.environ)
    env.pop("GITHUB_ACTIONS", None)
    zett = os.environ.get("ZETT", os.path.expanduser("~/.local/bin/zett"))
    salidas["vm"] = subprocess.run([zett, "run", os.path.join(AQUI, "prueba.titan"), ruta],
                                   capture_output=True, text=True, env=env).stdout.splitlines()
    if nativo:
        salidas["nativo"] = subprocess.run([nativo, ruta], capture_output=True,
                                           text=True).stdout.splitlines()
    fallos = 0
    for nombre, lineas in salidas.items():
        if len(lineas) != len(casos):
            print("%s: %d lineas, esperaba %d" % (nombre, len(lineas), len(casos)))
            fallos += 1
        for i, (m, lv, b) in enumerate(casos):
            if i < len(lineas) and lineas[i] != esperado[i]:
                fallos += 1
                if fallos <= 5:
                    print("DIFERENCIA %s caso %d: %s nivel %d tam %d" % (nombre, i, m, lv, len(b)))
                    print("  modelo:", esperado[i][:160])
                    print("  %s:" % nombre, lineas[i][:160])
    tam = sum(len(b) for _, _, b in casos)
    print("semilla %d: %d casos, %d bytes, %d fallos" % (semilla, len(casos), tam, fallos))
    sys.exit(1 if fallos else 0)


if __name__ == "__main__":
    main()
