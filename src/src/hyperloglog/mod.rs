//! HyperLogLog implementation for cardinality estimation.
//!
//! HyperLogLog is a probabilistic data structure that estimates the number
//! of distinct elements in a multiset using very little memory.

use crate::hash::{default_hasher, double_hash, DefaultHasher};
use crate::traits::CardinalitySketch;
use core::hash::{BuildHasher, Hash};

#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A probabilistic data structure for cardinality (distinct count) estimation.
///
/// Use [`HyperLogLog::builder()`] to construct with desired parameters.
///
/// # Example
///
/// ```rust
/// use roughly::HyperLogLog;
/// use roughly::traits::CardinalitySketch;
///
/// let mut hll = HyperLogLog::builder().std_error(0.01).build();
///
/// for i in 0..100_000u64 {
///     hll.insert(&i);
/// }
///
/// let estimate = hll.count();
/// println!("Estimated distinct count: {estimate}");
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "H: Default")))]
pub struct HyperLogLog<H = DefaultHasher> {
    registers: Vec<u8>,
    m: usize,
    b: u32,
    alpha: f64,
    std_error: f64,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    build_hasher: H,
}

impl HyperLogLog<DefaultHasher> {
    /// Creates a new builder with default hasher.
    #[must_use]
    pub fn builder() -> HyperLogLogBuilder<DefaultHasher> {
        HyperLogLogBuilder::new()
    }
}

impl<H: BuildHasher> HyperLogLog<H> {
    /// Creates a new builder with a custom hasher.
    #[must_use]
    pub fn builder_with_hasher(hasher: H) -> HyperLogLogBuilder<H> {
        HyperLogLogBuilder::new_with_hasher(hasher)
    }

    /// Returns the number of registers (2^precision).
    #[must_use]
    pub fn num_registers(&self) -> usize {
        self.m
    }

    /// Returns the precision parameter (number of bits for register index).
    #[must_use]
    pub fn precision(&self) -> u32 {
        self.b
    }
}

impl<T: Hash, H: BuildHasher> CardinalitySketch<T> for HyperLogLog<H> {
    fn insert(&mut self, item: &T) {
        let (h, _) = double_hash(&self.build_hasher, item);

        let idx = (h >> (64 - self.b)) as usize;

        let remaining = h << self.b;
        let rho = remaining.leading_zeros() + 1;

        if rho > u32::from(self.registers[idx]) {
            self.registers[idx] = rho as u8;
        }
    }

    fn count(&self) -> u64 {
        let m = self.m as f64;

        let z: f64 = self
            .registers
            .iter()
            .map(|&r| 2.0f64.powi(-(i32::from(r))))
            .sum();
        let raw = self.alpha * m * m / z;

        if raw <= 2.5 * m {
            let zeros = self.registers.iter().filter(|&&r| r == 0).count() as f64;
            if zeros > 0.0 {
                return (m * (m / zeros).ln()).round() as u64;
            }
        }

        let two_pow_32 = 2.0f64.powi(32);
        if raw > two_pow_32 / 30.0 {
            return (-two_pow_32 * (1.0 - raw / two_pow_32).ln()).round() as u64;
        }

        raw.round() as u64
    }

    fn std_error(&self) -> f64 {
        self.std_error
    }

    fn merge(&mut self, other: &Self) {
        assert_eq!(
            self.m, other.m,
            "cannot merge HyperLogLog with different precision"
        );
        for (a, b) in self.registers.iter_mut().zip(other.registers.iter()) {
            *a = (*a).max(*b);
        }
    }

    fn clear(&mut self) {
        self.registers.iter_mut().for_each(|r| *r = 0);
    }
}

/// Builder for creating a [`HyperLogLog`] with desired parameters.
#[derive(Debug)]
pub struct HyperLogLogBuilder<H = DefaultHasher> {
    std_error: f64,
    hasher: H,
}

impl HyperLogLogBuilder<DefaultHasher> {
    fn new() -> Self {
        Self {
            std_error: 0.01,
            hasher: default_hasher(),
        }
    }
}

