//! Direct WebAssembly backend for Titan's portable numeric bytecode.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::Serialize;
use thiserror::Error;
use titan_codegen::{BytecodeFunc, BytecodeType, CompiledModule, Op};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, DataSection, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection, Instruction, MemArg,
    MemorySection, MemoryType, Module, TypeSection, ValType,
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
    #[error("function '{function}' references invalid string-table entry {string}")]
    InvalidString { function: String, string: usize },
    #[error("WebAssembly string data exceeds the 32-bit linear-memory ABI")]
    StringDataTooLarge,
    #[error("function '{function}' has an ambiguous '+' at instruction {instruction}; string and numeric operands cannot be distinguished from erased bytecode")]
    AmbiguousAdd {
        function: String,
        instruction: usize,
    },
    #[error("function name '{0}' is reserved by the WebAssembly runtime ABI")]
    ReservedFunction(String),
    #[error("could not encode WebAssembly source metadata: {0}")]
    SourceMap(String),
    #[error("enum discriminant collision between '{first}' and '{second}' at tag {tag:#018x}")]
    EnumTagCollision { first: String, second: String, tag: u64 },
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmArtifact {
    #[serde(skip)]
    pub wasm: Vec<u8>,
    pub source_map: WasmSourceMap,
    pub standard_source_map: StandardSourceMap,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmSourceMap {
    pub format: &'static str,
    pub version: u32,
    pub imported_function_count: u32,
    pub enum_tags: BTreeMap<String, u64>,
    pub functions: Vec<WasmFunctionMap>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StandardSourceMap {
    pub version: u32,
    pub sources: Vec<String>,
    #[serde(rename = "sourcesContent", skip_serializing_if = "Option::is_none")]
    pub sources_content: Option<Vec<Option<String>>>,
    pub names: Vec<String>,
    pub mappings: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmFunctionMap {
    pub titan_function_index: u32,
    pub wasm_function_index: u32,
    pub name: String,
    pub source_file: Option<String>,
    pub instructions: Vec<WasmInstructionMap>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmInstructionMap {
    pub titan_instruction: u32,
    pub wasm_offset: Option<usize>,
    pub operation: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy)]
struct HostImport {
    native: Option<&'static str>,
    field: &'static str,
    params: usize,
    returns_value: bool,
}

const PRINT_IMPORT: HostImport = HostImport {
    native: None,
    field: "print",
    params: 1,
    returns_value: false,
};

const WEB_IMPORTS: &[HostImport] = &[
    HostImport { native: Some("std::web::query_exists"), field: "dom_query_exists", params: 1, returns_value: true },
    HostImport { native: Some("std::web::set_text"), field: "dom_set_text", params: 2, returns_value: false },
    HostImport { native: Some("std::web::set_html"), field: "dom_set_html", params: 2, returns_value: false },
    HostImport { native: Some("std::web::set_attribute"), field: "dom_set_attribute", params: 3, returns_value: false },
    HostImport { native: Some("std::web::add_class"), field: "dom_add_class", params: 2, returns_value: false },
    HostImport { native: Some("std::web::remove_class"), field: "dom_remove_class", params: 2, returns_value: false },
    HostImport { native: Some("std::web::focus"), field: "dom_focus", params: 1, returns_value: false },
    HostImport { native: Some("std::web::set_title"), field: "dom_set_title", params: 1, returns_value: false },
    HostImport { native: Some("std::web::listen"), field: "dom_listen", params: 3, returns_value: true },
    HostImport { native: Some("std::web::unlisten"), field: "dom_unlisten", params: 1, returns_value: true },
    HostImport { native: Some("std::web::event_type"), field: "dom_event_type", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_value"), field: "dom_event_value", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_key"), field: "dom_event_key", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_target_id"), field: "dom_event_target_id", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_checked"), field: "dom_event_checked", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_x"), field: "dom_event_x", params: 0, returns_value: true },
    HostImport { native: Some("std::web::event_y"), field: "dom_event_y", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch"), field: "fetch_start", params: 4, returns_value: true },
    HostImport { native: Some("std::web::fetch_cancel"), field: "fetch_cancel", params: 1, returns_value: true },
    HostImport { native: Some("std::web::fetch_ok"), field: "fetch_ok", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch_status"), field: "fetch_status", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch_body"), field: "fetch_body", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch_url"), field: "fetch_url", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch_error"), field: "fetch_error", params: 0, returns_value: true },
    HostImport { native: Some("std::web::fetch_headers"), field: "fetch_headers", params: 0, returns_value: true },
    HostImport { native: Some("std::web::request"), field: "fetch_request", params: 7, returns_value: true },
    HostImport { native: Some("std::web::ws_connect"), field: "ws_connect", params: 7, returns_value: true },
    HostImport { native: Some("std::web::ws_send"), field: "ws_send", params: 2, returns_value: true },
    HostImport { native: Some("std::web::ws_close"), field: "ws_close", params: 3, returns_value: true },
    HostImport { native: Some("std::web::ws_id"), field: "ws_id", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_message"), field: "ws_message", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_protocol"), field: "ws_protocol", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_close_code"), field: "ws_close_code", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_close_reason"), field: "ws_close_reason", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_was_clean"), field: "ws_was_clean", params: 0, returns_value: true },
    HostImport { native: Some("std::web::ws_error"), field: "ws_error", params: 0, returns_value: true },
    HostImport { native: Some("std::web::canvas_resize"), field: "canvas_resize", params: 3, returns_value: false },
    HostImport { native: Some("std::web::canvas_clear"), field: "canvas_clear", params: 2, returns_value: false },
    HostImport { native: Some("std::web::canvas_fill_rect"), field: "canvas_fill_rect", params: 6, returns_value: false },
    HostImport { native: Some("std::web::canvas_stroke_rect"), field: "canvas_stroke_rect", params: 7, returns_value: false },
    HostImport { native: Some("std::web::canvas_line"), field: "canvas_line", params: 7, returns_value: false },
    HostImport { native: Some("std::web::canvas_text"), field: "canvas_text", params: 6, returns_value: false },
    HostImport { native: Some("std::web::animation_start"), field: "animation_start", params: 1, returns_value: true },
    HostImport { native: Some("std::web::animation_cancel"), field: "animation_cancel", params: 1, returns_value: true },
    HostImport { native: Some("std::web::frame_id"), field: "frame_id", params: 0, returns_value: true },
    HostImport { native: Some("std::web::frame_time_ms"), field: "frame_time_ms", params: 0, returns_value: true },
    HostImport { native: Some("std::web::frame_delta_ms"), field: "frame_delta_ms", params: 0, returns_value: true },
    HostImport { native: Some("std::web::frame_count"), field: "frame_count", params: 0, returns_value: true },
    HostImport { native: Some("std::web::webgl_supported"), field: "webgl_supported", params: 1, returns_value: true },
    HostImport { native: Some("std::web::webgl_create"), field: "webgl_create", params: 5, returns_value: true },
    HostImport { native: Some("std::web::webgl_uniform_f32"), field: "webgl_uniform_f32", params: 4, returns_value: true },
    HostImport { native: Some("std::web::webgl_draw"), field: "webgl_draw", params: 2, returns_value: true },
    HostImport { native: Some("std::web::webgl_delete"), field: "webgl_delete", params: 1, returns_value: true },
];

struct HostImports {
    definitions: Vec<HostImport>,
    print: Option<u32>,
    natives: HashMap<&'static str, u32>,
}

fn array_native_arity(name: &str) -> Option<usize> {
    match name {
        "std::array::set" => Some(3),
        "std::array::push" => Some(2),
        "std::array::pop" => Some(1),
        "std::array::slice" => Some(3),
        "std::array::concat" => Some(2),
        _ => None,
    }
}

fn wasm_heap_native_arity(name: &str) -> Option<usize> {
    match name {
        "std::wasm::heap_used"
        | "std::wasm::heap_capacity"
        | "std::wasm::heap_limit"
        | "std::wasm::heap_checkpoint"
        | "std::wasm::heap_allocations"
        | "std::wasm::heap_allocated_bytes"
        | "std::wasm::heap_restores"
        | "std::wasm::heap_reclaimed_bytes"
        | "std::wasm::heap_peak_used"
        | "std::wasm::heap_reset_counters"
        | "std::wasm::heap_scope_begin" => Some(0),
        "std::wasm::heap_set_limit" | "std::wasm::heap_restore" | "std::wasm::heap_scope_end" => Some(1),
        _ => None,
    }
}

fn web_import(name: &str) -> Option<&'static HostImport> {
    WEB_IMPORTS
        .iter()
        .find(|definition| definition.native == Some(name))
}

fn collect_host_imports(module: &CompiledModule) -> HostImports {
    let needs_print = module.functions.iter().any(|function| {
        function.code.iter().any(|operation| matches!(operation, Op::Print(_)))
    });
    let mut definitions = Vec::new();
    let mut print = None;
    let mut natives = HashMap::new();
    if needs_print {
        print = Some(0);
        definitions.push(PRINT_IMPORT);
    }
    for definition in WEB_IMPORTS {
        let Some(native) = definition.native else { continue };
        let used = module.functions.iter().any(|function| {
            function.code.iter().any(|operation| {
                matches!(operation, Op::CallNative { name, .. } if name == native)
            })
        });
        if used {
            let index = definitions.len() as u32;
            definitions.push(*definition);
            natives.insert(native, index);
        }
    }
    HostImports { definitions, print, natives }
}

/// Compile Titan numeric bytecode directly to a self-contained WebAssembly module.
///
/// Numeric operand-stack values are assigned to WebAssembly locals. A structured
/// Wasm dispatch loop represents Titan branches, including backward branches,
/// without relying on the host VM. This supports arbitrary validated bytecode
/// control-flow graphs while preserving WebAssembly's structured-control rules.
/// Operations requiring managed runtime values are rejected explicitly.
pub fn compile(module: &CompiledModule) -> Result<Vec<u8>, WasmError> {
    Ok(compile_artifact(module)?.wasm)
}

pub fn compile_artifact(module: &CompiledModule) -> Result<WasmArtifact, WasmError> {
    compile_artifact_with_source_root(module, None)
}

