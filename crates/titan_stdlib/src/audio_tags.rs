//! Track metadata and music-library scanning (`std::audio::tags`,
//! `std::audio::duration`, `std::audio::scan_library`) — 100% pure Rust,
//! zero new dependencies. This is the foundation for building a music
//! player in TITAN itself (Fase 41).
//!
//! What it reads, per format:
//!
//! * **MP3** — ID3v2.2/2.3/2.4 text frames (TIT2, TPE1, TALB, TPE2, TCON,
//!   TYER/TDRC, TRCK) + cover detection (APIC/PIC); ID3v1 fallback tag;
//!   duration from the Xing/Info frame counter (VBR) or a CBR bitrate
//!   estimate; sample rate / channels / bitrate from the first frame header.
//! * **WAV** — `fmt ` (rate, channels, bits), `data` (exact duration) and
//!   `LIST`/`INFO` chunks (INAM title, IART artist, IPRD album, IGNR genre,
//!   ICRD year, ITRK track); embedded `id3 `/`ID3 ` chunks too.
//! * **FLAC** — STREAMINFO (exact duration, rate, channels) and
//!   VORBIS_COMMENT (tags), PICTURE (cover detection).
//! * **OGG Vorbis / Opus** — identification header (rate, channels), the
//!   comment header (Vorbis comments / OpusTags) and the last page granule
//!   position (exact duration).
//!
//! Everything is bounds-checked with hard iteration guards, so a corrupt
//! file can never make the VM spin or explode: at worst the track reports
//! fewer fields than it should.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use thiserror::Error;

/// Errors produced while reading track metadata.
#[derive(Debug, Error)]
pub enum TagsError {
    #[error("audio I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or corrupt audio file: {0}")]
    Format(String),
    #[error("invalid parameter: {0}")]
    Invalid(String),
}

/// Everything a music player needs to know about one track.
#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub path: String,
    /// `"mp3"`, `"wav"`, `"flac"`, `"ogg"` or `"opus"`.
    pub format: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub track_number: Option<u32>,
    /// Seconds. `0.0` when the format cannot tell.
    pub duration_secs: f64,
    /// `0` when unknown.
    pub sample_rate: u32,
    /// `0` when unknown.
    pub channels: u16,
    /// kbps; `0` when unknown (WAV/FLAC/OGG report 0, it is meaningless).
    pub bitrate_kbps: u32,
    /// Embedded cover art was seen (APIC/PIC/PICTURE).
    pub has_cover: bool,
}

/// File-extension filter used by [`scan_library`].
const AUDIO_EXTS: [&str; 6] = ["mp3", "wav", "flac", "ogg", "oga", "opus"];

const SCAN_MAX_FILES: usize = 5000;
const SCAN_MAX_DEPTH: usize = 8;

/// How much of a file we read for metadata: 1 MiB covers the tag area of
/// every realistic file (even a 10-cover-art MP3), 128 KiB of the tail
/// covers ID3v1 and the last OGG page.
const HEAD_MAX: usize = 1024 * 1024;
const TAIL_MAX: usize = 128 * 1024;

// ---------------------------------------------------------------- reading

/// Read the metadata of one audio file.
pub fn read_track(path: &str) -> Result<TrackInfo, TagsError> {
    let (head, tail, file_len) = read_head_tail(path)?;
    if head.is_empty() {
        return Err(TagsError::Format(path.to_string()));
    }
    let kind = detect(&head);
    let (tags, dur, rate, ch, kbps, format_name) = match kind {
        "wav" => {
            let (t, d, r, c) = parse_wav(&head);
            (t, d, r, c, 0, "wav")
        }
        "flac" => {
            let (t, d, r, c) = parse_flac(&head);
            (t, d, r, c, 0, "flac")
        }
        "ogg" => parse_ogg(&head, &tail),
        _ => parse_mp3(&head, &tail, file_len),
    };
    if kind.is_empty() {
        return Err(TagsError::Format(format!("unsupported audio file: {path}")));
    }
    Ok(TrackInfo {
        path: path.to_string(),
        format: format_name.to_string(),
        title: tags.title,
        artist: tags.artist,
        album: tags.album,
        album_artist: tags.album_artist,
        genre: tags.genre,
        year: tags.year,
        track_number: tags.track,
        duration_secs: dur,
        sample_rate: rate,
        channels: ch,
        bitrate_kbps: kbps,
        has_cover: tags.has_cover,
    })
}

/// Duration of one audio file in seconds (`0.0` when unknown).
pub fn duration_secs(path: &str) -> Result<f64, TagsError> {
    Ok(read_track(path)?.duration_secs)
}

/// Walk `dir` recursively and read the metadata of every supported audio
/// file, sorted by path. Unreadable files are skipped (best effort), so a
/// single broken track never breaks the library scan.
pub fn scan_library(dir: &str) -> Result<Vec<TrackInfo>, TagsError> {
    if !Path::new(dir).is_dir() {
        return Err(TagsError::Invalid(format!("'{dir}' is not a directory")));
    }
    let mut out = Vec::new();
    walk(Path::new(dir), 0, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<TrackInfo>) {
    if depth > SCAN_MAX_DEPTH || out.len() >= SCAN_MAX_FILES {
        return;
    }
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return, // unreadable directory: skip silently
    };
    for entry in rd.flatten() {
        if out.len() >= SCAN_MAX_FILES {
            return;
        }
        let p = entry.path();
        if p.is_dir() {
            walk(&p, depth + 1, out);
            continue;
        }
        let is_audio = p
            .extension()
            .map(|e| {
                let low = e.to_string_lossy().to_ascii_lowercase();
                AUDIO_EXTS.contains(&low.as_ref())
            })
            .unwrap_or(false);
        if is_audio {
            if let Ok(info) = read_track(&p.to_string_lossy()) {
                out.push(info);
            }
        }
    }
}

