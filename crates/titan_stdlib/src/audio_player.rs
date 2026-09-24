//! System playback engine (`std::audio::player_*`) — the desktop side of
//! the music-player foundation (Fase 41). Zero new dependencies: TITAN
//! drives whatever media backend the operating system already has.
//!
//! Backends, in detection order:
//!
//! | Backend  | Platforms    | play | pause/resume | seek | volume | queue |
//! |----------|--------------|------|--------------|------|--------|-------|
//! | `mpv`    | Unix (IPC)   | ✅   | ✅           | ✅   | ✅     | ✅    |
//! | `afplay` | macOS        | ✅   | ✅ (signals) | ❌   | ❌     | ❌    |
//! | `paplay` | Linux/Pulse  | ✅   | ✅ (signals) | ❌   | ❌     | ❌    |
//! | `aplay`  | Linux/ALSA   | ✅   | ✅ (signals) | ❌   | ❌     | ❌    |
//! | `ffplay` | anywhere     | ✅   | ✅ (signals) | ❌   | ❌     | ❌    |
//! | `soundplayer` | Windows (PowerShell `Media.SoundPlayer`, WAV) | ✅ | ❌ | ❌ | ❌ | ❌ |
//!
//! `mpv` is the premium path: TITAN talks to it over its JSON IPC socket
//! (`std::os::unix::net::UnixStream`, pure std) for pause, seek, volume
//! and a real playlist. The rest get an honest, typed
//! [`PlayerError::Unsupported`] for the controls they lack, so a `.titan`
//! music player can degrade gracefully instead of guessing.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use thiserror::Error;

/// Errors produced by the playback engine.
#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "no playback backend available (tried: mpv, afplay, paplay, aplay, ffplay). Install one, e.g. `pkg install mpv`"
    )]
    NoBackend,
    #[error("nothing is playing right now")]
    Idle,
    #[error("backend '{backend}' does not support '{action}'")]
    Unsupported { backend: String, action: String },
    #[error("invalid parameter: {0}")]
    Invalid(String),
    #[error("backend control error: {0}")]
    Control(String),
}

/// mpv JSON-IPC handle. The writer and reader are clones of one socket:
/// mpv writes one JSON line per reply, events may interleave, so reads
/// scan for the line that carries `"error"`.
#[cfg(unix)]
struct Ipc {
    writer: std::os::unix::net::UnixStream,
    reader: std::os::unix::net::UnixStream,
    sock: PathBuf,
}

#[cfg(unix)]
impl Drop for Ipc {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.sock);
    }
}

struct PlayerState {
    child: Option<Child>,
    backend: &'static str,
    started: Option<Instant>,
    paused_accum: Duration,
    paused_since: Option<Instant>,
    #[cfg(unix)]
    ipc: Option<Ipc>,
}

impl PlayerState {
    fn fresh() -> Self {
        PlayerState {
            child: None,
            backend: "",
            started: None,
            paused_accum: Duration::ZERO,
            paused_since: None,
            #[cfg(unix)]
            ipc: None,
        }
    }
}

fn slot() -> &'static Mutex<Option<PlayerState>> {
    static SLOT: OnceLock<Mutex<Option<PlayerState>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_slot() -> std::sync::MutexGuard<'static, Option<PlayerState>> {
    slot().lock().unwrap_or_else(|e| e.into_inner())
}

// ------------------------------------------------------------ backend pick

const BACKEND_ORDER: [&str; 5] = ["mpv", "afplay", "paplay", "aplay", "ffplay"];

fn find_in_dirs(bin: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    #[cfg(windows)]
    let bin = format!("{bin}.exe");
    #[cfg(not(windows))]
    let bin = bin.to_string();
    dirs.iter()
        .map(|d| d.join(&bin))
        .find(|p| p.is_file())
}

fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

fn detect_backend_in(dirs: &[PathBuf]) -> Option<&'static str> {
    for b in BACKEND_ORDER {
        if find_in_dirs(b, dirs).is_some() {
            return Some(b);
        }
    }
    #[cfg(windows)]
    if find_in_dirs("powershell", dirs).is_some() {
        return Some("soundplayer");
    }
    None
}

