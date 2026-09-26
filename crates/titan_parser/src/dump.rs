//! Canonical, deterministic dump of the AST (self-hosting oracle, phase 2).
//!
//! `dump_ast(source)` runs the real lexer and the real parser and prints the
//! resulting tree in a fixed line-oriented format (one node per line, two
//! spaces of indentation per level, spans on every node that has one). The
//! parser written in Titan (`selfhost/parser.titan`) prints the same format,
//! so both parsers can be compared byte for byte.
//!
//! The pipeline mirrors the compiler (`titan_pkg::parse_file`): if the lexer
//! reports errors, parsing does not start and each lexer error is printed as
//! `lex-error: ...`; if the parser fails, every collected parse error is
//! printed as `error: ...`.
//!
//! This format is intentionally NOT Rust's `Debug` output: it only uses
//! decimal integers, `f64` `Display` (shortest round-trip digits, so distinct
//! values never print the same) and a small explicit string escape.

use crate::Parser;
use titan_ast::*;
use titan_lexer::{Lexer, Span};

pub fn dump_ast(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let (tokens, errors) = lexer.tokenize();
    let mut d = Dumper { out: String::new() };
    if !errors.is_empty() {
        for error in errors {
            d.out.push_str("lex-error: ");
            d.out.push_str(&esc(&error.to_string()));
            d.out.push('\n');
        }
        return d.out;
    }
    let mut parser = Parser::new(tokens.to_vec());
    match parser.parse_program() {
        Ok(program) => {
            d.line(0, "program");
            for item in &program.items {
                d.item(1, "", item);
            }
        }
        Err(_) => {
            for error in parser.errors() {
                d.out.push_str("error: ");
                d.out.push_str(&esc(&error.to_string()));
                d.out.push('\n');
            }
        }
    }
    d.out
}

/// Escape used for every string in the dump: backslash, double quote, the
/// usual control escapes, and any other char below U+0020 or U+007F as
/// `\u{hex}` (lowercase). Everything else is copied verbatim.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            other => out.push(other),
        }
    }
    out
}

fn q(text: &str) -> String {
    format!("\"{}\"", esc(text))
}

fn sp(span: Span) -> String {
    format!(
        "@{}:{}:{}..{}",
        span.line, span.column, span.start, span.end
    )
}

fn binop(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "Add",
        BinaryOp::Sub => "Sub",
        BinaryOp::Mul => "Mul",
        BinaryOp::Div => "Div",
        BinaryOp::Mod => "Mod",
        BinaryOp::And => "And",
        BinaryOp::Or => "Or",
        BinaryOp::Xor => "Xor",
        BinaryOp::Eq => "Eq",
        BinaryOp::Neq => "Neq",
        BinaryOp::Lt => "Lt",
        BinaryOp::Gt => "Gt",
        BinaryOp::Lte => "Lte",
        BinaryOp::Gte => "Gte",
        BinaryOp::LazyAnd => "LazyAnd",
        BinaryOp::LazyOr => "LazyOr",
    }
}

fn unop(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "Neg",
        UnaryOp::Not => "Not",
        UnaryOp::BitNot => "BitNot",
        UnaryOp::Ref => "Ref",
        UnaryOp::RefMut => "RefMut",
        UnaryOp::Deref => "Deref",
    }
}

struct Dumper {
    out: String,
}

