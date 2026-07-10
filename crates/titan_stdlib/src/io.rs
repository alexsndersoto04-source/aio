//! Titan Stdlib — I/O.

use std::fs;
use std::io;

pub fn read_file(path: &str) -> io::Result<String> { fs::read_to_string(path) }
pub fn write_file(path: &str, c: &str) -> io::Result<()> { fs::write(path, c) }
pub fn exists(path: &str) -> bool { std::path::Path::new(path).exists() }
pub fn is_file(path: &str) -> bool { std::path::Path::new(path).is_file() }
pub fn is_dir(path: &str) -> bool { std::path::Path::new(path).is_dir() }
pub fn create_dir(path: &str) -> io::Result<()> { fs::create_dir_all(path) }
pub fn remove_file(path: &str) -> io::Result<()> { fs::remove_file(path) }
pub fn remove_dir(path: &str) -> io::Result<()> { fs::remove_dir_all(path) }
pub fn list_dir(path: &str) -> io::Result<Vec<String>> {
    Ok(fs::read_dir(path)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
}
pub fn file_size(path: &str) -> io::Result<u64> { fs::metadata(path).map(|m| m.len()) }
pub fn copy(src: &str, dst: &str) -> io::Result<u64> { fs::copy(src, dst) }
pub fn rename(from: &str, to: &str) -> io::Result<()> { fs::rename(from, to) }
pub fn stdin_line() -> io::Result<String> { let mut l = String::new(); io::stdin().read_line(&mut l)?; Ok(l.trim_end().into()) }
pub fn stdout_line(s: &str) { println!("{}", s); }