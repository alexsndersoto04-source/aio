#!/usr/bin/env python3
"""Prueba diferencial de std::yaml (parse y parse_multi): genera textos YAML
(válidos, mutados, sopa de caracteres especiales, documentos grandes para
los trozos de 16 KB del lector), los ejecuta con prueba.titan en la VM
(zett run) y compilado con el backend nativo, y compara línea a línea.

  python3 fuzz.py CASOS SEMILLA BINARIO_NATIVO
"""
import os, random, subprocess, sys

R = None

SCALARS = ["a", "b c", "~", "null", "Null", "NULL", "true", "False", "TRUE", "yes", "0", "-0", "+0",
           "007", "-007", "+007", "0x1F", "0X1F", "-0x1f", "0x-1", "+0x5", "0o17", "-0o17", "0b101", "-0b11",
           "0x", "0o", "0b2", "1_000", "1.5", "-1.5", "+1.5", ".5", "5.", "1e3", "1E-3", "+1e+3", "1e400",
           "-1e400", ".inf", "-.Inf", "+.INF", ".nan", ".NaN", "+.nan", "inf", "nan", "infinity", "-inf",
           "9223372036854775807", "9223372036854775808", "-9223372036854775808", "-9223372036854775809",
           "18446744073709551615", "18446744073709551616", "340282366920938463463374607431768211455",
           "340282366920938463463374607431768211456", "-170141183460469231731687303715884105728",
           "-170141183460469231731687303715884105729", "0xFFFFFFFFFFFFFFFF", "0x10000000000000000",
           "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", "0x100000000000000000000000000000000",
           "-0x8000000000000000", "-0x8000000000000001", "12:30", "a:b", "a#b", "ñandú", "日本語",
           "e\u0301", "x y z", "-", "?", ":", "%", "@", "`", "''", "\"\"", "'it''s'", "\"esc \\t \\n \\x41 \\u00e9 \\U0001F600\"",
           "\"\\N\\_\\L\\P\\e\\0\\a\\b\\v\\f\\r\\ \\/\\\\\\\"\"", "\"bad \\q\"", "\"\\xZZ\"", "\"\\uD800\"", "\"\\U00110000\"",
           "!!str 12", "!!int 12", "!!int x", "!!int 0x10", "!!float 1", "!!float x", "!!float .inf", "!!bool true",
           "!!bool yes", "!!null ~", "!!null ''", "!!null x", "!foo bar", "! x", "!<tag:x> y", "!<!e> z",
           "!!binary aGk=", "!x!y z", "!<%C3%A9> v", "!<%00> 5", "!<%C0%80> 7", "!<%FF> 1", "!e%41 q",
           "&a 1", "*a", "&b [1, 2]", "*b", "*nope"]

KEYS = ["a", "b", "k", "key", "x y", "1", "true", "~", "'q'", "\"dq\"", "? c", "ñ", "&k kk", "*k", "[1]", "{a: 1}", "!t t", "a.b", ""]

BAD = {"*a", "*b", "*nope", "\"bad \\q\"", "\"\\xZZ\"", "\"\\uD800\"", "\"\\U00110000\"", "!!int x", "!!float x",
       "!!bool yes", "!!null ''", "!!null x", "!foo bar", "! x", "!<!e> z", "!x!y z", "!e%41 q", "@", "`", "%",
       "340282366920938463463374607431768211455", "340282366920938463463374607431768211456",
       "-170141183460469231731687303715884105728", "-170141183460469231731687303715884105729",
       "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", "0x100000000000000000000000000000000", "0x10000000000000000",
       "18446744073709551616", "-9223372036854775809", "-0x8000000000000001", "!!int 0x10", "!<%00> 5", "!<%FF> 1",
       "!<%C0%80> 7", "?", ":", "-", "!!binary aGk="}
SAFE = [x for x in SCALARS if x not in BAD]
CLEAN = False

def scal():
    return R.choice(SAFE if CLEAN else SCALARS)

def block(depth, ind):
    k = R.random()
    pad = " " * ind
    if depth <= 0 or k < 0.3:
        s = scal()
        if R.random() < 0.15:
            body = R.choice(["line1\n", "a\n b\n\n c\n", "  lead\nx\n", "t\ty\n", "\n\nz\n\n"])
            hdr = R.choice(["|", ">", "|-", ">+", "|2", ">1-", "|+", ">0", "|x"])
            return hdr + "\n" + "".join(pad + "  " + l + "\n" if l else "\n" for l in body.split("\n")[:-1])
        return s + "\n"
    if k < 0.65:
        out = "\n"
        for _ in range(R.randint(0, 4)):
            out += pad + "- " + block(depth - 1, ind + 2).lstrip(" ")
        return out if out != "\n" else "[]\n"
    out = "\n"
    for _ in range(R.randint(0, 4)):
        key = R.choice(KEYS)
        out += pad + key + ":" + " " + block(depth - 1, ind + 2).lstrip(" ")
    return out if out != "\n" else "{}\n"

