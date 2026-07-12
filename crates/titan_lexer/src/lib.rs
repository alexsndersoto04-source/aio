//! Lexer for the Titan programming language.

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Self { start, end, line, column }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Let, Mut, Fn, Return, If, Else, Match, For, While, Loop,
    Break, Continue, In, Struct, Enum, Trait, Impl, Module, Import,
    Pub, Const, Unsafe, Spawn, Go, True, False, Nil, Self_, As,
    Plus, Minus, Star, Slash, Percent, Ampersand, Pipe, Caret, Tilde,
    Bang, Question, PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    ThinArrow, FatArrow, EqEq, NotEq, LtEq, GtEq, Lt, Gt, LazyAnd,
    LazyOr, ColonColon, Eq, Range, RangeInclusive,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semicolon, Colon, Dot, Underscore,
    IntLit(String), FloatLit(String), StringLit(String), CharLit(char),
    Ident(String), Eof, Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexerError {
    #[error("unterminated string at {line}:{column}")]
    UnterminatedString { line: usize, column: usize },
    #[error("unterminated character literal at {line}:{column}")]
    UnterminatedChar { line: usize, column: usize },
    #[error("invalid escape \\{escape} at {line}:{column}")]
    InvalidEscape { escape: char, line: usize, column: usize },
    #[error("invalid character '{character}' at {line}:{column}")]
    InvalidCharacter { character: char, line: usize, column: usize },
    #[error("unterminated block comment at {line}:{column}")]
    UnterminatedComment { line: usize, column: usize },
}

