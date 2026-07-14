//! Thread-safe in-process metrics for server observability.
use std::collections::BTreeMap;
use std::sync::{OnceLock,RwLock};
use thiserror::Error;
#[derive(Error,Debug)]pub enum MetricsError{#[error("invalid metric name")]Name,#[error("metric value must be finite")]Value,#[error("metrics registry poisoned")]Poisoned}
#[derive(Debug,Clone,PartialEq)]pub struct Histogram{pub count:u64,pub sum:f64,pub min:f64,pub max:f64}
#[derive(Debug,Clone,PartialEq,Default)]pub struct Snapshot{pub counters:BTreeMap<String,u64>,pub gauges:BTreeMap<String,f64>,pub histograms:BTreeMap<String,Histogram>}
#[derive(Default)]struct Registry{counters:BTreeMap<String,u64>,gauges:BTreeMap<String,f64>,histograms:BTreeMap<String,Histogram>}
static METRICS:OnceLock<RwLock<Registry>>=OnceLock::new();
fn registry()->&'static RwLock<Registry>{METRICS.get_or_init(||RwLock::new(Registry::default()))}
fn validate(name:&str)->Result<(),MetricsError>{if name.is_empty()||name.len()>200||!name.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'_'|b'.'|b'-')){Err(MetricsError::Name)}else{Ok(())}}
pub fn counter_add(name:&str,amount:u64)->Result<u64,MetricsError>{validate(name)?;let mut metrics=registry().write().map_err(|_|MetricsError::Poisoned)?;let value=metrics.counters.entry(name.into()).or_default();*value=value.saturating_add(amount);Ok(*value)}
pub fn gauge_set(name:&str,value:f64)->Result<(),MetricsError>{validate(name)?;if !value.is_finite(){return Err(MetricsError::Value)}registry().write().map_err(|_|MetricsError::Poisoned)?.gauges.insert(name.into(),value);Ok(())}
pub fn histogram_record(name:&str,value:f64)->Result<(),MetricsError>{validate(name)?;if !value.is_finite(){return Err(MetricsError::Value)}let mut metrics=registry().write().map_err(|_|MetricsError::Poisoned)?;let histogram=metrics.histograms.entry(name.into()).or_insert(Histogram{count:0,sum:0.0,min:value,max:value});histogram.count=histogram.count.saturating_add(1);histogram.sum+=value;histogram.min=histogram.min.min(value);histogram.max=histogram.max.max(value);Ok(())}
pub fn snapshot()->Result<Snapshot,MetricsError>{let metrics=registry().read().map_err(|_|MetricsError::Poisoned)?;Ok(Snapshot{counters:metrics.counters.clone(),gauges:metrics.gauges.clone(),histograms:metrics.histograms.clone()})}
pub fn reset()->Result<(),MetricsError>{*registry().write().map_err(|_|MetricsError::Poisoned)?=Registry::default();Ok(())}
#[cfg(test)]mod tests{use super::*;#[test]fn records_and_snapshots_all_metric_types(){reset().unwrap();counter_add("http.requests",2).unwrap();gauge_set("http.active",3.0).unwrap();histogram_record("http.duration_ms",10.0).unwrap();histogram_record("http.duration_ms",20.0).unwrap();let value=snapshot().unwrap();assert_eq!(value.counters["http.requests"],2);assert_eq!(value.gauges["http.active"],3.0);assert_eq!(value.histograms["http.duration_ms"],Histogram{count:2,sum:30.0,min:10.0,max:20.0});}#[test]fn rejects_invalid_names_and_values(){assert!(counter_add("bad name",1).is_err());assert!(gauge_set("valid",f64::NAN).is_err());}}
