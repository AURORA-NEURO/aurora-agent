package controlplane

import (
	"encoding/hex"
	"errors"
	"fmt"
)

// ErrZeroID reports a raw identifier of zero, which no constructor issues: a
// decoded id of zero from a transport frame is a protocol error, not a handle.
var ErrZeroID = errors.New("identifier raw value 0 is never issued")

// AgentID identifies a logical agent registered in a control plane. Thousands
// may exist; none implies a thread or an OS process. The type exists so an
// AgentID can never be assigned to a TaskID slot by accident: they are distinct
// named types with no conversion path and private fields.
type AgentID struct{ raw uint64 }

// NewAgentID constructs an id from a counter value; zero is rejected.
func NewAgentID(raw uint64) (AgentID, error) {
	if raw == 0 {
		return AgentID{}, ErrZeroID
	}
	return AgentID{raw}, nil
}

// Raw exposes the counter value for placement keys and telemetry.
func (a AgentID) Raw() uint64 { return a.raw }

func (a AgentID) String() string { return fmt.Sprintf("agent-%d", a.raw) }

// TaskID identifies one unit of work admitted to a control plane.
type TaskID struct{ raw uint64 }

// NewTaskID constructs an id from a counter value; zero is rejected.
func NewTaskID(raw uint64) (TaskID, error) {
	if raw == 0 {
		return TaskID{}, ErrZeroID
	}
	return TaskID{raw}, nil
}

// Raw exposes the counter value; it is also the rendezvous placement key.
func (t TaskID) Raw() uint64 { return t.raw }

func (t TaskID) String() string { return fmt.Sprintf("task-%d", t.raw) }

// ShardID names one logical placement partition — an in-process queue lane,
// never a host.
type ShardID struct{ raw uint64 }

// NewShardID constructs an id from a partition index; zero is rejected so the
// zero value stays unusable, matching every other id here.
func NewShardID(raw uint64) (ShardID, error) {
	if raw == 0 {
		return ShardID{}, ErrZeroID
	}
	return ShardID{raw}, nil
}

// Raw exposes the zero-based partition index callers use to select a queue.
func (s ShardID) Raw() uint64 { return s.raw }

func (s ShardID) String() string { return fmt.Sprintf("shard-%d", s.raw) }

// LeaseEpoch is minted by a LeaseTable on every grant. Two handles with equal
// epochs can never both be live for one task because grants bump the epoch
// monotonically under the table's exclusivity rule. The constructor is
// unexported on purpose: only a LeaseTable may mint one.
type LeaseEpoch struct{ raw uint64 }

func newLeaseEpoch(raw uint64) LeaseEpoch { return LeaseEpoch{raw} }

// Raw exposes the counter value for audit logs and cross-checking receipts.
func (e LeaseEpoch) Raw() uint64 { return e.raw }

func (e LeaseEpoch) String() string { return fmt.Sprintf("epoch-%d", e.raw) }

// IdempotencyKey deduplicates submissions. Callers may supply one; otherwise
// Derive builds it from task identity so a retried submission is recognized as
// a duplicate while a genuinely new submission is not.
type IdempotencyKey [16]byte

// Derive deterministically mixes two 64-bit halves into a key, matching
// agent-fabric's IdempotencyKey::derive byte for byte.
func DeriveIdempotencyKey(a, b uint64) IdempotencyKey {
	var k IdempotencyKey
	bePutUint64(k[0:8], Mix64(a))
	bePutUint64(k[8:16], Mix64(b))
	return k
}

// Hex renders the full key; this exact string appears in parity vectors.
func (k IdempotencyKey) Hex() string { return hex.EncodeToString(k[:]) }

// Mix64 is the SplitMix64 finalizer shared by shard rendezvous scoring and key
// derivation. It lives in one place because those call sites need exactly this
// avalanche behaviour and nothing more, and parity requires that nothing
// substitutes a different mixer.
func Mix64(x uint64) uint64 {
	x += 0x9E3779B97F4A7C15
	x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9
	x = (x ^ (x >> 27)) * 0x94D049BB133111EB
	return x ^ (x >> 31)
}

func bePutUint64(b []byte, v uint64) {
	for i := 7; i >= 0; i-- {
		b[i] = byte(v)
		v >>= 8
	}
}

func beUint64(b []byte) uint64 {
	var v uint64
	for _, c := range b {
		v = v<<8 | uint64(c)
	}
	return v
}
