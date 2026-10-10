#!/usr/bin/env python3
"""Cliente de la prueba diferencial de std::server (ver run.sh).

Lanza un guion fijo de peticiones HTTP y sesiones WebSocket contra el servidor
de server.titan (puerto 19890) e imprime lo que recibe, byte a byte. run.sh lo
ejecuta contra la VM de Rust y contra el binario nativo y compara las dos
salidas (la de este cliente y la del servidor)."""
import base64
import hashlib
import socket
import struct
import sys
import time

PORT = 19890
MAGIC = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
KEY = base64.b64encode(b"0123456789abcdef").decode()


def wait_ready(path):
    for _ in range(600):
        try:
            if "READY" in open(path).read():
                return
        except OSError:
            pass
        time.sleep(0.1)
    print("el servidor no llegó a READY", file=sys.stderr)
    sys.exit(2)


def conn():
    s = socket.create_connection(("127.0.0.1", PORT), timeout=10)
    return s


def read_all(s, idle=3.0):
    """Lee hasta EOF o hasta `idle` segundos sin datos."""
    s.settimeout(idle)
    out = b""
    while True:
        try:
            d = s.recv(65536)
        except socket.timeout:
            return out + b"<IDLE>"
        except ConnectionResetError:
            return out + b"<RESET>"
        if not d:
            return out
        out += d


def show(label, data):
    if len(data) > 400:
        data = data[:300] + b"...(%d bytes)" % len(data)
    print(label, repr(data), flush=True)


def exchange(label, raw, idle=3.0):
    s = conn()
    s.sendall(raw)
    data = read_all(s, idle)
    s.close()
    show(label, data)
    return data


def head(method, target, headers, version="HTTP/1.1"):
    lines = [f"{method} {target} {version}"] + headers
    return ("\r\n".join(lines) + "\r\n\r\n").encode("latin-1")


H = ["Host: t"]


def http_basic():
    exchange("info", head("GET", "/info?a=1&b=%20x", ["Host: Example.com", "X-A: one", "x-a:  two ", "Accept:\ttab", "User-Agent: t"]))
    exchange("echo", head("POST", "/echo", H + ["Content-Length: 5"]) + b"hello")
    exchange("echo-empty-cl0", head("POST", "/echo", H + ["Content-Length: 0"]))
    exchange("echo-cl-zeros", head("POST", "/echo", H + ["Content-Length: 00005"]) + b"hello")
    exchange("echo-nonbody", head("GET", "/echo", H))
    exchange("bin", head("POST", "/bin", H + ["Content-Length: 4"]) + b"\x00\xff\x80\x01")
    exchange("nonutf8", head("POST", "/echo", H + ["Content-Length: 2"]) + b"\xc3\x28")
    # Expect: 100-continue
    s = conn()
    s.sendall(head("POST", "/echo", H + ["Content-Length: 5", "Expect: 100-continue"]))
    s.settimeout(5)
    first = s.recv(1024)
    s.sendall(b"hello")
    rest = read_all(s)
    s.close()
    show("expect-100", first + b"|" + rest)
    exchange("expect-nobody", head("GET", "/head", H + ["Expect: 100-continue"]))
    exchange("http10", head("GET", "/head", [], "HTTP/1.0"))
    exchange("head-method", head("HEAD", "/head", H))
    exchange("s204", head("GET", "/s204", H))
    exchange("s304", head("GET", "/s304", H))
    exchange("noct", head("GET", "/noct", H))
    exchange("twice", head("GET", "/twice", H))
    exchange("unknown-path", head("GET", "/zzz", H))
    exchange("post-large", head("POST", "/bin", H + ["Content-Length: 300000"]) + b"A" * 300000)


