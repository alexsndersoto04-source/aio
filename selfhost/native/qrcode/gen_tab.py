#!/usr/bin/env python3
# Genera selfhost/native/std_qrcode_tab.titan a partir de las fuentes de los
# crates (qrcode 0.14.1 y fdeflate 0.3.7) extraídos en un directorio.
# Uso: python3 gen_tab.py /tmp/cr > ../std_qrcode_tab.titan
import re, sys, ast

cr = sys.argv[1]
qr = open(f"{cr}/qrcode-0.14.1/src/ec.rs").read()
bits = open(f"{cr}/qrcode-0.14.1/src/bits.rs").read()
canvas = open(f"{cr}/qrcode-0.14.1/src/canvas.rs").read()
opt = open(f"{cr}/qrcode-0.14.1/src/optimize.rs").read()
fdt = open(f"{cr}/fdeflate-0.3.7/src/tables.rs").read()
fdc = open(f"{cr}/fdeflate-0.3.7/src/compress.rs").read()

def rust_bytes(lit):
    # lit: contenido entre comillas de un b"..." de Rust
    lit = re.sub(r"\\\n\s*", "", lit)
    return ast.literal_eval('b"' + lit + '"')

def block(src, name):
    m = re.search(r"static " + name + r"\b[^=]*=\s*(.*?)\n\];?", src, re.S)
    if not m:
        m = re.search(r"const " + name + r"\b[^=]*=\s*(.*?)\n\];?", src, re.S)
    return m.group(1)

def nocomments(s):
    return re.sub(r"//[^\n]*", "", s)

def ints(s):
    return [int(x, 0) for x in re.findall(r"0x[0-9a-fA-F]+|\d+", nocomments(s))]

def hexs(vals, w):
    for v in vals:
        assert 0 <= v < 16 ** w, (v, w)
    return "".join(format(v, "0%dx" % w) for v in vals)

out = []
def emit(name, s, comment):
    out.append(f"// {comment}\nfn {name}() -> string {{\n    \"{s}\"\n}}\n")

# GF(256)
exp = rust_bytes(re.search(r'static EXP_TABLE: &\[u8\] = b"(.*?)";', qr, re.S).group(1))
log = rust_bytes(re.search(r'static LOG_TABLE: &\[u8\] = b"(.*?)";', qr, re.S).group(1))
assert len(exp) == 256 and len(log) == 256
x = 1
for i in range(255):
    assert exp[i] == x
    assert log[x] == i
    x <<= 1
    if x & 256:
        x ^= 0x11d
emit("qr_exp_hex", hexs(exp, 2), "EXP_TABLE (ec.rs), 256 bytes")
emit("qr_log_hex", hexs(log, 2), "LOG_TABLE (ec.rs), 256 bytes")

gp = block(qr, "GENERATOR_POLYNOMIALS")
polys = [rust_bytes(p) for p in re.findall(r'b"(.*?)"', gp, re.S)]
assert len(polys) == 70
# Comprobación independiente: generador = prod (x - a^i), en forma logarítmica
def gmul(a, b):
    if a == 0 or b == 0:
        return 0
    return exp[(log[a] + log[b]) % 255]
for n, p in enumerate(polys):
    if n == 0:
        assert p == b""
        continue
    g = [1]
    for i in range(n):
        ng = [0] * (len(g) + 1)
        for j, c in enumerate(g):
            ng[j] ^= c
            ng[j + 1] ^= gmul(c, exp[i])
        g = ng
    if len(p) == n:
        assert [log[c] for c in g[1:]] == list(p), n
offs = []
flat = b""
for p in polys:
    offs.append(len(flat))
    flat += p
emit("qr_gen_hex", hexs(flat, 2), "GENERATOR_POLYNOMIALS concatenados (ec.rs)")
emit("qr_gen_off_hex", hexs(offs, 4), "desplazamiento de cada polinomio en qr_gen_hex")

