use crate::{CompiledModule, Op};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAGIC: &[u8] = b"TITAN-BYTECODE 1\n";
const FORMAT_VERSION: u32 = 1;
const MAX_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const MAX_FUNCTIONS: usize = 100_000;
const MAX_INSTRUCTIONS: usize = 10_000_000;
const MAX_STRINGS: usize = 1_000_000;

#[derive(Error, Debug)]
pub enum ArtifactError {
    #[error("bytecode artifact exceeds the {0} byte limit")]
    TooLarge(usize),
    #[error("invalid TITAN bytecode header")]
    InvalidHeader,
    #[error("unsupported bytecode format version {0}")]
    UnsupportedVersion(u32),
    #[error("malformed bytecode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bytecode checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid bytecode: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Envelope {
    format_version: u32,
    compiler_version: String,
    checksum_crc32: u32,
    module: CompiledModule,
}

pub struct BytecodeArtifact;

impl BytecodeArtifact {
    pub fn encode(module: &CompiledModule) -> Result<Vec<u8>, ArtifactError> {
        validate(module)?;
        let module_bytes = serde_json::to_vec(module)?;
        let envelope = Envelope {
            format_version: FORMAT_VERSION,
            compiler_version: env!("CARGO_PKG_VERSION").into(),
            checksum_crc32: titan_stdlib::checksum::crc32(&module_bytes),
            module: module.clone(),
        };
        let mut output = Vec::with_capacity(MAGIC.len() + module_bytes.len() + 128);
        output.extend_from_slice(MAGIC);
        output.extend(serde_json::to_vec_pretty(&envelope)?);
        output.push(b'\n');
        if output.len() > MAX_ARTIFACT_BYTES { return Err(ArtifactError::TooLarge(MAX_ARTIFACT_BYTES)); }
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<CompiledModule, ArtifactError> {
        if bytes.len() > MAX_ARTIFACT_BYTES { return Err(ArtifactError::TooLarge(MAX_ARTIFACT_BYTES)); }
        let payload = bytes.strip_prefix(MAGIC).ok_or(ArtifactError::InvalidHeader)?;
        let envelope: Envelope = serde_json::from_slice(payload)?;
        if envelope.format_version != FORMAT_VERSION { return Err(ArtifactError::UnsupportedVersion(envelope.format_version)); }
        let module_bytes = serde_json::to_vec(&envelope.module)?;
        if titan_stdlib::checksum::crc32(&module_bytes) != envelope.checksum_crc32 { return Err(ArtifactError::ChecksumMismatch); }
        validate(&envelope.module)?;
        Ok(envelope.module)
    }
}

fn validate(module: &CompiledModule) -> Result<(), ArtifactError> {
    if module.functions.is_empty() { return invalid("module has no functions"); }
    if module.functions.len() > MAX_FUNCTIONS { return invalid("function count exceeds safety limit"); }
    if module.entry >= module.functions.len() { return invalid("entry function is out of bounds"); }
    if module.string_table.len() > MAX_STRINGS { return invalid("string table exceeds safety limit"); }
    let instruction_count: usize = module.functions.iter().map(|function| function.code.len()).sum();
    if instruction_count > MAX_INSTRUCTIONS { return invalid("instruction count exceeds safety limit"); }

    for (function_index, function) in module.functions.iter().enumerate() {
        if function.name.len() > 4096 { return invalid(&format!("function {function_index} name is too long")); }
        if function.source_file.as_ref().is_some_and(|source| source.len() > 32_768) { return invalid(&format!("function '{}' source path is too long", function.name)); }
        if !function.debug_locations.is_empty() && function.debug_locations.len() != function.code.len() { return invalid(&format!("function '{}' has a malformed source map", function.name)); }
        if function.captures + function.arity > function.locals { return invalid(&format!("function '{}' has fewer locals than captures and arguments", function.name)); }
        if function.max_stack == 0 || function.max_stack > 1_000_000 { return invalid(&format!("function '{}' has invalid max stack", function.name)); }
        for (instruction_index, instruction) in function.code.iter().enumerate() {
            let location = || format!("function '{}' instruction {instruction_index}", function.name);
            match instruction {
                Op::PushStr(index) if *index >= module.string_table.len() => return invalid(&format!("{} references missing string {index}", location())),
                Op::PushLocal(index) | Op::StoreLocal(index) if *index >= function.locals => return invalid(&format!("{} references missing local {index}", location())),
                Op::Jump(target) | Op::JumpIfFalse(target) if *target > function.code.len() => return invalid(&format!("{} has invalid jump target {target}", location())),
                Op::Call { function: target, argc } => {
                    let target_function = module.functions.get(*target).ok_or_else(|| ArtifactError::Invalid(format!("{} calls missing function {target}", location())))?;
                    if *argc != target_function.arity { return invalid(&format!("{} calls '{}' with wrong arity", location(), target_function.name)); }
                    if target_function.captures != 0 { return invalid(&format!("{} directly calls closure body '{}'", location(), target_function.name)); }
                }
                Op::CallNative { name, argc } => {
                    let signature = titan_stdlib::native::lookup(name).ok_or_else(|| ArtifactError::Invalid(format!("{} calls unknown native '{name}'", location())))?;
                    if *argc != signature.params.len() { return invalid(&format!("{} calls native '{name}' with wrong arity", location())); }
                }
                Op::MakeClosure { function: target, captures } => {
                    let target_function = module.functions.get(*target).ok_or_else(|| ArtifactError::Invalid(format!("{} references missing closure {target}", location())))?;
                    if captures.len() != target_function.captures { return invalid(&format!("{} supplies wrong capture count for '{}'", location(), target_function.name)); }
                    if captures.iter().any(|capture| *capture >= function.locals) { return invalid(&format!("{} captures a missing local", location())); }
                }
                Op::NewArray(count) | Op::NewTuple(count) | Op::Print(count) | Op::CallValue(count) if *count > 1_000_000 => return invalid(&format!("{} has an excessive operand count", location())),
                Op::NewStruct { name, fields } if name.len() > 4096 || fields.len() > 100_000 => return invalid(&format!("{} has invalid struct metadata", location())),
                Op::PushFloat(value) if !value.is_finite() => return invalid(&format!("{} contains a non-finite float", location())),
                _ => {}
            }
        }
    }
    Ok(())
}

fn invalid<T>(message: &str) -> Result<T, ArtifactError> { Err(ArtifactError::Invalid(message.into())) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BytecodeFunc, CompiledModule, Op};

    fn module() -> CompiledModule {
        CompiledModule {
            functions: vec![BytecodeFunc { name: "main".into(), source_file: Some("main.titan".into()), arity: 0, captures: 0, locals: 0, max_stack: 8, code: vec![Op::PushInt(42), Op::Ret], debug_locations: vec![None, None] }],
            entry: 0,
            string_table: Vec::new(),
        }
    }

    #[test]
    fn round_trips_valid_artifacts() {
        let module = module(); let encoded = BytecodeArtifact::encode(&module).unwrap();
        assert!(encoded.starts_with(MAGIC)); assert_eq!(BytecodeArtifact::decode(&encoded).unwrap(), module);
    }

    #[test]
    fn rejects_corruption_and_invalid_references() {
        let mut encoded = BytecodeArtifact::encode(&module()).unwrap();
        let last = encoded.len() - 2; encoded[last] ^= 1;
        assert!(BytecodeArtifact::decode(&encoded).is_err());
        let mut invalid_module = module(); invalid_module.functions[0].code.insert(0, Op::PushLocal(99));
        assert!(BytecodeArtifact::encode(&invalid_module).is_err());
    }
}
