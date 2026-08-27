//! Federated continual interpretation and visualization interoperability gateway.
//!
//! Atlas feature: `AFA-cli-P14-F24`.
//!
//! This gateway exchanges only typed, digest-only interpretation summaries. It does not render
//! images, fit models, move raw observations, or make a clinical decision. Unsupported and
//! contradictory panels remain visible and a non-qualified federation always fails closed.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ProvenanceLink, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P14-F24";
pub const CONTRACT_VERSION: &str = "cli-federated-continual-interpretation-visualization-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult5@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationPanel {
    pub panel_id: String,
    pub label: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub interpretation_score_milli: u16,
    pub result_digest: ContentHash,
    pub visualization_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub comparable: bool,
    pub local_data: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationGatewayRequest {
    pub request_id: String,
    pub federation_id: String,
    pub source_institution: String,
    pub target_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_panel_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub panels: Vec<InterpretationPanel>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget: u64,
    pub max_budget: u64,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationGatewayEnvelope {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub source_institution: String,
    pub target_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub panel_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub exchanged_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_panel_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationGatewayError {
    #[error("invalid interpretation gateway request: {0}")]
    Invalid(String),
    #[error("interpretation gateway artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> InterpretationGatewayError {
    InterpretationGatewayError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

pub fn interpretation_interoperability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "cli".into(),
        consumers: [
            "integration engineer".into(),
            "imaging core reviewer".into(),
            "federation operator".into(),
        ]
        .into(),
        behavior: "negotiates digest-only federated interpretation and visualization panels with semantic, evidence, provenance, replay, policy, and locality gates".into(),
        value: "lets institutions compare reproducible preclinical interpretations without exporting raw observations or treating unsupported displays as conclusions".into(),
        inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::FederationExport, Effect::WriteLocalArtifact].into(),
        permissions: ["exchange:interpretation-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "federation interpretation steward".into(), reason: "aggregate interpretation exchange requires institutional authorization".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl InterpretationGatewayEnvelope {
    pub fn validate(&self) -> Result<(), InterpretationGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.source_institution.trim().is_empty()
            || self.target_institution.trim().is_empty()
            || self.source_institution == self.target_institution
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || !matches!(self.disposition.as_str(), "qualified" | "unresolved" | "blocked")
            || self.panel_order.is_empty()
            || self.ranked_order.len() != self.panel_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("interpretation envelope identity, locality, partitions, disposition, or effects are incomplete"));
        }
        for values in [
            &self.panel_order,
            &self.exchanged_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_panel_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("interpretation envelope ordering is not canonical"));
            }
        }
        let panels = self.panel_order.iter().collect::<BTreeSet<_>>();
        let partitions = self
            .exchanged_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if partitions.iter().any(|id| !panels.contains(id))
            || partitions.len() != panels.len()
            || partitions.iter().collect::<BTreeSet<_>>().len() != partitions.len()
            || self.missing_panel_order.iter().any(|id| panels.contains(id))
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != panels
        {
            return Err(invalid("interpretation panel states do not partition observed panels"));
        }
        for value in [&self.replay_identity, &self.envelope_digest, &self.artifact.content_hash] {
            if !digest(value) {
                return Err(invalid("interpretation envelope digest is invalid"));
            }
        }
        self.artifact.validate_metadata().map_err(|error| InterpretationGatewayError::Artifact(error.to_string()))?;
        if self.artifact.content_type != "application/vnd.aurora.interactive-interpretation+json" {
            return Err(invalid("interpretation envelope artifact type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1 || !self.effect_receipts[0].starts_with("exchange:interpretation-artifact:") {
                return Err(invalid("qualified interpretation exchange effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified interpretation exchange must block release"));
        }
        Ok(())
    }
}

