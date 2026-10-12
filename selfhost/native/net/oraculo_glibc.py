#!/usr/bin/env python3
"""Oráculo de `native/net/comparar.sh`: resuelve y conecta con el glibc REAL.

El arnés comparaba la VM de Rust (`zett run prueba.titan urls.txt`) con el
binario nativo. La VM de Rust no tenía nada propio aquí: `std::net` de Rust
llamaba a `getaddrinfo`, `socket` y `connect` de glibc. Desde que no hay Rust,
este oráculo hace esas mismas llamadas de verdad —`getaddrinfo` por ctypes,
sockets y `connect` por el módulo `socket`— y reproduce la salida del programa
de prueba byte a byte: por cada URL, `url -> estado cuerpo` o `url ERR mensaje`.

Uso: python3 oraculo_glibc.py urls.txt

Notas:
- Los literales IPv4/IPv6 se prueban primero (como `Ipv4Addr::from_str`,
  `Ipv6Addr::from_str` de Rust y el propio port), y solo si no lo son se llama
  a getaddrinfo con las mismas pistas que usa Rust (`SOCK_STREAM`,
  `IPPROTO_TCP`, familia sin especificar, sin AI_ADDRCONFIG; con
  ORACULO_HINTS=addrconfig se añade ese flag, para experimentar).
- Los mensajes de error son los mismos que da `std::net` en el port, que a su
  vez son los de `io::Error` de Rust: `gai_strerror` para getaddrinfo y
  `strerror` con el formato "Texto (os error N)" para el resto. Van precedidos
  por `native function 'std::net::http_get' failed: `, que es lo que devuelve
  `std::try::catch` en las dos máquinas (el programa de prueba lo imprime tal
  cual) — es parte de la salida que se compara, no adorno.
"""

import ctypes
import os
import socket
import sys

libc = ctypes.CDLL("libc.so.6", use_errno=True)
libc.gai_strerror.restype = ctypes.c_char_p
libc.gai_strerror.argtypes = [ctypes.c_int]

AF_UNSPEC, AF_INET, AF_INET6 = 0, 2, 10
SOCK_STREAM, IPPROTO_TCP = 1, 6
AI_ADDRCONFIG = 32


class AddrInfo(ctypes.Structure):
    pass


AddrInfo._fields_ = [
    ("ai_flags", ctypes.c_int),
    ("ai_family", ctypes.c_int),
    ("ai_socktype", ctypes.c_int),
    ("ai_protocol", ctypes.c_int),
    ("ai_addrlen", ctypes.c_uint32),
    ("ai_addr", ctypes.c_void_p),
    ("ai_canonname", ctypes.c_char_p),
    ("ai_next", ctypes.POINTER(AddrInfo)),
]


def gai_message(code: int) -> str:
    """cvt_gai de Rust: EAI_SYSTEM usa el errno y el resto gai_strerror."""
    if code == socket.EAI_SYSTEM:
        err = ctypes.get_errno()
        return f"{os.strerror(err)} (os error {err})"
    text = libc.gai_strerror(code).decode()
    return f"failed to lookup address information: {text}"


def sockaddr_bytes(ai) -> bytes:
    return ctypes.string_at(ai.ai_addr, ai.ai_addrlen)


def resolve(host: str, port: int):
    """(direcciones, error) con la semántica de TcpStream::connect de Rust."""
    for fam in (AF_INET, AF_INET6):
        try:
            packed = socket.inet_pton(fam, host)
        except OSError:
            continue
        return [((fam, packed), port)], None
    hints = AddrInfo()
    hints.ai_family = AF_UNSPEC
    hints.ai_socktype = SOCK_STREAM
    hints.ai_protocol = IPPROTO_TCP
    hints.ai_flags = AI_ADDRCONFIG if os.environ.get("ORACULO_HINTS") == "addrconfig" else 0
    res = ctypes.POINTER(AddrInfo)()
    ctypes.set_errno(0)
    code = libc.getaddrinfo(host.encode(), str(port).encode(), ctypes.byref(hints), ctypes.byref(res))
    if code != 0:
        return None, gai_message(code)
    out = []
    node = res
    while node:
        ai = node.contents
        if ai.ai_family in (AF_INET, AF_INET6):
            out.append((sockaddr_bytes(ai), port))
        node = ai.ai_next
    libc.freeaddrinfo(res)
    if not out:
        return None, "could not resolve to any addresses"
    return out, None


def addr_tuple(raw: bytes, port: int):
    fam = int.from_bytes(raw[0:2], "little")
    if fam == AF_INET:
        return (socket.inet_ntop(AF_INET, raw[4:8]), port)
    return (socket.inet_ntop(AF_INET6, raw[8:24]), port, 0, int.from_bytes(raw[24:28], "little"))


def os_error(err: int) -> str:
    return f"{os.strerror(err)} (os error {err})"


PREFIX = "native function 'std::net::http_get' failed: "


def http_get(url: str) -> str:
    if url.startswith("https://"):
        return "ERR " + PREFIX + "HTTPS requires a TLS-enabled client"
    if not url.startswith("http://"):
        return "ERR " + PREFIX + "URL must start with http://"
    rest = url[7:]
    slash = rest.find("/")
    authority = rest if slash < 0 else rest[:slash]
    path = "/" if slash < 0 else rest[slash:]
    colon = authority.rfind(":")
    host = authority if colon < 0 else authority[:colon]
    port = 80
    if colon >= 0:
        digits = authority[colon + 1:]
        if not digits.isdigit() or int(digits) > 65535:
            return "ERR " + PREFIX + "invalid port"
        port = int(digits)
    if host == "":
        return "ERR " + PREFIX + "URL has no host"

    addresses, err = resolve(host, port)
    if err is not None:
        return "ERR " + PREFIX + err
    last = 0
    stream = None
    for raw, _ in addresses:
        fam = int.from_bytes(raw[0:2], "little")
        try:
            candidate = socket.socket(fam, socket.SOCK_STREAM)
        except OSError as exc:
            last = exc.errno or 0
            continue
        try:
            candidate.connect(addr_tuple(raw, port))
        except OSError as exc:
            last = exc.errno or 0
            candidate.close()
            continue
        stream = candidate
        break
    if stream is None:
        return "ERR " + PREFIX + os_error(last)

    request = (
        f"GET {path} HTTP/1.1\r\nHost: {authority}\r\n"
        "User-Agent: Titan/0.2\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    ).encode()
    try:
        stream.sendall(request)
        stream.settimeout(60)
        chunks = []
        while True:
            chunk = stream.recv(65536)
            if not chunk:
                break
            chunks.append(chunk)
    except OSError as exc:
        stream.close()
        return "ERR " + PREFIX + os_error(exc.errno or 11)
    stream.close()
    all_bytes = b"".join(chunks)

    sep = all_bytes.find(b"\r\n\r\n")
    if sep < 0:
        return "ERR " + PREFIX + "malformed HTTP response"
    head = all_bytes[:sep].decode("utf-8", errors="replace")
    body = all_bytes[sep + 4:].decode("utf-8", errors="replace")
    first = head.split("\r\n", 1)[0]
    words = first.split()
    if len(words) < 2 or not words[1].isdigit() or int(words[1]) > 65535:
        return "ERR " + PREFIX + "invalid HTTP status"
    return f"-> {int(words[1])} {body}"


def main() -> None:
    text = open(sys.argv[1], encoding="utf-8").read()
    for line in text.split("\n"):
        if line == "":
            continue
        result = http_get(line)
        if result.startswith("ERR "):
            print(f"{line} {result}")
        else:
            print(f"{line} {result}")


if __name__ == "__main__":
    main()
