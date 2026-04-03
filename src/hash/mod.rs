//! Hashing utilities for probabilistic data structures.

use core::hash::{BuildHasher, Hash, Hasher};

/// Computes two independent hash values for double hashing schemes.
#[inline]
pub fn double_hash<T: Hash, B: BuildHasher>(build_hasher: &B, item: &T) -> (u64, u64) {
    let mut hasher = build_hasher.build_hasher();
    item.hash(&mut hasher);
    let h1 = hasher.finish();

    let h2 = mix64(h1 ^ 0x9e37_79b9_7f4a_7c15);

    (h1, h2)
}

/// Computes the i-th hash value using the double hashing formula: h1 + i * h2 mod m.
#[inline]
pub fn nth_hash(h1: u64, h2: u64, i: u64, m: u64) -> u64 {
    h1.wrapping_add(i.wrapping_mul(h2)) % m
}

#[inline]
fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The default hasher type used by all data structures (ahash).
pub type DefaultHasher = ahash::RandomState;

/// Creates a new instance of the default hasher.
#[must_use]
pub fn default_hasher() -> DefaultHasher {
    ahash::RandomState::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_hash_differs() {
        let bh = default_hasher();
        let (h1, h2) = double_hash(&bh, &"roughly");
        assert_ne!(h1, h2, "h1 and h2 should differ");
    }

    #[test]
    fn nth_hash_in_range() {
        let bh = default_hasher();
        let (h1, h2) = double_hash(&bh, &42u64);
        for i in 0..20 {
            let h = nth_hash(h1, h2, i, 1000);
            assert!(h < 1000);
        }
    }

    #[test]
    fn mix64_avalanche() {

        let a = mix64(1u64);
        let b = mix64(0u64);
        let diff = (a ^ b).count_ones();
        assert!(diff > 20, "poor avalanche: only {diff} bits differ");
    }
}
