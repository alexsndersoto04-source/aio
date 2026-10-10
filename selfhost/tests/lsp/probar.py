#!/usr/bin/env python3
"""Pruebas del servidor LSP escrito en Titan (`titan lsp`).

Uso: python3 probar.py [CLI]        (por defecto selfhost/titan)

El LSP de Rust no se puede compilar aquí, así que no hay comparación diferencial: los casos son los
siete tests unitarios de `crates/titan_lsp` (portados) más casos escritos leyendo su código. Los
mensajes de diagnóstico se contrastan con `zett check` en `contraste_zett.py` (si hay `zett`)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cliente import sesion, frame

CLI = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else os.path.abspath(os.path.join(os.path.dirname(__file__), "../../titan"))
ok = 0
mal = 0

def check(desc, cond, detalle=""):
    global ok, mal
    if cond:
        ok += 1
    else:
        mal += 1
        print("FALLA:", desc, detalle if detalle else "")

def req(i, method, params=None):
    return {"jsonrpc": "2.0", "id": i, "method": method, "params": params if params is not None else {}}

def note(method, params):
    return {"jsonrpc": "2.0", "method": method, "params": params}

def abrir(uri, text, version=1):
    return note("textDocument/didOpen", {"textDocument": {"uri": uri, "version": version, "text": text}})

def doc(uri):
    return {"textDocument": {"uri": uri}}

def pos(uri, line, ch, **extra):
    d = {"textDocument": {"uri": uri}, "position": {"line": line, "character": ch}}
    d.update(extra)
    return d

EXIT = {"jsonrpc": "2.0", "method": "exit"}

def correr(msgs, **kw):
    out, err, rc = sesion(CLI, msgs + [EXIT], **kw)
    return [json.loads(o) for o in out], err, rc

def respuesta(out, i):
    for o in out:
        if o.get("id") == i:
            return o
    return None

def rng(sl, sc, el, ec):
    return {"start": {"line": sl, "character": sc}, "end": {"line": el, "character": ec}}

X = "file:///x.titan"

# ---- 1. test del servidor de Rust: initialize + didOpen + documentSymbol (bytes exactos)
body = [req(1, "initialize"), abrir(X, "fn main() { 1 }"), req(2, "textDocument/documentSymbol", doc(X)), EXIT]
out, err, rc = sesion(CLI, body)
txt = b"\n".join(out).decode()
check("initialize/didOpen/symbols: rc 0, sin stderr", rc == 0 and err == "", (rc, err))
check("contiene titan-lsp, publishDiagnostics y main", "titan-lsp" in txt and "publishDiagnostics" in txt and "main" in txt)
check("claves ordenadas y compactas (serde_json)", out[1] == b'{"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"diagnostics":[],"uri":"file:///x.titan"}}', out[1])
cap = json.loads(out[0])["result"]
check("capacidades", cap["capabilities"]["textDocumentSync"] == {"openClose": True, "change": 2}
      and cap["capabilities"]["completionProvider"] == {"triggerCharacters": [".", ":"]}
      and cap["capabilities"]["signatureHelpProvider"] == {"triggerCharacters": ["(", ","]}
      and cap["capabilities"]["semanticTokensProvider"] == {"legend": {"tokenTypes": ["keyword", "function", "type", "variable", "string", "number", "operator"], "tokenModifiers": []}, "full": True}
      and cap["capabilities"]["renameProvider"] == {"prepareProvider": False}
      and all(cap["capabilities"][k] is True for k in ("hoverProvider", "definitionProvider", "referencesProvider", "documentSymbolProvider", "workspaceSymbolProvider"))
      and cap["serverInfo"] == {"name": "titan-lsp", "version": "1.0.0"}, cap)

# ---- 2. símbolos, definición, referencias (test de Rust) + hover
src = "fn double(x: int) -> int { x * 2 }\nfn main() { double(21) }"
out, err, rc = correr([req(1, "initialize"), abrir(X, src),
                       req(2, "textDocument/documentSymbol", doc(X)),
                       req(3, "textDocument/definition", pos(X, 1, 13)),
                       req(4, "textDocument/references", pos(X, 0, 4)),
                       req(5, "textDocument/hover", pos(X, 1, 13)),
                       req(6, "textDocument/hover", pos(X, 0, 0)),
                       req(7, "textDocument/definition", pos(X, 0, 0)),
                       req(8, "textDocument/hover", pos(X, 9, 0)),
                       req(9, "workspace/symbol", {"query": "DOU"})])
syms = respuesta(out, 2)["result"]
check("dos símbolos", [s["name"] for s in syms] == ["double", "main"] and syms[0]["kind"] == 12 and syms[0]["detail"] == "function", syms)
check("rango del símbolo", syms[0]["range"] == rng(0, 3, 0, 9) and syms[0]["selectionRange"] == rng(0, 3, 0, 9) and syms[1]["range"] == rng(1, 3, 1, 7), syms)
check("definición de double", respuesta(out, 3)["result"] == {"uri": X, "range": rng(0, 3, 0, 9)}, respuesta(out, 3))
check("dos referencias", respuesta(out, 4)["result"] == [{"uri": X, "range": rng(0, 3, 0, 9)}, {"uri": X, "range": rng(1, 12, 1, 18)}], respuesta(out, 4))
check("hover", respuesta(out, 5)["result"] == {"contents": {"kind": "markdown", "value": "`double`\n\nfunction"}}, respuesta(out, 5))
check("hover sobre palabra clave = null", respuesta(out, 6)["result"] is None)
check("definición sobre palabra clave = null", respuesta(out, 7)["result"] is None)
check("hover fuera del documento = null", respuesta(out, 8)["result"] is None)
check("workspace/symbol sin distinguir mayúsculas", respuesta(out, 9)["result"] == [{"name": "double", "kind": 12, "location": {"uri": X, "range": rng(0, 3, 0, 9)}}], respuesta(out, 9))

# ---- 3. cambios incrementales en UTF-16 y rename (test de Rust)
out, err, rc = correr([abrir(X, "fn café() { 1 }"),
                       note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"range": rng(0, 3, 0, 7), "text": "tea"}]}),
                       req(1, "textDocument/documentSymbol", doc(X)),
                       req(2, "textDocument/rename", pos(X, 0, 4, newName="brew")),
                       req(3, "textDocument/rename", pos(X, 0, 4, newName="1x")),
                       req(4, "textDocument/rename", pos(X, 0, 0, newName="ok")),
                       req(5, "textDocument/rename", pos(X, 0, 4)),
                       req(6, "textDocument/rename", pos(X, 0, 4, newName="é_ñ2")),
                       req(7, "textDocument/rename", pos(X, 0, 4, newName=""))])
check("didChange publica diagnósticos", out[1]["method"] == "textDocument/publishDiagnostics" and out[1]["params"]["diagnostics"] == [], out[1])
check("tras el cambio el símbolo es tea", respuesta(out, 1)["result"][0]["name"] == "tea" and respuesta(out, 1)["result"][0]["range"] == rng(0, 3, 0, 6), respuesta(out, 1))
check("rename", respuesta(out, 2)["result"] == {"changes": {X: [{"newText": "brew", "range": rng(0, 3, 0, 6)}]}}, respuesta(out, 2))
check("rename: identificador inválido", respuesta(out, 3)["error"] == {"code": -32602, "message": "invalid Titan identifier"}, respuesta(out, 3))
check("rename: sin identificador", respuesta(out, 4)["error"]["message"] == "no identifier at position", respuesta(out, 4))
check("rename: falta newName", respuesta(out, 5)["error"]["message"] == "missing newName", respuesta(out, 5))
check("rename: identificador con letras Unicode", "result" in respuesta(out, 6), respuesta(out, 6))
check("rename: nombre vacío", respuesta(out, 7)["error"]["message"] == "invalid Titan identifier", respuesta(out, 7))

# ---- 4. posiciones UTF-16 con un emoji (2 unidades) y varias líneas
src = 'fn a() { "😀"; let zz = 1 }\nfn bé() {}\nfn c() { let π = 2; π }'
out, err, rc = correr([abrir(X, src), req(1, "textDocument/documentSymbol", doc(X)), req(2, "textDocument/references", pos(X, 2, 13))])
s = {x["name"]: x["range"] for x in respuesta(out, 1)["result"]}
check("símbolo tras un emoji: UTF-16", s["zz"] == rng(0, 19, 0, 21), s)
check("símbolo con acento", s["bé"] == rng(1, 3, 1, 5), s)
check("símbolo con letra griega", s["π"] == rng(2, 13, 2, 14), s)
check("referencias de π", [r["range"] for r in respuesta(out, 2)["result"]] == [rng(2, 13, 2, 14), rng(2, 20, 2, 21)], respuesta(out, 2))
out, err, rc = correr([abrir(X, src), req(1, "textDocument/hover", pos(X, 0, 11))])
check("posición en medio de un par sustituto = null", respuesta(out, 1)["result"] is None, respuesta(out, 1))

# ---- 5. tokens semánticos y ayuda de firma (test de Rust)
out, err, rc = correr([abrir(X, "fn main() { std::stats::mean([1, 2]) }"),
                       req(1, "textDocument/semanticTokens/full", doc(X)),
                       req(2, "textDocument/signatureHelp", pos(X, 0, 33)),
                       req(3, "textDocument/signatureHelp", pos(X, 0, 30)),
                       req(4, "textDocument/signatureHelp", pos(X, 0, 5)),
                       req(5, "textDocument/signatureHelp", pos(X, 0, 28))])
data = respuesta(out, 1)["result"]["data"]
check("tokens: múltiplo de 5 y no vacío", len(data) > 0 and len(data) % 5 == 0)
check("tokens: primeros (fn, main, …)", data[:10] == [0, 0, 2, 0, 0, 0, 3, 4, 1, 0], data[:10])
sig = respuesta(out, 2)["result"]
check("firma de std::stats::mean", sig["signatures"][0]["label"].startswith("std::stats::mean(arg0:") and sig["activeParameter"] == 1 and sig["activeSignature"] == 0, sig)
check("firma: activo 0 antes de la coma", respuesta(out, 3)["result"]["activeParameter"] == 0)
check("firma: sin paréntesis = null", respuesta(out, 4)["result"] is None)
check("firma: sobre el '(' se usa el '(' anterior (rfind sobre el prefijo)", respuesta(out, 5)["result"]["signatures"][0]["label"] == "main(…)", respuesta(out, 5))
out, err, rc = correr([abrir(X, "fn double(a: int, b: int) { 0 }\nfn main() { double(1, 2, 3) }"),
                       req(1, "textDocument/signatureHelp", pos(X, 1, 22)),
                       req(2, "textDocument/signatureHelp", pos(X, 1, 25))])
check("firma de función propia", respuesta(out, 1)["result"]["signatures"][0] == {"label": "double(…)", "parameters": []} and respuesta(out, 1)["result"]["activeParameter"] == 1, respuesta(out, 1))
check("firma: tercer argumento (2 comas)", respuesta(out, 2)["result"]["activeParameter"] == 2, respuesta(out, 2))
src = 'fn main() {\n  let s = "multi\nline"\n  let t = 1 // c\n}'
out, err, rc = correr([abrir(X, src), req(1, "textDocument/semanticTokens/full", doc(X))])
d = respuesta(out, 1)["result"]["data"]
filas = [d[i:i + 5] for i in range(0, len(d), 5)]
check("tokens: se salta la cadena de varias líneas", all(f[3] != 4 for f in filas), filas)
check("tokens: let en la línea 1", [1, 2, 3, 0, 0] in filas, filas)

# ---- 6. diagnósticos
out, err, rc = correr([abrir(X, "fn main( {"), abrir("file:///t.titan", "fn main() {\n    1 + true\n}"), abrir("file:///d.titan", "fn main() {}\nfn duplicate() {}\nfn duplicate() {}"),
                       abrir("file:///l.titan", 'fn main() { "abc'), abrir("file:///c.titan", "fn main() { let x = 1 }")])
diag = {o["params"]["uri"]: o["params"]["diagnostics"] for o in out if o.get("method") == "textDocument/publishDiagnostics"}
check("parseo inválido", len(diag[X]) >= 1 and diag[X][0]["severity"] == 1 and diag[X][0]["source"] == "titan" and "expected" in diag[X][0]["message"], diag[X])
t = [d for d in diag["file:///t.titan"] if "invalid operands" in d["message"]]
check("error de tipos en el rango de la expresión", len(t) == 1 and t[0]["range"]["start"]["line"] == 1 and t[0]["range"]["start"]["character"] > 0 and t[0]["range"]["end"]["character"] > t[0]["range"]["start"]["character"], diag["file:///t.titan"])
d2 = [d for d in diag["file:///d.titan"] if "duplicate function" in d["message"]]
check("función duplicada", len(d2) == 1 and d2[0]["range"]["start"]["line"] == 2 and d2[0]["range"]["end"]["character"] > d2[0]["range"]["start"]["character"], diag["file:///d.titan"])
check("error léxico: solo errores léxicos, un carácter de ancho", diag["file:///l.titan"] == [{"range": rng(0, 12, 0, 13), "severity": 1, "source": "titan", "message": "unterminated string at 1:13"}], diag["file:///l.titan"])
check("documento correcto = sin diagnósticos", diag["file:///c.titan"] == [], diag["file:///c.titan"])

# ---- 7. completado
out, err, rc = correr([abrir(X, "fn main() { 1 }\nconst K = 2\nstruct Punto { x: int }"), req(1, "textDocument/completion", pos(X, 0, 0))])
items = respuesta(out, 1)["result"]
labels = [i["label"] for i in items]
by = {i["label"]: i for i in items}
check("completado ordenado por etiqueta (bytes)", labels == sorted(labels, key=lambda s: s.encode()), labels[:5])
check("palabra clave", by["fn"] == {"label": "fn", "kind": 14, "detail": "Titan keyword"}, by.get("fn"))
check("símbolo del documento", by["main"] == {"label": "main", "kind": 12, "detail": "function"} and by["K"]["kind"] == 14 and by["Punto"]["kind"] == 23, (by.get("main"), by.get("K"), by.get("Punto")))
check("nativa con firma", by["std::text::length"] == {"label": "std::text::length", "kind": 3, "detail": "native [String] -> Int"}, by.get("std::text::length"))
check("nativa sin parámetros", by["std::map::new"]["detail"] == "native [] -> Map", by.get("std::map::new"))
check("nativa con varios parámetros", by["std::text::replace"]["detail"] == "native [String, String, String] -> String", by.get("std::text::replace"))
check("cantidad: 20 palabras + nativas + 3 símbolos", len(items) == 20 + 3 + 823, len(items))

# ---- 8. varios documentos: workspace/symbol, referencias y rename entre archivos
A, B = "file:///a.titan", "file:///b.titan"
out, err, rc = correr([abrir(B, "fn util() {}\nfn zeta() {}"), abrir(A, "fn util2() { util() }\nfn alfa() {}"),
                       req(1, "workspace/symbol", {"query": "util"}), req(2, "workspace/symbol", {}),
                       req(3, "textDocument/references", pos(A, 0, 13)),
                       req(4, "textDocument/rename", pos(A, 0, 13, newName="helper")),
                       req(5, "textDocument/definition", pos(A, 0, 13))])
check("workspace/symbol ordenado por nombre", [s["name"] for s in respuesta(out, 1)["result"]] == ["util", "util2"], respuesta(out, 1))
check("workspace/symbol sin consulta = todos", [s["name"] for s in respuesta(out, 2)["result"]] == ["alfa", "util", "util2", "zeta"], respuesta(out, 2))
refs = respuesta(out, 3)["result"]
check("referencias en dos documentos", sorted((r["uri"], r["range"]["start"]["line"], r["range"]["start"]["character"]) for r in refs) == [(A, 0, 13), (B, 0, 3)], refs)
ch = respuesta(out, 4)["result"]["changes"]
check("rename en dos documentos, claves ordenadas", list(ch.keys()) == [A, B] and ch[A] == [{"newText": "helper", "range": rng(0, 13, 0, 17)}] and ch[B][0]["range"] == rng(0, 3, 0, 7), ch)
check("definición entre documentos", respuesta(out, 5)["result"] == {"uri": B, "range": rng(0, 3, 0, 7)}, respuesta(out, 5))

# ---- 9. protocolo: ids, shutdown, método desconocido, errores de parámetros
out, err, rc = correr([{"jsonrpc": "2.0", "id": "abc", "method": "initialize"},
                       {"jsonrpc": "2.0", "id": None, "method": "textDocument/hover"},
                       req(3, "nope/nothing"),
                       req(4, "textDocument/hover", {}),
                       req(5, "textDocument/hover", {"textDocument": {"uri": X}}),
                       req(6, "textDocument/hover", {"textDocument": {"uri": X}, "position": {"line": 1}}),
                       req(7, "textDocument/hover", {"textDocument": {"uri": X}, "position": {"line": -1, "character": 0}}),
                       req(8, "textDocument/hover", {"textDocument": {"uri": X}, "position": {"line": 1.0, "character": 0}}),
                       req(9, "textDocument/documentSymbol", {"textDocument": {"uri": 5}}),
                       req(10, "textDocument/semanticTokens/full", doc("file:///nada.titan")),
                       req(11, "textDocument/completion", {"textDocument": {"uri": X}}),
                       req(12, "textDocument/rename", {"textDocument": {"uri": X}, "position": {"line": 0, "character": 0}}),
                       req(13, "shutdown"), req(14, "workspace/symbol", {"query": 7})])
check("id de texto se devuelve igual", out[0]["id"] == "abc" and "result" in out[0])
check("id null cuenta como petición", out[1]["id"] is None and out[1]["error"] == {"code": -32602, "message": "missing textDocument.uri"}, out[1])
check("método desconocido", respuesta(out, 3)["error"] == {"code": -32602, "message": "method not found"})
check("falta textDocument.uri", respuesta(out, 4)["error"]["message"] == "missing textDocument.uri")
check("falta position", respuesta(out, 5)["error"]["message"] == "missing position")
check("falta character", respuesta(out, 6)["error"]["message"] == "missing character")
check("línea negativa", respuesta(out, 7)["error"]["message"] == "missing line")
check("línea decimal", respuesta(out, 8)["error"]["message"] == "missing line")
check("uri que no es texto", respuesta(out, 9)["error"]["message"] == "missing textDocument.uri")
check("documento no abierto: tokens vacíos", respuesta(out, 10)["result"] == {"data": []})
check("completion sin position", respuesta(out, 11)["error"]["message"] == "missing position")
check("rename sin newName", respuesta(out, 12)["error"]["message"] == "missing newName")
check("shutdown devuelve null", respuesta(out, 13)["result"] is None and "result" in respuesta(out, 13))
check("consulta que no es texto = consulta vacía", isinstance(respuesta(out, 14)["result"], list))
out, err, rc = correr([req(1, "shutdown")])
check("exit tras shutdown: código 0", rc == 0 and respuesta(out, 1) is not None)

# ---- 10. cierre de documentos
out, err, rc = correr([abrir(X, "fn main() {}"), note("textDocument/didClose", doc(X)), req(1, "textDocument/documentSymbol", doc(X)), req(2, "workspace/symbol", {})])
check("didClose publica diagnósticos vacíos", out[1] == {"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": {"diagnostics": [], "uri": X}}, out[1])
check("tras cerrar no hay símbolos", respuesta(out, 1)["result"] == [] and respuesta(out, 2)["result"] == [])

# ---- 11. cambios: reemplazo total, varios cambios, versiones y errores que terminan el servidor
out, err, rc = correr([abrir(X, "fn a() {}"),
                       note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"text": "fn b() {}\nfn c() {}"}, {"range": rng(1, 3, 1, 4), "text": "d"}]}),
                       note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"range": rng(0, 9, 0, 9), "text": "\nfn e() {}"}]}),
                       req(1, "textDocument/documentSymbol", doc(X))])
check("reemplazo total + varios cambios + misma versión", [s["name"] for s in respuesta(out, 1)["result"]] == ["b", "e", "d"], respuesta(out, 1))
def termina(desc, msgs, mensaje):
    out, err, rc = sesion(CLI, msgs)
    check(desc, rc == 1 and err.strip() == "titan-lsp: " + mensaje, (rc, err))
    return out
termina("versión vieja", [abrir(X, "fn a() {}", 5), note("textDocument/didChange", {"textDocument": {"uri": X, "version": 4}, "contentChanges": [{"text": "x"}]})], "invalid LSP frame: stale document version")
termina("documento no abierto", [note("textDocument/didChange", {"textDocument": {"uri": X, "version": 4}, "contentChanges": [{"text": "x"}]})], "invalid LSP frame: document is not open")
termina("inicio inválido", [abrir(X, "fn a() {}"), note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"range": rng(5, 0, 5, 0), "text": "x"}]})], "invalid LSP frame: invalid change start")
termina("fin inválido", [abrir(X, "fn a() {}"), note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"range": rng(0, 0, 0, 99), "text": "x"}]})], "invalid LSP frame: invalid change end")
termina("rango invertido", [abrir(X, "fn a() {}"), note("textDocument/didChange", {"textDocument": {"uri": X, "version": 2}, "contentChanges": [{"range": rng(0, 5, 0, 2), "text": "x"}]})], "invalid LSP frame: change range is reversed")
termina("falta textDocument", [note("textDocument/didChange", {})], "invalid LSP frame: missing textDocument")
termina("falta uri en didChange", [note("textDocument/didChange", {"textDocument": {}})], "invalid LSP frame: missing uri")
termina("falta text en un cambio", [abrir(X, "x"), note("textDocument/didChange", {"textDocument": {"uri": X}, "contentChanges": [{}]})], "invalid LSP frame: missing text")
termina("rango sin start", [abrir(X, "x"), note("textDocument/didChange", {"textDocument": {"uri": X}, "contentChanges": [{"range": {}, "text": "y"}]})], "invalid LSP frame: missing range.start")
termina("rango sin end", [abrir(X, "x"), note("textDocument/didChange", {"textDocument": {"uri": X}, "contentChanges": [{"range": {"start": {"line": 0, "character": 0}}, "text": "y"}]})], "invalid LSP frame: missing range.end")
termina("didOpen sin uri", [note("textDocument/didOpen", {"textDocument": {"text": "x"}})], "invalid LSP frame: missing uri")
termina("didOpen sin text", [note("textDocument/didOpen", {"textDocument": {"uri": X}})], "invalid LSP frame: missing text")
termina("didClose sin uri", [note("textDocument/didClose", {})], "invalid LSP frame: missing textDocument.uri")
out, err, rc = correr([note("textDocument/didOpen", {}), note("otra/cosa", {}), {"jsonrpc": "2.0", "method": 5}, req(1, "shutdown")])
check("didOpen sin textDocument se ignora; notificaciones desconocidas también", rc == 0 and len(out) == 1, (out, err))

# ---- 12. transporte: cabeceras, mayúsculas, longitudes y JSON inválido
def crudo(desc, data, mensaje=None, rc_esperado=1, nsal=None):
    out, err, rc = sesion(CLI, [], crudo=data)
    if mensaje is None:
        check(desc, rc == rc_esperado and err == "" and (nsal is None or len(out) == nsal), (rc, err, out))
    else:
        check(desc, rc == rc_esperado and err.strip() == "titan-lsp: " + mensaje, (rc, err))
    return out
b = json.dumps(req(1, "shutdown")).encode()
crudo("entrada vacía: fin limpio", b"", rc_esperado=0, nsal=0)
out = crudo("content-length en minúsculas", b"content-length: %d\r\n\r\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("cabeceras extra y terminación solo \\n", b"Content-Type: x\nContent-Length: %d\n\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("+ delante del número", b"Content-Length: +%d\r\n\r\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("espacios alrededor del número", b"Content-Length:   %d  \r\n\r\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("ceros a la izquierda", b"Content-Length: 000%d\r\n\r\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("la última cabecera repetida gana", b"Content-Length: 3\r\nContent-Length: %d\r\n\r\n" % len(b) + b, rc_esperado=0, nsal=1)
crudo("longitud no numérica", b"Content-Length: abc\r\n\r\n{}", "invalid LSP frame: invalid Content-Length")
crudo("longitud negativa", b"Content-Length: -1\r\n\r\n{}", "invalid LSP frame: invalid Content-Length")
crudo("longitud vacía", b"Content-Length:\r\n\r\n{}", "invalid LSP frame: invalid Content-Length")
crudo("longitud mayor que u64", b"Content-Length: 18446744073709551616\r\n\r\n{}", "invalid LSP frame: invalid Content-Length")
crudo("longitud de 17 MiB", b"Content-Length: 17825792\r\n\r\n{}", "invalid LSP frame: message exceeds 16 MiB")
crudo("longitud u64 máxima: supera 16 MiB", b"Content-Length: 18446744073709551615\r\n\r\n{}", "invalid LSP frame: message exceeds 16 MiB")
crudo("sin Content-Length", b"X-Foo: 1\r\n\r\n{}", "invalid LSP frame: missing Content-Length")
crudo("fin de archivo en las cabeceras", b"Content-Length: 5\r\n", "invalid LSP frame: unexpected EOF in headers")
crudo("fin de archivo en una cabecera sin salto", b"Content-Length: 5", "invalid LSP frame: unexpected EOF in headers")
crudo("cuerpo truncado", b"Content-Length: 50\r\n\r\n{}", "LSP I/O error: failed to fill whole buffer")
crudo("JSON inválido", b"Content-Length: 4\r\n\r\n{bad", "invalid JSON-RPC message: key must be a string at line 1 column 2")
crudo("caracteres sobrantes", b"Content-Length: 4\r\n\r\n{} x", "invalid JSON-RPC message: trailing characters at line 1 column 4")
crudo("espacios sobrantes están permitidos", b"Content-Length: 4\r\n\r\n{}  ", rc_esperado=0, nsal=0)
crudo("mensajes que no son objetos se ignoran", frame([1, 2]) + frame(None) + frame("x") + frame(req(1, "shutdown")), rc_esperado=0, nsal=1)
g = json.dumps(req(7, "initialize")).encode()
out, err, rc = sesion(CLI, [], crudo=b"Content-Length: %d\r\n\r\n" % len(g) + g[:10] + g[10:] + b"Content-Length: %d\r\n\r\n" % len(b) + b)
check("dos mensajes seguidos", rc == 0 and len(out) == 2, (rc, err))
# mensaje grande (cerca de 1 MiB de texto) y UTF-8 en el cuerpo
grande = "fn f0() {}\n" * 40000
out, err, rc = correr([abrir(X, grande), req(1, "textDocument/documentSymbol", doc(X))], timeout=300)
check("documento grande (440 KB, 40 000 funciones)", rc == 0 and len(respuesta(out, 1)["result"]) == 40000 and out[0]["params"]["diagnostics"] != [], (rc, err))
out, err, rc = correr([abrir(X, "fn ñandú() { \"日本語\" }"), req(1, "textDocument/documentSymbol", doc(X))])
check("UTF-8 en el documento", respuesta(out, 1)["result"][0]["name"] == "ñandú" and respuesta(out, 1)["result"][0]["range"] == rng(0, 3, 0, 8), respuesta(out, 1))

print("lsp: %d comprobaciones correctas, %d fallan" % (ok, mal))
sys.exit(1 if mal else 0)
