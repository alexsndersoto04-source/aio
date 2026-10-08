#!/usr/bin/env python3
# Genera ../std_icu_tab.titan con los datos Unicode que usa idna_adapter 1.2.2
# (y por lo tanto url 2.5.8) en el Titan de Rust:
#   icu_normalizer_data 2.2.0: normalizer_uts46_data_v1 (trie de la tabla
#     UTS 46 + descomposiciones), normalizer_nfd_tables_v1 y
#     normalizer_nfkd_tables_v1 (secuencias de descomposición),
#     normalizer_nfc_v1 (composiciones canónicas, Char16Trie);
#   icu_properties_data 2.2.0: property_enum_bidi_class_v1,
#     property_enum_joining_type_v1, property_enum_general_category_v1.
#
# Los archivos se leen tal cual de selfhost/fuentes/*.crate (descargados de
# crates.io por el workflow fuentes-crates.yml y comprobados contra las sumas
# SHA-256 de Cargo.lock). No se recalcula nada: se copian los mismos bytes
# que el compilador de Rust mete en el binario.
import hashlib
import os
import re
import sys
import tarfile

AQUI = os.path.dirname(os.path.abspath(__file__))
FUENTES = os.path.join(AQUI, '..', '..', 'fuentes')
SALIDA = os.path.join(AQUI, '..', 'std_icu_tab.titan')
LOCK = os.path.join(AQUI, '..', '..', '..', 'Cargo.lock')


def lock_sum(name, version):
    text = open(LOCK).read()
    m = re.search(r'name = "' + re.escape(name) + r'"\nversion = "' + re.escape(version) +
                  r'"\nsource = [^\n]*\nchecksum = "([0-9a-f]+)"', text)
    if not m:
        sys.exit('no encuentro ' + name + ' en Cargo.lock')
    return m.group(1)


def crate_file(name, version, path):
    fn = os.path.join(FUENTES, '%s-%s.crate' % (name, version))
    data = open(fn, 'rb').read()
    sha = hashlib.sha256(data).hexdigest()
    if sha != lock_sum(name, version):
        sys.exit('la suma de ' + fn + ' no coincide con Cargo.lock')
    with tarfile.open(fn, 'r:gz') as t:
        return t.extractfile('%s-%s/%s' % (name, version, path)).read().decode(), sha


def rust_bytes(lit):
    # Contenido de un literal b"..." de Rust -> bytes.
    out = bytearray()
    i = 0
    n = len(lit)
    while i < n:
        c = lit[i]
        if c != '\\':
            out += c.encode()
            i += 1
            continue
        e = lit[i + 1]
        if e == 'x':
            out.append(int(lit[i + 2:i + 4], 16))
            i += 4
        elif e == '0':
            out.append(0)
            i += 2
        elif e == 'n':
            out.append(10)
            i += 2
        elif e == 'r':
            out.append(13)
            i += 2
        elif e == 't':
            out.append(9)
            i += 2
        elif e in '\\"\'':
            out.append(ord(e))
            i += 2
        elif e == '\n':
            # continuación de línea: se salta el salto y los espacios
            i += 2
            while i < n and lit[i] in ' \t\n\r':
                i += 1
        else:
            sys.exit('escape desconocido \\' + e)
    return bytes(out)


LIT = r'b"((?:\\.|[^"\\])*)"'


def singleton(text, name):
    i = text.index('pub const ' + name + ':')
    j = text.index(';\n', i)
    return text[i:j]


def trie(text, name):
    s = singleton(text, name)
    hdr = {}
    for k in ('high_start', 'shifted12_high_start', 'index3_null_offset',
              'data_null_offset', 'null_value'):
        hdr[k] = int(re.search(k + r': (\d+)u', s).group(1))
    hdr['small'] = 'TrieType::Small' in s
    lits = re.findall(LIT, s, re.S)
    assert len(lits) == 2, len(lits)
    return hdr, rust_bytes(lits[0]), rust_bytes(lits[1]), s


def words(b, width):
    assert len(b) % width == 0
    return [int.from_bytes(b[i:i + width], 'little') for i in range(0, len(b), width)]


sums = {}
nd = {}
for f in ('normalizer_uts46_data_v1', 'normalizer_nfd_tables_v1',
          'normalizer_nfkd_tables_v1', 'normalizer_nfc_v1'):
    nd[f], sums['icu_normalizer_data'] = crate_file('icu_normalizer_data', '2.2.0',
                                                    'data/' + f + '.rs.data')
pd = {}
for f in ('bidi_class', 'joining_type', 'general_category'):
    pd[f], sums['icu_properties_data'] = crate_file('icu_properties_data', '2.2.0',
                                                    'data/property_enum_' + f + '_v1.rs.data')

arrays = []   # (nombre, lista de enteros)
consts = []   # (nombre, valor)

