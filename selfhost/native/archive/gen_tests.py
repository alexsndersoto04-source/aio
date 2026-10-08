#!/usr/bin/env python3
"""Genera tests/native/archive_{tar,zip,defectos,zip64}.titan.

Los casos salen de los mismos generadores que fuzz.py (con semilla fija) y de
casos hechos a mano. La salida esperada no se escribe a mano:
verify_native.sh la compara con la VM. archive_defectos contiene zips cuyo
resultado cambió con la corrección del defecto (ver selfhost/ESTADO.md): con
la VM anterior daban datos inventados o cortados.
  python3 gen_tests.py"""
import os, random, struct, sys, zlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fuzz

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "tests", "native")

HEAD = '''fn hx(s: string) -> string {
    std::encoding::hex_encode(std::encoding::utf8_encode(s))
}

fn entries_text(v: any) -> string {
    let a: array = v
    let mut out = ""
    let mut i = 0
    while i < len(a) {
        let m = a[i]
        let name: string = std::map::get(m, "name")
        let data: bytes = std::map::get(m, "bytes")
        if i > 0 {
            out = out + ","
        }
        if len(data) <= 40 {
            out = out + name + "=" + std::encoding::hex_encode(data)
        } else {
            let n = len(data)
            let c = std::checksum::crc32(data)
            out = out + name + "=len" + "{n}" + "crc" + "{c}"
        }
        i += 1
    }
    out
}

fn show_b(r: Result) -> string {
    match r {
        Result::Ok(v) => {
            let b: bytes = v
            let n = len(b)
            let c = std::checksum::crc32(b)
            "Ok(len={n} crc={c})"
        }
        Result::Err(e) => "Err(" + e + ")",
    }
}

fn show_e(r: Result) -> string {
    match r {
        Result::Ok(v) => "Ok(" + entries_text(v) + ")",
        Result::Err(e) => "Err(" + e + ")",
    }
}

fn show_n(r: Result) -> string {
    match r {
        Result::Ok(v) => {
            let a: array = v
            let mut out = ""
            let mut i = 0
            while i < len(a) {
                let s: string = a[i]
                if i > 0 {
                    out = out + ","
                }
                out = out + s
                i += 1
            }
            "Ok(" + out + ")"
        }
        Result::Err(e) => "Err(" + e + ")",
    }
}

fn anyv(x: any) -> any {
    x
}

fn entry(name: any, data: any) -> any {
    let m = std::map::insert(std::map::new(), "name", name)
    std::map::insert(m, "bytes", data)
}

fn parse_list(s: string) -> array {
    let mut out = []
    if s == "" {
        return out
    }
    let items = std::text::lines(std::text::replace(s, ",", "\\n"))
    let mut i = 0
    while i < len(items) {
        let kv = std::text::lines(std::text::replace(items[i], ":", "\\n"))
        let name = std::encoding::utf8_decode(std::encoding::hex_decode(kv[0]))
        let mut d = ""
        if len(kv) > 1 {
            d = kv[1]
        }
        if std::text::starts_with(d, "s") {
            let h = std::text::replace(d, "s", "")
            out = std::array::push(out, entry(name, std::encoding::utf8_decode(std::encoding::hex_decode(h))))
        } else {
            out = std::array::push(out, entry(name, std::encoding::hex_decode(d)))
        }
        i += 1
    }
    out
}

fn t(h: string) {
    let data = std::encoding::hex_decode(h)
    println(show_e(std::try::catch(|| { std::archive::tar_unpack(data) })))
}

fn z(h: string) {
    let data = std::encoding::hex_decode(h)
    println(show_e(std::try::catch(|| { std::archive::zip_unpack(data) })))
}

fn l(h: string) {
    let data = std::encoding::hex_decode(h)
    println(show_n(std::try::catch(|| { std::archive::zip_list(data) })))
}

fn tp(s: string) {
    let e = parse_list(s)
    let r = std::try::catch(|| { std::archive::tar_pack(e) })
    println(show_b(r))
    match r {
        Result::Ok(v) => println(show_e(std::try::catch(|| { std::archive::tar_unpack(v) }))),
        Result::Err(e) => println("-"),
    }
}

fn zp(s: string) {
    let e = parse_list(s)
    let r = std::try::catch(|| { std::archive::zip_pack(e) })
    println(show_b(r))
    match r {
        Result::Ok(v) => {
            println(show_e(std::try::catch(|| { std::archive::zip_unpack(v) })))
            println(show_n(std::try::catch(|| { std::archive::zip_list(v) })))
        }
        Result::Err(e) => println("-"),
    }
}
'''

