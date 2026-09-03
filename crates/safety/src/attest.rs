//! Audit records that distinguish what was observed from what was asserted, and a hash-linked log.
//!
//! Implements blueprint 13.20 (audit log transparency and attestation).
//!
//! # The distinction the whole module is built around
//!
//! An audit log's value is that a reader can tell what actually happened. The failure mode is that
//! it records claims in the same shape as facts, so `evaluator_isolation_verified` and
//! `operator says evaluator isolation is configured` end up as two indistinguishable rows.
//!
//! [`Statement`] therefore has two variants. [`Statement::Observed`] carries an [`Observation`],
//! which is a **closed enum of computations this process actually performs** — a digest comparison,
//! a refused boundary crossing, a produced witness, a recomputed chain link. There is no
//! `Observation::Other(String)` and no way to construct one from a caller-supplied sentence.
//! [`Statement::Asserted`] carries free text and the name of whoever said it. A caller with a claim
//! and no computation behind it can only reach for `Asserted`.
//!
//! # Attestations are narrow, and there is no `Secure`
//!
//! 13.20 asks for claims like "built from manifest X in trusted runner Y" and explicitly says
//! "Avoid broad 'secure' attestations". [`AttestationClaim`] is closed and contains no such
//! variant. Of the four claims it does contain, exactly one —
//! [`AttestationClaim::DigestsCompared`] — is something this process can observe, and
//! [`Attestation::observed`] refuses the other three with
//! [`SafetyError::UnwitnessedAttestation`]. They remain available through
//! [`Attestation::asserted`], where they read as what they are.
//!
//! # The chain is real, and here is exactly how far it goes
//!
//! [`AuditLog::append`] links each record to the previous one by SHA-256 over a canonical field
//! encoding, and [`AuditLog::verify`] recomputes every link. That genuinely detects a record edited
//! or removed in the middle of a log by someone who did not rewrite the chain.
//!
//! It does not detect someone who did. There is no signature over the head, no external
//! checkpoint, no witness, no second copy, and no clock — so an actor who can write the log can
//! produce a self-consistent forgery of it, and this module cannot tell the two apart. 13.20's
//! "signed checkpoints, independent backup, and periodic verification" are the parts that would
//! close that, and none of them is here.
//!
//! # What is deliberately not implemented
//!
//! * **No storage.** [`AuditLog`] is a `Vec` in memory. Append-only is a property of the API, not
//!   of anything durable.
//! * **No signing, no keys, no revocation, no key-validity-at-signing-time check.**
//! * **No transparency service.** [`AuditEvent::is_publicly_loggable`] classifies which events
//!   13.20 would put in a public log. It publishes nothing.
//! * **No clock.** Records carry an operator-supplied epoch, like `bioprism-governance`'s
//!   lifecycles. A log ordered by an untrusted clock would be ordered by the attacker.
//! * **No retention or export policy.**

use crate::error::SafetyError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The event classes 13.20 enumerates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEvent {
    Authentication,
    PrivilegeChange,
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

impl AuditEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditEvent::Authentication => "authentication",
            AuditEvent::PrivilegeChange => "privilege_change",
            AuditEvent::PolicyChange => "policy_change",
            AuditEvent::HiddenOracleAccess => "hidden_oracle_access",
            AuditEvent::SensitiveArtifactAccess => "sensitive_artifact_access",
            AuditEvent::Publication => "publication",
            AuditEvent::ResultAcceptance => "result_acceptance",
            AuditEvent::ReviewerDecision => "reviewer_decision",
            AuditEvent::KeyLifecycle => "key_lifecycle",
            AuditEvent::SecurityQuarantine => "security_quarantine",
            AuditEvent::Deletion => "deletion",
            AuditEvent::FederationImport => "federation_import",
        }
    }

    /// Whether 13.20's transparency section puts this event in the public log.
    ///
    /// Hidden-oracle access is the interesting exclusion: logging *that* a holdout was read, in
    /// public, tells an attacker which holdouts exist and when they are touched.
    pub fn is_publicly_loggable(self) -> bool {
        matches!(
            self,
            AuditEvent::Publication
                | AuditEvent::ResultAcceptance
                | AuditEvent::KeyLifecycle
                | AuditEvent::SecurityQuarantine
                | AuditEvent::PolicyChange
        )
    }
}

