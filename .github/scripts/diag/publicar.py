#!/usr/bin/env python3
"""Publica en el commit lo que vio el navegador (y una captura reducida).

Uso: python3 publicar.py <sha>
Nunca se cae: si algo falta, lo cuenta dentro del propio comentario.
"""
import base64
import os
import re
import subprocess
import sys
import traceback

SHA = sys.argv[1] if len(sys.argv) > 1 else 'HEAD'
LIMITE = 60000
notas = []


def leer(ruta, tope=100000):
    try:
        return open(ruta, encoding='utf-8', errors='replace').read()[:tope]
    except OSError as e:
        return '(no se pudo leer %s: %s)' % (ruta, e)


def sonda(ruta):
    texto = leer(ruta)
    m = re.search(r'DIAG-INICIO(.*?)DIAG-FIN', texto, re.S)
    if m:
        return m.group(1).strip()
    return '(la sonda no escribió nada; final del volcado: %s)' % texto[-900:].replace('\n', ' ')


def miniatura(ruta, ancho):
    if not os.path.exists(ruta):
        notas.append('sin captura %s' % ruta)
        return ''
    destino = ruta.replace('.png', '.jpg')
    for orden in (['convert', ruta, '-resize', '%dx' % ancho, '-quality', '45', destino],
                  ['magick', ruta, '-resize', '%dx' % ancho, '-quality', '45', destino]):
        try:
            subprocess.run(orden, check=False, timeout=60,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except OSError as e:
            notas.append('%s no está: %s' % (orden[0], e))
        if os.path.exists(destino):
            break
    if not os.path.exists(destino):
        notas.append('no se pudo reducir %s' % ruta)
        return ''
    datos = open(destino, 'rb').read()
    notas.append('%s -> %d bytes' % (destino, len(datos)))
    if len(datos) > 24000:
        return ''
    return base64.b64encode(datos).decode()


partes = ['### Cómo se ve el menú de reacciones (navegador de verdad, no supuesto)', '']
try:
    partes += ['**Teléfono 390×844**', '', '```', sonda('/tmp/dom-telefono.html')[:4200], '```', '']
    partes += ['**Computadora 1280×900**', '', '```', sonda('/tmp/dom-computadora.html')[:4200], '```', '']
    for nombre, ruta, ancho in (('teléfono', '/tmp/menu-telefono.png', 210),
                                ('computadora', '/tmp/menu-computadora.png', 430)):
        b64 = miniatura(ruta, ancho)
        if b64:
            partes += ['', '**Captura (%s) — el bloque de abajo es un .jpg en base64**' % nombre,
                       '', '```texto', b64, '```']
    for archivo in ('/tmp/chrome-telefono.log', '/tmp/chrome-computadora.log', '/tmp/servidor.log'):
        contenido = leer(archivo, 700).strip()
        if contenido:
            partes += ['', '**%s**' % os.path.basename(archivo), '', '```', contenido[:700], '```']
except Exception:
    partes += ['', '**Falla al armar el informe**', '', '```', traceback.format_exc()[-1500:], '```']

if notas:
    partes += ['', '**Notas de la herramienta**: ' + ' · '.join(notas[:8])]

cuerpo = '\n'.join(partes)[:LIMITE]
open('/tmp/comentario.md', 'w', encoding='utf-8').write(cuerpo)

try:
    salida = subprocess.run(['gh', 'api', '-X', 'POST',
                             'repos/%s/commits/%s/comments' % (os.environ.get('GITHUB_REPOSITORY', ''), SHA),
                             '-F', 'body=@/tmp/comentario.md'],
                            check=False, timeout=90, capture_output=True, text=True)
    print('gh dijo: %s %s' % (salida.returncode, (salida.stderr or '')[:300]))
except Exception:
    print(traceback.format_exc()[-800:])
print('informe de %d caracteres' % len(cuerpo))
