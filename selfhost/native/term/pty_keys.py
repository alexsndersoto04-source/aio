#!/usr/bin/env python3
# Ejecuta un comando dentro de una pseudoterminal y le "teclea" cosas.
# Espera a que el programa escriba LISTO y luego ejecuta los pasos:
#   s:SEG        esperar SEG segundos
#   k:HEX        escribir esos bytes (en hexadecimal) en la terminal
#   r:FILAS:COL  cambiar el tamaño de la ventana (el núcleo manda SIGWINCH)
#   t            anotar la configuración de la terminal (termios) en ese momento
# Al final escribe repr() de todo lo que el programa dibujó, las anotaciones
# de termios y el estado de salida.
# Uso: pty_keys.py FILAS COLUMNAS 'paso paso ...' comando args...
import fcntl, os, pty, select, struct, sys, termios, time

rows, cols = int(sys.argv[1]), int(sys.argv[2])
steps = sys.argv[3].split()
pid, fd = pty.fork()
if pid == 0:
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
    attrs = termios.tcgetattr(0)
    attrs[1] &= ~termios.OPOST
    termios.tcsetattr(0, termios.TCSANOW, attrs)
    os.execvp(sys.argv[4], sys.argv[4:])

out = b''
snaps = []


def pump(timeout):
    global out
    r, _, _ = select.select([fd], [], [], timeout)
    if not r:
        return True
    try:
        d = os.read(fd, 65536)
    except OSError:
        return False
    if not d:
        return False
    out += d
    return True


deadline = time.time() + 60
while b'LISTO' not in out and time.time() < deadline:
    if not pump(0.05):
        break
alive = True
for st in steps:
    if st.startswith('s:'):
        end = time.time() + float(st[2:])
        while alive and time.time() < end:
            alive = pump(max(0.0, end - time.time()))
    elif st.startswith('k:'):
        os.write(fd, bytes.fromhex(st[2:]))
    elif st.startswith('r:'):
        _, r_, c_ = st.split(':')
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', int(r_), int(c_), 0, 0))
    elif st == 't':
        a = termios.tcgetattr(fd)
        snaps.append((a[0], a[1], a[2], a[3], a[6][termios.VMIN], a[6][termios.VTIME]))
end = time.time() + 20
code = None
while time.time() < end:
    if alive:
        alive = pump(0.1)
    w, stt = os.waitpid(pid, os.WNOHANG)
    if w:
        while alive:
            alive = pump(0.1)
        code = os.waitstatus_to_exitcode(stt)
        break
    if not alive:
        time.sleep(0.05)
if code is None:
    os.kill(pid, 9)
    _, stt = os.waitpid(pid, 0)
    code = 'colgado (matado)'
print(repr(out))
for s in snaps:
    print('termios', s)
print('estado', code)
