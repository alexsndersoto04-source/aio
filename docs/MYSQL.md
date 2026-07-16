# TITAN MySQL

`titan_mysql` uses the maintained Rust `mysql` protocol driver with `minimal-rust` and `rustls-tls`, avoiding OpenSSL. It provides URL connections, prepared positional parameters, typed rows, affected-row counts, last insert ID and explicit transactions with rollback-on-drop.

Supported values include NULL, signed/unsigned integers, float/double, text/binary bytes, DATE/DATETIME and TIME. MySQL bytes are returned as UTF-8 text when valid and bytes otherwise. Live integration runs when `TITAN_MYSQL_TEST_URL` is configured; it is skipped without an external MySQL server.

TITAN APIs are `mysql::connect`, `execute`, `query`, `begin`, `commit`, `rollback`, `last_insert_id`, and `close`. Connection handles are mutex protected, require Network capability, convert rows to maps and reject JSON serialization. The driver is built with rustls support.

`Pool::new(url, maximum)` provides bounded reuse, condition-variable acquisition timeout, RAII return, stats and controlled close. Connections open outside the lock; failed opens release capacity; idle connections close immediately during shutdown and checked-out connections close when returned.

TITAN exposes `mysql::pool(url, maximum)`, `acquire(pool, timeout_ms)`, `pool_stats(pool)`, and `pool_close(pool)`. Acquire returns `Option::Some(Mysql)` or `Option::None`; leases support all query/transaction APIs and return automatically through `mysql::close(lease)`. Migrations are the next MySQL block.
