//! Safe stack-based virtual machine for Titan bytecode.

mod native;

use std::collections::BTreeMap;
use thiserror::Error;
use titan_codegen::{CompiledModule, Op};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64), Float(f64), Bool(bool), Char(char), Str(String), Bytes(Vec<u8>), Nil,
    Array(Vec<Value>), Tuple(Vec<Value>), Map(BTreeMap<String, Value>),
    Struct { name: String, fields: BTreeMap<String, Value> },
    Enum { name: String, variant: String, payload: Option<Box<Value>> },
}

pub fn val_to_string(value: &Value) -> String {
    match value {
        Value::Int(v) => v.to_string(), Value::Float(v) => v.to_string(),
        Value::Bool(v) => v.to_string(), Value::Char(v) => v.to_string(),
        Value::Str(v) => v.clone(), Value::Bytes(v) => format!("bytes[{}]", v.len()), Value::Nil => "nil".into(),
        Value::Array(values) => format!("[{}]", values.iter().map(val_to_string).collect::<Vec<_>>().join(", ")),
        Value::Tuple(values) => format!("({})", values.iter().map(val_to_string).collect::<Vec<_>>().join(", ")),
        Value::Map(values) => format!("{{{}}}", values.iter().map(|(k, v)| format!("{}: {}", k, val_to_string(v))).collect::<Vec<_>>().join(", ")),
        Value::Struct { name, fields } => format!("{} {{ {} }}", name, fields.iter().map(|(k, v)| format!("{}: {}", k, val_to_string(v))).collect::<Vec<_>>().join(", ")),
        Value::Enum { name, variant, payload } => match payload { Some(value) => format!("{}::{}({})", name, variant, val_to_string(value)), None => format!("{}::{}", name, variant) },
    }
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum VmError {
    #[error("stack underflow in function '{0}'")]
    StackUnderflow(String),
    #[error("invalid local {index} in function '{function}'")]
    InvalidLocal { function: String, index: usize },
    #[error("invalid function index {0}")]
    InvalidFunction(usize),
    #[error("function '{function}' expected {expected} arguments, found {found}")]
    Arity { function: String, expected: usize, found: usize },
    #[error("type error: {0}")]
    Type(String),
    #[error("division by zero")]
    DivisionByZero,
    #[error("integer overflow")]
    Overflow,
    #[error("index {index} out of bounds for length {length}")]
    IndexOutOfBounds { index: usize, length: usize },
    #[error("unknown field '{0}'")]
    UnknownField(String),
    #[error("instruction limit exceeded")]
    InstructionLimit,
    #[error("call depth limit exceeded")]
    CallDepth,
    #[error("native function '{function}' failed: {message}")]
    Native { function: String, message: String },
    #[error("native function '{function}' requires capability '{capability}'")]
    PermissionDenied { function: String, capability: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub filesystem: bool,
    pub process: bool,
    pub network: bool,
    pub environment: bool,
}
impl RuntimeCapabilities {
    pub const fn all() -> Self { Self { filesystem: true, process: true, network: true, environment: true } }
    pub const fn sandboxed() -> Self { Self { filesystem: false, process: false, network: false, environment: false } }
}
impl Default for RuntimeCapabilities { fn default() -> Self { Self::all() } }

pub struct Vm {
    module: CompiledModule,
    instruction_limit: usize,
    instructions: usize,
    max_call_depth: usize,
    capabilities: RuntimeCapabilities,
}

impl Vm {
    pub fn new(module: CompiledModule) -> Self { Self { module, instruction_limit: 10_000_000, instructions: 0, max_call_depth: 4096, capabilities: RuntimeCapabilities::all() } }
    pub fn sandboxed(module: CompiledModule) -> Self { Self { capabilities: RuntimeCapabilities::sandboxed(), ..Self::new(module) } }
    pub fn with_instruction_limit(mut self, limit: usize) -> Self { self.instruction_limit = limit; self }
    pub fn with_capabilities(mut self, capabilities: RuntimeCapabilities) -> Self { self.capabilities = capabilities; self }

    pub fn run(&mut self) -> Result<Option<Value>, VmError> {
        self.instructions = 0;
        let result = self.execute(self.module.entry, Vec::new(), 0)?;
        Ok(Some(result))
    }

    fn execute(&mut self, function_id: usize, args: Vec<Value>, depth: usize) -> Result<Value, VmError> {
        if depth >= self.max_call_depth { return Err(VmError::CallDepth); }
        let function = self.module.functions.get(function_id).cloned().ok_or(VmError::InvalidFunction(function_id))?;
        if args.len() != function.arity { return Err(VmError::Arity { function: function.name, expected: function.arity, found: args.len() }); }
        let mut locals = vec![Value::Nil; function.locals.max(args.len())];
        for (slot, value) in locals.iter_mut().zip(args) { *slot = value; }
        let mut stack = Vec::with_capacity(function.max_stack);
        let mut ip = 0usize;
        while ip < function.code.len() {
            self.instructions += 1;
            if self.instructions > self.instruction_limit { return Err(VmError::InstructionLimit); }
            match function.code[ip].clone() {
                Op::PushInt(v) => stack.push(Value::Int(v)), Op::PushFloat(v) => stack.push(Value::Float(v)),
                Op::PushBool(v) => stack.push(Value::Bool(v)), Op::PushChar(v) => stack.push(Value::Char(v)),
                Op::PushNil => stack.push(Value::Nil),
                Op::PushStr(index) => stack.push(Value::Str(self.module.string_table.get(index).cloned().unwrap_or_default())),
                Op::PushLocal(index) => stack.push(locals.get(index).cloned().ok_or_else(|| VmError::InvalidLocal { function: function.name.clone(), index })?),
                Op::StoreLocal(index) => { let value = pop(&mut stack, &function.name)?; let slot = locals.get_mut(index).ok_or_else(|| VmError::InvalidLocal { function: function.name.clone(), index })?; *slot = value; }
                Op::Pop => { pop(&mut stack, &function.name)?; }
                Op::Dup => { let value = stack.last().cloned().ok_or_else(|| VmError::StackUnderflow(function.name.clone()))?; stack.push(value); }
                Op::Add => binary(&mut stack, &function.name, add)?, Op::Sub => binary(&mut stack, &function.name, sub)?,
                Op::Mul => binary(&mut stack, &function.name, mul)?, Op::Div => binary(&mut stack, &function.name, div)?,
                Op::Mod => binary(&mut stack, &function.name, modulo)?,
                Op::Eq => compare(&mut stack, &function.name, |a, b| a == b)?, Op::Neq => compare(&mut stack, &function.name, |a, b| a != b)?,
                Op::Lt => ordered(&mut stack, &function.name, |a, b| a < b)?, Op::Gt => ordered(&mut stack, &function.name, |a, b| a > b)?,
                Op::Lte => ordered(&mut stack, &function.name, |a, b| a <= b)?, Op::Gte => ordered(&mut stack, &function.name, |a, b| a >= b)?,
                Op::BitAnd => integer_binary(&mut stack, &function.name, |a, b| a & b)?,
                Op::BitOr => integer_binary(&mut stack, &function.name, |a, b| a | b)?,
                Op::BitXor => integer_binary(&mut stack, &function.name, |a, b| a ^ b)?,
                Op::Neg => { let value = pop(&mut stack, &function.name)?; stack.push(match value { Value::Int(v) => Value::Int(v.checked_neg().ok_or(VmError::Overflow)?), Value::Float(v) => Value::Float(-v), other => return Err(VmError::Type(format!("cannot negate {}", val_to_string(&other)))) }); }
                Op::Not => { let value = pop(&mut stack, &function.name)?; stack.push(Value::Bool(!truthy(&value))); }
                Op::BitNot => { let value = pop(&mut stack, &function.name)?; if let Value::Int(v) = value { stack.push(Value::Int(!v)); } else { return Err(VmError::Type("bitwise not requires int".into())); } }
                Op::Jump(target) => { ip = target; continue; }
                Op::JumpIfFalse(target) => { let condition = pop(&mut stack, &function.name)?; if !truthy(&condition) { ip = target; continue; } }
                Op::Call { function: callee, argc } => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    if callee == usize::MAX { stack.push(make_range(args)?); }
                    else { stack.push(self.execute(callee, args, depth + 1)?); }
                }
                Op::CallNative { name, argc } => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    stack.push(native::invoke(&name, args, self.capabilities)?);
                }
                Op::Ret => return Ok(stack.pop().unwrap_or(Value::Nil)),
                Op::Print(argc) => {
                    let args = take_args(&mut stack, argc, &function.name)?;
                    println!("{}", args.iter().map(val_to_string).collect::<Vec<_>>().join(" "));
                    stack.push(Value::Nil);
                }
                Op::Len => {
                    let value = pop(&mut stack, &function.name)?;
                    let length = match value { Value::Array(v) | Value::Tuple(v) => v.len(), Value::Str(v) => v.chars().count(), Value::Bytes(v) => v.len(), Value::Map(v) => v.len(), _ => return Err(VmError::Type("len requires an array, tuple, string, bytes, or map".into())) };
                    stack.push(Value::Int(length as i64));
                }
                Op::ToString => { let value = pop(&mut stack, &function.name)?; stack.push(Value::Str(val_to_string(&value))); }
                Op::NewArray(count) => { let values = take_args(&mut stack, count, &function.name)?; stack.push(Value::Array(values)); }
                Op::NewTuple(count) => { let values = take_args(&mut stack, count, &function.name)?; stack.push(Value::Tuple(values)); }
                Op::Index => {
                    let index_value = pop(&mut stack, &function.name)?; let target = pop(&mut stack, &function.name)?;
                    let value = match (target, index_value) {
                        (Value::Map(values), Value::Str(key)) => values.get(&key).cloned().ok_or(VmError::UnknownField(key))?,
                        (target, Value::Int(index)) => {
                            let index = usize::try_from(index).map_err(|_| VmError::IndexOutOfBounds { index: usize::MAX, length: 0 })?;
                            match target {
                                Value::Array(v) | Value::Tuple(v) => v.get(index).cloned().ok_or(VmError::IndexOutOfBounds { index, length: v.len() })?,
                                Value::Str(v) => v.chars().nth(index).map(Value::Char).ok_or(VmError::IndexOutOfBounds { index, length: v.chars().count() })?,
                                Value::Bytes(v) => v.get(index).map(|value| Value::Int(i64::from(*value))).ok_or(VmError::IndexOutOfBounds { index, length: v.len() })?,
                                _ => return Err(VmError::Type("value is not indexable by integer".into())),
                            }
                        }
                        _ => return Err(VmError::Type("index must be int, or string for maps".into())),
                    }; stack.push(value);
                }
                Op::NewStruct { name, fields } => {
                    let values = take_args(&mut stack, fields.len(), &function.name)?;
                    stack.push(Value::Struct { name, fields: fields.into_iter().zip(values).collect() });
                }
                Op::GetField(field) => {
                    let value = pop(&mut stack, &function.name)?;
                    match value {
                        Value::Struct { fields, .. } | Value::Map(fields) => stack.push(fields.get(&field).cloned().ok_or(VmError::UnknownField(field))?),
                        _ => return Err(VmError::Type("field access requires a struct or map".into())),
                    }
                }
                Op::NewEnum { name, variant, has_payload } => {
                    let payload = if has_payload { Some(Box::new(pop(&mut stack, &function.name)?)) } else { None };
                    stack.push(Value::Enum { name, variant, payload });
                }
                Op::EnumIs { name, variant } => {
                    let value = pop(&mut stack, &function.name)?;
                    stack.push(Value::Bool(matches!(value, Value::Enum { name: n, variant: v, .. } if n == name && v == variant)));
                }
                Op::EnumPayload => {
                    let value = pop(&mut stack, &function.name)?;
                    if let Value::Enum { payload: Some(value), .. } = value { stack.push(*value); }
                    else { return Err(VmError::Type("enum variant has no payload".into())); }
                }
                Op::Nop => {}
                Op::Halt => return Ok(stack.pop().unwrap_or(Value::Nil)),
            }
            ip += 1;
        }
        Ok(stack.pop().unwrap_or(Value::Nil))
    }
}

