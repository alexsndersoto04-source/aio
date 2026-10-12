#!/bin/sh
# Compila y ejecuta la prueba del ancho de texto (unicode-width 0.2.2).
# Uso (desde la raíz del repositorio): sh selfhost/native/uwidth/verificar.sh
# Sin Rust: el compilador de pruebas (prueba_interna.titan) se arma con el port.
set -e
cd "$(dirname "$0")/../.."   # esta carpeta es selfhost/
./titan compile prueba_interna.titan -o prueba_interna
./prueba_interna native/uwidth/prueba.titan /tmp/uwidth_prueba
/tmp/uwidth_prueba native/uwidth/casos.txt