pub fn compile_artifact_with_source_root(
    module: &CompiledModule,
    source_root: Option<&Path>,
) -> Result<WasmArtifact, WasmError> {
    if module.entry >= module.functions.len() {
        return Err(WasmError::Entry);
    }

    let strings = StringLayout::new(&module.string_table)?;
    let enum_tags = collect_enum_tags(module)?;
    let layouts = module
        .functions
        .iter()
        .map(|function| analyze_function(module, function))
        .collect::<Result<Vec<_>, _>>()?;
    let needs_concat = layouts
        .iter()
        .any(|layout| !layout.string_adds.is_empty());
    let needs_arrays = module.functions.iter().any(|function| {
        function
            .code
            .iter()
            .any(|operation| {
                matches!(operation, Op::NewArray(_) | Op::NewTuple(_) | Op::NewStruct { .. } | Op::NewEnum { .. })
                    || matches!(operation, Op::CallNative { name, .. } if array_native_arity(name).is_some())
            })
    });
    let needs_heap_api = module.functions.iter().any(|function| {
        function.code.iter().any(|operation| {
            matches!(operation, Op::CallNative { name, .. } if wasm_heap_native_arity(name).is_some())
        })
    });
    let host_imports = collect_host_imports(module);
    let needs_host_strings = host_imports.natives.keys().any(|name| {
        matches!(
            *name,
            "std::web::event_type"
                | "std::web::event_value"
                | "std::web::event_key"
                | "std::web::event_target_id"
                | "std::web::fetch_body"
                | "std::web::fetch_url"
                | "std::web::fetch_error"
                | "std::web::fetch_headers"
                | "std::web::ws_message"
                | "std::web::ws_protocol"
                | "std::web::ws_close_reason"
                | "std::web::ws_error"
        )
    });
    if needs_host_strings
        && module
            .functions
            .iter()
            .any(|function| function.name == "__titan_alloc_string")
    {
        return Err(WasmError::ReservedFunction("__titan_alloc_string".into()));
    }
    let needs_scopes = module.functions.iter().any(|function| {
        function.code.iter().any(|operation| {
            matches!(operation, Op::CallNative { name, .. } if matches!(name.as_str(), "std::wasm::heap_scope_begin" | "std::wasm::heap_scope_end"))
        })
    });
    let needs_allocator = needs_host_strings || needs_arrays || needs_scopes;
    let needs_heap = needs_concat || needs_allocator || needs_heap_api;
    let function_bias = host_imports.definitions.len() as u32;
    let concat_function = needs_concat
        .then_some(function_bias + module.functions.len() as u32);
    let allocator_function = needs_allocator.then_some(
        function_bias + module.functions.len() as u32 + if needs_concat { 1 } else { 0 },
    );
    let mut output = Module::new();

    let mut types = TypeSection::new();
    for function in &module.functions {
        types
            .ty()
            .function(vec![ValType::I64; function.arity], [ValType::I64]);
    }
    let first_import_type = types.len();
    for import in &host_imports.definitions {
        let results = if import.returns_value {
            vec![ValType::I64]
        } else {
            Vec::new()
        };
        types
            .ty()
            .function(vec![ValType::I64; import.params], results);
    }
    let concat_type = types.len();
    if needs_concat {
        types
            .ty()
            .function([ValType::I64, ValType::I64], [ValType::I64]);
    }
    let allocator_type = types.len();
    if needs_allocator {
        types
            .ty()
            .function([ValType::I32, ValType::I32], [ValType::I64]);
    }
    output.section(&types);

    if !host_imports.definitions.is_empty() {
        let mut imports = ImportSection::new();
        for (offset, import) in host_imports.definitions.iter().enumerate() {
            imports.import(
                "titan",
                import.field,
                EntityType::Function(first_import_type + offset as u32),
            );
        }
        output.section(&imports);
    }

    let mut functions = FunctionSection::new();
    for index in 0..module.functions.len() {
        functions.function(index as u32);
    }
    if needs_concat {
        functions.function(concat_type);
    }
    if needs_allocator {
        functions.function(allocator_type);
    }
    output.section(&functions);

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: strings.minimum_pages,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    output.section(&memories);

    if needs_heap {
        let mut globals = GlobalSection::new();
        let mutable_i32 = GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        };
        globals.global(
            mutable_i32,
            &ConstExpr::i32_const(strings.heap_start as i32),
        );
        globals.global(mutable_i32, &ConstExpr::i32_const(-1));
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: false,
                shared: false,
            },
            &ConstExpr::i32_const(strings.heap_start as i32),
        );
        let counter_type = GlobalType {
            val_type: ValType::I64,
            mutable: true,
            shared: false,
        };
        for _ in 0..4 {
            globals.global(counter_type, &ConstExpr::i64_const(0));
        }
        globals.global(
            counter_type,
            &ConstExpr::i64_const(i64::from(strings.heap_start)),
        );
        globals.global(mutable_i32, &ConstExpr::i32_const(0));
        output.section(&globals);
    }

    let mut exports = ExportSection::new();
    exports.export(
        "main",
        ExportKind::Func,
        module.entry as u32 + function_bias,
    );
    exports.export("memory", ExportKind::Memory, 0);
    if needs_host_strings {
        exports.export(
            "__titan_alloc_string",
            ExportKind::Func,
            allocator_function.expect("host strings require allocator"),
        );
    }
    for (index, function) in module.functions.iter().enumerate() {
        if function.name != "main" && !function.name.starts_with('<') {
            exports.export(
                &function.name,
                ExportKind::Func,
                index as u32 + function_bias,
            );
        }
    }
    output.section(&exports);

    let mut code = CodeSection::new();
    for (function, layout) in module.functions.iter().zip(&layouts) {
        code.function(&compile_function(
            function,
            layout,
            &strings,
            &host_imports,
            concat_function,
            allocator_function,
        )?);
    }
    if needs_concat {
        code.function(&compile_string_concat());
    }
    if needs_allocator {
        code.function(&compile_host_string_allocator());
    }
    output.section(&code);

    if !strings.data.is_empty() {
        let mut data = DataSection::new();
        data.active(
            0,
            &ConstExpr::i32_const(StringLayout::DATA_START as i32),
            strings.data.iter().copied(),
        );
        output.section(&data);
    }

    let mut wasm = output.finish();
    let offsets = extract_instruction_offsets(&wasm, &layouts, module.functions.len())?;
    let source_map = build_source_map(module, function_bias, source_root, &offsets, enum_tags);
    let standard_source_map = build_standard_source_map(&source_map);
    let source_map_bytes = serde_json::to_vec(&source_map)
        .map_err(|error| WasmError::SourceMap(error.to_string()))?;
    append_custom_section(&mut wasm, "titan.source_map", &source_map_bytes);

    Ok(WasmArtifact {
        wasm,
        source_map,
        standard_source_map,
    })
}

fn build_source_map(
    module: &CompiledModule,
    function_bias: u32,
    source_root: Option<&Path>,
    offsets: &HashMap<(usize, usize), usize>,
    enum_tags: BTreeMap<String, u64>,
) -> WasmSourceMap {
    let functions = module
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, function)| {
            let instructions = function
                .debug_locations
                .iter()
                .enumerate()
                .filter_map(|(instruction, location)| {
                    location.map(|location| WasmInstructionMap {
                        titan_instruction: instruction as u32,
                        wasm_offset: offsets.get(&(function_index, instruction)).copied(),
                        operation: function
                            .code
                            .get(instruction)
                            .map_or_else(|| "<missing>".into(), |operation| format!("{operation:?}")),
                        start: location.start,
                        end: location.end,
                        line: location.line,
                        column: location.column,
                    })
                })
                .collect();
            WasmFunctionMap {
                titan_function_index: function_index as u32,
                wasm_function_index: function_bias + function_index as u32,
                name: function.name.clone(),
                source_file: normalize_source_file(function.source_file.as_deref(), source_root),
                instructions,
            }
        })
        .collect();
    WasmSourceMap {
        format: "titan-wasm-source-map",
        version: 1,
        imported_function_count: function_bias,
        enum_tags,
        functions,
    }
}

fn extract_instruction_offsets(
    wasm: &[u8],
    layouts: &[FunctionLayout],
    titan_function_count: usize,
) -> Result<HashMap<(usize, usize), usize>, WasmError> {
    let mut offsets = HashMap::new();
    let mut body_index = 0usize;
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|error| WasmError::SourceMap(error.to_string()))?;
        let wasmparser::Payload::CodeSectionEntry(body) = payload else {
            continue;
        };
        if body_index < titan_function_count {
            let expected: Vec<_> = layouts[body_index]
                .heights
                .iter()
                .enumerate()
                .filter_map(|(instruction, height)| height.map(|_| instruction))
                .collect();
            let mut marker = 0usize;
            let mut previous_was_end = false;
            let mut reader = body
                .get_operators_reader()
                .map_err(|error| WasmError::SourceMap(error.to_string()))?;
            while !reader.eof() {
                let (operator, offset) = reader
                    .read_with_offset()
                    .map_err(|error| WasmError::SourceMap(error.to_string()))?;
                if previous_was_end && matches!(&operator, wasmparser::Operator::Nop) {
                    let instruction = expected.get(marker).ok_or_else(|| {
                        WasmError::SourceMap(format!(
                            "unexpected source marker in function {body_index}"
                        ))
                    })?;
                    offsets.insert((body_index, *instruction), offset);
                    marker += 1;
                }
                previous_was_end = matches!(&operator, wasmparser::Operator::End);
            }
            if marker != expected.len() {
                return Err(WasmError::SourceMap(format!(
                    "function {body_index} emitted {marker} source markers, expected {}",
                    expected.len()
                )));
            }
        }
        body_index += 1;
    }
    if body_index < titan_function_count {
        return Err(WasmError::SourceMap(format!(
            "module contains {body_index} function bodies, expected at least {titan_function_count}"
        )));
    }
    Ok(offsets)
}

fn build_standard_source_map(source_map: &WasmSourceMap) -> StandardSourceMap {
    let mut source_names = BTreeMap::new();
    let mut function_names = BTreeMap::new();
    for function in &source_map.functions {
        if let Some(source) = &function.source_file {
            source_names.insert(source.clone(), 0usize);
        }
        function_names.insert(function.name.clone(), 0usize);
    }
    let sources: Vec<_> = source_names.keys().cloned().collect();
    let names: Vec<_> = function_names.keys().cloned().collect();
    for (index, source) in sources.iter().enumerate() {
        source_names.insert(source.clone(), index);
    }
    for (index, name) in names.iter().enumerate() {
        function_names.insert(name.clone(), index);
    }

    let mut entries = Vec::new();
    for function in &source_map.functions {
        let Some(source) = &function.source_file else {
            continue;
        };
        for instruction in &function.instructions {
            if let Some(offset) = instruction.wasm_offset {
                entries.push((
                    offset,
                    source_names[source],
                    instruction.line.saturating_sub(1),
                    instruction.column.saturating_sub(1),
                    function_names[&function.name],
                ));
            }
        }
    }
    entries.sort_unstable_by_key(|entry| entry.0);

    let mut mappings = String::new();
    let mut previous_generated = 0i64;
    let mut previous_source = 0i64;
    let mut previous_line = 0i64;
    let mut previous_column = 0i64;
    let mut previous_name = 0i64;
    for (index, (generated, source, line, column, name)) in entries.into_iter().enumerate() {
        if index > 0 {
            mappings.push(',');
        }
        let generated = generated as i64;
        let source = source as i64;
        let line = line as i64;
        let column = column as i64;
        let name = name as i64;
        encode_vlq(generated - previous_generated, &mut mappings);
        encode_vlq(source - previous_source, &mut mappings);
        encode_vlq(line - previous_line, &mut mappings);
        encode_vlq(column - previous_column, &mut mappings);
        encode_vlq(name - previous_name, &mut mappings);
        previous_generated = generated;
        previous_source = source;
        previous_line = line;
        previous_column = column;
        previous_name = name;
    }

    StandardSourceMap {
        version: 3,
        sources,
        sources_content: None,
        names,
        mappings,
    }
}

fn encode_vlq(value: i64, output: &mut String) {
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut value = if value < 0 {
        ((-value) as u64) << 1 | 1
    } else {
        (value as u64) << 1
    };
    loop {
        let mut digit = (value & 31) as u8;
        value >>= 5;
        if value != 0 {
            digit |= 32;
        }
        output.push(char::from(BASE64[usize::from(digit)]));
        if value == 0 {
            break;
        }
    }
}

fn append_custom_section(wasm: &mut Vec<u8>, name: &str, data: &[u8]) {
    let mut extension = Module::new();
    extension.section(&CustomSection {
        name: Cow::Borrowed(name),
        data: Cow::Borrowed(data),
    });
    let encoded = extension.finish();
    wasm.extend_from_slice(&encoded[8..]);
}

pub fn append_source_mapping_url(wasm: &mut Vec<u8>, url: &str) -> Result<(), WasmError> {
    if !wasm.starts_with(b"\0asm") {
        return Err(WasmError::SourceMap(
            "cannot append sourceMappingURL to invalid Wasm bytes".into(),
        ));
    }
    append_custom_section(wasm, "sourceMappingURL", url.as_bytes());
    Ok(())
}

fn normalize_source_file(source: Option<&str>, source_root: Option<&Path>) -> Option<String> {
    let source = source?;
    let path = Path::new(source);
    let displayed = source_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    Some(displayed)
}

struct StringLayout {
    handles: Vec<i64>,
    data: Vec<u8>,
    minimum_pages: u64,
    heap_start: u32,
}

impl StringLayout {
    const DATA_START: u32 = 8;
    const PAGE_SIZE: u64 = 65_536;

    fn new(strings: &[String]) -> Result<Self, WasmError> {
        let mut handles = Vec::with_capacity(strings.len());
        let mut data = Vec::new();
        for string in strings {
            while !(Self::DATA_START as usize + data.len()).is_multiple_of(4) {
                data.push(0);
            }
            let byte_length =
                u32::try_from(string.len()).map_err(|_| WasmError::StringDataTooLarge)?;
            let scalar_length = u32::try_from(string.chars().count())
                .map_err(|_| WasmError::StringDataTooLarge)?;
            let data_length =
                u32::try_from(data.len()).map_err(|_| WasmError::StringDataTooLarge)?;
            let pointer = Self::DATA_START
                .checked_add(data_length)
                .and_then(|address| address.checked_add(4))
                .ok_or(WasmError::StringDataTooLarge)?;
            handles.push(i64::from_ne_bytes(
                ((u64::from(byte_length) << 32) | u64::from(pointer)).to_ne_bytes(),
            ));
            data.extend_from_slice(&scalar_length.to_le_bytes());
            data.extend_from_slice(string.as_bytes());
        }
        let end = u64::from(Self::DATA_START)
            .checked_add(u64::try_from(data.len()).map_err(|_| WasmError::StringDataTooLarge)?)
            .ok_or(WasmError::StringDataTooLarge)?;
        let aligned_end = end
            .checked_add(3)
            .ok_or(WasmError::StringDataTooLarge)?
            & !3;
        let heap_start = u32::try_from(aligned_end).map_err(|_| WasmError::StringDataTooLarge)?;
        let minimum_pages = aligned_end.div_ceil(Self::PAGE_SIZE).max(1);
        Ok(Self {
            handles,
            data,
            minimum_pages,
            heap_start,
        })
    }
}

