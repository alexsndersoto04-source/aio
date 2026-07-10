//! Titan Codegen — Bytecode compiler from AST.
use thiserror::Error;
use titan_ast::*;

#[derive(Error, Debug)]
pub enum CodegenError {
    #[error("Codegen: {0}")]
    Generic(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    PushInt(i64), PushFloat(f64), PushBool(bool), PushNil,
    PushStr(usize), PushLocal(usize), PushGlobal(usize),
    StoreLocal(usize), StoreGlobal(usize),
    Add, Sub, Mul, Div, Neg,
    Eq, Neq, Lt, Gt,
    Jump(usize), JumpIfFalse(usize), Loop(usize),
    Call(usize), Ret, RetVoid,
    Pop, NewArray(usize), NewTuple(usize),
    Print, Nop, Halt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunc {
    pub name: String,
    pub arity: usize,
    pub locals: usize,
    pub max_stack: usize,
    pub code: Vec<Op>,
    pub const_pool: Vec<Constant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64), Float(f64), Bool(bool), Str(String), Nil,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModule {
    pub functions: Vec<BytecodeFunc>,
    pub entry: usize,
    pub string_table: Vec<String>,
    pub global_names: Vec<String>,
}

pub struct AstCompiler {
    module: CompiledModule,
    func: BytecodeFunc,
    locals: std::collections::HashMap<String, usize>,
    next_local: usize,
    strings: std::collections::HashMap<String, usize>,
    break_targets: Vec<usize>,
    continue_targets: Vec<usize>,
}

impl AstCompiler {
    pub fn new() -> Self {
        AstCompiler {
            module: CompiledModule {
                functions: Vec::new(), entry: 0,
                string_table: vec!["".into()], global_names: Vec::new(),
            },
            func: BytecodeFunc {
                name: String::new(), arity: 0, locals: 0, max_stack: 32,
                code: Vec::new(), const_pool: Vec::new(),
            },
            locals: std::collections::HashMap::new(),
            next_local: 0,
            strings: std::collections::HashMap::new(),
            break_targets: Vec::new(), continue_targets: Vec::new(),
        }
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<CompiledModule, CodegenError> {
        let mut compiled = Vec::new();
        let mut entry = 0;
        for item in &program.items {
            if let Item::Function(f) = item {
                let bc = self.compile_function(f)?;
                let idx = compiled.len();
                if f.name == "main" { entry = idx; }
                compiled.push(bc);
            }
        }
        self.module.functions = compiled;
        self.module.entry = entry;
        Ok(self.module.clone())
    }

    fn compile_function(&mut self, func: &FunctionDecl) -> Result<BytecodeFunc, CodegenError> {
        self.func = BytecodeFunc {
            name: func.name.clone(), arity: func.params.len(),
            locals: func.params.len(), max_stack: 64,
            code: Vec::new(), const_pool: Vec::new(),
        };
        self.locals.clear();
        self.next_local = 0;
        self.break_targets.clear();
        self.continue_targets.clear();
        for p in &func.params {
            self.add_local(&p.name);
        }
        if let Some(body) = &func.body {
            self.compile_block(body)?;
        }
        self.func.code.push(Op::RetVoid);
        self.func.locals = self.next_local;
        Ok(self.func.clone())
    }

    fn compile_block(&mut self, block: &Block) -> Result<(), CodegenError> {
        for stmt in &block.stmts {
            self.compile_stmt(stmt)?;
        }
        if let Some(expr) = &block.final_expr {
            self.compile_expr(expr)?;
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        match stmt {
            Stmt::Expr(e) => {
                self.compile_expr(e)?;
                self.func.code.push(Op::Pop);
            }
            Stmt::Let { name, value, .. } => {
                self.compile_expr(value)?;
                let idx = self.add_local(name);
                self.func.code.push(Op::StoreLocal(idx));
            }
            Stmt::Assign { target, value, .. } => {
                self.compile_expr(value)?;
                if let Expr::Ident { name, .. } = target {
                    if let Some(&idx) = self.locals.get(name) {
                        self.func.code.push(Op::StoreLocal(idx));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), CodegenError> {
        match expr {
            Expr::Int { value, .. } => self.func.code.push(Op::PushInt(*value)),
            Expr::Float { value, .. } => self.func.code.push(Op::PushFloat(*value)),
            Expr::Bool { value, .. } => self.func.code.push(Op::PushBool(*value)),
            Expr::String { value, .. } => {
                let idx = self.string_intern(value);
                self.func.code.push(Op::PushStr(idx));
            }
            Expr::Nil { .. } => self.func.code.push(Op::PushNil),
            Expr::Ident { name, .. } => {
                if let Some(&idx) = self.locals.get(name) {
                    self.func.code.push(Op::PushLocal(idx));
                } else {
                    let gidx = self.global_intern(name);
                    self.func.code.push(Op::PushGlobal(gidx));
                }
            }
            Expr::Binary { left, op, right, .. } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                match op {
                    BinaryOp::Add => self.func.code.push(Op::Add),
                    BinaryOp::Sub => self.func.code.push(Op::Sub),
                    BinaryOp::Mul => self.func.code.push(Op::Mul),
                    BinaryOp::Div => self.func.code.push(Op::Div),
                    BinaryOp::Eq => self.func.code.push(Op::Eq),
                    BinaryOp::Neq => self.func.code.push(Op::Neq),
                    BinaryOp::Lt => self.func.code.push(Op::Lt),
                    BinaryOp::Gt => self.func.code.push(Op::Gt),
                    _ => {}
                }
            }
            Expr::Unary { op: UnaryOp::Neg, expr: inner, .. } => {
                self.compile_expr(inner)?;
                self.func.code.push(Op::Neg);
            }
            Expr::Unary { op: UnaryOp::Not, expr: inner, .. } => {
                self.compile_expr(inner)?;
                self.compile_expr(&Expr::Bool { value: false, span: titan_lexer::Span::new(0,0,0,0) })?;
                self.func.code.push(Op::Eq);
            }
            Expr::Call { callee, args, .. } => {
                // Check if callee is a builtin function like print()
                let is_print = matches!(callee.as_ref(), Expr::Ident { name, .. } if name == "print");
                
                for arg in args.iter().rev() {
                    self.compile_expr(arg)?;
                }
                
                if is_print {
                    // print() — pop one arg and print it
                    self.func.code.push(Op::Print);
                    // pop remaining args
                    for _ in 0..args.len().saturating_sub(1) {
                        self.func.code.push(Op::Pop);
                    }
                    if args.is_empty() {
                        self.func.code.push(Op::PushNil);
                    } else {
                        self.func.code.push(Op::PushNil);
                    }
                } else {
                    self.compile_expr(callee)?;
                    self.func.code.push(Op::Call(args.len()));
                    // Clean up
                    for _ in 0..args.len() { self.func.code.push(Op::Pop); }
                    self.func.code.push(Op::PushNil);
                }
            }
            Expr::If { condition, then_branch, else_branch, .. } => {
                self.compile_expr(condition)?;
                // Jump to else if false
                let jump_else = self.emit_jump(Op::JumpIfFalse(0));
                self.compile_block(then_branch)?;
                let jump_end = self.emit_jump(Op::Jump(0));
                let else_pos = self.func.code.len();
                self.patch_jump(jump_else, else_pos);
                if let Some(else_blk) = else_branch {
                    self.compile_block(else_blk)?;
                } else {
                    self.func.code.push(Op::PushNil);
                }
                let end_pos = self.func.code.len();
                self.patch_jump(jump_end, end_pos);
            }
            Expr::While { condition, body, .. } => {
                let loop_start = self.func.code.len();
                self.compile_expr(condition)?;
                let jump_exit = self.emit_jump(Op::JumpIfFalse(0));
                // Save old break/continue targets
                let saved_breaks = std::mem::take(&mut self.break_targets);
                let saved_continues = std::mem::take(&mut self.continue_targets);
                
                self.compile_block(body)?;
                self.func.code.push(Op::Loop(loop_start));
                let exit_pos = self.func.code.len();
                self.patch_jump(jump_exit, exit_pos);
                
                // Patch all break jumps - clone to avoid borrow conflict
                let breaks: Vec<usize> = self.break_targets.iter().copied().collect();
                for j in breaks {
                    self.patch_jump(j, exit_pos);
                }
                // Restore
                self.break_targets = saved_breaks;
                self.continue_targets = saved_continues;
                
                self.func.code.push(Op::PushNil);
            }
            Expr::Return { value, .. } => {
                if let Some(v) = value {
                    self.compile_expr(v)?;
                    self.func.code.push(Op::Ret);
                } else {
                    self.func.code.push(Op::RetVoid);
                }
            }
            Expr::Break { .. } => {
                let j = self.emit_jump(Op::Jump(0));
                self.break_targets.push(j);
            }
            Expr::Continue { .. } => {
                let j = self.emit_jump(Op::Jump(0));
                self.continue_targets.push(j);
            }
            Expr::Let { name, value, .. } => {
                self.compile_expr(value)?;
                let idx = self.add_local(name);
                self.func.code.push(Op::StoreLocal(idx));
                self.func.code.push(Op::PushLocal(idx));
            }
            Expr::Assign { target, value, .. } => {
                self.compile_expr(value)?;
                self.func.code.push(Op::Nop);
                if let Expr::Ident { name, .. } = target.as_ref() {
                    if let Some(&idx) = self.locals.get(name) {
                        self.func.code.push(Op::StoreLocal(idx));
                    }
                }
            }
            Expr::Block(inner) => {
                self.compile_block(inner)?;
            }
            Expr::Array { elements, .. } => {
                let count = elements.len();
                for el in elements.iter().rev() {
                    self.compile_expr(el)?;
                }
                self.func.code.push(Op::NewArray(count));
            }
            Expr::Tuple { elements, .. } => {
                let count = elements.len();
                for el in elements.iter().rev() {
                    self.compile_expr(el)?;
                }
                self.func.code.push(Op::NewTuple(count));
            }
            Expr::FieldAccess { target, field, .. } => {
                self.compile_expr(target)?;
                let _fidx = self.string_intern(field);
                // Simplified field access
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                self.compile_expr(receiver)?;
                for arg in args.iter().rev() {
                    self.compile_expr(arg)?;
                }
                let _midx = self.string_intern(method);
                for _ in 0..args.len() { self.func.code.push(Op::Pop); }
                self.func.code.push(Op::PushNil);
            }
            _ => self.func.code.push(Op::PushNil),
        }
        Ok(())
    }

    fn add_local(&mut self, name: &str) -> usize {
        let idx = self.next_local;
        self.next_local += 1;
        self.locals.insert(name.into(), idx);
        idx
    }

    fn string_intern(&mut self, s: &str) -> usize {
        if let Some(&idx) = self.strings.get(s) { idx }
        else {
            let idx = self.module.string_table.len();
            self.module.string_table.push(s.into());
            self.strings.insert(s.into(), idx);
            idx
        }
    }

    fn global_intern(&mut self, name: &str) -> usize {
        if let Some(pos) = self.module.global_names.iter().position(|n| n == name) { pos }
        else {
            let pos = self.module.global_names.len();
            self.module.global_names.push(name.into());
            pos
        }
    }

    fn emit_jump(&mut self, placeholder: Op) -> usize {
        let pos = self.func.code.len();
        self.func.code.push(placeholder);
        pos
    }

    fn patch_jump(&mut self, jump_pos: usize, target: usize) {
        if jump_pos < self.func.code.len() {
            match &mut self.func.code[jump_pos] {
                Op::Jump(ref mut dest) => *dest = target,
                Op::JumpIfFalse(ref mut dest) => *dest = target,
                Op::Loop(ref mut dest) => *dest = target,
                _ => {}
            }
        }
    }
}

impl Default for AstCompiler {
    fn default() -> Self { Self::new() }
}