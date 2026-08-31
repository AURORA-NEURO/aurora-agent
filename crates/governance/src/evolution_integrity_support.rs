//! Governance P32: deterministic schema-evolution integrity qualification.
//!
//! This product surface turns migration, digest, deprecation, and compatibility evidence into a
//! replayable release gate. It never migrates data or silently approves an undeclared loss.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str = "application/vnd.aurora.governance.evolution-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionChange4 {
    pub change_id: String,
    pub field_path: String,
    pub required_class: String,
    pub declared_class: String,
    pub old_version: String,
    pub new_version: String,
    pub digest_affecting: bool,
    pub loss_declared: bool,
    pub roundtrip_preserved: bool,
    pub deprecation_stage: String,
    pub evidence_state: String,
    pub evidence_digest: String,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionIntegrityRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub changes: Vec<EvolutionChange4>,
    pub required_change_order: Vec<String>,
    pub required_version_bump: String,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub policy_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: String,
    pub adversarial_events: Vec<String>,
    pub change_budget: usize,
    pub declared_change_count: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvolutionIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub change_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub class_order: Vec<String>,
    pub version_order: Vec<String>,
    pub loss_order: Vec<String>,
    pub deprecation_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: EvolutionIntegrityArtifact4,
}

#[derive(Debug, Error)]
pub enum EvolutionIntegrityError {
    #[error("evolution integrity input is invalid: {0}")]
    Invalid(String),
    #[error("evolution integrity digest failed: {0}")]
    Digest(String),
}

fn digest(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) }
fn canonical(values: &[String]) -> bool { values.windows(2).all(|w| w[0] < w[1]) }
fn invalid(message: impl Into<String>) -> EvolutionIntegrityError { EvolutionIntegrityError::Invalid(message.into()) }

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"governance","consumers":["schema registry","migration release gate","artifact verifier","research workbench"],"behavior":format!("qualify digest-safe schema evolution at {scale} ({mode})"),"value":"prevents undeclared compatibility breaks, lossy migrations, and premature deprecation from releasing","input_schema":"EvolutionIntegrityRequest4@1","output_schema":"EvolutionIntegrityCard7@1","effects":["emit:evolution-card","retain:loss-witness","block:unsafe-migration"],"permissions":["read:local-migration-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn validate_card(card: &EvolutionIntegrityCard7) -> Result<(), EvolutionIntegrityError> {
    if card.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || card.feature_id.is_empty() || card.boundary != BOUNDARY || card.artifact.boundary != BOUNDARY || !card.raw_data_local || !card.aggregate_only || !digest(&card.replay_identity) || !digest(&card.closure_digest) || card.artifact.content_type != CONTENT_TYPE || card.artifact.content_hash != card.closure_digest { return Err(invalid("identity, locality, artifact, digest, or boundary is incomplete")); }
    for values in [&card.change_order,&card.accepted_order,&card.rejected_order,&card.unknown_order,&card.omitted_order,&card.class_order,&card.version_order,&card.loss_order,&card.deprecation_order,&card.effect_receipts] { if !canonical(values) { return Err(invalid("evolution vectors are not canonical")); } }
    let ids = card.change_order.iter().collect::<BTreeSet<_>>();
    let states = card.accepted_order.iter().chain(&card.rejected_order).chain(&card.unknown_order).chain(&card.omitted_order).collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids { return Err(invalid("change states do not partition changes")); }
    Ok(())
}