fn compile_function(
    function: &BytecodeFunc,
    layout: &FunctionLayout,
    strings: &StringLayout,
    host_imports: &HostImports,
    concat_function: Option<u32>,
    allocator_function: Option<u32>,
) -> Result<Function, WasmError> {
    let extra = function
        .locals
        .checked_sub(function.arity)
        .ok_or_else(|| WasmError::Locals(function.name.clone()))?;
    let linear = is_linear_function(function, layout);
    let direct_control = has_reducible_control_flow(function, layout);
    let direct = linear || direct_control;
    // One scratch i64 local supports managed-value construction without
    // overwriting operand slots before their payload is copied.
    let numeric_locals = extra + layout.stack_slots + 1;
    let mut locals = Vec::new();
    if numeric_locals > 0 {
        locals.push((numeric_locals as u32, ValType::I64));
    }
    if !direct {
        locals.push((1, ValType::I32));
    }

    let mut body = Function::new(locals);
    let context = EmitContext {
        function,
        layout,
        strings,
        host_imports,
        concat_function,
        allocator_function,
        managed_scratch: layout.pc_local,
        pc_local: layout.pc_local + 1,
    };
    let reachable: Vec<_> = layout
        .heights
        .iter()
        .enumerate()
        .filter_map(|(instruction, height)| height.map(|height| (instruction, height)))
        .collect();
    if reachable.is_empty() {
        body.instruction(&Instruction::I64Const(0));
        body.instruction(&Instruction::End);
        return Ok(body);
    }
    if linear {
        for (instruction, height) in reachable {
            // Empty block + marker keeps the post-encoding offset pass uniform
            // across direct and dispatcher-based lowering.
            emit_source_marker(&mut body);
            emit_operation(&context, instruction, height, None, &mut body)?;
        }
        body.instruction(&Instruction::End);
        return Ok(body);
    }
    if direct_control {
        emit_direct_region(&context, 0, function.code.len(), None, &mut body)?;
        body.instruction(&Instruction::End);
        return Ok(body);
    }

    // Arbitrary Titan jumps are dispatched in O(1) through nested structured
    // blocks and br_table. The outer block is the invalid-PC trap target.
    body.instruction(&Instruction::Block(BlockType::Empty));
    body.instruction(&Instruction::Loop(BlockType::Empty));
    for _ in &reachable {
        body.instruction(&Instruction::Block(BlockType::Empty));
    }
    let default_depth = reachable.len() as u32 + 1;
    let mut targets = vec![default_depth; function.code.len()];
    for (depth, (instruction, _)) in reachable.iter().enumerate() {
        targets[*instruction] = depth as u32;
    }
    body.instruction(&Instruction::LocalGet(context.pc_local));
    body.instruction(&Instruction::BrTable(Cow::Owned(targets), default_depth));

    for (position, (instruction, height)) in reachable.iter().enumerate() {
        body.instruction(&Instruction::End);
        // A marker for the post-encoding binary-offset source-map pass.
        body.instruction(&Instruction::Nop);
        let dispatch_depth = (reachable.len() - position - 1) as u32;
        emit_operation(
            &context,
            *instruction,
            *height,
            Some(dispatch_depth),
            &mut body,
        )?;
    }

    // Every case either returns or branches back to the loop. Invalid PCs leave
    // the outer block and trap here instead of spinning forever.
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::I64Const(0));
    body.instruction(&Instruction::End);
    Ok(body)
}

#[derive(Clone, Copy)]
struct DirectLoopBranches {
    continue_target: usize,
    break_target: usize,
    continue_depth: u32,
    break_depth: u32,
}

impl DirectLoopBranches {
    fn deeper(self, levels: u32) -> Self {
        Self {
            continue_depth: self.continue_depth + levels,
            break_depth: self.break_depth + levels,
            ..self
        }
    }
}

fn emit_direct_region(
    context: &EmitContext<'_>,
    start: usize,
    end: usize,
    active_loop: Option<DirectLoopBranches>,
    body: &mut Function,
) -> Result<(), WasmError> {
    let mut instruction = start;
    while instruction < end {
        if context.layout.heights[instruction].is_none() {
            instruction += 1;
            continue;
        }
        if let Some(region) = direct_loop_region_at(context.function, instruction, end) {
            body.instruction(&Instruction::Block(BlockType::Empty));
            body.instruction(&Instruction::Loop(BlockType::Empty));
            emit_direct_region(
                context,
                instruction,
                region.condition,
                active_loop.map(|branches| branches.deeper(2)),
                body,
            )?;
            emit_source_marker(body);
            let condition_height = context.layout.heights[region.condition]
                .expect("direct loop condition is reachable");
            body.instruction(&Instruction::LocalGet(
                context.layout.stack_base + (condition_height - 1) as u32,
            ));
            body.instruction(&Instruction::I64Eqz);
            body.instruction(&Instruction::BrIf(1));
            emit_direct_region(
                context,
                region.condition + 1,
                region.back_jump,
                Some(DirectLoopBranches {
                    continue_target: instruction,
                    break_target: region.end,
                    continue_depth: 0,
                    break_depth: 1,
                }),
                body,
            )?;
            // Preserve the logical backward Jump when it is reachable. The Br is
            // still emitted in unreachable position to keep the loop closed.
            if context.layout.heights[region.back_jump].is_some() {
                emit_source_marker(body);
            }
            body.instruction(&Instruction::Br(0));
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::End);
            instruction = region.end;
        } else if matches!(&context.function.code[instruction], Op::JumpIfFalse(_)) {
            let region = direct_if_region_at(context.function, instruction, end)
                .expect("reducible regions are validated before emission");
            emit_source_marker(body);
            let condition_height = context.layout.heights[instruction]
                .expect("direct condition is reachable");
            body.instruction(&Instruction::LocalGet(
                context.layout.stack_base + (condition_height - 1) as u32,
            ));
            body.instruction(&Instruction::I64Eqz);
            body.instruction(&Instruction::I32Eqz);
            body.instruction(&Instruction::If(BlockType::Empty));
            emit_direct_region(
                context,
                instruction + 1,
                region.then_jump,
                active_loop.map(|branches| branches.deeper(1)),
                body,
            )?;
            // Preserve a source location for a reachable logical jump now
            // represented by the structured else edge.
            if context.layout.heights[region.then_jump].is_some() {
                emit_source_marker(body);
            }
            body.instruction(&Instruction::Else);
            emit_direct_region(
                context,
                region.else_start,
                region.end,
                active_loop.map(|branches| branches.deeper(1)),
                body,
            )?;
            body.instruction(&Instruction::End);
            instruction = region.end;
        } else if let Op::Jump(target) = &context.function.code[instruction] {
            let branches = active_loop.expect("reducible jumps require an active loop");
            emit_source_marker(body);
            if *target == branches.continue_target {
                body.instruction(&Instruction::Br(branches.continue_depth));
            } else {
                debug_assert_eq!(*target, branches.break_target);
                body.instruction(&Instruction::Br(branches.break_depth));
            }
            instruction += 1;
        } else {
            emit_source_marker(body);
            let height = context.layout.heights[instruction]
                .expect("direct region instruction is reachable");
            emit_operation(context, instruction, height, None, body)?;
            instruction += 1;
        }
    }
    Ok(())
}

fn collect_enum_tags(module: &CompiledModule) -> Result<BTreeMap<String, u64>, WasmError> {
    let mut tags = BTreeMap::new();
    let mut owners = HashMap::new();
    for operation in module.functions.iter().flat_map(|function| &function.code) {
        let (name, variant) = match operation {
            Op::NewEnum { name, variant, .. } | Op::EnumIs { name, variant } => (name, variant),
            _ => continue,
        };
        let label = format!("{name}::{variant}");
        let tag = enum_tag(name, variant);
        if let Some(first) = owners.insert(tag, label.clone()) {
            if first != label {
                return Err(WasmError::EnumTagCollision { first, second: label, tag });
            }
        }
        tags.insert(label, tag);
    }
    Ok(tags)
}

