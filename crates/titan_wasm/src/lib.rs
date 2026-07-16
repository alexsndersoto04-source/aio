//! Direct WebAssembly backend for Titan's portable numeric bytecode.

use std::collections::VecDeque;

use thiserror::Error;
use titan_codegen::{BytecodeFunc, CompiledModule, Op};
use wasm_encoder::{
    BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
    Module, TypeSection, ValType,
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
    #[error("function '{function}' jumps to invalid instruction {target}")]
    InvalidJump { function: String, target: usize },
    #[error("operand stack underflow in function '{function}' at instruction {instruction}")]
    StackUnderflow {
        function: String,
        instruction: usize,
    },
    #[error(
        "inconsistent operand stack in function '{function}' at instruction {instruction}: {first} versus {second} values"
    )]
    StackMerge {
        function: String,
        instruction: usize,
        first: usize,
        second: usize,
    },
    #[error(
        "operand stack in function '{function}' requires {required} values, exceeding declared maximum {declared}"
    )]
    StackLimit {
        function: String,
        required: usize,
        declared: usize,
    },
}

/// Compile Titan numeric bytecode directly to a self-contained WebAssembly module.
///
/// Numeric operand-stack values are assigned to WebAssembly locals. A structured
/// Wasm dispatch loop represents Titan branches, including backward branches,
/// without relying on the host VM. This supports arbitrary validated bytecode
/// control-flow graphs while preserving WebAssembly's structured-control rules.
/// Operations requiring managed runtime values are rejected explicitly.
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
    let layout = analyze_function(module, function)?;
    let numeric_locals = extra + layout.stack_slots;
    let mut locals = Vec::new();
    if numeric_locals > 0 {
        locals.push((numeric_locals as u32, ValType::I64));
    }
    locals.push((1, ValType::I32));

    let mut body = Function::new(locals);
    body.instruction(&Instruction::Loop(BlockType::Empty));

    for (instruction, height) in layout.heights.iter().enumerate() {
        let Some(height) = height else {
            continue;
        };
        body.instruction(&Instruction::LocalGet(layout.pc_local));
        body.instruction(&Instruction::I32Const(instruction as i32));
        body.instruction(&Instruction::I32Eq);
        body.instruction(&Instruction::If(BlockType::Empty));
        emit_operation(
            module,
            function,
            instruction,
            *height,
            &layout,
            &mut body,
        )?;
        body.instruction(&Instruction::End);
    }

    // Stack analysis guarantees every assigned program counter has a case. This
    // instruction traps corrupted state instead of returning a fabricated value.
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I64Const(0));
    body.instruction(&Instruction::End);
    Ok(body)
}

struct FunctionLayout {
    heights: Vec<Option<usize>>,
    stack_base: u32,
    stack_slots: usize,
    pc_local: u32,
}

fn analyze_function(
    module: &CompiledModule,
    function: &BytecodeFunc,
) -> Result<FunctionLayout, WasmError> {
    if function.code.is_empty() {
        return Ok(FunctionLayout {
            heights: Vec::new(),
            stack_base: function.locals as u32,
            stack_slots: 0,
            pc_local: function.locals as u32,
        });
    }

    let mut heights = vec![None; function.code.len()];
    heights[0] = Some(0);
    let mut pending = VecDeque::from([0]);
    let mut maximum = 0;

    while let Some(instruction) = pending.pop_front() {
        let height = heights[instruction].expect("queued instructions have a stack height");
        let operation = &function.code[instruction];
        validate_operation(module, function, operation)?;
        let (consumed, produced) = stack_effect(operation);
        if height < consumed {
            return Err(WasmError::StackUnderflow {
                function: function.name.clone(),
                instruction,
            });
        }
        let next_height = height - consumed + produced;
        maximum = maximum.max(next_height);
        if maximum > function.max_stack {
            return Err(WasmError::StackLimit {
                function: function.name.clone(),
                required: maximum,
                declared: function.max_stack,
            });
        }

        match operation {
            Op::Jump(target) => {
                enqueue_successor(function, &mut heights, &mut pending, *target, next_height)?;
            }
            Op::JumpIfFalse(target) => {
                enqueue_successor(function, &mut heights, &mut pending, *target, next_height)?;
                enqueue_fallthrough(
                    function,
                    &mut heights,
                    &mut pending,
                    instruction,
                    next_height,
                )?;
            }
            Op::Ret | Op::Halt => {}
            _ => enqueue_fallthrough(
                function,
                &mut heights,
                &mut pending,
                instruction,
                next_height,
            )?,
        }
    }

    Ok(FunctionLayout {
        heights,
        stack_base: function.locals as u32,
        stack_slots: maximum,
        pc_local: (function.locals + maximum) as u32,
    })
}

