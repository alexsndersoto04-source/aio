#!/usr/bin/env python3
"""Limpia las publicaciones de prueba de fotos.

Uso: python3 limpiar-pruebas.py <url-api> <token> <id-del-post-nuevo>

1. Borra el post de prueba que acaba de crear la verificación.
2. Busca publicaciones que contengan «prueba de foto» y las borra entrando
   con la cuenta de pruebas dueña de cada una (todas usan la misma clave).
"""
import json
import sys
import urllib.request


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
    borradas = 0
    if mio and mio.isdigit():
        ok = req(api, 'DELETE', '/api/posts/' + mio, tok=token)
        if ok and ok.get('ok'):
            borradas += 1
    posts = req(api, 'GET', '/api/search?q=prueba+de+foto&type=posts&historial=0', tok=token) or []
    por_autor = {}
    for p in posts:
        if isinstance(p, dict) and 'id' in p:
            por_autor.setdefault(p.get('author_username') or '', []).append(p['id'])
    for autor, ids in por_autor.items():
        if not autor.startswith('prueba'):
            continue
        ent = req(api, 'POST', '/api/auth/login',
                  {'username': autor, 'password': 'clave-de-moon-2026'}) or {}
        tk = ent.get('access_token')
        if not tk:
            continue
        for pid in ids:
            ok = req(api, 'DELETE', '/api/posts/' + str(pid), tok=tk)
            if ok and ok.get('ok'):
                borradas += 1
    print('pruebas borradas: ' + str(borradas))


if __name__ == '__main__':
    main()