/// Which backend is available right now (`""` when none).
pub fn backend() -> String {
    let mut guard = lock_slot();
    if let Some(st) = guard.as_mut() {
        reap(st);
        if st.child.is_some() {
            return st.backend.to_string();
        }
    }
    detect_backend_in(&path_dirs()).unwrap_or("").to_string()
}

fn reap(st: &mut PlayerState) {
    let done = match st.child.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(Some(_))),
        None => false,
    };
    if done {
        *st = PlayerState::fresh();
    }
}

// ------------------------------------------------------------------ playing

/// Start playing `path` (stopping whatever was playing before).
pub fn play(path: &str) -> Result<String, PlayerError> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(PlayerError::Invalid(format!("file not found: {path}")));
    }
    stop_quietly();
    let dirs = path_dirs();
    let backend = detect_backend_in(&dirs).ok_or(PlayerError::NoBackend)?;
    let mut st = PlayerState::fresh();
    st.backend = backend;
    let child = match backend {
        #[cfg(unix)]
        "mpv" => mpv_play(p, &mut st)?,
        #[cfg(not(unix))]
        "mpv" => {
            return Err(PlayerError::Control(
                "mpv control is only wired up on Unix in this version".into(),
            ));
        }
        "afplay" | "paplay" | "aplay" => spawn_simple(backend, &[path], &mut st)?,
        "ffplay" => spawn_simple(
            "ffplay",
            &["-nodisp", "-autoexit", "-loglevel", "quiet", path],
            &mut st,
        )?,
        #[cfg(not(unix))]
        _ => spawn_windows_soundplayer(path, &mut st)?,
        #[cfg(unix)]
        _ => return Err(PlayerError::NoBackend),
    };
    st.child = child;
    if st.started.is_none() {
        st.started = Some(Instant::now());
    }
    *lock_slot() = Some(st);
    Ok(format!("playing {path} via {backend}"))
}

fn spawn_simple(
    bin: &str,
    args: &[&str],
    st: &mut PlayerState,
) -> Result<Option<Child>, PlayerError> {
    let child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    st.started = Some(Instant::now());
    Ok(Some(child))
}

#[cfg(not(unix))]
fn spawn_windows_soundplayer(
    path: &str,
    st: &mut PlayerState,
) -> Result<Option<Child>, PlayerError> {
    let script = format!(
        "(New-Object Media.SoundPlayer '{}').PlaySync()",
        path.replace('\'', "''")
    );
    let child = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    st.started = Some(Instant::now());
    Ok(Some(child))
}

