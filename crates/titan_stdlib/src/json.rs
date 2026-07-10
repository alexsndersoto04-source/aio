//! Titan Stdlib — JSON.

pub use serde_json::Value as JsonValue;
pub use serde_json::Map as JsonMap;
pub use serde_json::Error as JsonError;

pub fn parse(text: &str) -> Result<JsonValue, JsonError> { serde_json::from_str(text) }
pub fn stringify(value: &JsonValue) -> String { value.to_string() }
pub fn object() -> JsonValue { JsonValue::Object(JsonMap::new()) }
pub fn array() -> JsonValue { JsonValue::Array(Vec::new()) }
pub fn null() -> JsonValue { JsonValue::Null }