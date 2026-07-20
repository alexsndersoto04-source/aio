//! Recursive-descent and Pratt parser for Titan.

use thiserror::Error;
use titan_ast::*;
use titan_lexer::{Span, Token, TokenKind};

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("expected {expected}, found {found} at {line}:{column}")]
    Expected { expected: String, found: String, line: usize, column: usize },
    #[error("{message} at {line}:{column}")]
    Message { message: String, line: usize, column: usize },
}

pub type Result<T> = std::result::Result<T, ParseError>;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0, errors: Vec::new() } }
    pub fn errors(&self) -> &[ParseError] { &self.errors }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        while !self.at(TokenKind::Eof) {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(error) => {
                    self.errors.push(error);
                    self.synchronize_item();
                }
            }
        }
        if let Some(error) = self.errors.first().cloned() { Err(error) } else { Ok(Program { items }) }
    }

    fn parse_item(&mut self) -> Result<Item> {
        self.eat(TokenKind::Pub);
        match self.peek_kind() {
            Some(TokenKind::Fn) | Some(TokenKind::Extern) => self.parse_function().map(Item::Function),
            Some(TokenKind::Struct) => self.parse_struct().map(Item::Struct),
            Some(TokenKind::Enum) => self.parse_enum().map(Item::Enum),
            Some(TokenKind::Trait) => self.parse_trait().map(Item::Trait),
            Some(TokenKind::Impl) => self.parse_impl().map(Item::Impl),
            Some(TokenKind::Module) => self.parse_module().map(Item::Module),
            Some(TokenKind::Import) => self.parse_import().map(Item::Import),
            Some(TokenKind::Const) => self.parse_const().map(Item::Const),
            _ => Err(self.expected("a declaration (fn, struct, enum, trait, impl, mod, import or const)")),
        }
    }

    fn parse_function(&mut self) -> Result<FunctionDecl> {
        let is_extern = self.eat(TokenKind::Extern);
        let abi = if is_extern {
            if let Some(TokenKind::StringLit(s)) = self.peek_kind().cloned() {
                self.advance();
                Some(s)
            } else {
                Some("C".to_string())
            }
        } else {
            None
        };
        let start = self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) {
            let pspan = self.span();
            self.eat(TokenKind::Mut);
            let pname = if self.eat(TokenKind::Self_) { "self".into() } else { self.expect_ident()? };
            let type_ann = if self.eat(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
            let default = if self.eat(TokenKind::Eq) { Some(Box::new(self.parse_expr()?)) } else { None };
            params.push(Param { name: pname, type_ann, default, span: pspan });
            if !self.eat(TokenKind::Comma) { break; }
        }
        self.expect(TokenKind::RParen)?;
        let return_type = if self.eat(TokenKind::ThinArrow) { Some(self.parse_type()?) } else { None };
        let body = if self.eat(TokenKind::Semicolon) { None } else {
            self.expect(TokenKind::LBrace)?;
            Some(self.parse_block_after_open(start)?)
        };
        Ok(FunctionDecl { name, source_file: None, params, return_type, body, is_extern, abi, span: start })
    }

    fn parse_struct(&mut self) -> Result<StructDecl> {
        let span = self.expect(TokenKind::Struct)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RBrace) {
            self.eat(TokenKind::Pub);
            let fspan = self.span();
            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let type_ann = self.parse_type()?;
            fields.push(StructField { name: field_name, type_ann, span: fspan });
            if !self.eat(TokenKind::Comma) { self.eat(TokenKind::Semicolon); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(StructDecl { name, fields, span })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl> {
        let span = self.expect(TokenKind::Enum)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(TokenKind::RBrace) {
            let vspan = self.span();
            let variant = self.expect_ident()?;
            let payload = if self.eat(TokenKind::LParen) {
                let ty = self.parse_type()?;
                self.expect(TokenKind::RParen)?;
                Some(ty)
            } else { None };
            variants.push(EnumVariant { name: variant, payload, span: vspan });
            if !self.eat(TokenKind::Comma) { self.eat(TokenKind::Semicolon); }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(EnumDecl { name, variants, span })
    }

    fn parse_trait(&mut self) -> Result<TraitDecl> {
        let span = self.expect(TokenKind::Trait)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(TokenKind::RBrace) {
            let function = self.parse_function()?;
            methods.push(TraitMethod { name: function.name, params: function.params, return_type: function.return_type, span: function.span });
        }
        self.expect(TokenKind::RBrace)?;
        Ok(TraitDecl { name, methods, span })
    }

    fn parse_impl(&mut self) -> Result<ImplBlock> {
        let span = self.expect(TokenKind::Impl)?;
        let first = self.parse_type()?;
        let (trait_name, target_type) = if self.eat(TokenKind::For) {
            let trait_name = match first { TypeExpr::Named { name, .. } => Some(name), _ => return Err(self.message("trait name must be a named type")) };
            (trait_name, self.parse_type()?)
        } else { (None, first) };
        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(TokenKind::RBrace) { methods.push(self.parse_function()?); }
        self.expect(TokenKind::RBrace)?;
        Ok(ImplBlock { trait_name, target_type, methods, span })
    }

    fn parse_module(&mut self) -> Result<ModuleDecl> {
        let span = self.expect(TokenKind::Module)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while !self.at(TokenKind::RBrace) { items.push(self.parse_item()?); }
        self.expect(TokenKind::RBrace)?;
        Ok(ModuleDecl { name, items, span })
    }

    fn parse_import(&mut self) -> Result<ImportDecl> {
        let span = self.expect(TokenKind::Import)?;
        let mut path = vec![self.expect_ident()?];
        while self.eat(TokenKind::ColonColon) { path.push(self.expect_ident()?); }
        self.eat(TokenKind::Semicolon);
        Ok(ImportDecl { path, span })
    }

    fn parse_const(&mut self) -> Result<ConstDecl> {
        let span = self.expect(TokenKind::Const)?;
        let name = self.expect_ident()?;
        let type_ann = if self.eat(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
        self.expect(TokenKind::Eq)?;
        let value = Box::new(self.parse_expr()?);
        self.eat(TokenKind::Semicolon);
        Ok(ConstDecl { name, type_ann, value, span })
    }

    fn parse_type(&mut self) -> Result<TypeExpr> {
        if self.eat(TokenKind::Bang) { return Ok(TypeExpr::Never); }
        if self.eat(TokenKind::Ampersand) {
            let is_mut = self.eat(TokenKind::Mut);
            return Ok(TypeExpr::Reference { inner: Box::new(self.parse_type()?), is_mut });
        }
        if self.eat(TokenKind::LBracket) {
            let inner = Box::new(self.parse_type()?);
            if self.eat(TokenKind::Semicolon) {
                let size = Box::new(self.parse_expr()?);
                self.expect(TokenKind::RBracket)?;
                return Ok(TypeExpr::Array { inner, size });
            }
            self.expect(TokenKind::RBracket)?;
            return Ok(TypeExpr::Slice { inner });
        }
        if self.eat(TokenKind::LParen) {
            if self.eat(TokenKind::RParen) { return Ok(TypeExpr::Unit); }
            let mut elements = vec![self.parse_type()?];
            while self.eat(TokenKind::Comma) { if self.at(TokenKind::RParen) { break; } elements.push(self.parse_type()?); }
            self.expect(TokenKind::RParen)?;
            return Ok(TypeExpr::Tuple { elements });
        }
        let name = self.expect_ident()?;
        let mut generics = Vec::new();
        if self.eat(TokenKind::Lt) {
            loop {
                generics.push(self.parse_type()?);
                if !self.eat(TokenKind::Comma) { break; }
            }
            self.expect(TokenKind::Gt)?;
        }
        Ok(TypeExpr::Named { name, generics })
    }

    fn parse_block_after_open(&mut self, start: Span) -> Result<Block> {
        let mut stmts = Vec::new();
        let mut final_expr = None;
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) { return Err(self.expected("'}'")); }
            if self.eat(TokenKind::Let) {
                let span = self.previous_span();
                self.eat(TokenKind::Mut);
                let name = self.expect_ident()?;
                let type_ann = if self.eat(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
                self.expect(TokenKind::Eq)?;
                let value = self.parse_expr()?;
                self.eat(TokenKind::Semicolon);
                stmts.push(Stmt::Let { name, type_ann, value, span });
                continue;
            }
            if self.at_any(&[TokenKind::Fn, TokenKind::Extern, TokenKind::Struct, TokenKind::Enum, TokenKind::Const]) {
                stmts.push(Stmt::Item(self.parse_item()?));
                continue;
            }
            let expr = self.parse_expr()?;
            if self.eat(TokenKind::Semicolon) {
                stmts.push(Stmt::Expr(expr));
            } else if self.at(TokenKind::RBrace) {
                final_expr = Some(Box::new(expr));
                break;
            } else {
                // Titan permits newline-separated statements. Expression parsing naturally
                // stops when the following token cannot continue the expression.
                stmts.push(Stmt::Expr(expr));
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Block { stmts, final_expr, span: start })
    }

    fn parse_expr(&mut self) -> Result<Expr> { self.parse_assignment() }

    fn parse_assignment(&mut self) -> Result<Expr> {
        let left = self.parse_range()?;
        let op = match self.peek_kind() {
            Some(TokenKind::Eq) => { self.advance(); Some(None) },
            Some(TokenKind::PlusEq) => { self.advance(); Some(Some(BinaryOp::Add)) },
            Some(TokenKind::MinusEq) => { self.advance(); Some(Some(BinaryOp::Sub)) },
            Some(TokenKind::StarEq) => { self.advance(); Some(Some(BinaryOp::Mul)) },
            Some(TokenKind::SlashEq) => { self.advance(); Some(Some(BinaryOp::Div)) },
            Some(TokenKind::PercentEq) => { self.advance(); Some(Some(BinaryOp::Mod)) },
            _ => None,
        };
        if let Some(op) = op {
            let span = left.span();
            let value = self.parse_assignment()?;
            Ok(Expr::Assign { target: Box::new(left), op, value: Box::new(value), span })
        } else { Ok(left) }
    }

    fn parse_range(&mut self) -> Result<Expr> {
        let left = self.parse_binary(0)?;
        if self.eat(TokenKind::Range) || self.eat(TokenKind::RangeInclusive) {
            let inclusive = matches!(self.tokens[self.pos - 1].kind, TokenKind::RangeInclusive);
            let span = left.span();
            let end = self.parse_binary(0)?;
            Ok(Expr::Range { start: Box::new(left), end: Box::new(end), inclusive, span })
        } else { Ok(left) }
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        while let Some((op, precedence)) = self.binary_op() {
            if precedence < min_precedence { break; }
            self.advance();
            let right = self.parse_binary(precedence + 1)?;
            let span = left.span();
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn binary_op(&self) -> Option<(BinaryOp, u8)> {
        Some(match self.peek_kind()? {
            TokenKind::LazyOr => (BinaryOp::LazyOr, 1), TokenKind::LazyAnd => (BinaryOp::LazyAnd, 2),
            TokenKind::Pipe => (BinaryOp::Or, 3), TokenKind::Caret => (BinaryOp::Xor, 4),
            TokenKind::Ampersand => (BinaryOp::And, 5),
            TokenKind::EqEq => (BinaryOp::Eq, 6), TokenKind::NotEq => (BinaryOp::Neq, 6),
            TokenKind::Lt => (BinaryOp::Lt, 7), TokenKind::Gt => (BinaryOp::Gt, 7),
            TokenKind::LtEq => (BinaryOp::Lte, 7), TokenKind::GtEq => (BinaryOp::Gte, 7),
            TokenKind::Plus => (BinaryOp::Add, 8), TokenKind::Minus => (BinaryOp::Sub, 8),
            TokenKind::Star => (BinaryOp::Mul, 9), TokenKind::Slash => (BinaryOp::Div, 9),
            TokenKind::Percent => (BinaryOp::Mod, 9), _ => return None,
        })
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        let span = self.span();
        let op = match self.peek_kind() {
            Some(TokenKind::Minus) => Some(UnaryOp::Neg), Some(TokenKind::Bang) => Some(UnaryOp::Not),
            Some(TokenKind::Tilde) => Some(UnaryOp::BitNot), Some(TokenKind::Star) => Some(UnaryOp::Deref),
            Some(TokenKind::Ampersand) => Some(UnaryOp::Ref), _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let op = if op == UnaryOp::Ref && self.eat(TokenKind::Mut) { UnaryOp::RefMut } else { op };
            return Ok(Expr::Unary { op, expr: Box::new(self.parse_unary()?), span });
        }
        if self.eat(TokenKind::Spawn) || self.eat(TokenKind::Go) {
            return Ok(Expr::Spawn { expr: Box::new(self.parse_unary()?), span });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.eat(TokenKind::LParen) {
                let mut args = Vec::new();
                while !self.at(TokenKind::RParen) {
                    args.push(self.parse_expr()?);
                    if !self.eat(TokenKind::Comma) { break; }
                }
                self.expect(TokenKind::RParen)?;
                let span = expr.span();
                expr = Expr::Call { callee: Box::new(expr), args, span };
            } else if self.eat(TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                let span = expr.span();
                expr = Expr::Index { target: Box::new(expr), index: Box::new(index), span };
            } else if self.eat(TokenKind::Dot) {
                let name = self.expect_ident()?;
                let span = expr.span();
                if self.eat(TokenKind::LParen) {
                    let mut args = Vec::new();
                    while !self.at(TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        if !self.eat(TokenKind::Comma) { break; }
                    }
                    self.expect(TokenKind::RParen)?;
                    expr = Expr::MethodCall { receiver: Box::new(expr), method: name, args, span };
                } else { expr = Expr::FieldAccess { target: Box::new(expr), field: name, span }; }
            } else if self.eat(TokenKind::Question) {
                let span = expr.span();
                expr = Expr::Try { expr: Box::new(expr), span };
            } else { break; }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        let span = self.span();
        match self.peek_kind().cloned() {
            Some(TokenKind::IntLit(value)) => { self.advance(); Ok(Expr::Int { value: parse_int(&value, span)?, span }) }
            Some(TokenKind::FloatLit(value)) => { self.advance(); Ok(Expr::Float { value: parse_float(&value, span)?, span }) }
            Some(TokenKind::StringLit(value)) => {
                self.advance();
                if contains_interpolation(&value) { Ok(Expr::StringTemplate { value, span }) } else { Ok(Expr::String { value, span }) }
            }
            Some(TokenKind::CharLit(value)) => { self.advance(); Ok(Expr::Char { value, span }) }
            Some(TokenKind::True) => { self.advance(); Ok(Expr::Bool { value: true, span }) }
            Some(TokenKind::False) => { self.advance(); Ok(Expr::Bool { value: false, span }) }
            Some(TokenKind::Nil) => { self.advance(); Ok(Expr::Nil { span }) }
            Some(TokenKind::Self_) => { self.advance(); Ok(Expr::Ident { name: "self".into(), span }) }
            Some(TokenKind::Ident(name)) => {
                self.advance();
                let mut qualified = name;
                while self.eat(TokenKind::ColonColon) { qualified.push_str("::"); qualified.push_str(&self.expect_ident()?); }
                if self.at(TokenKind::LBrace) && qualified.chars().next().is_some_and(char::is_uppercase) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !self.at(TokenKind::RBrace) {
                        let field = self.expect_ident()?;
                        let value = if self.eat(TokenKind::Colon) { self.parse_expr()? } else { Expr::Ident { name: field.clone(), span } };
                        fields.push((field, value));
                        if !self.eat(TokenKind::Comma) { break; }
                    }
                    self.expect(TokenKind::RBrace)?;
                    Ok(Expr::StructLit { name: qualified, fields, span })
                } else { Ok(Expr::Ident { name: qualified, span }) }
            }
            Some(TokenKind::LParen) => {
                self.advance();
                if self.eat(TokenKind::RParen) { return Ok(Expr::Tuple { elements: Vec::new(), span }); }
                let first = self.parse_expr()?;
                if self.eat(TokenKind::Comma) {
                    let mut elements = vec![first];
                    while !self.at(TokenKind::RParen) { elements.push(self.parse_expr()?); if !self.eat(TokenKind::Comma) { break; } }
                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::Tuple { elements, span })
                } else { self.expect(TokenKind::RParen)?; Ok(first) }
            }
            Some(TokenKind::LBracket) => {
                self.advance();
                let mut elements = Vec::new();
                while !self.at(TokenKind::RBracket) { elements.push(self.parse_expr()?); if !self.eat(TokenKind::Comma) { break; } }
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::Array { elements, span })
            }
            Some(TokenKind::LBrace) => { self.advance(); Ok(Expr::Block(Box::new(self.parse_block_after_open(span)?))) }
            Some(TokenKind::Pipe) | Some(TokenKind::LazyOr) => self.parse_closure(),
            Some(TokenKind::If) => self.parse_if(),
            Some(TokenKind::While) => self.parse_while(),
            Some(TokenKind::For) => self.parse_for(),
            Some(TokenKind::Loop) => self.parse_loop(),
            Some(TokenKind::Match) => self.parse_match(),
            Some(TokenKind::Return) => {
                self.advance();
                let value = if self.at_any(&[TokenKind::RBrace, TokenKind::Semicolon, TokenKind::Eof]) { None } else { Some(Box::new(self.parse_expr()?)) };
                Ok(Expr::Return { value, span })
            }
            Some(TokenKind::Break) => {
                self.advance();
                let value = if self.at_any(&[TokenKind::RBrace, TokenKind::Semicolon, TokenKind::Eof]) { None } else { Some(Box::new(self.parse_expr()?)) };
                Ok(Expr::Break { value, span })
            }
            Some(TokenKind::Continue) => { self.advance(); Ok(Expr::Continue { span }) }
            _ => Err(self.expected("an expression")),
        }
    }

    fn parse_closure(&mut self) -> Result<Expr> {
        let span = self.span();
        if self.eat(TokenKind::LazyOr) {
            let return_type = if self.eat(TokenKind::ThinArrow) { Some(self.parse_type()?) } else { None };
            return Ok(Expr::Closure { params: Vec::new(), return_type, body: Box::new(self.parse_expr()?), span });
        }
        self.expect(TokenKind::Pipe)?;
        let mut params = Vec::new();
        while !self.at(TokenKind::Pipe) {
            let param_span = self.span();
            let name = self.expect_ident()?;
            let type_ann = if self.eat(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
            params.push(Param { name, type_ann, default: None, span: param_span });
            if !self.eat(TokenKind::Comma) { break; }
        }
        self.expect(TokenKind::Pipe)?;
        let return_type = if self.eat(TokenKind::ThinArrow) { Some(self.parse_type()?) } else { None };
        let body = Box::new(self.parse_expr()?);
        Ok(Expr::Closure { params, return_type, body, span })
    }

    fn parse_if(&mut self) -> Result<Expr> {
        let span = self.expect(TokenKind::If)?;
        let condition = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;
        let then_branch = self.parse_block_after_open(span)?;
        let else_branch = if self.eat(TokenKind::Else) {
            if self.at(TokenKind::If) {
                let nested = self.parse_if()?;
                Some(Block { stmts: Vec::new(), final_expr: Some(Box::new(nested)), span })
            } else { self.expect(TokenKind::LBrace)?; Some(self.parse_block_after_open(span)?) }
        } else { None };
        Ok(Expr::If { condition, then_branch, else_branch, span })
    }

    fn parse_while(&mut self) -> Result<Expr> {
        let span = self.expect(TokenKind::While)?;
        let condition = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;
        let body = self.parse_block_after_open(span)?;
        Ok(Expr::While { condition, body, span })
    }

    fn parse_for(&mut self) -> Result<Expr> {
        let span = self.expect(TokenKind::For)?;
        let pattern = Box::new(self.parse_pattern()?);
        self.expect(TokenKind::In)?;
        let iterator = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;
        let body = self.parse_block_after_open(span)?;
        Ok(Expr::For { pattern, iterator, body, span })
    }

    fn parse_loop(&mut self) -> Result<Expr> {
        let span = self.expect(TokenKind::Loop)?;
        self.expect(TokenKind::LBrace)?;
        Ok(Expr::Loop { body: self.parse_block_after_open(span)?, span })
    }

    fn parse_match(&mut self) -> Result<Expr> {
        let span = self.expect(TokenKind::Match)?;
        let scrutinee = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(TokenKind::RBrace) {
            let arm_span = self.span();
            let pattern = self.parse_pattern()?;
            let guard = if self.eat(TokenKind::If) { Some(Box::new(self.parse_expr()?)) } else { None };
            self.expect(TokenKind::FatArrow)?;
            let body = if self.eat(TokenKind::LBrace) { self.parse_block_after_open(arm_span)? } else {
                Block { stmts: Vec::new(), final_expr: Some(Box::new(self.parse_expr()?)), span: arm_span }
            };
            arms.push(MatchArm { pattern, guard, body, span: arm_span });
            self.eat(TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expr::Match { scrutinee, arms, span })
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        let span = self.span();
        let mut pattern = match self.peek_kind().cloned() {
            Some(TokenKind::Underscore) => { self.advance(); Pattern::Wildcard { span } }
            Some(TokenKind::Ident(name)) => {
                self.advance();
                if self.eat(TokenKind::ColonColon) {
                    let variant = self.expect_ident()?;
                    let inner = if self.eat(TokenKind::LParen) { let p = self.parse_pattern()?; self.expect(TokenKind::RParen)?; Some(Box::new(p)) } else { None };
                    Pattern::Enum { name, variant, inner, span }
                } else { Pattern::Ident { name, span } }
            }
            Some(TokenKind::IntLit(value)) => { self.advance(); Pattern::Literal { value: Box::new(Expr::Int { value: parse_int(&value, span)?, span }), span } }
            Some(TokenKind::StringLit(value)) => { self.advance(); Pattern::Literal { value: Box::new(Expr::String { value, span }), span } }
            Some(TokenKind::True) => { self.advance(); Pattern::Literal { value: Box::new(Expr::Bool { value: true, span }), span } }
            Some(TokenKind::False) => { self.advance(); Pattern::Literal { value: Box::new(Expr::Bool { value: false, span }), span } }
            _ => return Err(self.expected("a pattern")),
        };
        while self.eat(TokenKind::Pipe) { pattern = Pattern::Or { left: Box::new(pattern), right: Box::new(self.parse_pattern()?), span }; }
        Ok(pattern)
    }

    fn peek_kind(&self) -> Option<&TokenKind> { self.tokens.get(self.pos).map(|t| &t.kind) }
    fn span(&self) -> Span { self.tokens.get(self.pos).map(|t| t.span).unwrap_or_default() }
    fn previous_span(&self) -> Span { self.tokens.get(self.pos.saturating_sub(1)).map(|t| t.span).unwrap_or_default() }
    fn advance(&mut self) -> Option<Token> { let token = self.tokens.get(self.pos).cloned()?; self.pos += 1; Some(token) }
    fn at(&self, kind: TokenKind) -> bool { self.peek_kind().is_some_and(|k| same_variant(k, &kind)) }
    fn at_any(&self, kinds: &[TokenKind]) -> bool { kinds.iter().any(|k| self.at(k.clone())) }
    fn eat(&mut self, kind: TokenKind) -> bool { if self.at(kind) { self.advance(); true } else { false } }
    fn expect(&mut self, kind: TokenKind) -> Result<Span> {
        if self.at(kind.clone()) { Ok(self.advance().unwrap().span) } else { Err(self.expected(&format!("{:?}", kind))) }
    }
    fn expect_ident(&mut self) -> Result<String> {
        if let Some(TokenKind::Ident(name)) = self.peek_kind().cloned() { self.advance(); Ok(name) } else { Err(self.expected("an identifier")) }
    }
    fn expected(&self, expected: &str) -> ParseError {
        let token = self.tokens.get(self.pos);
        ParseError::Expected {
            expected: expected.into(),
            found: token.map(|t| format!("{:?}", t.kind)).unwrap_or_else(|| "end of input".into()),
            line: token.map_or(0, |t| t.span.line), column: token.map_or(0, |t| t.span.column),
        }
    }
    fn message(&self, message: &str) -> ParseError { let s = self.span(); ParseError::Message { message: message.into(), line: s.line, column: s.column } }
    fn synchronize_item(&mut self) {
        while !self.at(TokenKind::Eof) {
            if self.at_any(&[TokenKind::Fn, TokenKind::Extern, TokenKind::Struct, TokenKind::Enum, TokenKind::Trait, TokenKind::Impl, TokenKind::Module, TokenKind::Import, TokenKind::Const]) { return; }
            self.advance();
        }
    }
}

fn contains_interpolation(value: &str) -> bool {
    let mut remainder = value;
    while let Some(open) = remainder.find('{') {
        let after_open = &remainder[open + 1..];
        let Some(close) = after_open.find('}') else { return false };
        if valid_interpolation(after_open[..close].trim()) { return true; }
        remainder = &after_open[close + 1..];
    }
    false
}

fn valid_interpolation(value: &str) -> bool {
    if valid_identifier(value) { return true; }
    let Some((function, arguments)) = value.split_once('(') else { return false };
    if !value.ends_with(')') || !valid_identifier(function.trim()) { return false; }
    let arguments = &arguments[..arguments.len().saturating_sub(1)];
    arguments.trim().is_empty() || arguments.split(',').map(str::trim).all(|argument| !argument.is_empty() && (valid_identifier(argument) || argument.parse::<i64>().is_ok()))
}

fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first == '_' || first.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}

fn same_variant(a: &TokenKind, b: &TokenKind) -> bool { std::mem::discriminant(a) == std::mem::discriminant(b) }

fn parse_int(value: &str, span: Span) -> Result<i64> {
    value.replace('_', "").parse().map_err(|_| ParseError::Message { message: format!("integer literal out of range: {value}"), line: span.line, column: span.column })
}
fn parse_float(value: &str, span: Span) -> Result<f64> {
    value.replace('_', "").parse().map_err(|_| ParseError::Message { message: format!("invalid float literal: {value}"), line: span.line, column: span.column })
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_lexer::Lexer;

    fn parse(source: &str) -> Result<Program> {
        let mut lexer = Lexer::new(source);
        let (tokens, errors) = lexer.tokenize();
        assert!(errors.is_empty(), "{errors:?}");
        Parser::new(tokens.to_vec()).parse_program()
    }

    #[test]
    fn parses_functions_types_and_control_flow() {
        let program = parse("fn fib(n: int) -> int { if n <= 1 { return n } fib(n-1) + fib(n-2) }").unwrap();
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn parses_declarations_and_match() {
        let source = "struct Point { x: int, y: int } enum Maybe { None, Some(int) } fn main() { match true { true => 1, _ => 0 } }";
        assert_eq!(parse(source).unwrap().items.len(), 3);
    }

    #[test]
    fn parses_closures_and_try_operator() {
        assert!(parse("fn main() { let add = |x: int, y: int| -> int x + y add(1, 2) }").is_ok());
        assert!(parse("fn unwrap(value: Result) { value? }").is_ok());
    }

    #[test]
    fn distinguishes_json_braces_from_interpolation() {
        let program = parse(r#"fn main() { "{\"answer\":42}" }"#).unwrap();
        let Item::Function(function) = &program.items[0] else { panic!("expected function") };
        let expression = function.body.as_ref().and_then(|body| body.final_expr.as_deref()).unwrap();
        assert!(matches!(expression, Expr::String { .. }));
        assert!(contains_interpolation("answer={answer}"));
    }

    #[test]
    fn rejects_invalid_programs() { assert!(parse("fn broken( {").is_err()); }

    #[test]
    fn parses_extern_functions() {
        let program = parse("extern \"C\" fn puts(s: &str) -> int; extern fn getpid() -> int;").unwrap();
        assert_eq!(program.items.len(), 2);
        if let Item::Function(f1) = &program.items[0] {
            assert!(f1.is_extern);
            assert_eq!(f1.abi.as_deref(), Some("C"));
            assert!(f1.body.is_none());
        } else {
            panic!("expected function");
        }
        if let Item::Function(f2) = &program.items[1] {
            assert!(f2.is_extern);
            assert_eq!(f2.abi.as_deref(), Some("C"));
            assert!(f2.body.is_none());
        } else {
            panic!("expected function");
        }
    }
}
