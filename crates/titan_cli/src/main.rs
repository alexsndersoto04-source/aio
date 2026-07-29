//! Titan command-line compiler and project tooling.

use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "titan", version, about = "Titan language compiler")]
pub struct Cli { #[command(subcommand)] pub command: Command }

#[derive(Subcommand)]
pub enum Command {
    /// Create a project with Titan.toml and src/main.titan
    New { path: String },
    /// Add a remote dependency requirement to Titan.toml
    Add { package: String, #[arg(default_value="*")] requirement: String, #[arg(long,default_value=".")] project: String },
    /// Resolve, download, verify, and install remote dependencies
    Fetch { #[arg(long,default_value=".")] project: String, #[arg(long,default_value="https://registry.titan-lang.org")] registry: String, #[arg(long)] offline: bool },
    /// Re-resolve and update remote dependencies
    Update { #[arg(long,default_value=".")] project: String, #[arg(long,default_value="https://registry.titan-lang.org")] registry: String },
    /// Generate a private Ed25519 package signing key
    Keygen { path: String },
    /// Build and sign a deterministic .tpkg archive
    Pack { #[arg(long,default_value=".")] project: String, #[arg(long)] key: String, #[arg(long)] output: String },
    /// Build, sign, and upload a package using TITAN_REGISTRY_TOKEN
    Publish { #[arg(long,default_value=".")] project: String, #[arg(long)] key: String, #[arg(long,default_value="https://registry.titan-lang.org")] registry: String },
    /// Parse and type-check a file or project without producing an artifact
    Check { #[arg(default_value = ".")] input: String },
    /// Compile a file or project to inspectable bytecode
    Build { #[arg(default_value = ".")] input: String, #[arg(short, long)] output: Option<String> },
    /// Compile a file or project to standalone WebAssembly plus standard/logical source maps
    Wasm { #[arg(default_value = ".")] input: String, #[arg(short, long)] output: Option<String> },
    /// Interactive bytecode debugger with breakpoints and stack inspection
    Debug {
        #[arg(default_value = ".")] input: String,
        #[arg(short, long)] breakpoints: Vec<String>,
        #[arg(long)] sandbox: bool,
    },
    /// Execute an already-compiled .tbc bytecode artifact directly
    Exec { #[arg(default_value = "target/program.tbc")] input: String, #[arg(long)] sandbox: bool },
    /// Execute bytecode using the high-performance VM
    Run {
        #[arg(default_value = ".")] input: String,
        #[arg(long)] sandbox: bool,
        #[arg(trailing_var_arg = true)] args: Vec<String>,
    },
    /// Run all .titan unit tests discovered in the tests/ directory
    Test { #[arg(default_value = ".")] input: String, #[arg(long)] sandbox: bool },
    /// Launch the interactive read-eval-print loop
    Repl,
    /// Print version details
    Version,
}

fn main() {
    match Cli::parse().command {
        Command::New { path } => cmd_new(&path),
        Command::Add { package, requirement, project } => cmd_add(&project,&package,&requirement),
        Command::Fetch { project, registry, offline } => cmd_fetch(&project,&registry,offline),
        Command::Update { project, registry } => cmd_fetch(&project,&registry,false),
        Command::Keygen { path } => { titan_pkg::generate_signing_key(Path::new(&path)).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));println!("Generated signing key at {path}"); },
        Command::Pack { project, key, output } => { let publication=titan_pkg::build_package(Path::new(&project),Path::new(&key),Path::new(&output)).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));println!("Packed {} {} -> {}\nsha256={}\nsigning_key={}\nsignature={}",publication.name,publication.version,publication.archive.display(),publication.sha256,publication.signing_key,publication.signature); },
        Command::Publish { project, key, registry } => cmd_publish(&project,&key,&registry),
        Command::Check { input } => cmd_check(&input),
        Command::Build { input, output } => cmd_build(&input, output),
        Command::Wasm { input, output } => cmd_wasm(&input, output),
        Command::Debug { input, breakpoints, sandbox } => cmd_debug(&input, &breakpoints, sandbox),
        Command::Exec { input, sandbox } => cmd_exec(&input, sandbox),
        Command::Run { input, sandbox, args } => cmd_run(&input, sandbox, args),
        Command::Test { input, sandbox } => cmd_test(&input, sandbox),

        Command::Repl => cmd_repl(),
        Command::Version => cmd_version(),
    }
}

fn cmd_new(path: &str) {
    let root = Path::new(path);
    let name = root.file_name().and_then(|name| name.to_str()).unwrap_or(path);
    match titan_pkg::create_project(root, name) {
        Ok(root) => println!("Created Titan project '{}' at {}", name, root.display()),
        Err(error) => fatal("PROJECT ERROR", error),
    }
}

fn cmd_add(project:&str,package:&str,requirement:&str){let path=Path::new(project);let root=titan_pkg::find_project_root(path).unwrap_or_else(||path.to_path_buf());titan_pkg::add_remote_dependency(&root,package,requirement).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));println!("Added {package} {requirement}");}
fn cmd_fetch(project:&str,registry:&str,offline:bool){let path=Path::new(project);let root=titan_pkg::find_project_root(path).unwrap_or_else(||path.to_path_buf());let lock=titan_pkg::sync_remote_dependencies(&root,registry,offline).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));println!("Synchronized {} remote packages{}",lock.packages.len(),if offline{" (offline)"}else{""});}

fn cmd_publish(project:&str,key:&str,registry:&str){let token=std::env::var("TITAN_REGISTRY_TOKEN").unwrap_or_else(|_|fatal_message("PACKAGE ERROR","TITAN_REGISTRY_TOKEN is not set"));let archive=std::env::temp_dir().join(format!("titan-publish-{}.tpkg",std::process::id()));let _=std::fs::remove_file(&archive);let publication=titan_pkg::build_package(Path::new(project),Path::new(key),&archive).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));let publisher=titan_pkg::Publisher::new(registry,&token).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));publisher.publish(&publication).unwrap_or_else(|error|fatal("PACKAGE ERROR",error));let _=std::fs::remove_file(archive);println!("Published {} {}",publication.name,publication.version);}

