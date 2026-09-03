//! Multimodal laboratory-integration control plane.
//!
//! Atlas feature: `AFA-bioir-P11-F30`. This module selects and preflights a locally governed
//! instrument capability for multiple studies. It produces a non-executing receipt: hardware,
//! animals, material, and clinical decisions are permanently outside this boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioir-P11-F30";
pub const CONTRACT_VERSION: &str = "bioir-multimodal-laboratory-integration-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "InstrumentActionRequest2@1";
pub const OUTPUT_SCHEMA: &str = "InstrumentActionReceipt8@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.instrument-action-receipt-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCapability {
    pub instrument_id: String,
    pub site_id: String,
    pub protocol_profile: String,
    pub supported_operation_order: Vec<String>,
    pub interlock_order: Vec<String>,
    pub semantic_profile: String,
    pub calibration_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub permitted: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionRequest {
    pub request_id: String,
    pub federation_id: String,
    pub operation: String,
    pub required_capability_order: Vec<String>,
    pub required_interlock_order: Vec<String>,
    pub target_instrument_id: Option<String>,
    pub semantic_profile: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub capabilities: Vec<InstrumentCapability>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub network_available: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget: u64,
    pub max_budget: u64,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub operation: String,
    pub decision: String,
    pub candidate_order: Vec<String>,
    pub selected_instrument_id: Option<String>,
    pub selected_site_id: Option<String>,
    pub selected_protocol_profile: Option<String>,
    pub satisfied_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub missing_interlock_order: Vec<String>,
    pub preflight_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub receipt_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub executed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LaboratoryControlError {
    #[error("invalid laboratory control request: {0}")]
    Invalid(String),
    #[error("laboratory control artifact failed: {0}")]
    Artifact(String),
}
fn invalid(v: impl Into<String>) -> LaboratoryControlError {
    LaboratoryControlError::Invalid(v.into())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64
}

pub fn laboratory_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"bioir".into(),consumers:["research program lead".into(),"instrument steward".into(),"federation operator".into()].into(),behavior:"preflights and selects a governed multimodal instrument capability without executing hardware or moving raw observations".into(),value:"turns heterogeneous instrument manifests into reproducible, approval-bound action receipts for cross-study research".into(),inputs:vec![TypedPort{name:"instrument_action_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"instrument_action_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::FederationExport,Effect::WriteLocalArtifact].into(),permissions:["operate:institution-node".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"mcp-2025-06-18".into(),state:EvidenceState::Supported,locator:Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into())}],authority_requirements:vec![AuthorityRequirement{role:"instrument steward".into(),reason:"preflight approval is required before any separately authorized hardware executor may act".into()}],autonomy_tier:AutonomyTier::A2,surfaces:[ResearchSurface::Api,ResearchSurface::Protocol,ResearchSurface::Operator,ResearchSurface::Policy,ResearchSurface::Ui].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

