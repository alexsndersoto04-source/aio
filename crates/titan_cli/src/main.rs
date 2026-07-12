//! Titan CLI — Full pipeline: lex → parse → typecheck → codegen → VM run

use clap::{Parser, Subcommand};
use std::fs;

#[derive(Parser)]
#[command(name = "titan", version, about = "Titan Language Compiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Compile a Titan source file to bytecode
    Build {
        #[arg(default_value = "main.titan")]
        input: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Compile and run a Titan program
    Run {
        #[arg(default_value = "main.titan")]
        input: String,
        /// Deny filesystem, process, network, and environment native functions
        #[arg(long)]
        sandbox: bool,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Start interactive REPL
    Repl,
    /// Print version
    Version,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { input, output } => cmd_build(&input, output),
        Command::Run { input, sandbox, args: _ } => cmd_run(&input, sandbox),
        Command::Repl => cmd_repl(),
        Command::Version => cmd_version(),
    }
}

fn cmd_build(input: &str, output: Option<String>) {
    let source = fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("ERROR: Cannot read '{}': {}", input, e);
        std::process::exit(1);
    });

    // Full pipeline
    let module = compile_titan(&source).unwrap_or_else(|e| {
        eprintln!("Compilation failed:\n{}", e);
        std::process::exit(1);
    });

    let target = output.unwrap_or_else(|| {
        let path = std::path::Path::new(input);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("program");
        format!("{}.tbc", stem)
    });

    // The textual bytecode format is deterministic, inspectable and useful for
    // tooling. A stable binary container can be added without pretending that
    // merely printing a target path created an artifact.
    let artifact = format!("TITAN-BYTECODE 1\n{:#?}\n", module);
    fs::write(&target, artifact).unwrap_or_else(|e| {
        eprintln!("ERROR: Cannot write '{}': {}", target, e);
        std::process::exit(1);
    });

    println!("BUILD: {} → {}", input, target);
    println!("  Functions: {}", module.functions.len());
    for f in &module.functions {
        println!("  fn {} ({} ops, {} locals)", f.name, f.code.len(), f.locals);
    }
    println!("  Entry: fn[{}]", module.entry);
}

fn cmd_run(input: &str, sandbox: bool) {
    let source = fs::read_to_string(input).unwrap_or_else(|e| {
        eprintln!("ERROR: Cannot read '{}': {}", input, e);
        std::process::exit(1);
    });

    match compile_and_run_with_capabilities(&source, sandbox) {
        Ok(result) => {
            if let Some(v) = result {
                println!("=> {}", titan_vm::val_to_string(&v));
            }
        }
        Err(e) => {
            eprintln!("RUNTIME ERROR:\n{}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_repl() {
    println!("⚔️  TITAN REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("   Type Titan code. Type :q to quit, :h for help.");
    println!();

    loop {
        print!("titan> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() { continue; }

        if line.starts_with(':') {
            match line {
                ":q" | ":quit" => { println!("Goodbye."); break; }
                ":h" | ":help" => {
                    println!("Commands:");
                    println!("  :q, :quit  — Exit REPL");
                    println!("  :h, :help  — Help");
                    println!("  :v         — Version");
                    println!();
                    println!("Any other input is compiled and executed as Titan code.");
                }
                ":v" => println!("TITAN v{}", env!("CARGO_PKG_VERSION")),
                _ => println!("Unknown: {} (:h for help)", line),
            }
            continue;
        }

        // Wrap expression/statement in a function
        let source = format!("fn main() {{ {} }}", line);
        match compile_and_run(&source) {
            Ok(result) => {
                if let Some(v) = result {
                    let s = titan_vm::val_to_string(&v);
                    if s != "nil" { println!("  => {}", s); }
                }
            }
            Err(e) => eprintln!("  Error: {}", e),
        }
    }
}

fn cmd_version() {
    println!("TITAN Language Compiler v{}", env!("CARGO_PKG_VERSION"));
    println!("The Executioner of Programming Languages");
}

// ─── Full compilation pipeline ───

fn compile_titan(source: &str) -> Result<titan_codegen::CompiledModule, String> {
    // 1. Lex
    let mut lexer = titan_lexer::Lexer::new(source);
    let (tokens, lex_errors) = lexer.tokenize();
    if !lex_errors.is_empty() {
        return Err(format!("Lexer errors: {:?}", lex_errors));
    }

    // 2. Parse
    let mut parser = titan_parser::Parser::new(tokens.to_vec());
    let program = parser.parse_program().map_err(|e| {
        let mut s = e.to_string();
        for err in parser.errors() { s.push_str(&format!("\n  {}", err)); }
        s
    })?;

    // 3. Type check
    let mut env = titan_typechecker::TypeEnv::new();
    env.check_program(&program).map_err(|errors| {
        errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
    })?;

    // 4. Codegen
    let mut compiler = titan_codegen::AstCompiler::new();
    compiler.compile_program(&program).map_err(|e| e.to_string())
}

fn compile_and_run(source: &str) -> Result<Option<titan_vm::Value>, String> {
    compile_and_run_with_capabilities(source, false)
}

fn compile_and_run_with_capabilities(source: &str, sandbox: bool) -> Result<Option<titan_vm::Value>, String> {
    let module = compile_titan(source)?;
    let mut vm = if sandbox { titan_vm::Vm::sandboxed(module) } else { titan_vm::Vm::new(module) };
    vm.run().map_err(|error| error.to_string())
}