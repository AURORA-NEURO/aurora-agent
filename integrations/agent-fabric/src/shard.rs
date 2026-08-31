//! Shard placement by rendezvous (highest-random-weight) hashing.
//!
//! Tasks get a deterministic preference order over shards derived from task identity; agents are
//! placed on shards the same way from their registration name. The property that matters is
//! *minimal disruption*: when a shard is added or removed, only keys whose top choice changed
//! move — everything else keeps its affinity. Scores use [`ids::mix64`] so placement is
//! reproducible across runs and platforms without any external dependency.

use crate::ids::{mix64, ShardId};

/// Rendezvous score for `key` on `shard`. Deterministic and avalanche-mixed.
fn score(key: u64, shard: u64) -> u64 {
    mix64(mix64(key) ^ mix64(shard.wrapping_add(0x517C_C1B7_2722_0A95)))
}

/// Full preference order over `shard_count` shards for `key`, best first. Panics on a zero or
/// absurd shard count because both indicate a configuration bug, not a runtime condition.
pub fn preference_order(shard_count: u64, key: u64) -> Vec<ShardId> {
    assert!(
        shard_count > 0 && shard_count <= 4096,
        "shard count must be in 1..=4096, got {shard_count}"
    );
    let mut ranked: Vec<(u64, ShardId)> = (0..shard_count)
        .map(|s| (score(key, s), ShardId::new(s)))
        .collect();
    // Sort by score descending with shard id as tiebreak so equal scores cannot reorder runs.
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.raw().cmp(&b.1.raw())));
    ranked.into_iter().map(|(_, s)| s).collect()
}

/// The single best shard for `key` — equivalent to `preference_order(..)[0]`.
pub fn home_shard(shard_count: u64, key: u64) -> ShardId {
    preference_order(shard_count, key)[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn placement_is_deterministic_across_calls() {
        let a = home_shard(16, 12345);
        let b = home_shard(16, 12345);
        assert_eq!(a, b);
    }

    #[test]
    fn adding_a_shard_moves_only_a_minority_of_keys() {
        // The rendezvous guarantee: growing 8 -> 9 shards reassigns roughly 1/9 of keys, and
        // never more than half of them even at this tiny scale.
        let mut moved = 0;
        for key in 0..2_000u64 {
            if home_shard(8, key) != home_shard(9, key) {
                moved += 1;
            }
        }
        assert!(
            moved < 1_000,
            "{moved}/2000 moved — disruption bound broken"
        );
    }

    #[test]
    fn keys_spread_across_shards_without_hotspots() {
        let mut counts: HashMap<ShardId, usize> = HashMap::new();
        for key in 0..10_000u64 {
            *counts.entry(home_shard(16, key)).or_default() += 1;
        }
        let max = counts.values().copied().max().expect("nonempty");
        let min = counts.values().copied().min().expect("nonempty");
        assert!(
            max < 3 * min,
            "hotspot: max {max} vs min {min} over 16 shards"
        );
    }

    #[test]
    fn tie_breaks_cannot_reorder_between_runs() {
        let o1 = preference_order(7, 42);
        let o2 = preference_order(7, 42);
        assert_eq!(o1, o2);
        assert_eq!(o1.len(), 7);
    }

    #[test]
    #[should_panic(expected = "shard count")]
    fn zero_shards_is_a_configuration_panic_not_a_silent_empty_route() {
        home_shard(0, 1);
    }
}
