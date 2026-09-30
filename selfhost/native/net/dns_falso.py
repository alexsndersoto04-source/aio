# Servidor DNS falso (UDP y TCP) para las pruebas de std::net: respuestas
# normales y también raras (truncadas, con otro ID, SERVFAIL, REFUSED,
# CNAME, mayúsculas cambiadas, pregunta distinta, paquetes cortos...).
# Uso: sudo python3 dns_falso.py DIRECCION [PUERTO]
import socket, struct, sys, threading, time

HOST = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.53"
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 53
hits = {}

def enc_name(n):
    out = b""
    for lab in n.rstrip(".").split("."):
        if lab:
            out += bytes([len(lab)]) + lab.encode()
    return out + b"\0"

def parse_q(pkt):
    i = 12
    labs = []
    while pkt[i] != 0:
        l = pkt[i]
        labs.append(pkt[i + 1:i + 1 + l].decode("latin1"))
        i += 1 + l
    i += 1
    qt, qc = struct.unpack(">HH", pkt[i:i + 4])
    return ".".join(labs), qt, qc, pkt[12:i + 4]

def rr(name, t, data, ttl=60):
    return enc_name(name) + struct.pack(">HHIH", t, 1, ttl, len(data)) + data

def a(ip):
    return socket.inet_pton(socket.AF_INET, ip)

def aaaa(ip):
    return socket.inet_pton(socket.AF_INET6, ip)

def build(pid, flags, question, answers, rcode=0, qd=1, ar=b"", arcount=0):
    h = struct.pack(">HHHHHH", pid, flags | rcode, qd, len(answers), 0, arcount)
    return h + question + b"".join(answers) + ar