fn enqueue_fallthrough(
    function: &BytecodeFunc,
    heights: &mut [Option<usize>],
    pending: &mut VecDeque<usize>,
    instruction: usize,
    height: usize,
) -> Result<(), WasmError> {
    let next = instruction + 1;
    if next >= function.code.len() {
        return Err(WasmError::InvalidJump {
            function: function.name.clone(),
            target: next,
        });
    }
    enqueue_successor(function, heights, pending, next, height)
}

fn enqueue_successor(
    function: &BytecodeFunc,
    heights: &mut [Option<usize>],
    pending: &mut VecDeque<usize>,
    target: usize,
    height: usize,
) -> Result<(), WasmError> {
    let Some(existing) = heights.get_mut(target) else {
        return Err(WasmError::InvalidJump {
            function: function.name.clone(),
            target,
        });
    };
    match *existing {
        Some(first) if first != height => Err(WasmError::StackMerge {
            function: function.name.clone(),
            instruction: target,
            first,
            second: height,
        }),
        Some(_) => Ok(()),
        None => {
            *existing = Some(height);
            pending.push_back(target);
            Ok(())
        }
    }
}

fn validate_operation(
    module: &CompiledModule,
    function: &BytecodeFunc,
    operation: &Op,
) -> Result<(), WasmError> {
    match operation {
        Op::PushLocal(index) | Op::StoreLocal(index) if *index >= function.locals => {
            Err(WasmError::InvalidLocal {
                function: function.name.clone(),
                local: *index,
            })
        }
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
            Ok(())
        }
        Op::PushInt(_)
        | Op::PushBool(_)
        | Op::PushNil
        | Op::PushLocal(_)
        | Op::StoreLocal(_)
        | Op::Pop
        | Op::Dup
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Neg
        | Op::Not
        | Op::BitNot
        | Op::Eq
        | Op::Neq
        | Op::Lt
        | Op::Gt
        | Op::Lte
        | Op::Gte
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor
        | Op::Jump(_)
        | Op::JumpIfFalse(_)
        | Op::Ret
        | Op::Nop
        | Op::Halt => Ok(()),
        other => Err(WasmError::Unsupported {
            function: function.name.clone(),
            operation: format!("{other:?}"),
        }),
    }
}

fn stack_effect(operation: &Op) -> (usize, usize) {
    match operation {
        Op::PushInt(_) | Op::PushBool(_) | Op::PushNil | Op::PushLocal(_) => (0, 1),
        Op::StoreLocal(_) | Op::Pop | Op::JumpIfFalse(_) => (1, 0),
        Op::Dup => (1, 2),
        Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Mod
        | Op::Eq
        | Op::Neq
        | Op::Lt
        | Op::Gt
        | Op::Lte
        | Op::Gte
        | Op::BitAnd
        | Op::BitOr
        | Op::BitXor => (2, 1),
        Op::Neg | Op::Not | Op::BitNot => (1, 1),
        Op::Call { argc, .. } => (*argc, 1),
        Op::Jump(_) | Op::Nop => (0, 0),
        Op::Ret | Op::Halt => (0, 0),
        _ => (0, 0),
    }
}

