//! Thread-safe in-process metrics for server observability.
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock, RwLock};
use thiserror::Error;

const MAX_METRICS_PER_RUNTIME: usize = 4_096;
#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("invalid metric name")]
    Name,
    #[error("metric value must be finite")]
    Value,
    #[error("metrics registry poisoned")]
    Poisoned,
    #[error("metric quota exceeded ({0})")]
    Quota(usize),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub counters: BTreeMap<String, u64>,
    pub gauges: BTreeMap<String, f64>,
    pub histograms: BTreeMap<String, Histogram>,
}
#[derive(Default)]
struct Registry {
    counters: BTreeMap<String, u64>,
    gauges: BTreeMap<String, f64>,
    histograms: BTreeMap<String, Histogram>,
}
fn registries() -> &'static RwLock<HashMap<u64, Arc<RwLock<Registry>>>> {
    static REGISTRIES: OnceLock<RwLock<HashMap<u64, Arc<RwLock<Registry>>>>> = OnceLock::new();
    REGISTRIES.get_or_init(|| RwLock::new(HashMap::new()))
}
fn registry() -> Arc<RwLock<Registry>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registries = crate::native::write_recover(registries());
    Arc::clone(
        registries
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(RwLock::new(Registry::default()))),
    )
}
pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::write_recover(registries())
            .remove(&runtime_id)
            .is_some(),
    )
}
fn validate(name: &str) -> Result<(), MetricsError> {
    if name.is_empty()
        || name.len() > 200
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        Err(MetricsError::Name)
    } else {
        Ok(())
    }
}
fn metric_count(metrics: &Registry) -> usize {
    metrics
        .counters
        .len()
        .saturating_add(metrics.gauges.len())
        .saturating_add(metrics.histograms.len())
}
fn require_metric_slot(metrics: &Registry, exists: bool) -> Result<(), MetricsError> {
    if !exists && metric_count(metrics) >= MAX_METRICS_PER_RUNTIME {
        Err(MetricsError::Quota(MAX_METRICS_PER_RUNTIME))
    } else {
        Ok(())
    }
}
pub fn counter_add(name: &str, amount: u64) -> Result<u64, MetricsError> {
    validate(name)?;
    let registry = registry();
    let mut metrics = registry.write().map_err(|_| MetricsError::Poisoned)?;
    require_metric_slot(&metrics, metrics.counters.contains_key(name))?;
    let value = metrics.counters.entry(name.into()).or_default();
    *value = value.saturating_add(amount);
    Ok(*value)
}
pub fn counter_get(name: &str) -> Result<u64, MetricsError> {
    validate(name)?;
    let registry = registry();
    let metrics = registry.read().map_err(|_| MetricsError::Poisoned)?;
    Ok(metrics.counters.get(name).copied().unwrap_or(0))
}
pub fn gauge_set(name: &str, value: f64) -> Result<(), MetricsError> {
    validate(name)?;
    if !value.is_finite() {
        return Err(MetricsError::Value);
    }
    let registry = registry();
    let mut metrics = registry.write().map_err(|_| MetricsError::Poisoned)?;
    require_metric_slot(&metrics, metrics.gauges.contains_key(name))?;
    metrics.gauges.insert(name.into(), value);
    Ok(())
}
pub fn gauge_get(name: &str) -> Result<f64, MetricsError> {
    validate(name)?;
    let registry = registry();
    let metrics = registry.read().map_err(|_| MetricsError::Poisoned)?;
    Ok(metrics.gauges.get(name).copied().unwrap_or(0.0))
}
pub fn histogram_record(name: &str, value: f64) -> Result<(), MetricsError> {
    validate(name)?;
    if !value.is_finite() {
        return Err(MetricsError::Value);
    }
    let registry = registry();
    let mut metrics = registry.write().map_err(|_| MetricsError::Poisoned)?;
    require_metric_slot(&metrics, metrics.histograms.contains_key(name))?;
    let histogram = metrics.histograms.entry(name.into()).or_insert(Histogram {
        count: 0,
        sum: 0.0,
        min: value,
        max: value,
    });
    histogram.count = histogram.count.saturating_add(1);
    histogram.sum += value;
    histogram.min = histogram.min.min(value);
    histogram.max = histogram.max.max(value);
    Ok(())
}
pub fn snapshot() -> Result<Snapshot, MetricsError> {
    let registry = registry();
    let metrics = registry.read().map_err(|_| MetricsError::Poisoned)?;
    Ok(Snapshot {
        counters: metrics.counters.clone(),
        gauges: metrics.gauges.clone(),
        histograms: metrics.histograms.clone(),
    })
}
pub fn prometheus_export() -> Result<String, MetricsError> {
    let snap = snapshot()?;
    let mut out = String::new();
    for (name, val) in &snap.counters {
        let prom_name = name.replace('.', "_").replace('-', "_");
        out.push_str(&format!("# TYPE {prom_name} counter\n{prom_name} {val}\n"));
    }
    for (name, val) in &snap.gauges {
        let prom_name = name.replace('.', "_").replace('-', "_");
        out.push_str(&format!("# TYPE {prom_name} gauge\n{prom_name} {val}\n"));
    }
    for (name, hist) in &snap.histograms {
        let prom_name = name.replace('.', "_").replace('-', "_");
        out.push_str(&format!("# TYPE {prom_name} summary\n{prom_name}_count {}\n{prom_name}_sum {}\n{prom_name}_min {}\n{prom_name}_max {}\n",hist.count,hist.sum,hist.min,hist.max));
    }
    Ok(out)
}
pub fn reset() -> Result<(), MetricsError> {
    let registry = registry();
    *registry.write().map_err(|_| MetricsError::Poisoned)? = Registry::default();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    fn test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        crate::native::lock_recover(LOCK.get_or_init(|| Mutex::new(())))
    }

    #[test]
    fn records_and_snapshots_all_metric_types() {
        let _guard = test_lock();
        reset().unwrap();
        counter_add("http.requests", 2).unwrap();
        gauge_set("http.active", 3.0).unwrap();
        histogram_record("http.duration_ms", 10.0).unwrap();
        histogram_record("http.duration_ms", 20.0).unwrap();
        let value = snapshot().unwrap();
        assert_eq!(value.counters["http.requests"], 2);
        assert_eq!(value.gauges["http.active"], 3.0);
        assert_eq!(
            value.histograms["http.duration_ms"],
            Histogram {
                count: 2,
                sum: 30.0,
                min: 10.0,
                max: 20.0
            }
        );
    }
    #[test]
    fn exports_prometheus_text_and_gets_values() {
        let _guard = test_lock();
        reset().unwrap();
        counter_add("http.requests", 5).unwrap();
        gauge_set("http.active", 2.5).unwrap();
        histogram_record("http.duration-ms", 100.0).unwrap();
        assert_eq!(counter_get("http.requests").unwrap(), 5);
        assert_eq!(counter_get("missing").unwrap(), 0);
        assert_eq!(gauge_get("http.active").unwrap(), 2.5);
        assert_eq!(gauge_get("missing").unwrap(), 0.0);
        let prom = prometheus_export().unwrap();
        assert!(prom.contains("# TYPE http_requests counter\nhttp_requests 5\n"));
        assert!(prom.contains("# TYPE http_active gauge\nhttp_active 2.5\n"));
        assert!(prom.contains("# TYPE http_duration_ms summary\nhttp_duration_ms_count 1\n"));
    }
    #[test]
    fn rejects_invalid_names_and_values() {
        let _guard = test_lock();
        assert!(counter_add("bad name", 1).is_err());
        assert!(gauge_set("valid", f64::NAN).is_err());
    }
    #[test]
    fn runtimes_cannot_read_or_reset_each_others_metrics() {
        crate::native::with_runtime_context(70_003, || {
            counter_add("private.counter", 9).unwrap();
            gauge_set("private.gauge", 4.5).unwrap();
        });
        crate::native::with_runtime_context(70_004, || {
            assert_eq!(counter_get("private.counter").unwrap(), 0);
            assert_eq!(gauge_get("private.gauge").unwrap(), 0.0);
            counter_add("private.counter", 2).unwrap();
            reset().unwrap();
        });
        crate::native::with_runtime_context(70_003, || {
            assert_eq!(counter_get("private.counter").unwrap(), 9);
            assert_eq!(gauge_get("private.gauge").unwrap(), 4.5);
        });
        assert_eq!(cleanup_runtime(70_003), 1);
        assert_eq!(cleanup_runtime(70_004), 1);
    }
    #[test]
    fn metric_name_quota_is_per_runtime_and_reset_recovers_capacity() {
        let runtime_id = 85_006;
        crate::native::with_runtime_context(runtime_id, || {
            for index in 0..MAX_METRICS_PER_RUNTIME {
                counter_add(&format!("metric.{index}"), 1).unwrap();
            }
            assert!(matches!(
                counter_add("metric.overflow", 1),
                Err(MetricsError::Quota(MAX_METRICS_PER_RUNTIME))
            ));
            counter_add("metric.0", 1).unwrap();
            reset().unwrap();
            counter_add("metric.recovered", 1).unwrap();
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }

}
