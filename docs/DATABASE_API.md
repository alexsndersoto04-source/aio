# TITAN Common Database API

> **Nota:** este documento se escribió para el prototipo en Rust. La API de Titan que describe (funciones, argumentos, resultados)
> sigue siendo la del lenguaje, pero las referencias a la VM, a hilos del sistema, a «capabilities» o a bibliotecas de Rust ya no aplican:
> hoy todo está en Titan y no hay VM al ejecutar. Lo vigente y verificado está en `selfhost/ESTADO.md`.

`std::db` dispatches at runtime across SQLite, PostgreSQL and MySQL connection or pooled-lease handles.

Common operations are `execute(db, sql, params)`, `query`, `begin`, `commit`, `rollback`, `migrate`, and `close`. Rows always become maps and affected/migration counts become integers. Backend-specific connect/pool/last-insert/cancel operations remain in their namespaces because their semantics differ.

The API shares application repository/service logic while preserving backend-specific prepared placeholder syntax (`?` for SQLite/MySQL, `$1` for PostgreSQL) and migration guarantees. It does not rewrite SQL or pretend the dialects are identical. Runtime handle dispatch rejects non-database values explicitly.