# --- UTS 46
h, idx, dat, s = trie(nd['normalizer_uts46_data_v1'], 'SINGLETON_NORMALIZER_UTS46_DATA_V1')
assert h['small']
assert s.rstrip().endswith('0u32) }, passthrough_cap: 65u16 }'), s[-80:]
arrays.append(('u46_index', words(idx, 2)))
arrays.append(('u46_data', words(dat, 4)))
consts.append(('u46_high_start', h['high_start']))
consts.append(('u46_error', 0))
consts.append(('passthrough_cap', 65))

# --- tablas de descomposición
for f, pre in (('normalizer_nfd_tables_v1', 'nfd'), ('normalizer_nfkd_tables_v1', 'nfkd')):
    s = singleton(nd[f], 'SINGLETON_' + f.upper())
    lits = re.findall(LIT, s, re.S)
    assert len(lits) == 2 and 'scalars16' in s and 'scalars24' in s
    arrays.append((pre + '_s16', words(rust_bytes(lits[0]), 2)))
    arrays.append((pre + '_s24', words(rust_bytes(lits[1]), 3)))

# --- composiciones canónicas (Char16Trie)
s = singleton(nd['normalizer_nfc_v1'], 'SINGLETON_NORMALIZER_NFC_V1')
lits = re.findall(LIT, s, re.S)
assert len(lits) == 1
arrays.append(('nfc_trie', words(rust_bytes(lits[0]), 2)))

# --- propiedades (valores de un byte)
errs = {'bidi_class': 'BidiClass::LeftToRight', 'joining_type': 'JoiningType::NonJoining',
        'general_category': 'GeneralCategory::Unassigned'}
for f, pre in (('bidi_class', 'bc'), ('joining_type', 'jt'), ('general_category', 'gc')):
    h, idx, dat, s = trie(pd[f], 'SINGLETON_PROPERTY_ENUM_' + f.upper() + '_V1')
    assert h['small']
    # el valor de error es 0 en los tres (LeftToRight, NonJoining, Unassigned)
    assert s.rstrip().endswith('icu::properties::props::' + errs[f] + ') })'), s[-80:]
    arrays.append((pre + '_index', words(idx, 2)))
    arrays.append((pre + '_data', words(dat, 1)))
    consts.append((pre + '_high_start', h['high_start']))

# Formato de la cadena: por cada tabla, sus números en hexadecimal de ancho
# fijo (4 cifras para u16, 6 para char, 8 para u32, 2 para u8).
width = {}
for name, vals in arrays:
    m = max(vals) if vals else 0
    width[name] = 2 if m < 256 else 4 if m < 65536 else 6 if m < (1 << 24) else 8

blob = []
offs = {}
pos = 0
for name, vals in arrays:
    w = width[name]
    offs[name] = (pos, len(vals), w)
    blob.append(''.join(('%0' + str(w) + 'x') % v for v in vals))
    pos += len(vals)
hexs = ''.join(blob)

o = []
o.append('// GENERADO por native/icu/gen_icu.py a partir de los datos de ICU4X que usa')
o.append('// idna_adapter 1.2.2 (selfhost/fuentes/, sumas SHA-256 iguales a Cargo.lock):')
for k in sorted(sums):
    o.append('//   %s 2.2.0  %s' % (k, sums[k]))
o.append('// No editar a mano.')
o.append('')
o.append('// Todas las tablas seguidas, cada número en hexadecimal de ancho fijo.')
o.append('fn icu_tab_hex() -> string {')
chunk = 8192
lines = [hexs[i:i + chunk] for i in range(0, len(hexs), chunk)]
o.append('    return "' + ('" +\n        "'.join(lines)) + '"')
o.append('}')
o.append('')
o.append('// Número total de enteros.')
o.append('fn icu_tab_count() -> int { %d }' % pos)
o.append('')
o.append('// Tablas: (primer entero, cantidad, cifras hexadecimales por número).')
o.append('fn icu_tab_parts() -> int { %d }' % len(arrays))
o.append('fn icu_tab_part(k: int) -> (int, int, int) {')
for k, (name, vals) in enumerate(arrays):
    a, n, w = offs[name]
    o.append('    if k == %d {' % k)
    o.append('        return (%d, %d, %d)' % (a, n, w))
    o.append('    }')
o.append('    return (0, 0, 0)')
o.append('}')
o.append('')
for name, vals in arrays:
    a, n, w = offs[name]
    o.append('fn icu_off_%s() -> int { %d }' % (name, a))
    o.append('fn icu_len_%s() -> int { %d }' % (name, n))
for name, v in consts:
    o.append('fn icu_%s() -> int { %d }' % (name, v))
open(SALIDA, 'w').write('\n'.join(o) + '\n')
print('enteros', pos, 'hex', len(hexs))
for name, vals in arrays:
    print(' ', name, offs[name])
