#!/usr/bin/env python3
"""Modelo de referencia del compresor DEFLATE de miniz_oxide 0.8.9 (el que usa
flate2 1.1.9 dentro de la VM de Rust), portado de forma literal desde
src/deflate/{core,stored,zlib}.rs.  Sirve para validar byte a byte el
compresor escrito en Titan (std_compress.titan).

Las tablas (LEN_SYM, SMALL_DIST_SYM, ...) se leen directamente del core.rs
original para no teclear nada a mano:  tablas(ruta_core_rs).

Uso como biblioteca:  encode(datos: bytes, nivel: int, formato: 'gzip'|'zlib'|'deflate')
"""
import re
import zlib as _zlib

M16 = 0xFFFF
DICT = 32768
MASK = DICT - 1
FULL = DICT + 258
MAXM = 258
NUM_PROBES = [0, 1, 6, 32, 16, 32, 128, 256, 512, 768, 1500]
ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
GREEDY = 0x4000
RAW = 0x80000
ZHDR = 0x1000
T = {}


def tablas(core_rs):
    src = open(core_rs).read()
    for nombre in ["LEN_SYM", "LEN_EXTRA", "SMALL_DIST_SYM", "SMALL_DIST_EXTRA",
                   "LARGE_DIST_SYM", "LARGE_DIST_EXTRA"]:
        m = re.search(r"const " + nombre + r": \[u8; (\d+)\] = \[(.*?)\];", src, re.S)
        cuerpo = re.sub(r"//[^\n]*", "", m.group(2))
        vals = [int(x, 0) for x in re.findall(r"0x[0-9a-fA-F]+|\d+", cuerpo)]
        assert len(vals) == int(m.group(1)), nombre
        T[nombre] = vals


def flags_de_nivel(nivel, zlib_hdr):
    f = NUM_PROBES[min(10, nivel)] | (GREEDY if nivel <= 3 else 0)
    if zlib_hdr:
        f |= ZHDR
    if nivel == 0:
        f |= RAW
    return f


class Out:
    def __init__(s):
        s.b = bytearray()
        s.bb = 0
        s.bi = 0

    def put(s, bits, n):
        assert bits < (1 << n) or n == 0 and bits == 0
        s.bb |= bits << s.bi
        s.bi += n
        while s.bi >= 8:
            s.b.append(s.bb & 255)
            s.bb >>= 8
            s.bi -= 8

    def save(s):
        return (len(s.b), s.bb, s.bi)

    def load(s, sv):
        del s.b[sv[0]:]
        s.bb, s.bi = sv[1], sv[2]

    def pad(s):
        if s.bi:
            s.put(0, 8 - s.bi)


