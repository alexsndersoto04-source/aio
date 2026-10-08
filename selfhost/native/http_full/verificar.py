#!/usr/bin/env python3
"""Prueba real y diferencial del cliente HTTP local de std::http_full."""

from __future__ import annotations

import gzip
import json
import os
import shutil
import socketserver
import ssl
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PROGRAM = ROOT / "selfhost/native/http_full/prueba_http_full.titan"


def json_bytes(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def send_body(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)
        self.close_connection = True

    def do_GET(self) -> None:
        if self.path == "/json":
            self.send_body(200, json_bytes({"msg": "respuesta", "ok": True}), "application/json")
            return
        if self.path == "/gzip":
            body = json_bytes({"msg": "gzip", "ok": True})
            if "gzip" in self.headers.get("Accept-Encoding", "").lower():
                body = gzip.compress(body, mtime=0)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Encoding", "gzip")
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(body)
                self.close_connection = True
            else:
                self.send_body(200, body, "application/json")
            return
        if self.path == "/redirect":
            if (
                self.headers.get("Authorization") != "Bearer token-123"
                or self.headers.get("User-Agent") != "http-full-test/1"
                or self.headers.get("X-Probe") != "seen"
            ):
                self.send_body(401, b"request options were not applied", "text/plain")
                return
            self.send_response(302)
            self.send_header("Location", "/reflect")
            self.send_header("Content-Length", "0")
            self.send_header("Connection", "close")
            self.end_headers()
            self.close_connection = True
            return
        if self.path == "/reflect":
            body = json_bytes(
                {
                    "accept_encoding": self.headers.get("Accept-Encoding", ""),
                    "authorization": self.headers.get("Authorization", ""),
                    "user_agent": self.headers.get("User-Agent", ""),
                    "x_probe": self.headers.get("X-Probe", ""),
                }
            )
            self.send_body(200, body, "application/json")
            return
        self.send_body(404, b"not found", "text/plain")

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        payload = self.rfile.read(length)
        if self.path == "/json-echo":
            if self.headers.get_content_type() != "application/json":
                self.send_body(415, b"wrong content type", "text/plain")
                return
            try:
                body = json_bytes(json.loads(payload))
            except (UnicodeDecodeError, json.JSONDecodeError):
                self.send_body(400, b"invalid json", "text/plain")
                return
            self.send_body(200, body, "application/json")
            return
        if self.path == "/form-echo":
            if self.headers.get_content_type() != "application/x-www-form-urlencoded":
                self.send_body(415, b"wrong content type", "text/plain")
                return
            self.send_body(200, payload, "application/x-www-form-urlencoded")
            return
        self.send_body(404, b"not found", "text/plain")


class TLSHandler(socketserver.BaseRequestHandler):
    requests = 0

    def handle(self) -> None:
        request = bytearray()
        while b"\r\n\r\n" not in request:
            chunk = self.request.recv(4096)
            if not chunk:
                return
            request.extend(chunk)
        TLSHandler.requests += 1
        first_line = bytes(request).split(b"\r\n", 1)[0].split(b" ")
        path = first_line[1] if len(first_line) > 1 else b"/"
        if path == b"/plain-redirect":
            self.request.sendall(
                b"HTTP/1.1 302 Found\r\nLocation: /plain-chunked\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            return
        if path == b"/plain-chunked":
            body = b"std-http-tls-ok"
            self.request.sendall(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n"
                b"Transfer-Encoding: chunked\r\n\r\n"
                + f"{len(body):x}\r\n".encode()
                + body
                + b"\r\n0\r\n\r\n"
            )
            return
        if path == b"/redirect":
            self.request.sendall(
                b"HTTP/1.1 302 Found\r\nLocation: /gzip\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            return
        if path == b"/slow":
            time.sleep(0.5)
        if path == b"/gzip":
            body = gzip.compress(json_bytes({"ok": True, "via": "verified TLS"}), mtime=0)
            response = (
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\n"
                b"Connection: close\r\nTransfer-Encoding: chunked\r\n\r\n"
                + f"{len(body):x}\r\n".encode()
                + body
                + b"\r\n0\r\n\r\n"
            )
        else:
            body = json_bytes({"ok": True, "via": "verified TLS"})
            response = (
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                + str(len(body)).encode()
                + b"\r\nConnection: close\r\n\r\n"
                + body
            )
        self.request.sendall(response)


class VerifiedTLSServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, address: tuple[str, int], certificate: Path, private_key: Path):
        self.context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.context.load_cert_chain(certificate, private_key)
        super().__init__(address, TLSHandler)

    def get_request(self) -> tuple[ssl.SSLSocket, tuple[str, int]]:
        connection, address = super().get_request()
        return self.context.wrap_socket(connection, server_side=True), address

    def shutdown_request(self, request: ssl.SSLSocket) -> None:
        try:
            request.unwrap().close()
        except (OSError, ssl.SSLError):
            request.close()

    def handle_error(self, _request: object, _client_address: object) -> None:
        pass


def run(
    command: list[str], *, timeout: int = 600, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(command, cwd=ROOT, capture_output=True, timeout=timeout, check=False, env=env)


def fail(label: str, result: subprocess.CompletedProcess[bytes]) -> None:
    print(f"FALLO: {label} (código {result.returncode})", file=sys.stderr)
    if result.stdout:
        print(result.stdout.decode(errors="replace"), file=sys.stderr)
    if result.stderr:
        print(result.stderr.decode(errors="replace"), file=sys.stderr)
    raise SystemExit(1)


def verify_real_tls(executable: str, temp_dir: str) -> None:
    certificate = Path(temp_dir) / "tls-local-cert.pem"
    private_key = Path(temp_dir) / "tls-local-key.pem"
    generated = run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(private_key),
            "-out",
            str(certificate),
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
        ],
        timeout=30,
    )
    if generated.returncode != 0:
        fail("creación de certificado local para probar TLS", generated)

    TLSHandler.requests = 0
    server = VerifiedTLSServer(("127.0.0.1", 0), certificate, private_key)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    url = f"https://localhost:{port}/redirect"
    trusted_env = os.environ.copy()
    trusted_env["SSL_CERT_FILE"] = str(certificate)
    trusted_env["SSL_CERT_DIR"] = str(Path(temp_dir) / "no-system-ca-directory")
    expected = json_bytes({"ok": True, "via": "verified TLS"}) + b"\n"

    try:
        good = run([executable, url, "--tls"], timeout=30, env=trusted_env)
        if good.returncode != 0:
            fail("HTTPS nativo con certificado confiable", good)
        if good.stdout != expected:
            print("FALLO: respuesta JSON distinta tras HTTPS, redirección y gzip.", file=sys.stderr)
            print(good.stdout.decode(errors="replace"), file=sys.stderr)
            raise SystemExit(1)
        if TLSHandler.requests != 2:
            raise AssertionError(f"la redirección TLS generó {TLSHandler.requests} solicitudes; se esperaban 2")

        std_http_url = f"https://localhost:{port}/plain-redirect"
        std_http = run([executable, std_http_url, "--tls-std"], timeout=30, env=trusted_env)
        std_http_expected = f"200\nhttps://localhost:{port}/plain-chunked\nstd-http-tls-ok\n".encode()
        if std_http.returncode != 0 or std_http.stdout != std_http_expected:
            fail("HTTPS nativo de std::http con redirección y respuesta chunked", std_http)
        if TLSHandler.requests != 4:
            raise AssertionError(f"la prueba de std::http generó {TLSHandler.requests} solicitudes; se esperaban 4")
        print("std::http también pasó HTTPS con redirección y respuesta chunked.")

        empty_ca = Path(temp_dir) / "empty-ca.pem"
        empty_ca.write_bytes(b"")
        untrusted_env = trusted_env.copy()
        untrusted_env["SSL_CERT_FILE"] = str(empty_ca)
        before = TLSHandler.requests
        untrusted = run([executable, url, "--tls"], timeout=30, env=untrusted_env)
        if untrusted.returncode == 0 or TLSHandler.requests != before:
            raise AssertionError("el cliente aceptó una CA desconocida o envió HTTP sin validarla")

        ip_url = f"https://127.0.0.1:{port}/gzip"
        wrong_name = run([executable, ip_url, "--tls"], timeout=30, env=trusted_env)
        if wrong_name.returncode == 0 or TLSHandler.requests != before:
            raise AssertionError("el cliente aceptó un certificado para otro nombre de servidor")

        slow_url = f"https://localhost:{port}/slow"
        started = time.monotonic()
        slow = run([executable, slow_url, "--tls-timeout"], timeout=5, env=trusted_env)
        elapsed = time.monotonic() - started
        if slow.returncode == 0 or b"timed out" not in slow.stderr or elapsed >= 3:
            raise AssertionError(
                f"el plazo TLS no se aplicó correctamente (código {slow.returncode}, {elapsed:.2f} s): "
                f"{slow.stderr.decode(errors='replace')}"
            )

        print("HTTPS nativo: certificado y nombre verificados; también pasaron redirección HTTPS y cuerpo gzip.")
        print("Seguridad comprobada: rechazó una CA desconocida y un nombre incorrecto; el plazo también se respeta.")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def main() -> None:
    zett = os.environ.get("ZETT") or shutil.which("zett") or str(Path.home() / ".local/bin/zett")
    if not Path(zett).is_file() or not os.access(zett, os.X_OK):
        raise SystemExit("No encuentro el binario precompilado `zett`; instálalo antes de ejecutar esta prueba.")

    compiler = os.environ.get("COMPILER", "")
    if compiler and (not Path(compiler).is_file() or not os.access(compiler, os.X_OK)):
        raise SystemExit(f"No encuentro el compilador Titan nativo ejecutable: {compiler}")

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_address[1]}"

    try:
        with tempfile.TemporaryDirectory(prefix="titan-http-full-") as tmp:
            executable = str(Path(tmp) / "http_full_test")
            if compiler:
                print("Compilador de la prueba: ejecutable nativo Titan del bootstrap.")
                build_command = [compiler, str(PROGRAM.relative_to(ROOT)), executable]
            else:
                print("Compilador de la prueba: selfhost/build.titan ejecutado por Zett precompilado.")
                build_command = [zett, "run", "selfhost/build.titan", str(PROGRAM.relative_to(ROOT)), executable]
            built = run(build_command)
            if built.returncode != 0 or not Path(executable).is_file():
                fail("compilación con el compilador Titan", built)

            vm = run([zett, "run", str(PROGRAM.relative_to(ROOT)), base])
            if vm.returncode != 0:
                fail("cliente de la VM", vm)
            native = run([executable, base])
            if native.returncode != 0:
                fail("cliente nativo", native)

            if vm.stdout != native.stdout or vm.stderr != native.stderr:
                print("DIFERENCIA entre VM y ejecutable nativo:", file=sys.stderr)
                print("--- VM ---", file=sys.stderr)
                print(vm.stdout.decode(errors="replace"), file=sys.stderr)
                print(vm.stderr.decode(errors="replace"), file=sys.stderr)
                print("--- Titan nativo ---", file=sys.stderr)
                print(native.stdout.decode(errors="replace"), file=sys.stderr)
                print(native.stderr.decode(errors="replace"), file=sys.stderr)
                raise SystemExit(1)

            output = native.stdout.decode()
            expected = [
                'GET JSON\n{"msg":"respuesta","ok":true}',
                'GET gzip JSON\n{"msg":"gzip","ok":true}',
                'POST JSON\n{"msg":"hola","n":42}',
                'note=hola+mundo&path=a%2Fb%2Bc',
                'http-full-test/1',
                '"authorization":""',
                'Basic YWxpY2U6c2VjcmV0',
                'x_probe',
                'Plain HTTP port rejects TLS: true',
                'HTTP 404 rejected: true',
                'Redirect limit honored: true',
                base + "/reflect",
            ]
            missing = [item for item in expected if item not in output]
            if missing:
                print("La salida coincidió entre VM y nativo, pero no con la respuesta HTTP esperada:", file=sys.stderr)
                print("Falta: " + ", ".join(repr(item) for item in missing), file=sys.stderr)
                print(output, file=sys.stderr)
                raise SystemExit(1)

            verify_real_tls(executable, tmp)

            print("std::http_full: VM y ejecutable nativo idénticos para las pruebas HTTP.")
            print("Probado: JSON, gzip, POST JSON, formulario, autenticación, cabeceras, redirecciones, límite de redirecciones y 404.")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


if __name__ == "__main__":
    main()
