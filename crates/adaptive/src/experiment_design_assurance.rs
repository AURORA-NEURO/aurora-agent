//! Prospective high-throughput experiment-design assurance for `AFA-adaptive-P09-F27`.
//!
//! This contract qualifies caller-supplied power-aware design candidates and signed peer
//! summaries. It does not fit a model, allocate animals, operate instruments, or turn missing
//! evidence into a design recommendation. Every threshold witness, omission, contradiction,
//! and negative result is retained in the deterministic receipt.

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adaptive-P09-F27";
pub const CONTRACT_VERSION: &str =
    "adaptive-prospective-high-throughput-experiment-design-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentDesignRequest7@1";
pub const OUTPUT_SCHEMA: &str = "ExperimentDesignAssuranceReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.experiment-design-assurance-receipt-9+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignCandidate7 {
    pub candidate_id: String,
    pub design_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub power_milli: u32,
    pub variance_milli: u32,
    pub attrition_milli: u32,
    pub replication_milli: u32,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub independent_source: bool,
    pub local_data: bool,
    pub policy_allowed: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignPeer6 {
    pub peer_id: String,
    pub origin: String,
    pub design_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub power_milli: u32,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignRequest7 {
    pub request_id: String,
    pub federation_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub minimum_power_milli: u32,
    pub maximum_variance_milli: u32,
    pub maximum_attrition_milli: u32,
    pub minimum_replication_milli: u32,
    pub candidates: Vec<ExperimentDesignCandidate7>,
    pub peers: Vec<ExperimentDesignPeer6>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignArtifact9 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignAssuranceReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub alternative_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub power_witness_order: Vec<String>,
    pub variance_witness_order: Vec<String>,
    pub attrition_witness_order: Vec<String>,
    pub replication_witness_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub design_digest: ContentHash,
    pub artifact: ExperimentDesignArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentDesignAssuranceError {
    #[error("invalid experiment-design assurance request: {0}")]
    Invalid(String),
    #[error("experiment-design assurance artifact failed: {0}")]
    Artifact(String),
}

pub fn experiment_design_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "adaptive",
        "consumers": ["computational biologist", "power-design reviewer", "federation steward"],
        "behavior": "qualifies prospective power-aware experiment designs and peer attestations under explicit threshold and governance gates",
        "value": "prevents underpowered, high-variance, or non-reproducible design candidates from silently entering a preclinical workflow",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["retain:experiment-design-assurance", "exchange:aggregate-design-summary"],
        "permissions": ["retain:design-evidence", "exchange:aggregate-design"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY,
    })
}

