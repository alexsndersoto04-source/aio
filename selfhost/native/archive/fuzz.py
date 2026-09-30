#!/usr/bin/env python3
"""Prueba diferencial de std::archive: genera casos (tar y zip, válidos,
mutados y cortados), los ejecuta con prueba.titan en la VM (zett run) y
compilado con el backend nativo, y compara línea a línea.

  python3 fuzz.py CASOS SEMILLA BINARIO_NATIVO [tar|zip|todo]

Las diferencias en zip_unpack/zip_list debidas al defecto corregido (flujo
deflate cortado o con referencias a antes del principio, archivo almacenado
sin bytes suficientes) se comprueban con zlib de Python: solo se aceptan si
la entrada es realmente así y la VM devolvió datos."""
import io, os, random, subprocess, sys, tarfile, zipfile, zlib, struct

R = None

def rnd_bytes(n):
    k = R.random()
    if k < 0.3:
        return bytes(R.getrandbits(8) for _ in range(n))
    if k < 0.6:
        return bytes(R.choice(b"ab \n") for _ in range(n))
    return (b"hola mundo " * (n // 11 + 1))[:n]

SEGS = ["a", "b", "dir", ".", "", "ñandú", "x" * 60, "y" * 99, "日本", "z" * 150, "e\u0301"]

def rnd_name():
    k = R.random()
    if k < 0.05:
        return R.choice(["", ".", "./", "/", "a/", "//", "./a", "a/./b", "a//b/", "..", "a/../b",
                         "a\\b", "/abs", "x" * 100, "x" * 99, "x" * 101, "ñ" * 50, "a" * 99 + "ñ",
                         "..a", "a..", "./" * 60, "a" + "/" * 120, "." * 1])
    n = R.randint(1, 4)
    return "/".join(R.choice(SEGS) for _ in range(n)) + ("/" if R.random() < 0.1 else "")

def rnd_entries(maxn=4):
    out = []
    for _ in range(R.randint(0, maxn)):
        n = R.choice([0, 1, 5, 100, 511, 512, 513, 1500, R.randint(0, 5000)])
        out.append((rnd_name(), rnd_bytes(n), R.random() < 0.15))
    if out and R.random() < 0.1:
        out.append(R.choice(out))
    return out

def list_arg(ents):
    parts = []
    for name, data, as_str in ents:
        if as_str:
            try:
                s = data.decode("utf-8")
                parts.append(name.encode().hex() + ":s" + s.encode().hex())
                continue
            except UnicodeDecodeError:
                pass
        parts.append(name.encode().hex() + ":" + data.hex())
    return ",".join(parts)

def mutate(b):
    b = bytearray(b)
    k = R.random()
    if not b:
        return bytes(b)
    if k < 0.3:
        for _ in range(R.randint(1, 4)):
            b[R.randrange(len(b))] = R.getrandbits(8)
    elif k < 0.55:
        b = b[:R.randrange(len(b) + 1)]
    elif k < 0.7:
        i = R.randrange(len(b))
        del b[i:i + R.randint(1, 600)]
    elif k < 0.85:
        i = R.randrange(len(b))
        b[i:i] = rnd_bytes(R.randint(1, 600))
    else:
        i = R.randrange(len(b))
        b[i] ^= 1 << R.randrange(8)
    return bytes(b)

# ------------------------------------------------------------------ tar

def py_tar(fmt):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w", format=fmt) as t:
        for _ in range(R.randint(0, 4)):
            name = rnd_name().strip("/") or "f"
            data = rnd_bytes(R.choice([0, 3, 600, R.randint(0, 3000)]))
            ti = tarfile.TarInfo(name)
            ti.size = len(data)
            if R.random() < 0.15:
                ti.type = R.choice([tarfile.DIRTYPE, tarfile.SYMTYPE])
                ti.size = 0
                ti.linkname = "l" * R.choice([3, 150])
                try:
                    t.addfile(ti)
                except ValueError:
                    pass
            else:
                if fmt == tarfile.PAX_FORMAT and R.random() < 0.3:
                    ti.pax_headers = {"comment": "c" * R.randint(0, 50), "uid": "12"}
                try:
                    t.addfile(ti, io.BytesIO(data))
                except ValueError:
                    pass
    return buf.getvalue()

def hdr(name=b"f", size=0, typ=b"0", magic=b"ustar  \0", extra=None, cksum=None, prefix=b""):
    h = bytearray(512)
    h[0:len(name[:100])] = name[:100]
    h[100:108] = b"0000644\0"
    h[108:116] = b"0000000\0"
    h[116:124] = b"0000000\0"
    if isinstance(size, bytes):
        h[124:136] = size
    else:
        h[124:136] = b"%011o\0" % size if size < 8 ** 11 else b"\x80" + b"\0" * 3 + struct.pack(">Q", size)
    h[136:148] = b"00000000000\0"
    h[156:157] = typ
    h[257:265] = magic
    if prefix:
        h[345:345 + len(prefix)] = prefix
    if extra:
        for off, val in extra:
            h[off:off + len(val)] = val
    h[148:156] = b" " * 8
    s = sum(h)
    h[148:156] = cksum if cksum is not None else b"%06o\0 " % s
    return bytes(h)

def pad(d):
    return d + b"\0" * ((-len(d)) % 512)

def sparse_tar():
    # Entrada GNU dispersa (tipo 'S') con bloques y quizá extensión.
    real = 0
    blocks = []
    cur = 0
    data = b""
    for _ in range(R.randint(0, 8)):
        off = cur + R.choice([0, 0, 512, R.randint(0, 3000)])
        ln = R.choice([512, 1024, 0, R.randint(0, 900)])
        blocks.append((off, ln))
        data += rnd_bytes(ln)
        cur = off + ln
    real = cur + (R.choice([0, 0, 100]) if R.random() < 0.3 else 0)
    if R.random() < 0.2:
        R.shuffle(blocks)
    def sp(o, l):
        return (b"%011o\0" % o) + (b"%011o\0" % l)
    first = blocks[:4]
    rest = blocks[4:]
    extra = [(386 + 24 * i, sp(o, l)) for i, (o, l) in enumerate(first)]
    extra.append((483, b"%011o\0" % real))
    ext = b""
    if rest:
        extra.append((482, b"\x01"))
        while rest:
            chunk, rest = rest[:21], rest[21:]
            e = bytearray(512)
            for i, (o, l) in enumerate(chunk):
                e[24 * i:24 * i + 24] = sp(o, l)
            e[504] = 1 if rest else 0
            ext += bytes(e)
    size = len(data) if R.random() < 0.85 else len(data) + R.choice([-1, 512, 1])
    size = max(size, 0)
    magic = b"ustar  \0" if R.random() < 0.9 else b"ustar\x0000"
    h = hdr(b"sparse", size, b"S", magic, extra)
    return h + ext + pad(data) + b"\0" * 1024

def hand_tar():
    k = R.randrange(12)
    d = rnd_bytes(R.randint(0, 700))
    if k == 0:  # nombre largo GNU
        ln = (rnd_name() * 3).encode()
        return hdr(b"././@LongLink", len(ln) + R.choice([1, 0]), b"L") + pad(ln + b"\0") + hdr(b"short", len(d)) + pad(d) + b"\0" * 1024
    if k == 1:  # dos L
        return hdr(b"L", 2, b"L") + pad(b"a\0") + hdr(b"L", 2, b"L") + pad(b"b\0") + hdr(b"f", 0)
    if k == 2:  # pax con path y size
        recs = b""
        for key, val in [(b"path", rnd_name().encode()), (b"size", str(len(d)).encode())][:R.randint(0, 2)]:
            body = b" " + key + b"=" + val + b"\n"
            n = len(body) + 1
            while len(str(n)) + len(body) != n:
                n = len(str(n)) + len(body)
            recs += str(n).encode() + body
        if R.random() < 0.3:
            recs = b"5 bad\n" + recs
        if R.random() < 0.2:
            recs += b"\n9 path=q\n"
        return hdr(b"pax", len(recs), b"x", b"ustar\x0000") + pad(recs) + hdr(b"f", R.choice([len(d), 0, 5])) + pad(d) + b"\0" * 1024
    if k == 3:  # prefijo ustar
        return hdr(b"nm", len(d), b"0", b"ustar\x0000", prefix=b"pre/fix") + pad(d)
    if k == 4:  # cksum raro
        return hdr(b"f", len(d), cksum=R.choice([b" 12 \0\0\0\0", b"\xff\xfe\0\0\0\0\0\0", b"+17\0\0\0\0\0", b"        ", b"\0" * 8])) + pad(d)
    if k == 5:  # tamaño raro
        return hdr(b"f", R.choice([b"  +12 \0\0\0\0\0\0", b"zz\0\0\0\0\0\0\0\0\0\0", b"\xc3\xa9\0\0\0\0\0\0\0\0\0\0", b"\x80\0\0\0\0\0\0\0\0\0\0\x05", b"\xff" * 12, b"7" * 12, b"\x80\0\0\0\x7f" + b"\xff" * 7])) + pad(d)
    if k == 6:  # pax colgando
        return hdr(b"pax", 0, b"x", b"ustar\x0000") + b"\0" * 1024
    if k == 7:  # pax global
        return hdr(b"glob", 7, b"g", b"ustar\x0000") + pad(b"7 a=bc\n") + hdr(b"f", len(d)) + pad(d)
    if k == 8:  # barra invertida en nombre ustar
        return hdr(b"a\\b", 0, b"0", b"ustar\x0000")
    if k == 9:  # sin magia
        return hdr(b"old", len(d), b"0", b"\0" * 8) + pad(d)
    if k == 10:  # nombre con NUL y no UTF-8
        return hdr(b"\xff\xfeab", len(d)) + pad(d)
    return hdr(b"K", 3, b"K") + pad(b"lnk") + hdr(b"K", 3, b"K") + pad(b"lnk") + hdr(b"f", 0)

def tar_case():
    k = R.random()
    if k < 0.3:
        return "T " + list_arg(rnd_entries())
    if k < 0.34:
        return "X %d" % R.randrange(9)
    if k < 0.5:
        b = py_tar(R.choice([tarfile.GNU_FORMAT, tarfile.PAX_FORMAT, tarfile.USTAR_FORMAT]))
    elif k < 0.65:
        b = sparse_tar()
    else:
        b = hand_tar()
    if R.random() < 0.35:
        b = mutate(b)
    return "t " + b.hex()

# ------------------------------------------------------------------ zip

def py_zip():
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        seen = set()
        for _ in range(R.randint(0, 4)):
            name = rnd_name() or "f"
            if name in seen:
                continue
            seen.add(name)
            data = rnd_bytes(R.choice([0, 3, 600, R.randint(0, 3000)]))
            method = R.choice([zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED])
            zi = zipfile.ZipInfo(name)
            zi.compress_type = method
            if R.random() < 0.2:
                zi.extra = struct.pack("<HHBI", 0x5455, 5, 1, 12345)
            if R.random() < 0.1:
                zi.flag_bits |= 0x800
            z.writestr(zi, data)
        if R.random() < 0.2:
            z.comment = b"comentario" * R.randint(1, 5)
    return buf.getvalue()

def rnd_extra(name_raw, comment_dec):
    """Campos extra variados (algunos mal formados a propósito)."""
    out = b""
    for _ in range(R.randint(0, 3)):
        k = R.randrange(10)
        if k == 0:
            n = R.choice([0, 8, 16, 24, 28, 4])
            body = bytes(R.getrandbits(8) for _ in range(n))
            if R.random() < 0.5 and n >= 8:
                body = struct.pack("<Q", R.choice([5, 0, 3, 2 ** 40])) + body[8:]
            out += struct.pack("<HH", 1, n) + body
        elif k == 1:
            ln = R.choice([32, 32, 31])
            body = struct.pack("<IHH", 0, R.choice([1, 1, 2]), R.choice([24, 24, 23])) + b"\0" * 24
            out += struct.pack("<HH", 0x000a, ln) + body[:ln]
        elif k == 2:
            out += struct.pack("<HH", 0x9901, R.choice([7, 7, 6])) + struct.pack("<HHBH", R.choice([1, 2, 3]), R.choice([0x4541, 0x4541, 1]), R.choice([1, 3, 4]), R.choice([0, 8, 12]))
        elif k == 3:
            fl = R.choice([0, 1, 3, 7, 8, 2])
            ln = R.choice([5, 1 + 4 * bin(fl).count("1"), 9])
            out += struct.pack("<HHB", 0x5455, ln, fl) + b"\1" * (ln - 1)
        elif k == 4 or k == 5:
            tag = 0x7075 if k == 4 else 0x6375
            ref = name_raw if k == 4 else comment_dec
            crc = zlib.crc32(ref) if R.random() < 0.8 else 5
            content = R.choice(["ñame", "otro/nombre", "../x", "d/"]).encode()
            if R.random() < 0.1:
                content = b"\xff\xfe"
            out += struct.pack("<HHBI", tag, 5 + len(content), 1, crc) + content
        elif k == 6:
            out += struct.pack("<HH", 0x1234, 3) + b"abc"
        elif k == 7:
            out += b"\x01"
        else:
            out += struct.pack("<HH", 0x4444, 50) + b"z" * R.choice([50, 10])
    return out

def hand_zip():
    """Zip construido a mano: basura delante, zip64, extras, cifrado,
    métodos raros, nombres repetidos o en cp437, varios EOCD."""
    junk = rnd_bytes(R.choice([0, 0, 0, 7, 300]))
    body = bytearray(junk)
    cents = []
    names = []
    for _ in range(R.randint(0, 4)):
        if names and R.random() < 0.2:
            name = R.choice(names)
        else:
            name = R.choice([rnd_name().encode() or b"f", bytes([0x80, 0x81, 0xe9, 0x41]), b"caf\x82", "ñ".encode()])
        names.append(name)
        data = rnd_bytes(R.choice([0, 5, 700]))
        method = R.choice([0, 8, 8, 8, 12, 99])
        comp = zlib.compressobj(6, zlib.DEFLATED, -15)
        cdata = comp.compress(data) + comp.flush() if method in (8, 99) else data
        crc = zlib.crc32(data)
        flags = 0
        if R.random() < 0.3:
            flags |= 0x800
        if R.random() < 0.08:
            flags |= 1
        comment = b"" if R.random() < 0.7 else R.choice([b"hola", "é".encode(), b"\x82x"])
        comment_dec = comment.decode("utf-8", "replace").encode() if flags & 0x800 else comment.decode("cp437").encode()
        extra = rnd_extra(name, comment_dec)
        off = len(body) - (len(junk) if R.random() < 0.8 else 0)
        cs, us, hoff = len(cdata), len(data), off
        if R.random() < 0.1:
            z64 = struct.pack("<QQQ", us, cs, hoff)
            extra = struct.pack("<HH", 1, 24) + z64 + extra
            cs = us = hoff = 0xFFFFFFFF
        local = struct.pack("<IHHHHHIIIHH", 0x04034b50, 20, flags, method, 0, 0x21, crc, len(cdata), len(data), len(name), 0) + name
        body += local + cdata
        cents.append(struct.pack("<IHHHHHHIIIHHHHHII", 0x02014b50, 0x314, 20, flags, method, 0, 0x21, crc, cs, us, len(name), len(extra), len(comment), 0, 0, 0, hoff) + name + extra + comment)
    cd_start = len(body) - len(junk)
    cd = b"".join(cents)
    body += cd
    n = len(cents)
    nfield = n if R.random() < 0.9 else R.choice([0, n + 1, 65535])
    off_field = cd_start if R.random() < 0.9 else R.choice([0xFFFFFFFF, cd_start + 1, 0])
    if R.random() < 0.2:
        # EOCD64 + localizador
        e64 = len(body)
        rec = 44 if R.random() < 0.9 else R.choice([40, 50])
        body += struct.pack("<IQHHIIQQQQ", 0x06064b50, rec, 20, R.choice([20, 20, 45]), 0, R.choice([0, 0, 1]), n, R.choice([n, n, n + 1]), len(cd), cd_start)
        if rec > 44:
            body += b"\0" * R.choice([rec - 44, 0, rec - 40])
        body += struct.pack("<IIQI", 0x07064b50, 0, R.choice([e64 - len(junk), e64 - len(junk), e64 + 3, 2 ** 62]), R.choice([1, 1, 2]))
        nfield, off_field = 65535, 0xFFFFFFFF
    zc = b"" if R.random() < 0.8 else b"PK\x05\x06" + rnd_bytes(R.randint(0, 30))
    clen = len(zc) if R.random() < 0.9 else len(zc) + 5
    body += struct.pack("<IHHHHIIH", 0x06054b50, 0, 0, n if R.random() < 0.95 else n + 1, nfield, len(cd), off_field, clen) + zc
    if R.random() < 0.1:
        body += rnd_bytes(R.randint(1, 3000))
    return bytes(body)

def zip_case(native_pack=None):
    k = R.random()
    if k < 0.25:
        return "P " + list_arg(rnd_entries())
    if k < 0.29:
        return "Y %d" % R.randrange(9)
    b = py_zip() if R.random() < 0.5 else hand_zip()
    if R.random() < 0.3:
        b = mutate(b)
    return R.choice(["z ", "z ", "l "]) + b.hex()

# ------------------------------------------------------------------ main

def main():
    global R
    n = int(sys.argv[1]); seed = int(sys.argv[2]); nat = sys.argv[3]
    which = sys.argv[4] if len(sys.argv) > 4 else "todo"
    R = random.Random(seed)
    cases = []
    for _ in range(n):
        w = which if which != "todo" else R.choice(["tar", "zip"])
        cases.append(tar_case() if w == "tar" else zip_case())
    path = "/tmp/ax/casos_%d" % seed
    with open(path, "w") as f:
        f.write("\n".join(cases) + "\n")
    here = os.path.dirname(os.path.abspath(__file__))
    env = dict(os.environ); env.pop("GITHUB_ACTIONS", None)
    vm = subprocess.run([os.path.expanduser("~/.local/bin/zett"), "run", os.path.join(here, "prueba.titan"), path],
                        capture_output=True, text=True, env=env).stdout.splitlines()
    na = subprocess.run([nat, path], capture_output=True, text=True).stdout.splitlines()
    bad = 0
    acc = 0
    for i, c in enumerate(cases):
        a = vm[i] if i < len(vm) else "<falta>"
        b = na[i] if i < len(na) else "<falta>"
        if a == b:
            continue
        if c[0] in "zl" and defect_ok(bytes.fromhex(c[2:]), a, b):
            acc += 1
            continue
        bad += 1
        if bad <= 5:
            print("DIFERENCIA caso", i, c[:300])
            print("  vm :", a[:300], dec(a))
            print("  nat:", b[:300], dec(b))
    print("casos", n, "diferencias", bad, "defecto-corregido", acc)
    sys.exit(1 if bad else 0)

def dec(line):
    if line.startswith("ERR "):
        try:
            return bytes.fromhex(line[4:]).decode()
        except Exception:
            return ""
    return ""

def cd_entries(data):
    """(método, inicio de datos, tamaño comprimido) de cada entrada, leídos
    del directorio central como hace zip (EOCD más cercano al final)."""
    out = []
    e = data.rfind(b"PK\x05\x06")
    if e < 0 or e + 22 > len(data):
        return out
    n = struct.unpack("<H", data[e + 8:e + 10])[0]
    pos = struct.unpack("<I", data[e + 16:e + 20])[0]
    for _ in range(n):
        if data[pos:pos + 4] != b"PK\x01\x02":
            break
        method = struct.unpack("<H", data[pos + 10:pos + 12])[0]
        cs = struct.unpack("<I", data[pos + 20:pos + 24])[0]
        nl, el, cl = struct.unpack("<HHH", data[pos + 28:pos + 34])
        off = struct.unpack("<I", data[pos + 42:pos + 46])[0]
        pos += 46 + nl + el + cl
        if data[off:off + 4] != b"PK\x03\x04":
            continue
        lnl, lel = struct.unpack("<HH", data[off + 26:off + 30])
        out.append((method, off + 30 + lnl + lel, cs))
    return out

def broken(method, comp, cs):
    if method == 0:
        return len(comp) < cs
    if method == 8:
        d = zlib.decompressobj(-15)
        try:
            d.decompress(comp)
            return not d.eof
        except zlib.error:
            return True
    return False

def defect_ok(data, vm, nat):
    """La VM dio datos (o un fallo de CRC por datos inventados) y la nativa un
    error del defecto: se comprueba con zlib que algún archivo del zip es de
    verdad un flujo roto o corto."""
    if not (vm.startswith("ok") or dec(vm).endswith("archive I/O error: Invalid checksum")):
        return False
    msg = dec(nat)
    if not (msg.endswith("archive I/O error: unexpected end of file") or msg.endswith("archive I/O error: corrupt deflate stream")):
        return False
    for method, start, cs in cd_entries(data):
        if broken(method, data[start:start + cs], cs):
            return True
    return False

if __name__ == "__main__":
    main()
