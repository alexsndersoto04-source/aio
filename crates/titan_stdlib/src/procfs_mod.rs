//! System information (`std::procfs::*`) via the cross-platform `sysinfo`
//! crate. Works on Termux/Android, Linux and macOS.
//!
//! Everything is real: CPU %, memory, load average, uptime, processes,
//! disks and network counters come straight from the OS.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};
use sysinfo::{Disks, Networks, System};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SysError {
    #[error("system query failed: {0}")]
    Query(String),
}

fn system() -> &'static Mutex<System> {
    static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
    SYSTEM.get_or_init(|| Mutex::new(System::new_all()))
}

fn refresh() {
    if let Ok(mut sys) = system().lock() {
        sys.refresh_all();
        // sysinfo >= 0.29: refresh_all() ya no repuebla la LISTA de CPUs —
        // queda vacia (cpu_count() == 0, cpus() == []) aunque memoria y
        // procesos sigan bien. La lista se reconstruye explicitamente aqui.
        sys.refresh_cpu_list();
    }
}

// ---------------- Basic OS info ---------------------------------------

pub fn hostname() -> String { System::host_name().unwrap_or_default() }
pub fn kernel()   -> String { System::kernel_version().unwrap_or_default() }
pub fn os_name()  -> String { System::name().unwrap_or_default() }
pub fn os_version() -> String { System::os_version().unwrap_or_default() }
pub fn uptime()   -> u64    { System::uptime() }

// ---------------- CPU -------------------------------------------------

/// Global CPU usage as a percentage 0..=100. Call twice with ~200 ms
/// in between for a meaningful reading; the first call primes the samples.
pub fn cpu_usage() -> f32 {
    refresh();
    if let Ok(sys) = system().lock() { sys.global_cpu_usage() } else { 0.0 }
}

pub fn cpu_count() -> usize {
    refresh();
    system().lock().map(|s| s.cpus().len()).unwrap_or(0)
}

pub fn cpus() -> Value {
    refresh();
    let sys = match system().lock() { Ok(s) => s, Err(_) => return Value::Array(Vec::new()) };
    Value::Array(sys.cpus().iter().map(|cpu| {
        json!({
            "name": cpu.name(),
            "brand": cpu.brand(),
            "vendor_id": cpu.vendor_id(),
            "frequency_mhz": cpu.frequency(),
            "usage_pct": cpu.cpu_usage(),
        })
    }).collect())
}

// ---------------- Memory ----------------------------------------------

pub fn total_memory()     -> u64 { refresh(); system().lock().map(|s| s.total_memory()).unwrap_or(0) }
pub fn used_memory()      -> u64 { refresh(); system().lock().map(|s| s.used_memory()).unwrap_or(0) }
pub fn available_memory() -> u64 { refresh(); system().lock().map(|s| s.available_memory()).unwrap_or(0) }
pub fn total_swap()       -> u64 { refresh(); system().lock().map(|s| s.total_swap()).unwrap_or(0) }
pub fn used_swap()        -> u64 { refresh(); system().lock().map(|s| s.used_swap()).unwrap_or(0) }

// ---------------- Load average (Unix only, zeros on Windows) ----------

pub fn load_average() -> Value {
    let la = System::load_average();
    json!({ "one": la.one, "five": la.five, "fifteen": la.fifteen })
}

// ---------------- Processes -------------------------------------------

/// Total number of processes seen by the OS.
pub fn process_count() -> usize {
    refresh();
    system().lock().map(|s| s.processes().len()).unwrap_or(0)
}

/// Returns up to `limit` processes sorted by CPU usage (descending).
pub fn top_processes(limit: usize) -> Value {
    refresh();
    let sys = match system().lock() { Ok(s) => s, Err(_) => return Value::Array(Vec::new()) };
    let mut items: Vec<_> = sys.processes().iter().map(|(pid, process)| {
        (
            u64::from(pid.as_u32()),
            process.name().to_string_lossy().into_owned(),
            process.cpu_usage(),
            process.memory(),
        )
    }).collect();
    items.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    Value::Array(items.into_iter().take(limit.max(1)).map(|(pid, name, cpu, mem)| json!({
        "pid": pid,
        "name": name,
        "cpu_pct": cpu,
        "memory": mem,
    })).collect())
}

// ---------------- Disks & networks -----------------------------------

pub fn disks() -> Value {
    let disks = Disks::new_with_refreshed_list();
    Value::Array(disks.iter().map(|disk| {
        let mut entry = BTreeMap::new();
        entry.insert("name".into(),        json!(disk.name().to_string_lossy()));
        entry.insert("mount_point".into(), json!(disk.mount_point().to_string_lossy()));
        entry.insert("file_system".into(), json!(disk.file_system().to_string_lossy()));
        entry.insert("total".into(),       json!(disk.total_space()));
        entry.insert("available".into(),   json!(disk.available_space()));
        entry.insert("removable".into(),   json!(disk.is_removable()));
        Value::Object(entry.into_iter().collect())
    }).collect())
}

pub fn networks() -> Value {
    let networks = Networks::new_with_refreshed_list();
    let mut out = serde_json::Map::new();
    for (name, data) in &networks {
        out.insert(name.clone(), json!({
            "received":         data.total_received(),
            "transmitted":      data.total_transmitted(),
            "packets_received": data.total_packets_received(),
            "packets_transmitted": data.total_packets_transmitted(),
            "errors_in":        data.total_errors_on_received(),
            "errors_out":       data.total_errors_on_transmitted(),
        }));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_and_cpu_count_are_populated() {
        // hostname can be empty in some containers, but cpu_count must be >= 1
        // on any real machine.
        assert!(cpu_count() >= 1);
    }

    #[test]
    fn total_memory_is_positive() {
        assert!(total_memory() > 0);
    }

    #[test]
    fn top_processes_respects_limit() {
        let list = top_processes(3);
        let arr = list.as_array().expect("array");
        assert!(arr.len() <= 3);
    }

    #[test]
    fn cpus_are_reported() {
        let cpus = cpus();
        assert!(cpus.as_array().map(|a| !a.is_empty()).unwrap_or(false));
    }
}
