#!/usr/bin/env python3
"""Pruebas del gestor de paquetes nativo (add, keygen, pack, extracción de .tpkg).

Uso:
  python3 paquetes.py CLI                  compara la CLI con paquetes_esperado.json
  python3 paquetes.py --generar ORACULO    regenera paquetes_esperado.json con la CLI de Rust
                                           (solo mientras exista crates/titan_cli)

Las expectativas son el resultado de la CLI de Rust sobre exactamente los mismos casos.
Necesita openssl (para firmar los paquetes de prueba)."""
import subprocess, os, shutil, io, tarfile, gzip, hashlib, base64, json, sys, random, tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
ESPERADO = os.path.join(AQUI, "paquetes_esperado.json")
SEED = bytes(range(32))
W = tempfile.mkdtemp(prefix="paquetes_")
T = tarfile

def norm(s):
    return s.replace("zett ", "titan ").replace("zett:", "titan:").strip()

def run(cli, args, **kw):
    r = subprocess.run([cli] + args, capture_output=True, text=True, **kw)
    return norm(r.stdout + r.stderr), r.returncode

# ---------------------------------------------------------------- add
def casos_add():
    random.seed(11)
    base = '[package]\nname="a"\nversion="1.0.0"\nedition="2021"\n'
    fuzz = json.load(open(os.path.join(AQUI, "paquetes_cadenas.json")))
    reqs = fuzz["reqs"]
    out = [(base, "foo", r) for r in reqs]
    for n in ["foo", "bad name", "", "a-b_c", "x/y", "é", "A1"]:
        out.append((base, n, "^1"))
    mans = [
        '[package]\nname="a"\nversion="1.0.0"\nedition="2021"\n[dependencies]\nzed = {path="../z"}\nalpha={version="1", path="p"}\nfoo={version="0.1"}\n',
        '[package]\nname="a"\nversion="1.x"\nedition="2021"\n',
        '[package]\nname="a"\nversion="1.0.0"\nedition="2021"\nlicense="Apache 2"\n[dependencies]\n"we ird"={version="1"}\n',
        '[package]\nname="a"\nversion="1.0.0"\nedition="2021"\nextra=1\n[x]\ny=2\n',
    ]
    for m in mans:
        out.append((m, "foo", "^1.2"))
    for k in fuzz["strings"]:
        out.append(('[package]\nname="a"\nversion="1.0.0"\nedition="2021"\ndescription=%s\nlicense=%s\n' % (json.dumps(k), json.dumps(k[::-1])), "foo", "1"))
    return out

def ejecutar_add(cli, caso):
    manifiesto, nombre, req = caso
    d = os.path.join(W, "add")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    open(d + "/Titan.toml", "w").write(manifiesto)
    out, rc = run(cli, ["add", nombre, req, "--project", d])
    return {"out": out, "rc": rc, "toml": open(d + "/Titan.toml").read()}

# ---------------------------------------------------------------- extracción offline
def firmar(dig):
    open(W + "/seed.pem", "w").write("-----BEGIN PRIVATE KEY-----\n" + base64.encodebytes(bytes.fromhex("302e020100300506032b657004220420") + SEED).decode() + "-----END PRIVATE KEY-----\n")
    open(W + "/dig.bin", "wb").write(dig)
    subprocess.run(["openssl", "pkeyutl", "-sign", "-inkey", W + "/seed.pem", "-rawin", "-in", W + "/dig.bin", "-out", W + "/sig.bin"], check=True)
    return open(W + "/sig.bin", "rb").read()

def clave_publica():
    return subprocess.run(["openssl", "pkey", "-in", W + "/seed.pem", "-pubout", "-outform", "DER"], capture_output=True).stdout[-32:]

def mk_tar(entries, fmt=T.GNU_FORMAT):
    bio = io.BytesIO()
    with tarfile.open(fileobj=bio, mode="w", format=fmt) as t:
        for e in entries:
            ti = tarfile.TarInfo(e["name"])
            ti.type = e.get("type", T.REGTYPE)
            data = e.get("data", b"")
            if ti.type == T.REGTYPE:
                ti.size = len(data)
            if "linkname" in e:
                ti.linkname = e["linkname"]
            if ti.type == T.DIRTYPE:
                ti.mode = 0o755
            t.addfile(ti, io.BytesIO(data) if ti.type == T.REGTYPE else None)
    return bio.getvalue()

TOML = b'[package]\nname="p"\nversion="1.0.0"\nedition="2021"\n'
GOOD = [dict(name="Titan.toml", data=TOML), dict(name="src/lib.titan", data=b"fn x(){}")]

