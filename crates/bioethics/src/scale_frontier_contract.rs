//! Bioethics capacity-envelope typed contract.
//!
//! Atlas feature: `AFA-bioethics-P29-F08`.
//!
//! This compatibility API records measured scale and ethical controls for institution-local
//! research operations. It never schedules work, grants authority, or treats throughput as a
//! safety conclusion; incomplete, contradictory, or unmeasured controls remain visible.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P29-F08";
pub const CONTRACT_VERSION: &str =
    "bioethics-federated-continual-bioethics-scale-frontier-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "BioethicsScaleWorkload4@1";
pub const OUTPUT_SCHEMA: &str = "BioethicsCapacityReport2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-capacity-report-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsScaleWorkload4 {
    pub workload_id: String,
    pub institution_id: String,
    pub study_id: String,
    pub modality: String,
    pub operation: String,
    pub requested_parallelism: u32,
    pub observed_parallelism: u32,
    pub throughput_per_hour: u32,
    pub p99_latency_millis: u64,
    pub error_rate_basis_points: u32,
    pub benchmark_digest: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub measured: bool,
    pub dual_use_clear: bool,
    pub privacy_clear: bool,
    pub institution_authorized: bool,
    pub no_clinical_use: bool,
    pub negative_result: bool,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsScaleFrontierRequest {
    pub request_id: String,
    pub reviewer: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub required_workload_order: Vec<String>,
    pub required_institution_order: Vec<String>,
    pub required_operation_order: Vec<String>,
    pub workloads: Vec<BioethicsScaleWorkload4>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub max_error_rate_basis_points: u32,
    pub max_p99_latency_millis: u64,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsCapacityReport2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub reviewer: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub workload_order: Vec<String>,
    pub selected_workload_order: Vec<String>,
    pub unresolved_workload_order: Vec<String>,
    pub blocked_workload_order: Vec<String>,
    pub missing_workload_order: Vec<String>,
    pub institution_order: Vec<String>,
    pub operation_order: Vec<String>,
    pub selected_institution_order: Vec<String>,
    pub selected_operation_order: Vec<String>,
    pub missing_institution_order: Vec<String>,
    pub missing_operation_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub capacity_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: String,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BioethicsScaleFrontierError {
    #[error("invalid bioethics scale-frontier request: {0}")]
    Invalid(String),
    #[error("bioethics capacity artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl BioethicsCapacityReport2 {
    pub fn validate(&self) -> Result<(), BioethicsScaleFrontierError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.reviewer.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.workload_order.is_empty()
            || self.institution_order.is_empty()
            || self.operation_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.autonomy_tier != "a1"
            || !self.raw_data_local
            || !self.aggregate_only
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(BioethicsScaleFrontierError::Invalid(
                "capacity identity, axes, locality, autonomy, boundary, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.workload_order,
            &self.selected_workload_order,
            &self.unresolved_workload_order,
            &self.blocked_workload_order,
            &self.missing_workload_order,
            &self.institution_order,
            &self.operation_order,
            &self.selected_institution_order,
            &self.selected_operation_order,
            &self.missing_institution_order,
            &self.missing_operation_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(BioethicsScaleFrontierError::Invalid(
                    "capacity ordering is not canonical".into(),
                ));
            }
        }
        let partition = self
            .selected_workload_order
            .iter()
            .chain(self.unresolved_workload_order.iter())
            .chain(self.blocked_workload_order.iter())
            .chain(self.missing_workload_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.workload_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.workload_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(BioethicsScaleFrontierError::Invalid(
                "workload states do not partition capacity report".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("validate:bioethics-capacity:") && effect != "block:unsafe-release"
        }) {
            return Err(BioethicsScaleFrontierError::Invalid(
                "effect is outside capacity validation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BioethicsScaleFrontierError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, BioethicsScaleFrontierError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| BioethicsScaleFrontierError::Artifact(error.to_string()))?,
        )
        .map_err(|error| BioethicsScaleFrontierError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(), consumers: BTreeSet::from(["institutional safety reviewer".into(),"research governance specialist".into(),"platform operator".into()]), behavior: "serializes measured bioethics capacity envelopes and validates ethical control closure without scheduling or granting authority".into(), value: "keeps scale claims separate from safety claims while exposing privacy, dual-use, authorization, provenance, replay, and negative evidence".into(), inputs: vec![TypedPort{name:"bioethics_scale_workload".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs: vec![TypedPort{name:"bioethics_capacity_report".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects: BTreeSet::new(), permissions: BTreeSet::from(["read:local-research-artifacts".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())},EvidenceReference{source_id:"anndata-format".into(),state:EvidenceState::Supported,locator:Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into())}], authority_requirements: vec![AuthorityRequirement{role:"institutional safety reviewer".into(),reason:"capacity-envelope compatibility requires independent ethical review".into()}], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Protocol,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Policy,ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(
    request: &BioethicsScaleFrontierRequest,
) -> Result<(), BioethicsScaleFrontierError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.reviewer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_workload_order.is_empty()
        || request.required_institution_order.is_empty()
        || request.required_operation_order.is_empty()
        || request.workloads.is_empty()
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(BioethicsScaleFrontierError::Invalid(
            "capacity request identity, closure, locality, boundary, or schema is invalid".into(),
        ));
    }
    let ids = request
        .workloads
        .iter()
        .map(|workload| workload.workload_id.clone())
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(BioethicsScaleFrontierError::Invalid(
            "workload identifiers must be present and unique".into(),
        ));
    }
    Ok(())
}

pub fn evaluate_capacity(
    request: &BioethicsScaleFrontierRequest,
) -> Result<BioethicsCapacityReport2, BioethicsScaleFrontierError> {
    validate_request(request)?;
    let mut workloads = request.workloads.clone();
    workloads.sort_by(|left, right| {
        left.institution_id
            .cmp(&right.institution_id)
            .then(left.operation.cmp(&right.operation))
            .then(left.workload_id.cmp(&right.workload_id))
    });
    let workload_order = workloads
        .iter()
        .map(|workload| workload.workload_id.clone())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut missing = Vec::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for workload in &workloads {
        if workload.artifact_digest.is_none() || workload.provenance_digest.is_none() {
            missing.push(workload.workload_id.clone());
            omission.insert(format!(
                "{}:artifact-or-provenance-missing",
                workload.workload_id
            ));
        } else if !workload.dual_use_clear
            || !workload.privacy_clear
            || !workload.institution_authorized
            || !workload.no_clinical_use
        {
            blocked.push(workload.workload_id.clone());
            omission.insert(format!("{}:ethical-control-denied", workload.workload_id));
        } else if workload.evidence_state == EvidenceState::Contradicted || !workload.measured {
            unresolved.push(workload.workload_id.clone());
            uncertainty.insert(format!(
                "{}:contradicted-or-unmeasured",
                workload.workload_id
            ));
        } else if workload.error_rate_basis_points > request.max_error_rate_basis_points
            || workload.p99_latency_millis > request.max_p99_latency_millis
        {
            unresolved.push(workload.workload_id.clone());
            uncertainty.insert(format!("{}:capacity-threshold-unmet", workload.workload_id));
        } else {
            selected.push(workload.workload_id.clone());
            if workload.negative_result {
                negative.insert(format!("{}:negative-result", workload.workload_id));
            }
            omission.extend(
                workload
                    .omissions
                    .iter()
                    .map(|entry| format!("{}:{entry}", workload.workload_id)),
            );
        }
    }
    let institution_order = workloads
        .iter()
        .map(|workload| workload.institution_id.clone())
        .chain(request.required_institution_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let operation_order = workloads
        .iter()
        .map(|workload| workload.operation.clone())
        .chain(request.required_operation_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let present_i = workloads
        .iter()
        .map(|workload| workload.institution_id.clone())
        .collect::<BTreeSet<_>>();
    let present_o = workloads
        .iter()
        .map(|workload| workload.operation.clone())
        .collect::<BTreeSet<_>>();
    let missing_i = request
        .required_institution_order
        .iter()
        .filter(|id| !present_i.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let missing_o = request
        .required_operation_order
        .iter()
        .filter(|id| !present_o.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    omission.extend(
        missing_i
            .iter()
            .map(|id| format!("institution:{id}:missing")),
    );
    omission.extend(missing_o.iter().map(|id| format!("operation:{id}:missing")));
    omission.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("request:adversarial:{event}")),
    );
    let selected_set = selected.iter().collect::<BTreeSet<_>>();
    let selected_i = institution_order
        .iter()
        .filter(|id| {
            workloads.iter().any(|workload| {
                selected_set.contains(&workload.workload_id) && &workload.institution_id == *id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_o = operation_order
        .iter()
        .filter(|id| {
            workloads.iter().any(|workload| {
                selected_set.contains(&workload.workload_id) && &workload.operation == *id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.federation_approved
        && request.raw_data_local
        && request.aggregate_only
        && request.adversarial_events.is_empty();
    let disposition =
        if !global_open || !blocked.is_empty() || !missing_i.is_empty() || !missing_o.is_empty() {
            "blocked"
        } else if !missing.is_empty() || !unresolved.is_empty() {
            "unresolved"
        } else {
            "qualified"
        };
    let effect = if disposition == "qualified" {
        vec![format!(
            "validate:bioethics-capacity:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":OUTPUT_SCHEMA,"request_id":request.request_id,"workload_order":workload_order,"selected_workload_order":selected,"unresolved_workload_order":unresolved,"blocked_workload_order":blocked,"missing_workload_order":missing,"institution_order":institution_order,"operation_order":operation_order,"disposition":disposition,"replay_identity":request.replay_identity});
    let capacity_digest = ContentHash::of_value(&payload)
        .map_err(|error| BioethicsScaleFrontierError::Artifact(error.to_string()))?;
    let semantic_loss = omission
        .iter()
        .map(|entry| SemanticLoss {
            field: entry.clone(),
            reason: "capacity or ethical control was omitted or gated".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioethics-capacity:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.request_id.clone(),
            relation: "bioethics-scale-frontier".into(),
            digest: capacity_digest.clone(),
        }],
    )
    .map_err(|error| BioethicsScaleFrontierError::Artifact(error.to_string()))?;
    let report = BioethicsCapacityReport2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        reviewer: request.reviewer.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        workload_order: payload["workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_workload_order: payload["selected_workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_workload_order: payload["unresolved_workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_workload_order: payload["blocked_workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_workload_order: payload["missing_workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        institution_order: payload["institution_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        operation_order: payload["operation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_institution_order: selected_i,
        selected_operation_order: selected_o,
        missing_institution_order: missing_i,
        missing_operation_order: missing_o,
        omission_order: omission.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        capacity_digest,
        artifact,
        effect_receipts: effect,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        autonomy_tier: "a1".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }
    fn workload(id: &str) -> BioethicsScaleWorkload4 {
        BioethicsScaleWorkload4 {
            workload_id: id.into(),
            institution_id: "institution:a".into(),
            study_id: "study:one".into(),
            modality: "imaging".into(),
            operation: "ingest".into(),
            requested_parallelism: 8,
            observed_parallelism: 8,
            throughput_per_hour: 100,
            p99_latency_millis: 100,
            error_rate_basis_points: 10,
            benchmark_digest: hash("benchmark"),
            artifact_digest: Some(hash(&format!("artifact:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            measured: true,
            dual_use_clear: true,
            privacy_clear: true,
            institution_authorized: true,
            no_clinical_use: true,
            negative_result: false,
            omissions: Vec::new(),
        }
    }
    fn request() -> BioethicsScaleFrontierRequest {
        BioethicsScaleFrontierRequest {
            request_id: "request:ethics".into(),
            reviewer: "safety-reviewer".into(),
            purpose: "capacity-review".into(),
            semantic_profile: "ethics:v1".into(),
            schema_version: INPUT_SCHEMA.into(),
            required_workload_order: vec!["workload:a".into()],
            required_institution_order: vec!["institution:a".into()],
            required_operation_order: vec!["ingest".into()],
            workloads: vec![workload("workload:a")],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            max_error_rate_basis_points: 100,
            max_p99_latency_millis: 200,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn ethical_capacity_qualifies() {
        let report = evaluate_capacity(&request()).unwrap();
        assert_eq!(report.disposition, "qualified");
    }
    #[test]
    fn missing_artifact_is_unresolved() {
        let mut value = request();
        value.workloads[0].artifact_digest = None;
        assert_eq!(evaluate_capacity(&value).unwrap().disposition, "unresolved");
    }
    #[test]
    fn denied_ethical_control_blocks() {
        let mut value = request();
        value.workloads[0].privacy_clear = false;
        assert_eq!(evaluate_capacity(&value).unwrap().disposition, "blocked");
    }
    #[test]
    fn unmeasured_capacity_is_unresolved() {
        let mut value = request();
        value.workloads[0].measured = false;
        assert_eq!(evaluate_capacity(&value).unwrap().disposition, "unresolved");
    }
    #[test]
    fn policy_and_adversarial_gates_block() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(evaluate_capacity(&value).unwrap().disposition, "blocked");
        value.policy_allow = true;
        value.adversarial_events = vec!["prompt-injection".into()];
        assert_eq!(evaluate_capacity(&value).unwrap().disposition, "blocked");
    }
    #[test]
    fn manifest_is_a1_effect_free_and_byte_stable() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
        assert!(manifest.effects.is_empty());
    }
}
