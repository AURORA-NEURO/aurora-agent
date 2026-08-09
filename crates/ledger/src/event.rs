//! The typed event that gets appended, and the vocabulary it is built from.
//!
//! Blueprint 40.09 lists the inputs as "typed event payload", "actor/role", "causal parent(s)"
//! and "valid/record/release time"; blueprint 12.04 adds that payloads are large and are
//! referenced rather than inlined. Both shape this module.
//!
//! The payload is held behind its own content digest ([`Payload`]) rather than being hashed
//! inline with the rest of the event. That indirection buys the one thing an append-only log
//! normally cannot offer: 12.22 requires that a lawful deletion request "remove or
//! cryptographically destroy protected payloads while retaining minimal non-identifying
//! tombstones". Because the chain commits to the payload *digest*, a redacted entry still
//! verifies — the record of what happened survives the destruction of what it said.

use crate::error::LedgerError;
use crate::time::EventTimes;
use bioprism_ids::{ContentHash, EventId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fmt;

/// The five transition families 40.09 requires the ledger to record.
///
/// Kept as a closed enum because the set is a claim about what the platform considers a
/// material change, not an extension point; a new family is a blueprint change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    /// A change in the world itself: a specimen, a measurement, an observed outcome.
    Material,
    /// A change in what is believed or known: a claim, a retraction, a confidence revision.
    Epistemic,
    /// A change in a run: a job started, a trial finished, an attempt abandoned.
    Execution,
    /// A change in what is permitted: a policy decision, a grant, a revocation.
    Policy,
    /// A change in a score or a judgement.
    Evaluation,
}

impl EventClass {
    pub const ALL: [EventClass; 5] = [
        EventClass::Material,
        EventClass::Epistemic,
        EventClass::Execution,
        EventClass::Policy,
        EventClass::Evaluation,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            EventClass::Material => "material",
            EventClass::Epistemic => "epistemic",
            EventClass::Execution => "execution",
            EventClass::Policy => "policy",
            EventClass::Evaluation => "evaluation",
        }
    }
}

impl fmt::Display for EventClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

macro_rules! text_newtype {
    ($(#[$meta:meta])* $name:ident, $field:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, LedgerError> {
                let value = value.into();
                if value.is_empty() || value.chars().any(|c| c.is_control()) {
                    return Err(LedgerError::MalformedField { field: $field, value });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = LedgerError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

text_newtype!(
    /// The specific event type within its class, e.g. `specimen.collected`.
    ///
    /// Free-form by default; a closed [`SchemaCatalog`] turns an unrecognised kind into the
    /// "schema unknown" failure 40.09 declares.
    EventKind,
    "event kind"
);
text_newtype!(
    /// What the event is *about*.
    ///
    /// Projections and as-of state queries are keyed on this. Without it "what was true at
    /// time T" has no referent, because the answer is per-thing, not per-log.
    SubjectKey,
    "subject"
);
text_newtype!(
    /// The client-supplied deduplication key of 12.17 ("operation create by client key").
    IdempotencyKey,
    "idempotency key"
);

/// Who caused the event, and in what capacity.
///
/// Role is mandatory and separate from identity because 40.09's security section requires
/// least authority: the same person acting as a curator and as an evaluator are different
/// actors for audit purposes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub role: String,
}

impl Actor {
    pub fn new(id: impl Into<String>, role: impl Into<String>) -> Result<Self, LedgerError> {
        let id = id.into();
        let role = role.into();
        if id.is_empty() || id.chars().any(|c| c.is_control()) {
            return Err(LedgerError::MalformedField {
                field: "actor id",
                value: id,
            });
        }
        if role.is_empty() || role.chars().any(|c| c.is_control()) {
            return Err(LedgerError::MalformedField {
                field: "actor role",
                value: role,
            });
        }
        Ok(Actor { id, role })
    }
}

/// The event body, or the tombstone left where it used to be.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum PayloadBody {
    Present {
        value: Value,
    },
    /// 12.22 privacy deletion: the bytes are gone, the fact that they existed is not.
    Redacted {
        reason: String,
    },
}

/// A payload addressed by the digest of its original bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    digest: ContentHash,
    body: PayloadBody,
}

impl Payload {
    pub fn new(value: Value) -> Result<Self, LedgerError> {
        let digest = ContentHash::of_value(&value)?;
        Ok(Payload {
            digest,
            body: PayloadBody::Present { value },
        })
    }

    /// The digest of the *original* value, which survives redaction. This is what the hash
    /// chain commits to.
    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    pub fn body(&self) -> &PayloadBody {
        &self.body
    }

    pub fn is_redacted(&self) -> bool {
        matches!(self.body, PayloadBody::Redacted { .. })
    }

    /// Reads the body, or explains that it was destroyed and why.
    pub fn value(&self, id: &EventId) -> Result<&Value, LedgerError> {
        match &self.body {
            PayloadBody::Present { value } => Ok(value),
            PayloadBody::Redacted { reason } => Err(LedgerError::PayloadRedacted {
                id: id.clone(),
                reason: reason.clone(),
            }),
        }
    }

    pub(crate) fn redact(&mut self, reason: String) {
        self.body = PayloadBody::Redacted { reason };
    }
}

/// A submission: everything the caller supplies, before the ledger assigns it an identity.
///
/// The type is separate from the recorded entry because the caller cannot choose a sequence
/// number, an event id or a chain digest — those are the ledger's to assign (40.09 execution
/// step 2, "assign monotonic ID").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub class: EventClass,
    pub kind: EventKind,
    pub actor: Actor,
    pub subject: SubjectKey,
    pub times: EventTimes,
    /// Sorted and deduplicated on construction, so two submissions that differ only in the
    /// order they listed their parents are the same event.
    causal_parents: Vec<EventId>,
    pub supersedes: Option<EventId>,
    idempotency_key: Option<IdempotencyKey>,
    pub payload: Payload,
}