impl InstrumentActionReceipt {
    pub fn validate(&self) -> Result<(), LaboratoryControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.executed
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.operation.trim().is_empty()
            || !matches!(
                self.decision.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
            || self.candidate_order.is_empty()
            || self.preflight_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("instrument receipt identity, non-execution, locality, preflight, or effects are incomplete"));
        }
        for v in [
            &self.candidate_order,
            &self.satisfied_capability_order,
            &self.missing_capability_order,
            &self.missing_interlock_order,
            &self.preflight_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(invalid("instrument receipt ordering is not canonical"));
            }
        }
        for d in [
            &self.replay_identity,
            &self.receipt_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(d) {
                return Err(invalid("instrument receipt digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| LaboratoryControlError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("instrument receipt artifact type is invalid"));
        }
        if self.decision == "qualified" {
            if self.selected_instrument_id.is_none()
                || self.selected_site_id.is_none()
                || self.effect_receipts.len() != 2
                || self.effect_receipts[0]
                    != format!("exchange:permitted-summaries:{}", self.request_id)
                || self.effect_receipts[1] != format!("manage:local-capability:{}", self.request_id)
            {
                return Err(invalid(
                    "qualified instrument effects or selection are invalid",
                ));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid(
                "non-qualified instrument receipt must block release",
            ));
        }
        Ok(())
    }
}

pub fn preflight_instrument_action(
    request: &InstrumentActionRequest,
) -> Result<InstrumentActionReceipt, LaboratoryControlError> {
    validate_request(request)?;
    let mut caps = request.capabilities.clone();
    caps.sort_by(|a, b| {
        a.instrument_id
            .cmp(&b.instrument_id)
            .then(a.site_id.cmp(&b.site_id))
    });
    let candidate_order = caps
        .iter()
        .map(|c| format!("{}@{}", c.instrument_id, c.site_id))
        .collect::<Vec<_>>();
    let required_caps = request
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_interlocks = request
        .required_interlock_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let matching = caps.iter().find(|c| {
        request
            .target_instrument_id
            .as_ref()
            .map_or(true, |id| id == &c.instrument_id)
            && c.semantic_profile == request.semantic_profile
            && c.supported_operation_order.contains(&request.operation)
            && required_caps.is_subset(&c.supported_operation_order.iter().cloned().collect())
            && required_interlocks.is_subset(&c.interlock_order.iter().cloned().collect())
            && c.permitted
            && c.raw_data_local
    });
    let (selected_instrument_id, selected_site_id, selected_protocol_profile, satisfied) = matching
        .map(|c| {
            (
                Some(c.instrument_id.clone()),
                Some(c.site_id.clone()),
                Some(c.protocol_profile.clone()),
                request.required_capability_order.clone(),
            )
        })
        .unwrap_or((None, None, None, Vec::new()));
    let missing_capability = request
        .required_capability_order
        .iter()
        .filter(|x| matching.map_or(true, |c| !c.supported_operation_order.contains(x)))
        .cloned()
        .collect::<Vec<_>>();
    let missing_interlock = request
        .required_interlock_order
        .iter()
        .filter(|x| matching.map_or(true, |c| !c.interlock_order.contains(x)))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if matching.is_none() {
        omissions.insert("instrument:capability-or-interlock-match-missing".into());
        uncertainty.insert("instrument:visible-capability-absence-is-not-impossibility".into());
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
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    if !request.network_available {
        negative.insert("request:network-unavailable".into());
    }
    if request.budget > request.max_budget {
        omissions.insert("request:budget-ceiling-exceeded".into());
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
        || !request.federation_approved
        || !request.network_available
        || !request.raw_data_local
        || !request.aggregate_only
        || request.budget > request.max_budget
        || !request.adversarial_events.is_empty();
    let decision = if global {
        "blocked"
    } else if matching.is_none() {
        "unresolved"
    } else {
        "qualified"
    };
    let preflight = vec![
        "calibration-and-provenance-digests-bound".into(),
        "capability-manifest-verified".into(),
        "hardware-execution-explicitly-not-performed".into(),
        "operation-and-interlock-closure-verified".into(),
        "policy-authority-federation-gates-evaluated".into(),
    ];
    let effects = if decision == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"operation":request.operation,"decision":decision,"candidate_order":candidate_order,"selected_instrument_id":selected_instrument_id,"selected_site_id":selected_site_id,"selected_protocol_profile":selected_protocol_profile,"satisfied_capability_order":satisfied,"missing_capability_order":missing_capability,"missing_interlock_order":missing_interlock,"preflight_order":preflight,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"replay_identity":request.replay_identity,"effect_receipts":effects,"executed":false,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let receipt_digest = ContentHash::of_value(&payload)
        .map_err(|e| LaboratoryControlError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioir-instrument-action:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        caps.iter()
            .map(|c| ProvenanceLink {
                source_id: format!("{}@{}", c.instrument_id, c.site_id),
                relation: "evaluated-local-instrument-capability".into(),
                digest: c.provenance_digest.clone(),
            })
            .collect(),
    )
    .map_err(|e| LaboratoryControlError::Artifact(e.to_string()))?;
    let out = InstrumentActionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        operation: request.operation.clone(),
        decision: decision.into(),
        candidate_order,
        selected_instrument_id,
        selected_site_id,
        selected_protocol_profile,
        satisfied_capability_order: satisfied,
        missing_capability_order: missing_capability,
        missing_interlock_order: missing_interlock,
        preflight_order: preflight,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        receipt_digest,
        artifact,
        effect_receipts: effects,
        executed: false,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

fn validate_request(r: &InstrumentActionRequest) -> Result<(), LaboratoryControlError> {
    if r.request_id.trim().is_empty()
        || r.federation_id.trim().is_empty()
        || r.operation.trim().is_empty()
        || r.required_capability_order.is_empty()
        || r.required_interlock_order.is_empty()
        || r.semantic_profile.trim().is_empty()
        || r.study_order.is_empty()
        || r.modality_order.is_empty()
        || r.capabilities.is_empty()
        || !canonical(&r.required_capability_order)
        || !canonical(&r.required_interlock_order)
        || !canonical(&r.study_order)
        || !canonical(&r.modality_order)
        || !canonical(&r.adversarial_events)
        || !digest(&r.replay_identity)
        || r.budget == 0
        || r.max_budget == 0
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(invalid("instrument request identity, closure, digest, budget, locality, or boundary is invalid"));
    }
    let mut ids = BTreeSet::new();
    for c in &r.capabilities {
        if c.instrument_id.trim().is_empty()
            || c.site_id.trim().is_empty()
            || c.protocol_profile.trim().is_empty()
            || c.supported_operation_order.is_empty()
            || !canonical(&c.supported_operation_order)
            || !canonical(&c.interlock_order)
            || c.semantic_profile.trim().is_empty()
            || !digest(&c.calibration_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.replay_identity)
            || !ids.insert(format!("{}@{}", c.instrument_id, c.site_id))
        {
            return Err(invalid(format!(
                "instrument capability {} is malformed or duplicated",
                c.instrument_id
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
    fn cap(id: &str) -> InstrumentCapability {
        InstrumentCapability {
            instrument_id: id.into(),
            site_id: "site-a".into(),
            protocol_profile: "ome-ngff-0.5".into(),
            supported_operation_order: vec!["image.acquire".into()],
            interlock_order: vec!["preflight-signed".into()],
            semantic_profile: "preclinical-neural".into(),
            calibration_digest: h("calibration"),
            provenance_digest: h(&format!("provenance:{id}")),
            replay_identity: h("replay"),
            raw_data_local: true,
            permitted: true,
        }
    }
    fn req(caps: Vec<InstrumentCapability>) -> InstrumentActionRequest {
        InstrumentActionRequest {
            request_id: "request:instrument".into(),
            federation_id: "fed:lab".into(),
            operation: "image.acquire".into(),
            required_capability_order: vec!["image.acquire".into()],
            required_interlock_order: vec!["preflight-signed".into()],
            target_instrument_id: None,
            semantic_profile: "preclinical-neural".into(),
            study_order: vec!["study-a".into()],
            modality_order: vec!["imaging".into()],
            capabilities: caps,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            network_available: true,
            raw_data_local: true,
            aggregate_only: true,
            budget: 4,
            max_budget: 8,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = laboratory_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2)
    }
    #[test]
    fn qualified_is_non_executing() {
        let r = preflight_instrument_action(&req(vec![cap("scope-a")])).unwrap();
        assert_eq!(r.decision, "qualified");
        assert!(!r.executed);
        assert_eq!(r.effect_receipts.len(), 2)
    }
    #[test]
    fn absent_capability_is_unresolved() {
        let mut c = cap("scope-a");
        c.supported_operation_order = vec!["omics.sequence".into()];
        let r = preflight_instrument_action(&req(vec![c])).unwrap();
        assert_eq!(r.decision, "unresolved");
        assert!(!r.uncertainty.is_empty())
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = req(vec![cap("scope-a")]);
        q.policy_allow = false;
        let r = preflight_instrument_action(&q).unwrap();
        assert_eq!(r.decision, "blocked")
    }
    #[test]
    fn network_partition_blocks() {
        let mut q = req(vec![cap("scope-a")]);
        q.network_available = false;
        let r = preflight_instrument_action(&q).unwrap();
        assert_eq!(r.decision, "blocked")
    }
    #[test]
    fn duplicate_rejected() {
        let q = req(vec![cap("scope-a"), cap("scope-a")]);
        assert!(preflight_instrument_action(&q).is_err())
    }
    #[test]
    fn deterministic_selection() {
        let a = preflight_instrument_action(&req(vec![cap("scope-b"), cap("scope-a")])).unwrap();
        let b = preflight_instrument_action(&req(vec![cap("scope-a"), cap("scope-b")])).unwrap();
        assert_eq!(a.candidate_order, b.candidate_order);
        assert_eq!(a.receipt_digest, b.receipt_digest)
    }
}
