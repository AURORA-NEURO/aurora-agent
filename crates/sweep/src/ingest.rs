//! The ingestion pipeline: two phases, one quarantine, one idempotency key.
//!
//! Implements blueprint 04.01 (Ingestion Pipeline). Three of its five design parts are structural
//! and are here. The fourth, loss accounting, is **already discharged** by `bioprism-adapter`'s
//! `SemanticLoss` — which separates `Lossless` from `Unaudited` for exactly the reason this crate
//! keeps separating things — and is not rebuilt. The fifth, the stage list, is recorded as
//! [`Stage`] because the pipeline's order is a claim about when redaction happens relative to
//! persistence, and that claim is worth being able to assert.
//!
//! # Two-phase import is a type, not a convention
//!
//! 04.01: "Phase one reads metadata and constructs a proposed import plan. Phase two materializes
//! potentially sensitive or executable artifacts only after policy approval."
//!
//! Written as a convention this is a code-review item that fails open. Written as types it is:
//! [`materialize`] takes an [`ApprovedPlan`], whose only constructor is [`approve`], which takes an
//! [`ImportPlan`]. There is no path from a source to materialised artifacts that does not pass
//! through an approval, because no other function returns an `ApprovedPlan`.
//!
//! The approval binds to the plan's digest, so a plan that changed after approval is refused at
//! materialisation rather than silently materialising the new one — the same time-of-check
//! discipline [`crate::effects`] applies to 13.11's effect previews.
//!
//! # Quarantine is a sink
//!
//! 04.01: "The platform never 'best-effort executes' an untrusted pack during import."
//!
//! [`propose`] returns `Result<ImportPlan, Quarantined>`, and [`Quarantined`] exposes its
//! diagnostics and nothing else. There is no `Quarantined::into_plan`, no `force`, no
//! `unwrap_or_default`. A malformed source cannot reach phase two by any route in this module.
//!
//! # Idempotency
//!
//! 04.01: "The same source artifact, adapter version, and policy should yield the same normalized
//! content digest. Re-importing does not create duplicate logical traces."
//!
//! [`normalization_key`] is that digest, over exactly those three inputs and nothing else — no
//! clock, no counter, no host. [`ImportLog`] uses it to make a second import of the same triple a
//! [`RecordOutcome::Duplicate`] rather than a second trace.
//!
//! # What is not implemented
//!
//! No parsers, no format detection, no causal inference. The stages are named; the two that carry
//! a structural guarantee (approval and quarantine) are enforced. Actually reading a trace format
//! is `bioprism-adapter`'s and `bioprism-trace`'s work.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{require_nonempty, SweepError};

/// 04.01's pipeline stages, in the blueprint's order.
///
/// The order is the claim: redaction precedes schema validation and both precede the write, so
/// nothing sensitive is persisted before policy has run over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Discover,
    IdentifyFormat,
    Parse,
    MapSourceIds,
    NormalizeEvents,
    ResolveArtifacts,
    InferCausalLinks,
    RedactOrTokenize,
    ValidateSchemas,
    WriteTraceAndProvenance,
}

impl Stage {
    pub const ORDER: [Stage; 10] = [
        Stage::Discover,
        Stage::IdentifyFormat,
        Stage::Parse,
        Stage::MapSourceIds,
        Stage::NormalizeEvents,
        Stage::ResolveArtifacts,
        Stage::InferCausalLinks,
        Stage::RedactOrTokenize,
        Stage::ValidateSchemas,
        Stage::WriteTraceAndProvenance,
    ];

    /// Whether this stage persists anything outside the importer.
    pub fn persists(self) -> bool {
        matches!(self, Stage::WriteTraceAndProvenance)
    }
}

/// A source as it arrives: an identifier and the digest of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: String,
    pub digest: ContentHash,
    /// What the discovery stage believes the format is. `None` when detection failed.
    pub format: Option<String>,
}

