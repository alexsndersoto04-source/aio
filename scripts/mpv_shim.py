#!/usr/bin/env python3
"""Shim de mpv para el sandbox (sin dispositivo de audio).

Implementa el subconjunto del protocolo JSON IPC de mpv que usa
`std::audio` de TITAN (crates/titan_stdlib/src/audio_player.rs):

  comandos:  get_property, set_property, seek, loadfile (append-play),
             playlist-clear, playlist-next, playlist-prev, quit
  props:     time-pos, playlist-index, playlist-count, media-title,
             filename, duration, volume, pause

NO es mpv real: la reproducción es silenciosa (no hay audio en el
sandbox) pero el reloj de reproducción avanza en tiempo real, de modo
que play/posicion/pausa/seek/cola/next/prev/current/stop se ejercitan
de verdad contra el cliente IPC de TITAN.

Instalado como /usr/local/bin/mpv en el sandbox de Arena.
"""
import json
import os
import socket
import sys
import threading
import time

LOG_PATH = "/tmp/mpv-shim-debug.log"
_log_lock = threading.Lock()


def _dbg(msg):
    try:
        with _log_lock:
            with open(LOG_PATH, "a") as lf:
                lf.write(msg + "\n")
    except Exception:
        pass


def parse_args(argv):
    sock = None
    files = []
    after_dd = False
    for a in argv:
        if a == "--":
            after_dd = True
        elif after_dd:
            files.append(a)
        elif a.startswith("--input-ipc-server="):
            sock = a.split("=", 1)[1]
    return sock, files


def wav_duration(path):
    """Duración real de un WAV mono/estéreo PCM leyendo la cabecera RIFF."""
    try:
        with open(path, "rb") as f:
            head = f.read(256)
        if head[:4] != b"RIFF" or head[8:12] != b"WAVE":
            return None
        byte_rate = None
        data_size = None
        i = 12
        n = len(head)
        while i + 8 <= n:
            cid = head[i:i + 4]
            csize = int.from_bytes(head[i + 4:i + 8], "little")
            if cid == b"fmt " and i + 24 <= n:
                byte_rate = int.from_bytes(head[i + 16:i + 20], "little")
            elif cid == b"data":
                data_size = csize
                break
            i += 8 + csize
        if byte_rate and data_size is not None:
            return data_size / byte_rate
    except Exception:
        pass
    return None


def main():
    sock_path, files = parse_args(sys.argv[1:])
    if not sock_path:
        return

    st = {
        "playlist": list(files),
        "index": 0,
        "volume": 100,
        "paused": False,
        "quit": False,
    }
    pos = {"t": 0.0, "resumed_at": time.monotonic()}

    _dbg(f"START sock={sock_path} files={files} pid={os.getpid()}")
    try:
        os.unlink(sock_path)
    except FileNotFoundError:
        pass

    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(sock_path)
    srv.listen(4)
    srv.settimeout(0.5)

    def cur_pos():
        if st["paused"]:
            return pos["t"]
        pos["t"] += time.monotonic() - pos["resumed_at"]
        pos["resumed_at"] = time.monotonic()
        return pos["t"]

    def current_path():
        if 0 <= st["index"] < len(st["playlist"]):
            return st["playlist"][st["index"]]
        return None

    def handle(client):
        try:
            f = client.makefile("rwb")
            # evento de arranque (el cliente de TITAN ignora líneas sin "error")
            p = current_path()
            if p:
                f.write((json.dumps({"event": "file-loaded", "filename": p}, separators=(",", ":")) + "\n").encode())
                f.flush()
            buf = b""
            while not st["quit"]:
                byte = f.read(1)
                if not byte:
                    break
                if byte != b"\n":
                    buf += byte
                    continue
                raw = bytes(buf)
                line = buf.decode("utf-8", "replace").strip()
                _dbg(f"RAWLINE pid={os.getpid()} tid={threading.get_ident()} " + repr(raw))
                buf = b""
                if not line:
                    continue
                try:
                    req = json.loads(line)
                except Exception:
                    # mpv real responde {"error":"Invalid JSON"} al JSON inválido
                    _dbg("INVALID-JSON " + repr(line))
                    f.write((json.dumps({"error": "Invalid JSON"},
                                        separators=(",", ":")) + "\n").encode())
                    f.flush()
                    continue
                cmd = req.get("command") or []
                if not cmd:
                    continue
                op = cmd[0]
                _dbg("RECV " + line)
                data = None
                if op == "get_property" and len(cmd) > 1:
                    prop = cmd[1]
                    p = current_path()
                    if prop == "time-pos":
                        data = round(cur_pos(), 3)
                    elif prop == "playlist-index":
                        data = st["index"] if st["playlist"] else None
                    elif prop == "playlist-count":
                        data = len(st["playlist"])
                    elif prop == "media-title":
                        data = os.path.basename(p) if p else None
                    elif prop == "filename":
                        data = p
                    elif prop == "duration":
                        d = wav_duration(p) if p else None
                        data = round(d, 3) if d else None
                    elif prop == "volume":
                        data = st["volume"]
                    elif prop == "pause":
                        data = st["paused"]
                elif op == "set_property" and len(cmd) > 2:
                    prop, val = cmd[1], cmd[2]
                    if prop == "pause":
                        if val and not st["paused"]:
                            pos["t"] = cur_pos()
                            st["paused"] = True
                            pos["resumed_at"] = time.monotonic()
                        elif not val and st["paused"]:
                            st["paused"] = False
                            pos["resumed_at"] = time.monotonic()
                    elif prop == "volume":
                        try:
                            st["volume"] = int(val)
                        except Exception:
                            pass
                elif op == "seek" and len(cmd) > 1:
                    try:
                        pos["t"] = float(cmd[1])
                        pos["resumed_at"] = time.monotonic()
                    except Exception:
                        pass
                elif op == "loadfile" and len(cmd) > 1:
                    # "append-play": añade y salta a reproducirla
                    st["playlist"].append(cmd[1])
                    st["index"] = len(st["playlist"]) - 1
                    pos["t"] = 0.0
                    pos["resumed_at"] = time.monotonic()
                elif op == "playlist-clear":
                    st["playlist"] = []
                    st["index"] = 0
                elif op == "playlist-next":
                    if st["index"] + 1 < len(st["playlist"]):
                        st["index"] += 1
                        pos["t"] = 0.0
                        pos["resumed_at"] = time.monotonic()
                elif op == "playlist-prev":
                    if st["index"] - 1 >= 0:
                        st["index"] -= 1
                        pos["t"] = 0.0
                        pos["resumed_at"] = time.monotonic()
                elif op == "quit":
                    f.write((json.dumps({"error": "success"}, separators=(",", ":")) + "\n").encode())
                    f.flush()
                    st["quit"] = True
                    break
                f.write((json.dumps({"error": "success", "data": data}, separators=(",", ":")) + "\n").encode())
                f.flush()
        except Exception as e:
            _dbg(f"EXC {type(e).__name__}: {e}")
            raise
        finally:
            try:
                client.close()
            except Exception:
                pass

    try:
        while not st["quit"]:
            try:
                client, _ = srv.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            threading.Thread(target=handle, args=(client,), daemon=True).start()
    finally:
        try:
            os.unlink(sock_path)
        except Exception:
            pass


if __name__ == "__main__":
    main()
