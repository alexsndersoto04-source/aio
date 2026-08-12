//! Non-cryptographic checksums and constant-time byte comparison.
//! These functions are for integrity/hash tables, not passwords or signatures.

pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut difference = a.len() ^ b.len();
    let max = a.len().max(b.len());
    for i in 0..max {
        difference |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_vectors() {
        assert_eq!(fnv1a_64(b"hello"), 0xa430d84680aabd0b);
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        assert!(constant_time_eq(b"same", b"same"));
        assert!(!constant_time_eq(b"a", b"b"));
    }
}
