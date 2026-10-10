#!/usr/bin/env python3
"""Pruebas de `titan dap` (servidor DAP en Titan). Cada caso habla con el servidor de verdad (se
compila y ejecuta el programa de depuración) y comprueba las respuestas y eventos contra lo que
hace `titan_dap` (crates/titan_dap/src/lib.rs). El oráculo `zett` v1.0.0 no trae `dap`, así que
las expectativas salen de ese código fuente.
Uso: python3 probar.py [ruta/a/titan] [filtro]"""
import os, subprocess, sys, tempfile, time, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cliente import Dap, frame

AQUI = os.path.dirname(os.path.abspath(__file__))
TITAN = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(AQUI, "..", "..", "titan"))
FILTRO = sys.argv[2] if len(sys.argv) > 2 else ""
P = lambda n: os.path.join(AQUI, n)
casos = fallos = 0

def caso(nombre):
    def deco(f):
        global casos, fallos
        if FILTRO and FILTRO not in nombre:
            return f
        casos += 1
        t = time.time()
        try:
            f()
            print("ok   %s (%.1fs)" % (nombre, time.time() - t))
        except Exception as e:
            fallos += 1
            print("FALLA %s: %s: %s" % (nombre, type(e).__name__, str(e)[:600]))
        return f
    return deco

def eq(a, b, msg=""):
    assert a == b, "%s esperado %r, obtenido %r" % (msg, b, a)

def arranque(d, programa, bps=None, extra_launch=None, lineas=None):
    """initialize + launch + setBreakpoints + configurationDone. Devuelve la respuesta de configurationDone."""
    r, ev = d.request("initialize"); assert r["success"]
    args = {"program": programa}
    args.update(extra_launch or {})
    r, _ = d.request("launch", **args); assert r["success"], r
    if bps:
        r, _ = d.request("setBreakpoints", source={"path": programa}, breakpoints=[{"line": l} for l in bps])
        assert r["success"], r
    r, _ = d.request("configurationDone", timeout=120)
    return r

def fin(d):
    return d.close()

# ---------------------------------------------------------------- antes de lanzar

@caso("initialize: capacidades, evento initialized y números de secuencia")
def _():
    d = Dap(TITAN)
    r, ev = d.request("initialize")
    eq(r, {"body": {"supportsConfigurationDoneRequest": True, "supportsEvaluateForHovers": False,
                    "supportsRestartRequest": False, "supportsSetVariable": False,
                    "supportsStepBack": False, "supportsTerminateRequest": True},
           "command": "initialize", "message": None, "request_seq": 1, "seq": 1, "success": True,
           "type": "response"})
    e, _ = d.event("initialized")
    eq(e, {"body": {}, "event": "initialized", "seq": 2, "type": "event"})
    rc, err = fin(d)
    eq((rc, err), (0, ""))

@caso("peticiones sin programa: threads, scopes, stackTrace, variables, errores")
def _():
    d = Dap(TITAN)
    r, _ = d.request("threads"); eq(r["body"], {"threads": [{"id": 1, "name": "Titan main thread"}]})
    r, _ = d.request("scopes"); eq(r["body"], {"scopes": [
        {"name": "Locals", "variablesReference": 100, "expensive": False},
        {"name": "Operand Stack", "variablesReference": 101, "expensive": False}]})
    r, _ = d.request("stackTrace"); eq(r["body"], {"stackFrames": [], "totalFrames": 0})
    r, _ = d.request("variables", variablesReference=100); eq(r["body"], {"variables": []})
    r, _ = d.request("variables"); eq((r["success"], r["message"], r["body"]), (False, "variablesReference is required", None))
    r, _ = d.request("configurationDone")
    eq((r["success"], r["message"]), (False, "launch must be sent before configurationDone"))
    r, _ = d.request("bogus"); eq((r["success"], r["message"]), (False, "unsupported DAP command 'bogus'"))
    r, ev = d.request("continue")
    eq((r["success"], r["message"]), (False, "debuggee is not running"))
    e, _ = d.event("continued"); eq(e["body"], {"threadId": 1, "allThreadsContinued": True})
    for c in ("next", "stepIn", "stepOut", "pause"):
        r, _ = d.request(c); eq((r["success"], r["message"]), (False, "debuggee is not running"), c)
    r, _ = d.request("terminate"); eq(r["success"], True)
    r, _ = d.request("disconnect"); eq(r["success"], True)
    rc, err = fin(d); eq((rc, err), (0, ""))

