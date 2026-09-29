#!/usr/bin/env python3
# Genera tests/native/compress_{inflate,codigos,defectos}.titan con flujos
# hechos por el zlib de C (módulo zlib) y por un escritor de bits. La salida
# esperada no se escribe a mano: verify_native.sh la compara con la VM.
# Uso: python3 gen_tests.py   (desde cualquier sitio)
import gzip, os, zlib

out_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'tests', 'native')


class Bits:
    def __init__(s): s.b = []
    def put(s, v, n): s.b += [(v >> i) & 1 for i in range(n)]
    def code(s, c, n): s.b += [(c >> i) & 1 for i in range(n - 1, -1, -1)]
    def bytes(s):
        b = s.b + [0] * (-len(s.b) % 8)
        return bytes(sum(b[i + j] << j for j in range(8)) for i in range(0, len(b), 8))


def fixed_lit(w, sym):
    if sym < 144: w.code(0x30 + sym, 8)
    elif sym < 256: w.code(0x190 + sym - 144, 9)
    elif sym < 280: w.code(sym - 256, 7)
    else: w.code(0xc0 + sym - 280, 8)


def raw(data, lvl=6, strat=zlib.Z_DEFAULT_STRATEGY):
    c = zlib.compressobj(lvl, zlib.DEFLATED, -15, 9, strat)
    return c.compress(data) + c.flush()


def zwrap(body, data, cmf=0x78):
    flg = (31 - (cmf * 256) % 31) % 31
    return bytes([cmf, flg]) + body + zlib.adler32(data).to_bytes(4, 'big')


def gzwrap(body, data, flags=0, extra=b'', name=b'', comment=b'', hcrc_ok=True):
    h = bytearray([31, 139, 8, flags, 0, 0, 0, 0, 0, 255])
    if flags & 4: h += len(extra).to_bytes(2, 'little') + extra
    if flags & 8: h += name + b'\0'
    if flags & 16: h += comment + b'\0'
    if flags & 2: h += ((zlib.crc32(bytes(h)) ^ (0 if hcrc_ok else 1)) & 0xffff).to_bytes(2, 'little')
    return bytes(h) + body + zlib.crc32(data).to_bytes(4, 'little') + len(data).to_bytes(4, 'little')


HEAD = '''fn show(r: Result) -> string {
    match r {
        Result::Ok(v) => {
            let b: bytes = v
            if len(b) <= 40 {
                "Ok(" + std::encoding::hex_encode(b) + ")"
            } else {
                let n = len(b)
                let c = std::checksum::crc32(b)
                "Ok(len={n} crc={c})"
            }
        }
        Result::Err(e) => "Err(" + e + ")",
    }
}

fn g(h: string) -> string {
    show(std::try::catch(|| { std::compress::gzip_decode(std::encoding::hex_decode(h)) }))
}

fn z(h: string) -> string {
    show(std::try::catch(|| { std::compress::zlib_decode(std::encoding::hex_decode(h)) }))
}

fn d(h: string) -> string {
    show(std::try::catch(|| { std::compress::deflate_decode(std::encoding::hex_decode(h)) }))
}

fn main() {
'''


def write(name, comment, cases, extra=''):
    lines = [f'// {l}' if l else '//' for l in comment.split('\n')]
    lines.append('// Generado por selfhost/native/compress/gen_tests.py.')
    body = HEAD
    for label, mode, data in cases:
        body += f'    println("{label}: " + {mode}("{data.hex()}"))\n'
    body += extra + '}\n'
    open(os.path.join(out_dir, name), 'w').write('\n'.join(lines) + '\n\n' + body)


