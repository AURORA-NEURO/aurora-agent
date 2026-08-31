//! Federated continual experiment-design interoperability gateway (`AFA-lab-P09-F24`).
//!
//! The gateway negotiates versioned design capabilities from caller-supplied manifests and
//! emits a deterministic, content-addressed executable-design artifact. It never executes a
//! protocol, contacts an instrument, or moves raw preclinical observations.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P09-F24";
pub const CONTRACT_VERSION: &str =
    "lab-federated-continual-experiment-design-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective4@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.executable-experiment-design-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective4 {
    pub objective_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_modalities: Vec<String>,
    pub required_controls: Vec<String>,
    pub protocol_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignCapability4 {
    pub provider_id: String,
    pub api_versions: Vec<String>,
    pub modalities: Vec<String>,
    pub controls: Vec<String>,
    pub instrument_profile: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub evidence_state: EvidenceState,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub objective: ExperimentObjective4,
    pub replay_identity: ContentHash,
    pub capabilities: Vec<DesignCapability4>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignAssignment8 {
    pub provider_id: String,
    pub modality: String,
    pub protocol_version: String,
    pub instrument_profile: String,
    pub controls: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesign8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub negotiated_protocol_version: String,
    pub disposition: String,
    pub provider_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub missing_control_order: Vec<String>,
    pub comparability_conflict_order: Vec<String>,
    pub migration_loss_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub assignments: Vec<DesignAssignment8>,
    pub replay_identity: ContentHash,
    pub design_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LabExperimentDesignGatewayError {
    #[error("invalid experiment-design request: {0}")]
    Invalid(String),
    #[error("experiment-design artifact failed: {0}")]
    Artifact(String),
    #[error("experiment-design output failed: {0}")]
    Output(String),
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn text(value: &str) -> bool {
    !value.trim().is_empty()
}

pub fn lab_experiment_design_interoperability_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"lab".into(),consumers:BTreeSet::from(["agent developer".into(),"preclinical study designer".into(),"protocol integrator".into()]),behavior:"negotiate versioned experiment-design capabilities into a deterministic executable-design artifact without dispatching protocols".into(),value:"lets independent research institutions compare design capabilities while preserving migration loss, omissions, and local-data boundaries".into(),inputs:vec![TypedPort{name:"experiment_objective".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"executable_design".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:BTreeSet::from([Effect::ReadLocalData,Effect::WriteLocalArtifact]),permissions:BTreeSet::from(["negotiate:design-capabilities".into()]),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"cwl-v1.2".into(),state:EvidenceState::Supported,locator:Some("https://www.commonwl.org/specification/".into())},EvidenceReference{source_id:"ga4gh-wes".into(),state:EvidenceState::Supported,locator:Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into())}],authority_requirements:vec![AuthorityRequirement{role:"institution study-design steward".into(),reason:"approve negotiated design contracts before any downstream protocol execution".into()}],autonomy_tier:AutonomyTier::A2,surfaces:BTreeSet::from([ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator]),boundary:PRECLINICAL_BOUNDARY.into()}
}
fn validate_request(
    request: &ExperimentDesignRequest4,
) -> Result<(), LabExperimentDesignGatewayError> {
    if request.schema_version != INPUT_SCHEMA
        || !text(&request.request_id)
        || !text(&request.objective.objective_id)
        || !text(&request.objective.requester)
        || !text(&request.objective.purpose)
        || !text(&request.objective.semantic_profile)
        || !text(&request.objective.protocol_version)
        || request.objective.required_modalities.is_empty()
        || !ordered(&request.objective.required_modalities)
        || !ordered(&request.objective.required_controls)
        || !digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LabExperimentDesignGatewayError::Invalid(
            "identity, objective closure, replay, or boundary is invalid".into(),
        ));
    }
    for capability in &request.capabilities {
        if !text(&capability.provider_id)
            || !ordered(&capability.api_versions)
            || !ordered(&capability.modalities)
            || !ordered(&capability.controls)
            || !text(&capability.instrument_profile)
            || !digest(&capability.artifact_digest)
            || !digest(&capability.provenance_digest)
            || capability.replay_identity != request.replay_identity
            || !ordered(&capability.omission_order)
        {
            return Err(LabExperimentDesignGatewayError::Invalid(
                "capability identity, ordering, digest, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}
impl ExecutableExperimentDesign8 {
    pub fn validate(&self) -> Result<(), LabExperimentDesignGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !text(&self.request_id)
            || !text(&self.objective_id)
            || !text(&self.negotiated_protocol_version)
            || self.effect_receipts.is_empty()
        {
            return Err(LabExperimentDesignGatewayError::Output(
                "design identity, locality, protocol, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.provider_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_modality_order,
            &self.missing_control_order,
            &self.comparability_conflict_order,
            &self.migration_loss_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(LabExperimentDesignGatewayError::Output(
                    "design ordering is not canonical".into(),
                ));
            }
        }
        let all = self.provider_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all.len() != self.provider_order.len() || all != parts {
            return Err(LabExperimentDesignGatewayError::Output(
                "provider dispositions do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.design_digest)
            || self.artifact.content_hash != self.design_digest
        {
            return Err(LabExperimentDesignGatewayError::Output(
                "design digest or artifact metadata is invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| LabExperimentDesignGatewayError::Output(e.to_string()))
    }
}

pub fn negotiate_lab_experiment_design(
    request: &ExperimentDesignRequest4,
) -> Result<ExecutableExperimentDesign8, LabExperimentDesignGatewayError> {
    validate_request(request)?;
    let mut caps = request.capabilities.clone();
    caps.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
    let provider_order = caps
        .iter()
        .map(|c| c.provider_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_modality = BTreeSet::new();
    let mut missing_control = BTreeSet::new();
    let mut conflicts = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut assignments = Vec::new();
    for cap in &caps {
        omission.extend(
            cap.omission_order
                .iter()
                .map(|v| format!("{}:{v}", cap.provider_id)),
        );
        if cap.negative_result {
            negative.insert(format!("{}:negative-result", cap.provider_id));
        }
        let mm = request
            .objective
            .required_modalities
            .iter()
            .filter(|m| !cap.modalities.contains(m))
            .cloned()
            .collect::<Vec<_>>();
        let mc = request
            .objective
            .required_controls
            .iter()
            .filter(|m| !cap.controls.contains(m))
            .cloned()
            .collect::<Vec<_>>();
        let version = cap
            .api_versions
            .contains(&request.objective.protocol_version);
        let hard = !cap.signed
            || !cap.permitted
            || !cap.raw_data_local
            || !cap.aggregate_only
            || cap.replay_identity != request.replay_identity
            || cap.semantic_profile != request.objective.semantic_profile
            || !digest(&cap.artifact_digest)
            || !digest(&cap.provenance_digest);
        if hard {
            blocked.insert(cap.provider_id.clone());
            omission.insert(format!(
                "{}:authorization-locality-or-semantic-blocked",
                cap.provider_id
            ));
        } else if !mm.is_empty()
            || !mc.is_empty()
            || !version
            || matches!(
                cap.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
        {
            unresolved.insert(cap.provider_id.clone());
            missing_modality.extend(mm.iter().map(|v| format!("{}:{v}", cap.provider_id)));
            missing_control.extend(mc.iter().map(|v| format!("{}:{v}", cap.provider_id)));
            if !version {
                migration.insert(format!("{}:protocol-version-migration", cap.provider_id));
            }
            uncertainty.insert(format!("{}:incomplete-evidence", cap.provider_id));
        } else if cap.evidence_state == EvidenceState::Contradicted {
            unresolved.insert(cap.provider_id.clone());
            negative.insert(format!("{}:contradicted", cap.provider_id));
        } else {
            selected.insert(cap.provider_id.clone());
            for modality in &request.objective.required_modalities {
                assignments.push(DesignAssignment8 {
                    provider_id: cap.provider_id.clone(),
                    modality: modality.clone(),
                    protocol_version: request.objective.protocol_version.clone(),
                    instrument_profile: cap.instrument_profile.clone(),
                    controls: request.objective.required_controls.clone(),
                });
            }
        }
    }
    let mut profiles = BTreeSet::new();
    for a in &assignments {
        profiles.insert(format!("{}:{}", a.modality, a.instrument_profile));
    }
    if profiles.len() > request.objective.required_modalities.len() {
        conflicts.insert("instrument-profile:cross-provider-incomparable".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(provider_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        assignments.clear();
        omission.insert("request:governance-or-locality-blocked".into());
    }
    let disposition = if global || (!blocked.is_empty() && selected.is_empty()) {
        "blocked"
    } else if !unresolved.is_empty()
        || !blocked.is_empty()
        || !missing_modality.is_empty()
        || !missing_control.is_empty()
        || !conflicts.is_empty()
        || !migration.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omission.insert("request:design-closure-not-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"objective_id":request.objective.objective_id,"requester":request.objective.requester,"purpose":request.objective.purpose,"semantic_profile":request.objective.semantic_profile,"negotiated_protocol_version":request.objective.protocol_version,"disposition":disposition,"provider_order":provider_order,"selected_order":selected.iter().cloned().collect::<Vec<_>>(),"unresolved_order":unresolved.iter().cloned().collect::<Vec<_>>(),"blocked_order":blocked.iter().cloned().collect::<Vec<_>>(),"missing_modality_order":missing_modality.iter().cloned().collect::<Vec<_>>(),"missing_control_order":missing_control.iter().cloned().collect::<Vec<_>>(),"comparability_conflict_order":conflicts.iter().cloned().collect::<Vec<_>>(),"migration_loss_order":migration.iter().cloned().collect::<Vec<_>>(),"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"assignments":assignments,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("fabric-executable-design-8:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| LabExperimentDesignGatewayError::Artifact(e.to_string()))?;
    let design_digest = artifact.content_hash.clone();
    let effect_receipts = if disposition == "qualified" {
        vec![format!(
            "negotiate:design-capability:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let out = ExecutableExperimentDesign8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective.objective_id.clone(),
        requester: request.objective.requester.clone(),
        purpose: request.objective.purpose.clone(),
        semantic_profile: request.objective.semantic_profile.clone(),
        negotiated_protocol_version: request.objective.protocol_version.clone(),
        disposition: disposition.into(),
        provider_order: provider_order.clone(),
        selected_order: selected.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        missing_modality_order: missing_modality.into_iter().collect(),
        missing_control_order: missing_control.into_iter().collect(),
        comparability_conflict_order: conflicts.into_iter().collect(),
        migration_loss_order: migration.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        assignments: serde_json::from_value(payload["assignments"].clone()).unwrap(),
        replay_identity: request.replay_identity.clone(),
        design_digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn negotiate_lab_experiment_design_json(value: &Value) -> Result<Value, String> {
    let request: ExperimentDesignRequest4 =
        serde_json::from_value(value.get("request").cloned().ok_or("request is required")?)
            .map_err(|e| format!("invalid fabric experiment-design request: {e}"))?;
    serde_json::to_value(negotiate_lab_experiment_design(&request).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_lab_experiment_design_json(
    value: &Value,
) -> Result<ExecutableExperimentDesign8, String> {
    let receipt: ExecutableExperimentDesign8 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid fabric experiment-design receipt: {e}"))?;
    receipt.validate().map_err(|e| e.to_string())?;
    if receipt.feature_id != FEATURE_ID {
        return Err("fabric experiment-design feature id mismatch".into());
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ExperimentDesignRequest4 {
        ExperimentDesignRequest4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "r".into(),
            objective: ExperimentObjective4 {
                objective_id: "o".into(),
                requester: "agent developer".into(),
                purpose: "design".into(),
                semantic_profile: "profile:v1".into(),
                required_modalities: vec!["imaging".into()],
                required_controls: vec!["vehicle".into()],
                protocol_version: "cwl-1.2".into(),
            },
            replay_identity: h("replay"),
            capabilities: vec![DesignCapability4 {
                provider_id: "site-a".into(),
                api_versions: vec!["cwl-1.2".into()],
                modalities: vec!["imaging".into()],
                controls: vec!["vehicle".into()],
                instrument_profile: "ome".into(),
                semantic_profile: "profile:v1".into(),
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("replay"),
                signed: true,
                permitted: true,
                raw_data_local: true,
                aggregate_only: true,
                evidence_state: EvidenceState::Supported,
                omission_order: vec![],
                negative_result: false,
            }],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_design() {
        assert_eq!(
            negotiate_lab_experiment_design(&req()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn version_migration_is_explicit() {
        let mut r = req();
        r.capabilities[0].api_versions = vec!["cwl-1.1".into()];
        let o = negotiate_lab_experiment_design(&r).unwrap();
        assert_eq!(o.disposition, "unresolved");
        assert!(!o.migration_loss_order.is_empty())
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allow = false;
        assert_eq!(
            negotiate_lab_experiment_design(&r).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn missing_modality_is_explicit() {
        let mut r = req();
        r.capabilities[0].modalities.clear();
        assert!(!negotiate_lab_experiment_design(&r)
            .unwrap()
            .missing_modality_order
            .is_empty())
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            negotiate_lab_experiment_design(&req())
                .unwrap()
                .design_digest,
            negotiate_lab_experiment_design(&req())
                .unwrap()
                .design_digest
        )
    }
}
