//! Federated continual mechanism-exploration workflow safety fabric.
//!
//! Atlas feature: `AFA-safety-P08-F16`.
//!
//! This module schedules only policy-approved, replayable research workflows. It ranks competing
//! mechanism candidates but never treats a ranking as a scientific conclusion. Unknown,
//! contradicted, unmeasured, unsafe-action, and incomplete-closure states remain visible, and
//! physical or clinical actions are rejected at the boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-safety-P08-F16";
pub const CONTRACT_VERSION: &str = "safety-federated-mechanism-workflow/1.0";
pub const MAX_CANDIDATES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub study_id: String,
    pub origin_institution: String,
    pub scope: String,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: Option<ContentHash>,
    pub support_score: u16,
    pub risk_score: u16,
    pub requested_actions: Vec<String>,
    pub state: WorkflowEvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub required_mechanism_ids: Vec<String>,
    pub candidates: Vec<MechanismCandidate>,
    pub allowed_actions: Vec<String>,
    pub autonomy_tier: String,
    pub checkpoint_id: String,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub budget: u64,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub disposition: SafetyDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub action_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub checkpoint_id: String,
    pub checkpoint_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub workflow_artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismWorkflowError {
    #[error("invalid mechanism workflow request: {0}")]
    Invalid(String),
    #[error("mechanism workflow artifact failed: {0}")]
    Artifact(String),
    #[error("mechanism workflow serialization failed: {0}")]
    Serialization(String),
}

impl MechanismWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MechanismWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MechanismWorkflowError::Invalid(
                "mechanism workflow identity, candidates, checkpoint, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.ranked_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.mechanism_order,
            &self.action_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismWorkflowError::Invalid(
                    "mechanism workflow ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.evidence_order, &self.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismWorkflowError::Invalid(
                    "mechanism workflow digest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .ranked_order
            .iter()
            .any(|id| !self.candidate_order.contains(id))
            || self
                .admitted_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MechanismWorkflowError::Invalid(
                "mechanism workflow state order is not covered by candidate order".into(),
            ));
        }
        self.workflow_artifact
            .validate_metadata()
            .map_err(|error| MechanismWorkflowError::Artifact(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, MechanismWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismWorkflowError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismWorkflowError::Serialization(error.to_string()))
    }
}