@caso("launch: errores (sin program, inexistente, error de tipos, sandbox)")
def _():
    d = Dap(TITAN)
    r, _ = d.request("launch"); eq((r["success"], r["message"]), (False, "launch.program is required"))
    r, _ = d.request("launch", program="/no/existe/x.titan"); assert not r["success"], r
    r, _ = d.request("launch", program=P("tipo.titan"))
    assert not r["success"] and "tipo.titan:2:5: invalid operands" in r["message"], r
    r, _ = d.request("launch", program=P("prog.titan"), sandbox=True)
    assert not r["success"] and "sandbox" in r["message"], r
    r, _ = d.request("launch", program=P("prog.titan")); assert r["success"], r
    d.request("disconnect"); fin(d)

@caso("setBreakpoints: verificados, sin instrucción, errores")
def _():
    d = Dap(TITAN)
    prog = os.path.realpath(P("prog.titan"))
    r, _ = d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 2}])
    eq(r["body"]["breakpoints"], [{"line": 2, "message": "no executable instruction on this line", "source": {"path": prog}, "verified": False}], "sin launch")
    d.request("launch", program=prog)
    r, _ = d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 2}, {"line": 99}, {"line": 1}])
    eq([(b["line"], b["verified"]) for b in r["body"]["breakpoints"]], [(2, True), (99, False), (1, False)])
    r, _ = d.request("setBreakpoints", breakpoints=[]); eq(r["message"], "source.path is required")
    r, _ = d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 2}, {"x": 1}])
    eq((r["success"], r["message"]), (False, "breakpoint.line is required"))
    r, _ = d.request("setBreakpoints", source={"path": "/no/existe.titan"}, breakpoints=[])
    eq((r["success"], r["message"]), (False, "No such file or directory (os error 2)"))
    r, _ = d.request("setBreakpoints", source={"path": prog})
    eq(r["body"], {"breakpoints": []})
    d.request("disconnect"); fin(d)

# ---------------------------------------------------------------- sesión completa

@caso("sesión: punto de interrupción, stackTrace, variables, next, stepIn, continue, terminated")
def _():
    d = Dap(TITAN)
    prog = os.path.realpath(P("prog.titan"))
    r = arranque(d, prog, bps=[7])
    eq((r["success"], r["body"]), (True, {}))
    # salida del programa antes de la parada, en un evento por línea
    st, antes = d.event("stopped")
    eq(st["body"], {"reason": "breakpoint", "threadId": 1, "allThreadsStopped": True})
    outs = [e for e in antes if e["event"] == "output"]
    eq([e["body"] for e in outs], [{"category": "stdout", "output": "hola\n"}])
    r, _ = d.request("stackTrace", threadId=1)
    f = r["body"]["stackFrames"][0]
    eq((f["id"], f["name"], f["line"], f["source"]), (1, "main", 7, {"name": "prog.titan", "path": prog}))
    eq(r["body"]["totalFrames"], 1)
    r, _ = d.request("variables", variablesReference=100)
    eq([(v["name"], v["type"], v["value"]) for v in r["body"]["variables"]], [("0", "nil", "nil"), ("1", "nil", "nil")])
    r, _ = d.request("variables", variablesReference=101)
    assert r["success"]
    r, _ = d.request("variables", variablesReference=7); eq(r["body"], {"variables": []})
    # paso a paso: entra en `suma`
    r, ev = d.request("stepIn", threadId=1); eq((r["success"], r["body"]), (True, {}))
    seen = False
    for _ in range(40):
        st, pre = d.event("stopped")
        r, _ = d.request("stackTrace")
        if r["body"]["stackFrames"][0]["name"] == "suma":
            eq(st["body"]["reason"], "step")
            seen = True
            break
        d.request("stepIn")
    assert seen, "stepIn nunca entró en suma"
    r, _ = d.request("variables", variablesReference=100)
    assert [v["type"] for v in r["body"]["variables"]][:2] == ["int", "int"], r["body"]
    assert [v["value"] for v in r["body"]["variables"]][:2] == ["1", "2"], r["body"]
    # stepOut vuelve a main
    d.request("stepOut", threadId=1)
    st, _ = d.event("stopped")
    r, _ = d.request("stackTrace"); eq(r["body"]["stackFrames"][0]["name"], "main")
    # continue hasta el final (la línea 7 vuelve a parar en cada una de sus instrucciones)
    d.request("continue", threadId=1)
    while True:
        m = d.read(60)
        assert m is not None, "el servidor cerró antes de terminated"
        if m["type"] == "event" and m["event"] == "terminated":
            break
        if m["type"] == "event" and m["event"] == "stopped":
            d.request("continue", threadId=1)
    r, _ = d.request("stackTrace"); eq(r["body"], {"stackFrames": [], "totalFrames": 0})
    r, _ = d.request("continue"); eq((r["success"], r["message"]), (False, "debuggee is not running"))
    r, _ = d.request("configurationDone")
    eq((r["success"], r["message"]), (False, "launch must be sent before configurationDone"))
    r, _ = d.request("disconnect"); eq(r["success"], True)
    rc, err = fin(d); eq((rc, err), (0, ""))
    lines = [m["body"]["output"] for m in d.log if m.get("event") == "output"]
    eq("".join(lines), "hola\nx = 3\ny = 13\n")

