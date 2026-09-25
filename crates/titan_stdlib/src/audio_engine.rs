//! Native playback engine (`std::audio::engine_*`) — Fase 42A.
//!
//! Titan's own sound: [`crate::audio_decode`] turns MP3/FLAC/Vorbis/Opus/WAV
//! into PCM and this module pushes it to the speakers through `cpal`
//! (CoreAudio / WASAPI / ALSA). No mpv, no afplay, no helpers — the
//! binary you ship IS the player.
//!
//! Honest availability contract:
//!
//! * Linux / Windows / macOS with an output device: full playback.
//! * Headless boxes (CI, containers): [`play`] reports [`EngineError::NoDevice`]
//!   instead of pretending; [`decode_report`] still verifies files.
//! * Android: `cpal` is compiled out (the app shell owns audio there, Fase
//!   42B); decode, queue management and config work, output answers
//!   [`EngineError::Unsupported`].
//!
//! Design: one worker thread decodes ahead into a shared ring (`~8 s`),
//! the `cpal` callback drains it. Volume/EQ/crossfade apply in the worker
//! so the real-time callback only memcpys. Everything the future settings
//! screen needs (volume, crossfade, gapless, EQ, device) already lives in
//! [`EngineConfig`] + [`EngineStatus`].

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use thiserror::Error;

#[cfg(not(target_os = "android"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio_decode::{self, Eq3, FileDecoder};

/// Errors produced by the native playback engine.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("decode error: {0}")]
    Decode(String),
    #[error("no audio output device available (headless machine?)")]
    NoDevice,
    #[error("nothing is playing right now")]
    Idle,
    #[error("unsupported on this target: {0}")]
    Unsupported(String),
    #[error("invalid parameter: {0}")]
    Invalid(String),
    #[error("output error: {0}")]
    Control(String),
}

/// Everything the settings screen can tune. Survives across tracks:
/// `play()` seeds the new session from the last saved config.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// `0.0..=1.0` linear gain.
    pub volume: f32,
    /// Overlap between tracks, `0.0..=12.0` seconds. `0.0` disables.
    pub crossfade_secs: f64,
    /// No silence inserted between tracks.
    pub gapless: bool,
    /// Bass / mid / treble in dB, each `-12.0..=12.0`.
    pub eq_db: [f32; 3],
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            volume: 0.8,
            crossfade_secs: 0.0,
            gapless: true,
            eq_db: [0.0, 0.0, 0.0],
        }
    }
}

/// Full snapshot for `std::audio::engine_status` (drives UI + settings).
#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub state: String,
    pub path: String,
    pub codec: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub volume_percent: i64,
    pub crossfade_secs: f64,
    pub gapless: bool,
    pub eq_db: [i64; 3],
    pub queue_len: usize,
    pub device: String,
}

/// Headless decode check for `std::audio::engine_decode`.
#[derive(Debug, Clone)]
pub struct DecodeReport {
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: String,
    pub verified_secs: f64,
    pub peak: f32,
}

/// Now-playing snapshot for `std::audio::engine_current`.
#[derive(Debug, Clone)]
pub struct CurrentTrack {
    pub path: String,
    pub codec: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub state: String,
    pub queue_len: usize,
}

const CHUNK_FRAMES: usize = 4096;
const RING_CAP: usize = 4096;
const MAX_QUEUE: usize = 500;
const HISTORY_CAP: usize = 200;
const VERIFY_SECS: f64 = 5.0;
const PREV_RESTART_SECS: f64 = 3.0;

// ------------------------------------------------------------ shared state

struct Shared {
    buf: VecDeque<f32>,
    cap_samples: usize,
    /// Output channels; only the `cpal` callback reads it.
    #[cfg(not(target_os = "android"))]
    channels: u16,
    /// Callback-side pause gate (mirror of `Control::paused`, set together).
    /// Lets pause/resume work from any thread; the stream itself stays put.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    paused: AtomicBool,
    /// Output-rate frames the callback has rendered, all tracks.
    frames_rendered: u64,
}

struct CurrentMeta {
    path: String,
    codec: String,
    duration_secs: f64,
    rate: u32,
    channels: u16,
    /// `frames_rendered` when this track (re)started at the output.
    start_frame: u64,
}

struct Control {
    stop: AtomicBool,
    paused: AtomicBool,
    seek_to: Mutex<Option<f64>>,
    switch_to: Mutex<Option<String>>,
    queue: Mutex<VecDeque<String>>,
    history: Mutex<Vec<String>>,
    current: Mutex<CurrentMeta>,
    config: Mutex<EngineConfig>,
    /// Mono mix post-FX for the visualizer, newest at the back.
    levels_ring: Mutex<VecDeque<f32>>,
}

struct Engine {
    worker: Option<JoinHandle<()>>,
    control: Option<Arc<Control>>,
    /// Created once with the output stream, reused across plays so the
    /// `cpal` callback never points at a dead buffer.
    shared: Option<Arc<Mutex<Shared>>>,
    device_label: String,
    out_rate: u32,
    out_channels: u16,
    /// Last saved settings (seed for the next session).
    config: EngineConfig,
    /// Queued before the first `play()`; drained into the session.
    pending_queue: VecDeque<String>,
}

impl Engine {
    fn fresh() -> Self {
        Engine {
            worker: None,
            control: None,
            shared: None,
            device_label: String::new(),
            out_rate: 0,
            out_channels: 0,
            config: EngineConfig::default(),
            pending_queue: VecDeque::new(),
        }
    }
}

fn slot() -> &'static Mutex<Option<Engine>> {
    static SLOT: OnceLock<Mutex<Option<Engine>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_slot() -> std::sync::MutexGuard<'static, Option<Engine>> {
    slot().lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// `cpal::Stream` is `!Send` by design (same story as `minifb::Window`),
