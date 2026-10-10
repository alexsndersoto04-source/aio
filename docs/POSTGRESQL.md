# TITAN PostgreSQL

> **Nota:** este documento se escribió para el prototipo en Rust. La API de Titan que describe (funciones, argumentos, resultados)
> sigue siendo la del lenguaje, pero las referencias a la VM, a hilos del sistema, a «capabilities» o a bibliotecas de Rust ya no aplican:
> hoy todo está en Titan y no hay VM al ejecutar. Lo vigente y verificado está en `selfhost/ESTADO.md`.

`titan_postgres` uses the maintained synchronous `postgres` protocol driver. It provides URL connection, server-prepared statements, positional parameters, typed row maps, explicit transactions, rollback-on-drop, affected-row counts and PostgreSQL cancel tokens.

Supported values: NULL, bool, int2/int4/int8, float4/float8, text/varchar/bpchar/name, bytea and JSON/JSONB. Unknown PostgreSQL types return an explicit error rather than lossy strings.

TITAN APIs are `postgres::connect`, `connect_tls`, `execute`, `query`, `begin`, `commit`, `rollback`, `cancel`, and `close`. Connection handles are mutex protected, require Network capability, work across tasks, convert rows to maps and reject JSON serialization.

`connect_tls` uses Titan's own TLS implementation (`std_tls.titan`) with SNI and hostname/certificate-chain validation; there is no insecure certificate verifier. Plain `connect` remains available for trusted local Unix/TCP deployments.

`Pool::new(url, maximum, tls)` provides bounded plain/TLS connection reuse, condition-variable acquisition with timeout, RAII return, stats and controlled close. Connections are opened outside the global lock and failed opens release reserved capacity. Idle connections close immediately during shutdown; checked-out connections close when returned.

TITAN exposes `postgres::pool(url, maximum, tls)`, `acquire(pool, timeout_ms)`, `pool_stats(pool)`, and `pool_close(pool)`. Acquire returns `Option::Some(Postgres)` or `Option::None`; the lease supports all query/transaction/cancel APIs and returns automatically when `postgres::close(lease)` is called.

`postgres::migrate(db, migrations)` uses a process-independent PostgreSQL advisory lock so multiple application instances cannot migrate concurrently. It records version/name/FNV checksum/timestamp in `_titan_migrations`, applies pending versions atomically, skips prior versions and rejects changed history or invalid ordering.

The live integration test runs when `TITAN_POSTGRES_TEST_URL` is configured; it is skipped otherwise because PostgreSQL is an external service. VM pool handles are the next integration block.
