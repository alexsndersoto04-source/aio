#!/usr/bin/env python3
# Genera selfhost/native/std_libm.titan y std_libm_tab.titan.
#
# - std_libm.titan sale de std_libm.titan.in cambiando cada @...@ por los bits
#   (int64 con signo) de la constante: @u:EXPR@ es un entero de 64 bits
#   (expresión de Python), @inf@/@-inf@, y cualquier otra cosa es un float
#   de C (hexadecimal 0x1.8p52 o decimal), redondeado como lo hace C.
# - std_libm_tab.titan tiene las tablas de glibc 2.36, leídas de la libm.so.6
#   del sistema (la misma que usa la VM de Rust). Cada tabla se busca por su
#   contenido (no por posición) y se comprueba antes de usarla.
#
# Uso: python3 selfhost/native/libm/gen_libm.py [ruta/a/libm.so.6]
import math
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.dirname(HERE)
LIBM = sys.argv[1] if len(sys.argv) > 1 else '/lib/x86_64-linux-gnu/libm.so.6'
d = open(LIBM, 'rb').read()


def signed(u):
    u &= (1 << 64) - 1
    return u - (1 << 64) if u >= (1 << 63) else u


def lit(v):
    if v == -(1 << 63):
        return '(-9223372036854775807 - 1)'
    return str(v)


def fbits(x):
    return signed(struct.unpack('<Q', struct.pack('<d', x))[0])


def find_all(pat):
    r, i = [], d.find(pat)
    while i >= 0:
        r.append(i)
        i = d.find(pat, i + 1)
    return r


D = lambda x: struct.pack('<d', x)
f64 = lambda o, n: list(struct.unpack_from('<%dd' % n, d, o))
u64 = lambda o, n: list(struct.unpack_from('<%dQ' % n, d, o))
dd = lambda o: struct.unpack_from('<d', d, o)[0]

# __exp_data: invln2N, negln2hiN, negln2loN, poly[4], shift, exp2_shift,
# exp2_poly[5], tab[256].
e = find_all(D(float.fromhex('0x1.71547652b82fep7')) + D(float.fromhex('-0x1.62e42fefa0000p-8')))
assert len(e) == 1, e
exp_tab = u64(e[0] + 14 * 8, 256)
assert exp_tab[0] == 0 and exp_tab[1] == 0x3ff0000000000000
assert dd(e[0] + 7 * 8) == float.fromhex('0x1.8p52')

# __log_data y __pow_log_data empiezan con ln2hi, ln2lo; el tercer valor
# distingue: log -0x1.0000000000001p-1, pow -0.5.
ln2 = D(float.fromhex('0x1.62e42fefa3800p-1')) + D(float.fromhex('0x1.ef35793c76730p-45'))
L = P = None
for o in find_all(ln2):
    if dd(o + 16) == float.fromhex('-0x1.0000000000001p-1'):
        L = o
    elif dd(o + 16) == -0.5:
        P = o
assert L is not None and P is not None
log_tab = u64(L + 18 * 8, 256)
pow_raw = f64(P + 9 * 8, 512)
pow_tab = []
for i in range(128):
    invc, pad, logc, logctail = pow_raw[4 * i:4 * i + 4]
    assert pad == 0.0 and abs(logc + math.log(invc)) < 1e-4
    pow_tab += [fbits(invc), fbits(logc), fbits(logctail)]
for i in range(128):
    invc = struct.unpack('<d', struct.pack('<Q', log_tab[2 * i]))[0]
    logc = struct.unpack('<d', struct.pack('<Q', log_tab[2 * i + 1]))[0]
    assert abs(logc + math.log(invc)) < 1e-15, i

# utan.tbl xfg[186][4]: x, tan(x), cot(x), cola; hay una copia por variante
# (todas iguales).
xf = []
for al in range(8):
    for o in range(al, len(d) - 64, 8):
        a = dd(o)
        if 0.0625 <= a < 0.06251 and abs(dd(o + 8) - math.tan(a)) < 1e-15 and abs(dd(o + 16) - 1 / math.tan(a)) < 1e-12:
            xf.append(o)
xf = sorted(set(xf))
assert xf, 'xfg'
copies = [d[o:o + 186 * 32] for o in xf]
assert all(c == copies[0] for c in copies)
xfg = u64(xf[0], 744)
for i in range(186):
    x, fi, gi = [struct.unpack('<d', struct.pack('<Q', v))[0] for v in xfg[4 * i:4 * i + 3]]
    assert abs(fi - math.tan(x)) < 1e-15 and abs(gi - 1 / math.tan(x)) < 1e-13, i

# __sincostab: por cada i = 0..109, sin(i/128), cola, cos(i/128), cola.
sc = []
for al in range(8):
    for o in range(al, len(d) - 64, 8):
        if dd(o) == 0.0 and dd(o + 8) == 0.0 and dd(o + 16) == 1.0 and dd(o + 24) == 0.0 \
                and abs(dd(o + 32) - math.sin(1 / 128)) < 1e-16 and abs(dd(o + 48) - math.cos(1 / 128)) < 1e-16:
            sc.append(o)
assert len(sc) == 1, sc
sct = u64(sc[0], 440)
for i in range(110):
    s, _, c, _ = [struct.unpack('<d', struct.pack('<Q', v))[0] for v in sct[4 * i:4 * i + 4]]
    assert abs(s - math.sin(i / 128)) < 1e-15 and abs(c - math.cos(i / 128)) < 1e-15, i

allv = exp_tab + log_tab + [v & ((1 << 64) - 1) for v in pow_tab] + xfg + sct
assert len(allv) == 2080
hexs = ''.join('%016x' % v for v in allv)
lines = [hexs[i:i + 96] for i in range(0, len(hexs), 96)]
with open(os.path.join(OUT, 'std_libm_tab.titan'), 'w') as f:
    f.write('// GENERADO por selfhost/native/libm/gen_libm.py a partir de la libm.so.6\n')
    f.write('// de glibc 2.36 (las mismas tablas que usa la VM). No editar a mano.\n')
    f.write('// 2080 valores de 64 bits en hexadecimal; ver lm_tab() en std_libm.titan.\n')
    f.write('fn lm_tables_hex() -> string {\n    return "' + lines[0] + '" +\n')
    for ln in lines[1:-1]:
        f.write('        "' + ln + '" +\n')
    f.write('        "' + lines[-1] + '"\n}\n')


def conv(m):
    t = m.group(1)
    if t.startswith('u:'):
        return lit(signed(eval(t[2:])))
    if t == 'inf':
        return lit(fbits(math.inf))
    if t == '-inf':
        return lit(fbits(-math.inf))
    x = float.fromhex(t) if 'x' in t.lower() else float(t)
    return lit(fbits(x))


src = open(os.path.join(HERE, 'std_libm.titan.in')).read()
out = re.sub(r'@([^@\n]+)@', conv, src)
assert '@' not in out
open(os.path.join(OUT, 'std_libm.titan'), 'w').write(out)
print('std_libm.titan y std_libm_tab.titan generados')
