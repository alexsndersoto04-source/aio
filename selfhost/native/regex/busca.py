#!/usr/bin/env python3
# Prueba diferencial de la búsqueda de std::regex: busca.titan en la VM de
# Rust (zett run) contra el mismo programa compilado con el backend nativo.
# Compara is_match, find, find_all, captures, split y replace_all (y los
# errores) sobre patrones, textos y reemplazos aleatorios.
# Uso: python3 busca.py NATIVO SEMILLA CANTIDAD
import os
import random
import subprocess
import sys

nat, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
rnd = random.Random(seed)
here = os.path.dirname(os.path.abspath(__file__))

ATOM = ['a', 'b', 'c', 'ab', '.', r'\w', r'\W', r'\d', r'\s', r'\S', '[a-c]', '[^a]', '[[:alpha:]]', r'\b', r'\B',
        '^', '$', '(?m:^)', '(?m:$)', '(?R:$)', '(?Rm:^)', '(?Rm:$)', 'é', 'ß', 'σ', 'k', r'\pL', r'\p{Greek}',
        r'\x{1F600}', r'(?-u:\b)', r'(?-u:\B)', r'\b{start}', r'\b{end}', r'\<', r'\>', r'\b{start-half}',
        r'\b{end-half}', r'(?-u:\b{start-half})', r'\n', r'\r', '(?s:.)', '(?i)k', '(?i:σ)', r'[^\n]', r'\A',
        r'\z', '(?U)a*', '', '(?:)', r'[\pL&&\p{Greek}]', r'[a-z--c]', '(?i)[a-c]', r'(?-u:\w)', r'(?i)ǅ', '中',
        r'\p{Han}', r'[\x{1F600}-\x{1F64F}]', r'(?R).', r'(?Rs:.)', r'\PL', r'\D', ' ', '-', '_']
REP = ['', '', '', '*', '+', '?', '*?', '+?', '??', '{2}', '{1,3}', '{0,2}?', '{2,}']
GRP = ['(', '(?:', '(?P<n1>', '(?<n2>', '(?i:', '(?s:', '(?m:', '(']
HAY = ['a', 'b', 'c', 'ab', 'abc', ' ', '\n', '\r\n', '\r', 'é', 'ß', 'σ', 'Σ', 'ς', 'k', 'K', 'K', '中', '😀', '1', '_',
       '-', '.', 'ǅ', 'ſ', 'aa', 'bb', '\u0301', 'A', 'Ω']
REPL = ['X', '$0', '$1', '${1}', '$2', '$n1', '${n2}', '$$', '$', '${', '${+1}', '$1a', '${1}a', '$9',
        '$99999999999999999999999', '-', '', 'é$0é', '${n1}${n2}', '<$1|$2>', '${}', '$ ', '$é', '$$1', '${0}${0}']


def gen(d):
    r = rnd.random()
    if d > 4 or r < 0.4:
        s = rnd.choice(ATOM)
    elif r < 0.6:
        s = ''.join(gen(d + 1) for _ in range(rnd.randint(1, 4)))
    elif r < 0.75:
        s = '|'.join(gen(d + 1) for _ in range(rnd.randint(2, 3)))
    else:
        s = rnd.choice(GRP) + gen(d + 1) + ')'
    return s + rnd.choice(REP)


def hay():
    return ''.join(rnd.choice(HAY) for _ in range(rnd.randint(0, 12)))


def repl():
    return ''.join(rnd.choice(REPL) for _ in range(rnd.randint(0, 3)))


MANUAL = [
    ('a+', 'caaab aa', 'X'), ('', 'héllo', '-'), ('', '', 'x'), ('a*', 'baaa', '<$0>'), ('a*?', 'aaa', '.'),
    (r'\b', 'hi there', '|'), (r'\B', 'é中😀', '|'), ('(a)|(b)', 'ab', '[$1,$2]'), ('(a)(b){0}', 'ab', '$2'),
    ('(b)(a){0}', 'b', '$1'), ('(?P<x>a)(?P<y>b){0}', 'ab', '${x}${y}'), ('x*', '😀', '-'), (r'(?m)^', 'a\nb\n', '>'),
    (r'(?m)$', 'a\nb\n', '<'), (r'(?Rm)^', 'a\r\nb\r\n', '>'), (r'(?Rm)$', 'a\r\nb\r', '<'), ('^|$', 'ab', '#'),
    ('(?i)straße', 'STRASSE straße STRAßE', 'x'), ('(?i)k', 'kKK', '_'), (r'\w+', 'héllo wörld 中文', '[$0]'),
    ('(a|ab)(c|bcd)(d*)', 'abcd', '$1-$2-$3'), ('(a+)+b', 'aaaaaaaaaaaaaaaaaaaaaaaaaaac', 'x'),
    (r'(\d+)-(\d+)', '10-20 30-40', '$2-$1'), ('.', 'aé中😀', '.'), ('(?s).', 'a\nb', '_'), ('', 'aé', '$$'),
    (r'\b{start}\w', 'ab cd', 'X'), (r'\w\b{end}', 'ab cd', 'X'), (r'\<', 'ab cd', '|'), (r'\>', 'ab cd', '|'),
    ('a|', 'aba', '-'), ('|a', 'aba', '-'), ('(|a)*', 'aa', '<$1>'), ('(a*)*', 'b', '<$1>'), ('(a|b)*?c', 'abc', '$1'),
    ('(?U)a+', 'aaa', 'x'), ('(?U)a+?', 'aaa', 'x'), ('[^a]', 'aé', 'x'), (r'\S+', ' a\tb\u00a0c\u2028d ', '[$0]'),
    ('ab|ab|ab', 'xabx', '$0$0'), ('abc|abd|xyz', 'abd xyz', 'M'),
]


def run(args):
    return subprocess.run(args, capture_output=True, text=True).stdout.split('\n')


cases = list(MANUAL) if seed == 0 else []
while len(cases) < count:
    cases.append((gen(0), hay(), repl()))
path = '/tmp/rx_busca_%d.txt' % seed
with open(path, 'w') as f:
    for p, t, r in cases:
        f.write(p.encode().hex() + '\n' + t.encode().hex() + '\n' + r.encode().hex() + '\n')
vm = run(['zett', 'run', os.path.join(here, 'busca.titan'), path])
na = run([nat, path])
bad = 0
errs = 0
for i, (p, t, r) in enumerate(cases):
    a = vm[i] if i < len(vm) else '<falta>'
    b = na[i] if i < len(na) else '<falta>'
    if a.startswith('ERR'):
        errs += 1
    if a != b:
        bad += 1
        if bad <= 10:
            print('DIFERENCIA', repr(p), repr(t), repr(r))
            print('  VM:    ', a[:300])
            print('  nativo:', b[:300])
print('casos', len(cases), 'errores', errs, 'diferencias', bad)
