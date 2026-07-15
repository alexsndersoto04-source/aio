//! SQLite adapter with prepared parameters and typed rows.
use rusqlite::{Connection,params_from_iter,types::{Value as SqlValue,ValueRef}};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;
#[derive(Debug,Clone,PartialEq)]pub enum DbValue{Null,Integer(i64),Real(f64),Text(String),Blob(Vec<u8>)}
#[derive(Error,Debug)]pub enum DbError{#[error("SQLite error: {0}")]Sqlite(#[from]rusqlite::Error),#[error("transaction is already active")]TransactionActive,#[error("no active transaction")]NoTransaction}
pub struct Database{connection:Connection,in_transaction:bool}
impl Database{
pub fn open(path:impl AsRef<Path>)->Result<Self,DbError>{let connection=Connection::open(path)?;configure(&connection)?;Ok(Self{connection,in_transaction:false})}
pub fn memory()->Result<Self,DbError>{let connection=Connection::open_in_memory()?;configure(&connection)?;Ok(Self{connection,in_transaction:false})}
pub fn execute(&mut self,sql:&str,params:&[DbValue])->Result<usize,DbError>{Ok(self.connection.execute(sql,params_from_iter(params.iter().map(to_sql)))?)}
pub fn query(&mut self,sql:&str,params:&[DbValue])->Result<Vec<BTreeMap<String,DbValue>>,DbError>{let mut statement=self.connection.prepare(sql)?;let names:Vec<String>=statement.column_names().iter().map(|name|(*name).into()).collect();let rows=statement.query_map(params_from_iter(params.iter().map(to_sql)),|row|{let mut output=BTreeMap::new();for(index,name)in names.iter().enumerate(){output.insert(name.clone(),from_sql(row.get_ref(index)?));}Ok(output)})?;Ok(rows.collect::<Result<Vec<_>,_>>()?)}
pub fn begin(&mut self)->Result<(),DbError>{if self.in_transaction{return Err(DbError::TransactionActive)}self.connection.execute_batch("BEGIN IMMEDIATE")?;self.in_transaction=true;Ok(())}
pub fn commit(&mut self)->Result<(),DbError>{if !self.in_transaction{return Err(DbError::NoTransaction)}self.connection.execute_batch("COMMIT")?;self.in_transaction=false;Ok(())}
pub fn rollback(&mut self)->Result<(),DbError>{if !self.in_transaction{return Err(DbError::NoTransaction)}self.connection.execute_batch("ROLLBACK")?;self.in_transaction=false;Ok(())}
pub fn last_insert_id(&self)->i64{self.connection.last_insert_rowid()}
pub fn changes(&self)->u64{self.connection.changes()}
}
impl Drop for Database{fn drop(&mut self){if self.in_transaction{let _=self.connection.execute_batch("ROLLBACK");}}}
fn configure(connection:&Connection)->Result<(),rusqlite::Error>{connection.busy_timeout(std::time::Duration::from_secs(5))?;connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;Ok(())}
fn to_sql(value:&DbValue)->SqlValue{match value{DbValue::Null=>SqlValue::Null,DbValue::Integer(value)=>SqlValue::Integer(*value),DbValue::Real(value)=>SqlValue::Real(*value),DbValue::Text(value)=>SqlValue::Text(value.clone()),DbValue::Blob(value)=>SqlValue::Blob(value.clone())}}
fn from_sql(value:ValueRef<'_>)->DbValue{match value{ValueRef::Null=>DbValue::Null,ValueRef::Integer(value)=>DbValue::Integer(value),ValueRef::Real(value)=>DbValue::Real(value),ValueRef::Text(value)=>DbValue::Text(String::from_utf8_lossy(value).into()),ValueRef::Blob(value)=>DbValue::Blob(value.to_vec())}}
#[cfg(test)]mod tests{use super::*;#[test]fn prepared_queries_and_transactions_work(){let mut db=Database::memory().unwrap();db.execute("CREATE TABLE users(id INTEGER PRIMARY KEY,name TEXT NOT NULL)",&[]).unwrap();db.begin().unwrap();db.execute("INSERT INTO users(name) VALUES (?)",&[DbValue::Text("Ada".into())]).unwrap();assert_eq!(db.last_insert_id(),1);db.commit().unwrap();let rows=db.query("SELECT id,name FROM users WHERE name=?",&[DbValue::Text("Ada".into())]).unwrap();assert_eq!(rows[0]["id"],DbValue::Integer(1));assert_eq!(rows[0]["name"],DbValue::Text("Ada".into()));}#[test]fn rollback_and_foreign_keys_work(){let mut db=Database::memory().unwrap();db.execute("CREATE TABLE values_(value INTEGER)",&[]).unwrap();db.begin().unwrap();db.execute("INSERT INTO values_ VALUES (?)",&[DbValue::Integer(7)]).unwrap();db.rollback().unwrap();assert!(db.query("SELECT * FROM values_",&[]).unwrap().is_empty());}}
