# TITAN SQLite

TITAN embeds SQLite through rusqlite's bundled build, providing consistent availability on Termux and supported desktop targets.

APIs: `sqlite::open(path)`, `memory()`, `execute(db, sql, params)`, `query(db, sql, params)`, `begin`, `commit`, `rollback`, `last_insert_id`, and `close`.

All values use prepared positional parameters; supported parameter/column types are nil/NULL, int/INTEGER, finite float/REAL, string/TEXT and bytes/BLOB. Query rows become maps keyed by column name. Connections are VM handles protected by mutexes and cannot be serialized.

Connections enable foreign keys, use a five-second busy timeout and request WAL journal mode. Transactions use BEGIN IMMEDIATE, reject nesting, require explicit commit/rollback, and automatically rollback if a connection is dropped while active. Opening file databases requires Filesystem capability; in-memory databases remain available in sandbox mode.

`sqlite::migrate(db, migrations)` accepts ordered maps containing `version`, `name`, and `sql`. It creates `_titan_migrations`, applies all pending migrations atomically, records timestamp and deterministic FNV-1a SQL checksum, skips already-applied versions, rejects duplicate/out-of-order versions, and fails if historical name/SQL changes. `applied_migrations` is available in the Rust driver for tooling.
