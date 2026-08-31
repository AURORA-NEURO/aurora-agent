//! High-throughput retrieval assurance harness.
//!
//! Atlas feature: `AFA-brain-P02-F27`. Queue and checkpoint witnesses are release predicates;
//! overflow, unknown evidence, and replay drift remain explicit counterexamples.

use crate::retrieval_synthesis::SynthesisDisposition;
use crate::throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, ThroughputRetrievalQuery,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F27";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-assurance-harness/1.0";
const ASSURANCE_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-retrieval-assurance+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputRetrievalAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub checkpoint_seq: u64,
    pub verdict: ThroughputRetrievalAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub verification_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputRetrievalAssuranceError {
    #[error("invalid throughput retrieval assurance request: {0}")]
    Invalid(String),
    #[error("throughput retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval assurance synthesis failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalAssuranceError::Invalid("throughput assurance identity, queue, checkpoint, witnesses, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.batch_id, "batch_id"),
            (&self.partition, "partition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        for (values, field) in [
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        for (values, field) in [
            (&self.witness_order, "witness_order"),
            (&self.counterexample_order, "counterexample_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "throughput assurance candidate states must partition candidates".into(),
            ));
        }
        let expected_verdict = if !self.counterexample_order.is_empty() {
            ThroughputRetrievalAssuranceVerdict::Blocked
        } else if qualified_values == candidate_values
            && blocked_values.is_empty()
            && unknown_values.is_empty()
        {
            ThroughputRetrievalAssuranceVerdict::Qualified
        } else {
            ThroughputRetrievalAssuranceVerdict::Unresolved
        };
        if self.verdict != expected_verdict {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "throughput assurance verdict does not match witnesses and candidate state".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalAssuranceError::Invalid(
                    "throughput assurance digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts =
            if self.verdict == ThroughputRetrievalAssuranceVerdict::Qualified {
                vec![format!(
                    "assurance:local-throughput-retrieval:{}",
                    self.request_id
                )]
            } else {
                vec!["block:unsafe-release".into()]
            };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "throughput assurance effects do not match verdict".into(),
            ));
        }
        if !self.raw_data_local
            && (self.verdict != ThroughputRetrievalAssuranceVerdict::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "assurance:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "non-local throughput assurance must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_verification_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "batch_id": self.batch_id,
            "partition": self.partition,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "witness_order": self.witness_order,
            "counterexample_order": self.counterexample_order,
            "verdict": self.verdict,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))?;
        if self.verification_digest != expected_verification_digest {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "throughput assurance digest is not bound to verification state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-assurance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ASSURANCE_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalAssuranceError::Invalid(
                "throughput assurance artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "throughput release gate".into()].into(), behavior: "verifies high-throughput retrieval with queue, checkpoint, overflow, provenance, replay, and fail-closed witnesses".into(), value: "prevents capacity pressure or replay drift from becoming silent retrieval loss".into(), inputs: vec![TypedPort { name: "throughput_retrieval_query".into(), schema: "ScopedRetrievalQuery3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_retrieval_assurance".into(), schema: "ThroughputRetrievalAssuranceReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:retrieval-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_throughput_retrieval_assurance(
    request: &ThroughputRetrievalQuery,
) -> Result<ThroughputRetrievalAssuranceReceipt, ThroughputRetrievalAssuranceError> {
    let synthesis = synthesize_throughput_retrieval(request)
        .map_err(|error| ThroughputRetrievalAssuranceError::Engine(error.to_string()))?;
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:queue-admission".to_string(),
        "gate:checkpoint".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    if synthesis.checkpoint_seq == 0 {
        counterexamples.insert("counterexample:checkpoint-missing".into());
    }
    if synthesis.disposition != SynthesisDisposition::Qualified {
        witnesses.insert("gate:non-qualified-throughput-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !counterexamples.is_empty()
        || synthesis.disposition == SynthesisDisposition::Blocked
    {
        ThroughputRetrievalAssuranceVerdict::Blocked
    } else if synthesis.disposition == SynthesisDisposition::Qualified {
        ThroughputRetrievalAssuranceVerdict::Qualified
    } else {
        ThroughputRetrievalAssuranceVerdict::Unresolved
    };
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict, "replay_identity": request.replay_identity, "raw_data_local": true})).map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if verdict == ThroughputRetrievalAssuranceVerdict::Qualified {
        vec![format!(
            "assurance:local-throughput-retrieval:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "checkpoint_seq": synthesis.checkpoint_seq, "verdict": verdict, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis.synthesis_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-retrieval-assurance:{}",
            request.request_id
        ),
        ASSURANCE_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalAssuranceError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        checkpoint_seq: synthesis.checkpoint_seq,
        verdict,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        queue_digest: synthesis.queue_digest,
        synthesis_digest: synthesis.synthesis_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalAssuranceError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalAssuranceError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalAssuranceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalAssuranceError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalAssuranceError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalAssuranceError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalAssuranceReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "checkpoint_seq": receipt.checkpoint_seq,
        "verdict": receipt.verdict,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "witness_order": receipt.witness_order,
        "counterexample_order": receipt.counterexample_order,
        "queue_digest": receipt.queue_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "verification_digest": receipt.verification_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> ThroughputRetrievalQuery {
        ThroughputRetrievalQuery {
            request_id: "request:throughput-assurance".into(),
            batch_id: "batch:assurance".into(),
            partition: "partition:one".into(),
            max_items: 8,
            minimum_support_milli: 700,
            candidates: vec![RetrievalCandidate {
                evidence_id: "evidence:throughput".into(),
                source_id: "source:throughput".into(),
                study_id: "study:batch".into(),
                scope: "organoid:neural".into(),
                modality: "imaging".into(),
                support_milli: 900,
                state,
                semantic_digest: hash("semantic"),
                artifact_digest: hash("artifact"),
                provenance_digest: hash("provenance"),
                replay_identity: hash("replay"),
                omissions: Vec::new(),
                negative_evidence: Vec::new(),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let manifest = throughput_retrieval_assurance_harness_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn qualified_has_queue_witness() {
        let receipt =
            verify_throughput_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(
            receipt.verdict,
            ThroughputRetrievalAssuranceVerdict::Qualified
        );
        assert!(receipt
            .witness_order
            .iter()
            .any(|value| value == "gate:checkpoint"));
    }
    #[test]
    fn unknown_is_unresolved() {
        let receipt =
            verify_throughput_retrieval_assurance(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(
            receipt.verdict,
            ThroughputRetrievalAssuranceVerdict::Unresolved
        );
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request(EvidenceState::Supported);
        value.raw_data_local = false;
        let receipt = verify_throughput_retrieval_assurance(&value).unwrap();
        assert_eq!(
            receipt.verdict,
            ThroughputRetrievalAssuranceVerdict::Blocked
        );
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "assurance:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn assurance_artifact_payload_is_bound() {
        let mut receipt =
            verify_throughput_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        receipt.partition = "partition:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt =
            verify_throughput_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        receipt.qualified_order[0] = receipt.qualified_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt =
            verify_throughput_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
