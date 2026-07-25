//! XML utilities (`std::xml::*`) built on top of `quick-xml`.
//!
//! Two entry points:
//!
//! * `parse(text)` — returns a lightweight tree as JSON-compatible `Value`s
//!   that the VM already knows how to bridge to Titan maps/arrays/strings.
//! * `stringify(value)` — serializes that tree back to well-formed XML.
//! * `escape_text` / `escape_attr` — one-line helpers for building XML by hand.
//!
//! The tree shape used for `parse` is:
//!
//! ```json
//! { "tag": "root",
//!   "attrs": { "id": "1" },
//!   "children": [
//!     { "tag": "child", "attrs": {}, "children": [], "text": "hola" }
//!   ],
//!   "text": ""
//! }
//! ```

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use serde_json::{json, Value};
use std::io::Cursor;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XmlError {
    #[error("XML parse error: {0}")]
    Parse(String),
    #[error("XML emit error: {0}")]
    Emit(String),
}

fn map_err(error: impl std::fmt::Display) -> XmlError { XmlError::Parse(error.to_string()) }

/// Parses `text` into a tree of `{tag, attrs, children, text}` maps.
pub fn parse(text: &str) -> Result<Value, XmlError> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    // Stack of in-progress nodes. Each item mirrors the shape documented above.
    let mut stack: Vec<Node> = vec![Node::new("__root__".into())];

    loop {
        match reader.read_event().map_err(map_err)? {
            Event::Start(event) => {
                let node = build_start(&event)?;
                stack.push(node);
            }
            Event::Empty(event) => {
                let node = build_start(&event)?;
                let parent = stack.last_mut().expect("stack invariant");
                parent.children.push(node.into_value());
            }
            Event::End(event) => close(&mut stack, &event)?,
            Event::Text(event) => {
                let text = event.unescape().map_err(map_err)?.into_owned();
                if !text.is_empty() {
                    let parent = stack.last_mut().expect("stack invariant");
                    if parent.text.is_empty() { parent.text = text; }
                    else { parent.text.push_str(&text); }
                }
            }
            Event::CData(event) => {
                let text = String::from_utf8(event.into_inner().into_owned())
                    .map_err(|error| XmlError::Parse(error.to_string()))?;
                let parent = stack.last_mut().expect("stack invariant");
                parent.text.push_str(&text);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let root = stack.pop().expect("root node");
    // Unwrap our synthetic __root__: if there is exactly one child, that's the document element.
    if root.tag == "__root__" && root.children.len() == 1 && root.text.is_empty() && root.attrs.is_empty() {
        return Ok(root.children.into_iter().next().unwrap());
    }
    Ok(root.into_value())
}

/// Emits the tree returned by [`parse`] back to XML.
pub fn stringify(value: &Value) -> Result<String, XmlError> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write_node(&mut writer, value)?;
    Ok(String::from_utf8(writer.into_inner().into_inner())
        .map_err(|error| XmlError::Emit(error.to_string()))?)
}

pub fn escape_text(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

pub fn escape_attr(text: &str) -> String {
    // Attribute escaping additionally quotes " and '.
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------- internals ----------

struct Node {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<Value>,
    text: String,
}
impl Node {
    fn new(tag: String) -> Self { Self { tag, attrs: Vec::new(), children: Vec::new(), text: String::new() } }
    fn into_value(self) -> Value {
        let mut attrs = serde_json::Map::new();
        for (key, value) in self.attrs { attrs.insert(key, Value::String(value)); }
        json!({
            "tag": self.tag,
            "attrs": Value::Object(attrs),
            "children": Value::Array(self.children),
            "text": self.text,
        })
    }
}

fn build_start(event: &BytesStart<'_>) -> Result<Node, XmlError> {
    let tag = std::str::from_utf8(event.name().as_ref())
        .map_err(map_err)?
        .to_string();
    let mut node = Node::new(tag);
    for attribute in event.attributes() {
        let attribute = attribute.map_err(map_err)?;
        let key = std::str::from_utf8(attribute.key.as_ref()).map_err(map_err)?.to_string();
        let value = attribute.unescape_value().map_err(map_err)?.into_owned();
        node.attrs.push((key, value));
    }
    Ok(node)
}

fn close(stack: &mut Vec<Node>, event: &BytesEnd<'_>) -> Result<(), XmlError> {
    let name = std::str::from_utf8(event.name().as_ref()).map_err(map_err)?;
    let finished = stack.pop().ok_or_else(|| XmlError::Parse("unbalanced close tag".into()))?;
    if finished.tag != name {
        return Err(XmlError::Parse(format!("mismatched close tag: expected </{}> but found </{}>", finished.tag, name)));
    }
    let parent = stack.last_mut().ok_or_else(|| XmlError::Parse("close without root".into()))?;
    parent.children.push(finished.into_value());
    Ok(())
}

fn write_node(writer: &mut Writer<Cursor<Vec<u8>>>, value: &Value) -> Result<(), XmlError> {
    let object = value.as_object().ok_or_else(|| XmlError::Emit("expected an XML node object".into()))?;
    let tag = object.get("tag").and_then(Value::as_str).ok_or_else(|| XmlError::Emit("node is missing 'tag'".into()))?;
    let mut start = BytesStart::new(tag);
    if let Some(attrs) = object.get("attrs").and_then(Value::as_object) {
        for (key, val) in attrs {
            if let Some(text) = val.as_str() {
                start.push_attribute((QName(key.as_bytes()), text.as_bytes()));
            }
        }
    }
    writer.write_event(Event::Start(start)).map_err(|error| XmlError::Emit(error.to_string()))?;
    if let Some(text) = object.get("text").and_then(Value::as_str) {
        if !text.is_empty() {
            writer.write_event(Event::Text(BytesText::new(text))).map_err(|error| XmlError::Emit(error.to_string()))?;
        }
    }
    if let Some(children) = object.get("children").and_then(Value::as_array) {
        for child in children { write_node(writer, child)?; }
    }
    writer.write_event(Event::End(BytesEnd::new(tag))).map_err(|error| XmlError::Emit(error.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_and_children() {
        let tree = parse(r#"<root id="1"><child>hola</child><child>mundo</child></root>"#).unwrap();
        assert_eq!(tree["tag"], "root");
        assert_eq!(tree["attrs"]["id"], "1");
        assert_eq!(tree["children"][0]["tag"], "child");
        assert_eq!(tree["children"][0]["text"], "hola");
        assert_eq!(tree["children"][1]["text"], "mundo");
    }

    #[test]
    fn round_trips_a_document() {
        let source = r#"<root id="1"><child>hola</child></root>"#;
        let tree = parse(source).unwrap();
        let out = stringify(&tree).unwrap();
        // The output must be parseable and equal to what we parsed.
        assert_eq!(parse(&out).unwrap(), tree);
    }

    #[test]
    fn self_closing_and_cdata() {
        let tree = parse(r#"<msg><b/><![CDATA[hola & mundo]]></msg>"#).unwrap();
        assert_eq!(tree["children"][0]["tag"], "b");
        assert_eq!(tree["text"], "hola & mundo");
    }

    #[test]
    fn escape_helpers() {
        assert_eq!(escape_text("a<b>c&d"), "a&lt;b&gt;c&amp;d");
        assert_eq!(escape_attr(r#"o'brien "hi""#), "o&apos;brien &quot;hi&quot;");
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(parse("<a><b></a>").is_err());
    }
}
