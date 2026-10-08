#!/usr/bin/env python3
# Extrae los casos de prueba de unicode-width 0.2.2 (tests/tests.rs y
# tests/emoji-test.txt, dentro de selfhost/fuentes/unicode-width-0.2.2.crate)
# y los escribe en casos.txt para prueba.titan. Solo el ancho sin CJK (el que
# usa console/indicatif). Las pruebas con bucles (todos los caracteres) están
# traducidas a mano en prueba.titan.
#
# Formato de casos.txt, una línea por caso:
#   S cp,cp,... ANCHO    ancho de la cadena (UnicodeWidthStr::width)
#   S - 0                cadena vacía
#   C cp ANCHO           ancho del carácter (-1 = None)
import os
import re
import sys
import tarfile

AQUI = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.join(AQUI, '..', '..', 'fuentes', 'unicode-width-0.2.2.crate')
with tarfile.open(CRATE, 'r:gz') as t:
    tests = t.extractfile('unicode-width-0.2.2/tests/tests.rs').read().decode()
    emoji = t.extractfile('unicode-width-0.2.2/tests/emoji-test.txt').read().decode()


def lit(s, i):
    # Literal de Rust ("..." o '...') que empieza en s[i]; devuelve (cps, fin).
    q = s[i]
    i += 1
    out = []
    while s[i] != q:
        c = s[i]
        if c == '\\':
            e = s[i + 1]
            if e == 'u':
                j = s.index('}', i)
                out.append(int(s[i + 3:j], 16))
                i = j + 1
                continue
            if e == 'x':
                out.append(int(s[i + 2:i + 4], 16))
                i += 4
                continue
            out.append({'0': 0, 'n': 10, 'r': 13, 't': 9, '\\': 92, '"': 34, "'": 39}[e])
            i += 2
            continue
        out.append(ord(c))
        i += 1
    return out, i + 1


casos = []
omitidos = 0
for m in re.finditer(r'assert_width!\(\s*', tests):
    i = m.end()
    if tests[i] not in '"\'':
        omitidos += 1          # argumento no literal: está en los bucles
        continue
    kind = 'S' if tests[i] == '"' else 'C'
    cps, j = lit(tests, i)
    r = re.match(r'\s*,\s*(Some\((\d+)\)|None|(\d+))', tests[j:])
    if r.group(1) == 'None':
        w = -1
    else:
        w = int(r.group(2) or r.group(3))
    casos.append((kind, cps, w))
for m in re.finditer(r'assert_eq!\("', tests):
    cps, j = lit(tests, m.end() - 1)
    r = re.match(r'\.width\(\),\s*(\d+)\)', tests[j:])
    casos.append(('S', cps, int(r.group(1))))

n_emoji = 0
for line in emoji.splitlines():
    if not line or line.startswith('#'):
        continue
    cps, status = line.split(';', 1)
    status = status.strip()
    if status.startswith('fully-qualified') or status.startswith('component'):
        casos.append(('S', [int(x, 16) for x in cps.strip().split(' ')], 2))
        n_emoji += 1

with open(os.path.join(AQUI, 'casos.txt'), 'w') as f:
    for kind, cps, w in casos:
        if kind == 'C':
            f.write('C %d %d\n' % (cps[0], w))
        else:
            f.write('S %s %d\n' % (','.join(map(str, cps)) or '-', w))
print('casos', len(casos), 'de ellos emoji', n_emoji, 'no literales', omitidos)
