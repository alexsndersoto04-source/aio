# Modelo de referencia del DEScompresor zstd tal como lo usa la VM de Titan:
#   std::compress::zstd_decode(data) = zstd::stream::decode_all(data)   (crate zstd 0.13.3)
#   -> io::copy(Decoder, &mut Vec)  -> stack_buffer_copy con buffer de 8192
#   -> zio::Reader + BufReader(131075) -> ZSTD_decompressStream (libzstd 1.5.7)
# Es un port linea por linea de lib/decompress/*.c, lib/common/{entropy_common,
# fse_decompress,bitstream,xxhash}.  La memoria se modela como un unico arreglo de
# bytes con punteros enteros, para reproducir tambien los casos raros (referencias
# fuera de ventana que leen el buffer circular, copias "salvajes" de 16/32 bytes).
# Configuracion de la VM: sin formatos legacy, cadenas de error completas,
# HUF X1 y X2, ruta rapida de Huffman activa (x86-64 con BMI2 / aarch64).

M64 = (1 << 64) - 1
M32 = 0xFFFFFFFF


class ZErr(Exception):
    def __init__(self, code):
        Exception.__init__(self, code)
        self.code = code


class IoErr(Exception):
    pass


ERRNAMES = {
    1: "Error (generic)", 10: "Unknown frame descriptor", 12: "Version not supported",
    14: "Unsupported frame parameter", 16: "Frame requires too much memory for decoding",
    20: "Data corruption detected", 22: "Restored data doesn't match checksum",
    24: "Header of Literals' block doesn't respect format specification",
    30: "Dictionary is corrupted", 32: "Dictionary mismatch",
    44: "tableLog requires too much memory : unsupported",
    46: "Unsupported max Symbol Value : too large", 48: "Specified maxSymbolValue is too small",
    60: "Operation not authorized at current processing stage",
    64: "Allocation error : not enough memory", 70: "Destination buffer is too small",
    72: "Src size is incorrect", 74: "Operation on NULL destination buffer",
    80: "Operation made no progress over multiple calls, due to output buffer being full",
    82: "Operation made no progress over multiple calls, due to input being empty",
    104: "Destination buffer is wrong",
}

E_GENERIC, E_PREFIX, E_FP_UNSUP, E_WINDOW, E_CORRUPT, E_CHECKSUM, E_LITHDR = 1, 10, 14, 16, 20, 22, 24
E_DICT_CORRUPT, E_DICT_WRONG, E_TABLELOG, E_MSV_LARGE, E_MSV_SMALL = 30, 32, 44, 46, 48
E_DST_SMALL, E_SRC_WRONG, E_DST_NULL, E_NFP_DEST, E_NFP_IN = 70, 72, 74, 80, 82


def ERROR(code):
    return (-code) & M64


def is_error(v):
    return v > M64 - 119


MAGIC = 0xFD2FB528
SKIP_START = 0x184D2A50
SKIP_MASK = 0xFFFFFFF0
BLOCKSIZE_MAX = 1 << 17
CS_UNKNOWN = M64
CS_ERROR = M64 - 1
WLEN = 32          # WILDCOPY_OVERLENGTH
WVEC = 16          # WILDCOPY_VECLEN
LITEXTRA = 1 << 16  # ZSTD_LITBUFFEREXTRASIZE
WINDOWLOG_MAX = 31
MAXWINDOW_DEFAULT = (1 << 27) + 1
DSTREAM_IN = BLOCKSIZE_MAX + 3
FCS_FIELD = [0, 2, 4, 8]
DID_FIELD = [0, 1, 2, 4]
REP_START = [1, 4, 8]


def highbit32(v):
    return v.bit_length() - 1


def ctz(v):
    return (v & -v).bit_length() - 1


# ---------------------------------------------------------------- memoria
class Mem:
    def __init__(self):
        self.b = bytearray(4096)
        self.top = 4096

    def alloc(self, n):
        a = self.top
        self.top += (n + 128 + 63) & ~63
        if len(self.b) < self.top:
            self.b.extend(bytes(self.top - len(self.b)))
        return a

    def r(self, a, n):
        assert 0 < a and a + n <= len(self.b), ("lectura fuera", a, n)
        return int.from_bytes(self.b[a:a + n], "little")

    def w(self, a, n, v):
        assert 0 < a and a + n <= len(self.b), ("escritura fuera", a, n)
        self.b[a:a + n] = (v & ((1 << (8 * n)) - 1)).to_bytes(n, "little")

    def copy(self, d, s, n):          # memcpy / memmove (resultado de memmove)
        if n <= 0:
            return
        assert 0 < d and d + n <= len(self.b) and 0 < s and s + n <= len(self.b)
        self.b[d:d + n] = self.b[s:s + n]

    def fill(self, d, v, n):
        if n <= 0:
            return
        assert 0 < d and d + n <= len(self.b)
        self.b[d:d + n] = bytes([v]) * n


# ---------------------------------------------------------------- bitstream.h (64 bits)
OVF, UNF, EOB, COMPL = 3, 0, 1, 2   # overflow, unfinished, endOfBuffer, completed


class BitD:
    __slots__ = ("start", "limit", "ptr", "cont", "cons")

    def __init__(self):
        self.start = self.limit = self.ptr = self.cont = self.cons = 0


def bit_init(m, bd, src, size):
    if size < 1:
        bd.start = bd.limit = bd.ptr = bd.cont = bd.cons = 0
        raise ZErr(E_SRC_WRONG)
    bd.start = src
    bd.limit = src + 8
    if size >= 8:
        bd.ptr = src + size - 8
        bd.cont = m.r(bd.ptr, 8)
        last = m.b[src + size - 1]
        bd.cons = 8 - highbit32(last) if last else 0
        if last == 0:
            raise ZErr(E_GENERIC)
    else:
        bd.ptr = src
        bd.cont = m.r(src, size)
        last = m.b[src + size - 1]
        bd.cons = 8 - highbit32(last) if last else 0
        if last == 0:
            raise ZErr(E_CORRUPT)
        bd.cons += (8 - size) * 8
    return size


def bit_look(bd, nb):
    return (bd.cont >> ((64 - bd.cons - nb) & 63)) & ((1 << nb) - 1)


def bit_look_fast(bd, nb):
    return ((bd.cont << (bd.cons & 63)) & M64) >> ((64 - nb) & 63)


def bit_read(bd, nb):
    v = bit_look(bd, nb)
    bd.cons += nb
    return v


def bit_read_fast(bd, nb):
    v = bit_look_fast(bd, nb)
    bd.cons += nb
    return v


ZERO_FILLED = -8   # direccion simbolica del "zeroFilled" estatico (nunca se lee)


def bit_reload(m, bd):
    if bd.cons > 64:
        bd.ptr = ZERO_FILLED
        return OVF
    if bd.ptr >= bd.limit:
        bd.ptr -= bd.cons >> 3
        bd.cons &= 7
        bd.cont = m.r(bd.ptr, 8)
        return UNF
    if bd.ptr == bd.start:
        if bd.cons < 64:
            return EOB
        return COMPL
    nb = bd.cons >> 3
    res = UNF
    if bd.ptr - nb < bd.start:
        nb = bd.ptr - bd.start
        res = EOB
    bd.ptr -= nb
    bd.cons -= nb * 8
    bd.cont = m.r(bd.ptr, 8)
    return res


def bit_reload_fast(m, bd):
    if bd.ptr < bd.limit:
        return OVF
    bd.ptr -= bd.cons >> 3
    bd.cons &= 7
    bd.cont = m.r(bd.ptr, 8)
    return UNF


def bit_end(bd):
    return bd.ptr == bd.start and bd.cons == 64


# ---------------------------------------------------------------- entropy_common.c
def fse_read_ncount(data, maxSV):
    """FSE_readNCount sobre un bytes; devuelve (norm, maxSV, tableLog, size)."""
    hb = len(data)
    if hb < 8:
        r = fse_read_ncount(bytes(data) + bytes(8 - hb), maxSV)
        if r[3] > hb:
            raise ZErr(E_CORRUPT)
        return r
    norm = [0] * (maxSV + 1)
    iend = hb
    ip = 0
    charnum = 0
    maxSV1 = maxSV + 1
    previous0 = 0

    def rd32(p):
        return int.from_bytes(data[p:p + 4], "little")
    bitStream = rd32(ip)
    nbBits = (bitStream & 0xF) + 5
    if nbBits > 15:
        raise ZErr(E_TABLELOG)
    bitStream >>= 4
    bitCount = 4
    tableLog = nbBits
    remaining = (1 << nbBits) + 1
    threshold = 1 << nbBits
    nbBits += 1
    while True:
        if previous0:
            repeats = ctz((~bitStream & M32) | 0x80000000) >> 1
            while repeats >= 12:
                charnum += 3 * 12
                if ip <= iend - 7:
                    ip += 3
                else:
                    bitCount -= 8 * (iend - 7 - ip)
                    bitCount &= 31
                    ip = iend - 4
                bitStream = rd32(ip) >> bitCount
                repeats = ctz((~bitStream & M32) | 0x80000000) >> 1
            charnum += 3 * repeats
            bitStream >>= 2 * repeats
            bitCount += 2 * repeats
            charnum += bitStream & 3
            bitCount += 2
            if charnum >= maxSV1:
                break
            if ip <= iend - 7 or ip + (bitCount >> 3) <= iend - 4:
                ip += bitCount >> 3
                bitCount &= 7
            else:
                bitCount -= 8 * (iend - 4 - ip)
                bitCount &= 31
                ip = iend - 4
            bitStream = rd32(ip) >> bitCount
        mx = (2 * threshold - 1) - remaining
        if (bitStream & (threshold - 1)) < (mx & M32):
            count = bitStream & (threshold - 1)
            bitCount += nbBits - 1
        else:
            count = bitStream & (2 * threshold - 1)
            if count >= threshold:
                count -= mx
            bitCount += nbBits
        count -= 1
        if count >= 0:
            remaining -= count
        else:
            remaining += count
        norm[charnum] = count
        charnum += 1
        previous0 = 1 if count == 0 else 0
        if remaining < threshold:
            if remaining <= 1:
                break
            nbBits = highbit32(remaining) + 1
            threshold = 1 << (nbBits - 1)
        if charnum >= maxSV1:
            break
        if ip <= iend - 7 or ip + (bitCount >> 3) <= iend - 4:
            ip += bitCount >> 3
            bitCount &= 7
        else:
            bitCount -= 8 * (iend - 4 - ip)
            bitCount &= 31
            ip = iend - 4
        bitStream = rd32(ip) >> bitCount
    if remaining != 1:
        raise ZErr(E_CORRUPT)
    if charnum > maxSV1:
        raise ZErr(E_MSV_SMALL)
    if bitCount > 32:
        raise ZErr(E_CORRUPT)
    ip += (bitCount + 7) >> 3
    return norm, charnum - 1, tableLog, ip


# FSE_DECOMPRESS_WKSP_SIZE_U32(tl, msv)
def fse_dwksp_u32(tl, msv):
    build = (2 * (msv + 1) + (1 << tl) + 8 + 3) // 4
    return (1 + (1 << tl)) + 1 + build + 128 + 1


HUF_STATS_WKSP = fse_dwksp_u32(6, 11) * 4      # bytes