fn emit_operation(
    _module: &CompiledModule,
    function: &BytecodeFunc,
    instruction: usize,
    height: usize,
    layout: &FunctionLayout,
    body: &mut Function,
) -> Result<(), WasmError> {
    let operation = &function.code[instruction];
    match operation {
        Op::PushInt(value) => store_constant(body, layout.stack_base + height as u32, *value),
        Op::PushBool(value) => {
            store_constant(body, layout.stack_base + height as u32, i64::from(*value));
        }
        Op::PushNil => store_constant(body, layout.stack_base + height as u32, 0),
        Op::PushLocal(local) => {
            body.instruction(&Instruction::LocalGet(*local as u32));
            body.instruction(&Instruction::LocalSet(layout.stack_base + height as u32));
        }
        Op::StoreLocal(local) => {
            body.instruction(&Instruction::LocalGet(
                layout.stack_base + (height - 1) as u32,
            ));
            body.instruction(&Instruction::LocalSet(*local as u32));
        }
        Op::Pop => {}
        Op::Dup => {
            body.instruction(&Instruction::LocalGet(
                layout.stack_base + (height - 1) as u32,
            ));
            body.instruction(&Instruction::LocalSet(layout.stack_base + height as u32));
        }
        Op::Add => binary(body, layout, height, Instruction::I64Add),
        Op::Sub => binary(body, layout, height, Instruction::I64Sub),
        Op::Mul => binary(body, layout, height, Instruction::I64Mul),
        Op::Div => binary(body, layout, height, Instruction::I64DivS),
        Op::Mod => binary(body, layout, height, Instruction::I64RemS),
        Op::BitAnd => binary(body, layout, height, Instruction::I64And),
        Op::BitOr => binary(body, layout, height, Instruction::I64Or),
        Op::BitXor => binary(body, layout, height, Instruction::I64Xor),
        Op::Eq => comparison(body, layout, height, Instruction::I64Eq),
        Op::Neq => comparison(body, layout, height, Instruction::I64Ne),
        Op::Lt => comparison(body, layout, height, Instruction::I64LtS),
        Op::Gt => comparison(body, layout, height, Instruction::I64GtS),
        Op::Lte => comparison(body, layout, height, Instruction::I64LeS),
        Op::Gte => comparison(body, layout, height, Instruction::I64GeS),
        Op::Neg => {
            let slot = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalGet(slot));
            body.instruction(&Instruction::I64Sub);
            body.instruction(&Instruction::LocalSet(slot));
        }
        Op::Not => {
            let slot = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::LocalGet(slot));
            body.instruction(&Instruction::I64Eqz);
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::LocalSet(slot));
        }
        Op::BitNot => {
            let slot = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::LocalGet(slot));
            body.instruction(&Instruction::I64Const(-1));
            body.instruction(&Instruction::I64Xor);
            body.instruction(&Instruction::LocalSet(slot));
        }
        Op::Call {
            function: callee,
            argc,
        } => {
            let first = height - *argc;
            for argument in first..height {
                body.instruction(&Instruction::LocalGet(
                    layout.stack_base + argument as u32,
                ));
            }
            body.instruction(&Instruction::Call(*callee as u32));
            body.instruction(&Instruction::LocalSet(layout.stack_base + first as u32));
        }
        Op::Jump(target) => {
            set_pc(body, layout.pc_local, *target);
            body.instruction(&Instruction::Br(1));
            return Ok(());
        }
        Op::JumpIfFalse(target) => {
            body.instruction(&Instruction::LocalGet(
                layout.stack_base + (height - 1) as u32,
            ));
            body.instruction(&Instruction::I64Eqz);
            body.instruction(&Instruction::If(BlockType::Empty));
            set_pc(body, layout.pc_local, *target);
            body.instruction(&Instruction::Else);
            set_pc(body, layout.pc_local, instruction + 1);
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::Br(1));
            return Ok(());
        }
        Op::Ret | Op::Halt => {
            if height == 0 {
                body.instruction(&Instruction::I64Const(0));
            } else {
                body.instruction(&Instruction::LocalGet(
                    layout.stack_base + (height - 1) as u32,
                ));
            }
            body.instruction(&Instruction::Return);
            return Ok(());
        }
        Op::Nop => {}
        _ => unreachable!("operations are checked during stack analysis"),
    }

    set_pc(body, layout.pc_local, instruction + 1);
    body.instruction(&Instruction::Br(1));
    Ok(())
}