@caso("sin puntos de interrupción: el programa corre hasta terminar y emite la salida")
def _():
    d = Dap(TITAN)
    arranque(d, P("prog.titan"))
    e, antes = d.event("terminated", timeout=60)
    outs = [m["body"]["output"] for m in antes if m.get("event") == "output"]
    eq(outs, ["hola\n", "x = 3\n", "y = 13\n"])
    assert not [m for m in antes if m.get("event") == "stopped"]
    d.request("disconnect"); rc, err = fin(d); eq((rc, err), (0, ""))

@caso("error en el programa: output stderr + terminated, el servidor sigue")
def _():
    d = Dap(TITAN)
    arranque(d, P("err.titan"))
    e, antes = d.event("terminated", timeout=60)
    ev = [m for m in antes if m.get("event") == "output"]
    eq([(m["body"]["category"], m["body"]["output"]) for m in ev],
       [("stdout", "antes\n"), ("stderr", "division by zero\n")])
    r, _ = d.request("threads"); assert r["success"]
    r, _ = d.request("disconnect"); eq(r["success"], True)
    rc, err = fin(d); eq((rc, err), (0, ""))

@caso("terminate con el programa parado: stderr 'execution terminated by debugger' y terminated")
def _():
    d = Dap(TITAN)
    prog = os.path.realpath(P("prog.titan"))
    arranque(d, prog, bps=[7])
    d.event("stopped")
    r, _ = d.request("terminate"); eq(r["body"], {})
    e, antes = d.event("terminated", timeout=30)
    errs = [m["body"] for m in antes if m.get("event") == "output" and m["body"]["category"] == "stderr"]
    eq(errs, [{"category": "stderr", "output": "execution terminated by debugger\n"}])
    r, _ = d.request("stackTrace"); eq(r["body"]["totalFrames"], 0)
    d.request("disconnect"); rc, err = fin(d); eq((rc, err), (0, ""))

@caso("disconnect con el programa parado: termina y el proceso sale con 0")
def _():
    d = Dap(TITAN)
    arranque(d, P("prog.titan"), bps=[7])
    d.event("stopped")
    r, _ = d.request("disconnect"); eq(r["success"], True)
    e, antes = d.event("terminated", timeout=30)
    rc, err = fin(d); eq((rc, err), (0, ""))

@caso("pause con el programa corriendo: para en la siguiente instrucción")
def _():
    d = Dap(TITAN)
    arranque(d, P("bucle.titan"))
    e, antes = d.event("output", timeout=30)
    eq(e["body"], {"category": "stdout", "output": "girando\n"})
    r, _ = d.request("pause"); eq((r["success"], r["body"]), (True, {}))
    st, _ = d.event("stopped", timeout=30)
    # main está a profundidad 0: titan_dap lo llama 'breakpoint' (solo >0 es 'step')
    eq(st["body"], {"reason": "breakpoint", "threadId": 1, "allThreadsStopped": True})
    r, _ = d.request("stackTrace"); eq(r["body"]["stackFrames"][0]["name"], "main")
    r, _ = d.request("variables", variablesReference=100)
    v = r["body"]["variables"]
    assert v[0]["type"] == "int" and int(v[0]["value"]) > 0, v
    r, _ = d.request("terminate")
    d.event("terminated", timeout=30)
    d.request("disconnect"); rc, err = fin(d); eq((rc, err), (0, ""))

