//! Bloom filter implementation for probabilistic set membership testing.
//!
//! A Bloom filter is a space-efficient probabilistic data structure that tests
//! whether an element is a member of a set. False positives are possible, but
//! false negatives are not.

use crate::hash::{default_hasher, double_hash, nth_hash, DefaultHasher};
use crate::traits::MembershipSketch;
use core::hash::{BuildHasher, Hash};

#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A space-efficient probabilistic set for membership testing.
///
/// Use [`BloomFilter::builder()`] to construct with desired parameters.
///
/// # Example
///
/// ```rust
/// use roughly::BloomFilter;
/// use roughly::traits::MembershipSketch;
///
/// let mut filter = BloomFilter::builder()
///     .expected_items(1000)
///     .false_positive_rate(0.01)
///     .build();
///
/// filter.insert(&"hello");
/// assert!(filter.contains(&"hello"));
/// assert!(!filter.contains(&"world")); // probably false
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "H: Default")))]
pub struct BloomFilter<H = DefaultHasher> {
    bits: Vec<u64>,
    num_bits: u64,
    num_hashes: u32,
    count: usize,
    fpp: f64,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    build_hasher: H,
}

impl BloomFilter<DefaultHasher> {
    /// Creates a new builder with default hasher.
    #[must_use]
    pub fn builder() -> BloomFilterBuilder<DefaultHasher> {
        BloomFilterBuilder::new()
    }
}

impl<H: BuildHasher> BloomFilter<H> {
    /// Creates a new builder with a custom hasher.
    #[must_use]
    pub fn builder_with_hasher(hasher: H) -> BloomFilterBuilder<H> {
        BloomFilterBuilder::new_with_hasher(hasher)
    }

    /// Returns the number of bits in the filter.
    #[must_use]
    pub fn num_bits(&self) -> u64 {
        self.num_bits
    }

    /// Returns the number of hash functions used.
    #[must_use]
    pub fn num_hashes(&self) -> u32 {
        self.num_hashes
    }

    /// Returns the estimated false positive probability based on current fill.
    ///
    /// This may differ from the configured rate as elements are inserted.
    #[must_use]
    pub fn estimated_fpp(&self) -> f64 {
        let k = f64::from(self.num_hashes);
        let m = self.num_bits as f64;
        let n = self.count as f64;
        (1.0 - (-k * n / m).exp()).powf(k)
    }

    #[inline]
    fn set_bit(&mut self, pos: u64) {
        let word = (pos / 64) as usize;
        let bit = pos % 64;
        self.bits[word] |= 1u64 << bit;
    }

    #[inline]
    fn get_bit(&self, pos: u64) -> bool {
        let word = (pos / 64) as usize;
        let bit = pos % 64;
        (self.bits[word] >> bit) & 1 == 1
    }
}

impl<T: Hash, H: BuildHasher> MembershipSketch<T> for BloomFilter<H> {
    fn insert(&mut self, item: &T) {
        let (h1, h2) = double_hash(&self.build_hasher, item);
        for i in 0..u64::from(self.num_hashes) {
            let pos = nth_hash(h1, h2, i, self.num_bits);
            self.set_bit(pos);
        }
        self.count += 1;
    }

    fn contains(&self, item: &T) -> bool {
        let (h1, h2) = double_hash(&self.build_hasher, item);
        for i in 0..u64::from(self.num_hashes) {
            let pos = nth_hash(h1, h2, i, self.num_bits);
            if !self.get_bit(pos) {
                return false;
            }
        }
        true
    }

    fn false_positive_rate(&self) -> f64 {
        self.fpp
    }

    fn len(&self) -> usize {
        self.count
    }

    fn clear(&mut self) {
        self.bits.iter_mut().for_each(|w| *w = 0);
        self.count = 0;
    }
}

/// Builder for creating a [`BloomFilter`] with desired parameters.
#[derive(Debug)]
pub struct BloomFilterBuilder<H = DefaultHasher> {
    expected_items: Option<usize>,
    fpp: f64,
    hasher: H,
}

impl BloomFilterBuilder<DefaultHasher> {
    fn new() -> Self {
        Self {
            expected_items: None,
            fpp: 0.01,
            hasher: default_hasher(),
        }
    }
}

impl<H: BuildHasher> BloomFilterBuilder<H> {
    fn new_with_hasher(hasher: H) -> Self {
        Self {
            expected_items: None,
            fpp: 0.01,
            hasher,
        }
    }

    /// Sets the expected number of items to be inserted.
    ///
    /// This is required and must be called before [`build()`](Self::build).
    #[must_use]
    pub fn expected_items(mut self, n: usize) -> Self {
        self.expected_items = Some(n);
        self
    }

    /// Sets the desired false positive rate (default: 0.01).
    ///
    /// # Panics
    ///
    /// Panics if `fpp` is not in the range (0, 1).
    #[must_use]
    pub fn false_positive_rate(mut self, fpp: f64) -> Self {
        assert!(fpp > 0.0 && fpp < 1.0, "false_positive_rate must be in (0, 1)");
        self.fpp = fpp;
        self
    }

    /// Builds the Bloom filter with the configured parameters.
    ///
    /// # Panics
    ///
    /// Panics if `expected_items` was not set or is zero.
    #[must_use]
    pub fn build(self) -> BloomFilter<H> {
        let n = self.expected_items.expect("expected_items must be set");
        assert!(n > 0, "expected_items must be > 0");

        let (num_bits, num_hashes) = optimal_params(n, self.fpp);
        let num_words = ((num_bits + 63) / 64) as usize;

        BloomFilter {
            bits: vec![0u64; num_words],
            num_bits,
            num_hashes,
            count: 0,
            fpp: self.fpp,
            build_hasher: self.hasher,
        }
    }
}

fn optimal_params(n: usize, p: f64) -> (u64, u32) {
    let ln2 = core::f64::consts::LN_2;
    let m = (-(n as f64) * p.ln() / (ln2 * ln2)).ceil() as u64;
    let k = ((m as f64 / n as f64) * ln2).round() as u32;
    (m.max(1), k.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MembershipSketch;

    fn build(n: usize, fpp: f64) -> BloomFilter {
        BloomFilter::builder()
            .expected_items(n)
            .false_positive_rate(fpp)
            .build()
    }

    #[test]
    fn no_false_negatives() {
        let mut f = build(1000, 0.01);
        for i in 0..1000u64 {
            f.insert(&i);
        }
        for i in 0..1000u64 {
            assert!(f.contains(&i), "false negative for {i}");
        }
    }

    #[test]
    fn false_positive_rate_reasonable() {
        let n = 10_000;
        let fpp = 0.01;
        let mut f = build(n, fpp);
        for i in 0..n as u64 {
            f.insert(&i);
        }
        let mut fp = 0usize;
        let trials = 100_000u64;
        for i in (n as u64)..(n as u64 + trials) {
            if f.contains(&i) {
                fp += 1;
            }
        }
        let actual_fpp = fp as f64 / trials as f64;

        assert!(
            actual_fpp < fpp * 3.0,
            "fpp too high: {actual_fpp:.4} (target {fpp})"
        );
    }

    #[test]
    fn clear_resets() {
        let mut f = build(100, 0.01);
        f.insert(&"hello");
        assert!(f.contains(&"hello"));
        f.clear();
        assert_eq!(f.len(), 0);
    }

    #[test]
    fn optimal_params_sanity() {
        let (m, k) = optimal_params(1_000_000, 0.01);

        assert!(m > 9_000_000 && m < 11_000_000, "unexpected m={m}");
        assert!(k >= 6 && k <= 8, "unexpected k={k}");
    }
}
