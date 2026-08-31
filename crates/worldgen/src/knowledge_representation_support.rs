//! Deterministic, omission-aware knowledge representation for Worldgen P04 F01-F04.
//!
//! This module treats a knowledge graph as a product artifact: nodes and relations are typed,
//! content-addressed, provenance-linked, replayable, and explicitly partitioned into qualified,
//! unknown, blocked, or omitted state.  It never infers a clinical conclusion.

use std::collections::BTreeSet;

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.knowledge-representation-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeNode {
    pub node_id: String,
    pub semantic_type: String,
    pub label: String,
    pub confidence_milli: u16,
    pub state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub negative_result: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRelation {
    pub relation_id: String,
    pub subject_id: String,
    pub predicate: String,
    pub object_id: String,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRepresentationRequest {
    pub request_id: String,
    pub namespace: String,
    pub required_node_order: Vec<String>,
    pub required_relation_order: Vec<String>,
    pub minimum_confidence_milli: u16,
    pub nodes: Vec<KnowledgeNode>,
    pub relations: Vec<KnowledgeRelation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRepresentationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub namespace: String,
    pub disposition: String,
    pub required_node_order: Vec<String>,
    pub resolved_node_order: Vec<String>,
    pub unknown_node_order: Vec<String>,
    pub blocked_node_order: Vec<String>,
    pub omitted_node_order: Vec<String>,
    pub relation_order: Vec<String>,
    pub resolved_relation_order: Vec<String>,
    pub omitted_relation_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub graph_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KnowledgeRepresentationError {
    #[error("invalid knowledge representation request: {0}")]
    Invalid(String),
    #[error("knowledge representation artifact failed: {0}")]
    Artifact(String),
    #[error("invalid knowledge representation receipt: {0}")]
    Receipt(String),
}

fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn sorted(values: &[String]) -> Vec<String> { let mut output = values.to_vec(); output.sort(); output.dedup(); output }

impl KnowledgeRepresentationReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeRepresentationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str()) != Some(CONTENT_TYPE)
            || self.artifact.get("raw_nodes").and_then(|value| value.as_bool()) != Some(false)
            || !self.raw_data_local || !self.aggregate_only || self.request_id.trim().is_empty()
            || self.namespace.trim().is_empty() || self.required_node_order.is_empty() || self.relation_order.is_empty()
            || self.effect_receipts.is_empty() || ![&self.replay_identity, &self.graph_digest].into_iter().all(digest)
        { return Err(KnowledgeRepresentationError::Receipt("graph identity, node/relation partitions, locality, digests, or effects are incomplete".into())); }
        for values in [&self.required_node_order, &self.resolved_node_order, &self.unknown_node_order, &self.blocked_node_order, &self.omitted_node_order, &self.relation_order, &self.resolved_relation_order, &self.omitted_relation_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if !ordered(values) { return Err(KnowledgeRepresentationError::Receipt("knowledge representation ordering is not canonical".into())); }
        }
        let required = self.required_node_order.iter().cloned().collect::<BTreeSet<_>>();
        let node_parts = self.resolved_node_order.iter().chain(&self.unknown_node_order).chain(&self.blocked_node_order).chain(&self.omitted_node_order).cloned().collect::<Vec<_>>();
        if required.len() != self.required_node_order.len() || node_parts.len() != required.len() || node_parts.iter().cloned().collect::<BTreeSet<_>>() != required { return Err(KnowledgeRepresentationError::Receipt("knowledge nodes do not form a complete partition".into())); }
        let relations = self.relation_order.iter().cloned().collect::<BTreeSet<_>>();
        let relation_parts = self.resolved_relation_order.iter().chain(&self.omitted_relation_order).cloned().collect::<Vec<_>>();
        if relations.len() != self.relation_order.len() || relation_parts.len() != relations.len() || relation_parts.iter().cloned().collect::<BTreeSet<_>>() != relations { return Err(KnowledgeRepresentationError::Receipt("knowledge relations do not form a complete partition".into())); }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("represent:worldgen-knowledge:")) { return Err(KnowledgeRepresentationError::Receipt("knowledge representation effect is outside the typed graph gate".into())); }
        if self.artifact.get("content_hash").and_then(|value| value.as_str()) != Some(self.graph_digest.as_str()) { return Err(KnowledgeRepresentationError::Receipt("knowledge graph artifact digest is inconsistent".into())); }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value { json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["knowledge engineer","preclinical neuroscientist","research program lead","downstream graph consumer"],"behavior":format!("compile a typed omission-aware knowledge graph for {scale}"),"value":"turns evidence into replayable typed nodes and relations without inventing unsupported facts","input_schema":input_schema,"output_schema":"KnowledgeGraphReceipt1@1","effects":["represent:worldgen-knowledge","block:unsafe-release"],"permissions":["represent:local-research-knowledge"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY,"contract_version":version}) }

