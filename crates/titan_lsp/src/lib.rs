//! Titan LSP - Language Server Protocol.
use std::collections::HashMap;

pub struct TitanLsp {
    documents: HashMap<String, String>,
}

impl TitanLsp {
    pub fn new() -> Self {
        TitanLsp { documents: HashMap::new() }
    }
    pub fn open_document(&mut self, uri: &str, text: &str) {
        self.documents.insert(uri.to_string(), text.to_string());
    }
    pub fn close_document(&mut self, uri: &str) {
        self.documents.remove(uri);
    }
    pub fn analyze(&self, _uri: &str) -> Vec<String> {
        Vec::new()
    }
}

impl Default for TitanLsp {
    fn default() -> Self { Self::new() }
}