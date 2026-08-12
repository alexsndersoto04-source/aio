//! Audio (`std::audio::*`) — real WAV I/O with `hound`, plus playback and
//! recording through Termux:API's `termux-media-player` and
//! `termux-microphone-record` binaries.
//!
//! Design:
//!
//! * **WAV I/O and synthesis** are 100% pure Rust (crate `hound`). They
//!   work on any machine — Termux, Linux desktop, macOS, CI — with no
//!   native audio dependencies at all. No ALSA, no AAudio, no PulseAudio.
//!   That keeps the Termux binary small and its build bulletproof.
//!
//! * **Actual playback and recording** shell out to the Termux:API tools
//!   (which use the real Android audio stack). If the user hasn't
//!   installed `termux-api`, every playback/recording helper returns a
//!   typed `AudioError::MissingCli` and the .titan program can degrade
//!   gracefully. Availability probe: `is_termux_media_available()`.

use std::f32::consts::PI;
use std::io::Cursor;
use std::path::Path;
use std::process::{Command, Stdio};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WAV error: {0}")]
    Wav(String),
    #[error("termux-api tool '{tool}' is not installed. Run: pkg install termux-api")]
    MissingCli { tool: String },
    #[error("termux-api '{tool}' failed: {stderr}")]
    Failed { tool: String, stderr: String },
    #[error("invalid parameter: {0}")]
    Invalid(String),
}

fn map_wav(error: hound::Error) -> AudioError {
    AudioError::Wav(error.to_string())
}

// ---------------- WAV I/O -----------------------------------------------

/// Read a WAV file into (samples, sample_rate, channels, bits_per_sample).
///
/// Samples are normalized to `f32` in `[-1.0, 1.0]`. This works both for
/// integer PCM and IEEE-float WAVs so `.titan` sees one consistent shape.
pub fn read_wav(path: &str) -> Result<(Vec<f32>, u32, u16, u16), AudioError> {
    let mut reader = WavReader::open(path).map_err(map_wav)?;
    let spec = reader.spec();
    let samples = collect_samples(&mut reader, &spec)?;
    Ok((
        samples,
        spec.sample_rate,
        spec.channels,
        spec.bits_per_sample,
    ))
}

/// Same as `read_wav`, but accepts the file as raw bytes (useful for
/// pipelines that pass through `std::http_full` or `std::compress`).
pub fn read_wav_bytes(bytes: &[u8]) -> Result<(Vec<f32>, u32, u16, u16), AudioError> {
    let mut reader = WavReader::new(Cursor::new(bytes)).map_err(map_wav)?;
    let spec = reader.spec();
    let samples = collect_samples(&mut reader, &spec)?;
    Ok((
        samples,
        spec.sample_rate,
        spec.channels,
        spec.bits_per_sample,
    ))
}

fn collect_samples<R: std::io::Read>(
    reader: &mut WavReader<R>,
    spec: &WavSpec,
) -> Result<Vec<f32>, AudioError> {
    match spec.sample_format {
        SampleFormat::Float => Ok(reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_wav)?),
        SampleFormat::Int => {
            // Scale integer samples to [-1.0, 1.0].
            let max = (1i64 << (spec.bits_per_sample as i64 - 1)) as f32;
            Ok(reader
                .samples::<i32>()
                .map(|value| value.map(|value| value as f32 / max))
                .collect::<Result<Vec<_>, _>>()
                .map_err(map_wav)?)
        }
    }
}

/// Write `samples` (interleaved if `channels > 1`, values in [-1.0, 1.0])
/// as a 16-bit PCM WAV file.
pub fn write_wav(
    path: &str,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), AudioError> {
    if channels == 0 {
        return Err(AudioError::Invalid("channels must be >= 1".into()));
    }
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(Path::new(path), spec).map_err(map_wav)?;
    for sample in samples {
        let clipped = sample.clamp(-1.0, 1.0);
        writer
            .write_sample((clipped * i16::MAX as f32) as i16)
            .map_err(map_wav)?;
    }
    writer.finalize().map_err(map_wav)
}