// so it lives thread-locally on the OS thread that first called `play()`.
// Everything else (buffer, queue, config, position) is `Send` and stays
// global, and pause/resume also gate the callback + worker flags, so
// transport from another thread degrades instead of breaking.
#[cfg(not(target_os = "android"))]
struct Output {
    stream: cpal::Stream,
}

#[cfg(not(target_os = "android"))]
thread_local! {
    static OUTPUT: std::cell::RefCell<Option<Output>> =
        const { std::cell::RefCell::new(None) };
}

/// Set once the owner thread builds the stream: a second thread calling
/// `play()` gets an honest error instead of racing a second stream.
#[cfg(not(target_os = "android"))]
static STREAM_TAKEN: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_os = "android"))]
enum StreamOp {
    Play,
    Pause,
}

/// Best-effort control of the owner thread's stream (no-op elsewhere;
// the pause flags already stop the sound on every thread).
#[cfg(not(target_os = "android"))]
fn tls_stream(op: StreamOp) {
    let _ = OUTPUT.try_with(|output| {
        if let Some(held) = output.borrow().as_ref() {
            match op {
                StreamOp::Play => {
                    let _ = held.stream.play();
                }
                StreamOp::Pause => {
                    let _ = held.stream.pause();
                }
            }
        }
    });
}

/// Join a worker that finished on its own (queue drained).
fn reap_finished(eng: &mut Engine) {
    let done = eng
        .worker
        .as_ref()
        .map(|w| w.is_finished())
        .unwrap_or(false);
    if done {
        if let Some(worker) = eng.worker.take() {
            let _ = worker.join();
        }
        eng.control = None;
    }
}

fn stop_locked(eng: &mut Engine) {
    if let Some(control) = eng.control.as_ref() {
        control.stop.store(true, Ordering::SeqCst);
    }
    if let Some(worker) = eng.worker.take() {
        let _ = worker.join();
    }
    eng.control = None;
    if let Some(shared) = eng.shared.as_ref() {
        let mut held = crate::native::lock_recover(shared);
        held.buf.clear();
        held.paused.store(false, Ordering::SeqCst);
    }
    #[cfg(not(target_os = "android"))]
    tls_stream(StreamOp::Pause);
}

// ------------------------------------------------------------------ output

#[cfg(not(target_os = "android"))]
fn pull_frames(shared: &Mutex<Shared>, n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    if let Ok(mut s) = shared.try_lock() {
        if s.paused.load(Ordering::SeqCst) {
            // Silence without draining: resume continues seamlessly.
            return out;
        }
        let take = s.buf.len().min(n);
        for slot in out.iter_mut().take(take) {
            *slot = s.buf.pop_front().unwrap_or(0.0);
        }
        s.frames_rendered += (take / s.channels.max(1) as usize) as u64;
    }
    out
}

#[cfg(not(target_os = "android"))]
fn render_f32(shared: &Mutex<Shared>, data: &mut [f32]) {
    let pulled = pull_frames(shared, data.len());
    data.copy_from_slice(&pulled);
}

#[cfg(not(target_os = "android"))]
fn render_i16(shared: &Mutex<Shared>, data: &mut [i16]) {
    let n = data.len();
    for (slot, v) in data.iter_mut().zip(pull_frames(shared, n).iter()) {
        *slot = (v.clamp(-1.0, 1.0) * 32767.0) as i16;
    }
}

#[cfg(not(target_os = "android"))]
fn render_u16(shared: &Mutex<Shared>, data: &mut [u16]) {
    let n = data.len();
    for (slot, v) in data.iter_mut().zip(pull_frames(shared, n).iter()) {
        *slot = ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16;
    }
}

#[cfg(not(target_os = "android"))]
fn stream_error(err: cpal::StreamError) {
    eprintln!("[titan-audio] output stream error: {err}");
}

