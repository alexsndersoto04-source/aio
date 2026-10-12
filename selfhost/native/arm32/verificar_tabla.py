#!/usr/bin/env python3
"""Comprueba contra las cabeceras del núcleo Linux lo que usa `sys_arm32.titan`.

`sys_arm32.titan` traduce los números de llamada al sistema de Linux **x86-64**
(los que usa el runtime de Titan) a los de **ARM de 32 bits**, y también los
indicadores de `open`, que **no son los mismos** en las dos arquitecturas: en
ARM `O_DIRECTORY` es 040000, `O_NOFOLLOW` 0100000, `O_DIRECT` 0200000 y
`O_LARGEFILE` 0400000 (en x86-64, en cambio, 040000 es `O_DIRECT`, 0200000
`O_DIRECTORY` y 0400000 `O_NOFOLLOW`). Un número equivocado ahí no da error de
compilación: se convierte en otra llamada al sistema (o en otro permiso de
apertura) solo cuando el programa corre en ARM.

Este guion compara la tabla escrita en el código con las tablas y cabeceras
oficiales del núcleo (que hay que descargar una vez):

    curl -sSL -H "Accept: application/vnd.github.raw" \\
      "https://api.github.com/repos/torvalds/linux/contents/arch/arm/tools/syscall.tbl?ref=v6.6" -o /tmp/arm.tbl
    curl -sSL -H "Accept: application/vnd.github.raw" \\
      "https://api.github.com/repos/torvalds/linux/contents/arch/x86/entry/syscalls/syscall_64.tbl?ref=v6.6" -o /tmp/x86_64.tbl
    curl -sSL -H "Accept: application/vnd.github.raw" \\
      "https://api.github.com/repos/torvalds/linux/contents/arch/arm/include/uapi/asm/fcntl.h?ref=v6.6" -o /tmp/arm_fcntl.h
    curl -sSL -H "Accept: application/vnd.github.raw" \\
      "https://api.github.com/repos/torvalds/linux/contents/include/uapi/asm-generic/fcntl.h?ref=v6.6" -o /tmp/asm_generic_fcntl.h

Uso:

    python3 selfhost/native/arm32/verificar_tabla.py \\
        TABLA_ARM TABLA_X86_64 [FCNTL_ARM FCNTL_ASM_GENERIC]

Sin los dos últimos ficheros solo comprueba los números de llamada; con ellos
comprueba además los indicadores de `open` (la tabla del ejecutor
`selfhost/native/arm32/correr.py` y la traducción de `sys_arm32.titan`).

Salida: una línea por cosa comprobada y, al final, el resumen. Código de salida
1 si algo no cuadra.
"""
import glob
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
FUENTE = os.path.join(AQUI, "..", "..", "native", "sys_arm32.titan")
EJECUTOR = os.path.join(AQUI, "correr.py")
RAIZ = os.path.normpath(os.path.join(AQUI, "..", ".."))

# Llamadas que usan las fuentes del runtime y que NO se traducen a ARM a
# propósito: la de señales (hace falta un `restorer` que el ejecutor no monta).
SIN_TRADUCIR_A_PROPOSITO = {13}


def cargar_tabla(ruta):
    tabla = {}
    for linea in open(ruta):
        if linea.startswith("#") or not linea.strip():
            continue
        campos = linea.split()
        if len(campos) < 3:
            continue
        try:
            numero = int(campos[0], 0)
        except ValueError:
            continue
        if campos[1] in ("common", "64"):
            tabla[campos[2]] = numero
    return tabla


def cargar_banderas(ruta):
    """`#define O_NOMBRE 040000` (y las variantes con paréntesis)."""
    banderas = {}
    for m in re.finditer(r"^#define\s+(O_[A-Z0-9_]+)\s+\(?\s*(0[0-7]*)\s*\)?\s*(?:/\*|\n|$)",
                         open(ruta).read(), re.M):
        banderas[m.group(1)] = int(m.group(2), 8)
    return banderas


def revisar_numeros(texto, arm, x64):
    fallos = 0
    # 1. Pares "x86-64 -> ARM" con el nombre en el comentario:
    #    `// nombre` (mismo nombre) o `// nombre (ARM: otro)`.
    pares = re.findall(r"if n == (\d+) \{ return (\d+) \}\s*//\s*([A-Za-z0-9_]+)(?: \(ARM: ([A-Za-z0-9_]+)[^)]*\))?", texto)
    if not pares:
        print("no se encontró ningún par en %s" % FUENTE)
        return 1
    for n_x, n_arm, nombre_x, nombre_arm in pares:
        n_x, n_arm = int(n_x), int(n_arm)
        esperado_x = x64.get(nombre_x)
        esperado_arm = arm.get(nombre_arm or nombre_x)
        problema = ""
        if esperado_x != n_x:
            problema = "x86-64: %s es el %s y el código dice %d" % (nombre_x, esperado_x, n_x)
        elif esperado_arm != n_arm:
            problema = "ARM: %s es el %s y el código dice %d" % (nombre_arm or nombre_x, esperado_arm, n_arm)
        if problema:
            fallos += 1
            print("MAL  %-22s %s" % (nombre_x, problema))
    print("pares traducidos: %d" % len(pares))

    # 2. Llamadas directas `std::raw::svc(N, ...)`: el número tiene que existir
    #    en ARM.
    usados = sorted({int(x) for x in re.findall(r"std::raw::svc\((\d+)", texto)})
    conocidos = set(arm.values())
    for n in usados:
        if n not in conocidos:
            fallos += 1
            print("MAL  svc(%d): ese número no existe en ARM" % n)
    print("números de svc usados directamente: %d, todos conocidos: %s"
          % (len(usados), "sí" if all(n in conocidos for n in usados) else "no"))
    return fallos


