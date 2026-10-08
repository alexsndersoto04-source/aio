#!/usr/bin/env python3
"""Compara std::qrcode entre la VM y el binario nativo.
  python3 fuzz.py CASOS SEMILLA BINARIO_NATIVO
"""
import os, random, subprocess, sys, tempfile

N = int(sys.argv[1])
SEED = int(sys.argv[2])
BIN = sys.argv[3]
ZETT = os.environ.get("ZETT", "zett")
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
HARNESS = os.path.join(os.path.dirname(__file__), "prueba.titan")
R = random.Random(SEED)

ALPH = [bytes([c]) for c in range(256)]
WORDS = [b"", b"a", b"A", b"hola", b"HOLA", b"12345", b"https://arena.ai",
         b"hola titan", b" ", b"$%*+-./:", b"\x81\x40", b"\xff\xfe",
         "ñandú".encode(), "日本語".encode(), "😀".encode()]

def payload():
    k = R.random()
    if k < 0.2:
        return R.choice(WORDS)
    if k < 0.45:
        n = R.randint(0, 40)
        out = bytearray()
        for _ in range(n):
            t = R.random()
            if t < 0.6:
                out.append(R.randint(0, 127))
            elif t < 0.85:
                out.extend(chr(R.randint(128, 2047)).encode())
            else:
                out.extend(chr(R.randint(2048, 0xD7FF)).encode())
        return bytes(out)
    if k < 0.7:
        n = R.randint(0, 200)
        return bytes(R.choice(b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:") for _ in range(n))
    n = R.randint(0, 120)
    return bytes(R.randint(32, 126) for _ in range(n))

LEVELS = ["L", "M", "Q", "H", "l", "m", "q", "h", "Z", "", "LL", "x"]

def run(cmd, env=None):
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=30, env=env)
    return p.returncode, p.stdout, p.stderr

diff = 0
for i in range(N):
    data = payload()
    lv = R.choice(LEVELS)
    if R.random() < 0.05:
        lv = "".join(R.choice("lmqhLMQH z") for _ in range(R.randint(0, 3)))
    open("/tmp/qr_in.txt", "wb").write(data)
    open("/tmp/qr_lv.txt", "w").write(lv)
    vm = run([ZETT, "run", HARNESS])
    nat = run([BIN])
    if vm[1] != nat[1] or vm[0] != nat[0]:
        diff += 1
        print("DIF", i, "lv", repr(lv), "n", len(data), "data", data[:40])
        print(" VM", vm[0], vm[1][:200])
        print(" NAT", nat[0], nat[1][:200])
        if diff >= 8:
            break
    if (i + 1) % 50 == 0:
        print("...", i + 1, "iguales", i + 1 - diff, flush=True)

print("casos", N, "distintos", diff)
sys.exit(0 if diff == 0 else 1)
