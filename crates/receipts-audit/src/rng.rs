//! Deterministic pseudo-randomness for generated audit cases.
//!
//! SplitMix64, hand-rolled for the same reason `bioprism_worldgen` hand-rolls it: a case that
//! exposes a hole is worth nothing unless the person reading the failure can regenerate it. Every
//! battery run is a pure function of its seed, and the seed is printed with every reported hole,
//! so a failure message is a complete reproduction recipe.

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

    /// A value in `0..bound`, or `0` when the range is empty.
    pub fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SplitMix64;

    #[test]
    fn the_same_seed_replays_the_same_stream() {
        let mut left = SplitMix64::new(0x5EED);
        let mut right = SplitMix64::new(0x5EED);
        let left: Vec<u64> = (0..64).map(|_| left.next_u64()).collect();
        let right: Vec<u64> = (0..64).map(|_| right.next_u64()).collect();
        assert_eq!(left, right);
    }

    #[test]
    fn different_seeds_diverge_on_the_first_draw() {
        assert_ne!(SplitMix64::new(1).next_u64(), SplitMix64::new(2).next_u64());
    }

    #[test]
    fn below_stays_inside_its_bound_and_returns_zero_for_an_empty_range() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..256 {
            assert!(rng.below(5) < 5);
        }
        assert_eq!(rng.below(0), 0);
    }
}
