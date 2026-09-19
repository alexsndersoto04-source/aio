#!/usr/bin/env python3
"""Prueba en vivo las opciones del menú «⋯» de una publicación.

Uso: python3 probar-opciones.py <url-api> <token> <id-de-mi-publicacion>

Usa la publicación de prueba del propio run (se borra al terminar), así que
no deja ningún rastro visible. Comprueba:
  - Editar: el servidor guarda el texto nuevo y marca la publicación.
  - Fijar: se fija y se suelta.
  - No me interesa: desaparece del inicio, de lo más nuevo y de buscar.
"""
import json
import sys
import urllib.error
import urllib.request

API, TOKEN, POST = sys.argv[1], sys.argv[2], str(sys.argv[3])


def req(metodo, ruta, cuerpo=None):
    r = urllib.request.Request(API + ruta, method=metodo)
    r.add_header('Authorization', 'Bearer ' + TOKEN)
    datos = json.dumps(cuerpo).encode() if cuerpo is not None else None
    if datos:
        r.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(r, datos, timeout=45) as f:
            return json.load(f)
    except urllib.error.HTTPError as e:
        try:
            detalle = e.read().decode()[:160]
        except Exception:
            detalle = ''
        return {'_error': e.code, '_detalle': detalle}
    except Exception:
        return None


def ids_en(ruta):
    """Ids de publicación que devuelve una lista del servidor."""
    datos = req('GET', ruta)
    if isinstance(datos, list):
        filas = datos
    elif isinstance(datos, dict):
        filas = datos.get('items') or datos.get('data') or datos.get('results') or datos.get('posts') or []
    else:
        filas = []
    return {str(f.get('id')) for f in filas if isinstance(f, dict)}


# ---------- Editar ----------
texto = 'prueba de foto (editada)'
editado = req('PATCH', '/api/posts/' + POST, {'content': texto})
if isinstance(editado, dict) and editado.get('content') == texto and editado.get('edited_at'):
    editar = 'editar: OK (guarda el texto y marca «editado»)'
elif isinstance(editado, dict) and editado.get('content') == texto:
    editar = 'editar: guarda el texto, pero no marca «editado»'
else:
    editar = 'editar: FALLA (%s)' % str(editado)[:120]

# ---------- Fijar ----------
puesto = req('POST', '/api/posts/' + POST + '/pin', {})
soltado = req('POST', '/api/posts/' + POST + '/pin', {})
if isinstance(puesto, dict) and puesto.get('pinned') and isinstance(soltado, dict) and not soltado.get('pinned'):
    fijar = 'fijar: OK (fija arriba y se puede soltar)'
else:
    fijar = 'fijar: FALLA (fijó=%s, soltó=%s)' % (
        (puesto or {}).get('pinned'), (soltado or {}).get('pinned'))

# ---------- No me interesa ----------
inicio_antes = ids_en('/api/feed?limit=50')
req('POST', '/api/posts/' + POST + '/interesa', {'no': True})
inicio = ids_en('/api/feed?limit=50')
nuevos = ids_en('/api/feed/latest?limit=50')
buscar = ids_en('/api/search?q=prueba+de+foto&type=posts&historial=0')
req('POST', '/api/posts/' + POST + '/interesa', {'no': False})  # se deja limpio
estaba = POST in inicio_antes
problemas = [nombre for nombre, conjunto in
             (('inicio', inicio), ('nuevos', nuevos), ('buscar', buscar)) if POST in conjunto]
if not estaba:
    interesa = 'no me interesa: no se pudo comprobar (la publicación no salía en el inicio)'
elif problemas:
    interesa = 'no me interesa: FALLA, sigue saliendo en %s' % ', '.join(problemas)
else:
    interesa = 'no me interesa: OK (desaparece del inicio, de lo más nuevo y de buscar)'

print(' | '.join([editar, fijar, interesa]))
