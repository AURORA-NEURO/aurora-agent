//! Factory P32: lease/fencing integrity for retry-safe research execution.
//!
//! This contract classifies caller-supplied worker leases without reading a clock
//! or touching a queue. It proves identity, generation fencing, attestation,
//! idempotency posture, and replay identity before a workflow controller may
//! consider a lease. Ambiguous non-idempotent expiry is quarantined, never
//! silently retried; the receipt retains every omitted and unresolved lease.

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = RESEARCH_CONTRACT_SCHEMA_VERSION;
pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str = "application/vnd.aurora.factory.lease-fencing-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerLease4 {
    pub lease_id: String,
    pub worker_id: String,
    pub job_id: String,
    pub generation: u64,
    pub expiry_epoch: u64,
    pub fence_token: String,
    pub idempotency: String,
    pub evidence_state: String,
    pub attested: bool,
    pub compensation_ready: bool,
    pub deterministic: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseFencingIntegrityRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub leases: Vec<WorkerLease4>,
    pub required_lease_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub lease_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseFencingArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub fence_tokens: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseFencingIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub lease_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub fencing_order: Vec<String>,
    pub worker_order: Vec<String>,
    pub job_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub admitted_lease_count: u64,
    pub total_lease_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: LeaseFencingArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LeaseFencingIntegrityError {
    #[error("lease fencing integrity input is invalid: {0}")]
    Invalid(String),
    #[error("lease fencing integrity digest failed: {0}")]
    Digest(String),
}

fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
fn invalid(message: impl Into<String>) -> LeaseFencingIntegrityError {
    LeaseFencingIntegrityError::Invalid(message.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"factory","consumers":["workflow operator","worker scheduler","recovery auditor","site reliability engineer"],"behavior":format!("qualify retry-safe worker leases at {scale} ({mode})"),"value":"prevents stale, forged, or ambiguously retried leases from duplicating research effects","input_schema":"LeaseFencingIntegrityRequest4@1","output_schema":"LeaseFencingIntegrityCard7@1","effects":["emit:lease-integrity-card","retain:quarantined-leases","block:unsafe-retry"],"permissions":["read:local-worker-attestations","exchange:aggregate-lease-metadata"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn validate_card(card: &LeaseFencingIntegrityCard7) -> Result<(), LeaseFencingIntegrityError> {
    if card.schema_version != SCHEMA_VERSION
        || card.feature_id.is_empty()
        || card.request_id.is_empty()
        || card.purpose.is_empty()
        || card.boundary != BOUNDARY
        || card.artifact.boundary != BOUNDARY
        || !card.raw_data_local
        || !card.aggregate_only
        || !digest(&card.replay_identity)
        || !digest(&card.closure_digest)
        || card.artifact.content_type != CONTENT_TYPE
        || card.artifact.content_hash != card.closure_digest
        || card.admitted_lease_count > card.total_lease_count
    {
        return Err(invalid(
            "lease identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for values in [
        &card.lease_order,
        &card.admitted_order,
        &card.rejected_order,
        &card.unknown_order,
        &card.omitted_order,
        &card.fencing_order,
        &card.worker_order,
        &card.job_order,
        &card.effect_order,
        &card.effect_receipts,
    ] {
        if !canonical(values) {
            return Err(invalid("lease vectors are not canonical"));
        }
    }
    let ids = card.lease_order.iter().collect::<BTreeSet<_>>();
    let states = card
        .admitted_order
        .iter()
        .chain(&card.rejected_order)
        .chain(&card.unknown_order)
        .chain(&card.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("lease states do not partition leases"));
    }
    if card.admitted_lease_count != card.admitted_order.len() as u64 {
        return Err(invalid(
            "admitted lease count does not match admitted order",
        ));
    }
    Ok(())
}

pub fn qualify(
    request: &LeaseFencingIntegrityRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<LeaseFencingIntegrityCard7, LeaseFencingIntegrityError> {
    if request.schema_version != SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.leases.is_empty()
        || request.lease_budget == 0
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !canonical(&request.required_lease_order)
        || !canonical(&request.adversarial_events)
    {
        return Err(invalid(
            "lease identity, ordering, replay, locality, boundary, or budget is invalid",
        ));
    }
    let mut leases = request.leases.clone();
    leases.sort_by(|left, right| left.lease_id.cmp(&right.lease_id));
    let mut seen = BTreeSet::new();
    let mut workers = BTreeSet::new();
    let mut jobs = BTreeSet::new();
    let mut fencing = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut tokens = BTreeSet::new();
    let mut admitted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut generation_by_job: HashMap<String, u64> = HashMap::new();
    let mut semantic_loss = Vec::new();
    for lease in &leases {
        if lease.lease_id.trim().is_empty()
            || lease.worker_id.trim().is_empty()
            || lease.job_id.trim().is_empty()
            || lease.generation == 0
            || lease.expiry_epoch == 0
            || !digest(&lease.fence_token)
            || lease.idempotency.trim().is_empty()
            || lease.evidence_state.trim().is_empty()
            || !lease.attested
            || !lease.local
            || !lease.aggregate_only
        {
            return Err(invalid("lease identity, generation, expiry, fence token, attestation, evidence, or locality is incomplete"));
        }
        if !seen.insert(lease.lease_id.clone()) {
            return Err(invalid(format!("duplicate lease {}", lease.lease_id)));
        }
        let duplicate_generation = generation_by_job
            .get(&lease.job_id)
            .is_some_and(|previous| *previous == lease.generation);
        generation_by_job.insert(lease.job_id.clone(), lease.generation);
        workers.insert(format!("{}:{}", lease.lease_id, lease.worker_id));
        jobs.insert(format!("{}:{}", lease.lease_id, lease.job_id));
        fencing.insert(format!("{}:{}", lease.lease_id, lease.generation));
        effects.insert(format!("{}:{}", lease.lease_id, lease.idempotency));
        tokens.insert(lease.fence_token.clone());
        if duplicate_generation {
            rejected.insert(lease.lease_id.clone());
            semantic_loss.push(lease.lease_id.clone());
        } else {
            match lease.evidence_state.as_str() {
                "supported" | "proven" => {
                    if lease.required
                        && lease.deterministic
                        && lease.attested
                        && (lease.idempotency == "idempotent" || lease.compensation_ready)
                    {
                        admitted.insert(lease.lease_id.clone());
                    } else {
                        rejected.insert(lease.lease_id.clone());
                        semantic_loss.push(lease.lease_id.clone());
                    }
                }
                "contradicted" | "rejected" => {
                    rejected.insert(lease.lease_id.clone());
                    semantic_loss.push(lease.lease_id.clone());
                }
                "unknown" | "speculative" | "unmeasured" => {
                    unknown.insert(lease.lease_id.clone());
                    semantic_loss.push(lease.lease_id.clone());
                }
                _ => {
                    omitted.insert(lease.lease_id.clone());
                    semantic_loss.push(lease.lease_id.clone());
                }
            }
        }
    }
    if request.required_lease_order.iter().collect::<BTreeSet<_>>()
        != seen.iter().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "required lease order is not the canonical lease set",
        ));
    }
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_manifest
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || leases.len() > request.lease_budget;
    if global_block {
        omitted.extend(seen.clone());
        admitted.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global_block {
        "blocked"
    } else if !unknown.is_empty() {
        "unknown"
    } else if !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let lease_order = seen.iter().cloned().collect::<Vec<_>>();
    let body = json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"lease_order":lease_order});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|error| LeaseFencingIntegrityError::Digest(error.to_string()))?
        .to_string();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let card = LeaseFencingIntegrityCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        disposition: disposition.into(),
        lease_order,
        admitted_order: admitted_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        fencing_order: fencing.into_iter().collect(),
        worker_order: workers.into_iter().collect(),
        job_order: jobs.into_iter().collect(),
        effect_order: effects.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        closure_digest: closure_digest.clone(),
        admitted_lease_count: admitted_order.len() as u64,
        total_lease_count: leases.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "qualified" {
            vec![format!("prepare:lease-integrity:{}", request.request_id)]
        } else {
            vec!["block:unsafe-retry".into()]
        },
        artifact: LeaseFencingArtifact4 {
            artifact_id: format!("factory-lease-integrity:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: closure_digest,
            semantic_loss: if global_block {
                seen.iter().cloned().collect()
            } else {
                semantic_loss
            },
            fence_tokens: tokens.into_iter().collect(),
            boundary: BOUNDARY.into(),
        },
    };
    validate_card(&card)?;
    let _ = (scale, mode);
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> LeaseFencingIntegrityRequest4 {
        LeaseFencingIntegrityRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "lease-1".into(),
            purpose: "qualify retry posture".into(),
            leases: vec![WorkerLease4 {
                lease_id: "lease-a".into(),
                worker_id: "worker-a".into(),
                job_id: "job-a".into(),
                generation: 1,
                expiry_epoch: 10,
                fence_token: "a".repeat(64),
                idempotency: "idempotent".into(),
                evidence_state: "supported".into(),
                attested: true,
                compensation_ready: false,
                deterministic: true,
                local: true,
                aggregate_only: true,
                required: true,
            }],
            required_lease_order: vec!["lease-a".into()],
            replay_identity: "b".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            lease_budget: 2,
            boundary: BOUNDARY.into(),
        }
    }
    #[test]
    fn qualifies_attested_idempotent_lease() {
        let card = qualify(
            &request(),
            "AFA-factory-P32-F01",
            "v1",
            "local",
            "inference",
        )
        .unwrap();
        assert_eq!(card.disposition, "qualified");
        assert_eq!(card.admitted_lease_count, 1);
    }
    #[test]
    fn non_idempotent_without_compensation_is_rejected() {
        let mut q = request();
        q.leases[0].idempotency = "non_idempotent".into();
        let card = qualify(&q, "AFA-factory-P32-F02", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "partial");
        assert_eq!(card.rejected_order, vec!["lease-a"]);
    }
}
