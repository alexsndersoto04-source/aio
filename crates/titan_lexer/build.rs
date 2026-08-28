//! Build script de titan_lexer — no-op.
//!
//! La logica del espejo de zett vive ahora en `titan_parser/build.rs`
//! (unidad nueva: cargo la compila y ejecuta de forma garantizada; la
//! unidad de este crate tenia una version cacheada vieja que no corria).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
