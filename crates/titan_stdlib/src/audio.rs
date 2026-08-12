use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

struct AudioBuffer {
    samples: Vec<f32>,
    volume: f64,
    playing: bool,
    looping: bool,
}

struct AudioState {
    initialized: bool,
    next_handle: i64,
    buffers: HashMap<i64, AudioBuffer>,
}

impl AudioState {
    fn new() -> Self {
        Self {
            initialized: false,
            next_handle: 1,
            buffers: HashMap::new(),
        }
    }
}

fn audio_states() -> &'static Mutex<HashMap<u64, Arc<Mutex<AudioState>>>> {
    static STATES: OnceLock<Mutex<HashMap<u64, Arc<Mutex<AudioState>>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_audio_state() -> Arc<Mutex<AudioState>> {
    let runtime_id = crate::native::current_runtime_id();
    let mut states = crate::native::lock_recover(audio_states());
    Arc::clone(
        states
            .entry(runtime_id)
            .or_insert_with(|| Arc::new(Mutex::new(AudioState::new()))),
    )
}

pub(crate) fn cleanup_runtime(runtime_id: u64) -> usize {
    usize::from(
        crate::native::lock_recover(audio_states())
            .remove(&runtime_id)
            .is_some(),
    )
}

pub fn init() -> bool {
    if let Ok(mut state) = get_audio_state().lock() {
        state.initialized = true;
        true
    } else {
        false
    }
}

pub fn load_wave(freq_hz: f64, duration_ms: i64) -> i64 {
    if let Ok(mut state) = get_audio_state().lock() {
        if !state.initialized {
            return -1;
        }
        let sample_rate = 44100.0;
        let total_samples = ((duration_ms.max(1) as f64 / 1000.0) * sample_rate) as usize;
        let mut samples = Vec::with_capacity(total_samples);
        for i in 0..total_samples {
            let t = i as f64 / sample_rate;
            let sample = (2.0 * PI * freq_hz * t).sin() as f32;
            samples.push(sample);
        }
        let handle = state.next_handle;
        state.next_handle = state.next_handle.saturating_add(1);
        state.buffers.insert(
            handle,
            AudioBuffer {
                samples,
                volume: 1.0,
                playing: false,
                looping: false,
            },
        );
        handle
    } else {
        -1
    }
}

pub fn sample_count(handle: i64) -> usize {
    if let Ok(state) = get_audio_state().lock() {
        if let Some(buffer) = state.buffers.get(&handle) {
            return buffer.samples.len();
        }
    }
    0
}

pub fn play(handle: i64, loop_audio: bool) -> bool {
    if let Ok(mut state) = get_audio_state().lock() {
        if let Some(buffer) = state.buffers.get_mut(&handle) {
            if !buffer.samples.is_empty() {
                buffer.playing = true;
                buffer.looping = loop_audio;
                return true;
            }
        }
    }
    false
}

pub fn set_volume(handle: i64, volume: f64) -> bool {
    if let Ok(mut state) = get_audio_state().lock() {
        if let Some(buffer) = state.buffers.get_mut(&handle) {
            buffer.volume = volume.clamp(0.0, 1.0);
            return true;
        }
    }
    false
}

pub fn stop(handle: i64) -> bool {
    if let Ok(mut state) = get_audio_state().lock() {
        if let Some(buffer) = state.buffers.get_mut(&handle) {
            buffer.playing = false;
            return true;
        }
    }
    false
}

pub fn shutdown() -> bool {
    if let Ok(mut state) = get_audio_state().lock() {
        state.buffers.clear();
        state.initialized = false;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_buffer_generation() {
        assert!(init());
        let handle = load_wave(440.0, 100);
        assert!(handle > 0);
        assert!(sample_count(handle) > 0);
        assert!(play(handle, false));
        assert!(set_volume(handle, 0.5));
        assert!(stop(handle));
        assert!(shutdown());
    }
}
