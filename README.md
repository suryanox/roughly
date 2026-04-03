# roughly

[![CI](https://github.com/suryanox/roughly/actions/workflows/ci.yaml/badge.svg)](https://github.com/suryanox/roughly/actions)
[![Crates.io](https://img.shields.io/crates/v/roughly.svg)](https://crates.io/crates/roughly)
[![Downloads](https://img.shields.io/crates/d/roughly)](https://crates.io/crates/roughly)

> Probabilistic data structures for Rust, because sometimes close enough is good enough.

`roughly` provides a unified, ergonomic API for the three most widely-used probabilistic data structures. Each answers a different question about your data stream:

| Structure | Question | Memory | Error |
|---|---|---|---|
| `BloomFilter` | Is this item in the set? | O(n) bits | Tunable false-positive rate |
| `HyperLogLog` | How many unique items? | O(log log n) | ~2% std error |
| `CountMinSketch` | How often does this item appear? | O(1/ε · log 1/δ) | Tunable ε, δ |

---
## Why `roughly`?

The Rust ecosystem has several fragmented crates for individual probabilistic structures, but no single crate that:

- Provides all three structures with a **consistent, ergonomic API**
- Uses **builder pattern** with human-readable parameters (set false-positive rate, not number of bits)
- Abstracts hashing behind a **generic `BuildHasher`** trait
- Exposes **common traits** (`MembershipSketch`, `CardinalitySketch`, `FrequencySketch`) for trait-object use
- Supports **`serde`** serialization and **`no_std`** environments

## Installation

```toml
[dependencies]
roughly = "0.1"
```

## Quick Start

```rust
use roughly::prelude::*;

// "Have I seen this URL before?"

let mut bloom = BloomFilter::builder()
    .expected_items(1_000_000)
    .false_positive_rate(0.01)   // 1% false positive rate
    .build();

bloom.insert(&"https://example.com");
assert!(bloom.contains(&"https://example.com"));  // always true
bloom.contains(&"https://other.com") => probably false
```


```rust
// "How many unique users visited today?"

let mut hll = HyperLogLog::builder()
    .std_error(0.01)   // ±1% standard error
    .build();

for user_id in 0..1_000_000u64 {
    hll.insert(&user_id);
}
println!("~{} unique users", hll.count()); // ~1_000_000
```


```rust
// "What's the frequency of this search term?"

let mut cms = CountMinSketch::builder()
    .error_rate(0.001)   // error ≤ 0.1% of total events
    .confidence(0.99)    // holds with 99% probability
    .build();

for term in search_log {
    cms.insert(&term);
}
println!("'rust' appears ~{} times", cms.estimate(&"rust"));
```
---

## Design

### Traits

All structures implement one of three traits, enabling polymorphic use:

```rust
pub trait MembershipSketch<T: Hash> {
    fn insert(&mut self, item: &T);
    fn contains(&self, item: &T) -> bool;
    fn false_positive_rate(&self) -> f64;
    fn len(&self) -> usize;
    fn clear(&mut self);
}

pub trait CardinalitySketch<T: Hash> {
    fn insert(&mut self, item: &T);
    fn count(&self) -> u64;
    fn std_error(&self) -> f64;
    fn merge(&mut self, other: &Self);
    fn clear(&mut self);
}

pub trait FrequencySketch<T: Hash> {
    fn insert(&mut self, item: &T);
    fn insert_many(&mut self, item: &T, count: u64);
    fn estimate(&self, item: &T) -> u64;
    fn error_rate(&self) -> f64;
    fn confidence(&self) -> f64;
    fn total(&self) -> u64;
    fn clear(&mut self);
}
```
---
### Custom Hashers

All structures are generic over `BuildHasher`. The default is `AHash`:

```rust
use std::collections::hash_map::RandomState;

let bloom = BloomFilter::builder_with_hasher(RandomState::new())
    .expected_items(10_000)
    .false_positive_rate(0.01)
    .build();
```

### Serialization

Enable the `serde` feature to serialize/deserialize any structure:

```toml
roughly = { version = "0.1", features = ["serde"] }
```

```rust
let serialized = serde_json::to_string(&bloom)?;
let restored: BloomFilter = serde_json::from_str(&serialized)?;
```

### `no_std`

Disable the `std` feature (requires `alloc`):

```toml
roughly = { version = "0.1", default-features = false }
```

## Algorithms

### BloomFilter

Uses the **Kirsch–Mitzenmacher** double-hashing trick to simulate `k` independent hash functions from two base hashes, avoiding the cost of `k` full hash evaluations. Optimal `m` (bits) and `k` (hash functions) are computed from your target false-positive rate and expected item count.

### HyperLogLog

64-bit HyperLogLog with:
- Bias-corrected harmonic mean estimator (Flajolet et al.)
- Small-range linear counting correction
- Large-range logarithmic correction
- `merge()` support for distributed cardinality estimation

### CountMinSketch

Standard Count-Min Sketch (Cormode & Muthukrishnan). Width and depth computed from `error_rate` and `confidence`:

```
width = ceil(e / error_rate)
depth = ceil(ln(1/(1-confidence)) / ln(2))
```

## License

Licensed under either of [MIT](LICENSE)
