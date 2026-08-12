//! Vector math (`std::vector::*`) — pure Rust, no deps.
//!
//! Small helpers for working with `Vec<f32>` embeddings and other dense
//! numeric arrays. Everything is pure Rust, uses no crates, and is
//! deliberately branchless-when-possible so the loops autovectorize on
//! ARM NEON (compiled by rustc with -C opt-level=3, which we already
//! enable via the release profile).
//!
//! Designed to complement `std::onnx::run_bert_pooled` (Fase 12 pt.4)
//! for on-device semantic search: encode texts into embeddings, then
//! use `cosine_similarity` to rank them by relevance to a query.

/// Dot product of two vectors. Panics if lengths differ (caller error).
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "dot: length mismatch {} vs {}",
        a.len(),
        b.len()
    );
    // Manual unrolled loop; on release + NEON rustc turns this into VMLA.
    let mut sum = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        sum += x * y;
    }
    sum
}

/// L2 norm (Euclidean length) of a vector.
pub fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity in `[-1, 1]`. Returns `0.0` when either vector is
/// all-zeros, so the caller can rank without a special case.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "cosine: length mismatch {} vs {}",
        a.len(),
        b.len()
    );
    let na = norm(a);
    let nb = norm(b);
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot(a, b) / (na * nb)
}

/// Return a new vector with unit L2 norm. Returns zeros unchanged.
pub fn normalize(v: &[f32]) -> Vec<f32> {
    let n = norm(v);
    if n == 0.0 {
        return v.to_vec();
    }
    v.iter().map(|x| x / n).collect()
}

/// Element-wise sum. Panics on length mismatch.
pub fn add(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(
        a.len(),
        b.len(),
        "add: length mismatch {} vs {}",
        a.len(),
        b.len()
    );
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Element-wise difference. Panics on length mismatch.
pub fn sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(
        a.len(),
        b.len(),
        "sub: length mismatch {} vs {}",
        a.len(),
        b.len()
    );
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// Multiply every element by `k`.
pub fn scale(v: &[f32], k: f32) -> Vec<f32> {
    v.iter().map(|x| x * k).collect()
}

/// Return the index of the maximum element. `None` for empty input.
pub fn argmax(v: &[f32]) -> Option<usize> {
    if v.is_empty() {
        return None;
    }
    let mut best_i = 0;
    let mut best_v = v[0];
    for (i, &x) in v.iter().enumerate().skip(1) {
        if x > best_v {
            best_v = x;
            best_i = i;
        }
    }
    Some(best_i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
    }

    #[test]
    fn dot_and_norm_match_reference() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, -5.0, 6.0];
        eq(dot(&a, &b), 4.0 - 10.0 + 18.0);
        eq(norm(&a), (1.0f32 + 4.0 + 9.0).sqrt());
    }

    #[test]
    fn cosine_bounds() {
        let a = [1.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [-1.0, 0.0, 0.0];
        let d = [0.0, 1.0, 0.0];
        eq(cosine_similarity(&a, &b), 1.0);
        eq(cosine_similarity(&a, &c), -1.0);
        eq(cosine_similarity(&a, &d), 0.0);
    }

    #[test]
    fn normalize_unit_length() {
        let a = [3.0, 4.0];
        let n = normalize(&a);
        eq(norm(&n), 1.0);
        eq(n[0], 0.6);
        eq(n[1], 0.8);
    }

    #[test]
    fn zero_vector_stays_zero() {
        let z = [0.0, 0.0, 0.0];
        assert_eq!(normalize(&z), z);
        eq(cosine_similarity(&z, &[1.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn argmax_works() {
        assert_eq!(argmax(&[]), None);
        assert_eq!(argmax(&[1.0, 3.0, 2.0]), Some(1));
        assert_eq!(argmax(&[-1.0, -2.0, -3.0]), Some(0));
    }
}
