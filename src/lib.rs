//! # roughly
//!
//! Probabilistic data structures for Rust — because sometimes close enough is good enough.
//!
//! This crate provides space-efficient, approximate data structures for common tasks:
//!
//! - [`BloomFilter`] — membership testing with configurable false positive rate
//! - [`HyperLogLog`] — cardinality estimation (count distinct elements)
//! - [`CountMinSketch`] — frequency estimation (how often items appear)
//!
//! ## Quick Start
//!
//! ```rust
//! use roughly::prelude::*;
//!
//! // Bloom filter: "Have I seen this before?"
//! let mut bloom = BloomFilter::builder()
//!     .expected_items(10_000)
//!     .false_positive_rate(0.01)
//!     .build();
//! bloom.insert(&"hello");
//! assert!(bloom.contains(&"hello"));
//!
//! // HyperLogLog: "How many unique items?"
//! let mut hll = HyperLogLog::builder().std_error(0.01).build();
//! for i in 0..100_000u64 {
//!     hll.insert(&i);
//! }
//! println!("Estimated unique count: {}", hll.count());
//!
//! // Count-Min Sketch: "How often did this appear?"
//! let mut cms = CountMinSketch::builder().build();
//! cms.insert_many(&"popular", 1000);
//! println!("Estimated frequency: {}", cms.estimate(&"popular"));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod bloom;
pub mod countmin;
pub mod hash;
pub mod hyperloglog;

pub use bloom::BloomFilter;
pub use countmin::CountMinSketch;
pub use hyperloglog::HyperLogLog;

/// Traits defining common interfaces for probabilistic data structures.
pub mod traits {
    use core::hash::Hash;

    /// A probabilistic set for membership testing.
    ///
    /// Implementations may return false positives but never false negatives.
    pub trait MembershipSketch<T: Hash> {
        /// Inserts an item into the sketch.
        fn insert(&mut self, item: &T);

        /// Returns `true` if the item may be in the set.
        fn contains(&self, item: &T) -> bool;

        /// Returns the configured false positive rate.
        fn false_positive_rate(&self) -> f64;

        /// Returns the number of items inserted.
        fn len(&self) -> usize;

        /// Returns `true` if no items have been inserted.
        fn is_empty(&self) -> bool {
            self.len() == 0
        }

        /// Resets the sketch to its initial empty state.
        fn clear(&mut self);
    }

    /// A probabilistic data structure for cardinality estimation.
    ///
    /// Estimates the number of distinct elements in a stream.
    pub trait CardinalitySketch<T: Hash> {
        /// Inserts an item into the sketch.
        fn insert(&mut self, item: &T);

        /// Returns the estimated number of distinct items.
        fn count(&self) -> u64;

        /// Returns the standard error of the estimate.
        fn std_error(&self) -> f64;

        /// Merges another sketch into this one.
        fn merge(&mut self, other: &Self);

        /// Resets the sketch to its initial empty state.
        fn clear(&mut self);
    }

    /// A probabilistic data structure for frequency estimation.
    ///
    /// May overcount but never undercounts.
    pub trait FrequencySketch<T: Hash> {
        /// Inserts an item once.
        fn insert(&mut self, item: &T);

        /// Inserts an item with a given count.
        fn insert_many(&mut self, item: &T, count: u64);

        /// Returns the estimated frequency of an item.
        fn estimate(&self, item: &T) -> u64;

        /// Returns the configured error rate.
        fn error_rate(&self) -> f64;

        /// Returns the configured confidence level.
        fn confidence(&self) -> f64;

        /// Returns the total count of all insertions.
        fn total(&self) -> u64;

        /// Resets the sketch to its initial empty state.
        fn clear(&mut self);
    }
}

/// Convenient re-exports of the main types and traits.
pub mod prelude {
    pub use crate::bloom::BloomFilter;
    pub use crate::countmin::CountMinSketch;
    pub use crate::hyperloglog::HyperLogLog;
    pub use crate::traits::{CardinalitySketch, FrequencySketch, MembershipSketch};
}
