//! Benchcompiler P32: trajectory-to-decision benchmark compilation integrity contracts.
use crate::mechanism_control::{PRECLINICAL_BOUNDARY, SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.benchcompiler.benchmark-compilation-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkCase4 {
    pub case_id: String,
    pub trajectory_digest: String,
    pub decision_cell: String,
    pub baseline_digest: String,
    pub benchmark_world_digest: String,
    pub causal_divergence_digest: String,
    pub evidence_state: String,
    pub independently_reproduced: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkCompileRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub cases: Vec<BenchmarkCase4>,
    pub required_case_order: Vec<String>,
    pub baseline_digest: String,
    pub benchmark_world_digest: String,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub case_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub case_order: Vec<String>,
    pub compiled_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub trajectory_order: Vec<String>,
    pub decision_order: Vec<String>,
    pub baseline_order: Vec<String>,
    pub divergence_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub compiled_case_count: u64,
    pub total_case_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: BenchmarkArtifact4,
}

#[derive(Debug, Error)]
pub enum BenchmarkCompilationIntegrityError {
    #[error("benchmark compilation integrity input is invalid: {0}")]
    Invalid(String),
    #[error("benchmark compilation integrity digest failed: {0}")]
    Digest(String),
}
fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn invalid(v: impl Into<String>) -> BenchmarkCompilationIntegrityError {
    BenchmarkCompilationIntegrityError::Invalid(v.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"benchcompiler","consumers":["benchmark compiler","evaluation engine","causal analysis operator","research workbench"],"behavior":format!("compile trajectory evidence into decision cells at {scale} ({mode})"),"value":"turns reproducible benchmark trajectories into omission-aware decision-cell artifacts while preserving causal divergence and refusal states","input_schema":"BenchmarkCompileRequest4@1","output_schema":"BenchmarkCard7@1","effects":["emit:benchmark-card","retain:negative-result-evidence","block:unsafe-claim"],"permissions":["read:local-benchmark-traces"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn validate_card(c: &BenchmarkCard7) -> Result<(), BenchmarkCompilationIntegrityError> {
    if c.schema_version != SCHEMA_VERSION
        || c.feature_id.is_empty()
        || c.request_id.is_empty()
        || c.purpose.is_empty()
        || c.boundary != BOUNDARY
        || c.artifact.boundary != BOUNDARY
        || !c.raw_data_local
        || !c.aggregate_only
        || !digest(&c.replay_identity)
        || !digest(&c.closure_digest)
        || c.artifact.content_type != CONTENT_TYPE
        || c.artifact.content_hash != c.closure_digest
        || c.compiled_case_count > c.total_case_count
    {
        return Err(invalid(
            "benchmark identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for v in [
        &c.case_order,
        &c.compiled_order,
        &c.rejected_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.trajectory_order,
        &c.decision_order,
        &c.baseline_order,
        &c.divergence_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("benchmark vectors are not canonical"));
        }
    }
    let ids = c.case_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .compiled_order
        .iter()
        .chain(&c.rejected_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("benchmark states do not partition cases"));
    }
    Ok(())
}

pub fn compile(
    q: &BenchmarkCompileRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<BenchmarkCard7, BenchmarkCompilationIntegrityError> {
    if q.schema_version != SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.cases.is_empty()
        || q.required_case_order.is_empty()
        || !canonical(&q.required_case_order)
        || !digest(&q.baseline_digest)
        || !digest(&q.benchmark_world_digest)
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.adversarial_events)
        || q.case_budget == 0
    {
        return Err(invalid("benchmark identity, ordering, baseline, digest, locality, boundary, or budget is invalid"));
    }
    let mut rows = q.cases.clone();
    rows.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    let mut seen = BTreeSet::new();
    let mut compiled = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut trajectories = BTreeSet::new();
    let mut decisions = BTreeSet::new();
    let mut baselines = BTreeSet::new();
    let mut divergences = BTreeSet::new();
    for c in &rows {
        if c.case_id.trim().is_empty()
            || !seen.insert(c.case_id.clone())
            || !digest(&c.trajectory_digest)
            || c.decision_cell.trim().is_empty()
            || !digest(&c.baseline_digest)
            || !digest(&c.benchmark_world_digest)
            || !digest(&c.causal_divergence_digest)
            || c.evidence_state.trim().is_empty()
            || !c.local
            || !c.aggregate_only
        {
            return Err(invalid(
                "case identity, trajectory, decision, digest, evidence, or locality is invalid",
            ));
        }
        trajectories.insert(format!("{}:{}", c.case_id, c.trajectory_digest));
        decisions.insert(format!("{}:{}", c.case_id, c.decision_cell));
        baselines.insert(format!("{}:{}", c.case_id, c.baseline_digest));
        divergences.insert(format!("{}:{}", c.case_id, c.causal_divergence_digest));
        if c.evidence_state == "unknown" || c.trajectory_digest == q.replay_identity {
            unknown.insert(c.case_id.clone());
        } else if c.baseline_digest != q.baseline_digest
            || c.benchmark_world_digest != q.benchmark_world_digest
            || !c.independently_reproduced
        {
            rejected.insert(c.case_id.clone());
        } else if !q.required_case_order.contains(&c.case_id) {
            omitted.insert(c.case_id.clone());
        } else {
            compiled.insert(c.case_id.clone());
        }
    }
    let missing = q
        .required_case_order
        .iter()
        .filter(|x| !seen.contains(*x))
        .cloned()
        .collect::<Vec<_>>();
    let global = !q.policy_allowed
        || !q.protected_closure
        || !q.signed_manifest
        || !q.raw_data_local
        || !q.aggregate_only
        || !q.adversarial_events.is_empty()
        || rows.len() > q.case_budget;
    if global {
        omitted.extend(seen.iter().cloned());
        compiled.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global {
        "blocked"
    } else if !missing.is_empty() || !unknown.is_empty() {
        "unknown"
    } else if compiled.is_empty() || !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "compiled"
    };
    let body = json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":q.request_id,"purpose":q.purpose,"disposition":disposition,"case_order":seen.iter().cloned().collect::<Vec<_>>()});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| BenchmarkCompilationIntegrityError::Digest(e.to_string()))?
        .to_string();
    let compiled_order = compiled.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let artifact = BenchmarkArtifact4 {
        artifact_id: format!("benchcompiler-benchmark-compilation:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: omitted_order.clone(),
        evidence_digests: divergences.iter().cloned().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = BenchmarkCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        case_order: body["case_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        compiled_order: compiled_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        trajectory_order: trajectories.into_iter().collect(),
        decision_order: decisions.into_iter().collect(),
        baseline_order: baselines.into_iter().collect(),
        divergence_order: divergences.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        compiled_case_count: compiled_order.len() as u64,
        total_case_count: rows.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "compiled" {
            vec![format!("emit:benchmark:{}", q.request_id)]
        } else {
            vec!["block:unsafe-claim".into()]
        },
        artifact,
    };
    validate_card(&c)?;
    let _ = (scale, mode);
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_reproduced_case() {
        let q = BenchmarkCompileRequest4 {
            schema_version: SCHEMA_VERSION.into(), request_id: "req-1".into(), purpose: "compile trajectory".into(),
            cases: vec![BenchmarkCase4 { case_id: "case-a".into(), trajectory_digest: "a".repeat(64), decision_cell: "cell-1".into(), baseline_digest: "b".repeat(64), benchmark_world_digest: "c".repeat(64), causal_divergence_digest: "d".repeat(64), evidence_state: "supported".into(), independently_reproduced: true, local: true, aggregate_only: true }],
            required_case_order: vec!["case-a".into()], baseline_digest: "b".repeat(64), benchmark_world_digest: "c".repeat(64), replay_identity: "e".repeat(64), policy_allowed: true, protected_closure: true, signed_manifest: true, raw_data_local: true, aggregate_only: true, adversarial_events: vec![], case_budget: 2, boundary: BOUNDARY.into(),
        };
        let card = compile(&q, "AFA-benchcompiler-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "compiled");
        assert_eq!(card.compiled_case_count, 1);
    }
}
