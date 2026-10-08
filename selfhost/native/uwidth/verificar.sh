#!/bin/sh
# Compila y ejecuta la prueba del ancho de texto (unicode-width 0.2.2).
# Uso (desde la raíz del repositorio): sh selfhost/native/uwidth/verificar.sh
set -e
cd "$(dirname "$0")/../.."
zett run prueba_interna.titan native/uwidth/prueba.titan /tmp/uwidth_prueba
/tmp/uwidth_prueba native/uwidth/casos.txt
