#!/usr/bin/env python3
# Genera ../std_uwidth_tab.titan con las tablas de unicode-width 0.2.2 (las que
# usa console 0.15.11 para medir texto y, por lo tanto, indicatif 0.17.11).
#
# Las tablas se leen del archivo original src/tables.rs dentro de
# selfhost/fuentes/unicode-width-0.2.2.crate (descargado de crates.io por el
# workflow fuentes-crates.yml y comprobado contra la suma SHA-256 de
# Cargo.lock). No se recalcula nada: se copian los mismos bytes.
#
# El Titan de Rust compila unicode-width con sus características por defecto
# ("cjk" activada), así que WIDTH_MIDDLE tiene 20 subtablas y WIDTH_LEAVES 186;
# las funciones sin CJK solo usan las primeras, igual que en Rust.
import hashlib
import os
import re
import sys
import tarfile

AQUI = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.join(AQUI, '..', '..', 'fuentes', 'unicode-width-0.2.2.crate')
SALIDA = os.path.join(AQUI, '..', 'std_uwidth_tab.titan')
SHA = 'b4ac048d71ede7ee76d585517add45da530660ef4390e49b098733c6e897f254'

data = open(CRATE, 'rb').read()
if hashlib.sha256(data).hexdigest() != SHA:
    sys.exit('la suma del .crate no coincide con Cargo.lock')
with tarfile.open(CRATE, 'r:gz') as t:
    src = t.extractfile('unicode-width-0.2.2/src/tables.rs').read().decode()


def body(name):
    # Texto entre "static NAME...= " y el "];" que cierra la definición.
    m = re.search(r'^static ' + name + r'\b[^=]*=\s*', src, re.M)
    if not m:
        sys.exit('no encuentro ' + name)
    end = src.index('\n]);', m.end()) if src[m.end():].startswith('Align') \
        else src.index('\n];', m.end())
    return src[m.end():end]


def nums(text):
    # Los atributos #[cfg(feature = "cjk")] se dejan pasar (cjk activada).
    text = re.sub(r'#\[cfg\([^\]]*\)\]', '', text)
    return [int(x, 16) for x in re.findall(r'0x([0-9A-Fa-f]+)', text)]


tables = {}
root = nums(body('WIDTH_ROOT'))
assert len(root) == 256, len(root)
middle = nums(body('WIDTH_MIDDLE'))
assert len(middle) == 20 * 64, len(middle)
leaves = nums(body('WIDTH_LEAVES'))
assert len(leaves) == 186 * 32, len(leaves)
ntz = nums(body('NON_TRANSPARENT_ZERO_WIDTHS'))
assert len(ntz) == 71 * 6, len(ntz)
emo = nums(body('EMOJI_PRESENTATION_LEAVES'))
assert len(emo) == 7 * 128, len(emo)

blob = []
offs = {}


def put(name, vals):
    offs[name] = len(blob)
    blob.extend(vals)


put('root', root)
put('middle', middle)
put('leaves', leaves)
# Rangos (lo, hi) de 3 bytes little-endian -> se guardan igual.
put('ntz', ntz)
put('emo', emo)
leafsets = {}
for fam, count in (('TEXT_PRESENTATION_LEAF_', 10), ('EMOJI_MODIFIER_LEAF_', 8)):
    for k in range(count):
        v = nums(body(fam + str(k)))
        assert len(v) % 2 == 0
        put(fam + str(k), v)
        leafsets[fam + str(k)] = len(v) // 2
assert all(0 <= b < 256 for b in blob)

# Las ramas de starts_non_ideographic_text_presentation_seq y de
# is_emoji_modifier_base (top_bits -> hoja), copiadas del código de tables.rs.
def branches(fn):
    m = re.search(r'pub fn ' + fn + r'\(c: char\) -> bool \{(.*?)\n\}', src, re.S)
    return [(int(a, 16), b) for a, b in
            re.findall(r'0x([0-9A-F]+) => &([A-Z_0-9]+),', m.group(1))]


text_br = branches('starts_non_ideographic_text_presentation_seq')
emod_br = branches('is_emoji_modifier_base')
m = re.search(r'let idx_of_leaf: usize = match top_bits \{(.*?)_ => return false',
              src, re.S)
emo_br = [(int(a, 16), int(b)) for a, b in re.findall(r'0x([0-9A-F]+) => (\d+),', m.group(1))]

hexs = ''.join('%02x' % b for b in blob)
o = []
o.append('// GENERADO por native/uwidth/gen_uwidth.py a partir de src/tables.rs de')
o.append('// unicode-width 0.2.2 (selfhost/fuentes/unicode-width-0.2.2.crate, SHA-256')
o.append('// ' + SHA + ',')
o.append('// la misma que en Cargo.lock). No editar a mano.')
o.append('')
o.append('// Todas las tablas en una sola cadena hexadecimal (%d bytes).' % len(blob))
o.append('fn uw_tab_hex() -> string {')
lines = [hexs[i:i + 96] for i in range(0, len(hexs), 96)]
o.append('    return "' + ('" +\n        "'.join(lines)) + '"')
o.append('}')
o.append('')
for name in ('root', 'middle', 'leaves', 'ntz', 'emo'):
    o.append('fn uw_off_%s() -> int { %d }' % (name, offs[name]))
o.append('')
o.append('// starts_emoji_presentation_seq: cp >> 10 -> número de hoja (o -1).')
o.append('fn uw_emo_leaf(top: int) -> int {')
for a, b in emo_br:
    o.append('    if top == %d {' % a)
    o.append('        return %d' % b)
    o.append('    }')
o.append('    -1')
o.append('}')
o.append('')
for fn, br in (('uw_text_leaf', text_br), ('uw_emod_leaf', emod_br)):
    o.append('// cp >> 8 -> (desplazamiento, cantidad de rangos) de la hoja, o (-1, 0).')
    o.append('fn %s(top: int) -> (int, int) {' % fn)
    for a, name in br:
        o.append('    if top == %d {' % a)
        o.append('        return (%d, %d)' % (offs[name], leafsets[name]))
        o.append('    }')
    o.append('    return (-1, 0)')
    o.append('}')
    o.append('')
open(SALIDA, 'w').write('\n'.join(o))
print('bytes', len(blob), 'offsets', offs['root'], offs['middle'], offs['leaves'],
      offs['ntz'], offs['emo'])
