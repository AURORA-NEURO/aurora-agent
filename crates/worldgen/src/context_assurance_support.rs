//! Evaluation-heavy context-compilation assurance harnesses (P03 F25-F28).
//!
//! This harness verifies a compiled context against a preregistered baseline, replication-site
//! quorum, replay identity, policy, and protected closure.  It never runs the scientific method
//! under test; it issues a qualification or an explicit falsification/uncertainty receipt.

use std::collections::BTreeSet;

use super::context_compilation_support::{self, ContextCompilationRequest};
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.context-assurance-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceRequest {
    pub context_request: ContextCompilationRequest,
    pub benchmark_id: String,
    pub benchmark_digest: ContentHash,
    pub baseline_discovery_rate_milli: u32,
    pub candidate_discovery_rate_milli: u32,
    pub required_site_order: Vec<String>,
    pub achieved_site_order: Vec<String>,
    pub minimum_site_quorum: u16,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub benchmark_id: String,
    pub disposition: String,
    pub baseline_discovery_rate_milli: u32,
    pub candidate_discovery_rate_milli: u32,
    pub delta_discovery_rate_milli: i32,
    pub required_site_order: Vec<String>,
    pub achieved_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub context_disposition: String,
    pub context_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub assurance_digest: ContentHash,
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
pub enum ContextAssuranceError {
    #[error("invalid context assurance request: {0}")]
    Invalid(String),
    #[error("context assurance compilation failed: {0}")]
    Compilation(String),
    #[error("context assurance artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn sorted(values: &[String]) -> Vec<String> {
    let mut output = values.to_vec();
    output.sort();
    output.dedup();
    output
}

impl ContextAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str())
                != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str())
                != Some(CONTENT_TYPE)
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.required_site_order.is_empty()
            || !ordered(&self.required_site_order)
            || !ordered(&self.achieved_site_order)
            || !ordered(&self.missing_site_order)
            || self.effect_receipts.is_empty()
            || ![&self.context_digest, &self.benchmark_digest, &self.replay_identity, &self.assurance_digest]
                .into_iter().all(digest)
        {
            return Err(ContextAssuranceError::Invalid(
                "assurance identity, benchmark, quorum, locality, ordering, digests, or effects are incomplete".into(),
            ));
        }
        let required = self.required_site_order.iter().cloned().collect::<BTreeSet<_>>();
        let achieved = self.achieved_site_order.iter().cloned().collect::<BTreeSet<_>>();
        let missing = self.missing_site_order.iter().cloned().collect::<BTreeSet<_>>();
        if required.len() != self.required_site_order.len()
            || achieved.union(&missing).cloned().collect::<BTreeSet<_>>() != required
            || achieved.intersection(&missing).next().is_some()
        {
            return Err(ContextAssuranceError::Invalid("replication sites do not partition".into()));
        }
        if self.artifact.get("content_hash").and_then(|value| value.as_str()) != Some(self.assurance_digest.as_str())
            || self.artifact.get("raw_data").and_then(|value| value.as_bool()) != Some(false)
        {
            return Err(ContextAssuranceError::Invalid("assurance artifact digest or raw-data boundary is inconsistent".into()));
        }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("assure:worldgen-context:")) {
            return Err(ContextAssuranceError::Invalid("assurance effect is outside qualification gate".into()));
        }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": version,
        "owner_crate": "worldgen",
        "consumers": ["benchmark curator", "research program lead", "independent replication site", "downstream evaluator"],
        "behavior": format!("assure context compilation against baseline and replication quorum at {scale}"),
        "value": "turns context output into a reproducible, falsifiable qualification with explicit negative evidence",
        "input_schema": input_schema,
        "output_schema": "EvaluationCardContext1@1",
        "effects": ["assure:worldgen-context", "block:unsafe-release"],
        "permissions": ["read:local-evaluation-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": PRECLINICAL_BOUNDARY,
        "contract_version": version
    })
}

