# Informe técnico del repositorio TITAN

**Fecha del análisis:** 12 de julio de 2026  
**Rama analizada:** `arena/019f57ed-aio`  
**Commit base:** `a2201f2` (`Add-docs`)  
**Estado de Git al iniciar:** limpio, sin cambios pendientes

## 1. Resumen ejecutivo

El repositorio presenta una arquitectura modular razonable para un compilador, pero **actualmente no está en estado compilable ni ejecutable como pipeline completo**. La causa inmediata más grave es que los archivos de los crates `titan_parser` y `titan_vm` fueron guardados con secuencias literales `\n` en vez de saltos de línea reales. En Rust, ambos archivos quedan convertidos de hecho en una única línea de comentario de documentación (`//! ...`), por lo que sus APIs no existen para los demás crates.

Además, aun restaurando esos saltos de línea, el parser, el type checker, el codegen y la VM contienen implementaciones incompletas o incompatibles entre sí. Los ejemplos y el README anuncian capacidades que todavía no están implementadas.

### Evaluación general

| Área | Estado | Observación |
|---|---|---|
| Estructura del workspace | 🟢 Buena base | 15 crates bien separados por responsabilidad |
| Compilación | 🔴 Bloqueada | Parser y VM quedan comentados por corrupción de saltos de línea |
| Pipeline extremo a extremo | 🔴 No funcional | APIs ausentes y etapas incompletas |
| Lexer | 🟡 Parcial | Tokeniza sintaxis básica, pero no reporta correctamente varios errores |
| Parser | 🔴 No disponible | Archivo corrupto; su contenido recuperable también tiene fallos lógicos |
| Type checker | 🔴 Esqueleto | No comprueba expresiones ni devuelve errores reales |
| Codegen/VM | 🔴 Incompleto | Llamadas, retornos, control de flujo y stack no forman un runtime fiable |
| Tests | 🔴 Insuficientes | Solo 3 tests efectivos del lexer; tests de parser/VM están dentro del comentario |
| CI/CD | 🔴 Ausente | No existe `.github/workflows` |
| Documentación | 🟡 Clara pero inexacta | Describe características no implementadas y contiene inconsistencias |
| Licencia | 🔴 Inválida/incompleta | El archivo `LICENSE` termina en “Permission is hereby granted...” |

## 2. Fallos críticos

### 2.1 `titan_parser` y `titan_vm` están serializados incorrectamente

Archivos afectados:

- `crates/titan_parser/src/lib.rs`
- `crates/titan_vm/src/lib.rs`

Datos observados:

- `titan_parser/src/lib.rs`: 14.990 bytes, **0 saltos de línea reales** y 355 secuencias literales `\n`.
- `titan_vm/src/lib.rs`: 8.337 bytes, **0 saltos de línea reales** y 206 secuencias literales `\n`.

Como ambos empiezan con `//!`, Rust interpreta todo el archivo como un comentario. Por tanto:

- `titan_parser::Parser` no existe.
- `titan_vm::Vm`, `titan_vm::Value` y `titan_vm::val_to_string` no existen.
- `titan_cli`, `titan_runtime` y otras dependencias no pueden compilar.
- Los tests escritos dentro de esos archivos no son descubiertos ni ejecutados.

Este parece ser un fallo de escritura/escape, probablemente al generar los archivos desde JSON, shell o alguna herramienta automática.

### 2.2 El parser recuperable no reconoce las palabras clave del lexer

Incluso convirtiendo los `\n` a saltos reales, el parser usa llamadas como:

```rust
self.eat_ident("fn")
self.eat_ident("struct")
self.eat_ident("if")
self.eat_ident("let")
```

Sin embargo, el lexer produce variantes específicas como `TokenKind::Fn`, `TokenKind::Struct`, `TokenKind::If` y `TokenKind::Let`, no `TokenKind::Ident("fn")`. En consecuencia, el parser no reconocería declaraciones ni buena parte del control de flujo.

### 2.3 Manejo de `return` roto

