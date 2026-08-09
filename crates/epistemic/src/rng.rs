//! A seeded generator, so a measured approximation ratio is a fact rather than a run.
//!
//! SplitMix64. Every instance family in this crate is generated from an explicit `u64` seed, and
//! no code path reads a clock or the system entropy pool. The approximation ratio reported in
//! [`crate::optimal::measure_ratio`] is a property of a named seed and a named family; a ratio
//! measured against an unseeded generator is a number that cannot be reproduced or argued with.

/// SplitMix64, the reference constants.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A float in `[0, 1)`, from the top 53 bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A float in `[lo, hi)`.
    pub fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }

    /// An integer in `[0, bound)`. Returns `0` for a bound of zero rather than dividing by it.
    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}
