#!/usr/bin/env python3
"""Verificacion estatica del fix Phase 34 (sin compilar Rust).

1. La tabla NATIVES no debe tener nombres duplicados (lookup() = primera coincidencia).
2. Cada llamada std::mod::fn(...) en los .titan debe existir en la tabla
   y recibir exactamente la cantidad de argumentos declarada.
3. Cada brazo de VM usado por Phase 34 existe en la tabla.
"""
import re, sys, pathlib

root = pathlib.Path(__file__).parent
errors = []

# ---------- 1) Tabla de firmas ----------
src = (root / "crates/titan_stdlib/src/native.rs").read_text()
entries = re.findall(r'native!\("([^"]+)",\s*\[([^\]]*)\]', src)
seen, sig = {}, {}
for name, params in entries:
    n = 0 if not params.strip() else len([p for p in params.split(",") if p.strip()])
    if name in seen:
        errors.append(f"DUPLICADO en NATIVES: {name} ({seen[name]} args vs {n} args)")
    else:
        seen[name] = n
        sig[name] = n
print(f"[1] NATIVES: {len(entries)} entradas, {len(sig)} unicas")

# ---------- 2) Llamadas std::* en archivos .titan ----------
call_re = re.compile(r'(std::[a-z_][a-z_0-9]*::[a-z_][a-z_0-9]*)\s*\(')

def strip_strings(text):
    # Tambien elimina comentarios // hasta el fin de linea (falsos positivos).
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == '/' and i + 1 < n and text[i + 1] == '/':
            while i < n and text[i] != '\n':
                i += 1
            continue
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == '\\':
                    i += 1
                i += 1
            out.append('""')
        else:
            out.append(c)
        i += 1
    return ''.join(out)

def count_args(text, start):
    depth, commas, has_content, i = 0, 0, False, start
    pairs = {')': '(', ']': '[', '}': '{'}
    opens = set(pairs.values())
    while i < len(text):
        c = text[i]
        if c in opens:
            depth += 1
            if depth > 1 and not c.isspace():
                has_content = True
        elif c in pairs:
            depth -= 1
            if depth == 0:
                return 0 if not has_content else commas + 1, i
        elif c == ',' and depth == 1:
            commas += 1
        if depth == 1 and not c.isspace() and c != '(':
            has_content = True
        if depth >= 1 and c != '(' and not c.isspace():
            has_content = True
        i += 1
    return None, None

# Funciones registradas fuera de la tabla NATIVES (hardcoded en el typechecker
# + ops dedicadas de la VM): sqlite, mysql, postgres, tls.
tc = (root / "crates/titan_typechecker/src/lib.rs").read_text()
for extra_name in re.findall(r'functions\.insert\("(std::[^"]+)"', tc):
    sig.setdefault(extra_name, -1)  # -1 = solo comprobar existencia, no aridad

checked = 0
for tf in list((root / "examples").rglob("*.titan")) + list((root / "stdlib").rglob("*.titan")):
    text = strip_strings(tf.read_text())
    for m in call_re.finditer(text):
        name = m.group(1)
        argc, _ = count_args(text, m.end() - 1)
        checked += 1
        if name not in sig:
            errors.append(f"{tf.name}: funcion desconocida {name}")
        elif argc is not None and sig[name] >= 0 and argc != sig[name]:
            errors.append(f"{tf.name}: {name} declarada con {sig[name]} args pero la llamada pasa {argc}")
print(f"[2] {checked} llamadas std::* verificadas en examples/ y stdlib/")

# ---------- 3) Brazos Phase 34 de la VM existen en la tabla ----------
vm = (root / "crates/titan_vm/src/native.rs").read_text()
arms34 = re.findall(r'cfg\(feature = "(?:process_mod|collections_mod|datetime_ext_mod)"\)\]\s*"(std::[^"]+)"', vm)
missing = [a for a in arms34 if a not in sig]
if missing:
    errors.append(f"brazos VM sin firma en tabla: {missing}")
print(f"[3] {len(arms34)} brazos Phase 34 de la VM con firma en la tabla")

# ---------- 4) Features declaradas en titan_vm ----------
toml = (root / "crates/titan_vm/Cargo.toml").read_text()
for feat in ("process_mod", "collections_mod", "datetime_ext_mod"):
    if f'{feat} ' not in toml and f'{feat}\n' not in toml and f'"{feat}"' not in toml:
        errors.append(f"titan_vm/Cargo.toml no declara feature {feat}")
print("[4] features Phase 34 declaradas en titan_vm/Cargo.toml")

print()
if errors:
    print("❌ PROBLEMAS ENCONTRADOS:")
    for e in errors:
        print("  -", e)
    sys.exit(1)
print("✅ TODO VERIFICADO — tabla limpia, todas las llamadas cuadran")
