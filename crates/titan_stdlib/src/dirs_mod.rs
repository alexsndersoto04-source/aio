//! Well-known user and system directories (`std::dirs::*`) backed by the
//! `dirs` crate. Behavior on Termux/Android matches Linux XDG conventions.
//!
//! Every helper returns the path as a `String` (empty when the platform does
//! not expose the directory) so `.titan` code never has to deal with `Option`.

use std::path::{Path, PathBuf};

fn opt(path: Option<std::path::PathBuf>) -> String {
    path.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn home() -> String {
    opt(dirs::home_dir())
}
pub fn config() -> String {
    opt(dirs::config_dir())
}
pub fn cache() -> String {
    opt(dirs::cache_dir())
}
pub fn data() -> String {
    opt(dirs::data_dir())
}
pub fn data_local() -> String {
    opt(dirs::data_local_dir())
}
pub fn state() -> String {
    opt(dirs::state_dir())
}
pub fn executable() -> String {
    opt(dirs::executable_dir())
}
pub fn runtime() -> String {
    opt(dirs::runtime_dir())
}
pub fn preference() -> String {
    opt(dirs::preference_dir())
}

// Common user "content" folders.
// ---------------------------------------------------------------------------
// Carpetas de usuario (escritorio, documentos…) según `user-dirs.dirs`.
//
// Mismas reglas que `dirs_sys::user_dir` 0.4.1 (lo que usa `dirs` 5.0.1 en
// Linux), pero sin sus dos panics con líneas mal formadas: una clave
// `XDG_DIR` (el crate corta `&key[4..3]`) o un valor que es solo `"` (corta
// `&value[1..0]`). En los dos casos la línea se ignora, como cualquier otra
// línea que no se entiende. Una sola línea así hacía caer todo el programa
// al pedir cualquier carpeta de usuario.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios"))))]
fn user_dir(name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let bytes = std::fs::read(config.join("user-dirs.dirs")).unwrap_or_default();
    parse_user_dir(&home, name, &bytes)
}

#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "ios")))))]
fn user_dir(name: &str) -> Option<PathBuf> {
    match name {
        "DESKTOP" => dirs::desktop_dir(),
        "DOCUMENTS" => dirs::document_dir(),
        "DOWNLOAD" => dirs::download_dir(),
        "PICTURES" => dirs::picture_dir(),
        "MUSIC" => dirs::audio_dir(),
        "VIDEOS" => dirs::video_dir(),
        "PUBLICSHARE" => dirs::public_dir(),
        _ => None,
    }
}

// Sin uso en macOS/Windows (allí `user_dir` delega en el crate).
#[allow(dead_code)]
fn parse_user_dir(home: &Path, name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let wanted = format!("XDG_{name}_DIR");
    for line in bytes.split(|byte| *byte == b'\n') {
        let Some(eq) = line.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if trim_blank(&line[..eq]) != wanted.as_bytes() {
            continue;
        }
        // xdg-user-dirs-update escribe el valor entre comillas dobles.
        let value = trim_blank(&line[eq + 1..]);
        if value.len() < 2 || value[0] != b'"' || value[value.len() - 1] != b'"' {
            continue;
        }
        let value = &value[1..value.len() - 1];
        // "$HOME/" solo = carpeta desactivada.
        let (relative, rest) = if value == b"$HOME/" {
            continue;
        } else if let Some(rest) = value.strip_prefix(b"$HOME/") {
            (true, rest)
        } else if value.starts_with(b"/") {
            (false, value)
        } else {
            continue;
        };
        let path = bytes_to_path(shell_unescape(rest));
        return Some(if relative { home.join(path) } else { path });
    }
    None
}

#[cfg(unix)]
#[allow(dead_code)]
fn bytes_to_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn bytes_to_path(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[allow(dead_code)]
fn trim_blank(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count();
    let bytes = &bytes[start..];
    let end = bytes
        .iter()
        .rev()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count();
    &bytes[..bytes.len() - end]
}

/// Quita las barras invertidas de las comillas dobles del shell.
#[allow(dead_code)]
fn shell_unescape(escaped: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(escaped.len());
    let mut bytes = escaped.iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            if let Some(next) = bytes.next() {
                out.push(next);
            }
        } else {
            out.push(byte);
        }
    }
    out
}

pub fn desktop() -> String {
    opt(user_dir("DESKTOP"))
}
pub fn documents() -> String {
    opt(user_dir("DOCUMENTS"))
}
pub fn downloads() -> String {
    opt(user_dir("DOWNLOAD"))
}
pub fn pictures() -> String {
    opt(user_dir("PICTURES"))
}
pub fn music() -> String {
    opt(user_dir("MUSIC"))
}
pub fn videos() -> String {
    opt(user_dir("VIDEOS"))
}
pub fn public() -> String {
    opt(user_dir("PUBLICSHARE"))
}

/// Temporary directory (always present).
pub fn temp() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

/// Current working directory of the running process, or empty on error.
pub fn current() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_and_current_are_always_available() {
        assert!(!temp().is_empty());
        assert!(!current().is_empty());
    }

    #[test]
    fn home_is_reported_when_env_is_set() {
        if std::env::var_os("HOME").is_some() || std::env::var_os("USERPROFILE").is_some() {
            assert!(!home().is_empty());
        }
    }

    #[test]
    fn user_dirs_file_is_parsed_like_dirs_sys() {
        let home = Path::new("/home/ana");
        let file = b"# comentario\nXDG_DESKTOP_DIR=\"$HOME/Escritorio\"\n  XDG_MUSIC_DIR = \"/m\\\\\\\"x\" \nXDG_VIDEOS_DIR=\"$HOME/\"\nXDG_PICTURES_DIR=relativo\n";
        assert_eq!(parse_user_dir(home, "DESKTOP", file), Some(PathBuf::from("/home/ana/Escritorio")));
        assert_eq!(parse_user_dir(home, "MUSIC", file), Some(PathBuf::from("/m\\\"x")));
        assert_eq!(parse_user_dir(home, "VIDEOS", file), None);
        assert_eq!(parse_user_dir(home, "PICTURES", file), None);
        assert_eq!(parse_user_dir(home, "DOCUMENTS", file), None);
    }

    #[test]
    fn malformed_user_dirs_lines_are_skipped_not_panics() {
        // dirs-sys 0.4.1 hacía panic con estas dos líneas.
        let home = Path::new("/home/ana");
        let file = b"XDG_DIR=\"x\"\nXDG_DESKTOP_DIR=\"\nXDG_DESKTOP_DIR=\"/escritorio\"\n";
        assert_eq!(parse_user_dir(home, "DESKTOP", file), Some(PathBuf::from("/escritorio")));
        assert_eq!(parse_user_dir(home, "MUSIC", file), None);
    }

    #[test]
    fn all_helpers_return_without_panicking() {
        let _ = (
            config(),
            cache(),
            data(),
            data_local(),
            state(),
            executable(),
            runtime(),
            preference(),
            desktop(),
            documents(),
            downloads(),
            pictures(),
            music(),
            videos(),
            public(),
        );
    }
}
