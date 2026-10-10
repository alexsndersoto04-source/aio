#!/usr/bin/env python3
"""Genera artefactos .tbc inválidos (con CRC correcto) a partir de uno válido."""
import copy, json, sys, zlib

src, out = sys.argv[1], sys.argv[2]
env = json.loads(open(src).read().split("\n", 1)[1])
base = env["module"]

def write(name, mod):
    compact = json.dumps(mod, separators=(",", ":"), ensure_ascii=False, sort_keys=True)
    e = {"format_version": 1, "compiler_version": "1.0.0",
         "checksum_crc32": zlib.crc32(compact.encode()) & 0xFFFFFFFF, "module": mod}
    open(f"{out}/{name}.tbc", "w").write("TITAN-BYTECODE 1\n" + json.dumps(e, indent=2, ensure_ascii=False) + "\n")

def ins(name, op):
    m = copy.deepcopy(base)
    f = m["functions"][0]
    f["code"].insert(0, op)
    f["debug_locations"].insert(0, None)
    write(name, m)

def edit(name, fn):
    m = copy.deepcopy(base)
    fn(m)
    write(name, m)

raw = open(src).read()
open(f"{out}/header.tbc", "w").write(raw.replace("TITAN-BYTECODE 1", "TITAN-BYTECODE 2", 1))
open(f"{out}/version.tbc", "w").write(raw.replace('"format_version": 1', '"format_version": 2', 1))
import re
open(f"{out}/crc.tbc", "w").write(re.sub(r'"checksum_crc32": \d+', '"checksum_crc32": 1', raw, 1))
open(f"{out}/trunc.tbc", "w").write(raw[:100])
open(f"{out}/garbage.tbc", "w").write("garbage")
open(f"{out}/empty.tbc", "w").write("")
edit("entry", lambda m: m.__setitem__("entry", 5))
edit("nofunc", lambda m: m.__setitem__("functions", []))
edit("stack", lambda m: m["functions"][0].__setitem__("max_stack", 0))
edit("srcmap", lambda m: m["functions"][0]["code"].insert(0, "Pop"))
ins("str", {"PushStr": 9})
ins("local", {"PushLocal": 9})
ins("jump", {"Jump": 99})
ins("call", {"Call": {"function": 7, "argc": 0}})
ins("call_arity", {"Call": {"function": 0, "argc": 2}})
ins("native", {"CallNative": {"name": "nope", "argc": 0}})
ins("native_arity", {"CallNative": {"name": "std::math::abs", "argc": 3}})
ins("closure_caps", {"MakeClosure": {"function": 0, "captures": [1]}})
ins("closure_missing", {"MakeClosure": {"function": 4, "captures": []}})
ins("array", {"NewArray": 2000000})
ins("struct", {"NewStruct": {"name": "x" * 5000, "fields": []}})
ins("range_argc", {"Call": {"function": 18446744073709551615, "argc": 2}})
