#!/usr/bin/env python3
# Prueba diferencial de std::router: prueba.titan en la VM de Rust (zett run)
# contra el mismo programa compilado con el backend nativo, con rutas y
# caminos aleatorios. Uso: python3 fuzz.py NATIVO SEMILLA CANTIDAD
import os
import random
import subprocess
import sys

nat, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
rnd = random.Random(seed)
here = os.path.dirname(os.path.abspath(__file__))

PIECE = ['/', '/', '/', 'a', 'b', 'ab', 'users', 'x', '{id}', '{name}', '{id}', '{*rest}', '{*p}', '{a}.png',
         '{x}-{y}', '.png', '.', '-', '{{', '}}', '{', '}', '{}', '{*}', '{a*}', '{a/b}', 'é', '中', '😀', '{é}',
         '{id}x', 'x{id}', '{f}.{e}', '/{id}', '/static', '/{*all}', '{{x}}', '{ab}}}c}', '{a}}b}', 'v1', '{ id }']
PATHP = ['/', '/', '/', 'a', 'b', 'ab', 'users', 'x', '42', 'é', '中', '😀', '.png', 'a.png', 'x-y', '-', '.',
         '{', '}', '{id}', 'static', 'v1', '', 'c', 'x.png.png', '%20', ' ', 'xx', 'a.b']
MANUAL = [
    (['/', '/users/{id}', '/files/{*rest}'], ['/', '/users/42', '/files/a/b/c.txt', '/nope', '/users/', '/files/']),
    (['/x', '/x'], ['/x']),
    (['/users/{id}', '/users/{name}'], ['/users/1']),
    (['/{a}.png', '/{a}.jpg', '/{a}'], ['/x.png', '/x.jpg', '/x', '/.png', '/x.png/']),
    (['/a/{x}/b', '/a/c/b', '/a/{y}/d'], ['/a/c/b', '/a/c/d', '/a/q/b', '/a/q/d']),
    (['/{{x}}', '/{{', '/}}'], ['/{x}', '/{', '/}', '/{{']),
    (['/{*rest}', '/a'], ['/a', '/b', '/', '/a/b']),
    (['/a', '/{*rest}'], ['/a', '/b', '/']),
    (['/{x}-{y}'], ['/a-b']),
    (['/{}', '/{a*}', '/{*r}/x', '/}', '/a{'], ['/']),
    (['/é/{id}', '/è/{id}', '/é'], ['/é/1', '/è/2', '/é', '/è']),
    ([''], ['', '/']),
    (['/{a}/{b}/{c}/{d}/{e}/{f}/{g}/{h}/{i}/{j}'], ['/1/2/3/4/5/6/7/8/9/10', '/1/2']),
    (['/api/v1/{*path}', '/api/{version}/users', '/api/v1/users'], ['/api/v1/users', '/api/v2/users', '/api/v1/x/y']),
]


def route():
    while True:
        s = ''.join(rnd.choice(PIECE) for _ in range(rnd.randint(1, 6)))
        if s.count('{') < 12:
            return s


def path():
    return ''.join(rnd.choice(PATHP) for _ in range(rnd.randint(0, 7)))


cases = list(MANUAL) if seed == 0 else []
while len(cases) < count:
    rs = [route() for _ in range(rnd.randint(1, 8))]
    ps = []
    for _ in range(rnd.randint(1, 10)):
        if rnd.random() < 0.5:
            # Un camino parecido a una ruta: los parámetros cambiados por texto.
            r = rnd.choice(rs)
            out, j = '', 0
            while j < len(r):
                if r[j] == '{' and '}' in r[j:]:
                    e = r.index('}', j)
                    out += rnd.choice(['42', 'x', 'a/b', '', 'é', 'x.png'])
                    j = e + 1
                else:
                    out += r[j]
                    j += 1
            ps.append(out)
        else:
            ps.append(path())
    cases.append((rs, ps))

hexs = lambda s: s.encode('utf-8').hex()
lines = []
for rs, ps in cases:
    lines.append(f'{len(rs)} {len(ps)}')
    lines += [hexs(r) for r in rs] + [hexs(p) for p in ps]
data = f'/tmp/router_fuzz_{seed}.txt'
open(data, 'w').write('\n'.join(lines) + '\n')

vm = subprocess.run(['zett', 'run', os.path.join(here, 'prueba.titan'), data], capture_output=True, text=True)
nv = subprocess.run([nat, data], capture_output=True, text=True)
if vm.returncode != 0 or nv.returncode != 0:
    print('código de salida VM', vm.returncode, 'nativo', nv.returncode)
    print(vm.stderr[-2000:])
    print(nv.stderr[-2000:])
a, b = vm.stdout.splitlines(), nv.stdout.splitlines()
diff = sum(1 for x, y in zip(a, b) if x != y) + abs(len(a) - len(b))
for x, y in list(zip(a, b)):
    if x != y:
        print('VM    ', x[:200], bytes.fromhex(x.split()[-1]).decode(errors='replace')[:200])
        print('nativo', y[:200], bytes.fromhex(y.split()[-1]).decode(errors='replace')[:200])
        break
errs = sum(1 for x in a if x.startswith('I ERR'))
print(f'casos {len(cases)} líneas {len(a)} errores de inserción {errs} diferencias {diff}')
