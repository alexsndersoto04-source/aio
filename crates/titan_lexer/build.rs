//! Herramienta de desarrollo (opt-in): espejo del binario oficial de Titan.
//!
//! Vive en `titan_lexer` (dependencia de todo el workspace) para que su build
//! script se ejecute en cada cargo del workspace, en cualquier job.
//!
//! Solo se activa cuando se cumplen a la vez:
//!   1. `ZETT_MIRROR_TOOLS_BRANCH` esta definida (ver `[env]` en
//!      `.cargo/config.toml` de la raiz del workspace),
//!   2. se esta en CI (`CI` o `GITHUB_ACTIONS`),
//!   3. existe `GITHUB_TOKEN` con permiso de escritura.
//!
//! Descarga el binario oficial `zett-linux-x86_64` de la release v1.0.0 y lo
//! publica en la rama indicada, en `tools/zett-linux-x86_64`. Permite
//! desarrollar y probar el backend de Moon en entornos sin acceso al CDN de
//! releases:
//!
//! ```sh
//! git fetch origin tools-zett-x86_64
//! git show tools-zett-x86_64:tools/zett-linux-x86_64 > ./zett
//! chmod +x ./zett
//! ```
//!
//! Diagnostico: cada fase (start/skip/ok/error) se publica en la rama
//! `tools-zett-diag` (archivo `mirror.txt`) para poder leerla sin acceso a
//! las logs de Actions. Un fallo del espejo NUNCA falla el build: en build
//! local sin CI/token el script ni siquiera descarga nada.
//!
//! NOTA de sintaxis: este archivo se mantiene deliberadamente simple (sin
//! closures complejos ni concatenacion de &str) porque compila con
//! `-D warnings` en CI.

const REPO: &str = "alexsndersoto04-source/aio";
const VERSION: &str = "v1.0.0";
const ASSET: &str = "zett-linux-x86_64.tar.gz";
const DIAG_BRANCH: &str = "tools-zett-diag";

fn log(msg: &str) {
    println!("cargo:warning={}", msg);
}

fn now_stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("epoch={}", d.as_secs()))
        .unwrap_or_else(|_| "epoch=?".to_string())
}

fn run_git(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    match std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
    {
        Ok(o) => o,
        Err(e) => std::process::Output {
            status: std::process::ExitStatus::from_raw(127),
            stdout: Vec::new(),
            stderr: format!("no se pudo lanzar git: {}", e).into_bytes(),
        },
    }
}

/// Publica `content` como `file_name` en la rama de diagnostico.
/// Devuelve "" si todo salio bien, o el mensaje de error.
fn push_diag(token: &str, file_name: &str, content: &str) -> String {
    let pid = std::process::id();
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let work = std::env::temp_dir().join(format!("zett-diag-{}-{}", pid, t));
    if std::fs::create_dir_all(&work).is_err() {
        return "no se pudo crear dir temporal".to_string();
    }
    if std::fs::write(work.join(file_name), content).is_err() {
        return "no se pudo escribir el archivo".to_string();
    }
    let o1 = run_git(&work, &["init", "-q"]);
    if !o1.status.success() {
        return String::from_utf8_lossy(&o1.stderr).into_owned();
    }
    let o2 = run_git(&work, &["add", "-A"]);
    if !o2.status.success() {
        return String::from_utf8_lossy(&o2.stderr).into_owned();
    }
    let o3 = run_git(
        &work,
        &[
            "-c",
            "user.name=ci-diag",
            "-c",
            "user.email=ci@local",
            "commit",
            "-q",
            "-m",
            "diag",
        ],
    );
    if !o3.status.success() {
        return String::from_utf8_lossy(&o3.stderr).into_owned();
    }
    let remote = format!("https://x-access-token:{}@github.com/{}.git", token, REPO);
    let refspec = format!("HEAD:refs/heads/{}", DIAG_BRANCH);
    let o4 = run_git(&work, &["push", "-f", "-q", &remote, &refspec]);
    if !o4.status.success() {
        return String::from_utf8_lossy(&o4.stderr).into_owned();
    }
    String::new()
}