fn enum_tag(name: &str, variant: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.bytes().chain([b':', b':']).chain(variant.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn emit_source_marker(body: &mut Function) {
    body.instruction(&Instruction::Block(BlockType::Empty));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::Nop);
}

struct FunctionLayout {
    heights: Vec<Option<usize>>,
    stack_base: u32,
    stack_slots: usize,
    pc_local: u32,
    string_adds: HashSet<usize>,
    struct_fields: HashMap<usize, usize>,
}

fn is_linear_function(function: &BytecodeFunc, layout: &FunctionLayout) -> bool {
    layout.heights.iter().all(Option::is_some)
        && !function
            .code
            .iter()
            .any(|operation| matches!(operation, Op::Jump(_) | Op::JumpIfFalse(_)))
}

#[derive(Debug, Clone, Copy)]
struct DirectIfRegion {
    then_jump: usize,
    else_start: usize,
    end: usize,
}

fn direct_if_region_at(
    function: &BytecodeFunc,
    conditional: usize,
    limit: usize,
) -> Option<DirectIfRegion> {
    let Op::JumpIfFalse(else_start) = function.code.get(conditional)? else {
        return None;
    };
    let then_jump = else_start.checked_sub(1)?;
    let Op::Jump(end) = function.code.get(then_jump)? else {
        return None;
    };
    if conditional >= then_jump || *end <= *else_start || *end > limit {
        return None;
    }
    Some(DirectIfRegion {
        then_jump,
        else_start: *else_start,
        end: *end,
    })
}

#[derive(Debug, Clone, Copy)]
struct DirectLoopRegion {
    condition: usize,
    back_jump: usize,
    end: usize,
}

fn direct_loop_region_at(
    function: &BytecodeFunc,
    start: usize,
    limit: usize,
) -> Option<DirectLoopRegion> {
    for condition in start..limit {
        let Op::JumpIfFalse(end) = &function.code[condition] else {
            continue;
        };
        if *end <= condition + 1 || *end > limit {
            continue;
        }
        let back_jump = end.checked_sub(1)?;
        if matches!(function.code.get(back_jump), Some(Op::Jump(target)) if *target == start) {
            return Some(DirectLoopRegion {
                condition,
                back_jump,
                end: *end,
            });
        }
    }
    None
}

fn reducible_region(
    function: &BytecodeFunc,
    layout: &FunctionLayout,
    start: usize,
    end: usize,
    loop_targets: Option<(usize, usize)>,
) -> bool {
    let mut instruction = start;
    while instruction < end {
        if layout.heights[instruction].is_none() {
            instruction += 1;
            continue;
        }
        if let Some(region) = direct_loop_region_at(function, instruction, end) {
            if !reducible_region(function, layout, instruction, region.condition, loop_targets)
                || !reducible_region(
                    function,
                    layout,
                    region.condition + 1,
                    region.back_jump,
                    Some((instruction, region.end)),
                )
            {
                return false;
            }
            instruction = region.end;
            continue;
        }
        match &function.code[instruction] {
            Op::JumpIfFalse(_) => {
                let Some(region) = direct_if_region_at(function, instruction, end) else {
                    return false;
                };
                if !reducible_region(
                    function,
                    layout,
                    instruction + 1,
                    region.then_jump,
                    loop_targets,
                ) || !reducible_region(
                    function,
                    layout,
                    region.else_start,
                    region.end,
                    loop_targets,
                ) {
                    return false;
                }
                instruction = region.end;
            }
            Op::Jump(target)
                if loop_targets
                    .is_some_and(|(continue_target, break_target)| {
                        *target == continue_target || *target == break_target
                    }) =>
            {
                instruction += 1;
            }
            Op::Jump(_) => return false,
            _ => instruction += 1,
        }
    }
    true
}

fn has_reducible_control_flow(function: &BytecodeFunc, layout: &FunctionLayout) -> bool {
    function
        .code
        .iter()
        .any(|operation| matches!(operation, Op::Jump(_) | Op::JumpIfFalse(_)))
        && reducible_region(function, layout, 0, function.code.len(), None)
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
            string_adds: HashSet::new(),
            struct_fields: HashMap::new(),
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

    let (string_adds, struct_fields) = infer_value_operations(module, function, &heights)?;
    Ok(FunctionLayout {
        heights,
        stack_base: function.locals as u32,
        stack_slots: maximum,
        pc_local: (function.locals + maximum) as u32,
        string_adds,
        struct_fields,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueKind {
    Numeric,
    String,
    Array,
    Tuple,
    Struct(Vec<String>),
    Enum(String),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeState {
    stack: Vec<ValueKind>,
    locals: Vec<ValueKind>,
}

fn metadata_kind(module: &CompiledModule, ty: Option<&BytecodeType>) -> ValueKind {
    match ty {
        Some(BytecodeType::Int | BytecodeType::Bool) => ValueKind::Numeric,
        Some(BytecodeType::String) => ValueKind::String,
        Some(BytecodeType::Array) => ValueKind::Array,
        Some(BytecodeType::Tuple) => ValueKind::Tuple,
        Some(BytecodeType::Enum(name)) => ValueKind::Enum(name.clone()),
        Some(BytecodeType::Struct(name)) => module
            .struct_schemas
            .get(name)
            .cloned()
            .map_or(ValueKind::Unknown, ValueKind::Struct),
        _ => ValueKind::Unknown,
    }
}

fn infer_value_operations(
    module: &CompiledModule,
    function: &BytecodeFunc,
    heights: &[Option<usize>],
) -> Result<(HashSet<usize>, HashMap<usize, usize>), WasmError> {
    if function.code.is_empty() {
        return Ok((HashSet::new(), HashMap::new()));
    }
    let mut initial_locals = vec![ValueKind::Unknown; function.locals];
    for (local, ty) in initial_locals.iter_mut().zip(&function.param_types) {
        *local = metadata_kind(module, Some(ty));
    }
    let mut states = vec![None; function.code.len()];
    states[0] = Some(TypeState {
        stack: Vec::new(),
        locals: initial_locals,
    });
    let mut pending = VecDeque::from([0]);

    while let Some(instruction) = pending.pop_front() {
        let mut state = states[instruction]
            .clone()
            .expect("queued instructions have a type state");
        apply_type_effect(&function.code[instruction], &mut state, module);
        let mut successors = Vec::with_capacity(2);
        match &function.code[instruction] {
            Op::Jump(target) => successors.push(*target),
            Op::JumpIfFalse(target) => {
                successors.push(*target);
                if instruction + 1 < function.code.len() {
                    successors.push(instruction + 1);
                }
            }
            Op::Ret | Op::Halt => {}
            _ if instruction + 1 < function.code.len() => successors.push(instruction + 1),
            _ => {}
        }
        for successor in successors {
            let changed = merge_type_state(&mut states[successor], &state);
            if changed {
                pending.push_back(successor);
            }
        }
    }

    let mut string_adds = HashSet::new();
    let mut struct_fields = HashMap::new();
    for (instruction, operation) in function.code.iter().enumerate() {
        if heights[instruction].is_none() {
            continue;
        }
        let state = states[instruction]
            .as_ref()
            .expect("reachable instructions have a type state");
        if let Op::EnumIs { name, .. } = operation {
            match &state.stack[state.stack.len() - 1] {
                ValueKind::Enum(actual) if actual == name => {}
                ValueKind::Enum(actual) => return Err(WasmError::Unsupported {
                    function: function.name.clone(),
                    operation: format!("enum test expects '{name}', found '{actual}'"),
                }),
                ValueKind::Unknown => {},
                _ => return Err(WasmError::Unsupported {
                    function: function.name.clone(),
                    operation: format!("value is not enum '{name}' at instruction {instruction}"),
                }),
            }
        }
        if let Op::GetField(field) = operation {
            let ValueKind::Struct(fields) = &state.stack[state.stack.len() - 1] else {
                return Err(WasmError::Unsupported {
                    function: function.name.clone(),
                    operation: format!("unresolved struct field '{field}' at instruction {instruction}"),
                });
            };
            let index = fields.iter().position(|candidate| candidate == field).ok_or_else(|| {
                WasmError::Unsupported {
                    function: function.name.clone(),
                    operation: format!("unknown struct field '{field}' at instruction {instruction}"),
                }
            })?;
            struct_fields.insert(instruction, index);
        }
        if matches!(operation, Op::Index)
            && state.stack[state.stack.len() - 2] == ValueKind::String
        {
            return Err(WasmError::Unsupported {
                function: function.name.clone(),
                operation: format!("string Index at instruction {instruction}"),
            });
        }
        if !matches!(operation, Op::Add) {
            continue;
        }
        let left = &state.stack[state.stack.len() - 2];
        let right = &state.stack[state.stack.len() - 1];
        match (left, right) {
            (ValueKind::String, ValueKind::String) => {
                string_adds.insert(instruction);
            }
            (ValueKind::String, _) | (_, ValueKind::String) => {
                return Err(WasmError::AmbiguousAdd {
                    function: function.name.clone(),
                    instruction,
                });
            }
            _ => {}
        }
    }
    Ok((string_adds, struct_fields))
}

fn apply_type_effect(operation: &Op, state: &mut TypeState, module: &CompiledModule) {
    match operation {
        Op::PushStr(_) => state.stack.push(ValueKind::String),
        Op::PushInt(_) | Op::PushBool(_) | Op::PushChar(_) => {
            state.stack.push(ValueKind::Numeric);
        }
        Op::PushNil => state.stack.push(ValueKind::Unknown),
        Op::PushLocal(local) => state.stack.push(state.locals[*local].clone()),
        Op::StoreLocal(local) => {
            state.locals[*local] = state.stack.pop().expect("stack analysis ran first");
        }
        Op::Pop | Op::JumpIfFalse(_) => {
            let _ = state.stack.pop();
        }
        Op::Dup => {
            let value = state.stack.last().expect("stack analysis ran first").clone();
            state.stack.push(value);
        }
        Op::Add => {
            let right = state.stack.pop().expect("stack analysis ran first");
            let left = state.stack.pop().expect("stack analysis ran first");
            let result = if left == ValueKind::String && right == ValueKind::String {
                ValueKind::String
            } else if left == ValueKind::Numeric && right == ValueKind::Numeric {
                ValueKind::Numeric
            } else {
                ValueKind::Unknown
            };
            state.stack.push(result);
        }
        Op::Sub
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
        | Op::BitXor => {
            let _ = state.stack.pop();
            let _ = state.stack.pop();
            state.stack.push(ValueKind::Numeric);
        }
        Op::Neg | Op::Not | Op::BitNot | Op::Len => {
            let _ = state.stack.pop();
            state.stack.push(ValueKind::Numeric);
        }
        Op::NewArray(count) => {
            state.stack.truncate(state.stack.len() - *count);
            state.stack.push(ValueKind::Array);
        }
        Op::NewTuple(count) => {
            state.stack.truncate(state.stack.len() - *count);
            state.stack.push(ValueKind::Tuple);
        }
        Op::NewStruct { fields, .. } => {
            state.stack.truncate(state.stack.len() - fields.len());
            state.stack.push(ValueKind::Struct(fields.clone()));
        }
        Op::GetField(_) => {
            let _ = state.stack.pop();
            state.stack.push(ValueKind::Unknown);
        }
        Op::NewEnum { name, has_payload, .. } => {
            if *has_payload { let _ = state.stack.pop(); }
            state.stack.push(ValueKind::Enum(name.clone()));
        }
        Op::EnumIs { .. } => {
            let _ = state.stack.pop();
            state.stack.push(ValueKind::Numeric);
        }
        Op::EnumPayload => {
            let _ = state.stack.pop();
            state.stack.push(ValueKind::Unknown);
        }
        Op::Index => {
            state.stack.truncate(state.stack.len() - 2);
            state.stack.push(ValueKind::Unknown);
        }
        Op::Call { function, argc } => {
            state.stack.truncate(state.stack.len() - *argc);
            state.stack.push(metadata_kind(
                module,
                module.functions.get(*function).and_then(|target| target.return_type.as_ref()),
            ));
        }
        Op::CallNative { argc, .. } | Op::Print(argc) => {
            state.stack.truncate(state.stack.len() - *argc);
            state.stack.push(ValueKind::Unknown);
        }
        Op::Jump(_) | Op::Ret | Op::Nop | Op::Halt => {}
        _ => unreachable!("unsupported operations are rejected before type inference"),
    }
}

fn merge_type_state(destination: &mut Option<TypeState>, incoming: &TypeState) -> bool {
    let Some(current) = destination else {
        *destination = Some(incoming.clone());
        return true;
    };
    let mut changed = false;
    for (current, incoming) in current.stack.iter_mut().zip(&incoming.stack) {
        if &*current != incoming && !matches!(current, ValueKind::Unknown) {
            *current = ValueKind::Unknown;
            changed = true;
        }
    }
    for (current, incoming) in current.locals.iter_mut().zip(&incoming.locals) {
        if &*current != incoming && !matches!(current, ValueKind::Unknown) {
            *current = ValueKind::Unknown;
            changed = true;
        }
    }
    changed
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
        Op::PushStr(string) if *string >= module.string_table.len() => {
            Err(WasmError::InvalidString {
                function: function.name.clone(),
                string: *string,
            })
        }
        Op::NewArray(count) | Op::NewTuple(count)
            if *count > (u32::MAX as usize) / 8 =>
        {
            Err(WasmError::Unsupported {
                function: function.name.clone(),
                operation: format!("collection with {count} elements exceeds Wasm memory32"),
            })
        }
        Op::NewStruct { fields, .. } if fields.len() > (u32::MAX as usize) / 8 => {
            Err(WasmError::Unsupported {
                function: function.name.clone(),
                operation: "struct exceeds Wasm memory32".into(),
            })
        }
        Op::Print(argc) if *argc != 1 => Err(WasmError::Unsupported {
            function: function.name.clone(),
            operation: format!("Print({argc}) requires exactly one browser-host argument"),
        }),
        Op::CallNative { name, argc } => {
            let expected = array_native_arity(name)
                .or_else(|| wasm_heap_native_arity(name))
                .or_else(|| web_import(name).map(|import| import.params))
                .ok_or_else(|| WasmError::Unsupported {
                    function: function.name.clone(),
                    operation: format!("CallNative({name})"),
                })?;
            if *argc != expected {
                return Err(WasmError::InvalidArgumentCount {
                    function: function.name.clone(),
                    callee: name.clone(),
                    expected,
                    actual: *argc,
                });
            }
            Ok(())
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
        | Op::PushChar(_)
        | Op::PushNil
        | Op::PushStr(_)
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
        | Op::Print(1)
        | Op::Len
        | Op::NewArray(_)
        | Op::NewTuple(_)
        | Op::Index
        | Op::NewStruct { .. }
        | Op::GetField(_)
        | Op::NewEnum { .. }
        | Op::EnumIs { .. }
        | Op::EnumPayload
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
        Op::PushInt(_)
        | Op::PushBool(_)
        | Op::PushChar(_)
        | Op::PushNil
        | Op::PushStr(_)
        | Op::PushLocal(_) => (0, 1),
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
        Op::Neg | Op::Not | Op::BitNot | Op::Len => (1, 1),
        Op::NewArray(count) | Op::NewTuple(count) => (*count, 1),
        Op::NewStruct { fields, .. } => (fields.len(), 1),
        Op::Index => (2, 1),
        Op::GetField(_) => (1, 1),
        Op::NewEnum { has_payload, .. } => (if *has_payload { 1 } else { 0 }, 1),
        Op::EnumIs { .. } | Op::EnumPayload => (1, 1),
        Op::Call { argc, .. } | Op::CallNative { argc, .. } | Op::Print(argc) => (*argc, 1),
        Op::Jump(_) | Op::Nop => (0, 0),
        Op::Ret | Op::Halt => (0, 0),
        _ => (0, 0),
    }
}

struct EmitContext<'a> {
    function: &'a BytecodeFunc,
    layout: &'a FunctionLayout,
    strings: &'a StringLayout,
    host_imports: &'a HostImports,
    concat_function: Option<u32>,
    allocator_function: Option<u32>,
    managed_scratch: u32,
    pc_local: u32,
}

fn emit_operation(
    context: &EmitContext<'_>,
    instruction: usize,
    height: usize,
    dispatch_depth: Option<u32>,
    body: &mut Function,
) -> Result<(), WasmError> {
    let function = context.function;
    let layout = context.layout;
    let strings = context.strings;
    let function_bias = context.host_imports.definitions.len() as u32;
    let operation = &function.code[instruction];
    match operation {
        Op::PushInt(value) => store_constant(body, layout.stack_base + height as u32, *value),
        Op::PushBool(value) => {
            store_constant(body, layout.stack_base + height as u32, i64::from(*value));
        }
        Op::PushChar(value) => {
            store_constant(
                body,
                layout.stack_base + height as u32,
                i64::from(u32::from(*value)),
            );
        }
        Op::PushNil => store_constant(body, layout.stack_base + height as u32, 0),
        Op::PushStr(string) => {
            store_constant(
                body,
                layout.stack_base + height as u32,
                strings.handles[*string],
            );
        }
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
        Op::Add if layout.string_adds.contains(&instruction) => {
            let output = layout.stack_base + (height - 2) as u32;
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::LocalGet(output + 1));
            body.instruction(&Instruction::Call(
                context
                    .concat_function
                    .expect("string additions require the concat helper"),
            ));
            body.instruction(&Instruction::LocalSet(output));
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
            body.instruction(&Instruction::Call(*callee as u32 + function_bias));
            body.instruction(&Instruction::LocalSet(layout.stack_base + first as u32));
        }
        Op::CallNative { name, .. } if wasm_heap_native_arity(name).is_some() => {
            emit_wasm_heap_native(name, context, height, body);
        }
        Op::CallNative { name, .. } if name == "std::array::set" => {
            emit_array_set(context, height, body);
        }
        Op::CallNative { name, .. } if name == "std::array::push" => {
            emit_array_push(context, height, body);
        }
        Op::CallNative { name, .. } if name == "std::array::pop" => {
            emit_array_pop(context, height, body);
        }
        Op::CallNative { name, .. } if name == "std::array::slice" => {
            emit_array_slice(context, height, body);
        }
        Op::CallNative { name, .. } if name == "std::array::concat" => {
            emit_array_concat(context, height, body);
        }
        Op::CallNative { name, argc } => {
            let first = height - *argc;
            for argument in first..height {
                body.instruction(&Instruction::LocalGet(
                    layout.stack_base + argument as u32,
                ));
            }
            let import_index = context.host_imports.natives[name.as_str()];
            body.instruction(&Instruction::Call(import_index));
            let import = web_import(name).expect("browser natives are validated before emission");
            if import.returns_value {
                body.instruction(&Instruction::LocalSet(layout.stack_base + first as u32));
            } else {
                store_constant(body, layout.stack_base + first as u32, 0);
            }
        }
        Op::Print(_) => {
            body.instruction(&Instruction::LocalGet(
                layout.stack_base + (height - 1) as u32,
            ));
            body.instruction(&Instruction::Call(
                context
                    .host_imports
                    .print
                    .expect("print operations require the print import"),
            ));
            store_constant(body, layout.stack_base + (height - 1) as u32, 0);
        }
        Op::Jump(target) => {
            let dispatch_depth = dispatch_depth.expect("jumps require dispatcher lowering");
            set_pc(body, context.pc_local, *target);
            body.instruction(&Instruction::Br(dispatch_depth));
            return Ok(());
        }
        Op::JumpIfFalse(target) => {
            let dispatch_depth = dispatch_depth.expect("conditional jumps require dispatcher lowering");
            body.instruction(&Instruction::LocalGet(
                layout.stack_base + (height - 1) as u32,
            ));
            body.instruction(&Instruction::I64Eqz);
            body.instruction(&Instruction::If(BlockType::Empty));
            set_pc(body, context.pc_local, *target);
            body.instruction(&Instruction::Else);
            set_pc(body, context.pc_local, instruction + 1);
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::Br(dispatch_depth));
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
        Op::NewArray(_) | Op::NewTuple(_) | Op::NewStruct { .. } => {
            let count = match operation {
                Op::NewArray(count) | Op::NewTuple(count) => *count,
                Op::NewStruct { fields, .. } => fields.len(),
                _ => unreachable!(),
            };
            let first = height - count;
            let count_u32 = count as u32;
            let byte_length = count_u32 * 8;
            body.instruction(&Instruction::I32Const(byte_length as i32));
            body.instruction(&Instruction::I32Const(count_u32 as i32));
            body.instruction(&Instruction::Call(
                context
                    .allocator_function
                    .expect("collections require managed allocator"),
            ));
            body.instruction(&Instruction::LocalSet(context.managed_scratch));
            let element_memory = MemArg {
                offset: 0,
                align: 3,
                memory_index: 0,
            };
            for element in 0..count {
                body.instruction(&Instruction::LocalGet(context.managed_scratch));
                body.instruction(&Instruction::I32WrapI64);
                body.instruction(&Instruction::I32Const((element as u32 * 8) as i32));
                body.instruction(&Instruction::I32Add);
                body.instruction(&Instruction::LocalGet(
                    layout.stack_base + (first + element) as u32,
                ));
                body.instruction(&Instruction::I64Store(element_memory));
            }
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::LocalSet(layout.stack_base + first as u32));
        }
        Op::NewEnum { name, variant, has_payload } => {
            let output_slot = height - if *has_payload { 1 } else { 0 };
            let output = layout.stack_base + output_slot as u32;
            body.instruction(&Instruction::I32Const(16));
            body.instruction(&Instruction::I32Const(2));
            body.instruction(&Instruction::Call(context.allocator_function.expect("enums require allocator")));
            body.instruction(&Instruction::LocalSet(context.managed_scratch));
            for (slot, value_local) in [(0u32, None), (1u32, if *has_payload { Some(output) } else { None })] {
                body.instruction(&Instruction::LocalGet(context.managed_scratch));
                body.instruction(&Instruction::I32WrapI64);
                if slot != 0 { body.instruction(&Instruction::I32Const(8)); body.instruction(&Instruction::I32Add); }
                if slot == 0 { body.instruction(&Instruction::I64Const(enum_tag(name, variant) as i64)); }
                else if let Some(local) = value_local { body.instruction(&Instruction::LocalGet(local)); }
                else { body.instruction(&Instruction::I64Const(0)); }
                body.instruction(&Instruction::I64Store(MemArg { offset: 0, align: 3, memory_index: 0 }));
            }
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::LocalSet(output));
        }
        Op::EnumIs { name, variant } => {
            let output = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::LocalGet(output)); body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I64Load(MemArg { offset: 0, align: 3, memory_index: 0 }));
            body.instruction(&Instruction::I64Const(enum_tag(name, variant) as i64)); body.instruction(&Instruction::I64Eq);
            body.instruction(&Instruction::I64ExtendI32U); body.instruction(&Instruction::LocalSet(output));
        }
        Op::EnumPayload => {
            let output = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::LocalGet(output)); body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const(8)); body.instruction(&Instruction::I32Add);
            body.instruction(&Instruction::I64Load(MemArg { offset: 0, align: 3, memory_index: 0 }));
            body.instruction(&Instruction::LocalSet(output));
        }
        Op::GetField(_) => {
            let output = layout.stack_base + (height - 1) as u32;
            let field = layout.struct_fields[&instruction] as u32;
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const((field * 8) as i32));
            body.instruction(&Instruction::I32Add);
            body.instruction(&Instruction::I64Load(MemArg {
                offset: 0,
                align: 3,
                memory_index: 0,
            }));
            body.instruction(&Instruction::LocalSet(output));
        }
        Op::Index => {
            let output = layout.stack_base + (height - 2) as u32;
            let index = output + 1;
            body.instruction(&Instruction::LocalGet(index));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::I64LtS);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::Unreachable);
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::LocalGet(index));
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const(4));
            body.instruction(&Instruction::I32Sub);
            body.instruction(&Instruction::I32Load(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64GeU);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::Unreachable);
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::LocalGet(index));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const(3));
            body.instruction(&Instruction::I32Shl);
            body.instruction(&Instruction::I32Add);
            body.instruction(&Instruction::I64Load(MemArg {
                offset: 0,
                align: 3,
                memory_index: 0,
            }));
            body.instruction(&Instruction::LocalSet(output));
        }
        Op::Len => {
            let slot = layout.stack_base + (height - 1) as u32;
            body.instruction(&Instruction::LocalGet(slot));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const(4));
            body.instruction(&Instruction::I32Sub);
            body.instruction(&Instruction::I32Load(MemArg {
                offset: 0,
                align: 2,
                memory_index: 0,
            }));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::LocalSet(slot));
        }
        Op::Nop => {}
        _ => unreachable!("operations are checked during stack analysis"),
    }

    if let Some(dispatch_depth) = dispatch_depth {
        set_pc(body, context.pc_local, instruction + 1);
        body.instruction(&Instruction::Br(dispatch_depth));
    }
    Ok(())
}

