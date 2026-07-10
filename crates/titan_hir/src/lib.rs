//! Titan HIR — High-level Intermediate Representation.

use titan_ast::*;

pub struct HirProgram {
    pub functions: Vec<HirFunction>,
    pub structs: Vec<StructDecl>,
    pub enums: Vec<EnumDecl>,
}

pub struct HirFunction {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Option<Block>,
}

pub fn lower_to_hir(program: &Program) -> HirProgram {
    let mut hir = HirProgram { functions: Vec::new(), structs: Vec::new(), enums: Vec::new() };
    for item in &program.items {
        match item {
            Item::Function(f) => {
                hir.functions.push(HirFunction {
                    name: f.name.clone(),
                    params: f.params.clone(),
                    body: f.body.clone(),
                });
            }
            Item::Struct(s) => hir.structs.push(s.clone()),
            Item::Enum(e) => hir.enums.push(e.clone()),
            _ => {}
        }
    }
    hir
}