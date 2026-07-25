//! Random number generation (`std::random::*`).
//!
//! There are two sources:
//! * OS entropy: `int()`, `float()`, `bytes()`, `range()`, `bool()`, `pick()`,
//!   `shuffle()` — draw from `rand::rng()` (thread-local, seeded from the OS).
//! * Deterministic ChaCha20: `seeded_int(seed, min, max)` etc. — reproducible
//!   for testing, seeding, or Monte-Carlo runs that must be replayable.

use rand::{Rng, SeedableRng};
use rand::seq::{IndexedRandom, SliceRandom};
use rand_chacha::ChaCha20Rng;

/// Uniformly random `i64` in `[min, max]` (inclusive on both ends). Returns
/// `min` when the range is empty (`max < min`).
pub fn range(min: i64, max: i64) -> i64 {
    if max < min { return min; }
    rand::rng().random_range(min..=max)
}

/// A random `i64` uniformly distributed across the full range.
pub fn int() -> i64 { rand::rng().random::<i64>() }

/// Uniform `f64` in `[0.0, 1.0)`.
pub fn float() -> f64 { rand::rng().random::<f64>() }

/// A random boolean with 50/50 probability.
pub fn boolean() -> bool { rand::rng().random::<bool>() }

/// `n` random bytes drawn from the OS RNG.
pub fn bytes(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n];
    rand::rng().fill(&mut out[..]);
    out
}

/// Returns a random element from `items`, or `None` if the array is empty.
pub fn pick<T: Clone>(items: &[T]) -> Option<T> {
    items.choose(&mut rand::rng()).cloned()
}

/// Shuffles a copy of `items` in place using the Fisher–Yates algorithm.
pub fn shuffle<T: Clone>(items: &[T]) -> Vec<T> {
    let mut out = items.to_vec();
    out.shuffle(&mut rand::rng());
    out
}

// --- Deterministic (seeded) helpers ------------------------------------

pub fn seeded_int(seed: u64, min: i64, max: i64) -> i64 {
    if max < min { return min; }
    ChaCha20Rng::seed_from_u64(seed).random_range(min..=max)
}

pub fn seeded_float(seed: u64) -> f64 { ChaCha20Rng::seed_from_u64(seed).random::<f64>() }

pub fn seeded_bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut out = vec![0u8; n];
    rng.fill(&mut out[..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_within_bounds() {
        for _ in 0..1000 {
            let value = range(10, 20);
            assert!((10..=20).contains(&value));
        }
        assert_eq!(range(5, 5), 5);
        assert_eq!(range(10, 3), 10); // empty range -> min
    }

    #[test]
    fn float_in_unit_interval() {
        for _ in 0..500 {
            let value = float();
            assert!((0.0..1.0).contains(&value));
        }
    }

    #[test]
    fn bytes_length_matches_request() {
        assert_eq!(bytes(0).len(), 0);
        assert_eq!(bytes(64).len(), 64);
    }

    #[test]
    fn pick_and_shuffle() {
        let items = vec![1, 2, 3, 4, 5];
        assert!(pick(&items).map(|value| items.contains(&value)).unwrap_or(false));
        assert!(pick::<i32>(&[]).is_none());
        let shuffled = shuffle(&items);
        assert_eq!(shuffled.len(), items.len());
        let mut sorted = shuffled.clone();
        sorted.sort();
        assert_eq!(sorted, items);
    }

    #[test]
    fn seeded_is_reproducible() {
        assert_eq!(seeded_int(42, 0, 1_000_000), seeded_int(42, 0, 1_000_000));
        assert_eq!(seeded_bytes(7, 16), seeded_bytes(7, 16));
        assert_ne!(seeded_bytes(1, 16), seeded_bytes(2, 16));
    }
}
