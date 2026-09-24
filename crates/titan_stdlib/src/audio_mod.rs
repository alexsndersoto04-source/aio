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
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use thiserror::Error;

#[cfg(feature = "audio_decode_mod")]
use symphonia::core::audio::SampleBuffer;
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::errors::Error as SymphoniaError;
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::formats::FormatOptions;
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::io::MediaSourceStream;
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::meta::MetadataOptions;
#[cfg(feature = "audio_decode_mod")]
use symphonia::core::probe::Hint;

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
    #[error("audio decode error: {0}")]
    Decode(String),
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

/// Start playing `path` in the background using whichever player is available.
///
/// Priority order: `termux-media-player` first so Android keeps behaving
/// exactly as before, then `mpv`, `ffplay`, `paplay` and `aplay` for desktop
/// Linux. Returns a description of what was started.
pub fn play(path: &str) -> Result<String, AudioError> {
    let backend = backend();
    if backend == "none" {
        return Err(AudioError::MissingCli {
            tool: "ningun reproductor (instala mpv, ffplay, pulseaudio-utils o alsa-utils; en Termux: pkg install termux-api)".into(),
        });
    }
    play_with(path, backend)
}

/// Play `path` with an explicit backend. Use `std::audio::backends()` to list
/// what this machine actually has.
pub fn play_with(path: &str, backend: &str) -> Result<String, AudioError> {
    match backend {
        "termux-media-player" => {
            let out = spawn("termux-media-player", &["play", path])?;
            Ok(String::from_utf8_lossy(&out).into_owned())
        }
        "mpv" => start_detached("mpv", &["--no-video", "--really-quiet", path]),
        "ffplay" => start_detached("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet", path]),
        "paplay" => start_detached("paplay", &[path]),
        "aplay" => start_detached("aplay", &["-q", path]),
        other => Err(AudioError::Invalid(format!(
            "backend de audio desconocido: '{other}'. Usa std::audio::backends() para ver los disponibles"
        ))),
    }
}

pub fn pause() -> Result<String, AudioError> {
    if !is_termux_media_available() {
        return Err(AudioError::Invalid(
            "pause solo esta soportado por termux-media-player; en escritorio usa stop() y play() de nuevo".into(),
        ));
    }
    let out = spawn("termux-media-player", &["pause"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
pub fn resume() -> Result<String, AudioError> {
    if !is_termux_media_available() {
        return Err(AudioError::Invalid(
            "resume solo esta soportado por termux-media-player; en escritorio usa play() de nuevo".into(),
        ));
    }
    let out = spawn("termux-media-player", &["play"])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
pub fn stop() -> Result<String, AudioError> {
    if is_termux_media_available() {
        let out = spawn("termux-media-player", &["stop"])?;
        return Ok(String::from_utf8_lossy(&out).into_owned());
    }
    Ok(stop_child())
}

// ---------------- Playback backends -----------------------------------
//
// Titan no enlaza ALSA/CoreAudio/WASAPI: la salida se delega a un reproductor
// del sistema. En Android ese reproductor es `termux-media-player` (que usa el
// MediaPlayer del SO y por tanto decodifica mp3/m4a). En escritorio se usa lo
// que este instalado. El hijo queda registrado para que `stop()` pueda matarlo.

/// Backends in priority order: (tool on PATH, args builder lives in play_with).
const BACKEND_PRIORITY: &[&str] = &["termux-media-player", "mpv", "ffplay", "paplay", "aplay"];

fn player_slot() -> &'static Mutex<Option<Child>> {
    static PLAYER: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    PLAYER.get_or_init(|| Mutex::new(None))
}

fn lock_player() -> std::sync::MutexGuard<'static, Option<Child>> {
    player_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// True if `tool` can be spawned at all. Only presence matters: `--version`
/// exits fast, and a nonzero exit still means the binary exists.
fn tool_present(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

fn backend_available(backend: &str) -> bool {
    if backend == "termux-media-player" {
        return is_termux_media_available();
    }
    tool_present(backend)
}

/// Every backend this machine has, in the order `play()` would try them.
pub fn backends() -> Vec<&'static str> {
    BACKEND_PRIORITY
        .iter()
        .copied()
        .filter(|backend| backend_available(backend))
        .collect()
}

/// The backend `play()` would use, or `"none"`.
pub fn backend() -> &'static str {
    BACKEND_PRIORITY
        .iter()
        .copied()
        .find(|backend| backend_available(backend))
        .unwrap_or("none")
}

/// Spawn a player without waiting for it, replacing whatever was playing.
fn start_detached(tool: &str, args: &[&str]) -> Result<String, AudioError> {
    stop_child();
    let child = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AudioError::MissingCli { tool: tool.into() },
            _ => AudioError::Io(error),
        })?;
    let pid = child.id();
    *lock_player() = Some(child);
    Ok(format!("{tool} pid={pid}"))
}

