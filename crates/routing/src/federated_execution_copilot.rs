//! Federated continual computational-execution research copilot.
//!
//! Atlas feature: `AFA-routing-P12-F12`.
//!
//! The copilot chooses among already compiled, institution-local execution plans.  It never
//! dispatches a workflow or moves raw data: a receipt is the product boundary.  Plan evidence,
//! peer quorum, replay identity, policy, locality, and explicit omissions are all required before
//! a route can be qualified.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-routing-P12-F12";
pub const CONTRACT_VERSION: &str =
    "routing-federated-continual-computational-execution-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ExecutionPlanSet6@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRoutingReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.routing-execution-routing-receipt-9+json";
pub const MAX_PLANS: usize = 256;
pub const MAX_PEERS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPlanEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanCandidate6 {
    pub plan_id: String,
    pub workflow_digest: ContentHash,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub expected_discovery_milli: u32,
    pub risk_milli: u32,
    pub evidence_state: ExecutionPlanEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: String,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlanPeer5 {
    pub peer_id: String,
    pub plan_id: String,
    pub semantic_profile: String,
    pub utility_milli: u32,
    pub evidence_state: ExecutionPlanEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: String,
    pub authorized: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedExecutionCopilotRequest8 {
    pub request_id: String,
    pub task_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub candidates: Vec<ExecutionPlanCandidate6>,
    pub peers: Vec<ExecutionPlanPeer5>,
    pub replay_identity: String,
    pub minimum_peer_quorum: u32,
    pub utility_margin_milli: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRoutingReceipt9 {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub task_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub score_milli_order: Vec<i32>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: String,
    pub routing_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error)]
pub enum FederatedExecutionCopilotError {
    #[error("invalid federated execution copilot field: {0}")]
    InvalidField(String),
    #[error("federated execution copilot artifact error: {0}")]
    Artifact(String),
    #[error("federated execution copilot serialization error: {0}")]
    Serialization(String),
}

impl ExecutionRoutingReceipt9 {
    pub fn validate(&self) -> Result<(), FederatedExecutionCopilotError> {
        let identity_ok = self.schema_version == RESEARCH_CONTRACT_SCHEMA_VERSION
            && self.feature_id == FEATURE_ID
            && self.contract_version == CONTRACT_VERSION
            && self.boundary == PRECLINICAL_BOUNDARY
            && self.raw_data_local
            && self.aggregate_only
            && !self.request_id.trim().is_empty()
            && !self.task_id.trim().is_empty()
            && !self.purpose.trim().is_empty()
            && !self.semantic_profile.trim().is_empty()
            && !self.replay_identity.trim().is_empty()
            && !self.disposition.trim().is_empty();
        if !identity_ok
            || !ordered(&self.required_study_order)
            || !ordered(&self.required_modality_order)
            || !ordered(&self.candidate_order)
            || !ordered(&self.selected_order)
            || !ordered(&self.unresolved_order)
            || !ordered(&self.blocked_order)
            || !ordered(&self.missing_study_order)
            || !ordered(&self.missing_modality_order)
            || !ordered(&self.qualified_peer_order)
            || !ordered(&self.missing_peer_order)
            || !ordered(&self.evidence_order)
            || !ordered(&self.omission_order)
            || !ordered(&self.uncertainty_order)
            || !ordered(&self.negative_evidence_order)
            || !ordered(&self.effect_order)
            || !ordered(&self.effect_receipts)
            || self.candidate_order.is_empty()
            || self.ranked_order.len() != self.candidate_order.len()
            || !same_set(&self.ranked_order, &self.candidate_order)
            || self.score_milli_order.len() != self.candidate_order.len()
            || !partition(
                &self.candidate_order,
                &self.selected_order,
                &self.unresolved_order,
                &self.blocked_order,
            )
            || !peer_partition(&self.qualified_peer_order, &self.missing_peer_order)
            || self.selected_order.len() > 1
            || self.effect_order.is_empty()
        {
            return Err(FederatedExecutionCopilotError::InvalidField(
                "identity, canonical orders, partitions, locality, or score vector are incomplete"
                    .into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedExecutionCopilotError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedExecutionCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedExecutionCopilotError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedExecutionCopilotError::Serialization(error.to_string()))
    }
}

pub fn federated_execution_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "routing".into(),
        consumers: [
            "research program lead".into(),
            "execution planner".into(),
            "federation steward".into(),
        ]
        .into(),
        behavior: "ranks typed institution-local execution plans using evidence, risk, replay identity, and federated peer quorum, then emits an approval-aware routing receipt without dispatching effects".into(),
        value: "turns federated computational execution into an auditable, reproducible choice while keeping raw experimental data local and unresolved evidence explicit".into(),
        inputs: vec![TypedPort { name: "execution_plan_set".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "execution_routing_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(),
        permissions: ["route:local-execution-plan".into(), "read:federated-aggregate".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
            EvidenceReference { source_id: "slsa".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn route_federated_execution(
    request: &FederatedExecutionCopilotRequest8,
) -> Result<ExecutionRoutingReceipt9, FederatedExecutionCopilotError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.plan_id.cmp(&b.plan_id));
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));

    let qualified_peers: Vec<&ExecutionPlanPeer5> = peers
        .iter()
        .filter(|peer| {
            peer.semantic_profile == request.semantic_profile
                && peer.replay_identity == request.replay_identity
                && peer.authorized
                && matches!(
                    peer.evidence_state,
                    ExecutionPlanEvidenceState::Proven | ExecutionPlanEvidenceState::Supported
                )
        })
        .collect();
    let qualified_peer_order: Vec<String> = qualified_peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect();
    let missing_peer_order: Vec<String> = peers
        .iter()
        .filter(|peer| !qualified_peer_order.iter().any(|id| id == &peer.peer_id))
        .map(|peer| peer.peer_id.clone())
        .collect();
    let candidate_order: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.plan_id.clone())
        .collect();
    let mut scores = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let base = candidate.expected_discovery_milli as i32 - (candidate.risk_milli as i32 / 2);
        let peer_values: Vec<u32> = qualified_peers
            .iter()
            .filter(|peer| peer.plan_id == candidate.plan_id)
            .map(|peer| peer.utility_milli)
            .collect();
        let peer_bonus = if peer_values.is_empty() {
            0
        } else {
            (peer_values.iter().sum::<u32>() / peer_values.len() as u32 / 10) as i32
        };
        scores.push(base + peer_bonus);
    }
    let mut ranked: Vec<usize> = (0..candidates.len()).collect();
    ranked.sort_by(|left, right| {
        scores[*right]
            .cmp(&scores[*left])
            .then_with(|| candidate_order[*left].cmp(&candidate_order[*right]))
    });
    let ranked_order: Vec<String> = ranked
        .iter()
        .map(|index| candidate_order[*index].clone())
        .collect();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut evidence = Vec::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative = Vec::new();
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        omissions.push(
            "policy, protected-closure, approval, federation, or locality gate denied routing"
                .into(),
        );
    } else {
        for candidate in &candidates {
            match candidate.evidence_state {
                ExecutionPlanEvidenceState::Contradicted => {
                    blocked.push(candidate.plan_id.clone());
                    negative.push(format!(
                        "contradicted execution evidence: {}",
                        candidate.plan_id
                    ));
                }
                ExecutionPlanEvidenceState::Unknown | ExecutionPlanEvidenceState::Unmeasured => {
                    unresolved.push(candidate.plan_id.clone());
                    evidence.push(format!(
                        "unresolved execution evidence: {}",
                        candidate.plan_id
                    ));
                    uncertainty.push(format!(
                        "plan evidence is not closed: {}",
                        candidate.plan_id
                    ));
                }
                ExecutionPlanEvidenceState::Proven | ExecutionPlanEvidenceState::Supported => {}
            }
        }
        let eligible: Vec<usize> = ranked
            .iter()
            .copied()
            .filter(|index| {
                matches!(
                    candidates[*index].evidence_state,
                    ExecutionPlanEvidenceState::Proven | ExecutionPlanEvidenceState::Supported
                )
            })
            .collect();
        if qualified_peers.len() < request.minimum_peer_quorum as usize {
            unresolved.extend(eligible.iter().map(|index| candidate_order[*index].clone()));
            omissions.push(format!(
                "peer quorum unresolved: required {}, qualified {}",
                request.minimum_peer_quorum,
                qualified_peers.len()
            ));
            uncertainty
                .push("federated utility is unresolved below the minimum peer quorum".into());
        } else if let Some(top) = eligible.first() {
            let margin = eligible
                .get(1)
                .map(|second| scores[*top] - scores[*second])
                .unwrap_or(i32::MAX);
            if margin >= request.utility_margin_milli as i32 {
                selected.push(candidate_order[*top].clone());
                unresolved.extend(
                    eligible
                        .iter()
                        .skip(1)
                        .map(|index| candidate_order[*index].clone()),
                );
            } else {
                unresolved.extend(eligible.iter().map(|index| candidate_order[*index].clone()));
                uncertainty.push(format!(
                    "utility margin unresolved: observed {}, required {}",
                    margin, request.utility_margin_milli
                ));
            }
        }
    }
    let missing_study: Vec<String> = request
        .required_study_order
        .iter()
        .filter(|required| {
            !candidates
                .iter()
                .any(|candidate| &candidate.study_id == *required)
        })
        .cloned()
        .collect();
    let missing_modality: Vec<String> = request
        .required_modality_order
        .iter()
        .filter(|required| {
            !candidates
                .iter()
                .any(|candidate| &candidate.modality == *required)
        })
        .cloned()
        .collect();
    if !missing_study.is_empty() || !missing_modality.is_empty() {
        omissions.extend(
            missing_study
                .iter()
                .map(|item| format!("required study unresolved: {item}")),
        );
        omissions.extend(
            missing_modality
                .iter()
                .map(|item| format!("required modality unresolved: {item}")),
        );
        uncertainty.push("required study or modality coverage is incomplete".into());
        unresolved.extend(selected.drain(..));
    }
    unresolved.sort();
    unresolved.dedup();
    blocked.sort();
    blocked.dedup();
    evidence.sort();
    omissions.sort();
    uncertainty.sort();
    negative.sort();
    let disposition =
        if global_block || !blocked.is_empty() && blocked.len() == candidate_order.len() {
            "blocked"
        } else if !missing_study.is_empty() || !missing_modality.is_empty() || selected.len() != 1 {
            "unresolved"
        } else {
            "qualified"
        };
    let effect_order = if disposition == "qualified" {
        vec![
            "manage:local-capability".into(),
            "route:execution-plan".into(),
        ]
    } else {
        vec!["block:execution-effects".into()]
    };
    let effect_receipts = effect_order
        .iter()
        .map(|effect| format!("{effect}:{}", request.request_id))
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "task_id": request.task_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "required_study_order": request.required_study_order,
        "required_modality_order": request.required_modality_order,
        "candidate_order": candidate_order,
        "ranked_order": ranked_order,
        "selected_order": selected,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "missing_study_order": missing_study,
        "missing_modality_order": missing_modality,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "score_milli_order": scores,
        "evidence_order": evidence,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "effect_order": effect_order,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let routing_digest = ContentHash::of_value(&payload)
        .map_err(|error| FederatedExecutionCopilotError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("routing-execution-copilot:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedExecutionCopilotError::Artifact(error.to_string()))?;
    let receipt = ExecutionRoutingReceipt9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        task_id: request.task_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        required_study_order: request.required_study_order.clone(),
        required_modality_order: request.required_modality_order.clone(),
        candidate_order,
        ranked_order,
        selected_order: selected,
        unresolved_order: unresolved,
        blocked_order: blocked,
        missing_study_order: missing_study,
        missing_modality_order: missing_modality,
        qualified_peer_order,
        missing_peer_order,
        score_milli_order: scores,
        evidence_order: evidence,
        omission_order: omissions,
        uncertainty_order: uncertainty,
        negative_evidence_order: negative,
        effect_order,
        replay_identity: request.replay_identity.clone(),
        routing_digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedExecutionCopilotRequest8,
) -> Result<(), FederatedExecutionCopilotError> {
    if request.request_id.trim().is_empty()
        || request.task_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.replay_identity.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_PLANS
        || request.peers.len() > MAX_PEERS
        || request.minimum_peer_quorum == 0
        || request.minimum_peer_quorum as usize > request.peers.len()
        || !ordered(&request.required_study_order)
        || !ordered(&request.required_modality_order)
    {
        return Err(FederatedExecutionCopilotError::InvalidField("request identity, canonical requirements, bounds, quorum, locality, or boundary are incomplete".into()));
    }
    let mut candidate_ids = Vec::new();
    for candidate in &request.candidates {
        if candidate.plan_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.semantic_profile.trim().is_empty()
            || candidate.replay_identity.trim().is_empty()
            || candidate.semantic_profile != request.semantic_profile
            || candidate.replay_identity != request.replay_identity
            || candidate.expected_discovery_milli > 1000
            || candidate.risk_milli > 1000
            || !candidate.local
            || !candidate.aggregate_only
        {
            return Err(FederatedExecutionCopilotError::InvalidField("candidate typed identity, semantic/replay parity, bounded score, or locality is invalid".into()));
        }
        candidate_ids.push(candidate.plan_id.clone());
    }
    candidate_ids.sort();
    if candidate_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(FederatedExecutionCopilotError::InvalidField(
            "candidate plan IDs must be unique".into(),
        ));
    }
    let mut peer_ids = Vec::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || peer.plan_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.replay_identity.trim().is_empty()
            || peer.utility_milli > 1000
            || !peer.local
            || !peer.aggregate_only
        {
            return Err(FederatedExecutionCopilotError::InvalidField(
                "peer typed identity, bounded utility, or locality is invalid".into(),
            ));
        }
        peer_ids.push(peer.peer_id.clone());
    }
    peer_ids.sort();
    if peer_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(FederatedExecutionCopilotError::InvalidField(
            "peer IDs must be unique".into(),
        ));
    }
    Ok(())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn same_set(left: &[String], right: &[String]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort();
    right.sort();
    left == right
}

fn partition(all: &[String], first: &[String], second: &[String], third: &[String]) -> bool {
    let mut parts = first.to_vec();
    parts.extend_from_slice(second);
    parts.extend_from_slice(third);
    parts.sort();
    parts.dedup();
    same_set(&parts, all)
}

fn peer_partition(qualified: &[String], missing: &[String]) -> bool {
    ordered(qualified) && ordered(missing) && qualified.iter().all(|id| !missing.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        plan_id: &str,
        evidence_state: ExecutionPlanEvidenceState,
        expected: u32,
        risk: u32,
    ) -> ExecutionPlanCandidate6 {
        ExecutionPlanCandidate6 {
            plan_id: plan_id.into(),
            workflow_digest: ContentHash::of_bytes(plan_id.as_bytes()),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            semantic_profile: "profile:ome-ngff".into(),
            expected_discovery_milli: expected,
            risk_milli: risk,
            evidence_state,
            artifact_digest: ContentHash::of_bytes(format!("artifact:{plan_id}").as_bytes()),
            provenance_digest: ContentHash::of_bytes(format!("prov:{plan_id}").as_bytes()),
            replay_identity: "replay:v1".into(),
            local: true,
            aggregate_only: true,
        }
    }

    fn request(candidates: Vec<ExecutionPlanCandidate6>) -> FederatedExecutionCopilotRequest8 {
        FederatedExecutionCopilotRequest8 {
            request_id: "request:routing".into(),
            task_id: "task:execution".into(),
            purpose: "run preclinical multimodal computation".into(),
            semantic_profile: "profile:ome-ngff".into(),
            required_study_order: vec!["study:organoid".into()],
            required_modality_order: vec!["imaging".into()],
            candidates,
            peers: vec![ExecutionPlanPeer5 {
                peer_id: "peer:a".into(),
                plan_id: "plan:a".into(),
                semantic_profile: "profile:ome-ngff".into(),
                utility_milli: 900,
                evidence_state: ExecutionPlanEvidenceState::Supported,
                artifact_digest: ContentHash::of_bytes(b"peer-artifact"),
                provenance_digest: ContentHash::of_bytes(b"peer-prov"),
                replay_identity: "replay:v1".into(),
                authorized: true,
                local: true,
                aggregate_only: true,
            }],
            replay_identity: "replay:v1".into(),
            minimum_peer_quorum: 1,
            utility_margin_milli: 10,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = federated_execution_copilot_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert!(manifest.validate().is_ok());
    }
    #[test]
    fn selects_clear_supported_plan() {
        let receipt = route_federated_execution(&request(vec![
            candidate("plan:a", ExecutionPlanEvidenceState::Supported, 950, 100),
            candidate("plan:b", ExecutionPlanEvidenceState::Proven, 700, 100),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.selected_order, vec!["plan:a"]);
        assert!(receipt.digest().is_ok());
    }
    #[test]
    fn unknown_is_unresolved() {
        let receipt = route_federated_execution(&request(vec![candidate(
            "plan:a",
            ExecutionPlanEvidenceState::Unknown,
            950,
            100,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert_eq!(receipt.unresolved_order, vec!["plan:a"]);
    }
    #[test]
    fn contradiction_is_blocked() {
        let receipt = route_federated_execution(&request(vec![candidate(
            "plan:a",
            ExecutionPlanEvidenceState::Contradicted,
            950,
            100,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.blocked_order, vec!["plan:a"]);
    }
    #[test]
    fn quorum_failure_is_unresolved() {
        let mut input = request(vec![candidate(
            "plan:a",
            ExecutionPlanEvidenceState::Supported,
            950,
            100,
        )]);
        input.minimum_peer_quorum = 2;
        input.peers.push(ExecutionPlanPeer5 {
            peer_id: "peer:b".into(),
            plan_id: "plan:a".into(),
            semantic_profile: "profile:other".into(),
            utility_milli: 900,
            evidence_state: ExecutionPlanEvidenceState::Supported,
            artifact_digest: ContentHash::of_bytes(b"peer-b-artifact"),
            provenance_digest: ContentHash::of_bytes(b"peer-b-prov"),
            replay_identity: "replay:v1".into(),
            authorized: true,
            local: true,
            aggregate_only: true,
        });
        let receipt = route_federated_execution(&input).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
    }
    #[test]
    fn digest_is_deterministic() {
        let receipt = route_federated_execution(&request(vec![candidate(
            "plan:a",
            ExecutionPlanEvidenceState::Supported,
            950,
            100,
        )]))
        .unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
