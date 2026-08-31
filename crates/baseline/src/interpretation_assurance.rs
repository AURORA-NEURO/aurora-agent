//! Multimodal interpretation and visualization assurance harness.
//!
//! Atlas feature: `AFA-baseline-P14-F26`.  The harness checks typed summaries
//! from independent imaging and omics studies before a visualization surface
//! is released.  It never reads raw pixels, invents an interpretation, or
//! makes a clinical decision.

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

pub const FEATURE_ID: &str = "AFA-baseline-P14-F26";
pub const CONTRACT_VERSION: &str = "baseline-multimodal-interpretation-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult2@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.interactive-interpretation-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult {
    pub result_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub comparability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub support_score_milli: u64,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_data: bool,
    pub permitted: bool,
    pub cost_units: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationAssuranceRequest {
    pub schema_version: String,
    pub request_id: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub candidates: Vec<EvidenceBackedResult>,
    pub budget_units: u64,
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
pub struct InterpretationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub semantic_profile: String,
    pub disposition: InterpretationDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub interpretation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationAssuranceError {
    #[error("invalid interpretation assurance request: {0}")]
    Invalid(String),
    #[error("interpretation artifact failed: {0}")]
    Artifact(String),
}
fn invalid(value: impl Into<String>) -> InterpretationAssuranceError {
    InterpretationAssuranceError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl InterpretationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("interpretation assurance identity, locality, candidates, or effects are incomplete"));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.incomparable_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid(
                    "interpretation assurance ordering is not canonical",
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .qualified_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("interpretation states do not partition candidates"));
        }
        for value in [
            &self.replay_identity,
            &self.interpretation_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("interpretation assurance digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| InterpretationAssuranceError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("interpretation artifact type is invalid"));
        }
        if self.disposition == InterpretationDisposition::Qualified
            && self.effect_receipts != [format!("block:unsafe-release")].as_slice()
        {
            return Err(invalid(
                "qualified interpretation must still expose release gate receipt",
            ));
        }
        if self.disposition != InterpretationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified interpretation must block release"));
        }
        Ok(())
    }
}