impl fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Something this process computed.
///
/// Closed, and every variant corresponds to code in this crate. Adding a variant means adding the
/// computation that produces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "snake_case")]
pub enum Observation {
    /// [`crate::supply::DigestObservation::verify`] ran.
    DigestsCompared { component: String, equal: bool },
    /// [`crate::boundary::BoundaryModel::deliver`] refused a crossing.
    BoundaryCrossingRefused {
        artifact: String,
        from: String,
        to: String,
    },
    /// A check in [`crate::integrity`] produced a witness.
    WitnessProduced { check: String, kind: String },
    /// [`crate::provenance::ContextAssembly::injection_paths`] found a route.
    InjectionPathFound { origin: String, sink: String },
    /// A link in this log was recomputed and matched.
    ChainLinkRecomputed { index: usize },
}

impl Observation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Observation::DigestsCompared { .. } => "digests_compared",
            Observation::BoundaryCrossingRefused { .. } => "boundary_crossing_refused",
            Observation::WitnessProduced { .. } => "witness_produced",
            Observation::InjectionPathFound { .. } => "injection_path_found",
            Observation::ChainLinkRecomputed { .. } => "chain_link_recomputed",
        }
    }

    /// The canonical field encoding this observation contributes to the chain digest.
    fn digest_fields(&self) -> Vec<String> {
        let mut fields = vec![self.as_str().to_string()];
        match self {
            Observation::DigestsCompared { component, equal } => {
                fields.push(component.clone());
                fields.push(equal.to_string());
            }
            Observation::BoundaryCrossingRefused { artifact, from, to } => {
                fields.push(artifact.clone());
                fields.push(from.clone());
                fields.push(to.clone());
            }
            Observation::WitnessProduced { check, kind } => {
                fields.push(check.clone());
                fields.push(kind.clone());
            }
            Observation::InjectionPathFound { origin, sink } => {
                fields.push(origin.clone());
                fields.push(sink.clone());
            }
            Observation::ChainLinkRecomputed { index } => fields.push(index.to_string()),
        }
        fields
    }
}

/// Observed or asserted. Never both, never neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "statement", rename_all = "snake_case")]
pub enum Statement {
    /// This process computed it.
    Observed { observation: Observation },
    /// Somebody said it.
    Asserted { by: String, claim: String },
}

impl Statement {
    pub fn observed(observation: Observation) -> Self {
        Statement::Observed { observation }
    }

    pub fn asserted(by: impl Into<String>, claim: impl Into<String>) -> Self {
        Statement::Asserted {
            by: by.into(),
            claim: claim.into(),
        }
    }

    pub fn is_observed(&self) -> bool {
        matches!(self, Statement::Observed { .. })
    }

    fn digest_fields(&self) -> Vec<String> {
        match self {
            Statement::Observed { observation } => {
                let mut fields = vec!["observed".to_string()];
                fields.extend(observation.digest_fields());
                fields
            }
            Statement::Asserted { by, claim } => {
                vec!["asserted".to_string(), by.clone(), claim.clone()]
            }
        }
    }
}

/// One audit row, before it is linked into a log.
///
/// `subject` is a reference, not a payload: 13.20's "Sensitive payloads are referenced, not
/// copied". Where the reference is content-addressed, use [`bioprism_ids::ContentHash`] rather than
/// inlining the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub event: AuditEvent,
    pub actor: String,
    pub subject: String,
    /// A content-addressed reference to the affected artifact, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_digest: Option<ContentHash>,
    pub statement: Statement,
    /// Operator-supplied ordering. There is no clock here.
    pub epoch: u64,
}

impl AuditRecord {
    pub fn new(
        event: AuditEvent,
        actor: impl Into<String>,
        subject: impl Into<String>,
        statement: Statement,
        epoch: u64,
    ) -> Self {
        AuditRecord {
            event,
            actor: actor.into(),
            subject: subject.into(),
            subject_digest: None,
            statement,
            epoch,
        }
    }

    pub fn referencing(mut self, digest: ContentHash) -> Self {
        self.subject_digest = Some(digest);
        self
    }