pub fn represent(request: &KnowledgeRepresentationRequest, feature_id: &str, version: &str, scale: &str, require_federation: bool) -> Result<KnowledgeRepresentationReceipt, KnowledgeRepresentationError> {
    if request.request_id.trim().is_empty() || request.namespace.trim().is_empty() || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || !request.aggregate_only || request.required_node_order.is_empty() || request.relation_order_invalid() || request.required_node_order != sorted(&request.required_node_order) || request.required_node_order.iter().collect::<BTreeSet<_>>().len() != request.required_node_order.len() || !digest(&request.replay_identity) { return Err(KnowledgeRepresentationError::Invalid("graph identity, required nodes, relations, locality, boundary, ordering, or replay is invalid".into())); }
    if request.required_relation_order.iter().collect::<BTreeSet<_>>().len() != request.required_relation_order.len() || request.required_relation_order != sorted(&request.required_relation_order) { return Err(KnowledgeRepresentationError::Invalid("required relation order is not canonical".into())); }
    let required_nodes = request.required_node_order.iter().cloned().collect::<BTreeSet<_>>(); let required_relations = request.required_relation_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut nodes = request.nodes.clone(); nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id)); let mut relations = request.relations.clone(); relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    if nodes.iter().any(|node| !required_nodes.contains(&node.node_id) || node.boundary != PRECLINICAL_BOUNDARY || !node.raw_data_local || !digest(&node.evidence_digest) || !digest(&node.provenance_digest) || !digest(&node.artifact_digest) || !digest(&node.replay_identity) || node.replay_identity != request.replay_identity) { return Err(KnowledgeRepresentationError::Invalid("node identity, provenance, replay, locality, or boundary is invalid".into())); }
    if relations.iter().any(|relation| !required_relations.contains(&relation.relation_id) || !required_nodes.contains(&relation.subject_id) || !required_nodes.contains(&relation.object_id) || relation.boundary != PRECLINICAL_BOUNDARY || !relation.raw_data_local || !digest(&relation.evidence_digest) || !digest(&relation.provenance_digest) || !digest(&relation.replay_identity) || relation.replay_identity != request.replay_identity) { return Err(KnowledgeRepresentationError::Invalid("relation identity, endpoints, provenance, replay, locality, or boundary is invalid".into())); }
    let mut resolved = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omitted = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for node_id in &required_nodes { match nodes.iter().find(|node| node.node_id == *node_id) { None => { omitted.insert(node_id.clone()); omissions.insert(format!("node:{}:missing", node_id)); }, Some(node) if node.negative_result => { blocked.insert(node_id.clone()); negative.insert(format!("node:{}:negative-result-retained", node_id)); }, Some(node) if !request.policy_allow || !request.protected_closure || !node.raw_data_local || node.boundary != PRECLINICAL_BOUNDARY => { blocked.insert(node_id.clone()); omissions.insert(format!("node:{}:policy-or-locality-blocked", node_id)); }, Some(node) if node.state != EvidenceState::Supported || node.confidence_milli < request.minimum_confidence_milli => { unknown.insert(node_id.clone()); uncertainty.insert(format!("node:{}:unsupported-or-below-threshold", node_id)); }, Some(_) => { resolved.insert(node_id.clone()); } } }
    for relation_id in &required_relations { match relations.iter().find(|relation| relation.relation_id == *relation_id) { None => { omissions.insert(format!("relation:{}:missing", relation_id)); }, Some(relation) if !resolved.contains(&relation.subject_id) || !resolved.contains(&relation.object_id) => { omissions.insert(format!("relation:{}:endpoint-unresolved", relation_id)); }, Some(_) => {} } }
    if require_federation && !request.federation_approved { omissions.insert("request:federation-approval-missing".into()); }
    let authority = request.policy_allow && request.protected_closure && request.raw_data_local && (!require_federation || request.federation_approved); let disposition = if !authority { "blocked" } else if resolved.is_empty() { "unknown" } else if resolved.len() == required_nodes.len() && omissions.is_empty() && uncertainty.is_empty() && negative.is_empty() { "qualified" } else { "partial" };
    let resolved_relations = if disposition == "qualified" { required_relations.clone() } else { required_relations.iter().filter(|relation_id| relations.iter().any(|relation| relation.relation_id == **relation_id && resolved.contains(&relation.subject_id) && resolved.contains(&relation.object_id))).cloned().collect::<BTreeSet<_>>() }; let omitted_relations = required_relations.difference(&resolved_relations).cloned().collect::<BTreeSet<_>>();
    let effects = if disposition == "blocked" { vec!["block:unsafe-release".into()] } else { vec![format!("represent:worldgen-knowledge:{}", request.request_id)] };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"request_id":request.request_id,"namespace":request.namespace,"scale":scale,"disposition":disposition,"required_node_order":required_nodes,"resolved_node_order":resolved,"unknown_node_order":unknown,"blocked_node_order":blocked,"omitted_node_order":omitted,"relation_order":required_relations,"resolved_relation_order":resolved_relations,"omitted_relation_order":omitted_relations,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"effect_receipts":effects,"raw_nodes":false,"boundary":PRECLINICAL_BOUNDARY}); let graph_digest = ContentHash::of_value(&payload).map_err(|error| KnowledgeRepresentationError::Artifact(error.to_string()))?;
    let receipt = KnowledgeRepresentationReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:version.into(), feature_id:feature_id.into(), request_id:request.request_id.clone(), namespace:request.namespace.clone(), disposition:disposition.into(), required_node_order:required_nodes.iter().cloned().collect(), resolved_node_order:resolved.iter().cloned().collect(), unknown_node_order:unknown.iter().cloned().collect(), blocked_node_order:blocked.iter().cloned().collect(), omitted_node_order:omitted.iter().cloned().collect(), relation_order:required_relations.iter().cloned().collect(), resolved_relation_order:resolved_relations.iter().cloned().collect(), omitted_relation_order:omitted_relations.iter().cloned().collect(), replay_identity:request.replay_identity.clone(), graph_digest:graph_digest.clone(), omissions:omissions.into_iter().collect(), uncertainty:uncertainty.into_iter().collect(), negative_evidence:negative.into_iter().collect(), effect_receipts:effects, artifact:json!({"artifact_id":format!("worldgen-knowledge-graph:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":graph_digest,"raw_nodes":false,"boundary":PRECLINICAL_BOUNDARY}), raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

trait RelationOrderValidation { fn relation_order_invalid(&self) -> bool; }
impl RelationOrderValidation for KnowledgeRepresentationRequest { fn relation_order_invalid(&self) -> bool { self.required_relation_order.is_empty() } }

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn request() -> KnowledgeRepresentationRequest { let replay=hash("replay"); let node=KnowledgeNode{node_id:"node:a".into(),semantic_type:"gene".into(),label:"A".into(),confidence_milli:900,state:EvidenceState::Supported,evidence_digest:hash("e"),provenance_digest:hash("p"),artifact_digest:hash("a"),replay_identity:replay.clone(),negative_result:false,raw_data_local:true,boundary:PRECLINICAL_BOUNDARY.into()}; let relation=KnowledgeRelation{relation_id:"rel:a".into(),subject_id:"node:a".into(),predicate:"associated-with".into(),object_id:"node:a".into(),evidence_digest:hash("re"),provenance_digest:hash("rp"),replay_identity:replay.clone(),raw_data_local:true,boundary:PRECLINICAL_BOUNDARY.into()}; KnowledgeRepresentationRequest{request_id:"graph:req".into(),namespace:"preclinical".into(),required_node_order:vec!["node:a".into()],required_relation_order:vec!["rel:a".into()],minimum_confidence_milli:500,nodes:vec![node],relations:vec![relation],replay_identity:replay,policy_allow:true,protected_closure:true,federation_approved:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn qualified_graph_is_replayable(){let r=represent(&request(),"AFA-worldgen-P04-F01","worldgen-local-knowledge-representation/1.0","local single-study",false).unwrap();assert_eq!(r.disposition,"qualified");assert!(r.validate().is_ok());}
    #[test] fn missing_node_is_omitted(){let mut q=request();q.nodes.clear();let r=represent(&q,"AFA-worldgen-P04-F02","worldgen-multimodal-knowledge-representation/1.0","multimodal multi-study",false).unwrap();assert_eq!(r.disposition,"unknown");assert!(r.omitted_node_order.contains(&"node:a".into()));}
    #[test] fn federation_denial_blocks(){let mut q=request();q.federation_approved=false;let r=represent(&q,"AFA-worldgen-P04-F04","worldgen-federated-continual-knowledge-representation/1.0","federated continual autonomous",true).unwrap();assert_eq!(r.disposition,"blocked");}
}
