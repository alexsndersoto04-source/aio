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

Current multi-file limitation: AST nodes preserve spans but not source-file IDs after project merging. Bytecode maps are exact within each source buffer; attaching canonical source IDs to declarations is the next schema extension before cross-file source breakpoints are advertised as complete.
