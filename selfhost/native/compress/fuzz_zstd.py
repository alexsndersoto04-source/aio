#!/usr/bin/env python3
# Prueba diferencial de std::compress::zstd_decode.
#   python3 fuzz_zstd.py DIR SEMILLA CANTIDAD [NATIVO...]
# Genera marcos zstd (válidos con la libzstd 1.5.7 de python-zstandard, con
# parámetros variados, varios marcos, marcos saltables, y versiones mutadas:
# bits cambiados, bytes cambiados, cortes, basura añadida) y compara, línea a
# línea, la salida de prueba.titan en la VM de Rust (zett run) con el modelo
# zstd_ref.py y, si se dan, con los binarios nativos.
import os, random, subprocess, sys
import zstandard

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import zstd_ref  # noqa: E402

ZETT = os.path.expanduser('~/.local/bin/zett')
PREFIX = "native function 'std::compress::zstd_decode' failed: "


def gen_data(rnd):
    n = rnd.choice([0, 1, 2, 7, 30, 100, 255, 256, 300, 1000, 4000, 8191, 8192, 8193,
                    20000, 70000, 131072, 140000, 300000])
    k = rnd.randint(0, 5)
    if k == 0:
        return rnd.randbytes(n)
    if k == 1:
        alpha = rnd.randbytes(rnd.randint(1, 40))
        return bytes(rnd.choice(alpha) for _ in range(n))
    if k == 2:
        words = [rnd.randbytes(rnd.randint(1, 12)) for _ in range(rnd.randint(1, 60))]
        out = bytearray()
        while len(out) < n:
            out += rnd.choice(words)
        return bytes(out[:n])
    if k == 3:
        return (b'hola mundo ' * (n // 11 + 1))[:n]
    if k == 4:
        # texto sesgado: pocos símbolos muy frecuentes (Huffman X2 / tablas grandes)
        alpha = rnd.randbytes(rnd.randint(2, 200))
        w = [rnd.random() ** 4 for _ in alpha]
        return bytes(rnd.choices(alpha, w, k=n))
    base = rnd.randbytes(max(1, n // 8))
    out = bytearray()
    while len(out) < n:
        if rnd.random() < 0.5 and len(out) > 8:
            p = rnd.randrange(len(out))
            out += out[p:p + rnd.randint(3, 300)]
        else:
            out += base[rnd.randrange(len(base)):][:rnd.randint(1, 50)]
    return bytes(out[:n])


def compress(rnd, data):
    lvl = rnd.choice([-5, -1, 1, 2, 3, 4, 5, 7, 9, 12, 15, 17, 19, 20, 22])
    kw = {}
    if rnd.random() < 0.3:
        kw['window_log'] = rnd.randint(10, 20)
    p = zstandard.ZstdCompressionParameters.from_level(
        lvl, write_checksum=rnd.random() < 0.5,
        write_content_size=rnd.random() < 0.7, **kw)
    cc = zstandard.ZstdCompressor(compression_params=p)
    mode = rnd.random()
    if mode < 0.6:
        return cc.compress(data)
    # flujo: sin tamaño de contenido, a veces con bloques vaciados a mitad
    co = cc.compressobj()
    out = bytearray()
    i = 0
    while i < len(data):
        k = rnd.randint(1, 50000)
        out += co.compress(data[i:i + k])
        if rnd.random() < 0.3:
            out += co.flush(zstandard.COMPRESSOBJ_FLUSH_BLOCK)
        i += k
    out += co.flush()
    return bytes(out)


def skippable(rnd):
    body = rnd.randbytes(rnd.choice([0, 1, 5, 100]))
    return (0x184D2A50 + rnd.randrange(16)).to_bytes(4, 'little') + len(body).to_bytes(4, 'little') + body


def mutate(rnd, b):
    b = bytearray(b)
    for _ in range(rnd.randint(1, 4)):
        k = rnd.random()
        if not b:
            b += rnd.randbytes(rnd.randint(1, 10))
            continue
        if k < 0.35:
            i = rnd.randrange(len(b))
            b[i] ^= 1 << rnd.randrange(8)
        elif k < 0.55:
            i = rnd.randrange(len(b))
            b[i] = rnd.randrange(256)
        elif k < 0.7:
            del b[rnd.randrange(len(b)):]
        elif k < 0.8:
            b += rnd.randbytes(rnd.randint(1, 20))
        elif k < 0.9:
            i = rnd.randrange(len(b))
            del b[i:i + rnd.randint(1, 8)]
        else:
            i = rnd.randrange(len(b))
            b[i:i] = rnd.randbytes(rnd.randint(1, 8))
    return bytes(b)


def case(rnd):
    parts = []
    for _ in range(1 if rnd.random() < 0.8 else rnd.randint(2, 3)):
        if rnd.random() < 0.1:
            parts.append(skippable(rnd))
        else:
            parts.append(compress(rnd, gen_data(rnd)))
    fr = b''.join(parts)
    if rnd.random() < 0.55:
        fr = mutate(rnd, fr)
    return fr


def model_line(fr):
    st, v = zstd_ref.zstd_decode(fr)
    if st == 'ok':
        return 'ok ' + v.hex()
    return 'ERR ' + (PREFIX + v).encode().hex()


def run(cmd, path):
    # Límite de tiempo: un cuelgue cuenta como fallo (las líneas que falten
    # salen como diferencias) en vez de bloquear la prueba.
    try:
        r = subprocess.run(cmd + [path], capture_output=True, text=True, timeout=900,
                           env={k: v for k, v in os.environ.items() if k != 'GITHUB_ACTIONS'})
        return r.stdout.split('\n')
    except subprocess.TimeoutExpired as e:
        print('TIEMPO AGOTADO', cmd[0])
        out = e.stdout or b''
        if isinstance(out, bytes):
            out = out.decode(errors='replace')
        return out.split('\n')


def main():
    outdir, seed, count = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    nats = sys.argv[4:]
    rnd = random.Random(seed)
    cases = [case(rnd) for _ in range(count)]
    path = os.path.join(outdir, 'zstd_%d.txt' % seed)
    with open(path, 'w') as f:
        for c in cases:
            f.write('s ' + c.hex() + '\n')
    vm = run([ZETT, 'run', os.path.join(here, 'prueba.titan')], path)
    others = [(n, run([n], path)) for n in nats]
    bad = 0
    oks = 0
    for i, c in enumerate(cases):
        exp = model_line(c)
        if exp.startswith('ok'):
            oks += 1
        got = [('vm', vm[i] if i < len(vm) else '<nada>')] + [(n, o[i] if i < len(o) else '<nada>') for n, o in others]
        for name, g in got:
            if g != exp:
                bad += 1
                if bad <= 5:
                    print('DIFERENCIA caso', i, name, 'entrada', c[:64].hex())
                    print('  esperado', exp[:200])
                    print('  obtenido', g[:200])
    print('semilla', seed, 'casos', count, 'ok', oks, 'diferencias', bad)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
