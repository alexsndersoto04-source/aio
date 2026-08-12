//! Cross-platform path manipulation and filesystem metadata.

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

pub fn join(base: impl AsRef<Path>, child: impl AsRef<Path>) -> PathBuf {
    base.as_ref().join(child)
}
pub fn parent(path: impl AsRef<Path>) -> Option<PathBuf> {
    path.as_ref().parent().map(Path::to_path_buf)
}
pub fn file_name(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .file_name()
        .map(|s| s.to_string_lossy().into())
}
pub fn stem(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .file_stem()
        .map(|s| s.to_string_lossy().into())
}
pub fn extension(path: impl AsRef<Path>) -> Option<String> {
    path.as_ref()
        .extension()
        .map(|s| s.to_string_lossy().into())
}
pub fn with_extension(path: impl AsRef<Path>, value: &str) -> PathBuf {
    path.as_ref().with_extension(value)
}
pub fn absolute(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(normalize(path))
    } else {
        Ok(normalize(std::env::current_dir()?.join(path)))
    }
}
pub fn canonical(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::fs::canonicalize(path)
}
pub fn normalize(path: impl AsRef<Path>) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    output.push("..");
                }
            }
            Component::Normal(part) => output.push(part),
            Component::RootDir => output.push(Component::RootDir.as_os_str()),
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
        }
    }
    output
}
pub fn components(path: impl AsRef<Path>) -> Vec<OsString> {
    path.as_ref()
        .components()
        .map(|c| c.as_os_str().to_owned())
        .collect()
}
pub fn is_within(path: impl AsRef<Path>, root: impl AsRef<Path>) -> io::Result<bool> {
    let path = canonical(path)?;
    let root = canonical(root)?;
    Ok(path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manipulates_paths() {
        let value = normalize("a/./b/../c.txt");
        assert_eq!(value, PathBuf::from("a/c.txt"));
        assert_eq!(stem(&value).as_deref(), Some("c"));
        assert_eq!(extension(&value).as_deref(), Some("txt"));
    }
}
