//! Multimodal quality-control workflow fabric for the CLI.
//!
//! Atlas feature: `AFA-cli-P07-F14`.
//!
//! The fabric turns caller-supplied QC observations into a deterministic, resumable release
//! plan. It never reads raw instrument bytes, changes a sample, contacts an instrument, or
//! promotes unknown quality into a pass.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P07-F14";
pub const CONTRACT_VERSION: &str = "cli-multimodal-quality-control-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "QualityWorkflowDraft5@1";
pub const OUTPUT_SCHEMA: &str = "QualityWorkflowRun8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityObservation {
    pub observation_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub metric: String,
    pub observed_milli: i32,
    pub threshold_milli: i32,
    pub baseline_milli: Option<i32>,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub local_data: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityWorkflowDraft {
    pub request_id: String,
    pub run_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub required_observation_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub observations: Vec<QualityObservation>,
    pub stage_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityWorkflowRun {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub run_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub required_observation_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub passed_observation_order: Vec<String>,
    pub pending_observation_order: Vec<String>,
    pub quarantined_observation_order: Vec<String>,
    pub blocked_observation_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
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
pub enum QualityWorkflowError {
    #[error("invalid CLI quality-control workflow draft: {0}")]
    Invalid(String),
    #[error("quality-control workflow artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

const STAGES: [&str; 5] = ["admit", "measure", "checkpoint", "quarantine", "retain-receipt"];

impl QualityWorkflowRun {
    pub fn validate(&self) -> Result<(), QualityWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.stage_order != STAGES.iter().map(|value| (*value).to_string()).collect::<Vec<_>>()
            || self.required_observation_order.is_empty()
            || self.required_modality_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(QualityWorkflowError::Invalid("quality workflow identity, stage protocol, locality, modalities, observations, or effects are incomplete".into()));
        }
        for values in [
            &self.required_observation_order,
            &self.required_modality_order,
            &self.passed_observation_order,
            &self.pending_observation_order,
            &self.quarantined_observation_order,
            &self.blocked_observation_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(QualityWorkflowError::Invalid("quality workflow ordering is not canonical".into()));
            }
        }
        let required = self.required_observation_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self.passed_observation_order.iter().chain(self.pending_observation_order.iter()).chain(self.quarantined_observation_order.iter()).chain(self.blocked_observation_order.iter()).cloned().collect::<Vec<_>>();
        if required.len() != self.required_observation_order.len() || states.len() != required.len() || states.iter().cloned().collect::<BTreeSet<_>>() != required || self.missing_modality_order.iter().any(|modality| !self.required_modality_order.contains(modality)) {
            return Err(QualityWorkflowError::Invalid("quality observation or modality states do not partition the plan".into()));
        }
        for digest in [&self.checkpoint_digest, &self.replay_identity, &self.workflow_digest, &self.artifact.content_hash] {
            if digest.as_str().len() != 64 {
                return Err(QualityWorkflowError::Invalid("quality workflow digest is invalid".into()));
            }
        }
        if self.artifact.content_type != "application/vnd.aurora.quality-workflow-run+json" {
            return Err(QualityWorkflowError::Invalid("quality workflow artifact type is invalid".into()));
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("retain:quality-workflow:") && !effect.starts_with("quarantine:quality-workflow:") && effect != "block:unsafe-release") {
            return Err(QualityWorkflowError::Invalid("quality workflow effect is outside retention/quarantine gate".into()));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, QualityWorkflowError> {
        self.validate()?;
        ContentHash::of_value(&serde_json::to_value(self).map_err(|error| QualityWorkflowError::Artifact(error.to_string()))?).map_err(|error| QualityWorkflowError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "cli".into(), consumers: BTreeSet::from(["AURORA extension developer".into(), "quality-control operator".into(), "instrument gateway reviewer".into()]), behavior: "compiles multimodal quality observations into a checkpointed pass/pending/quarantine workflow without touching instruments or raw bytes".into(), value: "prevents unmeasured, contradictory, omitted, or threshold-failing quality evidence from being promoted as a research pass".into(), inputs: vec![TypedPort { name: "quality_workflow_draft".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "quality_workflow_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from(["retain:quality-receipts".into(), "quarantine:quality-observations".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }, EvidenceReference { source_id: "anndata".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }, EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "quality-control operator".into(), reason: "approve quarantine release and aggregate-only quality exchange".into() }], autonomy_tier: AutonomyTier::A2, surfaces: BTreeSet::from([ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure(draft: &QualityWorkflowDraft) -> Result<QualityWorkflowRun, QualityWorkflowError> {
    if draft.request_id.trim().is_empty() || draft.run_id.trim().is_empty() || draft.scope.trim().is_empty() || draft.semantic_profile.trim().is_empty() || draft.required_observation_order.is_empty() || !canonical(&draft.required_observation_order) || draft.required_modality_order.is_empty() || !canonical(&draft.required_modality_order) || draft.stage_order != STAGES.iter().map(|value| (*value).to_string()).collect::<Vec<_>>() || !draft.raw_data_local || !draft.aggregate_only || draft.budget_units == 0 || draft.max_budget_units == 0 || draft.budget_units > draft.max_budget_units || draft.boundary != PRECLINICAL_BOUNDARY {
        return Err(QualityWorkflowError::Invalid("quality workflow identity, stages, modalities, locality, aggregate boundary, or budget is invalid".into()));
    }
    let required = draft.required_observation_order.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = draft.required_modality_order.iter().cloned().collect::<BTreeSet<_>>();
    if required.len() != draft.required_observation_order.len() || modalities.len() != draft.required_modality_order.len() { return Err(QualityWorkflowError::Invalid("required quality observations or modalities are duplicated".into())); }
    let mut seen = BTreeSet::new(); let mut passed = BTreeSet::new(); let mut pending = BTreeSet::new(); let mut quarantined = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut observed_modalities = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for observation in &draft.observations {
        if observation.observation_id.trim().is_empty() || !required.contains(&observation.observation_id) || !seen.insert(observation.observation_id.clone()) || observation.study_id.trim().is_empty() || observation.modality.trim().is_empty() || !modalities.contains(&observation.modality) || observation.metric.trim().is_empty() || observation.baseline_milli.is_none() || observation.provenance_digest.is_none() || observation.semantic_profile != draft.semantic_profile || observation.replay_identity != draft.replay_identity || !observation.local_data || !canonical(&observation.omissions) || !canonical(&observation.uncertainty) { return Err(QualityWorkflowError::Invalid("quality observation identity, modality, baseline, provenance, profile, replay, locality, or annotations are invalid".into())); }
        observed_modalities.insert(observation.modality.clone()); for item in &observation.omissions { omissions.insert(format!("observation:{}:{item}", observation.observation_id)); } for item in &observation.uncertainty { uncertainty.insert(format!("observation:{}:{item}", observation.observation_id)); } if observation.negative_result { negative.insert(format!("observation:{}:negative-result", observation.observation_id)); }
        let threshold_ok = observation.observed_milli >= observation.threshold_milli;
        match observation.evidence_state { EvidenceState::Contradicted => { quarantined.insert(observation.observation_id.clone()); negative.insert(format!("observation:{}:contradicted", observation.observation_id)); }, EvidenceState::Unknown | EvidenceState::Speculative => { pending.insert(observation.observation_id.clone()); uncertainty.insert(format!("observation:{}:evidence-state", observation.observation_id)); }, EvidenceState::Proven | EvidenceState::Supported if threshold_ok && observation.omissions.is_empty() && observation.uncertainty.is_empty() => { passed.insert(observation.observation_id.clone()); }, _ => { quarantined.insert(observation.observation_id.clone()); omissions.insert(format!("observation:{}:threshold-or-closure", observation.observation_id)); } }
    }
    for id in required.difference(&seen) { pending.insert((*id).clone()); omissions.insert(format!("missing-observation:{id}")); uncertainty.insert(format!("missing-observation:{id}")); }
    let missing_modality = modalities.difference(&observed_modalities).cloned().collect::<BTreeSet<_>>(); for modality in &missing_modality { omissions.insert(format!("missing-modality:{modality}")); uncertainty.insert(format!("missing-modality:{modality}")); }
    let mut violations = BTreeSet::new(); if !draft.policy_allow { violations.insert("policy".into()); } if !draft.protected_closure { violations.insert("protected-closure".into()); } if !draft.signed_approval { violations.insert("signed-approval".into()); } for event in &draft.adversarial_events { violations.insert(format!("adversarial:{event}")); omissions.insert(format!("workflow:adversarial:{event}")); }
    let global_block = !violations.is_empty() || !draft.adversarial_events.is_empty(); let disposition = if global_block { "blocked" } else if !pending.is_empty() || !quarantined.is_empty() || !missing_modality.is_empty() || !uncertainty.is_empty() { "quarantine" } else { "qualified" }; if global_block { blocked = required.clone(); passed.clear(); pending.clear(); quarantined.clear(); }
    let passed_order = passed.into_iter().collect::<Vec<_>>(); let pending_order = pending.into_iter().collect::<Vec<_>>(); let quarantined_order = quarantined.into_iter().collect::<Vec<_>>(); let blocked_order = blocked.into_iter().collect::<Vec<_>>(); let checkpoint = json!({"request_id":draft.request_id,"run_id":draft.run_id,"stage_order":draft.stage_order,"required_observation_order":draft.required_observation_order,"passed_observation_order":passed_order,"pending_observation_order":pending_order,"quarantined_observation_order":quarantined_order,"blocked_observation_order":blocked_order,"replay_identity":draft.replay_identity}); let checkpoint_digest = ContentHash::of_value(&checkpoint).map_err(|error| QualityWorkflowError::Artifact(error.to_string()))?; let workflow_digest = ContentHash::of_value(&json!({"checkpoint_digest":checkpoint_digest,"disposition":disposition,"semantic_profile":draft.semantic_profile})).map_err(|error| QualityWorkflowError::Artifact(error.to_string()))?; let artifact = TypedResearchArtifact::from_payload(format!("cli-quality-workflow:{}",draft.request_id),"application/vnd.aurora.quality-workflow-run+json",&checkpoint,Vec::<SemanticLoss>::new(),vec![ProvenanceLink{source_id:draft.run_id.clone(),relation:"quality-workflow-checkpoint".into(),digest:checkpoint_digest.clone()}]).map_err(|error| QualityWorkflowError::Artifact(error.to_string()))?; let effect_receipts = if disposition == "qualified" { vec![format!("retain:quality-workflow:{}",draft.request_id)] } else if disposition == "quarantine" { vec![format!("quarantine:quality-workflow:{}",draft.request_id)] } else { vec!["block:unsafe-release".into()] }; let run = QualityWorkflowRun{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),contract_version:CONTRACT_VERSION.into(),feature_id:FEATURE_ID.into(),request_id:draft.request_id.clone(),run_id:draft.run_id.clone(),scope:draft.scope.clone(),semantic_profile:draft.semantic_profile.clone(),disposition:disposition.into(),stage_order:draft.stage_order.clone(),required_observation_order:draft.required_observation_order.clone(),required_modality_order:draft.required_modality_order.clone(),passed_observation_order:passed_order,pending_observation_order:pending_order,quarantined_observation_order:quarantined_order,blocked_observation_order:blocked_order,missing_modality_order:missing_modality.into_iter().collect(),omission_order:omissions.into_iter().collect(),uncertainty_order:uncertainty.into_iter().collect(),negative_evidence_order:negative.into_iter().collect(),checkpoint_digest,replay_identity:draft.replay_identity.clone(),workflow_digest,artifact,effect_receipts,raw_data_local:draft.raw_data_local,aggregate_only:draft.aggregate_only,boundary:PRECLINICAL_BOUNDARY.into()}; run.validate()?; Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash { ContentHash::of_bytes(b"cli-quality-workflow") }
    fn observation(id:&str,state:EvidenceState,modality:&str)->QualityObservation { QualityObservation{observation_id:id.into(),study_id:"study-a".into(),modality:modality.into(),semantic_profile:"qc:v1".into(),metric:"signal-to-noise".into(),observed_milli:900,threshold_milli:700,baseline_milli:Some(800),evidence_state:state,artifact_digest:hash(),provenance_digest:Some(hash()),replay_identity:hash(),local_data:true,omissions:Vec::new(),uncertainty:Vec::new(),negative_result:false} }
    fn draft()->QualityWorkflowDraft { QualityWorkflowDraft{request_id:"request:cli-quality".into(),run_id:"run:cli-quality".into(),scope:"organoid-qc".into(),semantic_profile:"qc:v1".into(),required_observation_order:vec!["obs-a".into(),"obs-b".into()],required_modality_order:vec!["imaging".into(),"omics".into()],observations:vec![observation("obs-a",EvidenceState::Supported,"imaging"),observation("obs-b",EvidenceState::Proven,"omics")],stage_order:STAGES.iter().map(|value|(*value).to_string()).collect(),replay_identity:hash(),policy_allow:true,protected_closure:true,signed_approval:true,raw_data_local:true,aggregate_only:true,budget_units:10,max_budget_units:10,adversarial_events:Vec::new(),boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn qualified_qc_retains_receipt(){let run=assure(&draft()).unwrap();assert_eq!(run.disposition,"qualified");assert_eq!(run.passed_observation_order,vec!["obs-a","obs-b"]);}
    #[test] fn unknown_qc_quarantines(){let mut value=draft();value.observations[0].evidence_state=EvidenceState::Unknown;let run=assure(&value).unwrap();assert_eq!(run.disposition,"quarantine");assert!(run.pending_observation_order.contains(&"obs-a".into()));}
    #[test] fn missing_modality_is_quarantine(){let mut value=draft();value.observations.pop();let run=assure(&value).unwrap();assert_eq!(run.disposition,"quarantine");assert!(run.missing_modality_order.contains(&"omics".into()));}
    #[test] fn contradiction_quarantines_and_retains_negative(){let mut value=draft();value.observations[0].evidence_state=EvidenceState::Contradicted;let run=assure(&value).unwrap();assert_eq!(run.disposition,"quarantine");assert!(run.negative_evidence_order.iter().any(|item|item.contains("contradicted")));}
    #[test] fn policy_and_adversarial_block(){let mut value=draft();value.policy_allow=false;value.adversarial_events=vec!["poisoned-qc".into()];let run=assure(&value).unwrap();assert_eq!(run.disposition,"blocked");assert_eq!(run.effect_receipts,vec!["block:unsafe-release"]);}
    #[test] fn manifest_is_a2_cli_and_federated(){let manifest=capability_manifest();assert_eq!(manifest.autonomy_tier,AutonomyTier::A2);assert!(manifest.effects.contains(&Effect::FederationExport));assert!(manifest.surfaces.contains(&ResearchSurface::Cli));}
}
