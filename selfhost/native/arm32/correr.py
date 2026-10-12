#!/usr/bin/env python3
"""Ejecuta un ejecutable ARM de 32 bits (ELF32/EM_ARM) en un procesador ARMv7 de
verdad (el motor de QEMU que hay dentro de unicorn) con las llamadas al sistema
de Linux puestas en Python.

Es el equivalente a `qemu-arm` para lo que hace falta aquí: carga el ELF, pone
la pila de arranque (argc/argv/envp/auxv), enciende la unidad de coma flotante
como hace el núcleo (FPEXC.EN: sin eso la primera instrucción VFP es
«instrucción inválida»), pasa el entorno del proceso y atiende cada `svc #0`
con las llamadas de ARM EABI (el número en r7, los argumentos en r0-r5). Las
que no están implementadas devuelven -ENOSYS y lo dicen por stderr: nunca se
inventa un resultado.

    python3 correr.py PROGRAMA.arm [--codigo] [--max-passos N]

La salida del programa va a sus descriptores (1 y 2) y el ejecutor termina con
el mismo código que el programa, como qemu. Con --codigo se imprime el código
de salida por stderr.

Palancas (variables de entorno), todas para mirar cuando algo va mal:

    TITAN_ARM32_TRAZA=1    una línea por llamada al sistema (número, argumentos
                           y resultado), por stderr.
    TITAN_ARM32_VENTANA=1  guarda las últimas instrucciones ejecutadas y las
                           desensambla si el programa muere o se corta. Cuesta
                           una llamada a Python por instrucción: solo para
                           depurar.
    TITAN_ARM32_PERFIL=1   lo anterior más el recuento de direcciones.
    TITAN_ARM32_PASOS=N    tope de instrucciones (por defecto 200000000, y con
                           --max-passos se cambia el tope de llamadas).
    TITAN_ARM32_RITMO=1    un aviso al final con los millones de instrucciones
                           ejecutadas y a qué velocidad: sirve para saber si un
                           programa pesado avanza o se quedó parado. Con
                           TITAN_ARM32_RITMO_SEGUNDOS=N, además, un aviso cada
                           N segundos con el número de instrucciones y la
                           dirección por la que va.
    TITAN_ARM32_FALLO=1    si el programa muere, además del aviso, la traza de
                           Python que lo provocó. Al ejecutar con esta palanca
                           puesta, `kill -USR1 PID` imprime por stderr dónde
                           está el ejecutor en ese momento (la pila de Python),
                           para mirar un programa que se queda colgado.

Esto **no** es un Linux: es un procesador ARM ejecutando el binario con las
llamadas al sistema servidas desde aquí. Para que un programa real corra hace
falta un núcleo Linux; ver «ARM de 32 bits» en selfhost/ESTADO.md.
"""
import os
import random
import signal
import struct
import sys
import time

try:
    from unicorn import (Uc, UC_ARCH_ARM, UC_MODE_ARM, UC_HOOK_INTR, UC_HOOK_CODE,
                         UC_PROT_ALL, UC_PROT_READ, UC_PROT_WRITE, UC_PROT_EXEC)
    from unicorn.arm_const import (
        UC_ARM_REG_SP, UC_ARM_REG_LR, UC_ARM_REG_PC, UC_ARM_REG_R0, UC_ARM_REG_R1,
        UC_ARM_REG_R2, UC_ARM_REG_R3, UC_ARM_REG_R4, UC_ARM_REG_R5, UC_ARM_REG_R7,
        UC_ARM_REG_FPEXC,
    )
except ImportError:  # pragma: no cover
    print("falta unicorn: pip install unicorn", file=sys.stderr)
    sys.exit(2)

WSO = __import__("re").compile(r"\[(\w+)(, #(0x[0-9a-f]+|-?\d+))?\]")

PAGE = 4096
STACK_TOP = 0x7fff0000
STACK_SIZE = 8 << 20
BASE = 0x10000

E_NOENT, E_NOSYS, E_INVAL = 2, 38, 22
E_NOTTY, E_CHILD, E_ESPIPE = 25, 10, 29


class Salida(Exception):
    def __init__(self, codigo):
        self.codigo = codigo


class Memoria:
    """Direcciones del proceso: lo que el programa pide con mmap2 y un brk."""

    # De dónde empieza a buscar sitio para una reserva sin dirección: por
    # debajo queda el ELF (0x10000) y el montón (0x8000000).
    PRIMERA = 0x10000000
    # Un proceso ARM de 32 bits de verdad ve 3 GB (el núcleo se queda el
    # último); aquí la pila está más abajo, en 0x7fff0000, pero las reservas
    # grandes (los codificadores de vídeo o de compresión piden cientos de
    # megabytes de una vez) llegan hasta 0xC0000000. La zona de la pila queda
    # anotada como ocupada, así que nadie la pisa.
    LIMITE = 0xC0000000

    def __init__(self, uc):
        self.uc = uc
        self.mapa = []          # [(inicio, fin)] de todo lo reservado
        self.brk = 0

    def anotar(self, inicio, fin):
        self.mapa.append((inicio, fin))
        self.mapa.sort()

    def mapear(self, direccion, tamano, permisos=UC_PROT_ALL):
        inicio = direccion & ~(PAGE - 1)
        fin = (direccion + tamano + PAGE - 1) & ~(PAGE - 1)
        if fin <= inicio:
            fin = inicio + PAGE
        try:
            self.uc.mem_map(inicio, fin - inicio, permisos)
        except Exception:
            # Ya estaba mapeado (por ejemplo, el propio ELF): no pasa nada.
            self.anotar(inicio, fin)
            return inicio
        self.anotar(inicio, fin)
        return inicio

    def hueco(self, tamano):
        # Primer hueco libre donde quepa, como hace el núcleo al colocar una
        # reserva sin dirección. Se mira todo el espacio de usuario: un programa
        # puede pedir cientos de megabytes (y el núcleo se los da: la memoria
        # que no se toca no cuesta nada).
        inicio = self.PRIMERA
        for (a, b) in self.mapa:
            if a - inicio >= tamano:
                return inicio
            if b > inicio:
                inicio = b
        if self.LIMITE - inicio >= tamano:
            return inicio
        return -1

    def mmap2(self, direccion, tamano, prot, flags, fd, desplazamiento):
        if tamano == 0:
            return -E_INVAL
        if direccion:
            return self.mapear(direccion, tamano)
        inicio = self.hueco((tamano + PAGE - 1) & ~(PAGE - 1))
        if inicio < 0:
            return -12  # ENOMEM
        return self.mapear(inicio, tamano)

    def munmap(self, direccion, tamano):
        inicio = direccion & ~(PAGE - 1)
        fin = (direccion + tamano + PAGE - 1) & ~(PAGE - 1)
        if fin <= inicio:
            return 0
        try:
            self.uc.mem_unmap(inicio, fin - inicio)
        except Exception:
            pass
        # Lo que se suelta deja de estar reservado (y se puede volver a usar).
        quedan = []
        for (a, b) in self.mapa:
            if b <= inicio or a >= fin:
                quedan.append((a, b))
                continue
            if a < inicio:
                quedan.append((a, inicio))
            if b > fin:
                quedan.append((fin, b))
        self.mapa = quedan
        return 0

    def reservar_pila(self):
        inicio = STACK_TOP - STACK_SIZE
        self.uc.mem_map(inicio, STACK_SIZE, UC_PROT_ALL)
        self.anotar(inicio, STACK_TOP)
        return inicio