impl Event {
    pub fn new(
        class: EventClass,
        kind: EventKind,
        actor: Actor,
        subject: SubjectKey,
        times: EventTimes,
        payload: Value,
    ) -> Result<Self, LedgerError> {
        Ok(Event {
            class,
            kind,
            actor,
            subject,
            times,
            causal_parents: Vec::new(),
            supersedes: None,
            idempotency_key: None,
            payload: Payload::new(payload)?,
        })
    }

    pub fn caused_by(mut self, parents: impl IntoIterator<Item = EventId>) -> Self {
        let set: BTreeSet<EventId> = self.causal_parents.drain(..).chain(parents).collect();
        self.causal_parents = set.into_iter().collect();
        self
    }

    /// Marks this event as the correction that replaces `target`. The original is never
    /// touched; 40.09 invariant 2 is "corrections supersede rather than delete".
    pub fn superseding(mut self, target: EventId) -> Self {
        self.supersedes = Some(target);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn causal_parents(&self) -> &[EventId] {
        &self.causal_parents
    }

    /// The digest of everything the caller supplied.
    ///
    /// Excludes sequence, id and chain position, so it is stable across replays of the same
    /// submission into differently-populated ledgers.
    pub fn content_digest(&self) -> String {
        ContentHash::of_value(&self.identity_value())
            .expect("event identity is strings and arrays of strings, which always canonicalize")
            .as_str()
            .to_string()
    }

    /// The key an append deduplicates on: the caller's, or the content digest.
    ///
    /// Defaulting to the content digest is what makes "the same event submitted twice does not
    /// duplicate" true without the caller having to opt in.
    pub fn idempotency_key(&self) -> IdempotencyKey {
        self.idempotency_key
            .clone()
            .unwrap_or_else(|| IdempotencyKey(format!("content:{}", self.content_digest())))
    }

    pub(crate) fn identity_value(&self) -> Value {
        json!({
            "class": self.class.as_str(),
            "kind": self.kind.as_str(),
            "actor_id": self.actor.id,
            "actor_role": self.actor.role,
            "subject": self.subject.as_str(),
            "valid_time": self.times.valid.to_string(),
            "record_time": self.times.record.to_string(),
            "release_time": self.times.release.to_string(),
            "causal_parents": self
                .causal_parents
                .iter()
                .map(|parent| parent.as_str())
                .collect::<Vec<_>>(),
            "supersedes": self.supersedes.as_ref().map(|target| target.as_str()),
            "payload_digest": self.payload.digest().as_str(),
        })
    }
}

/// Which event kinds the ledger will accept.
///
/// 40.09 execution step 1 is "validate schema and actor", and "schema unknown" is a declared
/// failure — but an open catalog is the right default for a general-purpose ledger, and a
/// closed one is the right choice for a production deployment. Both are available; neither is
/// implied.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SchemaCatalog {
    known: BTreeSet<String>,
    closed: bool,
}

impl SchemaCatalog {
    /// Accepts any well-formed kind.
    pub fn open() -> Self {
        SchemaCatalog::default()
    }

    /// Accepts only the listed kinds; anything else is [`LedgerError::UnknownSchema`].
    pub fn closed(kinds: impl IntoIterator<Item = &'static str>) -> Self {
        SchemaCatalog {
            known: kinds.into_iter().map(str::to_string).collect(),
            closed: true,
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn admits(&self, kind: &EventKind) -> Result<(), LedgerError> {
        if !self.closed || self.known.contains(kind.as_str()) {
            Ok(())
        } else {
            Err(LedgerError::UnknownSchema {
                kind: kind.as_str().to_string(),
            })
        }
    }
}
