# TITAN Debug Information and Debugger Roadmap

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
titan debug --sandbox . -b src/main.titan:4 -b src/math.titan:2
```

At every stop it displays source location, call depth, function identity, locals and operand stack. Interactive commands are `continue`, `step`, `next` (step over), `out`, `print`, and `quit`. With no explicit breakpoint it stops at the entry instruction, making every program immediately debuggable.

The project loader now assigns canonical source paths to every function and impl method before programs are merged. Paths propagate through named functions and nested closures into `.tbc`, debugger frames and `SourceLine` breakpoints. Two files can therefore use the same line number without colliding; the breakpoint matches both canonical source path and one-based line.
