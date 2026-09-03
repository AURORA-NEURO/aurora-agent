//! Prospective high-throughput retrieval-and-synthesis inference engine.
//!
//! Atlas feature: `AFA-brain-P02-F03`. Batch capacity, queue identity, and checkpoints remain
//! visible in the synthesis artifact; overflow and unresolved evidence are never silent.

use crate::retrieval_synthesis::{RetrievalCandidate, SynthesisDisposition};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F03";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-synthesis/1.0";
const SYNTHESIS_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-evidence-synthesis+json";
const MAX_CANDIDATES: usize = 4096;
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalQuery {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub max_items: usize,
    pub minimum_support_milli: u16,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: SynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputRetrievalError {
    #[error("invalid throughput retrieval query: {0}")]
    Invalid(String),
    #[error("throughput retrieval artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputEvidenceSynthesis {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_order.len() != self.support_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis identity, queue, ranking, support, or effects are incomplete"
                    .into(),
            ));
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked_values = self.ranked_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked_values != candidate_values
            || !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis candidate states do not form an exact partition".into(),
            ));
        }
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
        ] {
            validate_unique(values, field)?;
        }
        for (values, field) in [
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalError::Invalid(
                    "throughput synthesis digest is invalid".into(),
                ));
            }
        }
        let expected_disposition = if self.omissions.iter().any(|value| {
            value == "request:policy-denied"
                || value == "request:protected-closure-incomplete"
                || value == "request:raw-data-locality-failed"
        }) {
            SynthesisDisposition::Blocked
        } else if self.qualified_order.is_empty() {
            SynthesisDisposition::Unknown
        } else if blocked_values.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            SynthesisDisposition::Qualified
        } else {
            SynthesisDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis disposition does not match retained state".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "read:local-throughput-artifacts:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis effects do not match disposition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-throughput-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis effect is outside the local gate".into(),
            ));
        }
        let qualified_exact = self.qualified_order.iter().collect::<BTreeSet<_>>();
        let expected_qualified = self
            .ranked_order
            .iter()
            .filter(|candidate| qualified_exact.contains(candidate))
            .cloned()
            .collect::<Vec<_>>();
        if self.qualified_order != expected_qualified {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis qualified order must retain ranked order".into(),
            ));
        }
        let expected_checkpoint_seq = if self.candidate_order.is_empty() {
            0
        } else {
            1
        };
        if self.checkpoint_seq != expected_checkpoint_seq {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis checkpoint is not bound to candidate coverage".into(),
            ));
        }
        let expected_synthesis_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "support_order": self.support_order,
            "queue_digest": self.queue_digest,
            "disposition": self.disposition,
        }))
        .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
        if self.synthesis_digest != expected_synthesis_digest {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis digest is not bound to ranked evidence".into(),
            ));
        }
        let expected_artifact_id = format!("brain-throughput-retrieval:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != SYNTHESIS_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput synthesis artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_synthesis_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "throughput retrieval operator".into()].into(), behavior: "deterministically ranks bounded retrieval batches with capacity-aware queue and checkpoint receipts".into(), value: "provides auditable high-throughput synthesis without silent overflow, unsupported admission, or external data movement".into(), inputs: vec![TypedPort { name: "throughput_retrieval_query".into(), schema: "ScopedRetrievalQuery3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_evidence_synthesis".into(), schema: "EvidenceSynthesis1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn synthesize_throughput_retrieval(
    request: &ThroughputRetrievalQuery,
) -> Result<ThroughputEvidenceSynthesis, ThroughputRetrievalError> {
    validate_request(request)?;
    let submitted_candidate_count = request.candidates.len();
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let duplicate_candidate_count = candidates
        .windows(2)
        .filter(|pair| pair[0].evidence_id == pair[1].evidence_id)
        .count();
    candidates.dedup_by(|left, right| left.evidence_id == right.evidence_id);
    let candidate_order = candidates
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut ranked = candidates.clone();
    ranked.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let ranked_order = ranked
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let support_order = ranked
        .iter()
        .map(|item| item.support_milli)
        .collect::<Vec<_>>();
    let mut qualified = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if submitted_candidate_count > request.max_items {
        omissions.insert("batch:capacity-overflow".into());
    }
    if duplicate_candidate_count > 0 {
        omissions.insert("batch:duplicate-candidate-retained".into());
    }
    for candidate in &ranked {
        let capacity_ok = qualified.len() < request.max_items;
        let admissible = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && candidate.raw_data_local
            && capacity_ok
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.state == EvidenceState::Supported
            && candidate.omissions.is_empty()
            && candidate.replay_identity == request.replay_identity;
        if admissible {
            qualified.push(candidate.evidence_id.clone());
        } else {
            blocked.insert(candidate.evidence_id.clone());
            if matches!(
                candidate.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(candidate.evidence_id.clone());
                uncertainty.insert(format!(
                    "evidence:{}:state-not-qualified",
                    candidate.evidence_id
                ));
            }
            if candidate.state == EvidenceState::Contradicted
                || !candidate.negative_evidence.is_empty()
            {
                negative.insert(format!(
                    "evidence:{}:negative-result-retained",
                    candidate.evidence_id
                ));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    candidate.evidence_id
                ));
            }
            if !candidate.omissions.is_empty() {
                omissions.insert(format!(
                    "evidence:{}:protected-closure-incomplete",
                    candidate.evidence_id
                ));
            }
        }
    }
    if !request.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            SynthesisDisposition::Blocked
        } else if qualified.is_empty() {
            SynthesisDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            SynthesisDisposition::Qualified
        } else {
            SynthesisDisposition::Partial
        };
    let checkpoint_seq = if candidate_order.is_empty() { 0 } else { 1 };
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "partition": request.partition, "candidate_order": candidate_order, "max_items": request.max_items, "checkpoint_seq": checkpoint_seq})).map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
    let synthesis_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "ranked_order": ranked_order, "qualified_order": qualified, "support_order": support_order, "queue_digest": queue_digest, "disposition": disposition})).map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "support_order": support_order, "checkpoint_seq": checkpoint_seq, "queue_digest": queue_digest, "synthesis_digest": synthesis_digest, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-retrieval:{}", request.request_id),
        SYNTHESIS_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalError::Artifact(error.to_string()))?;
    let receipt = ThroughputEvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        disposition,
        candidate_order,
        ranked_order,
        qualified_order: qualified,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        support_order,
        checkpoint_seq,
        queue_digest,
        synthesis_digest,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if matches!(
            disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "read:local-throughput-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ThroughputRetrievalQuery) -> Result<(), ThroughputRetrievalError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.max_items == 0
        || request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputRetrievalError::Invalid("throughput retrieval identity, capacity, threshold, candidates, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.batch_id, "batch_id"),
        (&request.partition, "partition"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.replay_identity.as_str().len() != 64 {
        return Err(ThroughputRetrievalError::Invalid(
            "throughput retrieval replay identity is invalid".into(),
        ));
    }
    let mut exact_ids = BTreeSet::new();
    let mut identity_keys = BTreeSet::new();
    for candidate in &request.candidates {
        for (value, field) in [
            (&candidate.evidence_id, "candidate.evidence_id"),
            (&candidate.source_id, "candidate.source_id"),
            (&candidate.study_id, "candidate.study_id"),
            (&candidate.scope, "candidate.scope"),
            (&candidate.modality, "candidate.modality"),
            (&candidate.boundary, "candidate.boundary"),
        ] {
            validate_text(value, field)?;
        }
        let exact_duplicate = !exact_ids.insert(candidate.evidence_id.clone());
        let case_collision = !identity_keys.insert(candidate.evidence_id.to_ascii_lowercase());
        if candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || (case_collision && !exact_duplicate)
            || candidate.semantic_digest.as_str().len() != 64
            || candidate.artifact_digest.as_str().len() != 64
            || candidate.provenance_digest.as_str().len() != 64
            || candidate.replay_identity.as_str().len() != 64
        {
            return Err(ThroughputRetrievalError::Invalid(
                "throughput candidate identity, digest, support, boundary, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputEvidenceSynthesis) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "support_order": receipt.support_order,
        "checkpoint_seq": receipt.checkpoint_seq,
        "queue_digest": receipt.queue_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "replay_identity": receipt.replay_identity,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, state: EvidenceState) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            modality: "imaging".into(),
            support_milli: 900,
            state,
            semantic_digest: hash(id),
            artifact_digest: hash(&format!("a:{id}")),
            provenance_digest: hash(&format!("p:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<RetrievalCandidate>) -> ThroughputRetrievalQuery {
        ThroughputRetrievalQuery {
            request_id: "request:tp-retrieval".into(),
            batch_id: "batch:001".into(),
            partition: "partition:imaging".into(),
            max_items: 2,
            minimum_support_milli: 700,
            candidates,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = throughput_retrieval_synthesis_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_is_qualified() {
        let r = synthesize_throughput_retrieval(&request(vec![candidate(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Qualified);
        assert_eq!(r.checkpoint_seq, 1);
    }
    #[test]
    fn overflow_is_partial() {
        let r = synthesize_throughput_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported),
            candidate("b", EvidenceState::Supported),
            candidate("c", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Partial);
        assert!(r.omissions.iter().any(|v| v.contains("overflow")));
    }
    #[test]
    fn unknown_is_retained() {
        let r =
            synthesize_throughput_retrieval(&request(vec![candidate("a", EvidenceState::Unknown)]))
                .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Unknown);
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(vec![candidate("a", EvidenceState::Supported)]);
        q.policy_allow = false;
        let r = synthesize_throughput_retrieval(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = synthesize_throughput_retrieval(&request(vec![candidate(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }

    #[test]
    fn ranking_order_can_differ_from_canonical_candidate_order() {
        let mut lower_ranked = candidate("a", EvidenceState::Supported);
        lower_ranked.support_milli = 700;
        let higher_ranked = candidate("b", EvidenceState::Supported);
        let receipt =
            synthesize_throughput_retrieval(&request(vec![lower_ranked, higher_ranked])).unwrap();
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(receipt.ranked_order, vec!["evidence:b", "evidence:a"]);
        assert_eq!(receipt.qualified_order, receipt.ranked_order);
        receipt.validate().unwrap();
    }

    #[test]
    fn support_order_and_artifact_payload_are_bound() {
        let mut receipt = synthesize_throughput_retrieval(&request(vec![candidate(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        receipt.support_order[0] = 1;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_colliding_candidate_ids_are_rejected() {
        let mut upper = candidate("A", EvidenceState::Supported);
        upper.evidence_id = "evidence:A".into();
        let lower = candidate("a", EvidenceState::Supported);
        assert!(synthesize_throughput_retrieval(&request(vec![upper, lower])).is_err());
    }
}