def chunked():
    th = head("POST", "/echo", H + ["Transfer-Encoding: chunked"])
    exchange("chunk-ok", th + b"5;ext=1\r\nhello\r\n6\r\n world\r\n0\r\nTrailer: v\r\n\r\n")
    exchange("chunk-hex-upper", th + b"A\r\n0123456789\r\n0\r\n\r\n")
    exchange("chunk-spaces", th + b" 3 \r\nabc\r\n0\r\n\r\n")
    exchange("chunk-te-case", head("POST", "/echo", H + ["transfer-encoding: CHUNKED"]) + b"1\r\na\r\n0\r\n\r\n")
    exchange("chunk-bad-len", th + b"zz\r\nab\r\n0\r\n\r\n")
    exchange("chunk-empty-len", th + b"\r\n0\r\n\r\n")
    exchange("chunk-17-digits", th + b"00000000000000001\r\na\r\n0\r\n\r\n")
    exchange("chunk-16-digits-ok", th + b"0000000000000001\r\na\r\n0\r\n\r\n")
    exchange("chunk-ffff", th + b"ffffffffffffffff\r\n")
    exchange("chunk-too-big", th + b"800001\r\n")
    exchange("chunk-missing-crlf", th + b"1\r\nabc")
    exchange("chunk-sign", th + b"+1\r\na\r\n0\r\n\r\n")
    exchange("chunk-nonascii", th + b"\xc3\xa9\r\n")
    exchange("chunk-invalid-utf8", th + b"\xff\r\n")
    exchange("chunk-nbsp", th + b"\xc2\xa01\xc2\xa0\r\na\r\n0\r\n\r\n")
    exchange("chunk-line-long", th + b"1;" + b"a" * 8200 + b"\r\n")
    exchange("trailer-malformed", th + b"0\r\nBadTrailer\r\n\r\n")
    exchange("trailer-badname", th + b"0\r\nb@d name: x\r\n\r\n")
    exchange("trailer-ctrl", th + b"0\r\nX: a\x01b\r\n\r\n")
    exchange("trailer-33", th + b"0\r\n" + b"X: 1\r\n" * 33 + b"\r\n")
    exchange("trailer-32", th + b"0\r\n" + b"X: 1\r\n" * 32 + b"\r\n")
    exchange("trailer-bytes", th + b"0\r\n" + (b"X: " + b"a" * 6000 + b"\r\n") * 3 + b"\r\n")
    exchange("trailer-line-long", th + b"0\r\nX: " + b"a" * 9000 + b"\r\n\r\n")
    exchange("trailer-invalid-utf8", th + b"0\r\nX: \xff\r\n\r\n")
    # cuerpo cortado: el cliente cierra
    s = conn()
    s.sendall(head("POST", "/echo", H + ["Content-Length: 10"]) + b"abc")
    s.shutdown(socket.SHUT_WR)
    show("body-eof", read_all(s))
    s.close()
    s = conn()
    s.sendall(th + b"5\r\nab")
    s.shutdown(socket.SHUT_WR)
    show("chunk-eof", read_all(s))
    s.close()
    # cuerpo que no llega: plazo de 5 s del servidor
    s = conn()
    s.sendall(head("POST", "/echo", H + ["Content-Length: 10"]) + b"abc")
    time.sleep(6)
    show("body-timeout", read_all(s, 1.0))
    s.close()


