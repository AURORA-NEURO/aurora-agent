//! Federated continual statistical/causal/ML analysis workflow fabric for Examples.
//!
//! Atlas feature: `AFA-examples-P13-F16`.
//!
//! The fabric compiles caller-supplied analysis attestations into a resumable workflow receipt.
//! It does not fit models, execute notebooks, access clinical data, or move raw preclinical
//! observations; a separately governed executor may consume a qualified plan.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-examples-P13-F16";
pub const CONTRACT_VERSION: &str =
    "examples-federated-continual-statistical-analysis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisWorkflowDraft5@1";
pub const OUTPUT_SCHEMA: &str = "AnalysisWorkflowRun8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub candidate_id: String,
    pub estimand: String,
    pub method_family: String,
    pub input_digest: ContentHash,
    pub output_digest: ContentHash,
    pub baseline_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub quality_score_milli: u32,
    pub local_data: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkflowDraft {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub candidates: Vec<AnalysisCandidate>,
    pub stage_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkflowRun {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub required_candidate_order: Vec<String>,
    pub selected_candidate_order: Vec<String>,
    pub pending_candidate_order: Vec<String>,
    pub blocked_candidate_order: Vec<String>,
    pub compensated_candidate_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub checkpoint_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub workflow_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalysisWorkflowError {
    #[error("invalid statistical analysis workflow draft: {0}")]
    Invalid(String),
    #[error("statistical analysis workflow artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl AnalysisWorkflowRun {
    pub fn validate(&self) -> Result<(), AnalysisWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.stage_order.is_empty()
            || self.required_candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(AnalysisWorkflowError::Invalid(
                "analysis workflow identity, stages, candidates, locality, aggregate boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.required_candidate_order,
            &self.selected_candidate_order,
            &self.pending_candidate_order,
            &self.blocked_candidate_order,
            &self.compensated_candidate_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(AnalysisWorkflowError::Invalid(
                    "analysis workflow ordering is not canonical".into(),
                ));
            }
        }
        if self.stage_order
            != vec![
                "admit".to_string(),
                "checkpoint".to_string(),
                "validate".to_string(),
                "schedule".to_string(),
                "retain-receipt".to_string(),
            ]
        {
            return Err(AnalysisWorkflowError::Invalid(
                "analysis workflow stage protocol is invalid".into(),
            ));
        }
        let required = self
            .required_candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .selected_candidate_order
            .iter()
            .chain(self.pending_candidate_order.iter())
            .chain(self.blocked_candidate_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if required.len() != self.required_candidate_order.len()
            || states.len() != required.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != required
            || self
                .compensated_candidate_order
                .iter()
                .any(|candidate| !required.contains(candidate))
        {
            return Err(AnalysisWorkflowError::Invalid(
                "analysis candidate states do not partition the required plan".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:analysis-workflow:")
                && !effect.starts_with("compensate:analysis-workflow:")
                && effect != "block:unsafe-release"
        }) {
            return Err(AnalysisWorkflowError::Invalid(
                "analysis workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, AnalysisWorkflowError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))?,
        )
        .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "examples".into(),
        consumers: BTreeSet::from([
            "research program lead".into(),
            "analysis workflow operator".into(),
            "federated evaluation gateway".into(),
        ]),
        behavior: "compiles typed statistical, causal, and ML analysis attestations into a resumable checkpointed workflow plan without executing models".into(),
        value: "makes analysis selection, evidence closure, compensation, and federation readiness auditable before computation".into(),
        inputs: vec![TypedPort {
            name: "analysis_workflow_draft".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "analysis_workflow_run".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]),
        permissions: BTreeSet::from([
            "schedule:analysis-workflows".into(),
            "exchange:aggregate-analysis-manifests".into(),
        ]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "cwl".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.commonwl.org/specification/".into()),
            },
            EvidenceReference {
                source_id: "opentelemetry".into(),
                state: EvidenceState::Supported,
                locator: Some("https://opentelemetry.io/docs/specs/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "analysis workflow operator".into(),
            reason: "approve scheduling and aggregate-only federation of analysis manifests".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure(draft: &AnalysisWorkflowDraft) -> Result<AnalysisWorkflowRun, AnalysisWorkflowError> {
    if draft.request_id.trim().is_empty()
        || draft.federation_id.trim().is_empty()
        || draft.purpose.trim().is_empty()
        || draft.semantic_profile.trim().is_empty()
        || draft.required_candidate_order.is_empty()
        || !canonical(&draft.required_candidate_order)
        || draft.stage_order
            != vec![
                "admit".to_string(),
                "checkpoint".to_string(),
                "validate".to_string(),
                "schedule".to_string(),
                "retain-receipt".to_string(),
            ]
        || !draft.raw_data_local
        || !draft.aggregate_only
        || draft.budget_units == 0
        || draft.max_budget_units == 0
        || draft.budget_units > draft.max_budget_units
        || draft.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(AnalysisWorkflowError::Invalid(
            "analysis workflow identity, stages, locality, aggregate boundary, or budget is invalid".into(),
        ));
    }
    let required = draft
        .required_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != draft.required_candidate_order.len() {
        return Err(AnalysisWorkflowError::Invalid(
            "required analysis candidates must be unique".into(),
        ));
    }
    let mut candidates = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut pending = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &draft.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !required.contains(&candidate.candidate_id)
            || !canonical(&candidate.omissions)
            || !canonical(&candidate.uncertainty)
            || candidate.estimand.trim().is_empty()
            || candidate.method_family.trim().is_empty()
            || candidate.semantic_profile != draft.semantic_profile
            || candidate.baseline_digest.is_none()
            || candidate.provenance_digest.is_none()
            || !candidate.local_data
            || candidate.quality_score_milli > 1000
            || !candidates.insert(candidate.candidate_id.clone())
        {
            return Err(AnalysisWorkflowError::Invalid(
                "analysis candidate identity, evidence, baseline, provenance, profile, quality, or locality is invalid".into(),
            ));
        }
        for item in &candidate.omissions {
            omissions.insert(format!("candidate:{}:{item}", candidate.candidate_id));
        }
        for item in &candidate.uncertainty {
            uncertainty.insert(format!("candidate:{}:{item}", candidate.candidate_id));
        }
        if candidate.negative_result {
            negative.insert(format!(
                "candidate:{}:negative-result",
                candidate.candidate_id
            ));
        }
        match candidate.evidence_state {
            EvidenceState::Proven | EvidenceState::Supported
                if candidate.quality_score_milli >= 700
                    && candidate.omissions.is_empty()
                    && candidate.uncertainty.is_empty() =>
            {
                selected.insert(candidate.candidate_id.clone());
            }
            EvidenceState::Contradicted => {
                blocked.insert(candidate.candidate_id.clone());
                negative.insert(format!("candidate:{}:contradicted", candidate.candidate_id));
            }
            _ => {
                pending.insert(candidate.candidate_id.clone());
                uncertainty.insert(format!(
                    "candidate:{}:not-qualified",
                    candidate.candidate_id
                ));
            }
        }
    }
    for candidate in required.difference(&candidates) {
        pending.insert((*candidate).clone());
        omissions.insert(format!("missing-candidate:{candidate}"));
        uncertainty.insert(format!("missing-candidate:{candidate}"));
    }
    if selected.len() + pending.len() + blocked.len() != required.len() {
        return Err(AnalysisWorkflowError::Invalid(
            "analysis candidate outcomes do not partition required candidates".into(),
        ));
    }
    let mut violations = BTreeSet::new();
    if !draft.policy_allow {
        violations.insert("policy".into());
    }
    if !draft.protected_closure {
        violations.insert("protected-closure".into());
    }
    if !draft.signed_approval {
        violations.insert("signed-approval".into());
    }
    if !draft.federation_approved {
        violations.insert("federation-approval".into());
    }
    for event in &draft.adversarial_events {
        violations.insert(format!("adversarial:{event}"));
        omissions.insert(format!("workflow:adversarial:{event}"));
    }
    let global_block = !violations.is_empty() || !draft.adversarial_events.is_empty();
    let disposition = if global_block {
        "blocked"
    } else if !pending.is_empty() || !blocked.is_empty() || !uncertainty.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(required.iter().cloned());
        selected.clear();
        pending.clear();
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let pending_order = pending.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let compensated_order = pending_order.clone();
    let checkpoint_payload = json!({
        "request_id": draft.request_id,
        "stage_order": draft.stage_order,
        "required_candidate_order": draft.required_candidate_order,
        "selected_candidate_order": selected_order,
        "pending_candidate_order": pending_order,
        "blocked_candidate_order": blocked_order,
        "replay_identity": draft.replay_identity,
    });
    let checkpoint_digest = ContentHash::of_value(&checkpoint_payload)
        .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({
        "checkpoint_digest": checkpoint_digest,
        "semantic_profile": draft.semantic_profile,
        "disposition": disposition,
    }))
    .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("examples-analysis-workflow:{}", draft.request_id),
        "application/vnd.aurora.analysis-workflow-run+json",
        &checkpoint_payload,
        Vec::<SemanticLoss>::new(),
        vec![ProvenanceLink {
            source_id: draft.federation_id.clone(),
            relation: "analysis-workflow-checkpoint".into(),
            digest: checkpoint_digest.clone(),
        }],
    )
    .map_err(|error| AnalysisWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "qualified" {
        vec![format!("schedule:analysis-workflow:{}", draft.request_id)]
    } else if disposition == "partial" {
        vec![format!("compensate:analysis-workflow:{}", draft.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let run = AnalysisWorkflowRun {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: draft.request_id.clone(),
        federation_id: draft.federation_id.clone(),
        purpose: draft.purpose.clone(),
        semantic_profile: draft.semantic_profile.clone(),
        disposition: disposition.into(),
        stage_order: draft.stage_order.clone(),
        required_candidate_order: draft.required_candidate_order.clone(),
        selected_candidate_order: selected_order,
        pending_candidate_order: pending_order,
        blocked_candidate_order: blocked_order,
        compensated_candidate_order: compensated_order,
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        checkpoint_digest,
        replay_identity: draft.replay_identity.clone(),
        workflow_digest,
        artifact,
        effect_receipts,
        raw_data_local: draft.raw_data_local,
        aggregate_only: draft.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"examples-analysis-workflow")
    }

    fn candidate(id: &str, state: EvidenceState) -> AnalysisCandidate {
        AnalysisCandidate {
            candidate_id: id.into(),
            estimand: "effect-on-organoid-growth".into(),
            method_family: "doubly-robust".into(),
            input_digest: hash(),
            output_digest: hash(),
            baseline_digest: Some(hash()),
            provenance_digest: Some(hash()),
            replay_identity: hash(),
            semantic_profile: "analysis:v1".into(),
            evidence_state: state,
            quality_score_milli: 900,
            local_data: true,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_result: false,
        }
    }

    fn draft() -> AnalysisWorkflowDraft {
        AnalysisWorkflowDraft {
            request_id: "request:examples-analysis".into(),
            federation_id: "federation:analysis".into(),
            purpose: "preclinical-replication".into(),
            semantic_profile: "analysis:v1".into(),
            required_candidate_order: vec!["analysis-a".into(), "analysis-b".into()],
            candidates: vec![
                candidate("analysis-a", EvidenceState::Supported),
                candidate("analysis-b", EvidenceState::Proven),
            ],
            stage_order: vec![
                "admit".into(),
                "checkpoint".into(),
                "validate".into(),
                "schedule".into(),
                "retain-receipt".into(),
            ],
            replay_identity: hash(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            budget_units: 10,
            max_budget_units: 10,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualified_workflow_schedules() {
        let run = assure(&draft()).unwrap();
        assert_eq!(run.disposition, "qualified");
        assert_eq!(
            run.selected_candidate_order,
            vec!["analysis-a", "analysis-b"]
        );
        assert!(run.effect_receipts[0].starts_with("schedule:analysis-workflow:"));
    }
    #[test]
    fn unknown_candidate_is_partial_and_compensated() {
        let mut value = draft();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        let run = assure(&value).unwrap();
        assert_eq!(run.disposition, "partial");
        assert!(run.pending_candidate_order.contains(&"analysis-a".into()));
        assert!(run.effect_receipts[0].starts_with("compensate:analysis-workflow:"));
    }
    #[test]
    fn missing_candidate_is_partial() {
        let mut value = draft();
        value.candidates.pop();
        let run = assure(&value).unwrap();
        assert_eq!(run.disposition, "partial");
        assert!(run
            .omission_order
            .iter()
            .any(|item| item.contains("missing-candidate")));
    }
    #[test]
    fn contradiction_blocks() {
        let mut value = draft();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        let run = assure(&value).unwrap();
        assert_eq!(run.disposition, "partial");
        assert!(run.blocked_candidate_order.contains(&"analysis-a".into()));
    }
    #[test]
    fn policy_and_adversarial_events_block() {
        let mut value = draft();
        value.policy_allow = false;
        value.adversarial_events = vec!["poisoned-model-card".into()];
        let run = assure(&value).unwrap();
        assert_eq!(run.disposition, "blocked");
        assert_eq!(run.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn manifest_is_a2_and_federated() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
}
