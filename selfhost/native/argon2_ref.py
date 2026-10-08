#!/usr/bin/env python3
# Argon2 de referencia (RFC 9106) en Python puro con hashlib.blake2b,
# para generar cadenas PHC con parámetros variados.
import hashlib, base64, random, struct, sys

M64 = (1 << 64) - 1


def blake2b(data, n):
    return hashlib.blake2b(data, digest_size=n).digest()


def hprime(data, n):
    x = struct.pack('<I', n) + data
    if n <= 64:
        return blake2b(x, n)
    out = bytearray()
    v = blake2b(x, 64)
    out += v[:32]
    while n - len(out) > 64:
        v = blake2b(v, 64)
        out += v[:32]
    out += blake2b(v, n - len(out))
    return bytes(out)


def rotr(x, n):
    return ((x >> n) | (x << (64 - n))) & M64


def G(v, a, b, c, d):
    def f(x, y):
        return (x + y + 2 * (x & 0xffffffff) * (y & 0xffffffff)) & M64
    v[a] = f(v[a], v[b]); v[d] = rotr(v[d] ^ v[a], 32)
    v[c] = f(v[c], v[d]); v[b] = rotr(v[b] ^ v[c], 24)
    v[a] = f(v[a], v[b]); v[d] = rotr(v[d] ^ v[a], 16)
    v[c] = f(v[c], v[d]); v[b] = rotr(v[b] ^ v[c], 63)


def P(v):
    G(v, 0, 4, 8, 12); G(v, 1, 5, 9, 13); G(v, 2, 6, 10, 14); G(v, 3, 7, 11, 15)
    G(v, 0, 5, 10, 15); G(v, 1, 6, 11, 12); G(v, 2, 7, 8, 13); G(v, 3, 4, 9, 14)


def compress(x, y):
    r = [a ^ b for a, b in zip(x, y)]
    q = r[:]
    for i in range(8):
        idx = list(range(16 * i, 16 * i + 16))
        v = [q[j] for j in idx]; P(v)
        for j, k in enumerate(idx): q[k] = v[j]
    for i in range(8):
        idx = []
        for k in range(8):
            idx += [2 * i + 16 * k, 2 * i + 16 * k + 1]
        v = [q[j] for j in idx]; P(v)
        for j, k in enumerate(idx): q[k] = v[j]
    return [a ^ b for a, b in zip(q, r)]


def argon2(alg, version, m, t, p, pwd, salt, data, outlen):
    h0 = blake2b(struct.pack('<IIIIII', p, outlen, m, t, version, alg)
                 + struct.pack('<I', len(pwd)) + pwd + struct.pack('<I', len(salt)) + salt
                 + struct.pack('<I', 0) + struct.pack('<I', len(data)) + data, 64)
    mb = max(m, 8 * p)
    seg = mb // (4 * p)
    ll = 4 * seg
    count = ll * p
    B = [None] * count
    for l in range(p):
        for i in range(2):
            h = hprime(h0 + struct.pack('<II', i, l), 1024)
            B[l * ll + i] = list(struct.unpack('<128Q', h))
    zero = [0] * 128
    for ps in range(t):
        for sl in range(4):
            indep = alg == 1 or (alg == 2 and ps == 0 and sl < 2)
            for lane in range(p):
                addr = [0] * 128
                inp = [0] * 128
                if indep:
                    inp[:6] = [ps, lane, sl, count, t, alg]
                first = 0
                if ps == 0 and sl == 0:
                    if indep:
                        inp[6] += 1; addr = compress(zero, compress(zero, inp))
                    first = 2
                cur = lane * ll + sl * seg + first
                prev = cur + ll - 1 if (sl == 0 and first == 0) else cur - 1
                for blk in range(first, seg):
                    if indep:
                        ai = blk % 128
                        if ai == 0:
                            inp[6] += 1; addr = compress(zero, compress(zero, inp))
                        rnd = addr[ai]
                    else:
                        rnd = B[prev][0]
                    ref_lane = lane if (ps == 0 and sl == 0) else (rnd >> 32) % p
                    minus = 1 if blk == 0 else 0
                    if ps == 0:
                        if sl == 0:
                            area = blk - 1
                        elif ref_lane == lane:
                            area = sl * seg + blk - 1
                        else:
                            area = sl * seg - minus
                    else:
                        area = ll - seg + blk - 1 if ref_lane == lane else ll - seg - minus
                    mp = ((rnd & 0xffffffff) ** 2) >> 32
                    rel = area - 1 - ((area * mp) >> 32)
                    start = (sl + 1) * seg if (ps != 0 and sl != 3) else 0
                    ri = ref_lane * ll + (start + rel) % ll
                    res = compress(B[prev], B[ri])
                    if version == 16 or ps == 0:
                        B[cur] = res
                    else:
                        B[cur] = [a ^ b for a, b in zip(B[cur], res)]
                    prev = cur
                    cur += 1
    fin = B[ll - 1][:]
    for l in range(1, p):
        fin = [a ^ b for a, b in zip(fin, B[l * ll + ll - 1])]
    return hprime(struct.pack('<128Q', *fin), outlen)


def b64(b):
    return base64.b64encode(b).decode().rstrip('=')


def phc(alg, version, m, t, p, pwd, salt, data, outlen, with_v=True, keyid=None):
    h = argon2(alg, version, m, t, p, pwd, salt, data, outlen)
    name = ['argon2d', 'argon2i', 'argon2id'][alg]
    s = '$' + name
    if with_v:
        s += '$v=%d' % version
    params = 'm=%d,t=%d,p=%d' % (m, t, p)
    if keyid is not None:
        params += ',keyid=' + b64(keyid)
    if data:
        params += ',data=' + b64(data)
    s += '$' + params + '$' + b64(salt) + '$' + b64(h)
    return s


if __name__ == '__main__':
    random.seed(int(sys.argv[1]) if len(sys.argv) > 1 else 1)
    cases = []
    for k in range(int(sys.argv[2]) if len(sys.argv) > 2 else 24):
        alg = random.randrange(3)
        version = random.choice([16, 19])
        p = random.choice([1, 1, 2, 3, 4])
        m = random.choice([8 * p, 8 * p + 3, 16 * p, 33, 64, 100]) 
        if m < 8 * p:
            m = 8 * p
        t = random.choice([1, 2, 3])
        pwd = bytes(random.choice(b"abcXYZ019 !#%&()*+,-./:;<=>?@[]^_|~") for _ in range(random.randrange(0, 20)))
        salt = bytes(random.randrange(256) for _ in range(random.choice([8, 9, 16, 33, 48])))
        data = bytes(random.randrange(256) for _ in range(random.choice([0, 0, 1, 7, 32])))
        outlen = random.choice([10, 16, 32, 33, 63, 64])
        keyid = random.choice([None, None, bytes(random.randrange(256) for _ in range(random.randrange(0, 9)))])
        with_v = version == 16 or random.random() < 0.7
        s = phc(alg, version, m, t, p, pwd, salt, data, outlen, with_v, keyid)
        cases.append((s, pwd.decode()))
    for s, pw in cases:
        print(s + '\t' + pw)