text = b'hola mundo, hola Titan. ' * 50
rnd_bytes = bytes((i * 7919 + (i >> 3) * 31) & 255 for i in range(3000))
body = raw(text)
cases = [
    ('gzip nivel 6', 'g', gzip.compress(text, 6, mtime=0)),
    ('gzip nivel 0', 'g', gzip.compress(text, 0, mtime=0)),
    ('gzip nivel 1', 'g', gzip.compress(rnd_bytes, 1, mtime=0)),
    ('gzip nivel 9', 'g', gzip.compress(rnd_bytes, 9, mtime=0)),
    ('gzip vacío', 'g', gzip.compress(b'', 6, mtime=0)),
    ('gzip FEXTRA FNAME FCOMMENT FHCRC', 'g', gzwrap(body, text, 2 | 4 | 8 | 16, b'xy12', b'datos.txt', b'hola')),
    ('gzip FTEXT', 'g', gzwrap(body, text, 1)),
    ('gzip con basura detrás', 'g', gzwrap(body, text) + b'basura'),
    ('gzip dos miembros (solo el primero)', 'g', gzip.compress(b'uno', mtime=0) + gzip.compress(b'dos', mtime=0)),
    ('gzip magia mala', 'g', b'\x1f\x8c' + gzwrap(body, text)[2:]),
    ('gzip método 7', 'g', b'\x1f\x8b\x07' + gzwrap(body, text)[3:]),
    ('gzip flags reservados', 'g', gzwrap(body, text, 32)),
    ('gzip cabecera corta', 'g', bytes([31, 139, 8, 0, 0])),
    ('gzip FNAME sin fin', 'g', bytes([31, 139, 8, 8, 0, 0, 0, 0, 0, 255]) + b'nombre'),
    ('gzip FEXTRA corto', 'g', bytes([31, 139, 8, 4, 0, 0, 0, 0, 0, 255, 9, 0, 1, 2])),
    ('gzip CRC de cabecera mal', 'g', gzwrap(body, text, 2, hcrc_ok=False)),
    ('gzip CRC mal', 'g', gzwrap(body, text)[:-8] + b'\0\0\0\0' + gzwrap(body, text)[-4:]),
    ('gzip longitud mal', 'g', gzwrap(body, text)[:-4] + b'\1\0\0\0'),
    ('gzip sin cola', 'g', gzwrap(body, text)[:-8]),
    ('gzip cola a medias', 'g', gzwrap(body, text)[:-3]),
    ('gzip cuerpo truncado', 'g', gzwrap(body, text)[:30]),
    ('gzip cuerpo corrupto', 'g', gzwrap(b'\xff\xff\xff' + body, text)),
    ('zlib nivel 6', 'z', zlib.compress(text, 6)),
    ('zlib nivel 0', 'z', zlib.compress(rnd_bytes, 0)),
    ('zlib nivel 9', 'z', zlib.compress(rnd_bytes, 9)),
    ('zlib ventana 256', 'z', zwrap(body, text, 0x08)),
    ('zlib ventana 64K', 'z', zwrap(body, text, 0x88)),
    ('zlib método 9', 'z', zwrap(body, text, 0x79)),
    ('zlib comprobación mala', 'z', bytes([0x78, 0x9d]) + zlib.compress(text)[2:]),
    ('zlib diccionario (FDICT)', 'z', bytes([0x78, 0xbb]) + b'\0\0\0\1' + zlib.compress(text)[2:]),
    ('zlib Adler-32 mal', 'z', zlib.compress(text)[:-1] + bytes([zlib.compress(text)[-1] ^ 1])),
    ('zlib con basura detrás', 'z', zlib.compress(text) + b'mas'),
    ('deflate fijo (Z_FIXED)', 'd', raw(text, 6, zlib.Z_FIXED)),
    ('deflate solo Huffman', 'd', raw(rnd_bytes, 6, zlib.Z_HUFFMAN_ONLY)),
    ('deflate RLE', 'd', raw(b'\0' * 5000 + b'x' * 300, 6, zlib.Z_RLE)),
    ('deflate con basura detrás', 'd', raw(text) + b'zzz'),
]
# Bloques hechos a mano.
w = Bits(); w.put(1, 1); w.put(3, 2); cases.append(('deflate bloque tipo 3', 'd', w.bytes()))
cases.append(('deflate almacenado LEN/NLEN mal', 'd', bytes([1, 3, 0, 0xfc, 0xfe]) + b'abc'))
cases.append(('deflate almacenado vacío', 'd', bytes([1, 0, 0, 0xff, 0xff])))
cases.append(('deflate almacenado + fijo', 'd', bytes([0, 2, 0, 0xfd, 0xff]) + b'hi' + raw(b'!', 6, zlib.Z_FIXED)))
w = Bits(); w.put(1, 1); w.put(1, 2); fixed_lit(w, 65); fixed_lit(w, 286); cases.append(('deflate fijo símbolo 286', 'd', w.bytes()))
w = Bits(); w.put(1, 1); w.put(1, 2); fixed_lit(w, 65); fixed_lit(w, 257); w.code(30, 5); cases.append(('deflate fijo distancia 30', 'd', w.bytes()))
w = Bits(); w.put(1, 1); w.put(2, 2); w.put(30, 5); w.put(0, 5); w.put(0, 4); cases.append(('deflate HLIT 287', 'd', w.bytes()))
w = Bits(); w.put(1, 1); w.put(2, 2); w.put(0, 5); w.put(30, 5); w.put(0, 4); cases.append(('deflate HDIST 31', 'd', w.bytes()))
ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]