/// Kill the registered player, if any. Returns a human-readable result.
fn stop_child() -> String {
    let child = lock_player().take();
    match child {
        Some(mut child) => match child.kill() {
            Ok(()) => {
                let _ = child.wait();
                "reproduccion detenida".into()
            }
            Err(error) => format!("no se pudo detener: {error}"),
        },
        None => "no habia nada reproduciendose".into(),
    }
}

// ---------------- Decodificacion (symphonia) --------------------------
//
// Todo este bloque existe solo con la feature `audio_decode_mod`. Convierte
// cualquier contenedor/codec que symphonia entienda a PCM f32 interleaved,
// que es exactamente lo que ya consumen `write_wav` y `encode_wav`.

/// PCM plano tras decodificar: muestras f32 interleaved en `[-1.0, 1.0]`.
#[cfg(feature = "audio_decode_mod")]
pub struct Decoded {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
}

/// Metadata de un archivo de audio, sin decodificarlo entero.
#[cfg(feature = "audio_decode_mod")]
pub struct Probed {
    pub format: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub frames: u64,
}

/// Formatos que este build sabe decodificar.
#[cfg(feature = "audio_decode_mod")]
pub fn formats() -> Vec<&'static str> {
    vec![
        "wav", "aiff", "mp3", "mp2", "mp1", "flac", "ogg/vorbis", "m4a/aac", "alac", "adpcm", "mkv",
    ]
}

