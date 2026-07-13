fn main() {
    if let Err(error) = titan_lsp::run_stdio() {
        eprintln!("titan-lsp: {error}");
        std::process::exit(1);
    }
}
