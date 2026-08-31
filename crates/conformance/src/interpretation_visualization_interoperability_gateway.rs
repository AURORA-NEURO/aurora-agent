//! Federated interpretation and visualization interoperability gateway (`AFA-conformance-P14-F24`).
//!
//! The gateway exchanges typed, aggregate-only interpretation envelopes between research
//! institutions. It qualifies comparability and evidence closure, but never renders images,
//! moves raw data, or makes clinical decisions.

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-conformance-P14-F24";
pub const CONTRACT_VERSION: &str =
    "conformance-federated-continual-interpretation-visualization-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "InterpretationVisualizationRequest8@1";
pub const OUTPUT_SCHEMA: &str = "FederatedInterpretationVisualizationEnvelope10@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-interpretation-visualization-envelope-10+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationCandidate10 {
    pub candidate_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub visualization_order: Vec<String>,
    pub semantic_profile: String,
    pub support_milli: u16,
    pub uncertainty_milli: u16,
    pub evidence_state: GatewayEvidenceState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
    pub policy_allowed: bool,
    pub comparable: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub contradiction_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationPeer9 {
    pub peer_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub policy_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationVisualizationRequest8 {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub required_visualization_order: Vec<String>,
    pub semantic_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub candidates: Vec<InterpretationCandidate10>,
    pub peers: Vec<InterpretationPeer9>,
    pub checkpoint: u64,
    pub minimum_quorum: u16,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationVisualizationArtifact10 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedInterpretationVisualizationEnvelope10 {
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
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub missing_visualization_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub support_witness: Vec<String>,
    pub quorum_witness: Vec<String>,
    pub replay_identity: ContentHash,
    pub interpretation_digest: ContentHash,
    pub artifact: InterpretationVisualizationArtifact10,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationVisualizationGatewayError {
    #[error("invalid interpretation visualization request: {0}")]
    Invalid(String),
    #[error("interpretation visualization envelope failed validation: {0}")]
    Output(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn interpretation_visualization_interoperability_gateway_manifest() -> Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "conformance",
        "consumers": ["interpretation reviewer", "federation operator", "visualization workbench", "downstream conformance suite"],
        "behavior": "qualify continual federated interpretation and visualization envelopes using typed semantic, evidence, quorum, and policy gates",
        "value": "lets consortia compare reproducible aggregate interpretations without exporting raw experimental data",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["verify:interpretation-visualization", "emit:federation-envelope", "block:unsafe-release"],
        "permissions": ["evaluate:capability-runs", "federate:aggregate-research-artifacts"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY,
    })
}

fn validate_request(
    request: &InterpretationVisualizationRequest8,
) -> Result<(), InterpretationVisualizationGatewayError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.required_visualization_order.is_empty()
        || !ordered(&request.required_study_order)
        || !ordered(&request.required_modality_order)
        || !ordered(&request.required_visualization_order)
        || !ordered(&request.adversarial_event_order)
        || request.checkpoint == 0
        || request.minimum_quorum == 0
        || !valid_hash(&request.semantic_digest)
        || !valid_hash(&request.replay_identity)
        || request.candidates.is_empty()
        || request.peers.is_empty()
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationVisualizationGatewayError::Invalid(
            "identity, ordered requirements, quorum, digests, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !candidate_ids.insert(candidate.candidate_id.clone())
            || candidate.study_order.is_empty()
            || candidate.modality_order.is_empty()
            || candidate.visualization_order.is_empty()
            || !ordered(&candidate.study_order)
            || !ordered(&candidate.modality_order)
            || !ordered(&candidate.visualization_order)
            || candidate.semantic_profile.trim().is_empty()
            || candidate.support_milli > 1000
            || candidate.uncertainty_milli > 1000
            || !valid_hash(&candidate.semantic_digest)
            || !valid_hash(&candidate.artifact_digest)
            || !valid_hash(&candidate.provenance_digest)
            || !valid_hash(&candidate.replay_identity)
            || !ordered(&candidate.omission_order)
            || !ordered(&candidate.contradiction_order)
        {
            return Err(InterpretationVisualizationGatewayError::Invalid(
                "candidate identity, axes, scores, digests, or ordering is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.checkpoint == 0
            || !valid_hash(&peer.semantic_digest)
            || !valid_hash(&peer.artifact_digest)
            || !valid_hash(&peer.provenance_digest)
            || !valid_hash(&peer.replay_identity)
        {
            return Err(InterpretationVisualizationGatewayError::Invalid(
                "peer identity, checkpoint, digests, or origin is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedInterpretationVisualizationEnvelope10 {
    pub fn validate(&self) -> Result<(), InterpretationVisualizationGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || self.raw_data_local != true
            || self.aggregate_only != true
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
        {
            return Err(InterpretationVisualizationGatewayError::Output(
                "identity, locality, candidate/peer orders, or release gate is incomplete".into(),
            ));
        }
        for values in [
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.incomparable_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.missing_visualization_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.support_witness,
            &self.quorum_witness,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(InterpretationVisualizationGatewayError::Output(
                    "gateway order is not canonical".into(),
                ));
            }
        }
        let candidate_ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.candidate_order.len()
            || self.ranked_order.len() != self.candidate_order.len()
            || self.ranked_order.iter().cloned().collect::<BTreeSet<_>>() != candidate_ids
        {
            return Err(InterpretationVisualizationGatewayError::Output(
                "candidate and ranked orders are not a deterministic permutation".into(),
            ));
        }
        let states = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.incomparable_order)
            .cloned()
            .collect::<Vec<_>>();
        if states.len() != candidate_ids.len() || BTreeSet::from_iter(states) != candidate_ids {
            return Err(InterpretationVisualizationGatewayError::Output(
                "candidate outcomes do not partition the ranked candidates".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if peers.len() != self.peer_order.len()
            || peer_states.len() != peers.len()
            || BTreeSet::from_iter(peer_states) != peers
        {
            return Err(InterpretationVisualizationGatewayError::Output(
                "peer outcomes do not partition peers".into(),
            ));
        }
        if !valid_hash(&self.replay_identity)
            || !valid_hash(&self.interpretation_digest)
            || self.artifact.content_hash != self.interpretation_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|hash| !valid_hash(hash))
        {
            return Err(InterpretationVisualizationGatewayError::Output(
                "interpretation digest or artifact provenance is invalid".into(),
            ));
        }
        if self.disposition == "qualified"
            && !self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("verify:interpretation-visualization:"))
        {
            return Err(InterpretationVisualizationGatewayError::Output(
                "qualified gateway envelope lacks verification effect".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != vec!["block:unsafe-release"] {
            return Err(InterpretationVisualizationGatewayError::Output(
                "non-qualified gateway envelope must block release".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationVisualizationGatewayError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self).map_err(|error| {
                InterpretationVisualizationGatewayError::Output(error.to_string())
            })?,
        )
        .map_err(|error| InterpretationVisualizationGatewayError::Output(error.to_string()))
    }
}

pub fn assure_interpretation_visualization_gateway(
    request: &InterpretationVisualizationRequest8,
) -> Result<FederatedInterpretationVisualizationEnvelope10, InterpretationVisualizationGatewayError>
{
    validate_request(request)?;
    let mut rows = request.candidates.clone();
    rows.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(left.uncertainty_milli.cmp(&right.uncertainty_milli))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let candidate_order = rows
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let peer_order = request
        .peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && peer.policy_allowed
            && peer.semantic_profile == request.semantic_profile
            && peer.semantic_digest == request.semantic_digest
            && peer.replay_identity == request.replay_identity
            && peer.checkpoint >= request.checkpoint
        {
            qualified_peers.insert(peer.peer_id.clone());
        }
    }
    let missing_peers = peer_order
        .iter()
        .filter(|peer| !qualified_peers.contains(*peer))
        .cloned()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut missing_study = BTreeSet::new();
    let mut missing_modality = BTreeSet::new();
    let mut missing_visualization = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut support_witness = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for candidate in &rows {
        let id = candidate.candidate_id.clone();
        provenance.insert(candidate.provenance_digest.clone());
        omission.extend(
            candidate
                .omission_order
                .iter()
                .map(|value| format!("{id}:{value}")),
        );
        uncertainty.extend(
            candidate
                .uncertainty_milli
                .gt(&300)
                .then(|| format!("{id}:support-or-uncertainty-threshold")),
        );
        contradiction.extend(
            candidate
                .contradiction_order
                .iter()
                .map(|value| format!("{id}:{value}")),
        );
        if candidate.negative_result || candidate.evidence_state == GatewayEvidenceState::Negative {
            negative.insert(format!("{id}:negative-result"));
        }
        if !candidate.local
            || !candidate.aggregate_only
            || !candidate.policy_allowed
            || candidate.replay_identity != request.replay_identity
        {
            blocked.insert(id);
        } else if request
            .required_study_order
            .iter()
            .any(|value| !candidate.study_order.contains(value))
        {
            missing_study.extend(
                request
                    .required_study_order
                    .iter()
                    .filter(|value| !candidate.study_order.contains(value))
                    .map(|value| format!("{id}:{value}")),
            );
            incomparable.insert(id.clone());
        } else if request
            .required_modality_order
            .iter()
            .any(|value| !candidate.modality_order.contains(value))
        {
            missing_modality.extend(
                request
                    .required_modality_order
                    .iter()
                    .filter(|value| !candidate.modality_order.contains(value))
                    .map(|value| format!("{id}:{value}")),
            );
            incomparable.insert(id.clone());
        } else if request
            .required_visualization_order
            .iter()
            .any(|value| !candidate.visualization_order.contains(value))
        {
            missing_visualization.extend(
                request
                    .required_visualization_order
                    .iter()
                    .filter(|value| !candidate.visualization_order.contains(value))
                    .map(|value| format!("{id}:{value}")),
            );
            incomparable.insert(id.clone());
        } else if !candidate.comparable
            || candidate.semantic_profile != request.semantic_profile
            || candidate.semantic_digest != request.semantic_digest
        {
            incomparable.insert(id.clone());
            uncertainty.insert(format!("{id}:semantic-comparability-mismatch"));
        } else if matches!(
            candidate.evidence_state,
            GatewayEvidenceState::Contradicted | GatewayEvidenceState::Negative
        ) {
            unresolved.insert(id.clone());
            contradiction.insert(format!("{id}:contradicted-or-negative"));
        } else if !matches!(
            candidate.evidence_state,
            GatewayEvidenceState::Proven | GatewayEvidenceState::Supported
        ) || candidate.support_milli < 700
            || candidate.uncertainty_milli > 300
        {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:support-or-uncertainty-threshold"));
        } else {
            qualified.insert(id.clone());
            support_witness.insert(format!("{id}:support={}milli", candidate.support_milli));
        }
    }
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allowed
        || !request.raw_data_local
        || !request.aggregate_only
        || request.adversarial_event_order.iter().next().is_some()
        || qualified_peers.len() < request.minimum_quorum as usize;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        incomparable.clear();
        omission.insert("request:governance-quorum-or-adversarial-blocked".into());
    }
    uncertainty.extend(
        request
            .adversarial_event_order
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let incomparable_order = incomparable.into_iter().collect::<Vec<_>>();
    let disposition = if global_block {
        "blocked"
    } else if !unresolved_order.is_empty()
        || !blocked_order.is_empty()
        || !incomparable_order.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    let mut omission_order = omission.into_iter().collect::<Vec<_>>();
    if disposition != "qualified" {
        omission_order.push("request:interpretation-closure-not-ready".into());
        omission_order.sort();
    }
    let mut payload = json!({
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
        "candidate_order": candidate_order.clone(),
        "ranked_order": candidate_order,
        "qualified_order": qualified_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "incomparable_order": incomparable_order,
        "missing_study_order": missing_study.into_iter().collect::<Vec<_>>(),
        "missing_modality_order": missing_modality.into_iter().collect::<Vec<_>>(),
        "missing_visualization_order": missing_visualization.into_iter().collect::<Vec<_>>(),
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers.iter().cloned().collect::<Vec<_>>(),
        "missing_peer_order": missing_peers,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty.into_iter().collect::<Vec<_>>(),
        "contradiction_order": contradiction.into_iter().collect::<Vec<_>>(),
        "negative_evidence_order": negative.into_iter().collect::<Vec<_>>(),
        "support_witness": support_witness.into_iter().collect::<Vec<_>>(),
        "quorum_witness": [format!("qualified-peers={}", qualified_peers.len()), format!("required-quorum={}", request.minimum_quorum)],
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| InterpretationVisualizationGatewayError::Output(error.to_string()))?;
    payload["interpretation_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id": format!("federated-interpretation-visualization-10:{}", request.request_id), "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": provenance.into_iter().collect::<Vec<_>>(), "boundary": PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = if disposition == "qualified" {
        json!([format!(
            "verify:interpretation-visualization:{}",
            request.request_id
        )])
    } else {
        json!(["block:unsafe-release"])
    };
    let envelope: FederatedInterpretationVisualizationEnvelope10 = serde_json::from_value(payload)
        .map_err(|error| InterpretationVisualizationGatewayError::Output(error.to_string()))?;
    envelope.validate()?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate() -> InterpretationCandidate10 {
        InterpretationCandidate10 {
            candidate_id: "c1".into(),
            study_order: vec!["study-1".into()],
            modality_order: vec!["imaging".into()],
            visualization_order: vec!["panel".into()],
            semantic_profile: "ome-ngff@0.5".into(),
            support_milli: 850,
            uncertainty_milli: 120,
            evidence_state: GatewayEvidenceState::Supported,
            semantic_digest: hash("semantic"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            local: true,
            aggregate_only: true,
            policy_allowed: true,
            comparable: true,
            negative_result: false,
            omission_order: vec![],
            contradiction_order: vec![],
        }
    }
    fn peer() -> InterpretationPeer9 {
        InterpretationPeer9 {
            peer_id: "peer-1".into(),
            origin: "site-a".into(),
            semantic_profile: "ome-ngff@0.5".into(),
            checkpoint: 2,
            semantic_digest: hash("semantic"),
            artifact_digest: hash("peer-artifact"),
            provenance_digest: hash("peer-provenance"),
            replay_identity: hash("replay"),
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
            policy_allowed: true,
        }
    }
    fn request() -> InterpretationVisualizationRequest8 {
        InterpretationVisualizationRequest8 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req-1".into(),
            federation_id: "fed-1".into(),
            researcher: "researcher".into(),
            purpose: "compare panels".into(),
            semantic_profile: "ome-ngff@0.5".into(),
            required_study_order: vec!["study-1".into()],
            required_modality_order: vec!["imaging".into()],
            required_visualization_order: vec!["panel".into()],
            semantic_digest: hash("semantic"),
            replay_identity: hash("replay"),
            candidates: vec![candidate()],
            peers: vec![peer()],
            checkpoint: 1,
            minimum_quorum: 1,
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            federation_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_envelope_emits_verification() {
        let output = assure_interpretation_visualization_gateway(&request()).unwrap();
        assert_eq!(output.disposition, "qualified");
        assert!(output.effect_receipts[0].starts_with("verify:interpretation-visualization:"));
    }
    #[test]
    fn high_uncertainty_remains_unresolved() {
        let mut input = request();
        input.candidates[0].uncertainty_milli = 900;
        let output = assure_interpretation_visualization_gateway(&input).unwrap();
        assert_eq!(output.disposition, "unresolved");
        assert!(!output.uncertainty_order.is_empty());
    }
    #[test]
    fn adversarial_event_blocks_every_candidate() {
        let mut input = request();
        input.adversarial_event_order = vec!["prompt-injection".into()];
        let output = assure_interpretation_visualization_gateway(&input).unwrap();
        assert_eq!(output.disposition, "blocked");
        assert_eq!(output.blocked_order, vec!["c1"]);
    }
}
