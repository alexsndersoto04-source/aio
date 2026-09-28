# Backend LLVM de Titan

Segundo backend del compilador escrito en Titan. El backend propio
(`native/backend.titan`) escribe código máquina x86-64 directamente. Este
(`native/llvm.titan`) escribe **LLVM IR** en texto, y LLVM de verdad (clang/llc)
lo optimiza y lo convierte en código máquina, para x86-64 y (próximamente)
ARM64/AArch64.

```
PROGRAMA.titan ─┐                         (todo en Titan)
                ├─ cargador → typechecker → bytecode ─ native/llvm.titan ─→ PROGRAMA.ll
runtime.titan ──┘                                                            │
                                                     clang -O2 (LLVM real) ←─┘
                                                            │
                                                  ejecutable sin dependencias
```

LLVM es una herramienta externa escrita en C++, como `ld` o `as`. Titan no la
reescribe; la usa. El IR lo genera por completo el compilador en Titan, y el
backend propio sigue existiendo y es el de por defecto (el que usa el punto
fijo).

## Uso

```sh
cd selfhost
zett run build.titan build_llvm.titan build_llvm          # compilador LLVM, nativo
./build_llvm PROGRAMA.titan prog.ll                       # un programa
./build_llvm --lote DIR a.titan b.titan ...               # varios (el runtime se revisa una vez)
clang -O2 -nostdlib -static -fno-pie -no-pie -Wl,-e,_start prog.ll -o prog
```

Sin clang (este sandbox): `python3 native/llvm/llc.py prog.ll prog.o -O2` (LLVM 22
vía llvmlite) y `ld -static -e _start -o prog prog.o`.

Verificación: `bash selfhost/native/llvm/verificar.sh [archivos]` compila cada
programa por LLVM y compara salida, errores y código de salida con `zett run`.
La CI (`.github/workflows/llvm.yml`) hace lo mismo con clang.

## Modelo (el mismo que el backend propio)

| Qué | Backend propio | LLVM |
|---|---|---|
| Valor | 16 bytes: etiqueta + contenido | `%V = {i64, i64}` |
| Pila de operandos | la pila de la máquina | un `alloca` por hueco; la profundidad de cada instrucción se calcula al compilar (`lv_depths`) y mem2reg los pasa a registros |
| Variables locales | `[rbp-16(i+1)]` | un `alloca` por variable |
| Llamada | argumentos apilados, resultado en rax/rdx | `fastcc %V @f(i64 closure, i64 t0, i64 b0, ...)` |
| Closure | objeto en r12 | primer parámetro |
| Globales | r15 | `@G` (4096 bytes) |
| Refcount | g_inc / g_dec / g_drop | `@g_inc` / `@g_dec` (y el epílogo suelta cada local) |
| Strings, esquemas, enums sin contenido | datos estáticos con cuenta -1 | `internal global` con cuenta -1 |
| Errores (desbordamiento, /0, profundidad) | g_overflow… → rt_fatal | `@g_overflow`… → `@g_raise` → rt_fatal |
| Arranque | g_start: pila de 1 GiB con mmap | `_start` (asm de módulo) → `@lv_start` (mmap) → `@lv_main` |

Cada instrucción del bytecode se traduce igual en los dos: mismos caminos
rápidos en línea (enteros, `a[i]`, `x.campo`, igualdad) y mismas llamadas al
runtime en el resto, con las mismas reglas de referencias.

Operaciones de bajo nivel (`std::raw::`):

- aritmética con desbordamiento: `llvm.sadd/ssub/smul.with.overflow`;
- floats: `bitcast` + `fadd`… (IEEE estricto, sin fast-math);
- `fsqrt`: `llvm.sqrt.f64`;
- `ftoint`: comprobación de rango explícita, igual que `cvttsd2si`
  (fuera de rango o NaN = entero mínimo), y así también en ARM64;
- desplazamientos: la cuenta módulo 64 (en LLVM, ≥ 64 sería "poison");
- `syscall`: asm en línea (`syscall` en x86-64; `svc #0` en ARM64, pendiente);
- `cpuid`/`xgetbv`: asm en línea en x86-64; en ARM64 dan 0 (sin extensiones x86);
- `ffma`: `vfmadd231sd` (el runtime solo la usa si la CPU la tiene).

## Estado

- x86-64: en marcha y verificado contra la VM (ver ESTADO.md).
- ARM64 (Termux): pendiente. Falta el mapa de números de llamadas al sistema
  (el runtime usa los de x86-64) y emular las que ARM64 no tiene
  (open→openat, pipe→pipe2, poll→ppoll, fork→clone, dup2→dup3,
  access→faccessat…), más el arranque con `mov x0, sp` / `bl`.
- Punto fijo por LLVM (el compilador compilado por LLVM se compila a sí mismo
  y da el mismo resultado): pendiente.
