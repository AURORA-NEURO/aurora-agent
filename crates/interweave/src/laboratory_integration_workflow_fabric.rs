//! Governed multimodal laboratory-integration workflow fabric.
//!
//! Atlas feature: `AFA-interweave-P11-F14`.
//!
//! This module coordinates a typed instrument-action plan across comparable preclinical studies.
//! It is deliberately a workflow boundary rather than a hardware driver: preflight, approval,
//! checkpoint, compensation, replay, and release receipts are produced deterministically, while
//! every physical action remains pending for an institution's separately signed gateway.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-interweave-P11-F14";
pub const CONTRACT_VERSION: &str =
    "interweave-multimodal-laboratory-integration-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "InstrumentActionRequest2@1";
pub const OUTPUT_SCHEMA: &str = "InstrumentActionReceipt4@1";
pub const STAGES: [&str; 5] = [
    "preflight",
    "validate-closure",
    "schedule",
    "checkpoint",
    "retain-receipt",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyBinding {
    pub study_id: String,
    pub modality_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAction {
    pub action_id: String,
    pub study_id: String,
    pub modality: String,
    pub instrument_id: String,
    pub operation: String,
    pub resource: String,
    pub cost_units: u32,
    pub evidence_digest: Option<ContentHash>,
    pub reversible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub studies: Vec<StudyBinding>,
    pub required_modalities: Vec<String>,
    pub actions: Vec<InstrumentAction>,
    pub stage_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub expected_comparability_digest: ContentHash,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub action_order: Vec<String>,
    pub scheduled_order: Vec<String>,
    pub pending_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub decisions: Vec<serde_json::Value>,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LaboratoryWorkflowError {
    #[error("invalid laboratory workflow: {0}")]
    Invalid(String),
    #[error("laboratory workflow artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InstrumentActionReceipt {
    pub fn validate(&self) -> Result<(), LaboratoryWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.stage_order != STAGES
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.study_order.is_empty()
            || self.action_order.is_empty()
            || self.decisions.len() != self.action_order.len()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.effect_receipts.is_empty()
        {
            return Err(LaboratoryWorkflowError::Invalid(
                "workflow identity, stages, plans, locality, boundary, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.action_order,
            &self.scheduled_order,
            &self.pending_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.missing_modality_order,
            &self.incomparable_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(LaboratoryWorkflowError::Invalid(
                    "workflow order or evidence certificate is not canonical".into(),
                ));
            }
        }
        if self.decisions.iter().enumerate().any(|(index, value)| {
            value.get("action_id").and_then(serde_json::Value::as_str)
                != self.action_order.get(index).map(String::as_str)
        }) {
            return Err(LaboratoryWorkflowError::Invalid(
                "workflow decisions do not match action order".into(),
            ));
        }
        let partition = self
            .scheduled_order
            .iter()
            .chain(self.pending_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.action_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.action_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(LaboratoryWorkflowError::Invalid(
                "scheduled, pending, and blocked actions do not partition the plan".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:")
                && effect != "approval-required:instrument"
                && effect != "block:unsafe-release"
        }) {
            return Err(LaboratoryWorkflowError::Invalid(
                "workflow effect is outside the schedule and compensation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, LaboratoryWorkflowError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))?,
        )
        .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "interweave".into(),
        consumers: BTreeSet::from([
            "laboratory automation engineer".into(),
            "instrument gateway".into(),
            "research workflow operator".into(),
        ]),
        behavior: "preflights and schedules governed multimodal instrument actions without executing hardware".into(),
        value: "preserves study/modality closure, checkpoints, compensation, approval, replay, and negative evidence before a physical gateway is authorized".into(),
        inputs: vec![TypedPort { name: "instrument_action_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "instrument_action_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["execute:approved-workflows".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "instrument-operator".into(), reason: "physical execution remains separately authorized after preflight".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Protocol, ResearchSurface::Api, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &InstrumentActionRequest) -> Result<(), LaboratoryWorkflowError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.studies.is_empty()
        || request.required_modalities.is_empty()
        || request.actions.is_empty()
        || request.stage_order != STAGES
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LaboratoryWorkflowError::Invalid(
            "request identity, stages, study/action closure, budget, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&request.required_modalities)
        || request
            .required_modalities
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(LaboratoryWorkflowError::Invalid(
            "required modality order is not canonical".into(),
        ));
    }
    let mut studies = BTreeSet::new();
    for study in &request.studies {
        if study.study_id.trim().is_empty()
            || !studies.insert(study.study_id.clone())
            || !canonical(&study.modality_order)
            || study.modality_order.is_empty()
            || study.artifact_digest.is_none()
            || study.provenance_digest.is_none()
        {
            return Err(LaboratoryWorkflowError::Invalid(
                "study identities, modalities, artifact, or provenance are incomplete".into(),
            ));
        }
    }
    let mut actions = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || !actions.insert(action.action_id.clone())
            || action.study_id.trim().is_empty()
            || !studies.contains(&action.study_id)
            || action.modality.trim().is_empty()
            || action.instrument_id.trim().is_empty()
            || action.operation.trim().is_empty()
            || action.resource.trim().is_empty()
            || (!action.reversible && action.evidence_digest.is_none())
        {
            return Err(LaboratoryWorkflowError::Invalid(
                "action identity, study binding, operation, or irreversible evidence is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub fn orchestrate(
    request: &InstrumentActionRequest,
) -> Result<InstrumentActionReceipt, LaboratoryWorkflowError> {
    validate_request(request)?;
    let mut study_order = request
        .studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    study_order.sort();
    let modality_order = request.required_modalities.clone();
    let mut action_order = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    action_order.sort();
    let studies = request
        .studies
        .iter()
        .map(|study| (study.study_id.clone(), study))
        .collect::<BTreeMap<_, _>>();
    let mut missing_modalities = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    for study in &request.studies {
        for modality in request
            .required_modalities
            .iter()
            .filter(|modality| !study.modality_order.contains(modality))
        {
            missing_modalities.insert(format!("{}:{}", study.study_id, modality));
            omissions.insert(format!("{}:missing-modality:{}", study.study_id, modality));
        }
        if study.comparability_digest != request.expected_comparability_digest {
            incomparable.insert(study.study_id.clone());
            omissions.insert(format!("{}:comparability-mismatch", study.study_id));
        }
        for item in &study.omissions {
            omissions.insert(format!("{}:{}", study.study_id, item));
        }
        for item in &study.uncertainty {
            uncertainty.insert(format!("{}:{}", study.study_id, item));
        }
        if study.negative_result {
            negative.insert(format!("{}:negative-result", study.study_id));
        } else {
            negative.insert(format!("{}:negative-result-not-observed", study.study_id));
        }
        if matches!(
            study.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            uncertainty.insert(format!("{}:evidence-state", study.study_id));
        }
        if study.evidence_state == EvidenceState::Contradicted {
            semantic_loss.push(SemanticLoss {
                field: format!("study:{}", study.study_id),
                reason: "contradicted study cannot authorize physical action".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    let global_failures = [
        (!request.policy_allow, "policy-denied"),
        (!request.protected_closure, "protected-closure-incomplete"),
        (!request.signed_approval, "signed-approval-missing"),
        (!request.raw_data_local, "raw-data-locality-failed"),
        (!request.adversarial_events.is_empty(), "adversarial-event"),
    ]
    .into_iter()
    .filter(|(failed, _)| *failed)
    .map(|(_, reason)| reason)
    .collect::<Vec<_>>();
    let data_omissions = !omissions.is_empty();
    for reason in &global_failures {
        omissions.insert(format!("workflow:{reason}"));
    }
    let study_blocked = request
        .studies
        .iter()
        .any(|study| study.evidence_state == EvidenceState::Contradicted);
    let closure_incomplete = !missing_modalities.is_empty()
        || !incomparable.is_empty()
        || data_omissions
        || !uncertainty.is_empty()
        || study_blocked;
    let approval_only = !global_failures.is_empty()
        && global_failures
            .iter()
            .all(|reason| *reason == "signed-approval-missing")
        && !closure_incomplete;
    let disposition = if !global_failures.is_empty() && !approval_only {
        "blocked"
    } else if approval_only {
        "approval_required"
    } else if closure_incomplete {
        "partial"
    } else {
        "qualified"
    };
    let mut scheduled = Vec::new();
    let mut pending = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut spent = 0_u32;
    let mut decisions = Vec::new();
    for action_id in &action_order {
        let action = request
            .actions
            .iter()
            .find(|action| &action.action_id == action_id)
            .expect("validated action");
        let mut failed = global_failures
            .iter()
            .map(|reason| reason.to_string())
            .collect::<BTreeSet<_>>();
        let mut conditional = BTreeSet::new();
        let study = studies[&action.study_id];
        if !study.modality_order.contains(&action.modality) {
            conditional.insert("action-modality-missing".to_string());
        }
        if request
            .required_modalities
            .iter()
            .any(|modality| !study.modality_order.contains(modality))
        {
            conditional.insert("study-modality-closure-incomplete".to_string());
        }
        if study.comparability_digest != request.expected_comparability_digest {
            conditional.insert("study-incomparable".to_string());
        }
        if !study.omissions.is_empty() {
            conditional.insert("study-omissions".to_string());
        }
        if !study.uncertainty.is_empty() {
            conditional.insert("study-uncertainty".to_string());
        }
        if matches!(
            study.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            conditional.insert("evidence-state-not-qualified".to_string());
        }
        if study.evidence_state == EvidenceState::Contradicted {
            failed.insert("contradicted-evidence".into());
        }
        if action.cost_units > request.budget_units.saturating_sub(spent) {
            conditional.insert("budget-ceiling".into());
        }
        let action_disposition = if !failed.is_empty() {
            blocked.insert(action_id.clone());
            "blocked"
        } else if !conditional.is_empty() {
            pending.push(action_id.clone());
            "pending"
        } else {
            spent = spent.saturating_add(action.cost_units);
            scheduled.push(action_id.clone());
            "scheduled"
        };
        decisions.push(json!({"action_id": action_id, "study_id": action.study_id, "modality": action.modality, "disposition": action_disposition, "failed_gates": failed.into_iter().collect::<Vec<_>>(), "conditional_gates": conditional.into_iter().collect::<Vec<_>>(), "cost_units": action.cost_units}));
    }
    let mut pending_order = pending;
    pending_order.sort();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let compensation_order = pending_order
        .iter()
        .chain(blocked_order.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| format!("compensate:{id}"))
        .collect::<Vec<_>>();
    let stage_payload = json!({"feature_id": FEATURE_ID, "workflow_id": request.workflow_id, "stage_order": STAGES, "study_order": study_order, "action_order": action_order, "scheduled_order": scheduled, "pending_order": pending_order, "blocked_order": blocked_order, "replay_identity": request.replay_identity});
    let workflow_digest = ContentHash::of_value(&stage_payload)
        .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_digest": workflow_digest, "checkpoint": "pre-physical-execution", "spent_units": spent})).map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))?;
    let artifact_payload = json!({"schema_version": OUTPUT_SCHEMA, "request_id": request.request_id, "workflow_id": request.workflow_id, "disposition": disposition, "stage_order": STAGES, "scheduled_order": scheduled, "pending_order": pending_order, "blocked_order": blocked_order, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity});
    let artifact_digest = ContentHash::of_value(&artifact_payload)
        .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("instrument-action-receipt:{}", request.request_id),
        "application/vnd.aurora.instrument-action-receipt+json",
        &artifact_payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.workflow_id.clone(),
            relation: "laboratory-integration-workflow".into(),
            digest: artifact_digest.clone(),
        }],
    )
    .map_err(|error| LaboratoryWorkflowError::Artifact(error.to_string()))?;
    let mut effect_receipts = if disposition == "qualified" {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    } else if disposition == "approval_required" {
        vec!["approval-required:instrument".into()]
    } else {
        let mut effects = vec!["block:unsafe-release".into()];
        effects.extend(compensation_order.iter().cloned());
        effects
    };
    effect_receipts.sort();
    let receipt = InstrumentActionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        stage_order: STAGES.iter().map(|value| (*value).into()).collect(),
        study_order,
        modality_order,
        action_order,
        scheduled_order: scheduled,
        pending_order,
        blocked_order,
        compensation_order,
        missing_modality_order: missing_modalities.into_iter().collect(),
        incomparable_order: incomparable.into_iter().collect(),
        decisions,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"interweave-lab")
    }
    fn study(id: &str, state: EvidenceState) -> StudyBinding {
        StudyBinding {
            study_id: id.into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            comparability_digest: hash(),
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            evidence_state: state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_result: false,
        }
    }
    fn request() -> InstrumentActionRequest {
        InstrumentActionRequest {
            request_id: "request:lab".into(),
            workflow_id: "workflow:lab".into(),
            scope: "organoid-resilience".into(),
            semantic_profile: "ome-ngff+anndata".into(),
            schema_version: INPUT_SCHEMA.into(),
            studies: vec![
                study("study-a", EvidenceState::Supported),
                study("study-b", EvidenceState::Proven),
            ],
            required_modalities: vec!["imaging".into(), "omics".into()],
            actions: vec![
                InstrumentAction {
                    action_id: "action-b".into(),
                    study_id: "study-b".into(),
                    modality: "omics".into(),
                    instrument_id: "sequencer-1".into(),
                    operation: "capture".into(),
                    resource: "lane".into(),
                    cost_units: 2,
                    evidence_digest: Some(hash()),
                    reversible: false,
                },
                InstrumentAction {
                    action_id: "action-a".into(),
                    study_id: "study-a".into(),
                    modality: "imaging".into(),
                    instrument_id: "microscope-1".into(),
                    operation: "capture".into(),
                    resource: "minute".into(),
                    cost_units: 2,
                    evidence_digest: None,
                    reversible: true,
                },
            ],
            stage_order: STAGES.iter().map(|value| (*value).into()).collect(),
            replay_identity: hash(),
            expected_comparability_digest: hash(),
            budget_units: 10,
            max_budget_units: 10,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_workflow_schedules_without_hardware_effect() {
        let receipt = orchestrate(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.scheduled_order, vec!["action-a", "action-b"]);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("schedule:research-work:")));
    }
    #[test]
    fn missing_modality_is_partial_and_compensated() {
        let mut value = request();
        value.studies[0].modality_order = vec!["imaging".into()];
        let receipt = orchestrate(&value).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert!(receipt
            .missing_modality_order
            .iter()
            .any(|item| item == "study-a:omics"));
        assert!(receipt
            .compensation_order
            .iter()
            .any(|item| item == "compensate:action-a"));
    }
    #[test]
    fn unknown_and_contradiction_never_schedule() {
        let mut value = request();
        value.studies[0].evidence_state = EvidenceState::Unknown;
        value.studies[1].evidence_state = EvidenceState::Contradicted;
        let receipt = orchestrate(&value).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert!(receipt.pending_order.contains(&"action-a".into()));
        assert!(receipt.blocked_order.contains(&"action-b".into()));
    }
    #[test]
    fn approval_is_required_before_physical_schedule() {
        let mut value = request();
        value.signed_approval = false;
        let receipt = orchestrate(&value).unwrap();
        assert_eq!(receipt.disposition, "approval_required");
        assert_eq!(
            receipt.effect_receipts,
            vec!["approval-required:instrument"]
        );
    }
    #[test]
    fn policy_or_adversarial_failure_blocks_and_replays() {
        let mut value = request();
        value.policy_allow = false;
        value.adversarial_events = vec!["prompt-injection".into()];
        let receipt = orchestrate(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn capability_manifest_is_a2_and_local() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.surfaces.contains(&ResearchSurface::Operator));
    }
}
