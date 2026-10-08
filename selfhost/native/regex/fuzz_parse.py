#!/usr/bin/env python3
# Compara el analizador de regex en Titan (nat_parse, compilado con
# prueba_interna.titan) con la VM de Rust (vm_parse.titan) sobre patrones
# aleatorios y casos escritos a mano.
#   - Si la VM da un error del analizador (mensaje de ast::ErrorKind), el
#     texto del nativo debe ser idéntico.
#   - Si la VM acepta el patrón o da otro error (traducción, tamaño), el
#     analizador nativo debe aceptarlo.
# Uso: python3 fuzz_parse.py NATIVO SEMILLA CANTIDAD [hir|todo]
# Con 'todo' también se compara el error de tamaño del NFA.
# Con 'hir' el nativo es nat_hir (análisis + traducción): todos los errores
# deben coincidir salvo el de tamaño ("Compiled regex exceeds size limit").
import os
import random
import subprocess
import sys

nat, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
hir = len(sys.argv) > 4 and sys.argv[4] in ('hir', 'todo')
todo = len(sys.argv) > 4 and sys.argv[4] == 'todo'
rnd = random.Random(seed)
here = os.path.dirname(os.path.abspath(__file__))

AST = {
    'exceeded the maximum number of capturing groups (4294967295)',
    'invalid escape sequence found in character class',
    'invalid character class range, the start must be <= the end',
    'invalid range boundary, must be a literal',
    'unclosed character class', 'decimal literal empty',
    'decimal literal invalid', 'hexadecimal literal empty',
    'hexadecimal literal is not a Unicode scalar value',
    'invalid hexadecimal digit',
    'incomplete escape sequence, reached end of pattern prematurely',
    'unrecognized escape sequence', 'dangling flag negation operator',
    'duplicate flag', 'flag negation operator repeated',
    'expected flag but got end of regex', 'unrecognized flag',
    'duplicate capture group name', 'empty capture group name',
    'invalid capture group character', 'unclosed capture group name',
    'unclosed group', 'unopened group',
    'exceed the maximum number of nested parentheses/brackets (250)',
    'invalid repetition count range, the start must be <= the end',
    'repetition quantifier expects a valid decimal',
    'unclosed counted repetition', 'repetition operator missing expression',
    'special word boundary assertion is either unclosed or contains an invalid character',
    'unrecognized special word boundary assertion, valid choices are: start, end, start-half or end-half',
    'found either the beginning of a special word boundary or a bounded repetition on a \\b with an opening brace, but no closing brace',
    'invalid Unicode character class', 'backreferences are not supported',
    'look-around, including look-ahead and look-behind, is not supported',
}