fn cmd_check(input: &str) {
    match load_and_compile(input) {
        Ok((project, module)) => println!("CHECK OK: {} files, {} functions", project.load_order.len(), module.functions.len()),
        Err(error) => fatal_message("CHECK FAILED", &error),
    }
}

fn cmd_build(input: &str, output: Option<String>) {
    let (project, module) = load_and_compile(input).unwrap_or_else(|error| fatal_message("COMPILATION FAILED", &error));
    let target = output.map(PathBuf::from).unwrap_or_else(|| default_artifact(&project));
    if let Some(parent) = target.parent() { fs::create_dir_all(parent).unwrap_or_else(|error| fatal("BUILD ERROR", error)); }
    let artifact = titan_codegen::BytecodeArtifact::encode(&module).unwrap_or_else(|error| fatal("BUILD ERROR", error));
    fs::write(&target, artifact).unwrap_or_else(|error| fatal("BUILD ERROR", error));
    println!("BUILD: {} → {}", project.entry.display(), target.display());
    println!("  Sources: {}", project.load_order.len());
    println!("  Functions: {}", module.functions.len());
    for function in &module.functions { println!("  fn {} ({} ops, {} locals)", function.name, function.code.len(), function.locals); }
    println!("  Entry: fn[{}]", module.entry);
}

