//! Federated continual resource-discovery control plane (`AFA-mutation-P05-F32`).
//!
//! The control plane operates only on typed endpoint and peer attestations. It produces a
//! deterministic, content-addressed `QualifiedResourceSet8` and effect receipts for local
//! capability management and aggregate-only federation. No endpoint is contacted and no raw
//! preclinical data leaves an institution.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mutation-P05-F32";
pub const CONTRACT_VERSION: &str = "mutation-federated-continual-resource-discovery-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ResourceNeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedResourceSet8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.qualified-resource-set-8+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceEvidenceState { Proven, Supported, Speculative, Contradicted, Unknown, Unmeasured }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointStatus { Available, Stale, Protected, Unavailable, Revoked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationResourceNeed4 {
    pub request_id: String, pub federation_id: String, pub requester: String, pub purpose: String,
    pub semantic_profile: String, pub required_capabilities: Vec<String>, pub allowed_origins: Vec<String>,
    pub required_protocol_version: String, pub max_results: usize, pub minimum_peer_quorum: usize,
    pub replay_identity: ContentHash, pub policy_allow: bool, pub protected_closure: bool,
    pub signed_approval: bool, pub federation_approved: bool, pub raw_data_local: bool,
    pub aggregate_only: bool, pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationResourceEndpoint4 {
    pub resource_id: String, pub endpoint_id: String, pub origin: String, pub semantic_profile: String,
    pub protocol_versions: Vec<String>, pub capabilities: Vec<String>, pub fitness_milli: i64,
    pub status: EndpointStatus, pub evidence_state: ResourceEvidenceState, pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash, pub replay_identity: ContentHash, pub signed: bool, pub permitted: bool,
    pub raw_data_local: bool, pub aggregate_only: bool, pub negative_result: bool, pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationPeerResourceSummary4 {
    pub peer_id: String, pub origin: String, pub semantic_profile: String, pub protocol_version: String,
    pub summary_digest: ContentHash, pub evidence_state: ResourceEvidenceState, pub signed: bool,
    pub aggregate_only: bool, pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResource8 {
    pub resource_id: String, pub endpoint_id: String, pub origin: String, pub protocol_version: String,
    pub fitness_milli: i64, pub capability_order: Vec<String>, pub migration_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResourceSet8 {
    pub schema_version: String, pub contract_version: String, pub feature_id: String, pub request_id: String,
    pub federation_id: String, pub requester: String, pub purpose: String, pub semantic_profile: String,
    pub negotiated_protocol_version: String, pub disposition: String, pub endpoint_order: Vec<String>,
    pub qualified_order: Vec<String>, pub unresolved_order: Vec<String>, pub blocked_order: Vec<String>,
    pub missing_capability_order: Vec<String>, pub peer_order: Vec<String>, pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>, pub resources: Vec<QualifiedResource8>, pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>, pub negative_evidence_order: Vec<String>, pub migration_order: Vec<String>,
    pub replay_identity: ContentHash, pub selection_digest: ContentHash, pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>, pub raw_data_local: bool, pub aggregate_only: bool, pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MutationResourceDiscoveryError {
    #[error("invalid mutation resource-discovery request: {0}")] Invalid(String),
    #[error("resource-discovery artifact failed: {0}")] Artifact(String),
    #[error("resource-discovery output failed: {0}")] Output(String),
}

fn canonical(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn text(value: &str) -> bool { !value.trim().is_empty() }

pub fn mutation_federated_resource_discovery_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "mutation".into(),
        consumers: BTreeSet::from(["preclinical neuroscientist".into(), "resource steward".into(), "federation operator".into()]),
        behavior: "operate typed local resource registry and aggregate-only federation attestations into a qualified resource set".into(),
        value: "makes capability fitness, evidence, locality, permissions, omissions, and peer quorum explicit before a researcher selects a resource".into(),
        inputs: vec![TypedPort { name: "resource_need".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_resource_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from(["operate:institution-node".into()]), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }, EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "institution resource steward".into(), reason: "authorize local capability management and aggregate-only federation export".into() }], autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &MutationResourceNeed4) -> Result<(), MutationResourceDiscoveryError> {
    if !text(&request.request_id) || !text(&request.federation_id) || !text(&request.requester) || !text(&request.purpose) || !text(&request.semantic_profile) || !text(&request.required_protocol_version) || request.required_capabilities.is_empty() || !canonical(&request.required_capabilities) || !canonical(&request.allowed_origins) || request.max_results == 0 || request.max_results > 4096 || !digest(&request.replay_identity) || request.boundary != PRECLINICAL_BOUNDARY { return Err(MutationResourceDiscoveryError::Invalid("identity, required capabilities, protocol, bounds, replay, or boundary is invalid".into())); }
    Ok(())
}

impl QualifiedResourceSet8 {
    pub fn validate(&self) -> Result<(), MutationResourceDiscoveryError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || !self.aggregate_only || !text(&self.request_id) || !text(&self.federation_id) || !text(&self.requester) || !text(&self.purpose) || !text(&self.semantic_profile) || !text(&self.negotiated_protocol_version) || self.effect_receipts.is_empty() { return Err(MutationResourceDiscoveryError::Output("identity, locality, protocol, or effects are incomplete".into())); }
        for values in [&self.endpoint_order,&self.qualified_order,&self.unresolved_order,&self.blocked_order,&self.missing_capability_order,&self.peer_order,&self.qualified_peer_order,&self.missing_peer_order,&self.omission_order,&self.uncertainty_order,&self.negative_evidence_order,&self.migration_order,&self.effect_receipts] { if !canonical(values) { return Err(MutationResourceDiscoveryError::Output("resource output ordering is not canonical".into())); } }
        let endpoints = self.endpoint_order.iter().cloned().collect::<BTreeSet<_>>(); let parts = self.qualified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<BTreeSet<_>>();
        if endpoints.len() != self.endpoint_order.len() || endpoints != parts { return Err(MutationResourceDiscoveryError::Output("endpoint dispositions do not partition candidates".into())); }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>(); let peer_parts = self.qualified_peer_order.iter().chain(&self.missing_peer_order).cloned().collect::<BTreeSet<_>>(); if peers.len() != self.peer_order.len() || peers != peer_parts { return Err(MutationResourceDiscoveryError::Output("peer dispositions do not partition peers".into())); }
        if !digest(&self.replay_identity) || !digest(&self.selection_digest) || self.artifact.content_hash != self.selection_digest { return Err(MutationResourceDiscoveryError::Output("resource digest or artifact metadata is invalid".into())); }
        self.artifact.validate_metadata().map_err(|error| MutationResourceDiscoveryError::Output(error.to_string()))
    }
}

pub fn operate_mutation_federated_resource_discovery(
    request: &MutationResourceNeed4, endpoints: &[MutationResourceEndpoint4], peers: &[MutationPeerResourceSummary4],
) -> Result<QualifiedResourceSet8, MutationResourceDiscoveryError> {
    validate_request(request)?;
    let mut endpoint_rows = endpoints.to_vec(); endpoint_rows.sort_by(|a,b| a.endpoint_id.cmp(&b.endpoint_id));
    let mut peer_rows = peers.to_vec(); peer_rows.sort_by(|a,b| a.peer_id.cmp(&b.peer_id));
    let endpoint_order = endpoint_rows.iter().map(|row| row.endpoint_id.clone()).collect::<Vec<_>>();
    let mut qualified = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omission = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut migration = BTreeSet::new(); let mut resources = Vec::new(); let mut provenance = Vec::new();
    for row in &endpoint_rows {
        provenance.push(row.provenance_digest.clone()); if row.negative_result { negative.insert(format!("{}:negative-result", row.endpoint_id)); }
        omission.extend(row.omission_reasons.iter().map(|reason| format!("{}:{reason}", row.endpoint_id)));
        let missing = request.required_capabilities.iter().filter(|cap| !row.capabilities.contains(cap)).cloned().collect::<Vec<_>>(); let protocol = row.protocol_versions.contains(&request.required_protocol_version); let origin = request.allowed_origins.is_empty() || request.allowed_origins.contains(&row.origin);
        let hard_block = !row.permitted || !row.signed || !row.raw_data_local || !row.aggregate_only || row.replay_identity != request.replay_identity || !digest(&row.artifact_digest) || !digest(&row.provenance_digest) || matches!(row.status, EndpointStatus::Protected | EndpointStatus::Revoked | EndpointStatus::Unavailable) || row.semantic_profile != request.semantic_profile || !origin;
        if hard_block { blocked.insert(row.endpoint_id.clone()); omission.insert(format!("{}:policy-permission-locality-or-compatibility", row.endpoint_id)); }
        else if !missing.is_empty() || !protocol || matches!(row.status, EndpointStatus::Stale) || matches!(row.evidence_state, ResourceEvidenceState::Unknown | ResourceEvidenceState::Speculative | ResourceEvidenceState::Unmeasured) { unresolved.insert(row.endpoint_id.clone()); if !missing.is_empty() { omission.extend(missing.iter().map(|cap| format!("{}:missing-capability:{cap}", row.endpoint_id))); } if !protocol { migration.insert(format!("{}:protocol-version-migration", row.endpoint_id)); } if matches!(row.evidence_state, ResourceEvidenceState::Unknown | ResourceEvidenceState::Speculative | ResourceEvidenceState::Unmeasured) { uncertainty.insert(format!("{}:evidence-incomplete", row.endpoint_id)); } }
        else if row.evidence_state == ResourceEvidenceState::Contradicted { unresolved.insert(row.endpoint_id.clone()); negative.insert(format!("{}:contradicted", row.endpoint_id)); }
        else { qualified.insert(row.endpoint_id.clone()); resources.push(QualifiedResource8 { resource_id: row.resource_id.clone(), endpoint_id: row.endpoint_id.clone(), origin: row.origin.clone(), protocol_version: request.required_protocol_version.clone(), fitness_milli: row.fitness_milli, capability_order: row.capabilities.clone(), migration_notes: Vec::new() }); }
    }
    let mut qualified_peers = BTreeSet::new(); let mut missing_peers = BTreeSet::new(); let peer_order = peer_rows.iter().map(|row| row.peer_id.clone()).collect::<Vec<_>>();
    for peer in &peer_rows { if peer.signed && peer.aggregate_only && peer.raw_data_local && peer.semantic_profile == request.semantic_profile && peer.protocol_version == request.required_protocol_version && digest(&peer.summary_digest) && peer.evidence_state != ResourceEvidenceState::Contradicted { qualified_peers.insert(peer.peer_id.clone()); } else { missing_peers.insert(peer.peer_id.clone()); uncertainty.insert(format!("peer:{}:summary-incomplete", peer.peer_id)); } }
    if qualified_peers.len() < request.minimum_peer_quorum { uncertainty.insert("request:peer-quorum-incomplete".into()); }
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.federation_approved || !request.raw_data_local || !request.aggregate_only;
    if global_block { blocked.extend(endpoint_order.iter().cloned()); qualified.clear(); unresolved.clear(); resources.clear(); omission.insert("request:governance-locality-or-federation-blocked".into()); }
    let disposition = if global_block || (blocked.len() == endpoint_order.len() && !endpoint_order.is_empty()) { "blocked" } else if qualified_peers.len() < request.minimum_peer_quorum || !unresolved.is_empty() || !blocked.is_empty() || !migration.is_empty() { "unresolved" } else { "qualified" };
    if disposition != "qualified" { omission.insert("request:resource-closure-not-ready".into()); }
    resources.sort_by(|a,b| b.fitness_milli.cmp(&a.fitness_milli).then_with(|| a.endpoint_id.cmp(&b.endpoint_id))); resources.truncate(request.max_results);
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"negotiated_protocol_version":request.required_protocol_version,"disposition":disposition,"endpoint_order":endpoint_order,"qualified_order":qualified.iter().cloned().collect::<Vec<_>>(),"unresolved_order":unresolved.iter().cloned().collect::<Vec<_>>(),"blocked_order":blocked.iter().cloned().collect::<Vec<_>>(),"missing_capability_order":omission.iter().filter(|value| value.contains(":missing-capability:")).cloned().collect::<Vec<_>>(),"peer_order":peer_order,"qualified_peer_order":qualified_peers.iter().cloned().collect::<Vec<_>>(),"missing_peer_order":missing_peers.iter().cloned().collect::<Vec<_>>(),"resources":resources,"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"migration_order":migration.iter().cloned().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("mutation-qualified-resource-set-8:{}", request.request_id), CONTENT_TYPE, &payload, Vec::new(), Vec::new()).map_err(|error| MutationResourceDiscoveryError::Artifact(error.to_string()))?;
    let selection_digest = artifact.content_hash.clone(); let effect_receipts = if disposition == "qualified" { vec![format!("exchange:permitted-summaries:{}", request.request_id), format!("manage:local-capability:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let out = QualifiedResourceSet8 { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:CONTRACT_VERSION.into(), feature_id:FEATURE_ID.into(), request_id:request.request_id.clone(), federation_id:request.federation_id.clone(), requester:request.requester.clone(), purpose:request.purpose.clone(), semantic_profile:request.semantic_profile.clone(), negotiated_protocol_version:request.required_protocol_version.clone(), disposition:disposition.into(), endpoint_order:payload["endpoint_order"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().into()).collect(), qualified_order:qualified.into_iter().collect(), unresolved_order:unresolved.into_iter().collect(), blocked_order:blocked.into_iter().collect(), missing_capability_order:payload["missing_capability_order"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().into()).collect(), peer_order:payload["peer_order"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().into()).collect(), qualified_peer_order:qualified_peers.into_iter().collect(), missing_peer_order:missing_peers.into_iter().collect(), resources:serde_json::from_value(payload["resources"].clone()).unwrap(), omission_order:omission.into_iter().collect(), uncertainty_order:uncertainty.into_iter().collect(), negative_evidence_order:negative.into_iter().collect(), migration_order:migration.into_iter().collect(), replay_identity:request.replay_identity.clone(), selection_digest, artifact, effect_receipts, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    out.validate()?; Ok(out)
}

pub fn operate_mutation_federated_resource_discovery_json(value: &Value) -> Result<Value, String> {
    let request: MutationResourceNeed4 = serde_json::from_value(value.get("request").cloned().ok_or("request is required")?).map_err(|error| format!("invalid mutation resource request: {error}"))?;
    let endpoints: Vec<MutationResourceEndpoint4> = serde_json::from_value(value.get("endpoints").cloned().ok_or("endpoints are required")?).map_err(|error| format!("invalid mutation resource endpoints: {error}"))?;
    let peers: Vec<MutationPeerResourceSummary4> = serde_json::from_value(value.get("peers").cloned().unwrap_or_else(|| json!([]))).map_err(|error| format!("invalid mutation resource peers: {error}"))?;
    serde_json::to_value(operate_mutation_federated_resource_discovery(&request, &endpoints, &peers).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

pub fn validate_mutation_federated_resource_discovery_json(value: &Value) -> Result<QualifiedResourceSet8, String> {
    let receipt: QualifiedResourceSet8 = serde_json::from_value(value.clone()).map_err(|error| format!("invalid mutation resource receipt: {error}"))?; receipt.validate().map_err(|error| error.to_string())?; if receipt.feature_id != FEATURE_ID { return Err("mutation resource feature id mismatch".into()); } Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash { ContentHash::of_bytes(v.as_bytes()) }
    fn request() -> MutationResourceNeed4 { MutationResourceNeed4 { request_id:"r".into(), federation_id:"f".into(), requester:"preclinical neuroscientist".into(), purpose:"discover imaging resource".into(), semantic_profile:"ome-ngff".into(), required_capabilities:vec!["imaging".into()], allowed_origins:vec!["site-a".into()], required_protocol_version:"drs-1.3".into(), max_results:16, minimum_peer_quorum:1, replay_identity:hash("replay"), policy_allow:true, protected_closure:true, signed_approval:true, federation_approved:true, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() } }
    fn endpoint() -> MutationResourceEndpoint4 { MutationResourceEndpoint4 { resource_id:"res".into(), endpoint_id:"ep".into(), origin:"site-a".into(), semantic_profile:"ome-ngff".into(), protocol_versions:vec!["drs-1.3".into()], capabilities:vec!["imaging".into()], fitness_milli:900, status:EndpointStatus::Available, evidence_state:ResourceEvidenceState::Supported, artifact_digest:hash("artifact"), provenance_digest:hash("prov"), replay_identity:hash("replay"), signed:true, permitted:true, raw_data_local:true, aggregate_only:true, negative_result:false, omission_reasons:Vec::new() } }
    fn peer() -> MutationPeerResourceSummary4 { MutationPeerResourceSummary4 { peer_id:"peer-a".into(), origin:"site-a".into(), semantic_profile:"ome-ngff".into(), protocol_version:"drs-1.3".into(), summary_digest:hash("summary"), evidence_state:ResourceEvidenceState::Supported, signed:true, aggregate_only:true, raw_data_local:true } }
    #[test] fn qualified_control_plane_emits_two_effects() { let out=operate_mutation_federated_resource_discovery(&request(),&[endpoint()],&[peer()]).unwrap(); assert_eq!(out.disposition,"qualified"); assert_eq!(out.effect_receipts.len(),2); }
    #[test] fn unknown_evidence_is_unresolved() { let mut e=endpoint(); e.evidence_state=ResourceEvidenceState::Unknown; let out=operate_mutation_federated_resource_discovery(&request(),&[e],&[peer()]).unwrap(); assert_eq!(out.disposition,"unresolved"); }
    #[test] fn denied_governance_blocks() { let mut r=request(); r.federation_approved=false; let out=operate_mutation_federated_resource_discovery(&r,&[endpoint()],&[peer()]).unwrap(); assert_eq!(out.disposition,"blocked"); }
    #[test] fn missing_capability_is_explicit() { let mut e=endpoint(); e.capabilities.clear(); let out=operate_mutation_federated_resource_discovery(&request(),&[e],&[peer()]).unwrap(); assert!(!out.missing_capability_order.is_empty()); }
    #[test] fn deterministic_digest() { let out=operate_mutation_federated_resource_discovery(&request(),&[endpoint()],&[peer()]).unwrap(); assert_eq!(out.selection_digest,operate_mutation_federated_resource_discovery(&request(),&[endpoint()],&[peer()]).unwrap().selection_digest); }
    #[test] fn peer_quorum_is_retained() { let mut r=request(); r.minimum_peer_quorum=2; let out=operate_mutation_federated_resource_discovery(&r,&[endpoint()],&[peer()]).unwrap(); assert!(out.uncertainty_order.iter().any(|v|v.contains("peer-quorum"))); }
}