def flow(depth):
    k = R.random()
    if depth <= 0 or k < 0.35:
        return scal()
    if k < 0.7:
        return "[" + ", ".join(flow(depth - 1) for _ in range(R.randint(0, 4))) + R.choice(["", "", ","]) + "]"
    parts = []
    for _ in range(R.randint(0, 4)):
        if R.random() < 0.1:
            parts.append(R.choice(["? x", "a", "? ", ": v"]))
        else:
            parts.append(R.choice(KEYS) + ": " + flow(depth - 1))
    return "{" + ", ".join(parts) + "}"

SPECIAL = " \t\n\n\n\r-?:,[]{}#&*!|>'\"%@`~.0123456789abcxyTtrunlsNe\\ \u00e9\u65e5\u0085\u2028\u2029\ufeff\u00a0"

def soup(n):
    s = "".join(R.choice(SPECIAL) for _ in range(n))
    if R.random() < 0.1:
        s += R.choice(["\x01", "\x7f", "\x0b", "\x0c", "\x1b", "\ud7ff", "\ufffe", "\U0001F600"])
    return s

def document():
    k = R.random()
    if k < 0.4:
        d = block(R.randint(1, 4), 0).lstrip("\n")
    elif k < 0.7:
        d = flow(R.randint(1, 4)) + "\n"
    elif k < 0.85 and not CLEAN:
        d = soup(R.randint(0, 40))
    else:
        d = R.choice(["---\n", "--- a\n", "...\n", "%YAML 1.2\n---\nx\n", "%YAML 1.1\n%YAML 1.2\n--- y\n",
                      "%YAML 2.0\n--- z\n", "%TAG ! tag:e.com,2000:\n--- !foo 1\n", "%TAG !e! tag:e.com:\n--- !e!b 2\n",
                      "%TAG !e! a\n%TAG !e! b\n---\n", "%FOO bar\n--- 1\n", "%YAML 1.1234567890\n---\n",
                      "--- !!map\na: 1\n", "--- !!seq [1]\n", "&x a: *x\n", "a: &x 1\nb: *x\n",
                      "? [a]\n: 1\n", "? {a: 1}\n: 2\n", "[a: 1, b]\n", "[? x : y]\n", "- - - x\n", "a:\n- 1\n- 2\n",
                      "a: 1\n a: 2\n", "a: 1\na: 2\n", "\ufeffa: 1\n", "\t- x\n", "a: \t1\n", "- |\n \tx\n", "key: 'unterminated\n",
                      "\"multi\n  line\n\n  q\"\n", "'a\n\n  b'\n", "plain\n  more\n\n  lines\n", "a: b: c\n", "- a\n-b\n", "[a, b\n",
                      "{a: 1\n", "]\n", "a\n]\n", "*a\n", "&a\n", "! \n", "!\n", "!<>\n", "!!\n", "&\n", "*\n", "a: [b, c]]\n"])
    if R.random() < (0.05 if CLEAN else 0.3):
        d = mutate(d)
    return d

def mutate(s):
    s = list(s)
    for _ in range(R.randint(1, 4)):
        k = R.random()
        if not s or k < 0.4:
            s.insert(R.randint(0, len(s)), R.choice(SPECIAL))
        elif k < 0.7:
            del s[R.randrange(len(s))]
        else:
            s[R.randrange(len(s))] = R.choice(SPECIAL)
    return "".join(s)

def nested(n, kind):
    if kind == 0:
        return "[" * n + "]" * n
    if kind == 1:
        return "{a: " * n + "1" + "}" * n
    return "".join("  " * i + "- \n" for i in range(n)) + "  " * n + "x\n"

def big():
    k = R.random()
    lines = []
    while sum(len(l) for l in lines) < R.choice([16000, 16384, 17000, 40000]):
        lines.append("k%d: v%d\n" % (len(lines), R.randint(0, 99)))
    s = "".join(lines)
    if k < 0.5:
        p = R.randint(0, len(s))
        s = s[:p] + R.choice(["\x01", "\x7f", "é", "日", "\ufeff", "\x00"]) + s[p:]
    return s