def canon(lengths):
    codes, code = {}, 0
    for ln in range(1, 16):
        for sym, l in enumerate(lengths):
            if l == ln: codes[sym] = (code, ln); code += 1
        code <<= 1
    return codes


def dynamic(hlit, hdist, hl, seq, lit=None, dist=None, symbols=(), tail=()):
    # seq: (símbolo de longitud de código, extra, bits extra); después
    # `symbols` con las tablas lit/dist ya decididas (listas de longitudes).
    w = Bits(); w.put(1, 1); w.put(2, 2); w.put(hlit - 257, 5); w.put(hdist - 1, 5)
    hclen = 19
    while hclen > 4 and hl[ORDER[hclen - 1]] == 0: hclen -= 1
    w.put(hclen - 4, 4)
    for k in range(hclen): w.put(hl[ORDER[k]], 3)
    hc = canon(hl)
    for sym, extra, n in seq: w.code(*hc[sym]); w.put(extra, n)
    if lit is not None:
        lc = canon(lit)
        for sym in symbols: w.code(*lc[sym])
    w.b += list(tail)
    return w.bytes()


cases.append(('deflate tabla de longitudes sobrada', 'd',
              dynamic(257, 1, [1, 1, 1] + [0] * 13 + [1, 0, 0], [])))
hl = [0] * 19; hl[0] = 1; hl[16] = 1
cases.append(('deflate 16 al principio', 'd', dynamic(257, 1, hl, [(16, 0, 2)])))
hl = [0] * 19; hl[0] = 1; hl[18] = 1
cases.append(('deflate longitudes de más', 'd', dynamic(257, 1, hl, [(18, 127, 7), (18, 127, 7), (18, 127, 7)])))
# Un solo código de literal (válido en miniz; zlib lo rechaza) y árbol vacío de
# distancias: 256 con longitud 1, todo lo demás 0. Sale un bloque vacío.
hl = [0] * 19; hl[0] = 2; hl[1] = 2; hl[18] = 1
lit = [0] * 256 + [1]
seq = [(18, 127, 7), (18, 107, 7), (1, 0, 0), (0, 0, 0)]
cases.append(('deflate un solo código', 'd', dynamic(257, 1, hl, seq, lit, [0], [256])))
# Un solo código de 12 bits (el 256): los caminos no usados del árbol valen 0
# (símbolo 0 = byte 0), rareza de miniz que se copia tal cual. Diez ceros y
# un uno dan el literal 0; doce ceros, el fin de bloque.
hl = [0] * 19; hl[0] = 2; hl[12] = 2; hl[18] = 1
lit = [0] * 256 + [12]
seq = [(18, 127, 7), (18, 107, 7), (12, 0, 0), (0, 0, 0)]
cases.append(('deflate un solo código largo', 'd', dynamic(257, 1, hl, seq, tail=[0] * 10 + [1] + [0] * 12)))
write('compress_inflate.titan',
      'std::compress: descompresión gzip / zlib / deflate válida e inválida (casos en\n'
      'los que la VM anterior y la corregida coinciden).', cases,
      '    println("string: " + show(std::try::catch(|| { std::compress::gzip_decode("hola") })))\n')