fn cmd_debug(input: &str, breakpoint_specs: &[String], sandbox: bool) {
    use std::io::Write;
    use titan_vm::{Breakpoint, DebugCommand, DebugEvent, Debugger};

    let (project, module) = load_and_compile(input).unwrap_or_else(|error| fatal_message("COMPILATION FAILED", &error));
    let mut breakpoints = Vec::new();
    for specification in breakpoint_specs {
        let (path, line) = specification.rsplit_once(':').unwrap_or_else(|| fatal_message("DEBUG ERROR", "breakpoints must use path:line"));
        let line = line.parse::<usize>().ok().filter(|line| *line > 0).unwrap_or_else(|| fatal_message("DEBUG ERROR", "breakpoint line must be a positive integer"));
        let path = Path::new(path); let path = if path.is_absolute() { path.to_path_buf() } else { project.root.join(path) };
        let canonical = path.canonicalize().unwrap_or_else(|error| fatal("DEBUG ERROR", error));
        breakpoints.push(Breakpoint::SourceLine { source_file: canonical.to_string_lossy().into_owned(), line });
    }
    if breakpoints.is_empty() { breakpoints.push(Breakpoint::Instruction { function: module.entry, instruction: 0 }); }

    let (controller, mut debugger) = Debugger::channel(breakpoints);
    let worker = std::thread::spawn(move || {
        let mut vm = if sandbox { titan_vm::Vm::sandboxed(module) } else { titan_vm::Vm::new(module) };
        vm.run_debug(&mut debugger)
    });

    loop {
        match controller.recv().unwrap_or_else(|error| fatal_message("DEBUG ERROR", &error)) {
            DebugEvent::Stopped(frame) => {
                let location = frame.location.map(|location| format!("{}:{}:{}", frame.source_file.as_deref().unwrap_or("<source>"), location.line, location.column)).unwrap_or_else(|| format!("{}:ip{}", frame.function_name, frame.instruction));
                println!("\nstopped at {location} [depth {}]", frame.depth);
                println!("function: {} (id {})", frame.function_name, frame.function_id);
                println!("locals:"); for (index, value) in frame.locals.iter().enumerate() { println!("  [{index}] = {}", titan_vm::val_to_string(value)); }
                println!("stack:"); for (index, value) in frame.stack.iter().enumerate() { println!("  [{index}] = {}", titan_vm::val_to_string(value)); }
                loop {
                    print!("debug [c=continue s=step n=next o=out p=print q=quit]> "); std::io::stdout().flush().unwrap_or_else(|error| fatal("DEBUG ERROR", error));
                    let mut command = String::new(); std::io::stdin().read_line(&mut command).unwrap_or_else(|error| fatal("DEBUG ERROR", error));
                    let command = match command.trim() { "c" | "continue" => Some(DebugCommand::Continue), "s" | "step" => Some(DebugCommand::StepIn), "n" | "next" => Some(DebugCommand::StepOver), "o" | "out" => Some(DebugCommand::StepOut), "q" | "quit" => Some(DebugCommand::Terminate), "p" | "print" => { println!("locals={:?}\nstack={:?}", frame.locals, frame.stack); None } _ => { println!("unknown debugger command"); None } };
                    if let Some(command) = command { controller.command(command).unwrap_or_else(|error| fatal_message("DEBUG ERROR", &error)); break; }
                }
            }
            DebugEvent::Terminated { error } => { if let Some(error) = error { println!("debuggee terminated: {error}"); } else { println!("debuggee exited successfully"); } break; }
        }
    }
    if worker.join().is_err() { fatal_message("DEBUG ERROR", "debuggee worker panicked"); }
}

