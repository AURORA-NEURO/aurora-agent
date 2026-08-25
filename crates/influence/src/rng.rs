//! Deterministic pseudo-randomness.
//!
//! SplitMix64, copied from `bioprism-worldgen` rather than depended on: this crate's dependency
//! set is deliberately the four it actually uses, and pulling in a world generator to obtain
//! sixty-four bits of arithmetic would be a worse trade than duplicating twenty lines. The
//! duplication is stated here so it does not read as an accident.
//!
//! Nothing in this crate reads a clock or a system entropy source. A soundness suite whose
//! counterexamples cannot be reproduced is a suite that reports its failures once.

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

    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }

    /// A value in `[0, 1)` with 53 bits of resolution.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A value in `[low, high)`.
    pub fn between(&mut self, low: f64, high: f64) -> f64 {
        low + self.unit() * (high - low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator is hand-copied into this crate, so its determinism is pinned per copy.
    ///
    /// A soundness counterexample is reported as a seed. If the seed did not fix the stream the
    /// report would name a run nobody else can reach, so the two instances below must agree and a
    /// different seed must not.
    #[test]
    fn the_same_seed_produces_the_same_stream() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        let mut c = SplitMix64::new(43);
        let from_a: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let from_b: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let from_c: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(from_a, from_b);
        assert_ne!(from_a, from_c);
    }
}