def invalid_requests():
    cases = [
        ("bad-method", b"G@T / HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("no-target", b"GET\r\n\r\n"),
        ("double-space", b"GET  / HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("no-version", b"GET /\r\n\r\n"),
        ("http2", b"GET / HTTP/2.0\r\nHost: a\r\n\r\n"),
        ("extra-part", b"GET / HTTP/1.1 x\r\nHost: a\r\n\r\n"),
        ("method-33", b"A" * 33 + b" / HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("method-32", b"A" * 32 + b" /head HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("target-long", b"GET /" + b"a" * 16384 + b" HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("target-ctrl", b"GET /a\x01b HTTP/1.1\r\nHost: a\r\n\r\n"),
        ("no-host", b"GET / HTTP/1.1\r\n\r\n"),
        ("two-hosts", b"GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n"),
        ("empty-host", b"GET / HTTP/1.1\r\nHost:\r\n\r\n"),
        ("folded", b"GET / HTTP/1.1\r\nHost: a\r\n x: y\r\n\r\n"),
        ("folded-tab", b"GET / HTTP/1.1\r\nHost: a\r\n\tx: y\r\n\r\n"),
        ("no-colon", b"GET / HTTP/1.1\r\nHost: a\r\nBadHeader\r\n\r\n"),
        ("name-space", b"GET / HTTP/1.1\r\nHost: a\r\nBad Name: x\r\n\r\n"),
        ("empty-name", b"GET / HTTP/1.1\r\nHost: a\r\n: x\r\n\r\n"),
        ("value-ctrl", b"GET / HTTP/1.1\r\nHost: a\r\nX: a\x01b\r\n\r\n"),
        ("value-del", b"GET / HTTP/1.1\r\nHost: a\r\nX: a\x7fb\r\n\r\n"),
        ("non-ascii", "GET / HTTP/1.1\r\nHost: a\r\nX: é\r\n\r\n".encode()),
        ("non-utf8", b"GET / HTTP/1.1\r\nHost: a\r\nX: \xff\r\n\r\n"),
        ("headers-129", b"GET / HTTP/1.1\r\nHost: a\r\n" + b"".join(b"X%d: v\r\n" % i for i in range(128)) + b"\r\n"),
        ("headers-128", b"GET /head HTTP/1.1\r\nHost: a\r\n" + b"".join(b"X%d: v\r\n" % i for i in range(127)) + b"\r\n"),
        ("name-257", b"GET / HTTP/1.1\r\nHost: a\r\n" + b"N" * 257 + b": v\r\n\r\n"),
        ("value-8193", b"GET / HTTP/1.1\r\nHost: a\r\nX: " + b"v" * 8193 + b"\r\n\r\n"),
        ("value-8192", b"GET /head HTTP/1.1\r\nHost: a\r\nX: " + b"v" * 8192 + b"\r\n\r\n"),
        ("head-too-big", b"GET / HTTP/1.1\r\nHost: a\r\n" + b"".join(b"X%d: " % i + b"v" * 8000 + b"\r\n" for i in range(9)) + b"\r\n"),
        ("two-cl", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\nx"),
        ("cl-abc", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: abc\r\n\r\n"),
        ("cl-neg", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: -1\r\n\r\n"),
        ("cl-plus", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: +1\r\n\r\n"),
        ("cl-empty", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length:\r\n\r\n"),
        ("cl-23", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 99999999999999999999999\r\n\r\n"),
        ("cl-u64max+1", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 18446744073709551616\r\n\r\n"),
        ("cl-u64max", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 18446744073709551615\r\n\r\n"),
        ("cl-8M+1", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 8388609\r\n\r\n"),
        ("cl-zeros-big", b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 000000000000000000000008388609\r\n\r\n"),
        ("te-cl", b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\nContent-Length: 1\r\n\r\n"),
        ("te-twice", b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n"),
        ("te-gzip", b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: gzip\r\n\r\n"),
        ("expect-foo", b"POST / HTTP/1.1\r\nHost: a\r\nExpect: foo\r\nContent-Length: 1\r\n\r\nx"),
        ("expect-twice", b"POST / HTTP/1.1\r\nHost: a\r\nExpect: 100-continue\r\nExpect: 100-continue\r\nContent-Length: 1\r\n\r\nx"),
        ("lf-only", b"GET / HTTP/1.1\nHost: a\n\n"),
    ]
    for label, raw in cases:
        exchange("bad:" + label, raw, 2.0)
    # conexión que se cierra sin enviar nada y con cabecera a medias
    s = conn()
    s.close()
    time.sleep(0.2)
    s = conn()
    s.sendall(b"GET /x HTTP/1.1\r\nHost")
    s.shutdown(socket.SHUT_WR)
    show("bad:eof-head", read_all(s, 2.0))
    s.close()
    # cabecera que no termina: plazo de 5 s
    s = conn()
    s.sendall(b"GET /x HT")
    time.sleep(6)
    show("bad:head-timeout", read_all(s, 1.0))
    s.close()
    exchange("after-bad", head("GET", "/head", H))


def response_checks():
    exchange("resp", head("GET", "/resp", H))
    exchange("resp-big", head("GET", "/resp-big", H))


def ws_accept(key):
    return base64.b64encode(hashlib.sha1(key.encode() + MAGIC).digest()).decode()


def ws_head(path="/ws", key=KEY, extra=None, method="GET", version="HTTP/1.1", conn_hdr="Upgrade", ver="13"):
    h = ["Host: t", "Upgrade: websocket", f"Connection: {conn_hdr}", f"Sec-WebSocket-Version: {ver}"]
    if key is not None:
        h.append(f"Sec-WebSocket-Key: {key}")
    h += extra or []
    return head(method, path, h, version)


def frame(op, payload=b"", fin=True, mask=True, rsv=0, form=None):
    b0 = (0x80 if fin else 0) | (rsv << 4) | op
    n = len(payload)
    mk = bytes([1, 2, 3, 4])
    if form is None:
        form = 0 if n < 126 else (2 if n < 65536 else 8)
    m = 0x80 if mask else 0
    if form == 0:
        hdr = bytes([b0, m | n])
    elif form == 2:
        hdr = bytes([b0, m | 126]) + struct.pack(">H", n)
    else:
        hdr = bytes([b0, m | 127]) + struct.pack(">Q", n)
    if mask:
        return hdr + mk + bytes(b ^ mk[i % 4] for i, b in enumerate(payload))
    return hdr + payload


def read_exact(s, n, timeout=5.0):
    s.settimeout(timeout)
    out = b""
    while len(out) < n:
        try:
            d = s.recv(n - len(out))
        except socket.timeout:
            return out
        if not d:
            return out
        out += d
    return out


def read_frame(s, timeout=5.0):
    h = read_exact(s, 2, timeout)
    if len(h) < 2:
        return ("eof", h)
    op, ln = h[0] & 15, h[1] & 127
    if ln == 126:
        ln = struct.unpack(">H", read_exact(s, 2))[0]
    elif ln == 127:
        ln = struct.unpack(">Q", read_exact(s, 8))[0]
    pl = read_exact(s, ln)
    return (op, h[0] >> 7, pl if len(pl) < 40 else pl[:16] + b"...(%d)" % len(pl))


def ws_open(path="/ws", prefetch=b"", **kw):
    s = conn()
    s.sendall(ws_head(path, **kw) + prefetch)
    s.settimeout(5)
    buf = b""
    while b"\r\n\r\n" not in buf:
        d = s.recv(4096)
        if not d:
            break
        buf += d
    return s, buf


def ws_checks():
    # cabeceras de upgrade incorrectas (el servidor responde 400 tras informar)
    bad = [
        ("no-upgrade", head("GET", "/wsfail", ["Host: t", "Connection: Upgrade", "Sec-WebSocket-Version: 13", f"Sec-WebSocket-Key: {KEY}"])),
        ("conn-keepalive", ws_head("/wsfail", conn_hdr="keep-alive")),
        ("conn-xupgrade", ws_head("/wsfail", conn_hdr="xupgrade")),
        ("version-12", ws_head("/wsfail", ver="12")),
        ("dup-upgrade", ws_head("/wsfail", extra=["Upgrade: websocket"])),
        ("dup-conn", ws_head("/wsfail", extra=["Connection: Upgrade"])),
        ("no-key", ws_head("/wsfail", key=None)),
        ("dup-key", ws_head("/wsfail", extra=[f"Sec-WebSocket-Key: {KEY}"])),
        ("key-not-b64", ws_head("/wsfail", key="notbase64!!")),
        ("key-15-bytes", ws_head("/wsfail", key=base64.b64encode(b"0123456789abcde").decode())),
        ("key-129", ws_head("/wsfail", key="A" * 129)),
        ("post", ws_head("/wsfail", method="POST")),
        ("http10", ws_head("/wsfail", version="HTTP/1.0")),
        ("with-body", ws_head("/wsfail", extra=["Content-Length: 3"]) [:-2] + b"\r\nabc"),
        ("upgrade-case", ws_head("/wsfail").replace(b"Upgrade: websocket", b"UPGRADE: WebSocket")),
    ]
    for label, raw in bad:
        exchange("wsbad:" + label, raw, 2.0)

    # sesión completa
    s, resp = ws_open("/ws", conn_hdr="keep-alive,  UPGRADE ")
    show("ws-handshake", resp)
    assert ws_accept(KEY).encode() in resp
    s.sendall(frame(1, b"hi"))
    s.sendall(frame(2, b"\x00\x01\x02"))
    s.sendall(frame(9, b"pp"))
    show("ws-pong", read_frame(s))
    s.sendall(frame(10, b"unsolicited"))
    s.sendall(frame(1, b"he", fin=False) + frame(9, b"mid") + frame(0, b"ll", fin=False) + frame(0, b"o"))
    show("ws-pong2", read_frame(s))
    s.sendall(frame(1, b"x" * 200))
    s.sendall(frame(2, b"y" * 70000))
    s.sendall(frame(1, "ñandú".encode()))
    s.sendall(frame(1, b""))
    s.sendall(frame(1, b"send-all"))
    for i in range(5):
        show("ws-recv%d" % i, read_frame(s, 3.0))
    s.sendall(frame(1, b"close-now"))
    show("ws-close-frame", read_frame(s))
    show("ws-after-close", read_all(s, 1.5))
    s.close()

    # cierre iniciado por el cliente (con código y razón) y eco
    s, resp = ws_open("/ws")
    s.sendall(frame(8, struct.pack(">H", 1000) + b"done"))
    show("ws-echo-close", read_frame(s))
    show("ws-after-echo", read_all(s, 1.5))
    s.close()
    s, resp = ws_open("/ws")
    s.sendall(frame(8, b""))
    show("ws-echo-close-empty", read_frame(s))
    s.close()

    # upgrade con datos ya llegados (prefetched) y marco partido en dos envíos
    s, resp = ws_open("/ws", prefetch=frame(1, b"first") + frame(1, b"sec")[:3])
    time.sleep(0.4)
    s.sendall(frame(1, b"sec")[3:])
    time.sleep(0.2)
    s.sendall(frame(8, b""))
    show("ws-prefetch-echo", read_frame(s))
    s.close()

    # errores de protocolo (una sesión por caso)
    cases = [
        ("unmasked", frame(1, b"hi", mask=False)),
        ("reserved", frame(1, b"hi", rsv=4)),
        ("opcode3", frame(3, b"hi")),
        ("opcode11", frame(11, b"")),
        ("ping-fragmented", frame(9, b"x", fin=False)),
        ("ping-126", frame(9, b"x" * 126)),
        ("close-1byte", frame(8, b"x")),
        ("close-1005", frame(8, struct.pack(">H", 1005))),
        ("close-1004", frame(8, struct.pack(">H", 1004))),
        ("close-2999", frame(8, struct.pack(">H", 2999))),
        ("close-3000", frame(8, struct.pack(">H", 3000) + b"ok")),
        ("close-badutf8", frame(8, struct.pack(">H", 1000) + b"\xff")),
        ("cont-orphan", frame(0, b"x")),
        ("interleaved", frame(1, b"a", fin=False) + frame(1, b"b")),
        ("text-badutf8", frame(1, b"\xff")),
        ("text-badutf8-frag", frame(1, b"\xc3", fin=False) + frame(0, b"\x28")),
        ("text-split-utf8-ok", frame(1, b"\xc3", fin=False) + frame(0, b"\xb1")),
        ("nonminimal-126", frame(1, b"x" * 10, form=2)),
        ("nonminimal-127-small", frame(1, b"x" * 10, form=8)),
        ("nonminimal-127-65535", frame(1, b"x" * 65535, form=8)),
        ("len-top-bit", bytes([0x81, 0xFF]) + struct.pack(">Q", 1 << 63) + b"\x00\x00\x00\x00"),
    ]
    for label, raw in cases:
        s, resp = ws_open("/ws")
        s.sendall(raw)
        s.settimeout(3)
        show("wsproto:" + label, read_all(s, 1.5))
        s.close()
    # límites de mensaje (máximo 100 bytes)
    for label, raw in [
        ("small-ok", frame(1, b"x" * 100)),
        ("small-101", frame(1, b"x" * 101)),
        ("small-frag-101", frame(1, b"x" * 60, fin=False) + frame(0, b"x" * 41)),
        ("small-frag-100", frame(1, b"x" * 60, fin=False) + frame(0, b"x" * 40)),
    ]:
        s, resp = ws_open("/ws-small")
        s.sendall(raw)
        show("wslimit:" + label, read_all(s, 1.5))
        s.close()
    # el cliente se va sin cerrar
    s, resp = ws_open("/ws")
    s.sendall(frame(1, b"bye"))
    time.sleep(0.2)
    s.close()
    time.sleep(0.5)
    # silencio: evento timeout y luego un mensaje
    s, resp = ws_open("/ws")
    time.sleep(5.6)
    s.sendall(frame(1, b"late"))
    time.sleep(0.2)
    s.sendall(frame(8, b""))
    show("ws-timeout-echo", read_frame(s, 3.0))
    s.close()


def limits():
    # 256 peticiones pendientes: la 257.ª no se acepta hasta que se libera una
    socks = []
    s = conn()
    s.sendall(head("GET", "/hold256", H))
    socks.append(s)
    for i in range(255):
        s = conn()
        s.sendall(head("GET", "/h%d" % i, H))
        socks.append(s)
    extra = conn()
    extra.sendall(head("GET", "/head", H))
    ok = 0
    others = {}
    for s in socks:
        d = read_all(s, 10.0)
        k = d.split(b"\r\n", 1)[0]
        others[k] = others.get(k, 0) + 1
        s.close()
    print("hold256 respuestas:", sorted(others.items()), flush=True)
    show("hold256-extra", read_all(extra, 5.0))
    extra.close()
    # 64 WebSockets vivos
    live = []
    for i in range(64):
        s, resp = ws_open("/ws-hold" if i == 0 else "/ws-more")
        live.append((s, resp.split(b"\r\n", 1)[0]))
    s, resp = ws_open("/ws-more")
    show("ws65", resp)
    states = {}
    for s, first in live:
        st = states.setdefault(first, 0)
        states[first] = st + 1
        s.close()
    print("ws64 respuestas:", sorted(states.items()), flush=True)


def main():
    wait_ready(sys.argv[1])
    http_basic()
    chunked()
    invalid_requests()
    response_checks()
    ws_checks()
    limits()
    exchange("quit", head("GET", "/quit", H))


main()
