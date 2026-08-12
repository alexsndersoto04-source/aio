//! Safe extraction of signed Titan package archives.
use flate2::read::GzDecoder;
use std::collections::HashSet;
use std::path::{Component, Path};
use thiserror::Error;
#[derive(Debug, Clone)]
pub struct ExtractionLimits {
    pub files: usize,
    pub total_bytes: u64,
    pub file_bytes: u64,
}
impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            files: 10_000,
            total_bytes: 128 * 1024 * 1024,
            file_bytes: 32 * 1024 * 1024,
        }
    }
}
#[derive(Error, Debug)]
pub enum ArchiveError {
    #[error("archive I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsafe archive path or entry type")]
    UnsafeEntry,
    #[error("archive exceeds extraction limits")]
    Limit,
    #[error("package does not contain root Titan.toml")]
    MissingManifest,
    #[error("destination already exists")]
    DestinationExists,
}
pub fn extract(
    archive: &Path,
    destination: &Path,
    limits: &ExtractionLimits,
) -> Result<(), ArchiveError> {
    if destination.exists() {
        return Err(ArchiveError::DestinationExists);
    }
    let parent = destination.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".tpkg-extract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir(&staging)?;
    let result = extract_into(archive, &staging, limits);
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if !staging.join("Titan.toml").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(ArchiveError::MissingManifest);
    }
    match std::fs::rename(&staging, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(error.into())
        }
    }
}
fn extract_into(
    archive: &Path,
    destination: &Path,
    limits: &ExtractionLimits,
) -> Result<(), ArchiveError> {
    let file = std::fs::File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut paths = HashSet::new();
    let mut files = 0;
    let mut total = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if !safe(&path) || !paths.insert(path.clone()) {
            return Err(ArchiveError::UnsafeEntry);
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(destination.join(path))?;
            continue;
        }
        if !kind.is_file() {
            return Err(ArchiveError::UnsafeEntry);
        }
        files += 1;
        if files > limits.files {
            return Err(ArchiveError::Limit);
        }
        let size = entry.size();
        if size > limits.file_bytes || total.saturating_add(size) > limits.total_bytes {
            return Err(ArchiveError::Limit);
        }
        total += size;
        let output = destination.join(path);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?
        }
        let mut target = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        let copied = std::io::copy(&mut entry, &mut target)?;
        if copied != size {
            return Err(ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "archive size mismatch",
            )));
        }
    }
    Ok(())
}
fn safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    #[test]
    fn extracts_regular_package_atomically() {
        let root = std::env::temp_dir().join(format!("titan-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("p.tpkg");
        let encoder = GzEncoder::new(
            std::fs::File::create(&archive).unwrap(),
            Compression::default(),
        );
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        let data = b"[package]\nname='demo'\nversion='1.0.0'\nedition='2021'\n";
        header.set_size(data.len() as u64);
        header.set_cksum();
        builder
            .append_data(&mut header, "Titan.toml", &data[..])
            .unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_size(11);
        header.set_cksum();
        builder
            .append_data(&mut header, "src/lib.titan", &b"fn demo(){}"[..])
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap();
        let destination = root.join("out");
        extract(&archive, &destination, &ExtractionLimits::default()).unwrap();
        assert!(destination.join("src/lib.titan").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}
