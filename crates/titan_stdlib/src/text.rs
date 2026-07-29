//! Unicode-aware text utilities. Operations use Unicode scalar values (`char`),
//! not bytes; user-perceived grapheme segmentation is intentionally not faked.

pub fn length(text: &str) -> usize { text.chars().count() }
pub fn is_empty(text: &str) -> bool { text.is_empty() }
pub fn codepoints(text: &str) -> Vec<u32> { text.chars().map(u32::from).collect() }
pub fn from_codepoints(values: &[u32]) -> Option<String> { values.iter().map(|v| char::from_u32(*v)).collect() }
pub fn reverse(text: &str) -> String { text.chars().rev().collect() }
pub fn contains_ignore_ascii_case(text: &str, needle: &str) -> bool { text.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) }
pub fn lines(text: &str) -> Vec<&str> { text.lines().collect() }
pub fn words(text: &str) -> Vec<&str> { text.split_whitespace().collect() }
pub fn repeat(text: &str, count: usize) -> String { text.repeat(count) }
pub fn trim(text: &str) -> &str { text.trim() }
pub fn truncate(text: &str, max_chars: usize, suffix: &str) -> String {
    if length(text) <= max_chars { return text.into(); }
    if max_chars == 0 { return String::new(); }
    let suffix_len = length(suffix);
    if suffix_len >= max_chars { return suffix.chars().take(max_chars).collect(); }
    let mut result: String = text.chars().take(max_chars - suffix_len).collect(); result.push_str(suffix); result
}
pub fn pad_start(text: &str, width: usize, fill: char) -> String { let missing = width.saturating_sub(length(text)); format!("{}{}", fill.to_string().repeat(missing), text) }
pub fn pad_end(text: &str, width: usize, fill: char) -> String { let missing = width.saturating_sub(length(text)); format!("{}{}", text, fill.to_string().repeat(missing)) }
pub fn capitalize(text: &str) -> String { let mut chars = text.chars(); chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default() }
pub fn escape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() { match c { '&' => result.push_str("&amp;"), '<' => result.push_str("&lt;"), '>' => result.push_str("&gt;"), '"' => result.push_str("&quot;"), '\'' => result.push_str("&#39;"), _ => result.push(c) } }
    result
}
pub fn slugify(text: &str) -> String {
    let mut result = String::new(); let mut separator = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() { if separator && !result.is_empty() { result.push('-'); } result.push(c); separator = false; }
        else { separator = true; }
    }
    result
}
/// Parse a signed decimal integer. Returns None if the string is
/// empty, malformed, or out of i64 range. Trims whitespace first.
pub fn parse_int(text: &str) -> Option<i64> { text.trim().parse::<i64>().ok() }

/// Parse a floating-point number (accepts "3.14", "-0.5", "1e10", etc.)
pub fn parse_float(text: &str) -> Option<f64> { text.trim().parse::<f64>().ok() }

/// Character-based substring. Titan's other string ops are Unicode-aware,
/// so we count chars not bytes. Bounds are clamped to [0, len] and if
/// end < start we return empty string.
pub fn substring(text: &str, start: usize, end: usize) -> String {
    let len = length(text);
    let start = start.min(len);
    let end = end.min(len).max(start);
    text.chars().skip(start).take(end - start).collect()
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect(); let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ac) in a.chars().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, bc) in b.iter().enumerate() { current[j + 1] = (previous[j + 1] + 1).min(current[j] + 1).min(previous[j] + usize::from(ac != *bc)); }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)] mod tests { use super::*; #[test] fn unicode_operations() { assert_eq!(length("año"), 3); assert_eq!(reverse("año"), "oña"); assert_eq!(truncate("abcdef", 4, "…"), "abc…"); } #[test] fn escaping_and_distance() { assert_eq!(escape_html("<a & b>"), "&lt;a &amp; b&gt;"); assert_eq!(levenshtein("kitten", "sitting"), 3); } }
