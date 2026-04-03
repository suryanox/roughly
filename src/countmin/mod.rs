//! Count-Min Sketch implementation for frequency estimation.
//!
//! A Count-Min Sketch is a probabilistic data structure that estimates
//! the frequency of elements in a stream. It may overcount but never undercounts.

use crate::hash::{default_hasher, double_hash, nth_hash, DefaultHasher};
use crate::traits::FrequencySketch;
use core::hash::{BuildHasher, Hash};

#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// A probabilistic data structure for frequency estimation.
///
/// Use [`CountMinSketch::builder()`] to construct with desired parameters.
///
/// # Example
///
/// ```rust
/// use roughly::CountMinSketch;
/// use roughly::traits::FrequencySketch;
///
/// let mut sketch = CountMinSketch::builder()
///     .error_rate(0.001)
///     .confidence(0.99)
///     .build();
///
/// sketch.insert_many(&"apple", 100);
/// sketch.insert(&"banana");
/// assert!(sketch.estimate(&"apple") >= 100);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "H: Default")))]
pub struct CountMinSketch<H = DefaultHasher> {
    counters: Vec<u64>,
    width: usize,
    depth: usize,
    total: u64,
    error_rate: f64,
    confidence: f64,
    #[cfg_attr(feature = "serde", serde(skip, default))]
    build_hasher: H,
}

impl CountMinSketch<DefaultHasher> {
    /// Creates a new builder with default hasher.
    #[must_use]
    pub fn builder() -> CountMinSketchBuilder<DefaultHasher> {
        CountMinSketchBuilder::new()
    }
}

impl<H: BuildHasher> CountMinSketch<H> {
    /// Creates a new builder with a custom hasher.
    #[must_use]
    pub fn builder_with_hasher(hasher: H) -> CountMinSketchBuilder<H> {
        CountMinSketchBuilder::new_with_hasher(hasher)
    }

    /// Returns the width (number of columns) of the sketch.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the depth (number of rows/hash functions) of the sketch.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Merges another sketch into this one by adding counters element-wise.
    ///
    /// # Panics
    ///
    /// Panics if the sketches have different dimensions.
    pub fn merge(&mut self, other: &Self) {
        assert_eq!(
            (self.width, self.depth),
            (other.width, other.depth),
            "cannot merge CountMinSketch with different dimensions"
        );
        for (a, b) in self.counters.iter_mut().zip(other.counters.iter()) {
            *a += b;
        }
        self.total += other.total;
    }

    #[inline]
    fn counter_mut(&mut self, row: usize, col: u64) -> &mut u64 {
        &mut self.counters[row * self.width + col as usize]
    }

    #[inline]
    fn counter(&self, row: usize, col: u64) -> u64 {
        self.counters[row * self.width + col as usize]
    }

    /// Returns the error rate (epsilon) of the sketch.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        self.error_rate
    }

    /// Returns the confidence level (1 - delta) of the sketch.
    #[must_use]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns the total count of all insertions.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Resets the sketch to its initial empty state.
    pub fn clear(&mut self) {
        self.counters.iter_mut().for_each(|c| *c = 0);
        self.total = 0;
    }
}

impl<T: Hash, H: BuildHasher> FrequencySketch<T> for CountMinSketch<H> {
    fn insert(&mut self, item: &T) {
        self.insert_many(item, 1);
    }

    fn insert_many(&mut self, item: &T, count: u64) {
        let (h1, h2) = double_hash(&self.build_hasher, item);
        let w = self.width as u64;
        for row in 0..self.depth {
            let col = nth_hash(h1, h2, row as u64, w);
            *self.counter_mut(row, col) += count;
        }
        self.total += count;
    }

    fn estimate(&self, item: &T) -> u64 {
        let (h1, h2) = double_hash(&self.build_hasher, item);
        let w = self.width as u64;
        (0..self.depth)
            .map(|row| {
                let col = nth_hash(h1, h2, row as u64, w);
                self.counter(row, col)
            })
            .min()
            .unwrap_or(0)
    }

