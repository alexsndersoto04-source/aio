# TITAN MySQL

`titan_mysql` uses the maintained Rust `mysql` protocol driver with `minimal-rust` and `rustls-tls`, avoiding OpenSSL. It provides URL connections, prepared positional parameters, typed rows, affected-row counts, last insert ID and explicit transactions with rollback-on-drop.

Supported values include NULL, signed/unsigned integers, float/double, text/binary bytes, DATE/DATETIME and TIME. MySQL bytes are returned as UTF-8 text when valid and bytes otherwise. Live integration runs when `TITAN_MYSQL_TEST_URL` is configured; it is skipped without an external MySQL server.

The first block is the standalone driver. VM handles, pooling, migrations and explicit TLS policy are subsequent MySQL blocks before production readiness.
