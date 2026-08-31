//! Hub P32: signed submission-release integrity contracts.
//!
//! The public hub is an evidence boundary, not a popularity counter.  This contract
//! turns a caller-supplied submission candidate into a deterministic release card,
//! preserving provenance, licence, reproducibility, negative results, omissions, and
//! unresolved evidence.  It never publishes, uploads, dereferences, or executes a
//! submission; a separate governed service consumes the card.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.hub.submission-release-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionCandidate4 {
    pub candidate_id: String,
    pub artifact_digest: String,
    pub scope: String,
    pub provenance_digest: String,
    pub licence: String,
    pub verification_state: String,
    pub evidence_state: String,
    pub reproducible: bool,
    pub negative_result: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionReleaseRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub candidates: Vec<SubmissionCandidate4>,
    pub required_candidate_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub candidate_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionReleaseCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub released_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub scope_order: Vec<String>,
    pub provenance_order: Vec<String>,
    pub licence_order: Vec<String>,
    pub verification_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub released_candidate_count: u64,
    pub total_candidate_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: SubmissionArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SubmissionReleaseIntegrityError {
    #[error("submission release request is invalid: {0}")]
    Invalid(String),
    #[error("submission release digest could not be computed: {0}")]
    Digest(String),
}

fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}

fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

fn invalid(v: impl Into<String>) -> SubmissionReleaseIntegrityError {
    SubmissionReleaseIntegrityError::Invalid(v.into())
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
        "consumer": "hub moderator, researcher workbench, and downstream registry",
        "effects": ["emit typed release card", "retain rejected and unresolved evidence"],
        "determinism": "canonical ordered vectors and content-addressed closure",
        "autonomy": "A1 policy-bounded release preparation; no publication side effect",
    })
}

fn validate_card(c: &SubmissionReleaseCard7) -> Result<(), SubmissionReleaseIntegrityError> {
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
        || c.released_candidate_count > c.total_candidate_count
    {
        return Err(invalid(
            "submission identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for v in [
        &c.candidate_order,
        &c.released_order,
        &c.rejected_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.scope_order,
        &c.provenance_order,
        &c.licence_order,
        &c.verification_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("submission vectors are not canonical"));
        }
    }
    let ids = c.candidate_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .released_order
        .iter()
        .chain(&c.rejected_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("submission states do not partition candidates"));
    }
    if c.released_candidate_count != c.released_order.len() as u64 {
        return Err(invalid(
            "released candidate count does not match released order",
        ));
    }
    Ok(())
}

pub fn release(
    q: &SubmissionReleaseRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<SubmissionReleaseCard7, SubmissionReleaseIntegrityError> {
    if q.schema_version != SCHEMA_VERSION
        || q.request_id.is_empty()
        || q.purpose.is_empty()
        || q.candidates.is_empty()
        || q.candidate_budget == 0
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.required_candidate_order)
        || q.adversarial_events.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(invalid(
            "submission identity, ordering, replay, locality, boundary, or budget is invalid",
        ));
    }
    let rows = q.candidates.iter().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut released = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut licences = BTreeSet::new();
    let mut verification = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut global_block = !q.policy_allowed
        || !q.protected_closure
        || !q.signed_approval
        || !q.raw_data_local
        || !q.aggregate_only
        || !q.adversarial_events.is_empty()
        || rows.len() > q.candidate_budget;
    for candidate in rows {
        if candidate.candidate_id.is_empty()
            || !digest(&candidate.artifact_digest)
            || !digest(&candidate.provenance_digest)
            || candidate.scope.is_empty()
            || candidate.licence.is_empty()
            || candidate.verification_state.is_empty()
        {
            return Err(invalid(
                "candidate identity, scope, licence, provenance, or verification is incomplete",
            ));
        }
        if !seen.insert(candidate.candidate_id.clone()) {
            return Err(invalid(format!(
                "duplicate candidate {}",
                candidate.candidate_id
            )));
        }
        scopes.insert(candidate.scope.clone());
        provenance.insert(candidate.provenance_digest.clone());
        licences.insert(candidate.licence.clone());
        verification.insert(candidate.verification_state.clone());
        evidence.insert(candidate.artifact_digest.clone());
        evidence.insert(candidate.provenance_digest.clone());
        if candidate.negative_result {
            semantic_loss.push(format!("{}:negative-result", candidate.candidate_id));
        }
        if !candidate.local || !candidate.aggregate_only {
            global_block = true;
        }
        match candidate.evidence_state.as_str() {
            "supported" | "proven"
                if candidate.reproducible
                    && candidate.required
                    && candidate.negative_result == false =>
            {
                released.insert(candidate.candidate_id.clone());
            }
            "contradicted" | "rejected" => {
                rejected.insert(candidate.candidate_id.clone());
                semantic_loss.push(candidate.candidate_id.clone());
            }
            "unknown" | "speculative" | "unmeasured" => {
                unknown.insert(candidate.candidate_id.clone());
                semantic_loss.push(candidate.candidate_id.clone());
            }
            _ => {
                omitted.insert(candidate.candidate_id.clone());
                semantic_loss.push(candidate.candidate_id.clone());
            }
        }
    }
    let required = q
        .required_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required != seen {
        return Err(invalid(
            "required candidate order is not the canonical candidate set",
        ));
    }
    if global_block {
        omitted.extend(seen.clone());
        released.clear();
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
        "released"
    };
    let body = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": q.request_id,
        "purpose": q.purpose,
        "disposition": disposition,
        "candidate_order": seen.iter().cloned().collect::<Vec<_>>(),
    });
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| SubmissionReleaseIntegrityError::Digest(e.to_string()))?
        .to_string();
    let released_order = released.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let candidate_order = body["candidate_order"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let artifact = SubmissionArtifact4 {
        artifact_id: format!("hub-submission-release:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss,
        evidence_digests: evidence.into_iter().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = SubmissionReleaseCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        candidate_order,
        released_order: released_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        scope_order: scopes.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        licence_order: licences.into_iter().collect(),
        verification_order: verification.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        released_candidate_count: released_order.len() as u64,
        total_candidate_count: q.candidates.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "released" {
            vec![format!("prepare:submission-release:{}", q.request_id)]
        } else {
            vec!["block:unsafe-publication".into()]
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

    fn request() -> SubmissionReleaseRequest4 {
        SubmissionReleaseRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "req-1".into(),
            purpose: "prepare public result card".into(),
            candidates: vec![SubmissionCandidate4 {
                candidate_id: "submission-a".into(),
                artifact_digest: "a".repeat(64),
                scope: "preclinical-cell-study".into(),
                provenance_digest: "b".repeat(64),
                licence: "CC-BY-4.0".into(),
                verification_state: "independent-reproduction".into(),
                evidence_state: "supported".into(),
                reproducible: true,
                negative_result: false,
                local: true,
                aggregate_only: true,
                required: true,
            }],
            required_candidate_order: vec!["submission-a".into()],
            replay_identity: "c".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            candidate_budget: 2,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn releases_reproducible_submission_without_publishing() {
        let card = release(&request(), "AFA-hub-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "released");
        assert_eq!(card.released_candidate_count, 1);
        assert_eq!(
            card.effect_receipts,
            vec!["prepare:submission-release:req-1"]
        );
    }

    #[test]
    fn retains_negative_result_as_partial() {
        let mut q = request();
        q.candidates[0].negative_result = true;
        let card = release(&q, "AFA-hub-P32-F02", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "partial");
        assert_eq!(card.released_candidate_count, 0);
        assert!(card
            .artifact
            .semantic_loss
            .iter()
            .any(|v| v.ends_with("negative-result")));
    }
}