def casos_extraer():
    c = []
    def add(label, entries=None, raw=None, fmt=T.GNU_FORMAT, **kw):
        c.append(dict(label=label, entries=entries, raw=raw, fmt=fmt, **kw))
    add("valid gnu", GOOD)
    add("valid ustar", GOOD, fmt=T.USTAR_FORMAT)
    add("valid pax", GOOD, fmt=T.PAX_FORMAT)
    add("with dir entries", [dict(name="src", type=T.DIRTYPE), dict(name="src/sub", type=T.DIRTYPE)] + GOOD)
    add("long name gnu", GOOD + [dict(name="a" * 120 + "/" + "b" * 120 + "/f.titan", data=b"1")])
    add("long name pax", GOOD + [dict(name="a" * 120 + "/" + "b" * 120 + "/f.titan", data=b"1")], fmt=T.PAX_FORMAT)
    add("ustar prefix", GOOD + [dict(name="d" * 80 + "/" + "e" * 60 + "/f", data=b"1")], fmt=T.USTAR_FORMAT)
    add("dotdot", GOOD + [dict(name="../evil", data=b"1")])
    add("dotdot mid", GOOD + [dict(name="src/../evil", data=b"1")])
    add("absolute", GOOD + [dict(name="/abs", data=b"1")])
    add("dot prefix", [dict(name="./Titan.toml", data=TOML)])
    add("dot mid", [dict(name="Titan.toml", data=TOML), dict(name="src/./x", data=b"1")])
    add("double slash", [dict(name="Titan.toml", data=TOML), dict(name="src//x", data=b"1")])
    add("symlink", GOOD + [dict(name="ln", type=T.SYMTYPE, linkname="/etc/passwd")])
    add("hardlink", GOOD + [dict(name="hl", type=T.LNKTYPE, linkname="Titan.toml")])
    add("fifo", GOOD + [dict(name="ff", type=T.FIFOTYPE)])
    add("duplicate", GOOD + [dict(name="src/lib.titan", data=b"2")])
    add("dup via dir", GOOD + [dict(name="src", data=b"2")])
    add("file then dir same name", [dict(name="Titan.toml", data=TOML), dict(name="x", data=b"1"), dict(name="x", type=T.DIRTYPE)])
    add("no manifest", [dict(name="src/lib.titan", data=b"1")])
    add("manifest in subdir", [dict(name="p/Titan.toml", data=TOML)])
    add("empty tar", [])
    add("big file 33MiB", GOOD + [dict(name="big", data=b"\0" * (33 * 1024 * 1024))])
    add("file 32MiB exact", GOOD + [dict(name="big", data=b"\0" * (32 * 1024 * 1024))])
    add("5x28MiB total limit", GOOD + [dict(name="f%d" % k, data=b"\0" * (28 * 1024 * 1024)) for k in range(5)])
    add("4x32MiB = 128MiB", GOOD + [dict(name="f%d" % k, data=b"\0" * (32 * 1024 * 1024)) for k in range(4)])
    add("10001 files", GOOD + [dict(name="d/f%d" % k, data=b"") for k in range(10000)])
    add("10000 files", GOOD + [dict(name="d/f%d" % k, data=b"") for k in range(9999)])
    add("tamper", GOOD, tamper=True)
    add("bad signature", GOOD, badsig=True)
    add("no cache offline", GOOD, nocache=True)
    add("garbage tar", raw=b"not a tar at all" * 100)
    add("empty file", GOOD + [dict(name="e", data=b"")])
    add("utf8 name", GOOD + [dict(name="src/ñandú.titan", data=b"1")])
    full = mk_tar(GOOD + [dict(name="src/x", data=b"y" * 700)])
    for n in (0, 1, 100, 511, 512, 513, 520, 603, 700, 1023, 1024, 1100, 1535, 1536, 1700, 2047, 2048, 2200, len(full) - 1025, len(full) - 1024, len(full) - 512, len(full) - 1):
        add("trunc %d" % n, raw=full[:n])
    b = bytearray(full); b[148] = ord("9"); add("cksum digit 9", raw=bytes(b))
    b = bytearray(full); b[150] = ord("1") if b[150] != ord("1") else ord("2"); add("cksum changed digit", raw=bytes(b))
    b = bytearray(full); b[124] = ord("z"); add("size not number", raw=bytes(b))
    return c

