//! Local evidence-surveillance research copilot.
//!
//! Atlas feature: `AFA-brain-P01-F09`. The copilot compiles caller-supplied evidence into a
//! bounded local plan; it does not fetch sources, invoke external tools, or make a decision
//! about a person, diagnosis, treatment, or enrollment.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F09";
pub const CONTRACT_VERSION: &str = "brain-evidence-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_ACTIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCopilotRequest {
    pub request: EvidenceFeedRequest,
    pub operator_id: String,
    pub action_allow_list: Vec<String>,
    pub max_actions: usize,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: EvidenceSurveillanceDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub evidence_receipt_digest: ContentHash,
    pub plan_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceCopilotError {
    #[error("invalid evidence copilot request: {0}")]
    Invalid(String),
    #[error("evidence copilot artifact failed: {0}")]
    Artifact(String),
    #[error("evidence copilot engine failed: {0}")]
    Engine(String),
}

impl EvidenceCopilotReceipt {
    pub fn validate(&self) -> Result<(), EvidenceCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.plan_order.is_empty()
            || self.action_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(EvidenceCopilotError::Invalid(
                "copilot identity, bounded plan, locality, budget, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(EvidenceCopilotError::Invalid(
                "copilot evidence state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.plan_order,
            &self.action_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceCopilotError::Invalid(
                    "copilot ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(EvidenceCopilotError::Invalid(
                "copilot effect is outside local read/compute gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceCopilotError::Artifact(error.to_string()))
    }
}

pub fn evidence_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "agent developer".into()].into(), behavior: "compiles local EvidenceFeed receipts into a bounded read/compute research plan without external effects".into(), value: "turns qualified evidence into replayable researcher actions while preserving uncertainty, omissions, and negative results".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed1@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_evidence_copilot(
    request: &EvidenceCopilotRequest,
) -> Result<EvidenceCopilotReceipt, EvidenceCopilotError> {
    validate_request(request)?;
    let evidence = surveil_evidence(&request.request)
        .map_err(|error| EvidenceCopilotError::Engine(error.to_string()))?;
    let mut action_order = BTreeSet::new();
    let mut plan_order = BTreeSet::new();
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for evidence_id in &evidence.qualified_order {
        action_order.insert(format!("action:inspect:{evidence_id}"));
        plan_order.insert(format!("plan:inspect:{evidence_id}"));
    }
    if evidence.qualified_order.is_empty() {
        action_order.insert("action:retain-unknown-evidence".into());
        plan_order.insert("plan:retain-unknown-evidence".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-local-evidence")
    {
        negative.insert("copilot:inspect-local-evidence-not-allowed".into());
    }
    if request.budget_units < action_order.len() as u32 {
        omissions.insert("copilot:action-budget-exhausted".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let actionable = request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-local-evidence")
        && request.budget_units >= action_order.len() as u32
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local;
    let disposition = if !actionable {
        EvidenceSurveillanceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let action_vec = action_order.into_iter().collect::<Vec<_>>();
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_vec, "action_order": action_vec, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| EvidenceCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "study_id": request.request.study_id, "scope": request.request.scope, "disposition": disposition, "plan_order": plan_vec, "action_order": action_vec, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "evidence_receipt_digest": evidence.digest().map_err(|error| EvidenceCopilotError::Engine(error.to_string()))?, "plan_digest": plan_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-copilot:{}", request.request.request_id),
        "application/vnd.aurora.qualified-evidence-set-3+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceCopilotError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceCopilotError::Engine(error.to_string()))?;
    let has_effect = actionable && !evidence.qualified_order.is_empty();
    let receipt = EvidenceCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.scope.clone(),
        disposition,
        plan_order: payload
            .get("plan_order")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default(),
        action_order: payload
            .get("action_order")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default(),
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        evidence_receipt_digest: evidence_digest,
        plan_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if has_effect {
            vec![format!(
                "read:local-research-artifacts:{}",
                request.request.request_id
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

fn validate_request(request: &EvidenceCopilotRequest) -> Result<(), EvidenceCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.action_allow_list.is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
    {
        return Err(EvidenceCopilotError::Invalid("copilot operator, bounded action allow-list, budget, replay, or boundary is incomplete".into()));
    }
    if request.request.observations.len() > request.max_actions.saturating_mul(64) {
        return Err(EvidenceCopilotError::Invalid(
            "copilot evidence feed exceeds bounded plan capacity".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> EvidenceCopilotRequest {
        EvidenceCopilotRequest {
            request: EvidenceFeedRequest {
                request_id: "request:copilot".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_relevance_milli: 700,
                observations,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:researcher".into(),
            action_allow_list: vec!["inspect-local-evidence".into()],
            max_actions: 8,
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = evidence_research_copilot_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_evidence_compiles_to_bounded_plan() {
        let receipt =
            compile_evidence_copilot(&request(vec![observation("a", EvidenceState::Supported)]))
                .unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Qualified
        );
        assert!(receipt
            .action_order
            .iter()
            .any(|item| item.contains("inspect")));
    }
    #[test]
    fn unknown_evidence_is_retained() {
        let receipt =
            compile_evidence_copilot(&request(vec![observation("a", EvidenceState::Unknown)]))
                .unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Unknown
        );
        assert!(!receipt.unknown_order.is_empty());
    }
    #[test]
    fn action_allow_list_denial_blocks() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.action_allow_list = vec!["write-external".into()];
        let receipt = compile_evidence_copilot(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
    }
    #[test]
    fn budget_exhaustion_blocks() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.budget_units = 0;
        assert!(compile_evidence_copilot(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.replay_identity = hash("different");
        assert!(compile_evidence_copilot(&input).is_err());
    }
}
