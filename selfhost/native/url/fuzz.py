#!/usr/bin/env python3
# Casos aleatorios para std::url: genera N líneas, las pasa por la VM de Rust
# (zett run fuzz.titan) y por el ejecutable nativo, y compara línea a línea.
# Uso: python3 fuzz.py NATIVO SEMILLA CANTIDAD
import random
import subprocess
import sys

nat, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
rnd = random.Random(seed)

schemes = ['http', 'https', 'ws', 'wss', 'ftp', 'file', 'HTTP', 'sc', 'mailto', 'a+b', 'x-y.z', '1x', '']
uni = ('ÀàÉéÜüßẞſﬁﬀ﬩ＡａＺ０９．。｡、ǅǄΣσςİıK℃①Ⅻ㍱ℌ𝒜'
       'אבגעת١٢٣۱۲۳ابتثجع\u0600\u0660\u06F0\u064B\u0652\u0610'
       '\u0300\u0301\u0308\u0327\u0345\u034F\u0340\u0341\u0343\u0344\u0F73\u0F75\u0F81'
       '\uFF9E\uFF9F\u3099\u309A\u200C\u200D\u094D\u0915\u0928\u0BCD\u0B95'
       '\u00AD\u180B\uFE00\uFEFF\u2060\u200B\u2028\uFFFD\uFDFA\uFB2A'
       '가각ᄀᄁ\u1161\u1162\u11A8\u11C2ㄱ'
       '例えテスト中文☃😀👍🏽\U0001F1FA\U0001F1F8\U000E0100\U0010FFFD'
       '\u0080\u009F\u00A0\u1680\u3000\uD7FF\uE000\uFFFE')
asc = 'abcxyzABCXYZ0189-._~!$&\'()*+,;=:@/?#[]%^|<>\\` "\t'
tlds = ['com', 'org', 'de', 'xn--p1ai', 'XN--ZCKZAH', 'рф', 'テスト', 'عربي', '']


def rchar():
    r = rnd.random()
    if r < 0.55:
        return rnd.choice('abcdefghijklmnopqrstuvwxyz0123456789-')
    if r < 0.8:
        return rnd.choice(uni)
    return rnd.choice(asc)


def label():
    r = rnd.random()
    if r < 0.12:
        return 'xn--' + ''.join(rnd.choice('abcdefghijklmnopqrstuvwxyz0123456789-') for _ in range(rnd.randint(0, 12)))
    if r < 0.2:
        return '%' + rnd.choice(['41', 'C3%BC', 'E2%98%83', 'zz', '2E', '00', 'FF', '25', 'c3'])
    return ''.join(rchar() for _ in range(rnd.randint(0, 8)))


def host():
    r = rnd.random()
    if r < 0.1:
        return '[' + ':'.join(rnd.choice(['', '0', '1', 'ffff', 'FFFF', '12345', 'g', '1.2.3.4', '255.0.0.1', '01.2.3.4'])
                              for _ in range(rnd.randint(1, 9))) + ']'
    if r < 0.2:
        return '.'.join(rnd.choice(['0', '1', '255', '256', '0x7f', '0X', '010', '09', '4294967295', '', 'a'])
                        for _ in range(rnd.randint(1, 5)))
    n = rnd.randint(1, 4)
    parts = [label() for _ in range(n)]
    if rnd.random() < 0.6:
        parts.append(rnd.choice(tlds))
    return rnd.choice(['.', '.', '.', '。', '．']).join(parts)


def path():
    segs = [rnd.choice(['a', 'b', '.', '..', '%2e', '%2E%2e', 'C:', 'c|', 'é', 'x y', '%41', '', '😀', '?', '#'])
            for _ in range(rnd.randint(0, 5))]
    return rnd.choice(['/', '\\', '']) + '/'.join(segs)


def url():
    r = rnd.random()
    s = rnd.choice(schemes)
    if r < 0.75:
        u = s + ':' + rnd.choice(['//', '//', '/', '', '\\\\', '///'])
        if rnd.random() < 0.15:
            u += rnd.choice(['user', 'u:p', ':p', '', 'a@b', 'ü']) + '@'
        u += host()
        if rnd.random() < 0.2:
            u += ':' + rnd.choice(['80', '443', '0', '65535', '65536', '', '8a', '-1', '0080'])
        u += path()
    elif r < 0.9:
        u = s + ':' + ''.join(rchar() for _ in range(rnd.randint(0, 12)))
    else:
        u = ''.join(rchar() for _ in range(rnd.randint(0, 20)))
    if rnd.random() < 0.2:
        u += '?' + ''.join(rchar() for _ in range(rnd.randint(0, 8)))
    if rnd.random() < 0.15:
        u += '#' + ''.join(rchar() for _ in range(rnd.randint(0, 8)))
    if rnd.random() < 0.05:
        u = rnd.choice([' ', '\u0001', '\u001f']) + u + rnd.choice([' ', '\u0000', ''])
    return u


def clean(s):
    return s.replace('\n', ' ').replace('\r', ' ').replace('\t', ' ')


lines = []
for _ in range(count):
    r = rnd.random()
    if r < 0.7:
        lines.append(('U', clean(url())))
    elif r < 0.9:
        rel = rnd.choice([path(), '//' + host() + path(), '?' + label(), '#' + label(), url(), '..', '', label()])
        lines.append(('J', clean(url()), clean(rel)))
    else:
        q = '&'.join(label() + rnd.choice(['=', '', '==']) + label() for _ in range(rnd.randint(0, 5)))
        lines.append(('Q', clean(rnd.choice(['', '?']) + q.replace('%', rnd.choice(['%', '+', '%2'])))))
open('/tmp/url_fuzz_in.txt', 'w', encoding='utf-8').write(''.join(x + '\n' for c in lines for x in c))
vm = subprocess.run(['zett', 'run', 'selfhost/native/url/fuzz.titan', '/tmp/url_fuzz_in.txt'],
                    capture_output=True)
na = subprocess.run([nat, '/tmp/url_fuzz_in.txt'], capture_output=True)
a = vm.stdout.split(b'\n')
b = na.stdout.split(b'\n')
bad = 0
for i in range(max(len(a), len(b))):
    x = a[i] if i < len(a) else b'<nada>'
    y = b[i] if i < len(b) else b'<nada>'
    if x != y:
        bad += 1
        if bad <= 5:
            print('CASO', lines[i] if i < len(lines) else '?')
            print('  VM ', x.decode('utf-8', 'replace'))
            print('  NAT', y.decode('utf-8', 'replace'))
if vm.returncode != na.returncode or vm.stderr != na.stderr:
    print('salida distinta', vm.returncode, na.returncode, vm.stderr[:300], na.stderr[:300])
    bad += 1
print('semilla', seed, 'casos', count, 'diferencias', bad)
