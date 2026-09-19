#!/usr/bin/env python3
"""Limpieza de cuentas y publicaciones de prueba.

Uso: python3 limpiar-pruebas.py <url-api> <token> <id-del-post-nuevo>

1. Borra el post de prueba que acaba de crear la verificación.
2. Busca todas las cuentas cuyo nombre empiece por «prueba», entra en cada
   una (comparten la misma clave), las marca como no buscables (así dejan de
   salir en sugerencias y búsquedas) y borra cualquier publicación suya.
"""
import json
import sys
import time
import urllib.request

CLAVE = 'clave-de-moon-2026'


def req(api, metodo, ruta, cuerpo=None, tok=None):
    r = urllib.request.Request(api + ruta, method=metodo)
    if tok:
        r.add_header('Authorization', 'Bearer ' + tok)
    datos = json.dumps(cuerpo).encode() if cuerpo is not None else None
    if datos:
        r.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(r, datos, timeout=30) as f:
            return json.load(f)
    except Exception:
        return None


def main():
    api, token, mio = sys.argv[1], sys.argv[2], sys.argv[3]
    posts_borrados = 0
    cuentas_ocultadas = 0

    if mio and mio.isdigit():
        ok = req(api, 'DELETE', '/api/posts/' + mio, tok=token)
        if ok and ok.get('ok'):
            posts_borrados += 1

    usuarios = req(api, 'GET', '/api/search?q=prueba&type=users&historial=0', tok=token) or []
    nombres = []
    for u in usuarios:
        if isinstance(u, dict) and str(u.get('username') or '').startswith('prueba'):
            nombres.append(u['username'])
    for nombre in sorted(set(nombres)):
        ent = req(api, 'POST', '/api/auth/login',
                  {'username': nombre, 'password': CLAVE}) or {}
        tk = ent.get('access_token')
        if not tk:
            continue
        priv = req(api, 'PATCH', '/api/auth/privacy',
                   {'searchable': False, 'is_private': True}, tok=tk)
        if priv:
            cuentas_ocultadas += 1
        feed = req(api, 'GET', '/api/feed?limit=100', tok=tk) or []
        for p in feed:
            if isinstance(p, dict) and p.get('is_mine') and p.get('id'):
                ok = req(api, 'DELETE', '/api/posts/' + str(p['id']), tok=tk)
                if ok and ok.get('ok'):
                    posts_borrados += 1
        time.sleep(1)

    print('cuentas de prueba ocultadas: %d, publicaciones de prueba borradas: %d'
          % (cuentas_ocultadas, posts_borrados))


if __name__ == '__main__':
    main()