class Huff:
    def __init__(s):
        s.count = [[0] * 288 for _ in range(3)]
        s.codes = [[0] * 288 for _ in range(3)]
        s.sizes = [[0] * 288 for _ in range(3)]

    @staticmethod
    def radix(syms):
        # syms: lista de [key, idx]
        n = len(syms)
        hist1 = [0] * 256
        for k, _ in syms:
            hist1[(k >> 8) & 255] += 1
        passes = 1 if n == hist1[0] else 2
        cur = syms
        for p in range(passes):
            h = [0] * 256
            for k, _ in cur:
                h[(k >> (8 * p)) & 255] += 1
            off = [0] * 256
            o = 0
            for i in range(256):
                off[i] = o
                o += h[i]
            nue = [None] * n
            for sym in cur:
                j = (sym[0] >> (8 * p)) & 255
                nue[off[j]] = sym
                off[j] += 1
            cur = nue
        return [list(x) for x in cur]

    @staticmethod
    def min_red(A):
        n = len(A)
        if n == 0:
            return
        if n == 1:
            A[0][0] = 1
            return
        A[0][0] = (A[0][0] + A[1][0]) & M16
        root = 0
        leaf = 2
        for nx in range(1, n - 1):
            if leaf >= n or A[root][0] < A[leaf][0]:
                A[nx][0] = A[root][0]
                A[root][0] = nx
                root += 1
            else:
                A[nx][0] = A[leaf][0]
                leaf += 1
            if leaf >= n or (root < nx and A[root][0] < A[leaf][0]):
                A[nx][0] = (A[nx][0] + A[root][0]) & M16
                A[root][0] = nx
                root += 1
            else:
                A[nx][0] = (A[nx][0] + A[leaf][0]) & M16
                leaf += 1
        A[n - 2][0] = 0
        for nx in range(n - 3, -1, -1):
            A[nx][0] = (A[A[nx][0]][0] + 1) & M16
        avbl = 1
        used = 0
        dpth = 0
        root = n - 2
        nx = n - 1
        while avbl > 0:
            while root >= 0 and A[root][0] == dpth:
                used += 1
                root -= 1
            while avbl > used:
                A[nx][0] = dpth
                nx -= 1
                avbl -= 1
            avbl = 2 * used
            dpth += 1
            used = 0

    @staticmethod
    def enforce(nc, ln, mx):
        if ln <= 1:
            return
        nc[mx] += sum(nc[mx + 1:])
        total = 0
        for L in range(1, mx + 1):
            total += nc[L] << (mx - L)
        for _ in range(1 << mx, total):
            nc[mx] -= 1
            for i in range(mx - 1, 0, -1):
                if nc[i] != 0:
                    nc[i] -= 1
                    nc[i + 1] += 2
                    break

    def optimize(s, t, tlen, limit, static):
        nc = [0] * 33
        if static:
            for cs in s.sizes[t][:tlen]:
                nc[cs] += 1
        else:
            syms = [[s.count[t][i], i] for i in range(tlen) if s.count[t][i] != 0]
            used = len(syms)
            syms = s.radix(syms)
            s.min_red(syms)
            for k, _ in syms:
                nc[k] += 1
            s.enforce(nc, used, limit)
            s.sizes[t] = [0] * 288
            s.codes[t] = [0] * 288
            last = used
            for i in range(1, limit + 1):
                first = last - nc[i]
                for k, idx in syms[first:last]:
                    s.sizes[t][idx] = i
                last = first
        nxt = [0] * 16
        j = 0
        for i in range(2, limit + 1):
            j = (j + nc[i - 1]) << 1
            nxt[i] = j
        for i in range(tlen):
            cs = s.sizes[t][i]
            if cs == 0:
                continue
            code = nxt[cs] & M16
            nxt[cs] += 1
            rev = int(format(code, "016b")[::-1], 2) >> (16 - cs)
            s.codes[t][i] = rev

    def start_static(s, o):
        s.sizes[0][0:144] = [8] * 144
        s.sizes[0][144:256] = [9] * 112
        s.sizes[0][256:280] = [7] * 24
        s.sizes[0][280:288] = [8] * 8
        s.sizes[1][0:32] = [5] * 32
        s.optimize(0, 288, 15, True)
        s.optimize(1, 32, 15, True)
        o.put(1, 2)

    def start_dynamic(s, o):
        s.count[0][256] = 1
        s.optimize(0, 288, 15, False)
        s.optimize(1, 32, 15, False)
        nl = 286
        for x in reversed(s.sizes[0][257:286]):
            if x != 0:
                break
            nl -= 1
        nd = 30
        for x in reversed(s.sizes[1][1:30]):
            if x != 0:
                break
            nd -= 1
        pack = s.sizes[0][:nl] + s.sizes[1][:nd]
        packed = []
        c2 = s.count[2]
        for i in range(19):
            c2[i] = 0
        st = {"z": 0, "r": 0, "p": 0xFF}

        def prev():
            if st["r"] != 0:
                if st["r"] < 3:
                    c2[st["p"]] = (c2[st["p"]] + st["r"]) & M16
                    packed.extend([st["p"]] * st["r"])
                else:
                    c2[16] = (c2[16] + 1) & M16
                    packed.extend([16, st["r"] - 3])
                st["r"] = 0

        def zero():
            if st["z"] != 0:
                if st["z"] < 3:
                    c2[0] = (c2[0] + st["z"]) & M16
                    packed.extend([0] * st["z"])
                elif st["z"] <= 10:
                    c2[17] = (c2[17] + 1) & M16
                    packed.extend([17, st["z"] - 3])
                else:
                    c2[18] = (c2[18] + 1) & M16
                    packed.extend([18, st["z"] - 11])
                st["z"] = 0

        for cs in pack:
            if cs == 0:
                prev()
                st["z"] += 1
                if st["z"] == 138:
                    zero()
            else:
                zero()
                if cs != st["p"]:
                    prev()
                    c2[cs] = (c2[cs] + 1) & M16
                    packed.append(cs)
                else:
                    st["r"] += 1
                    if st["r"] == 6:
                        prev()
            st["p"] = cs
        if st["r"] != 0:
            prev()
        else:
            zero()
        s.optimize(2, 19, 7, False)
        o.put(2, 2)
        o.put(nl - 257, 5)
        o.put(nd - 1, 5)
        nb = 18
        for sw in reversed(ORDER):
            if s.sizes[2][sw] != 0:
                break
            nb -= 1
        nb = max(4, nb + 1)
        o.put(nb - 4, 4)
        for sw in ORDER[:nb]:
            o.put(s.sizes[2][sw], 3)
        i = 0
        while i < len(packed):
            c = packed[i]
            i += 1
            o.put(s.codes[2][c], s.sizes[2][c])
            if c >= 16:
                o.put(packed[i], [2, 3, 7][c - 16])
                i += 1


