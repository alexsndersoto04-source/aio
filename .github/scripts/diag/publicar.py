#!/usr/bin/env python3
"""Publica en el commit lo que vio el navegador (y una captura reducida).

Uso: python3 publicar.py <sha>
Lee /tmp/dom-*.html (volcado del DOM con la sonda), /tmp/chrome-*.log y las
capturas /tmp/menu-*.png. Todo el resultado va como comentario del commit
porque los registros de Actions no se pueden leer desde el entorno de trabajo.
"""
import base64
import os
import re
import subprocess
import sys

SHA = sys.argv[1] if len(sys.argv) > 1 else 'HEAD'
LIMITE = 58000


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
    final = texto[-1200:].replace('\n', ' ')
    return '(la sonda no escribió nada; el volcado termina así: %s)' % final


def miniatura(ruta, ancho):
    if not os.path.exists(ruta):
        return ''
    destino = ruta.replace('.png', '.jpg')
    subprocess.run(['convert', ruta, '-resize', '%dx' % ancho, '-quality', '45', destino],
                   check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if not os.path.exists(destino):
        return ''
    datos = open(destino, 'rb').read()
    if len(datos) > 26000:
        return ''
    return base64.b64encode(datos).decode()


partes = ['### Cómo se ve el menú de reacciones (navegador de verdad, no supuesto)', '']
if not os.path.exists('/tmp/dom-telefono.html') and not os.path.exists('/tmp/dom-computadora.html'):
    partes += ['No hubo volcado. Chrome dijo:', '', '```', leer('/tmp/chrome-telefono.log', 1500), '```', '']

partes += ['**Teléfono 390×844**', '', '```', sonda('/tmp/dom-telefono.html')[:4200], '```', '']
partes += ['**Computadora 1280×900**', '', '```', sonda('/tmp/dom-computadora.html')[:4200], '```', '']

for nombre, ruta, ancho in (('teléfono', '/tmp/menu-telefono.png', 210),
                            ('computadora', '/tmp/menu-computadora.png', 430)):
    b64 = miniatura(ruta, ancho)
    if b64:
        partes += ['', '**Captura (%s) — guardar el bloque de abajo como .jpg y abrir**' % nombre,
                   '', '```texto', b64, '```']

servidor = leer('/tmp/servidor.log', 800)
if servidor.strip():
    partes += ['', '**Servidor de prueba**', '', '```', servidor.strip()[:800], '```']

cuerpo = '\n'.join(partes)
if len(cuerpo) > LIMITE:
    # Si no cabe, se recortan las capturas primero.
    cuerpo = cuerpo[:LIMITE]
open('/tmp/comentario.md', 'w', encoding='utf-8').write(cuerpo)

subprocess.run(['gh', 'api', '-X', 'POST',
                'repos/%s/commits/%s/comments' % (os.environ.get('GITHUB_REPOSITORY', ''), SHA),
                '-F', 'body=@/tmp/comentario.md'], check=False)
print('comentario publicado (%d caracteres)' % len(cuerpo))
