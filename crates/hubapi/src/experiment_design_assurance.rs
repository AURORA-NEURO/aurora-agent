//! Federated continual experiment-design assurance harness (`AFA-hubapi-P09-F28`).
//!
//! This A1 verifier checks caller-supplied, digest-bound experiment-design candidates and peer
//! capability declarations. It never designs an experiment, runs a protocol, contacts an
//! instrument, exports raw data, or makes a clinical decision. Missing power, modality, control,
//! provenance, replay, policy, or peer evidence remains explicit and fails closed.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-hubapi-P09-F28";
pub const CONTRACT_VERSION: &str =
    "hubapi-federated-continual-experiment-design-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective4@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.hubapi-experiment-design-assurance-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignCandidate4 {
    pub design_id: String,
    pub objective_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub modality_order: Vec<String>,
    pub control_order: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub power_milli: u16,
    pub permitted: bool,
    pub signed: bool,
    pub local_only: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignPeer4 {
    pub peer_id: String,
    pub semantic_profile: String,
    pub capability_schema: String,
    pub scope: String,
    pub checkpoint_seq: u64,
    pub signed: bool,
    pub policy_allowed: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub attestation_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective4 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub required_modality_order: Vec<String>,
    pub required_control_order: Vec<String>,
    pub required_peer_quorum: usize,
    pub checkpoint_seq: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub candidates: Vec<ExperimentDesignCandidate4>,
    pub peers: Vec<ExperimentDesignPeer4>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesignArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesign7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub missing_control_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub assurance_digest: ContentHash,
    pub artifact: ExecutableExperimentDesignArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExperimentDesignAssuranceError {
    #[error("invalid experiment-design assurance request or receipt: {0}")]
    Invalid(String),
    #[error("experiment-design assurance artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn experiment_design_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "hubapi".into(),
        consumers: ["integration engineer".into(), "experiment-design steward".into(), "federated workflow operator".into()].into(),
        behavior: "verify federated continual experiment-design candidates and peer capability closure with deterministic evidence and policy witnesses".into(),
        value: "prevents unsupported, underpowered, incomparable, or unauthorized designs from being mistaken for executable research plans".into(),
        inputs: vec![TypedPort { name: "experiment_objective".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "executable_experiment_design".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl ExecutableExperimentDesign7 {
    pub fn validate(&self) -> Result<(), ExperimentDesignAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.candidate_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || [
                &self.request_id,
                &self.consumer,
                &self.purpose,
                &self.target_scope,
                &self.semantic_profile,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design identity, bounds, candidates, peers, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_modality_order,
            &self.missing_control_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ExperimentDesignAssuranceError::Invalid(
                    "design ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
            || peers.len() != self.peer_order.len()
            || peer_states.len() != peers.len()
            || peer_states.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design or peer states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.assurance_digest)
            || self.artifact.content_hash != self.assurance_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(ExperimentDesignAssuranceError::Artifact(
                "design assurance digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release")
        {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "assurance harness has an unauthorized effect".into(),
            ));
        }
        if self.effect_receipts != ["block:unsafe-release"] {
            return Err(ExperimentDesignAssuranceError::Invalid(
                "design assurance must remain verification-only".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_federated_experiment_design(
    request: &ExperimentObjective4,
) -> Result<ExecutableExperimentDesign7, ExperimentDesignAssuranceError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.target_scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modality_order.is_empty()
        || request.required_control_order.is_empty()
        || request.required_peer_quorum == 0
        || request.checkpoint_seq == 0
        || request.candidates.is_empty()
        || request.peers.is_empty()
        || !ordered(&request.required_modality_order)
        || !ordered(&request.required_control_order)
        || !digest(&request.replay_identity)
        || !request.aggregate_only
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ExperimentDesignAssuranceError::Invalid(
            "objective identity, requirements, bounds, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let candidate_order = request
        .candidates
        .iter()
        .map(|c| c.design_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_order.len() != request.candidates.len()
        || candidate_order.iter().any(|v| v.trim().is_empty())
    {
        return Err(ExperimentDesignAssuranceError::Invalid(
            "design ids must be unique and non-empty".into(),
        ));
    }
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_controls = request
        .required_control_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut missing_modalities = BTreeSet::new();
    let mut missing_controls = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.negative_result {
            negative.insert(candidate.design_id.clone());
        }
        let modalities = candidate
            .modality_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let controls = candidate
            .control_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing_m = required_modalities
            .difference(&modalities)
            .cloned()
            .collect::<Vec<_>>();
        let missing_c = required_controls
            .difference(&controls)
            .cloned()
            .collect::<Vec<_>>();
        missing_modalities.extend(
            missing_m
                .iter()
                .map(|m| format!("{}:{}", candidate.design_id, m)),
        );
        missing_controls.extend(
            missing_c
                .iter()
                .map(|m| format!("{}:{}", candidate.design_id, m)),
        );
        let hard = !candidate.permitted
            || !candidate.signed
            || !candidate.local_only
            || candidate.scope != request.target_scope
            || candidate.semantic_profile != request.semantic_profile
            || candidate.power_milli < 800
            || !digest(&candidate.artifact_digest)
            || !digest(&candidate.provenance_digest)
            || candidate.replay_identity != request.replay_identity
            || !ordered(&candidate.modality_order)
            || !ordered(&candidate.control_order);
        if !candidate.omission_order.is_empty() {
            omissions.extend(
                candidate
                    .omission_order
                    .iter()
                    .map(|o| format!("{}:{}", candidate.design_id, o)),
            );
        }
        if !missing_m.is_empty() || !missing_c.is_empty() {
            unresolved.insert(candidate.design_id.clone());
            uncertainty.insert(format!("{}:required-closure", candidate.design_id));
        } else if hard
            || matches!(
                candidate.evidence_state,
                EvidenceState::Contradicted | EvidenceState::Unknown
            )
        {
            if hard {
                blocked.insert(candidate.design_id.clone());
                omissions.insert(format!(
                    "{}:design-integrity-or-policy",
                    candidate.design_id
                ));
            } else {
                unresolved.insert(candidate.design_id.clone());
                uncertainty.insert(format!("{}:evidence-state", candidate.design_id));
            }
        } else {
            qualified.insert(candidate.design_id.clone());
        }
    }
    let peer_order = request
        .peers
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    if peer_order.len() != request.peers.len() || peer_order.iter().any(|p| p.trim().is_empty()) {
        return Err(ExperimentDesignAssuranceError::Invalid(
            "peer ids must be unique and non-empty".into(),
        ));
    }
    let qualified_peer_order = request
        .peers
        .iter()
        .filter(|p| {
            p.signed
                && p.policy_allowed
                && p.local_only
                && p.aggregate_only
                && p.semantic_profile == request.semantic_profile
                && p.capability_schema == INPUT_SCHEMA
                && p.scope == request.target_scope
                && p.checkpoint_seq == request.checkpoint_seq
                && digest(&p.attestation_digest)
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peer_order = peer_order
        .difference(&qualified_peer_order)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_peer_order.is_empty() {
        omissions.insert(format!(
            "peer-quorum:{}/{}",
            qualified_peer_order.len(),
            request.required_peer_quorum
        ));
        uncertainty.insert("peer-closure-incomplete".into());
    }
    for (flag, label) in [
        (request.policy_allow, "workflow:policy-denied"),
        (
            request.protected_closure,
            "workflow:protected-closure-incomplete",
        ),
        (
            request.federation_approved,
            "workflow:federation-approval-missing",
        ),
        (request.signed_approval, "workflow:signed-approval-missing"),
    ] {
        if !flag {
            omissions.insert(label.into());
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.signed_approval;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if qualified.is_empty()
        || !unresolved.is_empty()
        || qualified_peer_order.len() < request.required_peer_quorum
    {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
    }
    omissions.insert("workflow:verification-only".into());
    let checkpoint_digest = ContentHash::of_value(&json!({"request_id":request.request_id,"checkpoint_seq":request.checkpoint_seq,"target_scope":request.target_scope,"replay_identity":request.replay_identity})).map_err(|e| ExperimentDesignAssuranceError::Artifact(e.to_string()))?;
    let payload = json!({"candidate_order":candidate_order,"qualified_order":qualified,"unresolved_order":unresolved,"blocked_order":blocked,"missing_modality_order":missing_modalities,"missing_control_order":missing_controls,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"checkpoint_digest":checkpoint_digest,"replay_identity":request.replay_identity});
    let assurance_digest = ContentHash::of_value(&payload)
        .map_err(|e| ExperimentDesignAssuranceError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = ExecutableExperimentDesign7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        target_scope: request.target_scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order: strings("candidate_order"),
        qualified_order: strings("qualified_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        missing_modality_order: strings("missing_modality_order"),
        missing_control_order: strings("missing_control_order"),
        peer_order: strings("peer_order"),
        qualified_peer_order: strings("qualified_peer_order"),
        missing_peer_order: strings("missing_peer_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        assurance_digest: assurance_digest.clone(),
        artifact: ExecutableExperimentDesignArtifact7 {
            artifact_id: format!("hubapi-experiment-design-assurance:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: assurance_digest,
            semantic_loss: vec!["verification-only; no executable dispatch".into()],
            provenance_digests: request
                .candidates
                .iter()
                .map(|c| c.provenance_digest.clone())
                .collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: vec!["block:unsafe-release".into()],
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> ExperimentObjective4 {
        ExperimentObjective4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "design-req".into(),
            consumer: "integration".into(),
            purpose: "preclinical design assurance".into(),
            target_scope: "organoid-study".into(),
            semantic_profile: "design:v1".into(),
            required_modality_order: vec!["imaging".into()],
            required_control_order: vec!["vehicle".into()],
            required_peer_quorum: 1,
            checkpoint_seq: 4,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates: vec![ExperimentDesignCandidate4 {
                design_id: "d1".into(),
                objective_id: "o1".into(),
                scope: "organoid-study".into(),
                semantic_profile: "design:v1".into(),
                modality_order: vec!["imaging".into()],
                control_order: vec!["vehicle".into()],
                artifact_digest: h("artifact"),
                provenance_digest: h("provenance"),
                replay_identity: h("replay"),
                evidence_state: EvidenceState::Supported,
                power_milli: 900,
                permitted: true,
                signed: true,
                local_only: true,
                negative_result: false,
                omission_order: vec![],
            }],
            peers: vec![ExperimentDesignPeer4 {
                peer_id: "peer-a".into(),
                semantic_profile: "design:v1".into(),
                capability_schema: INPUT_SCHEMA.into(),
                scope: "organoid-study".into(),
                checkpoint_seq: 4,
                signed: true,
                policy_allowed: true,
                local_only: true,
                aggregate_only: true,
                attestation_digest: h("attestation"),
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            experiment_design_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_design_is_verification_only() {
        let r = assure_federated_experiment_design(&request()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn missing_modality_is_partial() {
        let mut q = request();
        q.candidates[0].modality_order.clear();
        let r = assure_federated_experiment_design(&q).unwrap();
        assert_eq!(r.disposition, "partial");
        assert!(!r.missing_modality_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            assure_federated_experiment_design(&q).unwrap().disposition,
            "blocked"
        );
    }
}