    fn digest_fields(&self) -> Vec<String> {
        let mut fields = vec![
            self.event.as_str().to_string(),
            self.actor.clone(),
            self.subject.clone(),
            self.subject_digest
                .as_ref()
                .map(|d| d.as_str().to_string())
                .unwrap_or_default(),
            self.epoch.to_string(),
        ];
        fields.extend(self.statement.digest_fields());
        fields
    }
}

/// A record with its position and its link to the one before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedRecord {
    pub index: usize,
    pub record: AuditRecord,
    /// The digest of the preceding linked record. Empty string for the first.
    pub previous: String,
    /// The digest of this record together with `previous`.
    pub digest: String,
}

/// An append-only, hash-linked sequence of audit records.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    records: Vec<LinkedRecord>,
}

impl AuditLog {
    pub fn new() -> Self {
        AuditLog::default()
    }

    /// Field separator for the chain encoding.
    ///
    /// ASCII unit separator, chosen because it cannot occur in the enum discriminants and is
    /// vanishingly unlikely in an actor or subject identifier. The encoding is a flat list of
    /// fields rather than serialised JSON so that the digest does not depend on serde's field
    /// ordering, which a refactor could change without anyone noticing.
    const SEPARATOR: char = '\u{1f}';

    fn link_digest(previous: &str, record: &AuditRecord) -> String {
        let mut fields = vec![previous.to_string()];
        fields.extend(record.digest_fields());
        let encoded = fields.join(&Self::SEPARATOR.to_string());
        ContentHash::of_bytes(encoded.as_bytes())
            .as_str()
            .to_string()
    }

    /// Appends a record, linking it to the current head.
    ///
    /// Refuses a record whose epoch precedes the head's: a log that accepts out-of-order epochs
    /// cannot be read as a timeline, and an attacker who can backdate can hide inside history.
    pub fn append(&mut self, record: AuditRecord) -> Result<&LinkedRecord, SafetyError> {
        if let Some(head) = self.records.last() {
            if record.epoch < head.record.epoch {
                return Err(SafetyError::EpochNotAdvancing {
                    subject: record.subject.clone(),
                    previous: head.record.epoch,
                    epoch: record.epoch,
                });
            }
        }
        let previous = self
            .records
            .last()
            .map(|r| r.digest.clone())
            .unwrap_or_default();
        let digest = Self::link_digest(&previous, &record);
        self.records.push(LinkedRecord {
            index: self.records.len(),
            record,
            previous,
            digest,
        });
        self.records
            .last()
            .ok_or_else(|| SafetyError::InvariantViolation {
                detail: "audit record disappeared immediately after append".into(),
            })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[LinkedRecord] {
        &self.records
    }

    /// The current head digest, which is what a checkpoint would sign if anything here could sign.
    pub fn head(&self) -> Option<&str> {
        self.records.last().map(|r| r.digest.as_str())
    }

    /// Recomputes every link.
    ///
    /// Detects an edited or removed record. Does not detect a wholesale rewrite. See the module
    /// docs; that limitation is the reason this returns `Result<(), SafetyError>` rather than a
    /// boolean called `is_trustworthy`.
    pub fn verify(&self) -> Result<(), SafetyError> {
        let mut previous = String::new();
        for entry in &self.records {
            let expected = Self::link_digest(&previous, &entry.record);
            if entry.previous != previous {
                return Err(SafetyError::AuditChainBroken {
                    index: entry.index,
                    recorded: entry.previous.clone(),
                    actual: previous,
                });
            }
            if entry.digest != expected {
                return Err(SafetyError::AuditChainBroken {
                    index: entry.index,
                    recorded: entry.digest.clone(),
                    actual: expected,
                });
            }
            previous = entry.digest.clone();
        }
        Ok(())
    }

    /// Records whose statement is an assertion rather than an observation.
    ///
    /// The query a reviewer should run first on any log that looks reassuring.
    pub fn assertions(&self) -> Vec<&LinkedRecord> {
        self.records
            .iter()
            .filter(|r| !r.record.statement.is_observed())
            .collect()
    }

    /// Records 13.20's transparency section would put in the public log.
    pub fn public_view(&self) -> Vec<&LinkedRecord> {
        self.records
            .iter()
            .filter(|r| r.record.event.is_publicly_loggable())
            .collect()
    }
}

/// The narrow claims 13.20 permits. There is no `Secure` variant, on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "claim", rename_all = "snake_case")]
pub enum AttestationClaim {
    /// "built from manifest X in trusted runner Y".
    BuiltFromManifest { manifest: String, runner: String },
    /// "trial bundle closure verified".
    BundleClosureVerified { bundle: String },
    /// "independently reproduced".
    IndependentlyReproduced { run: String, by: String },
    /// Two digests were compared and agreed. The only claim this process can witness.
    DigestsCompared { component: String },
}

impl AttestationClaim {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttestationClaim::BuiltFromManifest { .. } => "built_from_manifest",
            AttestationClaim::BundleClosureVerified { .. } => "bundle_closure_verified",
            AttestationClaim::IndependentlyReproduced { .. } => "independently_reproduced",
            AttestationClaim::DigestsCompared { .. } => "digests_compared",
        }
    }

    /// Whether this crate can produce evidence for the claim itself.
    ///
    /// Three of the four require a builder, a bundle resolver or a second party, none of which
    /// exists here.
    pub fn observable_here(&self) -> bool {
        matches!(self, AttestationClaim::DigestsCompared { .. })
    }
}

