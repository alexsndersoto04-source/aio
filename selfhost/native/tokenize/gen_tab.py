#!/usr/bin/env python3
# Genera selfhost/native/std_tokenize_tab.titan: tablas Unicode que usa `tokenizers` 0.22.2.
#   - unicode-normalization-alignments 0.1.12 (Unicode 9.0.0): clases de combinación, descomposiciones
#     canónica y de compatibilidad, composición y marcas combinantes (NFC/NFD/NFKC/NFKD, StripAccents).
#   - unicode_categories 0.1.1: Cc/Cf/Co (is_other), Pc..Ps (is_punctuation) y Mn (is_mark_nonspacing).
# Las fuentes están en selfhost/fuentes/tokenizers-fuentes.tar.gz (copias de los repositorios upstream).
# Formato: cada tabla es una cadena de enteros decimales separados por comas.
import re, sys, tarfile, hashlib, os
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..'))
tgz = os.path.join(root, 'selfhost', 'fuentes', 'tokenizers-fuentes.tar.gz')
out_path = os.path.join(root, 'selfhost', 'native', 'std_tokenize_tab.titan')
tf = tarfile.open(tgz)
def src(n): return tf.extractfile(n).read().decode('utf-8')
sha = hashlib.sha256(open(tgz, 'rb').read()).hexdigest()
un = src('un/src/tables.rs')
uc = src('uc/src/tables.rs')

def const_block(text, name):
    m = re.search(r'const ' + name + r'[^=]*=\s*&\[(.*?)\n\];', text, re.S)
    return m.group(1)

def cp(tok):
    return int(tok.group(1), 16)

# ccc: kv = (cp << 8) | class
ccc = sorted((int(x, 16) >> 8, int(x, 16) & 255) for x in re.findall(r'0x([0-9A-Fa-f]+)', const_block(un, 'CANONICAL_COMBINING_CLASS_KV')))
# composición: ((c1<<16|c2), r) en el BMP + tabla astral
comp = []
for m in re.finditer(r"\(0x([0-9A-Fa-f]+), '\\u\{([0-9A-Fa-f]+)\}'\)", const_block(un, 'COMPOSITION_TABLE_KV')):
    k = int(m.group(1), 16)
    comp.append((k >> 16, k & 0xFFFF, int(m.group(2), 16)))
astral = re.search(r'fn composition_table_astral.*?match \(c1, c2\) \{(.*?)_ => None', un, re.S).group(1)
for m in re.finditer(r"\('\\u\{([0-9A-Fa-f]+)\}', '\\u\{([0-9A-Fa-f]+)\}'\) => Some\('\\u\{([0-9A-Fa-f]+)\}'\)", astral):
    comp.append(tuple(int(g, 16) for g in m.groups()))
comp.sort()
def decomp(name):
    res = []
    for m in re.finditer(r"\(0x([0-9A-Fa-f]+), &\[(.*?)\]\)", const_block(un, name)):
        ds = [int(x, 16) for x in re.findall(r"\\u\{([0-9A-Fa-f]+)\}", m.group(2))]
        res.append((int(m.group(1), 16), ds))
    res.sort()
    return res
canon = decomp('CANONICAL_DECOMPOSED_KV')
compat = decomp('COMPATIBILITY_DECOMPOSED_KV')
marks = sorted(int(x, 16) for x in re.findall(r'0x([0-9A-Fa-f]+)', const_block(un, 'COMBINING_MARK_KV')))

def uc_table(name):
    m = re.search(r'static ' + name + r"\s*:[^=]*=\s*&\[(.*?)\];", uc, re.S)
    body = m.group(1)
    return [int(x, 16) for x in re.findall(r"\\u\{([0-9A-Fa-f]+)\}", body)]
def ranges(vals):
    vals = sorted(set(vals))
    out = []
    for v in vals:
        if out and out[-1][1] + 1 == v:
            out[-1][1] = v
        else:
            out.append([v, v])
    return out
def flat_ranges(vals):
    r = ranges(vals)
    return ','.join('%d,%d' % (a, b) for a, b in r)
