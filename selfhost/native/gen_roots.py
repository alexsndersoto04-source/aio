#!/usr/bin/env python3
"""Genera std_roots.titan: las anclas de confianza (nombre del sujeto y clave pública de cada
raíz) del paquete Mozilla CA (certifi: `pip download certifi`).
Uso: python3 gen_roots.py cacert.pem > std_roots.titan
Formato del bloque (base64): por ancla, u16 largo + sujeto (Name DER) + u16 largo + SPKI (DER)."""
import sys, re, base64

def tlv(b, i):
    tag = b[i]
    l = b[i + 1]
    k = i + 2
    if l & 0x80:
        n = l & 0x7F
        l = int.from_bytes(b[k:k + n], 'big')
        k += n
    return tag, k, l, k + l

def parse(der):
    t, s, l, e = tlv(der, 0)
    t, s2, l2, e2 = tlv(der, s)          # tbs
    i = s2
    t, a, b, c = tlv(der, i)
    if t == 0xA0:
        i = c
    t, a, b, c = tlv(der, i); i = c      # serial
    t, a, b, c = tlv(der, i); i = c      # sigalg
    t, a, b, c = tlv(der, i); i = c      # issuer
    t, a, b, c = tlv(der, i); i = c      # validity
    t, a, b, c = tlv(der, i)
    subject = der[i:c]; i = c            # subject
    t, a, b, c = tlv(der, i)
    spki = der[i:c]
    return subject, spki

pem = open(sys.argv[1]).read()
blocks = re.findall(r'-----BEGIN CERTIFICATE-----(.*?)-----END CERTIFICATE-----', pem, re.S)
out = b''
for blk in blocks:
    der = base64.b64decode(''.join(blk.split()))
    subj, spki = parse(der)
    out += len(subj).to_bytes(2, 'big') + subj + len(spki).to_bytes(2, 'big') + spki
b64 = base64.b64encode(out).decode()
print('// GENERADO por gen_roots.py a partir de certifi (Mozilla CA). No editar.')
print('// %d anclas de confianza: sujeto y clave pública (SPKI) de cada una.' % len(blocks))
print('')
print('fn xr_count() -> int { %d }' % len(blocks))
print('')
print('fn xr_data() -> string {')
chunks = [b64[i:i + 4000] for i in range(0, len(b64), 4000)]
print('    let mut s = ""')
for c in chunks:
    print('    s = s + "%s"' % c)
print('    s')
print('}')
