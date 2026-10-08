#!/usr/bin/env python3
"""Perfil por muestreo de un ejecutable nativo de Titan (herramienta de desarrollo).

Uso:
    zett run selfhost/build.titan PROG.titan /tmp/prog /tmp/prog.map
    python3 selfhost/native/profile.py /tmp/prog.map /tmp/prog ARGUMENTOS...

Detiene el programa unas mil veces por segundo con ptrace, anota en qué
función estaba (propio) y qué funciones había en la cadena de llamadas
(inclusivo, siguiendo rbp: cada marco guarda el rbp anterior y la dirección
de vuelta), y al terminar imprime las funciones que más tiempo acumulan.
Solo lee el programa; no cambia lo que hace.
"""
import bisect
import ctypes
import os
import signal
import struct
import subprocess
import sys
import time

PTRACE_GETREGS = 12
PTRACE_CONT = 7
PTRACE_SEIZE = 0x4206
PTRACE_INTERRUPT = 0x4207
PTRACE_DETACH = 17

libc = ctypes.CDLL(None, use_errno=True)
libc.ptrace.restype = ctypes.c_long
libc.ptrace.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]


class Regs(ctypes.Structure):
    _fields_ = [(n, ctypes.c_ulonglong) for n in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    starts, names = [], []
    for line in open(sys.argv[1]):
        a, name = line.rstrip("\n").split(" ", 1)
        starts.append(int(a))
        names.append(name)

    def where(addr):
        i = bisect.bisect_right(starts, addr) - 1
        return names[i] if i >= 0 else "?"

    child = subprocess.Popen(sys.argv[2:], stdout=subprocess.DEVNULL)
    pid = child.pid
    if libc.ptrace(PTRACE_SEIZE, pid, None, None) != 0:
        print("ptrace no permitido:", os.strerror(ctypes.get_errno()))
        child.wait()
        return 1
    mem = open(f"/proc/{pid}/mem", "rb", buffering=0)
    own, incl, samples = {}, {}, 0
    regs = Regs()
    started = time.time()
    while True:
        time.sleep(0.001)
        if libc.ptrace(PTRACE_INTERRUPT, pid, None, None) != 0:
            break
        _, status = os.waitpid(pid, 0)
        if os.WIFEXITED(status) or os.WIFSIGNALED(status):
            break
        libc.ptrace(PTRACE_GETREGS, pid, None, ctypes.byref(regs))
        samples += 1
        f = where(regs.rip)
        own[f] = own.get(f, 0) + 1
        seen = {f}
        rbp = regs.rbp
        for _ in range(4000):
            try:
                mem.seek(rbp)
                saved, ret = struct.unpack("<QQ", mem.read(16))
            except (OSError, ValueError, struct.error):
                break
            g = where(ret)
            seen.add(g)
            if saved <= rbp:
                break
            rbp = saved
        for g in seen:
            incl[g] = incl.get(g, 0) + 1
        libc.ptrace(PTRACE_CONT, pid, None, None)
    child.wait()
    elapsed = time.time() - started
    print(f"{samples} muestras en {elapsed:.1f} s")
    print("\n-- tiempo propio --")
    for name, n in sorted(own.items(), key=lambda kv: -kv[1])[:25]:
        print(f"{100.0 * n / max(samples, 1):6.1f}%  {name}")
    print("\n-- tiempo inclusivo --")
    for name, n in sorted(incl.items(), key=lambda kv: -kv[1])[:40]:
        print(f"{100.0 * n / max(samples, 1):6.1f}%  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
