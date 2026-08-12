//! Well-known user and system directories (`std::dirs::*`) backed by the
//! `dirs` crate. Behavior on Termux/Android matches Linux XDG conventions.
//!
//! Every helper returns the path as a `String` (empty when the platform does
//! not expose the directory) so `.titan` code never has to deal with `Option`.

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
pub fn desktop() -> String {
    opt(dirs::desktop_dir())
}
pub fn documents() -> String {
    opt(dirs::document_dir())
}
pub fn downloads() -> String {
    opt(dirs::download_dir())
}
pub fn pictures() -> String {
    opt(dirs::picture_dir())
}
pub fn music() -> String {
    opt(dirs::audio_dir())
}
pub fn videos() -> String {
    opt(dirs::video_dir())
}
pub fn public() -> String {
    opt(dirs::public_dir())
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