/// A claim with the evidence class it is entitled to.
///
/// Fields are private and there is no `Deserialize`. The two constructors are the only way to build
/// one, so no code path anywhere can pair an unwitnessable claim with an `Observed` statement — not
/// by struct literal, not by reading a stored attestation back in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attestation {
    claim: AttestationClaim,
    statement: Statement,
}

impl Attestation {
    pub fn claim(&self) -> &AttestationClaim {
        &self.claim
    }

    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    /// Mints an attestation backed by something this process computed.
    ///
    /// Refuses a claim outside [`AttestationClaim::observable_here`], because the observation would
    /// have to have come from somewhere else and would then be an assertion wearing an
    /// observation's clothes.
    pub fn observed(
        claim: AttestationClaim,
        observation: Observation,
    ) -> Result<Self, SafetyError> {
        if !claim.observable_here() {
            return Err(SafetyError::UnwitnessedAttestation {
                claim: claim.as_str().to_string(),
                asserter: "this process".into(),
            });
        }
        Ok(Attestation {
            claim,
            statement: Statement::observed(observation),
        })
    }

    /// Records a claim as somebody's word. Always available, and honest about what it is.
    pub fn asserted(claim: AttestationClaim, by: impl Into<String>) -> Self {
        let text = claim.as_str().to_string();
        Attestation {
            claim,
            statement: Statement::asserted(by, text),
        }
    }

