//! SQLite adapter with prepared parameters and typed rows.
use rusqlite::{hooks::{AuthAction,Authorization},Connection,params_from_iter,types::{Value as SqlValue,ValueRef}};
use std::collections::BTreeMap;
use std::path::{Path,PathBuf};
use std::sync::{Arc,Condvar,Mutex};
use std::time::{Duration,Instant};
use thiserror::Error;
#[derive(Debug,Clone,PartialEq)]pub enum DbValue{Null,Integer(i64),Real(f64),Text(String),Blob(Vec<u8>)}
#[derive(Error,Debug)]pub enum DbError{#[error("SQLite error: {0}")]Sqlite(#[from]rusqlite::Error),#[error("transaction is already active")]TransactionActive,#[error("no active transaction")]NoTransaction,#[error("migration versions must be positive, unique, and increasing")]MigrationOrder,#[error("previously applied migration {version} has changed")]MigrationChanged{version:i64},#[error("SQLite pool size must be positive")]PoolSize,#[error("SQLite pool acquisition timed out")]PoolTimeout,#[error("SQLite pool is closed")]PoolClosed,#[error("SQLite pool lock poisoned")]PoolPoisoned}
#[derive(Debug,Clone)]pub struct Migration{pub version:i64,pub name:String,pub sql:String}
#[derive(Debug,Clone,PartialEq,Eq)]pub struct AppliedMigration{pub version:i64,pub name:String,pub checksum:String}
pub struct Database{connection:Connection,in_transaction:bool}
impl Database{
pub fn open(path:impl AsRef<Path>)->Result<Self,DbError>{let connection=Connection::open(path)?;configure(&connection)?;Ok(Self{connection,in_transaction:false})}
pub fn memory()->Result<Self,DbError>{let connection=Connection::open_in_memory()?;configure(&connection)?;Ok(Self{connection,in_transaction:false})}
/// Opens an in-memory database that cannot attach or create filesystem databases.
/// Temporary SQLite storage is also forced into memory so sandboxed callers cannot
/// cause implicit spill files without the Filesystem capability.
pub fn memory_restricted()->Result<Self,DbError>{let database=Self::memory()?;database.connection.execute_batch("PRAGMA temp_store = MEMORY;")?;database.connection.authorizer(Some(|context|match context.action{AuthAction::Attach{..}|AuthAction::Detach{..}|AuthAction::Unknown{..}=>Authorization::Deny,AuthAction::Pragma{pragma_name,..}if pragma_name.eq_ignore_ascii_case("temp_store")||pragma_name.eq_ignore_ascii_case("temp_store_directory")||pragma_name.eq_ignore_ascii_case("data_store_directory")=>Authorization::Deny,_=>Authorization::Allow}));Ok(database)}
pub fn execute(&mut self,sql:&str,params:&[DbValue])->Result<usize,DbError>{Ok(self.connection.execute(sql,params_from_iter(params.iter().map(to_sql)))?)}
pub fn query(&mut self,sql:&str,params:&[DbValue])->Result<Vec<BTreeMap<String,DbValue>>,DbError>{let mut statement=self.connection.prepare(sql)?;let names:Vec<String>=statement.column_names().iter().map(|name|(*name).into()).collect();let rows=statement.query_map(params_from_iter(params.iter().map(to_sql)),|row|{let mut output=BTreeMap::new();for(index,name)in names.iter().enumerate(){output.insert(name.clone(),from_sql(row.get_ref(index)?));}Ok(output)})?;Ok(rows.collect::<Result<Vec<_>,_>>()?)}
pub fn begin(&mut self)->Result<(),DbError>{if self.in_transaction{return Err(DbError::TransactionActive)}self.connection.execute_batch("BEGIN IMMEDIATE")?;self.in_transaction=true;Ok(())}
pub fn commit(&mut self)->Result<(),DbError>{if !self.in_transaction{return Err(DbError::NoTransaction)}self.connection.execute_batch("COMMIT")?;self.in_transaction=false;Ok(())}
pub fn rollback(&mut self)->Result<(),DbError>{if !self.in_transaction{return Err(DbError::NoTransaction)}self.connection.execute_batch("ROLLBACK")?;self.in_transaction=false;Ok(())}
pub fn migrate(&mut self,migrations:&[Migration])->Result<usize,DbError>{if self.in_transaction{return Err(DbError::TransactionActive)}let mut previous=0;for migration in migrations{if migration.version<=previous{return Err(DbError::MigrationOrder)}previous=migration.version;}self.connection.execute_batch("CREATE TABLE IF NOT EXISTS _titan_migrations(version INTEGER PRIMARY KEY,name TEXT NOT NULL,checksum TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;let applied=self.applied_migrations()?.into_iter().map(|migration|(migration.version,migration)).collect::<BTreeMap<_,_>>();for migration in migrations{if let Some(existing)=applied.get(&migration.version){if existing.checksum!=checksum(&migration.sql)||existing.name!=migration.name{return Err(DbError::MigrationChanged{version:migration.version});}}}self.begin()?;let result=(||{let mut count=0;for migration in migrations{if applied.contains_key(&migration.version){continue}self.connection.execute_batch(&migration.sql)?;self.connection.execute("INSERT INTO _titan_migrations(version,name,checksum) VALUES (?,?,?)",rusqlite::params![migration.version,migration.name,checksum(&migration.sql)])?;count+=1;}Ok::<usize,rusqlite::Error>(count)})();match result{Ok(count)=>{self.commit()?;Ok(count)}Err(error)=>{let _=self.rollback();Err(DbError::Sqlite(error))}}}
pub fn applied_migrations(&self)->Result<Vec<AppliedMigration>,DbError>{let mut statement=self.connection.prepare("SELECT version,name,checksum FROM _titan_migrations ORDER BY version")?;let rows=statement.query_map([],|row|Ok(AppliedMigration{version:row.get(0)?,name:row.get(1)?,checksum:row.get(2)?}))?;Ok(rows.collect::<Result<_,_>>()?)}
pub fn last_insert_id(&self)->i64{self.connection.last_insert_rowid()}
pub fn changes(&self)->u64{self.connection.changes()}
pub fn ping(&mut self)->Result<bool,DbError>{let mut stmt=self.connection.prepare("SELECT 1")?;let mut rows=stmt.query([])?;Ok(rows.next()?.is_some())}
}
impl Drop for Database{fn drop(&mut self){if self.in_transaction{let _=self.connection.execute_batch("ROLLBACK");}}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub struct PoolStats{pub maximum:usize,pub total:usize,pub idle:usize,pub checked_out:usize,pub closed:bool}
struct PoolState{idle:Vec<Database>,total:usize,closed:bool}
struct PoolInner{path:PathBuf,maximum:usize,state:Mutex<PoolState>,available:Condvar}
#[derive(Clone)]pub struct Pool(Arc<PoolInner>);
pub struct PooledConnection{database:Option<Database>,pool:Arc<PoolInner>}
impl Pool{pub fn new(path:impl AsRef<Path>,maximum:usize)->Result<Self,DbError>{if maximum==0{return Err(DbError::PoolSize)}Ok(Self(Arc::new(PoolInner{path:path.as_ref().into(),maximum,state:Mutex::new(PoolState{idle:Vec::new(),total:0,closed:false}),available:Condvar::new()})))}
pub fn acquire(&self,timeout:Duration)->Result<PooledConnection,DbError>{
let deadline=Instant::now()+timeout;
loop{
let mut state=self.0.state.lock().map_err(|_|DbError::PoolPoisoned)?;
if state.closed{return Err(DbError::PoolClosed);}
if let Some(database)=state.idle.pop(){return Ok(PooledConnection{database:Some(database),pool:Arc::clone(&self.0)});}
if state.total<self.0.maximum{
state.total+=1;drop(state);
match Database::open(&self.0.path){
Ok(database)=>return Ok(PooledConnection{database:Some(database),pool:Arc::clone(&self.0)}),
Err(error)=>{let mut state=self.0.state.lock().map_err(|_|DbError::PoolPoisoned)?;state.total-=1;self.0.available.notify_one();return Err(error)}
}
}
let remaining=deadline.saturating_duration_since(Instant::now());
if remaining.is_zero(){return Err(DbError::PoolTimeout);}
let (_state_after_wait, wait_result) = self
    .0
    .available
    .wait_timeout(state, remaining)
    .map_err(|_| DbError::PoolPoisoned)?;
if wait_result.timed_out() { return Err(DbError::PoolTimeout); }
}
}
pub fn stats(&self)->Result<PoolStats,DbError>{let state=self.0.state.lock().map_err(|_|DbError::PoolPoisoned)?;Ok(PoolStats{maximum:self.0.maximum,total:state.total,idle:state.idle.len(),checked_out:state.total-state.idle.len(),closed:state.closed})}
pub fn health_check(&self,timeout:Duration)->bool{match self.acquire(timeout){Ok(mut conn)=>conn.ping().unwrap_or(false),Err(_)=>false}}
pub fn close(&self)->Result<(),DbError>{let mut state=self.0.state.lock().map_err(|_|DbError::PoolPoisoned)?;state.closed=true;let idle=state.idle.len();state.idle.clear();state.total-=idle;self.0.available.notify_all();Ok(())}}
impl std::ops::Deref for PooledConnection{type Target=Database;fn deref(&self)->&Self::Target{self.database.as_ref().expect("pooled connection invariant")}}
impl std::ops::DerefMut for PooledConnection{fn deref_mut(&mut self)->&mut Self::Target{self.database.as_mut().expect("pooled connection invariant")}}
impl Drop for PooledConnection{fn drop(&mut self){let Some(database)=self.database.take()else{return};if let Ok(mut state)=self.pool.state.lock(){if state.closed{state.total=state.total.saturating_sub(1)}else{state.idle.push(database)}self.pool.available.notify_one();}}}
fn checksum(sql:&str)->String{let mut hash=0xcbf29ce484222325u64;for byte in sql.as_bytes(){hash^=u64::from(*byte);hash=hash.wrapping_mul(0x100000001b3);}format!("fnv1a64:{hash:016x}")}
fn configure(connection:&Connection)->Result<(),rusqlite::Error>{connection.busy_timeout(std::time::Duration::from_secs(5))?;connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;Ok(())}
fn to_sql(value:&DbValue)->SqlValue{match value{DbValue::Null=>SqlValue::Null,DbValue::Integer(value)=>SqlValue::Integer(*value),DbValue::Real(value)=>SqlValue::Real(*value),DbValue::Text(value)=>SqlValue::Text(value.clone()),DbValue::Blob(value)=>SqlValue::Blob(value.clone())}}
fn from_sql(value:ValueRef<'_>)->DbValue{match value{ValueRef::Null=>DbValue::Null,ValueRef::Integer(value)=>DbValue::Integer(value),ValueRef::Real(value)=>DbValue::Real(value),ValueRef::Text(value)=>DbValue::Text(String::from_utf8_lossy(value).into()),ValueRef::Blob(value)=>DbValue::Blob(value.to_vec())}}
#[cfg(test)]mod tests{use super::*;#[test]fn pool_reuses_connections_and_enforces_timeout(){let path=std::env::temp_dir().join(format!("titan-pool-{}.db",std::process::id()));let _=std::fs::remove_file(&path);let pool=Pool::new(&path,1).unwrap();let mut first=pool.acquire(Duration::from_secs(1)).unwrap();first.execute("CREATE TABLE items(value INTEGER)",&[]).unwrap();assert!(matches!(pool.acquire(Duration::from_millis(5)),Err(DbError::PoolTimeout)));drop(first);let mut reused=pool.acquire(Duration::from_secs(1)).unwrap();reused.execute("INSERT INTO items VALUES (1)",&[]).unwrap();drop(reused);let stats=pool.stats().unwrap();assert_eq!(stats.total,1);assert_eq!(stats.idle,1);pool.close().unwrap();assert!(matches!(pool.acquire(Duration::ZERO),Err(DbError::PoolClosed)));let _=std::fs::remove_file(path);}#[test]fn applies_migrations_once_and_detects_changes(){let mut db=Database::memory().unwrap();let migrations=vec![Migration{version:1,name:"create_users".into(),sql:"CREATE TABLE users(id INTEGER PRIMARY KEY);".into()},Migration{version:2,name:"add_name".into(),sql:"ALTER TABLE users ADD COLUMN name TEXT;".into()}];assert_eq!(db.migrate(&migrations).unwrap(),2);assert_eq!(db.migrate(&migrations).unwrap(),0);assert_eq!(db.applied_migrations().unwrap().len(),2);let changed=vec![Migration{version:1,name:"create_users".into(),sql:"CREATE TABLE changed(id INTEGER);".into()}];assert!(matches!(db.migrate(&changed),Err(DbError::MigrationChanged{version:1})));}#[test]fn prepared_queries_and_transactions_work(){let mut db=Database::memory().unwrap();db.execute("CREATE TABLE users(id INTEGER PRIMARY KEY,name TEXT NOT NULL)",&[]).unwrap();db.begin().unwrap();db.execute("INSERT INTO users(name) VALUES (?)",&[DbValue::Text("Ada".into())]).unwrap();assert_eq!(db.last_insert_id(),1);db.commit().unwrap();let rows=db.query("SELECT id,name FROM users WHERE name=?",&[DbValue::Text("Ada".into())]).unwrap();assert_eq!(rows[0]["id"],DbValue::Integer(1));assert_eq!(rows[0]["name"],DbValue::Text("Ada".into()));}#[test]fn rollback_and_foreign_keys_work(){let mut db=Database::memory().unwrap();db.execute("CREATE TABLE values_(value INTEGER)",&[]).unwrap();db.begin().unwrap();db.execute("INSERT INTO values_ VALUES (?)",&[DbValue::Integer(7)]).unwrap();db.rollback().unwrap();assert!(db.query("SELECT * FROM values_",&[]).unwrap().is_empty());}
#[test]fn restricted_memory_database_blocks_filesystem_escapes(){let attach_path=std::env::temp_dir().join(format!("titan-restricted-attach-{}.db",std::process::id()));let vacuum_path=std::env::temp_dir().join(format!("titan-restricted-vacuum-{}.db",std::process::id()));let _=std::fs::remove_file(&attach_path);let _=std::fs::remove_file(&vacuum_path);let mut db=Database::memory_restricted().unwrap();db.execute("CREATE TABLE safe(value INTEGER)",&[]).unwrap();db.execute("INSERT INTO safe VALUES (1)",&[]).unwrap();assert_eq!(db.query("SELECT value FROM safe",&[]).unwrap()[0]["value"],DbValue::Integer(1));let attach_sql=format!("ATTACH DATABASE '{}' AS escaped",attach_path.display().to_string().replace('\'',"''"));assert!(db.execute(&attach_sql,&[]).is_err());let vacuum_sql=format!("VACUUM INTO '{}'",vacuum_path.display().to_string().replace('\'',"''"));assert!(db.execute(&vacuum_sql,&[]).is_err());assert!(db.execute("PRAGMA temp_store = FILE",&[]).is_err());assert!(!attach_path.exists());assert!(!vacuum_path.exists());}
#[test]fn unrestricted_memory_database_can_attach_files(){let path=std::env::temp_dir().join(format!("titan-unrestricted-attach-{}.db",std::process::id()));let _=std::fs::remove_file(&path);let mut db=Database::memory().unwrap();let sql=format!("ATTACH DATABASE '{}' AS allowed",path.display().to_string().replace('\'',"''"));db.execute(&sql,&[]).unwrap();assert!(path.exists());drop(db);let _=std::fs::remove_file(path);}}
