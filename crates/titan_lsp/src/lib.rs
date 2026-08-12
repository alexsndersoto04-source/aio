//! Editor-independent language intelligence for Titan.

mod server;
pub use server::{run_stdio, ServerError};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use titan_lexer::{Lexer, LexerError, Span, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: u8,
    pub source: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: u8,
    pub detail: String,
    pub uri: String,
    pub range: Range,
    pub selection_range: Range,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

#[derive(Debug, Clone)]
struct Document {
    text: String,
    version: i64,
}

#[derive(Default)]
pub struct TitanLsp {
    documents: HashMap<String, Document>,
}

impl TitanLsp {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn open_document(&mut self, uri: &str, text: &str, version: i64) {
        self.documents.insert(
            uri.into(),
            Document {
                text: text.into(),
                version,
            },
        );
    }
    pub fn update_document(&mut self, uri: &str, text: &str, version: i64) {
        if self
            .documents
            .get(uri)
            .is_none_or(|doc| version >= doc.version)
        {
            self.open_document(uri, text, version);
        }
    }
    pub fn apply_change(
        &mut self,
        uri: &str,
        range: Option<Range>,
        replacement: &str,
        version: i64,
    ) -> Result<(), String> {
        let document = self.documents.get_mut(uri).ok_or("document is not open")?;
        if version < document.version {
            return Err("stale document version".into());
        }
        if let Some(range) = range {
            let start =
                position_to_offset(&document.text, range.start).ok_or("invalid change start")?;
            let end = position_to_offset(&document.text, range.end).ok_or("invalid change end")?;
            if start > end {
                return Err("change range is reversed".into());
            }
            document.text.replace_range(start..end, replacement);
            document.version = version;
        } else {
            document.text = replacement.into();
            document.version = version;
        }
        Ok(())
    }
    pub fn close_document(&mut self, uri: &str) {
        self.documents.remove(uri);
    }
    pub fn text(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|doc| doc.text.as_str())
    }

    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        let Some(source) = self.text(uri) else {
            return Vec::new();
        };
        let mut lexer = Lexer::new(source);
        let (tokens, lexer_errors) = lexer.tokenize();
        let mut output: Vec<_> = lexer_errors
            .iter()
            .map(|error| {
                let (line, column) = lexer_error_position(error);
                Diagnostic {
                    range: point_range(
                        line.saturating_sub(1) as u32,
                        column.saturating_sub(1) as u32,
                    ),
                    severity: 1,
                    source: "titan".into(),
                    message: error.to_string(),
                }
            })
            .collect();
        if lexer_errors.is_empty() {
            let mut parser = titan_parser::Parser::new(tokens.to_vec());
            match parser.parse_program() {
                Ok(program) => {
                    let mut types = titan_typechecker::TypeEnv::new();
                    if let Err(errors) = types.check_program(&program) {
                        output.extend(errors.into_iter().map(|error| Diagnostic {
                            range: Range::default(),
                            severity: 1,
                            source: "titan".into(),
                            message: error.to_string(),
                        }));
                    }
                }
                Err(_) => output.extend(parser.errors().iter().map(|error| {
                    let (line, column) = match error {
                        titan_parser::ParseError::Expected { line, column, .. }
                        | titan_parser::ParseError::Message { line, column, .. } => {
                            (*line, *column)
                        }
                    };
                    Diagnostic {
                        range: point_range(
                            line.saturating_sub(1) as u32,
                            column.saturating_sub(1) as u32,
                        ),
                        severity: 1,
                        source: "titan".into(),
                        message: error.to_string(),
                    }
                })),
            }
        }
        output
    }

    pub fn symbols(&self, uri: &str) -> Vec<Symbol> {
        self.text(uri)
            .map(|text| index_symbols(uri, text))
            .unwrap_or_default()
    }
    pub fn workspace_symbols(&self, query: &str) -> Vec<Symbol> {
        let query = query.to_lowercase();
        let mut result: Vec<_> = self
            .documents
            .iter()
            .flat_map(|(uri, doc)| index_symbols(uri, &doc.text))
            .filter(|symbol| symbol.name.to_lowercase().contains(&query))
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name).then(a.uri.cmp(&b.uri)));
        result
    }
    pub fn completions(&self, uri: &str) -> Vec<(String, u8, String)> {
        let mut values: BTreeMap<String, (u8, String)> = BTreeMap::new();
        for keyword in [
            "fn", "let", "if", "else", "match", "for", "while", "loop", "return", "break",
            "continue", "struct", "enum", "trait", "impl", "import", "const", "true", "false",
            "nil",
        ] {
            values.insert(keyword.into(), (14, "Titan keyword".into()));
        }
        for symbol in self.workspace_symbols("") {
            values.insert(symbol.name, (symbol.kind, symbol.detail));
        }
        for native in titan_stdlib::native::NATIVES {
            values.insert(
                native.name.into(),
                (
                    3,
                    format!("native {:?} -> {:?}", native.params, native.result),
                ),
            );
        }
        let _ = uri;
        values
            .into_iter()
            .map(|(name, (kind, detail))| (name, kind, detail))
            .collect()
    }
    pub fn definition(&self, uri: &str, position: Position) -> Option<Symbol> {
        let name = self.word_at(uri, position)?;
        self.workspace_symbols(&name)
            .into_iter()
            .find(|symbol| symbol.name == name)
    }
    pub fn references(&self, uri: &str, position: Position) -> Vec<(String, Range)> {
        let Some(name) = self.word_at(uri, position) else {
            return Vec::new();
        };
        self.documents
            .iter()
            .flat_map(|(uri, doc)| {
                identifier_ranges(&doc.text, &name)
                    .into_iter()
                    .map(|range| (uri.clone(), range))
            })
            .collect()
    }
    pub fn rename(
        &self,
        uri: &str,
        position: Position,
        new_name: &str,
    ) -> Result<BTreeMap<String, Vec<TextEdit>>, String> {
        if !valid_identifier(new_name) {
            return Err("invalid Titan identifier".into());
        }
        let name = self
            .word_at(uri, position)
            .ok_or("no identifier at position")?;
        let mut changes = BTreeMap::new();
        for (uri, document) in &self.documents {
            let edits: Vec<_> = identifier_ranges(&document.text, &name)
                .into_iter()
                .map(|range| TextEdit {
                    range,
                    new_text: new_name.into(),
                })
                .collect();
            if !edits.is_empty() {
                changes.insert(uri.clone(), edits);
            }
        }
        Ok(changes)
    }
    pub fn hover(&self, uri: &str, position: Position) -> Option<String> {
        let name = self.word_at(uri, position)?;
        if let Some(native) = titan_stdlib::native::lookup(&name) {
            return Some(format!(
                "`{}`\n\nNative `{:?} -> {:?}`; capability: `{:?}`",
                name, native.params, native.result, native.capability
            ));
        }
        self.workspace_symbols(&name)
            .into_iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| format!("`{}`\n\n{}", symbol.name, symbol.detail))
    }
    pub fn signature_help(
        &self,
        uri: &str,
        position: Position,
    ) -> Option<(String, Vec<String>, usize)> {
        let text = self.text(uri)?;
        let offset = position_to_offset(text, position)?;
        let prefix = &text[..offset];
        let open = prefix.rfind('(')?;
        let name = prefix[..open]
            .trim_end()
            .chars()
            .rev()
            .take_while(|character| {
                character.is_alphanumeric() || *character == '_' || *character == ':'
            })
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if name.is_empty() {
            return None;
        }
        let active = prefix[open + 1..]
            .chars()
            .filter(|character| *character == ',')
            .count();
        if let Some(native) = titan_stdlib::native::lookup(&name) {
            let params: Vec<_> = native
                .params
                .iter()
                .enumerate()
                .map(|(index, ty)| format!("arg{index}: {ty:?}"))
                .collect();
            return Some((
                format!("{}({}) -> {:?}", name, params.join(", "), native.result),
                params,
                active,
            ));
        }
        self.workspace_symbols(&name)
            .into_iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| (format!("{}(…)", symbol.name), Vec::new(), active))
    }
    pub fn semantic_tokens(&self, uri: &str) -> Vec<u32> {
        let Some(source) = self.text(uri) else {
            return Vec::new();
        };
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().0.to_vec();
        let mut data = Vec::new();
        let mut previous_line = 0u32;
        let mut previous_start = 0u32;
        for (index, token) in tokens.iter().enumerate() {
            let token_type = semantic_kind(
                &token.kind,
                index
                    .checked_sub(1)
                    .and_then(|previous| tokens.get(previous))
                    .map(|token| &token.kind),
            );
            let Some(token_type) = token_type else {
                continue;
            };
            let range = span_range(source, token.span);
            if range.start.line != range.end.line {
                continue;
            }
            let delta_line = range.start.line - previous_line;
            let delta_start = if delta_line == 0 {
                range.start.character - previous_start
            } else {
                range.start.character
            };
            let length = range.end.character.saturating_sub(range.start.character);
            if length == 0 {
                continue;
            }
            data.extend([delta_line, delta_start, length, token_type, 0]);
            previous_line = range.start.line;
            previous_start = range.start.character;
        }
        data
    }
    fn word_at(&self, uri: &str, position: Position) -> Option<String> {
        let text = self.text(uri)?;
        let offset = position_to_offset(text, position)?;
        token_at(text, offset)
    }
}

