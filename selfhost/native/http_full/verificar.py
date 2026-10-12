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

# Sin Rust: el lado de comparación es la VM de bytecode del port (`titan build`
# + `titan exec`, ver selfhost/native/vm_port.py) y el nativo lo arma la CLI.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from vm_port import titan_path, vm_run  # noqa: E402


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


def build_with_test_roots(authority: Path, temp_dir: str, executable: str) -> None:
    """Compila el programa con un compilador cuya copia de native/ tiene un
    std_roots.titan con solo `authority` como ancla (gen_roots.py).

    Es el mismo procedimiento de `tests/pkg/registro/estatico.sh`: el cliente
    TLS confía en las anclas escritas al compilar, no en SSL_CERT_FILE.
    """
    compiler = Path(os.environ.get("COMPILER") or (ROOT / "selfhost" / "titanc3"))
    if not compiler.is_file() or not os.access(compiler, os.X_OK):
        raise SystemExit(f"No encuentro el compilador nativo para la prueba TLS: {compiler}")
    tree = Path(temp_dir) / "tc"
    (tree / "native").mkdir(parents=True, exist_ok=True)
    shutil.copy2(compiler, tree / "titanc")
    shutil.copytree(ROOT / "selfhost" / "native", tree / "native", dirs_exist_ok=True)
    roots = subprocess.run(
        ["python3", str(ROOT / "selfhost" / "native" / "gen_roots.py"), str(authority)],
        cwd=ROOT,
        capture_output=True,
        timeout=120,
    )
    if roots.returncode != 0 or not roots.stdout:
        print(roots.stderr.decode(errors="replace"), file=sys.stderr)
        raise SystemExit("gen_roots.py no pudo generar std_roots.titan para la CA de prueba")
    (tree / "native" / "std_roots.titan").write_bytes(roots.stdout)
    built = run([str(tree / "titanc"), str(PROGRAM.relative_to(ROOT)), executable], timeout=900)
    if built.returncode != 0 or not Path(executable).is_file():
        fail("compilación del cliente TLS con la CA de prueba en std_roots.titan", built)


