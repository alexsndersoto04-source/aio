//! Cross-platform Clipboard and OS Notification System (`std::clipboard`, `std::notify`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

struct SystemServices {
    clipboard_text: String,
    notifications: Vec<(String, String)>,
}

fn service_registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<SystemServices>>>> {
    static SERVICES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<SystemServices>>>>> = OnceLock::new();
    SERVICES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn services() -> Arc<Mutex<SystemServices>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut registry = crate::native::lock_recover(service_registry());
    Arc::clone(registry.entry(runtime_id).or_insert_with(|| {
        Arc::new(Mutex::new(SystemServices {
            clipboard_text: String::new(),
            notifications: Vec::new(),
        }))
    }))
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(service_registry())
            .remove(&runtime_id)
            .is_some(),
    )
}

pub fn get_text() -> String {
    if let Ok(srv) = services().lock() {
        return srv.clipboard_text.clone();
    }
    String::new()
}

pub fn set_text(text: &str) -> bool {
    if let Ok(mut srv) = services().lock() {
        srv.clipboard_text = text.to_string();
        return true;
    }
    false
}

pub fn send_notification(title: &str, body: &str) -> bool {
    if let Ok(mut srv) = services().lock() {
        srv.notifications
            .push((title.to_string(), body.to_string()));
        return true;
    }
    false
}

pub fn poll_notifications() -> Vec<(String, String)> {
    if let Ok(mut srv) = services().lock() {
        return srv.notifications.drain(..).collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_and_notifications() {
        assert!(set_text("Hello from TITAN clipboard"));
        assert_eq!(get_text(), "Hello from TITAN clipboard");

        assert!(send_notification("Update", "Compilation finished"));
        let notifs = poll_notifications();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].0, "Update");
        assert_eq!(notifs[0].1, "Compilation finished");
    }
}
