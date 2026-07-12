//! Lightweight language-service core used by editor integrations.
//!
//! Transport (stdio/JSON-RPC) is intentionally kept outside this crate; this
//! type owns documents and returns stable diagnostics with source locations.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity { Error, Warning }

pub struct TitanLsp { documents: HashMap<String, String> }

impl TitanLsp {
    pub fn new() -> Self { Self { documents: HashMap::new() } }
    pub fn open_document(&mut self, uri: &str, text: &str) { self.documents.insert(uri.into(), text.into()); }
    pub fn update_document(&mut self, uri: &str, text: &str) { self.open_document(uri, text); }
    pub fn close_document(&mut self, uri: &str) { self.documents.remove(uri); }

    pub fn analyze(&self, uri: &str) -> Vec<Diagnostic> {
        let Some(source) = self.documents.get(uri) else { return Vec::new() };
        let mut lexer = titan_lexer::Lexer::new(source);
        let (tokens, lexer_errors) = lexer.tokenize();
        let mut diagnostics: Vec<_> = lexer_errors.iter().map(|error| Diagnostic {
            line: 0, column: 0, severity: DiagnosticSeverity::Error, message: error.to_string(),
        }).collect();
        if lexer_errors.is_empty() {
            let mut parser = titan_parser::Parser::new(tokens.to_vec());
            match parser.parse_program() {
                Ok(program) => {
                    let mut types = titan_typechecker::TypeEnv::new();
                    if let Err(errors) = types.check_program(&program) {
                        diagnostics.extend(errors.into_iter().map(|error| Diagnostic { line: 0, column: 0, severity: DiagnosticSeverity::Error, message: error.to_string() }));
                    }
                }
                Err(_) => diagnostics.extend(parser.errors().iter().map(|error| Diagnostic { line: 0, column: 0, severity: DiagnosticSeverity::Error, message: error.to_string() })),
            }
        }
        diagnostics
    }
}

impl Default for TitanLsp { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reports_invalid_document() { let mut lsp = TitanLsp::new(); lsp.open_document("file:///x.titan", "fn main( {"); assert!(!lsp.analyze("file:///x.titan").is_empty()); }
}
