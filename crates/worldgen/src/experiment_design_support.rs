//! Deterministic power-aware experiment design for Worldgen P09 F01-F04.
use std::collections::{BTreeMap, BTreeSet};
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.experiment-design-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignCandidate {
    pub design_id: String,
    pub objective: String,
    pub sample_size: u32,
    pub power_milli: u32,
    pub variance_milli: u32,
    pub attrition_milli: u32,
    pub replication_milli: u32,
    pub evidence_state: String,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignQuestion {
    pub request_id: String,
    pub objective: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_design_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub candidates: Vec<ExperimentDesignCandidate>,
    pub replay_identity: ContentHash,
    pub min_power_milli: u32,
    pub max_variance_milli: u32,
    pub max_attrition_milli: u32,
    pub min_replication_milli: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignPortfolio {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_design_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub power_milli_order: Vec<u32>,
    pub variance_milli_order: Vec<u32>,
    pub attrition_milli_order: Vec<u32>,
    pub replication_milli_order: Vec<u32>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub contradiction: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExperimentDesignError {
    #[error("invalid experiment design request: {0}")] Invalid(String),
    #[error("experiment design artifact failed: {0}")] Artifact(String),
    #[error("invalid experiment design portfolio: {0}")] Portfolio(String),
}

fn valid_hash(v: &ContentHash) -> bool { v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }
fn ordered(v: &[String]) -> bool { v.windows(2).all(|p| p[0] < p[1]) }
fn sorted(mut v: Vec<String>) -> Vec<String> { v.sort(); v.dedup(); v }

impl ExperimentDesignPortfolio {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|v| v.as_str()) != Some(CONTENT_TYPE)
            || !self.raw_data_local || !self.aggregate_only
            || self.request_id.trim().is_empty() || self.objective.trim().is_empty()
            || self.purpose.trim().is_empty() || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty() || self.effect_receipts.is_empty()
            || ![&self.replay_identity, &self.portfolio_digest].into_iter().all(valid_hash)
        { return Err(ExperimentDesignError::Portfolio("design identity, candidates, locality, digests, or effects are incomplete".into())); }
        for v in [&self.required_design_order, &self.candidate_order, &self.selected_order, &self.unresolved_order, &self.blocked_order, &self.omitted_order, &self.omissions, &self.uncertainty, &self.contradiction, &self.negative_evidence, &self.effect_receipts] {
            if !ordered(v) { return Err(ExperimentDesignError::Portfolio("experiment design vectors are not canonical".into())); }
        }
        let ids = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self.selected_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).chain(&self.omitted_order).cloned().collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len() || parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
            || self.power_milli_order.len() != self.candidate_order.len() || self.variance_milli_order.len() != self.candidate_order.len()
            || self.attrition_milli_order.len() != self.candidate_order.len() || self.replication_milli_order.len() != self.candidate_order.len()
        { return Err(ExperimentDesignError::Portfolio("design states or score vectors do not partition".into())); }
        if self.artifact.get("content_hash").and_then(|v| v.as_str()) != Some(self.portfolio_digest.as_str()) { return Err(ExperimentDesignError::Portfolio("design artifact digest is inconsistent".into())); }
        if self.effect_receipts.iter().any(|e| e != "block:unsafe-release" && !e.starts_with("design:worldgen-experiment:")) { return Err(ExperimentDesignError::Portfolio("design effect is outside experiment-design gate".into())); }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["research program lead","preclinical neuroscientist","bioinformatician","imaging core scientist"],"behavior":format!("compute power-aware preclinical experiment designs for {scale}"),"value":"makes power, variance, attrition, replication, evidence, omissions, and authority explicit before a study is proposed","input_schema":input_schema,"output_schema":"ExecutableExperimentDesign1@1","effects":["design:worldgen-experiment","block:unsafe-release"],"permissions":["design:local-preclinical-study"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY,"contract_version":version})
}

