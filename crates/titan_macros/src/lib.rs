//! Titan Macros — Macro expansion engine.
// Use titan_ast macros indirectly via registry

pub struct MacroRegistry {
    macros: std::collections::HashMap<String, String>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        MacroRegistry {
            macros: std::collections::HashMap::new(),
        }
    }
    pub fn register(&mut self, name: &str, body: &str) {
        self.macros.insert(name.to_string(), body.to_string());
    }
    pub fn contains(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }
}

impl Default for MacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}
