//! Prospective high-throughput provenance and signing assurance (`AFA-ids-P18-F27`).
//!
//! The harness verifies a typed artifact/derivation envelope before a research workflow may
//! release it. All lineage failures remain machine-readable; no bytes are signed, uploaded, or
//! interpreted as scientific truth by this module.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P18-F27";
pub const CONTRACT_VERSION: &str = "ids-prospective-high-throughput-provenance-signing-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ArtifactAndDerivation3@1";
pub const OUTPUT_SCHEMA: &str = "SignedProvenanceEnvelope7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.signed-provenance-envelope-7+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivationEvidenceState { Proven, Supported, Unknown, Unmeasured, Contradicted, Negative }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAndDerivation3 {
    pub artifact_id: String,
    pub derivation_id: String,
    pub parent_ids: Vec<String>,
    pub content_digest: ContentHash,
    pub actor: String,
    pub semantic_profile: String,
    pub evidence_state: DerivationEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signature_valid: bool,
    pub protected_closure: bool,
    pub local: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAndDerivationRequest3 {
    pub schema_version: String,
    pub request_id: String,
    pub semantic_profile: String,
    pub artifacts: Vec<ArtifactAndDerivation3>,
    pub expected_root: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceEnvelopeArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceEnvelope7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub artifact_order: Vec<String>,
    pub verified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_parent_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub invalid_signature_order: Vec<String>,
    pub root_mismatch_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub root_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signature_mode: String,
    pub receipt_digest: ContentHash,
    pub artifact: SignedProvenanceEnvelopeArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProspectiveProvenanceError {
    #[error("invalid artifact/derivation request: {0}")] Invalid(String),
    #[error("signed provenance envelope failed validation: {0}")] Output(String),
}

fn nonempty(value: &str) -> bool { !value.trim().is_empty() }
fn ordered(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }

pub fn prospective_provenance_assurance_manifest() -> serde_json::Value { json!({
    "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION,
    "owner_crate":"ids", "consumers":["research-object steward","provenance auditor","release operator"],
    "behavior":"verify prospective high-throughput artifact derivation lineage and detached signatures",
    "value":"prevents root drift, missing parents, cycles, invalid signatures, and incomplete evidence from entering release",
    "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA,
    "effects":["verify:provenance-envelope","block:unsafe-release"], "permissions":["evaluate:capability-runs"],
    "autonomy_tier":"A1", "boundary":PRECLINICAL_BOUNDARY
}) }

impl SignedProvenanceEnvelope7 {
    pub fn validate(&self) -> Result<(), ProspectiveProvenanceError> {
        if self.schema_version != "aurora-research-contract/1.0" || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || self.artifact.content_type != CONTENT_TYPE || self.signature_mode != "detached-digest-attestation" || !self.raw_data_local || !self.aggregate_only || !nonempty(&self.request_id) || !nonempty(&self.semantic_profile) || self.artifact_order.is_empty() || self.effect_receipts.is_empty() || !["qualified","unresolved","blocked"].contains(&self.disposition.as_str()) { return Err(ProspectiveProvenanceError::Output("identity, signature, locality, artifact, or effect closure is incomplete".into())); }
        for values in [&self.artifact_order,&self.verified_order,&self.unresolved_order,&self.blocked_order,&self.missing_parent_order,&self.cycle_order,&self.invalid_signature_order,&self.root_mismatch_order,&self.omission_order,&self.uncertainty_order,&self.negative_evidence_order,&self.effect_receipts] { if !ordered(values) { return Err(ProspectiveProvenanceError::Output("provenance ordering is not canonical".into())); } }
        let all = self.artifact_order.iter().cloned().collect::<BTreeSet<_>>(); let states = self.verified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<Vec<_>>();
        if all.len() != self.artifact_order.len() || states.len() != all.len() || BTreeSet::from_iter(states) != all { return Err(ProspectiveProvenanceError::Output("artifact states do not partition the input".into())); }
        if !digest(&self.root_digest) || !digest(&self.replay_identity) || !digest(&self.receipt_digest) || self.artifact.content_hash != self.receipt_digest || self.artifact.provenance_digests.iter().any(|value| !digest(value)) { return Err(ProspectiveProvenanceError::Output("provenance digest or artifact metadata is invalid".into())); }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("verify:provenance-envelope:")) { return Err(ProspectiveProvenanceError::Output("effect is outside the provenance gate".into())); }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ProspectiveProvenanceError> { self.validate()?; ContentHash::of_value(&serde_json::to_value(self).map_err(|error| ProspectiveProvenanceError::Output(error.to_string()))?).map_err(|error| ProspectiveProvenanceError::Output(error.to_string())) }
}

fn validate_request(request: &ArtifactAndDerivationRequest3) -> Result<(), ProspectiveProvenanceError> {
    if request.schema_version != INPUT_SCHEMA || !nonempty(&request.request_id) || !nonempty(&request.semantic_profile) || request.artifacts.is_empty() || !digest(&request.expected_root) || !digest(&request.replay_identity) || !ordered(&request.adversarial_events) || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || !request.aggregate_only { return Err(ProspectiveProvenanceError::Invalid("request identity, lineage, digest, locality, or boundary is invalid".into())); }
    let mut ids = BTreeSet::new();
    for item in &request.artifacts { if !nonempty(&item.artifact_id) || !nonempty(&item.derivation_id) || !nonempty(&item.actor) || !nonempty(&item.semantic_profile) || !digest(&item.content_digest) || !digest(&item.provenance_digest) || !digest(&item.replay_identity) || !ordered(&item.parent_ids) || !ordered(&item.omission_order) || !ids.insert(item.artifact_id.clone()) { return Err(ProspectiveProvenanceError::Invalid("artifact identity, parent ordering, digests, or uniqueness is invalid".into())); } }
    Ok(())
}

fn cycles(id: &str, parents: &BTreeMap<String, Vec<String>>, visiting: &mut BTreeSet<String>, done: &mut BTreeSet<String>, found: &mut BTreeSet<String>) { if done.contains(id) { return; } if !visiting.insert(id.to_string()) { found.insert(id.to_string()); return; } if let Some(items) = parents.get(id) { for parent in items { if parents.contains_key(parent) { cycles(parent, parents, visiting, done, found); } } } visiting.remove(id); done.insert(id.to_string()); }

pub fn assure_prospective_provenance(request: &ArtifactAndDerivationRequest3) -> Result<SignedProvenanceEnvelope7, ProspectiveProvenanceError> {
    validate_request(request)?;
    let mut artifacts = request.artifacts.clone(); artifacts.sort_by(|left,right| left.artifact_id.cmp(&right.artifact_id)); let artifact_order = artifacts.iter().map(|item| item.artifact_id.clone()).collect::<Vec<_>>();
    let known = artifact_order.iter().cloned().collect::<BTreeSet<_>>(); let parents = artifacts.iter().map(|item|(item.artifact_id.clone(),item.parent_ids.clone())).collect::<BTreeMap<_,_>>();
    let mut missing=BTreeSet::new(); let mut cycle=BTreeSet::new(); let mut invalid_signature=BTreeSet::new(); let mut unresolved=BTreeSet::new(); let mut blocked=BTreeSet::new(); let mut verified=BTreeSet::new(); let mut root_mismatch=BTreeSet::new(); let mut omission=BTreeSet::new(); let mut uncertainty=BTreeSet::new(); let mut negative=BTreeSet::new(); let mut provenance=BTreeSet::new();
    let mut visiting=BTreeSet::new(); let mut done=BTreeSet::new(); for id in &artifact_order { cycles(id,&parents,&mut visiting,&mut done,&mut cycle); }
    for item in &artifacts { let id=item.artifact_id.clone(); provenance.insert(item.provenance_digest.clone()); omission.extend(item.omission_order.iter().map(|value| format!("{id}:{value}"))); if item.negative_result || item.evidence_state==DerivationEvidenceState::Negative { negative.insert(format!("{id}:negative-result")); } if item.parent_ids.iter().any(|parent| !known.contains(parent)) { missing.insert(id.clone()); omission.insert(format!("{id}:missing-parent")); } if cycle.contains(&id) { blocked.insert(id.clone()); omission.insert(format!("{id}:cycle")); } else if !item.signature_valid { invalid_signature.insert(id.clone()); blocked.insert(id.clone()); omission.insert(format!("{id}:invalid-signature")); } else if item.semantic_profile != request.semantic_profile || !item.local { blocked.insert(id.clone()); omission.insert(format!("{id}:semantic-profile-or-locality-mismatch")); } else if item.replay_identity != request.replay_identity || !item.protected_closure { unresolved.insert(id.clone()); uncertainty.insert(format!("{id}:replay-or-protected-closure-unresolved")); } else if !matches!(item.evidence_state,DerivationEvidenceState::Proven|DerivationEvidenceState::Supported) { unresolved.insert(id.clone()); uncertainty.insert(format!("{id}:evidence-not-supported")); } else if !missing.contains(&id) { verified.insert(id); } }
    let root_payload = artifacts.iter().map(|item| (&item.artifact_id,&item.content_digest)).collect::<Vec<_>>(); let calculated_root = ContentHash::of_value(&json!(root_payload)).map_err(|error| ProspectiveProvenanceError::Output(error.to_string()))?; if calculated_root != request.expected_root { root_mismatch.extend(artifact_order.iter().cloned()); omission.insert("request:root-mismatch".into()); }
    let global_block = !request.policy_allowed || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !request.aggregate_only || !request.adversarial_events.is_empty(); if global_block { blocked.extend(artifact_order.iter().cloned()); verified.clear(); unresolved.clear(); omission.insert("request:governance-or-adversarial-gate-blocked".into()); }
    uncertainty.extend(request.adversarial_events.iter().map(|event| format!("adversarial:{event}"))); if !root_mismatch.is_empty() { blocked.extend(root_mismatch.iter().cloned()); verified.clear(); unresolved.clear(); }
    let vo=verified.iter().cloned().collect::<Vec<_>>(); let uo=unresolved.iter().cloned().collect::<Vec<_>>(); let bo=blocked.iter().cloned().collect::<Vec<_>>(); let disposition=if global_block || (vo.is_empty() && uo.is_empty()) {"blocked"} else if !missing.is_empty() || !cycle.is_empty() || !invalid_signature.is_empty() || !root_mismatch.is_empty() || !bo.is_empty() || !uo.is_empty() {"unresolved"} else {"qualified"}; if disposition!="qualified" { omission.insert("request:provenance-closure-not-ready".into()); }
    let mut payload=json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"semantic_profile":request.semantic_profile,"disposition":disposition,"artifact_order":artifact_order,"verified_order":vo,"unresolved_order":uo,"blocked_order":bo,"missing_parent_order":missing.iter().cloned().collect::<Vec<_>>(),"cycle_order":cycle.iter().cloned().collect::<Vec<_>>(),"invalid_signature_order":invalid_signature.iter().cloned().collect::<Vec<_>>(),"root_mismatch_order":root_mismatch.iter().cloned().collect::<Vec<_>>(),"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"root_digest":calculated_root,"replay_identity":request.replay_identity,"signature_mode":"detached-digest-attestation","raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY}); let receipt_digest=ContentHash::of_value(&payload).map_err(|error|ProspectiveProvenanceError::Output(error.to_string()))?; payload["receipt_digest"]=json!(receipt_digest); payload["artifact"]=json!({"artifact_id":format!("signed-provenance-envelope-7:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":receipt_digest,"semantic_loss":omission.iter().cloned().collect::<Vec<_>>(),"provenance_digests":provenance.iter().cloned().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY}); payload["effect_receipts"]=json!(if disposition=="qualified"{vec![format!("verify:provenance-envelope:{}",request.request_id)]}else{vec!["block:unsafe-release".to_string()]}); let output:SignedProvenanceEnvelope7=serde_json::from_value(payload).map_err(|error|ProspectiveProvenanceError::Output(error.to_string()))?; output.validate()?; Ok(output)
}

