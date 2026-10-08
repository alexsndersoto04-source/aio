#!/usr/bin/env python3
# Genera selfhost/tests/native/compress_zstd.titan: marcos zstd hechos con la
# libzstd 1.5.7 de python-zstandard (la misma versión que la VM), válidos y
# estropeados, que la prueba descomprime con std::compress::zstd_decode.
# verify_native.sh compara la salida del ejecutable nativo con la de la VM.
# Uso: python3 gen_zstd_test.py   (determinista; no editar el .titan a mano)
import os
import random
import zstandard

here = os.path.dirname(os.path.abspath(__file__))
dest = os.path.join(here, '..', '..', 'tests', 'native', 'compress_zstd.titan')
rnd = random.Random(20260929)


def comp(data, level=3, chk=False, fcs=True):
    p = zstandard.ZstdCompressionParameters.from_level(level, write_checksum=chk,
                                                       write_content_size=fcs)
    return zstandard.ZstdCompressor(compression_params=p).compress(data)


def _stream(c, data):
    o = c.compressobj()
    return o.compress(data) + o.flush()


def texto(n):
    voc = [bytes(rnd.choice(b'abcdefghijklmnopqrstuvwxyz') for _ in range(rnd.randint(2, 9)))
           for _ in range(300)]
    out = bytearray()
    while len(out) < n:
        out += rnd.choice(voc) + b' '
    return bytes(out[:n])


def skippable(payload, k=0):
    return (0x184D2A50 + k).to_bytes(4, 'little') + len(payload).to_bytes(4, 'little') + payload


t1 = texto(3000)
t2 = texto(20000)
rep = (b'patron repetido 0123456789 ' * 12000)[:300000]
cases = [
    ('vacio', b''),
    ('marco de 0 bytes', comp(b'')),
    ('hola', comp(b'hola mundo')),
    ('texto 3000', comp(t1)),
    ('texto 3000 nivel 19 + checksum', comp(t1, 19, chk=True)),
    ('texto 20000 sin tamano (por bloques)', comp(t2, 5, fcs=False)),
    ('repetido 300000 (salida > 8192)', comp(rep, 3, chk=True)),
    ('repetido 300000 nivel 1 sin tamano', comp(rep, 1, fcs=False)),
    ('bytes aleatorios (bloques raw)', comp(bytes(rnd.getrandbits(8) for _ in range(9000)))),
    ('ceros (bloque RLE)', comp(bytes(200000), 3)),
    ('dos marcos', comp(b'primero ') + comp(b'segundo')),
    ('saltable + marco + saltable', skippable(b'xyz', 3) + comp(t1[:500]) + skippable(b'')),
    ('solo saltable', skippable(b'1234567')),
    ('ventana 2^20 streaming', _stream(zstandard.ZstdCompressor(level=4), t2 * 3)),
]
good = comp(t1, 3, chk=True)
bad = bytearray(good)
bad[-1] ^= 0xFF
cases.append(('checksum incorrecto', bytes(bad)))
cases.append(('truncado', good[:len(good) // 2]))
cases.append(('solo cabecera', good[:6]))
cases.append(('magia mala', b'\x00\x01\x02\x03' + good[4:]))
cases.append(('basura final', good + b'\x07\x07\x07'))
cases.append(('un byte', b'\x28'))
cases.append(('diccionario pedido', bytes.fromhex('28b52ffd2101') + good[5:]))
cases.append(('ventana demasiado grande', bytes.fromhex('28b52ffd00a8') + good[6:]))
cases.append(('bit reservado', bytes([0x28, 0xb5, 0x2f, 0xfd, good[4] | 8]) + good[5:]))
for k in range(12):
    b = bytearray(comp(t1[:1500], rnd.choice([1, 3, 9, 19])))
    for _ in range(rnd.randint(1, 3)):
        b[rnd.randrange(4, len(b))] ^= 1 << rnd.randrange(8)
    cases.append(('corrupto %d' % k, bytes(b)))

lines = []
lines.append('// GENERADO por native/compress/gen_zstd_test.py (no editar a mano).')
lines.append('// std::compress::zstd_decode con marcos hechos por la libzstd 1.5.7 real:')
lines.append('// válidos (varios marcos, saltables, checksum, RLE, raw, salida grande) y')
lines.append('// estropeados. Se imprime la salida (entera si es corta, si no longitud y')
lines.append('// CRC-32) o el mensaje de error, que tiene que ser el mismo que en la VM.')
lines.append('')
lines.append('fn desc(b: bytes) -> string {')
lines.append('    let n = len(b)')
lines.append('    if n <= 40 {')
lines.append('        return std::encoding::hex_encode(b)')
lines.append('    }')
lines.append('    let c = std::checksum::crc32(b)')
lines.append('    "len={n} crc={c}"')
lines.append('}')
lines.append('')
lines.append('fn main() {')
lines.append('    let casos = [')
for name, data in cases:
    lines.append('        "%s",' % name)
    lines.append('        "%s",' % data.hex())
lines.append('    ]')
lines.append('    let mut i = 0')
lines.append('    while i < len(casos) {')
lines.append('        let nombre = casos[i]')
lines.append('        let datos = std::encoding::hex_decode(casos[i + 1])')
lines.append('        let r = std::try::catch(|| { std::compress::zstd_decode(datos) })')
lines.append('        let s = match r {')
lines.append('            Result::Ok(v) => "ok " + desc(v),')
lines.append('            Result::Err(e) => "error: " + e,')
lines.append('        }')
lines.append('        println("{nombre}: {s}")')
lines.append('        i += 2')
lines.append('    }')
lines.append('}')
open(dest, 'w').write('\n'.join(lines) + '\n')
print(dest, len(cases), 'casos', sum(len(d) for _, d in cases), 'bytes')