En el contenido recuperable del parser, el primer bloque que detecta `return` construye parcialmente el valor y luego devuelve deliberadamente:

```rust
Err(ParseError::Expected {
    expected: "EndRet".into(),
    ...
})
```

Existe un segundo bloque para `return` más abajo, pero es inalcanzable porque el primero consume antes el token.

### 2.4 Los errores de parseo se ocultan

`parse_program()` acumula errores, sincroniza y finalmente devuelve siempre `Ok(Program { items })`. La CLI solo consulta `parser.errors()` dentro de `map_err`, es decir, únicamente si `parse_program()` devuelve `Err`. En la práctica, un archivo inválido puede continuar hacia type checking y codegen como un programa vacío o parcial.

### 2.5 El type checker no realiza type checking

`crates/titan_typechecker/src/lib.rs`:

- Solo registra los parámetros de funciones con el tipo textual `"unknown"`.
- No recorre cuerpos ni expresiones.
- No valida variables, operadores, retornos, llamadas o tipos declarados.
- `check_program()` siempre devuelve `Ok(())`.
- Los errores `Mismatch` y `UnknownVariable` están definidos pero nunca producidos.

Por eso no son correctas todavía las afirmaciones “full type inference”, seguridad tipo Rust o pattern matching exhaustivo.

### 2.6 La VM y el codegen no implementan llamadas reales

En el contenido recuperable de la VM, `Op::Call` solo hace:

```rust
self.push(Value::Int(0));
```

No hay frames, salto a funciones, paso de argumentos ni retorno al llamador. En codegen, además, una llamada normal compila el callee como global, emite `Call`, elimina argumentos y empuja `nil`. Esto impide recursión y hace imposible ejecutar Fibonacci.

## 3. Fallos funcionales importantes

### 3.1 Las expresiones finales no retornan su valor

`compile_function()` compila el bloque y siempre añade `Op::RetVoid`. `compile_block()` deja el valor de `final_expr` en el stack, pero no emite `Ret`. Por ello, un programa como:

```titan
fn main() { 40 + 2 }
```

terminaría devolviendo `nil`, no `42`. El test recuperable `test_vm_arithmetic` fallaría aunque parser y VM se reparasen.

### 3.2 `build` no genera el archivo `.bc`

`cmd_build()` calcula un nombre de salida y muestra información, pero nunca serializa ni escribe `CompiledModule` al disco. El mensaje `BUILD: input → output` da la impresión de que creó el bytecode cuando no lo hizo.

### 3.3 Los ejemplos no son soportados por la implementación

`examples/fibonacci.titan` necesita:

- Parámetros y tipos de retorno.
- Operador `<=`.
- `return` funcional.
- Llamadas recursivas reales.
- `for`.
- Rangos `0..20`.
- Interpolación de strings.

El parser recuperable solo acepta `fn nombre()` sin parámetros ni retorno, no implementa `for`, rangos o interpolación, y sus llamadas no son ejecutables en la VM.

`examples/hello.titan` también depende de interpolación (`"Hello, {name}!"`), pero el lexer trata todo como una cadena literal y no existe una etapa de interpolación.

### 3.4 Diagnósticos del lexer incompletos

Aunque `LexerError` define errores, `self.errors` nunca recibe elementos:

- Una cadena sin cerrar se devuelve como `StringLit`.
- Un carácter inválido se convierte en `TokenKind::Error`, pero no se añade a `errors`.
- `lex_char()` avanza sin validar fin de archivo, cierre o escapes; una entrada incompleta puede provocar acceso fuera de rango.
- No se implementan escapes en strings o chars.
- Los spans usan índices de `char`, no offsets de bytes; esto puede causar inconsistencias con Unicode.
- El span de EOF usa `end = pos + 1`, fuera del final real de la fuente.

### 3.5 Control de flujo incompleto en codegen

