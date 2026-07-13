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
    /// Parse and type-check a file or project without producing an artifact
    Check { #[arg(default_value = ".")] input: String },
    /// Compile a file or project to inspectable bytecode
    Build { #[arg(default_value = ".")] input: String, #[arg(short, long)] output: Option<String> },
    /// Execute a previously built .tbc artifact without source compilation
    Exec {
        input: String,
        /// Deny filesystem, process, network, and environment native functions
        #[arg(long)]
        sandbox: bool,
    },
    /// Compile and run a file or project
    Run {
        #[arg(default_value = ".")] input: String,
        /// Deny filesystem, process, network, and environment native functions
        #[arg(long)] sandbox: bool,
        #[arg(last = true)] args: Vec<String>,
    },
    /// Execute every .titan test program under tests/
    Test { #[arg(default_value = ".")] input: String, #[arg(long)] sandbox: bool },
    /// Start the interactive REPL
    Repl,
    /// Print version
    Version,
}

fn main() {
    match Cli::parse().command {
        Command::New { path } => cmd_new(&path),
        Command::Check { input } => cmd_check(&input),
        Command::Build { input, output } => cmd_build(&input, output),
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

fn cmd_exec(input: &str, sandbox: bool) {
    let bytes = fs::read(input).unwrap_or_else(|error| fatal("ARTIFACT READ ERROR", error));
    let module = titan_codegen::BytecodeArtifact::decode(&bytes).unwrap_or_else(|error| fatal("INVALID ARTIFACT", error));
    match run_module(module, sandbox) {
        Ok(Some(value)) if !matches!(value, titan_vm::Value::Nil) => println!("=> {}", titan_vm::val_to_string(&value)),
        Ok(_) => {}
        Err(error) => fatal("RUNTIME ERROR", error),
    }
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
