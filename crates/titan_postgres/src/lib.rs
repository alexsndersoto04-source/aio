//! Synchronous PostgreSQL driver with prepared parameters and typed rows.
use postgres::{
    types::{ToSql, Type},
    Client, NoTls, Row,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Bool(bool),
    Integer(i64),
    Real(f64),
    Text(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
}
#[derive(Error, Debug)]
pub enum PgError {
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("transaction already active")]
    TransactionActive,
    #[error("no active transaction")]
    NoTransaction,
    #[error("unsupported PostgreSQL column type '{0}'")]
    UnsupportedType(String),
    #[error("PostgreSQL pool size must be positive")]
    PoolSize,
    #[error("PostgreSQL pool acquisition timed out")]
    PoolTimeout,
    #[error("PostgreSQL pool is closed")]
    PoolClosed,
    #[error("PostgreSQL pool lock poisoned")]
    PoolPoisoned,
    #[error("migration versions must be positive, unique, and increasing")]
    MigrationOrder,
    #[error("previously applied migration {version} has changed")]
    MigrationChanged { version: i64 },
}
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub sql: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub checksum: String,
}
pub struct Database {
    client: Client,
    in_transaction: bool,
}
impl Database {
    pub fn connect(url: &str) -> Result<Self, PgError> {
        Ok(Self {
            client: Client::connect(url, NoTls)?,
            in_transaction: false,
        })
    }
    pub fn connect_tls(url: &str) -> Result<Self, PgError> {
        let config = (*titan_tls::client_config()).clone();
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(config);
        Ok(Self {
            client: Client::connect(url, tls)?,
            in_transaction: false,
        })
    }
    pub fn execute(&mut self, sql: &str, params: &[DbValue]) -> Result<u64, PgError> {
        let values = parameters(params);
        let refs: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
        let statement = self.client.prepare(sql)?;
        Ok(self.client.execute(&statement, &refs)?)
    }
    pub fn query(
        &mut self,
        sql: &str,
        params: &[DbValue],
    ) -> Result<Vec<BTreeMap<String, DbValue>>, PgError> {
        let values = parameters(params);
        let refs: Vec<&(dyn ToSql + Sync)> = values.iter().map(|value| value.as_ref()).collect();
        let statement = self.client.prepare(sql)?;
        self.client
            .query(&statement, &refs)?
            .into_iter()
            .map(row)
            .collect()
    }
    pub fn begin(&mut self) -> Result<(), PgError> {
        if self.in_transaction {
            return Err(PgError::TransactionActive);
        }
        self.client.batch_execute("BEGIN")?;
        self.in_transaction = true;
        Ok(())
    }
    pub fn commit(&mut self) -> Result<(), PgError> {
        if !self.in_transaction {
            return Err(PgError::NoTransaction);
        }
        self.client.batch_execute("COMMIT")?;
        self.in_transaction = false;
        Ok(())
    }
    pub fn rollback(&mut self) -> Result<(), PgError> {
        if !self.in_transaction {
            return Err(PgError::NoTransaction);
        }
        self.client.batch_execute("ROLLBACK")?;
        self.in_transaction = false;
        Ok(())
    }
    pub fn migrate(&mut self, migrations: &[Migration]) -> Result<usize, PgError> {
        validate_migrations(migrations)?;
        const LOCK: i64 = 0x544954414e;
        self.client
            .query_one("SELECT pg_advisory_lock($1)", &[&LOCK])?;
        let result = self.migrate_locked(migrations);
        let unlock = self
            .client
            .query_one("SELECT pg_advisory_unlock($1)", &[&LOCK]);
        match (result, unlock) {
            (Ok(count), Ok(_)) => Ok(count),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error.into()),
        }
    }
    fn migrate_locked(&mut self, migrations: &[Migration]) -> Result<usize, PgError> {
        self.client.batch_execute("CREATE TABLE IF NOT EXISTS _titan_migrations(version BIGINT PRIMARY KEY,name TEXT NOT NULL,checksum TEXT NOT NULL,applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP)")?;
        let applied = self
            .applied_migrations()?
            .into_iter()
            .map(|migration| (migration.version, migration))
            .collect::<BTreeMap<_, _>>();
        for migration in migrations {
            if let Some(existing) = applied.get(&migration.version) {
                if existing.name != migration.name || existing.checksum != checksum(&migration.sql)
                {
                    return Err(PgError::MigrationChanged {
                        version: migration.version,
                    });
                }
            }
        }
        self.begin()?;
        let result = (|| {
            let mut count = 0;
            for migration in migrations {
                if applied.contains_key(&migration.version) {
                    continue;
                }
                self.client.batch_execute(&migration.sql)?;
                self.client.execute(
                    "INSERT INTO _titan_migrations(version,name,checksum) VALUES ($1,$2,$3)",
                    &[
                        &migration.version,
                        &migration.name,
                        &checksum(&migration.sql),
                    ],
                )?;
                count += 1;
            }
            Ok::<usize, postgres::Error>(count)
        })();
        match result {
            Ok(count) => {
                self.commit()?;
                Ok(count)
            }
            Err(error) => {
                let _ = self.rollback();
                Err(error.into())
            }
        }
    }
    pub fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>, PgError> {
        Ok(self
            .client
            .query(
                "SELECT version,name,checksum FROM _titan_migrations ORDER BY version",
                &[],
            )?
            .into_iter()
            .map(|row| AppliedMigration {
                version: row.get(0),
                name: row.get(1),
                checksum: row.get(2),
            })
            .collect())
    }
    pub fn cancel(&self) -> Result<(), PgError> {
        self.client.cancel_token().cancel_query(NoTls)?;
        Ok(())
    }
    pub fn ping(&mut self) -> Result<bool, PgError> {
        let row = self.client.query_one("SELECT 1", &[])?;
        Ok(row.get::<_, i32>(0) == 1)
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        if self.in_transaction {
            let _ = self.client.batch_execute("ROLLBACK");
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolStats {
    pub maximum: usize,
    pub total: usize,
    pub idle: usize,
    pub checked_out: usize,
    pub closed: bool,
    pub tls: bool,
}
struct PoolState {
    idle: Vec<Database>,
    total: usize,
    closed: bool,
}
struct PoolInner {
    url: String,
    maximum: usize,
    tls: bool,
    state: Mutex<PoolState>,
    available: Condvar,
}
#[derive(Clone)]
pub struct Pool(Arc<PoolInner>);
pub struct PooledConnection {
    database: Option<Database>,
    pool: Arc<PoolInner>,
}
impl Pool {
    pub fn new(url: impl Into<String>, maximum: usize, tls: bool) -> Result<Self, PgError> {
        if maximum == 0 {
            return Err(PgError::PoolSize);
        }
        Ok(Self(Arc::new(PoolInner {
            url: url.into(),
            maximum,
            tls,
            state: Mutex::new(PoolState {
                idle: Vec::new(),
                total: 0,
                closed: false,
            }),
            available: Condvar::new(),
        })))
    }
    pub fn acquire(&self, timeout: Duration) -> Result<PooledConnection, PgError> {
        let deadline = Instant::now() + timeout;
        loop {
            let mut state = self.0.state.lock().map_err(|_| PgError::PoolPoisoned)?;
            if state.closed {
                return Err(PgError::PoolClosed);
            }
            if let Some(database) = state.idle.pop() {
                return Ok(PooledConnection {
                    database: Some(database),
                    pool: Arc::clone(&self.0),
                });
            }
            if state.total < self.0.maximum {
                state.total += 1;
                drop(state);
                let result = if self.0.tls {
                    Database::connect_tls(&self.0.url)
                } else {
                    Database::connect(&self.0.url)
                };
                match result {
                    Ok(database) => {
                        return Ok(PooledConnection {
                            database: Some(database),
                            pool: Arc::clone(&self.0),
                        })
                    }
                    Err(error) => {
                        let mut state = self.0.state.lock().map_err(|_| PgError::PoolPoisoned)?;
                        state.total -= 1;
                        self.0.available.notify_one();
                        return Err(error);
                    }
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(PgError::PoolTimeout);
            }
            let (_state_after_wait, wait) = self
                .0
                .available
                .wait_timeout(state, remaining)
                .map_err(|_| PgError::PoolPoisoned)?;
            if wait.timed_out() {
                return Err(PgError::PoolTimeout);
            }
        }
    }
    pub fn stats(&self) -> Result<PoolStats, PgError> {
        let state = self.0.state.lock().map_err(|_| PgError::PoolPoisoned)?;
        Ok(PoolStats {
            maximum: self.0.maximum,
            total: state.total,
            idle: state.idle.len(),
            checked_out: state.total - state.idle.len(),
            closed: state.closed,
            tls: self.0.tls,
        })
    }
    pub fn health_check(&self, timeout: Duration) -> bool {
        match self.acquire(timeout) {
            Ok(mut conn) => conn.ping().unwrap_or(false),
            Err(_) => false,
        }
    }
    pub fn close(&self) -> Result<(), PgError> {
        let mut state = self.0.state.lock().map_err(|_| PgError::PoolPoisoned)?;
        state.closed = true;
        let idle = state.idle.len();
        state.idle.clear();
        state.total -= idle;
        self.0.available.notify_all();
        Ok(())
    }
}
impl std::ops::Deref for PooledConnection {
    type Target = Database;
    fn deref(&self) -> &Self::Target {
        self.database.as_ref().expect("pool invariant")
    }
}
impl std::ops::DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.database.as_mut().expect("pool invariant")
    }
}
impl Drop for PooledConnection {
    fn drop(&mut self) {
        let Some(database) = self.database.take() else {
            return;
        };
        if let Ok(mut state) = self.pool.state.lock() {
            if state.closed {
                state.total = state.total.saturating_sub(1)
            } else {
                state.idle.push(database)
            }
            self.pool.available.notify_one();
        }
    }
}
fn validate_migrations(migrations: &[Migration]) -> Result<(), PgError> {
    let mut previous = 0;
    for migration in migrations {
        if migration.version <= previous {
            return Err(PgError::MigrationOrder);
        }
        previous = migration.version;
    }
    Ok(())
}
fn checksum(sql: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
fn parameters(values: &[DbValue]) -> Vec<Box<dyn ToSql + Sync>> {
    values
        .iter()
        .map(|value| match value {
            DbValue::Null => Box::new(None::<String>) as Box<dyn ToSql + Sync>,
            DbValue::Bool(value) => Box::new(*value),
            DbValue::Integer(value) => Box::new(*value),
            DbValue::Real(value) => Box::new(*value),
            DbValue::Text(value) => Box::new(value.clone()),
            DbValue::Bytes(value) => Box::new(value.clone()),
            DbValue::Json(value) => Box::new(value.clone()),
        })
        .collect()
}
fn row(row: Row) -> Result<BTreeMap<String, DbValue>, PgError> {
    let mut output = BTreeMap::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = match column.type_() {
            &Type::BOOL => nullable(&row, index, DbValue::Bool)?,
            &Type::INT2 => nullable(&row, index, |value: i16| DbValue::Integer(value.into()))?,
            &Type::INT4 => nullable(&row, index, |value: i32| DbValue::Integer(value.into()))?,
            &Type::INT8 => nullable(&row, index, DbValue::Integer)?,
            &Type::FLOAT4 => nullable(&row, index, |value: f32| DbValue::Real(value.into()))?,
            &Type::FLOAT8 => nullable(&row, index, DbValue::Real)?,
            &Type::TEXT | &Type::VARCHAR | &Type::BPCHAR | &Type::NAME => {
                nullable(&row, index, DbValue::Text)?
            }
            &Type::BYTEA => nullable(&row, index, DbValue::Bytes)?,
            &Type::JSON | &Type::JSONB => nullable(&row, index, DbValue::Json)?,
            other => return Err(PgError::UnsupportedType(other.name().into())),
        };
        output.insert(column.name().into(), value);
    }
    Ok(output)
}
fn nullable<T>(
    row: &Row,
    index: usize,
    convert: impl FnOnce(T) -> DbValue,
) -> Result<DbValue, postgres::Error>
where
    T: for<'a> postgres::types::FromSql<'a>,
{
    Ok(row
        .try_get::<_, Option<T>>(index)?
        .map(convert)
        .unwrap_or(DbValue::Null))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_migration_order_and_checksums_without_server() {
        let valid = vec![
            Migration {
                version: 1,
                name: "one".into(),
                sql: "SELECT 1".into(),
            },
            Migration {
                version: 2,
                name: "two".into(),
                sql: "SELECT 2".into(),
            },
        ];
        assert!(validate_migrations(&valid).is_ok());
        assert!(validate_migrations(&[
            Migration {
                version: 2,
                name: "two".into(),
                sql: "".into()
            },
            Migration {
                version: 1,
                name: "one".into(),
                sql: "".into()
            }
        ])
        .is_err());
        assert_eq!(checksum("SELECT 1"), checksum("SELECT 1"));
        assert_ne!(checksum("SELECT 1"), checksum("SELECT 2"));
    }
    #[test]
    fn validates_pool_lifecycle_without_connecting() {
        assert!(matches!(
            Pool::new("postgresql://localhost/test", 0, false),
            Err(PgError::PoolSize)
        ));
        let pool = Pool::new("postgresql://localhost/test", 1, true).unwrap();
        assert!(pool.stats().unwrap().tls);
        pool.close().unwrap();
        assert!(matches!(
            pool.acquire(Duration::ZERO),
            Err(PgError::PoolClosed)
        ));
    }
    #[test]
    fn builds_rustls_connector() {
        // `titan_tls::client_config()` installs a default rustls CryptoProvider
        // exactly once per process, which is now required by rustls 0.23 when
        // multiple providers (aws-lc-rs, ring) are linked in transitively (for
        // instance through `ureq` used by `std::http_full`).
        let config = (*titan_tls::client_config()).clone();
        let _connector = tokio_postgres_rustls::MakeRustlsConnect::new(config);
    }
    #[test]
    fn parameter_conversion_covers_supported_types() {
        let values = vec![
            DbValue::Null,
            DbValue::Bool(true),
            DbValue::Integer(1),
            DbValue::Real(1.5),
            DbValue::Text("x".into()),
            DbValue::Bytes(vec![1]),
            DbValue::Json(serde_json::json!({"x":1})),
        ];
        assert_eq!(parameters(&values).len(), values.len());
    }
    #[test]
    fn live_postgres_round_trip_when_configured() {
        let Ok(url) = std::env::var("TITAN_POSTGRES_TEST_URL") else {
            return;
        };
        let mut db = Database::connect(&url).unwrap();
        let row = db
            .query("SELECT $1::bigint AS value", &[DbValue::Integer(42)])
            .unwrap();
        assert_eq!(row[0]["value"], DbValue::Integer(42));
    }
}