#[cfg(test)]
mod tests { use super::*; fn h(value:&str)->ContentHash{ContentHash::of_bytes(value.as_bytes())} fn item(id:&str,parent:Vec<String>)->ArtifactAndDerivation3{ArtifactAndDerivation3{artifact_id:id.into(),derivation_id:format!("d:{id}"),parent_ids:parent,content_digest:h(id),actor:"researcher".into(),semantic_profile:"prov-v1".into(),evidence_state:DerivationEvidenceState::Proven,provenance_digest:h("prov"),replay_identity:h("replay"),signature_valid:true,protected_closure:true,local:true,omission_order:vec![],negative_result:false}} fn request()->ArtifactAndDerivationRequest3{let a=item("a",vec![]);let b=item("b",vec!["a".into()]);let root=ContentHash::of_value(&json!([(&a.artifact_id,&a.content_digest),(&b.artifact_id,&b.content_digest)])).unwrap();ArtifactAndDerivationRequest3{schema_version:INPUT_SCHEMA.into(),request_id:"prov:req".into(),semantic_profile:"prov-v1".into(),artifacts:vec![b,a],expected_root:root,replay_identity:h("replay"),policy_allowed:true,protected_closure:true,signed_approval:true,raw_data_local:true,aggregate_only:true,adversarial_events:vec![],boundary:PRECLINICAL_BOUNDARY.into()}} #[test]fn qualified(){assert_eq!(assure_prospective_provenance(&request()).unwrap().disposition,"qualified")} #[test]fn missing_parent_is_visible(){let mut q=request();q.artifacts[0].parent_ids=vec!["missing".into()];assert!(!assure_prospective_provenance(&q).unwrap().missing_parent_order.is_empty())} #[test]fn policy_blocks(){let mut q=request();q.policy_allowed=false;assert_eq!(assure_prospective_provenance(&q).unwrap().disposition,"blocked")} }
