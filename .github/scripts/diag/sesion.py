#!/usr/bin/env python3
"""Escribe dist/diag-datos.js con la sesión que usará la sonda del navegador.

Uso: python3 sesion.py <respuesta-json-de-login-o-register> <archivo-de-salida>
"""
import json
import sys

origen, salida = sys.argv[1], sys.argv[2]
datos = json.load(open(origen, encoding='utf-8'))
partes = []
if datos.get('access_token'):
    partes.append('token: %s' % json.dumps(datos['access_token']))
if datos.get('refresh_token'):
    partes.append('refresh: %s' % json.dumps(datos['refresh_token']))
if datos.get('user'):
    partes.append('user: %s' % json.dumps(datos['user']))
with open(salida, 'w', encoding='utf-8') as f:
    f.write('window.__DIAG = { %s };\n' % ', '.join(partes))
print('diag-datos.js con: %s' % ', '.join(sorted(datos.keys())))
