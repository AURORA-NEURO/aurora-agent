package controlplane

import (
	"errors"
	"testing"
)

func TestPlacementIsDeterministicAcrossCalls(t *testing.T) {
	a, err := HomeShard(16, 12345)
	if err != nil {
		t.Fatalf("home shard: %v", err)
	}
	b, _ := HomeShard(16, 12345)
	if a != b {
		t.Fatalf("same key placed on %s then %s", a, b)
	}
}

func TestAddingAShardMovesOnlyAMinorityOfKeys(t *testing.T) {
	moved := 0
	for key := uint64(0); key < 2000; key++ {
		old, _ := HomeShard(8, key)
		neo, _ := HomeShard(9, key)
		if old != neo {
			moved++
		}
	}
	if moved >= 1000 {
		t.Fatalf("%d/2000 keys moved growing 8 to 9 shards — disruption bound broken", moved)
	}
}

func TestKeysSpreadAcrossShardsWithoutHotspots(t *testing.T) {
	counts := make(map[uint64]int)
	for key := uint64(0); key < 10000; key++ {
		s, _ := HomeShard(16, key)
		counts[s.Raw()]++
	}
	if len(counts) != 16 {
		t.Fatalf("only %d of 16 shards ever received a key", len(counts))
	}
	minCount, maxCount := 1<<31-1, 0
	for _, c := range counts {
		if c < minCount {
			minCount = c
		}
		if c > maxCount {
			maxCount = c
		}
	}
	if maxCount >= 3*minCount {
		t.Fatalf("hotspot: max %d vs min %d over 16 shards", maxCount, minCount)
	}
}

func TestPreferenceOrderIsStableCompleteAndTieBrokenByIndex(t *testing.T) {
	first, _ := PreferenceOrder(7, 42)
	second, _ := PreferenceOrder(7, 42)
	if len(first) != 7 {
		t.Fatalf("preference order lost shards: %d", len(first))
	}
	seen := map[uint64]bool{}
	for _, s := range first {
		if seen[s.Raw()] {
			t.Fatalf("shard %d appears twice", s.Raw())
		}
		seen[s.Raw()] = true
	}
	for i := range first {
		if first[i] != second[i] {
			t.Fatalf("order unstable at %d: %s vs %s", i, first[i], second[i])
		}
	}
}

func TestOutOfRangeShardCountIsAnErrorNotASilentRoute(t *testing.T) {
	if _, err := HomeShard(0, 1); !errors.Is(err, ErrShardCount) {
		t.Fatalf("zero shards: got %v", err)
	}
	if _, err := HomeShard(maxShardCount+1, 1); !errors.Is(err, ErrShardCount) {
		t.Fatalf("%d shards: got %v", maxShardCount+1, err)
	}
}

func TestSHA256AssignmentMatchesTheCPythonScaleLayerVectors(t *testing.T) {
	loadParity(t)
	var vectors []assignShardVector
	paritySection(t, "assign_shard", &vectors)
	for _, v := range vectors {
		got, err := AssignShardSHA256(v.Key, v.ShardCount)
		if err != nil {
			t.Fatalf("assign %q/%d: %v", v.Key, v.ShardCount, err)
		}
		if got != v.Shard {
			t.Fatalf("AssignShardSHA256(%q,%d) = %d, CPython says %d", v.Key, v.ShardCount, got, v.Shard)
		}
	}
}

func TestAssignShardRejectsNonPositiveShardCounts(t *testing.T) {
	if _, err := AssignShardSHA256("same", 0); !errors.Is(err, ErrShardCountPositive) {
		t.Fatalf("zero shard count: got %v", err)
	}
}

func TestRendezvousHomeShardMatchesTheRustExecutedVectors(t *testing.T) {
	loadParity(t)
	var vectors []homeShardVector
	paritySection(t, "home_shard", &vectors)
	for _, v := range vectors {
		got, err := HomeShard(v.ShardCount, mustU64(t, v.Key))
		if err != nil {
			t.Fatalf("home %d/%s: %v", v.ShardCount, v.Key, err)
		}
		if int(got.Raw()) != v.Shard {
			t.Fatalf("HomeShard(%d,%s) = %s, Rust vector says shard %d",
				v.ShardCount, v.Key, got, v.Shard)
		}
	}
}

func TestFullPreferenceOrderMatchesTheRustExecutedVector(t *testing.T) {
	loadParity(t)
	var vectors []preferenceOrderVector
	paritySection(t, "preference_order", &vectors)
	for _, v := range vectors {
		got, err := PreferenceOrder(v.ShardCount, mustU64(t, v.Key))
		if err != nil {
			t.Fatalf("order %d/%s: %v", v.ShardCount, v.Key, err)
		}
		if len(got) != len(v.Order) {
			t.Fatalf("order length %d, vector says %d", len(got), len(v.Order))
		}
		for i := range got {
			if int(got[i].Raw()) != v.Order[i] {
				t.Fatalf("position %d = %s, vector says %d", i, got[i], v.Order[i])
			}
		}
	}
}
