//! Task envelopes, dispatch jobs, outcomes, completions and receipts.
//!
//! Provenance binding is structural: [`TaskEnvelope::compose`] computes the payload digest at
//! composition and the field is private, so an envelope that exists carries *some* digest and
//! `verify_payload` checks bytes against it. Receipts are minted only by the scheduler's
//! settlement path (`pub(crate)` constructor), so a receipt in circulation means the fabric
//! settled that task — there is no public constructor to forge one.

use crate::capability::CapabilitySet;
use crate::digest::{sha256, Digest};
use crate::ids::{AgentId, IdempotencyKey, LeaseEpoch, TaskId};
use crate::json::Value;
use std::fmt;

/// An admitted unit of work. The payload digest is computed once here and never recomputed from
/// mutable state — the envelope is immutable after composition.
#[derive(Clone, Debug)]
pub struct TaskEnvelope {
    task: TaskId,
    caps: CapabilitySet,
    payload: Vec<u8>,
    digest: Digest,
    key: IdempotencyKey,
    created_tick: u64,
    max_attempts: u32,
}

impl TaskEnvelope {
    /// Composes an envelope, binding the payload digest at birth. `key` may be supplied for
    /// caller-chosen idempotency; otherwise it is derived from task identity.
    pub fn compose(
        task: TaskId,
        payload: Vec<u8>,
        caps: CapabilitySet,
        key: Option<IdempotencyKey>,
        created_tick: u64,
        max_attempts: u32,
    ) -> Self {
        assert!(
            !caps.is_empty(),
            "a task with no required capability has no routing lane"
        );
        let digest = sha256(&payload);
        let key = key.unwrap_or_else(|| IdempotencyKey::derive(task.raw(), 0));
        Self {
            task,
            caps,
            payload,
            digest,
            key,
            created_tick,
            max_attempts,
        }
    }

    pub fn task(&self) -> TaskId {
        self.task
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.caps
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_digest(&self) -> Digest {
        self.digest
    }

    pub fn idempotency_key(&self) -> IdempotencyKey {
        self.key
    }

    pub fn created_tick(&self) -> u64 {
        self.created_tick
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Checks candidate bytes against the bound digest. Used by worker-side verification so a
    /// transport that mangled the payload fails loudly instead of executing garbage.
    pub fn verify_payload(&self, candidate: &[u8]) -> bool {
        sha256(candidate) == self.digest
    }
}

/// What a driver hands to the executor for one attempt.
#[derive(Clone, Debug)]
pub struct DispatchJob {
    pub envelope: TaskEnvelope,
    pub agent: AgentId,
    /// 1-based attempt number.
    pub attempt: u32,
    pub lease_epoch: LeaseEpoch,
    /// Cancellation generation observed at dispatch; a later generation means cancelled.
    pub cancel_gen: u64,
}

/// The result of one execution attempt. `Succeeded` carries raw result bytes whose digest the
/// scheduler re-derives at settlement — drivers claiming success do not get trusted on faith.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Succeeded {
        result: Vec<u8>,
    },
    Failed {
        reason: String,
    },
    /// Worker died or went silent before producing any verdict; detected via lease expiry.
    Crashed,
    CancelledBeforeStart,
}

impl Outcome {
    /// Terminal classification for the receipt ledger. `attempts` is the total used.
    pub fn terminal(&self) -> crate::envelope::Terminal {
        match self {
            Outcome::Succeeded { .. } => Terminal::Succeeded,
            Outcome::Failed { reason } => Terminal::Failed {
                reason: reason.clone(),
            },
            Outcome::Crashed => Terminal::Dropped,
            Outcome::CancelledBeforeStart => Terminal::Cancelled,
        }
    }
}

/// Settlement classification recorded on receipts. Distinct from [`Outcome`] because some paths
/// (payload corruption found by a worker, result-digest mismatch found at settlement) exist only
/// here, and collapsing them into a generic failure would hide exactly the events this fabric
/// exists to surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminal {
    Succeeded,
    Failed {
        reason: String,
    },
    Cancelled,
    /// Executed bytes did not match the envelope digest — provenance broken upstream.
    CorruptedPayload,
    /// Result bytes did not hash to the claimed digest — settlement refused the claim.
    ResultDigestMismatch,
    /// No verdict ever arrived; recovered by lease expiry after final attempt.
    Dropped,
}

impl fmt::Display for Terminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminal::Succeeded => write!(f, "succeeded"),
            Terminal::Failed { reason } => write!(f, "failed: {reason}"),
            Terminal::Cancelled => write!(f, "cancelled"),
            Terminal::CorruptedPayload => write!(f, "corrupted payload"),
            Terminal::ResultDigestMismatch => write!(f, "result digest mismatch"),
            Terminal::Dropped => write!(f, "dropped without a verdict"),
        }
    }
}

