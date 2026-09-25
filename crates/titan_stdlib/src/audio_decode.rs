//! Native decode core (`symphonia`) — Fase 42A.
//!
//! This is the muscle behind `std::audio::engine_*`: real MP3/FLAC/Vorbis/
//! Opus/WAV decoding **inside** the Titan binary, 100% pure Rust, zero
//! system libraries. The companion module [`crate::audio_engine`] pushes
//! the PCM produced here to the speakers.
//!
//! Split by design:
//!
//! * [`FileDecoder`] — incremental, seekable decoder feeding the playback
//!   thread chunk by chunk (never loads a whole song into memory).
//! * [`decode_file`] — bounded whole-file decode for headless checks,
//!   tests and the future Telegram streaming cache (Fase 43).
//! * [`resample_linear`], [`Eq3`], [`spectrum_32`] — tiny DSP helpers with
//!   no dependencies so they also compile on Android (Fase 42B).

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use thiserror::Error;

/// Errors produced while decoding audio.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("audio file not found: {0}")]
    NotFound(String),
    #[error("audio I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported or corrupt audio file: {0}")]
    Unsupported(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("seek out of range: {0}")]
    Seek(String),
    #[error("invalid parameter: {0}")]
    Invalid(String),
}

/// Container-level facts about a stream, without decoding audio.
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
}

/// Bounded whole-file decode: interleaved `f32` PCM in `-1.0..=1.0`.
#[derive(Debug, Clone, Default)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    /// Header duration (`0.0` when the container does not say).
    pub duration_secs: f64,
    /// Measured from the samples actually decoded.
    pub decoded_secs: f64,
    pub codec: String,
}

fn time_to_secs(time: Time) -> f64 {
    time.seconds as f64 + time.frac
}

fn time_from_secs(secs: f64) -> Time {
    let clamped = if secs.is_finite() { secs.max(0.0) } else { 0.0 };
    Time {
        seconds: clamped.trunc() as u64,
        frac: clamped.fract(),
    }
}

// ------------------------------------------------------------------ decoder

/// Incremental, seekable decoder over one audio file.
///
/// The playback thread calls [`FileDecoder::fill`] in a loop; each call
/// appends up to `max_frames` of interleaved `f32` PCM (at the file's own
/// sample rate — resampling to the output device happens in the engine).
pub struct FileDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    duration_secs: f64,
    codec: String,
    decoded_frames: u64,
    finished: bool,
}

