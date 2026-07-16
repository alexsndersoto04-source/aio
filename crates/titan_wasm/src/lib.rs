//! Direct WebAssembly backend for Titan's portable numeric bytecode.

use thiserror::Error;
use titan_codegen::{BytecodeFunc, CompiledModule, Op};
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum WasmError {
    #[error("invalid module entry point")]
    Entry,
    #[error("function '{function}' uses unsupported WebAssembly operation: {operation}")]
    Unsupported {
        function: String,
        operation: String,
    },
    #[error("local count underflow in function '{0}'")]
    Locals(String),
    #[error("function '{function}' accesses invalid local {local}")]
    InvalidLocal { function: String, local: usize },
    #[error("function '{function}' calls invalid function {callee}")]
    InvalidFunction { function: String, callee: usize },
    #[error(
        "function '{function}' calls '{callee}' with {actual} arguments, but it expects {expected}"
    )]
    InvalidArgumentCount {
        function: String,
        callee: String,
        expected: usize,
        actual: usize,
    },
}

/// Compile Titan numeric bytecode directly to a self-contained WebAssembly module.
///
/// Every currently supported Titan value is represented as an `i64`. Operations
/// requiring the managed Titan runtime (strings, collections, closures, I/O,
/// concurrency, and networking) are rejected instead of being silently lowered
/// with different semantics.
pub fn compile(module: &CompiledModule) -> Result<Vec<u8>, WasmError> {
    if module.entry >= module.functions.len() {
        return Err(WasmError::Entry);
    }

    let mut output = Module::new();

    let mut types = TypeSection::new();
    for function in &module.functions {
        types
            .ty()
            .function(vec![ValType::I64; function.arity], [ValType::I64]);
    }
    output.section(&types);

    let mut functions = FunctionSection::new();
    for index in 0..module.functions.len() {
        functions.function(index as u32);
    }
    output.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, module.entry as u32);
    for (index, function) in module.functions.iter().enumerate() {
        if function.name != "main" && !function.name.starts_with('<') {
            exports.export(&function.name, ExportKind::Func, index as u32);
        }
    }
    output.section(&exports);

    let mut code = CodeSection::new();
    for function in &module.functions {
        code.function(&compile_function(module, function)?);
    }
    output.section(&code);

    Ok(output.finish())
}

fn compile_function(
    module: &CompiledModule,
    function: &BytecodeFunc,
) -> Result<Function, WasmError> {
    let extra = function
        .locals
        .checked_sub(function.arity)
        .ok_or_else(|| WasmError::Locals(function.name.clone()))?;
    let locals = if extra == 0 {
        Vec::new()
    } else {
        vec![(extra as u32, ValType::I64)]
    };
    let mut body = Function::new(locals);

    for operation in &function.code {
        emit_operation(module, function, operation, &mut body)?;
    }

    if !matches!(function.code.last(), Some(Op::Ret | Op::Halt)) {
        body.instruction(&Instruction::I64Const(0));
    }
    body.instruction(&Instruction::End);
    Ok(body)
}