fn cmd_exec(input: &str, sandbox: bool) {
    let bytes = fs::read(input).unwrap_or_else(|error| fatal("ARTIFACT READ ERROR", error));
    let module = titan_codegen::BytecodeArtifact::decode(&bytes).unwrap_or_else(|error| fatal("INVALID ARTIFACT", error));
    match run_module(module, sandbox) {
        Ok(Some(value)) if !matches!(value, titan_vm::Value::Nil) => println!("=> {}", titan_vm::val_to_string(&value)),
        Ok(_) => {}
        Err(error) => fatal("RUNTIME ERROR", error),
    }
}
fn cmd_wasm(input: &str, output: Option<String>) {
    let (project, module) = load_and_compile(input)
        .unwrap_or_else(|error| fatal_message("COMPILATION FAILED", &error));
    let target = output.map(PathBuf::from).unwrap_or_else(|| {
        let name = project
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("program");
        project.root.join("target").join(format!("{name}.wasm"))
    });
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| fatal("WASM BUILD ERROR", error));
    }
    let mut artifact = titan_wasm::compile_artifact_with_source_root(&module, Some(&project.root))
        .unwrap_or_else(|error| fatal("WASM BUILD ERROR", error));
    artifact.standard_source_map.sources_content = Some(
        artifact
            .standard_source_map
            .sources
            .iter()
            .map(|source| {
                let source = Path::new(source);
                let source = if source.is_absolute() {
                    source.to_path_buf()
                } else {
                    project.root.join(source)
                };
                project.sources.get(&source).cloned()
            })
            .collect(),
    );

    let mut logical_map_name = target.as_os_str().to_os_string();
    logical_map_name.push(".map.json");
    let logical_map_target = PathBuf::from(logical_map_name);
    let logical_map = serde_json::to_vec_pretty(&artifact.source_map)
        .unwrap_or_else(|error| fatal("WASM SOURCE MAP ERROR", error));

    let mut standard_map_name = target.as_os_str().to_os_string();
    standard_map_name.push(".map");
    let standard_map_target = PathBuf::from(standard_map_name);
    let standard_map = serde_json::to_vec(&artifact.standard_source_map)
        .unwrap_or_else(|error| fatal("WASM SOURCE MAP ERROR", error));
    let source_map_filename = standard_map_target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| fatal_message("WASM SOURCE MAP ERROR", "map filename is not valid UTF-8"));
    let source_map_url = percent_encode_path_segment(source_map_filename);
    titan_wasm::append_source_mapping_url(&mut artifact.wasm, &source_map_url)
        .unwrap_or_else(|error| fatal("WASM SOURCE MAP ERROR", error));

    fs::write(&target, artifact.wasm).unwrap_or_else(|error| fatal("WASM BUILD ERROR", error));
    fs::write(&logical_map_target, logical_map)
        .unwrap_or_else(|error| fatal("WASM SOURCE MAP ERROR", error));
    fs::write(&standard_map_target, standard_map)
        .unwrap_or_else(|error| fatal("WASM SOURCE MAP ERROR", error));

    println!("WASM: {} -> {}", project.entry.display(), target.display());
    println!("TITAN SOURCE MAP: {}", logical_map_target.display());
    println!("STANDARD SOURCE MAP: {}", standard_map_target.display());
}


fn percent_encode_path_segment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    encoded
}

fn cmd_run(input: &str, sandbox: bool, _args: Vec<String>) {
    let (_, module) = load_and_compile(input).unwrap_or_else(|error| fatal_message("COMPILATION FAILED", &error));
    match run_module(module, sandbox) {
        Ok(Some(value)) if !matches!(value, titan_vm::Value::Nil) => println!("=> {}", titan_vm::val_to_string(&value)),
        Ok(_) => {}
        Err(error) => fatal("RUNTIME ERROR", error),
    }
}

fn cmd_test(input: &str, sandbox: bool) {
    let input = Path::new(input);
    let root = titan_pkg::find_project_root(input).unwrap_or_else(|| input.to_path_buf());
    let tests_root = root.join("tests");
    let files = discover_titan_files(&tests_root).unwrap_or_else(|error| fatal("TEST DISCOVERY ERROR", error));
    if files.is_empty() { fatal_message("TEST ERROR", &format!("no .titan tests found under {}", tests_root.display())); }
    let mut passed = 0usize;
    for file in &files {
        print!("test {} ... ", file.strip_prefix(&root).unwrap_or(file).display());
        match load_and_compile_path(file).and_then(|(_, module)| run_module(module, sandbox).map_err(|error| error.to_string())) {
            Ok(_) => { passed += 1; println!("ok"); }
            Err(error) => println!("FAILED\n  {error}"),
        }
    }
    println!("\ntest result: {}. {} passed; {} failed", if passed == files.len() { "ok" } else { "FAILED" }, passed, files.len() - passed);
    if passed != files.len() { std::process::exit(1); }
}