def write(name, comment, body_lines, extra=""):
    with open(os.path.join(OUT, name), "w") as f:
        f.write(comment + "\n// Generado por selfhost/native/archive/gen_tests.py.\n\n")
        f.write(HEAD + extra + "\nfn main() {\n")
        for ln in body_lines:
            f.write("    " + ln + "\n")
        f.write("}\n")

def call(case):
    op, _, arg = case.partition(" ")
    fn = {"t": "t", "z": "z", "l": "l", "T": "tp", "P": "zp"}[op]
    return '%s("%s")' % (fn, arg)

def pick(gen, n, seed, maxlen=6000):
    fuzz.R = random.Random(seed)
    out = []
    while len(out) < n:
        c = gen()
        if c[0] in "XY" or len(c) > maxlen:
            continue
        if c[0] in "zl" and any(fuzz.broken(m, bytes.fromhex(c[2:])[st:st + cs], cs)
                                for m, st, cs in fuzz.cd_entries(bytes.fromhex(c[2:]))):
            # Resultado cambiado por la corrección del defecto: va en
            # archive_defectos, no aquí.
            continue
        out.append(call(c))
    return out

BAD = ['5', '[entry("a", "x"), 7]', '[entry("a", "x"), std::map::insert(std::map::new(), "bytes", "x")]',
       '[entry("a", "x"), entry(3, "x")]', '[entry("a", "x"), std::map::insert(std::map::new(), "name", "b")]',
       '[entry("a", "x"), entry("b", 4.5)]', '[entry("../x", "x"), entry(3, "x")]', '(entry("a", "x"), entry("q", "zz"))']

def bad_lines(fn):
    return ['println(show_b(std::try::catch(|| { std::archive::%s(anyv(%s)) })))' % (fn, b) for b in BAD]

def tar_hand():
    H = fuzz.hdr
    P = fuzz.pad
    cases = []
    long = ("carpeta/" * 20 + "archivo.txt").encode()
    cases.append(H(b"././@LongLink", len(long) + 1, b"L") + P(long + b"\0") + H(b"corto", 3) + P(b"abc") + b"\0" * 1024)
    cases.append(H(b"x", 2, b"L") + P(b"a\0") + H(b"x", 2, b"L") + P(b"b\0") + H(b"f", 0))
    cases.append(H(b"pax", 0, b"x", b"ustar\x0000") + b"\0" * 1024)
    rec = b"20 path=pax/nombre\n"
    cases.append(H(b"pax", len(rec), b"x", b"ustar\x0000") + P(rec) + H(b"f", 0) + b"\0" * 1024)
    rec = b"12 size=10\n"
    cases.append(H(b"pax", len(rec), b"x", b"ustar\x0000") + P(rec) + H(b"f", 3) + P(b"0123456789") + b"\0" * 1024)
    cases.append(H(b"nm", 2, b"0", b"ustar\x0000", prefix=b"pre/fix") + P(b"hi"))
    cases.append(H(b"f", 1, cksum=b"77777\0 \0") + P(b"x"))
    cases.append(H(b"f", b"zz\0\0\0\0\0\0\0\0\0\0"))
    cases.append(H(b"f", b"\xc3\xa9\0\0\0\0\0\0\0\0\0\0"))
    cases.append(H(b"f", b"\xff" * 12))
    cases.append(H(b"f", 5) + b"ab")
    cases.append(H(b"f", 0) + b"\0" * 100)
    cases.append(H(b"a\\b", 0, b"0", b"ustar\x0000"))
    cases.append(H(b"\xff\xfeab", 1) + P(b"z"))
    cases.append(H(b"S", 0, b"S", b"ustar\x0000"))
    sp = lambda o, n: (b"%011o\0" % o) + (b"%011o\0" % n)
    ex = [(386, sp(1024, 512)), (410, sp(2048, 0)), (483, b"%011o\0" % 2048)]
    cases.append(H(b"disperso", 512, b"S", extra=ex) + P(b"D" * 512) + b"\0" * 1024)
    ex = [(386, sp(0, 512)), (482, b"\x01"), (483, b"%011o\0" % 3072)]
    e = bytearray(512); e[0:24] = sp(2048, 512); e[24:48] = sp(3072, 0)
    cases.append(H(b"ext", 1024, b"S", extra=ex) + bytes(e) + P(b"A" * 512 + b"B" * 512) + b"\0" * 1024)
    ex = [(386, sp(100, 10)), (410, sp(512, 10)), (483, b"%011o\0" % 522)]
    cases.append(H(b"mal", 20, b"S", extra=ex) + P(b"x" * 20))
    return ['t("%s")' % c.hex() for c in cases]

