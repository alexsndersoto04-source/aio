#!/usr/bin/env python3
"""Ejecuta un programa Titan sin sistema operativo (aarch64-none) en un
procesador ARM64 emulado (unicorn = el motor de CPU de QEMU), para probarlo
en máquinas sin QEMU. En CI se prueba además con qemu-system-aarch64 de verdad.

Uso: correr.py PROGRAMA.elf [--entrada TEXTO] [--pasos N]

Máquina (la misma disposición que "virt" de QEMU):
  - RAM de 1 GiB en 0x40000000 (el ELF se carga en sus direcciones);
  - puerto serie PL011 en 0x09000000: DR (+0x00) escribe en la salida
    estándar / lee de --entrada; FR (+0x18) dice TXFF=0 y RXFE según quede
    entrada;
  - PSCI por hvc #0: SYSTEM_OFF (0x84000008) termina con el código de x1.
El código de salida del proceso es el que el programa dejó en x1.
"""
import struct
import sys

from unicorn import (Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_INTR,
                     UC_PROT_ALL, UC_TLB_CPU, UcError)
from unicorn.arm64_const import (UC_ARM64_REG_X0, UC_ARM64_REG_X1,
                                 UC_ARM64_REG_PC)

RAM, RAM_SIZE = 0x40000000, 0x40000000
UART = 0x09000000


def main():
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 2
    path = args[0]
    entrada = b""
    pasos = 0
    i = 1
    while i < len(args):
        if args[i] == "--entrada":
            entrada = args[i + 1].encode()
            i += 2
        elif args[i] == "--pasos":
            pasos = int(args[i + 1])
            i += 2
        else:
            print("argumento desconocido: " + args[i], file=sys.stderr)
            return 2
    data = open(path, "rb").read()
    if data[:4] != b"\x7fELF" or struct.unpack_from("<H", data, 18)[0] != 183:
        print("no es un ELF de AArch64", file=sys.stderr)
        return 2
    entry, phoff = struct.unpack_from("<QQ", data, 24)
    phentsize, phnum = struct.unpack_from("<HH", data, 54)

    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    # La MMU del procesador de verdad (tablas de páginas del programa), no la
    # traducción directa que unicorn usa por defecto.
    uc.ctl_set_tlb_mode(UC_TLB_CPU)
    uc.mem_map(RAM, RAM_SIZE, UC_PROT_ALL)
    for k in range(phnum):
        p_type, _flags, off, vaddr, _paddr, filesz, _memsz = struct.unpack_from(
            "<IIQQQQQ", data, phoff + k * phentsize)
        if p_type == 1 and filesz > 0:
            if vaddr < RAM or vaddr + filesz > RAM + RAM_SIZE:
                print("segmento fuera de la RAM: %#x" % vaddr, file=sys.stderr)
                return 2
            uc.mem_write(vaddr, data[off:off + filesz])

    salida = sys.stdout.buffer
    estado = {"pos": 0, "codigo": None}

    def uart_leer(uc, offset, size, user):
        if offset == 0x18:  # FR: TXFF (bit 5) nunca; RXFE (bit 4) si no queda entrada
            return 0 if estado["pos"] < len(entrada) else 0x10
        if offset == 0x00:
            if estado["pos"] < len(entrada):
                c = entrada[estado["pos"]]
                estado["pos"] += 1
                return c
            return 0
        return 0

    def uart_escribir(uc, offset, size, value, user):
        if offset == 0x00:
            salida.write(bytes([value & 0xFF]))

    uc.mmio_map(UART, 0x1000, uart_leer, None, uart_escribir, None)

    def interrupcion(uc, intno, user):
        x0 = uc.reg_read(UC_ARM64_REG_X0)
        pc = uc.reg_read(UC_ARM64_REG_PC)
        # hvc #0 (0xd4000002): sin EL2 el procesador lo da como excepción 1
        # (instrucción no definida) o 5 (hvc); aquí hace de firmware PSCI.
        es_hvc = 0xd4000002 in (struct.unpack("<I", uc.mem_read(pc, 4))[0],
                                struct.unpack("<I", uc.mem_read(pc - 4, 4))[0])
        if es_hvc and x0 == 0x84000008:  # PSCI SYSTEM_OFF
            estado["codigo"] = uc.reg_read(UC_ARM64_REG_X1) & 0xFF
            uc.emu_stop()
            return
        print("\n[correr.py] excepción %d en pc=%#x (x0=%#x)" % (intno, pc, x0),
              file=sys.stderr)
        estado["codigo"] = 134
        uc.emu_stop()

    uc.hook_add(UC_HOOK_INTR, interrupcion)
    try:
        uc.emu_start(entry, 0, count=pasos)
    except UcError as e:
        pc = uc.reg_read(UC_ARM64_REG_PC)
        salida.flush()
        print("\n[correr.py] fallo del procesador: %s en pc=%#x" % (e, pc),
              file=sys.stderr)
        return 134
    salida.flush()
    if estado["codigo"] is None:
        print("\n[correr.py] el programa se paró sin apagar la máquina",
              file=sys.stderr)
        return 135
    return estado["codigo"]


if __name__ == "__main__":
    sys.exit(main())
