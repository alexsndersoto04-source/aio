#!/usr/bin/env python3
"""Genera hash_consts.titan: constantes de SHA-2 y Keccak calculadas desde su
definición matemática (raíces de primos y el LFSR de Keccak), en decimal con
signo de 64 bits porque Titan no tiene literales hexadecimales."""
from math import isqrt

def primes(n):
    out, k = [], 2
    while len(out) < n:
        if all(k % p for p in out if p * p <= k):
            out.append(k)
        k += 1
    return out

def icbrt(x):
    r = int(round(x ** (1 / 3))) if x < 2**1000 else 1 << (x.bit_length() // 3 + 1)
    while True:
        s = (2 * r + x // (r * r)) // 3
        if s >= r:
            break
        r = s
    while r ** 3 > x: r -= 1
    while (r + 1) ** 3 <= x: r += 1
    return r

def signed(v):
    return v - (1 << 64) if v >= 1 << 63 else v

P = primes(80)
k512 = [icbrt(p << 192) & (2**64 - 1) for p in P]
k256 = [v >> 32 for v in k512[:64]]
iv512 = [isqrt(p << 128) & (2**64 - 1) for p in P[:8]]
iv256 = [v >> 32 for v in iv512]
iv384 = [isqrt(p << 128) & (2**64 - 1) for p in primes(16)[8:16]]

def rc_bit(t):
    if t % 255 == 0: return 1
    r = 1
    for _ in range(t % 255):
        r <<= 1
        if r & 0x100: r ^= 0x171
    return r & 1

rc = []
for i in range(24):
    v = 0
    for j in range(7):
        if rc_bit(j + 7 * i): v |= 1 << ((1 << j) - 1)
    rc.append(v)
rot = [0] * 25
x, y = 1, 0
for t in range(24):
    rot[x + 5 * y] = ((t + 1) * (t + 2) // 2) % 64
    x, y = y, (2 * x + 3 * y) % 5

assert k256[0] == 0x428a2f98 and iv512[0] == 0x6a09e667f3bcc908 and iv384[0] == 0xcbbb9d5dc1059ed8
assert rc[1] == 0x8082 and rc[23] == 0x8000000080008008 and rot[1] == 1 and rot[24] == 14

def arr(name, vals):
    body = ",\n".join(f"        {signed(v)}" for v in vals)
    hexes = " ".join(f"{v:x}" for v in vals[:2])
    return f"// {name}: {len(vals)} valores (empieza por 0x{hexes}...)\nfn {name}() -> array {{\n    [\n{body},\n    ]\n}}\n"

with open("hash_consts.titan", "w") as f:
    f.write("// GENERADO por gen_hash_consts.py; no editar a mano.\n\n")
    for n, v in [("hc_k256", k256), ("hc_k512", k512), ("hc_iv256", iv256), ("hc_iv384", iv384), ("hc_iv512", iv512), ("hc_keccak_rc", rc), ("hc_keccak_rot", rot)]:
        f.write(arr(n, v) + "\n")