def ejecutar_extraer(cli, caso):
    tarb = caso["raw"] if caso["raw"] is not None else mk_tar(caso["entries"], caso["fmt"])
    gz = gzip.compress(tarb, mtime=0)
    sha = hashlib.sha256(gz).hexdigest()
    sig = firmar(hashlib.sha256(gz).digest() if not caso.get("badsig") else b"x" * 32)
    key = clave_publica()
    d = os.path.join(W, "ext")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    open(d + "/Titan.toml", "w").write('[package]\nname="app"\nversion="0.1.0"\nedition="2021"\n[dependencies.p]\nversion="*"\n')
    lock = {"version": 1, "packages": [{"name": "p", "version": "1.0.0", "archive": "https://example.invalid/p.tpkg", "sha256": sha, "signing_key": base64.b64encode(key).decode(), "signature": base64.b64encode(sig).decode(), "dependencies": {}}]}
    json.dump(lock, open(d + "/Titan.remote.lock", "w"))
    if not caso.get("nocache"):
        cd = "%s/.titan/cache/p/1.0.0" % d
        os.makedirs(cd)
        open("%s/%s.tpkg" % (cd, sha), "wb").write(gz + b"x" if caso.get("tamper") else gz)
    out, rc = run(cli, ["fetch", "--offline", "--project", d])
    tree = []
    base = d + "/.titan/packages"
    if os.path.exists(base):
        for root, dirs, files in os.walk(base):
            for n in sorted(dirs):
                tree.append(["D", os.path.relpath(os.path.join(root, n), base)])
            for n in sorted(files):
                p = os.path.join(root, n)
                tree.append(["F", os.path.relpath(p, base), hashlib.sha256(open(p, "rb").read()).hexdigest()])
    tmp = sorted(os.listdir(base + "/p")) if os.path.exists(base + "/p") else []
    return {"out": out, "rc": rc, "tree": sorted(tree), "staging": tmp}

# ---------------------------------------------------------------- keygen / pack
def tar_resumen(tpkg):
    t = tarfile.open(fileobj=io.BytesIO(gzip.decompress(open(tpkg, "rb").read())))
    return [[m.name, m.size, oct(m.mode), m.mtime, m.uid, m.gid, m.type.decode(), hashlib.sha256(t.extractfile(m).read()).hexdigest()] for m in t.getmembers()]

