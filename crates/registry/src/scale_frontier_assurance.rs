//! Multimodal registry-scale frontier assurance for `AFA-registry-P29-F26`.
//!
//! This surface measures a declared, synthetic or institution-local workload against explicit
//! study/artifact/byte/operation envelopes. It is a release gate, not a benchmark claim: missing
//! modality closure, unknown quality, contradictory attestations, or unsafe policy state can only
//! produce unresolved or blocked receipts.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-registry-P29-F26";
pub const CONTRACT_VERSION: &str = "registry-multimodal-scale-frontier-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "RegistryScaleWorkload2@1";
pub const OUTPUT_SCHEMA: &str = "RegistryCapacityReport7@1";
pub const TOOL_NAME: &str = "registry_multimodal_scale_frontier_assurance";
const CONTENT_TYPE: &str = "application/vnd.aurora.registry-capacity-report-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadState {
    Complete,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryStudyWorkload {
    pub study_id: String,
    pub modality_order: Vec<String>,
    pub artifact_count: u64,
    pub bytes: u64,
    pub operation_units: u64,
    pub workload_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: WorkloadState,
    pub local_only: bool,
    pub permitted: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryScaleWorkload {
    pub schema_version: String,
    pub request_id: String,
    pub registry_id: String,
    pub semantic_profile: String,
    pub required_modality_order: Vec<String>,
    pub studies: Vec<RegistryStudyWorkload>,
    pub max_studies: u64,
    pub max_artifacts: u64,
    pub max_bytes: u64,
    pub max_operation_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryCapacityReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub registry_id: String,
    pub semantic_profile: String,
    pub disposition: CapacityDisposition,
    pub study_order: Vec<String>,
    pub qualified_study_order: Vec<String>,
    pub unresolved_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub capacity_exceeded_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub observed_studies: u64,
    pub observed_artifacts: u64,
    pub observed_bytes: u64,
    pub observed_operation_units: u64,
    pub replay_identity: ContentHash,
    pub report_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryScaleAssuranceError {
    #[error("invalid registry scale workload: {0}")]
    Invalid(String),
    #[error("registry capacity artifact failed: {0}")]
    Artifact(String),
}
fn invalid(message: impl Into<String>) -> RegistryScaleAssuranceError {
    RegistryScaleAssuranceError::Invalid(message.into())
}
fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl RegistryCapacityReport {
    pub fn validate(&self) -> Result<(), RegistryScaleAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.registry_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.study_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "registry capacity identity, locality, studies, or effects are incomplete",
            ));
        }
        for values in [
            &self.study_order,
            &self.qualified_study_order,
            &self.unresolved_study_order,
            &self.blocked_study_order,
            &self.missing_modality_order,
            &self.capacity_exceeded_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("registry capacity ordering is not canonical"));
            }
        }
        let ids = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .qualified_study_order
            .iter()
            .chain(self.unresolved_study_order.iter())
            .chain(self.blocked_study_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("registry study states do not partition workload"));
        }
        for value in [
            &self.replay_identity,
            &self.report_digest,
            &self.artifact.content_hash,
        ] {
            if !digest_is_valid(value) {
                return Err(invalid("registry capacity digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RegistryScaleAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("registry capacity artifact type is invalid"));
        }
        if self.disposition == CapacityDisposition::Qualified
            && self.effect_receipts != [format!("measure:registry-capacity:{}", self.registry_id)]
        {
            return Err(invalid("qualified registry capacity effect is invalid"));
        }
        if self.disposition != CapacityDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid(
                "non-qualified registry capacity must block release",
            ));
        }
        Ok(())
    }
}