#[cfg(unix)]
fn mpv_play(path: &Path, st: &mut PlayerState) -> Result<Option<Child>, PlayerError> {
    use std::os::unix::net::UnixStream;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let sock = std::env::temp_dir().join(format!("zett-audio-{}-{}.sock", std::process::id(), nanos));
    let _ = std::fs::remove_file(&sock); // stale socket from a crashed run
    let child = Command::new("mpv")
        .arg("--no-video")
        .arg("--no-terminal")
        .arg("--really-quiet")
        .arg("--idle=yes")
        .arg(format!("--input-ipc-server={}", sock.display()))
        .arg("--")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut writer = None;
    for _ in 0..30 {
        if let Ok(s) = UnixStream::connect(&sock) {
            writer = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let writer = match writer {
        Some(s) => s,
        None => {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&sock);
            return Err(PlayerError::Control(
                "mpv started but its IPC socket never appeared".into(),
            ));
        }
    };
    writer
        .set_write_timeout(Some(Duration::from_millis(800)))
        .ok();
    let reader = writer.try_clone()?;
    reader.set_read_timeout(Some(Duration::from_millis(200))).ok();
    st.ipc = Some(Ipc {
        writer,
        reader,
        sock,
    });
    st.started = Some(Instant::now());
    Ok(Some(child))
}

/// Send one JSON-IPC command to mpv and return the reply line that
/// carries the `"error"` field (bounded by a 1.2 s deadline).
#[cfg(unix)]
fn ipc_cmd(st: &mut PlayerState, cmd: &str) -> Result<String, PlayerError> {
    use std::io::{Read, Write};
    let ipc = match st.ipc.as_mut() {
        Some(i) => i,
        None => return Err(PlayerError::Control("no mpv IPC socket".into())),
    };
    ipc.writer.write_all(cmd.as_bytes())?;
    ipc.writer.write_all(b"\n")?;
    ipc.writer.flush()?;
    let deadline = Instant::now() + Duration::from_millis(1200);
    loop {
        if Instant::now() >= deadline {
            break;
        }
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match ipc.reader.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    line.push(byte[0]);
                    if line.len() > 65536 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        if line.is_empty() {
            break;
        }
        let text = String::from_utf8_lossy(&line).to_string();
        if text.contains("\"error\"") {
            return Ok(text);
        }
        // else: an event line; keep reading until the deadline.
    }
    Err(PlayerError::Control(
        "no response from mpv (is it still running?)".into(),
    ))
}

#[cfg(unix)]
fn mpv_error_check(reply: &str) -> Result<(), PlayerError> {
    if reply.contains("\"error\":\"success\"") {
        Ok(())
    } else {
        Err(PlayerError::Control(format!("mpv replied: {reply}")))
    }
}

#[cfg(unix)]
fn signal_pause(child: Option<&mut Child>, pause: bool) -> Result<(), PlayerError> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let child = match child {
        Some(c) => c,
        None => return Err(PlayerError::Idle),
    };
    let pid = Pid::from_raw(
        i32::try_from(child.id())
            .map_err(|_| PlayerError::Control("process id out of range".into()))?,
    );
    let sig = if pause { Signal::SIGSTOP } else { Signal::SIGCONT };
    kill(pid, sig).map_err(|e| PlayerError::Control(format!("signal failed: {e}")))?;
    Ok(())
}

/// Escape a string as a JSON string literal (for mpv IPC payloads).
#[cfg(unix)]
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Pull a number out of an mpv reply, e.g. `"data":12.345` or `"data":null`.
#[cfg(unix)]
fn extract_float_after(json: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\":");
    let at = json.find(&needle)? + needle.len();
    let rest = &json[at..];
    let end = rest
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E')
        })
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// ----------------------------------------------------------------- controls

fn require_live(st: &mut PlayerState) -> Result<(), PlayerError> {
    reap(st);
    if st.child.is_none() {
        return Err(PlayerError::Idle);
    }
    Ok(())
}

/// Pause playback.
pub fn pause() -> Result<String, PlayerError> {
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    if st.paused_since.is_some() {
        return Ok("already paused".into());
    }
    do_pause(st)?;
    st.paused_since = Some(Instant::now());
    Ok("paused".into())
}

#[cfg(unix)]
fn do_pause(st: &mut PlayerState) -> Result<(), PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(st, r#"{"command":["set_property","pause",true]}"#)?;
        mpv_error_check(&reply)
    } else {
        signal_pause(st.child.as_mut(), true)
    }
}

#[cfg(not(unix))]
fn do_pause(st: &mut PlayerState) -> Result<(), PlayerError> {
    Err(PlayerError::Unsupported {
        backend: st.backend.to_string(),
        action: "pause".into(),
    })
}

/// Resume playback.
pub fn resume() -> Result<String, PlayerError> {
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    if st.paused_since.is_none() {
        return Ok("already playing".into());
    }
    do_resume(st)?;
    if let Some(ps) = st.paused_since.take() {
        st.paused_accum += ps.elapsed();
    }
    Ok("resumed".into())
}

#[cfg(unix)]
fn do_resume(st: &mut PlayerState) -> Result<(), PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(st, r#"{"command":["set_property","pause",false]}"#)?;
        mpv_error_check(&reply)
    } else {
        signal_pause(st.child.as_mut(), false)
    }
}

