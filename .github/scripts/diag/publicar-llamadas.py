#!/usr/bin/env python3
"""Publica en el commit el informe de la verificación de llamadas + capturas.

Uso: python3 publicar-llamadas.py <sha> <informe.json> <carpeta-de-capturas>
Nunca se cae: lo que falte se cuenta dentro del propio comentario.
"""
import base64
import glob
import json
import os
import subprocess
import sys
import traceback

SHA = sys.argv[1] if len(sys.argv) > 1 else 'HEAD'
INFORME = sys.argv[2] if len(sys.argv) > 2 else '/tmp/llamada.json'
FOTOS = sys.argv[3] if len(sys.argv) > 3 else '/tmp'
LIMITE = 60000
notas = []


def miniatura(ruta, ancho=300):
    destino = ruta.rsplit('.', 1)[0] + '.jpg'
    try:
        from PIL import Image
        im = Image.open(ruta).convert('RGB')
        if im.width > ancho:
            im = im.resize((ancho, max(1, round(im.height * ancho / im.width))))
        im.save(destino, 'JPEG', quality=42, optimize=True)
    except Exception as e:
        notas.append('miniatura de %s: %s' % (os.path.basename(ruta), e))
        return ''
    datos = open(destino, 'rb').read()
    notas.append('%s %d bytes' % (os.path.basename(destino), len(datos)))
    if len(datos) > 22000:
        return ''
    return base64.b64encode(datos).decode()


partes = ['### Llamadas de voz y video — prueba con dos navegadores de verdad', '']
try:
    if os.path.exists(INFORME):
        informe = json.load(open(INFORME, encoding='utf-8'))
        partes += ['**Resultado:** ' + ('funciona de punta a punta' if informe.get('ok') else 'FALLÓ'),
                   '', '```', json.dumps(informe, ensure_ascii=False, indent=1)[:5200], '```']
    else:
        partes += ['No se escribió el informe (¿falló antes de empezar?).']
except Exception:
    partes += ['```', traceback.format_exc()[-1200:], '```']

salida = ''
for ruta in ('/tmp/salida-llamada.txt', '/tmp/servidor.log'):
    try:
        texto = open(ruta, encoding='utf-8', errors='replace').read().strip()
    except OSError:
        continue
    if texto:
        salida += '\n\n**%s**\n\n```\n%s\n```' % (os.path.basename(ruta), texto[-2600:])
if salida:
    partes += ['', '**Lo que dijo la prueba**', salida]

for foto in sorted(glob.glob(os.path.join(FOTOS, '[0-9]-*.png'))):
    b64 = miniatura(foto)
    if b64:
        partes += ['', '**%s** — el bloque de abajo es un .jpg en base64' % os.path.basename(foto),
                   '', '```texto', b64, '```']

if notas:
    partes += ['', '**Notas**: ' + ' · '.join(notas[:8])]

cuerpo = '\n'.join(partes)[:LIMITE]
open('/tmp/comentario-llamadas.md', 'w', encoding='utf-8').write(cuerpo)
try:
    salida = subprocess.run(['gh', 'api', '-X', 'POST',
                             'repos/%s/commits/%s/comments' % (os.environ.get('GITHUB_REPOSITORY', ''), SHA),
                             '-F', 'body=@/tmp/comentario-llamadas.md'],
                            check=False, timeout=90, capture_output=True, text=True)
    print('gh dijo:', salida.returncode, (salida.stderr or '')[:300])
except Exception:
    print(traceback.format_exc()[-600:])
print('informe de %d caracteres' % len(cuerpo))
