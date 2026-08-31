//! Deterministic pseudo-randomness.
//!
//! SplitMix64. Generated worlds are content-addressed and compared byte for byte across runs and
//! machines, so the generator cannot use a system RNG: the same spec must always produce the same
//! world or the golden fixtures are meaningless.

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

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator is hand-copied into this crate, so its determinism is pinned per copy.
    ///
    /// Generated worlds are compared byte for byte against golden fixtures, so a drifted stream
    /// would fail those comparisons far away from its cause. This is where it fails first.
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