fn emit_wasm_heap_native(
    name: &str,
    context: &EmitContext<'_>,
    height: usize,
    body: &mut Function,
) {
    let output_slot = if matches!(
        name,
        "std::wasm::heap_set_limit" | "std::wasm::heap_restore" | "std::wasm::heap_scope_end"
    ) {
        height - 1
    } else {
        height
    };
    let output = context.layout.stack_base + output_slot as u32;
    match name {
        "std::wasm::heap_used" | "std::wasm::heap_checkpoint" => {
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::I64ExtendI32U);
        }
        "std::wasm::heap_capacity" => {
            body.instruction(&Instruction::MemorySize(0));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64Const(65_536));
            body.instruction(&Instruction::I64Mul);
        }
        "std::wasm::heap_limit" => {
            body.instruction(&Instruction::GlobalGet(1));
            body.instruction(&Instruction::I64ExtendI32U);
        }
        "std::wasm::heap_allocations" => {
            body.instruction(&Instruction::GlobalGet(3));
        }
        "std::wasm::heap_allocated_bytes" => {
            body.instruction(&Instruction::GlobalGet(4));
        }
        "std::wasm::heap_restores" => {
            body.instruction(&Instruction::GlobalGet(5));
        }
        "std::wasm::heap_reclaimed_bytes" => {
            body.instruction(&Instruction::GlobalGet(6));
        }
        "std::wasm::heap_peak_used" => {
            body.instruction(&Instruction::GlobalGet(7));
        }
        "std::wasm::heap_reset_counters" => {
            for counter in 3..=6 {
                body.instruction(&Instruction::I64Const(0));
                body.instruction(&Instruction::GlobalSet(counter));
            }
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::GlobalSet(7));
            body.instruction(&Instruction::I64Const(1));
        }
        "std::wasm::heap_scope_begin" => {
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::I32Const(8));
            body.instruction(&Instruction::I32Const(2));
            body.instruction(&Instruction::Call(
                context.allocator_function.expect("heap scope requires allocator"),
            ));
            body.instruction(&Instruction::LocalSet(context.managed_scratch));
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::GlobalGet(8));
            body.instruction(&Instruction::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Store(MemArg { offset: 4, align: 2, memory_index: 0 }));
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::GlobalSet(8));
            return;
        }
        "std::wasm::heap_set_limit" => {
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::I64LtS);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::Else);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I64Const(i64::from(u32::MAX)));
            body.instruction(&Instruction::I64GtU);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64LtU);
            body.instruction(&Instruction::I32Or);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::Else);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::GlobalSet(1));
            body.instruction(&Instruction::I64Const(1));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::End);
            return;
        }
        "std::wasm::heap_scope_end" => {
            body.instruction(&Instruction::GlobalGet(8));
            body.instruction(&Instruction::I32Eqz);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::Else);
            body.instruction(&Instruction::GlobalGet(8));
            body.instruction(&Instruction::I32Load(MemArg { offset: 4, align: 2, memory_index: 0 }));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I64Ne);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::Else);
            body.instruction(&Instruction::GlobalGet(8));
            body.instruction(&Instruction::I32Load(MemArg { offset: 0, align: 2, memory_index: 0 }));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::LocalSet(context.managed_scratch));
            emit_heap_rewind(body, output);
            body.instruction(&Instruction::LocalGet(context.managed_scratch));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::GlobalSet(8));
            body.instruction(&Instruction::I64Const(1));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::End);
            body.instruction(&Instruction::End);
            return;
        }
        "std::wasm::heap_restore" => {
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::I64LtS);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64GtU);
            body.instruction(&Instruction::I32Or);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::GlobalGet(2));
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64LtU);
            body.instruction(&Instruction::I32Or);
            body.instruction(&Instruction::If(BlockType::Empty));
            body.instruction(&Instruction::I64Const(0));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::Else);
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Const(0));
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Sub);
            body.instruction(&Instruction::MemoryFill(0));
            body.instruction(&Instruction::GlobalGet(5));
            body.instruction(&Instruction::I64Const(1));
            body.instruction(&Instruction::I64Add);
            body.instruction(&Instruction::GlobalSet(5));
            body.instruction(&Instruction::GlobalGet(6));
            body.instruction(&Instruction::GlobalGet(0));
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::I32Sub);
            body.instruction(&Instruction::I64ExtendI32U);
            body.instruction(&Instruction::I64Add);
            body.instruction(&Instruction::GlobalSet(6));
            body.instruction(&Instruction::LocalGet(output));
            body.instruction(&Instruction::I32WrapI64);
            body.instruction(&Instruction::GlobalSet(0));
            body.instruction(&Instruction::I32Const(0));
            body.instruction(&Instruction::GlobalSet(8));
            body.instruction(&Instruction::I64Const(1));
            body.instruction(&Instruction::LocalSet(output));
            body.instruction(&Instruction::End);
            return;
        }
        _ => unreachable!("heap native validated before emission"),
    }
    body.instruction(&Instruction::LocalSet(output));
}

fn emit_heap_rewind(body: &mut Function, checkpoint_local: u32) {
    body.instruction(&Instruction::LocalGet(checkpoint_local));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalGet(checkpoint_local));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::MemoryFill(0));
    body.instruction(&Instruction::GlobalGet(5));
    body.instruction(&Instruction::I64Const(1));
    body.instruction(&Instruction::I64Add);
    body.instruction(&Instruction::GlobalSet(5));
    body.instruction(&Instruction::GlobalGet(6));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalGet(checkpoint_local));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Add);
    body.instruction(&Instruction::GlobalSet(6));
    body.instruction(&Instruction::LocalGet(checkpoint_local));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::GlobalSet(0));
}

fn emit_array_set(context: &EmitContext<'_>, height: usize, body: &mut Function) {
    let output = context.layout.stack_base + (height - 3) as u32;
    let index = output + 1;
    let value = output + 2;
    let count_memory = MemArg { offset: 0, align: 2, memory_index: 0 };
    let element_memory = MemArg { offset: 0, align: 3, memory_index: 0 };

    body.instruction(&Instruction::LocalGet(index));
    body.instruction(&Instruction::I64Const(0));
    body.instruction(&Instruction::I64LtS);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(index));
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64GeU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::Call(
        context.allocator_function.expect("array set requires allocator"),
    ));
    body.instruction(&Instruction::LocalSet(context.managed_scratch));

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(index));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(value));
    body.instruction(&Instruction::I64Store(element_memory));
    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::LocalSet(output));
}

fn emit_array_push(context: &EmitContext<'_>, height: usize, body: &mut Function) {
    const MAX_ELEMENTS: i32 = (u32::MAX / 8) as i32;
    let output = context.layout.stack_base + (height - 2) as u32;
    let value = output + 1;
    let count_memory = MemArg { offset: 0, align: 2, memory_index: 0 };
    let element_memory = MemArg { offset: 0, align: 3, memory_index: 0 };

    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(MAX_ELEMENTS));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::Call(
        context.allocator_function.expect("array push requires allocator"),
    ));
    body.instruction(&Instruction::LocalSet(context.managed_scratch));

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(value));
    body.instruction(&Instruction::I64Store(element_memory));
    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::LocalSet(output));
}

fn emit_array_pop(context: &EmitContext<'_>, height: usize, body: &mut Function) {
    let output = context.layout.stack_base + (height - 1) as u32;
    let count_memory = MemArg { offset: 0, align: 2, memory_index: 0 };

    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(BlockType::Empty));
    // Empty arrays are already the correct persistent result.
    body.instruction(&Instruction::Else);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::Call(
        context.allocator_function.expect("array pop requires allocator"),
    ));
    body.instruction(&Instruction::LocalSet(context.managed_scratch));

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });
    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::LocalSet(output));
    body.instruction(&Instruction::End);
}