impl FileDecoder {
    /// Open `path` and pick the first track the bundled codecs can decode.
    pub fn open(path: &str) -> Result<Self, DecodeError> {
        let file = File::open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DecodeError::NotFound(path.to_string())
            } else {
                DecodeError::Io(e)
            }
        })?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            hint.with_extension(&ext.to_ascii_lowercase());
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| DecodeError::Unsupported(format!("{path}: {e}")))?;
        let format = probed.format;

        // First track one of the bundled decoders accepts wins. Trying is
        // more robust than matching codec ids: containers lie sometimes.
        let mut selected: Option<(u32, Box<dyn Decoder>, u32, u16, f64, String)> = None;
        for track in format.tracks().iter() {
            let params = &track.codec_params;
            if let Ok(decoder) =
                symphonia::default::get_codecs().make(params, &DecoderOptions::default())
            {
                let rate = params.sample_rate.unwrap_or(0);
                let channels = params
                    .channels
                    .as_ref()
                    .map(|c| c.count() as u16)
                    .unwrap_or(0);
                let duration = params
                    .n_frames
                    .and_then(|n| {
                        params
                            .time_base
                            .as_ref()
                            .map(|tb| time_to_secs(tb.calc_time(n)))
                    })
                    .unwrap_or(0.0);
                let codec = format!("{:?}", params.codec).to_ascii_lowercase();
                selected = Some((track.id, decoder, rate, channels, duration, codec));
                break;
            }
        }
        let (track_id, decoder, sample_rate, channels, duration_secs, codec) =
            selected.ok_or_else(|| {
                DecodeError::Unsupported(format!("no decodable audio track: {path}"))
            })?;
        Ok(FileDecoder {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            duration_secs,
            codec,
            decoded_frames: 0,
            finished: false,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn duration_secs(&self) -> f64 {
        self.duration_secs
    }
    pub fn codec(&self) -> &str {
        &self.codec
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Seconds decoded so far, at the file's own sample rate.
    pub fn position_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.decoded_frames as f64 / self.sample_rate as f64
        }
    }

    /// Decode up to `max_frames` (interleaved) into `out`.
    ///
    /// Call again until [`FileDecoder::is_finished`] is true. Corrupt
    /// packets are skipped, never fatal: at worst the track reports fewer
    /// samples than it should.
    pub fn fill(&mut self, out: &mut Vec<f32>, max_frames: usize) -> Result<(), DecodeError> {
        if self.finished || max_frames == 0 {
            return Ok(());
        }
        // Hard iteration guard: a hostile file can never spin the VM.
        for _ in 0..4096 {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(_)) => {
                    self.finished = true;
                    return Ok(());
                }
                Err(e) => return Err(DecodeError::Decode(e.to_string())),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(SymphoniaError::IoError(_)) => {
                    self.finished = true;
                    return Ok(());
                }
                Err(e) => return Err(DecodeError::Decode(e.to_string())),
            };
            // Trust the decoded buffers, not the container headers: the
            // real spec is whatever the decoder produced.
            let spec = *decoded.spec();
            self.sample_rate = spec.rate;
            self.channels = spec.channels.count() as u16;
            // A fresh buffer per packet: correct under every symphonia
            // buffering semantic (clear-then-copy vs overwrite) and robust
            // to mid-stream spec changes. ~9 KiB per packet is noise.
            let mut pcm =
                symphonia::core::audio::SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            pcm.copy_interleaved_ref(decoded);
            let samples = pcm.samples();
            let channels = self.channels.max(1) as usize;
            self.decoded_frames += (samples.len() / channels) as u64;
            out.extend_from_slice(samples);
            if out.len() / channels >= max_frames {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Accurate seek. Decoder state is reset so the next [`FileDecoder::fill`]
    /// returns audio from (very close to) `secs`.
    pub fn seek(&mut self, secs: f64) -> Result<(), DecodeError> {
        if !secs.is_finite() || secs < 0.0 {
            return Err(DecodeError::Seek(format!("{secs}")));
        }
        if self.duration_secs > 0.0 && secs > self.duration_secs {
            return Err(DecodeError::Seek(format!(
                "{secs} exceeds {dur:.1}s",
                dur = self.duration_secs
            )));
        }
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: time_from_secs(secs),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| DecodeError::Seek(e.to_string()))?;
        self.decoder.reset();
        self.finished = false;
        self.decoded_frames = (secs.max(0.0) * self.sample_rate.max(1) as f64) as u64;
        Ok(())
    }
}

/// Container facts without decoding any audio.
pub fn probe(path: &str) -> Result<StreamInfo, DecodeError> {
    let decoder = FileDecoder::open(path)?;
    Ok(StreamInfo {
        duration_secs: decoder.duration_secs(),
        sample_rate: decoder.sample_rate(),
        channels: decoder.channels(),
        codec: decoder.codec().to_string(),
    })
}

/// Decode at most `max_secs` of `path` into memory.
///
/// Bounded on purpose: a 3-hour podcast must never OOM the VM. Used by
/// `std::audio::engine_decode`, headless CI and (later) the Telegram
/// streaming cache of Fase 43.
pub fn decode_file(path: &str, max_secs: f64) -> Result<DecodedAudio, DecodeError> {
    if !max_secs.is_finite() || max_secs <= 0.0 {
        return Err(DecodeError::Invalid(format!("max_secs={max_secs}")));
    }
    let mut decoder = FileDecoder::open(path)?;
    let mut out = DecodedAudio {
        sample_rate: decoder.sample_rate(),
        channels: decoder.channels(),
        duration_secs: decoder.duration_secs(),
        codec: decoder.codec().to_string(),
        ..DecodedAudio::default()
    };
    let mut chunk: Vec<f32> = Vec::new();
    loop {
        chunk.clear();
        decoder.fill(&mut chunk, 8192)?;
        out.sample_rate = decoder.sample_rate();
        out.channels = decoder.channels();
        out.samples.extend_from_slice(&chunk);
        let rate = out.sample_rate.max(1) as f64;
        let channels = out.channels.max(1) as f64;
        out.decoded_secs = out.samples.len() as f64 / rate / channels;
        if decoder.is_finished() || out.decoded_secs >= max_secs {
            break;
        }
    }
    Ok(out)
}