def fse_build_dtable(norm, maxSV, tableLog, wksize):
    """FSE_buildDTable_internal: tabla de (newState, symbol, nbBits) + fastMode."""
    if 2 * (maxSV + 1) + (1 << tableLog) + 8 > wksize:
        raise ZErr(E_MSV_LARGE)
    if maxSV > 255:
        raise ZErr(E_MSV_LARGE)
    if tableLog > 12:
        raise ZErr(E_TABLELOG)
    size = 1 << tableLog
    high = size - 1
    sym = [0] * size
    nxt = [0] * (maxSV + 1)
    fast = 1
    large = 1 << (tableLog - 1)
    for s in range(maxSV + 1):
        if norm[s] == -1:
            sym[high] = s
            high -= 1
            nxt[s] = 1
        else:
            if norm[s] >= large:
                fast = 0
            nxt[s] = norm[s] & 0xFFFF
    mask = size - 1
    step = (size >> 1) + (size >> 3) + 3
    if high == size - 1:
        spread = []
        for s in range(maxSV + 1):
            spread.extend([s] * max(norm[s], 0))
        spread.extend([0] * 16)
        pos = 0
        s = 0
        while s < size:
            for u in range(2):
                sym[(pos + u * step) & mask] = spread[s + u]
            pos = (pos + 2 * step) & mask
            s += 2
    else:
        pos = 0
        for s in range(maxSV + 1):
            for _ in range(norm[s]):
                sym[pos] = s
                pos = (pos + step) & mask
                while pos > high:
                    pos = (pos + step) & mask
        if pos != 0:
            raise ZErr(E_GENERIC)
    newst = [0] * size
    nbb = [0] * size
    for u in range(size):
        s = sym[u]
        ns = nxt[s]
        nxt[s] = (ns + 1) & 0xFFFF
        nb = tableLog - highbit32(ns)
        nbb[u] = nb & 0xFF
        newst[u] = ((ns << nb) - size) & 0xFFFF
    return {"log": tableLog, "fast": fast, "sym": sym, "new": newst, "nb": nbb}


def fse_decompress_weights(m, dstcap, src, srcSize):
    """FSE_decompress_wksp_bmi2(huffWeight, 255, ip+1, iSize, 6, wksp, 876): lista de pesos."""
    wk = HUF_STATS_WKSP
    if wk < 512:
        raise ZErr(E_GENERIC)
    norm, msv, tl, ncl = fse_read_ncount(bytes(m.b[src:src + srcSize]), 255)
    if tl > 6:
        raise ZErr(E_TABLELOG)
    ip = src + ncl
    csize = srcSize - ncl
    if fse_dwksp_u32(tl, msv) * 4 > wk:
        raise ZErr(E_TABLELOG)
    rest = wk - 512 - 4 * (1 + (1 << tl))
    dt = fse_build_dtable(norm, msv, tl, rest)
    fast = dt["fast"]
    out = []
    bd = BitD()
    bit_init(m, bd, ip, csize)

    def init_state():
        st = bit_read(bd, dt["log"])
        bit_reload(m, bd)
        return st
    s1 = init_state()
    s2 = init_state()
    if bit_reload(m, bd) == OVF:
        raise ZErr(E_CORRUPT)
    st = [s1, s2]

    def get(i):
        s = st[i]
        nb = dt["nb"][s]
        sy = dt["sym"][s]
        low = bit_read_fast(bd, nb) if fast else bit_read(bd, nb)
        st[i] = dt["new"][s] + low
        return sy
    omax = dstcap
    olimit = omax - 3
    op = 0
    while (bit_reload(m, bd) == UNF) and op < olimit:
        out.append(get(0))
        out.append(get(1))
        out.append(get(0))
        out.append(get(1))
        op += 4
    while True:
        if op > omax - 2:
            raise ZErr(E_DST_SMALL)
        out.append(get(0))
        op += 1
        if bit_reload(m, bd) == OVF:
            out.append(get(1))
            op += 1
            break
        if op > omax - 2:
            raise ZErr(E_DST_SMALL)
        out.append(get(1))
        op += 1
        if bit_reload(m, bd) == OVF:
            out.append(get(0))
            op += 1
            break
    return out