TOK = ['a', 'b', 'z', '0', '9', '_', ' ', '\n', '\r', '\t', '#', '.', '^', '$', '|', '(', ')', '[', ']',
       '{', '}', '*', '+', '?', '-', '&&', '--', '~~', '&', '~', ',', ':', '<', '>', '=', '!', 'P', 'p',
       '\\', '\\d', '\\D', '\\w', '\\s', '\\W', '\\b', '\\B', '\\A', '\\z', '\\<', '\\>', '\\n', '\\t',
       '\\x', '\\x4', '\\x41', '\\xZ', '\\x{', '\\x{41}', '\\x{}', '\\x{110000}', '\\x{D800}', '\\x{ffffffffff}',
       '\\u0041', '\\u12', '\\U0001F600', '\\UFFFFFFFF', '\\u{2603}', '\\p', '\\pL', '\\pN', '\\p{', '\\p{L}',
       '\\p{Greek}', '\\p{sc=Greek}', '\\p{gc:Lu}', '\\p{sc!=Latn}', '\\P{Lu}', '\\p\\', '\\1', '\\0', '\\8',
       '\\q', '\\e', '\\.', '\\-', '\\ ', '\\#', '\\&', '\\~', '\\%', '\\"', '\\é', '\\b{start}', '\\b{end}',
       '\\b{start-half}', '\\b{end-half}', '\\b{', '\\b{x}', '\\b{star', '\\b{ start }', '\\b{3}', '\\b{',
       '(?', '(?i)', '(?x)', '(?-x)', '(?i:', '(?-)', '(?i-)', '(?--i)', '(?ii)', '(?q)', '(?)', '(?:',
       '(?P<', '(?P<n>', '(?<n>', '(?<n1>', '(?<a.b[c]>', '(?<1>', '(?<>', '(?<n', '(?=', '(?!', '(?<=',
       '(?<!', '(?smUuRx)', '(?x:', '{2}', '{2,}', '{2,5}', '{5,2}', '{,5}', '{ 2 , 5 }', '{', '{2', '{2,',
       '{x}', '{4294967296}', '{4294967295}', '{99999999999}', '[:alpha:]', '[[:alpha:]]', '[[:^digit:]]',
       '[[:foo:]]', '[[:alpha:', '[^', '[]', '[]]', '[^]]', '[-]', '[--]', '[a-]', '[a-z]', '[z-a]',
       '[\\d-z]', '[a-\\d]', '[\\A]', '[\\b]', '[a&&b]', '[a--b]', '[a~~b]', '[[a]&&[b]]', 'é', '☃', '𝒜',
       '\u0085', '\u00a0', '\u2028', '\u3000', 'Ω', 'ﬁ',
       '(?-u)', '(?u)', '(?i)', '(?-i)', '(?s)', '(?m)', '(?R)', '(?U)', '(?-u:', '(?i-u:', '(?-u)\\xff',
       '(?-u)\\x7f', '(?-u)[\\xff]', '(?-u)[é]', '(?-u)é', '(?-u).', '(?-u)\\W', '(?-u)\\w', '(?-u)\\pL',
       '(?-u)[^a]', '(?-u)[[:^alpha:]]', '(?-u)[\\x00-\\x7f]', '(?-u)\\S', '(?-u)\\D', '(?s-u).',
       '\\p{Greek}', '\\p{greek}', '\\p{Is_Greek}', '\\p{isgreek}', '\\p{sc=grek}', '\\p{scx=Grek}',
       '\\p{Script_Extensions=Greek}', '\\p{gc=L}', '\\p{General_Category=Lu}', '\\p{Lu}', '\\p{L&}',
       '\\p{LC}', '\\p{lc}', '\\p{Cf}', '\\p{cf}', '\\p{sc}', '\\p{Sc}', '\\p{isc}', '\\p{Is c}',
       '\\p{Any}', '\\p{Assigned}', '\\p{ASCII}', '\\p{age=6.0}', '\\p{Age=V1_1}', '\\p{age:3.2}',
       '\\p{age=99}', '\\p{Alphabetic}', '\\p{alpha}', '\\p{White_Space}', '\\p{wspace}',
       '\\p{Decimal_Number}', '\\p{nd}', '\\p{gcb=Extend}', '\\p{wb=ALetter}', '\\p{sb=Upper}',
       '\\p{Grapheme_Cluster_Break=LV}', '\\p{bc=L}', '\\p{Bidi_Class=AL}', '\\p{Block=Basic_Latin}',
       '\\p{foo}', '\\p{sc=foo}', '\\p{foo=bar}', '\\p{gc=foo}', '\\p{Emoji}', '\\p{Extended_Pictographic}',
       '\\p{Changes_When_Casefolded}', '\\p{cwcf}', '\\p{Age}', '\\p{Script}', '\\p{L-}', '\\p{ L }',
       '\\p{é}', '\\pé', '\\p{Grek}', '\\p{Latin}', '\\p{Han}', '\\P{Han}', '\\p{sc!=Han}', '\\P{sc!=Han}',
       '\\pZ', '\\pC', '\\pX', '\\pz', '[\\pL&&\\p{Greek}]', '[\\w--\\p{Latin}]', '(?i)[a-z&&k]',
       '(?i)k', '(?i)ß', '(?i)σ', '(?i)Σ', '(?i)[^k]', '(?i)\\p{Lu}', '(?i)[[:upper:]]']


ATOM = ['a', 'b', 'xy', '.', '\\d', '\\W', '\\pL', '\\p{Greek}', '\\x{263a}', '\\u00e9', '[a-z]', '[^0-9_]',
        '[[:alpha:]\\d]', '[a-z&&[^aeiou]]', '[\\w--\\d]', '[a~~b]', '^', '$', '\\b', '\\B', '\\A', '\\z',
        '\\b{start}', '\\<', 'é', '☃', '\\.', '\\n', '[]a]', '[-a]', '[a-]', '(?i)', '(?-u)']
REP = ['', '', '', '*', '+', '?', '*?', '{2}', '{1,3}', '{2,}?', '{0}', '{ 1 , 2 }']


def gen(d):
    r = rnd.random()
    if d > 5 or r < 0.35:
        s = rnd.choice(ATOM)
    elif r < 0.55:
        s = ''.join(gen(d + 1) for _ in range(rnd.randint(1, 4)))
    elif r < 0.7:
        s = '|'.join(gen(d + 1) for _ in range(rnd.randint(2, 3)))
    else:
        g = rnd.choice(['(', '(?:', '(?i:', '(?x:', '(?P<n%d>' % rnd.randint(0, 3), '(?<m%d>' % rnd.randint(0, 3)])
        s = g + gen(d + 1) + ')'
    return s + rnd.choice(REP)


def rpat():
    r = rnd.random()
    if r < 0.5:
        s = gen(0)
        if rnd.random() < 0.3:
            i = rnd.randint(0, len(s))
            s = s[:i] + rnd.choice(TOK) + s[i + rnd.randint(0, 2):]
        return s
    n = rnd.randint(0, 12)
    s = ''.join(rnd.choice(TOK) for _ in range(n))
    if r < 0.2:
        s = '(?x)' + s
    elif r < 0.25:
        s = '(' * rnd.randint(245, 255) + s + ')' * rnd.randint(245, 255)
    elif r < 0.28:
        s = '[' * rnd.randint(1, 260) + 'a' + ']' * rnd.randint(1, 260)
    elif r < 0.30:
        s = 'a' + '*' * rnd.randint(248, 253)
    return s


