//! Titan Lexer - Tokenizer for the Titan language.
use thiserror::Error;

/// Source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize, pub end: usize,
    pub line: usize, pub column: usize,
}
impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Span { start, end, line, column }
    }
}

/// Token kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Let, Mut, Fn, Return, If, Else, Match, For, While, Loop,
    Break, Continue, In, Struct, Enum, Trait, Impl,
    Module, Import, Pub, Const, Unsafe, Spawn, Go,
    True, False, Nil, Self_,
    Plus, Minus, Star, Slash, Percent,
    Ampersand, Pipe, Caret, Tilde, Bang, Question,
    PlusEq, MinusEq, StarEq, SlashEq, ThinArrow, FatArrow,
    EqEq, NotEq, LtEq, GtEq, Lt, Gt, LazyAnd, LazyOr, ColonColon,
    Eq, // single = for assignment
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Comma, Semicolon, Colon, Dot, Underscore,
    IntLit(String), FloatLit(String), StringLit(String), CharLit(char),
    Ident(String),
    Eof, Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Error, Debug)]
pub enum LexerError {
    #[error("Unterminated string at line {line}")]
    UnterminatedString { line: usize },
    #[error("Invalid character at line {line}")]
    InvalidCharacter { line: usize },
}

pub struct Lexer {
    source: Vec<char>,
    pos: usize, line: usize, col: usize,
    tokens: Vec<Token>,
    errors: Vec<LexerError>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0, line: 1, col: 1,
            tokens: Vec::new(), errors: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> (&[Token], &[LexerError]) {
        self.tokens.clear(); self.errors.clear();
        self.pos = 0; self.line = 1; self.col = 1;
        while self.pos < self.source.len() {
            self.skip_whitespace();
            if self.pos >= self.source.len() { break; }
            let tok = self.next_token();
            self.tokens.push(tok);
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(self.pos, self.pos+1, self.line, self.col),
        });
        (&self.tokens, &self.errors)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.source.len() {
            match self.source[self.pos] {
                ' ' | '\t' | '\r' => { self.pos += 1; self.col += 1; }
                '\n' => { self.pos += 1; self.line += 1; self.col = 1; }
                '/' if self.pos+1 < self.source.len() && self.source[self.pos+1] == '/' => {
                    while self.pos < self.source.len() && self.source[self.pos] != '\n' {
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Token {
        let start = self.pos; let sl = self.line; let sc = self.col;
        let c = self.advance();
        let kind = match c {
            '+' => if self.eat('=') { TokenKind::PlusEq } else { TokenKind::Plus },
            '-' => if self.eat('>') { TokenKind::ThinArrow } else if self.eat('=') { TokenKind::MinusEq } else { TokenKind::Minus },
            '*' => if self.eat('=') { TokenKind::StarEq } else { TokenKind::Star },
            '/' => if self.eat('=') { TokenKind::SlashEq } else { TokenKind::Slash },
            '%' => TokenKind::Percent,
            '&' => if self.eat('&') { TokenKind::LazyAnd } else { TokenKind::Ampersand },
            '|' => if self.eat('|') { TokenKind::LazyOr } else { TokenKind::Pipe },
            '!' => if self.eat('=') { TokenKind::NotEq } else { TokenKind::Bang },
            '=' => if self.eat('=') { TokenKind::EqEq } else if self.eat('>') { TokenKind::FatArrow } else { TokenKind::Eq },
            '<' => if self.eat('=') { TokenKind::LtEq } else { TokenKind::Lt },
            '>' => if self.eat('=') { TokenKind::GtEq } else { TokenKind::Gt },
            '(' => TokenKind::LParen, ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace, '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket, ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma, ';' => TokenKind::Semicolon,
            ':' => if self.eat(':') { TokenKind::ColonColon } else { TokenKind::Colon },
            '.' => TokenKind::Dot,
            '_' => TokenKind::Underscore,
            '"' => self.lex_string(),
            '\'' => self.lex_char(),
            '0'..='9' => self.lex_number(c),
            'a'..='z' | 'A'..='Z' => self.lex_identifier(c),
            _ => TokenKind::Error(format!("{}", c)),
        };
        Token { kind, span: Span::new(start, self.pos, sl, sc) }
    }

    fn lex_string(&mut self) -> TokenKind {
        let mut s = String::new();
        while self.pos < self.source.len() && self.source[self.pos] != '"' {
            s.push(self.advance());
        }
        if self.pos < self.source.len() { self.advance(); }
        TokenKind::StringLit(s)
    }

    fn lex_char(&mut self) -> TokenKind {
        let c = self.advance();
        self.advance();
        TokenKind::CharLit(c)
    }

    fn lex_number(&mut self, first: char) -> TokenKind {
        let mut num = String::from(first);
        let mut is_float = false;
        while self.pos < self.source.len() {
            let c = self.source[self.pos];
            if c.is_ascii_digit() || c == '_' { num.push(self.advance()); }
            else if c == '.' && !is_float { is_float = true; num.push(self.advance()); }
            else { break; }
        }
        if is_float { TokenKind::FloatLit(num) } else { TokenKind::IntLit(num) }
    }

    fn lex_identifier(&mut self, first: char) -> TokenKind {
        let mut ident = String::from(first);
        while self.pos < self.source.len() {
            let c = self.source[self.pos];
            if c.is_alphanumeric() || c == '_' { ident.push(self.advance()); }
            else { break; }
        }
        match ident.as_str() {
            "let" => TokenKind::Let, "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn, "return" => TokenKind::Return,
            "if" => TokenKind::If, "else" => TokenKind::Else,
            "match" => TokenKind::Match, "for" => TokenKind::For,
            "while" => TokenKind::While, "loop" => TokenKind::Loop,
            "break" => TokenKind::Break, "continue" => TokenKind::Continue,
            "in" => TokenKind::In,
            "struct" => TokenKind::Struct, "enum" => TokenKind::Enum,
            "trait" => TokenKind::Trait, "impl" => TokenKind::Impl,
            "mod" => TokenKind::Module, "import" => TokenKind::Import,
            "pub" => TokenKind::Pub, "const" => TokenKind::Const,
            "unsafe" => TokenKind::Unsafe,
            "spawn" => TokenKind::Spawn, "go" => TokenKind::Go,
            "true" => TokenKind::True, "false" => TokenKind::False,
            "nil" => TokenKind::Nil, "self" => TokenKind::Self_,
            _ => TokenKind::Ident(ident),
        }
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.pos];
        self.pos += 1; self.col += 1;
        c
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.pos < self.source.len() && self.source[self.pos] == expected {
            self.pos += 1; self.col += 1;
            true
        } else { false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_keywords() {
        let mut lex = Lexer::new("let fn if");
        let (tokens, _) = lex.tokenize();
        assert!(matches!(tokens[0].kind, TokenKind::Let));
        assert!(matches!(tokens[1].kind, TokenKind::Fn));
    }
    #[test]
    fn test_numbers() {
        let mut lex = Lexer::new("42 3.14");
        let (tokens, _) = lex.tokenize();
        assert!(matches!(tokens[0].kind, TokenKind::IntLit(_)));
    }
    #[test]
    fn test_eq_token() {
        let mut lex = Lexer::new("=");
        let (tokens, _) = lex.tokenize();
        assert!(matches!(tokens[0].kind, TokenKind::Eq));
    }
}