#!/usr/bin/env python3
"""Perfilador por muestreo (sin perf): ejecuta un programa, lo detiene cada
pocos milisegundos con ptrace, lee el contador de programa (RIP) y cuenta en
qué función estaba, usando la tabla de símbolos del ejecutable (nm).

Uso: python3 selfhost/native/perfil.py [--cada MS] [--top N] PROGRAMA ARGS...

Solo Linux x86-64. Sirve para ejecutables estáticos de Titan (backend propio
o LLVM), que conservan los nombres de sus funciones.
"""
import bisect
import ctypes
import os
import signal
import subprocess
import sys
import time

PTRACE_TRACEME, PTRACE_CONT, PTRACE_GETREGS, PTRACE_ATTACH = 0, 7, 12, 16
libc = ctypes.CDLL(None, use_errno=True)
libc.ptrace.restype = ctypes.c_long
libc.ptrace.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p]


class Regs(ctypes.Structure):
    _fields_ = [(n, ctypes.c_ulonglong) for n in (
        "r15 r14 r13 r12 rbp rbx r11 r10 r9 r8 rax rcx rdx rsi rdi orig_rax "
        "rip cs eflags rsp ss fs_base gs_base ds es fs gs").split()]


def symbols(path):
    out = subprocess.run(["nm", "-n", path], capture_output=True, text=True).stdout
    addrs, names = [], []
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] in "tTwW":
            addrs.append(int(parts[0], 16))
            names.append(parts[2])
    return addrs, names


def main():
    args = sys.argv[1:]
    every, top = 2.0, 30
    while args and args[0].startswith("--"):
        if args[0] == "--cada":
            every = float(args[1])
        elif args[0] == "--top":
            top = int(args[1])
        args = args[2:]
    addrs, names = symbols(args[0])
    pid = os.fork()
    if pid == 0:
        libc.ptrace(PTRACE_TRACEME, 0, None, None)
        os.execv(args[0], args)
    os.waitpid(pid, 0)  # parada en execv
    counts = {}
    total = 0
    regs = Regs()
    libc.ptrace(PTRACE_CONT, pid, None, None)
    start = time.time()
    while True:
        time.sleep(every / 1000.0)
        try:
            os.kill(pid, signal.SIGSTOP)
        except ProcessLookupError:
            break
        _, status = os.waitpid(pid, 0)
        if os.WIFEXITED(status) or os.WIFSIGNALED(status):
            break
        sig = os.WSTOPSIG(status)
        if libc.ptrace(PTRACE_GETREGS, pid, None, ctypes.byref(regs)) == 0:
            i = bisect.bisect_right(addrs, regs.rip) - 1
            name = names[i] if i >= 0 else "?"
            counts[name] = counts.get(name, 0) + 1
            total += 1
        # Reenvía las señales que no son la nuestra.
        libc.ptrace(PTRACE_CONT, pid, None, None if sig == signal.SIGSTOP else ctypes.c_void_p(sig))
    elapsed = time.time() - start
    print(f"muestras: {total}  tiempo: {elapsed:.1f} s", file=sys.stderr)
    for name, c in sorted(counts.items(), key=lambda kv: -kv[1])[:top]:
        print(f"{100.0 * c / max(total, 1):6.2f}%  {c:6d}  {name}", file=sys.stderr)


if __name__ == "__main__":
    main()