ecb = ints(block(qr, "EC_BYTES_PER_BLOCK"))
assert len(ecb) == 44 * 4
emit("qr_ecb_hex", hexs(ecb[:160], 2), "EC_BYTES_PER_BLOCK, versiones normales 1..40 x [L,M,Q,H]")
dbb = ints(block(qr, "DATA_BYTES_PER_BLOCK"))
assert len(dbb) == 44 * 16
emit("qr_dbb_hex", hexs(dbb[:640], 2), "DATA_BYTES_PER_BLOCK (tam1, n1, tam2, n2), versiones 1..40 x [L,M,Q,H]")
dl = ints(block(bits, "DATA_LENGTHS"))
assert len(dl) == 44 * 4
emit("qr_dlen_hex", hexs(dl[:160], 4), "DATA_LENGTHS (bits.rs), versiones 1..40 x [L,M,Q,H]")
# coherencia: DATA_LENGTHS == 8 * bytes de datos
for v in range(40):
    for l in range(4):
        a, b, c, d = dbb[(v * 4 + l) * 4:(v * 4 + l) * 4 + 4]
        assert dl[v * 4 + l] == 8 * (a * b + c * d)

ap = block(canvas, "ALIGNMENT_PATTERN_POSITIONS")
rows = re.findall(r"&\[([^\]]*)\]", ap)
assert len(rows) == 34
aflat = []
for r in rows:
    v = ints(r)
    aflat.append(len(v))
    aflat += v
emit("qr_align_hex", hexs(aflat, 2), "ALIGNMENT_PATTERN_POSITIONS v7..40: cuenta y posiciones")
vi = ints(block(canvas, "VERSION_INFOS"))
assert len(vi) == 34
emit("qr_vinfo_hex", hexs(vi, 5), "VERSION_INFOS v7..40")
fi = ints(block(canvas, "FORMAT_INFOS_QR"))
assert len(fi) == 32
emit("qr_finfo_hex", hexs(fi, 4), "FORMAT_INFOS_QR")
for name in ["FORMAT_INFO_COORDS_QR_MAIN", "FORMAT_INFO_COORDS_QR_SIDE", "VERSION_INFO_COORDS_BL", "VERSION_INFO_COORDS_TR"]:
    v = [int(t) for t in re.findall(r"-?\d+", nocomments(block(canvas, name)).split("=", 1)[-1])]
    # (x, y) con desplazamiento +16 para que sean positivos
    emit("qr_" + name.lower() + "_hex", hexs([t + 16 for t in v], 2), name + " (x+16, y+16)")

st = block(opt, "STATE_TRANSITION")
states = {"Init": 0, "Numeric": 1, "Alpha": 2, "Byte": 3, "KanjiHi12": 4, "KanjiHi3": 5, "Kanji": 6}
actions = {"Idle": 0, "Numeric": 1, "Alpha": 2, "Byte": 3, "Kanji": 4, "KanjiAndSingleByte": 5}
tr = re.findall(r"\(State::(\w+),\s*Action::(\w+)\)", st)
assert len(tr) == 70
emit("qr_trans_hex", "".join("%d%d" % (states[s], actions[a]) for s, a in tr), "STATE_TRANSITION (optimize.rs): estado/10 y acción")

hl = ints(block(fdt, "HUFFMAN_LENGTHS"))
assert len(hl) == 286
codes = [0] * 286
code = 0
for ln in range(1, 17):
    for i, l in enumerate(hl):
        if l == ln:
            codes[i] = int(format(code, "016b")[::-1], 2) >> (16 - ln)
            code += 1
    code <<= 1
assert code == 2 << 16
emit("fd_hlen_hex", hexs(hl, 1), "fdeflate HUFFMAN_LENGTHS")
emit("fd_hcode_hex", hexs(codes, 4), "fdeflate HUFFMAN_CODES (compute_codes)")
ls = ints(block(fdt, "LENGTH_TO_SYMBOL"))
le = ints(block(fdt, "LENGTH_TO_LEN_EXTRA"))
assert len(ls) == 256 and len(le) == 256
emit("fd_lsym_hex", hexs([s - 257 for s in ls], 2), "fdeflate LENGTH_TO_SYMBOL - 257")
emit("fd_lext_hex", hexs(le, 1), "fdeflate LENGTH_TO_LEN_EXTRA")
hdr = ints(re.search(r"const HEADER: \[u8; 54\] = \[(.*?)\];", fdc, re.S).group(1))
assert len(hdr) == 54
emit("fd_header_hex", hexs(hdr, 2), "fdeflate Compressor::write_headers HEADER (53 bytes + 5 bits)")

print("// GENERADO por selfhost/native/qrcode/gen_tab.py desde qrcode 0.14.1 y")
print("// fdeflate 0.3.7 (selfhost/fuentes). No editar a mano.\n")
print("\n".join(out), end="")