pub fn assure_interpretation_exchange(
    request: &InterpretationGatewayRequest,
) -> Result<InterpretationGatewayEnvelope, InterpretationGatewayError> {
    validate_request(request)?;
    let mut panels = request.panels.clone();
    panels.sort_by(|left, right| right.interpretation_score_milli.cmp(&left.interpretation_score_milli).then(left.panel_id.cmp(&right.panel_id)));
    let ranked_order = panels.iter().map(|panel| panel.panel_id.clone()).collect::<Vec<_>>();
    let mut panel_order = ranked_order.clone(); panel_order.sort();
    let panel_map = panels.iter().map(|panel| (panel.panel_id.clone(), panel)).collect::<std::collections::BTreeMap<_, _>>();
    let required = request.required_panel_order.iter().cloned().collect::<BTreeSet<_>>();
    let missing_panel_order = required.iter().filter(|id| !panel_map.contains_key(*id)).cloned().collect::<Vec<_>>();
    let studies = request.required_study_order.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request.required_modality_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut exchanged = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut contradiction = BTreeSet::new(); let mut negative = BTreeSet::new();
    for panel in &panels {
        if panel.negative_result { negative.insert(format!("{}:negative-result", panel.panel_id)); }
        omissions.extend(panel.omissions.iter().map(|item| format!("{}:{item}", panel.panel_id)));
        uncertainty.extend(panel.uncertainty.iter().map(|item| format!("{}:{item}", panel.panel_id)));
        if panel.evidence_state == EvidenceState::Contradicted { blocked.insert(panel.panel_id.clone()); contradiction.insert(format!("{}:contradicted-evidence", panel.panel_id)); continue; }
        if matches!(panel.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) { unresolved.insert(panel.panel_id.clone()); uncertainty.insert(format!("{}:evidence-unresolved", panel.panel_id)); continue; }
        let panel_studies = panel.study_order.iter().cloned().collect::<BTreeSet<_>>(); let panel_modalities = panel.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let complete = panel.label.trim() != "" && panel.semantic_profile == request.semantic_profile && panel.replay_identity == request.replay_identity && panel.source_digest_valid() && panel.provenance_digest.is_some() && studies.is_subset(&panel_studies) && modalities.is_subset(&panel_modalities) && panel.comparable && panel.omissions.is_empty() && panel.uncertainty.is_empty() && panel.local_data && panel.permitted && panel.interpretation_score_milli >= 600;
        if complete && matches!(panel.evidence_state, EvidenceState::Proven | EvidenceState::Supported) { exchanged.insert(panel.panel_id.clone()); }
        else { unresolved.insert(panel.panel_id.clone()); if panel.provenance_digest.is_none() { omissions.insert(format!("{}:provenance-missing", panel.panel_id)); } if !studies.is_subset(&panel_studies) { omissions.insert(format!("{}:required-study-coverage-incomplete", panel.panel_id)); } if !modalities.is_subset(&panel_modalities) { omissions.insert(format!("{}:required-modality-coverage-incomplete", panel.panel_id)); } if panel.interpretation_score_milli < 600 { uncertainty.insert(format!("{}:interpretation-threshold-not-met", panel.panel_id)); } if !panel.local_data || !panel.permitted { blocked.insert(panel.panel_id.clone()); unresolved.remove(&panel.panel_id); omissions.insert(format!("{}:locality-or-permission-denied", panel.panel_id)); } }
    }
    for id in &missing_panel_order { omissions.insert(format!("{id}:required-panel-missing")); }
    let missing_study_order = request.required_study_order.iter().filter(|study| !panels.iter().any(|panel| panel.study_order.contains(study))).cloned().collect::<Vec<_>>();
    let missing_modality_order = request.required_modality_order.iter().filter(|modality| !panels.iter().any(|panel| panel.modality_order.contains(modality))).cloned().collect::<Vec<_>>();
    for study in &missing_study_order { omissions.insert(format!("required-study-missing:{study}")); }
    for modality in &missing_modality_order { omissions.insert(format!("required-modality-missing:{modality}")); }
    negative.extend(request.adversarial_events.iter().map(|event| format!("adversarial:{event}")));
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.federation_approved || !request.raw_data_local || !request.aggregate_only || request.budget > request.max_budget || !request.adversarial_events.is_empty();
    if !request.policy_allow { uncertainty.insert("request:policy-denied".into()); } if !request.protected_closure { uncertainty.insert("request:protected-closure-incomplete".into()); } if !request.signed_approval || !request.federation_approved { uncertainty.insert("request:institutional-approval-incomplete".into()); } if request.budget > request.max_budget { omissions.insert("request:budget-ceiling-exceeded".into()); }
    let disposition = if global_block { "blocked" } else if missing_panel_order.is_empty() && missing_study_order.is_empty() && missing_modality_order.is_empty() && !exchanged.is_empty() && unresolved.is_empty() && blocked.is_empty() { "qualified" } else { "unresolved" };
    let exchanged_order = exchanged.into_iter().collect::<Vec<_>>(); let unresolved_order = unresolved.into_iter().collect::<Vec<_>>(); let blocked_order = blocked.into_iter().collect::<Vec<_>>(); let omission_order = omissions.into_iter().collect::<Vec<_>>(); let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>(); let contradiction_order = contradiction.into_iter().collect::<Vec<_>>(); let negative_evidence_order = negative.into_iter().collect::<Vec<_>>(); let adversarial_event_order = request.adversarial_events.clone();
    let effect_receipts = if disposition == "qualified" { vec![format!("exchange:interpretation-artifact:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"source_institution":request.source_institution,"target_institution":request.target_institution,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"panel_order":panel_order,"ranked_order":ranked_order,"exchanged_order":exchanged_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_panel_order":missing_panel_order,"missing_study_order":missing_study_order,"missing_modality_order":missing_modality_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"contradiction_order":contradiction_order,"negative_evidence_order":negative_evidence_order,"adversarial_event_order":adversarial_event_order,"replay_identity":request.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":request.raw_data_local,"aggregate_only":request.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let envelope_digest = ContentHash::of_value(&payload).map_err(|error| InterpretationGatewayError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(format!("cli-interpretation:{}",request.request_id),"application/vnd.aurora.interactive-interpretation+json",&payload,Vec::new(),vec![ProvenanceLink{source_id:format!("federation:{}",request.federation_id),relation:"derived-from-local-interpretation-manifest".into(),digest:request.replay_identity.clone()}]).map_err(|error| InterpretationGatewayError::Artifact(error.to_string()))?;
    let envelope = InterpretationGatewayEnvelope { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:CONTRACT_VERSION.into(), feature_id:FEATURE_ID.into(), request_id:request.request_id.clone(), federation_id:request.federation_id.clone(), source_institution:request.source_institution.clone(), target_institution:request.target_institution.clone(), purpose:request.purpose.clone(), semantic_profile:request.semantic_profile.clone(), disposition:disposition.into(), panel_order, ranked_order, exchanged_order, unresolved_order, blocked_order, missing_panel_order, missing_study_order, missing_modality_order, omission_order, uncertainty_order, contradiction_order, negative_evidence_order, adversarial_event_order, replay_identity:request.replay_identity.clone(), envelope_digest, artifact, effect_receipts, raw_data_local:request.raw_data_local, aggregate_only:request.aggregate_only, boundary:PRECLINICAL_BOUNDARY.into() };
    envelope.validate()?; Ok(envelope)
}

impl InterpretationPanel {
    fn source_digest_valid(&self) -> bool { digest(&self.result_digest) && digest(&self.visualization_digest) }
}

fn validate_request(request: &InterpretationGatewayRequest) -> Result<(), InterpretationGatewayError> {
    if request.request_id.trim().is_empty() || request.federation_id.trim().is_empty() || request.source_institution.trim().is_empty() || request.target_institution.trim().is_empty() || request.source_institution == request.target_institution || request.purpose.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_panel_order.is_empty() || request.required_study_order.is_empty() || request.required_modality_order.is_empty() || request.panels.is_empty() || !canonical(&request.required_panel_order) || !canonical(&request.required_study_order) || !canonical(&request.required_modality_order) || !canonical(&request.adversarial_events) || !digest(&request.replay_identity) || request.budget == 0 || request.max_budget == 0 || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || !request.aggregate_only { return Err(invalid("interpretation request identity, federation, requirements, budget, locality, or boundary is invalid")); }
    let mut seen = BTreeSet::new();
    for panel in &request.panels { if panel.panel_id.trim().is_empty() || panel.label.trim().is_empty() || !seen.insert(panel.panel_id.clone()) || panel.study_order.is_empty() || panel.modality_order.is_empty() || !canonical(&panel.study_order) || !canonical(&panel.modality_order) || panel.interpretation_score_milli > 1000 || !digest(&panel.result_digest) || !digest(&panel.visualization_digest) || panel.provenance_digest.as_ref().is_some_and(|value| !digest(value)) || !digest(&panel.replay_identity) || panel.semantic_profile.trim().is_empty() || !canonical(&panel.omissions) || !canonical(&panel.uncertainty) { return Err(invalid(format!("panel {} is malformed or duplicated",panel.panel_id))); } }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value:&str)->ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn panel(id:&str,state:EvidenceState,score:u16)->InterpretationPanel { InterpretationPanel{panel_id:id.into(),label:format!("panel-{id}"),study_order:vec!["study-a".into()],modality_order:vec!["imaging".into(),"omics".into()],semantic_profile:"preclinical-neural-organoid".into(),evidence_state:state,interpretation_score_milli:score,result_digest:hash(&format!("result-{id}")),visualization_digest:hash(&format!("viz-{id}")),provenance_digest:Some(hash(&format!("prov-{id}"))),replay_identity:hash("replay"),omissions:vec![],uncertainty:vec![],negative_result:false,comparable:true,local_data:true,permitted:true} }
    fn request(panels:Vec<InterpretationPanel>)->InterpretationGatewayRequest { InterpretationGatewayRequest{request_id:"request-1".into(),federation_id:"federation-1".into(),source_institution:"site-a".into(),target_institution:"site-b".into(),purpose:"interpretation-exchange".into(),semantic_profile:"preclinical-neural-organoid".into(),required_panel_order:vec!["panel-a".into()],required_study_order:vec!["study-a".into()],required_modality_order:vec!["imaging".into(),"omics".into()],panels,replay_identity:hash("replay"),policy_allow:true,protected_closure:true,signed_approval:true,federation_approved:true,raw_data_local:true,aggregate_only:true,budget:4,max_budget:8,adversarial_events:vec![],boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn manifest_is_a2(){let m=interpretation_interoperability_manifest();assert_eq!(m.autonomy_tier,AutonomyTier::A2);m.validate().unwrap();}
    #[test] fn supported_panel_exchanges(){let e=assure_interpretation_exchange(&request(vec![panel("panel-a",EvidenceState::Supported,900)])).unwrap();assert_eq!(e.disposition,"qualified");e.validate().unwrap();}
    #[test] fn missing_modality_unresolved(){let mut q=request(vec![panel("panel-a",EvidenceState::Supported,900)]);q.required_modality_order.push("spatial".into());q.required_modality_order.sort();let e=assure_interpretation_exchange(&q).unwrap();assert_eq!(e.disposition,"unresolved");}
    #[test] fn unknown_and_contradiction_retained(){let mut q=request(vec![panel("panel-a",EvidenceState::Unknown,900),panel("panel-b",EvidenceState::Contradicted,900)]);q.required_panel_order=vec!["panel-a".into()];let e=assure_interpretation_exchange(&q).unwrap();assert!(e.unresolved_order.contains(&"panel-a".into()));assert!(e.blocked_order.contains(&"panel-b".into()));}
    #[test] fn adversarial_blocks(){let mut q=request(vec![panel("panel-a",EvidenceState::Supported,900)]);q.adversarial_events=vec!["poisoned-panel".into()];let e=assure_interpretation_exchange(&q).unwrap();assert_eq!(e.effect_receipts,vec!["block:unsafe-release"]);}
    #[test] fn duplicate_rejected(){let q=request(vec![panel("panel-a",EvidenceState::Supported,900),panel("panel-a",EvidenceState::Supported,800)]);assert!(matches!(assure_interpretation_exchange(&q),Err(InterpretationGatewayError::Invalid(_))));}
    #[test] fn ranking_deterministic(){let a=assure_interpretation_exchange(&request(vec![panel("panel-b",EvidenceState::Supported,700),panel("panel-a",EvidenceState::Supported,900)])).unwrap();let b=assure_interpretation_exchange(&request(vec![panel("panel-a",EvidenceState::Supported,900),panel("panel-b",EvidenceState::Supported,700)])).unwrap();assert_eq!(a.ranked_order,b.ranked_order);assert_eq!(a.envelope_digest,b.envelope_digest);}
}
