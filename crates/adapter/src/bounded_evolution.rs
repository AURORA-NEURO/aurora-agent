//! Bounded, replayable evolution admission for high-throughput research workflows.
//!
//! Atlas feature: `AFA-adapter-P32-F23`.
//! The gateway admits versioned candidate changes only when deterministic replay,
//! evidence, policy, safety review, and resource ceilings are all explicit. It
//! never mutates source code or deploys an unreviewed candidate.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, EvolutionIdentity};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P32-F23";
pub const CONTRACT_VERSION: &str = "adapter-bounded-evolution/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionCandidate {
    pub candidate_id: String,
    pub artifact_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub required_evidence: Vec<ContentHash>,
    pub replayable: bool,
    pub deterministic: bool,
    pub safety_reviewed: bool,
    pub policy_allow: bool,
    pub resource_cost: u64,
    pub affected_surface: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedEvolutionRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub candidates: Vec<EvolutionCandidate>,
    pub evidence_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub max_concurrency: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionDisposition {
    Admitted,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedEvolutionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub disposition: EvolutionDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub replay_order: Vec<ContentHash>,
    pub budget: u64,
    pub budget_remaining: u64,
    pub max_concurrency: u32,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl BoundedEvolutionReceipt {
    pub fn validate(&self) -> Result<(), BoundedEvolutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_remaining > self.budget
            || self.max_concurrency == 0
        {
            return Err(BoundedEvolutionError::Invalid(
                "evolution identity, candidates, budget, checks, effects, locality, or boundary are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(BoundedEvolutionError::Invalid(
                    "evolution ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.evidence_order, &self.replay_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(BoundedEvolutionError::Invalid(
                    "evolution digest ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BoundedEvolutionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, BoundedEvolutionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum BoundedEvolutionError {
    #[error("invalid bounded evolution request: {0}")]
    Invalid(String),
    #[error("bounded evolution artifact error: {0}")]
    Artifact(String),
    #[error("bounded evolution serialization error: {0}")]
    Serialization(String),
}

pub fn admit_bounded_evolution(
    request: &BoundedEvolutionRequest,
) -> Result<BoundedEvolutionReceipt, BoundedEvolutionError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut replay = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let evidence = request
        .evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let evidence_complete = candidate
            .required_evidence
            .iter()
            .all(|digest| evidence.contains(digest));
        let budget_ok = candidate.resource_cost <= request.budget.saturating_sub(spent);
        let gate_ok = candidate.replayable
            && candidate.deterministic
            && candidate.safety_reviewed
            && candidate.policy_allow
            && evidence_complete
            && budget_ok
            && !candidate
                .affected_surface
                .to_ascii_lowercase()
                .contains("clinical");
        if gate_ok {
            spent = spent.saturating_add(candidate.resource_cost);
            admitted.insert(candidate.candidate_id.clone());
            replay.insert(candidate.baseline_digest.clone());
            replay.insert(candidate.artifact_digest.clone());
        } else {
            blocked.insert(candidate.candidate_id.clone());
            if !candidate.replayable || !candidate.deterministic {
                omissions.insert(format!(
                    "candidate:{}:replay-or-determinism-gate-unmet",
                    candidate.candidate_id
                ));
            }
            if !candidate.safety_reviewed {
                omissions.insert(format!(
                    "candidate:{}:independent-safety-review-missing",
                    candidate.candidate_id
                ));
            }
            if !candidate.policy_allow {
                omissions.insert(format!(
                    "candidate:{}:policy-denied",
                    candidate.candidate_id
                ));
            }
            if !evidence_complete {
                negative.insert(format!(
                    "candidate:{}:required-evidence-not-present",
                    candidate.candidate_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "candidate:{}:budget-ceiling-exceeded",
                    candidate.candidate_id
                ));
            }
            if candidate
                .affected_surface
                .to_ascii_lowercase()
                .contains("clinical")
            {
                negative.insert(format!(
                    "candidate:{}:clinical-surface-out-of-boundary",
                    candidate.candidate_id
                ));
            }
        }
    }
    if request.max_concurrency > candidate_order.len() as u32 {
        uncertainty.insert("request:concurrency-ceiling-exceeds-candidate-set".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.signed_approval {
        EvolutionDisposition::Blocked
    } else if !request.protected_closure {
        EvolutionDisposition::Unknown
    } else if admitted_order.is_empty() {
        EvolutionDisposition::Unknown
    } else if blocked_order.is_empty() {
        EvolutionDisposition::Admitted
    } else {
        EvolutionDisposition::Partial
    };
    let mut checks = vec![
        "candidate ids and digest orders are canonical".into(),
        "replay, determinism, safety, evidence, policy, budget, and boundary gates are explicit"
            .into(),
        "admission produces a receipt and never mutates or deploys a candidate".into(),
    ];
    checks.sort();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let replay_order = replay.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(
        disposition,
        EvolutionDisposition::Admitted | EvolutionDisposition::Partial
    ) {
        vec!["effect:admission-receipt-only-no-deployment".into()]
    } else {
        vec![format!("block:bounded-evolution:{disposition:?}").to_lowercase()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "objective_id": request.objective_id,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "evidence_order": evidence_order,
        "replay_order": replay_order,
        "budget": request.budget,
        "budget_remaining": request.budget.saturating_sub(spent),
        "max_concurrency": request.max_concurrency,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-bounded-evolution:{}", request.request_id),
        "application/vnd.aurora.adapter-bounded-evolution+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| BoundedEvolutionError::Artifact(error.to_string()))?;
    let receipt = BoundedEvolutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        disposition,
        candidate_order,
        admitted_order,
        blocked_order,
        evidence_order,
        replay_order,
        budget: request.budget,
        budget_remaining: request.budget.saturating_sub(spent),
        max_concurrency: request.max_concurrency,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &BoundedEvolutionRequest) -> Result<(), BoundedEvolutionError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.candidates.is_empty()
        || request
            .evidence_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.max_concurrency == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(BoundedEvolutionError::Invalid(
            "evolution identity, candidates, evidence, replay identity, concurrency, locality, or boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.affected_surface.trim().is_empty()
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || candidate
                .required_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(BoundedEvolutionError::Invalid(format!(
                "candidate {} is invalid or duplicated",
                candidate.candidate_id
            )));
        }
        EvolutionIdentity::new(
            request.workflow_id.clone(),
            candidate.candidate_id.clone(),
            1,
            None,
            candidate.baseline_digest.clone(),
            candidate.artifact_digest.clone(),
            request.replay_identity.clone(),
        )
        .map_err(|error| BoundedEvolutionError::Invalid(error.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn candidate(id: &str, cost: u64) -> EvolutionCandidate {
        EvolutionCandidate {
            candidate_id: id.into(),
            artifact_digest: digest(&format!("artifact:{id}")),
            baseline_digest: digest("baseline:v1"),
            required_evidence: vec![digest("evidence:v1")],
            replayable: true,
            deterministic: true,
            safety_reviewed: true,
            policy_allow: true,
            resource_cost: cost,
            affected_surface: "adapter:research-contract".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request() -> BoundedEvolutionRequest {
        BoundedEvolutionRequest {
            request_id: "evolution:adapter".into(),
            workflow_id: "workflow:high-throughput".into(),
            objective_id: "objective:bounded-evolution".into(),
            candidates: vec![candidate("candidate:b", 4), candidate("candidate:a", 3)],
            evidence_order: vec![digest("evidence:v1")],
            replay_identity: digest("replay:v1"),
            budget: 8,
            max_concurrency: 2,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn admits_replayable_candidates_within_budget() {
        let receipt = admit_bounded_evolution(&request()).unwrap();
        assert_eq!(receipt.disposition, EvolutionDisposition::Admitted);
        assert_eq!(receipt.admitted_order, vec!["candidate:a", "candidate:b"]);
        assert_eq!(receipt.budget_remaining, 1);
    }

    #[test]
    fn retains_budget_blocked_candidate() {
        let mut request = request();
        request.candidates[0].resource_cost = 20;
        let receipt = admit_bounded_evolution(&request).unwrap();
        assert_eq!(receipt.disposition, EvolutionDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value.contains("budget-ceiling")));
    }

    #[test]
    fn protected_gap_is_unknown() {
        let mut request = request();
        request.protected_closure = false;
        assert_eq!(
            admit_bounded_evolution(&request).unwrap().disposition,
            EvolutionDisposition::Unknown
        );
    }

    #[test]
    fn unsigned_or_policy_denied_request_is_blocked() {
        let mut request = request();
        request.signed_approval = false;
        assert_eq!(
            admit_bounded_evolution(&request).unwrap().disposition,
            EvolutionDisposition::Blocked
        );
    }

    #[test]
    fn clinical_surface_is_never_admitted() {
        let mut request = request();
        request.candidates[0].affected_surface = "clinical-decision".into();
        let receipt = admit_bounded_evolution(&request).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("clinical-surface")));
    }
}