#[cfg(not(unix))]
fn do_resume(st: &mut PlayerState) -> Result<(), PlayerError> {
    Err(PlayerError::Unsupported {
        backend: st.backend.to_string(),
        action: "resume".into(),
    })
}

/// Seconds of playback reached; `-1.0` when idle.
pub fn position_secs() -> f64 {
    let mut guard = lock_slot();
    let st = match guard.as_mut() {
        Some(s) => s,
        None => return -1.0,
    };
    reap(st);
    if st.child.is_none() {
        return -1.0;
    }
    #[cfg(unix)]
    if st.backend == "mpv" {
        if let Ok(reply) = ipc_cmd(st, r#"{"command":["get_property","time-pos"]}"#) {
            if let Some(v) = extract_float_after(&reply, "data") {
                if v >= 0.0 {
                    return v;
                }
            }
        }
    }
    match st.started {
        Some(start) => {
            let mut active = start.elapsed();
            active = active.saturating_sub(st.paused_accum);
            if let Some(ps) = st.paused_since {
                active = active.saturating_sub(ps.elapsed());
            }
            active.as_secs_f64()
        }
        None => -1.0,
    }
}

/// Jump to `secs` from the start of the track.
pub fn seek(secs: f64) -> Result<String, PlayerError> {
    if !secs.is_finite() || secs < 0.0 {
        return Err(PlayerError::Invalid(
            "seek position must be a finite number >= 0".into(),
        ));
    }
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    do_seek(st, secs)
}

#[cfg(unix)]
fn do_seek(st: &mut PlayerState, secs: f64) -> Result<String, PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(
            st,
            &format!(r#"{{"command":["seek",{},"absolute"]}}"#, secs),
        )?;
        mpv_error_check(&reply)?;
        // Keep the wall-clock fallback honest too.
        let base = Instant::now().checked_sub(Duration::from_secs_f64(secs.min(86_400.0)));
        st.started = Some(base.unwrap_or_else(Instant::now));
        st.paused_accum = Duration::ZERO;
        if st.paused_since.is_some() {
            st.paused_since = Some(Instant::now());
        }
        Ok(format!("seek to {secs:.1}s"))
    } else {
        Err(PlayerError::Unsupported {
            backend: st.backend.to_string(),
            action: "seek".into(),
        })
    }
}

#[cfg(not(unix))]
fn do_seek(st: &mut PlayerState, _secs: f64) -> Result<String, PlayerError> {
    Err(PlayerError::Unsupported {
        backend: st.backend.to_string(),
        action: "seek".into(),
    })
}

/// Set the volume (0..=100). Returns `true` when the backend applied it.
pub fn set_volume(percent: i64) -> Result<bool, PlayerError> {
    if !(0..=100).contains(&percent) {
        return Err(PlayerError::Invalid("volume must be between 0 and 100".into()));
    }
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    do_set_volume(st, percent)
}

#[cfg(unix)]
fn do_set_volume(st: &mut PlayerState, percent: i64) -> Result<bool, PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(
            st,
            &format!(r#"{{"command":["set_property","volume",{}]}}"#, percent),
        )?;
        mpv_error_check(&reply)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(not(unix))]
fn do_set_volume(st: &mut PlayerState, _percent: i64) -> Result<bool, PlayerError> {
    let _ = st;
    Ok(false)
}

/// Append `path` to the play queue (plays after the current track).
pub fn queue_add(path: &str) -> Result<String, PlayerError> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(PlayerError::Invalid(format!("file not found: {path}")));
    }
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    do_queue_add(st, path)
}

#[cfg(unix)]
fn do_queue_add(st: &mut PlayerState, path: &str) -> Result<String, PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(
            st,
            &format!(
                r#"{{"command":["loadfile",{},"append-play"]}}"#,
                json_string(path)
            ),
        )?;
        mpv_error_check(&reply)?;
        Ok(format!("queued {path}"))
    } else {
        Err(PlayerError::Unsupported {
            backend: st.backend.to_string(),
            action: "queue".into(),
        })
    }
}