/// Build the output once: device facts + shared buffer go global (all
/// `Send`), the `!Send` stream stays thread-local on this OS thread.
#[cfg(not(target_os = "android"))]
fn ensure_output(eng: &mut Engine) -> Result<(), EngineError> {
    if eng.shared.is_some() {
        let owned_here = OUTPUT.try_with(|o| o.borrow().is_some()).unwrap_or(false);
        if owned_here {
            return Ok(());
        }
        return Err(EngineError::Unsupported(
            "audio output is owned by another OS thread; keep engine_* transport on one thread"
                .to_string(),
        ));
    }
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(EngineError::NoDevice)?;
    eng.device_label = device.name().unwrap_or_else(|_| "default".to_string());
    let supported = device
        .default_output_config()
        .map_err(|e| EngineError::Control(e.to_string()))?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    eng.out_rate = config.sample_rate.0;
    eng.out_channels = config.channels.max(1);
    let shared = Arc::new(Mutex::new(Shared {
        buf: VecDeque::new(),
        cap_samples: eng.out_rate as usize * eng.out_channels as usize * 8,
        channels: eng.out_channels,
        paused: AtomicBool::new(false),
        frames_rendered: 0,
    }));
    eng.shared = Some(Arc::clone(&shared));
    if STREAM_TAKEN.swap(true, Ordering::SeqCst) {
        eng.shared = None;
        return Err(EngineError::Unsupported(
            "audio output is owned by another OS thread; keep engine_* transport on one thread"
                .to_string(),
        ));
    }
    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| render_f32(&shared, data),
            stream_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| render_i16(&shared, data),
            stream_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| render_u16(&shared, data),
            stream_error,
            None,
        ),
        _ => {
            eng.shared = None;
            STREAM_TAKEN.store(false, Ordering::SeqCst);
            return Err(EngineError::Unsupported("output sample format".to_string()));
        }
    }
    .map_err(|e| {
        eng.shared = None;
        STREAM_TAKEN.store(false, Ordering::SeqCst);
        EngineError::Control(e.to_string())
    })?;
    let stored = OUTPUT
        .try_with(|o| {
            *o.borrow_mut() = Some(Output { stream });
        })
        .is_ok();
    if !stored {
        eng.shared = None;
        STREAM_TAKEN.store(false, Ordering::SeqCst);
        return Err(EngineError::Control(
            "audio thread state is gone".to_string(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "android")]
fn ensure_output(_eng: &mut Engine) -> Result<(), EngineError> {
    Err(EngineError::Unsupported(
        "native output is wired by the app shell (Fase 42B); decode-only on this target".to_string(),
    ))
}

// ------------------------------------------------------------------ worker

struct Overlap {
    next: FileDecoder,
    path: String,
    total: usize,
    done: usize,
}

fn push_history(control: &Control, path: &str) {
    if path.is_empty() {
        return;
    }
    let mut history = crate::native::lock_recover(&control.history);
    if history.last().map(|p| p.as_str()) == Some(path) {
        return;
    }
    history.push(path.to_string());
    if history.len() > HISTORY_CAP {
        history.remove(0);
    }
}

fn refresh_current_meta(
    control: &Control,
    shared: &Mutex<Shared>,
    decoder: &FileDecoder,
    path: &str,
) {
    let rendered = crate::native::lock_recover(shared).frames_rendered;
    let mut current = crate::native::lock_recover(&control.current);
    current.path = path.to_string();
    current.codec = decoder.codec().to_string();
    current.duration_secs = decoder.duration_secs();
    current.rate = decoder.sample_rate();
    current.channels = decoder.channels();
    current.start_frame = rendered;
}

fn convert_channels(input: &[f32], from: usize, to: usize) -> Vec<f32> {
    let from = from.max(1);
    let to = to.max(1);
    if from == to {
        return input.to_vec();
    }
    let frames = input.len() / from;
    let mut out = Vec::with_capacity(frames * to);
    for n in 0..frames {
        if to == 1 {
            let mut acc = 0.0f32;
            for c in 0..from {
                acc += input[n * from + c];
            }
            out.push(acc / from as f32);
        } else if from == 1 {
            for _ in 0..to {
                out.push(input[n]);
            }
        } else {
            for c in 0..to {
                out.push(if c < from { input[n * from + c] } else { 0.0 });
            }
        }
    }
    out
}

/// Push PCM into the ring, waiting when full (decode runs ahead of sound).
fn push_blocking(control: &Control, shared: &Mutex<Shared>, samples: &[f32]) {
    let mut idx = 0;
    while idx < samples.len() {
        if control.stop.load(Ordering::SeqCst) {
            return;
        }
        if control.paused.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let mut s = crate::native::lock_recover(shared);
        let room = s.cap_samples.saturating_sub(s.buf.len());
        if room == 0 {
            drop(s);
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        let take = room.min(samples.len() - idx);
        s.buf.extend(samples[idx..idx + take].iter().copied());
        idx += take;
    }
}

fn worker_main(control: Arc<Control>, shared: Arc<Mutex<Shared>>, out_rate: u32, out_ch: u16) {
    let out_rate = out_rate.max(8000);
    let out_ch = out_ch.max(1) as usize;
    // Definite assignment below (the open() match returns on failure),
    // so no dummy initializer that would trip `unused_assignments`.
    let mut decoder: Option<FileDecoder>;
    let mut overlap: Option<Overlap> = None;
    let mut chunk: Vec<f32> = Vec::with_capacity(CHUNK_FRAMES * out_ch);
    let mut eq = Eq3::new([0.0, 0.0, 0.0], out_rate, out_ch);
    let mut eq_cfg = [0.0f32; 3];

    let first = crate::native::lock_recover(&control.current).path.clone();
    if first.is_empty() {
        return;
    }
    match FileDecoder::open(&first) {
        Ok(d) => {
            refresh_current_meta(&control, &shared, &d, &first);
            decoder = Some(d);
        }
        // play() probes first, so this is unreachable in practice.
        Err(_) => return,
    }

    loop {
        if control.stop.load(Ordering::SeqCst) {
            return;
        }
        if control.paused.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }
        // Hard switch requested by next()/prev().
        if let Some(path) = crate::native::lock_recover(&control.switch_to).take() {
            match FileDecoder::open(&path) {
                Ok(d) => {
                    overlap = None;
                    crate::native::lock_recover(&shared).buf.clear();
                    refresh_current_meta(&control, &shared, &d, &path);
                    decoder = Some(d);
                }
                // Rotten file: drop the decoder, the advance path below
                // skips to whatever is queued next.
                Err(_) => decoder = None,
            }
        }
        // Seek requested by seek().
        if let Some(secs) = crate::native::lock_recover(&control.seek_to).take() {
            if let Some(d) = decoder.as_mut() {
                // The API validated the range; a miss here just keeps audio.
                let _ = d.seek(secs);
                overlap = None;
            }
        }
        // No decoder: advance from the queue or exit after draining.
        if decoder.is_none() {
            let next_path = crate::native::lock_recover(&control.queue).pop_front();
            match next_path {
                Some(path) => match FileDecoder::open(&path) {
                    Ok(d) => {
                        let prev = crate::native::lock_recover(&control.current).path.clone();
                        push_history(&control, &prev);
                        refresh_current_meta(&control, &shared, &d, &path);
                        decoder = Some(d);
                    }
                    Err(_) => continue,
                },
                None => {
                    if crate::native::lock_recover(&shared).buf.is_empty() {
                        crate::native::lock_recover(&control.current).path.clear();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            continue;
        }

        // Decode one chunk (scoped borrow: the advance logic below moves
        // `decoder`, so nothing here may hold it across statements).
        let (file_rate, file_ch, eof_now, decoded_pos) = {
            let d = match decoder.as_mut() {
                Some(d) => d,
                None => continue,
            };
            chunk.clear();
            if d.fill(&mut chunk, CHUNK_FRAMES).is_err() {
                chunk.clear();
            }
            (
                d.sample_rate().max(8000),
                d.channels().max(1) as usize,
                d.is_finished() && chunk.is_empty(),
                d.position_secs(),
            )
        };
        if chunk.is_empty() && !eof_now {
            // Guard spin (hostile file): never busy-loop the VM.
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        let cfg = crate::native::lock_recover(&control.config).clone();
        if cfg.eq_db != eq_cfg {
            eq = Eq3::new(cfg.eq_db, out_rate, out_ch);
            eq_cfg = cfg.eq_db;
        }

        // Overlap trigger: RENDERED tail (decode runs ~8 s ahead, so the
        // decoder's own clock would start the fade way too early).
        if overlap.is_none() && cfg.crossfade_secs > 0.0 {
            let (rendered, start_frame, duration) = {
                let s = crate::native::lock_recover(&shared);
                let c = crate::native::lock_recover(&control.current);
                (s.frames_rendered, c.start_frame, c.duration_secs)
            };
            let rendered_pos =
                rendered.saturating_sub(start_frame) as f64 / out_rate.max(1) as f64;
            let remaining = duration - rendered_pos;
            if duration > 0.0 && remaining < cfg.crossfade_secs && remaining > 0.25 {
                if let Some(path) = crate::native::lock_recover(&control.queue).pop_front() {
                    if let Ok(next) = FileDecoder::open(&path) {
                        overlap = Some(Overlap {
                            next,
                            path,
                            total: ((cfg.crossfade_secs * out_rate as f64) as usize).max(1),
                            done: 0,
                        });
                    } else {
                        // Rotten next file: put the queue back as it was.
                        crate::native::lock_recover(&control.queue).push_front(path);
                    }
                }
            }
        }

        // Resample file PCM to the output shape.
        let mut main = audio_decode::resample_linear(&chunk, file_ch, file_rate, out_rate);
        main = convert_channels(&main, file_ch, out_ch);

        // Crossfade mix against the head of the next track.
        let mut promote = false;
        if let Some(ov) = overlap.as_mut() {
            let mut head = Vec::new();
            let _ = ov
                .next
                .fill(&mut head, main.len() / out_ch.max(1));
            let ov_ch = ov.next.channels().max(1) as usize;
            let mut head_rs =
                audio_decode::resample_linear(&head, ov_ch, ov.next.sample_rate(), out_rate);
            head_rs = convert_channels(&head_rs, ov_ch, out_ch);
            head_rs.resize(main.len(), 0.0);
            let total = ov.total.max(1);
            for (i, v) in main.iter_mut().enumerate() {
                let frame = ov.done + i / out_ch.max(1);
                let t = (frame as f32 / total as f32).clamp(0.0, 1.0);
                *v = *v * (1.0 - t) + head_rs[i] * t;
            }
            ov.done += main.len() / out_ch.max(1);
            if eof_now || ov.done >= total || (ov.next.is_finished() && head.is_empty()) {
                promote = true;
            }
        }
        if promote {
            if let Some(ov) = overlap.take() {
                let prev = crate::native::lock_recover(&control.current).path.clone();
                push_history(&control, &prev);
                refresh_current_meta(&control, &shared, &ov.next, &ov.path);
                decoder = Some(ov.next);
            }
        }

        // Volume + EQ + safety clamp (EQ can push past full scale).
        for v in main.iter_mut() {
            *v *= cfg.volume;
        }
        eq.process(&mut main);
        for v in main.iter_mut() {
            *v = v.clamp(-1.0, 1.0);
        }

        // Visualizer feed: mono mix of exactly what hits the speakers.
        {
            let mut ring = crate::native::lock_recover(&control.levels_ring);
            for n in 0..main.len() / out_ch {
                let mut acc = 0.0f32;
                for c in 0..out_ch {
                    acc += main[n * out_ch + c];
                }
                ring.push_back(acc / out_ch as f32);
            }
            while ring.len() > RING_CAP {
                ring.pop_front();
            }
        }

        push_blocking(&control, &shared, &main);

        // Plain EOF (no overlap in flight): finalize, advance or exit.
        if eof_now && overlap.is_none() {
            if decoded_pos > 0.0 {
                let mut current = crate::native::lock_recover(&control.current);
                if current.duration_secs <= 0.0 {
                    current.duration_secs = decoded_pos;
                }
            }
            let next_path = crate::native::lock_recover(&control.queue).pop_front();
            match next_path {
                Some(path) => match FileDecoder::open(&path) {
                    Ok(d) => {
                        if !cfg.gapless {
                            let silence = vec![0.0f32; out_rate as usize * out_ch / 4];
                            push_blocking(&control, &shared, &silence);
                        }
                        let prev = crate::native::lock_recover(&control.current).path.clone();
                        push_history(&control, &prev);
                        refresh_current_meta(&control, &shared, &d, &path);
                        decoder = Some(d);
                    }
                    Err(_) => decoder = None,
                },
                None => {
                    if crate::native::lock_recover(&shared).buf.is_empty() {
                        crate::native::lock_recover(&control.current).path.clear();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

// ------------------------------------------------------------------ API

/// Start playing `path` (stopping whatever played before).
pub fn play(path: &str) -> Result<String, EngineError> {
    // Fase 43: `cloud:<id>` no es un archivo; el probe lo valida.
    if crate::audio_decode::cloud_id(path).is_none() && !std::path::Path::new(path).is_file() {
        return Err(EngineError::Invalid(format!("file not found: {path}")));
    }
    // Fail fast on undecodable files instead of spawning a worker that dies.
    let info = audio_decode::probe(path).map_err(|e| EngineError::Decode(e.to_string()))?;
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    reap_finished(eng);
    // The outgoing track goes to history; the queue survives the switch.
    let (mut saved_queue, mut saved_history) = match eng.control.as_ref() {
        Some(control) => (
            crate::native::lock_recover(&control.queue).clone(),
            crate::native::lock_recover(&control.history).clone(),
        ),
        None => (VecDeque::new(), Vec::new()),
    };
    if let Some(control) = eng.control.as_ref() {
        let outgoing = crate::native::lock_recover(&control.current).path.clone();
        if !outgoing.is_empty() {
            saved_history.push(outgoing);
        }
    }
    // Plus anything queued before the first play().
    saved_queue.extend(std::mem::take(&mut eng.pending_queue));
    stop_locked(eng);
    ensure_output(eng)?;
    let shared = eng.shared.clone().ok_or(EngineError::NoDevice)?;
    let control = Arc::new(Control {
        stop: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        seek_to: Mutex::new(None),
        switch_to: Mutex::new(None),
        queue: Mutex::new(saved_queue),
        history: Mutex::new(saved_history),
        current: Mutex::new(CurrentMeta {
            path: path.to_string(),
            codec: info.codec.clone(),
            duration_secs: info.duration_secs,
            rate: info.sample_rate,
            channels: info.channels,
            start_frame: 0,
        }),
        config: Mutex::new(eng.config.clone()),
        levels_ring: Mutex::new(VecDeque::new()),
    });
    let worker_control = Arc::clone(&control);
    let worker_shared = Arc::clone(&shared);
    let (out_rate, out_channels) = (eng.out_rate, eng.out_channels);
    eng.control = Some(control);
    eng.worker = Some(std::thread::spawn(move || {
        worker_main(worker_control, worker_shared, out_rate, out_channels);
    }));
    if let Some(shared) = eng.shared.as_ref() {
        crate::native::lock_recover(shared)
            .paused
            .store(false, Ordering::SeqCst);
    }
    #[cfg(not(target_os = "android"))]
    tls_stream(StreamOp::Play);
    Ok(format!("playing {path} via hifi-engine"))
}

/// Stop playback. The queue is dropped with the session.
pub fn stop() -> Result<String, EngineError> {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return Err(EngineError::Idle),
    };
    reap_finished(eng);
    if eng.control.is_none() {
        return Err(EngineError::Idle);
    }
    stop_locked(eng);
    Ok("stopped".to_string())
}

pub fn pause() -> Result<String, EngineError> {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return Err(EngineError::Idle),
    };
    reap_finished(eng);
    let control = eng.control.clone().ok_or(EngineError::Idle)?;
    control.paused.store(true, Ordering::SeqCst);
    if let Some(shared) = eng.shared.as_ref() {
        crate::native::lock_recover(shared)
            .paused
            .store(true, Ordering::SeqCst);
    }
    #[cfg(not(target_os = "android"))]
    tls_stream(StreamOp::Pause);
    Ok("paused".to_string())
}

pub fn resume() -> Result<String, EngineError> {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return Err(EngineError::Idle),
    };
    reap_finished(eng);
    let control = eng.control.clone().ok_or(EngineError::Idle)?;
    control.paused.store(false, Ordering::SeqCst);
    if let Some(shared) = eng.shared.as_ref() {
        crate::native::lock_recover(shared)
            .paused
            .store(false, Ordering::SeqCst);
    }
    #[cfg(not(target_os = "android"))]
    tls_stream(StreamOp::Play);
    Ok("resumed".to_string())
}

/// Seconds into the current track (`-1.0` when idle, like `player_position`).
pub fn position_secs() -> f64 {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return -1.0,
    };
    reap_finished(eng);
    let (control, shared) = match (eng.control.clone(), eng.shared.clone()) {
        (Some(c), Some(s)) => (c, s),
        _ => return -1.0,
    };
    let rate = eng.out_rate.max(1) as f64;
    drop(guard);
    let rendered = crate::native::lock_recover(&shared).frames_rendered;
    let current = crate::native::lock_recover(&control.current);
    let pos = rendered.saturating_sub(current.start_frame) as f64 / rate;
    if current.duration_secs > 0.0 {
        pos.min(current.duration_secs)
    } else {
        pos
    }
}

/// Duration of the current track (`0.0` when idle or unknown).
pub fn duration_secs() -> f64 {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return 0.0,
    };
    reap_finished(eng);
    match eng.control.clone() {
        Some(control) => crate::native::lock_recover(&control.current).duration_secs,
        None => 0.0,
    }
}

/// `"idle"`, `"playing"` or `"paused"`.
pub fn state_string() -> String {
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return "idle".to_string(),
    };
    reap_finished(eng);
    match eng.control.as_ref() {
        None => "idle".to_string(),
        Some(control) => {
            if control.paused.load(Ordering::SeqCst) {
                "paused".to_string()
            } else {
                "playing".to_string()
            }
        }
    }
}

/// Sample-accurate seek. Reads back the target immediately; the worker
/// lands the decoder a few milliseconds later.
pub fn seek(secs: f64) -> Result<String, EngineError> {
    if !secs.is_finite() || secs < 0.0 {
        return Err(EngineError::Invalid(format!("seek={secs}")));
    }
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return Err(EngineError::Idle),
    };
    reap_finished(eng);
    let (control, shared) = match (eng.control.clone(), eng.shared.clone()) {
        (Some(c), Some(s)) => (c, s),
        _ => return Err(EngineError::Idle),
    };
    let duration = crate::native::lock_recover(&control.current).duration_secs;
    if duration > 0.0 && secs > duration {
        return Err(EngineError::Invalid(format!(
            "seek {secs:.1}s exceeds {duration:.1}s"
        )));
    }
    *crate::native::lock_recover(&control.seek_to) = Some(secs);
    {
        let mut s = crate::native::lock_recover(&shared);
        s.buf.clear();
        let target = (secs * eng.out_rate.max(1) as f64) as u64;
        crate::native::lock_recover(&control.current).start_frame =
            s.frames_rendered.saturating_sub(target);
    }
    Ok(format!("seek to {secs:.1}s"))
}

/// Master volume, `0..=100`. Saved: survives across tracks and sessions.
pub fn set_volume(percent: i64) -> Result<bool, EngineError> {
    if !(0..=100).contains(&percent) {
        return Err(EngineError::Invalid(format!("volume={percent}")));
    }
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    eng.config.volume = percent as f32 / 100.0;
    if let Some(control) = eng.control.clone() {
        crate::native::lock_recover(&control.config).volume = eng.config.volume;
    }
    Ok(true)
}

/// Crossfade overlap, `0.0..=12.0` seconds (`0.0` disables). Saved.
pub fn set_crossfade(secs: f64) -> Result<bool, EngineError> {
    if !secs.is_finite() || secs < 0.0 || secs > 12.0 {
        return Err(EngineError::Invalid(format!("crossfade={secs}")));
    }
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    eng.config.crossfade_secs = secs;
    if let Some(control) = eng.control.clone() {
        crate::native::lock_recover(&control.config).crossfade_secs = secs;
    }
    Ok(true)
}

/// Gapless advance (no silence between tracks). Saved.
pub fn set_gapless(on: bool) -> Result<bool, EngineError> {
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    eng.config.gapless = on;
    if let Some(control) = eng.control.clone() {
        crate::native::lock_recover(&control.config).gapless = on;
    }
    Ok(true)
}

/// 3-band EQ in dB, each `-12..=12` (bass, mid, treble). Saved.
pub fn set_eq(bass: i64, mid: i64, treble: i64) -> Result<bool, EngineError> {
    for (name, v) in [("bass", bass), ("mid", mid), ("treble", treble)] {
        if !(-12..=12).contains(&v) {
            return Err(EngineError::Invalid(format!("eq {name}={v}")));
        }
    }
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    eng.config.eq_db = [bass as f32, mid as f32, treble as f32];
    if let Some(control) = eng.control.clone() {
        crate::native::lock_recover(&control.config).eq_db = eng.config.eq_db;
    }
    Ok(true)
}

/// Append `path` to the queue (works before the first `play()` too).
pub fn queue_add(path: &str) -> Result<String, EngineError> {
    // Fase 43: `cloud:<id>` se valida contra la caché de la nube.
    if crate::audio_decode::cloud_id(path).is_none() && !std::path::Path::new(path).is_file() {
        return Err(EngineError::Invalid(format!("file not found: {path}")));
    }
    if let Some(id) = crate::audio_decode::cloud_id(path) {
        if crate::audio_decode::cloud_display(path) == path {
            return Err(EngineError::Invalid(format!(
                "cloud:{id} no está en caché; ejecuta cloud_library primero"
            )));
        }
    }
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    if let Some(control) = eng.control.clone() {
        let mut queue = crate::native::lock_recover(&control.queue);
        if queue.len() >= MAX_QUEUE {
            return Err(EngineError::Invalid("queue is full (500)".to_string()));
        }
        queue.push_back(path.to_string());
        Ok(format!("queued ({} waiting): {path}", queue.len()))
    } else {
        if eng.pending_queue.len() >= MAX_QUEUE {
            return Err(EngineError::Invalid("queue is full (500)".to_string()));
        }
        eng.pending_queue.push_back(path.to_string());
        Ok(format!(
            "queued ({} waiting): {path}",
            eng.pending_queue.len()
        ))
    }
}

pub fn queue_clear() -> Result<String, EngineError> {
    let mut guard = lock_slot();
    let eng = guard.get_or_insert_with(Engine::fresh);
    if let Some(control) = eng.control.clone() {
        crate::native::lock_recover(&control.queue).clear();
    }
    eng.pending_queue.clear();
    Ok("queue cleared".to_string())
}

pub fn queue_list() -> Vec<String> {
    let guard = lock_slot();
    match guard.as_ref().and_then(|eng| eng.control.clone()) {
        Some(control) => crate::native::lock_recover(&control.queue)
            .iter()
            .map(|p| crate::audio_decode::cloud_display(p))
            .collect(),
        None => guard
            .as_ref()
            .map(|eng| {
                eng.pending_queue
                    .iter()
                    .map(|p| crate::audio_decode::cloud_display(p))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Skip to the next queued track (stops at the end of the queue).
pub fn next() -> Result<String, EngineError> {
    let next_path = {
        let mut guard = lock_slot();
        let eng = match guard.as_mut() {
            Some(eng) => eng,
            None => return Err(EngineError::Idle),
        };
        reap_finished(eng);
        let control = eng.control.clone().ok_or(EngineError::Idle)?;
        let next_path = crate::native::lock_recover(&control.queue).pop_front();
        if let Some(ref path) = next_path {
            let outgoing = crate::native::lock_recover(&control.current).path.clone();
            push_history(&control, &outgoing);
            *crate::native::lock_recover(&control.switch_to) = Some(path.clone());
        }
        next_path
    };
    match next_path {
        Some(path) => Ok(format!("next: {}", crate::audio_decode::cloud_display(&path))),
        None => {
            let _ = stop();
            Ok("end of queue (stopped)".to_string())
        }
    }
}

/// Restart the track when past 3 s, else step back through history.
pub fn prev() -> Result<String, EngineError> {
    if position_secs() > PREV_RESTART_SECS {
        seek(0.0)?;
        return Ok("restarted track".to_string());
    }
    let mut guard = lock_slot();
    let eng = match guard.as_mut() {
        Some(eng) => eng,
        None => return Err(EngineError::Idle),
    };
    reap_finished(eng);
    let control = eng.control.clone().ok_or(EngineError::Idle)?;
    let prev_path = crate::native::lock_recover(&control.history).pop();
    match prev_path {
        Some(path) => {
            let outgoing = crate::native::lock_recover(&control.current).path.clone();
            if !outgoing.is_empty() {
                crate::native::lock_recover(&control.queue).push_front(outgoing);
            }
            *crate::native::lock_recover(&control.switch_to) = Some(path.clone());
            Ok(format!("previous: {}", crate::audio_decode::cloud_display(&path)))
        }
        None => Err(EngineError::Invalid("no previous track".to_string())),
    }
}

pub fn current_track() -> Result<CurrentTrack, EngineError> {
    let (path, codec, duration_secs, queue_len) = {
        let mut guard = lock_slot();
        let eng = match guard.as_mut() {
            Some(eng) => eng,
            None => return Err(EngineError::Idle),
        };
        reap_finished(eng);
        let control = eng.control.clone().ok_or(EngineError::Idle)?;
        let current = crate::native::lock_recover(&control.current);
        let queue_len = crate::native::lock_recover(&control.queue).len();
        (
            current.path.clone(),
            current.codec.clone(),
            current.duration_secs,
            queue_len,
        )
    };
    if path.is_empty() {
        return Err(EngineError::Idle);
    }
    Ok(CurrentTrack {
        path: crate::audio_decode::cloud_display(&path),
        codec,
        position_secs: position_secs(),
        duration_secs,
        state: state_string(),
        queue_len,
    })
}

/// 32 spectrum bars (`0.0..=1.0`) for the visualizer. Zeros when idle —
/// UIs poll this freely without error handling.
pub fn levels() -> [f32; 32] {
    let mut guard = lock_slot();
    let (control, rate) = match guard.as_mut() {
        Some(eng) => match eng.control.clone() {
            Some(control) => (control, eng.out_rate),
            None => return [0.0; 32],
        },
        None => return [0.0; 32],
    };
    drop(guard);
    let ring = crate::native::lock_recover(&control.levels_ring);
    let mono: Vec<f32> = ring.iter().copied().collect();
    audio_decode::spectrum_32(&mono, rate.max(8000))
}

/// Headless check: probe + decode up to 5 s. Works everywhere, needs no
/// output device — this is what CI and the future streaming cache use.
pub fn decode_report(path: &str) -> Result<DecodeReport, EngineError> {
    let decoded =
        audio_decode::decode_file(path, VERIFY_SECS).map_err(|e| EngineError::Decode(e.to_string()))?;
    let peak = decoded
        .samples
        .iter()
        .fold(0.0f32, |m, s| m.max(s.abs()));
    Ok(DecodeReport {
        duration_secs: if decoded.duration_secs > 0.0 {
            decoded.duration_secs
        } else {
            decoded.decoded_secs
        },
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
        codec: decoded.codec,
        verified_secs: decoded.decoded_secs,
        peak,
    })
}

/// Output device label (`""` when none / on Android).
pub fn device() -> String {
    lock_slot()
        .as_ref()
        .map(|eng| eng.device_label.clone())
        .unwrap_or_default()
}

pub fn status() -> EngineStatus {
    let (path, codec, sample_rate, channels, volume_percent, crossfade_secs, gapless, eq_db, queue_len, device) =
        {
            let guard = lock_slot();
            match guard.as_ref() {
                Some(eng) => {
                    let (path, codec, sample_rate, channels, queue_len) = match eng.control.clone()
                    {
                        Some(control) => {
                            let current = crate::native::lock_recover(&control.current);
                            let queue_len = crate::native::lock_recover(&control.queue).len();
                            (
                                current.path.clone(),
                                current.codec.clone(),
                                current.rate,
                                current.channels,
                                queue_len,
                            )
                        }
                        None => (String::new(), String::new(), 0, 0, eng.pending_queue.len()),
                    };
                    (
                        path,
                        codec,
                        sample_rate,
                        channels,
                        (eng.config.volume * 100.0).round() as i64,
                        eng.config.crossfade_secs,
                        eng.config.gapless,
                        [
                            eng.config.eq_db[0].round() as i64,
                            eng.config.eq_db[1].round() as i64,
                            eng.config.eq_db[2].round() as i64,
                        ],
                        queue_len,
                        eng.device_label.clone(),
                    )
                }
                None => (
                    String::new(),
                    String::new(),
                    0,
                    0,
                    80,
                    0.0,
                    true,
                    [0, 0, 0],
                    0,
                    String::new(),
                ),
            }
        };
    EngineStatus {
        state: state_string(),
        path: crate::audio_decode::cloud_display(&path),
        codec,
        position_secs: position_secs(),
        duration_secs: duration_secs(),
        sample_rate,
        channels,
        volume_percent,
        crossfade_secs,
        gapless,
        eq_db,
        queue_len,
        device,
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// The engine slot is process-global and cargo runs tests in parallel:
    /// every test touching playback state holds this guard so they can
    /// never observe each other mid-flight (zero flakes by construction).
    fn test_slot_guard() -> std::sync::MutexGuard<'static, ()> {
        static SLOT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        SLOT_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn pcm_wav_bytes(secs: f32) -> Vec<u8> {
        let rate = 44_100u32;
        let frames = (secs * rate as f32) as usize;
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(b"RIFF");
        let riff_at = v.len();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * 4).to_le_bytes());
        v.extend_from_slice(&4u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(frames as u32 * 4).to_le_bytes());
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            let s = (0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 32767.0) as i16;
            v.extend_from_slice(&s.to_le_bytes());
            v.extend_from_slice(&s.to_le_bytes());
        }
        let riff_len = (v.len() - 8) as u32;
        v[riff_at..riff_at + 4].copy_from_slice(&riff_len.to_le_bytes());
        v
    }

    #[test]
    fn channel_conversion_round_trips() {
        let stereo = vec![0.5f32, -0.5, 0.25, -0.25];
        assert_eq!(convert_channels(&stereo, 2, 2), stereo);
        assert_eq!(convert_channels(&stereo, 2, 1), vec![0.0, 0.0]);
        assert_eq!(
            convert_channels(&[0.5, 0.25], 1, 2),
            vec![0.5, 0.5, 0.25, 0.25]
        );
        assert!(convert_channels(&[], 2, 2).is_empty());
    }

    #[test]
    fn decode_report_verifies_a_real_file() {
        let dir = std::env::temp_dir().join(format!("zett-engine-report-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tono.wav");
        fs::write(&file, pcm_wav_bytes(1.0)).unwrap();
        let report = decode_report(file.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&dir).ok();
        assert_eq!(report.sample_rate, 44_100);
        assert_eq!(report.channels, 2);
        assert!((report.duration_secs - 1.0).abs() < 0.05, "{}", report.duration_secs);
        assert!(report.peak > 0.4, "peak={}", report.peak);
        assert!(report.verified_secs > 0.0);
        assert!(!report.codec.is_empty());
    }

    #[test]
    fn settings_validation_rejects_garbage() {
        let _guard = test_slot_guard();
        assert!(set_volume(80).unwrap());
        assert!(set_volume(0).unwrap());
        assert!(set_volume(100).unwrap());
        assert!(set_volume(-1).is_err());
        assert!(set_volume(101).is_err());
        assert!(set_crossfade(2.5).unwrap());
        assert!(set_crossfade(0.0).unwrap());
        assert!(set_crossfade(-1.0).is_err());
        assert!(set_crossfade(13.0).is_err());
        assert!(set_crossfade(f64::NAN).is_err());
        assert!(set_gapless(true).unwrap());
        assert!(set_eq(6, -3, 0).unwrap());
        assert!(set_eq(13, 0, 0).is_err());
        assert!(set_eq(0, 0, -13).is_err());
        // Leave defaults behind for the other tests.
        assert!(set_volume(80).unwrap());
        assert!(set_crossfade(0.0).unwrap());
        assert!(set_eq(0, 0, 0).unwrap());
    }

    #[test]
    fn queue_validates_files_and_clears() {
        let _guard = test_slot_guard();
        assert!(queue_add("/definitely/not/a/file/zett.mp3").is_err());
        let dir = std::env::temp_dir().join(format!("zett-engine-queue-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("cola.wav");
        fs::write(&file, pcm_wav_bytes(0.1)).unwrap();
        let msg = queue_add(file.to_str().unwrap()).unwrap();
        assert!(msg.contains("queued"), "{msg}");
        assert!(queue_list().iter().any(|p| p.ends_with("cola.wav")));
        assert_eq!(queue_clear().unwrap(), "queue cleared");
        assert!(queue_list().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn idle_state_reads_sane_defaults() {
        let _guard = test_slot_guard();
        // Serialized with the play test above, so the engine is idle here:
        // reads degrade gracefully instead of erroring.
        assert_eq!(levels(), [0.0; 32]);
        assert_eq!(position_secs(), -1.0);
        assert_eq!(duration_secs(), 0.0);
        assert_eq!(state_string(), "idle");
        assert!(stop().is_err());
        assert!(pause().is_err());
        assert!(seek(1.0).is_err());
        assert!(next().is_err());
    }

    #[test]
    fn play_degrades_gracefully_without_a_device() {
        let _guard = test_slot_guard();
        let dir = std::env::temp_dir().join(format!("zett-engine-play-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("corto.wav");
        fs::write(&file, pcm_wav_bytes(0.2)).unwrap();
        // On CI/headless/Android this never plays: NoDevice (no backend),
        // Unsupported (Android shell owns output) or Control (backend
        // present but unusable, e.g. ALSA with no sound cards). On a
        // machine with speakers it really plays. All are correct — no panics.
        match play(file.to_str().unwrap()) {
            Ok(msg) => {
                assert!(msg.contains("hifi-engine"), "{msg}");
                assert_eq!(state_string(), "playing");
                assert!(pause().is_ok());
                assert_eq!(state_string(), "paused");
                assert!(resume().is_ok());
                let _ = seek(0.05);
                let _ = current_track();
                let st = status();
                assert!(st.path.ends_with("corto.wav"), "{}", st.path);
                assert!(stop().is_ok());
                assert_eq!(state_string(), "idle");
            }
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("no audio output device")
                        || msg.contains("unsupported on this target")
                        || msg.contains("output error"),
                    "{msg}"
                );
                assert_eq!(state_string(), "idle");
            }
        }
        fs::remove_dir_all(&dir).ok();
    }
}