pub fn registry_scale_frontier_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "registry".into(), consumers: BTreeSet::from([String::from("research workflow operator"), String::from("registry capacity engineer"), String::from("platform reliability engineer")]), behavior: "measures a multimodal multi-study registry workload against explicit capacity, comparability, provenance, replay, policy, and safety gates without reading raw data".into(), value: "makes registry scale limits and semantic incompleteness observable before a benchmark or release is trusted".into(), inputs: vec![TypedPort { name: "registry_scale_workload".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "registry_capacity_report".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation]), permissions: BTreeSet::from([String::from("evaluate:capability-runs")]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_registry_scale_frontier(
    request: &RegistryScaleWorkload,
) -> Result<RegistryCapacityReport, RegistryScaleAssuranceError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|a, b| a.study_id.cmp(&b.study_id));
    let study_order = studies
        .iter()
        .map(|s| s.study_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_modalities = BTreeSet::new();
    let mut capacity_exceeded: BTreeSet<String> = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let observed_artifacts = studies.iter().map(|s| s.artifact_count).sum::<u64>();
    let observed_bytes = studies.iter().map(|s| s.bytes).sum::<u64>();
    let observed_operation_units = studies.iter().map(|s| s.operation_units).sum::<u64>();
    for study in &studies {
        let modalities = study
            .modality_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for modality in required.difference(&modalities) {
            missing_modalities.insert(format!("{}:{modality}", study.study_id));
        }
        omissions.extend(
            study
                .omissions
                .iter()
                .map(|v| format!("{}:{v}", study.study_id)),
        );
        uncertainty.extend(
            study
                .uncertainty
                .iter()
                .map(|v| format!("{}:{v}", study.study_id)),
        );
        if study.negative_result {
            negative.insert(format!("{}:negative-result", study.study_id));
        }
        if study.state == WorkloadState::Contradicted || !study.local_only || !study.permitted {
            blocked.insert(study.study_id.clone());
        } else if study.state == WorkloadState::Unknown
            || study.state == WorkloadState::Unmeasured
            || !study.omissions.is_empty()
            || !study.uncertainty.is_empty()
            || !required.is_subset(&modalities)
        {
            unresolved.insert(study.study_id.clone());
        } else {
            qualified.insert(study.study_id.clone());
        }
    }
    if studies.len() as u64 > request.max_studies {
        capacity_exceeded.insert("studies".into());
    }
    if observed_artifacts > request.max_artifacts {
        capacity_exceeded.insert("artifacts".into());
    }
    if observed_bytes > request.max_bytes {
        capacity_exceeded.insert("bytes".into());
    }
    if observed_operation_units > request.max_operation_units {
        capacity_exceeded.insert("operation-units".into());
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
            .map(|v| format!("adversarial:{v}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.adversarial_events.is_empty()
        || !capacity_exceeded.is_empty();
    if global_block {
        blocked.extend(study_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:registry-capacity-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        CapacityDisposition::Blocked
    } else if !unresolved.is_empty() {
        CapacityDisposition::Unresolved
    } else {
        CapacityDisposition::Qualified
    };
    let qualified_study_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_study_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_study_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_modality_order = missing_modalities.into_iter().collect::<Vec<_>>();
    let capacity_exceeded_order = capacity_exceeded.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == CapacityDisposition::Qualified {
        vec![format!("measure:registry-capacity:{}", request.registry_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "registry_id": request.registry_id, "semantic_profile": request.semantic_profile, "disposition": disposition, "study_order": study_order, "qualified_study_order": qualified_study_order, "unresolved_study_order": unresolved_study_order, "blocked_study_order": blocked_study_order, "missing_modality_order": missing_modality_order, "capacity_exceeded_order": capacity_exceeded_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_evidence_order, "observed_studies": studies.len(), "observed_artifacts": observed_artifacts, "observed_bytes": observed_bytes, "observed_operation_units": observed_operation_units, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let report_digest = ContentHash::of_value(&payload)
        .map_err(|error| RegistryScaleAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("registry-capacity-report:{}", request.registry_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RegistryScaleAssuranceError::Artifact(error.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect::<Vec<String>>()
    };
    let report = RegistryCapacityReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        registry_id: request.registry_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        study_order: strings("study_order"),
        qualified_study_order: strings("qualified_study_order"),
        unresolved_study_order: strings("unresolved_study_order"),
        blocked_study_order: strings("blocked_study_order"),
        missing_modality_order: strings("missing_modality_order"),
        capacity_exceeded_order: strings("capacity_exceeded_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        observed_studies: studies.len() as u64,
        observed_artifacts,
        observed_bytes,
        observed_operation_units,
        replay_identity: request.replay_identity.clone(),
        report_digest,
        artifact,
        effect_receipts: strings("effect_receipts"),
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

pub fn assure_registry_scale_frontier_json(value: &Value) -> Result<Value, String> {
    let request: RegistryScaleWorkload = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid registry scale workload: {error}"))?;
    let report = assure_registry_scale_frontier(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(report)
        .map_err(|error| format!("cannot serialize registry capacity report: {error}"))
}
pub fn validate_registry_scale_frontier_json(
    value: &Value,
) -> Result<RegistryCapacityReport, String> {
    let report: RegistryCapacityReport = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid registry capacity report: {error}"))?;
    report.validate().map_err(|error| error.to_string())?;
    Ok(report)
}
fn validate_request(request: &RegistryScaleWorkload) -> Result<(), RegistryScaleAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.registry_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_modality_order)
        || request.studies.is_empty()
        || request.max_studies == 0
        || request.max_artifacts == 0
        || request.max_bytes == 0
        || request.max_operation_units == 0
        || !digest_is_valid(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid("registry workload identity, modality closure, capacity, replay, locality, or boundary is invalid"));
    }
    let mut ids = BTreeSet::new();
    for study in &request.studies {
        if study.study_id.trim().is_empty()
            || !ids.insert(study.study_id.clone())
            || study.modality_order.is_empty()
            || !canonical(&study.modality_order)
            || !digest_is_valid(&study.workload_digest)
            || !digest_is_valid(&study.provenance_digest)
            || !digest_is_valid(&study.replay_identity)
            || !canonical(&study.omissions)
            || !canonical(&study.uncertainty)
        {
            return Err(invalid(format!(
                "study {} is malformed or duplicated",
                study.study_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> RegistryScaleWorkload {
        let d = hash("registry");
        let study = |id: &str| RegistryStudyWorkload {
            study_id: id.into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            artifact_count: 2,
            bytes: 10,
            operation_units: 3,
            workload_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            state: WorkloadState::Complete,
            local_only: true,
            permitted: true,
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
        };
        RegistryScaleWorkload {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "registry:one".into(),
            registry_id: "registry:test".into(),
            semantic_profile: "ome-ann:v1".into(),
            required_modality_order: vec!["imaging".into(), "omics".into()],
            studies: vec![study("study:a"), study("study:b")],
            max_studies: 4,
            max_artifacts: 8,
            max_bytes: 100,
            max_operation_units: 20,
            replay_identity: d,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            registry_scale_frontier_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_registry_scale_frontier(&request())
                .unwrap()
                .disposition,
            CapacityDisposition::Qualified
        );
    }
    #[test]
    fn deterministic() {
        let a = assure_registry_scale_frontier(&request()).unwrap();
        let b = assure_registry_scale_frontier(&request()).unwrap();
        assert_eq!(a.report_digest, b.report_digest);
    }
    #[test]
    fn missing_modality_unresolved() {
        let mut v = request();
        v.studies[0].modality_order = vec!["imaging".into()];
        assert_eq!(
            assure_registry_scale_frontier(&v).unwrap().disposition,
            CapacityDisposition::Unresolved
        );
    }
    #[test]
    fn capacity_blocks() {
        let mut v = request();
        v.max_bytes = 1;
        assert_eq!(
            assure_registry_scale_frontier(&v).unwrap().disposition,
            CapacityDisposition::Blocked
        );
    }
    #[test]
    fn contradiction_blocks() {
        let mut v = request();
        v.studies[0].state = WorkloadState::Contradicted;
        assert_eq!(
            assure_registry_scale_frontier(&v).unwrap().disposition,
            CapacityDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks() {
        let mut v = request();
        v.adversarial_events = vec!["poisoned-workload".into()];
        assert_eq!(
            assure_registry_scale_frontier(&v).unwrap().disposition,
            CapacityDisposition::Blocked
        );
    }
}