def huf_read_stats(m, src, srcSize):
    """HUF_readStats_wksp(huffWeight, 256, ...): (weights[0..nbSymbols], rankStats, nbSymbols, tableLog, size)."""
    if not srcSize:
        raise ZErr(E_SRC_WRONG)
    iSize = m.b[src]
    w = [0] * 256
    if iSize >= 128:
        oSize = iSize - 127
        iSize = (oSize + 1) // 2
        if iSize + 1 > srcSize:
            raise ZErr(E_SRC_WRONG)
        if oSize >= 256:
            raise ZErr(E_CORRUPT)
        for n in range(0, oSize, 2):
            b = m.b[src + 1 + n // 2]
            w[n] = b >> 4
            w[n + 1] = b & 15
    else:
        if iSize + 1 > srcSize:
            raise ZErr(E_SRC_WRONG)
        vals = fse_decompress_weights(m, 255, src + 1, iSize)
        oSize = len(vals)
        w[:oSize] = vals
    rank = [0] * 13
    total = 0
    for n in range(oSize):
        if w[n] > 12:
            raise ZErr(E_CORRUPT)
        rank[w[n]] += 1
        total += (1 << w[n]) >> 1
    if total == 0:
        raise ZErr(E_CORRUPT)
    tableLog = highbit32(total) + 1
    if tableLog > 12:
        raise ZErr(E_CORRUPT)
    rest = (1 << tableLog) - total
    verif = 1 << highbit32(rest)
    last = highbit32(rest) + 1
    if verif != rest:
        raise ZErr(E_CORRUPT)
    w[oSize] = last
    rank[last] += 1
    if rank[1] < 2 or (rank[1] & 1):
        raise ZErr(E_CORRUPT)
    return w, rank, oSize + 1, tableLog, iSize + 1


# ---------------------------------------------------------------- huf_decompress.c
# La tabla HUF_DTable vive en memoria: 4 bytes de descriptor (maxTableLog, tableType,
# tableLog, reserved) y despues entradas X1 de 2 bytes (nbBits, byte) o X2 de 4 bytes
# (sequence u16, nbBits, length).
FAST_TL = 11


def huf_desc(m, ht):
    return m.b[ht], m.b[ht + 1], m.b[ht + 2]


def huf_read_dtable_x1(m, ht, src, srcSize):
    w, rank, nbSym, tableLog, iSize = huf_read_stats(m, src, srcSize)
    maxTL = m.b[ht]
    target = min(maxTL + 1, FAST_TL)
    # HUF_rescaleStats
    if tableLog <= target:
        if tableLog < target:
            scale = target - tableLog
            for s in range(nbSym):
                if w[s]:
                    w[s] += scale
            for s in range(target, scale, -1):
                rank[s] = rank[s - scale]
            for s in range(scale, 0, -1):
                rank[s] = 0
        tableLog = target
    if tableLog > maxTL + 1:
        raise ZErr(E_TABLELOG)
    m.b[ht + 1] = 0
    m.b[ht + 2] = tableLog
    rstart = [0] * 13
    nxt = 0
    for n in range(tableLog + 1):
        rstart[n] = nxt
        nxt += rank[n]
    syms = [0] * 256
    for n in range(nbSym):
        ww = w[n]
        syms[rstart[ww]] = n
        rstart[ww] += 1
    symbol = rank[0]
    pos = 0
    dt = ht + 4
    for ww in range(1, tableLog + 1):
        cnt = rank[ww]
        length = (1 << ww) >> 1
        nbBits = tableLog + 1 - ww
        u = pos
        for s in range(cnt):
            e = bytes([nbBits, syms[symbol + s]]) * length
            m.b[dt + 2 * u:dt + 2 * (u + length)] = e
            u += length
        symbol += cnt
        pos += cnt * length
    return iSize


def _x2_u32(symbol, nbBits, baseSeq, level):
    seq = symbol if level == 1 else baseSeq + (symbol << 8)
    return (seq + (nbBits << 16) + (level << 24)) & M32


def _x2_fill_weight(m, dt, rank_pos, syms, nbBits, tableLog, baseSeq, level):
    length = 1 << ((tableLog - nbBits) & 0x1F)
    p = rank_pos
    for sy in syms:
        e = _x2_u32(sy, nbBits, baseSeq, level).to_bytes(4, "little") * length
        m.b[dt + 4 * p:dt + 4 * (p + length)] = e
        p += length


def _x2_fill_level2(m, dt, base, targetLog, consumed, rankVal, minWeight, maxWeight1,
                    sorted_, rs0, nbBase, baseSeq):
    if minWeight > 1:
        length = 1 << ((targetLog - consumed) & 0x1F)
        e = _x2_u32(baseSeq, consumed, 0, 1).to_bytes(4, "little")
        skip = rankVal[minWeight]
        if length == 2:
            n = 2
        elif length == 4:
            n = 4
        else:
            n = 0
            i = 0
            while i < skip:
                n = i + 8
                i += 8
        m.b[dt + 4 * base:dt + 4 * (base + n)] = e * n
    for ww in range(minWeight, maxWeight1):
        b0, e0 = rs0[ww], rs0[ww + 1]
        nbBits = nbBase - ww
        _x2_fill_weight(m, dt, base + rankVal[ww], sorted_[b0:e0], nbBits + consumed,
                        targetLog, baseSeq, 2)


def huf_read_dtable_x2(m, ht, src, srcSize):
    maxTL = m.b[ht]
    if maxTL > 12:
        raise ZErr(E_TABLELOG)
    w, rankStats, nbSym, tableLog, iSize = huf_read_stats(m, src, srcSize)
    if tableLog > maxTL:
        raise ZErr(E_TABLELOG)
    if tableLog <= FAST_TL and maxTL > FAST_TL:
        maxTL = FAST_TL
    maxW = tableLog
    while rankStats[maxW] == 0:
        maxW -= 1
    rs0 = [0] * 15          # rankStart0; rankStart[i] == rs0[i+1]
    nxt = 0
    for ww in range(1, maxW + 1):
        rs0[ww + 1] = nxt
        nxt += rankStats[ww]
    rs0[1] = nxt
    rs0[maxW + 2] = nxt
    sorted_ = [0] * 256
    for s in range(nbSym):
        ww = w[s]
        r = rs0[ww + 1]
        rs0[ww + 1] += 1
        sorted_[r] = s
    rs0[1] = 0
    rankVal = [None] * 12
    rv0 = [0] * 13
    rescale = (maxTL - tableLog) - 1
    nv = 0
    for ww in range(1, maxW + 1):
        rv0[ww] = nv
        nv += rankStats[ww] << (ww + rescale)
    rankVal[0] = rv0
    minBits = tableLog + 1 - maxW
    for consumed in range(minBits, maxTL - minBits + 1):
        row = rankVal[consumed] if rankVal[consumed] is not None else [0] * 13
        for ww in range(1, maxW + 1):
            row[ww] = rv0[ww] >> consumed
        rankVal[consumed] = row
    # HUF_fillDTableX2
    dt = ht + 4
    targetLog = maxTL
    nbBase = tableLog + 1
    rv = rankVal[0]
    scaleLog = nbBase - targetLog
    minBits2 = nbBase - maxW
    wEnd = maxW + 1
    for ww in range(1, wEnd):
        b0, e0 = rs0[ww], rs0[ww + 1]
        nbBits = nbBase - ww
        if targetLog - nbBits >= minBits2:
            start = rv[ww]
            length = 1 << ((targetLog - nbBits) & 0x1F)
            minWeight = nbBits + scaleLog
            if minWeight < 1:
                minWeight = 1
            row = rankVal[nbBits]
            assert row is not None, "rankVal sin inicializar"
            for s in range(b0, e0):
                _x2_fill_level2(m, dt, start, targetLog, nbBits, row, minWeight, wEnd,
                                sorted_, rs0, nbBase, sorted_[s])
                start += length
        else:
            _x2_fill_weight(m, dt, rv[ww], sorted_[b0:e0], nbBits, targetLog, 0, 1)
    m.b[ht + 2] = maxTL
    m.b[ht + 1] = 1
    return iSize


def _x1_sym(m, bd, dt, dtLog):
    v = bit_look_fast(bd, dtLog)
    e = dt + 2 * v
    bd.cons += m.b[e]
    return m.b[e + 1]


def huf_stream_x1(m, p, bd, pEnd, dt, dtLog):
    pStart = p
    if pEnd - p > 3:
        while True:
            r = bit_reload(m, bd) == UNF
            if not (r and p < pEnd - 3):
                break
            for _ in range(4):
                m.b[p] = _x1_sym(m, bd, dt, dtLog)
                p += 1
    else:
        bit_reload(m, bd)
    while p < pEnd:
        m.b[p] = _x1_sym(m, bd, dt, dtLog)
        p += 1
    return pEnd - pStart


def _x2_sym(m, op, bd, dt, dtLog):
    v = bit_look_fast(bd, dtLog)
    e = dt + 4 * v
    m.b[op] = m.b[e]
    m.b[op + 1] = m.b[e + 1]
    bd.cons += m.b[e + 2]
    return m.b[e + 3]


def _x2_last(m, op, bd, dt, dtLog):
    v = bit_look_fast(bd, dtLog)
    e = dt + 4 * v
    m.b[op] = m.b[e]
    if m.b[e + 3] == 1:
        bd.cons += m.b[e + 2]
    else:
        if bd.cons < 64:
            bd.cons += m.b[e + 2]
            if bd.cons > 64:
                bd.cons = 64
    return 1


def huf_stream_x2(m, p, bd, pEnd, dt, dtLog):
    pStart = p
    if pEnd - p >= 8:
        if dtLog <= 11:
            while True:
                r = bit_reload(m, bd) == UNF
                if not (r and p < pEnd - 9):
                    break
                for _ in range(5):
                    p += _x2_sym(m, p, bd, dt, dtLog)
        else:
            while True:
                r = bit_reload(m, bd) == UNF
                if not (r and p < pEnd - 7):
                    break
                for _ in range(4):
                    p += _x2_sym(m, p, bd, dt, dtLog)
    else:
        bit_reload(m, bd)
    if pEnd - p >= 2:
        while True:
            r = bit_reload(m, bd) == UNF
            if not (r and p <= pEnd - 2):
                break
            p += _x2_sym(m, p, bd, dt, dtLog)
        while p <= pEnd - 2:
            p += _x2_sym(m, p, bd, dt, dtLog)
    if p < pEnd:
        p += _x2_last(m, p, bd, dt, dtLog)
    return p - pStart


def huf_1x_using(m, ht, dst, dstSize, src, srcSize):
    """HUF_decompress1X{1,2}_usingDTable_internal (siempre generico, con chequeo final)."""
    _, ttype, dtLog = huf_desc(m, ht)
    bd = BitD()
    bit_init(m, bd, src, srcSize)
    if ttype:
        huf_stream_x2(m, dst, bd, dst + dstSize, ht + 4, dtLog)
    else:
        huf_stream_x1(m, dst, bd, dst + dstSize, ht + 4, dtLog)
    if not bit_end(bd):
        raise ZErr(E_CORRUPT)
    return dstSize


def _fast_init(m, ht, dst, dstSize, src, srcSize):
    dtLog = m.b[ht + 2]
    oend = dst + dstSize
    if dstSize == 0:
        return None
    if srcSize < 10:
        raise ZErr(E_CORRUPT)
    if dtLog != FAST_TL:
        return None
    l1, l2, l3 = m.r(src, 2), m.r(src + 2, 2), m.r(src + 4, 2)
    l4 = (srcSize - (l1 + l2 + l3 + 6)) & M64
    iend = [src + 6, src + 6 + l1, src + 6 + l1 + l2, src + 6 + l1 + l2 + l3]
    if l1 < 8 or l2 < 8 or l3 < 8 or l4 < 8:
        return None
    if l4 > srcSize:
        raise ZErr(E_CORRUPT)
    ip = [iend[1] - 8, iend[2] - 8, iend[3] - 8, src + srcSize - 8]
    seg = (dstSize + 3) // 4
    op = [dst, dst + seg, dst + 2 * seg, dst + 3 * seg]
    if op[3] >= oend:
        return None
    bits = []
    for k in range(4):
        last = m.b[ip[k] + 7]
        bc = 8 - highbit32(last) if last else 0
        bits.append(((m.r(ip[k], 8) | 1) << bc) & M64)
    return {"ip": ip, "op": op, "bits": bits, "ilowest": src, "oend": oend, "iend": iend}


def _fast_loop_x1(m, a, dt):
    bits, ip, op = a["bits"], a["ip"], a["op"]
    oend, ilow = a["oend"], a["ilowest"]
    while True:
        oiters = (oend - op[3]) // 5
        iiters = (ip[0] - ilow) // 7
        olimit = op[3] + min(oiters, iiters) * 5
        if op[3] == olimit:
            break
        if ip[1] < ip[0] or ip[2] < ip[1] or ip[3] < ip[2]:
            break
        while True:
            for sy in range(5):
                for s in range(4):
                    idx = bits[s] >> 53
                    e = m.b[dt + 2 * idx] | (m.b[dt + 2 * idx + 1] << 8)
                    bits[s] = (bits[s] << (e & 0x3F)) & M64
                    m.b[op[s] + sy] = (e >> 8) & 0xFF
            for s in range(4):
                c = ctz(bits[s]) if bits[s] else 64
                op[s] += 5
                ip[s] -= c >> 3
                bits[s] = ((m.r(ip[s], 8) | 1) << (c & 7)) & M64
            if not op[3] < olimit:
                break


def _fast_loop_x2(m, a, dt):
    bits, ip, op = a["bits"], a["ip"], a["op"]
    ilow = a["ilowest"]
    oend = [op[1], op[2], op[3], a["oend"]]

    def dec(s):
        idx = bits[s] >> 53
        e = dt + 4 * idx
        m.b[op[s]] = m.b[e]
        m.b[op[s] + 1] = m.b[e + 1]
        bits[s] = (bits[s] << (m.b[e + 2] & 0x3F)) & M64
        op[s] += m.b[e + 3]

    def reload(s):
        c = ctz(bits[s]) if bits[s] else 64
        ip[s] -= c >> 3
        bits[s] = ((m.r(ip[s], 8) | 1) << (c & 7)) & M64
    while True:
        iters = (ip[0] - ilow) // 7
        for s in range(4):
            iters = min(iters, (oend[s] - op[s]) // 10)
        olimit = op[3] + iters * 5
        if op[3] == olimit:
            break
        if ip[1] < ip[0] or ip[2] < ip[1] or ip[3] < ip[2]:
            break
        while True:
            # orden de huf_decompress_amd64.S (x86-64 BMI2): 5 rondas x 4 flujos
            for _ in range(5):
                for s in range(4):
                    dec(s)
            for s in range(4):
                reload(s)
            if not op[3] < olimit:
                break


def _huf_4x_fast(m, ht, dst, dstSize, src, srcSize, x2):
    a = _fast_init(m, ht, dst, dstSize, src, srcSize)
    if a is None:
        return 0
    dt = ht + 4
    if x2:
        _fast_loop_x2(m, a, dt)
    else:
        _fast_loop_x1(m, a, dt)
    seg = (dstSize + 3) // 4
    segEnd = dst
    oend = dst + dstSize
    for i in range(4):
        if seg <= oend - segEnd:
            segEnd += seg
        else:
            segEnd = oend
        if a["op"][i] > segEnd:
            raise ZErr(E_CORRUPT)
        if a["ip"][i] < a["iend"][i] - 8:
            raise ZErr(E_CORRUPT)
        bd = BitD()
        bd.cont = m.r(a["ip"][i], 8)
        bd.cons = ctz(a["bits"][i]) if a["bits"][i] else 64
        bd.start = a["ilowest"]
        bd.limit = bd.start + 8
        bd.ptr = a["ip"][i]
        if x2:
            a["op"][i] += huf_stream_x2(m, a["op"][i], bd, segEnd, dt, FAST_TL)
        else:
            a["op"][i] += huf_stream_x1(m, a["op"][i], bd, segEnd, dt, FAST_TL)
        if a["op"][i] != segEnd:
            raise ZErr(E_CORRUPT)
    return dstSize


def _huf_4x_body(m, ht, dst, dstSize, src, srcSize, x2):
    if srcSize < 10:
        raise ZErr(E_CORRUPT)
    if dstSize < 6:
        raise ZErr(E_CORRUPT)
    oend = dst + dstSize
    dt = ht + 4
    dtLog = m.b[ht + 2]
    l1, l2, l3 = m.r(src, 2), m.r(src + 2, 2), m.r(src + 4, 2)
    l4 = (srcSize - (l1 + l2 + l3 + 6)) & M64
    is1 = src + 6
    is2 = is1 + l1
    is3 = is2 + l2
    is4 = is3 + l3
    seg = (dstSize + 3) // 4
    o2 = dst + seg
    o3 = o2 + seg
    o4 = o3 + seg
    op = [dst, o2, o3, o4]
    if l4 > srcSize:
        raise ZErr(E_CORRUPT)
    if o4 > oend:
        raise ZErr(E_CORRUPT)
    bds = [BitD() for _ in range(4)]
    bit_init(m, bds[0], is1, l1)
    bit_init(m, bds[1], is2, l2)
    bit_init(m, bds[2], is3, l3)
    bit_init(m, bds[3], is4, l4)
    endSignal = True
    if not x2:
        olimit = oend - 3
        if oend - o4 >= 8:
            while endSignal and op[3] < olimit:
                for _ in range(4):
                    for s in range(4):
                        m.b[op[s]] = _x1_sym(m, bds[s], dt, dtLog)
                        op[s] += 1
                for s in range(4):
                    if bit_reload_fast(m, bds[s]) != UNF:
                        endSignal = False
    else:
        olimit = oend - 7
        if oend - o4 >= 8:
            while endSignal and op[3] < olimit:
                for _ in range(4):
                    for s in range(4):
                        op[s] += _x2_sym(m, op[s], bds[s], dt, dtLog)
                for s in range(4):
                    if bit_reload_fast(m, bds[s]) != UNF:
                        endSignal = False
    if op[0] > o2 or op[1] > o3 or op[2] > o4:
        raise ZErr(E_CORRUPT)
    ends = [o2, o3, o4, oend]
    for s in range(4):
        if x2:
            huf_stream_x2(m, op[s], bds[s], ends[s], dt, dtLog)
        else:
            huf_stream_x1(m, op[s], bds[s], ends[s], dt, dtLog)
    if not (bit_end(bds[0]) and bit_end(bds[1]) and bit_end(bds[2]) and bit_end(bds[3])):
        raise ZErr(E_CORRUPT)
    return dstSize


# Solo para pruebas: fuerzan rutas alternativas (con datos validos la salida no cambia).
DBG_NO_FAST_HUF = False
DBG_FORCE_LONG = False


def huf_4x_using(m, ht, dst, dstSize, src, srcSize):
    """HUF_decompress4X{1,2}_usingDTable_internal con la ruta rapida activa."""
    x2 = m.b[ht + 1] != 0
    r = 0 if DBG_NO_FAST_HUF else _huf_4x_fast(m, ht, dst, dstSize, src, srcSize, x2)
    if r != 0:
        return r
    return _huf_4x_body(m, ht, dst, dstSize, src, srcSize, x2)


ALGO_TIME = [
    ((0, 0), (1, 1)), ((0, 0), (1, 1)), ((150, 216), (381, 119)), ((170, 205), (514, 112)),
    ((177, 199), (539, 110)), ((197, 194), (644, 107)), ((221, 192), (735, 107)),
    ((256, 189), (881, 106)), ((359, 188), (1167, 109)), ((582, 187), (1570, 114)),
    ((688, 187), (1712, 122)), ((825, 186), (1965, 136)), ((976, 185), (2131, 150)),
    ((1180, 186), (2070, 175)), ((1377, 185), (1731, 202)), ((1412, 185), (1695, 202))]


def huf_select(dstSize, cSrcSize):
    Q = 15 if cSrcSize >= dstSize else (cSrcSize * 16 // dstSize)
    D256 = dstSize >> 8
    t0 = ALGO_TIME[Q][0][0] + ALGO_TIME[Q][0][1] * D256
    t1 = ALGO_TIME[Q][1][0] + ALGO_TIME[Q][1][1] * D256
    t1 += t1 >> 5
    return 1 if (t1 & M32) < (t0 & M32) else 0


def huf_1x1_dctx(m, ht, dst, dstSize, src, srcSize):
    h = huf_read_dtable_x1(m, ht, src, srcSize)
    if h >= srcSize:
        raise ZErr(E_SRC_WRONG)
    return huf_1x_using(m, ht, dst, dstSize, src + h, srcSize - h)


def huf_4x_huf_only(m, ht, dst, dstSize, src, srcSize):
    if dstSize == 0:
        raise ZErr(E_DST_SMALL)
    if srcSize == 0:
        raise ZErr(E_CORRUPT)
    if huf_select(dstSize, srcSize):
        h = huf_read_dtable_x2(m, ht, src, srcSize)
    else:
        h = huf_read_dtable_x1(m, ht, src, srcSize)
    if h >= srcSize:
        raise ZErr(E_SRC_WRONG)
    return huf_4x_using(m, ht, dst, dstSize, src + h, srcSize - h)


# ---------------------------------------------------------------- zstd_decompress_block.c
LL_BITS = [0] * 16 + [1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
LL_NORM = [4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
           2, 3, 2, 1, 1, 1, 1, 1, -1, -1, -1, -1]
ML_BITS = [0] * 32 + [1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
ML_NORM = [1, 4, 3, 2, 2, 2, 2, 2, 2] + [1] * 37 + [-1] * 7
OF_NORM = [1, 1, 1, 1, 1, 1, 2, 2, 2] + [1] * 15 + [-1] * 5
LL_BASE = list(range(16)) + [16, 18, 20, 22, 24, 28, 32, 40, 48, 64, 0x80, 0x100, 0x200,
                             0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000, 0x10000]
OF_BASE = [0, 1, 1, 5] + [(1 << k) - 3 for k in range(4, 32)]
OF_BITS = list(range(32))
ML_BASE = list(range(3, 35)) + [35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 0x83, 0x103,
                                0x203, 0x403, 0x803, 0x1003, 0x2003, 0x4003, 0x8003, 0x10003]
MAXLL, MAXML, MAXOFF = 35, 52, 31
LLFSELOG, MLFSELOG, OFFFSELOG = 9, 9, 8
set_basic, set_rle, set_compressed, set_repeat = 0, 1, 2, 3
bt_raw, bt_rle, bt_compressed, bt_reserved = 0, 1, 2, 3
NOT_IN_DST, IN_DST, SPLIT = 0, 1, 2


class SeqTable:
    __slots__ = ("log", "ns", "nba", "nbb", "base")


def build_seq_table(norm, maxSV, baseValue, nbAdd, tableLog):
    """ZSTD_buildFSETable_body."""
    size = 1 << tableLog
    high = size - 1
    sym = [0] * size
    nxt = [0] * (maxSV + 1)
    for s in range(maxSV + 1):
        if norm[s] == -1:
            sym[high] = s
            high -= 1
            nxt[s] = 1
        else:
            nxt[s] = norm[s] & 0xFFFF
    mask = size - 1
    step = (size >> 1) + (size >> 3) + 3
    if high == size - 1:
        spread = []
        for s in range(maxSV + 1):
            spread.extend([s] * max(norm[s], 0))
        spread.extend([0] * 16)
        pos = 0
        s = 0
        while s < size:
            for u in range(2):
                sym[(pos + u * step) & mask] = spread[s + u]
            pos = (pos + 2 * step) & mask
            s += 2
    else:
        pos = 0
        for s in range(maxSV + 1):
            for _ in range(norm[s]):
                sym[pos] = s
                pos = (pos + step) & mask
                while pos > high:
                    pos = (pos + step) & mask
    t = SeqTable()
    t.log = tableLog
    t.ns = [0] * size
    t.nba = [0] * size
    t.nbb = [0] * size
    t.base = [0] * size
    for u in range(size):
        s = sym[u]
        ns = nxt[s]
        nxt[s] = (ns + 1) & 0xFFFF
        nb = (tableLog - highbit32(ns)) & 0xFF
        t.nbb[u] = nb
        t.ns[u] = ((ns << nb) - size) & 0xFFFF
        t.nba[u] = nbAdd[s]
        t.base[u] = baseValue[s]
    return t


def rle_seq_table(base, nbAdd):
    t = SeqTable()
    t.log = 0
    t.ns, t.nba, t.nbb, t.base = [0], [nbAdd], [0], [base]
    return t


LL_DEFAULT = build_seq_table(LL_NORM, MAXLL, LL_BASE, LL_BITS, 6)
ML_DEFAULT = build_seq_table(ML_NORM, MAXML, ML_BASE, ML_BITS, 6)
OF_DEFAULT = build_seq_table(OF_NORM, 28, OF_BASE, OF_BITS, 5)


class DCtx:
    pass


def new_dctx(m):
    d = DCtx()
    d.m = m
    d.litExtra = m.alloc(LITEXTRA + WLEN)
    d.hdr = m.alloc(32)                  # headerBuffer[ZSTD_FRAMEHEADERSIZE_MAX]
    d.huf = m.alloc(4 * (1 + 4096))      # entropy.hufTable
    d.fParams = None
    d.maxWindowSize = MAXWINDOW_DEFAULT
    d.streamStage = 0
    d.noForwardProgress = 0
    d.oversizedDuration = 0
    d.inBuff = 0
    d.inBuffSize = 0
    d.outBuff = 0
    d.outBuffSize = 0
    d.stage = 0
    d.expected = 0
    d.lhSize = d.inPos = d.outStart = d.outEnd = 0
    d.hostageByte = 0
    d.dictID = 0
    return d


def decompress_begin(d):
    d.expected = 5
    d.stage = DS_GETFH
    d.processedCSize = 0
    d.decodedSize = 0
    d.previousDstEnd = 0
    d.prefixStart = 0
    d.virtualStart = 0
    d.dictEnd = 0
    d.m.w(d.huf, 4, 12 * 0x1000001)
    d.litEntropy = d.fseEntropy = 0
    d.dictID = 0
    d.bType = bt_reserved
    d.rep = list(REP_START)
    d.LLspace = d.MLspace = d.OFspace = None
    d.LLT = d.MLT = d.OFT = None        # None == tabla propia aun sin construir
    d.ddictIsCold = 0
    d.litBufferLocation = NOT_IN_DST
    d.xxh = None


def check_continuity(d, dst, dstSize):
    if dst != d.previousDstEnd and dstSize > 0:
        d.dictEnd = d.previousDstEnd
        d.virtualStart = dst - (d.previousDstEnd - d.prefixStart)
        d.prefixStart = dst
        d.previousDstEnd = dst


def get_cblock_size(m, src, srcSize):
    if srcSize < 3:
        raise ZErr(E_SRC_WRONG)
    h = m.r(src, 3)
    cSize = h >> 3
    last = h & 1
    bt = (h >> 1) & 3
    if bt == bt_rle:
        return 1, last, bt, cSize
    if bt == bt_reserved:
        raise ZErr(E_CORRUPT)
    return cSize, last, bt, cSize


def block_size_max(d):
    return d.fParams["blockSizeMax"]


def alloc_lit_buffer(d, dst, dstCap, litSize, streaming, ews, splitNow):
    bsm = block_size_max(d)
    if (not streaming) and dstCap > bsm + WLEN + litSize + WLEN:
        d.litBuffer = dst + bsm + WLEN
        d.litBufferEnd = d.litBuffer + litSize
        d.litBufferLocation = IN_DST
    elif litSize <= LITEXTRA:
        d.litBuffer = d.litExtra
        d.litBufferEnd = d.litBuffer + litSize
        d.litBufferLocation = NOT_IN_DST
    else:
        if splitNow:
            d.litBuffer = dst + ews - litSize + LITEXTRA - WLEN
            d.litBufferEnd = d.litBuffer + litSize - LITEXTRA
        else:
            d.litBuffer = dst + ews - litSize
            d.litBufferEnd = dst + ews
        d.litBufferLocation = SPLIT


def decode_literals(d, src, srcSize, dst, dstCap, streaming):
    m = d.m
    if srcSize < 2:
        raise ZErr(E_CORRUPT)
    b0 = m.b[src]
    enc = b0 & 3
    bsm = block_size_max(d)
    if enc in (set_repeat, set_compressed):
        if enc == set_repeat and d.litEntropy == 0:
            raise ZErr(E_DICT_CORRUPT)
        if srcSize < 5:
            raise ZErr(E_CORRUPT)
        lhl = (b0 >> 2) & 3
        lhc = m.r(src, 4)
        ews = min(bsm, dstCap)
        single = 0
        if lhl in (0, 1):
            single = 1 if lhl == 0 else 0
            lhSize = 3
            litSize = (lhc >> 4) & 0x3FF
            litCSize = (lhc >> 14) & 0x3FF
        elif lhl == 2:
            lhSize = 4
            litSize = (lhc >> 4) & 0x3FFF
            litCSize = lhc >> 18
        else:
            lhSize = 5
            litSize = (lhc >> 4) & 0x3FFFF
            litCSize = (lhc >> 22) + (m.b[src + 4] << 10)
        if litSize > 0 and dst == 0:
            raise ZErr(E_DST_SMALL)
        if litSize > bsm:
            raise ZErr(E_CORRUPT)
        if not single and litSize < 6:
            raise ZErr(E_LITHDR)
        if litCSize + lhSize > srcSize:
            raise ZErr(E_CORRUPT)
        if ews < litSize:
            raise ZErr(E_DST_SMALL)
        alloc_lit_buffer(d, dst, dstCap, litSize, streaming, ews, 0)
        herr = None
        try:
            if enc == set_repeat:
                if single:
                    huf_1x_using(m, d.huf, d.litBuffer, litSize, src + lhSize, litCSize)
                else:
                    huf_4x_using(m, d.huf, d.litBuffer, litSize, src + lhSize, litCSize)
            else:
                if single:
                    huf_1x1_dctx(m, d.huf, d.litBuffer, litSize, src + lhSize, litCSize)
                else:
                    huf_4x_huf_only(m, d.huf, d.litBuffer, litSize, src + lhSize, litCSize)
        except ZErr as e:
            herr = e
        if d.litBufferLocation == SPLIT:
            m.copy(d.litExtra, d.litBufferEnd - LITEXTRA, LITEXTRA)
            m.copy(d.litBuffer + LITEXTRA - WLEN, d.litBuffer, litSize - LITEXTRA)
            d.litBuffer += LITEXTRA - WLEN
            d.litBufferEnd -= WLEN
        if herr is not None:
            raise ZErr(E_CORRUPT)
        d.litPtr = d.litBuffer
        d.litSize = litSize
        d.litEntropy = 1
        return litCSize + lhSize
    if enc == set_basic:
        lhl = (b0 >> 2) & 3
        ews = min(bsm, dstCap)
        if lhl in (0, 2):
            lhSize = 1
            litSize = b0 >> 3
        elif lhl == 1:
            lhSize = 2
            litSize = m.r(src, 2) >> 4
        else:
            lhSize = 3
            if srcSize < 3:
                raise ZErr(E_CORRUPT)
            litSize = m.r(src, 3) >> 4
        if litSize > 0 and dst == 0:
            raise ZErr(E_DST_SMALL)
        if litSize > bsm:
            raise ZErr(E_CORRUPT)
        if ews < litSize:
            raise ZErr(E_DST_SMALL)
        alloc_lit_buffer(d, dst, dstCap, litSize, streaming, ews, 1)
        if lhSize + litSize + WLEN > srcSize:
            if litSize + lhSize > srcSize:
                raise ZErr(E_CORRUPT)
            if d.litBufferLocation == SPLIT:
                m.copy(d.litBuffer, src + lhSize, litSize - LITEXTRA)
                m.copy(d.litExtra, src + lhSize + litSize - LITEXTRA, LITEXTRA)
            else:
                m.copy(d.litBuffer, src + lhSize, litSize)
            d.litPtr = d.litBuffer
            d.litSize = litSize
            return lhSize + litSize
        d.litPtr = src + lhSize
        d.litSize = litSize
        d.litBufferEnd = d.litPtr + litSize
        d.litBufferLocation = NOT_IN_DST
        return lhSize + litSize
    # set_rle
    lhl = (b0 >> 2) & 3
    ews = min(bsm, dstCap)
    if lhl in (0, 2):
        lhSize = 1
        litSize = b0 >> 3
    elif lhl == 1:
        lhSize = 2
        if srcSize < 3:
            raise ZErr(E_CORRUPT)
        litSize = m.r(src, 2) >> 4
    else:
        lhSize = 3
        if srcSize < 4:
            raise ZErr(E_CORRUPT)
        litSize = m.r(src, 3) >> 4
    if litSize > 0 and dst == 0:
        raise ZErr(E_DST_SMALL)
    if litSize > bsm:
        raise ZErr(E_CORRUPT)
    if ews < litSize:
        raise ZErr(E_DST_SMALL)
    alloc_lit_buffer(d, dst, dstCap, litSize, streaming, ews, 1)
    v = m.b[src + lhSize]
    if d.litBufferLocation == SPLIT:
        m.fill(d.litBuffer, v, litSize - LITEXTRA)
        m.fill(d.litExtra, v, LITEXTRA)
    else:
        m.fill(d.litBuffer, v, litSize)
    d.litPtr = d.litBuffer
    d.litSize = litSize
    return lhSize + 1


def build_seq_slot(d, which, typ, mx, maxLog, src, srcSize, base, bits, default, nbSeq):
    m = d.m
    cur = getattr(d, which)
    if typ == set_rle:
        if not srcSize:
            raise ZErr(E_SRC_WRONG)
        sy = m.b[src]
        if sy > mx:
            raise ZErr(E_CORRUPT)
        setattr(d, which, rle_seq_table(base[sy], bits[sy]))
        return 1
    if typ == set_basic:
        setattr(d, which, default)
        return 0
    if typ == set_repeat:
        if not d.fseEntropy:
            raise ZErr(E_CORRUPT)
        assert cur is not None
        return 0
    norm, msv, tl, hs = fse_read_ncount(bytes(m.b[src:src + srcSize]), mx)
    if tl > maxLog:
        raise ZErr(E_CORRUPT)
    setattr(d, which, build_seq_table(norm, msv, base, bits, tl))
    return hs


def decode_seq_headers(d, src, srcSize):
    m = d.m
    iend = src + srcSize
    ip = src
    if srcSize < 1:
        raise ZErr(E_SRC_WRONG)
    nbSeq = m.b[ip]
    ip += 1
    if nbSeq > 0x7F:
        if nbSeq == 0xFF:
            if ip + 2 > iend:
                raise ZErr(E_SRC_WRONG)
            nbSeq = m.r(ip, 2) + 0x7F00
            ip += 2
        else:
            if ip >= iend:
                raise ZErr(E_SRC_WRONG)
            nbSeq = ((nbSeq - 0x80) << 8) + m.b[ip]
            ip += 1
    if nbSeq == 0:
        if ip != iend:
            raise ZErr(E_CORRUPT)
        return 0, ip - src
    if ip + 1 > iend:
        raise ZErr(E_SRC_WRONG)
    b = m.b[ip]
    if b & 3:
        raise ZErr(E_CORRUPT)
    llt, oft, mlt = b >> 6, (b >> 4) & 3, (b >> 2) & 3
    ip += 1
    for which, typ, mx, lg, base, bits, dflt in (
            ("LLT", llt, MAXLL, LLFSELOG, LL_BASE, LL_BITS, LL_DEFAULT),
            ("OFT", oft, MAXOFF, OFFFSELOG, OF_BASE, OF_BITS, OF_DEFAULT),
            ("MLT", mlt, MAXML, MLFSELOG, ML_BASE, ML_BITS, ML_DEFAULT)):
        try:
            h = build_seq_slot(d, which, typ, mx, lg, ip, iend - ip, base, bits, dflt, nbSeq)
        except ZErr:
            raise ZErr(E_CORRUPT)
        ip += h
    return nbSeq, ip - src


class SeqState:
    __slots__ = ("bd", "sLL", "sOF", "sML", "tLL", "tOF", "tML", "prev")


def decode_sequence(m, st, isLast):
    bd = st.bd
    tl, to, tm = st.tLL, st.tOF, st.tML
    il, io, im = st.sLL, st.sOF, st.sML
    ml = tm.base[im]
    ll = tl.base[il]
    ofBase = to.base[io]
    llBits, mlBits, ofBits = tl.nba[il], tm.nba[im], to.nba[io]
    total = llBits + mlBits + ofBits
    prev = st.prev
    if ofBits > 1:
        off = ofBase + bit_read_fast(bd, ofBits)
        prev[2] = prev[1]
        prev[1] = prev[0]
        prev[0] = off
    else:
        ll0 = 1 if ll == 0 else 0
        if ofBits == 0:
            off = prev[ll0]
            prev[1] = prev[1 - ll0]
            prev[0] = off
        else:
            off = ofBase + ll0 + bit_read_fast(bd, 1)
            temp = ((prev[0] - 1) & M64) if off == 3 else prev[off]
            if temp == 0:
                temp = M64
            if off != 1:
                prev[2] = prev[1]
            prev[1] = prev[0]
            prev[0] = off = temp
    if mlBits > 0:
        ml += bit_read_fast(bd, mlBits)
    if total >= 57 - (9 + 9 + 8):
        bit_reload(m, bd)
    if llBits > 0:
        ll += bit_read_fast(bd, llBits)
    if not isLast:
        st.sLL = tl.ns[il] + bit_read(bd, tl.nbb[il])
        st.sML = tm.ns[im] + bit_read(bd, tm.nbb[im])
        st.sOF = to.ns[io] + bit_read(bd, to.nbb[io])
        bit_reload(m, bd)
    return ll, ml, off


# --- copias (semantica de bytes de zstd_internal.h / decompress_block.c)
def copy16(m, d, s):
    m.copy(d, s, 16)


def wildcopy(m, dst, src, length, overlap):
    diff = dst - src
    op, ip = dst, src
    oend = op + length
    if overlap and diff < WVEC:
        while True:
            m.copy(op, ip, 8)
            op += 8
            ip += 8
            if not op < oend:
                break
    else:
        m.copy(op, ip, 16)
        if 16 >= length:
            return
        op += 16
        ip += 16
        while True:
            m.copy(op, ip, 16)
            op += 16
            ip += 16
            m.copy(op, ip, 16)
            op += 16
            ip += 16
            if not op < oend:
                break


DEC32 = [0, 1, 2, 1, 4, 4, 4, 4]
DEC64 = [8, 8, 8, 7, 8, 9, 10, 11]


def overlap_copy8(m, op, ip, off):
    if off < 8:
        sub2 = DEC64[off]
        for k in range(4):
            m.b[op + k] = m.b[ip + k]
        ip += DEC32[off]
        m.copy(op + 4, ip, 4)
        ip -= sub2
    else:
        m.copy(op, ip, 8)
    return op + 8, ip + 8


def bytecopy(m, op, ip, n):
    for k in range(n):
        m.b[op + k] = m.b[ip + k]


def safecopy(m, op, oend_w, ip, length, overlap):
    diff = op - ip
    oend = op + length
    if length < 8:
        bytecopy(m, op, ip, oend - op)
        return
    if overlap:
        op, ip = overlap_copy8(m, op, ip, diff)
        length -= 8
    if oend <= oend_w:
        wildcopy(m, op, ip, length, overlap)
        return
    if op <= oend_w:
        wildcopy(m, op, ip, oend_w - op, overlap)
        ip += oend_w - op
        op = oend_w
    bytecopy(m, op, ip, oend - op)


def safecopy_dst_before_src(m, op, ip, length):
    diff = op - ip
    oend = op + length
    if length < 8 or diff > -8:
        bytecopy(m, op, ip, oend - op)
        return
    if op <= oend - WLEN and diff < -WVEC:
        wildcopy(m, op, ip, oend - WLEN - op, False)
        ip += oend - WLEN - op
        op = oend - WLEN
    bytecopy(m, op, ip, oend - op)


class Lit:
    __slots__ = ("p",)


def _match_tail(m, op, oLitEnd, ll, ml, off, oend_w, prefixStart, vStart, dictEnd, fast):
    seqLen = ll + ml
    match = oLitEnd - off
    if off > oLitEnd - prefixStart:
        if off > oLitEnd - vStart:
            raise ZErr(E_CORRUPT)
        match = dictEnd + (match - prefixStart)
        if match + ml <= dictEnd:
            m.copy(oLitEnd, match, ml)
            return seqLen
        length1 = dictEnd - match
        m.copy(oLitEnd, match, length1)
        op = oLitEnd + length1
        ml -= length1
        match = prefixStart
    if not fast:
        safecopy(m, op, oend_w, match, ml, True)
        return seqLen
    if off >= WVEC:
        wildcopy(m, op, match, ml, False)
        return seqLen
    op, match = overlap_copy8(m, op, match, off)
    if ml > 8:
        wildcopy(m, op, match, ml - 8, True)
    return seqLen


def exec_seq_end(m, op, oend, seq, lit, litLimit, prefixStart, vStart, dictEnd):
    ll, ml, off = seq
    oLitEnd = op + ll
    seqLen = ll + ml
    iLitEnd = lit.p + ll
    oend_w = oend - WLEN
    if seqLen > oend - op:
        raise ZErr(E_DST_SMALL)
    if ll > ((litLimit - lit.p) & M64):
        raise ZErr(E_CORRUPT)
    safecopy(m, op, oend_w, lit.p, ll, False)
    lit.p = iLitEnd
    return _match_tail(m, oLitEnd, oLitEnd, ll, ml, off, oend_w, prefixStart, vStart, dictEnd, False)


def exec_seq_end_split(m, op, oend, oend_w, seq, lit, litLimit, prefixStart, vStart, dictEnd):
    ll, ml, off = seq
    oLitEnd = op + ll
    seqLen = ll + ml
    iLitEnd = lit.p + ll
    if seqLen > oend - op:
        raise ZErr(E_DST_SMALL)
    if ll > ((litLimit - lit.p) & M64):
        raise ZErr(E_CORRUPT)
    if op > lit.p and op < lit.p + ll:
        raise ZErr(E_DST_SMALL)
    safecopy_dst_before_src(m, op, lit.p, ll)
    lit.p = iLitEnd
    return _match_tail(m, oLitEnd, oLitEnd, ll, ml, off, oend_w, prefixStart, vStart, dictEnd, False)


def exec_seq(m, op, oend, seq, lit, litLimit, prefixStart, vStart, dictEnd):
    ll, ml, off = seq
    oLitEnd = op + ll
    oMatchEnd = op + ll + ml
    oend_w = oend - WLEN
    iLitEnd = lit.p + ll
    if iLitEnd > litLimit or oMatchEnd > oend_w:
        return exec_seq_end(m, op, oend, seq, lit, litLimit, prefixStart, vStart, dictEnd)
    copy16(m, op, lit.p)
    if ll > 16:
        wildcopy(m, op + 16, lit.p + 16, ll - 16, False)
    lit.p = iLitEnd
    return _match_tail(m, oLitEnd, oLitEnd, ll, ml, off, oend_w, prefixStart, vStart, dictEnd, True)


def exec_seq_split(m, op, oend, oend_w, seq, lit, litLimit, prefixStart, vStart, dictEnd):
    ll, ml, off = seq
    oLitEnd = op + ll
    oMatchEnd = op + ll + ml
    iLitEnd = lit.p + ll
    if iLitEnd > litLimit or oMatchEnd > oend_w:
        return exec_seq_end_split(m, op, oend, oend_w, seq, lit, litLimit, prefixStart, vStart, dictEnd)
    copy16(m, op, lit.p)
    if ll > 16:
        wildcopy(m, op + 16, lit.p + 16, ll - 16, False)
    lit.p = iLitEnd
    return _match_tail(m, oLitEnd, oLitEnd, ll, ml, off, oend_w, prefixStart, vStart, dictEnd, True)


def _init_seq_state(d, ip, size):
    m = d.m
    st = SeqState()
    st.prev = list(d.rep)
    st.bd = BitD()
    try:
        bit_init(m, st.bd, ip, size)
    except ZErr:
        raise ZErr(E_CORRUPT)
    st.tLL, st.tOF, st.tML = d.LLT, d.OFT, d.MLT
    st.sLL = bit_read(st.bd, st.tLL.log)
    bit_reload(m, st.bd)
    st.sOF = bit_read(st.bd, st.tOF.log)
    bit_reload(m, st.bd)
    st.sML = bit_read(st.bd, st.tML.log)
    bit_reload(m, st.bd)
    return st


def _save_rep(d, st):
    d.rep = [v & M32 for v in st.prev]


def seqs_body(d, dst, maxDst, ip, seqSize, nbSeq):
    m = d.m
    ostart = dst
    oend = ostart + maxDst if d.litBufferLocation == NOT_IN_DST else d.litBuffer
    op = ostart
    lit = Lit()
    lit.p = d.litPtr
    litEnd = lit.p + d.litSize
    ps, vs, de = d.prefixStart, d.virtualStart, d.dictEnd
    if nbSeq:
        d.fseEntropy = 1
        st = _init_seq_state(d, ip, seqSize)
        while nbSeq:
            seq = decode_sequence(m, st, nbSeq == 1)
            op += exec_seq(m, op, oend, seq, lit, litEnd, ps, vs, de)
            nbSeq -= 1
        if not bit_end(st.bd):
            raise ZErr(E_CORRUPT)
        _save_rep(d, st)
    last = litEnd - lit.p
    if last > oend - op:
        raise ZErr(E_DST_SMALL)
    m.copy(op, lit.p, last)
    op += last
    return op - ostart


def seqs_body_split(d, dst, maxDst, ip, seqSize, nbSeq):
    m = d.m
    ostart = dst
    oend = ostart + maxDst
    op = ostart
    lit = Lit()
    lit.p = d.litPtr
    litBufferEnd = d.litBufferEnd
    ps, vs, de = d.prefixStart, d.virtualStart, d.dictEnd
    if nbSeq:
        d.fseEntropy = 1
        st = _init_seq_state(d, ip, seqSize)
        seq = (0, 0, 0)
        while nbSeq:
            seq = decode_sequence(m, st, nbSeq == 1)
            if lit.p + seq[0] > d.litBufferEnd:
                break
            op += exec_seq_split(m, op, oend, lit.p + seq[0] - WLEN, seq, lit, litBufferEnd, ps, vs, de)
            nbSeq -= 1
        if nbSeq > 0:
            left = d.litBufferEnd - lit.p
            if left:
                if left > oend - op:
                    raise ZErr(E_DST_SMALL)
                safecopy_dst_before_src(m, op, lit.p, left)
                seq = (seq[0] - left, seq[1], seq[2])
                op += left
            lit.p = d.litExtra
            litBufferEnd = d.litExtra + LITEXTRA
            d.litBufferLocation = NOT_IN_DST
            op += exec_seq(m, op, oend, seq, lit, litBufferEnd, ps, vs, de)
            nbSeq -= 1
        if nbSeq > 0:
            while nbSeq:
                seq = decode_sequence(m, st, nbSeq == 1)
                op += exec_seq(m, op, oend, seq, lit, litBufferEnd, ps, vs, de)
                nbSeq -= 1
        if nbSeq:
            raise ZErr(E_CORRUPT)
        if not bit_end(st.bd):
            raise ZErr(E_CORRUPT)
        _save_rep(d, st)
    if d.litBufferLocation == SPLIT:
        last = litBufferEnd - lit.p
        if last > oend - op:
            raise ZErr(E_DST_SMALL)
        m.copy(op, lit.p, last)
        op += last
        lit.p = d.litExtra
        litBufferEnd = d.litExtra + LITEXTRA
        d.litBufferLocation = NOT_IN_DST
    last = litBufferEnd - lit.p
    if last > oend - op:
        raise ZErr(E_DST_SMALL)
    m.copy(op, lit.p, last)
    op += last
    return op - ostart


def seqs_long(d, dst, maxDst, ip, seqSize, nbSeq):
    m = d.m
    ostart = dst
    oend = d.litBuffer if d.litBufferLocation == IN_DST else ostart + maxDst
    op = ostart
    lit = Lit()
    lit.p = d.litPtr
    litBufferEnd = d.litBufferEnd
    ps, vs, de = d.prefixStart, d.virtualStart, d.dictEnd
    if nbSeq:
        seqs = [None] * 8
        adv = min(nbSeq, 8)
        d.fseEntropy = 1
        st = _init_seq_state(d, ip, seqSize)
        seqNb = 0
        while seqNb < adv:
            seqs[seqNb] = decode_sequence(m, st, seqNb == nbSeq - 1)
            seqNb += 1

        def run(k, cur):
            nonlocal op, litBufferEnd
            s = seqs[k]
            if d.litBufferLocation == SPLIT and lit.p + s[0] > d.litBufferEnd:
                left = d.litBufferEnd - lit.p
                if left:
                    if left > oend - op:
                        raise ZErr(E_DST_SMALL)
                    safecopy_dst_before_src(m, op, lit.p, left)
                    s = (s[0] - left, s[1], s[2])
                    seqs[k] = s
                    op += left
                lit.p = d.litExtra
                litBufferEnd = d.litExtra + LITEXTRA
                d.litBufferLocation = NOT_IN_DST
                n = exec_seq(m, op, oend, s, lit, litBufferEnd, ps, vs, de)
            else:
                if d.litBufferLocation == SPLIT:
                    n = exec_seq_split(m, op, oend, lit.p + s[0] - WLEN, s, lit, litBufferEnd, ps, vs, de)
                else:
                    n = exec_seq(m, op, oend, s, lit, litBufferEnd, ps, vs, de)
            if cur is not None:
                seqs[cur[0]] = cur[1]
            op += n
        while seqNb < nbSeq:
            sq = decode_sequence(m, st, seqNb == nbSeq - 1)
            run((seqNb - 8) & 7, (seqNb & 7, sq))
            seqNb += 1
        if not bit_end(st.bd):
            raise ZErr(E_CORRUPT)
        seqNb -= adv
        while seqNb < nbSeq:
            run(seqNb & 7, None)
            seqNb += 1
        _save_rep(d, st)
    if d.litBufferLocation == SPLIT:
        last = litBufferEnd - lit.p
        if last > oend - op:
            raise ZErr(E_DST_SMALL)
        m.copy(op, lit.p, last)
        op += last
        lit.p = d.litExtra
        litBufferEnd = d.litExtra + LITEXTRA
    last = litBufferEnd - lit.p
    if last > oend - op:
        raise ZErr(E_DST_SMALL)
    m.copy(op, lit.p, last)
    op += last
    return op - ostart


def decompress_block(d, dst, dstCap, src, srcSize, streaming):
    ip = src
    if srcSize > block_size_max(d):
        raise ZErr(E_SRC_WRONG)
    lc = decode_literals(d, src, srcSize, dst, dstCap, streaming)
    ip += lc
    srcSize -= lc
    bsm = min(dstCap, block_size_max(d))
    history = (dst + bsm) - d.virtualStart
    usePrefetch = d.ddictIsCold
    nbSeq, sh = decode_seq_headers(d, ip, srcSize)
    ip += sh
    srcSize -= sh
    if (dst == 0 or dstCap == 0) and nbSeq > 0:
        raise ZErr(E_DST_SMALL)
    if (not usePrefetch) and history > (1 << 24) and nbSeq > 8:
        t = d.OFT
        share = 0
        for u in range(1 << t.log):
            if t.nba[u] > 22:
                share += 1
        share <<= (OFFFSELOG - t.log)
        usePrefetch = 1 if share >= 7 else 0
    d.ddictIsCold = 0
    if DBG_FORCE_LONG and nbSeq:
        usePrefetch = 1
    if usePrefetch:
        return seqs_long(d, dst, dstCap, ip, srcSize, nbSeq)
    if d.litBufferLocation == SPLIT:
        return seqs_body_split(d, dst, dstCap, ip, srcSize, nbSeq)
    return seqs_body(d, dst, dstCap, ip, srcSize, nbSeq)


# ---------------------------------------------------------------- xxhash.h (XXH64, semilla 0)
P64_1 = 0x9E3779B185EBCA87
P64_2 = 0xC2B2AE3D27D4EB4F
P64_3 = 0x165667B19E3779F9
P64_4 = 0x85EBCA77C2B2AE63
P64_5 = 0x27D4EB2F165667C5


def _rotl(x, r):
    return ((x << r) | (x >> (64 - r))) & M64


def _xround(acc, v):
    acc = (acc + v * P64_2) & M64
    return (_rotl(acc, 31) * P64_1) & M64


def _xmerge(acc, v):
    acc ^= _xround(0, v)
    return (acc * P64_1 + P64_4) & M64


def xxh64(data, seed=0):
    n = len(data)
    p = 0
    if n >= 32:
        v1 = (seed + P64_1 + P64_2) & M64
        v2 = (seed + P64_2) & M64
        v3 = seed
        v4 = (seed - P64_1) & M64
        while p + 32 <= n:
            v1 = _xround(v1, int.from_bytes(data[p:p + 8], "little"))
            v2 = _xround(v2, int.from_bytes(data[p + 8:p + 16], "little"))
            v3 = _xround(v3, int.from_bytes(data[p + 16:p + 24], "little"))
            v4 = _xround(v4, int.from_bytes(data[p + 24:p + 32], "little"))
            p += 32
        h = (_rotl(v1, 1) + _rotl(v2, 7) + _rotl(v3, 12) + _rotl(v4, 18)) & M64
        for v in (v1, v2, v3, v4):
            h = _xmerge(h, v)
    else:
        h = (seed + P64_5) & M64
    h = (h + n) & M64
    while p + 8 <= n:
        h ^= _xround(0, int.from_bytes(data[p:p + 8], "little"))
        h = (_rotl(h, 27) * P64_1 + P64_4) & M64
        p += 8
    if p + 4 <= n:
        h ^= (int.from_bytes(data[p:p + 4], "little") * P64_1) & M64
        h = (_rotl(h, 23) * P64_2 + P64_3) & M64
        p += 4
    while p < n:
        h ^= (data[p] * P64_5) & M64
        h = (_rotl(h, 11) * P64_1) & M64
        p += 1
    h ^= h >> 33
    h = (h * P64_2) & M64
    h ^= h >> 29
    h = (h * P64_3) & M64
    h ^= h >> 32
    return h


# ---------------------------------------------------------------- zstd_decompress.c
DS_GETFH, DS_DECFH, DS_BH, DS_BLOCK, DS_LAST, DS_CHK, DS_SKIPH, DS_SKIP = range(8)
ZS_INIT, ZS_LOADH, ZS_READ, ZS_LOAD, ZS_FLUSH = range(5)
NFP_MAX = 16


def frame_header_size(m, src, srcSize):
    if srcSize < 5:
        raise ZErr(E_SRC_WRONG)
    fhd = m.b[src + 4]
    did = fhd & 3
    single = (fhd >> 5) & 1
    fcs = fhd >> 6
    return 5 + (0 if single else 1) + DID_FIELD[did] + FCS_FIELD[fcs] + (1 if single and not fcs else 0)


def get_frame_header(m, src, srcSize):
    """ZSTD_getFrameHeader_advanced: devuelve (0, params) o (bytes_necesarios, None)."""
    if srcSize < 5:
        if srcSize > 0:
            n = min(4, srcSize)
            hb = bytearray(MAGIC.to_bytes(4, "little"))
            hb[:n] = m.b[src:src + n]
            if int.from_bytes(hb, "little") != MAGIC:
                hb = bytearray(SKIP_START.to_bytes(4, "little"))
                hb[:n] = m.b[src:src + n]
                if (int.from_bytes(hb, "little") & SKIP_MASK) != SKIP_START:
                    raise ZErr(E_PREFIX)
        return 5, None
    mg = m.r(src, 4)
    if mg != MAGIC:
        if (mg & SKIP_MASK) == SKIP_START:
            if srcSize < 8:
                return 8, None
            return 0, {"frameType": 1, "dictID": mg - SKIP_START, "headerSize": 8,
                       "frameContentSize": m.r(src + 4, 4), "windowSize": 0,
                       "blockSizeMax": 0, "checksumFlag": 0}
        raise ZErr(E_PREFIX)
    fhs = frame_header_size(m, src, srcSize)
    if srcSize < fhs:
        return fhs, None
    fhd = m.b[src + 4]
    pos = 5
    didc = fhd & 3
    chk = (fhd >> 2) & 1
    single = (fhd >> 5) & 1
    fcsID = fhd >> 6
    ws = 0
    dictID = 0
    fcs = CS_UNKNOWN
    if fhd & 0x08:
        raise ZErr(E_FP_UNSUP)
    if not single:
        wl = m.b[src + pos]
        pos += 1
        wlog = (wl >> 3) + 10
        if wlog > WINDOWLOG_MAX:
            raise ZErr(E_WINDOW)
        ws = 1 << wlog
        ws += (ws >> 3) * (wl & 7)
    if didc == 1:
        dictID = m.b[src + pos]
        pos += 1
    elif didc == 2:
        dictID = m.r(src + pos, 2)
        pos += 2
    elif didc == 3:
        dictID = m.r(src + pos, 4)
        pos += 4
    if fcsID == 0:
        if single:
            fcs = m.b[src + pos]
    elif fcsID == 1:
        fcs = m.r(src + pos, 2) + 256
    elif fcsID == 2:
        fcs = m.r(src + pos, 4)
    else:
        fcs = m.r(src + pos, 8)
    if single:
        ws = fcs
    return 0, {"frameType": 0, "frameContentSize": fcs, "windowSize": ws,
               "blockSizeMax": min(ws, BLOCKSIZE_MAX), "dictID": dictID,
               "checksumFlag": chk, "headerSize": fhs}


def decode_frame_header(d, src, headerSize):
    r, fp = get_frame_header(d.m, src, headerSize)
    if r > 0:
        raise ZErr(E_SRC_WRONG)
    d.fParams = fp
    if fp["dictID"] and d.dictID != fp["dictID"]:
        raise ZErr(E_DICT_WRONG)
    d.validateChecksum = 1 if fp["checksumFlag"] else 0
    if d.validateChecksum:
        d.xxh = bytearray()
    d.processedCSize += headerSize


def read_skippable_size(m, src, srcSize):
    if srcSize < 8:
        raise ZErr(E_SRC_WRONG)
    s32 = m.r(src + 4, 4)
    if ((s32 + 8) & M32) < s32:
        raise ZErr(E_FP_UNSUP)
    sk = 8 + s32
    if sk > srcSize:
        raise ZErr(E_SRC_WRONG)
    return sk


def find_frame_csize(m, src, srcSize):
    """ZSTD_findFrameCompressedSize: tamano o error (como entero con is_error)."""
    try:
        if srcSize >= 8 and (m.r(src, 4) & SKIP_MASK) == SKIP_START:
            return read_skippable_size(m, src, srcSize)
        r, zfh = get_frame_header(m, src, srcSize)
        if r > 0:
            return ERROR(E_SRC_WRONG)
        ip = src + zfh["headerSize"]
        rem = srcSize - zfh["headerSize"]
        while True:
            cb, last, bt, _ = get_cblock_size(m, ip, rem)
            if 3 + cb > rem:
                return ERROR(E_SRC_WRONG)
            ip += 3 + cb
            rem -= 3 + cb
            if last:
                break
        if zfh["checksumFlag"]:
            if rem < 4:
                return ERROR(E_SRC_WRONG)
            ip += 4
        return ip - src
    except ZErr as e:
        return ERROR(e.code)


def copy_raw_block(m, dst, dstCap, src, srcSize):
    if srcSize > dstCap:
        raise ZErr(E_DST_SMALL)
    if dst == 0:
        if srcSize == 0:
            return 0
        raise ZErr(E_DST_NULL)
    m.copy(dst, src, srcSize)
    return srcSize


def set_rle_block(m, dst, dstCap, b, n):
    if n > dstCap:
        raise ZErr(E_DST_SMALL)
    if dst == 0:
        if n == 0:
            return 0
        raise ZErr(E_DST_NULL)
    m.fill(dst, b, n)
    return n


def decompress_frame(d, dst, dstCap, src, srcSize):
    """ZSTD_decompressFrame; devuelve (decodificado, nuevo_src, nuevo_srcSize)."""
    m = d.m
    ip = src
    ostart = dst
    oend = ostart + dstCap if dstCap != 0 else ostart
    op = ostart
    rem = srcSize
    if rem < 6 + 3:
        raise ZErr(E_SRC_WRONG)
    fhs = frame_header_size(m, ip, 5)
    if rem < fhs + 3:
        raise ZErr(E_SRC_WRONG)
    decode_frame_header(d, ip, fhs)
    ip += fhs
    rem -= fhs
    while True:
        oBlockEnd = oend
        cb, last, bt, orig = get_cblock_size(m, ip, rem)
        ip += 3
        rem -= 3
        if cb > rem:
            raise ZErr(E_SRC_WRONG)
        if ip >= op and ip < oBlockEnd:
            oBlockEnd = op + (ip - op)
        if bt == bt_compressed:
            ds = decompress_block(d, op, oBlockEnd - op, ip, cb, False)
        elif bt == bt_raw:
            ds = copy_raw_block(m, op, oend - op, ip, cb)
        elif bt == bt_rle:
            ds = set_rle_block(m, op, oBlockEnd - op, m.b[ip], orig)
        else:
            raise ZErr(E_CORRUPT)
        if d.validateChecksum:
            d.xxh += m.b[op:op + ds]
        if ds:
            op += ds
        ip += cb
        rem -= cb
        if last:
            break
    if d.fParams["frameContentSize"] != CS_UNKNOWN:
        if op - ostart != d.fParams["frameContentSize"]:
            raise ZErr(E_CORRUPT)
    if d.fParams["checksumFlag"]:
        if rem < 4:
            raise ZErr(E_CHECKSUM)
        calc = xxh64(bytes(d.xxh)) & M32
        if m.r(ip, 4) != calc:
            raise ZErr(E_CHECKSUM)
        ip += 4
        rem -= 4
    return op - ostart, ip, rem


def decompress_multi_frame(d, dst, dstCap, src, srcSize):
    m = d.m
    dststart = dst
    more = 0
    while srcSize >= 5:
        if srcSize >= 4:
            mg = m.r(src, 4)
            if (mg & SKIP_MASK) == SKIP_START:
                sk = read_skippable_size(m, src, srcSize)
                src += sk
                srcSize -= sk
                continue
        decompress_begin(d)
        check_continuity(d, dst, dstCap)
        try:
            res, src, srcSize = decompress_frame(d, dst, dstCap, src, srcSize)
        except ZErr as e:
            if e.code == E_PREFIX and more == 1:
                raise ZErr(E_SRC_WRONG)
            raise
        if res != 0:
            dst += res
        dstCap -= res
        more = 1
    if srcSize:
        raise ZErr(E_SRC_WRONG)
    return dst - dststart


def next_src_size_with_input(d, inputSize):
    if not (d.stage == DS_BLOCK or d.stage == DS_LAST):
        return d.expected
    if d.bType != bt_raw:
        return d.expected
    return max(1, min(inputSize, d.expected))


def decompress_continue(d, dst, dstCap, src, srcSize):
    m = d.m
    if srcSize != next_src_size_with_input(d, srcSize):
        raise ZErr(E_SRC_WRONG)
    check_continuity(d, dst, dstCap)
    d.processedCSize += srcSize
    st = d.stage
    if st == DS_GETFH:
        if (m.r(src, 4) & SKIP_MASK) == SKIP_START:
            m.copy(d.hdr, src, srcSize)
            d.expected = 8 - srcSize
            d.stage = DS_SKIPH
            return 0
        d.headerSize = frame_header_size(m, src, srcSize)
        m.copy(d.hdr, src, srcSize)
        d.expected = d.headerSize - srcSize
        d.stage = DS_DECFH
        return 0
    if st == DS_DECFH:
        m.copy(d.hdr + (d.headerSize - srcSize), src, srcSize)
        decode_frame_header(d, d.hdr, d.headerSize)
        d.expected = 3
        d.stage = DS_BH
        return 0
    if st == DS_BH:
        cb, last, bt, orig = get_cblock_size(m, src, 3)
        if cb > d.fParams["blockSizeMax"]:
            raise ZErr(E_CORRUPT)
        d.expected = cb
        d.bType = bt
        d.rleSize = orig
        if cb:
            d.stage = DS_LAST if last else DS_BLOCK
            return 0
        if last:
            if d.fParams["checksumFlag"]:
                d.expected = 4
                d.stage = DS_CHK
            else:
                d.expected = 0
                d.stage = DS_GETFH
        else:
            d.expected = 3
            d.stage = DS_BH
        return 0
    if st in (DS_LAST, DS_BLOCK):
        if d.bType == bt_compressed:
            rSize = decompress_block(d, dst, dstCap, src, srcSize, True)
            d.expected = 0
        elif d.bType == bt_raw:
            rSize = copy_raw_block(m, dst, dstCap, src, srcSize)
            d.expected -= rSize
        elif d.bType == bt_rle:
            rSize = set_rle_block(m, dst, dstCap, m.b[src], d.rleSize)
            d.expected = 0
        else:
            raise ZErr(E_CORRUPT)
        if rSize > d.fParams["blockSizeMax"]:
            raise ZErr(E_CORRUPT)
        d.decodedSize += rSize
        if d.validateChecksum:
            d.xxh += m.b[dst:dst + rSize]
        d.previousDstEnd = dst + rSize
        if d.expected > 0:
            return rSize
        if d.stage == DS_LAST:
            fcs = d.fParams["frameContentSize"]
            if fcs != CS_UNKNOWN and d.decodedSize != fcs:
                raise ZErr(E_CORRUPT)
            if d.fParams["checksumFlag"]:
                d.expected = 4
                d.stage = DS_CHK
            else:
                d.expected = 0
                d.stage = DS_GETFH
        else:
            d.stage = DS_BH
            d.expected = 3
        return rSize
    if st == DS_CHK:
        if d.validateChecksum:
            if m.r(src, 4) != (xxh64(bytes(d.xxh)) & M32):
                raise ZErr(E_CHECKSUM)
        d.expected = 0
        d.stage = DS_GETFH
        return 0
    if st == DS_SKIPH:
        m.copy(d.hdr + (8 - srcSize), src, srcSize)
        d.expected = m.r(d.hdr + 4, 4)
        d.stage = DS_SKIP
        return 0
    if st == DS_SKIP:
        d.expected = 0
        d.stage = DS_GETFH
        return 0
    raise ZErr(E_GENERIC)


def decoding_buffer_size(ws, fcs, bsm):
    bs = min(min(ws, BLOCKSIZE_MAX), bsm)
    need = ws + bs * 2 + WLEN * 2
    n = min(fcs, need)
    if n > M64:
        raise ZErr(E_WINDOW)
    return n


def continue_stream(d, op, oend, src, srcSize):
    skip = d.stage == DS_SKIP
    dstSize = 0 if skip else d.outBuffSize - d.outStart
    ds = decompress_continue(d, d.outBuff + d.outStart, dstSize, src, srcSize)
    if not ds and not skip:
        d.streamStage = ZS_READ
    else:
        d.outEnd = d.outStart + ds
        d.streamStage = ZS_FLUSH


def decompress_stream(d, obuf, ibuf):
    """ZSTD_decompressStream.  obuf/ibuf: listas [dst, size, pos]."""
    m = d.m
    src = ibuf[0]
    istart = src + ibuf[2] if ibuf[2] != 0 else src
    iend = src + ibuf[1] if ibuf[1] != 0 else src
    ip = istart
    dst = obuf[0]
    ostart = dst + obuf[2] if obuf[2] != 0 else dst
    oend = dst + obuf[1] if obuf[1] != 0 else dst
    op = ostart
    more = True
    if ibuf[2] > ibuf[1]:
        raise ZErr(E_SRC_WRONG)
    if obuf[2] > obuf[1]:
        raise ZErr(E_DST_SMALL)
    while more:
        ss = d.streamStage
        if ss == ZS_INIT:
            d.streamStage = ZS_LOADH
            d.lhSize = d.inPos = d.outStart = d.outEnd = 0
            d.hostageByte = 0
            ss = ZS_LOADH
        if ss == ZS_LOADH:
            hSize, fp = get_frame_header(m, d.hdr, d.lhSize)
            if fp is not None:
                d.fParams = fp
            if hSize != 0:
                toLoad = hSize - d.lhSize
                remIn = iend - ip
                if toLoad > remIn:
                    if remIn > 0:
                        m.copy(d.hdr + d.lhSize, ip, remIn)
                        d.lhSize += remIn
                    ibuf[2] = ibuf[1]
                    get_frame_header(m, d.hdr, d.lhSize)
                    return (max(6, hSize) - d.lhSize) + 3
                m.copy(d.hdr + d.lhSize, ip, toLoad)
                d.lhSize = hSize
                ip += toLoad
                continue
            fp = d.fParams
            if (fp["frameContentSize"] != CS_UNKNOWN and fp["frameType"] != 1
                    and (oend - op) >= fp["frameContentSize"]):
                cSize = find_frame_csize(m, istart, iend - istart)
                if cSize <= iend - istart:
                    ds = decompress_multi_frame(d, op, oend - op, istart, cSize)
                    ip = istart + cSize
                    op = op + ds if op else op
                    d.expected = 0
                    d.streamStage = ZS_INIT
                    more = False
                    break
            decompress_begin(d)
            d.fParams = fp
            if (m.r(d.hdr, 4) & SKIP_MASK) == SKIP_START:
                d.expected = m.r(d.hdr + 4, 4)
                d.stage = DS_SKIP
            else:
                decode_frame_header(d, d.hdr, d.lhSize)
                d.expected = 3
                d.stage = DS_BH
            fp = d.fParams
            fp["windowSize"] = max(fp["windowSize"], 1 << 10)
            if fp["windowSize"] > d.maxWindowSize:
                raise ZErr(E_WINDOW)
            needIn = max(fp["blockSizeMax"], 4)
            needOut = decoding_buffer_size(fp["windowSize"], fp["frameContentSize"], fp["blockSizeMax"])
            if (d.inBuffSize + d.outBuffSize) >= (needIn + needOut) * 3:
                d.oversizedDuration += 1
            else:
                d.oversizedDuration = 0
            tooSmall = d.inBuffSize < needIn or d.outBuffSize < needOut
            tooLarge = d.oversizedDuration >= 128
            if tooSmall or tooLarge:
                d.inBuff = m.alloc(needIn + needOut)
                d.inBuffSize = needIn
                d.outBuff = d.inBuff + d.inBuffSize
                d.outBuffSize = needOut
            d.streamStage = ZS_READ
            ss = ZS_READ
        if ss == ZS_READ:
            need = next_src_size_with_input(d, iend - ip)
            if need == 0:
                d.streamStage = ZS_INIT
                more = False
                break
            if iend - ip >= need:
                continue_stream(d, op, oend, ip, need)
                ip += need
                continue
            if ip == iend:
                more = False
                break
            d.streamStage = ZS_LOAD
            ss = ZS_LOAD
        if ss == ZS_LOAD:
            need = d.expected
            toLoad = need - d.inPos
            skip = d.stage == DS_SKIP
            if skip:
                loaded = min(toLoad, iend - ip)
            else:
                if toLoad > d.inBuffSize - d.inPos:
                    raise ZErr(E_CORRUPT)
                loaded = min(toLoad, iend - ip)
                if loaded > 0:
                    m.copy(d.inBuff + d.inPos, ip, loaded)
            if loaded != 0:
                ip += loaded
                d.inPos += loaded
            if loaded < toLoad:
                more = False
                break
            d.inPos = 0
            continue_stream(d, op, oend, d.inBuff, need)
            continue
        if ss == ZS_FLUSH:
            toFlush = d.outEnd - d.outStart
            fl = min(oend - op, toFlush)
            if fl > 0:
                m.copy(op, d.outBuff + d.outStart, fl)
            op = op + fl if op else op
            d.outStart += fl
            if fl == toFlush:
                d.streamStage = ZS_READ
                if (d.outBuffSize < d.fParams["frameContentSize"]
                        and d.outStart + d.fParams["blockSizeMax"] > d.outBuffSize):
                    d.outStart = d.outEnd = 0
                continue
            more = False
            break
    ibuf[2] = ip - ibuf[0]
    obuf[2] = op - obuf[0]
    if ip == istart and op == ostart:
        d.noForwardProgress += 1
        if d.noForwardProgress >= NFP_MAX:
            if op == oend:
                raise ZErr(E_NFP_DEST)
            if ip == iend:
                raise ZErr(E_NFP_IN)
    else:
        d.noForwardProgress = 0
    hint = d.expected
    if not hint:
        if d.outEnd == d.outStart:
            if d.hostageByte:
                if ibuf[2] >= ibuf[1]:
                    d.streamStage = ZS_READ
                    return 1
                ibuf[2] += 1
            return 0
        if not d.hostageByte:
            ibuf[2] -= 1
            d.hostageByte = 1
        return 1
    st = d.stage
    hint += 3 * (1 if st == DS_BLOCK else 0)
    hint -= d.inPos
    return hint


# ---------------------------------------------------------------- conductor (crate zstd + std)
BUFREADER_CAP = DSTREAM_IN     # zstd_safe::DCtx::in_size() = 131075
COPY_BUF = 8192                 # std::io::DEFAULT_BUF_SIZE (stack_buffer_copy)


class IncompleteFrame(Exception):
    pass


def decode_all(data):
    """zstd::stream::decode_all(data).  Devuelve bytes o lanza ZErr/IncompleteFrame."""
    m = Mem()
    d = new_dctx(m)
    n = len(data)
    srcA = m.alloc(max(n, 1))
    m.b[srcA:srcA + n] = data
    spos = 0                            # posicion del lector &[u8]
    br = m.alloc(BUFREADER_CAP)         # BufReader: buffer, pos, filled
    br_pos = br_filled = 0
    cbuf = m.alloc(COPY_BUF)
    out = bytearray()
    state = 0                           # 0 Reading, 1 PastEof, 2 Finished
    finished_frame = False

    def fill_buf():
        nonlocal br_pos, br_filled, spos
        if br_pos >= br_filled:
            k = min(BUFREADER_CAP, n - spos)
            m.copy(br, srcA + spos, k)
            spos += k
            br_pos = 0
            br_filled = k
        return br + br_pos, br_filled - br_pos

    while True:
        # Read::read(&mut decoder, &mut buf[..8192])
        first = True
        got = None
        while got is None:
            if state == 0:
                if first:
                    ia, il = cbuf, 0         # b"" : puntero colgante no nulo
                else:
                    ia, il = fill_buf()
                if not first and il == 0:
                    state = 1
                    continue
                first = False
                ib = [ia, il, 0]
                ob = [cbuf, COPY_BUF, 0]
                if finished_frame and il != 0:
                    d.streamStage = ZS_INIT      # reset(SessionOnly)
                    d.noForwardProgress = 0
                    finished_frame = False
                hint = decompress_stream(d, ob, ib)
                if hint == 0:
                    finished_frame = True
                br_pos = min(br_pos + ib[2], br_filled)
                if ob[2] > 0:
                    got = ob[2]
            elif state == 1:
                if not finished_frame:
                    raise IncompleteFrame()
                state = 2
                got = 0
            else:
                got = 0
        if got == 0:
            return bytes(out)
        out += m.b[cbuf:cbuf + got]


def zstd_decode(data):
    """Resultado tal como lo ve Titan: ('ok', bytes) o ('err', mensaje)."""
    try:
        return "ok", decode_all(bytes(data))
    except ZErr as e:
        return "err", "Zstd error: " + ERRNAMES[e.code]
    except IncompleteFrame:
        return "err", "Zstd error: incomplete frame"