fn cmd_repl() {
    println!("TITAN REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("Type Titan code. :q quits; :h shows help.");
    loop {
        print!("titan> ");
        use std::io::Write;
        if std::io::stdout().flush().is_err() { break; }
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) { Ok(0) | Err(_) => break, Ok(_) => {} }
        let line = line.trim();
        if line.is_empty() { continue; }
        match line {
            ":q" | ":quit" => break,
            ":h" | ":help" => { println!(":q quit  :h help  :v version"); continue; }
            ":v" => { cmd_version(); continue; }
            value if value.starts_with(':') => { println!("Unknown command: {value}"); continue; }
            _ => {}
        }
        let source = format!("fn main() {{ {line} }}");
        match compile_source(&source).and_then(|module| run_module(module, false).map_err(|error| error.to_string())) {
            Ok(Some(value)) if !matches!(value, titan_vm::Value::Nil) => println!("=> {}", titan_vm::val_to_string(&value)),
            Ok(_) => {}
            Err(error) => eprintln!("Error: {error}"),
        }
    }
}


fn cmd_version() { println!("TITAN Language Compiler v{}", env!("CARGO_PKG_VERSION")); }

fn load_and_compile(input: &str) -> Result<(titan_pkg::SourceProject, titan_codegen::CompiledModule), String> {
    let entry = titan_pkg::default_entry(input);
    load_and_compile_path(&entry)
}

fn load_and_compile_path(entry: &Path) -> Result<(titan_pkg::SourceProject, titan_codegen::CompiledModule), String> {
    let project = titan_pkg::SourceProject::load(entry).map_err(|error| error.to_string())?;
    if project.manifest.is_some() {
        titan_pkg::Lockfile::from_dependencies(&project.dependencies).and_then(|lock| lock.write(&project.root.join("Titan.lock"))).map_err(|error| error.to_string())?;
    }
    let module = compile_program(&project.program)?;
    Ok((project, module))
}

fn compile_source(source: &str) -> Result<titan_codegen::CompiledModule, String> {
    let mut lexer = titan_lexer::Lexer::new(source);
    let (tokens, errors) = lexer.tokenize();
    if !errors.is_empty() { return Err(errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")); }
    let mut parser = titan_parser::Parser::new(tokens.to_vec());
    let program = parser.parse_program().map_err(|error| error.to_string())?;
    compile_program(&program)
}

fn compile_program(program: &titan_ast::Program) -> Result<titan_codegen::CompiledModule, String> {
    let mut types = titan_typechecker::TypeEnv::new();
    types.check_program(program).map_err(|errors| errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))?;
    titan_codegen::AstCompiler::new().compile_program(program).map_err(|error| error.to_string())
}

fn run_module(module: titan_codegen::CompiledModule, sandbox: bool) -> Result<Option<titan_vm::Value>, titan_vm::VmError> {
    let mut vm = if sandbox { titan_vm::Vm::sandboxed(module) } else { titan_vm::Vm::new(module) };
    vm.run()
}

fn default_artifact(project: &titan_pkg::SourceProject) -> PathBuf {
    let name = project.root.file_name().and_then(|name| name.to_str()).unwrap_or("program");
    project.root.join("target").join(format!("{name}.tbc"))
}

fn discover_titan_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    fn visit(path: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
        if !path.exists() { return Ok(()); }
        let mut entries: Vec<_> = fs::read_dir(path)?.collect::<std::io::Result<_>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if entry.file_type()?.is_dir() { visit(&path, output)?; }
            else if path.extension().and_then(|value| value.to_str()) == Some("titan") { output.push(path); }
        }
        Ok(())
    }
    let mut output = Vec::new(); visit(root, &mut output)?; Ok(output)
}

fn fatal(label: &str, error: impl std::fmt::Display) -> ! { fatal_message(label, &error.to_string()) }
fn fatal_message(label: &str, message: &str) -> ! { eprintln!("{label}:\n{message}"); std::process::exit(1) }