#[cfg(feature = "audio_decode_mod")]
fn decode_file(path: &str) -> Result<Decoded, AudioError> {
    let file = std::fs::File::open(path)?;
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension.as_deref() {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AudioError::Decode("el archivo no tiene ninguna pista de audio decodificable".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| AudioError::Decode(error.to_string()))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut buffer: Option<SampleBuffer<f32>> = None;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Fin del archivo o pista encadenada: para un reproductor eso es
            // simplemente "termino", no un error.
            Err(SymphoniaError::ResetRequired) => break,
            Err(SymphoniaError::IoError(ref error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => break,
            Err(error) => return Err(AudioError::Decode(error.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // Un paquete corrupto no tira la cancion entera: se salta.
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(AudioError::Decode(error.to_string())),
        };
        if buffer.is_none() {
            let spec = *decoded.spec();
            sample_rate = spec.rate;
            channels = spec.channels.count() as u16;
            buffer = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }
        if let Some(buffer) = buffer.as_mut() {
            buffer.copy_interleaved_ref(decoded);
            samples.extend_from_slice(buffer.samples());
        }
    }

    let frames = if channels == 0 {
        0
    } else {
        samples.len() as u64 / u64::from(channels)
    };
    Ok(Decoded { samples, sample_rate, channels, frames })
}

/// Decodifica cualquier formato soportado a PCM f32.
#[cfg(feature = "audio_decode_mod")]
pub fn decode(path: &str) -> Result<Decoded, AudioError> {
    decode_file(path)
}

/// Decodifica `src_path` y escribe el resultado como WAV en `dst_path`, que es
/// lo que los backends de reproduccion si saben tocar. Devuelve el PCM por si
/// ademas se quiere procesar (fades, mezcla, visualizador).
#[cfg(feature = "audio_decode_mod")]
pub fn decode_to_wav(src_path: &str, dst_path: &str) -> Result<Decoded, AudioError> {
    let decoded = decode_file(src_path)?;
    write_wav(dst_path, &decoded.samples, decoded.sample_rate, decoded.channels)?;
    Ok(decoded)
}

/// Metadata sin decodificar el audio completo.
#[cfg(feature = "audio_decode_mod")]
pub fn probe(path: &str) -> Result<Probed, AudioError> {
    let file = std::fs::File::open(path)?;
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = extension.as_deref() {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let format_name = probed.mime_type.unwrap_or_else(|| "desconocido".into());
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AudioError::Decode("el archivo no tiene ninguna pista de audio".into()))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(0);
    let channels = track
        .codec_params
        .channels
        .map(|channels| channels.count() as u16)
        .unwrap_or(0);
    let frames = track.codec_params.n_frames.unwrap_or(0);
    let duration_ms = match (track.codec_params.time_base, track.codec_params.n_frames) {
        (Some(time_base), Some(frames)) => {
            let time = time_base.calc_time(frames);
            time.seconds.saturating_mul(1000) + (time.frac * 1000.0) as u64
        }
        _ => 0,
    };

    Ok(Probed { format: format_name, sample_rate, channels, duration_ms, frames })
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

    #[cfg(feature = "audio_decode_mod")]
    #[test]
    fn decode_round_trips_a_wav() {
        let file = std::env::temp_dir().join(format!("titan-decode-{}.wav", std::process::id()));
        let path = file.to_string_lossy().into_owned();
        // 440 Hz, 250 ms, mono a 44.1 kHz -> 11_025 muestras.
        let original = sine_wave(440.0, 250, 44_100, 0.8);
        write_wav(&path, &original, 44_100, 1).expect("write_wav");

        let decoded = decode(&path).expect("decode");
        let _ = std::fs::remove_file(&path);

        assert_eq!(decoded.sample_rate, 44_100);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), original.len());
        assert_eq!(decoded.frames, original.len() as u64);
        // write_wav cuantiza a 16 bits, asi que se tolera el error de redondeo.
        for (expected, actual) in original.iter().zip(decoded.samples.iter()) {
            assert!((expected - actual).abs() < 1e-3, "deriva al decodificar");
        }
    }

    #[cfg(feature = "audio_decode_mod")]
    #[test]
    fn probe_reads_metadata_without_decoding() {
        let file = std::env::temp_dir().join(format!("titan-probe-{}.wav", std::process::id()));
        let path = file.to_string_lossy().into_owned();
        let samples = sine_wave(440.0, 250, 44_100, 0.8);
        write_wav(&path, &samples, 44_100, 1).expect("write_wav");

        let probed = probe(&path).expect("probe");
        let _ = std::fs::remove_file(&path);

        assert_eq!(probed.sample_rate, 44_100);
        assert_eq!(probed.channels, 1);
        assert!(probed.frames >= 11_000, "frames = {}", probed.frames);
        // 250 ms exactos; se deja margen por el redondeo de la base de tiempo.
        assert!((240..=260).contains(&probed.duration_ms), "duration_ms = {}", probed.duration_ms);
        assert!(
            probed.format.contains("wav") || probed.format.contains("wave"),
            "format = {}",
            probed.format
        );
    }

    #[cfg(feature = "audio_decode_mod")]
    #[test]
    fn decode_to_wav_writes_a_readable_file() {
        let id = std::process::id();
        let src = std::env::temp_dir().join(format!("titan-d2w-src-{id}.wav"));
        let dst = std::env::temp_dir().join(format!("titan-d2w-dst-{id}.wav"));
        let src = src.to_string_lossy().into_owned();
        let dst = dst.to_string_lossy().into_owned();
        let samples = sine_wave(220.0, 100, 22_050, 0.5);
        write_wav(&src, &samples, 22_050, 1).expect("write_wav");

        let decoded = decode_to_wav(&src, &dst).expect("decode_to_wav");
        assert_eq!(decoded.sample_rate, 22_050);

        // El destino tiene que volver a leerse con el camino WAV existente.
        let (reread, sample_rate, channels, _bits) = read_wav(&dst).expect("read_wav");
        assert_eq!(sample_rate, 22_050);
        assert_eq!(channels, 1);
        assert_eq!(reread.len(), decoded.samples.len());

        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn backends_only_reports_known_players() {
        const KNOWN: &[&str] = &["termux-media-player", "mpv", "ffplay", "paplay", "aplay"];
        for backend in backends() {
            assert!(KNOWN.contains(&backend), "backend inesperado: {backend}");
        }
        let chosen = backend();
        assert!(chosen == "none" || KNOWN.contains(&chosen), "backend = {chosen}");
    }
}
