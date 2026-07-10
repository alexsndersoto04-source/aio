//! Titan Type Checker — Type inference and validation.

use thiserror::Error;
use titan_ast::*;

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Type mismatch: expected {expected}, got {found}")]
    Mismatch { expected: String, found: String },
    #[error("Unknown variable: {name}")]
    UnknownVariable { name: String },
}

pub struct TypeEnv {
    variables: std::collections::HashMap<String, String>,
}

impl TypeEnv {
    pub fn new() -> Self {
        TypeEnv { variables: std::collections::HashMap::new() }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<TypeError>> {
        for item in &program.items {
            self.check_item(item);
        }
        Ok(())
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => {
                for p in &f.params {
                    self.variables.insert(p.name.clone(), "unknown".into());
                }
            }
            _ => {}
        }
    }
}

impl Default for TypeEnv {
    fn default() -> Self { Self::new() }
}