fn semantic_kind(kind: &TokenKind, previous: Option<&TokenKind>) -> Option<u32> {
    Some(match kind {
        TokenKind::Fn
        | TokenKind::Let
        | TokenKind::Mut
        | TokenKind::Return
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::Match
        | TokenKind::For
        | TokenKind::While
        | TokenKind::Loop
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::In
        | TokenKind::Struct
        | TokenKind::Enum
        | TokenKind::Trait
        | TokenKind::Impl
        | TokenKind::Module
        | TokenKind::Import
        | TokenKind::Pub
        | TokenKind::Const
        | TokenKind::Unsafe
        | TokenKind::Spawn
        | TokenKind::Go
        | TokenKind::As => 0,
        TokenKind::Ident(_) if matches!(previous, Some(TokenKind::Fn)) => 1,
        TokenKind::Ident(_)
            if matches!(
                previous,
                Some(TokenKind::Struct) | Some(TokenKind::Enum) | Some(TokenKind::Trait)
            ) =>
        {
            2
        }
        TokenKind::Ident(_) | TokenKind::Self_ => 3,
        TokenKind::StringLit(_) | TokenKind::CharLit(_) => 4,
        TokenKind::IntLit(_) | TokenKind::FloatLit(_) => 5,
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Eq
        | TokenKind::EqEq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::LazyAnd
        | TokenKind::LazyOr
        | TokenKind::Ampersand
        | TokenKind::Pipe
        | TokenKind::Caret
        | TokenKind::Bang
        | TokenKind::Tilde
        | TokenKind::Question
        | TokenKind::Range
        | TokenKind::RangeInclusive
        | TokenKind::ThinArrow
        | TokenKind::FatArrow => 6,
        _ => return None,
    })
}