impl Dumper {
    fn line(&mut self, depth: usize, text: &str) {
        for _ in 0..depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn item(&mut self, d: usize, label: &str, item: &Item) {
        match item {
            Item::Function(f) => self.function(d, label, f),
            Item::Struct(s) => {
                self.line(d, &format!("{label}struct {} {}", q(&s.name), sp(s.span)));
                for field in &s.fields {
                    self.line(
                        d + 1,
                        &format!("field {} {}", q(&field.name), sp(field.span)),
                    );
                    self.ty(d + 2, "type: ", &field.type_ann);
                }
            }
            Item::Enum(e) => {
                self.line(d, &format!("{label}enum {} {}", q(&e.name), sp(e.span)));
                for v in &e.variants {
                    self.line(d + 1, &format!("variant {} {}", q(&v.name), sp(v.span)));
                    self.opt_ty(d + 2, "payload: ", &v.payload);
                }
            }
            Item::Trait(t) => {
                self.line(d, &format!("{label}trait {} {}", q(&t.name), sp(t.span)));
                for m in &t.methods {
                    self.line(d + 1, &format!("method {} {}", q(&m.name), sp(m.span)));
                    for p in &m.params {
                        self.param(d + 2, p);
                    }
                    self.opt_ty(d + 2, "ret: ", &m.return_type);
                    self.opt_block(d + 2, "body: ", &m.body);
                }
            }
            Item::Impl(i) => {
                let tr = match &i.trait_name {
                    Some(name) => q(name),
                    None => "-".into(),
                };
                self.line(d, &format!("{label}impl trait={tr} {}", sp(i.span)));
                self.ty(d + 1, "target: ", &i.target_type);
                for m in &i.methods {
                    self.function(d + 1, "", m);
                }
            }
            Item::Module(m) => {
                self.line(d, &format!("{label}module {} {}", q(&m.name), sp(m.span)));
                for it in &m.items {
                    self.item(d + 1, "", it);
                }
            }
            Item::Import(i) => {
                self.line(
                    d,
                    &format!("{label}import {} {}", q(&i.path.join("::")), sp(i.span)),
                );
            }
            Item::Const(c) => {
                self.line(d, &format!("{label}const {} {}", q(&c.name), sp(c.span)));
                self.opt_ty(d + 1, "type: ", &c.type_ann);
                self.expr(d + 1, "value: ", &c.value);
            }
            Item::TypeAlias(t) => {
                self.line(d, &format!("{label}type {} {}", q(&t.name), sp(t.span)));
                self.ty(d + 1, "target: ", &t.target);
            }
        }
    }

    fn function(&mut self, d: usize, label: &str, f: &FunctionDecl) {
        let abi = match &f.abi {
            Some(a) => q(a),
            None => "-".into(),
        };
        self.line(
            d,
            &format!(
                "{label}fn {} extern={} abi={abi} {}",
                q(&f.name),
                f.is_extern,
                sp(f.span)
            ),
        );
        for p in &f.params {
            self.param(d + 1, p);
        }
        self.opt_ty(d + 1, "ret: ", &f.return_type);
        self.opt_block(d + 1, "body: ", &f.body);
    }

    fn param(&mut self, d: usize, p: &Param) {
        self.line(
            d,
            &format!("param {} mut={} {}", q(&p.name), p.mutable, sp(p.span)),
        );
        self.opt_ty(d + 1, "type: ", &p.type_ann);
        match &p.default {
            Some(e) => self.expr(d + 1, "default: ", e),
            None => self.line(d + 1, "default: -"),
        }
    }

    fn opt_ty(&mut self, d: usize, label: &str, t: &Option<TypeExpr>) {
        match t {
            Some(t) => self.ty(d, label, t),
            None => self.line(d, &format!("{label}-")),
        }
    }

    fn ty(&mut self, d: usize, label: &str, t: &TypeExpr) {
        match t {
            TypeExpr::Named { name, generics } => {
                self.line(d, &format!("{label}named {}", q(name)));
                for g in generics {
                    self.ty(d + 1, "", g);
                }
            }
            TypeExpr::Reference { inner, is_mut } => {
                self.line(d, &format!("{label}ref mut={is_mut}"));
                self.ty(d + 1, "", inner);
            }
            TypeExpr::Slice { inner } => {
                self.line(d, &format!("{label}slice"));
                self.ty(d + 1, "", inner);
            }
            TypeExpr::Array { inner, size } => {
                self.line(d, &format!("{label}array-type"));
                self.ty(d + 1, "inner: ", inner);
                self.expr(d + 1, "size: ", size);
            }
            TypeExpr::Tuple { elements } => {
                self.line(d, &format!("{label}tuple-type"));
                for e in elements {
                    self.ty(d + 1, "", e);
                }
            }
            TypeExpr::Function {
                params,
                return_type,
            } => {
                self.line(d, &format!("{label}fn-type"));
                for p in params {
                    self.ty(d + 1, "param: ", p);
                }
                self.ty(d + 1, "ret: ", return_type);
            }
            TypeExpr::Unit => self.line(d, &format!("{label}unit")),
            TypeExpr::Never => self.line(d, &format!("{label}never")),
            TypeExpr::Infer(n) => self.line(d, &format!("{label}infer {n}")),
        }
    }

    fn opt_block(&mut self, d: usize, label: &str, b: &Option<Block>) {
        match b {
            Some(b) => self.block(d, label, b),
            None => self.line(d, &format!("{label}-")),
        }
    }

    fn block(&mut self, d: usize, label: &str, b: &Block) {
        self.line(d, &format!("{label}block {}", sp(b.span)));
        for s in &b.stmts {
            self.stmt(d + 1, s);
        }
        self.opt_expr(d + 1, "final: ", &b.final_expr);
    }

    fn stmt(&mut self, d: usize, s: &Stmt) {
        match s {
            Stmt::Expr(e) => self.expr(d, "expr: ", e),
            Stmt::Let {
                name,
                mutable,
                type_ann,
                value,
                span,
            } => {
                self.line(
                    d,
                    &format!("let-stmt {} mut={mutable} {}", q(name), sp(*span)),
                );
                self.opt_ty(d + 1, "type: ", type_ann);
                self.expr(d + 1, "value: ", value);
            }
            Stmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                let op = op.map_or("-", binop);
                self.line(d, &format!("assign-stmt op={op} {}", sp(*span)));
                self.expr(d + 1, "target: ", target);
                self.expr(d + 1, "value: ", value);
            }
            Stmt::Item(item) => self.item(d, "item: ", item),
        }
    }

