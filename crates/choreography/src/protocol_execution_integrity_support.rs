//! Choreography P32: typed protocol execution integrity contracts.
//!
//! This boundary turns a projected multiparty protocol into an auditable execution
//! record.  A card is only complete when every required step has a deterministic
//! state transition, a local/aggregate-only effect, and a replay identity.  Missing
//! closure, policy denial, adversarial input, or an exhausted budget is represented
//! explicitly instead of being treated as a successful protocol run.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.choreography.protocol-execution-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolStep4 {
    pub step_id: String,
    pub role: String,
    pub operation: String,
    pub input_digest: String,
    pub output_digest: Option<String>,
    pub state: String,
    pub evidence_state: String,
    pub deterministic: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolExecutionRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub steps: Vec<ProtocolStep4>,
    pub required_step_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub step_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolExecutionArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolExecutionCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub step_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub role_order: Vec<String>,
    pub operation_order: Vec<String>,
    pub state_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub completed_step_count: u64,
    pub total_step_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: ProtocolExecutionArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolExecutionIntegrityError {
    #[error("protocol execution request is invalid: {0}")]
    Invalid(String),
    #[error("protocol execution digest could not be computed: {0}")]
    Digest(String),
}

fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}

fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

fn invalid(v: impl Into<String>) -> ProtocolExecutionIntegrityError {
    ProtocolExecutionIntegrityError::Invalid(v.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({
        "feature_id": feature_id,
        "contract_version": contract_version,
        "schema_version": SCHEMA_VERSION,
        "content_type": CONTENT_TYPE,
        "boundary": BOUNDARY,
        "scale": scale,
        "mode": mode,
        "consumer": "protocol steward and downstream execution ledger",
        "effects": ["emit typed execution card", "retain blocked and omitted steps"],
        "determinism": "canonical ordered vectors and content-addressed closure",
        "autonomy": "A1 advisory or A2 policy-bounded execution; no clinical decisions",
    })
}

fn validate_card(c: &ProtocolExecutionCard7) -> Result<(), ProtocolExecutionIntegrityError> {
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
        || c.completed_step_count > c.total_step_count
    {
        return Err(invalid(
            "protocol identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for v in [
        &c.step_order,
        &c.completed_order,
        &c.blocked_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.role_order,
        &c.operation_order,
        &c.state_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("protocol vectors are not canonical"));
        }
    }
    let ids = c.step_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .completed_order
        .iter()
        .chain(&c.blocked_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("protocol states do not partition steps"));
    }
    if c.completed_step_count != c.completed_order.len() as u64 {
        return Err(invalid(
            "completed step count does not match completed order",
        ));
    }
    Ok(())
}