impl SourceRef {
    pub fn new(id: impl Into<String>, digest: ContentHash) -> Result<Self, SweepError> {
        let id = id.into();
        require_nonempty(&id, "SourceRef", "id")?;
        Ok(SourceRef { id, digest, format: None })
    }

    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }
}

/// A source that failed validation. It has no route onward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quarantined {
    source_id: String,
    diagnostics: Vec<String>,
}

impl Quarantined {
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// The parser diagnostics 04.01 requires a quarantined input to carry.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// A typed error for callers that need to propagate rather than branch.
    pub fn as_error(&self) -> SweepError {
        SweepError::Quarantined {
            source_id: self.source_id.clone(),
            diagnostic: self.diagnostics.join("; "),
        }
    }
}

/// The proposal phase one produces: metadata only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPlan {
    source: SourceRef,
    adapter_version: String,
    policy_digest: ContentHash,
    /// What phase two would materialise. Names and roles, not bytes.
    proposed_artifacts: BTreeMap<String, String>,
}

impl ImportPlan {
    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    pub fn adapter_version(&self) -> &str {
        &self.adapter_version
    }

    pub fn proposing(
        mut self,
        name: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        self.proposed_artifacts.insert(name.into(), role.into());
        self
    }

    pub fn proposed_artifacts(&self) -> &BTreeMap<String, String> {
        &self.proposed_artifacts
    }

    /// The digest an approval binds to. Covers every field, so any edit invalidates the approval.
    pub fn digest(&self) -> Result<ContentHash, SweepError> {
        let value = json!({
            "source_id": self.source.id,
            "source_digest": self.source.digest.as_str(),
            "format": self.source.format,
            "adapter_version": self.adapter_version,
            "policy_digest": self.policy_digest.as_str(),
            "proposed_artifacts": self.proposed_artifacts,
        });
        Ok(ContentHash::of_value(&value)?)
    }

    /// 04.01's idempotency key: source artifact, adapter version, policy. Nothing else.
    pub fn normalization_key(&self) -> Result<ContentHash, SweepError> {
        normalization_key(&self.source.digest, &self.adapter_version, &self.policy_digest)
    }
}

/// The same source, adapter version and policy always give the same key.
///
/// Free function as well as method because 04.01's claim is about the triple, not about a plan
/// object, and a caller checking the claim should be able to do so without constructing one.
pub fn normalization_key(
    source_digest: &ContentHash,
    adapter_version: &str,
    policy_digest: &ContentHash,
) -> Result<ContentHash, SweepError> {
    let value = json!({
        "source_digest": source_digest.as_str(),
        "adapter_version": adapter_version,
        "policy_digest": policy_digest.as_str(),
    });
    Ok(ContentHash::of_value(&value)?)
}

/// Phase one. Malformed sources leave through the quarantine, not through the plan.
///
/// The validity test here is deliberately thin — a source with no detected format is malformed —
/// because format detection belongs to an adapter. What matters structurally is the *shape* of the
/// return: the failure path yields a value that cannot become a plan.
pub fn propose(
    source: SourceRef,
    adapter_version: impl Into<String>,
    policy_digest: ContentHash,
) -> Result<ImportPlan, Quarantined> {
    if source.format.is_none() {
        return Err(Quarantined {
            source_id: source.id,
            diagnostics: vec!["format could not be identified".to_string()],
        });
    }
    Ok(ImportPlan {
        source,
        adapter_version: adapter_version.into(),
        policy_digest,
        proposed_artifacts: BTreeMap::new(),
    })
}

/// Send a source to quarantine with diagnostics.
pub fn quarantine(
    source: SourceRef,
    diagnostics: impl IntoIterator<Item = String>,
) -> Quarantined {
    let diagnostics: Vec<String> = diagnostics.into_iter().collect();
    Quarantined {
        source_id: source.id,
        diagnostics: if diagnostics.is_empty() {
            vec!["no diagnostic recorded".to_string()]
        } else {
            diagnostics
        },
    }
}