pub fn interpretation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "baseline".into(), consumers: ["computational biologist".into(), "visualization reviewer".into(), "release steward".into()].into(), behavior: "verifies typed multimodal interpretations with study/modality comparability and explicit release-gate witnesses".into(), value: "prevents an attractive visualization from hiding incomparable, missing, contradictory, or unproven preclinical evidence".into(), inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "W3C PROV-O".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "OME-NGFF RFC 5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "interpretation release reviewer".into(), reason: "multimodal visual summaries require independent preclinical review".into() }], autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Operator, ResearchSurface::Policy].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_multimodal_interpretation(
    request: &InterpretationAssuranceRequest,
) -> Result<InterpretationAssuranceReceipt, InterpretationAssuranceError> {
    validate_request(request)?;
    let mut rows = request.candidates.clone();
    rows.sort_by(|a, b| {
        b.support_score_milli
            .cmp(&a.support_score_milli)
            .then_with(|| a.result_id.cmp(&b.result_id))
    });
    let candidate_order = {
        let mut ids = rows.iter().map(|x| x.result_id.clone()).collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for row in &rows {
        if row.negative_result {
            negative.insert(format!("{}:negative-result", row.result_id));
        }
        omissions.extend(
            row.omissions
                .iter()
                .map(|x| format!("{}:{x}", row.result_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|x| format!("{}:{x}", row.result_id)),
        );
        if row.comparability_digest != request.comparability_digest {
            incomparable.insert(row.result_id.clone());
            unresolved.insert(row.result_id.clone());
            omissions.insert(format!("{}:comparability-mismatch", row.result_id));
            continue;
        }
        if row.evidence_state == EvidenceState::Contradicted || !row.local_data || !row.permitted {
            blocked.insert(row.result_id.clone());
            continue;
        }
        if matches!(
            row.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
        {
            unresolved.insert(row.result_id.clone());
            continue;
        }
        if row.semantic_profile != request.semantic_profile {
            incomparable.insert(row.result_id.clone());
            unresolved.insert(row.result_id.clone());
            continue;
        }
        if spent.saturating_add(row.cost_units) > request.budget_units {
            unresolved.insert(row.result_id.clone());
            omissions.insert(format!("{}:budget-exhausted", row.result_id));
            continue;
        }
        spent = spent.saturating_add(row.cost_units);
        qualified.insert(row.result_id.clone());
    }
    let observed_studies = rows
        .iter()
        .filter(|row| qualified.contains(&row.result_id))
        .map(|row| row.study_id.clone())
        .collect::<BTreeSet<_>>();
    let observed_modalities = rows
        .iter()
        .filter(|row| qualified.contains(&row.result_id))
        .map(|row| row.modality.clone())
        .collect::<BTreeSet<_>>();
    let missing_study = request
        .required_study_order
        .iter()
        .filter(|x| !observed_studies.contains(*x))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality = request
        .required_modality_order
        .iter()
        .filter(|x| !observed_modalities.contains(*x))
        .cloned()
        .collect::<Vec<_>>();
    for id in &missing_study {
        omissions.insert(format!("{id}:required-study-missing"));
    }
    for id in &missing_modality {
        omissions.insert(format!("{id}:required-modality-missing"));
    }
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
            .map(|x| format!("adversarial:{x}")),
    );
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        incomparable.clear();
        omissions.insert("request:release-gate-blocked".into());
    }
    let disposition = if global {
        InterpretationDisposition::Blocked
    } else if !missing_study.is_empty() || !missing_modality.is_empty() || qualified.is_empty() {
        InterpretationDisposition::Unresolved
    } else {
        InterpretationDisposition::Qualified
    };
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let incomparable_order = incomparable.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = vec!["block:unsafe-release".into()];
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "semantic_profile": request.semantic_profile, "disposition": disposition, "candidate_order": candidate_order, "qualified_order": qualified_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "incomparable_order": incomparable_order, "missing_study_order": missing_study, "missing_modality_order": missing_modality, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_evidence_order, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY});
    let interpretation_digest = ContentHash::of_value(&payload)
        .map_err(|e| InterpretationAssuranceError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("interactive-interpretation:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| InterpretationAssuranceError::Artifact(e.to_string()))?;
    let receipt = InterpretationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_order: payload["qualified_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        incomparable_order: payload["incomparable_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_study_order: payload["missing_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        interpretation_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &InterpretationAssuranceRequest,
) -> Result<(), InterpretationAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.candidates.is_empty()
        || !canonical(&request.required_study_order)
        || !canonical(&request.required_modality_order)
        || !canonical(&request.adversarial_events)
        || !digest(&request.comparability_digest)
        || !digest(&request.replay_identity)
        || request.budget_units == 0
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid("interpretation request identity, study/modality closure, digests, budget, locality, or boundary is invalid"));
    }
    let mut ids = BTreeSet::new();
    for row in &request.candidates {
        if row.result_id.trim().is_empty()
            || !ids.insert(row.result_id.clone())
            || row.study_id.trim().is_empty()
            || row.modality.trim().is_empty()
            || row.semantic_profile.trim().is_empty()
            || !digest(&row.comparability_digest)
            || !digest(&row.evidence_digest)
            || !digest(&row.provenance_digest)
            || !digest(&row.artifact_digest)
            || row.cost_units == 0
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "interpretation candidate {} is malformed or duplicated",
                row.result_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> InterpretationAssuranceRequest {
        let d = h("interpretation");
        let row =
            |id: &str, study: &str, modality: &str, state: EvidenceState| EvidenceBackedResult {
                result_id: id.into(),
                study_id: study.into(),
                modality: modality.into(),
                semantic_profile: "neural".into(),
                comparability_digest: d.clone(),
                evidence_digest: d.clone(),
                provenance_digest: d.clone(),
                artifact_digest: d.clone(),
                evidence_state: state,
                support_score_milli: 900,
                omissions: vec![],
                uncertainty: vec![],
                negative_result: false,
                local_data: true,
                permitted: true,
                cost_units: 1,
            };
        InterpretationAssuranceRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:interpretation".into(),
            semantic_profile: "neural".into(),
            required_study_order: vec!["study:a".into(), "study:b".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            comparability_digest: d.clone(),
            replay_identity: d.clone(),
            candidates: vec![
                row("result:a", "study:a", "imaging", EvidenceState::Supported),
                row("result:b", "study:b", "omics", EvidenceState::Supported),
            ],
            budget_units: 4,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            interpretation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_multimodal() {
        assert_eq!(
            assure_multimodal_interpretation(&request())
                .unwrap()
                .disposition,
            InterpretationDisposition::Qualified
        );
    }
    #[test]
    fn missing_modality_unresolved() {
        let mut r = request();
        r.required_modality_order = vec!["imaging".into(), "spatial".into()];
        assert_eq!(
            assure_multimodal_interpretation(&r).unwrap().disposition,
            InterpretationDisposition::Unresolved
        );
    }
    #[test]
    fn contradiction_blocks_candidate() {
        let mut r = request();
        r.candidates[0].evidence_state = EvidenceState::Contradicted;
        let out = assure_multimodal_interpretation(&r).unwrap();
        assert!(!out.blocked_order.is_empty());
    }
    #[test]
    fn incomparable_is_retained() {
        let mut r = request();
        r.candidates[0].comparability_digest = h("different");
        let out = assure_multimodal_interpretation(&r).unwrap();
        assert!(!out.incomparable_order.is_empty());
    }
    #[test]
    fn policy_blocks_release() {
        let mut r = request();
        r.policy_allow = false;
        assert_eq!(
            assure_multimodal_interpretation(&r).unwrap().disposition,
            InterpretationDisposition::Blocked
        );
    }
    #[test]
    fn deterministic_replay() {
        let a = assure_multimodal_interpretation(&request()).unwrap();
        let b = assure_multimodal_interpretation(&request()).unwrap();
        assert_eq!(a.interpretation_digest, b.interpretation_digest);
    }
}
