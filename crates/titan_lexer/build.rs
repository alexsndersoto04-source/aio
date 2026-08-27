//! Build script de titan_lexer — ETAPA 1 DE DIAGNOSTICO (no-op).
//!
//! La version anterior con la logica de espejo de zett provoco fallos de
//! compilacion en CI (-D warnings). Para aislar el problema, este build
//! script queda vacio: si la CI pasa, el workspace y la toolchain estan
//! sanos y la logica se reintroduce en etapas pequenas (etapa 2: solo
//! diagnostico; etapa 3: descarga; etapa 4: publicacion en rama tools).
//!
//! La version anterior completa queda en el historial (commit e59c65d^).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
