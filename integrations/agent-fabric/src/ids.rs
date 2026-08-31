//! Typed identifiers and the 64-bit mixing primitive shared by shard placement and key
//! derivation.
//!
//! Every identifier is a distinct type on purpose: an `AgentId` must never be assignable to a
//! `TaskId` slot by accident, so there are no `From` conversions between id types and the inner
//! integer is private. Zero is never issued, so `from_raw(0)` is `None` — a decoded id of zero
//! from a transport frame is a protocol error, not a valid handle.

use std::fmt;

macro_rules! plain_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub struct $name(u64);

        impl $name {
            /// Constructs a typed id from a raw counter value. `0` is never issued.
            pub fn from_raw(raw: u64) -> Option<Self> {
                if raw == 0 { None } else { Some(Self(raw)) }
            }

            pub(crate) fn new(raw: u64) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "-{}"), self.0)
            }
        }
    };
}

plain_id!(
    /// A logical agent registered in the fabric. Thousands may exist; none implies a thread or
    /// process.
    AgentId,
    "agent"
);
plain_id!(
    /// A unit of work admitted to the fabric.
    TaskId,
    "task"
);
plain_id!(
    /// A logical placement partition. Shards are in-process queues, not hosts.
    ShardId,
    "shard"
);

/// Epoch counter minted by the lease table on every grant or renewal. Two handles with equal
/// epochs can never both be live for one task because grants bump the epoch monotonically under
/// the table's exclusivity rule.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LeaseEpoch(u64);

impl LeaseEpoch {
    pub(crate) fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LeaseEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "epoch-{}", self.0)
    }
}

/// Deduplication key for submissions. Callers may supply one; otherwise the fabric derives it
/// from task identity and payload digest, so a retried *submission* (same key) is recognized as
/// a duplicate while a genuinely new submission is not.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IdempotencyKey([u8; 16]);

impl IdempotencyKey {
    /// Deterministically derives a key by mixing two 64-bit halves. Used when a caller does not
    /// name a key explicitly.
    pub fn derive(a: u64, b: u64) -> Self {
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&mix64(a).to_be_bytes());
        k[8..].copy_from_slice(&mix64(b).to_be_bytes());
        Self(k)
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn hex(&self) -> String {
        let mut s = String::with_capacity(32);
        for b in self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Short display keeps logs readable; full fidelity lives in hex().
        let mut s = String::with_capacity(8);
        for b in self.0.iter().take(4) {
            s.push_str(&format!("{b:02x}"));
        }
        write!(f, "idem-{s}…")
    }
}

/// SplitMix64 finalizer. Shared here rather than in a rng module because shard rendezvous
/// scoring, key derivation and the simulator's PRNG all need exactly this avalanche behaviour
/// and nothing more.
pub fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_never_a_valid_raw_id() {
        assert!(AgentId::from_raw(0).is_none());
        assert!(AgentId::from_raw(1).is_some());
    }

    #[test]
    fn distinct_id_types_do_not_interconvert() {
        // The claim is compile-time: these are different nominal types with no From/Into path.
        let a = AgentId::from_raw(3).expect("nonzero");
        let t = TaskId::from_raw(3).expect("nonzero");
        assert_ne!(a.to_string(), t.to_string(), "display names the kind");
    }

    #[test]
    fn derived_keys_are_deterministic_and_collision_resistant_for_trivial_inputs() {
        assert_eq!(IdempotencyKey::derive(1, 2), IdempotencyKey::derive(1, 2));
        assert_ne!(IdempotencyKey::derive(1, 2), IdempotencyKey::derive(2, 1));
        assert_eq!(IdempotencyKey::derive(1, 2).as_bytes().len(), 16);
    }

    #[test]
    fn mix64_is_a_bijective_finalizer_on_sample_points() {
        // Not a proof of bijectivity, but catches sign/truncation slips: distinct inputs stay
        // distinct and the function is stable across calls.
        for x in [0u64, 1, u64::MAX, 0xDEAD_BEEF] {
            assert_eq!(mix64(x), mix64(x));
        }
        assert_ne!(mix64(0), mix64(1));
    }
}
