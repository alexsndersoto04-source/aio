# TITAN PostgreSQL

`titan_postgres` uses the maintained synchronous `postgres` protocol driver. It provides URL connection, server-prepared statements, positional parameters, typed row maps, explicit transactions, rollback-on-drop, affected-row counts and PostgreSQL cancel tokens.

Supported values: NULL, bool, int2/int4/int8, float4/float8, text/varchar/bpchar/name, bytea and JSON/JSONB. Unknown PostgreSQL types return an explicit error rather than lossy strings.

TITAN APIs are `postgres::connect`, `connect_tls`, `execute`, `query`, `begin`, `commit`, `rollback`, `cancel`, and `close`. Connection handles are mutex protected, require Network capability, work across tasks, convert rows to maps and reject JSON serialization.

`connect_tls` uses rustls 0.23, WebPKI roots, SNI and hostname/certificate-chain validation through `tokio-postgres-rustls`; there is no insecure certificate verifier and no OpenSSL dependency. Plain `connect` remains available for trusted local Unix/TCP deployments.

The live integration test runs when `TITAN_POSTGRES_TEST_URL` is configured; it is skipped otherwise because PostgreSQL is an external service. Pooling is the next PostgreSQL block.