def cargar_elf(uc, camino):
    ejecutables = []
    datos = open(camino, "rb").read()
    if datos[:4] != b"\x7fELF" or datos[4] != 1:
        raise SystemExit("no es un ELF de 32 bits")
    if struct.unpack_from("<H", datos, 18)[0] != 40:
        raise SystemExit("no es un ELF de ARM (e_machine != 40)")
    entrada = struct.unpack_from("<I", datos, 24)[0]
    phoff = struct.unpack_from("<I", datos, 28)[0]
    phentsize = struct.unpack_from("<H", datos, 42)[0]
    phnum = struct.unpack_from("<H", datos, 44)[0]
    for i in range(phnum):
        p = phoff + i * phentsize
        tipo, off, vaddr, paddr, filesz, memsz, flags, align = struct.unpack_from("<8I", datos, p)
        if tipo != 1:
            continue
        inicio = vaddr & ~(PAGE - 1)
        fin = (vaddr + memsz + PAGE - 1) & ~(PAGE - 1)
        prot = UC_PROT_READ
        if flags & 2:
            prot |= UC_PROT_WRITE
        if flags & 1:
            prot |= UC_PROT_EXEC
        try:
            uc.mem_map(inicio, fin - inicio, prot)
        except Exception:
            pass
        uc.mem_write(vaddr, datos[off:off + filesz])
        if flags & 1:
            ejecutables.append((vaddr, datos[off:off + filesz]))
    cargar_elf.ejecutables = ejecutables
    return entrada


def pila_de_arranque(uc, argv, entorno, auxv):
    """Escribe argc/argv/envp/auxv como lo deja el núcleo y devuelve el SP."""
    cadena = b"".join(a.encode() + b"\0" for a in argv) + b"".join(e.encode() + b"\0" for e in entorno)
    total = len(cadena) + 4 * (1 + len(argv) + 1 + len(entorno) + 1 + 2 * len(auxv))
    sp = (STACK_TOP - total - 64) & ~15
    ptr = sp
    direcciones_env = []
    base = (STACK_TOP - len(cadena)) & ~15
    uc.mem_write(base, cadena)
    desplazamiento = 0
    argumentos = []
    for a in argv:
        argumentos.append(base + desplazamiento)
        desplazamiento += len(a.encode()) + 1
    for e in entorno:
        direcciones_env.append(base + desplazamiento)
        desplazamiento += len(e.encode()) + 1
    salida = b""
    salida += struct.pack("<I", len(argv))
    for a in argumentos:
        salida += struct.pack("<I", a)
    salida += struct.pack("<I", 0)
    for e in direcciones_env:
        salida += struct.pack("<I", e)
    salida += struct.pack("<I", 0)
    for k, v in auxv:
        salida += struct.pack("<II", k, v)
    salida += struct.pack("<II", 0, 0)
    uc.mem_write(sp, salida)
    return sp


