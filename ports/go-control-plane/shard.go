package controlplane

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"sort"
)

// maxShardCount bounds preference-order allocation: a larger count indicates a
// configuration bug, not a runtime condition, so it is refused up front instead
// of materializing an absurd slice.
const maxShardCount = 4096

// ErrShardCount reports a shard count outside the supported 1..=4096 range.
var ErrShardCount = errors.New("shard count must be in 1..=4096")

// ErrShardCountPositive is the Python-parity guard for AssignShardSHA256,
// whose upstream contract only demands positivity.
var ErrShardCountPositive = errors.New("shard_count must be positive")

// shardScore is the rendezvous weight of key on shard. The constant and the
// double mix are transcribed from agent-fabric's shard.rs; the wrapping add
// matters because the reference relies on u64 wraparound for avalanche.
func shardScore(key, shard uint64) uint64 {
	return Mix64(Mix64(key) ^ Mix64(shard+0x517CC1B727220A95))
}

// PreferenceOrder returns the full rendezvous preference order over shardCount
// shards for key, best first. Determinism is total: equal scores cannot
// reorder runs because the shard index breaks ties in ascending order.
func PreferenceOrder(shardCount int, key uint64) ([]ShardID, error) {
	if err := validateShardRange(shardCount); err != nil {
		return nil, err
	}
	type weighted struct {
		score uint64
		shard uint64
	}
	ranked := make([]weighted, shardCount)
	for s := 0; s < shardCount; s++ {
		ranked[s] = weighted{shardScore(key, uint64(s)), uint64(s)}
	}
	sort.Slice(ranked, func(i, j int) bool {
		if ranked[i].score != ranked[j].score {
			return ranked[i].score > ranked[j].score
		}
		return ranked[i].shard < ranked[j].shard
	})
	order := make([]ShardID, shardCount)
	for i, w := range ranked {
		order[i] = ShardID{w.shard}
	}
	return order, nil
}

// HomeShard returns the single best shard for key — PreferenceOrder[0].
func HomeShard(shardCount int, key uint64) (ShardID, error) {
	order, err := PreferenceOrder(shardCount, key)
	if err != nil {
		return ShardID{}, err
	}
	return order[0], nil
}

// AssignShardSHA256 is the Python scale layer's placement rule: sha256 of the
// string key, first eight bytes big-endian, modulo the shard count. It
// deliberately coexists with HomeShard because the two upstream layers made
// different choices; each function matches its own authority exactly, and the
// mismatch between them is documented rather than papered over.
func AssignShardSHA256(key string, shardCount int) (int, error) {
	if shardCount <= 0 {
		return 0, ErrShardCountPositive
	}
	sum := sha256.Sum256([]byte(key))
	return int(beUint64(sum[:8]) % uint64(shardCount)), nil
}

func validateShardRange(n int) error {
	if n <= 0 || n > maxShardCount {
		return fmt.Errorf("%w: got %d", ErrShardCount, n)
	}
	return nil
}