/// Reported by a driver when one attempt finished (or was abandoned).
#[derive(Clone, Debug)]
pub struct Completion {
    pub task: TaskId,
    pub agent: AgentId,
    pub attempt: u32,
    pub lease_epoch: LeaseEpoch,
    pub outcome: Outcome,
}

/// A per-agent record that a task reached settlement. Minted only inside the fabric; exposed
/// read-only. `cancel_requested` records whether cancellation raced the run — the outcome still
/// reflects what actually happened, because rewriting history would make receipts lies.
#[derive(Clone, Debug)]
pub struct Receipt {
    task: TaskId,
    agent: Option<AgentId>,
    attempts_used: u32,
    terminal: Terminal,
    payload_digest: Digest,
    submitted_tick: u64,
    settled_tick: u64,
    cancel_requested: bool,
}

impl Receipt {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        task: TaskId,
        agent: Option<AgentId>,
        attempts_used: u32,
        terminal: Terminal,
        payload_digest: Digest,
        submitted_tick: u64,
        settled_tick: u64,
        cancel_requested: bool,
    ) -> Self {
        Self {
            task,
            agent,
            attempts_used,
            terminal,
            payload_digest,
            submitted_tick,
            settled_tick,
            cancel_requested,
        }
    }

    pub fn task(&self) -> TaskId {
        self.task
    }

    pub fn agent(&self) -> Option<AgentId> {
        self.agent
    }

    pub fn attempts_used(&self) -> u32 {
        self.attempts_used
    }

    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub fn payload_digest(&self) -> Digest {
        self.payload_digest
    }

    pub fn submitted_tick(&self) -> u64 {
        self.submitted_tick
    }

    pub fn settled_tick(&self) -> u64 {
        self.settled_tick
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    pub fn to_json(&self) -> Value {
        let mut fields = vec![
            ("task_id", Value::Uint(self.task.raw())),
            (
                "agent_id",
                self.agent
                    .map(|a| Value::Uint(a.raw()))
                    .unwrap_or(Value::Null),
            ),
            ("attempts", Value::Uint(u64::from(self.attempts_used))),
            ("terminal", Value::str(self.terminal.to_string())),
            ("payload_digest", Value::str(self.payload_digest.hex())),
            ("submitted_tick", Value::Uint(self.submitted_tick)),
            ("settled_tick", Value::Uint(self.settled_tick)),
            ("cancel_requested", Value::Bool(self.cancel_requested)),
        ];
        if let Terminal::Failed { reason } = &self.terminal {
            fields.push(("reason", Value::str(reason.clone())));
        }
        Value::Obj(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;

    fn env(payload: &[u8]) -> TaskEnvelope {
        let caps = CapabilitySet::one(Capability::parse("compute").expect("cap"));
        TaskEnvelope::compose(
            TaskId::from_raw(1).expect("id"),
            payload.to_vec(),
            caps,
            None,
            0,
            3,
        )
    }

    #[test]
    fn composition_binds_the_payload_digest_and_verification_accepts_only_exact_bytes() {
        let e = env(b"hello");
        assert!(e.verify_payload(b"hello"));
        assert!(!e.verify_payload(b"hellO"), "one flipped bit must not pass");
        assert_eq!(e.payload_digest().hex().len(), 64);
    }

    #[test]
    fn envelopes_with_different_payloads_bind_different_digests() {
        let a = env(b"x");
        let b = env(b"y");
        assert_ne!(a.payload_digest(), b.payload_digest());
    }

    #[test]
    fn derived_idempotency_keys_follow_task_identity_when_not_supplied() {
        let e1 = env(b"same");
        // Different task ids must derive different keys even with identical payloads. Compose
        // both envelopes through the public constructor so the binding cannot be changed after
        // birth.
        let e2 = TaskEnvelope::compose(
            TaskId::from_raw(2).expect("id"),
            b"same".to_vec(),
            CapabilitySet::one(Capability::parse("compute").expect("cap")),
            None,
            0,
            3,
        );
        assert_ne!(e1.idempotency_key(), e2.idempotency_key());
    }

    #[test]
    fn receipt_json_round_trips_the_fields_a_transport_needs() {
        let r = Receipt::new(
            TaskId::from_raw(4).expect("id"),
            Some(AgentId::from_raw(9).expect("id")),
            2,
            Terminal::Failed {
                reason: "boom".into(),
            },
            Digest::from_hex(&"ab".repeat(32)).expect("hex"),
            5,
            9,
            true,
        );
        let v = r.to_json();
        assert_eq!(
            v.get("terminal").and_then(Value::as_str),
            Some("failed: boom")
        );
        assert_eq!(v.get("attempts").and_then(Value::as_u64), Some(2));
        assert_eq!(v.get("cancel_requested"), Some(&Value::Bool(true)));
    }
}
