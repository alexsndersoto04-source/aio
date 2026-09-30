//! TAR and ZIP archive read/write (`std::archive::*`).
//!
//! * `.tar` handled by the `tar` crate.
//! * `.zip` handled by the `zip` crate (deflate feature).
//!
//! In-memory API for now: `.titan` code passes raw bytes and gets back an
//! array of `{ name, bytes }` maps, or vice versa. Streaming file APIs can
//! be added on top later.

use std::collections::BTreeMap;
use std::io::{Cursor, Error as IoError, ErrorKind, Read, Write};

use flate2::Crc;
use miniz_oxide::inflate::core::{decompress, inflate_flags, DecompressorOxide};
use miniz_oxide::inflate::TINFLStatus;
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
    if name.starts_with('/') || name.contains("..") || name.contains('\\') || name.contains('\0') {
        return Err(ArchiveError::UnsafeName(name.into()));
    }
    Ok(())
}

// ---------------- TAR ----------------

pub fn tar_pack(entries: &[ArchiveEntry]) -> Result<Vec<u8>, ArchiveError> {
    for entry in entries {
        safe_name(&entry.name)?;
    }
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

/// Largest real size (4 GiB) accepted for a GNU sparse tar entry.
const SPARSE_LIMIT: u64 = 1 << 32;

pub fn tar_unpack(data: &[u8]) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let mut archive = tar::Archive::new(data);
    let mut out = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        safe_name(&path)?;
        // A GNU sparse entry declares its real size in the header and is
        // mostly zeros that get materialized in memory: refuse absurd sizes
        // instead of trying to allocate them.
        if entry.header().entry_type().is_gnu_sparse() && entry.size() > SPARSE_LIMIT {
            return Err(ArchiveError::Io(IoError::new(
                ErrorKind::Other,
                "sparse entry too large to unpack in memory",
            )));
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        out.push(ArchiveEntry { name: path, bytes });
    }
    Ok(out)
}

// ---------------- ZIP ----------------

pub fn zip_pack(entries: &[ArchiveEntry]) -> Result<Vec<u8>, ArchiveError> {
    for entry in entries {
        safe_name(&entry.name)?;
    }
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
        // `by_index` validates the entry (encryption, method, local header).
        let (name, method, crc, compressed_size) = {
            let file = archive.by_index(index)?;
            (
                file.name().to_string(),
                file.compression(),
                file.crc32(),
                file.compressed_size(),
            )
        };
        safe_name(&name)?;
        // Skip directory entries (common in zips).
        if name.ends_with('/') {
            continue;
        }
        // The payload is decoded here instead of through the zip crate's
        // reader: that reader returned `Ok` with partial or invented data for
        // a truncated deflate stream (the part decoded so far), a stream with
        // no final block, or a back-reference before the start of the output
        // (filled with zeros), as long as the CRC happened to match.
        let mut raw = Vec::new();
        archive.by_index_raw(index)?.read_to_end(&mut raw)?;
        let bytes = match method {
            zip::CompressionMethod::Stored => {
                if (raw.len() as u64) < compressed_size {
                    return Err(ArchiveError::Io(ErrorKind::UnexpectedEof.into()));
                }
                raw
            }
            zip::CompressionMethod::Deflated => inflate_raw(&raw)?,
            _ => {
                return Err(ArchiveError::Zip(zip::result::ZipError::UnsupportedArchive(
                    "Compression method not supported",
                )))
            }
        };
        let mut sum = Crc::new();
        sum.update(&bytes);
        if sum.sum() != crc {
            return Err(ArchiveError::Io(IoError::new(
                ErrorKind::InvalidData,
                "Invalid checksum",
            )));
        }
        out.push(ArchiveEntry { name, bytes });
    }
    Ok(out)
}

/// Inflates a raw deflate stream strictly: it must end with a final block
/// and never refer to bytes before the start of the output.
fn inflate_raw(input: &[u8]) -> Result<Vec<u8>, ArchiveError> {
    let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut out: Vec<u8> = vec![0; input.len().saturating_mul(2).max(64)];
    let mut decomp = Box::<DecompressorOxide>::default();
    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    loop {
        let (status, in_used, out_used) =
            decompress(&mut decomp, &input[in_pos..], &mut out, out_pos, flags);
        in_pos += in_used;
        out_pos += out_used;
        match status {
            TINFLStatus::Done => {
                out.truncate(out_pos);
                return Ok(out);
            }
            TINFLStatus::HasMoreOutput => {
                let new_len = out.len().saturating_mul(2);
                out.resize(new_len, 0);
            }
            TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
                return Err(ArchiveError::Io(ErrorKind::UnexpectedEof.into()));
            }
            _ => {
                return Err(ArchiveError::Io(IoError::new(
                    ErrorKind::InvalidInput,
                    "corrupt deflate stream",
                )))
            }
        }
    }
}

