//! Real regular-expression support powered by the `regex` crate.
//!
//! Exposed as `std::regex::*` in Titan bytecode. Patterns use the standard
//! Rust `regex` syntax (Perl-like, Unicode-aware) and are compiled on every
//! call — a small `OnceLock`-based cache could be added later if needed.

use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegexError {
    #[error("invalid regular expression '{pattern}': {source}")]
    Compile { pattern: String, #[source] source: regex::Error },
}

fn compile(pattern: &str) -> Result<Regex, RegexError> {
    Regex::new(pattern).map_err(|source| RegexError::Compile { pattern: pattern.into(), source })
}

/// Returns `true` if the pattern matches anywhere in `text`.
pub fn is_match(pattern: &str, text: &str) -> Result<bool, RegexError> {
    Ok(compile(pattern)?.is_match(text))
}

/// Returns the first match as a string, or an empty string if none.
pub fn find(pattern: &str, text: &str) -> Result<String, RegexError> {
    Ok(compile(pattern)?.find(text).map(|m| m.as_str().to_string()).unwrap_or_default())
}

/// Returns all non-overlapping matches as an array of strings.
pub fn find_all(pattern: &str, text: &str) -> Result<Vec<String>, RegexError> {
    Ok(compile(pattern)?.find_iter(text).map(|m| m.as_str().to_string()).collect())
}

/// Returns capture groups of the first match (index 0 = whole match).
pub fn captures(pattern: &str, text: &str) -> Result<Vec<String>, RegexError> {
    let re = compile(pattern)?;
    Ok(re.captures(text)
        .map(|caps| (0..caps.len()).map(|i| caps.get(i).map(|m| m.as_str().to_string()).unwrap_or_default()).collect())
        .unwrap_or_default())
}

/// Replaces every occurrence of `pattern` in `text` with `replacement`.
/// Uses `$1`, `$2`, ... in `replacement` to reference capture groups.
pub fn replace_all(pattern: &str, text: &str, replacement: &str) -> Result<String, RegexError> {
    Ok(compile(pattern)?.replace_all(text, replacement).into_owned())
}

/// Splits `text` on every match of `pattern`, returning the pieces.
pub fn split(pattern: &str, text: &str) -> Result<Vec<String>, RegexError> {
    Ok(compile(pattern)?.split(text).map(|piece| piece.to_string()).collect())
}

/// Validates that `pattern` compiles. Cheap way to sanity-check user input.
pub fn is_valid(pattern: &str) -> bool { Regex::new(pattern).is_ok() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_and_finds() {
        assert!(is_match(r"\d+", "abc 123 def").unwrap());
        assert_eq!(find(r"\d+", "abc 123 def").unwrap(), "123");
        assert_eq!(find_all(r"\d+", "12 34 56").unwrap(), vec!["12", "34", "56"]);
    }

    #[test]
    fn captures_groups() {
        let caps = captures(r"(\w+)@(\w+)", "hola juan@ejemplo").unwrap();
        assert_eq!(caps, vec!["juan@ejemplo", "juan", "ejemplo"]);
        assert!(captures(r"x", "y").unwrap().is_empty());
    }

    #[test]
    fn replaces_and_splits() {
        assert_eq!(replace_all(r"\d", "a1b2c3", "*").unwrap(), "a*b*c*");
        assert_eq!(replace_all(r"(\w+)-(\w+)", "foo-bar baz-qux", "$2-$1").unwrap(), "bar-foo qux-baz");
        assert_eq!(split(r"\s+", "hola   mundo bonito").unwrap(), vec!["hola", "mundo", "bonito"]);
    }

    #[test]
    fn rejects_bad_patterns_without_panic() {
        assert!(!is_valid("(unclosed"));
        assert!(is_match("(unclosed", "text").is_err());
    }

    #[test]
    fn unicode_aware() {
        assert_eq!(find_all(r"\p{Letter}+", "hola, mundo 123 café").unwrap(),
                   vec!["hola", "mundo", "café"]);
    }
}
