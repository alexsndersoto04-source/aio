//! Verificación de extremo a extremo de **Moon** (la aplicación), no del
//! lenguaje: levanta un PostgreSQL real en un contenedor Docker, arranca la API
//! con el binario que acaba de compilar `cargo test` y ejecuta
//! `projects/moon/test/e2e.mjs` (467 líneas, ~90 comprobaciones: registro,
//! login, 2FA, refresh, posts, feed, comentarios, likes, follows, mensajería
//! en vivo por WebSocket, notificaciones, subida de imágenes, reportes, admin y
//! seguridad).
//!
//! En un entorno sin Docker, sin Linux o sin un Node con `WebSocket` global, la
//! prueba se OMITE con un aviso para no romper `cargo test` en cualquier
//! máquina. En GitHub Actions sobre Linux no se omite nada: si falta el
//! entorno, la prueba falla en vez de dar una falsa sensación de verde.
//!
//! El resultado se publica siempre en la página del run
//! (`GITHUB_STEP_SUMMARY`, encabezado «Moon E2E») y, si falla, el motivo sale
//! además como anotación (`::error`, legible sin abrir el log del job).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Nombre del contenedor de PostgreSQL que usa la prueba.
const CONTAINER: &str = "moon-e2e-postgres";
/// Puerto del host para PostgreSQL (raro, para no chocar con nada).
const DB_PORT: u16 = 55_432;
/// Puerto del host para la API de Moon.
const API_PORT: u16 = 31_234;
/// Secreto JWT de desarrollo (64 hex, como el que genera `ops/start-api.sh`).
const JWT_SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// Recorte del mensaje que se publica como anotación.
const ANNOTATION_LIMIT: usize = 12000;
/// Versión de Node que se instala desde npm si el Node local es muy viejo.
const NODE_FALLBACK: &str = "node@22";

fn moon_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/moon")
}

fn database_url() -> String {
    format!("postgres://moon:moon@127.0.0.1:{DB_PORT}/moon")
}

fn api_url() -> String {
    format!("http://127.0.0.1:{API_PORT}")
}

// --------------------------------------------------------------------------
// Avisos, fallos y resumen del run.
// --------------------------------------------------------------------------

/// Recorta el texto quedándose con el final (lo más útil de un log).
fn clipped_message(text: &str) -> String {
    if text.len() <= ANNOTATION_LIMIT {
        return text.to_string();
    }
    let cut = text.len() - ANNOTATION_LIMIT;
    let start = text.char_indices().map(|(i, _)| i).find(|i| *i >= cut);
    text[start.unwrap_or(0)..].to_string()
}

/// Escapa el texto para que GitHub lo publique entero en la anotación.
fn annotation_message(text: &str) -> String {
    let percent = text.replace('%', "%25");
    let carriage = percent.replace('\r', "%0D");
    carriage.replace('\n', "%0A")
}

/// Escribe markdown en la página del run (queda como prueba permanente).
fn publish_summary(markdown: &str) {
    println!("{markdown}");
    let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{markdown}");
    }
}

/// Publica una anotación del job (visible sin abrir el log).
fn annotate(title: &str, message: &str) {
    let detail = clipped_message(message);
    println!("::error title={title}::{}", annotation_message(&detail));
}

/// Tamaño máximo que GitHub acepta en el mensaje de una anotación.
const ANNOTATION_CHUNK: usize = 3500;

