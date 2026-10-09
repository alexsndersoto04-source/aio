"""Cliente mínimo de LSP por stdio para las pruebas: envía mensajes con Content-Length y lee las respuestas."""
import json, subprocess

def frame(msg):
    body = json.dumps(msg, separators=(",", ":"), ensure_ascii=False).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body

def parse_frames(data):
    out = []
    i = 0
    while i < len(data):
        j = data.index(b"\r\n\r\n", i)
        head = data[i:j].decode()
        n = int(head.split(":")[1])
        out.append(data[j + 4:j + 4 + n])
        i = j + 4 + n
    return out

def sesion(cli, mensajes, crudo=None, timeout=60):
    """Ejecuta `cli lsp` con los mensajes; devuelve (cuerpos_crudos, stderr, código)."""
    entrada = crudo if crudo is not None else b"".join(frame(m) for m in mensajes)
    r = subprocess.run([cli, "lsp"], input=entrada, capture_output=True, timeout=timeout)
    return parse_frames(r.stdout), r.stderr.decode(), r.returncode
