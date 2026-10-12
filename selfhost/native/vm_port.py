#!/usr/bin/env python3
"""La VM de bytecode del port como lado de comparación de los fuzzers.

Cada fuzzer de `selfhost/native/` comparaba el ejecutable nativo contra la VM
de Rust (`zett run programa.titan DATOS`); al borrarse el Rust esa comparación
no se podía hacer. Aquí el lado VM es la VM del propio port: el programa se
compila una vez a un artefacto `.tbc` (se cachea por ruta y contenido) y se
ejecuta con `titan exec`, que ya acepta argumentos.

Uso:

    from vm_port import vm_run

    vm = vm_run(__file__, "prueba.titan", datos, text=True)

`vm_run` devuelve el mismo `subprocess.CompletedProcess` que devolvía la
llamada a `zett`, para que los fuzzers no cambien nada más. La CLI se puede
cambiar con TITAN=/ruta/a/titan (por omisión, la del repositorio) y la caché de
artefactos con TITAN_VM_DIR (por omisión /tmp/titan-vm-port).
"""

import hashlib
import os
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.normpath(os.path.join(AQUI, "..", ".."))


def titan_path():
    """La CLI del port: TITAN si está definida; si no, selfhost/titan del repositorio."""
    return os.environ.get("TITAN", os.path.join(RAIZ, "selfhost", "titan"))


def programa_desde(fuzzer, nombre):
    """Ruta absoluta de un programa de prueba que vive junto al fuzzer."""
    return os.path.join(os.path.dirname(os.path.abspath(fuzzer)), nombre)


def artefacto(programa, titan=None):
    """Compila el programa a .tbc (una vez por ruta+contenido) y devuelve la ruta."""
    titan = titan or titan_path()
    with open(programa, "rb") as f:
        digest = hashlib.sha256(f.read()).hexdigest()[:16]
    cache = os.environ.get("TITAN_VM_DIR", "/tmp/titan-vm-port")
    os.makedirs(cache, exist_ok=True)
    tbc = os.path.join(cache, "%s-%s.tbc" % (os.path.basename(programa), digest))
    if not os.path.exists(tbc):
        build = subprocess.run([titan, "build", programa, "-o", tbc],
                               capture_output=True, text=True)
        if build.returncode != 0 or not os.path.exists(tbc):
            sys.stderr.write("no compila a bytecode %s:\n%s\n" % (programa, build.stderr[-2000:]))
            raise SystemExit(1)
    return tbc


def vm_run(fuzzer, nombre, *args, titan=None, **kwargs):
    """Ejecuta `nombre` (junto al fuzzer) en la VM del port con los argumentos dados.

    Los `kwargs` (capture_output, text, env...) van a `subprocess.run`, igual que
    en la llamada a `zett` que sustituye.
    """
    tbc = artefacto(programa_desde(fuzzer, nombre), titan=titan)
    return subprocess.run([titan or titan_path(), "exec", tbc] + [str(a) for a in args], **kwargs)
