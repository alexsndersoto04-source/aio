//! JSON parsing, construction, querying, and merge operations.

pub use serde_json::{
    Error as JsonError, Map as JsonMap, Number as JsonNumber, Value as JsonValue,
};

pub fn parse(text: &str) -> Result<JsonValue, JsonError> {
    serde_json::from_str(text)
}
pub fn stringify(value: &JsonValue) -> String {
    value.to_string()
}
pub fn stringify_pretty(value: &JsonValue) -> Result<String, JsonError> {
    serde_json::to_string_pretty(value)
}
pub fn object() -> JsonValue {
    JsonValue::Object(JsonMap::new())
}
pub fn array() -> JsonValue {
    JsonValue::Array(Vec::new())
}
pub fn null() -> JsonValue {
    JsonValue::Null
}
pub fn pointer<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    value.pointer(path)
}
pub fn pointer_mut<'a>(value: &'a mut JsonValue, path: &str) -> Option<&'a mut JsonValue> {
    value.pointer_mut(path)
}
pub fn get_path<'a>(value: &'a JsonValue, path: &[&str]) -> Option<&'a JsonValue> {
    let mut current = value;
    for part in path {
        current = match current {
            JsonValue::Object(map) => map.get(*part)?,
            JsonValue::Array(values) => values.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}
pub fn merge(target: &mut JsonValue, patch: JsonValue) {
    match (target, patch) {
        (JsonValue::Object(target), JsonValue::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(&key);
                } else {
                    merge(target.entry(key).or_insert(JsonValue::Null), value);
                }
            }
        }
        (target, patch) => *target = patch,
    }
}
pub fn flatten(value: &JsonValue) -> Vec<(String, JsonValue)> {
    fn visit(value: &JsonValue, path: String, output: &mut Vec<(String, JsonValue)>) {
        match value {
            JsonValue::Object(map) => {
                for (key, value) in map {
                    visit(
                        value,
                        format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                        output,
                    );
                }
            }
            JsonValue::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    visit(value, format!("{path}/{index}"), output);
                }
            }
            value => output.push((path, value.clone())),
        }
    }
    let mut output = Vec::new();
    visit(value, String::new(), &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_and_merges() {
        let mut value = parse(r#"{"user":{"name":"Ada","active":true}}"#).unwrap();
        assert_eq!(
            pointer(&value, "/user/name").and_then(JsonValue::as_str),
            Some("Ada")
        );
        merge(
            &mut value,
            parse(r#"{"user":{"active":null,"age":36}}"#).unwrap(),
        );
        assert!(pointer(&value, "/user/active").is_none());
        assert_eq!(
            pointer(&value, "/user/age").and_then(JsonValue::as_i64),
            Some(36)
        );
    }
}