fn emit_operation(
    module: &CompiledModule,
    function: &BytecodeFunc,
    operation: &Op,
    body: &mut Function,
) -> Result<(), WasmError> {
    match operation {
        Op::PushInt(value) => {
            body.instruction(&Instruction::I64Const(*value));
        }
        Op::PushBool(value) => {
            body.instruction(&Instruction::I64Const(i64::from(*value)));
        }
        Op::PushNil => {
            body.instruction(&Instruction::I64Const(0));
        }
        Op::PushLocal(index) => {
            validate_local(function, *index)?;
            body.instruction(&Instruction::LocalGet(*index as u32));
        }
        Op::StoreLocal(index) => {
            validate_local(function, *index)?;
            body.instruction(&Instruction::LocalSet(*index as u32));
        }
        Op::Pop => {
            body.instruction(&Instruction::Drop);
        }
        Op::Add => {
            body.instruction(&Instruction::I64Add);
        }
        Op::Sub => {
            body.instruction(&Instruction::I64Sub);
        }
        Op::Mul => {
            body.instruction(&Instruction::I64Mul);
        }
        Op::Div => {
            body.instruction(&Instruction::I64DivS);
        }
        Op::Mod => {
            body.instruction(&Instruction::I64RemS);
        }
        Op::Neg => {
            body.instruction(&Instruction::I64Const(-1));
            body.instruction(&Instruction::I64Mul);
        }
        Op::BitAnd => {
            body.instruction(&Instruction::I64And);
        }
        Op::BitOr => {
            body.instruction(&Instruction::I64Or);
        }
        Op::BitXor => {
            body.instruction(&Instruction::I64Xor);
        }
        Op::BitNot => {
            body.instruction(&Instruction::I64Const(-1));
            body.instruction(&Instruction::I64Xor);
        }
        Op::Eq => comparison(body, Instruction::I64Eq),
        Op::Neq => comparison(body, Instruction::I64Ne),
        Op::Lt => comparison(body, Instruction::I64LtS),
        Op::Gt => comparison(body, Instruction::I64GtS),
        Op::Lte => comparison(body, Instruction::I64LeS),
        Op::Gte => comparison(body, Instruction::I64GeS),
        Op::Call {
            function: callee,
            argc,
        } => {
            let target = module.functions.get(*callee).ok_or_else(|| {
                WasmError::InvalidFunction {
                    function: function.name.clone(),
                    callee: *callee,
                }
            })?;
            if *argc != target.arity {
                return Err(WasmError::InvalidArgumentCount {
                    function: function.name.clone(),
                    callee: target.name.clone(),
                    expected: target.arity,
                    actual: *argc,
                });
            }
            body.instruction(&Instruction::Call(*callee as u32));
        }
        Op::Ret | Op::Halt => {
            body.instruction(&Instruction::Return);
        }
        Op::Nop => {
            body.instruction(&Instruction::Nop);
        }
        other => {
            return Err(WasmError::Unsupported {
                function: function.name.clone(),
                operation: format!("{other:?}"),
            });
        }
    }
    Ok(())
}

fn validate_local(function: &BytecodeFunc, local: usize) -> Result<(), WasmError> {
    if local >= function.locals {
        return Err(WasmError::InvalidLocal {
            function: function.name.clone(),
            local,
        });
    }
    Ok(())
}

fn comparison(function: &mut Function, instruction: Instruction<'_>) {
    function.instruction(&instruction);
    function.instruction(&Instruction::I64ExtendI32U);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with(code: Vec<Op>) -> CompiledModule {
        CompiledModule {
            functions: vec![BytecodeFunc {
                name: "main".into(),
                source_file: Some("main.titan".into()),
                arity: 0,
                captures: 0,
                locals: 0,
                max_stack: 4,
                debug_locations: vec![None; code.len()],
                code,
            }],
            entry: 0,
            string_table: Vec::new(),
        }
    }

    #[test]
    fn emits_valid_executable_wasm() {
        let module = module_with(vec![
            Op::PushInt(40),
            Op::PushInt(2),
            Op::Add,
            Op::Ret,
        ]);
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        assert_eq!(&wasm[..4], b"\0asm");
    }

    #[test]
    fn rejects_unsupported_runtime_values() {
        let module = module_with(vec![Op::PushStr(0), Op::Ret]);
        assert!(matches!(compile(&module), Err(WasmError::Unsupported { .. })));
    }

    #[test]
    fn rejects_invalid_calls_before_emitting_wasm() {
        let module = module_with(vec![
            Op::PushInt(1),
            Op::Call {
                function: 9,
                argc: 1,
            },
            Op::Ret,
        ]);
        assert!(matches!(
            compile(&module),
            Err(WasmError::InvalidFunction { callee: 9, .. })
        ));
    }
}