pub fn execute(
    q: &ProtocolExecutionRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<ProtocolExecutionCard7, ProtocolExecutionIntegrityError> {
    if q.schema_version != SCHEMA_VERSION || q.boundary != BOUNDARY {
        return Err(invalid("schema or preclinical boundary mismatch"));
    }
    if q.steps.is_empty() || q.step_budget == 0 {
        return Err(invalid(
            "protocol must contain at least one step and a positive budget",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut roles = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut states = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut evidence = BTreeSet::new();
    let mut global_block = !q.policy_allowed
        || !q.protected_closure
        || !q.signed_manifest
        || !q.raw_data_local
        || !q.aggregate_only
        || !q.adversarial_events.is_empty()
        || q.steps.len() > q.step_budget;
    for step in &q.steps {
        if step.step_id.is_empty()
            || !digest(&step.input_digest)
            || step.output_digest.as_ref().is_some_and(|v| !digest(v))
            || step.role.is_empty()
            || step.operation.is_empty()
        {
            return Err(invalid(
                "step identity, role, operation, or digest is incomplete",
            ));
        }
        if !seen.insert(step.step_id.clone()) {
            return Err(invalid(format!("duplicate step {}", step.step_id)));
        }
        roles.insert(step.role.clone());
        operations.insert(step.operation.clone());
        states.insert(step.state.clone());
        evidence.insert(step.input_digest.clone());
        if let Some(output) = &step.output_digest {
            evidence.insert(output.clone());
        }
        if !step.local || !step.aggregate_only || !step.deterministic {
            global_block = true;
        }
        match step.evidence_state.as_str() {
            "supported" | "proven" if step.required && step.output_digest.is_some() => {
                completed.insert(step.step_id.clone());
            }
            "contradicted" | "rejected" => {
                blocked.insert(step.step_id.clone());
                semantic_loss.push(step.step_id.clone());
            }
            "unknown" | "speculative" | "unmeasured" => {
                unknown.insert(step.step_id.clone());
                semantic_loss.push(step.step_id.clone());
            }
            _ => {
                omitted.insert(step.step_id.clone());
                semantic_loss.push(step.step_id.clone());
            }
        }
    }
    let required = q
        .required_step_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required != seen || !canonical(&q.required_step_order) {
        return Err(invalid("required step order is not the canonical step set"));
    }
    if global_block {
        let all = seen.clone();
        omitted.extend(all);
        completed.clear();
        blocked.clear();
        unknown.clear();
    }
    let disposition = if global_block {
        "blocked"
    } else if !unknown.is_empty() {
        "unknown"
    } else if !blocked.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "completed"
    };
    let body = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": q.request_id,
        "purpose": q.purpose,
        "disposition": disposition,
        "step_order": seen.iter().cloned().collect::<Vec<_>>(),
    });
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| ProtocolExecutionIntegrityError::Digest(e.to_string()))?
        .to_string();
    let completed_order = completed.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let step_order = body["step_order"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let artifact = ProtocolExecutionArtifact4 {
        artifact_id: format!("choreography-protocol-execution:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss,
        evidence_digests: evidence.into_iter().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = ProtocolExecutionCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        step_order,
        completed_order: completed_order.clone(),
        blocked_order,
        unknown_order,
        omitted_order,
        role_order: roles.into_iter().collect(),
        operation_order: operations.into_iter().collect(),
        state_order: states.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        completed_step_count: completed_order.len() as u64,
        total_step_count: q.steps.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "completed" {
            vec![format!("emit:protocol-execution:{}", q.request_id)]
        } else {
            vec!["block:unsafe-protocol-effect".into()]
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
    fn completes_replayed_local_protocol() {
        let digest = "a".repeat(64);
        let q = ProtocolExecutionRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "req-1".into(),
            purpose: "execute projected protocol".into(),
            steps: vec![ProtocolStep4 {
                step_id: "step-a".into(),
                role: "researcher".into(),
                operation: "ack".into(),
                input_digest: digest.clone(),
                output_digest: Some(digest.clone()),
                state: "done".into(),
                evidence_state: "supported".into(),
                deterministic: true,
                local: true,
                aggregate_only: true,
                required: true,
            }],
            required_step_order: vec!["step-a".into()],
            replay_identity: "b".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            step_budget: 2,
            boundary: BOUNDARY.into(),
        };
        let card = execute(&q, "AFA-choreography-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "completed");
        assert_eq!(card.completed_step_count, 1);
    }

    #[test]
    fn retains_unknown_steps_without_claiming_completion() {
        let q = ProtocolExecutionRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "req-2".into(),
            purpose: "execute uncertain protocol".into(),
            steps: vec![ProtocolStep4 {
                step_id: "step-u".into(),
                role: "agent".into(),
                operation: "observe".into(),
                input_digest: "c".repeat(64),
                output_digest: None,
                state: "pending".into(),
                evidence_state: "unknown".into(),
                deterministic: true,
                local: true,
                aggregate_only: true,
                required: true,
            }],
            required_step_order: vec!["step-u".into()],
            replay_identity: "d".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            step_budget: 2,
            boundary: BOUNDARY.into(),
        };
        let card = execute(&q, "AFA-choreography-P32-F02", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "unknown");
        assert_eq!(card.completed_step_count, 0);
        assert_eq!(card.unknown_order, vec!["step-u"]);
    }
}
