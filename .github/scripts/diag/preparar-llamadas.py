#!/usr/bin/env python3
"""Prepara la verificación de llamadas: dos cuentas de prueba con conversación.

Uso: python3 preparar-llamadas.py <url-api> <archivo-de-salida.json>

Las dos cuentas quedan ocultas (no buscables y privadas): no aparecen en el
inicio, ni en sugerencias, ni en búsquedas.
"""
import json
import sys
import urllib.error
import urllib.request

API, SALIDA = sys.argv[1], sys.argv[2]
CLAVE = 'clave-de-moon-2026'
CUENTAS = ['pruebafotos', 'prueballamada']


def pedir(metodo, ruta, cuerpo=None, tok=None):
    r = urllib.request.Request(API + ruta, method=metodo)
    if tok:
        r.add_header('Authorization', 'Bearer ' + tok)
    datos = json.dumps(cuerpo).encode() if cuerpo is not None else None
    if datos:
        r.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(r, datos, timeout=40) as f:
            return json.load(f)
    except urllib.error.HTTPError as e:
        try:
            return {'_error': e.code, '_detalle': e.read().decode()[:200]}
        except Exception:
            return {'_error': e.code}
    except Exception as e:
        return {'_error': str(e)}


def entrar(nombre):
    reg = pedir('POST', '/api/auth/register',
                {'username': nombre, 'email': '%s@prueba.test' % nombre, 'password': CLAVE})
    datos = reg if reg.get('access_token') else pedir('POST', '/api/auth/login',
                                                      {'username': nombre, 'password': CLAVE})
    if not datos.get('access_token'):
        raise SystemExit('no se pudo entrar con %s: %s' % (nombre, datos))
    tok = datos['access_token']
    # Fuera de la vista de todo el mundo y con mensajes abiertos (solo entre
    # estas dos cuentas de prueba se hablan).
    pedir('PATCH', '/api/auth/privacy',
          {'searchable': False, 'is_private': True, 'dm_privacy': 'todos'}, tok=tok)
    yo = pedir('GET', '/api/auth/me', tok=tok)
    datos['user'] = yo
    return datos


salida = {}
for nombre in CUENTAS:
    salida[nombre] = entrar(nombre)

a, b = salida[CUENTAS[0]], salida[CUENTAS[1]]
conv = pedir('POST', '/api/messages/conversations', {'user_id': b['user']['id']}, tok=a['access_token'])
if not conv or conv.get('_error') or not conv.get('conversation_id'):
    raise SystemExit('no se pudo abrir la conversación entre las cuentas: %s' % conv)
salida['conversacion'] = {'id': conv['conversation_id']}

with open(SALIDA, 'w', encoding='utf-8') as f:
    json.dump(salida, f, ensure_ascii=False)
print('cuentas listas: %s(%s) y %s(%s) · conversacion=%s' % (
    CUENTAS[0], a['user'].get('id'), CUENTAS[1], b['user'].get('id'),
    (conv or {}).get('id')))
