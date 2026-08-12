//! Filesystem and stream utilities with explicit errors.

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

pub fn read_file(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path)
}
pub fn read_bytes(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    fs::read(path)
}
pub fn read_limited(path: impl AsRef<Path>, max_bytes: u64) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds configured limit",
        ));
    }
    let mut output = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut output)?;
    if output.len() as u64 > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds configured limit",
        ));
    }
    Ok(output)
}
pub fn read_lines(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    BufReader::new(fs::File::open(path)?).lines().collect()
}
pub fn write_file(path: impl AsRef<Path>, content: &str) -> io::Result<()> {
    fs::write(path, content)
}
pub fn write_bytes(path: impl AsRef<Path>, content: &[u8]) -> io::Result<()> {
    fs::write(path, content)
}
pub fn append(path: impl AsRef<Path>, content: &[u8]) -> io::Result<()> {
    use std::fs::OpenOptions;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(content)
}
pub fn atomic_write(path: impl AsRef<Path>, content: &[u8]) -> io::Result<()> {
    let path = path.as_ref();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "titan".into());
    let mut attempt = 0u32;
    let temporary = loop {
        let candidate = parent.join(format!(
            ".{}.{}.{}.tmp",
            file_name,
            std::process::id(),
            attempt
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(content)?;
                file.sync_all()?;
                break candidate;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                attempt = attempt.checked_add(1).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "unable to allocate temporary path",
                    )
                })?;
            }
            Err(error) => return Err(error),
        }
    };
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}
pub fn exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().exists()
}
pub fn is_file(path: impl AsRef<Path>) -> bool {
    path.as_ref().is_file()
}
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    path.as_ref().is_dir()
}
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(path)
}
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    fs::remove_file(path)
}
pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    fs::remove_dir_all(path)
}
pub fn list_dir(path: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let mut entries: Vec<_> = fs::read_dir(path)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<io::Result<_>>()?;
    entries.sort();
    Ok(entries)
}
pub fn walk(path: impl AsRef<Path>, max_depth: usize) -> io::Result<Vec<PathBuf>> {
    fn visit(path: &Path, depth: usize, max: usize, output: &mut Vec<PathBuf>) -> io::Result<()> {
        if depth > max {
            return Ok(());
        }
        let mut entries: Vec<_> = fs::read_dir(path)?.collect::<io::Result<_>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let value = entry.path();
            output.push(value.clone());
            if entry.file_type()?.is_dir() {
                visit(&value, depth + 1, max, output)?;
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(path.as_ref(), 0, max_depth, &mut output)?;
    Ok(output)
}
pub fn file_size(path: impl AsRef<Path>) -> io::Result<u64> {
    fs::metadata(path).map(|m| m.len())
}
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    fs::copy(from, to)
}
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    fs::rename(from, to)
}
pub fn stdin_line() -> io::Result<String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim_end_matches(['\r', '\n']).into())
}
pub fn stdout_line(value: &str) {
    println!("{value}");
}