other = uc_table('OTHER_CONTROL') + uc_table('OTHER_FORMAT') + uc_table('OTHER_PRIVATE_USE')
other += list(range(0xE000, 0xF8FF + 1)) + list(range(0xF0000, 0xFFFFD + 1)) + list(range(0x100000, 0x10FFFD + 1))
punct = []
for n in ['PUNCTUATION_CONNECTOR', 'PUNCTUATION_DASH', 'PUNCTUATION_CLOSE', 'PUNCTUATION_FINAL_QUOTE',
          'PUNCTUATION_INITIAL_QUOTE', 'PUNCTUATION_OTHER', 'PUNCTUATION_OPEN']:
    punct += uc_table(n)
mn = uc_table('MARK_NONSPACING')

# --- Tablas de regex-syntax 0.8.11 (Unicode 16.0.0): \w, \s, \d, categorías y scripts ---
rtf = tarfile.open(os.path.join(root, 'selfhost', 'fuentes', 'regex-syntax-0.8.11.crate'))
def rsrc(n):
    return rtf.extractfile('regex-syntax-0.8.11/src/unicode_tables/' + n).read().decode('utf-8')
def chv(t):
    if t.startswith('\\u{'): return int(t[3:-1], 16)
    if t.startswith('\\x'): return int(t[2:], 16)
    if t.startswith('\\'):
        return {'n': 10, 'r': 13, 't': 9, '\\': 92, "'": 39, '0': 0}[t[1]]
    return ord(t)
CHR = r"'(\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^'\\])'"
def rtable(text, const):
    m = re.search(r"pub const " + const + r":[^=]*=\s*&\[(.*?)\];", text, re.S)
    return [(chv(a), chv(b)) for a, b in re.findall(r'\(' + CHR + ', ' + CHR + r'\)', m.group(1))]
def flat(rs):
    return ','.join('%d,%d' % r for r in rs)
def by_name(text):
    m = re.search(r'BY_NAME[^=]*=\s*&\[(.*?)\n\];', text, re.S)
    out = []
    for name, const in re.findall(r'\("([A-Za-z_0-9]+)", ([A-Z_0-9]+)\)', m.group(1)):
        out.append(name + '=' + flat(rtable(text, const)))
    return ';'.join(out)
gc_text = rsrc('general_category.rs')
sc_text = rsrc('script.rs')
word = rtable(rsrc('perl_word.rs'), 'PERL_WORD')
space = rtable(rsrc('property_bool.rs'), 'WHITE_SPACE')
digit = rtable(rsrc('perl_decimal.rs'), 'DECIMAL_NUMBER')
gcs = by_name(gc_text)
scs = by_name(sc_text)

def fn(name, text):
    return 'fn %s() -> string {\n    "%s"\n}\n\n' % (name, text)
o = []
o.append('// GENERADO por selfhost/native/tokenize/gen_tab.py. No editar a mano.\n')
o.append('// Fuente: selfhost/fuentes/tokenizers-fuentes.tar.gz (sha256 %s)\n' % sha)
o.append('// unicode-normalization-alignments 0.1.12 (Unicode 9.0.0) y unicode_categories 0.1.1.\n\n')
o.append(fn('tkt_ccc', ','.join('%d,%d' % p for p in ccc)))
o.append(fn('tkt_comp', ','.join('%d,%d,%d' % p for p in comp)))
o.append(fn('tkt_canon', ','.join('%d,%d,%s' % (c, len(d), ','.join(map(str, d))) for c, d in canon)))
o.append(fn('tkt_compat', ','.join('%d,%d,%s' % (c, len(d), ','.join(map(str, d))) for c, d in compat)))
o.append(fn('tkt_marks', flat_ranges(marks)))
o.append(fn('tkt_other', flat_ranges(other)))
o.append(fn('tkt_punct', flat_ranges(punct)))
o.append(fn('tkt_mn', flat_ranges(mn)))
o.append(fn('tkt_word', flat(word)))
o.append(fn('tkt_space', flat(space)))
o.append(fn('tkt_digit', flat(digit)))
o.append(fn('tkt_gc', gcs))
o.append(fn('tkt_script', scs))
open(out_path, 'w').write(''.join(o))
print('escrito', out_path, len(ccc), len(comp), len(canon), len(compat), len(marks))