pub fn safety_workflow_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "safety".into(),
        consumers: [
            "consortium operator".into(),
            "research governance specialist".into(),
            "workflow verification engineer".into(),
        ]
        .into(),
        behavior: "ranks and schedules competing preclinical mechanism workflows only when evidence, replay, autonomy, policy, authority, and safety gates close".into(),
        value: "turns mechanism exploration into a resumable, fail-closed workflow without allowing unsafe actions or hiding negative research results".into(),
        inputs: vec![TypedPort {
            name: "mechanism_workflow_request".into(),
            schema: "MechanismWorkflowRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "mechanism_workflow_receipt".into(),
            schema: "MechanismWorkflowReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation, Effect::FederationExport].into(),
        permissions: [
            "execute:approved-workflows".into(),
            "schedule:research-work".into(),
            "exchange:permitted-summaries".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "cwl".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.commonwl.org/specification/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "consortium workflow steward".into(),
            reason: "approve bounded research-work scheduling and digest-only federation".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn orchestrate_mechanism_workflow(
    request: &MechanismWorkflowRequest,
) -> Result<MechanismWorkflowReceipt, MechanismWorkflowError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_score
            .cmp(&left.support_score)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let mut candidate_ids = BTreeSet::new();
    let mut ranked = Vec::new();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut mechanisms = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let allowed_actions = request
        .allowed_actions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut spent = 0_u64;
    for candidate in &candidates {
        candidate_ids.insert(candidate.candidate_id.clone());
        ranked.push(candidate.candidate_id.clone());
        let cost = candidate.candidate_id.len() as u64 + candidate.mechanism_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let actions_ok = candidate
            .requested_actions
            .iter()
            .all(|action| allowed_actions.contains(action));
        let autonomy_ok = matches!(request.autonomy_tier.as_str(), "A0" | "A1" | "A2");
        let safe_score = candidate.risk_score <= 500;
        let complete = candidate.scope == request.scope
            && candidate.evidence_digest.is_some()
            && candidate.provenance_digest.is_some()
            && candidate.replay_identity.is_some()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && autonomy_ok
            && actions_ok
            && safe_score
            && candidate.state == WorkflowEvidenceState::Supported
            && complete
            && budget_ok;
        if gate {
            let (Some(evidence_digest), Some(provenance_digest)) = (
                candidate.evidence_digest.clone(),
                candidate.provenance_digest.clone(),
            ) else {
                return Err(MechanismWorkflowError::Invalid(
                    "admitted mechanism candidate is missing a required digest".into(),
                ));
            };
            spent = spent.saturating_add(cost);
            admitted.insert(candidate.candidate_id.clone());
            mechanisms.insert(candidate.mechanism_id.clone());
            evidence.insert(evidence_digest);
            provenance.insert(provenance_digest);
            actions.extend(candidate.requested_actions.iter().cloned());
        } else {
            match candidate.state {
                WorkflowEvidenceState::Unknown | WorkflowEvidenceState::Unmeasured => {
                    unknown.insert(candidate.candidate_id.clone());
                    uncertainty.insert(
                        format!(
                            "candidate:{}:state-{:?}-not-admitted",
                            candidate.candidate_id, candidate.state
                        )
                        .to_ascii_lowercase(),
                    );
                }
                WorkflowEvidenceState::Contradicted => {
                    blocked.insert(candidate.candidate_id.clone());
                    negative.insert(format!(
                        "candidate:{}:contradicted-mechanism-retained",
                        candidate.candidate_id
                    ));
                }
                WorkflowEvidenceState::Supported => {
                    blocked.insert(candidate.candidate_id.clone());
                }
            }
            if !actions_ok {
                omissions.insert(format!(
                    "candidate:{}:requested-action-not-permitted",
                    candidate.candidate_id
                ));
            }
            if !autonomy_ok {
                omissions.insert(format!(
                    "request:autonomy-tier-{}-requires-human-gate",
                    request.autonomy_tier
                ));
            }
            if !safe_score {
                negative.insert(format!(
                    "candidate:{}:risk-score-exceeds-safety-ceiling",
                    candidate.candidate_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "candidate:{}:scope-mismatch",
                    candidate.candidate_id
                ));
            }
            if candidate.evidence_digest.is_none()
                || candidate.provenance_digest.is_none()
                || candidate.replay_identity.is_none()
            {
                omissions.insert(format!(
                    "candidate:{}:evidence-provenance-or-replay-missing",
                    candidate.candidate_id
                ));
            }
            if !candidate.omissions.is_empty() {
                omissions.extend(
                    candidate
                        .omissions
                        .iter()
                        .map(|value| format!("candidate:{}:{value}", candidate.candidate_id)),
                );
            }
            if !candidate.uncertainty.is_empty() {
                uncertainty.extend(
                    candidate
                        .uncertainty
                        .iter()
                        .map(|value| format!("candidate:{}:{value}", candidate.candidate_id)),
                );
            }
            negative.extend(candidate.negative_evidence.iter().cloned());
            if !budget_ok {
                omissions.insert(format!(
                    "candidate:{}:budget-ceiling-exceeded",
                    candidate.candidate_id
                ));
            }
        }
    }
    for mechanism_id in &request.required_mechanism_ids {
        if !mechanisms.contains(mechanism_id) {
            omissions.insert(format!(
                "mechanism:{mechanism_id}:required-but-not-admitted"
            ));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let candidate_order = candidate_ids.into_iter().collect::<Vec<_>>();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let mechanism_order = mechanisms.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let action_order = actions.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let hard_block = !request.policy_allow
        || !request.federation_allow
        || !request.signed_approval
        || !request.raw_data_local;
    let disposition = if hard_block {
        SafetyDisposition::Blocked
    } else if admitted_order.is_empty() {
        SafetyDisposition::Unknown
    } else if blocked_order.is_empty()
        && unknown_order.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
    {
        SafetyDisposition::Qualified
    } else {
        SafetyDisposition::Partial
    };
    let checkpoint_digest = ContentHash::of_value(&json!({
        "checkpoint_id": request.checkpoint_id,
        "workflow_id": request.workflow_id,
        "replay_identity": request.replay_identity,
        "ranked_order": ranked,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "disposition": disposition,
    }))
    .map_err(|error| MechanismWorkflowError::Serialization(error.to_string()))?;
    let payload = json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "ranked_order": ranked,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "mechanism_order": mechanism_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "action_order": action_order,
        "checkpoint_digest": checkpoint_digest,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let workflow_artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-workflow:{}", request.request_id),
        "application/vnd.aurora.mechanism-workflow+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| MechanismWorkflowError::Artifact(error.to_string()))?;
    let mut effect_receipts = if !admitted_order.is_empty()
        && request.policy_allow
        && request.federation_allow
        && request.raw_data_local
    {
        vec![format!("schedule:approved-workflow:{}", request.request_id)]
    } else {
        Vec::new()
    };
    effect_receipts.push(format!(
        "checkpoint:mechanism-workflow:{}",
        request.checkpoint_id
    ));
    if disposition != SafetyDisposition::Qualified {
        effect_receipts.push(format!("block:safety-workflow:{}", request.request_id));
    }
    effect_receipts.sort();
    let receipt = MechanismWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        candidate_order,
        ranked_order: ranked,
        admitted_order,
        blocked_order,
        unknown_order,
        mechanism_order,
        evidence_order,
        provenance_order,
        action_order,
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        checkpoint_digest,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        workflow_artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &MechanismWorkflowRequest) -> Result<(), MechanismWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_mechanism_ids.is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.allowed_actions.is_empty()
        || request.autonomy_tier.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_mechanism_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .allowed_actions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MechanismWorkflowError::Invalid(
            "mechanism workflow identity, scope, required mechanisms, actions, autonomy, checkpoint, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.origin_institution.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.requested_actions.is_empty()
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.candidate_id.clone())
            || candidate
                .requested_actions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(MechanismWorkflowError::Invalid(format!(
                "mechanism candidate {} is invalid or duplicated",
                candidate.candidate_id
            )));
        }
    }
    if request.required_mechanism_ids.iter().any(|id| {
        !request
            .candidates
            .iter()
            .any(|candidate| &candidate.mechanism_id == id)
    }) {
        return Err(MechanismWorkflowError::Invalid(
            "required mechanism closure references an unknown candidate".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(id: &str, state: WorkflowEvidenceState) -> MechanismCandidate {
        MechanismCandidate {
            candidate_id: id.into(),
            mechanism_id: format!("mechanism:{id}"),
            study_id: "study:organoid".into(),
            origin_institution: "site:alpha".into(),
            scope: "organoid:neural".into(),
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            replay_identity: Some(hash(&format!("candidate-replay:{id}"))),
            support_score: if id.ends_with('a') { 950 } else { 800 },
            risk_score: 200,
            requested_actions: vec!["schedule:research-work".into()],
            state,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<MechanismCandidate>) -> MechanismWorkflowRequest {
        MechanismWorkflowRequest {
            request_id: "request:mechanism-workflow".into(),
            workflow_id: "workflow:mechanism".into(),
            federation_id: "federation:commons".into(),
            scope: "organoid:neural".into(),
            required_mechanism_ids: vec!["mechanism:candidate:a".into()],
            candidates,
            allowed_actions: vec!["schedule:research-work".into()],
            autonomy_tier: "A2".into(),
            checkpoint_id: "checkpoint:mechanism:7".into(),
            replay_identity: hash("replay"),
            benchmark_digest: hash("benchmark"),
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            budget: 200,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_declares_a2_workflow_safety() {
        let manifest = safety_workflow_manifest();
        assert_eq!(manifest.capability_id, FEATURE_ID);
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
    }

    #[test]
    fn ranks_and_schedules_safe_supported_mechanisms() {
        let receipt = orchestrate_mechanism_workflow(&request(vec![
            candidate("candidate:b", WorkflowEvidenceState::Supported),
            candidate("candidate:a", WorkflowEvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, SafetyDisposition::Qualified);
        assert_eq!(receipt.admitted_order, vec!["candidate:a", "candidate:b"]);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("schedule:")));
    }

    #[test]
    fn unknown_candidate_retains_uncertainty() {
        let receipt = orchestrate_mechanism_workflow(&request(vec![candidate(
            "candidate:a",
            WorkflowEvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, SafetyDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown")));
    }

    #[test]
    fn unsafe_action_is_blocked() {
        let mut unsafe_candidate = candidate("candidate:a", WorkflowEvidenceState::Supported);
        unsafe_candidate.requested_actions = vec!["instrument:execute".into()];
        let receipt = orchestrate_mechanism_workflow(&request(vec![unsafe_candidate])).unwrap();
        assert_eq!(receipt.disposition, SafetyDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("action-not-permitted")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("block:")));
    }

    #[test]
    fn high_autonomy_tier_requires_human_gate() {
        let mut input = request(vec![candidate(
            "candidate:a",
            WorkflowEvidenceState::Supported,
        )]);
        input.autonomy_tier = "A3".into();
        let receipt = orchestrate_mechanism_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SafetyDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("autonomy-tier-A3")));
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = orchestrate_mechanism_workflow(&request(vec![
            candidate("candidate:a", WorkflowEvidenceState::Supported),
            candidate("candidate:a", WorkflowEvidenceState::Supported),
        ]));
        assert!(result.is_err());
    }
}