MANUAL = [
    '', 'a', '(', ')', 'a)', '(a', 'a|b)', '(a|b', '((a)', '[', '[a', '[^', '[]', '[a-', 'a{', 'a{1',
    'a{1,', 'a{1,2', '*', '+a', '?', '{1}', '(?i)*', '(*)', '(|*)', '|*', 'a**', 'a*?', 'a{1}?', '\\',
    'a\\', '\\x', '\\x{', '\\u{', '\\p', '\\p{', '\\p{L', '\\b{', '\\b{a', '(?i', '(?i-', '(?-', '(?P',
    '(?P<', '(?P<a', '(?P<a>', '(?P<a>x)(?P<a>y)', '(?<b>x)(?<a>y)(?<b>z)', '(?<é>x)', '(?<_1>x)',
    '(?<1a>x)', '(?<a-b>x)', 'a\nb(', 'ab\n(?<x>a)(?<x>b)', '(\n\n)\n)', '\r\n(', '(\r', 'x\n\n',
    '(?x) a { 2 }', '(?x)a{ 2 , 3 }', '(?x)\\x 4 1', '(?x)\\x{ 4 1 }', '(?x)\\p { L }', '(?x)[ a - z ]',
    '(?x)[a - ]', '(?x)[a -#c\n]', '(?x)[a -#c\nb]', '(?x)# comment\n(', '(?x:a b)c d', '(?x)(?-x)a b',
    '(?x)\\b{ start }', '(?x)\\b {start}', '(?x)(? i)', '(?x)( ?i)', '(?x)a{ }', '\\b{start}', 'a{,5}',
    'a{ 5 }', 'a{5 ,}', 'a{05}', 'a{0}', '[[:alpha:]-z]', '[a-[:alpha:]]', '[\\pL-z]', '[a-\\pL]',
    '[\\x{61}-\\x{7a}]', '[&&]', '[a&&]', '[&&a]', '[a&&&b]', '[a---b]', '[a-z--x]', '[]-a]', '[^]-a]',
    '[---]', '[-a-]', '[a-z&&[^x]]', '[[:alpha:][:digit:]]', '[[:alpha:]', '[[:alpha', '[[:', '[[',
    '[[]]', '[a[b]c]', '[[a]', 'x{2}{3}', 'x{3,2}', '(?:)', '()', '(?:a|)', '(|)', '||', '(?i)(?-i)',
    '\\ba\\B\\A\\z\\<\\>', '\\Q', '\\K', '\\G', '\\Z', '\\h', '\\cA', '\\N', '\\R', '\\X', '\\o{1}',
    '\\💩', '\\ ', '(?<a>(?<b>(?<a>x)))', '(?P=a)', '(?#c)', '(?|a)', '(?>a)', '(?R)', '(?1)',
    'a{4294967295}', 'a{4294967296}', 'a{1,4294967296}', '\\x{00000000000041}', '\\x{FFFFFFFF}',
    '\\x{100000000}', '\\u{D7FF}', '\\u{E000}', '\\u{DFFF}', '\\U0010FFFF', '\\U00110000',
    'é(', 'éé\néé(?<ü>)(?<ü>)', '☃\n☃☃)', '(?<n>x)\n(?<n>y)', 'a\n(?ii)', '(?i\ni)',
]


def run(prog, path):
    out = subprocess.run(prog + [path], capture_output=True, text=True, timeout=3600)
    if out.returncode != 0:
        print('fallo al ejecutar', prog, out.stderr[-2000:])
        sys.exit(1)
    return out.stdout.split('\n')


pats = list(MANUAL) if seed == 0 else []
while len(pats) < count:
    pats.append(rpat())
path = '/tmp/rx_fuzz_%d.txt' % seed
with open(path, 'w') as f:
    for p in pats:
        f.write(p.encode('utf-8').hex() + '\n')
vm = run(['zett', 'run', os.path.join(here, 'vm_parse.titan')], path)
na = run([nat], path)
bad = 0
nerr = 0
for i, p in enumerate(pats):
    v, n = vm[i], na[i]
    msg = None
    if v != 'OK':
        e = bytes.fromhex(v).decode('utf-8')
        pre = "native function 'std::regex::is_match' failed: invalid regular expression '" + p + "': "
        assert e.startswith(pre), e
        msg = e[len(pre):]
        last = msg.rsplit('error: ', 1)[-1] if 'error: ' in msg else ''
        if hir:
            if not todo and msg.startswith('Compiled regex exceeds size limit'):
                msg = None
        elif last not in AST:
            msg = None
    got = None if n == 'OK' else bytes.fromhex(n).decode('utf-8')
    if msg is not None:
        nerr += 1
    if got != msg:
        bad += 1
        if bad <= 12:
            print('DIFERENCIA en', repr(p))
            print('  VM:    ', repr(msg))
            print('  nativo:', repr(got))
print('casos', len(pats), 'errores del analizador', nerr, 'diferencias', bad)
sys.exit(1 if bad else 0)
