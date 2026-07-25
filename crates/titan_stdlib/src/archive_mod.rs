//! TAR and ZIP archive read/write (`std::archive::*`).
//!
//! * `.tar` handled by the `tar` crate.
//! * `.zip` handled by the `zip` crate (deflate feature).
//!
//! In-memory API for now: `.titan` code passes raw bytes and gets back an
//! array of `{ name, bytes }` maps, or vice versa. Streaming file APIs can
//! be added on top later.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("archive I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive entry name '{0}' contains a path traversal sequence")]
    UnsafeName(String),
}

/// One file inside an archive.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

fn safe_name(name: &str) -> Result<(), ArchiveError> {
    // Guard against zip-slip / tar-slip: reject absolute paths and `..`.
    if name.starts_with('/')
        || name.contains("..")
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(ArchiveError::UnsafeName(name.into()));
    }
    Ok(())
}

// ---------------- TAR ----------------

pub fn tar_pack(entries: &[ArchiveEntry]) -> Result<Vec<u8>, ArchiveError> {
    for entry in entries { safe_name(&entry.name)?; }
    let mut builder = tar::Builder::new(Vec::new());
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(entry.bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, &entry.name, Cursor::new(&entry.bytes))?;
    }
    Ok(builder.into_inner()?)
}

pub fn tar_unpack(data: &[u8]) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut archive = tar::Archive::new(data);
    let mut out = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        safe_name(&path)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        out.push(ArchiveEntry { name: path, bytes });
    }
    Ok(out)
}

// ---------------- ZIP ----------------

pub fn zip_pack(entries: &[ArchiveEntry]) -> Result<Vec<u8>, ArchiveError> {
    for entry in entries { safe_name(&entry.name)?; }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for entry in entries {
        writer.start_file(&entry.name, options)?;
        writer.write_all(&entry.bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}

pub fn zip_unpack(data: &[u8]) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data))?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().to_string();
        safe_name(&name)?;
        // Skip directory entries (common in zips).
        if name.ends_with('/') { continue; }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        out.push(ArchiveEntry { name, bytes });
    }
    Ok(out)
}

/// Metadata-only listing (no payload) for cheap indexing.
pub fn zip_list(data: &[u8]) -> Result<Vec<String>, ArchiveError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data))?;
    Ok((0..archive.len()).filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string())).collect())
}

/// Convenience: build the map form used at the VM boundary.
pub fn entries_to_maps(entries: Vec<ArchiveEntry>) -> Vec<BTreeMap<String, EntryValue>> {
    entries.into_iter().map(|entry| {
        let mut map: BTreeMap<String, EntryValue> = BTreeMap::new();
        map.insert("name".into(), EntryValue::Text(entry.name));
        map.insert("bytes".into(), EntryValue::Bytes(entry.bytes));
        map
    }).collect()
}

#[derive(Debug, Clone)]
pub enum EntryValue { Text(String), Bytes(Vec<u8>) }

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<ArchiveEntry> {
        vec![
            ArchiveEntry { name: "hello.txt".into(), bytes: b"hola mundo".to_vec() },
            ArchiveEntry { name: "docs/readme.md".into(), bytes: b"# titan".to_vec() },
        ]
    }

    #[test]
    fn tar_round_trip_preserves_entries() {
        let bytes = tar_pack(&sample()).unwrap();
        let back = tar_unpack(&bytes).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "hello.txt");
        assert_eq!(back[0].bytes, b"hola mundo");
        assert_eq!(back[1].name, "docs/readme.md");
    }

    #[test]
    fn zip_round_trip_preserves_entries() {
        let bytes = zip_pack(&sample()).unwrap();
        let back = zip_unpack(&bytes).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "hello.txt");
        assert_eq!(back[1].bytes, b"# titan");
    }

    #[test]
    fn zip_list_reports_names_without_reading_bodies() {
        let bytes = zip_pack(&sample()).unwrap();
        let names = zip_list(&bytes).unwrap();
        assert_eq!(names, vec!["hello.txt".to_string(), "docs/readme.md".to_string()]);
    }

    #[test]
    fn rejects_zip_slip_style_names() {
        let bad = vec![ArchiveEntry { name: "../etc/passwd".into(), bytes: b"x".to_vec() }];
        assert!(tar_pack(&bad).is_err());
        assert!(zip_pack(&bad).is_err());
    }
}