impl ExperimentDesignAssuranceReceipt9 {
    pub fn validate(&self) -> Result<(), ExperimentDesignAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design identity, checkpoint, locality, candidates, peers, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.ranked_order,
            &self.selected_order,
            &self.alternative_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.power_witness_order,
            &self.variance_witness_order,
            &self.attrition_witness_order,
            &self.replication_witness_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ExperimentDesignAssuranceError::Invalid(
                    "experiment-design ordering is not canonical".into(),
                ));
            }
        }
        let universe = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.alternative_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_candidate_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if universe.len() != self.candidate_order.len() || universe != parts {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design candidates do not partition".into(),
            ));
        }
        let ranked = BTreeSet::from_iter(self.ranked_order.iter().cloned());
        if ranked.len() != self.ranked_order.len() || !ranked.is_subset(&universe) {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design ranking is not a candidate subset".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers.len() != self.peer_order.len() || peers != peer_parts {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design peers do not partition".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.design_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ExperimentDesignAssuranceError::Artifact(
                    "design digest is not a 256-bit hexadecimal hash".into(),
                ));
            }
        }
        if self.artifact.content_hash != self.design_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| digest.as_str().len() != 64)
        {
            return Err(ExperimentDesignAssuranceError::Artifact(
                "design artifact digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_experiment_design(
    request: &ExperimentDesignRequest7,
) -> Result<ExperimentDesignAssuranceReceipt9, ExperimentDesignAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| {
        b.power_milli
            .cmp(&a.power_milli)
            .then(a.variance_milli.cmp(&b.variance_milli))
            .then(a.attrition_milli.cmp(&b.attrition_milli))
            .then(b.replication_milli.cmp(&a.replication_milli))
            .then(a.candidate_id.cmp(&b.candidate_id))
    });
    let mut candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let studies = candidates
        .iter()
        .map(|candidate| candidate.study_id.clone())
        .collect::<BTreeSet<_>>();
    let modalities = candidates
        .iter()
        .map(|candidate| candidate.modality.clone())
        .collect::<BTreeSet<_>>();
    let missing_studies = request
        .required_study_order
        .iter()
        .filter(|study| !studies.contains(*study))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_modalities = request
        .required_modality_order
        .iter()
        .filter(|modality| !modalities.contains(*modality))
        .cloned()
        .collect::<BTreeSet<_>>();
    for missing in missing_studies.iter().chain(&missing_modalities) {
        candidate_order.insert(missing.clone());
    }
    let candidate_order = candidate_order.into_iter().collect::<Vec<_>>();
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut alternatives = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut power_witness = BTreeSet::new();
    let mut variance_witness = BTreeSet::new();
    let mut attrition_witness = BTreeSet::new();
    let mut replication_witness = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_design_id = None;
    for candidate in &candidates {
        if candidate.negative_result {
            negative.insert(format!("{}:negative-result", candidate.candidate_id));
        }
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                blocked.insert(candidate.candidate_id.clone());
                contradiction.insert(format!("{}:contradicted", candidate.candidate_id));
                continue;
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unresolved.insert(candidate.candidate_id.clone());
                uncertainty.insert(format!("{}:evidence-state", candidate.candidate_id));
                continue;
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        let power_ok = candidate.power_milli >= request.minimum_power_milli;
        let variance_ok = candidate.variance_milli <= request.maximum_variance_milli;
        let attrition_ok = candidate.attrition_milli <= request.maximum_attrition_milli;
        let replication_ok = candidate.replication_milli >= request.minimum_replication_milli;
        if !power_ok {
            power_witness.insert(format!("{}:power-below-threshold", candidate.candidate_id));
        }
        if !variance_ok {
            variance_witness.insert(format!(
                "{}:variance-above-threshold",
                candidate.candidate_id
            ));
        }
        if !attrition_ok {
            attrition_witness.insert(format!(
                "{}:attrition-above-threshold",
                candidate.candidate_id
            ));
        }
        if !replication_ok {
            replication_witness.insert(format!(
                "{}:replication-below-threshold",
                candidate.candidate_id
            ));
        }
        if power_ok
            && variance_ok
            && attrition_ok
            && replication_ok
            && candidate.independent_source
            && candidate.local_data
            && candidate.policy_allowed
            && candidate.semantic_profile == request.semantic_profile
        {
            if selected.is_empty() {
                selected.insert(candidate.candidate_id.clone());
                selected_design_id = Some(candidate.design_id.clone());
            } else {
                alternatives.insert(candidate.candidate_id.clone());
            }
        } else {
            unresolved.insert(candidate.candidate_id.clone());
            if !candidate.independent_source {
                uncertainty.insert(format!("{}:independence-missing", candidate.candidate_id));
            }
            if !candidate.local_data {
                uncertainty.insert(format!("{}:locality-missing", candidate.candidate_id));
            }
            if !candidate.policy_allowed {
                uncertainty.insert(format!("{}:policy-not-allowed", candidate.candidate_id));
            }
            if candidate.semantic_profile != request.semantic_profile {
                uncertainty.insert(format!(
                    "{}:semantic-profile-mismatch",
                    candidate.candidate_id
                ));
            }
        }
    }
    let missing_candidates = missing_studies
        .iter()
        .chain(&missing_modalities)
        .cloned()
        .collect::<BTreeSet<_>>();
    let omissions = missing_studies
        .iter()
        .map(|item| format!("study:{}:missing", item))
        .chain(
            missing_modalities
                .iter()
                .map(|item| format!("modality:{}:missing", item)),
        )
        .collect::<BTreeSet<_>>();
    let mut omissions = omissions;
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let qualified_peers = peers
        .iter()
        .filter(|peer| {
            selected_design_id.as_deref() == Some(peer.design_id.as_str())
                && peer.semantic_profile == request.semantic_profile
                && peer.checkpoint == request.checkpoint
                && peer.power_milli >= request.minimum_power_milli
                && peer.signed
                && peer.aggregate_only
                && peer.raw_data_local
                && matches!(
                    peer.evidence_state,
                    EvidenceState::Proven | EvidenceState::Supported
                )
        })
        .map(|peer| peer.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|peer| !qualified_peers.contains(*peer))
        .cloned()
        .collect::<BTreeSet<_>>();
    for peer in &missing_peers {
        uncertainty.insert(format!("peer:{}:not-qualified", peer));
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || !missing_studies.is_empty()
        || !missing_modalities.is_empty()
        || !unresolved.is_empty()
        || qualified_peers.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:design-not-release-ready".into());
    }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "researcher": request.researcher,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "ranked_order": ranked_order,
        "selected_order": selected,
        "alternative_order": alternatives,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "missing_candidate_order": missing_candidates,
        "missing_study_order": missing_studies,
        "missing_modality_order": missing_modalities,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers,
        "missing_peer_order": missing_peers,
        "power_witness_order": power_witness,
        "variance_witness_order": variance_witness,
        "attrition_witness_order": attrition_witness,
        "replication_witness_order": replication_witness,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "contradiction_order": contradiction,
        "negative_evidence_order": negative,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| ExperimentDesignAssuranceError::Artifact(error.to_string()))?;
    let artifact = ExperimentDesignArtifact9 {
        artifact_id: format!(
            "experiment-design-assurance-receipt-9:{}",
            request.request_id
        ),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: candidates
            .iter()
            .map(|candidate| candidate.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:aggregate-design-summary:{}", request.request_id),
            format!("retain:experiment-design-assurance:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ExperimentDesignAssuranceReceipt9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        researcher: request.researcher.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order,
        ranked_order,
        selected_order: selected.into_iter().collect(),
        alternative_order: alternatives.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        missing_candidate_order: missing_candidates.into_iter().collect(),
        missing_study_order: missing_studies.into_iter().collect(),
        missing_modality_order: missing_modalities.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        power_witness_order: power_witness.into_iter().collect(),
        variance_witness_order: variance_witness.into_iter().collect(),
        attrition_witness_order: attrition_witness.into_iter().collect(),
        replication_witness_order: replication_witness.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        design_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ExperimentDesignRequest7,
) -> Result<(), ExperimentDesignAssuranceError> {
    if ![
        &request.request_id,
        &request.federation_id,
        &request.researcher,
        &request.purpose,
        &request.semantic_profile,
    ]
    .iter()
    .all(|value| !value.trim().is_empty())
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.candidates.is_empty()
        || request.peers.is_empty()
        || request.checkpoint == 0
        || request.minimum_peer_quorum == 0
        || request.minimum_power_milli > 10_000
        || request.maximum_variance_milli > 10_000
        || request.maximum_attrition_milli > 10_000
        || request.minimum_replication_milli > 10_000
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ExperimentDesignAssuranceError::Invalid(
            "design identity, bounds, candidates, peers, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.design_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.semantic_profile.trim().is_empty()
            || candidate.power_milli > 10_000
            || candidate.variance_milli > 10_000
            || candidate.attrition_milli > 10_000
            || candidate.replication_milli > 10_000
            || candidate.artifact_digest.as_str().len() != 64
            || candidate.provenance_digest.as_str().len() != 64
            || candidate.replay_identity != request.replay_identity
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "candidate identity, metrics, digests, or replay is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.design_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.checkpoint == 0
            || peer.power_milli > 10_000
            || peer.artifact_digest.as_str().len() != 64
            || peer.provenance_digest.as_str().len() != 64
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "peer identity, checkpoint, metric, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> ExperimentDesignRequest7 {
        let replay = hash("replay");
        ExperimentDesignRequest7 {
            request_id: "request:design".into(),
            federation_id: "fed:design".into(),
            researcher: "researcher".into(),
            purpose: "power-aware-design".into(),
            semantic_profile: "neuro:v1".into(),
            required_study_order: vec!["study:a".into()],
            required_modality_order: vec!["imaging".into()],
            minimum_power_milli: 8000,
            maximum_variance_milli: 2000,
            maximum_attrition_milli: 1500,
            minimum_replication_milli: 7000,
            candidates: vec![ExperimentDesignCandidate7 {
                candidate_id: "candidate:a".into(),
                design_id: "design:a".into(),
                study_id: "study:a".into(),
                modality: "imaging".into(),
                semantic_profile: "neuro:v1".into(),
                power_milli: 9000,
                variance_milli: 1000,
                attrition_milli: 500,
                replication_milli: 8500,
                evidence_state: EvidenceState::Supported,
                artifact_digest: hash("artifact"),
                provenance_digest: hash("provenance"),
                replay_identity: replay.clone(),
                independent_source: true,
                local_data: true,
                policy_allowed: true,
                negative_result: false,
            }],
            peers: vec![ExperimentDesignPeer6 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                design_id: "design:a".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 4,
                power_milli: 8500,
                artifact_digest: hash("peer-artifact"),
                provenance_digest: hash("peer-provenance"),
                evidence_state: EvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 4,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: replay,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualified_design() {
        assert_eq!(
            assure_experiment_design(&request()).unwrap().disposition,
            "qualified"
        );
    }

    #[test]
    fn underpowered_design_is_unresolved() {
        let mut value = request();
        value.candidates[0].power_milli = 5000;
        assert_eq!(
            assure_experiment_design(&value).unwrap().disposition,
            "unresolved"
        );
    }

    #[test]
    fn contradiction_blocks_design() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        assert_eq!(
            assure_experiment_design(&value).unwrap().disposition,
            "blocked"
        );
    }
}