/// A plan that policy has approved. The seal on phase two.
///
/// Fields are private and there is no public constructor; [`approve`] is the only way to obtain
/// one. `Deserialize` is deliberately not derived — a deserialisable approval would let a caller
/// mint one from a JSON literal, which is the whole guarantee gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovedPlan {
    plan: ImportPlan,
    plan_digest: ContentHash,
    approver: String,
}

impl ApprovedPlan {
    pub fn plan(&self) -> &ImportPlan {
        &self.plan
    }

    pub fn approver(&self) -> &str {
        &self.approver
    }

    pub fn plan_digest(&self) -> &ContentHash {
        &self.plan_digest
    }
}

/// Approve a plan for materialisation.
pub fn approve(plan: ImportPlan, approver: impl Into<String>) -> Result<ApprovedPlan, SweepError> {
    let approver = approver.into();
    require_nonempty(&approver, "approve", "approver")?;
    let plan_digest = plan.digest()?;
    Ok(ApprovedPlan { plan, plan_digest, approver })
}

/// What phase two produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Materialization {
    pub source_id: String,
    pub normalization_key: ContentHash,
    pub artifacts: Vec<String>,
    pub approver: String,
}

/// Phase two. Requires an approval whose digest still matches the plan.
///
/// The `current` argument is the plan as it stands now. Passing the approved plan's own copy is the
/// common case; passing a re-read plan is the point — an import whose source or proposed artifact
/// set changed between approval and materialisation is refused.
pub fn materialize(
    approved: &ApprovedPlan,
    current: &ImportPlan,
) -> Result<Materialization, SweepError> {
    let current_digest = current.digest()?;
    if current_digest != approved.plan_digest {
        return Err(SweepError::ApprovalStale {
            approved: approved.plan_digest.as_str().to_string(),
            current: current_digest.as_str().to_string(),
        });
    }
    Ok(Materialization {
        source_id: current.source.id.clone(),
        normalization_key: current.normalization_key()?,
        artifacts: current.proposed_artifacts.keys().cloned().collect(),
        approver: approved.approver.clone(),
    })
}

/// What recording an import did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RecordOutcome {
    /// A new logical trace.
    Recorded,
    /// The same source, adapter version and policy as an earlier import. No second trace.
    Duplicate { of: String },
}

/// The set of logical traces an importer has produced, keyed by normalisation key.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportLog {
    traces: BTreeMap<String, String>,
}

impl ImportLog {
    pub fn new() -> Self {
        ImportLog::default()
    }

    pub fn record(&mut self, materialization: &Materialization) -> RecordOutcome {
        let key = materialization.normalization_key.as_str().to_string();
        match self.traces.get(&key) {
            Some(existing) => RecordOutcome::Duplicate { of: existing.clone() },
            None => {
                self.traces.insert(key, materialization.source_id.clone());
                RecordOutcome::Recorded
            }
        }
    }

