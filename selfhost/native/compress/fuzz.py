#!/usr/bin/env python3
# Prueba diferencial de la descompresión de std::compress: prueba.titan en la
# VM de Rust (zett run) contra el mismo programa compilado con el backend
# nativo. Uso: python3 fuzz.py NATIVO SEMILLA CANTIDAD
#
# La VM local es la versión de Rust ANTERIOR a la corrección, con dos
# defectos (ver compress_mod.rs): deflate/zlib truncados daban "ok" con lo
# que hubiera salido, y una distancia a antes del principio daba ceros
# inventados. Cada caso tiene que cumplir las dos cosas: VM == modelo con
# los defectos, y nativo == modelo corregido (ref() abajo). Así el modelo
# queda contrastado con el Rust real y las únicas diferencias VM/nativo son
# exactamente los dos defectos.
import os, random, subprocess, sys, zlib

nat, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
rnd = random.Random(seed)
here = os.path.dirname(os.path.abspath(__file__))
ZETT = os.path.expanduser('~/.local/bin/zett')


class Bits:
    def __init__(s): s.b = []
    def put(s, v, n):
        for i in range(n): s.b.append((v >> i) & 1)
    def code(s, c, n):
        for i in range(n - 1, -1, -1): s.b.append((c >> i) & 1)
    def bytes(s):
        b = s.b + [0] * (-len(s.b) % 8)
        return bytes(sum(b[i + j] << j for j in range(8)) for i in range(0, len(b), 8))


def canon(lengths):
    mx = max(lengths + [0])
    cnt = [0] * (mx + 2)
    for l in lengths:
        if l: cnt[l] += 1
    nxt, c = [0] * (mx + 2), 0
    for b in range(1, mx + 1):
        c = (c + cnt[b - 1]) << 1 if b > 1 else 0
        nxt[b] = c
    codes = {}
    for s, l in enumerate(lengths):
        if l:
            codes[s] = (nxt[l], l); nxt[l] += 1
    return codes


def rand_lengths(n, maxlen):
    # Longitudes que forman un código completo, incompleto o de un símbolo.
    kind = rnd.random()
    L = [0] * n
    if kind < 0.15:
        L[rnd.randrange(n)] = rnd.randint(1, maxlen)
        return L
    if kind < 0.25:
        for _ in range(rnd.randint(0, 3)): L[rnd.randrange(n)] = rnd.randint(1, maxlen)
        return L
    # completo: dividir hojas
    leaves = [1] if n < 2 else [1, 1]
    target = rnd.randint(len(leaves), n)
    while len(leaves) < target:
        cand = [i for i, d in enumerate(leaves) if d < maxlen]
        if not cand: break
        i = rnd.choice(cand); d = leaves.pop(i); leaves += [d + 1, d + 1]
    syms = rnd.sample(range(n), len(leaves))
    for s, d in zip(syms, leaves): L[s] = d
    if rnd.random() < 0.1 and n > 1:
        L[rnd.randrange(n)] = rnd.randint(0, maxlen)  # posible código roto
    return L