fn index_symbols(uri: &str, source: &str) -> Vec<Symbol> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().0.to_vec();
    let mut result = Vec::new();
    for pair in tokens.windows(2) {
        let (kind, symbol_kind, detail) = match &pair[0].kind {
            TokenKind::Fn => (true, 12, "function"),
            TokenKind::Struct => (true, 23, "struct"),
            TokenKind::Enum => (true, 10, "enum"),
            TokenKind::Trait => (true, 11, "trait"),
            TokenKind::Module => (true, 2, "module"),
            TokenKind::Const => (true, 14, "constant"),
            TokenKind::Let => (true, 13, "variable"),
            _ => (false, 0, ""),
        };
        if kind {
            if let TokenKind::Ident(name) = &pair[1].kind {
                let range = span_range(source, pair[1].span);
                result.push(Symbol {
                    name: name.clone(),
                    kind: symbol_kind,
                    detail: detail.into(),
                    uri: uri.into(),
                    range,
                    selection_range: range,
                });
            }
        }
    }
    result
}
fn identifier_ranges(source: &str, name: &str) -> Vec<Range> {
    let mut lexer = Lexer::new(source);
    lexer
        .tokenize()
        .0
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::Ident(value) if value == name => Some(span_range(source, token.span)),
            _ => None,
        })
        .collect()
}
fn token_at(source: &str, offset: usize) -> Option<String> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize().0.iter().find_map(|token| {
        if token.span.start <= offset && offset <= token.span.end {
            match &token.kind {
                TokenKind::Ident(name) => Some(name.clone()),
                _ => None,
            }
        } else {
            None
        }
    })
}
fn span_range(source: &str, span: Span) -> Range {
    Range {
        start: offset_to_position(source, span.start),
        end: offset_to_position(source, span.end),
    }
}
fn offset_to_position(text: &str, offset: usize) -> Position {
    let prefix = &text[..offset.min(text.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = prefix[line_start..].encode_utf16().count() as u32;
    Position { line, character }
}
fn position_to_offset(text: &str, position: Position) -> Option<usize> {
    let mut offset = 0;
    for _ in 0..position.line {
        let next = text[offset..].find('\n')?;
        offset += next + 1;
    }
    let line_end = text[offset..]
        .find('\n')
        .map_or(text.len(), |end| offset + end);
    let mut utf16 = 0;
    for (relative, character) in text[offset..line_end].char_indices() {
        if utf16 == position.character {
            return Some(offset + relative);
        }
        utf16 += character.len_utf16() as u32;
        if utf16 > position.character {
            return None;
        }
    }
    (utf16 == position.character).then_some(line_end)
}
fn point_range(line: u32, character: u32) -> Range {
    Range {
        start: Position { line, character },
        end: Position {
            line,
            character: character + 1,
        },
    }
}
fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}
fn lexer_error_position(error: &LexerError) -> (usize, usize) {
    match error {
        LexerError::UnterminatedString { line, column }
        | LexerError::UnterminatedChar { line, column }
        | LexerError::InvalidEscape { line, column, .. }
        | LexerError::InvalidCharacter { line, column, .. }
        | LexerError::UnterminatedComment { line, column } => (*line, *column),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provides_symbols_navigation_and_utf16_positions() {
        let mut lsp = TitanLsp::new();
        lsp.open_document(
            "file:///x.titan",
            "fn double(x: int) -> int { x * 2 }\nfn main() { double(21) }",
            1,
        );
        assert_eq!(lsp.symbols("file:///x.titan").len(), 2);
        let definition = lsp
            .definition(
                "file:///x.titan",
                Position {
                    line: 1,
                    character: 13,
                },
            )
            .unwrap();
        assert_eq!(definition.name, "double");
        assert_eq!(
            lsp.references(
                "file:///x.titan",
                Position {
                    line: 0,
                    character: 4
                }
            )
            .len(),
            2
        );
    }
    #[test]
    fn applies_incremental_utf16_changes_and_renames() {
        let mut lsp = TitanLsp::new();
        lsp.open_document("file:///x.titan", "fn café() { 1 }", 1);
        lsp.apply_change(
            "file:///x.titan",
            Some(Range {
                start: Position {
                    line: 0,
                    character: 3,
                },
                end: Position {
                    line: 0,
                    character: 7,
                },
            }),
            "tea",
            2,
        )
        .unwrap();
        assert!(lsp.text("file:///x.titan").unwrap().contains("tea"));
        assert!(lsp
            .rename(
                "file:///x.titan",
                Position {
                    line: 0,
                    character: 4
                },
                "brew"
            )
            .is_ok());
    }
    #[test]
    fn provides_semantic_tokens_and_native_signatures() {
        let mut lsp = TitanLsp::new();
        lsp.open_document(
            "file:///x.titan",
            "fn main() { std::stats::mean([1, 2]) }",
            1,
        );
        let tokens = lsp.semantic_tokens("file:///x.titan");
        assert!(!tokens.is_empty());
        assert_eq!(tokens.len() % 5, 0);
        let signature = lsp
            .signature_help(
                "file:///x.titan",
                Position {
                    line: 0,
                    character: 33,
                },
            )
            .unwrap();
        assert!(signature.0.contains("std::stats::mean"));
    }
    #[test]
    fn reports_invalid_document() {
        let mut lsp = TitanLsp::new();
        lsp.open_document("file:///x.titan", "fn main( {", 1);
        assert!(!lsp.diagnostics("file:///x.titan").is_empty());
    }
}