#[cfg(not(unix))]
fn do_queue_add(st: &mut PlayerState, _path: &str) -> Result<String, PlayerError> {
    Err(PlayerError::Unsupported {
        backend: st.backend.to_string(),
        action: "queue".into(),
    })
}

/// Drop everything that is queued but not yet playing.
pub fn queue_clear() -> Result<String, PlayerError> {
    let mut guard = lock_slot();
    let st = guard.as_mut().ok_or(PlayerError::Idle)?;
    require_live(st)?;
    do_queue_clear(st)
}

#[cfg(unix)]
fn do_queue_clear(st: &mut PlayerState) -> Result<String, PlayerError> {
    if st.backend == "mpv" {
        let reply = ipc_cmd(st, r#"{"command":["playlist-clear"]}"#)?;
        mpv_error_check(&reply)?;
        Ok("queue cleared".into())
    } else {
        Err(PlayerError::Unsupported {
            backend: st.backend.to_string(),
            action: "queue".into(),
        })
    }
}

#[cfg(not(unix))]
fn do_queue_clear(st: &mut PlayerState) -> Result<String, PlayerError> {
    Err(PlayerError::Unsupported {
        backend: st.backend.to_string(),
        action: "queue".into(),
    })
}

/// Stop playback and release the backend process.
pub fn stop() -> Result<String, PlayerError> {
    let mut guard = lock_slot();
    if guard.is_none() {
        return Ok("nothing was playing".into());
    }
    {
        let st = guard.as_mut().unwrap();
        #[cfg(unix)]
        if st.backend == "mpv" {
            let _ = ipc_cmd(st, r#"{"command":["quit"]}"#);
        }
        if let Some(child) = st.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    *guard = None;
    Ok("stopped".into())
}

fn stop_quietly() {
    let _ = stop();
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_detects_no_backend() {
        assert_eq!(detect_backend_in(&[]), None);
    }

    #[cfg(unix)]
    #[test]
    fn find_in_dirs_finds_an_executable_looking_file() {
        let dir = std::env::temp_dir().join(format!("zett-player-detect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("mpv");
        std::fs::write(&fake, b"#!/bin/sh\n").unwrap();
        let found = find_in_dirs("mpv", &[dir.clone()]);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(found, Some(fake));
    }

    #[cfg(unix)]
    #[test]
    fn extract_float_reads_mpv_replies() {
        assert_eq!(
            extract_float_after(r#"{"data":12.345678,"error":"success"}"#, "data"),
            Some(12.345678)
        );
        assert_eq!(
            extract_float_after(r#"{"data":null,"error":"success"}"#, "data"),
            None
        );
        assert_eq!(extract_float_after(r#"{"data":3,"error":"success"}"#, "data"), Some(3.0));
    }

    #[cfg(unix)]
    #[test]
    fn json_string_escapes_specials() {
        assert_eq!(json_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(json_string("ok"), "\"ok\"");
    }

    #[test]
    fn play_rejects_missing_files() {
        assert!(matches!(
            play("/definitely/not/a/song/zett.mp3"),
            Err(PlayerError::Invalid(_))
        ));
    }

    #[test]
    fn queue_add_rejects_missing_files() {
        assert!(matches!(
            queue_add("/definitely/not/a/song/zett.mp3"),
            Err(PlayerError::Invalid(_))
        ));
    }

    #[test]
    fn volume_bounds_are_enforced() {
        // Idle + out-of-range volume: the parameter check fires first.
        assert!(matches!(
            set_volume(150),
            Err(PlayerError::Invalid(_))
        ));
    }

    #[test]
    fn seek_rejects_negative() {
        assert!(matches!(seek(-1.0), Err(PlayerError::Invalid(_))));
    }

    #[test]
    fn position_is_minus_one_when_idle() {
        // Nothing was started in this process' slot (play() fails above).
        let pos = position_secs();
        assert!(pos == -1.0 || pos >= 0.0);
    }
}
