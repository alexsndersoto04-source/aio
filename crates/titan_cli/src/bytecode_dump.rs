//! Canonical text dump of a compiled module (self-hosting oracle).
//!
//! `titan bytecode FILE` prints this so the code generator written in Titan
//! (`selfhost/codegen.titan`) can be compared byte for byte with this one.
//! Deterministic: hash maps are printed sorted by key. Strings and chars use
//! the same escaping as the AST dump (`titan_parser::dump_escape`); floats use
//! `Display`, like the AST dump. Everything else uses the derived `Debug`.

use titan_codegen::{BytecodeFunc, CompiledModule, Op, SourceLocation};

fn q(text: &str) -> String {
    format!("\"{}\"", titan_parser::dump_escape(text))
}

fn op_text(op: &Op) -> String {
    match op {
        Op::PushFloat(value) => format!("PushFloat({value})"),
        Op::PushChar(value) => format!("PushChar({})", q(&value.to_string())),
        other => format!("{other:?}"),
    }
}

fn location(location: &Option<SourceLocation>) -> String {
    match location {
        Some(l) => format!("@{}:{}:{}..{}", l.line, l.column, l.start, l.end),
        None => "@-".into(),
    }
}

fn function(out: &mut String, index: usize, f: &BytecodeFunc) {
    out.push_str(&format!("fn {index} {}\n", q(&f.name)));
    match &f.source_file {
        Some(file) => out.push_str(&format!("  file {}\n", q(file))),
        None => out.push_str("  file-\n"),
    }
    out.push_str(&format!(
        "  arity {} captures {} locals {} max_stack {}\n",
        f.arity, f.captures, f.locals, f.max_stack
    ));
    for param in &f.param_types {
        out.push_str(&format!("  param {param:?}\n"));
    }
    match &f.return_type {
        Some(ty) => out.push_str(&format!("  return {ty:?}\n")),
        None => out.push_str("  return-\n"),
    }
    for (pc, op) in f.code.iter().enumerate() {
        let loc = f.debug_locations.get(pc).copied().flatten();
        out.push_str(&format!("  {pc} {} {}\n", op_text(op), location(&loc)));
    }
}

pub fn dump_module(module: &CompiledModule) -> String {
    let mut out = String::new();
    out.push_str(&format!("entry {}\n", module.entry));
    for (index, text) in module.string_table.iter().enumerate() {
        out.push_str(&format!("string {index} {}\n", q(text)));
    }
    let mut structs: Vec<_> = module.struct_schemas.iter().collect();
    structs.sort();
    for (name, fields) in structs {
        let fields: Vec<String> = fields.iter().map(|f| q(f)).collect();
        out.push_str(&format!("struct {} [{}]\n", q(name), fields.join(", ")));
    }
    let mut enums: Vec<_> = module.enum_schemas.iter().collect();
    enums.sort();
    for (name, variants) in enums {
        let variants: Vec<String> = variants.iter().map(|v| q(v)).collect();
        out.push_str(&format!("enum {} [{}]\n", q(name), variants.join(", ")));
    }
    let mut methods: Vec<_> = module.method_table.iter().collect();
    methods.sort();
    for (name, index) in methods {
        out.push_str(&format!("method {} {index}\n", q(name)));
    }
    for (index, f) in module.functions.iter().enumerate() {
        function(&mut out, index, f);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_codegen::AstCompiler;

    #[test]
    fn dump_is_deterministic_and_lists_every_function() {
        let source = "struct P { x: int } impl P { fn get(self) -> int { self.x } } \
                      fn main() { let p = P { x: 1 } print(p.get(), 2.5, 'a', \"t\") }";
        let mut lexer = titan_lexer::Lexer::new(source);
        let tokens = lexer.tokenize().0.to_vec();
        let program = titan_parser::Parser::new(tokens).parse_program().unwrap();
        let first = dump_module(&AstCompiler::new().compile_program(&program).unwrap());
        let second = dump_module(&AstCompiler::new().compile_program(&program).unwrap());
        assert_eq!(first, second);
        assert!(first.contains("fn 0 \"P::get\""), "{first}");
        assert!(first.contains("method \"P::get\" 0"), "{first}");
        assert!(first.contains("PushFloat(2.5)"), "{first}");
        assert!(first.contains("PushChar(\"a\")"), "{first}");
        assert!(first.contains("struct \"P\" [\"x\"]"), "{first}");
    }
}
