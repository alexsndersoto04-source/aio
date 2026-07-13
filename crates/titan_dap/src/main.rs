fn main() {
    if let Err(error) = titan_dap::run_stdio() {
        eprintln!("titan-dap: {error}");
        std::process::exit(1);
    }
}
