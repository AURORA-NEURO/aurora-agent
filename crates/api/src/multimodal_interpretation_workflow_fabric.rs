//! Multimodal multi-study interpretation workflow fabric.
//!
//! Atlas feature: `AFA-api-P14-F14`.
//!
//! This API-owned fabric is a deterministic orchestration boundary over caller-supplied study
//! summaries. It does not interpret raw images or omics, contact an external provider, or make a
//! clinical decision. It only admits a typed, comparable, provenance-complete set of research
//! artifacts into an `InteractiveInterpretation4` release candidate and retains every omission.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-api-P14-F14";
pub const CONTRACT_VERSION: &str = "api-multimodal-multi-study-interpretation-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult2@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation4@1";

const STAGES: [&str; 4] = [
    "compile-context",
    "compare-studies",
    "render-interpretation",
    "retain-receipt",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationStudy {
    pub study_id: String,
    pub modality_order: Vec<String>,
    pub interpretation_score_milli: u32,
    pub evidence_state: EvidenceState,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub comparability_digest: Option<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationWorkflowRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub query: String,
    pub input_schema: String,
    pub studies: Vec<InterpretationStudy>,
    pub required_modalities: Vec<String>,
    pub expected_comparability_digest: ContentHash,
    pub min_studies: u32,
    pub max_panels: usize,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub query: String,
    pub disposition: InterpretationDisposition,
    pub stage_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub comparability_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationWorkflowError {
    #[error("invalid multimodal interpretation workflow: {0}")]
    Invalid(String),
    #[error("multimodal interpretation artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InterpretationWorkflowReceipt {
    pub fn validate(&self) -> Result<(), InterpretationWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.query.trim().is_empty()
            || self.stage_order
                != STAGES
                    .iter()
                    .map(|stage| (*stage).to_string())
                    .collect::<Vec<_>>()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.rank_order.len() != self.study_order.len()
            || self.action_receipts.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(InterpretationWorkflowError::Invalid(
                "identity, stages, locality, studies, modalities, checks, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.incomparable_order,
            &self.missing_modality_order,
            &self.panel_order,
            &self.action_receipts,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(InterpretationWorkflowError::Invalid(
                    "interpretation orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let studies = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        if self.rank_order.iter().cloned().collect::<BTreeSet<_>>() != studies {
            return Err(InterpretationWorkflowError::Invalid(
                "rank order is not a study permutation".into(),
            ));
        }
        let mut partition = BTreeSet::<String>::new();
        for id in self
            .qualified_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
        {
            if !partition.insert(id.clone()) || !studies.contains(id) {
                return Err(InterpretationWorkflowError::Invalid(
                    "study disposition partition is duplicated or out of scope".into(),
                ));
            }
        }
        if partition != studies {
            return Err(InterpretationWorkflowError::Invalid(
                "study disposition partition is incomplete".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:interpretation-workflow:")
                && effect != "block:unsafe-release"
        }) {
            return Err(InterpretationWorkflowError::Invalid(
                "effect is outside the interpretation workflow gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationWorkflowError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))?,
        )
        .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "api".into(),
        consumers: BTreeSet::from([
            "multimodal research workbench".into(),
            "interpretation reviewer".into(),
            "downstream publication-release gate".into(),
        ]),
        behavior: "orchestrates comparable multimodal multi-study interpretation artifacts through a typed, policy-bounded workflow".into(),
        value: "turns evidence-backed imaging and omics summaries into replayable interpretation work products without silently hiding missing modalities or uncertainty".into(),
        inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["execute:approved-workflows".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ome-ngff".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) },
            EvidenceReference { source_id: "anndata".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "interpretation-reviewer".into(), reason: "multimodal interpretation workflow release".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator, ResearchSurface::Ui]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &InterpretationWorkflowRequest,
) -> Result<(), InterpretationWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.query.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.studies.is_empty()
        || request.required_modalities.is_empty()
        || request.min_studies == 0
        || request.max_panels == 0
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationWorkflowError::Invalid(
            "identity, schema, studies, modality closure, bounded budget, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&request.required_modalities)
        || request
            .required_modalities
            .iter()
            .any(|modality| modality.trim().is_empty())
    {
        return Err(InterpretationWorkflowError::Invalid(
            "required modalities must be unique, non-empty, and canonical".into(),
        ));
    }
    let mut ids = request
        .studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1])
        || request.studies.iter().any(|study| {
            study.study_id.trim().is_empty()
                || !canonical(&study.modality_order)
                || study
                    .modality_order
                    .iter()
                    .any(|modality| modality.trim().is_empty())
        })
    {
        return Err(InterpretationWorkflowError::Invalid(
            "study identifiers or modality orders are invalid".into(),
        ));
    }
    if request
        .adversarial_events
        .iter()
        .any(|event| event.trim().is_empty())
    {
        return Err(InterpretationWorkflowError::Invalid(
            "adversarial event labels must be non-empty".into(),
        ));
    }
    Ok(())
}

pub fn run(
    request: &InterpretationWorkflowRequest,
) -> Result<InterpretationWorkflowReceipt, InterpretationWorkflowError> {
    validate_request(request)?;
    let required_modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let modality_order = studies
        .iter()
        .flat_map(|study| study.modality_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let global_failed = [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("signed-approval", !request.signed_approval),
        ("raw-data-locality", !request.raw_data_local),
        ("adversarial-input", !request.adversarial_events.is_empty()),
    ]
    .into_iter()
    .filter_map(|(gate, failed)| failed.then_some(gate.to_string()))
    .collect::<BTreeSet<_>>();
    let mut scores = BTreeMap::new();
    let mut qualified = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut incomparable = BTreeSet::new();
    let mut missing_modality = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::new();
    for study in &studies {
        let mut failed = global_failed.clone();
        let mut conditional = BTreeSet::<String>::new();
        let modalities = study
            .modality_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for modality in required_modalities.difference(&modalities) {
            missing_modality.insert(format!("{}:{modality}", study.study_id));
            conditional.insert("required-modality-missing".into());
            omissions.insert(format!("{}:missing-modality:{modality}", study.study_id));
        }
        if study.artifact_digest.is_none() {
            conditional.insert("artifact-digest-missing".into());
            omissions.insert(format!("{}:artifact-digest-missing", study.study_id));
        }
        if study.provenance_digest.is_none() {
            conditional.insert("provenance-missing".into());
            omissions.insert(format!("{}:provenance-missing", study.study_id));
        }
        if study.comparability_digest.as_ref() != Some(&request.expected_comparability_digest) {
            conditional.insert("cross-study-incomparability".into());
            incomparable.insert(study.study_id.clone());
            omissions.insert(format!("{}:comparability-mismatch", study.study_id));
        }
        if !study.omissions.is_empty() {
            conditional.insert("study-omissions".into());
            omissions.extend(
                study
                    .omissions
                    .iter()
                    .map(|item| format!("{}:{item}", study.study_id)),
            );
        }
        if !study.uncertainty.is_empty() {
            conditional.insert("study-uncertainty".into());
            uncertainty.extend(
                study
                    .uncertainty
                    .iter()
                    .map(|item| format!("{}:{item}", study.study_id)),
            );
        }
        match study.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
                negative.insert(format!("{}:contradicted", study.study_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                conditional.insert("evidence-state-not-qualified".into());
                uncertainty.insert(format!("{}:evidence-state", study.study_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        negative.insert(format!(
            "{}:{}",
            study.study_id,
            if study.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        let score = study.interpretation_score_milli as i64
            + match study.evidence_state {
                EvidenceState::Proven => 20_000,
                EvidenceState::Supported => 10_000,
                _ => 0,
            }
            - conditional.len() as i64 * 500;
        scores.insert(study.study_id.clone(), score);
        let disposition = if !failed.is_empty() {
            blocked.push(study.study_id.clone());
            "blocked"
        } else if !conditional.is_empty() {
            unresolved.push(study.study_id.clone());
            "unresolved"
        } else {
            qualified.push(study.study_id.clone());
            "qualified"
        };
        decisions.push(json!({
            "study_id": study.study_id,
            "score_milli": score,
            "disposition": disposition,
            "failed_gates": failed.clone().into_iter().collect::<Vec<_>>(),
            "conditional_gates": conditional.into_iter().collect::<Vec<_>>(),
            "negative_result": study.negative_result,
        }));
        if !failed.is_empty() {
            semantic_loss.push(SemanticLoss {
                field: format!("study:{}", study.study_id),
                reason:
                    "study cannot enter a qualified multimodal interpretation after a failed gate"
                        .into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    let rank_order = study_order.iter().cloned().sorted_by(|left, right| {
        scores[right]
            .cmp(&scores[left])
            .then_with(|| left.cmp(right))
    });
    let mut selected = Vec::new();
    let mut spent = 0_u32;
    for study_id in &rank_order {
        if !qualified.contains(study_id) {
            continue;
        }
        if selected.len() >= request.max_panels {
            unresolved.push(study_id.clone());
            omissions.insert(format!("{study_id}:panel-capacity"));
            continue;
        }
        let cost = studies
            .iter()
            .find(|study| &study.study_id == study_id)
            .map(|study| study.modality_order.len() as u32 + 1)
            .unwrap_or(1);
        if cost > request.budget_units.saturating_sub(spent) {
            unresolved.push(study_id.clone());
            omissions.insert(format!("{study_id}:budget-ceiling"));
        } else {
            spent = spent.saturating_add(cost);
            selected.push(study_id.clone());
        }
    }
    selected.sort();
    selected.dedup();
    qualified.retain(|study_id| selected.contains(study_id));
    unresolved.sort();
    unresolved.dedup();
    blocked.sort();
    blocked.dedup();
    let selected_study_count = selected.len() as u32;
    if selected_study_count < request.min_studies {
        omissions.insert(format!(
            "study-quorum:{selected_study_count}/{}",
            request.min_studies
        ));
    }
    let disposition = if !global_failed.is_empty() || !blocked.is_empty() {
        InterpretationDisposition::Blocked
    } else if !unresolved.is_empty() || selected_study_count < request.min_studies {
        InterpretationDisposition::Unresolved
    } else {
        InterpretationDisposition::Qualified
    };
    let action_receipts = if matches!(disposition, InterpretationDisposition::Qualified) {
        vec![
            "action:render-interpretation".into(),
            "action:retain-research-object".into(),
        ]
    } else {
        vec!["action:retain-omission-certificate".into()]
    };
    let mut checks = STAGES
        .iter()
        .map(|stage| format!("stage:{stage}"))
        .chain(
            [
                "study-identity",
                "modality-closure",
                "cross-study-comparability",
                "provenance-closure",
                "negative-evidence-retention",
                "policy-boundary",
                "replay-identity",
            ]
            .into_iter()
            .map(String::from),
        )
        .collect::<Vec<_>>();
    checks.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "scope": request.scope,
        "semantic_profile": request.semantic_profile,
        "query": request.query,
        "stage_order": STAGES,
        "study_order": study_order,
        "rank_order": rank_order,
        "selected_order": selected,
        "decisions": decisions,
        "replay_identity": request.replay_identity,
        "comparability_digest": request.expected_comparability_digest,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let workflow_digest = ContentHash::of_value(&payload)
        .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))?;
    let comparability_digest = request.expected_comparability_digest.clone();
    let artifact = TypedResearchArtifact::from_payload(
        format!("interactive-interpretation:{}", request.workflow_id),
        "application/vnd.aurora.interactive-interpretation+json",
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.workflow_id.clone(),
            relation: "interpretation-workflow-fabric".into(),
            digest: workflow_digest.clone(),
        }],
    )
    .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(disposition, InterpretationDisposition::Qualified) {
        vec![format!(
            "operate:interpretation-workflow:{}",
            request.workflow_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = InterpretationWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        query: request.query.clone(),
        disposition,
        stage_order: STAGES.iter().map(|stage| (*stage).to_string()).collect(),
        study_order,
        modality_order,
        rank_order,
        qualified_order: qualified,
        unresolved_order: unresolved,
        blocked_order: blocked,
        incomparable_order: incomparable.into_iter().collect(),
        missing_modality_order: missing_modality.into_iter().collect(),
        panel_order: selected,
        action_receipts,
        checks,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        comparability_digest,
        workflow_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn run_json(value: &Value) -> Result<Value, InterpretationWorkflowError> {
    let request: InterpretationWorkflowRequest = serde_json::from_value(value.clone())
        .map_err(|error| InterpretationWorkflowError::Invalid(error.to_string()))?;
    serde_json::to_value(run(&request)?)
        .map_err(|error| InterpretationWorkflowError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"multimodal-interpretation-workflow")
    }

    fn study(id: &str, state: EvidenceState) -> InterpretationStudy {
        InterpretationStudy {
            study_id: id.into(),
            modality_order: vec!["image".into(), "omics".into()],
            interpretation_score_milli: 90_000,
            evidence_state: state,
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            comparability_digest: Some(hash()),
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_result: false,
        }
    }

    fn request() -> InterpretationWorkflowRequest {
        InterpretationWorkflowRequest {
            request_id: "request:interpretation".into(),
            workflow_id: "workflow:interpretation".into(),
            scope: "organoid-neuroscience".into(),
            semantic_profile: "ome-ngff+anndata:v1".into(),
            query: "compare resilience signatures".into(),
            input_schema: INPUT_SCHEMA.into(),
            studies: vec![
                study("study-a", EvidenceState::Supported),
                study("study-b", EvidenceState::Proven),
            ],
            required_modalities: vec!["image".into(), "omics".into()],
            expected_comparability_digest: hash(),
            min_studies: 2,
            max_panels: 4,
            budget_units: 20,
            max_budget_units: 20,
            replay_identity: hash(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn complete_workflow_qualifies_and_replays() {
        let receipt = run(&request()).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Qualified);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
        assert_eq!(receipt.panel_order.len(), 2);
    }

    #[test]
    fn missing_modality_and_unknown_evidence_are_unresolved() {
        let mut value = request();
        value.studies[0].modality_order = vec!["image".into()];
        value.studies[0].evidence_state = EvidenceState::Unknown;
        let receipt = run(&value).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unresolved);
        assert!(receipt
            .missing_modality_order
            .iter()
            .any(|item| item.starts_with("study-a:")));
    }

    #[test]
    fn contradiction_and_policy_block_release() {
        let mut value = request();
        value.studies[0].evidence_state = EvidenceState::Contradicted;
        value.policy_allow = false;
        let receipt = run(&value).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn incomparable_study_is_retained_as_evidence() {
        let mut value = request();
        value.studies[1].comparability_digest = Some(ContentHash::of_bytes(b"different-profile"));
        let receipt = run(&value).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unresolved);
        assert!(receipt.incomparable_order.contains(&"study-b".into()));
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("comparability")));
    }

    #[test]
    fn manifest_is_a2_and_multimodal() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A2);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Api));
    }
}

trait SortedBy: Iterator {
    fn sorted_by<F>(self, compare: F) -> Vec<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> std::cmp::Ordering;
}

impl<I: Iterator> SortedBy for I {
    fn sorted_by<F>(self, mut compare: F) -> Vec<Self::Item>
    where
        F: FnMut(&Self::Item, &Self::Item) -> std::cmp::Ordering,
    {
        let mut values = self.collect::<Vec<_>>();
        values.sort_by(|left, right| compare(left, right));
        values
    }
}