fn pop(stack: &mut Vec<Value>, function: &str) -> Result<Value, VmError> { stack.pop().ok_or_else(|| VmError::StackUnderflow(function.into())) }
fn take_args(stack: &mut Vec<Value>, count: usize, function: &str) -> Result<Vec<Value>, VmError> {
    if stack.len() < count { return Err(VmError::StackUnderflow(function.into())); }
    Ok(stack.split_off(stack.len() - count))
}
fn truthy(value: &Value) -> bool { !matches!(value, Value::Bool(false) | Value::Nil) }
fn binary<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(Value, Value) -> Result<Value, VmError> {
    let right = pop(stack, function)?; let left = pop(stack, function)?; stack.push(operation(left, right)?); Ok(())
}
fn compare<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(&Value, &Value) -> bool {
    let right = pop(stack, function)?; let left = pop(stack, function)?; stack.push(Value::Bool(operation(&left, &right))); Ok(())
}
fn ordered<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(f64, f64) -> bool {
    let right = pop(stack, function)?; let left = pop(stack, function)?;
    let (a, b) = match (left, right) { (Value::Int(a), Value::Int(b)) => (a as f64, b as f64), (Value::Float(a), Value::Float(b)) => (a, b), _ => return Err(VmError::Type("ordered comparison requires matching numbers".into())) };
    stack.push(Value::Bool(operation(a, b))); Ok(())
}
fn integer_binary<F>(stack: &mut Vec<Value>, function: &str, operation: F) -> Result<(), VmError> where F: FnOnce(i64, i64) -> i64 {
    binary(stack, function, |a, b| match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(operation(a, b))), _ => Err(VmError::Type("bitwise operation requires integers".into())) })
}
fn add(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_add(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)), (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)), _ => Err(VmError::Type("addition requires matching numbers or strings".into())) } }
fn sub(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_sub(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)), _ => Err(VmError::Type("subtraction requires matching numbers".into())) } }
fn mul(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_mul(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)), _ => Err(VmError::Type("multiplication requires matching numbers".into())) } }
fn div(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (_, Value::Int(0)) => Err(VmError::DivisionByZero), (_, Value::Float(v)) if v == 0.0 => Err(VmError::DivisionByZero), (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_div(b).ok_or(VmError::Overflow)?)), (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)), _ => Err(VmError::Type("division requires matching numbers".into())) } }
fn modulo(a: Value, b: Value) -> Result<Value, VmError> { match (a, b) { (_, Value::Int(0)) => Err(VmError::DivisionByZero), (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a.checked_rem(b).ok_or(VmError::Overflow)?)), _ => Err(VmError::Type("modulo requires integers".into())) } }
fn make_range(args: Vec<Value>) -> Result<Value, VmError> {
    let [Value::Int(start), Value::Int(end), Value::Bool(inclusive)] = args.as_slice() else { return Err(VmError::Type("range requires two integers".into())); };
    let stop = if *inclusive { end.checked_add(1).ok_or(VmError::Overflow)? } else { *end };
    let length = stop.saturating_sub(*start);
    if length > 1_000_000 { return Err(VmError::Type("range exceeds the one-million element safety limit".into())); }
    let values = if start <= &stop { (*start..stop).map(Value::Int).collect() } else { Vec::new() };
    Ok(Value::Array(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_codegen::AstCompiler;
    use titan_lexer::Lexer;
    use titan_parser::Parser;

    fn run(source: &str) -> Result<Value, String> {
        let mut lexer = Lexer::new(source); let tokens = lexer.tokenize().0.to_vec();
        let program = Parser::new(tokens).parse_program().map_err(|e| e.to_string())?;
        let module = AstCompiler::new().compile_program(&program).map_err(|e| e.to_string())?;
        Vm::new(module).run().map_err(|e| e.to_string()).map(|v| v.unwrap())
    }
    #[test] fn arithmetic_returns_value() { assert_eq!(run("fn main() { 40 + 2 }").unwrap(), Value::Int(42)); }
    #[test] fn recursion_works() { assert_eq!(run("fn fact(n: int) -> int { if n <= 1 { return 1 } n * fact(n-1) } fn main() { fact(5) }").unwrap(), Value::Int(120)); }
    #[test] fn loops_and_ranges_work() { assert_eq!(run("fn main() { let x = 0 for i in 0..5 { x += i } x }").unwrap(), Value::Int(10)); }
    #[test] fn structs_work() { assert_eq!(run("struct Point { x: int, y: int } fn main() { let p = Point { x: 2, y: 3 } p.x + p.y }").unwrap(), Value::Int(5)); }
    #[test] fn enum_matching_works() { assert_eq!(run("enum Maybe { None, Some(int) } fn main() { let x = Maybe::Some(7) match x { Maybe::Some(n) => n, Maybe::None => 0 } }").unwrap(), Value::Int(7)); }
    #[test] fn native_text_and_encoding_work() { assert_eq!(run("fn main() { std::text::reverse(\"Titan\") }").unwrap(), Value::Str("natiT".into())); assert_eq!(run("fn main() { std::encoding::utf8_decode(std::encoding::base64_decode(\"VGl0YW4=\")) }").unwrap(), Value::Str("Titan".into())); }
    #[test] fn native_json_maps_support_fields() { assert_eq!(run(r#"fn main() { std::json::parse("{\"answer\":42}").answer }"#).unwrap(), Value::Int(42)); }
    #[test] fn sandbox_blocks_effectful_natives() {
        let source = "fn main() { std::fs::read_text(\"secret\") }"; let mut lexer = Lexer::new(source); let tokens = lexer.tokenize().0.to_vec();
        let program = Parser::new(tokens).parse_program().unwrap(); let module = AstCompiler::new().compile_program(&program).unwrap();
        assert!(matches!(Vm::sandboxed(module).run(), Err(VmError::PermissionDenied { .. })));
    }
    #[test] fn runtime_errors_are_reported() { assert!(run("fn main() { 1 / 0 }").is_err()); }
}