    fn error_rate(&self) -> f64 {
        Self::error_rate(self)
    }

    fn confidence(&self) -> f64 {
        Self::confidence(self)
    }

    fn total(&self) -> u64 {
        Self::total(self)
    }

    fn clear(&mut self) {
        Self::clear(self)
    }
}

/// Builder for creating a [`CountMinSketch`] with desired parameters.
#[derive(Debug)]
pub struct CountMinSketchBuilder<H = DefaultHasher> {
    error_rate: f64,
    confidence: f64,
    hasher: H,
}

impl CountMinSketchBuilder<DefaultHasher> {
    fn new() -> Self {
        Self {
            error_rate: 0.001,
            confidence: 0.99,
            hasher: default_hasher(),
        }
    }
}

impl<H: BuildHasher> CountMinSketchBuilder<H> {
    fn new_with_hasher(hasher: H) -> Self {
        Self {
            error_rate: 0.001,
            confidence: 0.99,
            hasher,
        }
    }

    /// Sets the error rate (epsilon), controlling sketch width (default: 0.001).
    ///
    /// # Panics
    ///
    /// Panics if `error_rate` is not in the range (0, 1).
    #[must_use]
    pub fn error_rate(mut self, error_rate: f64) -> Self {
        assert!(
            error_rate > 0.0 && error_rate < 1.0,
            "error_rate must be in (0, 1)"
        );
        self.error_rate = error_rate;
        self
    }

    /// Sets the confidence level (delta), controlling sketch depth (default: 0.99).
    ///
    /// # Panics
    ///
    /// Panics if `confidence` is not in the range (0, 1).
    #[must_use]
    pub fn confidence(mut self, confidence: f64) -> Self {
        assert!(
            confidence > 0.0 && confidence < 1.0,
            "confidence must be in (0, 1)"
        );
        self.confidence = confidence;
        self
    }

    /// Builds the Count-Min Sketch with the configured parameters.
    #[must_use]
    pub fn build(self) -> CountMinSketch<H> {
        let e = core::f64::consts::E;
        let width = (e / self.error_rate).ceil() as usize;
        let depth = ((1.0 - self.confidence).ln() / 0.5f64.ln()).ceil() as usize;
        let depth = depth.max(1);

        CountMinSketch {
            counters: vec![0u64; width * depth],
            width,
            depth,
            total: 0,
            error_rate: self.error_rate,
            confidence: self.confidence,
            build_hasher: self.hasher,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::FrequencySketch;

    fn cms() -> CountMinSketch {
        CountMinSketch::builder()
            .error_rate(0.001)
            .confidence(0.99)
            .build()
    }

    #[test]
    fn no_undercount() {
        let mut c = cms();
        for _ in 0..500 { c.insert(&"key"); }
        assert!(c.estimate(&"key") >= 500, "estimate below true count");
    }

    #[test]
    fn insert_many() {
        let mut c = cms();
        c.insert_many(&"batch", 1000);
        assert!(c.estimate(&"batch") >= 1000);
    }

    #[test]
    fn unseen_item_low_estimate() {
        let mut c = cms();

        for i in 0..10_000u64 { c.insert(&i); }

        let estimate = c.estimate(&99_999_999u64);
        let allowed = (c.error_rate() * c.total() as f64 * 2.0) as u64;
        assert!(estimate <= allowed, "unseen item estimate {estimate} > allowed {allowed}");
    }

    #[test]
    fn clear_resets() {
        let mut c = cms();
        c.insert(&"x");
        c.clear();
        assert_eq!(c.total(), 0);
        assert_eq!(c.estimate(&"x"), 0);
    }

    #[test]
    fn merge_adds_counts() {
        let mut a = cms();
        let mut b = cms();
        for _ in 0..100 { a.insert(&"key"); }
        for _ in 0..200 { b.insert(&"key"); }
        a.merge(&b);
        assert!(a.estimate(&"key") >= 300);
        assert_eq!(a.total(), 300);
    }
}