pub struct Lexer {
    source: Vec<char>,
    byte_offsets: Vec<usize>,
    pos: usize,
    line: usize,
    col: usize,
    tokens: Vec<Token>,
    errors: Vec<LexerError>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        let mut byte_offsets: Vec<usize> = source.char_indices().map(|(i, _)| i).collect();
        byte_offsets.push(source.len());
        Self {
            source: source.chars().collect(), byte_offsets,
            pos: 0, line: 1, col: 1, tokens: Vec::new(), errors: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> (&[Token], &[LexerError]) {
        self.tokens.clear();
        self.errors.clear();
        self.pos = 0;
        self.line = 1;
        self.col = 1;
        while self.pos < self.source.len() {
            self.skip_trivia();
            if self.pos >= self.source.len() { break; }
            let token = self.next_token();
            self.tokens.push(token);
        }
        let end = self.byte_offset(self.pos);
        self.tokens.push(Token { kind: TokenKind::Eof, span: Span::new(end, end, self.line, self.col) });
        (&self.tokens, &self.errors)
    }

    fn skip_trivia(&mut self) {
        loop {
            while let Some(c) = self.peek() {
                if !c.is_whitespace() { break; }
                self.advance();
            }
            if self.peek() == Some('/') && self.peek_n(1) == Some('/') {
                while self.peek().is_some() && self.peek() != Some('\n') { self.advance(); }
                continue;
            }
            if self.peek() == Some('/') && self.peek_n(1) == Some('*') {
                let line = self.line;
                let column = self.col;
                self.advance(); self.advance();
                let mut depth = 1usize;
                while self.peek().is_some() && depth > 0 {
                    if self.peek() == Some('/') && self.peek_n(1) == Some('*') {
                        self.advance(); self.advance(); depth += 1;
                    } else if self.peek() == Some('*') && self.peek_n(1) == Some('/') {
                        self.advance(); self.advance(); depth -= 1;
                    } else { self.advance(); }
                }
                if depth != 0 { self.errors.push(LexerError::UnterminatedComment { line, column }); }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Token {
        let start_pos = self.pos;
        let start = self.byte_offset(start_pos);
        let line = self.line;
        let column = self.col;
        let c = self.advance().unwrap();
        let kind = match c {
            '+' => if self.eat('=') { TokenKind::PlusEq } else { TokenKind::Plus },
            '-' => if self.eat('>') { TokenKind::ThinArrow } else if self.eat('=') { TokenKind::MinusEq } else { TokenKind::Minus },
            '*' => if self.eat('=') { TokenKind::StarEq } else { TokenKind::Star },
            '/' => if self.eat('=') { TokenKind::SlashEq } else { TokenKind::Slash },
            '%' => if self.eat('=') { TokenKind::PercentEq } else { TokenKind::Percent },
            '&' => if self.eat('&') { TokenKind::LazyAnd } else { TokenKind::Ampersand },
            '|' => if self.eat('|') { TokenKind::LazyOr } else { TokenKind::Pipe },
            '^' => TokenKind::Caret,
            '~' => TokenKind::Tilde,
            '?' => TokenKind::Question,
            '!' => if self.eat('=') { TokenKind::NotEq } else { TokenKind::Bang },
            '=' => if self.eat('=') { TokenKind::EqEq } else if self.eat('>') { TokenKind::FatArrow } else { TokenKind::Eq },
            '<' => if self.eat('=') { TokenKind::LtEq } else { TokenKind::Lt },
            '>' => if self.eat('=') { TokenKind::GtEq } else { TokenKind::Gt },
            '(' => TokenKind::LParen, ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace, '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket, ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma, ';' => TokenKind::Semicolon,
            ':' => if self.eat(':') { TokenKind::ColonColon } else { TokenKind::Colon },
            '.' => if self.eat('.') { if self.eat('=') { TokenKind::RangeInclusive } else { TokenKind::Range } } else { TokenKind::Dot },
            '_' => if self.peek().is_some_and(is_ident_continue) { self.lex_identifier(c) } else { TokenKind::Underscore },
            '"' => self.lex_string(line, column),
            '\'' => self.lex_char(line, column),
            '0'..='9' => self.lex_number(c),
            ch if is_ident_start(ch) => self.lex_identifier(ch),
            character => {
                self.errors.push(LexerError::InvalidCharacter { character, line, column });
                TokenKind::Error(character.to_string())
            }
        };
        Token { kind, span: Span::new(start, self.byte_offset(self.pos), line, column) }
    }

    fn lex_string(&mut self, line: usize, column: usize) -> TokenKind {
        let mut value = String::new();
        while let Some(c) = self.peek() {
            match c {
                '"' => { self.advance(); return TokenKind::StringLit(value); }
                '\n' => {
                    self.errors.push(LexerError::UnterminatedString { line, column });
                    return TokenKind::Error("unterminated string".into());
                }
                '\\' => {
                    self.advance();
                    match self.advance() {
                        Some('n') => value.push('\n'), Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'), Some('0') => value.push('\0'),
                        Some('"') => value.push('"'), Some('\'') => value.push('\''),
                        Some('\\') => value.push('\\'),
                        Some(escape) => {
                            self.errors.push(LexerError::InvalidEscape { escape, line: self.line, column: self.col.saturating_sub(1) });
                            value.push(escape);
                        }
                        None => break,
                    }
                }
                _ => { value.push(c); self.advance(); }
            }
        }
        self.errors.push(LexerError::UnterminatedString { line, column });
        TokenKind::Error("unterminated string".into())
    }

    fn lex_char(&mut self, line: usize, column: usize) -> TokenKind {
        let value = match self.advance() {
            Some('\\') => match self.advance() {
                Some('n') => '\n', Some('r') => '\r', Some('t') => '\t',
                Some('0') => '\0', Some('\\') => '\\', Some('\'') => '\'',
                Some(escape) => {
                    self.errors.push(LexerError::InvalidEscape { escape, line, column });
                    escape
                }
                None => {
                    self.errors.push(LexerError::UnterminatedChar { line, column });
                    return TokenKind::Error("unterminated character".into());
                }
            },
            Some('\'') | Some('\n') | None => {
                self.errors.push(LexerError::UnterminatedChar { line, column });
                return TokenKind::Error("invalid character literal".into());
            }
            Some(c) => c,
        };
        if !self.eat('\'') {
            self.errors.push(LexerError::UnterminatedChar { line, column });
            return TokenKind::Error("unterminated character".into());
        }
        TokenKind::CharLit(value)
    }

    fn lex_number(&mut self, first: char) -> TokenKind {
        let mut number = String::from(first);
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') { number.push(self.advance().unwrap()); }
        let is_float = self.peek() == Some('.') && self.peek_n(1).is_some_and(|c| c.is_ascii_digit());
        if is_float {
            number.push(self.advance().unwrap());
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') { number.push(self.advance().unwrap()); }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            number.push(self.advance().unwrap());
            if matches!(self.peek(), Some('+' | '-')) { number.push(self.advance().unwrap()); }
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') { number.push(self.advance().unwrap()); }
            return TokenKind::FloatLit(number);
        }
        if is_float { TokenKind::FloatLit(number) } else { TokenKind::IntLit(number) }
    }

    fn lex_identifier(&mut self, first: char) -> TokenKind {
        let mut ident = String::from(first);
        while self.peek().is_some_and(is_ident_continue) { ident.push(self.advance().unwrap()); }
        match ident.as_str() {
            "let" => TokenKind::Let, "mut" => TokenKind::Mut, "fn" => TokenKind::Fn,
            "return" => TokenKind::Return, "if" => TokenKind::If, "else" => TokenKind::Else,
            "match" => TokenKind::Match, "for" => TokenKind::For, "while" => TokenKind::While,
            "loop" => TokenKind::Loop, "break" => TokenKind::Break, "continue" => TokenKind::Continue,
            "in" => TokenKind::In, "struct" => TokenKind::Struct, "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait, "impl" => TokenKind::Impl, "mod" => TokenKind::Module,
            "import" => TokenKind::Import, "pub" => TokenKind::Pub, "const" => TokenKind::Const,
            "unsafe" => TokenKind::Unsafe, "spawn" => TokenKind::Spawn, "go" => TokenKind::Go,
            "true" => TokenKind::True, "false" => TokenKind::False, "nil" => TokenKind::Nil,
            "self" => TokenKind::Self_, "as" => TokenKind::As,
            _ => TokenKind::Ident(ident),
        }
    }

    fn peek(&self) -> Option<char> { self.source.get(self.pos).copied() }
    fn peek_n(&self, n: usize) -> Option<char> { self.source.get(self.pos + n).copied() }
    fn byte_offset(&self, char_pos: usize) -> usize { self.byte_offsets[char_pos.min(self.byte_offsets.len() - 1)] }
    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        Some(c)
    }
    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) { self.advance(); true } else { false }
    }
}

fn is_ident_start(c: char) -> bool { c == '_' || c.is_alphabetic() }
fn is_ident_continue(c: char) -> bool { c == '_' || c.is_alphanumeric() }

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(src);
        lexer.tokenize().0.iter().map(|t| t.kind.clone()).collect()
    }

    #[test]
    fn lexes_keywords_operators_and_ranges() {
        let tokens = kinds("fn let if <= != 0..20 0..=20");
        assert!(matches!(tokens[0], TokenKind::Fn));
        assert!(tokens.contains(&TokenKind::LtEq));
        assert!(tokens.contains(&TokenKind::Range));
        assert!(tokens.contains(&TokenKind::RangeInclusive));
    }

    #[test]
    fn handles_unicode_spans_and_escapes() {
        let mut lexer = Lexer::new("let café = \"a\\n\"");
        let (tokens, errors) = lexer.tokenize();
        assert!(errors.is_empty());
        assert!(matches!(&tokens[1].kind, TokenKind::Ident(s) if s == "café"));
        assert!(matches!(&tokens[3].kind, TokenKind::StringLit(s) if s == "a\n"));
    }

    #[test]
    fn reports_bad_input_without_panicking() {
        for source in ["\"open", "'x", "@", "/* open"] {
            let mut lexer = Lexer::new(source);
            let (_, errors) = lexer.tokenize();
            assert!(!errors.is_empty(), "{source:?}");
        }
    }
}
