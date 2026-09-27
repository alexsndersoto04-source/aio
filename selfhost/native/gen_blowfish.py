#!/usr/bin/env python3
"""Genera std_blowfish_tab.titan: el estado inicial de Blowfish (P, 18
palabras, y las 4 cajas S de 256 palabras), que por definición (Schneier,
1993) son los dígitos hexadecimales de la parte fraccionaria de pi, en orden.

pi se calcula aquí con la fórmula de Machin
    pi = 16·arctan(1/5) − 4·arctan(1/239)
en aritmética entera exacta (con dígitos de guarda), sin tablas copiadas.
Se comprueba con los valores publicados de P[0], P[17], S0[0] y S3[255].

Uso: python3 selfhost/native/gen_blowfish.py
"""
import os

WORDS = 18 + 4 * 256
HEX = WORDS * 8
GUARD = 64


def arctan_inv(x, one):
    # arctan(1/x)·one con la serie de Taylor
    total = term = one // x
    x2 = x * x
    n = 1
    sign = -1
    while term:
        term //= x2
        total += sign * (term // (2 * n + 1))
        sign = -sign
        n += 1
    return total


def pi_hex_fraction(digits):
    bits = 4 * digits + GUARD
    one = 1 << bits
    pi = 16 * arctan_inv(5, one) - 4 * arctan_inv(239, one)
    frac = pi - 3 * one
    return (frac >> GUARD) , digits


def main():
    frac, digits = pi_hex_fraction(HEX)
    h = format(frac, '0%dx' % digits)
    assert len(h) == HEX
    words = [int(h[8 * i:8 * i + 8], 16) for i in range(WORDS)]
    # Valores publicados (Schneier, "Description of a New Variable-Length
    # Key, 64-Bit Block Cipher (Blowfish)").
    assert words[0] == 0x243F6A88 and words[1] == 0x85A308D3
    assert words[17] == 0x8979FB1B
    assert words[18] == 0xD1310BA6
    assert words[18 + 3 * 256 + 255] == 0x3AC372E6
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, 'std_blowfish_tab.titan'), 'w') as f:
        f.write('// GENERADO por gen_blowfish.py: estado inicial de Blowfish (P y cajas S),\n')
        f.write('// los dígitos hexadecimales de pi calculados con la fórmula de Machin.\n\n')
        f.write('fn bf_init_hex() -> string {\n')
        parts = [h[i:i + 96] for i in range(0, HEX, 96)]
        f.write(' +\n'.join('    "%s"' % p for p in parts) + '\n')
        f.write('}\n')
    print('palabras', WORDS)


if __name__ == '__main__':
    main()