/// Metadata-only listing (no payload) for cheap indexing.
pub fn zip_list(data: &[u8]) -> Result<Vec<String>, ArchiveError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data))?;
    Ok((0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect())
}

/// Convenience: build the map form used at the VM boundary.
pub fn entries_to_maps(entries: Vec<ArchiveEntry>) -> Vec<BTreeMap<String, EntryValue>> {
    entries
        .into_iter()
        .map(|entry| {
            let mut map: BTreeMap<String, EntryValue> = BTreeMap::new();
            map.insert("name".into(), EntryValue::Text(entry.name));
            map.insert("bytes".into(), EntryValue::Bytes(entry.bytes));
            map
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum EntryValue {
    Text(String),
    Bytes(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<ArchiveEntry> {
        vec![
            ArchiveEntry {
                name: "hello.txt".into(),
                bytes: b"hola mundo".to_vec(),
            },
            ArchiveEntry {
                name: "docs/readme.md".into(),
                bytes: b"# titan".to_vec(),
            },
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
        assert_eq!(
            names,
            vec!["hello.txt".to_string(), "docs/readme.md".to_string()]
        );
    }

    #[test]
    fn rejects_zip_slip_style_names() {
        let bad = vec![ArchiveEntry {
            name: "../etc/passwd".into(),
            bytes: b"x".to_vec(),
        }];
        assert!(tar_pack(&bad).is_err());
        assert!(zip_pack(&bad).is_err());
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // Broken zips (same as selfhost/tests/native/archive_defectos.titan)
    // whose CRC matches what the zip crate's reader used to return.
    #[test]
    fn zip_unpack_rejects_broken_payloads() {
        let cases: [(&str, &str); 7] = [
            (
                "504b030414000000080000002100823555c80e000000cc01000005000000612e747874cbc8cf4954c82dcd4bc9d751c800504b0102140314000000080000002100823555c80e000000cc010000050000000000000000000000000000000000612e747874504b0506000000000100010033000000310000000000",
                "archive I/O error: unexpected end of file",
            ),
            (
                "504b0304140000000800000021009c1538de040000000400000005000000622e7478744b044200504b01021403140000000800000021009c1538de0400000004000000050000000000000000000000000000000000622e747874504b0506000000000100010033000000270000000000",
                "archive I/O error: corrupt deflate stream",
            ),
            (
                "504b03041400000008000000210000000000000000000000000005000000632e747874504b0102140314000000080000002100000000000000000000000000050000000000000000000000000000000000632e747874504b0506000000000100010033000000230000000000",
                "archive I/O error: unexpected end of file",
            ),
            (
                "504b030414000000000000002100c241243540420f000300000005000000642e747874616263504b0102140314000000000000002100c241243540420f0003000000050000000000000000000000000000000000642e747874504b0506000000000100010033000000260000000000",
                "archive I/O error: unexpected end of file",
            ),
            (
                "504b03041400000008000000210000000000040000000000000005000000652e747874ffffffff504b0102140314000000080000002100000000000400000000000000050000000000000000000000000000000000652e747874504b0506000000000100010033000000270000000000",
                "archive I/O error: corrupt deflate stream",
            ),
            (
                "504b030414000000080000002100d20400001c000000cc01000005000000662e747874cbc8cf4954c82dcd4bc9d751c800b1433c431cfd20cc51e1a1250c00504b0102140314000000080000002100d20400001c000000cc010000050000000000000000000000000000000000662e747874504b05060000000001000100330000003f0000000000",
                "archive I/O error: Invalid checksum",
            ),
            (
                "504b03041400000008000000210088f9a06f06000000040000000100000078cbc8cf490400504b010214031400000008000000210088f9a06f06000000040000000100180000000000000000000000000000007801001c000000000000000000000000000000000000000000504b0506000000000100010047000000250000000000",
                "archive I/O error: unexpected end of file",
            ),
        ];
        for (hex, want) in cases {
            let err = zip_unpack(&unhex(hex)).unwrap_err();
            assert_eq!(err.to_string(), want);
            // Listing only reads the central directory.
            assert_eq!(zip_list(&unhex(hex)).unwrap().len(), 1);
        }
    }
}
