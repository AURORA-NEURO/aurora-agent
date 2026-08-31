//! A hash-linked audit chain, and an exact statement of who it is tamper-evident against.
//!
//! Blueprint 13.20 (audit log transparency and attestation) asks for "tamper-evident evidence of
//! sensitive actions, publications, key events, and evaluation provenance without exposing protected
//! content", implemented as "hash-linked batches, immutable storage, signed checkpoints, independent
//! backup, and periodic verification", and adds: "This provides tamper evidence, not a blockchain
//! requirement."
//!
//! # What the chain gives you
//!
//! Each entry's digest covers the previous entry's digest, so altering entry *n* changes every digest
//! from *n* onward. A reader holding any later digest can detect an edit to any earlier entry. That
//! is genuine, and it costs nothing but a hash.
//!
//! # Who it is tamper-evident *against*
//!
//! Against a party who can edit the log but cannot produce a checkpoint — no key. Such a party
//! rewrites entry 3, and every subsequent digest stops matching the checkpoint anyone else holds.
//!
//! **Not** against a party holding the key. They rewrite the log, recompute every link, and issue a
//! fresh checkpoint that verifies perfectly. Under symmetric authentication there is no external
//! witness, and this crate has none: no transparency log, no gossip protocol, no inclusion proof
//! against a third-party log, no Merkle consistency proof between two checkpoints, and no
//! timestamping authority to pin a checkpoint to a moment. 13.20 §Transparency's "public log for
//! releases, withdrawals, key revocations" is not implemented, and could not be made meaningful with
//! the primitives available.
//!
//! The one real defence against a key-holding rewriter is 13.20's "independent backup": a checkpoint
//! someone else already wrote down. That is an operational practice, not a property of this type.
//!
//! # Sensitive payloads are referenced, never copied
//!
//! 13.20 §Content requires the log to record "actor, subject, action, decision, policy/rationale,
//! operation, time, source, affected digests, and outcome" and that "sensitive payloads are
//! referenced, not copied". [`AuditEvent`] therefore has no payload field at all. There is nowhere to
//! put protected content, so no code path can put it there — the same move as everywhere else in this
//! crate. A caller who wants to log content logs its digest.
//!
//! # Deliberately not implemented
//!
//! No storage, no append-only medium, no retention or export policy, no batching (each entry links
//! individually), no time — [`AuditEvent::recorded_at`] is a caller-asserted string with nothing
//! corroborating it. No key lifecycle beyond an action name for one, no revocation, and no
//! redaction-with-proof.

use crate::attestation::{
    Attestation, AttestationPurpose, ClaimedProducer, KeyHolderAuthenticated,
};
use crate::error::BundleError;
use crate::mac::{KeyIdentity, SecretKey};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fmt;

/// The wire version of a chain link. Part of every link digest.
pub const AUDIT_SCHEMA_VERSION: &str = "bioprism-audit-chain/0.1";

/// The auditable actions 13.20 §Audit events enumerates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    AuthenticationOrPrivilegeChange,
    PolicyChange,
    HiddenOracleAccess,
    SensitiveArtifactAccess,
    Publication,
    ResultAcceptance,
    ReviewerDecision,
    KeyLifecycle,
    SecurityQuarantine,
    Deletion,
    FederationImport,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            AuditAction::AuthenticationOrPrivilegeChange => "authentication-or-privilege-change",
            AuditAction::PolicyChange => "policy-change",
            AuditAction::HiddenOracleAccess => "hidden-oracle-access",
            AuditAction::SensitiveArtifactAccess => "sensitive-artifact-access",
            AuditAction::Publication => "publication",
            AuditAction::ResultAcceptance => "result-acceptance",
            AuditAction::ReviewerDecision => "reviewer-decision",
            AuditAction::KeyLifecycle => "key-lifecycle",
            AuditAction::SecurityQuarantine => "security-quarantine",
            AuditAction::Deletion => "deletion",
            AuditAction::FederationImport => "federation-import",
        };
        f.write_str(text)
    }
}

/// What happened. `Failed` is distinct from `Denied`: a policy that refused and a system that broke
/// are different events, and merging them loses the one an operator needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AuditOutcome {
    Allowed,
    Denied { rationale: String },
    Failed { detail: String },
}