def dynamic_block(final=1):
    w = Bits(); w.put(final, 1); w.put(2, 2)
    hlit = rnd.randint(257, 286) if rnd.random() < 0.95 else rnd.randint(257, 288)
    hdist = rnd.randint(1, 30) if rnd.random() < 0.95 else rnd.randint(1, 32)
    lit = rand_lengths(hlit, 15)
    if rnd.random() < 0.8: lit[256] = lit[256] or rnd.randint(1, 15)
    dist = rand_lengths(hdist, 15)
    allv = lit + dist
    # codificar longitudes (con 16/17/18 a veces)
    seq, i = [], 0
    while i < len(allv):
        v = allv[i]; run = 1
        while i + run < len(allv) and allv[i + run] == v: run += 1
        if v == 0 and run >= 3 and rnd.random() < 0.8:
            r = min(run, 138); seq.append((18, r - 11, 7) if r >= 11 else (17, r - 3, 3)); i += r
        elif i > 0 and run >= 3 and rnd.random() < 0.6 and allv[i - 1] == v:
            r = min(run, 6); seq.append((16, r - 3, 2)); i += r
        else:
            seq.append((v, 0, 0)); i += 1
    if rnd.random() < 0.05: seq.insert(0, (16, 0, 2))
    used = sorted(set(s for s, _, _ in seq))
    hl = [0] * 19
    if rnd.random() < 0.9:
        hl_used = rand_lengths(len(used), 7)
        if sum(1 for x in hl_used if x) < len(used):
            hl_used = [max(1, x) for x in hl_used]
        for s, l in zip(used, hl_used): hl[s] = l
    else:
        hl = rand_lengths(19, 7)
    ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
    hclen = 19
    while hclen > 4 and hl[ORDER[hclen - 1]] == 0: hclen -= 1
    w.put(hlit - 257, 5); w.put(hdist - 1, 5); w.put(hclen - 4, 4)
    for k in range(hclen): w.put(hl[ORDER[k]], 3)
    hc = canon(hl)
    for s, e, n in seq:
        if s not in hc: break
        w.code(*hc[s]); w.put(e, n)
    lc, dc = canon(lit), canon(dist)
    out = 0
    for _ in range(rnd.randint(0, 40)):
        if rnd.random() < 0.6 or out == 0:
            s = rnd.choice(list(lc) or [0])
            if s in lc and s < 256: w.code(*lc[s]); out += 1
            elif s in lc and s > 256:
                w.code(*lc[s]); w.put(rnd.randint(0, 31), 5)
                d = rnd.choice(list(dc) or [0])
                if d in dc: w.code(*dc[d]); w.put(rnd.randint(0, 8191), 13)
        else:
            s = rnd.choice([x for x in lc if x > 256] or [0])
            if s in lc:
                w.code(*lc[s]); w.put(rnd.randint(0, 31), 5)
                d = rnd.choice(list(dc) or [0])
                if d in dc: w.code(*dc[d]); w.put(rnd.randint(0, 3), 13)
    if 256 in lc: w.code(*lc[256])
    return w.bytes()


