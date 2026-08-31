//! Multimodal interpretation engine for `AFA-services-P14-F02`.
//!
//! The engine turns caller-supplied, typed imaging and omics result declarations into an
//! omission-aware interpretation surface.  It computes no statistic over raw measurements and
//! does not infer biology; its product value is deterministic comparability, evidence partitioning,
//! and a replayable visualization/interpretation envelope for downstream research workflows.

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

pub const FEATURE_ID: &str = "AFA-services-P14-F02";
pub const CONTRACT_VERSION: &str =
    "services-multimodal-interpretation-visualization-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult2@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.services-interactive-interpretation-1+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult2 {
    pub result_id: String,
    pub study_id: String,
    pub modality: String,
    pub claim_label: String,
    pub value_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub comparable: bool,
    pub signed: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationRequest2 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub results: Vec<EvidenceBackedResult2>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretation1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub semantic_profile: String,
    pub disposition: InterpretationDisposition,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub result_order: Vec<String>,
    pub selected_result_order: Vec<String>,
    pub unresolved_result_order: Vec<String>,
    pub blocked_result_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub interpretation_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub interpretation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationEngineError {
    #[error("invalid multimodal interpretation request: {0}")]
    Invalid(String),
    #[error("interpretation artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> InterpretationEngineError {
    InterpretationEngineError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl InteractiveInterpretation1 {
    pub fn validate(&self) -> Result<(), InterpretationEngineError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.result_order.is_empty()
            || self.required_study_order.is_empty()
            || self.required_modality_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("interpretation identity, required axes, results, locality, or effects are incomplete"));
        }
        for values in [
            &self.required_study_order,
            &self.required_modality_order,
            &self.result_order,
            &self.selected_result_order,
            &self.unresolved_result_order,
            &self.blocked_result_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.interpretation_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("interpretation ordering is not canonical"));
            }
        }
        let results = self.result_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_result_order
            .iter()
            .chain(self.unresolved_result_order.iter())
            .chain(self.blocked_result_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if results.len() != self.result_order.len()
            || parts.len() != results.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != results
        {
            return Err(invalid("result states do not form a complete partition"));
        }
        let studies = self
            .required_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing_studies = self
            .missing_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing_studies.is_subset(&studies) {
            return Err(invalid("missing study state is outside required studies"));
        }
        let modalities = self
            .required_modality_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing_modalities = self
            .missing_modality_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing_modalities.is_subset(&modalities) {
            return Err(invalid(
                "missing modality state is outside required modalities",
            ));
        }
        for value in [&self.interpretation_digest, &self.artifact.content_hash] {
            if !digest(value) {
                return Err(invalid("interpretation digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationEngineError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.interpretation_digest
        {
            return Err(invalid(
                "interpretation artifact metadata or digest is inconsistent",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("observe:interpretation:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the interpretation gate"));
        }
        if self.disposition == InterpretationDisposition::Qualified
            && self.effect_receipts != [format!("observe:interpretation:{}", self.request_id)]
        {
            return Err(invalid("qualified interpretation effect is invalid"));
        }
        if self.disposition != InterpretationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified interpretation must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationEngineError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| InterpretationEngineError::Artifact(error.to_string()))?,
        )
        .map_err(|error| InterpretationEngineError::Artifact(error.to_string()))
    }
}

pub fn multimodal_interpretation_engine_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "services".into(),
        consumers: [
            "laboratory automation engineer".into(),
            "imaging scientist".into(),
            "multi-omics analyst".into(),
            "downstream research workflow".into(),
        ]
        .into(),
        behavior: "compiles caller-supplied imaging and omics result declarations into a deterministic omission-aware interpretation surface with explicit study/modality comparability".into(),
        value: "gives research operators a replayable multimodal interpretation and visualization envelope without hiding missing evidence or treating unknown as zero".into(),
        inputs: vec![TypedPort {
            name: "evidence_backed_result".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "interactive_interpretation".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into(), "evaluate:research-evidence".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ome-ngff-rfc5".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()),
            },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_multimodal_interpretation(
    request: &InterpretationRequest2,
) -> Result<InteractiveInterpretation1, InterpretationEngineError> {
    validate_request(request)?;
    let mut results = request.results.clone();
    results.sort_by(|left, right| left.result_id.cmp(&right.result_id));
    let result_order = results
        .iter()
        .map(|result| result.result_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_studies = BTreeSet::new();
    let mut selected_modalities = BTreeSet::new();
    for result in &results {
        if !result.local_only || !result.aggregate_only || !result.permitted {
            blocked.insert(result.result_id.clone());
            omissions.insert(format!("{}:locality-or-permission", result.result_id));
        } else if result.replay_identity != request.replay_identity
            || !result.signed
            || !result.comparable
            || !matches!(
                result.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(result.result_id.clone());
            if result.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", result.result_id));
            }
            if !result.signed {
                uncertainty.insert(format!("{}:signature-missing", result.result_id));
            }
            if !result.comparable {
                uncertainty.insert(format!("{}:comparability-unmeasured", result.result_id));
            }
            if !matches!(
                result.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            ) {
                uncertainty.insert(format!("{}:evidence-state", result.result_id));
            }
        } else {
            selected.insert(result.result_id.clone());
            selected_studies.insert(result.study_id.clone());
            selected_modalities.insert(result.modality.clone());
        }
        if result.negative_result {
            negative.insert(format!("{}:negative-result", result.result_id));
        }
        omissions.extend(
            result
                .omission_order
                .iter()
                .map(|item| format!("{}:{item}", result.result_id)),
        );
        uncertainty.extend(
            result
                .uncertainty_order
                .iter()
                .map(|item| format!("{}:{item}", result.result_id)),
        );
    }
    let required_studies = request
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_studies = required_studies
        .difference(&selected_studies)
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_modalities = required_modalities
        .difference(&selected_modalities)
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(
        missing_studies
            .iter()
            .map(|id| format!("study:{id}:missing")),
    );
    omissions.extend(
        missing_modalities
            .iter()
            .map(|id| format!("modality:{id}:missing")),
    );
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(result_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:interpretation-release-gate-blocked".into());
    }
    let disposition = if global_block || selected.is_empty() && !blocked.is_empty() {
        InterpretationDisposition::Blocked
    } else if selected.is_empty() || !missing_studies.is_empty() || !missing_modalities.is_empty() {
        InterpretationDisposition::Unresolved
    } else {
        InterpretationDisposition::Qualified
    };
    if disposition != InterpretationDisposition::Qualified {
        omissions.insert("request:interpretation-not-release-ready".into());
    }
    let selected_result_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_result_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_result_order = blocked.into_iter().collect::<Vec<_>>();
    let interpretation_order = if disposition == InterpretationDisposition::Qualified {
        selected_result_order.clone()
    } else {
        Vec::new()
    };
    let effect_receipts = if disposition == InterpretationDisposition::Qualified {
        vec![format!("observe:interpretation:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "researcher": request.researcher,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "required_study_order": request.required_study_order,
        "required_modality_order": request.required_modality_order,
        "result_order": result_order,
        "selected_result_order": selected_result_order,
        "unresolved_result_order": unresolved_result_order,
        "blocked_result_order": blocked_result_order,
        "missing_study_order": missing_studies,
        "missing_modality_order": missing_modalities,
        "interpretation_order": interpretation_order,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let interpretation_digest = ContentHash::of_value(&payload)
        .map_err(|error| InterpretationEngineError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("services-interactive-interpretation:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InterpretationEngineError::Artifact(error.to_string()))?;
    let receipt = InteractiveInterpretation1 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        required_study_order: request.required_study_order.clone(),
        required_modality_order: request.required_modality_order.clone(),
        result_order: payload["result_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_result_order: payload["selected_result_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_result_order: payload["unresolved_result_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_result_order: payload["blocked_result_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_study_order: payload["missing_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        interpretation_order: payload["interpretation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        interpretation_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &InterpretationRequest2) -> Result<(), InterpretationEngineError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_study_order)
        || !canonical(&request.required_modality_order)
        || request.results.is_empty()
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(invalid(
            "interpretation identity, axes, results, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for result in &request.results {
        if result.result_id.trim().is_empty()
            || result.study_id.trim().is_empty()
            || result.modality.trim().is_empty()
            || result.claim_label.trim().is_empty()
            || !ids.insert(result.result_id.clone())
            || !digest(&result.value_digest)
            || !digest(&result.provenance_digest)
            || !digest(&result.replay_identity)
            || !canonical(&result.omission_order)
            || !canonical(&result.uncertainty_order)
        {
            return Err(invalid(format!(
                "result {} is malformed or duplicated",
                result.result_id
            )));
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

    fn request() -> InterpretationRequest2 {
        let result = |id: &str, study: &str, modality: &str| EvidenceBackedResult2 {
            result_id: id.into(),
            study_id: study.into(),
            modality: modality.into(),
            claim_label: format!("claim:{id}"),
            value_digest: hash(id),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            comparable: true,
            signed: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        };
        InterpretationRequest2 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:interpretation".into(),
            researcher: "lab-automation-engineer".into(),
            semantic_profile: "imaging-omics:v1".into(),
            required_study_order: vec!["study:a".into(), "study:b".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            results: vec![
                result("result:a", "study:a", "imaging"),
                result("result:b", "study:b", "omics"),
            ],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_interpretation_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }

    #[test]
    fn qualified_interpretation_is_deterministic() {
        let receipt = compile_multimodal_interpretation(&request()).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Qualified);
        assert_eq!(receipt.interpretation_order.len(), 2);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn missing_modality_is_unresolved() {
        let mut request = request();
        request.results[1].comparable = false;
        let receipt = compile_multimodal_interpretation(&request).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unresolved);
        assert!(receipt.missing_modality_order.contains(&"omics".into()));
    }

    #[test]
    fn unauthorized_result_is_blocked() {
        let mut request = request();
        request.results[0].permitted = false;
        let receipt = compile_multimodal_interpretation(&request).unwrap();
        assert!(receipt.blocked_result_order.contains(&"result:a".into()));
        assert_eq!(receipt.disposition, InterpretationDisposition::Unresolved);
    }

    #[test]
    fn adversarial_request_blocks_all_results() {
        let mut request = request();
        request.adversarial_events = vec!["poisoned-annotation".into()];
        let receipt = compile_multimodal_interpretation(&request).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Blocked);
        assert!(receipt.selected_result_order.is_empty());
        assert_eq!(receipt.blocked_result_order.len(), 2);
    }

    #[test]
    fn negative_result_is_preserved() {
        let mut request = request();
        request.results[0].negative_result = true;
        let receipt = compile_multimodal_interpretation(&request).unwrap();
        assert!(receipt
            .negative_evidence_order
            .contains(&"result:a:negative-result".into()));
    }

    #[test]
    fn replay_mismatch_is_unresolved() {
        let mut request = request();
        request.results[0].replay_identity = hash("other-replay");
        let receipt = compile_multimodal_interpretation(&request).unwrap();
        assert!(receipt.unresolved_result_order.contains(&"result:a".into()));
        assert!(receipt
            .uncertainty_order
            .contains(&"result:a:replay-mismatch".into()));
    }
}