@caso("peticiones adelantadas: threads/stackTrace justo detrás de configurationDone")
def _():
    d = Dap(TITAN)
    prog = os.path.realpath(P("prog.titan"))
    d.request("initialize"); d.request("launch", program=prog)
    d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 7}])
    # tres peticiones de una sola escritura
    d.send_raw(b"".join(frame({"seq": 10 + i, "type": "request", "command": c, "arguments": {}})
                        for i, c in enumerate(["configurationDone", "threads", "scopes"])))
    resp = {}
    paro = False
    while len(resp) < 3 or not paro:
        m = d.read(120)
        if m["type"] == "response":
            resp[m["command"]] = m
        elif m["event"] == "stopped":
            paro = True
    assert all(r["success"] for r in resp.values()), resp
    eq(resp["threads"]["body"], {"threads": [{"id": 1, "name": "Titan main thread"}]})
    d.request("terminate"); d.event("terminated")
    d.request("disconnect"); fin(d)

@caso("salida abundante: 300 líneas en eventos output, texto sin salto al final")
def _():
    d = Dap(TITAN)
    arranque(d, P("salida.titan"))
    e, antes = d.event("terminated", timeout=60)
    outs = [m["body"]["output"] for m in antes if m.get("event") == "output"]
    eq("".join(outs), "".join("linea %d\n" % i for i in range(300)) + "sin salto")
    eq(outs[-1], "sin salto")
    assert all(o.endswith("\n") for o in outs[:-1])
    d.request("disconnect"); fin(d)

@caso("launch con un .tbc (artefacto de bytecode)")
def _():
    tbc = tempfile.mktemp(suffix=".tbc")
    r = subprocess.run([TITAN, "build", "--bytecode", P("prog.titan"), "-o", tbc], capture_output=True, text=True)
    if not os.path.exists(tbc):
        r = subprocess.run([TITAN, "build", P("prog.titan"), "--emit", "tbc", "-o", tbc], capture_output=True, text=True)
    assert os.path.exists(tbc), r.stdout + r.stderr
    d = Dap(TITAN)
    d.request("initialize")
    r, _ = d.request("launch", program=tbc); assert r["success"], r
    r, _ = d.request("configurationDone", timeout=120); assert r["success"], r
    d.event("terminated", timeout=60)
    d.request("disconnect"); fin(d); os.unlink(tbc)

@caso("configurationDone repetido y launch tras arrancar")
def _():
    d = Dap(TITAN)
    arranque(d, P("prog.titan"), bps=[7])
    d.event("stopped")
    r, _ = d.request("configurationDone"); eq((r["success"], r["message"]), (False, "debuggee already started"))
    r, _ = d.request("setBreakpoints", source={"path": P("prog.titan")}, breakpoints=[{"line": 7}])
    eq(r["body"]["breakpoints"][0]["verified"], False)
    r, _ = d.request("launch"); eq(r["message"], "launch.program is required")
    d.request("terminate"); d.event("terminated")
    d.request("disconnect"); fin(d)


@caso("punto dentro de una función: reason 'step' (profundidad > 0) y reemplazo de puntos por archivo")
def _():
    d = Dap(TITAN)
    prog = os.path.realpath(P("prog.titan"))
    d.request("initialize"); d.request("launch", program=prog)
    d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 7}])
    r, _ = d.request("setBreakpoints", source={"path": prog}, breakpoints=[{"line": 2}])   # reemplaza al anterior
    d.request("configurationDone", timeout=120)
    st, antes = d.event("stopped")
    eq(st["body"]["reason"], "step")
    r, _ = d.request("stackTrace")
    f = r["body"]["stackFrames"][0]
    eq((f["name"], f["line"]), ("suma", 2))
    r, _ = d.request("variables", variablesReference=101)
    assert r["success"]
    d.request("terminate"); d.event("terminated")
    d.request("disconnect"); fin(d)

