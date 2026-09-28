#!/usr/bin/env python3
"""Herramienta de desarrollo: LLVM IR (.ll) -> objeto (.o) con el LLVM real
que trae llvmlite (verificar, optimizar, generar código). Sirve en el sandbox,
que no tiene clang; en CI se usa clang directamente.

Uso: llc.py ENTRADA.ll SALIDA.o|SALIDA.s [--target x86_64|aarch64] [-O0|-O1|-O2|-O3]
"""
import sys
import llvmlite.binding as llvm

def main():
    args = sys.argv[1:]
    target, opt, pos = "x86_64", 2, []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--target":
            target = args[i + 1]; i += 2; continue
        if a.startswith("-O"):
            opt = int(a[2:]); i += 1; continue
        pos.append(a); i += 1
    src, out = pos
    llvm.initialize_all_targets()
    llvm.initialize_all_asmprinters()
    llvm.initialize_native_asmparser()
    triple = {"x86_64": "x86_64-unknown-linux-gnu", "aarch64": "aarch64-unknown-linux-gnu"}[target]
    mod = llvm.parse_assembly(open(src).read())
    mod.triple = triple
    mod.verify()
    tm = llvm.Target.from_triple(triple).create_target_machine(opt=opt, reloc="static", codemodel="small")
    if opt > 0:
        pto = llvm.create_pipeline_tuning_options(speed_level=opt)
        pb = llvm.create_pass_builder(tm, pto)
        pb.getModulePassManager().run(mod, pb)
    if out.endswith(".s"):
        # Ensamblador en texto (para aarch64 aquí no hay parser de ensamblador
        # en llvmlite, así que no puede emitir el objeto con asm en línea).
        open(out, "w").write(tm.emit_assembly(mod))
    else:
        open(out, "wb").write(tm.emit_object(mod))

if __name__ == "__main__":
    main()
