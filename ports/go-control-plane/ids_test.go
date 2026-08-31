package controlplane

import (
	"strings"
	"testing"
)

func TestZeroIsNeverIssuedForAnyTypedID(t *testing.T) {
	if _, err := NewAgentID(0); err == nil {
		t.Fatal("AgentID issued for raw 0")
	}
	if _, err := NewTaskID(0); err == nil {
		t.Fatal("TaskID issued for raw 0")
	}
	if _, err := NewShardID(0); err == nil {
		t.Fatal("ShardID issued for raw 0")
	}
	if a, _ := NewAgentID(1); a.Raw() != 1 {
		t.Fatalf("nonzero id mangled: %d", a.Raw())
	}
}

func TestDistinctIDTypesDoNotInterconvertAndDisplayNamesTheKind(t *testing.T) {
	a, _ := NewAgentID(3)
	tk, _ := NewTaskID(3)
	s, _ := NewShardID(3)
	if a.String() != "agent-3" || tk.String() != "task-3" || s.String() != "shard-3" {
		t.Fatalf("display must name the kind: %s %s %s", a, tk, s)
	}
}

func TestDerivedKeysAreDeterministicAndAsymmetricForTrivialInputs(t *testing.T) {
	if DeriveIdempotencyKey(1, 2) != DeriveIdempotencyKey(1, 2) {
		t.Fatal("derivation is not deterministic")
	}
	if DeriveIdempotencyKey(1, 2) == DeriveIdempotencyKey(2, 1) {
		t.Fatal("(a,b) and (b,a) collided; derivation is not injective on the sample set")
	}
}

func TestMix64MatchesTheRustExecutedVectorsExactly(t *testing.T) {
	loadParity(t)
	var vectors []mix64Vector
	paritySection(t, "mix64", &vectors)
	for _, v := range vectors {
		x, y := mustU64(t, v.X), mustU64(t, v.Y)
		if got := Mix64(x); got != y {
			t.Fatalf("Mix64(%d) = %d, vector says %d", x, got, y)
		}
	}
}

func TestIdempotencyKeyHexMatchesTheRustExecutedVectors(t *testing.T) {
	loadParity(t)
	var vectors []deriveVector
	paritySection(t, "idempotency_derive_hex", &vectors)
	for _, v := range vectors {
		key := DeriveIdempotencyKey(mustU64(t, v.A), mustU64(t, v.B))
		if key.Hex() != v.Hex {
			t.Fatalf("derive(%s,%s).hex = %s, vector says %s", v.A, v.B, key.Hex(), v.Hex)
		}
	}
}

func TestEpochsAreOnlyMintedByTablesSoForgingRequiresPackageInternals(t *testing.T) {
	table := NewLeaseTable()
	task, _ := NewTaskID(4)
	agent, _ := NewAgentID(4)
	h, err := table.Grant(task, agent, 0, 5)
	if err != nil {
		t.Fatalf("grant: %v", err)
	}
	if h.Epoch().Raw() == 0 {
		t.Fatal("minted epoch of zero")
	}
	if !strings.HasPrefix(h.Epoch().String(), "epoch-") {
		t.Fatalf("epoch display lost its kind prefix: %s", h.Epoch())
	}
}