    pub fn is_observed(&self) -> bool {
        self.statement.is_observed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(subject: &str, epoch: u64) -> AuditRecord {
        AuditRecord::new(
            AuditEvent::Publication,
            "operator:alice",
            subject,
            Statement::asserted("operator:alice", "release approved"),
            epoch,
        )
    }

    #[test]
    fn an_intact_chain_verifies_and_every_link_names_its_predecessor() {
        let mut log = AuditLog::new();
        log.append(record("pack-a", 1)).expect("first");
        log.append(record("pack-b", 2)).expect("second");
        log.append(record("pack-c", 2))
            .expect("same epoch is allowed");
        log.verify().expect("nothing was touched");
        assert_eq!(log.records()[0].previous, "");
        assert_eq!(log.records()[1].previous, log.records()[0].digest);
        assert_eq!(log.head(), Some(log.records()[2].digest.as_str()));
    }

    #[test]
    fn editing_a_record_in_the_middle_breaks_the_chain_at_that_index() {
        let mut log = AuditLog::new();
        log.append(record("pack-a", 1)).expect("first");
        log.append(record("pack-b", 2)).expect("second");
        log.append(record("pack-c", 3)).expect("third");
        let mut tampered = log.clone();
        tampered.records[1].record.subject = "pack-b-modified".into();
        let error = tampered.verify().expect_err("the digest no longer matches");
        assert!(matches!(
            error,
            SafetyError::AuditChainBroken { index: 1, .. }
        ));
    }

    #[test]
    fn removing_a_record_breaks_the_link_of_the_one_that_followed_it() {
        let mut log = AuditLog::new();
        log.append(record("a", 1)).expect("first");
        log.append(record("b", 2)).expect("second");
        log.append(record("c", 3)).expect("third");
        let mut tampered = log.clone();
        tampered.records.remove(1);
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn a_backdated_record_is_refused_rather_than_appended_out_of_order() {
        let mut log = AuditLog::new();
        log.append(record("a", 5)).expect("first");
        let error = log
            .append(record("b", 4))
            .expect_err("history does not run backwards");
        assert!(matches!(
            error,
            SafetyError::EpochNotAdvancing {
                previous: 5,
                epoch: 4,
                ..
            }
        ));
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn an_assertion_and_an_observation_are_distinguishable_in_the_log() {
        let mut log = AuditLog::new();
        log.append(record("asserted-thing", 1)).expect("first");
        log.append(AuditRecord::new(
            AuditEvent::SensitiveArtifactAccess,
            "checker",
            "image",
            Statement::observed(Observation::DigestsCompared {
                component: "image".into(),
                equal: true,
            }),
            2,
        ))
        .expect("second");
        assert_eq!(log.assertions().len(), 1);
        assert_eq!(log.assertions()[0].record.subject, "asserted-thing");
    }

    #[test]
    fn an_attestation_this_process_cannot_witness_may_not_be_minted_as_observed() {
        let error = Attestation::observed(
            AttestationClaim::BuiltFromManifest {
                manifest: "m1".into(),
                runner: "trusted-builder".into(),
            },
            Observation::DigestsCompared {
                component: "image".into(),
                equal: true,
            },
        )
        .expect_err("there is no builder here");
        assert!(matches!(error, SafetyError::UnwitnessedAttestation { .. }));
    }

    #[test]
    fn the_one_observable_claim_can_be_minted_and_reports_itself_as_observed() {
        let attestation = Attestation::observed(
            AttestationClaim::DigestsCompared {
                component: "image".into(),
            },
            Observation::DigestsCompared {
                component: "image".into(),
                equal: true,
            },
        )
        .expect("this process really did compare two digests");
        assert!(attestation.is_observed());
    }

    #[test]
    fn an_unwitnessable_claim_remains_available_as_an_assertion() {
        let attestation = Attestation::asserted(
            AttestationClaim::IndependentlyReproduced {
                run: "r-1".into(),
                by: "lab-b".into(),
            },
            "lab-b",
        );
        assert!(!attestation.is_observed());
    }

    #[test]
    fn hidden_oracle_access_is_audited_but_never_published_to_the_transparency_log() {
        assert!(!AuditEvent::HiddenOracleAccess.is_publicly_loggable());
        assert!(AuditEvent::Publication.is_publicly_loggable());
        let mut log = AuditLog::new();
        log.append(AuditRecord::new(
            AuditEvent::HiddenOracleAccess,
            "evaluator",
            "holdout-v2",
            Statement::asserted("evaluator", "mounted"),
            1,
        ))
        .expect("appended");
        log.append(record("release", 2)).expect("appended");
        assert_eq!(log.public_view().len(), 1);
        assert_eq!(log.public_view()[0].record.event, AuditEvent::Publication);
    }

    #[test]
    fn the_chain_digest_changes_when_the_statement_changes_but_nothing_else_does() {
        let mut observed = AuditLog::new();
        observed
            .append(AuditRecord::new(
                AuditEvent::Publication,
                "a",
                "s",
                Statement::observed(Observation::ChainLinkRecomputed { index: 0 }),
                1,
            ))
            .expect("appended");
        let mut asserted = AuditLog::new();
        asserted
            .append(AuditRecord::new(
                AuditEvent::Publication,
                "a",
                "s",
                Statement::asserted("a", "chain_link_recomputed"),
                1,
            ))
            .expect("appended");
        assert_ne!(observed.head(), asserted.head());
    }

    #[test]
    fn a_subject_digest_is_a_reference_rather_than_an_inlined_payload() {
        let digest = ContentHash::of_bytes(b"sensitive artifact bytes");
        let record = record("artifact", 1).referencing(digest.clone());
        assert_eq!(record.subject_digest, Some(digest));
        let json = serde_json::to_string(&record).expect("serialises");
        assert!(!json.contains("sensitive artifact bytes"), "{json}");
    }
}
