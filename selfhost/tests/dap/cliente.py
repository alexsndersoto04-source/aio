"""Cliente mínimo de DAP por stdio para las pruebas: habla con `titan dap` mensaje a mensaje."""
import json, queue, subprocess, threading

def frame(msg):
    body = json.dumps(msg, separators=(",", ":"), ensure_ascii=False).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body

def parse_frames(data):
    out = []
    i = 0
    while i < len(data):
        j = data.index(b"\r\n\r\n", i)
        n = int(data[i:j].decode().split(":")[1])
        out.append(json.loads(data[j + 4:j + 4 + n]))
        i = j + 4 + n
    return out

class Dap:
    def __init__(self, cli, cwd=None):
        self.p = subprocess.Popen([cli, "dap"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, cwd=cwd)
        self.q = queue.Queue()
        self.seq = 0
        self.log = []          # todo lo recibido, en orden
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        out = self.p.stdout
        while True:
            head = b""
            while not head.endswith(b"\r\n\r\n"):
                c = out.read(1)
                if not c:
                    self.q.put(None)
                    return
                head += c
            n = int(head.decode().split(":")[1])
            self.q.put(json.loads(out.read(n)))

    def send(self, command, **arguments):
        self.seq += 1
        self.p.stdin.write(frame({"seq": self.seq, "type": "request", "command": command, "arguments": arguments}))
        self.p.stdin.flush()
        return self.seq

    def send_raw(self, data):
        self.p.stdin.write(data)
        self.p.stdin.flush()

    def read(self, timeout=30):
        try:
            m = self.q.get(timeout=timeout)
        except queue.Empty:
            raise TimeoutError("sin mensaje DAP en %ss; recibido: %s" % (timeout, self.log[-5:]))
        if m is not None:
            self.log.append(m)
        return m

    def request(self, command, timeout=30, **arguments):
        """Envía y devuelve (respuesta, eventos que llegaron antes que ella)."""
        s = self.send(command, **arguments)
        events = []
        while True:
            m = self.read(timeout)
            if m is None:
                raise EOFError("el servidor cerró la salida; recibido: %s" % self.log[-5:])
            if m["type"] == "response" and m["request_seq"] == s:
                return m, events
            events.append(m)

    def event(self, name, timeout=30):
        """Espera un evento por nombre; devuelve (evento, otros eventos vistos antes)."""
        seen = []
        while True:
            m = self.read(timeout)
            if m is None:
                raise EOFError("el servidor cerró la salida; esperaba '%s'; recibido: %s" % (name, self.log[-5:]))
            if m["type"] == "event" and m["event"] == name:
                return m, seen
            seen.append(m)

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        try:
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()
        return self.p.returncode, self.p.stderr.read().decode()
