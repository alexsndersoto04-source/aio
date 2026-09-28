#!/usr/bin/env python3
# Ejecuta un comando dentro de una pseudoterminal (stdin/stdout/stderr son la
# terminal, tamaño FILAS x COLUMNAS) y escribe en la salida estándar la
# representación repr() de todos los bytes que el programa dibujó.
# Uso: pty_run.py FILAS COLUMNAS comando args...
import fcntl, os, pty, struct, sys, termios
rows, cols = int(sys.argv[1]), int(sys.argv[2])
pid, fd = pty.fork()
if pid == 0:
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
    # Sin conversión \n -> \r\n, para ver los bytes tal cual.
    attrs = termios.tcgetattr(0)
    attrs[1] &= ~termios.OPOST
    termios.tcsetattr(0, termios.TCSANOW, attrs)
    os.execvp(sys.argv[3], sys.argv[3:])
out = b''
while True:
    try:
        d = os.read(fd, 65536)
    except OSError:
        break
    if not d:
        break
    out += d
_, st = os.waitpid(pid, 0)
print(repr(out))
print('estado', os.waitstatus_to_exitcode(st))