def ejecutar_pack(cli, caso):
    d = os.path.join(W, "pack")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    res = {}
    def r(args):
        out, rc = run(cli, args, cwd=d)
        return out, rc
    # proyecto con árbol variado
    _ = r(["new", "proj"])
    for p, data in [(".git/a", "x"), ("target/b", "y"), (".titan/c", "z"), ("Titan.lock", "l"), ("src/Titan.remote.lock", "m"), ("src/deep/er/f.txt", "data"), ("src/a.txt", "q"), ("src/a-b", "q"), ("src/a/inner", "w"), ("d" * 90 + "/" + "e" * 90 + "/file.titan", "long")]:
        os.makedirs(os.path.dirname(os.path.join(d, "proj", p)), exist_ok=True)
        open(os.path.join(d, "proj", p), "w").write(data)
    seed = bytes(range(7, 39))
    open(d + "/key.bin", "wb").write(seed)
    res["keygen_new"] = r(["keygen", "k1"])
    res["keygen_perm"] = oct(os.stat(d + "/k1").st_mode & 0o777)
    res["keygen_len"] = os.path.getsize(d + "/k1")
    res["keygen_again"] = r(["keygen", "k1"])
    res["keygen_nested"] = r(["keygen", "sub/dir/k3"])
    res["keygen_nested_perm"] = oct(os.stat(d + "/sub/dir/k3").st_mode & 0o777)
    out, rc = r(["pack", "--project", "proj", "--key", "key.bin", "--output", "o.tpkg"])
    lines = out.splitlines()
    res["pack_rc"] = rc
    res["pack_head"] = lines[0]
    res["pack_key"] = lines[2]
    res["tar"] = tar_resumen(d + "/o.tpkg")
    # coherencia: sha256 y firma del propio paquete
    data = open(d + "/o.tpkg", "rb").read()
    sha = hashlib.sha256(data).hexdigest()
    res["sha_ok"] = lines[1] == "sha256=" + sha
    open(d + "/dig.bin", "wb").write(hashlib.sha256(data).digest())
    pub = base64.b64decode(lines[2].split("=", 1)[1] + "=" * 0)
    open(d + "/pub.pem", "w").write("-----BEGIN PUBLIC KEY-----\n" + base64.encodebytes(bytes.fromhex("302a300506032b6570032100") + pub).decode() + "-----END PUBLIC KEY-----\n")
    open(d + "/sig.bin", "wb").write(base64.b64decode(lines[3].split("=", 1)[1]))
    v = subprocess.run(["openssl", "pkeyutl", "-verify", "-pubin", "-inkey", d + "/pub.pem", "-rawin", "-in", d + "/dig.bin", "-sigfile", d + "/sig.bin"], capture_output=True, text=True)
    res["sig_ok"] = v.returncode == 0
    res["pack_again"] = r(["pack", "--project", "proj", "--key", "key.bin", "--output", "o.tpkg"])[0].splitlines()[0]
    res["tmp_leftover"] = os.path.exists(d + "/o.tpkg.tmp")
    # errores
    res["err_key_inside"] = r(["pack", "--project", "proj", "--key", "proj/k", "--output", "o2.tpkg"])
    open(d + "/proj/k", "wb").write(seed)
    res["err_key_inside2"] = r(["pack", "--project", "proj", "--key", "proj/k", "--output", "o2.tpkg"])
    res["err_out_inside"] = r(["pack", "--project", "proj", "--key", "key.bin", "--output", "proj/src/o2.tpkg"])
    open(d + "/short.key", "wb").write(b"abc")
    res["err_short_key"] = r(["pack", "--project", "proj", "--key", "short.key", "--output", "o3.tpkg"])
    res["err_no_key"] = r(["pack", "--project", "proj", "--key", "nokey", "--output", "o3.tpkg"])
    res["err_no_project"] = r(["pack", "--project", "nodir", "--key", "key.bin", "--output", "o3.tpkg"])
    os.symlink("/etc/passwd", d + "/proj/src/ln")
    res["err_symlink"] = r(["pack", "--project", "proj", "--key", "key.bin", "--output", "o4.tpkg"])
    os.remove(d + "/proj/src/ln")
    open(d + "/o5.tpkg.tmp", "w").write("x")
    res["err_tmp_exists"] = r(["pack", "--project", "proj", "--key", "key.bin", "--output", "o5.tpkg"])
    res["err_missing_args"] = r(["pack"])
    res["err_unknown_flag"] = r(["add", "--bogus", "x"])
    res["err_extra_arg"] = r(["fetch", "extra"])
    res["err_value_missing"] = r(["fetch", "--registry"])
    res["help_add"] = r(["add", "--help"])
    res["help_pack"] = r(["pack", "-h"])
    res["fetch_http"] = r(["fetch", "--registry", "http://x", "--project", "proj"])
    res["fetch_offline_nolock"] = r(["fetch", "--offline", "--project", "proj"])
    res["publish_notoken"] = subprocess.run([cli, "publish", "--key", "key.bin", "--project", "proj"], capture_output=True, text=True, cwd=d, env={k: v for k, v in os.environ.items() if k != "TITAN_REGISTRY_TOKEN"})
    res["publish_notoken"] = [norm(res["publish_notoken"].stdout + res["publish_notoken"].stderr), 0]
    return res

def ejecutar_todo(cli):
    return {
        "add": [dict(caso=list(c), res=ejecutar_add(cli, c)) for c in casos_add()],
        "extraer": [dict(label=c["label"], res=ejecutar_extraer(cli, c)) for c in casos_extraer()],
        "pack": ejecutar_pack(cli, None),
    }

def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--generar":
        cli = os.path.abspath(sys.argv[2])
        json.dump(ejecutar_todo(cli), open(ESPERADO, "w"), indent=0, sort_keys=True)
        print("escrito", ESPERADO)
        return 0
    if len(sys.argv) != 2:
        print(__doc__); return 2
    cli = os.path.abspath(sys.argv[1])
    esperado = json.load(open(ESPERADO))
    obtenido = json.loads(json.dumps(ejecutar_todo(cli), sort_keys=True))
    fallos = 0
    for suite in ("add", "extraer"):
        for e, o in zip(esperado[suite], obtenido[suite]):
            if e != o:
                fallos += 1
                print("FALLA", suite, e.get("label") or e["caso"][1:], "\n  esperado", json.dumps(e["res"])[:300], "\n  obtenido", json.dumps(o["res"])[:300])
        print("%-8s %d casos" % (suite, len(esperado[suite])))
    for k in esperado["pack"]:
        if esperado["pack"][k] != obtenido["pack"][k]:
            fallos += 1
            print("FALLA pack", k, "\n  esperado", json.dumps(esperado["pack"][k])[:300], "\n  obtenido", json.dumps(obtenido["pack"][k])[:300])
    print("pack     %d comprobaciones" % len(esperado["pack"]))
    shutil.rmtree(W, ignore_errors=True)
    print("fallos:", fallos)
    return 1 if fallos else 0

if __name__ == "__main__":
    sys.exit(main())