    pub fn trace_count(&self) -> usize {
        self.traces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ContentHash {
        ContentHash::of_bytes(b"policy-v1")
    }

    fn source() -> SourceRef {
        SourceRef::new("run-log", ContentHash::of_bytes(b"bytes")).unwrap().with_format("jsonl")
    }

    fn plan() -> ImportPlan {
        propose(source(), "0.3.0", policy()).unwrap().proposing("stdout.txt", "artifact")
    }

    #[test]
    fn redaction_precedes_persistence_in_the_stage_order() {
        let redact = Stage::ORDER.iter().position(|s| *s == Stage::RedactOrTokenize).unwrap();
        let write = Stage::ORDER.iter().position(|s| s.persists()).unwrap();
        assert!(redact < write);
        assert_eq!(Stage::ORDER.len(), 10);
    }

    #[test]
    fn a_source_whose_format_is_unknown_goes_to_quarantine_rather_than_to_a_plan() {
        let unknown = SourceRef::new("mystery", ContentHash::of_bytes(b"?")).unwrap();
        let quarantined = propose(unknown, "0.3.0", policy()).unwrap_err();
        assert_eq!(quarantined.source_id(), "mystery");
        assert!(!quarantined.diagnostics().is_empty());
        assert!(matches!(quarantined.as_error(), SweepError::Quarantined { .. }));
    }

    #[test]
    fn quarantine_always_carries_at_least_one_diagnostic() {
        let q = quarantine(source(), Vec::new());
        assert_eq!(q.diagnostics(), ["no diagnostic recorded"]);
    }

    #[test]
    fn materialization_requires_an_approval() {
        let approved = approve(plan(), "reviewer-a").unwrap();
        let materialized = materialize(&approved, approved.plan()).unwrap();
        assert_eq!(materialized.approver, "reviewer-a");
        assert_eq!(materialized.artifacts, ["stdout.txt"]);
    }

    #[test]
    fn an_approval_with_no_approver_is_refused() {
        assert!(approve(plan(), "   ").is_err());
    }

    #[test]
    fn a_plan_that_changed_after_approval_cannot_be_materialized() {
        let approved = approve(plan(), "reviewer-a").unwrap();
        let edited = plan().proposing("secrets.env", "artifact");
        let err = materialize(&approved, &edited).unwrap_err();
        assert!(matches!(err, SweepError::ApprovalStale { .. }));
    }

    #[test]
    fn changing_the_source_digest_also_invalidates_the_approval() {
        let approved = approve(plan(), "reviewer-a").unwrap();
        let other_source =
            SourceRef::new("run-log", ContentHash::of_bytes(b"other")).unwrap().with_format("jsonl");
        let edited = propose(other_source, "0.3.0", policy()).unwrap().proposing("stdout.txt", "artifact");
        assert!(materialize(&approved, &edited).is_err());
    }

    #[test]
    fn the_normalization_key_depends_on_the_source_the_adapter_and_the_policy_and_nothing_else() {
        let base = normalization_key(&ContentHash::of_bytes(b"s"), "1.0.0", &policy()).unwrap();
        assert_eq!(
            base,
            normalization_key(&ContentHash::of_bytes(b"s"), "1.0.0", &policy()).unwrap()
        );
        assert_ne!(
            base,
            normalization_key(&ContentHash::of_bytes(b"s"), "1.0.1", &policy()).unwrap()
        );
        assert_ne!(
            base,
            normalization_key(
                &ContentHash::of_bytes(b"s"),
                "1.0.0",
                &ContentHash::of_bytes(b"policy-v2")
            )
            .unwrap()
        );
        assert_ne!(
            base,
            normalization_key(&ContentHash::of_bytes(b"t"), "1.0.0", &policy()).unwrap()
        );
    }

    #[test]
    fn re_importing_the_same_triple_does_not_create_a_second_logical_trace() {
        let approved = approve(plan(), "reviewer-a").unwrap();
        let first = materialize(&approved, approved.plan()).unwrap();
        let mut log = ImportLog::new();
        assert_eq!(log.record(&first), RecordOutcome::Recorded);
        let second = materialize(&approved, approved.plan()).unwrap();
        assert!(matches!(log.record(&second), RecordOutcome::Duplicate { .. }));
        assert_eq!(log.trace_count(), 1);
    }

    #[test]
    fn a_new_adapter_version_produces_a_new_logical_trace() {
        let mut log = ImportLog::new();
        let a = approve(plan(), "r").unwrap();
        log.record(&materialize(&a, a.plan()).unwrap());
        let newer = propose(source(), "0.4.0", policy()).unwrap().proposing("stdout.txt", "artifact");
        let b = approve(newer, "r").unwrap();
        assert_eq!(log.record(&materialize(&b, b.plan()).unwrap()), RecordOutcome::Recorded);
        assert_eq!(log.trace_count(), 2);
    }
}
