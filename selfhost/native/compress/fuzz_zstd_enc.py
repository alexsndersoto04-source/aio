#!/usr/bin/env python3
"""Prueba diferencial de std::compress::zstd_encode.

Genera datos variados, los comprime con el binario del arnés (prueba.titan
compilado, líneas "S NIVEL HEX") y compara byte a byte con libzstd 1.5.7
(el paquete zstandard de Python trae esa misma versión) usada igual que
zstd::stream::encode_all: compressobj() -> compress(todo) (ZSTD_e_continue)
y flush() (ZSTD_e_end). Además comprueba que la salida se descomprime.

uso: fuzz_zstd_enc.py BINARIO SEMILLA CASOS [NIVELES] [TAMAÑO_MAX]
     NIVELES: lista separada por comas (por defecto 1..22).
"""
import os
import random
import subprocess
import sys
import tempfile

import zstandard


def ref(data, level):
    co = zstandard.ZstdCompressor(level=level).compressobj()
    return co.compress(data) + co.flush()


WORDS = [b"the", b"quick", b"brown", b"fox", b"jumps", b"over", b"lazy", b"dog",
         b"Titan", b"compress", b"zstd", b"\n", b" ", b"  ", b"0123456789",
         b"lorem", b"ipsum", b"dolor", b"sit", b"amet", b"{", b"}", b"\"key\": "]


def gen_data(rnd, maxsize):
    kind = rnd.randrange(10)
    n = rnd.choice([rnd.randrange(0, 64), rnd.randrange(0, 2048),
                    rnd.randrange(0, 40000), rnd.randrange(0, maxsize + 1)])
    if kind == 0:
        return bytes(rnd.randrange(256) for _ in range(n))
    if kind == 1:
        return bytes([rnd.randrange(256)]) * n
    if kind == 2:
        alpha = bytes(rnd.randrange(256) for _ in range(rnd.randrange(1, 6)))
        return bytes(rnd.choice(alpha) for _ in range(n))
    if kind == 3:
        out = bytearray()
        while len(out) < n:
            out += rnd.choice(WORDS)
        return bytes(out[:n])
    if kind == 4:
        seg = bytes(rnd.randrange(256) for _ in range(rnd.randrange(1, 3000)))
        out = bytearray()
        while len(out) < n:
            out += seg
            if rnd.random() < 0.3:
                out += bytes(rnd.randrange(256) for _ in range(rnd.randrange(1, 50)))
        return bytes(out[:n])
    if kind == 5:
        # mitad texto, mitad aleatorio (para el divisor de bloques)
        a = gen_text(rnd, n // 2)
        return a + os.urandom(n - len(a)) if rnd.random() < 0.5 else os.urandom(n // 2) + gen_text(rnd, n - n // 2)
    if kind == 6:
        return bytes((i * rnd.randrange(1, 7)) & 255 for i in range(n))
    if kind == 7:
        out = bytearray()
        while len(out) < n:
            out += bytes([rnd.randrange(256)]) * rnd.randrange(1, 300)
        return bytes(out[:n])
    if kind == 8:
        return gen_text(rnd, n)
    # datos con distancias largas
    base = gen_text(rnd, min(n, 70000))
    out = bytearray(base)
    while len(out) < n:
        out += os.urandom(rnd.randrange(1, 5000))
        out += base[: rnd.randrange(1, len(base) + 1)] if base else b""
    return bytes(out[:n])


def gen_text(rnd, n):
    out = bytearray()
    while len(out) < n:
        out += rnd.choice(WORDS)
        if rnd.random() < 0.05:
            out += str(rnd.randrange(10 ** 6)).encode()
    return bytes(out[:n])


def main():
    binary = sys.argv[1]
    seed = int(sys.argv[2])
    cases = int(sys.argv[3])
    levels = list(range(1, 23))
    if len(sys.argv) > 4 and sys.argv[4]:
        levels = [int(x) for x in sys.argv[4].split(",")]
    maxsize = int(sys.argv[5]) if len(sys.argv) > 5 else 300000
    rnd = random.Random(seed)
    items = []
    for _ in range(cases):
        data = gen_data(rnd, maxsize)
        items.append((rnd.choice(levels), data))
    with tempfile.NamedTemporaryFile("w", delete=False, suffix=".txt") as f:
        for lv, data in items:
            f.write("S %d %s\n" % (lv, data.hex()))
        path = f.name
    out = subprocess.run([binary, path], capture_output=True, text=True, timeout=36000)
    os.unlink(path)
    lines = out.stdout.splitlines()
    if len(lines) != len(items):
        print("salida incompleta: %d de %d líneas; stderr: %s" % (len(lines), len(items), out.stderr[-2000:]))
        return 1
    bad = 0
    for (lv, data), line in zip(items, lines):
        want = "ok " + ref(data, lv).hex()
        if line != want:
            bad += 1
            if bad <= 5:
                print("DIFERENCIA nivel %d tamaño %d" % (lv, len(data)))
                print("  datos:", data[:64].hex())
                print("  titan:", line[:200])
                print("  zstd :", want[:200])
                with open("/tmp/ze_fallo_%d.bin" % bad, "wb") as g:
                    g.write(data)
        else:
            got = bytes.fromhex(line[3:])
            if zstandard.ZstdDecompressor().decompressobj().decompress(got) != data:
                bad += 1
                print("NO SE DESCOMPRIME nivel %d tamaño %d" % (lv, len(data)))
    print("casos %d, diferencias %d" % (len(items), bad))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