/// Encode `samples` as a WAV blob without touching the filesystem.
pub fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>, AudioError> {
    if channels == 0 {
        return Err(AudioError::Invalid("channels must be >= 1".into()));
    }
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut buffer, spec).map_err(map_wav)?;
        for sample in samples {
            let clipped = sample.clamp(-1.0, 1.0);
            writer
                .write_sample((clipped * i16::MAX as f32) as i16)
                .map_err(map_wav)?;
        }
        writer.finalize().map_err(map_wav)?;
    }
    Ok(buffer.into_inner())
}

// ---------------- Synthesis ---------------------------------------------

/// Generate a mono sine-wave sample buffer of `duration_ms` at `frequency_hz`.
pub fn sine_wave(
    frequency_hz: f32,
    duration_ms: u32,
    sample_rate: u32,
    amplitude: f32,
) -> Vec<f32> {
    let total = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let amplitude = amplitude.clamp(0.0, 1.0);
    (0..total)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            amplitude * (2.0 * PI * frequency_hz * t).sin()
        })
        .collect()
}

/// Square wave (harsh, retro-console vibe).
pub fn square_wave(
    frequency_hz: f32,
    duration_ms: u32,
    sample_rate: u32,
    amplitude: f32,
) -> Vec<f32> {
    let total = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let amplitude = amplitude.clamp(0.0, 1.0);
    (0..total)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            if (2.0 * PI * frequency_hz * t).sin() >= 0.0 {
                amplitude
            } else {
                -amplitude
            }
        })
        .collect()
}

/// Sawtooth wave.
pub fn saw_wave(frequency_hz: f32, duration_ms: u32, sample_rate: u32, amplitude: f32) -> Vec<f32> {
    let total = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let amplitude = amplitude.clamp(0.0, 1.0);
    let period = sample_rate as f32 / frequency_hz;
    (0..total)
        .map(|i| {
            let phase = (i as f32 % period) / period;
            amplitude * (2.0 * phase - 1.0)
        })
        .collect()
}

/// White noise (random values in [-amp, amp]).
pub fn white_noise(duration_ms: u32, sample_rate: u32, amplitude: f32) -> Vec<f32> {
    let total = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let amplitude = amplitude.clamp(0.0, 1.0);
    // Use a simple LCG so we don't depend on `rand` (which is already an
    // optional feature of Phase 1).
    let mut state: u32 = 0x9E37_79B9;
    (0..total)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let unit = (state >> 8) as f32 / (1u32 << 24) as f32; // [0, 1)
            amplitude * (unit * 2.0 - 1.0)
        })
        .collect()
}

/// Simple linear fade-in from 0 to full amplitude over `fade_ms`.
///
/// Endpoint-inclusive ramp: the first faded sample is exactly 0.0 and the
/// last one exactly 1.0, so a 1-sample fade still lands on zero instead of
/// stopping one step short (off-by-one that left residue amplitude > 0).
pub fn fade_in(samples: &mut [f32], sample_rate: u32, fade_ms: u32) {
    let fade = (sample_rate as u64 * fade_ms as u64 / 1000).min(samples.len() as u64) as usize;
    if fade == 0 {
        return;
    }
    let denom = fade.saturating_sub(1).max(1) as f32;
    for (i, sample) in samples.iter_mut().take(fade).enumerate() {
        *sample *= i as f32 / denom;
    }
}

/// Fade-out over the last `fade_ms`.
///
/// Endpoint-inclusive ramp: the last faded sample is exactly 0.0 even when
/// the ramp is a single sample long.
pub fn fade_out(samples: &mut [f32], sample_rate: u32, fade_ms: u32) {
    let fade = (sample_rate as u64 * fade_ms as u64 / 1000).min(samples.len() as u64) as usize;
    if fade == 0 {
        return;
    }
    let denom = fade.saturating_sub(1).max(1) as f32;
    let start = samples.len().saturating_sub(fade);
    for (i, sample) in samples.iter_mut().skip(start).enumerate() {
        *sample *= (fade - 1 - i) as f32 / denom;
    }
}

// ---------------- Playback & recording via termux-api -------------------

