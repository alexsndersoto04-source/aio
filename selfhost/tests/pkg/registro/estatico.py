#!/usr/bin/env python3
"""Servidor HTTPS de ficheros estáticos: imita cómo GitHub sirve el directorio `registro/`.
Uso: estatico.py PUERTO CERT KEY DIRECTORIO"""
import http.server, ssl, sys, functools
port, cert, key, root = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
class H(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a): pass
handler = functools.partial(H, directory=root)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
srv.serve_forever()