- Los `continue` se guardan, pero nunca se parchean al inicio del bucle.
- `break` con valor ignora su valor.
- Muchas variantes AST caen silenciosamente en `_ => PushNil`.
- Operadores como `%`, `<=`, `>=`, lógicos y bitwise no generan bytecode útil.
- Una asignación a un destino no local puede consumir el valor sin reportar error.
- No hay validación de función `main`; si no existe, `entry` queda en 0 y se ejecutaría la primera función.

### 3.6 GC, runtime, LSP, MIR y macros son prototipos

- El GC no traza referencias entre objetos, no administra memoria real y no elimina roots obsoletos.
- MIR siempre devuelve un módulo sin funciones.
- El scheduler solo administra IDs; no ejecuta fibers.
- LSP `analyze()` siempre devuelve una lista vacía y no implementa el protocolo LSP.
- Macros es únicamente un mapa de strings, sin expansión.
- Estas etapas tampoco están conectadas al pipeline real de la CLI: codegen compila directamente desde AST, pese al diagrama AST → type checker → HIR → MIR → codegen.

## 4. Problemas de red y robustez

`stdlib/net.rs::http_get()` no es un cliente HTTP correcto:

- Acepta URLs `https://`, pero abre TCP plano; no implementa TLS.
- No separa correctamente host, path y query.
- Siempre solicita `GET /`.
- Si hay ruta, puede intentar usarla como parte del hostname.
- Devuelve siempre status `200` sin parsear la respuesta.
- Incluye headers HTTP completos dentro de `body`.
- El parsing de `host:port` no soporta IPv6.

No se observó código `unsafe` de Rust. Eso es positivo, aunque no equivale por sí solo a las garantías de seguridad anunciadas para el lenguaje.

## 5. Higiene del repositorio

### 5.1 Ruta accidental versionada

Existe un archivo rastreado con una ruta anómala:

```text
"/home/user/aio/examples/hello.titan"
```

Esto creó dentro del repositorio un directorio cuyo nombre es una comilla (`"`) y una jerarquía `home/user/aio/...`. El contenido también está escapado y entrecomillado. Debe eliminarse del índice y del árbol.

### 5.2 Licencia incompleta

El archivo `LICENSE` contiene solo:

```text
MIT License

Copyright (c) 2024 Titan Language Contributors

Permission is hereby granted...
```

Esto no es el texto completo de la licencia MIT y puede causar incertidumbre legal. Debe sustituirse por el texto oficial completo.

### 5.3 Inconsistencias de documentación

- Workspace: versión `0.1.0`.
- CLI: anuncia `1.0.0`.
- Spec: se titula `v1.0`.
- README: indica aproximadamente 2.600 líneas Rust; el recuento físico visible es menor y parser/VM están en una sola línea cada uno.
- Arquitectura y README usan extensión `.tt`, pero los ejemplos son `.titan`.
- El README pide hacer checkout de `arena/019f4510-aio`, una rama interna y no apropiada para instrucciones públicas/estables.
- Se afirma “Compilation ✅ Verified”, pero el estado actual contradice esa afirmación.

### 5.4 Falta de automatización

No se encontraron:

- `Cargo.lock`.
- Workflows de GitHub Actions.
- Configuración de `rustfmt` o `clippy`.
- Política de versión mínima de Rust (`rust-version`).
- Tests de integración extremo a extremo.
- Benchmarks o fuzzing para lexer/parser.

## 6. Pruebas realizadas y limitaciones

Se revisaron:

- Estado y estructura Git.
- Todos los manifiestos `Cargo.toml`.
- Código fuente de los 15 crates.
- README, arquitectura, especificación y ejemplos.
- Rutas rastreadas anómalas.
- Tests declarados y marcadores de riesgo.
- Integración estática entre lexer, parser, AST, type checker, codegen, VM y CLI.

Se intentó ejecutar:

```bash
cargo test --workspace --all-targets
```

pero el entorno de análisis no tiene `cargo` ni `rustc` instalados (`cargo: command not found`). Por tanto no se obtuvo una salida real del compilador. Aun así, el fallo de compilación del pipeline es determinista por inspección: los crates parser y VM exportan actualmente cero símbolos, mientras la CLI referencia `Parser`, `Vm`, `Value` y `val_to_string`.

