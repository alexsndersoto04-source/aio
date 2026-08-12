//! UUID generation and parsing (`std::uuid::*`) backed by the `uuid` crate.

use uuid::Uuid;

/// Generates a random UUID v4 (RFC 4122).
pub fn v4() -> String {
    Uuid::new_v4().to_string()
}

/// Generates a UUID v7 (time-ordered, monotonic). Great for database primary keys.
pub fn v7() -> String {
    Uuid::now_v7().to_string()
}

/// Returns `true` if `text` is a syntactically valid UUID.
pub fn is_valid(text: &str) -> bool {
    Uuid::parse_str(text).is_ok()
}

/// Normalizes a UUID to its canonical lowercase hyphenated form; returns
/// the empty string when input is not a valid UUID.
pub fn normalize(text: &str) -> String {
    Uuid::parse_str(text)
        .map(|u| u.to_string())
        .unwrap_or_default()
}

/// Returns the "nil" UUID (all zeros).
pub fn nil() -> String {
    Uuid::nil().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_is_valid_and_random() {
        let a = v4();
        let b = v4();
        assert_ne!(a, b);
        assert!(is_valid(&a));
        assert_eq!(a.len(), 36);
    }

    #[test]
    fn v7_is_valid_and_monotonic_ish() {
        let a = v7();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = v7();
        assert!(is_valid(&a) && is_valid(&b));
        // v7 encodes time in the high bits, so successive calls compare as
        // less-than-or-equal at the string level.
        assert!(a.as_str() <= b.as_str());
    }

    #[test]
    fn nil_and_validation() {
        assert_eq!(nil(), "00000000-0000-0000-0000-000000000000");
        assert!(!is_valid("not-a-uuid"));
        assert_eq!(
            normalize("550E8400-E29B-41D4-A716-446655440000"),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(normalize("nope"), "");
    }
}
