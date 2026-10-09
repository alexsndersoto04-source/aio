# Tareas y canales

Los ejecutables nativos de Titan ejecutan las tareas como **fibras cooperativas**: cada tarea tiene su propia pila (32 MiB
reservados con `mmap`, con página de guarda) y todas corren en el mismo hilo del sistema. El cambio de una a otra es real
(cambio de pila en ensamblador, para x86-64 y para ARM64). No hay paralelismo entre tareas.

```titan
fn trabajo(n: int) -> int {
    n * 2
}

fn main() {
    let t = spawn || { trabajo(21) }
    print(join(t))          // 42
}
```

## Canales

`channel(capacidad)` devuelve `(Sender, Receiver)`. Capacidad 0 es un encuentro (el emisor espera a que alguien reciba).

```titan
fn productor(tx: Sender, n: int) -> int {
    let mut i = 0
    while i < n {
        send(tx, i * 10)
        i += 1
    }
    n
}

fn main() {
    let (tx, rx) = channel(2)
    let t = spawn || { productor(tx, 5) }
    let mut k = 0
    while k < 5 {
        print(recv(rx))
        k += 1
    }
    print(join(t))
}
```

| Función | Qué hace |
|---|---|
| `spawn \|\| { ... }` | Crea una tarea y devuelve su handle. La tarea no empieza hasta que el hilo principal (u otra tarea) se bloquea. |
| `join(t)` | Espera el resultado; propaga el error de la tarea. Solo se puede llamar una vez por tarea. |
| `join_timeout(t, ms)` | `Some(resultado)` o `None` si vence el plazo. |
| `cancel(t)` | Marca la tarea como cancelada; se observa cuando arranca o vuelve de un punto de cesión. `join` da `task cancelled`. |
| `channel(n)`, `send`, `recv`, `recv_timeout(rx, ms)` | Canales acotados. |
| `select([rx1, rx2], ms)` | `Some((índice, valor))` del primero que esté listo, o `None` al vencer. |

## Cuándo se cede el control

Otra tarea solo se ejecuta cuando la actual se bloquea: `join`, `join_timeout`, `recv`, `recv_timeout`, `select`, `send` con
el canal lleno (o sin capacidad) y `std::time::sleep_ms`. **Una tarea que calcula sin bloquearse nunca cede**: el resto no avanza
hasta que ella termine o se bloquee. Las lecturas bloqueantes de red, archivos o stdin tampoco ceden.

Si todas las tareas están bloqueadas y ninguna tiene plazo, el programa termina con
`RUNTIME ERROR: deadlock: every task is blocked and none can ever wake up` (código 1). Al terminar `main`, las tareas pendientes
no se ejecutan ni se esperan.

## Límites

- 256 tareas, 1024 canales, capacidad máxima 65536.
- Sin paralelismo: para aprovechar varios núcleos hay que lanzar procesos (`std::process`).
- `std::runtime::spawn_quota` (tope de memoria por tarea) no existe en los ejecutables nativos.

## Inspección del runtime (`std::runtime`)

- `allocated_bytes()` bytes reales del montón en uso; `gc_live_count()` = `allocated_bytes / 64`.
- El runtime usa **conteo de referencias**, no hay recolector: `gc_collect()` devuelve 0, `gc_set_threshold(n)` solo guarda el valor
  y `memory_limit()` devuelve `-1` (sin tope).
- `active_tasks()` tareas registradas; `heap_dump(ruta)` escribe un JSON con esas cifras; `benchmark(n, || { ... })` mide un cierre
  (`iterations`, `total_ms`, `ns_per_op`, `ops_per_sec`).

Pruebas: `selfhost/tests/native/tareas_*.titan` y `error_tarea_*`/`error_canal_*`; se comparan con el backend x86-64 y con LLVM/ARM64
(`selfhost/tests/arm64_diff.sh`).