def answer(pkt, tcp):
    pid = struct.unpack(">H", pkt[:2])[0]
    name, qt, qc, question = parse_q(pkt)
    low = name.lower()
    key = (low, qt, tcp)
    hits[key] = hits.get(key, 0) + 1
    base = 0x8180  # QR RD RA
    A, AAAA, CNAME = 1, 28, 5
    def ok(recs):
        return [build(pid, base, question, recs)]
    if low in ("a1.test", "a1.lista.test", "a1.otra.test"):
        return ok([rr(name, A, a("127.0.0.2"))] if qt == A else [])
    if low == "multi.test":
        if qt == A:
            return ok([rr(name, A, a("127.0.0.3")), rr(name, A, a("127.0.0.2"))])
        return ok([rr(name, AAAA, aaaa("::1"))])
    if low == "v6.test":
        return ok([rr(name, AAAA, aaaa("::1"))] if qt == AAAA else [])
    if low == "cname.test":
        recs = [rr(name, CNAME, enc_name("a1.test"))]
        if qt == A:
            recs.append(rr("a1.test", A, a("127.0.0.2")))
        return ok(recs)
    if low == "cnamesolo.test":
        return ok([rr(name, CNAME, enc_name("otro.test"))])
    if low == "cnamemal.test":
        recs = [rr(name, CNAME, enc_name("a1.test"))]
        if qt == A:
            recs.append(rr("zzz.test", A, a("127.0.0.3")))
            recs.append(rr("A1.TEST", A, a("127.0.0.2")))
        return ok(recs)
    if low == "nx.test":
        return [build(pid, base, question, [], rcode=3)]
    if low == "servfail.test":
        return [build(pid, base, question, [], rcode=2)]
    if low == "refused.test":
        return [build(pid, base, question, [], rcode=5)]
    if low == "formerr.test":
        return [build(pid, base, question, [], rcode=1)]
    if low == "medio.test":
        # A bien, AAAA con SERVFAIL
        if qt == A:
            return ok([rr(name, A, a("127.0.0.3"))])
        return [build(pid, base, question, [], rcode=2)]
    if low == "vacio.test":
        # NOERROR sin respuestas y sin RA/AA: glibc pasa al siguiente servidor
        return [build(pid, 0x8100, question, [])]
    if low == "tc.test":
        if not tcp:
            return [build(pid, base | 0x0200, question, [])]
        return ok([rr(name, A, a("127.0.0.3"))] if qt == A else [])
    if low == "grande.test":
        recs = [rr(name, A, a("127.0.0.%d" % (2 + (i % 2)))) for i in range(120)] if qt == A else []
        if not tcp:
            return [build(pid, base | 0x0200, question, recs[:10])]
        return ok(recs)
    if low == "otroid.test":
        bad = build(pid ^ 0x5555, base, question, [rr(name, A, a("127.0.0.3"))] if qt == A else [])
        good = build(pid, base, question, [rr(name, A, a("127.0.0.2"))] if qt == A else [])
        return [bad, good]
    if low == "mayus.test":
        up = enc_name(name.upper())
        q2 = up + question[len(up):]
        recs = [rr(name.upper(), A, a("127.0.0.2"))] if qt == A else []
        return [build(pid, base, q2, recs)]
    if low == "otrapregunta.test":
        q2 = enc_name("distinta.test") + question[-4:]
        good = build(pid, base, question, [rr(name, A, a("127.0.0.3"))] if qt == A else [])
        return [build(pid, base, q2, [rr("distinta.test", A, a("127.0.0.2"))]), good]
    if low == "corto.test":
        return [pkt[:2] + b"\x81\x80\x00\x01\x00\x00\x00\x00"[:8]]
    if low == "sinaaaa.test":
        # no contesta a AAAA: hay que esperar (timeout) y reintentar
        if qt == AAAA and not tcp:
            return []
        return ok([rr(name, A, a("127.0.0.2"))] if qt == A else [])
    if low == "comprimido.test":
        # respuesta con punteros de compresión al nombre de la pregunta
        if qt == A:
            recs = [b"\xc0\x0c" + struct.pack(">HHIH", A, 1, 60, 4) + a("127.0.0.3")]
            return ok(recs)
        return ok([])
    if low == "bucle.test":
        if qt == A:
            recs = [b"\xc0\x0c" + struct.pack(">HHIH", CNAME, 1, 60, 2) + b"\xc0\x2d"]
            return ok(recs)
        return ok([])
    if low == "raro.test":
        # nombre con caracteres no válidos en el CNAME: no sirve de nombre canónico
        recs = [rr(name, CNAME, b"\x03a_b\x04test\x00")]
        if qt == A:
            recs.append(rr("a_b.test", A, a("127.0.0.2")))
        return ok(recs)
    if low == "lento.test":
        time.sleep(3)
        return ok([rr(name, A, a("127.0.0.2"))] if qt == A else [])
    if low in ("x.buscar", "solo.buscar"):
        return ok([rr(name, A, a("127.0.0.3"))] if qt == A else [])
    if low == "solo":
        return ok([rr(name, A, a("127.0.0.2"))] if qt == A else [])
    return [build(pid, base, question, [], rcode=3)]

def udp():
    fam = socket.AF_INET6 if ":" in HOST else socket.AF_INET
    s = socket.socket(fam, socket.SOCK_DGRAM)
    s.bind((HOST, PORT))
    while True:
        pkt, addr = s.recvfrom(65536)
        def reply(pkt=pkt, addr=addr):
            try:
                for r in answer(pkt, False):
                    s.sendto(r, addr)
            except Exception as e:
                print("error udp", e, file=sys.stderr)
        threading.Thread(target=reply, daemon=True).start()

def tcp_conn(c):
    try:
        while True:
            h = c.recv(2)
            if len(h) < 2:
                return
            n = struct.unpack(">H", h)[0]
            pkt = b""
            while len(pkt) < n:
                d = c.recv(n - len(pkt))
                if not d:
                    return
                pkt += d
            for r in answer(pkt, True):
                c.sendall(struct.pack(">H", len(r)) + r)
    finally:
        c.close()

def tcp():
    fam = socket.AF_INET6 if ":" in HOST else socket.AF_INET
    s = socket.socket(fam, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind((HOST, PORT))
    s.listen(20)
    while True:
        c, _ = s.accept()
        threading.Thread(target=tcp_conn, args=(c,), daemon=True).start()

threading.Thread(target=udp, daemon=True).start()
threading.Thread(target=tcp, daemon=True).start()
threading.Event().wait()