/// One auditable event. No payload field; see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub action: AuditAction,
    /// The key whose holder took the action. As everywhere in this crate, a key is not a party.
    pub actor_key_identity: KeyIdentity,
    /// A reference to the thing acted on — an id, a URI, a digest. Never the content itself.
    pub subject: String,
    /// 13.20 §Content's "policy/rationale".
    pub rationale: Option<String>,
    /// 13.20 §Content's "affected digests".
    pub affected_digests: Vec<ContentHash>,
    pub outcome: AuditOutcome,
    /// Caller-asserted. This crate reads no clock and nothing corroborates this string.
    pub recorded_at: Option<String>,
}

impl AuditEvent {
    pub fn new(
        action: AuditAction,
        actor_key_identity: KeyIdentity,
        subject: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        AuditEvent {
            action,
            actor_key_identity,
            subject: subject.into(),
            rationale: None,
            affected_digests: Vec::new(),
            outcome,
            recorded_at: None,
        }
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    pub fn affecting(mut self, digest: ContentHash) -> Self {
        self.affected_digests.push(digest);
        self
    }

    pub fn asserted_at(mut self, recorded_at: impl Into<String>) -> Self {
        self.recorded_at = Some(recorded_at.into());
        self
    }
}

/// An event with its position and its link into the chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedEntry {
    /// Assigned by the log, never by the caller, so a sequence cannot be chosen to fit a story.
    pub sequence: u64,
    pub previous: ContentHash,
    pub digest: ContentHash,
    pub event: AuditEvent,
}

/// An append-only hash-linked log.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    entries: Vec<LinkedEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        AuditLog::default()
    }

    /// The digest the first entry links to: SHA-256 of the empty byte string.
    ///
    /// A fixed, publicly known value, so a log's first link is not a place to hide a secret.
    pub fn genesis() -> ContentHash {
        ContentHash::of_bytes(&[])
    }

    /// The digest of the most recent entry, or [`AuditLog::genesis`] for an empty log.
    pub fn head(&self) -> ContentHash {
        self.entries
            .last()
            .map(|entry| entry.digest.clone())
            .unwrap_or_else(AuditLog::genesis)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[LinkedEntry] {
        &self.entries
    }

    /// Appends an event, linking it to the current head.
    ///
    /// There is no `insert`, no `remove` and no indexed mutation. Removing an entry from a Rust `Vec`
    /// held privately would require a method, and none exists — which is as close to append-only as a
    /// value in memory gets. It is not immutable storage, and 13.20 asks for that separately.
    pub fn append(&mut self, event: AuditEvent) -> Result<&LinkedEntry, BundleError> {
        let sequence = self.entries.len() as u64;
        let previous = self.head();
        let digest = link_digest(sequence, &previous, &event)?;
        self.entries.push(LinkedEntry {
            sequence,
            previous,
            digest,
            event,
        });
        self.entries
            .last()
            .ok_or_else(|| BundleError::SerializationFailed {
                context: "audit entry",
                detail: "entry disappeared immediately after append".to_string(),
            })
    }

    /// Recomputes every link. Never reads a recorded digest and calls it checked.
    pub fn verify_chain(&self) -> Result<ChainVerification, BundleError> {
        let mut expected_previous = AuditLog::genesis();
        for (index, entry) in self.entries.iter().enumerate() {
            let sequence = index as u64;
            if entry.sequence != sequence || entry.previous != expected_previous {
                return Ok(ChainVerification::BrokenAt {
                    sequence: entry.sequence,
                    recorded: entry.previous.clone(),
                    computed: expected_previous,
                });
            }
            let recomputed = link_digest(sequence, &entry.previous, &entry.event)?;
            if recomputed != entry.digest {
                return Ok(ChainVerification::BrokenAt {
                    sequence,
                    recorded: entry.digest.clone(),
                    computed: recomputed,
                });
            }
            expected_previous = entry.digest.clone();
        }
        Ok(ChainVerification::Intact {
            length: self.entries.len(),
            head: expected_previous,
        })
    }

    /// A checkpoint over the current head, per 13.20 §Integrity.
    ///
    /// Refuses to checkpoint a broken chain: a checkpoint over a log that does not verify would
    /// authenticate a contradiction.
    pub fn checkpoint(
        &self,
        key: &SecretKey,
        claimed_producer: ClaimedProducer,
    ) -> Result<AuditCheckpoint, BundleError> {
        let (length, head) = match self.verify_chain()? {
            ChainVerification::Intact { length, head } => (length, head),
            ChainVerification::BrokenAt {
                sequence,
                recorded,
                computed,
            } => {
                return Err(BundleError::AuditChainBroken {
                    sequence,
                    recorded: recorded.as_str().to_string(),
                    computed: computed.as_str().to_string(),
                });
            }
        };
        let subject = checkpoint_digest(&head, length)?;
        let attestation = Attestation::produce(
            AttestationPurpose::AuditCheckpoint,
            subject,
            key,
            claimed_producer,
        )?;
        Ok(AuditCheckpoint {
            head,
            length,
            attestation,
        })
    }
}

