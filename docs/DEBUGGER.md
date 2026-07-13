# TITAN Debug Information and Debugger Roadmap

The compiler now emits an instruction-aligned source map in every `BytecodeFunc`. Each `Op` has a parallel optional `SourceLocation` containing UTF-8 byte start/end plus one-based source line/column. Recursive expression compilation preserves parent locations while child operations receive their own spans.

Source maps are serialized in `.tbc` artifacts and validated on load: a non-empty map must contain exactly one entry per instruction. Older version-1 artifacts with no map remain loadable through the serde default, while newly built artifacts carry maps.

This is the foundation for the debugger currently being integrated. The next debugger block consumes these maps to provide:

- function/instruction and source-line breakpoints;
- pause/continue;
- step in/over/out;
- call-stack frames;
- locals, captures and operand-stack inspection;
- structured stopped/continued/terminated events.

Current multi-file limitation: AST nodes preserve spans but not source-file IDs after project merging. Bytecode maps are exact within each source buffer; attaching canonical source IDs to declarations is the next schema extension before cross-file source breakpoints are advertised as complete.
