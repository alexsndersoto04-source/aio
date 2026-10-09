#!/usr/bin/env python3
"""Registro de paquetes de prueba (HTTPS) para probar `titan fetch/update/publish`.
Uso: registry.py PUERTO CERT KEY DIRDATOS   (DIRDATOS contiene index/<nombre>.json y archive/<archivo>)"""
import http.server, ssl, sys, os, json, base64, threading
port, cert, key, data = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
log = open(os.path.join(data, "requests.log"), "a")
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def send(self, code, body=b"", ctype="application/json", extra=None):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in (extra or {}).items(): self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        log.write("GET %s accept=%s\n" % (self.path, self.headers.get("Accept"))); log.flush()
        p = self.path
        if p.startswith("/v1/packages/"):
            name = p[len("/v1/packages/"):]
            f = os.path.join(data, "index", name + ".json")
            if "/" in name or not os.path.exists(f):
                return self.send(404, b'{"error":"not found"}')
            return self.send(200, open(f, "rb").read())
        if p.startswith("/archive/"):
            f = os.path.join(data, "archive", p[len("/archive/"):])
            if not os.path.exists(f): return self.send(404)
            return self.send(200, open(f, "rb").read(), "application/octet-stream")
        if p.startswith("/redirect-http/"):
            return self.send(302, b"", extra={"Location": "http://localhost:1/x"})
        if p.startswith("/redirect/"):
            return self.send(302, b"", extra={"Location": "/archive/" + p[len("/redirect/"):]})
        self.send(404)
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n)
        log.write("POST %s auth=%s ctype=%s len=%d\n" % (self.path, self.headers.get("Authorization"), self.headers.get("Content-Type"), n)); log.flush()
        open(os.path.join(data, "last_post.json"), "wb").write(body)
        tok = self.headers.get("Authorization")
        mode = open(os.path.join(data, "post_mode")).read().strip() if os.path.exists(os.path.join(data, "post_mode")) else "ok"
        if mode == "ok" and tok == "Bearer secret-token": return self.send(201, b"{}")
        if mode == "conflict": return self.send(409, b"{}")
        if mode == "denied": return self.send(403, b"{}")
        self.send(401, b"{}")
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
srv = http.server.ThreadingHTTPServer(("127.0.0.1", port), H)
srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
srv.serve_forever()