/// The outcome of recomputing a chain. Two states, and the broken one names where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "chain", rename_all = "snake_case")]
pub enum ChainVerification {
    Intact {
        length: usize,
        head: ContentHash,
    },
    BrokenAt {
        sequence: u64,
        recorded: ContentHash,
        computed: ContentHash,
    },
}

impl ChainVerification {
    pub fn is_intact(&self) -> bool {
        matches!(self, ChainVerification::Intact { .. })
    }
}

/// A tag over a chain head and length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditCheckpoint {
    pub head: ContentHash,
    pub length: usize,
    pub attestation: Attestation,
}

impl AuditCheckpoint {
    /// Checks that `log` still hashes to this checkpoint's head, then checks the tag.
    ///
    /// Recompute first, authenticate second — the same order as [`crate::AttestedBundle::verify`],
    /// for the same reason.
    pub fn verify(
        &self,
        log: &AuditLog,
        key: &SecretKey,
    ) -> Result<KeyHolderAuthenticated, BundleError> {
        match log.verify_chain()? {
            ChainVerification::Intact { length, head } => {
                if head != self.head || length != self.length {
                    return Err(BundleError::AuditChainBroken {
                        sequence: self.length as u64,
                        recorded: self.head.as_str().to_string(),
                        computed: head.as_str().to_string(),
                    });
                }
            }
            ChainVerification::BrokenAt {
                sequence,
                recorded,
                computed,
            } => {
                return Err(BundleError::AuditChainBroken {
                    sequence,
                    recorded: recorded.as_str().to_string(),
                    computed: computed.as_str().to_string(),
                })
            }
        }
        let subject = checkpoint_digest(&self.head, self.length)?;
        if self.attestation.subject_digest != subject {
            return Err(BundleError::AttestationCoversDifferentManifest {
                attested: self.attestation.subject_digest.as_str().to_string(),
                actual: subject.as_str().to_string(),
            });
        }
        self.attestation
            .verify_for_or_error(key, AttestationPurpose::AuditCheckpoint)
    }

    /// The sentence a transparency page must print next to a checkpoint.
    pub fn honest_label(&self) -> String {
        format!(
            "checkpoint over {} entries at head {}; tamper-evident against an editor without key `{}`, \
             and against nobody who holds it — there is no external witness, inclusion proof or timestamp",
            self.length, self.head, self.attestation.key_identity
        )
    }
}

fn link_digest(
    sequence: u64,
    previous: &ContentHash,
    event: &AuditEvent,
) -> Result<ContentHash, BundleError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(AUDIT_SCHEMA_VERSION));
    map.insert("sequence".into(), json!(sequence));
    map.insert("previous".into(), json!(previous.as_str()));
    map.insert(
        "event".into(),
        serde_json::to_value(event).map_err(|error| BundleError::SerializationFailed {
            context: "audit event",
            detail: error.to_string(),
        })?,
    );
    let bytes = bioprism_ids::to_canonical_bytes(&Value::Object(map))?;
    Ok(ContentHash::of_bytes(&bytes))
}