def payload():
    k = rnd.random()
    n = rnd.choice([0, 1, 2, 5, 30, 200, 1000, 5000, 40000, 70000])
    if k < 0.3: return bytes(rnd.getrandbits(8) for _ in range(n))
    if k < 0.6: return bytes(rnd.choice(b'ab') for _ in range(n))
    if k < 0.8: return (b'hola mundo ' * (n // 11 + 1))[:n]
    return bytes(rnd.choice([0, 0, 0, 255, 7]) for _ in range(n))


def raw(data):
    lvl = rnd.choice([0, 1, 6, 9])
    strat = rnd.choice([zlib.Z_DEFAULT_STRATEGY, zlib.Z_FIXED, zlib.Z_HUFFMAN_ONLY, zlib.Z_RLE])
    c = zlib.compressobj(lvl, zlib.DEFLATED, -15, 9, strat)
    if rnd.random() < 0.3 and len(data) > 10:
        m = rnd.randrange(len(data))
        return c.compress(data[:m]) + c.flush(zlib.Z_FULL_FLUSH) + c.compress(data[m:]) + c.flush()
    return c.compress(data) + c.flush()


def gz_wrap(body, data):
    flags = rnd.choice([0, 0, 0, 2, 4, 8, 16, 30]) if rnd.random() < 0.9 else rnd.randint(0, 255)
    h = bytearray([31, 139, 8, flags, 0, 0, 0, 0, 0, 255])
    if flags & 4:
        x = bytes(rnd.getrandbits(8) for _ in range(rnd.randint(0, 20))); h += len(x).to_bytes(2, 'little') + x
    if flags & 8: h += b'nombre.txt\0'
    if flags & 16: h += b'comentario\0'
    if flags & 2: h += (zlib.crc32(bytes(h)) & 0xffff).to_bytes(2, 'little')
    return bytes(h) + body + (zlib.crc32(data)).to_bytes(4, 'little') + (len(data) & 0xffffffff).to_bytes(4, 'little')


def z_wrap(body, data):
    lvl = rnd.choice([0x78, 0x78, 0x68, 0x58, 0x08, 0x88])
    b = bytes([lvl, 0])
    f = (31 - (lvl * 256) % 31) % 31
    return bytes([lvl, f]) + body + zlib.adler32(data).to_bytes(4, 'big')


def mutate(b):
    b = bytearray(b)
    for _ in range(rnd.choice([0, 1, 1, 2, 5])):
        op = rnd.random()
        if not b: break
        i = rnd.randrange(len(b))
        if op < 0.4: b[i] ^= 1 << rnd.randrange(8)
        elif op < 0.6: b[i] = rnd.getrandbits(8)
        elif op < 0.75: del b[i:i + rnd.randint(1, 4)]
        elif op < 0.85: b[i:i] = bytes(rnd.getrandbits(8) for _ in range(rnd.randint(1, 3)))
        else: b = b[:i]
    if rnd.random() < 0.1: b += b'basura'
    return bytes(b)


def case():
    mode = rnd.choice('gzd')
    if rnd.random() < 0.35:
        body = dynamic_block()
        if rnd.random() < 0.3: body = dynamic_block(0)[:-1] + raw(b'fin')  # dos bloques (aprox.)
        data = b''
        try: data = zlib.decompressobj(-15).decompress(body)
        except zlib.error: pass
    elif rnd.random() < 0.05:
        body = bytes(rnd.getrandbits(8) for _ in range(rnd.randint(0, 30))); data = b''
    else:
        data = payload(); body = raw(data)
    s = {'g': gz_wrap, 'z': z_wrap, 'd': lambda b, d: b}[mode](body, data)
    if rnd.random() < 0.7: s = mutate(s)
    return mode, s


def zinfo(mode, s):
    wb = {'g': 31, 'z': 15, 'd': -15}[mode]
    d = zlib.decompressobj(wb)
    try:
        d.decompress(s)
        return 'end' if d.eof else 'truncated'
    except zlib.error as e:
        return str(e)


# ---------------------------------------------------------------- modelo
# Modelo de referencia de miniz_oxide + flate2 con dos interruptores:
# fixed=False reproduce la VM anterior (flate2 en modo "wrapping": sin error
# de distancia, lee ceros de un diccionario de 32 KiB; deflate/zlib truncados
# devuelven lo que salió) y fixed=True la versión corregida.
class Stop(Exception):
    def __init__(s, kind): s.kind = kind


def build(lengths, hufflen):
    look = [798] * 1024; tree = [0] * 576
    ts = [0] * 16
    for c in lengths: ts[c] += 1
    nc = [0] * 17; used = 0; total = 0
    for i in range(1, 16):
        used += ts[i]; total = (total + ts[i]) << 1; nc[i + 1] = total
    if total != 65536 and (used > 1 or hufflen): raise Stop('corrupt')
    nxt = -1
    for sym, cs in enumerate(lengths):
        cs &= 15
        if not cs: continue
        cur = nc[cs]; nc[cs] += 1
        code = cur & ((1 << cs) - 1)
        rev = int(format(code, '016b')[::-1], 2) >> (16 - cs)
        if cs <= 10:
            while rev < 1024: look[rev] = (cs << 9) | sym; rev += 1 << cs
            continue
        t = look[rev & 1023]
        if t == 798: look[rev & 1023] = nxt; t = nxt; nxt -= 2
        rev >>= 9
        for _ in range(11, cs):
            rev >>= 1; t -= rev & 1; idx = -t - 1
            if not 0 <= idx < 576: raise Stop('corrupt')
            if tree[idx] == 0: tree[idx] = nxt; t = nxt; nxt -= 2
            else: t = tree[idx]
        rev >>= 1; t -= rev & 1; idx = -t - 1
        if not 0 <= idx < 576: raise Stop('corrupt')
        tree[idx] = sym
    return look, tree


class Inflate:
    def __init__(s, data, pos, fixed):
        s.d, s.pos, s.bb, s.nb, s.out, s.fixed = data, pos, 0, 0, bytearray(), fixed
    def bits(s, k):
        while s.nb < k:
            if s.pos >= len(s.d): raise Stop('eof')
            s.bb |= s.d[s.pos] << s.nb; s.pos += 1; s.nb += 8
        v = s.bb & ((1 << k) - 1); s.bb >>= k; s.nb -= k
        return v
    def sym(s, tab):
        look, tree = tab
        while True:
            e = look[s.bb & 1023]
            if e >= 0:
                l = e >> 9
                if l <= s.nb: s.bb >>= l; s.nb -= l; return e & 511
            elif s.nb > 10:
                cl, v = 10, e
                while True:
                    idx = -v - 1 + ((s.bb >> cl) & 1)
                    v = tree[idx] if 0 <= idx < 576 else 32767
                    cl += 1
                    if v >= 0 or s.nb < cl + 1: break
                if v >= 0: s.bb >>= cl; s.nb -= cl; return v
            if s.pos >= len(s.d): raise Stop('eof')
            s.bb |= s.d[s.pos] << s.nb; s.pos += 1; s.nb += 8
    def run(s, zl):
        if zl:
            cmf, flg = s.bits(8), s.bits(8)
            if (cmf * 256 + flg) % 31 or flg & 32 or cmf & 15 != 8 or cmf >> 4 > 7: raise Stop('corrupt')
        while True:
            h = s.bits(3); bt = (h >> 1) & 3
            if bt == 0:
                s.bits(s.nb & 7)
                b = [s.bits(8) for _ in range(4)]
                ln, nl = b[0] | b[1] << 8, b[2] | b[3] << 8
                if ln != nl ^ 0xffff: raise Stop('corrupt')
                while ln and s.nb: s.out.append(s.bits(8)); ln -= 1
                k = min(ln, len(s.d) - s.pos)
                s.out += s.d[s.pos:s.pos + k]; s.pos += k
                if k < ln: raise Stop('eof')
            elif bt == 3: raise Stop('corrupt')
            else:
                if bt == 1:
                    dt = build([5] * 32, False)
                    lt = build([8] * 144 + [9] * 112 + [7] * 24 + [8] * 8, False)
                else:
                    hlit, hdist, hclen = s.bits(5) + 257, s.bits(5) + 1, s.bits(4) + 4
                    if hlit > 286 or hdist > 30: raise Stop('corrupt')
                    hl = [0] * 19
                    for i in range(hclen): hl[[16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15][i]] = s.bits(3)
                    ht = build(hl, True)
                    lc = [0] * 512; c = 0
                    while c < hlit + hdist:
                        y = s.sym(ht)
                        if y < 16: lc[c] = y; c += 1
                        elif y == 16 and c == 0: raise Stop('corrupt')
                        else:
                            n = s.bits([2, 3, 7][y - 16]) + [3, 3, 11][y - 16]
                            v = lc[c - 1] if y == 16 else 0
                            for j in range(c, c + n): lc[j] = v
                            c += n
                    if c != hlit + hdist: raise Stop('corrupt')
                    dt = build(lc[hlit:hlit + hdist], False)
                    lt = build(lc[:hlit], False)
                while True:
                    y = s.sym(lt)
                    if y < 256: s.out.append(y); continue
                    y &= 511
                    if y == 256: break
                    if y > 285: raise Stop('corrupt')
                    li = y - 257
                    base = 3 + li if li < 8 else 258 if li == 28 else ((4 + (li & 3)) << (li // 4 - 1)) + 3
                    ex = 0 if li < 8 or li == 28 else li // 4 - 1
                    ln = base + (s.bits(ex) if ex else 0)
                    d = s.sym(dt)
                    if d > 29: raise Stop('corrupt')
                    dist = (d + 1 if d < 4 else ((2 + (d & 1)) << (d // 2 - 1)) + 1)
                    de = max(d // 2 - 1, 0)
                    if de: dist += s.bits(de)
                    if dist > len(s.out):
                        if s.fixed: raise Stop('corrupt')
                        for _ in range(ln):
                            p = len(s.out) - dist
                            s.out.append(s.out[p] if p >= 0 else 0)  # diccionario a ceros
                        continue
                    for _ in range(ln): s.out.append(s.out[-dist])
            if h & 1: break
        s.pos -= s.nb >> 3; s.bb = s.nb = 0
        if zl:
            a = 0
            for _ in range(4): a = a * 256 + s.bits(8)
            if a != zlib.adler32(bytes(s.out)): raise Stop('corrupt')


MSG = {'eof': 'unexpected end of file', 'corrupt': 'corrupt deflate stream', 'hdr': 'invalid gzip header',
       'long': 'gzip header field too long', 'crc': 'corrupt gzip stream does not have a matching checksum'}
FN = {'g': 'gzip_decode', 'z': 'zlib_decode', 'd': 'deflate_decode'}


def gz_header(s):
    if len(s) < 10: raise Stop('eof')
    if s[0] != 31 or s[1] != 139 or s[2] != 8 or s[3] & 224: raise Stop('hdr')
    f, p = s[3], 10
    if f & 4:
        if len(s) - p < 2: raise Stop('eof')
        x = s[p] | s[p + 1] << 8; p += 2
        if len(s) - p < x: raise Stop('eof')
        p += x
    for bit in (8, 16):
        if f & bit:
            st = p
            while True:
                if p >= len(s): raise Stop('eof')
                if s[p] == 0: p += 1; break
                if p - st == 65535: raise Stop('long')
                p += 1
    if f & 2:
        if len(s) - p < 2: raise Stop('eof')
        if (s[p] | s[p + 1] << 8) != zlib.crc32(s[:p]) & 0xffff: raise Stop('crc')
        p += 2
    return p


def ref(mode, s, fixed):
    inf = None
    try:
        start = gz_header(s) if mode == 'g' else 0
        inf = Inflate(s, start, fixed)
        inf.run(mode == 'z')
        if mode == 'g':
            p = inf.pos
            if len(s) - p < 8: raise Stop('eof')
            if int.from_bytes(s[p:p + 4], 'little') != zlib.crc32(bytes(inf.out)) or \
               int.from_bytes(s[p + 4:p + 8], 'little') != len(inf.out) & 0xffffffff: raise Stop('crc')
        return 'ok ' + bytes(inf.out).hex()
    except Stop as e:
        if not fixed and mode in 'zd' and e.kind == 'eof' and inf is not None:
            return 'ok ' + bytes(inf.out).hex()  # defecto antiguo: truncado = "ok"
        m = f"native function 'std::compress::{FN[mode]}' failed: compression I/O error: {MSG[e.kind]}"
        return 'ERR ' + m.encode().hex()


cases = [case() for _ in range(count)]
path = f'/tmp/cz_fuzz_{seed}.txt'
with open(path, 'w') as f:
    for m, s in cases: f.write(f'{m} {s.hex()}\n')
vm = subprocess.run([ZETT, 'run', os.path.join(here, 'prueba.titan'), path], capture_output=True, text=True)
na = subprocess.run([nat, path], capture_output=True, text=True)
if vm.returncode or na.returncode:
    print('fallo al ejecutar', vm.returncode, na.returncode, vm.stderr[-500:], na.stderr[-500:]); sys.exit(1)
a, b = vm.stdout.splitlines(), na.stdout.splitlines()
assert len(a) == len(b) == count, (len(a), len(b))
stats = {'igual': 0, 'ok': 0, 'err': 0, 'defecto_corregido': 0}
bad = 0
for (m, s), x, y in zip(cases, a, b):
    old, new = ref(m, s, False), ref(m, s, True)
    if x != old or y != new:
        bad += 1
        if bad <= 8:
            print('DIFERENCIA', m, s.hex()[:300], '\n  VM   :', x[:160], '\n  model:', old[:160],
                  '\n  nat  :', y[:160], '\n  model:', new[:160], '\n  zlib :', zinfo(m, s))
        continue
    if x == y: stats['igual'] += 1; stats['ok' if x.startswith('ok') else 'err'] += 1
    else: stats['defecto_corregido'] += 1
print(stats, 'fuera del modelo:', bad)
sys.exit(1 if bad else 0)