pub fn assure(
    request: &ContextAssuranceRequest,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    require_approval: bool,
    require_federation: bool,
) -> Result<ContextAssuranceReceipt, ContextAssuranceError> {
    if request.benchmark_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.context_request.boundary != PRECLINICAL_BOUNDARY
        || !request.context_request.raw_data_local
        || !request.context_request.aggregate_only
        || request.required_site_order.is_empty()
        || sorted(&request.required_site_order) != request.required_site_order
        || sorted(&request.achieved_site_order) != request.achieved_site_order
        || request.achieved_site_order.iter().any(|site| !request.required_site_order.contains(site))
        || request.minimum_site_quorum as usize > request.required_site_order.len()
        || !digest(&request.benchmark_digest)
        || !digest(&request.replay_identity)
        || request.replay_identity != request.context_request.replay_identity
    {
        return Err(ContextAssuranceError::Invalid("assurance identity, benchmark, site quorum, locality, boundary, or replay is invalid".into()));
    }
    let context = context_compilation_support::compile(&request.context_request, feature_id, contract_version, scale, require_federation)
        .map_err(|error| ContextAssuranceError::Compilation(error.to_string()))?;
    let missing_site_order = request.required_site_order.iter().filter(|site| !request.achieved_site_order.contains(site)).cloned().collect::<Vec<_>>();
    let delta = request.candidate_discovery_rate_milli as i32 - request.baseline_discovery_rate_milli as i32;
    let approval_ok = !require_approval || request.signed_approval;
    let federation_ok = !require_federation || request.federation_approved;
    let quorum_ok = request.achieved_site_order.len() >= request.minimum_site_quorum as usize;
    let baseline_beaten = request.candidate_discovery_rate_milli > request.baseline_discovery_rate_milli;
    let safe = context.disposition == "qualified" && approval_ok && federation_ok && quorum_ok && baseline_beaten;
    let disposition = if !approval_ok || !federation_ok || context.disposition == "blocked" { "blocked" } else if safe { "qualified" } else { "partial" };
    let mut omissions = context.omissions.clone();
    if !approval_ok { omissions.push("assurance:signed-approval-missing".into()); }
    if !federation_ok { omissions.push("assurance:federation-approval-missing".into()); }
    if !quorum_ok { omissions.push("assurance:replication-quorum-missing".into()); }
    if !baseline_beaten { omissions.push("assurance:baseline-not-beaten".into()); }
    omissions.sort(); omissions.dedup();
    let mut negative_evidence = context.negative_evidence.clone();
    if !baseline_beaten { negative_evidence.push("assurance:candidate-did-not-beat-baseline".into()); }
    negative_evidence = sorted(&negative_evidence);
    let effect_receipts = if disposition == "qualified" { vec![format!("assure:worldgen-context:{}", request.benchmark_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request.context_request.request_id,
        "benchmark_id": request.benchmark_id,
        "disposition": disposition,
        "baseline_discovery_rate_milli": request.baseline_discovery_rate_milli,
        "candidate_discovery_rate_milli": request.candidate_discovery_rate_milli,
        "delta_discovery_rate_milli": delta,
        "required_site_order": request.required_site_order,
        "achieved_site_order": request.achieved_site_order,
        "missing_site_order": missing_site_order,
        "context_disposition": context.disposition,
        "context_digest": context.context_digest,
        "benchmark_digest": request.benchmark_digest,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": context.uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data": false,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let assurance_digest = ContentHash::of_value(&payload).map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
    let receipt = ContextAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: contract_version.into(), feature_id: feature_id.into(), request_id: request.context_request.request_id.clone(), benchmark_id: request.benchmark_id.clone(), disposition: disposition.into(), baseline_discovery_rate_milli: request.baseline_discovery_rate_milli, candidate_discovery_rate_milli: request.candidate_discovery_rate_milli, delta_discovery_rate_milli: delta, required_site_order: request.required_site_order.clone(), achieved_site_order: request.achieved_site_order.clone(), missing_site_order, context_disposition: context.disposition, context_digest: context.context_digest, benchmark_digest: request.benchmark_digest.clone(), replay_identity: request.replay_identity.clone(), assurance_digest: assurance_digest.clone(), omissions: sorted(&payload["omissions"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().to_owned()).collect::<Vec<_>>()), uncertainty: sorted(&context.uncertainty), negative_evidence, effect_receipts: sorted(&effect_receipts), artifact: json!({"artifact_id":format!("worldgen-context-evaluation:{}",request.benchmark_id),"content_type":CONTENT_TYPE,"content_hash":assurance_digest,"raw_data":false,"boundary":PRECLINICAL_BOUNDARY}), raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into()
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_compilation_support::{ContextCompilationRequest, ContextFact};
    use bioprism_foundation::EvidenceState;
    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn request() -> ContextAssuranceRequest {
        let replay = hash("replay"); let fact = ContextFact { fact_id: "fact:assure".into(), statement: "supported".into(), support_milli: 950, state: EvidenceState::Supported, evidence_digest: hash("e"), provenance_digest: hash("p"), artifact_digest: hash("a"), replay_identity: replay.clone(), negative_result: false, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
        ContextAssuranceRequest { context_request: ContextCompilationRequest { request_id: "assure:req".into(), objective: "evaluate context".into(), scope: "study:assure".into(), required_fact_order: vec!["fact:assure".into()], minimum_support_milli: 500, facts: vec![fact], replay_identity: replay.clone(), policy_allow: true, protected_closure: true, federation_approved: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() }, benchmark_id: "benchmark:context".into(), benchmark_digest: hash("benchmark"), baseline_discovery_rate_milli: 500, candidate_discovery_rate_milli: 800, required_site_order: vec!["site:a".into(), "site:b".into()], achieved_site_order: vec!["site:a".into(), "site:b".into()], minimum_site_quorum: 2, signed_approval: true, federation_approved: true, replay_identity: replay, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn qualified_assurance_beats_baseline() { let r = assure(&request(), "AFA-worldgen-P03-F25", "worldgen-local-context-assurance/1.0", "local single-study", false, false).unwrap(); assert_eq!(r.disposition, "qualified"); assert!(r.effect_receipts[0].starts_with("assure:worldgen-context:")); }
    #[test] fn baseline_failure_is_negative_evidence() { let mut q = request(); q.candidate_discovery_rate_milli = 400; let r = assure(&q, "AFA-worldgen-P03-F26", "worldgen-multimodal-context-assurance/1.0", "multimodal multi-study", false, false).unwrap(); assert_eq!(r.disposition, "partial"); assert!(r.negative_evidence.iter().any(|value| value.contains("did-not-beat"))); }
    #[test] fn missing_quorum_stays_partial() { let mut q = request(); q.achieved_site_order = vec!["site:a".into()]; let r = assure(&q, "AFA-worldgen-P03-F27", "worldgen-throughput-context-assurance/1.0", "prospective high-throughput", true, true).unwrap(); assert_eq!(r.disposition, "partial"); assert!(r.omissions.iter().any(|value| value.contains("quorum"))); }
}