impl<H: BuildHasher> HyperLogLogBuilder<H> {
    fn new_with_hasher(hasher: H) -> Self {
        Self {
            std_error: 0.01,
            hasher,
        }
    }

    /// Sets the desired standard error (default: 0.01).
    ///
    /// # Panics
    ///
    /// Panics if `std_error` is not in the range (0, 1).
    #[must_use]
    pub fn std_error(mut self, std_error: f64) -> Self {
        assert!(
            std_error > 0.0 && std_error < 1.0,
            "std_error must be in (0, 1)"
        );
        self.std_error = std_error;
        self
    }

    /// Sets the precision directly instead of deriving from standard error.
    ///
    /// # Panics
    ///
    /// Panics if `b` is not in the range [4, 18].
    #[must_use]
    pub fn precision(self, b: u32) -> HyperLogLogBuilderWithPrecision<H> {
        assert!((4..=18).contains(&b), "precision must be in [4, 18]");
        HyperLogLogBuilderWithPrecision {
            b,
            hasher: self.hasher,
        }
    }

    /// Builds the HyperLogLog with precision derived from the configured standard error.
    #[must_use]
    pub fn build(self) -> HyperLogLog<H> {
        let m_f = (1.04 / self.std_error).powi(2);
        let b = (m_f.log2().ceil() as u32).clamp(4, 18);
        build_hll(b, self.std_error, self.hasher)
    }
}

/// Builder state after precision has been explicitly set.
pub struct HyperLogLogBuilderWithPrecision<H> {
    b: u32,
    hasher: H,
}

impl<H: BuildHasher> HyperLogLogBuilderWithPrecision<H> {
    /// Builds the HyperLogLog with the specified precision.
    #[must_use]
    pub fn build(self) -> HyperLogLog<H> {
        let std_error = 1.04 / ((1usize << self.b) as f64).sqrt();
        build_hll(self.b, std_error, self.hasher)
    }
}

fn build_hll<H>(b: u32, std_error: f64, build_hasher: H) -> HyperLogLog<H> {
    let m = 1usize << b;
    let alpha = alpha(m);
    HyperLogLog {
        registers: vec![0u8; m],
        m,
        b,
        alpha,
        std_error,
        build_hasher,
    }
}

fn alpha(m: usize) -> f64 {
    match m {
        16 => 0.673,
        32 => 0.697,
        64 => 0.709,
        _ => 0.7213 / (1.0 + 1.079 / m as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::CardinalitySketch;

    fn hll(err: f64) -> HyperLogLog {
        HyperLogLog::builder().std_error(err).build()
    }

    #[test]
    fn count_empty_is_zero() {
        let h = hll(0.01);
        assert_eq!(h.count(), 0);
    }

    #[test]
    fn count_within_error_bounds() {
        let mut h = hll(0.02);
        let n = 100_000u64;
        for i in 0..n {
            h.insert(&i);
        }
        let est = h.count() as i64;
        let allowed_err = (n as f64 * 0.06) as i64;
        assert!(
            (est - n as i64).abs() < allowed_err,
            "estimate {est} too far from {n} (allowed ±{allowed_err})"
        );
    }

    #[test]
    fn merge_works() {
        let mut a = hll(0.01);
        let mut b = hll(0.01);
        for i in 0..50_000u64 { a.insert(&i); }
        for i in 50_000..100_000u64 { b.insert(&i); }
        a.merge(&b);
        let est = a.count() as i64;
        let allowed_err = 10_000i64;
        assert!(
            (est - 100_000).abs() < allowed_err,
            "merged estimate {est} too far from 100000"
        );
    }

    #[test]
    fn clear_resets() {
        let mut h = hll(0.01);
        for i in 0..1000u64 { h.insert(&i); }
        h.clear();
        assert_eq!(h.count(), 0);
    }

    #[test]
    fn duplicates_not_double_counted() {
        let mut h = hll(0.02);
        for _ in 0..1000 { h.insert(&"same"); }
        let est = h.count();

        assert!(est < 10, "duplicates counted multiple times: {est}");
    }
}