    fn opt_expr(&mut self, d: usize, label: &str, e: &Option<Box<Expr>>) {
        match e {
            Some(e) => self.expr(d, label, e),
            None => self.line(d, &format!("{label}-")),
        }
    }

    fn expr(&mut self, d: usize, label: &str, e: &Expr) {
        match e {
            Expr::Int { value, span } => self.line(d, &format!("{label}int {value} {}", sp(*span))),
            Expr::Float { value, span } => {
                self.line(d, &format!("{label}float {value} {}", sp(*span)))
            }
            Expr::String { value, span } => {
                self.line(d, &format!("{label}string {} {}", q(value), sp(*span)))
            }
            Expr::StringTemplate { value, span } => {
                self.line(d, &format!("{label}template {} {}", q(value), sp(*span)))
            }
            Expr::Char { value, span } => self.line(
                d,
                &format!("{label}char {} {}", q(&value.to_string()), sp(*span)),
            ),
            Expr::Bool { value, span } => {
                self.line(d, &format!("{label}bool {value} {}", sp(*span)))
            }
            Expr::Nil { span } => self.line(d, &format!("{label}nil {}", sp(*span))),
            Expr::Ident { name, span } => {
                self.line(d, &format!("{label}ident {} {}", q(name), sp(*span)))
            }
            Expr::Array { elements, span } => {
                self.line(d, &format!("{label}array {}", sp(*span)));
                for el in elements {
                    self.expr(d + 1, "", el);
                }
            }
            Expr::Tuple { elements, span } => {
                self.line(d, &format!("{label}tuple {}", sp(*span)));
                for el in elements {
                    self.expr(d + 1, "", el);
                }
            }
            Expr::StructLit { name, fields, span } => {
                self.line(d, &format!("{label}structlit {} {}", q(name), sp(*span)));
                for (fname, value) in fields {
                    self.expr(d + 1, &format!("{} = ", q(fname)), value);
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                self.line(d, &format!("{label}binary {} {}", binop(*op), sp(*span)));
                self.expr(d + 1, "", left);
                self.expr(d + 1, "", right);
            }
            Expr::Range {
                start,
                end,
                inclusive,
                span,
            } => {
                self.line(
                    d,
                    &format!("{label}range inclusive={inclusive} {}", sp(*span)),
                );
                self.expr(d + 1, "", start);
                self.expr(d + 1, "", end);
            }
            Expr::Unary { op, expr, span } => {
                self.line(d, &format!("{label}unary {} {}", unop(*op), sp(*span)));
                self.expr(d + 1, "", expr);
            }
            Expr::Call { callee, args, span } => {
                self.line(d, &format!("{label}call {}", sp(*span)));
                self.expr(d + 1, "callee: ", callee);
                for a in args {
                    self.expr(d + 1, "arg: ", a);
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => {
                self.line(d, &format!("{label}methodcall {} {}", q(method), sp(*span)));
                self.expr(d + 1, "recv: ", receiver);
                for a in args {
                    self.expr(d + 1, "arg: ", a);
                }
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                self.line(d, &format!("{label}index {}", sp(*span)));
                self.expr(d + 1, "", target);
                self.expr(d + 1, "", index);
            }
            Expr::FieldAccess {
                target,
                field,
                span,
            } => {
                self.line(d, &format!("{label}field {} {}", q(field), sp(*span)));
                self.expr(d + 1, "", target);
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                self.line(d, &format!("{label}if {}", sp(*span)));
                self.expr(d + 1, "cond: ", condition);
                self.block(d + 1, "then: ", then_branch);
                self.opt_block(d + 1, "else: ", else_branch);
            }
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => {
                self.line(d, &format!("{label}match {}", sp(*span)));
                self.expr(d + 1, "scrutinee: ", scrutinee);
                for arm in arms {
                    self.line(d + 1, &format!("arm {}", sp(arm.span)));
                    self.pattern(d + 2, "pat: ", &arm.pattern);
                    self.opt_expr(d + 2, "guard: ", &arm.guard);
                    self.block(d + 2, "body: ", &arm.body);
                }
            }
            Expr::For {
                pattern,
                iterator,
                body,
                span,
            } => {
                self.line(d, &format!("{label}for {}", sp(*span)));
                self.pattern(d + 1, "pat: ", pattern);
                self.expr(d + 1, "iter: ", iterator);
                self.block(d + 1, "body: ", body);
            }
            Expr::While {
                condition,
                body,
                span,
            } => {
                self.line(d, &format!("{label}while {}", sp(*span)));
                self.expr(d + 1, "cond: ", condition);
                self.block(d + 1, "body: ", body);
            }
            Expr::Loop { body, span } => {
                self.line(d, &format!("{label}loop {}", sp(*span)));
                self.block(d + 1, "body: ", body);
            }
            Expr::Break { value, span } => {
                self.line(d, &format!("{label}break {}", sp(*span)));
                self.opt_expr(d + 1, "value: ", value);
            }
            Expr::Continue { span } => self.line(d, &format!("{label}continue {}", sp(*span))),
            Expr::Return { value, span } => {
                self.line(d, &format!("{label}return {}", sp(*span)));
                self.opt_expr(d + 1, "value: ", value);
            }
            Expr::Let {
                name,
                mutable,
                type_ann,
                value,
                span,
            } => {
                self.line(
                    d,
                    &format!("{label}let {} mut={mutable} {}", q(name), sp(*span)),
                );
                self.opt_ty(d + 1, "type: ", type_ann);
                self.expr(d + 1, "value: ", value);
            }
            Expr::Assign {
                target,
                op,
                value,
                span,
            } => {
                let op = op.map_or("-", binop);
                self.line(d, &format!("{label}assign op={op} {}", sp(*span)));
                self.expr(d + 1, "target: ", target);
                self.expr(d + 1, "value: ", value);
            }
            Expr::Block(b) => self.block(d, label, b),
            Expr::Spawn { expr, span } => {
                self.line(d, &format!("{label}spawn {}", sp(*span)));
                self.expr(d + 1, "", expr);
            }
            Expr::Try { expr, span } => {
                self.line(d, &format!("{label}try {}", sp(*span)));
                self.expr(d + 1, "", expr);
            }
            Expr::Closure {
                params,
                return_type,
                body,
                span,
            } => {
                self.line(d, &format!("{label}closure {}", sp(*span)));
                for p in params {
                    self.param(d + 1, p);
                }
                self.opt_ty(d + 1, "ret: ", return_type);
                self.expr(d + 1, "body: ", body);
            }
        }
    }

    fn pattern(&mut self, d: usize, label: &str, p: &Pattern) {
        match p {
            Pattern::Wildcard { span } => self.line(d, &format!("{label}pwild {}", sp(*span))),
            Pattern::Ident { name, span } => {
                self.line(d, &format!("{label}pident {} {}", q(name), sp(*span)))
            }
            Pattern::Literal { value, span } => {
                self.line(d, &format!("{label}plit {}", sp(*span)));
                self.expr(d + 1, "", value);
            }
            Pattern::Struct {
                name,
                fields,
                rest,
                span,
            } => {
                self.line(
                    d,
                    &format!("{label}pstruct {} rest={rest} {}", q(name), sp(*span)),
                );
                for (fname, sub) in fields {
                    self.pattern(d + 1, &format!("{} = ", q(fname)), sub);
                }
            }
            Pattern::Enum {
                name,
                variant,
                inner,
                span,
            } => {
                self.line(
                    d,
                    &format!("{label}penum {} {} {}", q(name), q(variant), sp(*span)),
                );
                match inner {
                    Some(i) => self.pattern(d + 1, "inner: ", i),
                    None => self.line(d + 1, "inner: -"),
                }
            }
            Pattern::Tuple { elements, span } => {
                self.line(d, &format!("{label}ptuple {}", sp(*span)));
                for el in elements {
                    self.pattern(d + 1, "", el);
                }
            }
            Pattern::Or { left, right, span } => {
                self.line(d, &format!("{label}por {}", sp(*span)));
                self.pattern(d + 1, "", left);
                self.pattern(d + 1, "", right);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::dump_ast;

    #[test]
    fn dumps_a_small_program() {
        let dump = dump_ast("fn main() { let x = 1 + 2.5\n x }");
        let expected = "\
program
  fn \"main\" extern=false abi=- @1:1:0..2
    ret: -
    body: block @1:1:0..2
      let-stmt \"x\" mut=false @1:13:12..15
        type: -
        value: binary Add @1:21:20..21
          int 1 @1:21:20..21
          float 2.5 @1:25:24..27
      final: ident \"x\" @2:2:29..30
";
        assert_eq!(dump, expected);
    }

    #[test]
    fn dumps_lexer_and_parser_errors() {
        assert_eq!(
            dump_ast("fn main() { \"abc }"),
            "lex-error: unterminated string at 1:13\n"
        );
        let dump = dump_ast("fn broken( {");
        assert!(dump.starts_with("error: expected an identifier, found LBrace at 1:12"));
    }

    #[test]
    fn escapes_control_characters() {
        let dump = dump_ast("fn f() { \"a\\tb\u{1}\" }");
        assert!(dump.contains("string \"a\\tb\\u{1}\""), "{dump}");
    }
}