Tests efectivos detectados: únicamente 3 tests del lexer. Los 3 tests del parser y 4 de la VM son texto dentro de archivos comentados y no se compilan.

## 7. Aspectos positivos

- Separación conceptual clara en crates.
- AST amplio y con recursión correctamente encapsulada mediante `Box`.
- Uso consistente de dependencias del workspace.
- Lexer legible y fácil de extender.
- Bytecode y estructura de módulo proporcionan una base comprensible.
- No se detectó Rust `unsafe` ni secretos/credenciales visibles.
- README y documentos comunican bien la visión general, aunque deben distinguir visión futura de funcionalidad actual.

## 8. Plan de reparación recomendado

### Fase 0 — Restaurar un build verificable (prioridad inmediata)

1. Convertir correctamente los `\n` literales de parser y VM a saltos reales, eliminando también artefactos de comillas/JSON.
2. Corregir el parser para consumir `TokenKind::Fn`, `Let`, `If`, etc.
3. Eliminar el bloque roto de `return` y hacer que los errores de parser lleguen a la CLI.
4. Eliminar la ruta accidental `"/home/user/aio/examples/hello.titan"`.
5. Instalar/usar Rust estable y ejecutar `cargo fmt`, `cargo check`, `cargo clippy` y `cargo test`.
6. Añadir CI que ejecute esos comandos en cada push y pull request.

### Fase 1 — MVP honesto y ejecutable

Limitar inicialmente el lenguaje a:

- Funciones sin parámetros o con parámetros simples.
- `int`, `bool`, `string`, `nil`.
- `let`, operadores aritméticos/comparación.
- `if`, `while`, `return`.
- Llamadas y `print`.

Después:

1. Implementar frames reales en VM.
2. Resolver funciones a índices en codegen.
3. Corregir retornos y disciplina del stack.
4. Hacer que `build` escriba un formato de bytecode definido.
5. Añadir tests golden y extremo a extremo para `hello`, aritmética, llamadas, recursión y errores.

### Fase 2 — Type checker y diagnósticos

1. Definir una representación de tipos estructurada, no strings.
2. Implementar scopes por función/bloque.
3. Comprobar expresiones, llamadas, retornos y operadores.
4. Conservar spans reales desde lexer hasta diagnósticos.
5. Reportar múltiples errores sin aceptar silenciosamente programas parciales.

### Fase 3 — Funciones avanzadas

Solo después del MVP estable:

- HIR/MIR conectados de verdad.
- Structs, enums, pattern matching y exhaustividad.
- Rangos, `for`, closures e interpolación.
- GC trazador conectado con valores de VM.
- Concurrencia/fibers/channels.
- LSP real.
- Package manager y stdlib de red robusta con TLS mediante una biblioteca mantenida.

## 9. Prioridad resumida

| Prioridad | Acción |
|---|---|
| P0 | Reparar codificación de `titan_parser` y `titan_vm` |
| P0 | Alinear tokens del lexer con el parser |
| P0 | Añadir CI y conseguir `cargo test` verde |
| P0 | Corregir propagación de errores del parser |
| P1 | Implementar llamadas/frames/retornos reales |
| P1 | Crear tests end-to-end con ejemplos que sí sean soportados |
| P1 | Reemplazar `LICENSE` por MIT completa |
| P1 | Eliminar ruta accidental y corregir README/versiones/extensión |
| P2 | Implementar type checker real |
| P2 | Conectar HIR y MIR o retirarlos temporalmente del pipeline anunciado |
| P3 | GC, concurrencia, LSP, macros, networking y package manager completos |

## 10. Conclusión

TITAN está en una fase de **prototipo inicial**, no en versión 1.0 ni en estado de compilador de sistemas funcional. La visión y la división arquitectónica son útiles, pero el repositorio necesita primero recuperar un núcleo mínimo compilable y probado. El orden correcto es reparar parser/VM, establecer CI, reducir las promesas del README al estado real y construir un MVP extremo a extremo antes de ampliar las características.
