#!/usr/bin/env python3
"""Anota la dirección de la API dentro del archivo de sesiones."""
import json
import sys

ruta = sys.argv[1] if len(sys.argv) > 1 else '/tmp/sesiones.json'
api = sys.argv[2] if len(sys.argv) > 2 else 'https://moon-dal0.onrender.com'
datos = json.load(open(ruta, encoding='utf-8'))
datos['api'] = api
json.dump(datos, open(ruta, 'w', encoding='utf-8'))
print('api anotada:', api)