def zip_hand():
    L = []
    def z1(name, data, method=8, flags=0, extra=b"", comment=b"", junk=b""):
        cdata = data
        if method == 8:
            c = zlib.compressobj(6, zlib.DEFLATED, -15); cdata = c.compress(data) + c.flush()
        crc = zlib.crc32(data)
        loc = struct.pack("<IHHHHHIIIHH", 0x04034b50, 20, flags, method, 0, 0x21, crc, len(cdata), len(data), len(name), 0) + name + cdata
        cen = struct.pack("<IHHHHHHIIIHHHHHII", 0x02014b50, 0x314, 20, flags, method, 0, 0x21, crc, len(cdata), len(data), len(name), len(extra), len(comment), 0, 0, 0, 0) + name + extra + comment
        return junk + loc + cen + struct.pack("<IHHHHIIH", 0x06054b50, 0, 0, 1, 1, len(cen), len(loc), 0)
    L.append(z1(b"caf\x82", b"hola"))
    L.append(z1("ñ".encode(), b"hola", flags=0x800))
    L.append(z1(b"x", b"hola", flags=1))
    L.append(z1(b"x", b"hola", method=12))
    L.append(z1(b"x", b"hola", method=99))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HHHHBH", 0x9901, 7, 2, 0x4541, 3, 8)))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HHBI", 0x7075, 9, 1, zlib.crc32(b"x")) + b"nuevo"))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HHBI", 0x7075, 9, 1, 7) + b"nuevo"))
    L.append(z1(b"x", b"hola", comment=b"c", extra=struct.pack("<HHBI", 0x6375, 7, 1, zlib.crc32(b"c")) + b"cc"))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HHB", 0x5455, 9, 3) + b"\0" * 8))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HHB", 0x5455, 9, 1) + b"\0" * 8))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HH", 0x000a, 31) + b"\0" * 31))
    L.append(z1(b"x", b"hola", extra=struct.pack("<HH", 0x0001, 4) + b"\0" * 4))
    L.append(z1(b"x", b"hola", junk=b"basura delante " * 3))
    L.append(z1(b"d/", b""))
    L.append(z1(b"../x", b"a"))
    L.append(b"PK\x05\x06" + b"\0" * 18)
    L.append(b"PK\x05\x06" + b"\0" * 10)
    L.append(b"PK\x05\x06" + b"\0" * 16 + b"\x09\x00")
    L.append(b"hola")
    return ['z("%s")' % c.hex() for c in L] + ['l("%s")' % c.hex() for c in L[:8]]