fn checkpoint_digest(head: &ContentHash, length: usize) -> Result<ContentHash, BundleError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(AUDIT_SCHEMA_VERSION));
    map.insert("head".into(), json!(head.as_str()));
    map.insert("length".into(), json!(length));
    let bytes = bioprism_ids::to_canonical_bytes(&Value::Object(map))?;
    Ok(ContentHash::of_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SecretKey {
        SecretKey::new(KeyIdentity::new("hub-2026"), vec![0x55; 32])
    }

    fn event(subject: &str) -> AuditEvent {
        AuditEvent::new(
            AuditAction::Publication,
            KeyIdentity::new("hub-2026"),
            subject,
            AuditOutcome::Allowed,
        )
    }

    fn log_of(count: usize) -> AuditLog {
        let mut log = AuditLog::new();
        for index in 0..count {
            log.append(event(&format!("bundle-{index}")))
                .expect("appends");
        }
        log
    }

    #[test]
    fn an_empty_log_heads_at_the_publicly_known_genesis_digest() {
        let log = AuditLog::new();
        assert!(log.is_empty());
        assert_eq!(log.head(), AuditLog::genesis());
        assert_eq!(
            AuditLog::genesis().as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256 of the empty byte string"
        );
    }

    #[test]
    fn each_entry_links_to_the_digest_of_the_one_before_it() {
        let log = log_of(3);
        assert_eq!(log.entries()[0].previous, AuditLog::genesis());
        assert_eq!(log.entries()[1].previous, log.entries()[0].digest);
        assert_eq!(log.entries()[2].previous, log.entries()[1].digest);
        assert_eq!(log.head(), log.entries()[2].digest);
    }

    #[test]
    fn the_log_assigns_sequence_numbers_so_a_caller_cannot_choose_one() {
        let log = log_of(3);
        assert_eq!(
            log.entries().iter().map(|e| e.sequence).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn an_intact_chain_verifies_and_reports_its_length_and_head() {
        let log = log_of(4);
        assert_eq!(
            log.verify_chain().expect("verifies"),
            ChainVerification::Intact {
                length: 4,
                head: log.head()
            }
        );
    }

    #[test]
    fn editing_an_early_event_breaks_the_chain_at_that_sequence() {
        let mut log = log_of(4);
        log.entries[1].event.subject = "bundle-tampered".into();
        let verification = log.verify_chain().expect("verifies");
        match verification {
            ChainVerification::BrokenAt { sequence, .. } => assert_eq!(sequence, 1),
            other => panic!("expected a break at sequence 1, got {other:?}"),
        }
        assert!(!verification.is_intact());
    }

    #[test]
    fn re_linking_an_edited_entry_still_breaks_the_chain_at_the_next_one() {
        let mut log = log_of(4);
        log.entries[1].event.subject = "bundle-tampered".into();
        log.entries[1].digest =
            link_digest(1, &log.entries[1].previous.clone(), &log.entries[1].event)
                .expect("hashes");
        match log.verify_chain().expect("verifies") {
            ChainVerification::BrokenAt { sequence, .. } => assert_eq!(
                sequence, 2,
                "repairing one link pushes the break downstream; repairing all of them is exactly \
                 what a key holder can do, and is why this is tamper evidence and not tamper proof"
            ),
            other => panic!("expected a break at sequence 2, got {other:?}"),
        }
    }

    #[test]
    fn a_checkpoint_verifies_against_the_log_it_was_taken_over() {
        let log = log_of(3);
        let checkpoint = log
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");
        let authenticated = checkpoint.verify(&log, &key()).expect("verifies");
        assert_eq!(authenticated.key_identity().as_str(), "hub-2026");
        assert_eq!(authenticated.purpose(), AttestationPurpose::AuditCheckpoint);
    }

    #[test]
    fn a_checkpoint_does_not_verify_against_a_log_that_grew_afterwards() {
        let log = log_of(3);
        let checkpoint = log
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");
        let mut extended = log.clone();
        extended.append(event("bundle-3")).expect("appends");
        assert!(matches!(
            checkpoint.verify(&extended, &key()),
            Err(BundleError::AuditChainBroken { .. })
        ));
    }

    #[test]
    fn a_broken_chain_cannot_be_checkpointed() {
        let mut log = log_of(3);
        log.entries[0].event.subject = "tampered".into();
        assert!(matches!(
            log.checkpoint(&key(), ClaimedProducer::new("hub")),
            Err(BundleError::AuditChainBroken { sequence: 0, .. })
        ));
    }

    #[test]
    fn a_key_holder_can_rewrite_the_log_and_issue_a_checkpoint_that_verifies() {
        let honest = log_of(3);
        let honest_checkpoint = honest
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");

        let mut rewritten = AuditLog::new();
        rewritten.append(event("bundle-0")).expect("appends");
        rewritten
            .append(event("bundle-rewritten"))
            .expect("appends");
        rewritten.append(event("bundle-2")).expect("appends");
        let forged = rewritten
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");

        assert!(forged.verify(&rewritten, &key()).is_ok());
        assert_ne!(forged.head, honest_checkpoint.head);
        assert!(honest_checkpoint
            .honest_label()
            .contains("against nobody who holds it"));
    }

    #[test]
    fn a_checkpoint_tag_cannot_be_replayed_as_a_publisher_attestation() {
        let log = log_of(1);
        let checkpoint = log
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");
        assert!(matches!(
            checkpoint
                .attestation
                .verify_for_or_error(&key(), AttestationPurpose::PublisherManifest),
            Err(BundleError::PurposeMismatch { .. })
        ));
    }

    #[test]
    fn an_event_has_nowhere_to_put_protected_content() {
        let recorded = event("specimen-cohort-4")
            .with_rationale("reviewer requested access")
            .affecting(ContentHash::of_bytes(b"protected payload"));
        let json = serde_json::to_value(&recorded).expect("serialises");
        let fields: Vec<&str> = json
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            vec![
                "action",
                "actor_key_identity",
                "subject",
                "rationale",
                "affected_digests",
                "outcome",
                "recorded_at"
            ],
            "13.20 §Content: sensitive payloads are referenced, not copied — there is no payload field"
        );
    }

    #[test]
    fn a_denied_action_is_distinguishable_from_a_failed_one() {
        let denied = AuditEvent::new(
            AuditAction::HiddenOracleAccess,
            KeyIdentity::new("reviewer-1"),
            "oracle-7",
            AuditOutcome::Denied {
                rationale: "holdout is sealed".into(),
            },
        );
        let failed = AuditEvent::new(
            AuditAction::HiddenOracleAccess,
            KeyIdentity::new("reviewer-1"),
            "oracle-7",
            AuditOutcome::Failed {
                detail: "store unreachable".into(),
            },
        );
        assert_ne!(denied.outcome, failed.outcome);
        let json = serde_json::to_string(&denied).expect("serialises");
        assert!(json.contains("\"denied\""), "{json}");
    }

    #[test]
    fn appending_the_same_events_twice_produces_the_same_head() {
        assert_eq!(log_of(5).head(), log_of(5).head());
    }

    #[test]
    fn a_checkpoint_whose_attestation_covers_a_different_head_is_refused() {
        let log = log_of(2);
        let mut checkpoint = log
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");
        checkpoint.attestation = Attestation::produce(
            AttestationPurpose::AuditCheckpoint,
            ContentHash::of_bytes(b"some other head"),
            &key(),
            ClaimedProducer::new("hub"),
        )
        .expect("attests");
        assert!(matches!(
            checkpoint.verify(&log, &key()),
            Err(BundleError::AttestationCoversDifferentManifest { .. })
        ));
    }

    #[test]
    fn a_checkpoint_over_an_empty_log_covers_the_genesis_head() {
        let log = AuditLog::new();
        let checkpoint = log
            .checkpoint(&key(), ClaimedProducer::new("hub"))
            .expect("checkpoints");
        assert_eq!(checkpoint.head, AuditLog::genesis());
        assert_eq!(checkpoint.length, 0);
        checkpoint.verify(&log, &key()).expect("verifies");
    }

    #[test]
    fn reordering_two_events_changes_the_head() {
        let mut forward = AuditLog::new();
        forward.append(event("a")).expect("appends");
        forward.append(event("b")).expect("appends");
        let mut backward = AuditLog::new();
        backward.append(event("b")).expect("appends");
        backward.append(event("a")).expect("appends");
        assert_ne!(forward.head(), backward.head());
    }
}