fn store_constant(body: &mut Function, local: u32, value: i64) {
    body.instruction(&Instruction::I64Const(value));
    body.instruction(&Instruction::LocalSet(local));
}

fn binary(
    body: &mut Function,
    layout: &FunctionLayout,
    height: usize,
    operation: Instruction<'_>,
) {
    let output = layout.stack_base + (height - 2) as u32;
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::LocalGet(output + 1));
    body.instruction(&operation);
    body.instruction(&Instruction::LocalSet(output));
}

fn comparison(
    body: &mut Function,
    layout: &FunctionLayout,
    height: usize,
    operation: Instruction<'_>,
) {
    let output = layout.stack_base + (height - 2) as u32;
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::LocalGet(output + 1));
    body.instruction(&operation);
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::LocalSet(output));
}

fn set_pc(body: &mut Function, local: u32, instruction: usize) {
    body.instruction(&Instruction::I32Const(instruction as i32));
    body.instruction(&Instruction::LocalSet(local));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with(code: Vec<Op>, locals: usize) -> CompiledModule {
        CompiledModule {
            functions: vec![BytecodeFunc {
                name: "main".into(),
                source_file: Some("main.titan".into()),
                arity: 0,
                captures: 0,
                locals,
                max_stack: 16,
                debug_locations: vec![None; code.len()],
                code,
            }],
            entry: 0,
            string_table: Vec::new(),
        }
    }

    fn validate(module: &CompiledModule) {
        let wasm = compile(module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        assert_eq!(&wasm[..4], b"\0asm");
    }

    #[test]
    fn emits_valid_executable_wasm() {
        validate(&module_with(
            vec![Op::PushInt(40), Op::PushInt(2), Op::Add, Op::Ret],
            0,
        ));
    }

    #[test]
    fn emits_forward_conditional_control_flow() {
        validate(&module_with(
            vec![
                Op::PushBool(false),
                Op::JumpIfFalse(4),
                Op::PushInt(10),
                Op::Jump(5),
                Op::PushInt(20),
                Op::Ret,
            ],
            0,
        ));
    }

    #[test]
    fn emits_backward_loop_control_flow() {
        validate(&module_with(
            vec![
                Op::PushInt(0),
                Op::StoreLocal(0),
                Op::PushLocal(0),
                Op::PushInt(3),
                Op::Lt,
                Op::JumpIfFalse(11),
                Op::PushLocal(0),
                Op::PushInt(1),
                Op::Add,
                Op::StoreLocal(0),
                Op::Jump(2),
                Op::PushLocal(0),
                Op::Ret,
            ],
            1,
        ));
    }

    #[test]
    fn rejects_unsupported_runtime_values() {
        let module = module_with(vec![Op::PushStr(0), Op::Ret], 0);
        assert!(matches!(
            compile(&module),
            Err(WasmError::Unsupported { .. })
        ));
    }

    #[test]
    fn rejects_invalid_calls_before_emitting_wasm() {
        let module = module_with(
            vec![
                Op::PushInt(1),
                Op::Call {
                    function: 9,
                    argc: 1,
                },
                Op::Ret,
            ],
            0,
        );
        assert!(matches!(
            compile(&module),
            Err(WasmError::InvalidFunction { callee: 9, .. })
        ));
    }

    #[test]
    fn rejects_inconsistent_branch_stack_heights() {
        let module = module_with(
            vec![
                Op::PushBool(true),
                Op::JumpIfFalse(4),
                Op::PushInt(1),
                Op::Jump(5),
                Op::Nop,
                Op::Ret,
            ],
            0,
        );
        assert!(matches!(compile(&module), Err(WasmError::StackMerge { .. })));
    }
}