// --------------------------------------------------------------------- DSP

/// Linear-interpolation resampler (interleaved `f32`).
///
/// Transparent when rates match. Good enough for music playback; a windowed
/// sinc lives in the backlog, not in Fase 42A.
pub fn resample_linear(input: &[f32], channels: usize, from_rate: u32, to_rate: u32) -> Vec<f32> {
    let channels = channels.max(1);
    if input.is_empty() || from_rate == 0 || to_rate == 0 || from_rate == to_rate {
        return input.to_vec();
    }
    let in_frames = input.len() / channels;
    if in_frames == 0 {
        return Vec::new();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_frames = ((in_frames as f64 / ratio).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(out_frames * channels);
    for n in 0..out_frames {
        let pos = n as f64 * ratio;
        let i0 = (pos.floor() as usize).min(in_frames - 1);
        let i1 = (i0 + 1).min(in_frames - 1);
        let frac = (pos - pos.floor()) as f32;
        for ch in 0..channels {
            let a = input[i0 * channels + ch];
            let b = input[i1 * channels + ch];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn new(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        let a0 = if a0.abs() < 1e-9 { 1.0 } else { a0 };
        Biquad {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn lowshelf(freq: f32, rate: f32, db: f32) -> Self {
        // RBJ cookbook, S = 1.
        let a = 10f32.powf(db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0 * 2f32.sqrt();
        let sqrt_a = a.sqrt();
        Biquad::new(
            a * ((a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * alpha),
            (a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * alpha,
        )
    }

    fn highshelf(freq: f32, rate: f32, db: f32) -> Self {
        let a = 10f32.powf(db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0 * 2f32.sqrt();
        let sqrt_a = a.sqrt();
        Biquad::new(
            a * ((a + 1.0) + (a - 1.0) * cos + 2.0 * sqrt_a * alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - 2.0 * sqrt_a * alpha),
            (a + 1.0) - (a - 1.0) * cos + 2.0 * sqrt_a * alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - 2.0 * sqrt_a * alpha,
        )
    }

    fn peaking(freq: f32, rate: f32, db: f32) -> Self {
        let a = 10f32.powf(db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / 2.0; // Q = 1
        Biquad::new(
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        )
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// 3-band equalizer: lowshelf 250 Hz, peaking 1 kHz, highshelf 4 kHz.
///
/// Gains in dB, clamped to `-12..=+12`. All-zero bypasses processing
/// bit-transparent (and click-free: no filter state is touched).
#[derive(Debug, Clone)]
pub struct Eq3 {
    bands: Vec<Biquad>,
    channels: usize,
    bypass: bool,
}

impl Eq3 {
    pub fn new(db: [f32; 3], rate: u32, channels: usize) -> Self {
        let channels = channels.max(1);
        let rate = rate.max(8000) as f32;
        let gains = [
            db[0].clamp(-12.0, 12.0),
            db[1].clamp(-12.0, 12.0),
            db[2].clamp(-12.0, 12.0),
        ];
        let bypass = gains == [0.0, 0.0, 0.0];
        let mut bands = Vec::with_capacity(3 * channels);
        for _ in 0..channels {
            bands.push(Biquad::lowshelf(250.0, rate, gains[0]));
            bands.push(Biquad::peaking(1000.0, rate, gains[1]));
            bands.push(Biquad::highshelf(4000.0, rate, gains[2]));
        }
        Eq3 {
            bands,
            channels,
            bypass,
        }
    }

    pub fn is_bypass(&self) -> bool {
        self.bypass
    }

    /// In-place filtering of interleaved `f32` PCM.
    pub fn process(&mut self, samples: &mut [f32]) {
        if self.bypass || samples.is_empty() {
            return;
        }
        let frames = samples.len() / self.channels;
        for n in 0..frames {
            for ch in 0..self.channels {
                let idx = n * self.channels + ch;
                let base = ch * 3;
                let mut v = samples[idx];
                for b in 0..3 {
                    v = self.bands[base + b].process(v);
                }
                samples[idx] = v;
            }
        }
    }
}

/// 32-bar log-spaced spectrum (`60 Hz..16 kHz`) from mono `f32` samples.
///
/// Single-frequency DFTs (Goertzel-style math, written out for zero deps).
/// Magnitudes are normalized to `0.0..=1.0` for visualizers: silence reads
/// `0.0`, a full-scale tone at a band center reads `~1.0`.
pub fn spectrum_32(mono: &[f32], rate: u32) -> [f32; 32] {
    let mut bars = [0.0f32; 32];
    let n = mono.len().min(2048);
    if n < 64 || rate == 0 {
        return bars;
    }
    let rate = rate as f32;
    for (k, bar) in bars.iter_mut().enumerate() {
        let freq = 60.0 * (16_000.0f32 / 60.0).powf(k as f32 / 31.0);
        let omega = 2.0 * std::f32::consts::PI * freq / rate;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for i in 0..n {
            let phase = omega * i as f32;
            re += mono[i] * phase.cos();
            im += mono[i] * phase.sin();
        }
        let mag = (re * re + im * im).sqrt() / n as f32;
        // Full-scale sine peaks at mag 0.5; map to 0..1 with headroom.
        *bar = (mag * 2.2).clamp(0.0, 1.0);
    }
    bars
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Minimal but VALID 16-bit PCM WAV: symphonia decodes this for real.
    fn pcm_wav_bytes(freq: f32, secs: f32, rate: u32, channels: u16) -> Vec<u8> {
        let frames = (secs * rate as f32) as usize;
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(b"RIFF");
        let riff_at = v.len();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        let byte_rate = rate * channels as u32 * 2;
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&(channels * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(frames as u32 * channels as u32 * 2).to_le_bytes());
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            let s = (0.5 * (2.0 * std::f32::consts::PI * freq * t).sin() * 32767.0) as i16;
            for _ in 0..channels {
                v.extend_from_slice(&s.to_le_bytes());
            }
        }
        let riff_len = (v.len() - 8) as u32;
        v[riff_at..riff_at + 4].copy_from_slice(&riff_len.to_le_bytes());
        v
    }

    fn tmp_wav(name: &str, freq: f32, secs: f32) -> String {
        let dir = std::env::temp_dir().join(format!("zett-decode-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.wav"));
        fs::write(&path, pcm_wav_bytes(freq, secs, 44_100, 2)).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn probe_reads_wav_facts() {
        let path = tmp_wav("probe", 440.0, 1.0);
        let info = probe(&path).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.channels, 2);
        assert!((info.duration_secs - 1.0).abs() < 0.05, "{}", info.duration_secs);
        assert!(!info.codec.is_empty());
    }

    #[test]
    fn decode_file_returns_the_sine() {
        let path = tmp_wav("sine", 440.0, 0.5);
        let dec = decode_file(&path, 5.0).unwrap();
        fs::remove_file(&path).ok();
        assert_eq!(dec.sample_rate, 44_100);
        assert_eq!(dec.channels, 2);
        assert!((dec.decoded_secs - 0.5).abs() < 0.05, "{}", dec.decoded_secs);
        let peak = dec.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.4 && peak <= 1.0, "peak={peak}");
        // Stereo interleave: left and right carry the same tone.
        assert!((dec.samples[0] - dec.samples[1]).abs() < 1e-4);
    }

    #[test]
    fn decode_file_is_bounded() {
        let path = tmp_wav("bound", 440.0, 2.0);
        let dec = decode_file(&path, 0.25).unwrap();
        fs::remove_file(&path).ok();
        assert!(dec.decoded_secs < 0.6, "{}", dec.decoded_secs);
        assert!(!dec.samples.is_empty());
    }

    #[test]
    fn decoder_seeks_and_reports_position() {
        let path = tmp_wav("seek", 440.0, 2.0);
        let mut dec = FileDecoder::open(&path).unwrap();
        let mut chunk = Vec::new();
        dec.fill(&mut chunk, 4410).unwrap(); // ~0.1 s
        assert!(dec.position_secs() > 0.05, "{}", dec.position_secs());
        dec.seek(1.0).unwrap();
        assert!((dec.position_secs() - 1.0).abs() < 0.15, "{}", dec.position_secs());
        assert!(dec.seek(99.0).is_err());
        assert!(dec.seek(-1.0).is_err());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn garbage_is_rejected_not_decoded() {
        let dir = std::env::temp_dir().join(format!("zett-decode-garbage-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("nada.txt");
        fs::write(&file, b"definitivamente no es audio").unwrap();
        assert!(FileDecoder::open(file.to_str().unwrap()).is_err());
        assert!(probe("/definitely/not/a/file/zett.mp3").is_err());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resample_is_transparent_when_rates_match() {
        let input = vec![0.1f32, -0.2, 0.3, -0.4];
        assert_eq!(resample_linear(&input, 2, 44_100, 44_100), input);
        assert!(resample_linear(&[], 2, 44_100, 48_000).is_empty());
    }

    #[test]
    fn resample_changes_length_proportionally() {
        let input: Vec<f32> = (0..4410).map(|n| (n as f32) / 4410.0).collect();
        let up = resample_linear(&input, 1, 44_100, 48_000);
        assert!((up.len() as f64 / 4800.0 - 1.0).abs() < 0.05, "len={}", up.len());
        assert!((up[0] - 0.0).abs() < 1e-3);
        assert!((up[up.len() - 1] - 1.0).abs() < 0.05);
    }

    #[test]
    fn eq_bypass_is_bit_transparent() {
        let mut eq = Eq3::new([0.0, 0.0, 0.0], 44_100, 2);
        assert!(eq.is_bypass());
        let mut samples = vec![0.25f32, -0.5, 0.75, 0.125];
        let before = samples.clone();
        eq.process(&mut samples);
        assert_eq!(samples, before);
    }

    #[test]
    fn eq_bass_boost_lifts_a_low_tone() {
        let rate = 44_100u32;
        let tone: Vec<f32> = (0..8192)
            .map(|n| (2.0 * std::f32::consts::PI * 110.0 * n as f32 / rate as f32).sin() * 0.4)
            .collect();
        let mut flat = tone.clone();
        Eq3::new([0.0, 0.0, 0.0], rate, 1).process(&mut flat);
        let mut boosted = tone.clone();
        Eq3::new([9.0, 0.0, 0.0], rate, 1).process(&mut boosted);
        let rms = |v: &[f32]| (v.iter().skip(2048).map(|s| s * s).sum::<f32>() / 6144.0).sqrt();
        assert!(rms(&boosted) > rms(&flat) * 1.5, "flat vs boosted");
    }

    #[test]
    fn spectrum_finds_the_tone_and_ignores_silence() {
        assert_eq!(spectrum_32(&[], 44_100), [0.0; 32]);
        assert_eq!(spectrum_32(&[0.0; 1024], 44_100), [0.0; 32]);
        let rate = 44_100u32;
        let tone: Vec<f32> = (0..2048)
            .map(|n| (2.0 * std::f32::consts::PI * 440.0 * n as f32 / rate as f32).sin() * 0.9)
            .collect();
        let bars = spectrum_32(&tone, rate);
        let peak = bars.iter().fold(0.0f32, |m, b| m.max(*b));
        assert!(peak > 0.5, "peak={peak}");
        // 440 Hz falls in the lower-middle bands, not at the edges.
        assert!(bars[0] < peak * 0.6, "low edge leaked: {}", bars[0]);
        assert!(bars[31] < peak * 0.6, "high edge leaked: {}", bars[31]);
    }
}