fn read_head_tail(path: &str) -> Result<(Vec<u8>, Vec<u8>, u64), TagsError> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let head_len = std::cmp::min(HEAD_MAX as u64, len) as usize;
    let mut head = vec![0u8; head_len];
    file.read_exact(&mut head)?;
    let mut tail = Vec::new();
    if len > head_len as u64 {
        let tail_len = std::cmp::min(TAIL_MAX as u64, len - head_len as u64) as usize;
        tail = vec![0u8; tail_len];
        file.seek(SeekFrom::End(-(tail_len as i64)))?;
        file.read_exact(&mut tail)?;
    }
    Ok((head, tail, len))
}

fn detect(head: &[u8]) -> &'static str {
    if head.len() >= 12 && head.starts_with(b"RIFF") && &head[8..12] == b"WAVE" {
        "wav"
    } else if head.starts_with(b"fLaC") {
        "flac"
    } else if head.starts_with(b"OggS") {
        "ogg"
    } else if head.starts_with(b"ID3") || is_mpeg_sync(head) {
        "mp3"
    } else {
        ""
    }
}

fn is_mpeg_sync(head: &[u8]) -> bool {
    head.len() >= 2 && head[0] == 0xFF && (head[1] & 0xE0) == 0xE0
}

// ------------------------------------------------------- shared little helpers