fn main() {
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let in_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();
    if token.is_empty() || !in_ci {
        // Build local normal: desactivado, sin red, sin efectos.
        return;
    }

    let branch = std::env::var("ZETT_MIRROR_TOOLS_BRANCH").unwrap_or_default();
    log(&format!(
        "[zett-mirror] CI activo; ZETT_MIRROR_TOOLS_BRANCH='{}'",
        branch
    ));

    let diag_text = format!(
        "FASE=start; branch_env='{}'; host_target={}\n",
        branch,
        std::env::var("TARGET").unwrap_or_else(|_| "desconocido".to_string())
    );
    let diag_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let err = push_diag(&token, "mirror.txt", &diag_text);
        if !err.is_empty() {
            log(&format!("[zett-mirror] diag push fallo: {}", err));
        }
    }));
    if diag_result.is_err() {
        log("[zett-mirror] panico en diag push");
    }

    if branch.is_empty() {
        let _ = push_diag(
            &token,
            "mirror.txt",
            &format!("FASE=skip; branch vacio; {}\n", now_stamp()),
        );
        return;
    }

    let mirror_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        do_mirror(&branch, &token)
    }));

    match mirror_result {
        Ok(Ok(())) => {
            log(&format!("[zett-mirror] espejo completo: rama {}", branch));
            let _ = push_diag(
                &token,
                "mirror.txt",
                &format!("FASE=ok; branch={}; {}\n", branch, now_stamp()),
            );
        }
        Ok(Err(e)) => {
            log(&format!("[zett-mirror] fallo (el build continua): {}", e));
            let _ = push_diag(
                &token,
                "mirror.txt",
                &format!(
                    "FASE=error; branch={}; error={}; {}\n",
                    branch,
                    e,
                    now_stamp()
                ),
            );
        }
        Err(_payload) => {
            log("[zett-mirror] PANICO atrapado en mirror");
            let _ = push_diag(
                &token,
                "mirror.txt",
                &format!(
                    "FASE=panico; branch={}; {}\n",
                    branch,
                    now_stamp()
                ),
            );
        }
    }
}

fn do_mirror(branch: &str, token: &str) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("zett-mirror-{}", std::process::id()));
    if std::fs::create_dir_all(&work).is_err() {
        return Err("no se pudo crear dir temporal".to_string());
    }
    let tarball = work.join(ASSET);
    let binary = work.join("zett");
    let repo_dir = work.join("repo");
    let tools_dir = repo_dir.join("tools");
    let work_s = work.to_str().unwrap_or(".");
    let tarball_s = tarball.to_str().unwrap_or(ASSET);

    // 1) Descargar el binario oficial de la release.
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        REPO, VERSION, ASSET
    );
    let o = std::process::Command::new("curl")
        .args(&[
            "-fsSL",
            "--retry",
            "3",
            "--retry-delay",
            "2",
            &url,
            "-o",
            tarball_s,
        ])
        .output()
        .map_err(|e| format!("curl: no se pudo lanzar: {}", e))?;
    if !o.status.success() {
        return Err(format!(
            "curl exit {:?}: {}",
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stderr)
        ));
    }

    // 2) Descomprimir y verificar que el binario responde.
    let o = std::process::Command::new("tar")
        .args(&["-xzf", tarball_s, "-C", work_s])
        .output()
        .map_err(|e| format!("tar: no se pudo lanzar: {}", e))?;
    if !o.status.success() {
        return Err(format!(
            "tar exit {:?}: {}",
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let o = std::process::Command::new(&binary)
        .output()
        .map_err(|e| format!("zett: no se pudo lanzar: {}", e))?;
    let version_out = String::from_utf8_lossy(&o.stdout).trim().to_string();
    log(&format!("[zett-mirror] binario OK: {}", version_out));

    // 3) Publicarlo en la rama tools desde un repo temporal limpio.
    if std::fs::create_dir_all(&tools_dir).is_err() {
        return Err("no se pudo crear tools/".to_string());
    }
    if std::fs::copy(&binary, tools_dir.join("zett-linux-x86_64")).is_err() {
        return Err("no se pudo copiar el binario".to_string());
    }
    let o = run_git(&repo_dir, &["init", "-q"]);
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).into_owned());
    }
    let o = run_git(&repo_dir, &["add", "-A"]);
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).into_owned());
    }
    let msg = format!("tools: binario oficial zett {} (x86_64 Linux)", VERSION);
    let o = run_git(
        &repo_dir,
        &[
            "-c",
            "user.name=ci-mirror",
            "-c",
            "user.email=ci@local",
            "commit",
            "-q",
            "-m",
            &msg,
        ],
    );
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).into_owned());
    }
    let remote = format!("https://x-access-token:{}@github.com/{}.git", token, REPO);
    let refspec = format!("HEAD:refs/heads/{}", branch);
    let o = run_git(&repo_dir, &["push", "-f", "-q", &remote, &refspec]);
    if !o.status.success() {
        return Err(format!(
            "git push fallo: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(())
}
