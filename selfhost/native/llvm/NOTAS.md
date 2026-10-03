# Backend LLVM de Titan

Segundo backend del compilador escrito en Titan. El backend propio
(`native/backend.titan`) escribe código máquina x86-64 directamente. Este
(`native/llvm.titan`) escribe **LLVM IR** en texto, y LLVM de verdad (clang/llc)
lo optimiza y lo convierte en código máquina para x86-64 y ARM64/AArch64.

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

Si no hay `clang`, `verificar.sh` prueba primero `python3 -m ziglang cc`
(Clang/LLVM empaquetado con Zig) y después `llvmlite` mediante
`native/llvm/llc.py` más `ld`. Para ejecutar la suite, basta con proporcionar
una de esas cadenas de herramientas.

Verificación: `bash selfhost/native/llvm/verificar.sh [archivos]` compila cada
programa por LLVM y compara salida, errores y código de salida con `zett run`.
Los casos que escriben archivos reciben bases temporales separadas para VM y
LLVM. `image_webp_unsupported` se valida contra
`selfhost/native/image/webp_unsupported.expected`, porque el rechazo nativo de
animaciones difiere de la VM, que lee el primer cuadro. La CI
(`.github/workflows/llvm.yml`) hace la misma comparación con clang.

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
- `syscall`: en x86-64 se emite `syscall`; en AArch64, las llamadas POSIX de x86-64 pasan por `rt_sys_arm64`. La primitiva `std::raw::svc` emite la instrucción real `svc #0` en AArch64.
- `cpuid`/`xgetbv`: asm en línea en x86-64; en ARM64 dan 0 (sin extensiones x86);
- `ffma`: `vfmadd231sd` (el runtime solo la usa si la CPU la tiene).

## Estado

- x86-64: los 301 archivos `.titan` actuales pasaron con `-O2` usando
  `python3 -m ziglang cc` (clang 21.1.0) y el binario precompilado de Zett.
  La batería terminó en una sola invocación antes de añadir el control explícito
  de timeout (28 min 28 s), y se volvió a completar en 7 lotes con el control
  nuevo: 300 coincidieron con la VM en salida, errores y código de salida;
  `image_webp_unsupported` coincidió con el rechazo nativo esperado. Hubo 0
  diferencias, 0 tiempos agotados y 0 programas no admitidos. El intento
  monolítico con el control nuevo excedió el límite local de 30 minutos; la
  repetición en lotes sí terminó. El intento monolítico anterior con llvmlite
  también había agotado ese límite.
- El verificador usa `TIMEOUT` (60 s por defecto) y separa los timeouts de las
  diferencias. Una prueba real `loop {}` con `TIMEOUT=1` quedó reportada como
  tiempo agotado (no como coincidencia); otra prueba que termina normalmente
  con código 124 sí coincidió. `resumen_ci.py` también anota los timeouts.
- Punto fijo por LLVM (`punto_fijo.sh`): el compilador en Titan compilado por
  LLVM compila `build.titan` y da exactamente el mismo ejecutable que el
  compilador normal (byte a byte), y el backend LLVM compilado por LLVM
  escribe exactamente el mismo IR. Revalidado en esta sesión con `ziglang cc`
  (clang 21.1.0, -O2): `titanc_ref` y `titanc_por_llvm` dieron el SHA-256
  `84b33e340d747c4fd89425f8f2ce584c27adbd5ccb6d54c52f9099a9e19dbde4`; ambos
  backends dieron para `build.ll` el SHA-256
  `d8aca45f0d96d3c3cea1ae1acf1c5200073a7adab43d5d0d7d35bb9fac1c5b8a`. De
  paso: el compilador hecho por LLVM tarda 8 s en compilarse a sí mismo, frente
  a 45 s el del backend propio.
- Optimizaciones adicionales: 16 pruebas representativas pasaron con `-O0` y
  con `-O3` usando el mismo clang; en cada nivel, 15 coincidieron con la VM y
  `image_webp_unsupported` coincidió con el rechazo nativo esperado. En ambos
  niveles hubo 0 diferencias y 0 programas no admitidos.
- ARM64/AArch64 en Linux: `sys_arm64.titan` traduce cada llamada al sistema
  del runtime (escrito con los números y estructuras de x86-64) a la de ARM64:
  números distintos, llamadas que ARM64 no tiene (open→openat, stat→newfstatat,
  poll→ppoll, fork→clone, dup2→dup3, mkdir→mkdirat, rename→renameat,
  rmdir/unlink→unlinkat, readlink→readlinkat), `struct stat` y
  `struct epoll_event` con otra forma, y banderas de open con otro valor. Lo
  que no está en la tabla devuelve -ENOSYS. El arranque usa `mov x0, sp` / `bl`.
  La última ejecución completa registrada en una máquina ARM64 real dio
  241 / 241 idénticos. En esta sesión, `arm64_objetos.sh` compiló los 301 IR
  de la batería actual a objetos AArch64 con `ziglang cc` (clang 21.1.0, -O2):
  301 generados, 0 rechazados. `readelf` confirmó `Machine: AArch64` en los 301.
  Además, 17 casos representativos (aritmética, colecciones, errores, JSON,
  WebP y procesos) enlazaron como ejecutables estáticos AArch64 (`ET_EXEC`, sin
  intérprete dinámico). No se ejecutaron localmente. La CI usa
  `ubuntu-24.04-arm` para enlazar y ejecutar la batería.

### Lo que Rust hace distinto en cada CPU

La VM de Rust no siempre da lo mismo en x86-64 y en ARM64: `f64::min` y
`f64::max` se compilan a `minsd`/`maxsd` en x86-64 (con valores iguales gana el
segundo) y a `fminnm`/`fmaxnm` en ARM64 (-0 < +0). Para dar exactamente lo
mismo que la VM en cada máquina, el runtime pregunta `std::raw::arch()`
(0 = x86-64, 1 = ARM64; lo fija cada backend al compilar). Hoy lo usa
`std::metrics::histogram_record`. Al pasar a Titan otras nativas con `min`/`max`
de floats (audio, gui…), hay que hacer lo mismo.