pub fn design(request: &ExperimentDesignQuestion, feature_id: &str, version: &str, scale: &str, require_federation: bool) -> Result<ExperimentDesignPortfolio, ExperimentDesignError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.purpose.trim().is_empty() || request.semantic_profile.trim().is_empty()
        || request.required_design_order.is_empty() || request.candidate_order.is_empty() || request.candidate_order != sorted(request.candidate_order.clone())
        || request.required_design_order != sorted(request.required_design_order.clone()) || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local || !request.aggregate_only || request.min_power_milli > 1000 || request.max_variance_milli > 1000 || request.max_attrition_milli > 1000 || request.min_replication_milli > 1000 || !valid_hash(&request.replay_identity)
    { return Err(ExperimentDesignError::Invalid("design identity, ordering, thresholds, locality, boundary, or replay is invalid".into())); }
    if require_federation && !request.federation_approved { return Err(ExperimentDesignError::Invalid("experiment design federation approval is required".into())); }
    let ids = request.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut by_id = BTreeMap::new();
    for c in &request.candidates {
        if !ids.contains(&c.design_id) || c.design_id.trim().is_empty() || c.objective.trim().is_empty() || c.sample_size == 0 || c.boundary != PRECLINICAL_BOUNDARY || !c.raw_data_local || c.replay_identity != request.replay_identity || c.power_milli > 1000 || c.variance_milli > 1000 || c.attrition_milli > 1000 || c.replication_milli > 1000 || !valid_hash(&c.evidence_digest) || !valid_hash(&c.provenance_digest) || !valid_hash(&c.artifact_digest) || !valid_hash(&c.replay_identity) { return Err(ExperimentDesignError::Invalid("design candidate identity, metrics, provenance, locality, replay, or boundary is invalid".into())); }
        if by_id.insert(c.design_id.clone(), c).is_some() { return Err(ExperimentDesignError::Invalid("duplicate experiment design candidate".into())); }
    }
    let required = request.required_design_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = Vec::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omitted = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut contradiction = BTreeSet::new(); let mut negative = BTreeSet::new();
    for id in &ids { match by_id.get(id) {
        None => { omitted.insert(id.clone()); omissions.insert(format!("design:{}:missing", id)); },
        Some(c) if !request.policy_allow || !request.protected_closure || !c.permitted => { blocked.insert(id.clone()); omissions.insert(format!("design:{}:policy-or-permission-blocked", id)); },
        Some(c) if c.negative_result => { unresolved.insert(id.clone()); negative.insert(format!("design:{}:negative-result-retained", id)); },
        Some(c) if c.evidence_state == "contradicted" => { unresolved.insert(id.clone()); contradiction.insert(format!("design:{}:contradicted", id)); },
        Some(c) if c.evidence_state == "unknown" || c.evidence_state == "unmeasured" => { unresolved.insert(id.clone()); uncertainty.insert(format!("design:{}:{}", id, c.evidence_state)); },
        Some(c) if c.power_milli < request.min_power_milli || c.variance_milli > request.max_variance_milli || c.attrition_milli > request.max_attrition_milli || c.replication_milli < request.min_replication_milli => { unresolved.insert(id.clone()); uncertainty.insert(format!("design:{}:threshold-not-met", id)); },
        Some(c) if c.evidence_state != "supported" && c.evidence_state != "proven" => { unresolved.insert(id.clone()); uncertainty.insert(format!("design:{}:evidence-not-qualified", id)); },
        Some(c) => selected.push(c),
    }}
    selected.sort_by(|a,b| b.power_milli.cmp(&a.power_milli).then(a.variance_milli.cmp(&b.variance_milli)).then(a.attrition_milli.cmp(&b.attrition_milli)).then(b.replication_milli.cmp(&a.replication_milli)).then(a.design_id.cmp(&b.design_id)));
    let selected_ids = selected.iter().map(|c| c.design_id.clone()).collect::<BTreeSet<_>>();
    let authority = request.policy_allow && request.protected_closure && (!require_federation || request.federation_approved);
    let disposition = if !authority { "blocked" } else if selected.is_empty() { "unknown" } else if selected_ids.len() == ids.len() && omissions.is_empty() && unresolved.is_empty() && blocked.is_empty() { "qualified" } else { "partial" };
    let effects = if disposition == "blocked" { vec!["block:unsafe-release".into()] } else { vec![format!("design:worldgen-experiment:{}", request.objective)] };
    let power = request.candidate_order.iter().map(|id| by_id.get(id).map(|c| c.power_milli).unwrap_or(0)).collect::<Vec<_>>();
    let variance = request.candidate_order.iter().map(|id| by_id.get(id).map(|c| c.variance_milli).unwrap_or(0)).collect::<Vec<_>>();
    let attrition = request.candidate_order.iter().map(|id| by_id.get(id).map(|c| c.attrition_milli).unwrap_or(0)).collect::<Vec<_>>();
    let replication = request.candidate_order.iter().map(|id| by_id.get(id).map(|c| c.replication_milli).unwrap_or(0)).collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"request_id":request.request_id,"objective":request.objective,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"scale":scale,"disposition":disposition,"required_design_order":required,"candidate_order":ids,"selected_order":selected_ids,"unresolved_order":unresolved,"blocked_order":blocked,"omitted_order":omitted,"power_milli_order":power,"variance_milli_order":variance,"attrition_milli_order":attrition,"replication_milli_order":replication,"omissions":omissions,"uncertainty":uncertainty,"contradiction":contradiction,"negative_evidence":negative,"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload).map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?;
    let receipt = ExperimentDesignPortfolio { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:version.into(), feature_id:feature_id.into(), request_id:request.request_id.clone(), objective:request.objective.clone(), purpose:request.purpose.clone(), semantic_profile:request.semantic_profile.clone(), disposition:disposition.into(), required_design_order:required.iter().cloned().collect(), candidate_order:ids.iter().cloned().collect(), selected_order:selected_ids.iter().cloned().collect(), unresolved_order:unresolved.iter().cloned().collect(), blocked_order:blocked.iter().cloned().collect(), omitted_order:omitted.iter().cloned().collect(), power_milli_order:power, variance_milli_order:variance, attrition_milli_order:attrition, replication_milli_order:replication, omissions:omissions.into_iter().collect(), uncertainty:uncertainty.into_iter().collect(), contradiction:contradiction.into_iter().collect(), negative_evidence:negative.into_iter().collect(), replay_identity:request.replay_identity.clone(), portfolio_digest:digest.clone(), effect_receipts:effects, artifact:json!({"artifact_id":format!("worldgen-experiment-design:{}",request.objective),"content_type":CONTENT_TYPE,"content_hash":digest,"boundary":PRECLINICAL_BOUNDARY}), raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v:&str)->ContentHash { ContentHash::of_bytes(v.as_bytes()) }
    fn request()->ExperimentDesignQuestion { let replay=hash("replay"); let c=ExperimentDesignCandidate{design_id:"design:a".into(),objective:"obj".into(),sample_size:32,power_milli:900,variance_milli:200,attrition_milli:100,replication_milli:850,evidence_state:"supported".into(),evidence_digest:hash("e"),provenance_digest:hash("p"),artifact_digest:hash("a"),replay_identity:replay.clone(),permitted:true,raw_data_local:true,negative_result:false,boundary:PRECLINICAL_BOUNDARY.into()}; ExperimentDesignQuestion{request_id:"req:a".into(),objective:"obj".into(),purpose:"design".into(),semantic_profile:"preclinical-neural".into(),required_design_order:vec!["design:a".into()],candidate_order:vec!["design:a".into()],candidates:vec![c],replay_identity:replay,min_power_milli:800,max_variance_milli:500,max_attrition_milli:300,min_replication_milli:800,policy_allow:true,protected_closure:true,federation_approved:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn qualified_design(){ let r=design(&request(),"AFA-worldgen-P09-F01","worldgen-local-experiment-design/1.0","local single-study",false).unwrap(); assert_eq!(r.disposition,"qualified"); assert_eq!(r.selected_order,vec!["design:a"]); }
    #[test] fn low_power_is_uncertain(){ let mut q=request(); q.candidates[0].power_milli=100; let r=design(&q,"AFA-worldgen-P09-F02","worldgen-multimodal-experiment-design/1.0","multimodal multi-study",false).unwrap(); assert_eq!(r.disposition,"unknown"); assert!(!r.uncertainty.is_empty()); }
}
