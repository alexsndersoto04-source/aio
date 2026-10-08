#!/usr/bin/env python3
"""Genera selfhost/native/errno.titan: los mensajes de error del sistema
(strerror de glibc, los mismos que muestra Rust en "... (os error N)").
Es un DATO de glibc, como las tablas Unicode; os.strerror llama a strerror.
Uso: python3 selfhost/native/gen_errno.py > selfhost/native/errno.titan"""
import os
print("// GENERADO por selfhost/native/gen_errno.py (strerror de glibc). No editar a mano.")
print("// Mensaje del error del sistema número n (1..133), separados por '|'.")
msgs = [os.strerror(n) for n in range(1, 134)]
assert all("|" not in m and '"' not in m and "{" not in m for m in msgs)
print("fn errno_messages() -> string {")
print('    return "' + "|".join(msgs[:40]) + '|" +')
print('        "' + "|".join(msgs[40:90]) + '|" +')
print('        "' + "|".join(msgs[90:]) + '"')
print("}")