class Comp:
    def __init__(s, nivel, zlib_hdr):
        s.flags = flags_de_nivel(nivel, zlib_hdr)
        p = s.flags & 0xFFF
        s.probes = [1 + (p + 2) // 3, 1 + ((p >> 2) + 2) // 3]
        s.greedy = s.flags & GREEDY != 0
        s.dict = bytearray(FULL)
        s.hash = [0] * 32768
        s.next = [0] * 32768
        s.cbdp = 0          # code_buf_dict_pos
        s.la_size = 0
        s.la_pos = 0
        s.size = 0
        s.h = Huff()
        s.recs = []         # registros del bloque: ('L', b) | ('M', len-3, dist-1)
        s.code_pos = 1
        s.nflags = 8
        s.total = 0
        s.block_index = 0
        s.saved_lit = 0
        s.saved_dist = 0
        s.saved_len = 0
        s.flush = 0         # 0 None, 2 Finish
        s.o = Out()
        # llamada actual de compress(): tamaño del búfer de salida y cuánto lleva
        s.out_len = 0
        s.out_ofs = 0
        s.flush_remaining = 0
        s.finished = False
        s.done = False
        s.src_pos = 0

    # --- LZ ---
    def consume(s):
        s.nflags -= 1
        if s.nflags == 0:
            s.nflags = 8
            s.code_pos += 1

    def lit(s, b):
        s.total += 1
        s.code_pos += 1
        s.recs.append(("L", b))
        s.consume()
        s.h.count[0][b] = (s.h.count[0][b] + 1) & M16

    def match(s, ln, dist):
        s.total += ln
        dist -= 1
        l3 = (ln - 3) & 255
        s.code_pos += 3
        s.recs.append(("M", l3, dist))
        s.consume()
        sym = T["SMALL_DIST_SYM"][dist] if dist < 512 else T["LARGE_DIST_SYM"][(dist >> 8) & 127]
        s.h.count[1][sym] = (s.h.count[1][sym] + 1) & M16
        k = (T["LEN_SYM"][l3] & 31) + 256
        s.h.count[0][k] = (s.h.count[0][k] + 1) & M16

    # --- diccionario ---
    def rd(s, pos, n):
        pos &= MASK
        return s.dict[pos:pos + n]

    def u16(s, pos):
        return s.dict[pos] | (s.dict[pos + 1] << 8)

    def put_dict(s, dst, c):
        s.dict[dst] = c
        if dst < MAXM - 1:
            s.dict[DICT + dst] = c

    def find_match(s, la_pos, max_dist, max_len, mdist, mlen):
        max_len = min(MAXM, max_len)
        mlen = max(mlen, 1)
        if max_len <= mlen:
            return mdist, mlen
        pos = la_pos & MASK
        probe = pos
        left = s.probes[0] if mlen < 32 else s.probes[1]
        c01 = s.u16(pos + mlen - 1)
        s01 = s.u16(pos)
        while True:
            found = False
            while not found:
                left -= 1
                if left == 0:
                    return mdist, mlen
                for _ in range(3):
                    npp = s.next[probe]
                    dist = (la_pos - npp) & 0xFFFF
                    if npp == 0 or dist > max_dist or mlen - 1 >= MAXM:
                        return mdist, mlen
                    probe = npp & MASK
                    if s.u16(probe + mlen - 1) == c01:
                        found = True
                        break
            if dist == 0:
                return mdist, mlen
            if s.u16(probe) != s01:
                continue
            p = pos + 2
            q = probe + 2
            cont = False
            for _ in range(32):
                a = s.rd(p, 8)
                b = s.rd(q, 8)
                if a == b:
                    p += 8
                    q += 8
                else:
                    k = 0
                    while a[k] == b[k]:
                        k += 1
                    plen = p - pos + k
                    if plen > mlen:
                        mdist = dist
                        mlen = min(max_len, plen)
                        if mlen >= max_len:
                            return mdist, mlen
                        c01 = s.u16(max(pos + mlen - 1, 0))
                    cont = True
                    break
            if cont:
                continue
            return dist, min(max_len, MAXM)

    def insert(s, data, src, n, la_size, la_pos):
        """Parte común de compress_normal / compress_stored."""
        if la_size + s.size >= 2 and n > 0:
            dst = (la_pos + la_size) & MASK
            ins = la_pos + la_size - 2
            h = ((s.dict[ins & MASK] << 5) ^ s.dict[(ins + 1) & MASK]) & 32767
            la_size += n
            for c in data[src:src + n]:
                s.put_dict(dst, c)
                h = ((h << 5) ^ c) & 32767
                s.next[ins & MASK] = s.hash[h]
                s.hash[h] = ins & M16
                dst = (dst + 1) & MASK
                ins += 1
        else:
            for c in data[src:src + n]:
                dst = (la_pos + la_size) & MASK
                s.put_dict(dst, c)
                la_size += 1
                if la_size + s.size >= 3:
                    ins = la_pos + la_size - 3
                    h = ((s.dict[ins & MASK] << 10) ^ (s.dict[(ins + 1) & MASK] << 5) ^ c) & 32767
                    s.next[ins & MASK] = s.hash[h]
                    s.hash[h] = ins & M16
        return la_size

    def compress_normal(s, data, base, end):
        ln = end - base
        src = 0
        la_size = s.la_size
        la_pos = s.la_pos
        while src < ln or (s.flush != 0 and la_size != 0):
            n = min(ln - src, MAXM - la_size)
            la_size = s.insert(data, base + src, n, la_size, la_pos)
            src += n
            s.size = min(DICT - la_size, s.size)
            if s.flush == 0 and la_size < MAXM:
                break
            move = 1
            cdist = 0
            clen = s.saved_len if s.saved_len != 0 else 2
            cur = la_pos & MASK
            cdist, clen = s.find_match(la_pos, s.size, la_size, cdist, clen)
            if (clen == 3 and cdist >= 8192) or cur == cdist:
                cdist = 0
                clen = 0
            if s.saved_len != 0:
                if clen > s.saved_len:
                    s.lit(s.saved_lit)
                    if clen >= 128:
                        s.match(clen, cdist)
                        s.saved_len = 0
                        move = clen
                    else:
                        s.saved_lit = s.dict[cur]
                        s.saved_dist = cdist
                        s.saved_len = clen
                else:
                    s.match(s.saved_len, s.saved_dist)
                    move = s.saved_len - 1
                    s.saved_len = 0
            elif cdist == 0:
                s.lit(s.dict[cur])
            elif s.greedy or clen >= 128:
                s.match(clen, cdist)
                move = clen
            else:
                s.saved_lit = s.dict[cur]
                s.saved_dist = cdist
                s.saved_len = clen
            la_pos += move
            assert la_size >= move
            la_size -= move
            s.size = min(s.size + move, DICT)
            tight = s.code_pos > 65536 - 8
            fat = ((s.code_pos * 115) >> 7) >= s.total
            if tight or (s.total > 31 * 1024 and fat):
                s.la_size = la_size
                s.la_pos = la_pos
                if s.flush_block(0) != 0:
                    s.src_pos = src
                    return
        s.src_pos = src
        s.la_size = la_size
        s.la_pos = la_pos

    def compress_stored(s, data, base, end):
        ln = end - base
        s.saved_len = 0
        written = s.total
        src = 0
        la_size = s.la_size
        la_pos = s.la_pos
        while src < ln or (s.flush != 0 and la_size != 0):
            n = min(ln - src, MAXM - la_size)
            la_size = s.insert(data, base + src, n, la_size, la_pos)
            src += n
            s.size = min(DICT - la_size, s.size)
            if s.flush == 0 and la_size < MAXM:
                break
            written += 1
            la_pos += 1
            la_size -= 1
            s.size = min(s.size + 1, DICT)
            if written > 31 * 1024:
                s.total = written
                s.la_size = la_size
                s.la_pos = la_pos
                if s.flush_block(0) != 0:
                    s.src_pos = src
                    return
                written = s.total
        s.src_pos = src
        s.total = written
        s.la_size = la_size
        s.la_pos = la_pos

    def compress_fast(s, data, base, end):
        ln = end - base
        src = 0
        la_size = s.la_size
        la_pos = s.la_pos
        cur = la_pos & MASK
        while src < ln or (s.flush != 0 and la_size > 0):
            dst = (la_pos + la_size) & MASK
            n = min(ln - src, 4096 - la_size)
            la_size += n
            while n != 0:
                k = min(DICT - dst, n)
                for i in range(k):
                    s.put_dict(dst + i, data[base + src + i])
                src += k
                dst = (dst + k) & MASK
                n -= k
            s.size = min(DICT - la_size, s.size)
            if s.flush == 0 and la_size < 4096:
                break
            while la_size >= 4:
                clen = 1
                t3 = s.rd(cur, 3)
                first = t3[0] | (t3[1] << 8) | (t3[2] << 16)
                h = (first ^ (first >> 17)) & 4095
                probe = s.hash[h]
                s.hash[h] = la_pos & M16
                cdist = (la_pos - probe) & M16
                if cdist <= s.size:
                    probe &= MASK
                    t3 = s.rd(probe, 3)
                    tri = t3[0] | (t3[1] << 8) | (t3[2] << 16)
                    if first == tri:
                        p = cur + 3
                        q = probe + 3
                        clen = None
                        for _ in range(32):
                            a = s.rd(p, 8)
                            b = s.rd(q, 8)
                            if a == b:
                                p += 8
                                q += 8
                            else:
                                k = 0
                                while a[k] == b[k]:
                                    k += 1
                                clen = p - cur + k
                                break
                        if clen is None:
                            clen = 0 if cdist == 0 else MAXM
                        if clen < 3 or (clen == 3 and cdist >= 8192):
                            lit = first & 255
                            clen = 1
                            s.code_pos += 1
                            s.recs.append(("L", lit))
                            s.h.count[0][lit] = (s.h.count[0][lit] + 1) & M16
                        else:
                            clen = min(clen, la_size)
                            cdist -= 1
                            l3 = (clen - 3) & 255
                            s.code_pos += 3
                            s.recs.append(("M", l3, cdist))
                            sym = (T["SMALL_DIST_SYM"][cdist] if cdist < 512
                                   else T["LARGE_DIST_SYM"][cdist >> 8])
                            s.h.count[1][sym] = (s.h.count[1][sym] + 1) & M16
                            kk = (T["LEN_SYM"][l3] & 31) + 256
                            s.h.count[0][kk] = (s.h.count[0][kk] + 1) & M16
                    else:
                        lit = first & 255
                        s.code_pos += 1
                        s.recs.append(("L", lit))
                        s.h.count[0][lit] = (s.h.count[0][lit] + 1) & M16
                    s.consume()
                    s.total += clen
                    la_pos += clen
                    s.size = min(s.size + clen, DICT)
                    cur = (cur + clen) & MASK
                    la_size -= clen
                    if s.code_pos > 65536 - 8:
                        s.la_size = la_size
                        s.la_pos = la_pos
                        if s.flush_block(0) != 0:
                            s.src_pos = src
                            return
                        la_size = s.la_size
                        la_pos = s.la_pos
            while la_size != 0:
                lit = s.dict[cur]
                s.total += 1
                s.code_pos += 1
                s.recs.append(("L", lit))
                s.consume()
                s.h.count[0][lit] = (s.h.count[0][lit] + 1) & M16
                la_pos += 1
                s.size = min(s.size + 1, DICT)
                cur = (cur + 1) & MASK
                la_size -= 1
                if s.code_pos > 65536 - 8:
                    s.la_size = la_size
                    s.la_pos = la_pos
                    if s.flush_block(0) != 0:
                        s.src_pos = src
                        return
                    la_size = s.la_size
                    la_pos = s.la_pos
        s.src_pos = src
        s.la_size = la_size
        s.la_pos = la_pos

    # --- salida ---
    def lz_codes(s):
        h = s.h
        o = s.o
        for r in s.recs:
            if r[0] == "L":
                o.put(h.codes[0][r[1]], h.sizes[0][r[1]])
            else:
                l3, d = r[1], r[2]
                ls = (T["LEN_SYM"][l3] & 31) + 256
                o.put(h.codes[0][ls], h.sizes[0][ls])
                ne = T["LEN_EXTRA"][l3]
                o.put(l3 & ((1 << (ne & 7)) - 1), ne)
                if d < 512:
                    sym = T["SMALL_DIST_SYM"][d]
                    ne = T["SMALL_DIST_EXTRA"][d]
                else:
                    sym = T["LARGE_DIST_SYM"][d >> 8]
                    ne = T["LARGE_DIST_EXTRA"][d >> 8]
                o.put(h.codes[1][sym], h.sizes[1][sym])
                o.put(d & ((1 << ne) - 1), ne)
        o.put(h.codes[0][256], h.sizes[0][256])

    def comp_block(s, static):
        if static:
            s.h.start_static(s.o)
        else:
            s.h.start_dynamic(s.o)
        s.lz_codes()

    def flush_block(s, flush):
        o = s.o
        antes = len(o.b)
        raw = (s.flags & RAW != 0) and (s.la_pos - s.cbdp) <= s.size
        if s.flags & ZHDR and s.block_index == 0:
            cmf = 0x78 if not (s.flags & RAW) else 0x08
            probes = s.flags & 0xFFF
            if s.flags & GREEDY:
                lv = 0 if probes <= 1 else 1
            elif probes >= NUM_PROBES[9]:
                lv = 3
            else:
                lv = 2
            flg = lv << 6
            rem = (cmf * 256 + flg) % 31
            o.put(cmf, 8)
            o.put((flg & 0xE0) + (31 - rem), 8)
        o.put(1 if flush == 2 else 0, 1)
        saved = o.save()
        if not raw:
            s.comp_block(s.total < 48)
        expanded = (s.total > 32 and len(o.b) - saved[0] + 1 >= s.total
                    and (s.la_pos - s.cbdp) <= s.size)
        if raw or expanded:
            o.load(saved)
            o.put(0, 2)
            o.pad()
            o.put(s.total & M16, 16)
            o.put(~s.total & M16, 16)
            for i in range(s.total):
                o.b.append(s.dict[(s.cbdp + i) & MASK])
        if flush == 2:
            o.pad()
            if s.flags & ZHDR:
                o.b += s.adler.to_bytes(4, "big")
        s.h.count[0] = [0] * 288
        s.h.count[1] = [0] * 288
        s.recs = []
        s.code_pos = 1
        s.nflags = 8
        s.cbdp += s.total
        s.total = 0
        s.block_index += 1
        # CallbackBuf::flush_output: lo que no cabe queda pendiente
        pos = len(o.b) - antes
        if pos == 0:
            return s.flush_remaining
        n = min(pos, s.out_len - s.out_ofs)
        s.out_ofs += n
        if pos != n:
            s.flush_remaining = pos - n
        return s.flush_remaining

    def compress_inner(s, data, base, end, out_len, flush):
        """compress_inner: (terminado, consumidos, escritos)."""
        s.out_len = out_len
        s.out_ofs = 0
        s.src_pos = 0
        s.flush = flush
        if s.flush_remaining != 0 or s.finished:
            return s.flush_output_buffer()
        fast = (s.flags & 0xFFF) == 1 and s.greedy and not (s.flags & RAW)
        f = s.compress_stored if s.flags & RAW else (s.compress_fast if fast else s.compress_normal)
        f(data, base, end)
        remaining = (end - base - s.src_pos) != 0 or s.flush_remaining != 0
        if flush != 0 and s.la_size == 0 and not remaining:
            s.flush_block(flush)
            s.finished = flush == 2
        return s.flush_output_buffer()

    def flush_output_buffer(s):
        n = min(s.out_len - s.out_ofs, s.flush_remaining)
        s.flush_remaining -= n
        s.out_ofs += n
        return (s.finished and s.flush_remaining == 0, s.src_pos, s.out_ofs)

    def deflate(s, data, base, end, flush):
        """stream::deflate con el búfer de 32 KiB de flate2: (consumidos, escritos)."""
        if s.done:
            return 0, 0
        consumed = written = 0
        space = 32 * 1024
        while True:
            fin, inb, outb = s.compress_inner(data, base, end, space, flush)
            base += inb
            space -= outb
            consumed += inb
            written += outb
            if fin:
                s.done = True
                break
            if space == 0:
                break
            if base == end and flush != 2:
                break
        return consumed, written

    def run(s, data):
        s.adler = _zlib.adler32(data) & 0xFFFFFFFF
        n = len(data)
        pos = 0
        # write_all -> Writer::write -> write_with_status
        while pos < n:
            while True:
                c, _ = s.deflate(data, pos, n, 0)
                if c == 0 and not s.done:
                    continue
                break
            assert c > 0
            pos += c
        # finish
        while True:
            _, w = s.deflate(data, n, n, 2)
            if w == 0:
                break
        return bytes(s.o.b)


def encode(data, nivel, formato):
    c = Comp(nivel, formato == "zlib")
    cuerpo = c.run(bytes(data))
    if formato != "gzip":
        return cuerpo
    xfl = 2 if nivel >= 9 else (4 if nivel <= 1 else 0)
    cab = bytes([0x1F, 0x8B, 8, 0, 0, 0, 0, 0, xfl, 0xFF])
    fin = (_zlib.crc32(data) & 0xFFFFFFFF).to_bytes(4, "little") + len(data).to_bytes(4, "little")
    return cab + cuerpo + fin


if __name__ == "__main__":
    import sys
    tablas(sys.argv[1] if len(sys.argv) > 1 else "/tmp/cr/miniz_oxide-0.8.9/src/deflate/core.rs")
    print(encode(b"hola hola hola hola mundo", 6, "gzip").hex())
