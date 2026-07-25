//! YAML 1.2 parsing / emission (`std::yaml::*`) via `serde_yaml`.
//!
//! Roundtrips through `serde_json::Value`, which is the same universal
//! representation the VM already knows how to convert to/from Titan `Value`.
//! This means `.titan` code writes:
//!
//! ```titan
//! let doc = std::yaml::parse(source)     // -> Any (map / array / scalar)
//! let out = std::yaml::stringify(doc)    // -> String
//! ```
//!
//! and the runtime handles bytes/maps automatically.

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum YamlError {
    #[error("YAML parse error: {0}")]
    Parse(String),
    #[error("YAML emit error: {0}")]
    Emit(String),
}

/// Parse a YAML document into a JSON-compatible value. `null`, booleans,
/// numbers, strings, arrays and mappings map directly.
pub fn parse(text: &str) -> Result<Value, YamlError> {
    serde_yaml::from_str(text).map_err(|error| YamlError::Parse(error.to_string()))
}

/// Serialize a value to YAML. Ordering follows insertion order for maps.
pub fn stringify(value: &Value) -> Result<String, YamlError> {
    serde_yaml::to_string(value).map_err(|error| YamlError::Emit(error.to_string()))
}

/// Parse a multi-document YAML stream separated by `---`.
pub fn parse_multi(text: &str) -> Result<Vec<Value>, YamlError> {
    let mut docs = Vec::new();
    for document in serde_yaml::Deserializer::from_str(text) {
        let value = Value::deserialize(document).map_err(|error| YamlError::Parse(error.to_string()))?;
        docs.push(value);
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_scalar_and_map() {
        let doc = parse("name: TITAN\nversion: 0.2\nkeywords: [rust, vm, wasm]\n").unwrap();
        assert_eq!(doc["name"], json!("TITAN"));
        assert_eq!(doc["version"], json!(0.2));
        assert_eq!(doc["keywords"], json!(["rust", "vm", "wasm"]));

        let text = stringify(&doc).unwrap();
        let back = parse(&text).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn parses_nested_structures() {
        let doc = parse("server:\n  host: localhost\n  ports:\n    - 80\n    - 443\n").unwrap();
        assert_eq!(doc["server"]["host"], json!("localhost"));
        assert_eq!(doc["server"]["ports"][1], json!(443));
    }

    #[test]
    fn multi_document_stream() {
        let docs = parse_multi("---\nname: one\n---\nname: two\n").unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0]["name"], json!("one"));
        assert_eq!(docs[1]["name"], json!("two"));
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(parse("::: not yaml :::\n  - broken").is_err());
    }
}
