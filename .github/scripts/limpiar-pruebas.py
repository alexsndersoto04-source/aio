#!/usr/bin/env python3
"""Limpieza de cuentas y publicaciones de prueba.

Uso: python3 limpiar-pruebas.py <url-api> <token> <id-del-post-nuevo>

1. Borra el post de prueba que acaba de crear la verificación.
2. Busca publicaciones que contengan «prueba de foto» y las borra entrando
   con la cuenta de pruebas dueña de cada una (todas usan la misma clave),
   repitiendo hasta que no quede ninguna (máximo 3 pasadas).
3. Marca todas las cuentas «prueba*» como no buscables y privadas para que
   dejen de salir en sugerencias y búsquedas.
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


def entrar(api, nombre):
    for _ in range(2):
        ent = req(api, 'POST', '/api/auth/login',
                  {'username': nombre, 'password': CLAVE}) or {}
        if ent.get('access_token'):
            return ent['access_token']
        time.sleep(3)
    return None


def main():
    api, token, mio = sys.argv[1], sys.argv[2], sys.argv[3]
    posts_borrados = 0
    cuentas_ocultadas = 0
    sobras = -1

    if mio and mio.isdigit():
        ok = req(api, 'DELETE', '/api/posts/' + mio, tok=token)
        if ok and ok.get('ok'):
            posts_borrados += 1

    # Borrar publicaciones de prueba, repitiendo hasta que no quede ninguna.
    for pasada in range(3):
        posts = req(api, 'GET',
                    '/api/search?q=prueba+de+foto&type=posts&historial=0', tok=token) or []
        por_autor = {}
        for p in posts:
            if isinstance(p, dict) and 'id' in p:
                por_autor.setdefault(p.get('author_username') or '', []).append(p['id'])
        sobras = sum(len(v) for v in por_autor.values())
        if sobras == 0:
            break
        for autor, ids in por_autor.items():
            if not autor.startswith('prueba'):
                continue
            tk = entrar(api, autor)
            if not tk:
                continue
            for pid in ids:
                ok = req(api, 'DELETE', '/api/posts/' + str(pid), tok=tk)
                if ok and ok.get('ok'):
                    posts_borrados += 1
            time.sleep(2)

    # Ocultar las cuentas de prueba de sugerencias y búsquedas.
    usuarios = req(api, 'GET', '/api/search?q=prueba&type=users&historial=0', tok=token) or []
    nombres = sorted({str(u.get('username') or '') for u in usuarios
                      if isinstance(u, dict) and str(u.get('username') or '').startswith('prueba')})
    for nombre in nombres:
        tk = entrar(api, nombre)
        if not tk:
            continue
        if req(api, 'PATCH', '/api/auth/privacy',
               {'searchable': False, 'is_private': True}, tok=tk):
            cuentas_ocultadas += 1
        time.sleep(2)

    print('cuentas ocultadas: %d, publicaciones borradas: %d, pruebas restantes: %d'
          % (cuentas_ocultadas, posts_borrados, sobras))


if __name__ == '__main__':
    main()
