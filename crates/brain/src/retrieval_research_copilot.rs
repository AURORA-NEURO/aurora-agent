//! Local retrieval-and-synthesis research copilot.
//!
//! Atlas feature: `AFA-brain-P02-F09`. It compiles a bounded, replayable local research plan;
//! it never fetches external sources or upgrades unknown evidence into a conclusion.

use crate::retrieval_synthesis::{
    synthesize_retrieval, ScopedRetrievalQuery, SynthesisDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F09";
pub const CONTRACT_VERSION: &str = "brain-retrieval-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesisCopilot1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCopilotRequest {
    pub request: ScopedRetrievalQuery,
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
pub struct RetrievalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: SynthesisDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub synthesis_digest: ContentHash,
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
pub enum RetrievalCopilotError {
    #[error("invalid retrieval copilot request: {0}")]
    Invalid(String),
    #[error("retrieval copilot artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval copilot engine failed: {0}")]
    Engine(String),
}

impl RetrievalCopilotReceipt {
    pub fn validate(&self) -> Result<(), RetrievalCopilotError> {
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
            return Err(RetrievalCopilotError::Invalid(
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
            return Err(RetrievalCopilotError::Invalid(
                "copilot evidence state is not covered".into(),
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
                return Err(RetrievalCopilotError::Invalid(
                    "copilot ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalCopilotError::Invalid(
                "copilot effect is outside local gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, RetrievalCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalCopilotError::Artifact(error.to_string()))
    }
}

pub fn retrieval_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "retrieval agent developer".into()].into(), behavior: "compiles local retrieval synthesis into a bounded deterministic inspect-and-retain plan with explicit uncertainty and negative evidence".into(), value: "turns supported retrieval evidence into replayable researcher actions without external effects or silent unknown promotion".into(), inputs: vec![TypedPort { name: "retrieval_copilot_request".into(), schema: "ScopedRetrievalQuery1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis_copilot_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_retrieval_copilot(
    request: &RetrievalCopilotRequest,
) -> Result<RetrievalCopilotReceipt, RetrievalCopilotError> {
    validate_request(request)?;
    let synthesis = synthesize_retrieval(&request.request)
        .map_err(|error| RetrievalCopilotError::Engine(error.to_string()))?;
    let mut actions = BTreeSet::new();
    let mut plans = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for evidence_id in &synthesis.qualified_order {
        actions.insert(format!("action:inspect:{evidence_id}"));
        plans.insert(format!("plan:inspect:{evidence_id}"));
    }
    if synthesis.qualified_order.is_empty() {
        actions.insert("action:retain-unknown-evidence".into());
        plans.insert("plan:retain-unknown-evidence".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-local-evidence")
    {
        negative.insert("copilot:inspect-local-evidence-not-allowed".into());
    }
    if request.budget_units < actions.len() as u32 || actions.len() > request.max_actions {
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
        && request.budget_units >= actions.len() as u32
        && actions.len() <= request.max_actions
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local;
    let disposition = if !actionable {
        SynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let plan_order = plans.into_iter().collect::<Vec<_>>();
    let action_order = actions.into_iter().collect::<Vec<_>>();
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| RetrievalCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_order, "action_order": action_order, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| RetrievalCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "study_id": request.request.study_id, "scope": request.request.scope, "disposition": disposition, "plan_order": plan_order, "action_order": action_order, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "synthesis_digest": synthesis_digest, "plan_digest": plan_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-copilot:{}", request.request.request_id),
        "application/vnd.aurora.evidence-synthesis-copilot+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalCopilotError::Artifact(error.to_string()))?;
    let receipt = RetrievalCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.scope.clone(),
        disposition,
        plan_order,
        action_order,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        synthesis_digest,
        plan_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if actionable {
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

fn validate_request(request: &RetrievalCopilotRequest) -> Result<(), RetrievalCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > 64
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.candidates.is_empty()
    {
        return Err(RetrievalCopilotError::Invalid(
            "copilot operator, capacity, budget, candidates, or boundary is incomplete".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> RetrievalCopilotRequest {
        RetrievalCopilotRequest {
            request: ScopedRetrievalQuery {
                request_id: "request:copilot".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic phenotype".into(),
                minimum_support_milli: 700,
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
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
            },
            operator_id: "operator:research".into(),
            action_allow_list: vec!["inspect-local-evidence".into()],
            max_actions: 4,
            budget_units: 4,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = retrieval_research_copilot_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_plan_is_qualified() {
        let r = compile_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Qualified);
    }
    #[test]
    fn missing_action_permission_blocks() {
        let mut q = request();
        q.action_allow_list.clear();
        let r = compile_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn budget_blocks() {
        let mut q = request();
        q.budget_units = 0;
        assert!(compile_retrieval_copilot(&q).is_err());
    }
    #[test]
    fn protected_closure_blocks() {
        let mut q = request();
        q.protected_closure = false;
        let r = compile_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = compile_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