def defect_cases():
    """Zips cuyos datos están rotos pero la CRC coincide con lo que sacaba la
    versión anterior (datos cortados o ceros inventados)."""
    def mk(name, method, comp, crc, us, cs=None, cut=0):
        cs = len(comp) if cs is None else cs
        loc = struct.pack("<IHHHHHIIIHH", 0x04034b50, 20, 0, method, 0, 0x21, crc, cs, us, len(name), 0) + name + comp
        cen = struct.pack("<IHHHHHHIIIHHHHHII", 0x02014b50, 0x314, 20, 0, method, 0, 0x21, crc, cs, us, len(name), 0, 0, 0, 0, 0, 0) + name
        return loc + cen + struct.pack("<IHHHHIIH", 0x06054b50, 0, 0, 1, 1, len(cen), len(loc), 0)
    out = []
    data = b"hola mundo, hola TITAN " * 20
    c = zlib.compressobj(6, zlib.DEFLATED, -15); full = c.compress(data) + c.flush()
    half = full[:len(full) // 2]
    d = zlib.decompressobj(-15); part = d.decompress(half)
    out.append(("deflate cortado (la versión anterior daba Ok con la mitad)", mk(b"a.txt", 8, half, zlib.crc32(part), len(data))))
    w = fuzz_bits_far()
    out.append(("distancia a antes del principio (daba Ok con ceros)", mk(b"b.txt", 8, w, zlib.crc32(b"a\0a\0"), 4)))
    out.append(("deflate sin bloque final", mk(b"c.txt", 8, b"", zlib.crc32(b""), 0)))
    # Almacenado con tamaño mayor que lo que queda de archivo.
    raw = mk(b"d.txt", 0, b"abc", zlib.crc32(b"abc"), 3)
    loc_len = 30 + 5 + 3
    bad = bytearray(raw)
    struct.pack_into("<I", bad, 18, 10 ** 6)
    struct.pack_into("<I", bad, loc_len + 20, 10 ** 6)
    out.append(("almacenado al que le faltan bytes", bytes(bad)))
    out.append(("flujo con código inválido", mk(b"e.txt", 8, b"\xff\xff\xff\xff", 0, 0)))
    out.append(("CRC incorrecta de datos completos", mk(b"f.txt", 8, full, 1234, len(data))))
    # Campo zip64 cortado: deja el tamaño comprimido en 0 (flujo vacío).
    ext = struct.pack("<HH", 0x0001, 28) + b"\0" * 20
    c2 = zlib.compressobj(6, zlib.DEFLATED, -15); hola = c2.compress(b"hola") + c2.flush()
    loc = struct.pack("<IHHHHHIIIHH", 0x04034b50, 20, 0, 8, 0, 0x21, zlib.crc32(b"hola"), len(hola), 4, 1, 0) + b"x" + hola
    cen = struct.pack("<IHHHHHHIIIHHHHHII", 0x02014b50, 0x314, 20, 0, 8, 0, 0x21, zlib.crc32(b"hola"), len(hola), 4, 1, len(ext), 0, 0, 0, 0, 0) + b"x" + ext
    out.append(("tamaño comprimido 0 por un campo zip64 cortado", loc + cen + struct.pack("<IHHHHIIH", 0x06054b50, 0, 0, 1, 1, len(cen), len(loc), 0)))
    return out

def fuzz_bits_far():
    # Bloque fijo final: literal 'a', copia de longitud 3 a distancia 2.
    b = []
    def put(v, n): b.extend((v >> i) & 1 for i in range(n))
    def code(c, n): b.extend((c >> i) & 1 for i in range(n - 1, -1, -1))
    put(1, 1); put(1, 2)
    code(0x30 + ord("a"), 8)
    code(257 - 256, 7)
    code(1, 5)
    code(0, 7)
    b += [0] * (-len(b) % 8)
    return bytes(sum(b[i + j] << j for j in range(8)) for i in range(0, len(b), 8))

def main():
    tar = pick(fuzz.tar_case, 70, 1001) + tar_hand() + bad_lines("tar_pack") + [
        'tp("%s")' % fuzz.list_arg([(("dir/" * 40) + "x.txt", b"largo", False)]),
        'tp("%s")' % fuzz.list_arg([("ñ" * 60, b"utf8 cortado", False)]),
        'tp("%s")' % fuzz.list_arg([("./", b"", False)]),
        'tp("%s")' % fuzz.list_arg([("a//b/./c/", b"x", False)]),
        'tp("%s")' % fuzz.list_arg([("x" * 100, b"", False), ("y" * 99, b"", False)]),
    ]
    write("archive_tar.titan", "// std::archive::tar_pack / tar_unpack: casos aleatorios fijos (válidos,\n// mutados y cortados), nombres largos GNU, pax, ustar, dispersos y errores.", tar)
    zp = pick(fuzz.zip_case, 80, 2002) + zip_hand() + bad_lines("zip_pack") + [
        'zp("%s")' % fuzz.list_arg([("a", b"1", False), ("b", b"2", False), ("a", b"3", False)]),
        'zp("%s")' % fuzz.list_arg([("ñandú/日本.txt", b"x" * 3000, False)]),
        'zp("%s")' % fuzz.list_arg([("vacío", b"", False), ("s", b"texto", True)]),
    ]
    write("archive_zip.titan", "// std::archive::zip_pack / zip_unpack / zip_list: casos aleatorios fijos,\n// zip hechos a mano (zip64, campos extra, cifrado, métodos, cp437, basura\n// delante, varios EOCD) y errores.", zp)
    lines = []
    for desc, b in defect_cases():
        lines.append('println("%s")' % desc)
        lines.append('z("%s")' % b.hex())
        lines.append('l("%s")' % b.hex())
    write("archive_defectos.titan", "// std::archive::zip_unpack: datos rotos que la versión en Rust anterior daba\n// por buenos si la CRC coincidía (ver \"Defecto real\" de archive en\n// selfhost/ESTADO.md). Ahora son errores.", lines)
    # zip64 al empaquetar: más de 65535 entradas.
    z64 = '''
fn many() {
    let mut e = []
    let mut i = 0
    while i < 65540 {
        e = std::array::push(e, entry("f{i}", ""))
        i += 1
    }
    let b = std::archive::zip_pack(e)
    println(show_b(Result::Ok(b)))
    let n = std::archive::zip_list(b)
    let k = len(n)
    let last: string = n[k - 1]
    println("{k} {last}")
    let u = std::archive::zip_unpack(b)
    let ku = len(u)
    println("{ku}")
}
'''
    write("archive_zip64.titan", "// std::archive::zip_pack con más de 65535 entradas (EOCD64 y localizador)\n// y su lectura.", ["many()"], z64)

if __name__ == "__main__":
    main()