def verify_real_tls(executable: str, temp_dir: str) -> None:
    # Cadena de verdad: una autoridad (CA:TRUE, keyCertSign) y un certificado
    # de servidor firmado por ella (CA:FALSE, SAN localhost, serverAuth). Con
    # `openssl req -x509` a secas el certificado sale CA:TRUE y, usado como
    # certificado de servidor, es un error X.509 real (webpki: CaUsedAsEndEntity)
    # con el que no se puede probar nada.
    certificate = Path(temp_dir) / "tls-local-cert.pem"
    private_key = Path(temp_dir) / "tls-local-key.pem"
    authority = Path(temp_dir) / "tls-local-ca.pem"
    authority_key = Path(temp_dir) / "tls-local-ca-key.pem"
    request = Path(temp_dir) / "tls-local.csr"
    extensions = Path(temp_dir) / "tls-local-ext.cnf"
    extensions.write_text(
        "subjectAltName=DNS:localhost\n"
        "basicConstraints=critical,CA:FALSE\n"
        "keyUsage=critical,digitalSignature,keyEncipherment\n"
        "extendedKeyUsage=serverAuth\n"
    )
    generated = run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            str(authority_key),
            "-out",
            str(authority),
            "-days",
            "1",
            "-subj",
            "/CN=titan local test CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ],
        timeout=30,
    )
    if generated.returncode == 0:
        generated = run(
            [
                "openssl",
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                str(private_key),
                "-out",
                str(request),
                "-subj",
                "/CN=localhost",
            ],
            timeout=30,
        )
    if generated.returncode == 0:
        generated = run(
            [
                "openssl",
                "x509",
                "-req",
                "-in",
                str(request),
                "-CA",
                str(authority),
                "-CAkey",
                str(authority_key),
                "-CAcreateserial",
                "-out",
                str(certificate),
                "-days",
                "1",
                "-extfile",
                str(extensions),
            ],
            timeout=30,
        )
    if generated.returncode != 0:
        fail("creación de la cadena local (CA y certificado de servidor) para probar TLS", generated)

    # El cliente confía en las anclas de std_roots.titan, escritas al compilar
    # (no lee SSL_CERT_FILE). Para probar el camino bueno hay que armar un
    # cliente cuyo std_roots.titan solo tenga esta CA: el mismo truco de
    # tests/pkg/registro/estatico.sh (copia del compilador + gen_roots.py).
    trusted_executable = str(Path(temp_dir) / "http_full_test_tls")
    build_with_test_roots(authority, temp_dir, trusted_executable)

    TLSHandler.requests = 0
    server = VerifiedTLSServer(("127.0.0.1", 0), certificate, private_key)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    url = f"https://localhost:{port}/redirect"
    expected = json_bytes({"ok": True, "via": "verified TLS"}) + b"\n"

    try:
        # El cliente normal (raíces públicas de Mozilla) no confía en esta CA:
        # tiene que rechazarla sin llegar a mandar nada por un canal sin validar.
        untrusted = run([executable, url, "--tls"], timeout=30)
        if untrusted.returncode == 0 or TLSHandler.requests != 0:
            raise AssertionError("el cliente aceptó una CA desconocida o envió HTTP sin validarla")

        good = run([trusted_executable, url, "--tls"], timeout=30)
        if good.returncode != 0:
            fail("HTTPS nativo con certificado confiable", good)
        if good.stdout != expected:
            print("FALLO: respuesta JSON distinta tras HTTPS, redirección y gzip.", file=sys.stderr)
            print(good.stdout.decode(errors="replace"), file=sys.stderr)
            raise SystemExit(1)
        if TLSHandler.requests != 2:
            raise AssertionError(f"la redirección TLS generó {TLSHandler.requests} solicitudes; se esperaban 2")

        std_http_url = f"https://localhost:{port}/plain-redirect"
        std_http = run([trusted_executable, std_http_url, "--tls-std"], timeout=30)
        std_http_expected = f"200\nhttps://localhost:{port}/plain-chunked\nstd-http-tls-ok\n".encode()
        if std_http.returncode != 0 or std_http.stdout != std_http_expected:
            fail("HTTPS nativo de std::http con redirección y respuesta chunked", std_http)
        if TLSHandler.requests != 4:
            raise AssertionError(f"la prueba de std::http generó {TLSHandler.requests} solicitudes; se esperaban 4")
        print("std::http también pasó HTTPS con redirección y respuesta chunked.")

        before = TLSHandler.requests
        ip_url = f"https://127.0.0.1:{port}/gzip"
        wrong_name = run([trusted_executable, ip_url, "--tls"], timeout=30)
        if wrong_name.returncode == 0 or TLSHandler.requests != before:
            raise AssertionError("el cliente aceptó un certificado para otro nombre de servidor")

        slow_url = f"https://localhost:{port}/slow"
        started = time.monotonic()
        slow = run([trusted_executable, slow_url, "--tls-timeout"], timeout=5)
        elapsed = time.monotonic() - started
        detail = slow.stderr.decode(errors="replace")
        # El servidor tarda 0.5 s y el plazo del cliente es de 100 ms: tiene que
        # cortar antes, no esperar la respuesta. El texto del error es lo único
        # que cambia respecto de la referencia de Rust (que decía "timed out"):
        # el port informa el errno, igual que el camino HTTP simple
        # (`Resource temporarily unavailable (os error 11)`), consistente con el
        # resto de sus errores de E/S. Diferencia anotada en ESTADO.md.
        if slow.returncode == 0 or elapsed >= 0.45:
            raise AssertionError(
                f"el plazo TLS no se aplicó (código {slow.returncode}, {elapsed:.2f} s): {detail}"
            )
        if "timed out" in detail:
            print("El plazo TLS cortó con el mensaje de la referencia ('timed out').")
        elif "Resource temporarily unavailable (os error 11)" in detail:
            print("El plazo TLS cortó a tiempo; el port lo informa como errno EAGAIN, no como 'timed out' (diferencia anotada).")
        else:
            raise AssertionError(f"el plazo TLS falló por otro motivo: {detail}")

        print("HTTPS nativo: certificado y nombre verificados; también pasaron redirección HTTPS y cuerpo gzip.")
        print("Seguridad comprobada: rechazó una CA desconocida y un nombre incorrecto; el plazo también se respeta.")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def main() -> None:
    titan = titan_path()
    if not Path(titan).is_file() or not os.access(titan, os.X_OK):
        raise SystemExit(
            f"No encuentro la CLI del port ({titan}); compílala con "
            "`env -i PATH=/nonexistent ./selfhost/titanc3 selfhost/titan.titan selfhost/titan` "
            "o pásala en TITAN=/ruta/a/titan."
        )

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
                print("Compilador de la prueba: la CLI del port (`titan compile`).")
                build_command = [titan, "compile", str(PROGRAM.relative_to(ROOT)), "-o", executable]
            built = run(build_command)
            if built.returncode != 0 or not Path(executable).is_file():
                fail("compilación con el compilador Titan", built)

            vm = vm_run(
                __file__,
                "prueba_http_full.titan",
                base,
                cwd=str(ROOT),
                capture_output=True,
                timeout=600,
                check=False,
            )
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