fn emit_array_slice(context: &EmitContext<'_>, height: usize, body: &mut Function) {
    let output = context.layout.stack_base + (height - 3) as u32;
    let start = output + 1;
    let end = output + 2;
    let count_memory = MemArg { offset: 0, align: 2, memory_index: 0 };

    for index in [start, end] {
        body.instruction(&Instruction::LocalGet(index));
        body.instruction(&Instruction::I64Const(0));
        body.instruction(&Instruction::I64LtS);
        body.instruction(&Instruction::If(BlockType::Empty));
        body.instruction(&Instruction::Unreachable);
        body.instruction(&Instruction::End);
    }
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::I64GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(end));
    emit_collection_count(body, output, count_memory);
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::I64Sub);
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::I64Sub);
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::Call(
        context.allocator_function.expect("array slice requires allocator"),
    ));
    body.instruction(&Instruction::LocalSet(context.managed_scratch));

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(output));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::I64Sub);
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });
    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::LocalSet(output));
}

fn emit_array_concat(context: &EmitContext<'_>, height: usize, body: &mut Function) {
    const MAX_ELEMENTS: i32 = (u32::MAX / 8) as i32;
    let left = context.layout.stack_base + (height - 2) as u32;
    let right = left + 1;
    let count_memory = MemArg { offset: 0, align: 2, memory_index: 0 };

    emit_collection_count(body, right, count_memory);
    body.instruction(&Instruction::I32Const(MAX_ELEMENTS));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    emit_collection_count(body, left, count_memory);
    body.instruction(&Instruction::I32Const(MAX_ELEMENTS));
    emit_collection_count(body, right, count_memory);
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    emit_collection_count(body, left, count_memory);
    emit_collection_count(body, right, count_memory);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    emit_collection_count(body, left, count_memory);
    emit_collection_count(body, right, count_memory);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::Call(
        context.allocator_function.expect("array concat requires allocator"),
    ));
    body.instruction(&Instruction::LocalSet(context.managed_scratch));

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalGet(left));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, left, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });

    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, left, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(right));
    body.instruction(&Instruction::I32WrapI64);
    emit_collection_count(body, right, count_memory);
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Shl);
    body.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });
    body.instruction(&Instruction::LocalGet(context.managed_scratch));
    body.instruction(&Instruction::LocalSet(left));
}

fn emit_collection_count(body: &mut Function, handle_local: u32, memory: MemArg) {
    body.instruction(&Instruction::LocalGet(handle_local));
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Load(memory));
}

fn emit_allocation_counters(body: &mut Function, previous_heap: u32) {
    body.instruction(&Instruction::GlobalGet(3));
    body.instruction(&Instruction::I64Const(1));
    body.instruction(&Instruction::I64Add);
    body.instruction(&Instruction::GlobalSet(3));
    body.instruction(&Instruction::GlobalGet(4));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalGet(previous_heap));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Add);
    body.instruction(&Instruction::GlobalSet(4));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::GlobalGet(7));
    body.instruction(&Instruction::I64GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::GlobalSet(7));
    body.instruction(&Instruction::End);
}

fn compile_string_concat() -> Function {
    const LEFT_POINTER: u32 = 2;
    const RIGHT_POINTER: u32 = 3;
    const LEFT_LENGTH: u32 = 4;
    const RIGHT_LENGTH: u32 = 5;
    const OUTPUT_DESCRIPTOR: u32 = 6;
    const OUTPUT_POINTER: u32 = 7;
    const TOTAL_LENGTH: u32 = 8;
    const NEW_END: u32 = 9;
    const MEMORY_PAGES: u32 = 10;
    const REQUIRED_PAGES: u32 = 11;

    let mut body = Function::new([(10, ValType::I32)]);

    for (parameter, pointer, length) in [
        (0, LEFT_POINTER, LEFT_LENGTH),
        (1, RIGHT_POINTER, RIGHT_LENGTH),
    ] {
        body.instruction(&Instruction::LocalGet(parameter));
        body.instruction(&Instruction::I32WrapI64);
        body.instruction(&Instruction::LocalSet(pointer));
        body.instruction(&Instruction::LocalGet(parameter));
        body.instruction(&Instruction::I64Const(32));
        body.instruction(&Instruction::I64ShrU);
        body.instruction(&Instruction::I32WrapI64);
        body.instruction(&Instruction::LocalSet(length));
    }

    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalSet(OUTPUT_DESCRIPTOR));
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalSet(OUTPUT_POINTER));

    body.instruction(&Instruction::LocalGet(LEFT_LENGTH));
    body.instruction(&Instruction::LocalGet(RIGHT_LENGTH));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(TOTAL_LENGTH));
    body.instruction(&Instruction::LocalGet(LEFT_LENGTH));
    body.instruction(&Instruction::I32LtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::LocalGet(TOTAL_LENGTH));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(NEW_END));
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::I32LtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::GlobalGet(1));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    body.instruction(&Instruction::MemorySize(0));
    body.instruction(&Instruction::LocalSet(MEMORY_PAGES));
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(16));
    body.instruction(&Instruction::I32ShrU);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(65_535));
    body.instruction(&Instruction::I32And);
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(REQUIRED_PAGES));
    body.instruction(&Instruction::LocalGet(MEMORY_PAGES));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::LocalGet(REQUIRED_PAGES));
    body.instruction(&Instruction::LocalGet(MEMORY_PAGES));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::MemoryGrow(0));
    body.instruction(&Instruction::I32Const(-1));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);

    let scalar_count = MemArg {
        offset: 0,
        align: 2,
        memory_index: 0,
    };
    body.instruction(&Instruction::LocalGet(OUTPUT_DESCRIPTOR));
    body.instruction(&Instruction::LocalGet(LEFT_POINTER));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Load(scalar_count));
    body.instruction(&Instruction::LocalGet(RIGHT_POINTER));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Load(scalar_count));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Store(scalar_count));

    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::LocalGet(LEFT_POINTER));
    body.instruction(&Instruction::LocalGet(LEFT_LENGTH));
    body.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::LocalGet(LEFT_LENGTH));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(RIGHT_POINTER));
    body.instruction(&Instruction::LocalGet(RIGHT_LENGTH));
    body.instruction(&Instruction::MemoryCopy {
        src_mem: 0,
        dst_mem: 0,
    });

    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(-4));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(-4));
    body.instruction(&Instruction::I32And);
    body.instruction(&Instruction::GlobalSet(0));
    emit_allocation_counters(&mut body, OUTPUT_DESCRIPTOR);

    body.instruction(&Instruction::LocalGet(TOTAL_LENGTH));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Const(32));
    body.instruction(&Instruction::I64Shl);
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Or);
    body.instruction(&Instruction::End);
    body
}