class Proceso:
    def __init__(self, camino, argv, entorno, salida, max_passos, error=None):
        # `salida`/`error` pueden ser archivos binarios o el stdout/stderr del
        # intérprete. Cada descriptor de fichero va a su sitio, como siempre.
        if hasattr(salida, "buffer"):
            salida = salida.buffer
        if error is None:
            error = salida
        elif hasattr(error, "buffer"):
            error = error.buffer
        self.salida = salida
        self.error = error
        self.max_passos = max_passos
        # Ruta absoluta del programa emulado: es lo que se contesta en
        # /proc/self/exe (qemu-user hace lo mismo con el binario que ejecuta).
        self.ruta_programa = os.path.abspath(camino)
        self.uc = Uc(UC_ARCH_ARM, UC_MODE_ARM)
        try:
            from unicorn import UC_CPU_ARM_CORTEX_A15
            self.uc.ctl_set_cpu_model(UC_CPU_ARM_CORTEX_A15)
        except Exception:
            pass
        self.entrada = cargar_elf(self.uc, camino)
        self.mem = Memoria(self.uc)
        self.mem.reservar_pila()
        self.brk_inicio = 0x8000000
        self.brk = self.brk_inicio
        self.mem.mapear(self.brk_inicio, PAGE)
        auxv = [(6, PAGE), (7, 0), (9, self.entrada), (11, 1000), (13, 1000), (14, 1000), (16, 0x7fff0000), (23, 0)]
        sp = pila_de_arranque(self.uc, argv, entorno, auxv)
        self.sp = sp
        self.uc.reg_write(UC_ARM_REG_SP, sp)
        self.uc.reg_write(UC_ARM_REG_LR, 0)
        # El núcleo Linux deja la unidad de coma flotante encendida (FPEXC.EN)
        # antes de pasar a modo usuario; sin esto, la primera instrucción VFP
        # es una instrucción inválida.
        self.uc.reg_write(UC_ARM_REG_FPEXC, 0x40000000)
        self.pc = self.entrada
        self.aleatorio = random.Random(20261011)
        # Descriptores del programa -> descriptores del anfitrión. 0, 1 y 2 son
        # los de siempre (stdin/stdout/stderr) y los lleva escribir_fd.
        self.fds = {0: 0, 1: 1, 2: 2}
        self.siguiente_fd = 3
        self.dirs = {}
        self.traza = bool(os.environ.get("TITAN_ARM32_TRAZA"))
        # Ventana con las últimas instrucciones: sirve para ver dónde se quedó
        # un programa que se cuelga o muere.
        self.ventana = []
        self.instrucciones = 0
        self.tope = int(os.environ.get("TITAN_ARM32_PASOS", "0")) or None
        self.perfil = {} if os.environ.get("TITAN_ARM32_PERFIL") else None
        self.cortado = False
        self.entradas = self.destinos_de_bl()
        self.llamadas = []
        # La ventana de instrucciones cuesta una llamada a Python por
        # instrucción (es lo que hace lento al ejecutor), así que solo se
        # enciende cuando de verdad se va a mirar: con TITAN_ARM32_VENTANA=1 o
        # al pedir el perfil. Para contar el tope de pasos no hace falta.
        self.ventana_activa = bool(os.environ.get("TITAN_ARM32_VENTANA")) or self.perfil is not None
        if self.ventana_activa:
            self.uc.hook_add(UC_HOOK_CODE, self.paso)
        # Con TITAN_ARM32_RITMO=1 se mide, sin la ventana de instrucciones, cuánto
        # tarda el programa y a qué velocidad ejecuta el ejecutor.
        self.ritmo = bool(os.environ.get("TITAN_ARM32_RITMO"))
        self.ritmo_segundos = float(os.environ.get("TITAN_ARM32_RITMO_SEGUNDOS", "0") or 0)
        self.passos = 0
        self.uc.hook_add(UC_HOOK_INTR, self.interrupcion)

    # ---------------------------------------------------------------- memoria
    def leer(self, direccion, tamano):
        return self.uc.mem_read(direccion & 0xFFFFFFFF, tamano)

    def escribir(self, direccion, datos):
        self.uc.mem_write(direccion & 0xFFFFFFFF, datos)

    def cadena(self, direccion, limite=4096):
        # Se lee por trozos: una cadena puede terminar justo donde termina la
        # memoria mapeada (p. ej. el último byte de la zona de datos), y leer
        # de golpe 4096 bytes ahí dentro daría un fallo del procesador que el
        # programa no ha provocado. Donde ya no hay memoria, la cadena acaba.
        trozos = []
        leidos = 0
        while leidos < limite:
            cuanto = min(64, limite - leidos)
            try:
                datos = bytes(self.leer(direccion + leidos, cuanto))
            except Exception:
                if cuanto == 1:
                    break
                continue
            corte = datos.find(b"\0")
            if corte >= 0:
                trozos.append(datos[:corte])
                return b"".join(trozos)
            trozos.append(datos)
            leidos += len(datos)
        return b"".join(trozos)

    # Direcciones a las que salta algún `bl`: son las entradas de función.
    def destinos_de_bl(self):
        try:
            from capstone import Cs, CS_ARCH_ARM, CS_MODE_ARM
        except ImportError:
            return set()
        md = Cs(CS_ARCH_ARM, CS_MODE_ARM)
        destinos = set()
        for base, codigo in getattr(cargar_elf, "ejecutables", []):
            for i in md.disasm(codigo, base):
                if i.mnemonic in ("bl", "blx") and i.op_str.startswith("#"):
                    try:
                        destinos.add(int(i.op_str[1:], 16))
                    except ValueError:
                        pass
        return destinos

    def paso(self, uc, direccion, tamano, usuario):
        self.instrucciones += 1
        if direccion in self.entradas:
            self.llamadas.append(direccion)
            if len(self.llamadas) > 64:
                del self.llamadas[0]
        self.ventana.append((direccion, uc.reg_read(UC_ARM_REG_SP), uc.reg_read(UC_ARM_REG_LR)))
        if len(self.ventana) > 24:
            del self.ventana[0]
        # Con TITAN_ARM32_PERFIL=1 se cuentan las direcciones más ejecutadas
        # (con TITAN_ARM32_PASOS se corta antes y se imprime el recuento).
        if self.perfil is not None:
            self.perfil[direccion] = self.perfil.get(direccion, 0) + 1
        if self.tope is not None and self.instrucciones > self.tope:
            self.cortado = True
            raise Salida(255)

    def ritmo_correr(self):
        self.ritmo_arranque = time.perf_counter()
        self.ritmo_aviso_previo = 0.0
        # Ejecuta a trozos contando instrucciones (las cuenta la propia
        # máquina, no Python) hasta que el programa se acaba por su cuenta. El
        # recuento va de 4 millones en 4 millones: sirve para saber por dónde
        # va, no para contar exactamente.
        pc = self.entrada & ~1
        trozo = 1 << 22
        while self.instrucciones < (1 << 40):
            self.uc.emu_start(pc, 0, 0, trozo)
            self.instrucciones += trozo
            pc = self.uc.reg_read(UC_ARM_REG_PC)
            if self.ritmo_segundos:
                # Para mirar un programa largo: cada TITAN_ARM32_RITMO_SEGUNDOS
                # dice por dónde va (así se ve si avanza o se quedó parado).
                ahora = time.perf_counter()
                if ahora - self.ritmo_aviso_previo >= self.ritmo_segundos:
                    self.ritmo_aviso_previo = ahora
                    print("[arm32] vamos por %d millones de instrucciones (PC 0x%08x, %.0f s)"
                          % (self.instrucciones // 1000000, pc, ahora - self.ritmo_arranque),
                          file=sys.stderr)
        self.cortado = True
        raise Salida(255)

    def ritmo_aviso(self, segundos):
        if not self.ritmo or segundos <= 0:
            return
        if self.instrucciones == 0:
            print("[arm32] el programa acabó antes del primer trozo (%.1f s)" % segundos, file=sys.stderr)
            return
        print("[arm32] ~%d millones de instrucciones en %.1f s (%.1f M/s)"
              % (self.instrucciones // 1000000, segundos, self.instrucciones / segundos / 1e6),
              file=sys.stderr)

    def desmontar_ventana(self):
        try:
            from capstone import Cs, CS_ARCH_ARM, CS_MODE_ARM
        except ImportError:
            print("[arm32] (sin capstone no se puede desensamblar)", file=sys.stderr)
            return
        md = Cs(CS_ARCH_ARM, CS_MODE_ARM)
        print("[arm32] últimas %d instrucciones (pc, sp, lr):" % len(self.ventana), file=sys.stderr)
        for pc, sp, lr in self.ventana:
            try:
                datos = bytes(self.uc.mem_read(pc & 0xFFFFFFFF, 4))
                texto = ""
                for i in md.disasm(datos, pc):
                    texto = "%s %s" % (i.mnemonic, i.op_str)
            except Exception:
                texto = "(no se puede leer)"
            print("[arm32]   %08x  sp=%08x lr=%08x  %s" % (pc & 0xFFFFFFFF, sp & 0xFFFFFFFF, lr & 0xFFFFFFFF, texto), file=sys.stderr)

    def mostrar_perfil(self):
        if not self.perfil:
            return
        try:
            from capstone import Cs, CS_ARCH_ARM, CS_MODE_ARM
        except ImportError:
            return
        md = Cs(CS_ARCH_ARM, CS_MODE_ARM)
        print("[arm32] últimas entradas de función: %s" % " ".join("%08x" % x for x in self.llamadas[-28:]), file=sys.stderr)
        orden = sorted(self.perfil.items(), key=lambda kv: kv[1], reverse=True)[:14]
        print("[arm32] direcciones más ejecutadas (%d instrucciones):" % self.instrucciones, file=sys.stderr)
        for pc, veces in orden:
            try:
                texto = ""
                for i in md.disasm(bytes(self.uc.mem_read(pc, 4)), pc):
                    texto = "%s %s" % (i.mnemonic, i.op_str)
            except Exception:
                texto = "(?)"
            print("[arm32]   %5d x %08x  %s" % (veces, pc, texto), file=sys.stderr)

    # ------------------------------------------------------------- syscalls
    def interrupcion(self, uc, numero, usuario):
        pc = uc.reg_read(UC_ARM_REG_PC)
        # svc #0: el PC apunta a la instrucción siguiente; el número va en r7.
        self.passos += 1
        if self.passos > self.max_passos:
            raise Salida(255)
        n = uc.reg_read(UC_ARM_REG_R7)
        a = [uc.reg_read(UC_ARM_REG_R0), uc.reg_read(UC_ARM_REG_R1), uc.reg_read(UC_ARM_REG_R2),
             uc.reg_read(UC_ARM_REG_R3), uc.reg_read(UC_ARM_REG_R4), uc.reg_read(UC_ARM_REG_R5)]
        r = self.syscall(n, a)
        if self.traza:
            print("[svc %d] %s -> %s" % (n, [hex(x) for x in a], r), file=sys.stderr)
        uc.reg_write(UC_ARM_REG_R0, r & 0xFFFFFFFF)

    # ------------------------------------------------------------ ficheros
    # Del descriptor del programa al del anfitrión (0, 1 y 2 son los de
    # siempre y los maneja escribir_fd).
    def fd_de(self, g):
        return self.fds.get(g, -1)

    def nuevo_fd(self, host):
        g = self.siguiente_fd
        self.siguiente_fd += 1
        self.fds[g] = host
        return g

    def cerrar_fd(self, g):
        if g in (0, 1, 2):
            return 0
        host = self.fds.pop(g, -1)
        if host < 0:
            return -9  # EBADF
        self.dirs.pop(g, None)
        try:
            os.close(host)
        except OSError:
            pass
        return 0

    # Los indicadores de open de ARM no son los de x86-64: se traducen uno a
    # uno a los del anfitrión (Linux x86-64, que es donde corre el ejecutor).
    def flags_open(self, f):
        h = f & 3  # O_RDONLY/O_WRONLY/O_RDWR son iguales
        tabla = ((0o100, os.O_CREAT), (0o200, os.O_EXCL), (0o400, os.O_NOCTTY), (0o1000, os.O_TRUNC),
                 (0o2000, os.O_APPEND), (0o4000, os.O_NONBLOCK), (0o10000, os.O_DSYNC),
                 (0o20000, getattr(os, "O_ASYNC", 0)), (0o200000, os.O_DIRECT), (0o400000, os.O_LARGEFILE),
                 (0o40000, os.O_DIRECTORY), (0o100000, os.O_NOFOLLOW), (0o2000000, os.O_CLOEXEC),
                 (0o10000000, getattr(os, "O_PATH", 0)), (0o4010000, getattr(os, "O_TMPFILE", 0)))
        for arm, host in tabla:
            if f & arm:
                h |= host
        return h

    def ruta_de_proc(self, ruta):
        """Como qemu-user: el `/proc/.../exe` de este proceso es el programa
        emulado, no el intérprete (Python).

        Hacen falta las dos formas porque el runtime canonicaliza
        `/proc/self/exe` a mano: lee el enlace `/proc/self` (el núcleo contesta
        el pid del ejecutor), lo sigue y acaba preguntando por
        `/proc/<pid>/exe`. Sin esto, el compilador creía ser el intérprete y
        buscaba el runtime `native/` al lado de Python."""
        if ruta == "/proc/self/exe" or ruta == "/proc/%d/exe" % os.getpid():
            return self.ruta_programa
        return None

    def sys_openat(self, dirfd, p, flags, modo):
        ruta = os.fsdecode(self.cadena(p))
        propio = self.ruta_de_proc(ruta)
        if propio is not None:
            ruta = propio
        d = self.s32(dirfd)
        extra = {}
        if d != -100:
            host = self.fd_de(d)
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            host = os.open(ruta, self.flags_open(flags), modo & 0o7777, **extra)
        except OSError as error:
            return -error.errno
        return self.nuevo_fd(host)

    def sys_read(self, g, buf, cuenta):
        if cuenta == 0:
            return 0
        if g == 0:
            try:
                datos = os.read(0, cuenta)
            except OSError:
                return 0
            if not datos:
                return 0
            self.escribir(buf, datos)
            return len(datos)
        host = self.fd_de(g)
        if host < 0:
            return -9
        try:
            datos = os.read(host, cuenta)
        except OSError as error:
            return -error.errno
        self.escribir(buf, datos)
        return len(datos)

    def sys_write(self, g, buf, cuenta):
        if g in (1, 2):
            return self.escribir_fd(g, buf, cuenta)
        host = self.fd_de(g)
        if host < 0:
            return -9
        try:
            return os.write(host, bytes(self.leer(buf, cuenta)))
        except OSError as error:
            return -error.errno

    def sys_lseek(self, g, desplazamiento, origen):
        host = self.fd_de(g)
        if host < 0:
            return -9
        try:
            return os.lseek(host, desplazamiento, origen)
        except OSError as error:
            return -error.errno

    # struct statx: campos de tamaño fijo iguales en todas las arquitecturas.
    def sys_statx(self, dirfd, p, banderas, mascara, buf):
        d = self.s32(dirfd)
        extra = {}
        base = None
        if banderas & 0x1000:                 # AT_EMPTY_PATH: por el descriptor
            host = self.fd_de(d)
            if host < 0:
                return -2
            try:
                base = os.fstat(host)
            except OSError as error:
                return -error.errno
        else:
            ruta = os.fsdecode(self.cadena(p))
            if d != -100:
                host = self.fd_de(d)
                if host < 0:
                    return -9
                extra["dir_fd"] = host
            try:
                base = os.stat(ruta, follow_symlinks=not (banderas & 0x100), **extra)
            except OSError as error:
                return -error.errno
        stx = bytearray(256)
        struct.pack_into("<I", stx, 0, mascara if mascara else 0xFFF)
        struct.pack_into("<I", stx, 4, getattr(base, "st_blksize", 4096))
        struct.pack_into("<I", stx, 16, base.st_nlink)
        struct.pack_into("<I", stx, 20, base.st_uid)
        struct.pack_into("<I", stx, 24, base.st_gid)
        struct.pack_into("<H", stx, 28, base.st_mode)
        struct.pack_into("<Q", stx, 32, base.st_ino)
        struct.pack_into("<Q", stx, 40, base.st_size)
        struct.pack_into("<Q", stx, 48, getattr(base, "st_blocks", 0))
        # atime@64, btime@80 (no hay), ctime@96, mtime@112.
        for k, campos in ((64, ("st_atime", "st_atime_ns")), (80, ("st_ctime", "st_ctime_ns")),
                          (96, ("st_ctime", "st_ctime_ns")), (112, ("st_mtime", "st_mtime_ns"))):
            segundos = getattr(base, campos[0], 0)
            nanos = getattr(base, campos[1], 0)
            if not isinstance(nanos, int):
                nanos = 0
            struct.pack_into("<qI", stx, k, int(segundos), int(nanos % 1000000000))
        struct.pack_into("<I", stx, 128, os.major(base.st_rdev))
        struct.pack_into("<I", stx, 132, os.minor(base.st_rdev))
        struct.pack_into("<I", stx, 136, os.major(base.st_dev))
        struct.pack_into("<I", stx, 140, os.minor(base.st_dev))
        self.escribir(buf, bytes(stx))
        return 0

    # getdents64: struct linux_dirent64 (campos de 64 bits en todas).
    def sys_getdents64(self, g, buf, cuenta):
        host = self.fd_de(g)
        if host < 0:
            return -9
        if g not in self.dirs:
            ruta = "/proc/self/fd/%d" % host
            try:
                lista = []
                for e in os.scandir(ruta):
                    tipo = 4  # DT_DIR
                    if e.is_symlink():
                        tipo = 10
                    elif e.is_dir(follow_symlinks=False):
                        tipo = 4
                    elif e.is_file(follow_symlinks=False):
                        tipo = 8
                    nombre = os.fsencode(e.name)
                    lista.append((e.inode(), tipo, nombre))
                self.dirs[g] = lista
            except OSError as error:
                return -error.errno
        lista = self.dirs[g]
        salida = b""
        while lista:
            ino, tipo, nombre = lista[0]
            largo = 19 + len(nombre) + 1
            largo = (largo + 7) & ~7
            if len(salida) + largo > cuenta:
                break
            registro = struct.pack("<QqHB", ino, 0, largo, tipo) + nombre + b"\0"
            registro += b"\0" * (largo - len(registro))
            salida += registro
            lista.pop(0)
        if not salida:
            return 0
        self.escribir(buf, salida)
        return len(salida)

    def sys_fcntl64(self, g, orden, arg):
        host = self.fd_de(g)
        if host < 0:
            return -9
        if orden == 1:      # F_GETFD
            return 0
        if orden == 2:      # F_SETFD
            return 0
        if orden == 3:      # F_GETFL
            return 2
        if orden == 4:      # F_SETFL
            return 0
        return -22

    def s32(self, v):
        v &= 0xFFFFFFFF
        return v - 4294967296 if v >= 2147483648 else v

    def syscall(self, n, a):
        # Los números y las estructuras son los de ARM 32 bits (EABI): se
        # comprueban contra arch/arm/tools/syscall.tbl del núcleo
        # (selfhost/native/arm32/verificar_tabla.py hace lo mismo con la tabla
        # del compilador).
        if n == 1 or n == 248:      # exit / exit_group
            raise Salida(a[0] & 0xFF)
        if n == 3:                  # read
            return self.sys_read(a[0], a[1], a[2])
        if n == 4:                  # write
            return self.sys_write(a[0], a[1], a[2])
        if n == 5:                  # open
            return self.sys_openat(-100, a[0], a[1], a[2])
        if n == 6:                  # close
            return self.cerrar_fd(a[0])
        if n in (9, 330, 331, 332, 333, 334, 439, 325):   # linkat/symlinkat/... se ven abajo
            return self.sys_at(n, a)
        if n == 10:                 # unlink
            return self.sys_unlinkat(-100, a[0], 0)
        if n == 12:                 # chdir
            try:
                os.chdir(os.fsdecode(self.cadena(a[0])))
            except OSError as error:
                return -error.errno
            return 0
        if n == 15:                 # chmod
            try:
                os.chmod(os.fsdecode(self.cadena(a[0])), a[1] & 0o7777)
            except OSError as error:
                return -error.errno
            return 0
        if n == 19:                 # lseek
            return self.sys_lseek(a[0], self.s32(a[1]), a[2])
        if n == 20:                 # getpid
            return os.getpid()
        if n == 24:                 # getuid
            return os.getuid()
        if n == 47:                 # getgid
            return os.getgid()
        if n == 49:                 # geteuid
            return os.geteuid()
        if n == 50:                 # getegid
            return os.getegid()
        if n == 33:                 # access
            return self.sys_faccessat(-100, a[0], a[1], 0)
        if n == 37:                 # kill
            # Solo existe un proceso aquí: el que se mata a sí mismo.
            return 0 if a[0] == os.getpid() else -3  # ESRCH
        if n == 38:                 # rename
            return self.sys_renameat(-100, a[0], -100, a[1], 0)
        if n == 39:                 # mkdir
            return self.sys_mkdirat(-100, a[0], a[1])
        if n == 40:                 # rmdir
            return self.sys_unlinkat(-100, a[0], 0x200)
        if n == 41:                 # dup
            return self.sys_dup(a[0], -1)
        if n == 45:                 # brk
            return self.brk_syscall(a[0])
        if n == 54:                 # ioctl
            return -E_NOTTY
        if n == 60:                 # umask
            try:
                return os.umask(a[0] & 0o7777)
            except OSError:
                return 0
        if n == 63:                 # dup2
            return self.sys_dup(a[0], a[1])
        if n == 64:                 # getppid
            return os.getppid()
        if n == 78:                 # gettimeofday
            return self.sys_gettimeofday(a[0], a[1])
        if n == 83:                 # symlink
            return self.sys_symlinkat(a[0], -100, a[1])
        if n == 85:                 # readlink
            return self.sys_readlinkat(-100, a[0], a[1], a[2])
        if n == 88:                 # reboot (no)
            return -38
        if n == 91:                 # munmap
            return self.mem.munmap(a[0], a[1])
        if n == 92:                 # truncate
            try:
                os.truncate(os.fsdecode(self.cadena(a[0])), self.s32(a[1]))
            except OSError as error:
                return -error.errno
            return 0
        if n == 93:                 # ftruncate
            return self.sys_ftruncate(a[0], self.s32(a[1]))
        if n == 94:                 # fchmod
            host = self.fd_de(a[0])
            if host < 0:
                return -9
            try:
                os.fchmod(host, a[1] & 0o7777)
            except OSError as error:
                return -error.errno
            return 0
        if n == 114:                # wait4
            return -E_CHILD
        if n in (118, 148):         # fsync / fdatasync
            host = self.fd_de(a[0])
            if host < 0:
                return -9
            try:
                os.fsync(host)
            except OSError as error:
                return -error.errno
            return 0
        if n == 122:                # uname
            return self.uname(a[0])
        if n == 125:                # mprotect
            return 0
        if n == 133:                # fchdir
            host = self.fd_de(a[0])
            if host < 0:
                return -9
            try:
                os.chdir("/proc/self/fd/%d" % host)
            except OSError as error:
                return -error.errno
            return 0
        if n == 140:                # _llseek
            return self.sys_llseek(a[0], a[1], a[2], a[3], a[4])
        if n == 143:                # flock
            return 0
        if n == 145:                # readv
            return self.sys_readv(a[0], a[1], a[2])
        if n == 146:                # writev
            return self.writev(a[0], a[1], a[2])
        if n == 162:                # nanosleep
            return 0
        if n == 172:                # prctl
            return 0
        if n in (174, 175):         # rt_sigaction / rt_sigprocmask
            return 0
        if n == 180:                # pread64
            return self.sys_pread(a[0], a[1], a[2], a[4], a[5])
        if n == 181:                # pwrite64
            return self.sys_pwrite(a[0], a[1], a[2], a[4], a[5])
        if n == 183:                # getcwd
            return self.getcwd(a[0], a[1])
        if n == 192:                # mmap2
            return self.mem.mmap2(a[0], a[1], a[2], a[3], a[4], a[5])
        if n == 193:                # truncate64
            largo = (a[1] | (a[2] << 32)) & 0xFFFFFFFFFFFFFFFF
            try:
                os.truncate(os.fsdecode(self.cadena(a[0])), largo)
            except OSError as error:
                return -error.errno
            return 0
        if n == 194:                # ftruncate64
            return self.sys_ftruncate64(a[0], a[2], a[3])
        if n == 217:                # getdents64
            return self.sys_getdents64(a[0], a[1], a[2])
        if n == 220:                # madvise
            return self.sys_madvise(a[0], a[1], a[2])
        if n == 221:                # fcntl64
            return self.sys_fcntl64(a[0], a[1], a[2])
        if n == 224:                # gettid
            # Aquí no hay hilos: el hilo principal es el proceso.
            return os.getpid()
        if n == 240:                # futex: aquí no hay hilos del núcleo
            return -38
        if n == 242:                # sched_getaffinity
            return self.sys_getaffinity(a[0], a[1], a[2])
        if n == 251:                # epoll_ctl
            return -38
        if n == 263:                # clock_gettime (timespec de 32 bits)
            segundos, nanos = self.ahora(a[0])
            self.timespec32(a[1], segundos, nanos)
            return 0
        if n == 264:                # clock_getres (timespec de 32 bits)
            self.timespec32(a[1], 0, 1)
            return 0
        if n == 266:                # statfs64
            return self.sys_statfs64(a[0], a[1])
        if n == 256:                # set_tid_address
            return os.getpid()
        if n == 268:                # tgkill
            return 0 if a[1] == os.getpid() else -3  # ESRCH
        if n == 322:                # openat
            return self.sys_openat(a[0], a[1], a[2], a[3])
        if n == 323:                # mkdirat
            return self.sys_mkdirat(self.s32(a[0]), a[1], a[2])
        if n == 328:                # unlinkat
            return self.sys_unlinkat(self.s32(a[0]), a[1], a[2])
        if n == 329:                # renameat
            return self.sys_renameat(self.s32(a[0]), a[1], self.s32(a[2]), a[3], 0)
        if n == 336:                # ppoll (timespec de 32 bits)
            if a[2]:
                segundos = struct.unpack_from("<i", bytes(self.leer(a[2], 4)), 0)[0]
                nanos = struct.unpack_from("<i", bytes(self.leer(a[2] + 4, 4)), 0)[0]
                return self.sys_ppoll_tiempo(segundos + nanos / 1e9)
            return self.sys_ppoll(0)
        if n == 346:                # epoll_pwait
            return -38
        if n == 348:                # utimensat (timespec de 32 bits)
            return self.sys_utimensat(a[0], a[1], a[2], a[3], 32)
        if n == 350:                # timerfd_create
            return -38
        if n == 356:                # eventfd2
            return -38
        if n == 357:                # epoll_create1
            return -38
        if n == 358:                # dup3
            return self.sys_dup(a[0], a[1])
        if n == 359:                # pipe2
            return self.sys_pipe2(a[0], a[1])
        if n == 360:                # inotify_init1
            return -38
        if n == 374:                # sendmmsg
            return -38
        if n == 382:                # renameat2
            return self.sys_renameat(self.s32(a[0]), a[1], self.s32(a[2]), a[3], a[4])
        if n == 384:                # getrandom
            datos = bytes(self.aleatorio.randrange(256) for _ in range(a[1]))
            self.escribir(a[0], datos)
            return a[1]
        if n == 387:                # execveat
            return -38
        if n == 397:                # statx
            return self.sys_statx(a[0], a[1], a[2], a[3], a[4])
        if n == 403:                # clock_gettime64
            segundos, nanos = self.ahora(a[0])
            self.timespec(a[1], segundos, nanos)
            return 0
        if n == 406:                # clock_getres_time64
            self.timespec(a[1], 0, 1)
            return 0
        if n == 407:                # clock_nanosleep_time64
            return self.sys_clock_nanosleep64(a[0], a[1], a[2], a[3])
        if n == 412:                # utimensat_time64
            return self.sys_utimensat(a[0], a[1], a[2], a[3], 64)
        if n == 414:                # ppoll_time64
            return self.sys_ppoll(a[2])
        if n == 434:                # pidfd_open
            return -38
        if n == 436:                # close_range
            primero = a[0]
            ultimo = a[1]
            if ultimo == 0xFFFFFFFF:
                ultimo = 1 << 30
            for g in list(self.fds):
                if primero <= g <= ultimo and g > 2:
                    self.cerrar_fd(g)
            return 0
        if n == 437:                # openat2
            return -38
        # Nunca se inventa un resultado: lo que no está se avisa y se
        # devuelve -ENOSYS, como haría un núcleo con esa llamada sin soporte.
        print("[arm32] llamada al sistema %d sin implementar en el ejecutor: -ENOSYS" % n, file=sys.stderr)
        return -E_NOSYS

    # Las llamadas *at que no tienen una forma propia más simple.
    def sys_at(self, n, a):
        dirfd = self.s32(a[0])
        if n == 9:                  # link
            return self.sys_linkat(-100, a[0], -100, a[1], 0)
        if n == 330:                # linkat
            return self.sys_linkat(dirfd, a[1], self.s32(a[2]), a[3], a[4])
        if n == 331:                # symlinkat
            return self.sys_symlinkat(a[0], dirfd, a[1])
        if n == 332:                # readlinkat
            return self.sys_readlinkat(dirfd, a[1], a[2], a[3])
        if n == 333:                # fchmodat
            return self.sys_fchmodat(dirfd, a[1], a[2], a[3])
        if n == 334:                # faccessat
            return self.sys_faccessat(dirfd, a[1], a[2], 0)
        if n == 439:                # faccessat2
            return self.sys_faccessat(dirfd, a[1], a[2], a[3])
        if n == 325:                # fchownat
            return 0
        return -E_NOSYS

    def sys_linkat(self, viejo_fd, viejo, nuevo_fd, nuevo, banderas):
        extra = {}
        if self.s32(viejo_fd) != -100:
            host = self.fd_de(self.s32(viejo_fd))
            if host < 0:
                return -9
            extra["src_dir_fd"] = host
        if self.s32(nuevo_fd) != -100:
            host = self.fd_de(self.s32(nuevo_fd))
            if host < 0:
                return -9
            extra["dst_dir_fd"] = host
        try:
            os.link(os.fsdecode(self.cadena(viejo)), os.fsdecode(self.cadena(nuevo)),
                    follow_symlinks=not (banderas & 0x400), **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_symlinkat(self, destino, nuevo_fd, nuevo):
        extra = {}
        if self.s32(nuevo_fd) != -100:
            host = self.fd_de(self.s32(nuevo_fd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            os.symlink(os.fsdecode(self.cadena(destino)), os.fsdecode(self.cadena(nuevo)), **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_readlinkat(self, dirfd, p, buf, tamano):
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            # /proc/self/exe (y /proc/<pid>/exe): la ruta del programa emulado.
            propio = self.ruta_de_proc(os.fsdecode(self.cadena(p)))
            if propio is not None and self.s32(dirfd) == -100:
                destino = propio
            else:
                destino = os.readlink(os.fsdecode(self.cadena(p)), **extra)
        except OSError as error:
            return -error.errno
        datos = os.fsencode(destino)
        if len(datos) > tamano:
            datos = datos[:tamano]
        self.escribir(buf, datos)
        return len(datos)

    def sys_mkdirat(self, dirfd, p, modo):
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            os.mkdir(os.fsdecode(self.cadena(p)), modo & 0o7777, **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_unlinkat(self, dirfd, p, banderas):
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            ruta = os.fsdecode(self.cadena(p))
            if banderas & 0x200:    # AT_REMOVEDIR
                os.rmdir(ruta, **extra)
            else:
                os.unlink(ruta, **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_renameat(self, viejo_fd, viejo, nuevo_fd, nuevo, banderas):
        extra = {}
        if self.s32(viejo_fd) != -100:
            host = self.fd_de(self.s32(viejo_fd))
            if host < 0:
                return -9
            extra["src_dir_fd"] = host
        if self.s32(nuevo_fd) != -100:
            host = self.fd_de(self.s32(nuevo_fd))
            if host < 0:
                return -9
            extra["dst_dir_fd"] = host
        origen = os.fsdecode(self.cadena(viejo))
        destino = os.fsdecode(self.cadena(nuevo))
        try:
            if banderas & 1:        # RENAME_NOREPLACE
                if os.path.exists(destino):
                    return -17
            if banderas & 2:        # RENAME_EXCHANGE
                os.rename(destino, destino + ".titan-intercambio", **extra)
                os.rename(origen, destino, **extra)
                os.rename(destino + ".titan-intercambio", origen, **extra)
            else:
                os.rename(origen, destino, **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_fchmodat(self, dirfd, p, modo, banderas):
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            os.chmod(os.fsdecode(self.cadena(p)), modo & 0o7777, follow_symlinks=not (banderas & 0x100), **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_faccessat(self, dirfd, p, modo, banderas):
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        try:
            ok = os.access(os.fsdecode(self.cadena(p)), modo & 7, follow_symlinks=not (banderas & 0x100), **extra)
        except OSError as error:
            return -error.errno
        return 0 if ok else -13

    def sys_utimensat(self, dirfd, p, tiempos, banderas, bits):
        # struct timespec[2] (atime, mtime). En 64 bits los campos son de 64
        # bits; en 32, de 32. UTIME_OMIT (1073741822) deja ese tiempo como está
        # y UTIME_NOW (1073741823) pone el de ahora.
        if tiempos == 0:
            return 0
        ancho = 8 if bits == 64 else 4
        formato = "<q" if bits == 64 else "<i"
        valores = []
        for k in (0, 1):
            segundos = struct.unpack_from(formato, bytes(self.leer(tiempos + k * 2 * ancho, ancho)), 0)[0]
            nanos = struct.unpack_from(formato, bytes(self.leer(tiempos + k * 2 * ancho + ancho, ancho)), 0)[0]
            if nanos == 1073741822:      # UTIME_OMIT
                valores.append(None)
            elif nanos == 1073741823:    # UTIME_NOW
                valores.append(time.time())
            else:
                valores.append(segundos + nanos / 1e9)
        extra = {}
        if self.s32(dirfd) != -100:
            host = self.fd_de(self.s32(dirfd))
            if host < 0:
                return -9
            extra["dir_fd"] = host
        ruta = os.fsdecode(self.cadena(p))
        try:
            # `times` no admite None: el tiempo que se deja igual se pasa con
            # el valor que el fichero ya tiene.
            actual = os.stat(ruta, **extra)
            cuando = (valores[0] if valores[0] is not None else actual.st_atime,
                      valores[1] if valores[1] is not None else actual.st_mtime)
            os.utime(ruta, times=cuando, follow_symlinks=not (banderas & 0x100), **extra)
        except OSError as error:
            return -error.errno
        return 0

    def sys_madvise(self, direccion, largo, consejo):
        # MADV_DONTNEED (4) y MADV_FREE (8) sueltan memoria anónima: el programa
        # vuelve a leerla como ceros. Se imita eso (ceros) y se da por hecho,
        # que es lo único que el programa puede notar; el resto de consejos no
        # cambian nada de lo que se ve desde dentro.
        if consejo in (4, 8):
            if direccion % PAGE:
                return -22  # EINVAL: la dirección tiene que estar alineada
            largo = largo & ~(PAGE - 1)
            if largo > 0:
                try:
                    self.escribir(direccion, bytes(largo))
                except Exception:
                    pass    # lo que no está mapeado no hay que soltarlo
        return 0

    def sys_ftruncate(self, g, largo):
        host = self.fd_de(g)
        if host < 0:
            return -9
        try:
            os.ftruncate(host, largo)
        except OSError as error:
            return -error.errno
        return 0

    def sys_ftruncate64(self, g, lo, hi):
        largo = (lo | (hi << 32)) & 0xFFFFFFFFFFFFFFFF
        return self.sys_ftruncate(g, largo)

    def sys_llseek(self, g, hi, lo, resultado, origen):
        # _llseek(fd, offset_high, offset_low, result, whence): el resultado es
        # un desplazamiento de 64 bits que se escribe en dos palabras de 32.
        desplazamiento = (hi << 32) | lo
        if desplazamiento >= 1 << 63:
            desplazamiento -= 1 << 64
        host = self.fd_de(g)
        if host < 0:
            return -9
        try:
            nuevo = os.lseek(host, desplazamiento, origen)
        except OSError as error:
            return -error.errno
        self.escribir(resultado, struct.pack("<II", nuevo & 0xFFFFFFFF, (nuevo >> 32) & 0xFFFFFFFF))
        return 0

    def sys_pread(self, g, buf, cuenta, lo, hi):
        host = self.fd_de(g)
        if host < 0:
            return -9
        posicion = (hi << 32) | lo
        try:
            datos = os.pread(host, cuenta, posicion)
        except OSError as error:
            return -error.errno
        self.escribir(buf, datos)
        return len(datos)

    def sys_pwrite(self, g, buf, cuenta, lo, hi):
        host = self.fd_de(g)
        if host < 0:
            return -9
        posicion = (hi << 32) | lo
        try:
            return os.pwrite(host, bytes(self.leer(buf, cuenta)), posicion)
        except OSError as error:
            return -error.errno

    def sys_readv(self, g, iov, n):
        if g in (0,):
            return 0
        host = self.fd_de(g)
        if host < 0:
            return -9
        total = 0
        for i in range(n):
            base, largo = struct.unpack("<II", bytes(self.leer(iov + i * 8, 8)))
            if largo == 0:
                continue
            try:
                datos = os.read(host, largo)
            except OSError as error:
                return -error.errno if total == 0 else total
            if not datos:
                break
            self.escribir(base, datos)
            total += len(datos)
            if len(datos) < largo:
                break
        return total

    def sys_dup(self, viejo, nuevo):
        origen = self.fd_de(viejo) if viejo > 2 else viejo
        if origen < 0:
            return -9
        if nuevo in (0, 1, 2):
            return nuevo if nuevo == viejo else -9
        if nuevo < 0:
            try:
                return self.nuevo_fd(os.dup(origen))
            except OSError as error:
                return -error.errno
        # dup2: si el destino ya tenía descriptor, se reutiliza el del anfitrión.
        anterior = self.fd_de(nuevo)
        try:
            if anterior == origen:
                return nuevo
            if anterior >= 0:
                os.dup2(origen, anterior)
                return nuevo
            self.fds[nuevo] = os.dup(origen)
            self.siguiente_fd = max(self.siguiente_fd, nuevo + 1)
        except OSError as error:
            return -error.errno
        return nuevo

    def sys_pipe2(self, p, banderas):
        try:
            lectura, escritura = os.pipe()
        except OSError as error:
            return -error.errno
        self.escribir(p, struct.pack("<II", self.nuevo_fd(lectura), self.nuevo_fd(escritura)))
        return 0

    # El reloj es el del anfitrión: el programa no puede notar la diferencia
    # (y, si no, una marca de tiempo inventada haría fallar cuentas de fechas).
    def ahora(self, reloj):
        # 0 CLOCK_REALTIME, 1 CLOCK_MONOTONIC, 2 CPU del proceso,
        # 3 CPU del hilo, 4 CLOCK_MONOTONIC_RAW.
        if reloj == 0:
            t = time.time()
        elif reloj in (1, 4):
            t = time.monotonic()
        else:
            t = time.process_time()
        segundos = int(t)
        return segundos, int((t - segundos) * 1e9)

    def sys_clock_nanosleep64(self, reloj, banderas, req, rem):
        if req == 0:
            return -14  # EFAULT
        segundos = struct.unpack_from("<q", bytes(self.leer(req, 8)), 0)[0]
        nanos = struct.unpack_from("<I", bytes(self.leer(req + 8, 4)), 0)[0]
        if banderas & 1:            # TIMER_ABSTIME: se duerme hasta ese instante
            if reloj == 0:
                objetivo = segundos + nanos / 1e9 - time.time()
            else:
                objetivo = segundos + nanos / 1e9 - time.monotonic()
        else:
            objetivo = segundos + nanos / 1e9
        if objetivo > 0:
            time.sleep(objetivo)
        return 0

    def sys_ppoll(self, ts):
        if ts == 0:
            return 0              # sin límite de tiempo y nada listo: no pasa
        segundos = struct.unpack_from("<q", bytes(self.leer(ts, 8)), 0)[0]
        nanos = struct.unpack_from("<I", bytes(self.leer(ts + 8, 4)), 0)[0]
        return self.sys_ppoll_tiempo(segundos + nanos / 1e9)

    def sys_ppoll_tiempo(self, espera):
        if espera > 0:
            time.sleep(espera)
        return 0

    def sys_getaffinity(self, pid, mascara, tamano):
        # cpu_set_t: un bit por procesador. Se devuelve la afinidad de verdad
        # (la del ejecutor): el programa ve la misma máquina que en x86-64.
        if tamano < 8:
            return -22  # EINVAL
        cpus = sorted(os.sched_getaffinity(0))
        datos = bytearray(tamano)
        for c in cpus:
            if c // 8 < tamano:
                datos[c // 8] |= 1 << (c % 8)
        self.escribir(mascara, bytes(datos))
        return len(datos)

    def sys_gettimeofday(self, tv, tz):
        # struct timeval de 32 bits: segundos y microsegundos.
        if tv:
            t = time.time()
            segundos = int(t)
            self.escribir(tv, struct.pack("<II", segundos, int((t - segundos) * 1e6)))
        return 0

    def sys_statfs64(self, p, buf):
        # struct statfs64 de ARM (84 bytes): tipo@0 bsize@4 bloques@8 libres@16
        # disponibles@24 ficheros@32 libres@40 fsid@48 (8) namelen@56
        # frsize@60 banderas@64.
        try:
            info = os.statvfs(os.fsdecode(self.cadena(p)))
        except OSError as error:
            return -error.errno
        datos = bytearray(84)
        struct.pack_into("<iI", datos, 0, 0xEF53, info.f_bsize)
        struct.pack_into("<Q", datos, 8, info.f_blocks)
        struct.pack_into("<Q", datos, 16, info.f_bfree)
        struct.pack_into("<Q", datos, 24, info.f_bavail)
        struct.pack_into("<Q", datos, 32, info.f_files)
        struct.pack_into("<Q", datos, 40, info.f_ffree)
        struct.pack_into("<ii", datos, 48, 0, 0)
        struct.pack_into("<i", datos, 56, info.f_namemax)
        struct.pack_into("<i", datos, 60, info.f_frsize)
        struct.pack_into("<i", datos, 64, getattr(info, "f_flag", 0))
        self.escribir(buf, bytes(datos))
        return 0

    def timespec32(self, direccion, segundos, nanos):
        self.escribir(direccion, struct.pack("<ii", segundos, nanos))

    def escribir_fd(self, fd, buf, cuenta):
        if fd not in (1, 2):
            return -E_INVAL
        datos = bytes(self.leer(buf, cuenta))
        # La salida se escribe en binario: el programa decide su codificación.
        destino = self.salida if fd == 1 else self.error
        destino.write(datos)
        destino.flush()
        return cuenta

    def writev(self, fd, iov, n):
        total = 0
        for i in range(n):
            base, largo = struct.unpack("<II", bytes(self.leer(iov + i * 8, 8)))
            if base == 0 or largo == 0:
                continue
            r = self.sys_write(fd, base, largo)
            if r < 0:
                return r if total == 0 else total
            total += r
        return total

    def timespec(self, direccion, segundos, nanos):
        self.escribir(direccion, struct.pack("<qI", segundos, nanos))

    def getcwd(self, buf, tamano):
        texto = os.fsencode(os.getcwd()) + b"\0"
        if tamano < len(texto):
            return -34  # ERANGE
        self.escribir(buf, texto)
        return len(texto)

    def uname(self, buf):
        # struct utsname de ARM 32: seis campos de 65 bytes. Los datos del
        # sistema son los de la máquina donde se ejecuta el ejecutor (es la
        # misma que ejecutaría el binario en x86-64); la arquitectura es la del
        # procesador que se está emulando, que es la verdad para este binario.
        info = os.uname()
        campos = [b"Linux", os.fsencode(info.nodename), os.fsencode(info.release),
                  os.fsencode(info.version), b"armv7l", b"(none)"]
        datos = b"".join(c[:64] + b"\0" * (65 - len(c[:64])) for c in campos)
        self.escribir(buf, datos)
        return 0

    def brk_syscall(self, direccion):
        if direccion == 0:
            return self.brk
        if direccion < self.brk_inicio or direccion >= STACK_TOP - STACK_SIZE:
            return self.brk
        if direccion > self.brk:
            self.mem.mapear(self.brk, direccion - self.brk)
        self.brk = direccion
        return self.brk

    # ------------------------------------------------------------------ run
    def pista_de_fallo(self, pc):
        """Qué registros e instrucción había donde murió el programa."""
        nombres = [(UC_ARM_REG_R0 + k, "r%d" % k) for k in range(13)]
        nombres += [(UC_ARM_REG_SP, "sp"), (UC_ARM_REG_LR, "lr"), (UC_ARM_REG_PC, "pc")]
        valores = {}
        partes = []
        for registro, nombre in nombres:
            try:
                valor = self.uc.reg_read(registro)
            except Exception:
                continue
            valores[nombre] = valor
            partes.append("%s=%08x" % (nombre, valor))
        lineas = ["        registros: " + " ".join(partes)]
        try:
            trozo = bytes(self.uc.mem_read(pc, 4))
        except Exception:
            trozo = b""
        if trozo:
            from capstone import Cs, CS_ARCH_ARM, CS_MODE_ARM
            for ins in Cs(CS_ARCH_ARM, CS_MODE_ARM).disasm(trozo, pc):
                lineas.append("        instrucción: %s %s" % (ins.mnemonic, ins.op_str))
                coincide = WSO.search(ins.op_str)
                if coincide:
                    base = valores.get(coincide.group(1), 0)
                    desplazamiento = coincide.group(3)
                    if desplazamiento:
                        base = (base + int(desplazamiento, 0)) & 0xFFFFFFFF
                    lineas.append("        dirección: %08x (según %s)" % (base, coincide.group(1)))
        return lineas

    def correr(self):
        arranque = time.perf_counter()
        # Si el procesador vuelve sin que el programa haya terminado (por
        # ejemplo, porque se saltó a donde no hay código), no se puede dar por
        # bueno: se avisa y se cuenta como fallo.
        self.parado = False
        try:
            if self.ventana_activa or (self.tope is None and not self.ritmo):
                self.uc.emu_start(self.entrada & ~1, 0, 0, 0)
                self.parado = True
            elif self.tope is None:
                self.ritmo_correr()      # solo se quiere medir: nada corta
            else:
                # El tope de instrucciones lo cuenta la propia máquina. Se
                # avanza a trozos: si la máquina se para sola por haber llegado
                # al tope, el programa se cortó.
                restante = self.tope
                while restante > 0:
                    trozo = min(restante, 1 << 24)
                    self.uc.emu_start(self.uc.reg_read(UC_ARM_REG_PC), 0, 0, trozo)
                    restante -= trozo
                self.cortado = True
                raise Salida(255)
        except Salida as fin:
            self.ritmo_aviso(time.perf_counter() - arranque)
            if self.cortado:
                print("[arm32] se cortó en %d instrucciones (TITAN_ARM32_PASOS)" % self.tope, file=sys.stderr)
                self.desmontar_ventana()
                self.mostrar_perfil()
            return fin.codigo
        except Exception as error:
            # Un fallo del procesador se informa tal cual (no se disimula): el
            # programa murió como moriría en hardware.
            pc = self.uc.reg_read(UC_ARM_REG_PC)
            texto = str(error)
            print("[arm32] el programa murió en 0x%08x: %s" % (pc, texto), file=sys.stderr)
            for linea in self.pista_de_fallo(pc):
                print(linea, file=sys.stderr)
            print("        (con TITAN_ARM32_TRAZA=1 se ve cada llamada al sistema)", file=sys.stderr)
            if os.environ.get("TITAN_ARM32_FALLO"):
                import traceback
                traceback.print_exc()
            self.desmontar_ventana()
            return 139
        if self.parado:
            pc = self.uc.reg_read(UC_ARM_REG_PC)
            print("[arm32] el procesador se paró en 0x%08x sin que el programa terminara" % pc, file=sys.stderr)
            for linea in self.pista_de_fallo(pc):
                print(linea, file=sys.stderr)
            print("        (con TITAN_ARM32_VENTANA=1 se ven las últimas instrucciones)", file=sys.stderr)
            self.desmontar_ventana()
            return 139
        return 0


def main():
    # Las banderas del ejecutor (--codigo y --max-passos N) pueden ir antes o
    # después del programa; cualquier otra cosa, incluidos los argumentos que
    # empiezan por `--`, es del programa. Antes cualquier `--` se lo quedaba el
    # ejecutor y no llegaba al programa: `titan compile x --target armv7` corría
    # sin `--target` (y decidía la arquitectura por su cuenta).
    codigo = "--codigo" in sys.argv
    maximo = 200_000_000
    if "--max-passos" in sys.argv:
        maximo = int(sys.argv[sys.argv.index("--max-passos") + 1])
    libres = []
    i = 1
    while i < len(sys.argv):
        if sys.argv[i] == "--max-passos":
            i += 2
            continue
        if sys.argv[i] == "--codigo":
            i += 1
            continue
        libres.append(sys.argv[i])
        i += 1
    if not libres:
        print(__doc__)
        return 2
    camino = libres[0]
    if os.environ.get("TITAN_ARM32_FALLO"):
        # Para mirar un programa que se queda colgado: `kill -USR1 PID` imprime
        # por stderr dónde está el ejecutor en ese momento (la pila de Python).
        import faulthandler
        faulthandler.register(signal.SIGUSR1, all_threads=True)
    argv = ["%s" % os.path.basename(camino)] + libres[1:]
    # El programa escribe en stdout tal cual; lo del ejecutor va a stderr.
    # Y, como qemu-arm, el ejecutor termina con el mismo código que el
    # programa (así el arnés de pruebas solo tiene que mirar $?).
    # El entorno es el del ejecutor (como hace qemu-arm), con una marca para
    # que el programa sepa dónde está corriendo.
    entorno = [k + "=" + v for k, v in os.environ.items()]
    entorno.append("TITAN_ARM32=1")
    proceso = Proceso(camino, argv, entorno, sys.stdout, maximo, sys.stderr)
    fin = proceso.correr()
    if codigo:
        print(fin, file=sys.stderr)
    return fin


if __name__ == "__main__":
    sys.exit(main())
