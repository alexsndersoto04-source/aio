#!/usr/bin/env python3
# Genera selfhost/native/std_regex_tab.titan con las tablas Unicode de
# regex-syntax 0.8.11 (src/unicode_tables/*.rs, Unicode 16.0.0), leídas
# directamente de selfhost/fuentes/regex-syntax-0.8.11.crate.
#
# Formato de cada grupo de tablas: "Nombre datos Nombre datos ..." separados
# por un espacio. Los datos son números en base 16 de longitud variable:
# cifras finales 'A'..'P' y cifras de continuación 'a'..'p' (primero las más
# significativas). Rangos: (inicio - fin anterior, fin - inicio) por rango.
# Plegado de mayúsculas (case_folding_simple): por entrada, (c - c anterior,
# cuántos, y cada destino en zigzag respecto de c).
# Nombres: "norm canon norm canon ..."; valores: "Prop:norm canon ...|Prop:...".
import hashlib
import os
import re
import sys
import tarfile

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..'))
crate = os.path.join(root, 'selfhost', 'fuentes', 'regex-syntax-0.8.11.crate')
out_path = os.path.join(root, 'selfhost', 'native', 'std_regex_tab.titan')

data = open(crate, 'rb').read()
sha = hashlib.sha256(data).hexdigest()
tf = tarfile.open(crate)


def src(name):
    return tf.extractfile('regex-syntax-0.8.11/src/unicode_tables/' + name).read().decode('utf-8')


CH = r"'(\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^'\\])'"


def ch(tok):
    if tok.startswith('\\u{'):
        return int(tok[3:-1], 16)
    if tok.startswith('\\x'):
        return int(tok[2:], 16)
    if tok.startswith('\\'):
        return {'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, "'": 39, '"': 34}[tok[1]]
    assert len(tok) == 1
    return ord(tok)


def consts(text):
    # NOMBRE -> lista de rangos, para las constantes &[(char, char)].
    res = {}
    tup = r"\(\s*" + CH + r"\s*,\s*" + CH + r"\s*\)"
    for m in re.finditer(r"pub const ([A-Z0-9_]+): &'static \[\(char, char\)\] =\s*&\[((?:\s*" + tup + r"\s*,?)*)\s*\];", text, re.S):
        body = m.group(2)
        pairs = re.findall(r"\(\s*" + CH + r"\s*,\s*" + CH + r"\s*\)", body)
        res[m.group(1)] = [(ch(a), ch(b)) for a, b in pairs]
    return res


def by_name(text):
    m = re.search(r"pub const BY_NAME: [^=]*=\s*&\[(.*?)\];", text, re.S)
    return re.findall(r'\("([^"]*)",\s*([A-Z0-9_]+)\)', m.group(1))


def num(v, out):
    assert v >= 0
    digits = []
    while True:
        digits.append(v & 15)
        v >>= 4
        if v == 0:
            break
    digits.reverse()
    for d in digits[:-1]:
        out.append(chr(97 + d))
    out.append(chr(65 + digits[-1]))


def enc_ranges(rs):
    out = []
    prev = 0
    for i, (s, e) in enumerate(rs):
        assert s <= e and (i == 0 or s > prev)
        num(s - prev, out)
        num(e - s, out)
        prev = e
    return ''.join(out)


def group(fname):
    text = src(fname)
    cs = consts(text)
    parts = []
    for name, const in by_name(text):
        parts.append(name)
        parts.append(enc_ranges(cs[const]))
    return ' '.join(parts)


def single(fname, const):
    return enc_ranges(consts(src(fname))[const])


def fold():
    text = src('case_folding_simple.rs')
    m = re.search(r"pub const CASE_FOLDING_SIMPLE: [^=]*=\s*&\[(.*?)\];", text, re.S)
    out = []
    prev = 0
    n = 0
    for a, b in re.findall(r"\(\s*" + CH + r"\s*,\s*&\[(.*?)\]\s*\)", m.group(1)):
        c = ch(a)
        ts = [ch(t) for t in re.findall(CH, b)]
        assert c > prev or n == 0
        num(c - prev, out)
        num(len(ts), out)
        for t in ts:
            d = t - c
            num(2 * d if d >= 0 else -2 * d - 1, out)
        prev = c
        n += 1
    return ''.join(out), n


def names():
    text = src('property_names.rs')
    pairs = re.findall(r'\("([^"]*)",\s*"([^"]*)"\)', text)
    return ' '.join(a + ' ' + b for a, b in pairs), pairs


def values():
    text = src('property_values.rs')
    body = text[text.index('= &['):]
    res = []
    for m in re.finditer(r'\(\s*"([^"]*)",\s*&\[(.*?)\],?\s*\)', body, re.S):
        pairs = re.findall(r'\("([^"]*)",\s*"([^"]*)"\)', m.group(2))
        for a, b in pairs:
            assert ' ' not in a + b and '|' not in a + b and ':' not in a + b
        res.append(m.group(1) + ':' + ' '.join(a + ' ' + b for a, b in pairs))
    return '|'.join(res)


def titan_str(s):
    assert '"' not in s and '\\' not in s and '{' not in s and '}' not in s
    chunks = [s[i:i + 8000] for i in range(0, len(s), 8000)] or ['']
    return '    return "' + '"\n        + "'.join(chunks) + '"\n'


lines = [
    '// GENERADO por native/regex/gen_tab.py a partir de las tablas Unicode 16.0.0',
    '// de regex-syntax 0.8.11 (src/unicode_tables/*.rs), leídas de',
    '// selfhost/fuentes/regex-syntax-0.8.11.crate (SHA-256 ' + sha + ').',
    '// Formato descrito en gen_tab.py. No editar a mano.',
    '',
]


def fn(name, s):
    lines.append('fn ' + name + '() -> string {')
    lines.append(titan_str(s).rstrip('\n'))
    lines.append('}')
    lines.append('')


nm, _ = names()
fn('rxt_names', nm)
fn('rxt_values', values())
fn('rxt_gencat', group('general_category.rs'))
fn('rxt_script', group('script.rs'))
fn('rxt_scx', group('script_extension.rs'))
fn('rxt_bool', group('property_bool.rs'))
fn('rxt_age', group('age.rs'))
fn('rxt_gcb', group('grapheme_cluster_break.rs'))
fn('rxt_wb', group('word_break.rs'))
fn('rxt_sb', group('sentence_break.rs'))
fn('rxt_perl_word', single('perl_word.rs', 'PERL_WORD'))
fn('rxt_perl_space', single('perl_space.rs', 'WHITE_SPACE'))
fn('rxt_perl_digit', single('perl_decimal.rs', 'DECIMAL_NUMBER'))
f, nf = fold()
fn('rxt_fold', f)
lines.append('fn rxt_fold_count() -> int { ' + str(nf) + ' }')
lines.append('')
open(out_path, 'w').write('\n'.join(lines))
print('escrito', out_path, os.path.getsize(out_path), 'bytes')