fn compile_host_string_allocator() -> Function {
    const OUTPUT_DESCRIPTOR: u32 = 2;
    const OUTPUT_POINTER: u32 = 3;
    const NEW_END: u32 = 4;
    const MEMORY_PAGES: u32 = 5;
    const REQUIRED_PAGES: u32 = 6;

    let mut body = Function::new([(5, ValType::I32)]);
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalTee(OUTPUT_DESCRIPTOR));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(OUTPUT_POINTER));
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(NEW_END));
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::I32LtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::GlobalGet(1));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);

    body.instruction(&Instruction::MemorySize(0));
    body.instruction(&Instruction::LocalSet(MEMORY_PAGES));
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(16));
    body.instruction(&Instruction::I32ShrU);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(65_535));
    body.instruction(&Instruction::I32And);
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(REQUIRED_PAGES));
    body.instruction(&Instruction::LocalGet(MEMORY_PAGES));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::LocalGet(REQUIRED_PAGES));
    body.instruction(&Instruction::LocalGet(MEMORY_PAGES));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::MemoryGrow(0));
    body.instruction(&Instruction::I32Const(-1));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);

    body.instruction(&Instruction::LocalGet(OUTPUT_DESCRIPTOR));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        memory_index: 0,
    }));

    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(-4));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(NEW_END));
    body.instruction(&Instruction::I32Const(3));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(-4));
    body.instruction(&Instruction::I32And);
    body.instruction(&Instruction::GlobalSet(0));
    emit_allocation_counters(&mut body, OUTPUT_DESCRIPTOR);

    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Const(32));
    body.instruction(&Instruction::I64Shl);
    body.instruction(&Instruction::LocalGet(OUTPUT_POINTER));
    body.instruction(&Instruction::I64ExtendI32U);
    body.instruction(&Instruction::I64Or);
    body.instruction(&Instruction::End);
    body
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
                param_types: Vec::new(),
                return_type: None,
                captures: 0,
                locals,
                max_stack: 16,
                debug_locations: vec![None; code.len()],
                code,
            }],
            entry: 0,
            string_table: Vec::new(),
            struct_schemas: HashMap::new(),
            enum_schemas: HashMap::new(),
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
    fn embeds_and_returns_versioned_source_metadata() {
        let mut module = module_with(vec![Op::PushInt(42), Op::Ret], 0);
        module.functions[0].debug_locations[0] = Some(titan_codegen::SourceLocation {
            start: 12,
            end: 14,
            line: 3,
            column: 5,
        });
        let artifact = compile_artifact(&module).unwrap();
        assert_eq!(artifact.source_map.format, "titan-wasm-source-map");
        assert_eq!(artifact.source_map.version, 1);
        assert_eq!(artifact.source_map.functions[0].wasm_function_index, 0);
        assert_eq!(artifact.source_map.functions[0].instructions[0].line, 3);
        assert!(artifact.source_map.functions[0].instructions[0]
            .wasm_offset
            .is_some_and(|offset| offset > 8));
        assert_eq!(artifact.standard_source_map.version, 3);
        assert_eq!(
            artifact.standard_source_map.sources,
            vec!["main.titan".to_string()]
        );
        assert!(!artifact.standard_source_map.mappings.is_empty());

        let embedded = wasmparser::Parser::new(0)
            .parse_all(&artifact.wasm)
            .filter_map(Result::ok)
            .find_map(|payload| match payload {
                wasmparser::Payload::CustomSection(section)
                    if section.name() == "titan.source_map" =>
                {
                    Some(section.data().to_vec())
                }
                _ => None,
            })
            .expect("source map custom section");
        let json: serde_json::Value = serde_json::from_slice(&embedded).unwrap();
        assert_eq!(
            json["functions"][0]["instructions"][0]["column"].as_u64(),
            Some(5)
        );

        let mut linked = artifact.wasm.clone();
        append_source_mapping_url(&mut linked, "program.wasm.map").unwrap();
        wasmparser::Validator::new().validate_all(&linked).unwrap();
        let url = wasmparser::Parser::new(0)
            .parse_all(&linked)
            .filter_map(Result::ok)
            .find_map(|payload| match payload {
                wasmparser::Payload::CustomSection(section)
                    if section.name() == "sourceMappingURL" =>
                {
                    Some(section.data().to_vec())
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(url, b"program.wasm.map");

        let mut imported = module_with(vec![Op::PushStr(0), Op::Print(1), Op::Ret], 0);
        imported.string_table.push("mapped".into());
        let imported_artifact = compile_artifact(&imported).unwrap();
        assert_eq!(imported_artifact.source_map.imported_function_count, 1);
        assert_eq!(imported_artifact.source_map.functions[0].wasm_function_index, 1);
    }

    #[test]
    fn encodes_source_map_base64_vlq_segments() {
        let mut encoded = String::new();
        encode_vlq(0, &mut encoded);
        encode_vlq(1, &mut encoded);
        encode_vlq(-1, &mut encoded);
        encode_vlq(16, &mut encoded);
        assert_eq!(encoded, "ACDgB");
    }

    #[test]
    fn lowers_straight_line_functions_without_a_dispatcher() {
        let module = module_with(
            vec![Op::PushInt(40), Op::PushInt(2), Op::Add, Op::Ret],
            0,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                has_br_table |= body
                    .get_operators_reader()
                    .unwrap()
                    .into_iter()
                    .filter_map(Result::ok)
                    .any(|operator| matches!(operator, wasmparser::Operator::BrTable { .. }));
            }
        }
        assert!(!has_br_table);
    }

    #[test]
    fn uses_constant_time_br_table_dispatch() {
        let module = module_with(
            vec![Op::PushInt(1), Op::Jump(3), Op::PushInt(99), Op::Ret],
            0,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                let reader = body.get_operators_reader().unwrap();
                if reader
                    .into_iter()
                    .filter_map(Result::ok)
                    .any(|operator| matches!(operator, wasmparser::Operator::BrTable { .. }))
                {
                    has_br_table = true;
                    break;
                }
            }
        }
        assert!(has_br_table);
    }

    #[test]
    fn lowers_canonical_if_else_without_dispatcher() {
        let module = module_with(
            vec![
                Op::PushBool(false),
                Op::JumpIfFalse(4),
                Op::PushInt(10),
                Op::Jump(5),
                Op::PushInt(20),
                Op::Ret,
            ],
            0,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_if = false;
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                for operator in body.get_operators_reader().unwrap() {
                    let operator = operator.unwrap();
                    has_if |= matches!(&operator, wasmparser::Operator::If { .. });
                    has_br_table |= matches!(&operator, wasmparser::Operator::BrTable { .. });
                }
            }
        }
        assert!(has_if);
        assert!(!has_br_table);
    }

    #[test]
    fn recursively_lowers_nested_if_regions() {
        let module = module_with(
            vec![
                Op::PushBool(true),
                Op::JumpIfFalse(8),
                Op::PushBool(false),
                Op::JumpIfFalse(6),
                Op::PushInt(1),
                Op::Jump(7),
                Op::PushInt(2),
                Op::Jump(9),
                Op::PushInt(3),
                Op::Ret,
            ],
            0,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut if_count = 0usize;
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                for operator in body.get_operators_reader().unwrap() {
                    let operator = operator.unwrap();
                    if matches!(&operator, wasmparser::Operator::If { .. }) {
                        if_count += 1;
                    }
                    has_br_table |= matches!(&operator, wasmparser::Operator::BrTable { .. });
                }
            }
        }
        assert_eq!(if_count, 2);
        assert!(!has_br_table);
    }

    #[test]
    fn directly_lowers_canonical_while_loop() {
        let module = module_with(
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
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_loop = false;
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                for operator in body.get_operators_reader().unwrap() {
                    let operator = operator.unwrap();
                    has_loop |= matches!(&operator, wasmparser::Operator::Loop { .. });
                    has_br_table |= matches!(&operator, wasmparser::Operator::BrTable { .. });
                }
            }
        }
        assert!(has_loop);
        assert!(!has_br_table);
    }

    #[test]
    fn directly_lowers_nested_if_inside_while_loop() {
        let module = module_with(
            vec![
                Op::PushInt(0), Op::StoreLocal(0),
                Op::PushLocal(0), Op::PushInt(3), Op::Lt, Op::JumpIfFalse(19),
                Op::PushLocal(0), Op::PushInt(1), Op::Eq, Op::JumpIfFalse(12),
                Op::PushInt(100), Op::Jump(13), Op::PushInt(200), Op::Pop,
                Op::PushLocal(0), Op::PushInt(1), Op::Add, Op::StoreLocal(0), Op::Jump(2),
                Op::PushLocal(0), Op::Ret,
            ],
            1,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                for operator in body.get_operators_reader().unwrap() {
                    has_br_table |=
                        matches!(operator.unwrap(), wasmparser::Operator::BrTable { .. });
                }
            }
        }
        assert!(!has_br_table);
    }

    #[test]
    fn directly_lowers_conditional_break_in_while_loop() {
        let module = module_with(
            vec![
                Op::PushInt(0), Op::StoreLocal(0),
                Op::PushLocal(0), Op::PushInt(10), Op::Lt, Op::JumpIfFalse(19),
                Op::PushLocal(0), Op::PushInt(3), Op::Eq, Op::JumpIfFalse(12),
                Op::Jump(19), Op::Jump(13), Op::PushNil, Op::Pop,
                Op::PushLocal(0), Op::PushInt(1), Op::Add, Op::StoreLocal(0), Op::Jump(2),
                Op::PushNil, Op::Pop, Op::PushLocal(0), Op::Ret,
            ],
            1,
        );
        let wasm = compile(&module).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let mut has_br_table = false;
        for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
            if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
                for operator in body.get_operators_reader().unwrap() {
                    has_br_table |=
                        matches!(operator.unwrap(), wasmparser::Operator::BrTable { .. });
                }
            }
        }
        assert!(!has_br_table);
    }

    #[test]
    fn emits_utf8_strings_in_exported_linear_memory() {
        let text = "¡Hola, TITAN!";
        let mut module = module_with(vec![Op::PushStr(0), Op::Len, Op::Ret], 0);
        module.string_table.push(text.into());
        validate(&module);
        let wasm = compile(&module).unwrap();
        assert!(wasm
            .windows(text.len())
            .any(|bytes| bytes == text.as_bytes()));

        let layout = StringLayout::new(&module.string_table).unwrap();
        let handle = u64::from_ne_bytes(layout.handles[0].to_ne_bytes());
        assert_eq!(handle as u32, StringLayout::DATA_START + 4);
        assert_eq!((handle >> 32) as usize, text.len());
        assert_eq!(&layout.data[..4], &(text.chars().count() as u32).to_le_bytes());
    }

    #[test]
    fn emits_browser_host_print_import() {
        let mut module = module_with(vec![Op::PushStr(0), Op::Print(1), Op::Ret], 0);
        module.string_table.push("Hello from Titan".into());
        validate(&module);
    }

    #[test]
    fn rejects_multi_argument_print_until_typed_host_abi_is_available() {
        let module = module_with(
            vec![Op::PushInt(1), Op::PushInt(2), Op::Print(2), Op::Ret],
            0,
        );
        assert!(matches!(compile(&module), Err(WasmError::Unsupported { .. })));
    }

    #[test]
    fn emits_deduplicated_dom_host_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushStr(1),
                Op::CallNative {
                    name: "std::web::set_text".into(),
                    argc: 2,
                },
                Op::Pop,
                Op::PushStr(0),
                Op::CallNative {
                    name: "std::web::query_exists".into(),
                    argc: 1,
                },
                Op::Ret,
            ],
            0,
        );
        module.string_table = vec!["#status".into(), "Ready".into()];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 2);
        assert_eq!(imports.natives["std::web::query_exists"], 0);
        assert_eq!(imports.natives["std::web::set_text"], 1);
    }

    #[test]
    fn emits_event_listener_lifecycle_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushStr(1),
                Op::PushStr(2),
                Op::CallNative {
                    name: "std::web::listen".into(),
                    argc: 3,
                },
                Op::StoreLocal(0),
                Op::PushLocal(0),
                Op::CallNative {
                    name: "std::web::unlisten".into(),
                    argc: 1,
                },
                Op::Ret,
            ],
            1,
        );
        module.string_table = vec!["#action".into(), "click".into(), "on_click".into()];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 2);
        assert_eq!(imports.natives["std::web::listen"], 0);
        assert_eq!(imports.natives["std::web::unlisten"], 1);
    }

    #[test]
    fn emits_host_to_wasm_event_string_allocator() {
        let module = module_with(
            vec![
                Op::CallNative {
                    name: "std::web::event_value".into(),
                    argc: 0,
                },
                Op::Len,
                Op::Ret,
            ],
            0,
        );
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 1);
        assert_eq!(imports.natives["std::web::event_value"], 0);
    }

    #[test]
    fn composes_host_allocator_with_dynamic_concat_helper() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushStr(1),
                Op::Add,
                Op::StoreLocal(0),
                Op::CallNative {
                    name: "std::web::event_value".into(),
                    argc: 0,
                },
                Op::Pop,
                Op::PushLocal(0),
                Op::Ret,
            ],
            1,
        );
        module.string_table = vec!["host ".into(), "interop".into()];
        validate(&module);
    }

    #[test]
    fn emits_bounded_async_fetch_callback_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushInt(65_536),
                Op::PushInt(5_000),
                Op::PushStr(1),
                Op::CallNative {
                    name: "std::web::fetch".into(),
                    argc: 4,
                },
                Op::Pop,
                Op::CallNative {
                    name: "std::web::fetch_body".into(),
                    argc: 0,
                },
                Op::Ret,
            ],
            0,
        );
        module.string_table = vec!["./data.json".into(), "on_fetch".into()];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 2);
        assert_eq!(imports.natives["std::web::fetch"], 0);
        assert_eq!(imports.natives["std::web::fetch_body"], 1);
    }

    #[test]
    fn emits_configurable_http_request_and_headers_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushStr(1),
                Op::PushStr(2),
                Op::PushStr(3),
                Op::PushInt(32_768),
                Op::PushInt(3_000),
                Op::PushStr(4),
                Op::CallNative {
                    name: "std::web::request".into(),
                    argc: 7,
                },
                Op::Pop,
                Op::CallNative {
                    name: "std::web::fetch_headers".into(),
                    argc: 0,
                },
                Op::Ret,
            ],
            0,
        );
        module.string_table = vec![
            "POST".into(),
            "/api".into(),
            "{\"Content-Type\":\"application/json\"}".into(),
            "{\"ok\":true}".into(),
            "on_response".into(),
        ];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 2);
        assert_eq!(imports.natives["std::web::fetch_headers"], 0);
        assert_eq!(imports.natives["std::web::request"], 1);
    }

    #[test]
    fn emits_asynchronous_browser_websocket_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::PushStr(1),
                Op::PushInt(65_536),
                Op::PushStr(2),
                Op::PushStr(3),
                Op::PushStr(4),
                Op::PushStr(5),
                Op::CallNative {
                    name: "std::web::ws_connect".into(),
                    argc: 7,
                },
                Op::StoreLocal(0),
                Op::PushLocal(0),
                Op::PushStr(6),
                Op::CallNative {
                    name: "std::web::ws_send".into(),
                    argc: 2,
                },
                Op::Pop,
                Op::CallNative {
                    name: "std::web::ws_id".into(),
                    argc: 0,
                },
                Op::Pop,
                Op::CallNative {
                    name: "std::web::ws_message".into(),
                    argc: 0,
                },
                Op::Pop,
                Op::PushLocal(0),
                Op::PushInt(1_000),
                Op::PushStr(7),
                Op::CallNative {
                    name: "std::web::ws_close".into(),
                    argc: 3,
                },
                Op::Ret,
            ],
            1,
        );
        module.string_table = vec![
            "wss://example.invalid/socket".into(),
            "[\"titan.v1\"]".into(),
            "on_open".into(),
            "on_message".into(),
            "on_error".into(),
            "on_close".into(),
            "hello".into(),
            "done".into(),
        ];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 5);
        assert_eq!(imports.natives["std::web::ws_connect"], 0);
        assert_eq!(imports.natives["std::web::ws_send"], 1);
        assert_eq!(imports.natives["std::web::ws_close"], 2);
        assert_eq!(imports.natives["std::web::ws_id"], 3);
        assert_eq!(imports.natives["std::web::ws_message"], 4);
    }

    #[test]
    fn emits_canvas_2d_drawing_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0), Op::PushInt(640), Op::PushInt(360),
                Op::CallNative { name: "std::web::canvas_resize".into(), argc: 3 }, Op::Pop,
                Op::PushStr(0), Op::PushStr(1),
                Op::CallNative { name: "std::web::canvas_clear".into(), argc: 2 }, Op::Pop,
                Op::PushStr(0), Op::PushInt(20), Op::PushInt(20), Op::PushInt(120), Op::PushInt(80), Op::PushStr(2),
                Op::CallNative { name: "std::web::canvas_fill_rect".into(), argc: 6 }, Op::Pop,
                Op::PushStr(0), Op::PushInt(18), Op::PushInt(18), Op::PushInt(124), Op::PushInt(84), Op::PushStr(3), Op::PushInt(2),
                Op::CallNative { name: "std::web::canvas_stroke_rect".into(), argc: 7 }, Op::Pop,
                Op::PushStr(0), Op::PushInt(0), Op::PushInt(0), Op::PushInt(640), Op::PushInt(360), Op::PushStr(3), Op::PushInt(3),
                Op::CallNative { name: "std::web::canvas_line".into(), argc: 7 }, Op::Pop,
                Op::PushStr(0), Op::PushStr(4), Op::PushInt(180), Op::PushInt(80), Op::PushStr(3), Op::PushStr(5),
                Op::CallNative { name: "std::web::canvas_text".into(), argc: 6 }, Op::Ret,
            ],
            0,
        );
        module.string_table = vec![
            "#scene".into(), "#101827".into(), "#38bdf8".into(),
            "#f8fafc".into(), "TITAN Canvas".into(), "24px sans-serif".into(),
        ];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 6);
        assert_eq!(imports.natives["std::web::canvas_resize"], 0);
        assert_eq!(imports.natives["std::web::canvas_clear"], 1);
        assert_eq!(imports.natives["std::web::canvas_fill_rect"], 2);
        assert_eq!(imports.natives["std::web::canvas_stroke_rect"], 3);
        assert_eq!(imports.natives["std::web::canvas_line"], 4);
        assert_eq!(imports.natives["std::web::canvas_text"], 5);
    }

    #[test]
    fn emits_request_animation_frame_lifecycle_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::CallNative { name: "std::web::animation_start".into(), argc: 1 },
                Op::StoreLocal(0),
                Op::CallNative { name: "std::web::frame_id".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::web::frame_time_ms".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::web::frame_delta_ms".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::web::frame_count".into(), argc: 0 }, Op::Pop,
                Op::PushLocal(0),
                Op::CallNative { name: "std::web::animation_cancel".into(), argc: 1 },
                Op::Ret,
            ],
            1,
        );
        module.string_table = vec!["animate".into()];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 6);
        assert_eq!(imports.natives["std::web::animation_start"], 0);
        assert_eq!(imports.natives["std::web::animation_cancel"], 1);
        assert_eq!(imports.natives["std::web::frame_id"], 2);
        assert_eq!(imports.natives["std::web::frame_time_ms"], 3);
        assert_eq!(imports.natives["std::web::frame_delta_ms"], 4);
        assert_eq!(imports.natives["std::web::frame_count"], 5);
    }

    #[test]
    fn emits_webgl2_pipeline_lifecycle_imports() {
        let mut module = module_with(
            vec![
                Op::PushStr(0),
                Op::CallNative { name: "std::web::webgl_supported".into(), argc: 1 }, Op::Pop,
                Op::PushStr(0), Op::PushStr(1), Op::PushStr(2), Op::PushStr(3), Op::PushStr(4),
                Op::CallNative { name: "std::web::webgl_create".into(), argc: 5 },
                Op::StoreLocal(0),
                Op::PushLocal(0), Op::PushStr(5), Op::PushInt(25), Op::PushInt(100),
                Op::CallNative { name: "std::web::webgl_uniform_f32".into(), argc: 4 }, Op::Pop,
                Op::PushLocal(0), Op::PushStr(6),
                Op::CallNative { name: "std::web::webgl_draw".into(), argc: 2 }, Op::Pop,
                Op::PushLocal(0),
                Op::CallNative { name: "std::web::webgl_delete".into(), argc: 1 }, Op::Ret,
            ],
            1,
        );
        module.string_table = vec![
            "#gl-scene".into(), "vertex shader".into(), "fragment shader".into(),
            "[-0.5,-0.5,0.5,-0.5,0,0.5]".into(), "[0,1,2]".into(),
            "u_offset".into(), "[0.02,0.04,0.09,1]".into(),
        ];
        validate(&module);
        let imports = collect_host_imports(&module);
        assert_eq!(imports.definitions.len(), 5);
        assert_eq!(imports.natives["std::web::webgl_supported"], 0);
        assert_eq!(imports.natives["std::web::webgl_create"], 1);
        assert_eq!(imports.natives["std::web::webgl_uniform_f32"], 2);
        assert_eq!(imports.natives["std::web::webgl_draw"], 3);
        assert_eq!(imports.natives["std::web::webgl_delete"], 4);
    }

    #[test]
    fn rejects_non_browser_native_calls() {
        let module = module_with(
            vec![Op::CallNative {
                name: "std::time::unix_millis".into(),
                argc: 0,
            }, Op::Ret],
            0,
        );
        assert!(matches!(compile(&module), Err(WasmError::Unsupported { .. })));
    }

    #[test]
    fn rejects_invalid_string_table_references() {
        let module = module_with(vec![Op::PushStr(3), Op::Ret], 0);
        assert!(matches!(
            compile(&module),
            Err(WasmError::InvalidString { string: 3, .. })
        ));
    }

    #[test]
    fn emits_managed_numeric_arrays_tuples_len_and_index() {
        let module = module_with(
            vec![
                Op::PushInt(10), Op::PushInt(20), Op::PushInt(30), Op::NewArray(3),
                Op::StoreLocal(0), Op::PushLocal(0), Op::Len, Op::Pop,
                Op::PushLocal(0), Op::PushInt(1), Op::Index, Op::Ret,
            ],
            1,
        );
        validate(&module);

        let tuple = module_with(
            vec![
                Op::PushInt(7), Op::PushInt(9), Op::NewTuple(2),
                Op::PushInt(0), Op::Index, Op::Ret,
            ],
            0,
        );
        validate(&tuple);
    }

    #[test]
    fn emits_copy_on_write_array_set() {
        let module = module_with(
            vec![
                Op::PushInt(10), Op::PushInt(20), Op::NewArray(2),
                Op::PushInt(1), Op::PushInt(99),
                Op::CallNative { name: "std::array::set".into(), argc: 3 },
                Op::PushInt(1), Op::Index, Op::Ret,
            ],
            0,
        );
        validate(&module);
    }

    #[test]
    fn emits_copy_on_write_array_push() {
        let module = module_with(
            vec![
                Op::PushInt(10), Op::PushInt(20), Op::NewArray(2), Op::PushInt(30),
                Op::CallNative { name: "std::array::push".into(), argc: 2 },
                Op::PushInt(2), Op::Index, Op::Ret,
            ],
            0,
        );
        validate(&module);
    }

    #[test]
    fn emits_copy_on_write_array_pop_and_empty_fast_path() {
        let module = module_with(
            vec![
                Op::PushInt(10), Op::PushInt(20), Op::PushInt(30), Op::NewArray(3),
                Op::CallNative { name: "std::array::pop".into(), argc: 1 }, Op::Len, Op::Ret,
            ],
            0,
        );
        validate(&module);
        let empty = module_with(
            vec![
                Op::NewArray(0),
                Op::CallNative { name: "std::array::pop".into(), argc: 1 }, Op::Len, Op::Ret,
            ],
            0,
        );
        validate(&empty);
    }

    #[test]
    fn emits_copy_on_write_array_slice() {
        let module = module_with(
            vec![
                Op::PushInt(10), Op::PushInt(20), Op::PushInt(30), Op::PushInt(40), Op::NewArray(4),
                Op::PushInt(1), Op::PushInt(3),
                Op::CallNative { name: "std::array::slice".into(), argc: 3 },
                Op::PushInt(0), Op::Index, Op::Ret,
            ],
            0,
        );
        validate(&module);
    }

    #[test]
    fn emits_copy_on_write_array_concat() {
        let module = module_with(
            vec![
                Op::PushInt(1), Op::PushInt(2), Op::NewArray(2),
                Op::PushInt(3), Op::PushInt(4), Op::NewArray(2),
                Op::CallNative { name: "std::array::concat".into(), argc: 2 },
                Op::PushInt(3), Op::Index, Op::Ret,
            ],
            0,
        );
        validate(&module);
    }

    #[test]
    fn emits_heap_statistics_and_limit_controls() {
        let module = module_with(
            vec![
                Op::CallNative { name: "std::wasm::heap_used".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_capacity".into(), argc: 0 }, Op::Pop,
                Op::PushInt(65_536),
                Op::CallNative { name: "std::wasm::heap_set_limit".into(), argc: 1 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_limit".into(), argc: 0 }, Op::Ret,
            ],
            0,
        );
        validate(&module);
    }

    #[test]
    fn emits_heap_checkpoint_restore_and_zeroing() {
        let module = module_with(
            vec![
                Op::CallNative { name: "std::wasm::heap_checkpoint".into(), argc: 0 },
                Op::StoreLocal(0),
                Op::PushInt(1), Op::PushInt(2), Op::NewArray(2), Op::Pop,
                Op::PushLocal(0),
                Op::CallNative { name: "std::wasm::heap_restore".into(), argc: 1 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_allocations".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_allocated_bytes".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_restores".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_reclaimed_bytes".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_peak_used".into(), argc: 0 }, Op::Pop,
                Op::CallNative { name: "std::wasm::heap_reset_counters".into(), argc: 0 }, Op::Ret,
            ],
            1,
        );
        validate(&module);
    }

    #[test]
    fn emits_nested_lifo_heap_scopes() {
        let module = module_with(
            vec![
                Op::CallNative { name: "std::wasm::heap_scope_begin".into(), argc: 0 }, Op::StoreLocal(0),
                Op::PushInt(1), Op::NewArray(1), Op::Pop,
                Op::CallNative { name: "std::wasm::heap_scope_begin".into(), argc: 0 }, Op::StoreLocal(1),
                Op::PushInt(2), Op::NewArray(1), Op::Pop,
                Op::PushLocal(0), Op::CallNative { name: "std::wasm::heap_scope_end".into(), argc: 1 }, Op::Pop,
                Op::PushLocal(1), Op::CallNative { name: "std::wasm::heap_scope_end".into(), argc: 1 }, Op::Pop,
                Op::PushLocal(0), Op::CallNative { name: "std::wasm::heap_scope_end".into(), argc: 1 }, Op::Ret,
            ],
            2,
        );
        validate(&module);
    }

    #[test]
    fn rejects_string_indexing_as_array_memory() {
        let mut module = module_with(
            vec![Op::PushStr(0), Op::PushInt(0), Op::Index, Op::Ret],
            0,
        );
        module.string_table.push("not an i64 array".into());
        assert!(matches!(compile(&module), Err(WasmError::Unsupported { .. })));
    }

    #[test]
    fn emits_managed_struct_construction_and_fields() {
        let module = module_with(
            vec![
                Op::PushInt(7), Op::PushInt(9),
                Op::NewStruct { name: "Point".into(), fields: vec!["x".into(), "y".into()] },
                Op::StoreLocal(0), Op::PushLocal(0), Op::GetField("x".into()),
                Op::PushLocal(0), Op::GetField("y".into()), Op::Add, Op::Ret,
            ],
            1,
        );
        validate(&module);
    }

    #[test]
    fn preserves_struct_return_metadata_across_function_calls() {
        let mut module = module_with(
            vec![Op::Call { function: 1, argc: 0 }, Op::GetField("x".into()), Op::Ret],
            0,
        );
        module.functions[0].return_type = Some(BytecodeType::Int);
        module.functions.push(BytecodeFunc {
            name: "make_point".into(), source_file: Some("main.titan".into()), arity: 0,
            param_types: Vec::new(), return_type: Some(BytecodeType::Struct("Point".into())),
            captures: 0, locals: 0, max_stack: 4,
            code: vec![Op::PushInt(42), Op::NewStruct { name: "Point".into(), fields: vec!["x".into()] }, Op::Ret],
            debug_locations: vec![None; 3],
        });
        module.struct_schemas.insert("Point".into(), vec!["x".into()]);
        validate(&module);
    }

    #[test]
    fn preserves_array_parameter_and_return_metadata_across_calls() {
        let mut module = module_with(
            vec![
                Op::PushInt(5), Op::PushInt(7), Op::NewArray(2),
                Op::Call { function: 1, argc: 1 },
                Op::Call { function: 2, argc: 0 }, Op::PushInt(1), Op::Index, Op::Add, Op::Ret,
            ],
            0,
        );
        module.functions.push(BytecodeFunc {
            name: "first".into(), source_file: Some("main.titan".into()), arity: 1,
            param_types: vec![BytecodeType::Array], return_type: Some(BytecodeType::Int),
            captures: 0, locals: 1, max_stack: 4,
            code: vec![Op::PushLocal(0), Op::PushInt(0), Op::Index, Op::Ret],
            debug_locations: vec![None; 4],
        });
        module.functions.push(BytecodeFunc {
            name: "make".into(), source_file: Some("main.titan".into()), arity: 0,
            param_types: Vec::new(), return_type: Some(BytecodeType::Array),
            captures: 0, locals: 0, max_stack: 4,
            code: vec![Op::PushInt(20), Op::PushInt(22), Op::NewArray(2), Op::Ret],
            debug_locations: vec![None; 4],
        });
        validate(&module);
    }

    #[test]
    fn emits_managed_enum_tags_and_payloads() {
        let module = module_with(
            vec![
                Op::PushInt(42), Op::NewEnum { name: "Maybe".into(), variant: "Some".into(), has_payload: true },
                Op::StoreLocal(0), Op::PushLocal(0),
                Op::EnumIs { name: "Maybe".into(), variant: "Some".into() }, Op::Pop,
                Op::PushLocal(0), Op::EnumPayload, Op::Ret,
            ],
            1,
        );
        validate(&module);
        let artifact = compile_artifact(&module).unwrap();
        assert_eq!(
            artifact.source_map.enum_tags.get("Maybe::Some"),
            Some(&enum_tag("Maybe", "Some"))
        );
    }

    #[test]
    fn preserves_enum_return_metadata_across_calls() {
        let mut module = module_with(
            vec![Op::Call { function: 1, argc: 0 }, Op::EnumPayload, Op::Ret],
            0,
        );
        module.functions.push(BytecodeFunc {
            name: "make".into(), source_file: Some("main.titan".into()), arity: 0,
            param_types: Vec::new(), return_type: Some(BytecodeType::Enum("Maybe".into())),
            captures: 0, locals: 0, max_stack: 4,
            code: vec![Op::PushInt(42), Op::NewEnum { name: "Maybe".into(), variant: "Some".into(), has_payload: true }, Op::Ret],
            debug_locations: vec![None; 3],
        });
        module.enum_schemas.insert("Maybe".into(), vec!["None".into(), "Some".into()]);
        validate(&module);
    }

    #[test]
    fn rejects_unsupported_runtime_values() {
        let module = module_with(
            vec![
                Op::MakeClosure { function: 0, captures: Vec::new() },
                Op::Ret,
            ],
            0,
        );
        assert!(matches!(
            compile(&module),
            Err(WasmError::Unsupported { .. })
        ));
    }

    #[test]
    fn emits_dynamic_utf8_string_concatenation() {
        let mut module = module_with(
            vec![
                Op::PushInt(20),
                Op::PushInt(22),
                Op::Add,
                Op::StoreLocal(0),
                Op::PushStr(0),
                Op::PushStr(1),
                Op::Add,
                Op::PushStr(2),
                Op::Add,
                Op::Len,
                Op::Ret,
            ],
            1,
        );
        module.string_table = vec!["Titan ".into(), "Wasm ".into(), "🚀".into()];
        let layout = analyze_function(&module, &module.functions[0]).unwrap();
        assert!(!layout.string_adds.contains(&2));
        assert!(layout.string_adds.contains(&6));
        assert!(layout.string_adds.contains(&8));
        validate(&module);
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