fn u16be(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([s[0], s[1]]))
}

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn u32be(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64le(b: &[u8], at: usize) -> Option<u64> {
    let s = b.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

/// ID3 "syncsafe" integer: 4 bytes, 7 bits each, top bit always zero.
fn syncsafe(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(
        ((s[0] as u32) << 21) | ((s[1] as u32) << 14) | ((s[2] as u32) << 7) | (s[3] as u32),
    )
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn clean(raw: String) -> Option<String> {
    let t = raw.trim_matches('\0').trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

fn parse_track_number(raw: &str) -> Option<u32> {
    let first = raw.split('/').next()?;
    first.trim().parse().ok()
}

fn utf16_decode(bytes: &[u8], little: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode_text(enc: u8, bytes: &[u8]) -> Option<String> {
    let s = match enc {
        0 => bytes.iter().map(|&b| b as char).collect::<String>(),
        3 => String::from_utf8_lossy(bytes).to_string(),
        1 => {
            if bytes.starts_with(&[0xFF, 0xFE]) {
                utf16_decode(&bytes[2..], true)
            } else if bytes.starts_with(&[0xFE, 0xFF]) {
                utf16_decode(&bytes[2..], false)
            } else {
                utf16_decode(bytes, true)
            }
        }
        2 => utf16_decode(bytes, false),
        _ => return None,
    };
    Some(s)
}

/// Strip ID3v2 unsynchronisation (`FF 00` -> `FF`).
fn de_unsync(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0usize;
    while i < data.len() {
        out.push(data[i]);
        if data[i] == 0xFF && i + 1 < data.len() && data[i + 1] == 0x00 {
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

// ------------------------------------------------------------------ ID3 tags

#[derive(Debug, Default, Clone)]
pub(crate) struct Id3Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub track: Option<u32>,
    pub has_cover: bool,
}

impl Id3Tags {
    fn set(&mut self, id: &str, value: String) {
        match id {
            "TIT2" | "TT2" => self.title = clean(value),
            "TPE1" | "TP1" => self.artist = clean(value),
            "TALB" | "TAL" => self.album = clean(value),
            "TPE2" | "TP2" => self.album_artist = clean(value),
            "TCON" | "TCO" => self.genre = clean(value),
            "TYER" | "TYE" | "TDRC" => {
                let v = clean(value);
                if v.is_some() || self.year.is_none() {
                    self.year = v;
                }
            }
            "TRCK" | "TRK" => self.track = parse_track_number(&value),
            _ => {}
        }
    }

    fn set_vorbis(&mut self, key: &str, value: String) {
        match key {
            "TITLE" => self.title = clean(value),
            "ARTIST" => self.artist = clean(value),
            "ALBUM" => self.album = clean(value),
            "ALBUMARTIST" | "ALBUM ARTIST" => self.album_artist = clean(value),
            "GENRE" => self.genre = clean(value),
            "DATE" | "YEAR" => self.year = clean(value),
            "TRACKNUMBER" => self.track = parse_track_number(&value),
            _ => {}
        }
    }
}

fn merge_tags(dst: &mut Id3Tags, src: Id3Tags) {
    if dst.title.is_none() {
        dst.title = src.title;
    }
    if dst.artist.is_none() {
        dst.artist = src.artist;
    }
    if dst.album.is_none() {
        dst.album = src.album;
    }
    if dst.album_artist.is_none() {
        dst.album_artist = src.album_artist;
    }
    if dst.genre.is_none() {
        dst.genre = src.genre;
    }
    if dst.year.is_none() {
        dst.year = src.year;
    }
    if dst.track.is_none() {
        dst.track = src.track;
    }
    dst.has_cover |= src.has_cover;
}

fn decode_text_frame(content: &[u8]) -> Option<String> {
    let (enc, text) = content.split_first()?;
    let s = decode_text(*enc, text)?;
    clean(s)
}

/// Parse the frame area of an ID3v2 tag (everything after the 10-byte tag
/// header). `tag_flags` is the flag byte from the tag header.
pub(crate) fn parse_id3v2_frames(buf: &[u8], major: u8, tag_flags: u8) -> Id3Tags {
    let mut tags = Id3Tags::default();
    if !(2..=4).contains(&major) {
        return tags;
    }
    let id_len = if major == 2 { 3 } else { 4 };
    let hdr_len = if major == 2 { 6 } else { 10 };
    let end = buf.len();
    let mut pos = 0usize;
    // Extended header (v2.3: 4-byte plain size that excludes itself;
    // v2.4: syncsafe size that includes itself).
    if major != 2 && (tag_flags & 0x40) != 0 {
        if major == 4 {
            match syncsafe(buf, 0) {
                Some(sz) => pos = sz as usize,
                None => return tags,
            }
        } else {
            match u32be(buf, 0) {
                Some(sz) => pos = 4 + sz as usize,
                None => return tags,
            }
        }
    }
    let mut guard = 0usize;
    while pos + hdr_len <= end && guard < 4096 {
        guard += 1;
        let id = String::from_utf8_lossy(&buf[pos..pos + id_len]).to_string();
        if !id.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
            break;
        }
        let (fsize, fflags) = if major == 2 {
            (
                ((buf[pos + 3] as usize) << 16)
                    | ((buf[pos + 4] as usize) << 8)
                    | (buf[pos + 5] as usize),
                0u8,
            )
        } else if major == 4 {
            (syncsafe(buf, pos + 4).unwrap_or(0) as usize, buf[pos + 9])
        } else {
            (u32be(buf, pos + 4).unwrap_or(0) as usize, buf[pos + 9])
        };
        // Compressed or encrypted frames are skipped (we do not inflate).
        let broken_frame = (major == 3 && (fflags & 0xC0) != 0)
            || (major == 4 && (fflags & 0x0C) != 0);
        let fstart = pos + hdr_len;
        if fsize == 0 || fstart.checked_add(fsize).map(|e| e > end).unwrap_or(true) {
            break;
        }
        pos = fstart + fsize;
        if broken_frame {
            continue;
        }
        let mut content = buf[fstart..fstart + fsize].to_vec();
        if major == 4 && (fflags & 0x01) != 0 && content.len() > 4 {
            content = content.split_off(4); // data-length indicator
        }
        if major == 4 && (fflags & 0x02) != 0 {
            content = de_unsync(&content);
        }
        if id == "APIC" || id == "PIC" {
            tags.has_cover = true;
        } else if id.starts_with('T') {
            if let Some(text) = decode_text_frame(&content) {
                tags.set(&id, text);
            }
        }
    }
    tags
}

/// Parse an ID3v2 tag that includes its full header ("ID3"...), e.g. from
/// an `id3 ` chunk inside a WAV file.
fn parse_id3v2_whole(data: &[u8]) -> Id3Tags {
    if data.len() < 10 || !data.starts_with(b"ID3") {
        return Id3Tags::default();
    }
    let major = data[3];
    let flags = data[5];
    let tag_size = syncsafe(data, 6).unwrap_or(0) as usize;
    let end = std::cmp::min(10usize.checked_add(tag_size).unwrap_or(data.len()), data.len());
    let raw = &data[10..end];
    let buf = if (flags & 0x80) != 0 {
        de_unsync(raw)
    } else {
        raw.to_vec()
    };
    parse_id3v2_frames(&buf, major, flags)
}

/// The classic 80 ID3v1 genres (spec table).
const ID3V1_GENRES: [&str; 80] = [
    "Blues",
    "Classic Rock",
    "Country",
    "Dance",
    "Disco",
    "Funk",
    "Grunge",
    "Hip-Hop",
    "Jazz",
    "Metal",
    "New Age",
    "Oldies",
    "Other",
    "Pop",
    "R&B",
    "Rap",
    "Reggae",
    "Rock",
    "Techno",
    "Industrial",
    "Alternative",
    "Ska",
    "Death Metal",
    "Pranks",
    "Soundtrack",
    "Euro-Techno",
    "Ambient",
    "Trip-Hop",
    "Vocal",
    "Jazz+Funk",
    "Fusion",
    "Trance",
    "Classical",
    "Instrumental",
    "Acid",
    "House",
    "Game",
    "Sound Clip",
    "Gospel",
    "Noise",
    "Alt Rock",
    "Bass",
    "Soul",
    "Punk",
    "Space",
    "Meditative",
    "Instrumental Pop",
    "Instrumental Rock",
    "Ethnic",
    "Gothic",
    "Darkwave",
    "Techno-Industrial",
    "Electronic",
    "Pop-Folk",
    "Eurodance",
    "Dream",
    "Southern Rock",
    "Comedy",
    "Cult",
    "Gangsta",
    "Top 40",
    "Christian Rap",
    "Pop/Funk",
    "Jungle",
    "Native American",
    "Cabaret",
    "New Wave",
    "Psychedelic",
    "Rave",
    "Showtunes",
    "Trailer",
    "Lo-Fi",
    "Tribal",
    "Acid Punk",
    "Acid Jazz",
    "Polka",
    "Retro",
    "Musical",
    "Rock & Roll",
    "Hard Rock",
];

fn parse_id3v1(tail: &[u8]) -> Id3Tags {
    let mut tags = Id3Tags::default();
    if tail.len() < 128 || !tail[tail.len() - 128..].starts_with(b"TAG") {
        return tags;
    }
    let t = &tail[tail.len() - 128..];
    let get = |r: &[u8]| -> Option<String> {
        let cut = r.iter().position(|&b| b == 0).unwrap_or(r.len());
        clean(String::from_utf8_lossy(&r[..cut]).to_string())
    };
    tags.title = get(&t[3..33]);
    tags.artist = get(&t[33..63]);
    tags.album = get(&t[63..93]);
    tags.year = get(&t[93..97]);
    if t[125] == 0 && t[126] != 0 {
        tags.track = Some(t[126] as u32);
    }
    let genre_byte = t[127] as usize;
    if genre_byte < ID3V1_GENRES.len() {
        tags.genre = Some(ID3V1_GENRES[genre_byte].to_string());
    }
    tags
}

// ---------------------------------------------------------------------- MP3

const MP3_BITRATES_V1_L1: [u16; 15] = [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448];
const MP3_BITRATES_V1_L2: [u16; 15] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384];
const MP3_BITRATES_V1_L3: [u16; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const MP3_BITRATES_V2_L1: [u16; 15] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256];
const MP3_BITRATES_V2: [u16; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
const MP3_RATES_V1: [u16; 3] = [44100, 48000, 32000];
const MP3_RATES_V2: [u16; 3] = [22050, 24000, 16000];
const MP3_RATES_V25: [u16; 3] = [11025, 12000, 8000];

struct Mp3Frame {
    kbps: u32,
    rate: u32,
    channels: u16,
    samples_per_frame: u32,
    frame_len: usize,
    header_at: usize,
    is_v1: bool,
}

fn parse_mp3_frame(buf: &[u8], at: usize) -> Option<Mp3Frame> {
    if at.checked_add(4)? > buf.len() {
        return None;
    }
    let b0 = buf[at];
    let b1 = buf[at + 1];
    let b2 = buf[at + 2];
    let b3 = buf[at + 3];
    if b0 != 0xFF || (b1 & 0xE0) != 0xE0 {
        return None;
    }
    let version_bits = (b1 >> 3) & 0x03; // 11 = MPEG1, 10 = MPEG2, 00 = MPEG2.5
    let layer_bits = (b1 >> 1) & 0x03; // 01 = Layer III, 10 = II, 11 = I
    if version_bits == 0x01 || layer_bits == 0x00 {
        return None;
    }
    let br_idx = (b2 >> 4) as usize;
    let sr_idx = ((b2 >> 2) & 0x03) as usize;
    if br_idx == 0 || br_idx >= 15 || sr_idx >= 3 {
        return None;
    }
    let is_v1 = version_bits == 0x03;
    let layer = match layer_bits {
        0x03 => 1u8,
        0x02 => 2u8,
        _ => 3u8,
    };
    let kbps = if is_v1 {
        match layer {
            1 => MP3_BITRATES_V1_L1[br_idx],
            2 => MP3_BITRATES_V1_L2[br_idx],
            _ => MP3_BITRATES_V1_L3[br_idx],
        }
    } else if layer == 1 {
        MP3_BITRATES_V2_L1[br_idx]
    } else {
        MP3_BITRATES_V2[br_idx]
    } as u32;
    let rate = match version_bits {
        0x03 => MP3_RATES_V1[sr_idx],
        0x02 => MP3_RATES_V2[sr_idx],
        _ => MP3_RATES_V25[sr_idx],
    } as u32;
    if rate == 0 || kbps == 0 {
        return None;
    }
    let samples_per_frame: u32 = if is_v1 {
        if layer == 1 {
            384
        } else {
            1152
        }
    } else if layer == 1 {
        384
    } else if layer == 2 {
        1152
    } else {
        576
    };
    let pad = ((b2 >> 1) & 0x01) as usize;
    let frame_len = if layer == 1 {
        (12 * kbps as usize * 1000 / rate as usize + pad) * 4
    } else if is_v1 || layer == 2 {
        144 * kbps as usize * 1000 / rate as usize + pad
    } else {
        72 * kbps as usize * 1000 / rate as usize + pad
    };
    if frame_len < 24 {
        return None;
    }
    let channels: u16 = if (b3 >> 6) == 0x03 { 1 } else { 2 };
    Some(Mp3Frame {
        kbps,
        rate,
        channels,
        samples_per_frame,
        frame_len,
        header_at: at,
        is_v1,
    })
}

fn find_first_frame(buf: &[u8], from: usize) -> Option<Mp3Frame> {
    let start = from.min(buf.len());
    if buf.len() < 4 {
        return None;
    }
    let limit = buf.len() - 4;
    let mut i = start;
    let mut scanned = 0usize;
    while i <= limit && scanned < 65536 {
        if buf[i] == 0xFF && (buf[i + 1] & 0xE0) == 0xE0 {
            if let Some(f) = parse_mp3_frame(buf, i) {
                return Some(f);
            }
        }
        i += 1;
        scanned += 1;
    }
    None
}

fn parse_mp3(head: &[u8], tail: &[u8], file_len: u64) -> (Id3Tags, f64, u32, u16, u32, &'static str) {
    let mut tags = Id3Tags::default();
    let mut dur = 0f64;
    let mut rate = 0u32;
    let mut ch = 0u16;
    let mut kbps = 0u32;
    let mut audio_start = 0usize;
    if head.starts_with(b"ID3") && head.len() >= 10 {
        let tag_size = syncsafe(head, 6).unwrap_or(0) as usize;
        let footer = if head[5] & 0x10 != 0 { 10usize } else { 0 };
        audio_start = 10 + tag_size + footer;
        let end = std::cmp::min(audio_start, head.len());
        let raw = &head[10..end];
        let buf = if head[5] & 0x80 != 0 {
            de_unsync(raw)
        } else {
            raw.to_vec()
        };
        tags = parse_id3v2_frames(&buf, head[3], head[5]);
    }
    let has_v1 = tail.len() >= 128 && tail[tail.len() - 128..].starts_with(b"TAG");
    if tags.title.is_none() || tags.artist.is_none() || tags.album.is_none() {
        let v1 = parse_id3v1(tail);
        merge_tags(&mut tags, v1);
    }
    if let Some(f) = find_first_frame(head, audio_start) {
        rate = f.rate;
        ch = f.channels;
        kbps = f.kbps;
        // Xing/Info header (inside the first frame, after the side info).
        let side: usize = if f.is_v1 {
            if f.channels == 1 {
                17
            } else {
                32
            }
        } else if f.channels == 1 {
            9
        } else {
            17
        };
        let xing_at = f.header_at + 4 + side;
        if xing_at + 16 <= head.len() {
            let tag4 = &head[xing_at..];
            if tag4.starts_with(b"Xing") || tag4.starts_with(b"Info") {
                let xflags = u32be(head, xing_at + 4).unwrap_or(0);
                if xflags & 0x01 != 0 {
                    let frames = u32be(head, xing_at + 8).unwrap_or(0);
                    if frames > 0 && f.rate > 0 {
                        dur = frames as f64 * f.samples_per_frame as f64 / f.rate as f64;
                    }
                }
            }
        }
        if dur == 0.0 && kbps > 0 {
            // CBR estimate over the audio area (without ID3 tags).
            let mut audio_len = file_len.saturating_sub(audio_start as u64);
            if has_v1 {
                audio_len = audio_len.saturating_sub(128);
            }
            dur = (audio_len * 8) as f64 / (kbps as f64 * 1000.0);
        }
    }
    (tags, dur, rate, ch, kbps, "mp3")
}

// --------------------------------------------------------------------- FLAC

fn parse_flac(head: &[u8]) -> (Id3Tags, f64, u32, u16) {
    let mut tags = Id3Tags::default();
    let mut dur = 0f64;
    let mut rate = 0u32;
    let mut ch = 0u16;
    let mut pos = 4usize;
    let mut guard = 0usize;
    while pos + 4 <= head.len() && guard < 256 {
        guard += 1;
        let b0 = head[pos];
        let btype = b0 & 0x7F;
        let last = (b0 & 0x80) != 0;
        let blen = ((head[pos + 1] as usize) << 16)
            | ((head[pos + 2] as usize) << 8)
            | (head[pos + 3] as usize);
        let start = pos + 4;
        if start + blen > head.len() {
            break; // metadata larger than our 1 MiB head: stop gracefully
        }
        let body = &head[start..start + blen];
        match btype {
            0 => {
                // STREAMINFO: 10 bytes block sizes/frames, then 8 packed
                // bytes: 20 bits rate, 3 bits channels-1, 5 bits bps-1,
                // 36 bits total samples.
                if body.len() >= 18 {
                    rate = ((body[10] as u32) << 12)
                        | ((body[11] as u32) << 4)
                        | ((body[12] as u32) >> 4);
                    ch = (((body[12] >> 1) & 0x07) as u16) + 1;
                    let total = (((body[13] & 0x0F) as u64) << 32)
                        | ((body[14] as u64) << 24)
                        | ((body[15] as u64) << 16)
                        | ((body[16] as u64) << 8)
                        | (body[17] as u64);
                    if rate > 0 && total > 0 {
                        dur = total as f64 / rate as f64;
                    }
                }
            }
            4 => {
                for (k, v) in parse_vorbis_comments(body) {
                    tags.set_vorbis(&k, v);
                }
            }
            6 => tags.has_cover = true,
            _ => {}
        }
        pos = start + blen;
        if last {
            break;
        }
    }
    (tags, dur, rate, ch)
}

/// Vorbis comment block (FLAC type 4 / OGG comment packet): vendor string,
/// count, then `KEY=value` UTF-8 pairs, all little-endian lengths.
fn parse_vorbis_comments(data: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let vlen = match u32le(data, pos) {
        Some(v) => v as usize,
        None => return out,
    };
    pos = match pos.checked_add(4 + vlen) {
        Some(p) => p,
        None => return out,
    };
    let count = match u32le(data, pos) {
        Some(c) => c.min(4096),
        None => return out,
    };
    pos += 4;
    for _ in 0..count {
        let clen = match u32le(data, pos) {
            Some(c) => c as usize,
            None => break,
        };
        pos += 4;
        if clen > 16 * 1024 * 1024 || pos + clen > data.len() {
            break;
        }
        let kv = String::from_utf8_lossy(&data[pos..pos + clen]).to_string();
        pos += clen;
        if let Some(eq) = kv.find('=') {
            out.push((kv[..eq].to_ascii_uppercase(), kv[eq + 1..].to_string()));
        }
    }
    out
}

// ---------------------------------------------------------------------- OGG

fn parse_ogg(head: &[u8], tail: &[u8]) -> (Id3Tags, f64, u32, u16, u32, &'static str) {
    let mut tags = Id3Tags::default();
    let mut dur = 0f64;
    let mut rate = 0u32;
    let mut ch = 0u16;
    let mut preskip = 0u32;
    let mut format_name = "ogg";
    if let Some(p) = find(head, b"OpusHead") {
        format_name = "opus";
        if p + 12 <= head.len() {
            ch = head[p + 9] as u16;
            preskip = u16le(head, p + 10).unwrap_or(0) as u32;
        }
    } else if let Some(p) = find(head, b"\x01vorbis") {
        if p + 16 <= head.len() {
            ch = head[p + 11] as u16;
            rate = u32le(head, p + 12).unwrap_or(0);
        }
    }
    let comments_at = find(head, b"OpusTags")
        .map(|p| p + 8)
        .or_else(|| find(head, b"\x03vorbis").map(|p| p + 7));
    if let Some(c) = comments_at {
        if c < head.len() {
            for (k, v) in parse_vorbis_comments(&head[c..]) {
                tags.set_vorbis(&k, v);
            }
        }
    }
    if let Some(granule) = last_granule(tail) {
        if format_name == "opus" {
            let g = granule.saturating_sub(preskip as u64);
            dur = g as f64 / 48000.0;
        } else if rate > 0 {
            dur = granule as f64 / rate as f64;
        }
    }
    (tags, dur, rate, ch, 0, format_name)
}

/// Granule position of the last complete OGG page found in `tail`.
fn last_granule(tail: &[u8]) -> Option<u64> {
    if tail.len() < 14 {
        return None;
    }
    let mut found: Option<usize> = None;
    let mut i = 0usize;
    while i + 4 <= tail.len() {
        if &tail[i..i + 4] == b"OggS" && tail.get(i + 4) == Some(&0) {
            found = Some(i);
        }
        i += 1;
    }
    u64le(tail, found? + 6)
}

// ---------------------------------------------------------------------- WAV

fn four(b: &[u8]) -> [u8; 4] {
    [b[0], b[1], b[2], b[3]]
}

fn parse_wav(head: &[u8]) -> (Id3Tags, f64, u32, u16) {
    let mut tags = Id3Tags::default();
    let mut dur = 0f64;
    let mut rate = 0u32;
    let mut ch = 0u16;
    let mut bits = 0u16;
    if head.len() < 12 {
        return (tags, dur, rate, ch);
    }
    let mut pos = 12usize;
    let mut guard = 0usize;
    while pos + 8 <= head.len() && guard < 10_000 {
        guard += 1;
        let cid = four(&head[pos..pos + 4]);
        let csize = u32le(head, pos + 4).unwrap_or(0) as usize;
        let body_start = pos + 8;
        if cid == *b"data" {
            if rate > 0 && ch > 0 && bits > 0 {
                let byte_rate = rate as f64 * ch as f64 * bits as f64 / 8.0;
                if byte_rate > 0.0 {
                    dur = csize as f64 / byte_rate;
                }
            }
            break; // everything interesting lives before the samples
        }
        if body_start + csize > head.len() {
            break;
        }
        let body = &head[body_start..body_start + csize];
        if cid == *b"fmt " {
            if body.len() >= 16 {
                ch = u16le(body, 2).unwrap_or(0);
                rate = u32le(body, 4).unwrap_or(0);
                bits = u16le(body, 14).unwrap_or(0);
            }
        } else if cid == *b"LIST" {
            if body.len() >= 4 && &body[0..4] == b"INFO" {
                let mut sp = 4usize;
                let mut sguard = 0usize;
                while sp + 8 <= body.len() && sguard < 256 {
                    sguard += 1;
                    let sid = four(&body[sp..sp + 4]);
                    let ssize = u32le(body, sp + 4).unwrap_or(0) as usize;
                    let sstart = sp + 8;
                    if sstart + ssize > body.len() {
                        break;
                    }
                    let raw = String::from_utf8_lossy(&body[sstart..sstart + ssize]).to_string();
                    match sid {
                        *b"INAM" => tags.title = clean(raw),
                        *b"IART" => tags.artist = clean(raw),
                        *b"IPRD" => tags.album = clean(raw),
                        *b"IGNR" => tags.genre = clean(raw),
                        *b"ICRD" => tags.year = clean(raw),
                        *b"ITRK" => tags.track = parse_track_number(&raw),
                        _ => {}
                    }
                    sp = sstart + ssize + (ssize & 1);
                }
            }
        } else if cid == *b"id3 " || cid == *b"ID3 " {
            merge_tags(&mut tags, parse_id3v2_whole(body));
        }
        pos = body_start + csize + (csize & 1);
    }
    (tags, dur, rate, ch)
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id3v23_text_frames_are_parsed() {
        let mut frames: Vec<u8> = Vec::new();
        {
            let mut frame = |id: &[u8; 4], payload: &[u8]| {
                frames.extend_from_slice(id);
                frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                frames.extend_from_slice(&[0, 0]);
                frames.extend_from_slice(payload);
            };
            let mut tit = vec![0u8];
            tit.extend_from_slice(b"Cancion sin nombre");
            frame(b"TIT2", &tit);
            let mut tpe = vec![0u8];
            tpe.extend_from_slice(b"Artista Prueba");
            frame(b"TPE1", &tpe);
            let mut tal = vec![0u8];
            tal.extend_from_slice(b"Album Prueba");
            frame(b"TALB", &tal);
            let mut tyer = vec![0u8];
            tyer.extend_from_slice(b"1998");
            frame(b"TYER", &tyer);
            let mut trck = vec![0u8];
            trck.extend_from_slice(b"7/12");
            frame(b"TRCK", &trck);
            let mut apic = vec![0u8];
            apic.extend_from_slice(b"image/jpeg");
            apic.push(0);
            apic.push(3);
            apic.extend_from_slice(b"coverdata");
            frame(b"APIC", &apic);
        }
        let tags = parse_id3v2_frames(&frames, 3, 0);
        assert_eq!(tags.title.as_deref(), Some("Cancion sin nombre"));
        assert_eq!(tags.artist.as_deref(), Some("Artista Prueba"));
        assert_eq!(tags.album.as_deref(), Some("Album Prueba"));
        assert_eq!(tags.year.as_deref(), Some("1998"));
        assert_eq!(tags.track, Some(7));
        assert!(tags.has_cover);
    }

    #[test]
    fn id3v23_utf16_text_is_decoded() {
        let mut payload = vec![1u8, 0xFF, 0xFE];
        payload.extend("Título".encode_utf16().flat_map(|u| u.to_le_bytes()));
        let mut frames: Vec<u8> = Vec::new();
        frames.extend_from_slice(b"TIT2");
        frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frames.extend_from_slice(&[0, 0]);
        frames.extend_from_slice(&payload);
        let tags = parse_id3v2_frames(&frames, 3, 0);
        assert_eq!(tags.title.as_deref(), Some("Título"));
    }

    #[test]
    fn id3v1_fallback_tag_is_parsed() {
        let mut tail = vec![0u8; 128];
        tail[0..3].copy_from_slice(b"TAG");
        tail[3..16].copy_from_slice(b"Vieja Escuela");
        tail[33..41].copy_from_slice(b"Banda X");
        tail[63..71].copy_from_slice(b"Clasicos");
        tail[93..97].copy_from_slice(b"1999");
        tail[126] = 5;
        tail[127] = 17; // Rock
        let tags = parse_id3v1(&tail);
        assert_eq!(tags.title.as_deref(), Some("Vieja Escuela"));
        assert_eq!(tags.artist.as_deref(), Some("Banda X"));
        assert_eq!(tags.album.as_deref(), Some("Clasicos"));
        assert_eq!(tags.year.as_deref(), Some("1999"));
        assert_eq!(tags.track, Some(5));
        assert_eq!(tags.genre.as_deref(), Some("Rock"));
    }

    #[test]
    fn flac_streaminfo_and_comments_are_parsed() {
        // STREAMINFO: rate 48000, 2 channels, 16 bits, 96000 total samples.
        let mut si = vec![0u8; 34];
        let (rate, ch, total): (u32, u16, u64) = (48000, 2, 96000);
        si[10] = (rate >> 12) as u8;
        si[11] = (rate >> 4) as u8;
        si[12] = ((rate & 0x0F) as u8) << 4 | (((ch - 1) as u8) << 1);
        si[13] = (15u8) << 4; // bps code 15 (=16 bits) in the top nibble
        si[14..18].copy_from_slice(&(total as u32).to_be_bytes());
        // VORBIS_COMMENT with one TITLE entry.
        let mut vc: Vec<u8> = Vec::new();
        vc.extend_from_slice(&4u32.to_le_bytes());
        vc.extend_from_slice(b"test");
        vc.extend_from_slice(&1u32.to_le_bytes());
        let kv = b"TITLE=Flac Song";
        vc.extend_from_slice(&(kv.len() as u32).to_le_bytes());
        vc.extend_from_slice(kv);
        let mut head: Vec<u8> = Vec::new();
        head.extend_from_slice(b"fLaC");
        head.extend_from_slice(&[0x00, 0, 0, 34]);
        head.extend_from_slice(&si);
        head.extend_from_slice(&[0x84]); // last block, type 4
        head.extend_from_slice(&(vc.len() as u32).to_be_bytes());
        head.extend_from_slice(&vc);
        let (tags, dur, r, c) = parse_flac(&head);
        assert_eq!(tags.title.as_deref(), Some("Flac Song"));
        assert!((dur - 2.0).abs() < 1e-9);
        assert_eq!(r, 48000);
        assert_eq!(c, 2);
    }

    #[test]
    fn ogg_duration_comes_from_the_last_page() {
        // Minimal first page with a Vorbis identification packet.
        let mut packet: Vec<u8> = Vec::new();
        packet.extend_from_slice(b"\x01vorbis");
        packet.extend_from_slice(&0u32.to_le_bytes()); // version
        packet.push(2); // channels
        packet.extend_from_slice(&44100u32.to_le_bytes()); // rate
        packet.extend_from_slice(&0u32.to_le_bytes()); // bitrate nominal
        packet.extend_from_slice(&0u32.to_le_bytes()); // bitrate max
        packet.extend_from_slice(&0u32.to_le_bytes()); // bitrate min
        packet.push(0x38); // blocksize
        packet.push(1); // framing
        let mut head: Vec<u8> = Vec::new();
        head.extend_from_slice(b"OggS");
        head.push(0); // version
        head.push(0); // header type
        head.extend_from_slice(&0u64.to_le_bytes()); // granule
        head.extend_from_slice(&1u32.to_le_bytes()); // serial
        head.extend_from_slice(&0u32.to_le_bytes()); // sequence
        head.extend_from_slice(&0u32.to_le_bytes()); // CRC (not checked)
        head.push(1); // one segment
        head.push(packet.len() as u8);
        head.extend_from_slice(&packet);
        // Minimal last page carrying the final granule position.
        let mut tail: Vec<u8> = Vec::new();
        tail.extend_from_slice(b"OggS");
        tail.push(0);
        tail.push(4);
        tail.extend_from_slice(&96000u64.to_le_bytes()); // granule
        tail.extend_from_slice(&1u32.to_le_bytes());
        tail.extend_from_slice(&1u32.to_le_bytes());
        tail.extend_from_slice(&0u32.to_le_bytes());
        tail.push(1);
        tail.push(1);
        tail.push(0xFF);
        let (tags, dur, rate, ch, _kbps, format_name) = parse_ogg(&head, &tail);
        assert_eq!(tags.title, None);
        assert!((dur - 96000.0 / 44100.0).abs() < 1e-9);
        assert_eq!(rate, 44100);
        assert_eq!(ch, 2);
        assert_eq!(format_name, "ogg");
    }

    #[test]
    fn opus_duration_uses_48khz_and_preskip() {
        let mut head: Vec<u8> = Vec::new();
        head.extend_from_slice(b"OggS");
        head.push(0);
        head.push(0);
        head.extend_from_slice(&0u64.to_le_bytes());
        head.extend_from_slice(&1u32.to_le_bytes());
        head.extend_from_slice(&0u32.to_le_bytes());
        head.extend_from_slice(&0u32.to_le_bytes());
        head.push(1);
        head.push(19);
        let mut packet: Vec<u8> = Vec::new();
        packet.extend_from_slice(b"OpusHead");
        packet.push(1); // version
        packet.push(2); // channels
        packet.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
        packet.extend_from_slice(&48000u32.to_le_bytes()); // input rate
        packet.extend_from_slice(&0u16.to_le_bytes()); // gain
        packet.push(0); // channel mapping family
        head.extend_from_slice(&packet);
        let mut tail: Vec<u8> = Vec::new();
        tail.extend_from_slice(b"OggS");
        tail.push(0);
        tail.push(4);
        tail.extend_from_slice(&48000u64.to_le_bytes());
        tail.extend_from_slice(&1u32.to_le_bytes());
        tail.extend_from_slice(&1u32.to_le_bytes());
        tail.extend_from_slice(&0u32.to_le_bytes());
        tail.push(1);
        tail.push(1);
        tail.push(0xFF);
        let (_tags, dur, _rate, _ch, _kbps, format_name) = parse_ogg(&head, &tail);
        assert_eq!(format_name, "opus");
        assert!((dur - (48000.0 - 312.0) / 48000.0).abs() < 1e-9);
    }

    #[test]
    fn mp3_xing_frame_count_gives_exact_duration() {
        let mut head: Vec<u8> = Vec::new();
        // MPEG1 Layer III, 128 kbps, 44100 Hz, stereo.
        head.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        head.extend_from_slice(&[0u8; 32]); // side info (stereo MPEG1)
        head.extend_from_slice(b"Xing");
        head.extend_from_slice(&0x01u32.to_be_bytes()); // frames field present
        head.extend_from_slice(&1000u32.to_be_bytes()); // 1000 frames
        head.extend_from_slice(&[0u8; 16]);
        let (_tags, dur, rate, ch, kbps, format_name) = parse_mp3(&head, &[], head.len() as u64);
        assert_eq!(format_name, "mp3");
        assert_eq!(rate, 44100);
        assert_eq!(ch, 2);
        assert_eq!(kbps, 128);
        assert!((dur - 1000.0 * 1152.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn mp3_cbr_estimate_without_xing() {
        let head: Vec<u8> = vec![0xFF, 0xFB, 0x90, 0x00, 0, 0, 0, 0];
        let (_tags, dur, _rate, _ch, _kbps, _fmt) = parse_mp3(&head, &[], 1_000_000);
        assert!((dur - 62.5).abs() < 0.01);
    }

    /// Build a tiny but valid RIFF WAVE file with a LIST/INFO chunk.
    fn tiny_wav(title: &str, data_size: u32) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(b"RIFF");
        let riff_len_at = v.len();
        v.extend_from_slice(&0u32.to_le_bytes()); // patched below
        v.extend_from_slice(b"WAVE");
        // fmt
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&2u16.to_le_bytes()); // channels
        v.extend_from_slice(&44100u32.to_le_bytes()); // rate
        v.extend_from_slice(&176400u32.to_le_bytes()); // byte rate
        v.extend_from_slice(&4u16.to_le_bytes()); // block align
        v.extend_from_slice(&16u16.to_le_bytes()); // bits
        // LIST INFO
        let mut info: Vec<u8> = Vec::new();
        info.extend_from_slice(b"INFO");
        info.extend_from_slice(b"INAM");
        let title_field = format!("{title}\0");
        info.extend_from_slice(&(title_field.len() as u32).to_le_bytes());
        info.extend_from_slice(title_field.as_bytes());
        if title_field.len() % 2 == 1 {
            info.push(0);
        }
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&(info.len() as u32).to_le_bytes());
        v.extend_from_slice(&info);
        // data (declared size; body mostly absent on purpose)
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_size.to_le_bytes());
        v.extend_from_slice(&[0u8; 64]);
        let riff_len = (v.len() - 8) as u32;
        v[riff_len_at..riff_len_at + 4].copy_from_slice(&riff_len.to_le_bytes());
        v
    }

    #[test]
    fn wav_info_tags_and_duration_are_parsed() {
        let head = tiny_wav("Cancion WAV", 176_400);
        let (tags, dur, rate, ch) = parse_wav(&head);
        assert_eq!(tags.title.as_deref(), Some("Cancion WAV"));
        assert!((dur - 1.0).abs() < 1e-9); // 176400 bytes / 176400 B/s
        assert_eq!(rate, 44100);
        assert_eq!(ch, 2);
    }

    #[test]
    fn read_track_rejects_garbage() {
        let dir = std::env::temp_dir().join(format!("zett-tags-garbage-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("nada.txt");
        fs::write(&file, b"definitivamente no es audio").unwrap();
        let res = read_track(file.to_str().unwrap());
        fs::remove_dir_all(&dir).ok();
        assert!(res.is_err());
    }

    #[test]
    fn scan_library_finds_and_skips() {
        let dir = std::env::temp_dir().join(format!("zett-tags-scan-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let wav_path = dir.join("una.wav");
        fs::write(&wav_path, tiny_wav("Pista Uno", 176_400)).unwrap();
        let wav2_path = sub.join("dos.wav");
        fs::write(&wav2_path, tiny_wav("Pista Dos", 88_200)).unwrap();
        fs::write(dir.join("leeme.txt"), b"no es musica").unwrap();
        let lib = scan_library(dir.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&dir).ok();
        assert_eq!(lib.len(), 2);
        // Sorted by path: `sub/dos.wav` comes before `una.wav`.
        assert_eq!(lib[0].title.as_deref(), Some("Pista Dos"));
        assert!((lib[0].duration_secs - 0.5).abs() < 1e-9);
        assert_eq!(lib[1].title.as_deref(), Some("Pista Uno"));
        assert!((lib[1].duration_secs - 1.0).abs() < 1e-9);
    }

    #[test]
    fn scan_library_rejects_missing_dir() {
        assert!(scan_library("/definitely/not/a/dir/zett").is_err());
    }
}