STRS = ["", " ", "a", "a b", "a  b", " a", "a ", "a\n", "\n", "\na", "a\nb", "a\n\n", "a \nb", "a\n b", "\n\n",
        "a\rb", "a\tb", "\u0085", "a\u2028b", "\u2029", "a\u2028", "\u2028\u2028", "\ufeff", "x\ufeff", "\u00a0",
        "\x00", "\x01", "\x7f", "\x1b", "\x07\x08\x0b\x0c", "\"", "\\", "'", "''", "it's", "é", "日本", "\U0001F600",
        "\ud7ff", "\ue000", "\ufffd", "\ufffe", "\uffff", "---", "...", "--- a", "-", "- a", "-a", "?", "? a", "?a", ":",
        ": a", "a:", "a: b", "a:b", "#", "a #b", "a#b", ",", "a,b", "[", "]", "{", "}", "&a", "*a", "!a", "|", ">", "%", "@",
        "`", "~", "null", "Null", "NULL", "true", "False", "yes", "0", "-0", "007", "+1", "0x1f", "0o7", "0b1", "1.5", ".5",
        "1e3", "1e400", ".inf", "-.inf", ".nan", "inf", "nan", "1_000", "9223372036854775808", "1" * 40, "12:30"]

def jstr():
    k = R.random()
    if k < 0.6:
        return R.choice(STRS)
    if k < 0.9:
        return "".join(R.choice(STRS) for _ in range(R.randint(2, 4)))
    return "k" * R.choice([127, 128, 129, 200])

def jval(depth):
    import json
    k = R.random()
    if depth <= 0 or k < 0.45:
        c = R.random()
        if c < 0.15:
            return None
        if c < 0.25:
            return R.random() < 0.5
        if c < 0.4:
            return R.choice([0, 1, -1, 42, 2**63 - 1, -2**63, R.randint(-10**12, 10**12)])
        if c < 0.55:
            return R.choice([0.0, -0.0, 1.5, 0.1, 1e21, 1e-7, 1e16, 1e17, 123456789.125, 5e-324, 1.7976931348623157e308,
                             R.uniform(-1e6, 1e6), R.random() * 10 ** R.randint(-30, 30)])
        return jstr()
    if k < 0.7:
        return [jval(depth - 1) for _ in range(R.randint(0, 4))]
    return {jstr(): jval(depth - 1) for _ in range(R.randint(0, 4))}

def case():
    global CLEAN
    CLEAN = R.random() < 0.5
    k = R.random()
    if k < 0.03:
        t = big()
    elif k < 0.06:
        t = nested(R.choice([1, 50, 126, 127, 128, 129, 200]), R.randint(0, 2))
    elif k < 0.08:
        t = "a: &a [x, x, x, x, x, x, x, x, x]\n" + "".join("%s: &%s [*%s, *%s, *%s, *%s, *%s, *%s, *%s, *%s, *%s]\n" % ((chr(98 + i),) * 2 + (chr(97 + i),) * 9) for i in range(R.randint(1, 8)))
    else:
        docs = [document() for _ in range(R.choice([1, 1, 1, 2, 3]))]
        sep = R.choice(["---\n", "--- ", "...\n---\n", "\n---\n"])
        t = docs[0] + "".join(sep + d for d in docs[1:])
    t = t.replace("\x00", "")
    mode = R.choice("pmr")
    if R.random() < 0.35:
        import json
        mode = "s"
        t = json.dumps(jval(R.randint(0, 4)), ensure_ascii=False)
    if CLEAN and "---" in t and mode != "s":
        mode = "m"
    return mode + " " + t.encode("utf-8", "surrogatepass").hex()

def main():
    global R
    n = int(sys.argv[1]); seed = int(sys.argv[2]); nat = sys.argv[3]
    R = random.Random(seed)
    cases = []
    for _ in range(n):
        c = case()
        try:
            bytes.fromhex(c[2:]).decode("utf-8")
        except UnicodeDecodeError:
            continue
        cases.append(c)
    os.makedirs("/tmp/ym", exist_ok=True)
    path = "/tmp/ym/casos_%d" % seed
    with open(path, "w") as f:
        f.write("\n".join(cases) + "\n")
    here = os.path.dirname(os.path.abspath(__file__))
    env = dict(os.environ)
    env.pop("GITHUB_ACTIONS", None)
    vm = subprocess.run([os.path.expanduser("~/.local/bin/zett"), "run", os.path.join(here, "prueba.titan"), path],
                        capture_output=True, text=True, env=env).stdout.splitlines()
    na = subprocess.run([nat, path], capture_output=True, text=True).stdout.splitlines()
    bad = 0
    for i, c in enumerate(cases):
        a = vm[i] if i < len(vm) else "<falta>"
        b = na[i] if i < len(na) else "<falta>"
        if a == b:
            continue
        bad += 1
        if bad <= 5:
            def dec(x):
                p = x.split(" ", 1)
                return p[0] + " " + (bytes.fromhex(p[1]).decode() if len(p) > 1 else "")
            print("DIFERENCIA caso", i, repr(bytes.fromhex(c[2:]).decode()[:300]), c[0])
            print("  VM:     ", dec(a)[:400])
            print("  nativo: ", dec(b)[:400])
    print("casos", len(cases), "diferencias", bad, "VM", len(vm), "nativo", len(na))

main()
