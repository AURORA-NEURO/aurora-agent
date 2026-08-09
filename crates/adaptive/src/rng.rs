//! Deterministic pseudo-randomness for the cluster bootstrap.
//!
//! SplitMix64, seeded explicitly. Blueprint 08.08 requires the seed to appear in the selection
//! record, and 08.01 requires that "every selection records ... random seed". A resampling
//! interval drawn from a system RNG would make the reported uncertainty itself irreproducible,
//! which is a strange property for the number that decides whether a release ships.
//!
//! This duplicates the generator in `bioprism-worldgen` rather than depending on it. That is
//! deliberate: the inference path should not link the world generator, and a shared RNG would
//! couple the reproducibility of an interval to changes in an unrelated crate.

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

    /// A uniform index below `bound`.
    ///
    /// Uses the modulo reduction, which is very slightly biased when `bound` does not divide
    /// `2^64`. For bootstrap cluster indices — bounds in the tens or hundreds — the bias is of
    /// order `bound / 2^64` and is far below the Monte-Carlo error of the resampling itself.
    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }

    /// A uniform draw on `[0, 1)`, from the top 53 bits.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn unit_draws_stay_inside_the_half_open_unit_interval() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..10_000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }
}