@caso("launch con un proyecto (directorio con titan.toml) sin escribir titan.lock")
def _():
    raiz = tempfile.mkdtemp()
    r = subprocess.run([TITAN, "new", "proy", "--path", raiz], capture_output=True, text=True, cwd=raiz)
    proy = os.path.join(raiz, "proy")
    if not os.path.isdir(proy):
        r = subprocess.run([TITAN, "new", "proy"], capture_output=True, text=True, cwd=raiz)
    assert os.path.isdir(proy), r.stdout + r.stderr
    antes = set(os.listdir(proy))
    d = Dap(TITAN)
    d.request("initialize")
    r, _ = d.request("launch", program=proy); assert r["success"], r
    r, _ = d.request("configurationDone", timeout=120); assert r["success"], r
    e, ev = d.event("terminated", timeout=60)
    d.request("disconnect"); fin(d)
    eq(set(os.listdir(proy)), antes, "archivos del proyecto")

# ---------------------------------------------------------------- transporte

def crudo(datos, esperado_rc, esperado_err, antes=None):
    d = Dap(TITAN)
    d.send_raw(datos)
    d.p.stdin.close()
    d.p.wait(timeout=30)
    err = d.p.stderr.read().decode()
    eq(d.p.returncode, esperado_rc, "código")
    assert esperado_err in err, (esperado_err, err)

@caso("transporte: errores de marco y de JSON salen con 'titan-dap: …' y código 1")
def _():
    crudo(b"X-Other: 1\r\n\r\n{}", 1, "titan-dap: invalid DAP frame: missing Content-Length\n")
    crudo(b"Content-Length: abc\r\n\r\n", 1, "titan-dap: invalid DAP frame: invalid Content-Length\n")
    crudo(b"Content-Length: -3\r\n\r\n", 1, "titan-dap: invalid DAP frame: invalid Content-Length\n")
    crudo(b"Content-Length: 99999999\r\n\r\n", 1, "titan-dap: invalid DAP frame: message exceeds 16 MiB\n")
    crudo(b"Content-Length: 3\r\n\r\n{{{", 1, "titan-dap: invalid DAP JSON: ")
    crudo(b"Content-Length: 10\r\n\r\n{}", 1, "titan-dap: DAP I/O error: failed to fill whole buffer\n")
    crudo(b"Content-Length: 4\r\n\r\n\xff\xfe\xfd\xfc", 1, "titan-dap: invalid DAP JSON: ")

@caso("transporte: fin de entrada limpio, cabeceras en minúscula/mayúscula y extra")
def _():
    crudo(b"", 0, "")
    crudo(b"Content-Length: 2\r\n", 0, "")        # cabecera incompleta y fin: como Rust, sale sin error
    cuerpo = b'{"seq":5,"type":"request","command":"threads"}'
    d = Dap(TITAN)
    d.send_raw(b"content-length: %d\r\nX-Extra: 1\n\n" % len(cuerpo) + cuerpo)
    m = d.read(30); eq((m["command"], m["request_seq"], m["seq"]), ("threads", 5, 1))
    d.send_raw(b"Content-Length: +%d\r\n\r\n" % len(b"[]") + b"[]")      # no es objeto: comando ''
    m = d.read(30); eq((m["success"], m["message"], m["request_seq"]), (False, "unsupported DAP command ''", 0))
    d.send_raw(b"Content-Length: 4\r\n\r\nnull")
    m = d.read(30); eq(m["message"], "unsupported DAP command ''")
    rc, err = fin(d); eq((rc, err), (0, ""))

@caso("transporte tras arrancar: JSON inválido en el ejecutable de depuración")
def _():
    d = Dap(TITAN)
    arranque(d, P("prog.titan"), bps=[7])
    d.event("stopped")
    d.send_raw(b"Content-Length: 3\r\n\r\n{{{")
    d.p.wait(timeout=30)
    err = d.p.stderr.read().decode()
    eq(d.p.returncode, 1)
    assert err.startswith("titan-dap: invalid DAP JSON: "), err

@caso("EOF con el programa parado: sale con 0 (Rust esperaría para siempre)")
def _():
    d = Dap(TITAN)
    arranque(d, P("prog.titan"), bps=[7])
    d.event("stopped")
    d.p.stdin.close()
    d.p.wait(timeout=30)
    eq(d.p.returncode, 0)

print("\ncasos: %d, fallan: %d" % (casos, fallos))
sys.exit(1 if fallos else 0)
