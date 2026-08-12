//! Cross-platform Clipboard and OS Notification System (`std::clipboard`, `std::notify`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_CLIPBOARD_BYTES: usize = 1_048_576;
const MAX_NOTIFICATIONS: usize = 256;
const MAX_NOTIFICATION_TITLE_BYTES: usize = 4_096;
const MAX_NOTIFICATION_BODY_BYTES: usize = 65_536;

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
    if text.len() > MAX_CLIPBOARD_BYTES {
        return false;
    }
    if let Ok(mut srv) = services().lock() {
        srv.clipboard_text = text.to_string();
        return true;
    }
    false
}

pub fn send_notification(title: &str, body: &str) -> bool {
    if title.len() > MAX_NOTIFICATION_TITLE_BYTES || body.len() > MAX_NOTIFICATION_BODY_BYTES {
        return false;
    }
    if let Ok(mut srv) = services().lock() {
        if srv.notifications.len() >= MAX_NOTIFICATIONS {
            return false;
        }
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
    #[test]
    fn clipboard_and_notification_queues_are_bounded() {
        let runtime_id = 85_004;
        crate::native::with_runtime_context(runtime_id, || {
            assert!(!set_text(&"x".repeat(MAX_CLIPBOARD_BYTES + 1)));
            for index in 0..MAX_NOTIFICATIONS {
                assert!(send_notification("title", &index.to_string()));
            }
            assert!(!send_notification("overflow", "overflow"));
            assert_eq!(poll_notifications().len(), MAX_NOTIFICATIONS);
            assert!(send_notification("recovered", "slot"));
            assert_eq!(poll_notifications().len(), 1);
        });
        assert_eq!(cleanup_runtime(runtime_id), 1);
    }

}