fn spawn(tool: &str, args: &[&str]) -> Result<Vec<u8>, AudioError> {
    let output = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AudioError::MissingCli { tool: tool.into() },
            _ => AudioError::Io(error),
        })?;
    if !output.status.success() {
        return Err(AudioError::Failed {
            tool: tool.into(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.stdout)
}

/// True if `termux-media-player` is on PATH (i.e. `pkg install termux-api`
/// was run on-device).
pub fn is_termux_media_available() -> bool {
    Command::new("termux-media-player")
        .arg("info")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// Start playing `path` in the background. Returns the tool's stdout for
/// inspection (usually just "Now Playing:").
pub fn play(path: &str) -> Result<String, AudioError> {
    let out = spawn("termux-media-player", &["play", path])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

pub fn pause() -> Result<String, AudioError> {
    let out = spawn("termux-media-player", &["pause"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
pub fn resume() -> Result<String, AudioError> {
    let out = spawn("termux-media-player", &["play"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
pub fn stop() -> Result<String, AudioError> {
    let out = spawn("termux-media-player", &["stop"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
pub fn info() -> Result<String, AudioError> {
    let out = spawn("termux-media-player", &["info"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Start recording to `path`. Returns immediately; the recording keeps
/// running until `record_stop()` is called.
///
/// Common formats accepted by the Android encoder: `aac`, `amr_wb`,
/// `amr_nb`. Passing a WAV path will produce whatever the phone picks.
pub fn record_start(path: &str, seconds: u32) -> Result<String, AudioError> {
    let seconds = seconds.to_string();
    let out = spawn("termux-microphone-record", &["-f", path, "-l", &seconds])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

pub fn record_stop() -> Result<String, AudioError> {
    let out = spawn("termux-microphone-record", &["-q"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

pub fn record_info() -> Result<String, AudioError> {
    let out = spawn("termux-microphone-record", &["-i"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_has_correct_length_and_range() {
        let samples = sine_wave(440.0, 100, 44_100, 0.5);
        assert_eq!(samples.len(), 4410);
        for value in &samples {
            assert!((-0.6..=0.6).contains(value));
        }
    }

    #[test]
    fn write_and_read_wav_round_trip() {
        let source = sine_wave(220.0, 50, 22_050, 0.8);
        let path = std::env::temp_dir().join(format!("titan-audio-{}.wav", std::process::id()));
        let path_string = path.to_string_lossy().to_string();
        write_wav(&path_string, &source, 22_050, 1).unwrap();
        let (back, rate, channels, bits) = read_wav(&path_string).unwrap();
        assert_eq!(rate, 22_050);
        assert_eq!(channels, 1);
        assert_eq!(bits, 16);
        assert_eq!(back.len(), source.len());
        // 16-bit round trip loses a bit of precision; give a wide window.
        for (a, b) in source.iter().zip(back.iter()) {
            assert!((a - b).abs() < 0.01, "sample mismatch: {a} vs {b}");
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn encode_wav_starts_with_riff_header() {
        let bytes = encode_wav(&sine_wave(660.0, 20, 8_000, 0.5), 8_000, 1).unwrap();
        assert!(bytes.starts_with(b"RIFF"));
        assert!(bytes.len() > 44);
    }

    #[test]
    fn synthesizers_produce_expected_lengths() {
        assert_eq!(square_wave(440.0, 100, 44_100, 0.5).len(), 4410);
        assert_eq!(saw_wave(440.0, 100, 44_100, 0.5).len(), 4410);
        assert_eq!(white_noise(100, 44_100, 0.5).len(), 4410);
    }

    #[test]
    fn fades_scale_edges() {
        let mut samples = vec![1.0f32; 100];
        fade_in(&mut samples, 100, 10); // 10 ms at 100 Hz -> 1 sample
        assert!(samples[0].abs() < 1e-6);
        fade_out(&mut samples, 100, 10);
        assert!(samples.last().unwrap().abs() < 1e-6);
    }

    #[test]
    fn missing_cli_is_typed() {
        let out = spawn("termux-audio-definitely-does-not-exist-xyz", &[]);
        assert!(matches!(out, Err(AudioError::MissingCli { .. })));
    }
}