def revisar_uso(texto, raiz):
    """Cada llamada que usa el runtime tiene que estar traducida a ARM.

    Es lo que faltaba con `rmdir`: `std::fs::remove_dir` la usaba (x86-64 84),
    no estaba en la tabla, y en ARM devolvía -ENOSYS mientras en x86-64
    funcionaba. Se vio con el diferencial (`tokenize_std`); esto lo ve antes,
    sin compilar nada: lee las fuentes del runtime y compara los números que
    aparecen en `std::raw::syscall(...)` con los que traduce `sys_arm32.titan`.
    """
    fallos = 0
    cubiertos = {int(n) for n in re.findall(r"if n == (\d+)", texto)}
    usados = {}
    for ruta in sorted(glob.glob(os.path.join(raiz, "native", "*.titan")) +
                       glob.glob(os.path.join(raiz, "*.titan"))):
        for i, linea in enumerate(open(ruta, encoding="utf-8", errors="replace"), 1):
            for m in re.finditer(r"(?:std::raw::syscall|rt_path_call2?|rt_syscall)\((\d+)", linea):
                usados.setdefault(int(m.group(1)), "%s:%d" % (os.path.basename(ruta), i))
    faltan = sorted(n for n in usados if n not in cubiertos and n not in SIN_TRADUCIR_A_PROPOSITO)
    for n in faltan:
        fallos += 1
        print("MAL  llamada %d (usada en %s) no está traducida" % (n, usados[n]))
    print("llamadas usadas por el runtime: %d, traducidas: %d, sin traducir a propósito: %s"
          % (len(usados), len(usados) - len(faltan) - len(set(usados) & SIN_TRADUCIR_A_PROPOSITO),
             ", ".join(str(n) for n in sorted(set(usados) & SIN_TRADUCIR_A_PROPOSITO)) or "ninguna"))
    return fallos


def revisar_banderas(texto, arm, comun):
    """Los indicadores de `open`: la tabla del ejecutor y la traducción."""
    fallos = 0
    todas = dict(comun)
    todas.update(arm)

    # a) selfhost/native/arm32/correr.py: qué número de ARM se traduce a qué
    #    indicador del anfitrión.
    tabla = re.search(r"tabla = \((.*?)\)\n", open(EJECUTOR).read(), re.S)
    if not tabla:
        print("no se encontró la tabla de indicadores en %s" % EJECUTOR)
        return 1
    entradas = re.findall(r"\((0o[0-7]+),\s*(?:os\.([A-Z0-9_]+)|getattr\(os,\s*\"([A-Z0-9_]+)\")", tabla.group(1))
    revisadas = 0
    for numero, nombre1, nombre2 in entradas:
        nombre = nombre1 or nombre2
        if nombre not in todas:
            continue
        revisadas += 1
        if int(numero, 8) != todas[nombre]:
            fallos += 1
            print("MAL  %s: en ARM vale %s (0o%o) y el ejecutor dice %s"
                  % (nombre, todas[nombre], todas[nombre], numero))
    print("indicadores del ejecutor comprobados contra el núcleo: %d" % revisadas)

    # b) `ss_open_flags`: los tres indicadores que cambian de sitio tienen que
    #    estar (se leen del original y se comprueba que el número de x86-64 se
    #    mira y el de ARM se pone).
    i = texto.find("fn ss_open_flags")
    j = texto.find("\n}\n", i)
    cuerpo = texto[i:j]
    for nombre in ("O_DIRECT", "O_DIRECTORY", "O_NOFOLLOW"):
        if nombre not in comun:
            continue
        de_x64, de_arm = comun[nombre], arm.get(nombre)
        if de_arm is None:
            continue
        if ("& %d" % de_x64) not in cuerpo or ("| %d" % de_arm) not in cuerpo:
            fallos += 1
            print("MAL  ss_open_flags no traduce %s (x86-64 %d -> ARM %d)"
                  % (nombre, de_x64, de_arm))
    print("indicadores traducidos por ss_open_flags: %s"
          % ", ".join(n for n in ("O_DIRECT", "O_DIRECTORY", "O_NOFOLLOW") if n in comun and n in arm))
    return fallos


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    arm = cargar_tabla(sys.argv[1])
    x64 = cargar_tabla(sys.argv[2])
    texto = open(FUENTE).read()
    fallos = revisar_numeros(texto, arm, x64)
    fallos += revisar_uso(texto, RAIZ)
    if len(sys.argv) >= 5:
        fcntl_arm = cargar_banderas(sys.argv[3])
        fcntl_comun = cargar_banderas(sys.argv[4])
        fallos += revisar_banderas(texto, fcntl_arm, fcntl_comun)
    else:
        print("(sin las cabeceras de fcntl no se comprueban los indicadores de open)")
    print("TODO BIEN" if fallos == 0 else "HAY %d FALLOS" % fallos)
    return 0 if fallos == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
