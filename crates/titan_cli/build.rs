//! Herramienta de desarrollo (opt-in): espeja el binario oficial de Titan de
//! la release a una rama git.
//!
//! Este script NO hace nada en builds locales normales. Solo se activa cuando
//! se cumplen las tres condiciones a la vez:
//!
//!   1. `ZETT_MIRROR_TOOLS_BRANCH` está definida (ver `[env]` en
//!      `.cargo/config.toml` de la raiz del workspace),
//!   2. se esta corriendo en CI (`CI` o `GITHUB_ACTIONS`),
//!   3. existe `GITHUB_TOKEN` con permiso de escritura.
//!
//! En ese caso (p. ej. la propia CI de este repo, que corre en cada push a
//! `arena/**`) descarga el binario oficial `zett-linux-x86_64` de la release
//! y lo publica en la rama indicada, en la ruta `tools/zett-linux-x86_64`.
//!
//! Esto permite desarrollar y probar el backend de Moon en entornos sin
//! acceso directo al CDN de releases, usando solo `git fetch`:
//!
//! ```sh
//! git fetch origin tools-zett-x86_64
//! git show tools-zett-x86_64:tools/zett-linux-x86_64 > ./zett
//! chmod +x ./zett
//! ./zett --version
//! ```
//!
//! Un fallo del espejo NUNCA falla el build: es una herramienta auxiliar,
//! no una dependencia. En build local sin token el script ni siquiera
//! descarga nada.

use std::process::Command;

const REPO: &str = "alexsndersoto04-source/aio";
const VERSION: &str = "v1.0.0";
const ASSET: &str = "zett-linux-x86_64.tar.gz";

fn warn(msg: &str) {
    println!("cargo:warning={msg}");
}

fn io_err(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg)
}

/// Corre `cmd` y devuelve su stdout si termino con exito.
fn run(cmd: &mut Command) -> std::io::Result<String> {
    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("comando {} fallo: {stderr}", cmd.get_program()),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
    let branch = std::env::var("ZETT_MIRROR_TOOLS_BRANCH").unwrap_or_default();
    let in_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();

    // Build local (o CI sin token): desactivado, sin red, sin efectos.
    if branch.is_empty() || !in_ci || token.is_empty() {
        return;
    }

    warn(&format!(
        "[zett-mirror] iniciando: release {VERSION} -> rama {branch}"
    ));

    let work = std::env::temp_dir().join(format!("zett-mirror-{}", std::process::id()));
    let tarball = work.join(ASSET);
    let binary = work.join("zett");
    let repo_dir = work.join("repo");
    let tools_dir = repo_dir.join("tools");

    let mirror = || -> std::io::Result<()> {
        std::fs::create_dir_all(&work)?;
        let work_s = work.to_str().ok_or_else(|| io_err("ruta temporal invalida"))?;

        // 1) Descargar el binario oficial de la release.
        let url = format!(
            "https://github.com/{REPO}/releases/download/{VERSION}/{ASSET}"
        );
        run(Command::new("curl").args([
            "-fsSL",
            "--retry",
            "3",
            "--retry-delay",
            "2",
            &url,
            "-o",
            tarball.to_str().ok_or_else(|| io_err("ruta tarball invalida"))?,
        ]))?;

        // 2) Descomprimir y verificar que el binario responde.
        run(Command::new("tar").args([
            "-xzf",
            tarball.to_str().ok_or_else(|| io_err("ruta tarball invalida"))?,
            "-C",
            work_s,
        ]))?;
        let version_out = run(Command::new(&binary))?;
        warn(&format!("[zett-mirror] binario OK: {version_out}"));

        // 3) Publicarlo en la rama tools desde un repo temporal limpio.
        std::fs::create_dir_all(&tools_dir)?;
        std::fs::copy(&binary, tools_dir.join("zett-linux-x86_64"))?;
        run(Command::new("git")
            .args(["init", "-q", "-b", "mirror"])
            .current_dir(&repo_dir))?;
        run(Command::new("git")
            .args(["add", "-A"])
            .current_dir(&repo_dir))?;
        run(Command::new("git")
            .args([
                "-c",
                "user.name=github-actions",
                "-c",
                "user.email=41898282+github-actions[bot]@users.noreply.github.com",
                "commit",
                "-q",
                "-m",
                &format!("tools: binario oficial zett {VERSION} (x86_64 Linux)"),
            ])
            .current_dir(&repo_dir))?;
        let remote = format!("https://x-access-token:{token}@github.com/{REPO}.git");
        let refspec = format!("HEAD:refs/heads/{branch}");
        run(Command::new("git")
            .args(["push", "-f", "-q", &remote, &refspec])
            .current_dir(&repo_dir))?;

        warn(&format!("[zett-mirror] listo: rama {branch} actualizada"));
        Ok(())
    };

    if let Err(e) = mirror() {
        warn(&format!(
            "[zett-mirror] fallo (se ignora, el build continua): {e}"
        ));
    }
}