# Todos los códigos de longitud (257..285, extra mínimo y máximo) y de
# distancia (0..29, extra mínimo y máximo), en un bloque fijo.
LB = lambda i: 3 + i if i < 8 else 258 if i == 28 else ((4 + (i & 3)) << (i // 4 - 1)) + 3
LE = lambda i: 0 if i < 8 or i == 28 else i // 4 - 1
DB = lambda d: d + 1 if d < 4 else ((2 + (d & 1)) << (d // 2 - 1)) + 1
DE = lambda d: max(d // 2 - 1, 0)
w = Bits(); w.put(1, 1); w.put(1, 2)
n = 0
for c in b'Titan!':
    fixed_lit(w, c); n += 1
while n < 32768:                     # crecer con coincidencias de 258 a distancia 6
    fixed_lit(w, 285); w.code(4, 5); w.put(1, 1); n += 258
for i in range(29):
    for x in ([0] if LE(i) == 0 else [0, (1 << LE(i)) - 1]):
        for dcode in range(30):
            for y in ([0] if DE(dcode) == 0 else [0, (1 << DE(dcode)) - 1]):
                fixed_lit(w, 257 + i); w.put(x, LE(i)); w.code(dcode, 5); w.put(y, DE(dcode))
fixed_lit(w, 256)
stream = w.bytes()
exp = zlib.decompressobj(-15).decompress(stream)
assert len(exp) > 32768
write('compress_codigos.titan',
      'std::compress: todos los códigos de longitud y distancia (con el extra mínimo\n'
      'y máximo) en un bloque Huffman fijo; también como zlib y gzip.',
      [('todos los códigos (deflate)', 'd', stream), ('todos los códigos (zlib)', 'z', zwrap(stream, exp)),
       ('todos los códigos (gzip)', 'g', gzwrap(stream, exp))])

# Los dos defectos corregidos (aquí la VM anterior daba "ok").
w = Bits(); w.put(1, 1); w.put(1, 2); fixed_lit(w, 97); fixed_lit(w, 257); w.code(1, 5); fixed_lit(w, 256)
far = w.bytes()
full = raw(text)
write('compress_defectos.titan',
      'std::compress: flujos truncados y distancias a antes del principio son errores\n'
      '(la versión de Rust anterior devolvía los datos a medias o ceros inventados;\n'
      'ver "Defecto real" de compress en selfhost/ESTADO.md).',
      [('deflate vacío', 'd', b''), ('deflate xyz', 'd', b'xyz'), ('deflate truncado', 'd', full[:len(full) // 2]),
       ('deflate sin el último byte', 'd', full[:-1]), ('zlib solo cabecera', 'z', zlib.compress(text)[:2]),
       ('zlib sin Adler-32', 'z', zlib.compress(text)[:-4]), ('zlib Adler-32 a medias', 'z', zlib.compress(text)[:-2]),
       ('deflate distancia antes del principio', 'd', far), ('zlib distancia antes del principio', 'z', zwrap(far, b'a\0a\0')),
       ('gzip distancia antes del principio', 'g', gzwrap(far, b'a\0a\0'))])
print('ok')