/// Parte el texto en trozos que quepan en una anotación, cortando por
/// líneas. GitHub rechaza en silencio el mensaje entero si se pasa del
/// límite, y con el log de la API entero hace falta partirlo para poder
/// leerlo desde un run.
fn chunks(text: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut actual = String::new();
    for line in text.lines() {
        if !actual.is_empty() && actual.len() + line.len() + 1 > limit {
            out.push(std::mem::take(&mut actual));
        }
        if !actual.is_empty() {
            actual.push('\n');
        }
        actual.push_str(line);
    }
    if !actual.is_empty() {
        out.push(actual);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Publica un texto largo repartido en varias anotaciones numeradas.
fn annotate_long(title: &str, text: &str) {
    let trozos = chunks(text, ANNOTATION_CHUNK);
    let total = trozos.len();
    for (index, trozo) in trozos.into_iter().enumerate() {
        if total == 1 {
            annotate(title, &trozo);
        } else {
            annotate(&format!("{title} ({}/{total})", index + 1), &trozo);
        }
    }
}

/// Publica el motivo como anotación del job y falla la prueba.
fn fail_with(title: &str, message: &str) -> ! {
    annotate(title, message);
    panic!("{}", clipped_message(message));
}

/// Bloque «Fallos:» del E2E (la lista completa de comprobaciones fallidas).
fn failures_block(text: &str) -> String {
    match text.find("Fallos:") {
        Some(index) => text[index..].to_string(),
        None => tail_of_text(text, 30),
    }
}

/// ¿Estamos en el CI de GitHub sobre Linux, donde esto sí o sí debe correr?
fn must_run() -> bool {
    cfg!(target_os = "linux") && std::env::var("GITHUB_ACTIONS").is_ok()
}

/// Avisa de que la prueba no se pudo ejecutar (no es un fallo salvo en CI Linux).
fn skip_with(reason: &str) {
    if must_run() {
        fail_with(
            "Moon E2E",
            &format!("{reason}\n(el CI de Linux debe poder ejecutar este E2E)"),
        );
    }
    println!("::warning title=Moon E2E::{}", annotation_message(reason));
    println!("Moon E2E omitido: {reason}");
}

// --------------------------------------------------------------------------
// Helpers de procesos.
// --------------------------------------------------------------------------

fn command_of(program: &str, args: &[String]) -> Command {
    let mut command = Command::new(program);
    command.args(args);
    command
}

/// Ejecuta `program args...` y devuelve true si terminó con éxito.
fn runs_ok(program: &str, args: &[String]) -> bool {
    let mut command = command_of(program, args);
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command.status().is_ok_and(|status| status.success())
}

fn output_of(program: &str, args: &[String]) -> Option<String> {
    let output = command_of(program, args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn tail_of_text(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

fn tail_of_file(path: &Path, lines: usize) -> String {
    match fs::read_to_string(path) {
        Ok(text) => tail_of_text(&text, lines),
        Err(_) => format!("(no se pudo leer {})", path.display()),
    }
}

/// Línea `=== RESULTADO: N PASS / M FAIL ===` del E2E, si existe.
fn result_line(text: &str) -> String {
    text.lines()
        .find(|line| line.contains("RESULTADO:"))
        .map(|line| line.trim().trim_matches('=').trim().replace("RESULTADO:", ""))
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "sin línea de resultado".to_string())
}

// --------------------------------------------------------------------------
// Invocación de Node: el E2E usa el `WebSocket` global, que trae Node 22 (por
// defecto), Node 20.10+ / 21 (con `--experimental-websocket`) y cualquier
// versión instalada desde npm (`npx node@22`).
// --------------------------------------------------------------------------

/// Comprueba si `node <args>` expone el `WebSocket` global.
fn node_has_websocket(program: &str, args: &[String]) -> bool {
    let mut probe = args.to_vec();
    probe.push("-e".to_string());
    probe.push("process.stdout.write(typeof WebSocket)".to_string());
    output_of(program, &probe).is_some_and(|text| text == "function")
}

/// Devuelve la forma de invocar Node que sí trae `WebSocket` global.
fn node_invocation() -> Option<Vec<String>> {
    if node_has_websocket("node", &[]) {
        return Some(vec!["node".to_string()]);
    }
    let flagged = vec!["--experimental-websocket".to_string()];
    if node_has_websocket("node", &flagged) {
        return Some(vec!["node".to_string(), "--experimental-websocket".to_string()]);
    }
    let npx = ["--yes".to_string(), NODE_FALLBACK.to_string()];
    if node_has_websocket("npx", &npx) {
        return Some(vec![
            "npx".to_string(),
            "--yes".to_string(),
            NODE_FALLBACK.to_string(),
        ]);
    }
    None
}

fn node_version(node: &[String]) -> String {
    let mut args = node[1..].to_vec();
    args.push("--version".to_string());
    output_of(&node[0], &args).unwrap_or_else(|| "desconocida".to_string())
}

// --------------------------------------------------------------------------
// Recursos que hay que limpiar aunque la prueba falle.
// --------------------------------------------------------------------------

/// Borra el contenedor de PostgreSQL al salir, incluso si la prueba falla.
struct DatabaseGuard;

impl Drop for DatabaseGuard {
    fn drop(&mut self) {
        let mut command = command_of("docker", &["rm".to_string(), "-f".to_string()]);
        command.arg(CONTAINER);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        let _ = command.status();
    }
}

/// Mata la API al salir, incluso si la prueba falla.
struct ServerGuard(Option<Child>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// --------------------------------------------------------------------------
// PostgreSQL y API de Moon.
// --------------------------------------------------------------------------

fn docker_is_available() -> bool {
    runs_ok("docker", &["info".to_string()])
}

fn docker_args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

fn start_postgres() {
    let mut remove = command_of("docker", &docker_args(&["rm", "-f"]));
    remove.arg(CONTAINER);
    remove.stdout(Stdio::null());
    remove.stderr(Stdio::null());
    let _ = remove.status();

    let host_port = format!("127.0.0.1:{DB_PORT}:5432");
    let args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        CONTAINER.to_string(),
        "-e".to_string(),
        "POSTGRES_USER=moon".to_string(),
        "-e".to_string(),
        "POSTGRES_PASSWORD=moon".to_string(),
        "-e".to_string(),
        "POSTGRES_DB=moon".to_string(),
        "-p".to_string(),
        host_port,
        "postgres:16".to_string(),
    ];
    let mut command = command_of("docker", &args);
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    if !command.status().is_ok_and(|status| status.success()) {
        fail_with("Moon E2E", "no se pudo levantar el contenedor postgres:16");
    }
}

fn database_is_ready() -> bool {
    runs_ok("docker", &docker_args(&["exec", CONTAINER, "pg_isready", "-U", "moon"]))
}

fn wait_for_database() {
    let deadline = Instant::now() + Duration::from_secs(180);
    while !database_is_ready() {
        if Instant::now() > deadline {
            fail_with("Moon E2E", "PostgreSQL no respondió en 180 s");
        }
        sleep(Duration::from_secs(2));
    }
}

fn start_api(moon: &Path, log_path: &Path) -> Child {
    let log = File::create(log_path).expect("crear el log de la API");
    let stdout = log.try_clone().expect("clonar el log de la API");
    let mut command = Command::new(env!("CARGO_BIN_EXE_titan"));
    command.arg("run");
    command.arg("src/main.titan");
    command.current_dir(moon);
    command.env("DATABASE_URL", database_url());
    command.env("JWT_SECRET", JWT_SECRET);
    command.env("PORT", API_PORT.to_string());
    command.env("CORS_ORIGIN", "http://localhost:5173,http://127.0.0.1:5173");
    command.env("PUBLIC_BASE_URL", api_url());
    // Si algo revienta dentro de la API, queremos el rastro completo en el log
    // (que es lo que acaba en la anotación del run).
    command.env("RUST_BACKTRACE", "1");
    command.env("RUST_LIB_BACKTRACE", "1");
    // Traza por petición en el log: si algo se cuelga se ve dónde.
    command.env("MOON_TRACE", "1");
    command.stdout(Stdio::from(stdout));
    command.stderr(Stdio::from(log));
    command.spawn().expect("arrancar la API de Moon")
}

fn wait_for_api(log_path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if TcpStream::connect(("127.0.0.1", API_PORT)).is_ok() {
            return;
        }
        if Instant::now() > deadline {
            let tail = tail_of_file(log_path, 60);
            fail_with(
                "Moon E2E",
                &format!("la API de Moon no abrió el puerto {API_PORT}.\n{tail}"),
            );
        }
        sleep(Duration::from_millis(500));
    }
}

fn run_e2e(moon: &Path, node: &[String], log_path: &Path) {
    let mut command = command_of(&node[0], &node[1..]);
    command.arg("test/e2e.mjs");
    command.current_dir(moon);
    command.env("API_BASE", api_url());
    command.env("MOON_LOG", log_path);
    let output = command.output().expect("ejecutar test/e2e.mjs");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let ok = output.status.success();

    let title = if ok {
        "## Moon E2E (aplicación real) ✅"
    } else {
        "## Moon E2E (aplicación real) ❌"
    };
    let header = format!(
        "{title}\n\
         - Base de datos: PostgreSQL 16 en Docker, migraciones al arrancar\n\
         - API: binario `titan` recién compilado, puerto {API_PORT}\n\
         - Node: `{}` ({})\n\
         - Resultado: **{}**",
        node.join(" "),
        node_version(node),
        result_line(&stdout)
    );
    let tail = tail_of_text(&stdout, 40);

    if ok {
        let short = tail_of_text(&stdout, 12);
        publish_summary(&format!("{header}\n\n```\n{short}\n```"));
        println!("Moon E2E correcto");
        return;
    }

    let fallos = failures_block(&stdout);
    publish_summary(&format!("{header}\n\n```\n{fallos}\n```"));
    annotate(
        "Moon E2E · resumen",
        &format!("{header}\n\n{fallos}"),
    );
    // El log completo, en trozos: GitHub corta el mensaje de una anotación
    // a ~4 KB, así que un log largo hay que repartirlo o no se ve.
    annotate_long("Moon E2E · log API", &tail_of_file(log_path, 2000));
    let mut message = format!("la suite E2E de Moon falló ({})", output.status);
    message.push_str("\n--- E2E (últimas 40 líneas) ---\n");
    message.push_str(&tail);
    message.push_str("\n--- E2E stderr (últimas 15 líneas) ---\n");
    message.push_str(&tail_of_text(&stderr, 15));
    fail_with("Moon E2E", &message);
}

#[test]
fn moon_arranca_contra_postgres_y_pasa_el_e2e() {
    if !cfg!(target_os = "linux") && std::env::var("MOON_E2E_FORCE").is_err() {
        skip_with("solo se ejecuta en Linux (o con MOON_E2E_FORCE=1 y Docker activo)");
        return;
    }
    if !docker_is_available() {
        skip_with("no hay Docker disponible en esta máquina");
        return;
    }
    let Some(node) = node_invocation() else {
        skip_with("no encontré un Node con WebSocket global (hace falta Node 22 o superior)");
        return;
    };
    let moon = moon_path();
    assert!(
        moon.join("src/main.titan").is_file(),
        "no encuentro projects/moon/src/main.titan"
    );

    let _database = DatabaseGuard;
    start_postgres();
    wait_for_database();

    let log_path = std::env::temp_dir().join("moon-e2e-api.log");
    let server = start_api(&moon, &log_path);
    let _server = ServerGuard(Some(server));
    wait_for_api(&log_path);
    run_e2e(&moon, &node, &log_path);
}
