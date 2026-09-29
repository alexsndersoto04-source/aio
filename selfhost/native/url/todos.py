#!/usr/bin/env python3
# Todos los puntos de código (menos los sustitutos) dentro de un dominio,
# "http://a<c>b.com/", y pares <c><marca>; compara la VM con el nativo.
# Uso: python3 todos.py NATIVO DESDE HASTA
import subprocess
import sys

nat, lo, hi = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3], 0)
marks = ['\u0301', '\u0308', '\u0345', '\u094D', '\u200D', '\u200C', '\u3099', '\u0F71']
lines = []
for c in range(lo, hi):
    if 0xD800 <= c <= 0xDFFF or c in (10, 13):
        continue
    ch = chr(c)
    lines.append(('U', 'http://a' + ch + 'b.com/'))
    lines.append(('U', 'http://' + ch + marks[c % len(marks)] + '.x/'))
open('/tmp/url_todos_in.txt', 'w', encoding='utf-8').write(''.join(x + '\n' for c in lines for x in c))
vm = subprocess.run(['zett', 'run', 'selfhost/native/url/fuzz.titan', '/tmp/url_todos_in.txt'], capture_output=True)
na = subprocess.run([nat, '/tmp/url_todos_in.txt'], capture_output=True)
a = vm.stdout.split(b'\n')
b = na.stdout.split(b'\n')
bad = 0
for i in range(max(len(a), len(b))):
    x = a[i] if i < len(a) else b'<nada>'
    y = b[i] if i < len(b) else b'<nada>'
    if x != y:
        bad += 1
        if bad <= 5:
            print('CASO', repr(lines[i][1]) if i < len(lines) else '?')
            print('  VM ', x.decode('utf-8', 'replace')[:300])
            print('  NAT', y.decode('utf-8', 'replace')[:300])
if vm.returncode != na.returncode or vm.stderr != na.stderr:
    print('salida distinta', vm.returncode, na.returncode, vm.stderr[:300], na.stderr[:300])
    bad += 1
print('%#x-%#x casos %d diferencias %d' % (lo, hi, len(lines), bad))
