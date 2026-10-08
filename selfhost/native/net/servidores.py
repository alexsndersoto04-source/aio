# Servidores HTTP locales para las pruebas de std::net: cada uno responde con
# su propia dirección en el cuerpo (así se ve qué dirección eligió
# getaddrinfo). Uso: python3 servidores.py (hasta que se mate).
import socket, threading

DIRS = [("127.0.0.1", 8000), ("127.0.0.2", 8000), ("127.0.0.3", 8000), ("::1", 8000)]

def serve(host, port):
    fam = socket.AF_INET6 if ":" in host else socket.AF_INET
    s = socket.socket(fam, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    if fam == socket.AF_INET6:
        s.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
    s.bind((host, port))
    s.listen(50)
    while True:
        c, _ = s.accept()
        try:
            data = b""
            while b"\r\n\r\n" not in data:
                chunk = c.recv(4096)
                if not chunk:
                    break
                data += chunk
            host_hdr = b""
            for line in data.split(b"\r\n"):
                if line.lower().startswith(b"host:"):
                    host_hdr = line[5:].strip()
            body = host.encode() + b" " + host_hdr
            c.sendall(b"HTTP/1.0 200 OK\r\nX-A: 1\r\n\r\n" + body)
        finally:
            c.close()

for h, p in DIRS:
    threading.Thread(target=serve, args=(h, p), daemon=True).start()
threading.Event().wait()