pub fn qualify(request: &EvolutionIntegrityRequest4, feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Result<EvolutionIntegrityCard7, EvolutionIntegrityError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || request.request_id.trim().is_empty() || request.purpose.trim().is_empty() || request.changes.is_empty() || request.required_change_order.is_empty() || !canonical(&request.required_change_order) || request.required_version_bump.trim().is_empty() || !digest(&request.replay_identity) || request.boundary != BOUNDARY || !request.raw_data_local || !request.aggregate_only || !canonical(&request.adversarial_events) || request.declared_change_count != request.changes.len() || request.change_budget == 0 { return Err(invalid("identity, ordering, bump, digest, locality, boundary, or budget is invalid")); }
    let mut rows = request.changes.clone(); rows.sort_by(|a,b| a.change_id.cmp(&b.change_id));
    let mut seen = BTreeSet::new(); let mut accepted = BTreeSet::new(); let mut rejected = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut omitted = BTreeSet::new(); let mut classes = BTreeSet::new(); let mut versions = BTreeSet::new(); let mut losses = BTreeSet::new(); let mut deprecations = BTreeSet::new(); let mut evidence = BTreeSet::new();
    for change in &rows {
        if change.change_id.trim().is_empty() || !seen.insert(change.change_id.clone()) || change.field_path.trim().is_empty() || change.required_class.trim().is_empty() || change.declared_class.trim().is_empty() || change.old_version.trim().is_empty() || change.new_version.trim().is_empty() || change.deprecation_stage.trim().is_empty() || change.evidence_state.trim().is_empty() || !digest(&change.evidence_digest) || !change.local || !change.aggregate_only { return Err(invalid("change identity, versions, evidence, or locality is invalid")); }
        classes.insert(format!("{}:{}",change.change_id,change.declared_class)); versions.insert(format!("{}:{}->{}",change.change_id,change.old_version,change.new_version)); evidence.insert(change.evidence_digest.clone()); deprecations.insert(format!("{}:{}",change.change_id,change.deprecation_stage));
        if change.digest_affecting && change.declared_class != "major" { losses.insert(format!("{}:digest-affecting-requires-major",change.change_id)); }
        if !change.roundtrip_preserved && !change.loss_declared { losses.insert(format!("{}:undeclared-roundtrip-loss",change.change_id)); }
        if change.evidence_state == "unknown" || change.evidence_digest == request.replay_identity { unknown.insert(change.change_id.clone()); }
        else if change.deprecation_stage == "retired" || (change.digest_affecting && change.declared_class != "major") || (!change.roundtrip_preserved && !change.loss_declared) || change.declared_class != change.required_class { rejected.insert(change.change_id.clone()); }
        else if !request.required_change_order.contains(&change.change_id) { omitted.insert(change.change_id.clone()); }
        else { accepted.insert(change.change_id.clone()); }
    }
    let missing = request.required_change_order.iter().filter(|id| !seen.contains(*id)).cloned().collect::<Vec<_>>(); for id in missing { losses.insert(format!("{id}:required-change-missing")); }
    let global = !request.policy_allowed || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !request.aggregate_only || !request.adversarial_events.is_empty() || request.changes.len() > request.change_budget || request.required_version_bump == "unknown";
    if global { omitted.extend(seen.iter().cloned()); accepted.clear(); rejected.clear(); unknown.clear(); }
    let complete = request.required_change_order.iter().all(|id| seen.contains(id)); let disposition = if global { "blocked" } else if !complete || !unknown.is_empty() { "unknown" } else if !rejected.is_empty() || !omitted.is_empty() { "partial" } else { "qualified" };
    let change_order = seen.iter().cloned().collect::<Vec<_>>(); let accepted_order = accepted.into_iter().collect::<Vec<_>>(); let rejected_order = rejected.into_iter().collect::<Vec<_>>(); let unknown_order = unknown.into_iter().collect::<Vec<_>>(); let omitted_order = omitted.into_iter().collect::<Vec<_>>(); let effect_receipts = if disposition == "qualified" { vec![format!("approve:evolution:{}",request.request_id)] } else { vec!["block:unsafe-migration".into()] };
    let body = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"change_order":change_order,"accepted_order":accepted_order,"rejected_order":rejected_order,"unknown_order":unknown_order,"omitted_order":omitted_order,"class_order":classes.into_iter().collect::<Vec<_>>(),"version_order":versions.into_iter().collect::<Vec<_>>(),"loss_order":losses.into_iter().collect::<Vec<_>>(),"deprecation_order":deprecations.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    let closure_digest = ContentHash::of_value(&body).map_err(|e| EvolutionIntegrityError::Digest(e.to_string()))?.to_string();
    let artifact = EvolutionIntegrityArtifact4 { artifact_id: format!("governance-evolution:{}", request.request_id), content_type: CONTENT_TYPE.into(), content_hash: closure_digest.clone(), semantic_loss: body["omitted_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), evidence_digests: evidence.into_iter().collect(), boundary: BOUNDARY.into() };
    let card = EvolutionIntegrityCard7 { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: contract_version.into(), feature_id: feature_id.into(), request_id: request.request_id.clone(), purpose: request.purpose.clone(), disposition: disposition.into(), change_order: body["change_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), accepted_order, rejected_order, unknown_order, omitted_order, class_order: body["class_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), version_order: body["version_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), loss_order: body["loss_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), deprecation_order: body["deprecation_order"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect(), replay_identity: request.replay_identity.clone(), closure_digest, raw_data_local: true, aggregate_only: true, boundary: BOUNDARY.into(), effect_receipts, artifact };
    validate_card(&card)?; let _ = (scale, mode); Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(x: &str) -> String { ContentHash::of_bytes(x.as_bytes()).to_string() }
    fn change(id: &str) -> EvolutionChange4 { EvolutionChange4 { change_id:id.into(),field_path:format!("/field/{id}"),required_class:"minor".into(),declared_class:"minor".into(),old_version:"1.0.0".into(),new_version:"1.1.0".into(),digest_affecting:false,loss_declared:false,roundtrip_preserved:true,deprecation_stage:"active".into(),evidence_state:"supported".into(),evidence_digest:hash(&format!("evidence:{id}")),local:true,aggregate_only:true } }
    fn request(changes: Vec<EvolutionChange4>) -> EvolutionIntegrityRequest4 { EvolutionIntegrityRequest4 { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),request_id:"evolution-1".into(),purpose:"release".into(),required_change_order:changes.iter().map(|x|x.change_id.clone()).collect(),required_version_bump:"minor".into(),protected_closure:true,signed_approval:true,policy_allowed:true,raw_data_local:true,aggregate_only:true,replay_identity:hash("replay"),adversarial_events:vec![],change_budget:8,declared_change_count:changes.len(),boundary:BOUNDARY.into(),changes } }
    #[test] fn qualified() { assert_eq!(qualify(&request(vec![change("a")]),"AFA-governance-P32-F01","v1","local","inference").unwrap().disposition,"qualified"); }
    #[test] fn digest_break_rejected() { let mut c=change("a"); c.digest_affecting=true; assert_eq!(qualify(&request(vec![c]),"f","v","local","inference").unwrap().disposition,"partial"); }
    #[test] fn adversarial_blocks() { let mut q=request(vec![change("a")]); q.adversarial_events=vec!["poisoned-fixture".into()]; assert_eq!(qualify(&q,"f","v","local","inference").unwrap().disposition,"blocked"); }
}
