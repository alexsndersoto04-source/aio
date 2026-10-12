# TITAN Debug Information and Debugger Roadmap

> **Nota:** este documento se escribió para el prototipo en Rust. La API de Titan que describe (funciones, argumentos, resultados)
> sigue siendo la del lenguaje, pero las referencias a la VM, a hilos del sistema, a «capabilities» o a bibliotecas de Rust ya no aplican:
> hoy todo está en Titan y no hay VM al ejecutar. Lo vigente y verificado está en `selfhost/ESTADO.md`.

The compiler now emits an instruction-aligned source map in every `BytecodeFunc`. Each `Op` has a parallel optional `SourceLocation` containing UTF-8 byte start/end plus one-based source line/column. Recursive expression compilation preserves parent locations while child operations receive their own spans.

Source maps are serialized in `.tbc` artifacts and validated on load: a non-empty map must contain exactly one entry per instruction. Older version-1 artifacts with no map remain loadable through the serde default, while newly built artifacts carry maps.

The VM debugger consumes these maps during real execution. `Vm::run_debug` invokes a hook before every instruction and emits immutable frame snapshots. `Debugger::channel` provides a thread-safe controller/event pair with:

- function/instruction and source-line breakpoints;
- breakpoints added or removed while paused/running;
- continue and terminate;
- step in, over and out using call depth;
- function ID/name, instruction pointer and source location;
- locals, captures (stored in local slots) and operand-stack inspection;
- structured stopped and terminated events, including runtime errors.

The execution thread blocks while stopped and resumes only after a debugger command, so pause state is real rather than reconstructed after execution.

## Terminal debugger

`titan debug` compiles a source file/project, resolves canonical `path:line` breakpoints, starts the VM on a worker thread and controls it through the debugger channels:

```bash
titan debug . --break src/main.titan:4
titan debug . -b src/main.titan:4 -b src/math.titan:2
```

At every stop it displays source location, call depth, function identity, locals and operand stack:

```text
stopped at /tmp/dep.titan:3:13 [depth 0]
function: main (id 0)
locals:
  [0] = 2
  [1] = nil
stack:
debug [c=continue s=step n=next o=out p=print q=quit]>
```

The accepted commands are exactly `continue`/`c`, `step`/`s`, `next`/`n` (step over), `out`/`o`,
`print`/`p` and `quit`/`q`. Two things worth knowing before using it:

- `print` takes **no argument**: it reprints the current locals and operand stack on one line
  (`locals=[Int(2), Nil]`). There is no `print <expression>`; anything after the command makes the
  line an unknown command.
- Locals are shown **by slot index** (`[0]`, `[1]`, …), not by name. The slot order is the
  declaration order of the function's locals and parameters.

Anything else answers `unknown debugger command`, and end of input is treated as `quit`
(`debuggee terminated: execution terminated by debugger`). With no explicit breakpoint it stops at the entry instruction, making every program immediately debuggable.

The project loader now assigns canonical source paths to every function and impl method before programs are merged. Paths propagate through named functions and nested closures into `.tbc`, debugger frames and `SourceLine` breakpoints. Two files can therefore use the same line number without colliding; the breakpoint matches both canonical source path and one-based line.
