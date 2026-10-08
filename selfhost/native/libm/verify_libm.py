#!/usr/bin/env python3
# Compara std::math (exp, ln, sin, cos, tan, pow, log) de un ejecutable Titan
# con la libm del sistema (la que usa la VM), bit a bit.
# Uso: zett run selfhost/build.titan selfhost/native/libm/h.titan /tmp/h
#      python3 selfhost/native/libm/verify_libm.py SEMILLA CASOS /tmp/h
# (el ejecutable puede ser también un script que llame a `zett run h.titan`).
import ctypes, random, math, json, struct, subprocess, sys, tempfile, os
libm = ctypes.CDLL('libm.so.6')
for fn in ['exp','log','sin','cos','tan']:
    getattr(libm, fn).restype = ctypes.c_double; getattr(libm, fn).argtypes = [ctypes.c_double]
libm.pow.restype = ctypes.c_double; libm.pow.argtypes = [ctypes.c_double, ctypes.c_double]
def idiv(a, b):
    if b == 0.0:
        if a == 0.0 or math.isnan(a): return float('nan')
        return math.copysign(math.inf, a) * math.copysign(1.0, b)
    return a / b
def bits(x): return struct.unpack('<Q', struct.pack('<d', x))[0]
rnd = random.Random(int(sys.argv[1])); N = int(sys.argv[2]); exe = sys.argv[3]
INP = os.path.join(tempfile.mkdtemp(), 'in.json')
def mk(x):
    if x == 0: return (0, 0)
    m, e = math.frexp(x); m = int(m * 2**53); e -= 53
    while m % 2 == 0: m //= 2; e += 1
    if e < -1074:  # subnormal: m * 2^e con e >= -1074
        m >>= (-1074 - e); e = -1074
    return (m, e)
def rx(kind):
    r = rnd.random()
    if kind == 'bits':
        while True:
            x = struct.unpack('<d', struct.pack('<Q', rnd.getrandbits(64)))[0]
            if math.isfinite(x): return x
    if kind == 'small': return (r - 0.5) * 2 ** rnd.randint(-60, 2)
    if kind == 'mid': return (r - 0.5) * 2 ** rnd.randint(0, 30)
    if kind == 'big': return (r - 0.5) * 2 ** rnd.randint(26, 1023)
    if kind == 'exp': return (r - 0.5) * 1500
    if kind == 'near1': return 1 + (r - 0.5) * 2 ** rnd.randint(-52, -1)
    if kind == 'int': return float(rnd.randint(-40, 40))
    if kind == 'sub': return rnd.randint(1, 2**20) * 2.0 ** -1074
KINDS = ['bits','small','mid','big','exp','near1','int','sub']
rows, exp = [], []
while len(rows) < N:
    f = rnd.randint(0, 6)
    x = rx(rnd.choice(KINDS)); m, e = mk(x)
    x = math.ldexp(m, e)
    if f == 0: r = libm.exp(x)
    elif f == 1:
        x = abs(x); m, e = mk(x); x = math.ldexp(m, e); r = libm.log(x)
        if not math.isfinite(r): continue
    elif f == 2: r = libm.sin(x)
    elif f == 3: r = libm.cos(x)
    elif f == 4:
        r = libm.tan(x)
        if not math.isfinite(r): continue
    else:
        y = rx(rnd.choice(KINDS)); m2, e2 = mk(y); y = math.ldexp(m2, e2)
        if f == 5:
            if rnd.random() < 0.5: x = abs(x); m, e = mk(x)
            r = libm.pow(x, y)
            if not math.isfinite(r): continue
        else:
            r = idiv(libm.log(x), libm.log(y))
        rows.append([f, m, e, m2, e2]); exp.append(r); continue
    rows.append([f, m, e]); exp.append(r)
json.dump(rows, open(INP, 'w'))
out = subprocess.run([exe, INP], capture_output=True, text=True)
lines = out.stdout.split('\n')
bad = 0
names = ['exp','ln','sin','cos','tan','pow','log']
for i, r in enumerate(exp):
    got = float(lines[i]) if i < len(lines) and lines[i] else None
    ok = got is not None and (bits(got) == bits(r) or (math.isnan(got) and math.isnan(r)))
    if not ok:
        bad += 1
        if bad <= 8: print('DIFF', names[rows[i][0]], rows[i], 'libm', r.hex() if math.isfinite(r) else r, 'titan', lines[i] if i < len(lines) else None)
print('casos', N, 'distintos', bad, out.stderr[-300:])
sys.exit(1 if bad else 0)
