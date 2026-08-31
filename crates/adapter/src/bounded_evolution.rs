//! Bounded, replayable evolution admission for high-throughput research workflows.
//!
//! Atlas feature: `AFA-adapter-P32-F23`.
//! The gateway admits versioned candidate changes only when deterministic replay,
//! evidence, policy, safety review, and resource ceilings are all explicit. It
//! never mutates source code or deploys an unreviewed candidate.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, EvolutionIdentity};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P32-F23";
pub const CONTRACT_VERSION: &str = "adapter-bounded-evolution/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_CANDIDATES: usize = 8192;
const MAX_NOTE_ITEMS: usize = 16384;

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
    pub input: BoundedEvolutionRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub replay_identity: ContentHash,
    pub candidates: Vec<EvolutionCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub admission_digest: ContentHash,
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
            || self.candidates.is_empty()
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
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("objective_id", &self.objective_id)?;
        if self.replay_identity == ContentHash::of_bytes(b"")
            || self.admission_digest == ContentHash::of_bytes(b"")
        {
            return Err(BoundedEvolutionError::Invalid(
                "replay and admission digests are required".into(),
            ));
        }
        validate_sorted_strings(&self.candidate_order, "candidate_order", MAX_CANDIDATES)?;
        validate_sorted_strings(&self.admitted_order, "admitted_order", MAX_CANDIDATES)?;
        validate_sorted_strings(&self.blocked_order, "blocked_order", MAX_CANDIDATES)?;
        validate_sorted_strings(&self.checks, "checks", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.omissions, "omissions", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.uncertainty, "uncertainty", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.negative_evidence, "negative_evidence", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.effect_receipts, "effect_receipts", MAX_NOTE_ITEMS)?;
        validate_sorted_hashes(&self.evidence_order, "evidence_order")?;
        validate_sorted_hashes(&self.replay_order, "replay_order")?;
        let candidate_ids = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let admitted_ids = self.admitted_order.iter().collect::<BTreeSet<_>>();
        let blocked_ids = self.blocked_order.iter().collect::<BTreeSet<_>>();
        if admitted_ids.intersection(&blocked_ids).next().is_some()
            || admitted_ids
                .union(&blocked_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_ids
            || self.admitted_order.len() > self.max_concurrency as usize
        {
            return Err(BoundedEvolutionError::Invalid(
                "candidate admission partition or concurrency ceiling is inconsistent".into(),
            ));
        }
        let expected_effect = match self.disposition {
            EvolutionDisposition::Admitted | EvolutionDisposition::Partial => {
                "effect:admission-receipt-only-no-deployment"
            }
            EvolutionDisposition::Blocked => "block:bounded-evolution:blocked",
            EvolutionDisposition::Unknown => "block:bounded-evolution:unknown",
        };
        if self.effect_receipts != vec![expected_effect.to_string()] {
            return Err(BoundedEvolutionError::Invalid(
                "evolution effect does not match the disposition".into(),
            ));
        }
        let expected_admission_digest = ContentHash::of_value(&admission_digest_payload(self))
            .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))?;
        if self.admission_digest != expected_admission_digest {
            return Err(BoundedEvolutionError::Invalid(
                "admission digest does not bind the receipt state".into(),
            ));
        }
        validate_request(&self.input)?;
        if self.input_digest != bounded_evolution_input_digest(&self.input)? {
            return Err(BoundedEvolutionError::Invalid(
                "evolution retained input digest does not match the request".into(),
            ));
        }
        let expected = admit_bounded_evolution_internal(&self.input, false)?;
        if self != &expected {
            return Err(BoundedEvolutionError::Invalid(
                "evolution receipt is not derived from its retained request".into(),
            ));
        }
        if self.artifact.artifact_id != format!("adapter-bounded-evolution:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.adapter-bounded-evolution+json"
            || self.artifact != expected.artifact
        {
            return Err(BoundedEvolutionError::Artifact(
                "evolution artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BoundedEvolutionError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&evolution_payload(self))
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

fn validate_text(field: &str, value: &str) -> Result<(), BoundedEvolutionError> {
    if value.is_empty() || value.trim() != value {
        return Err(BoundedEvolutionError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(BoundedEvolutionError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn bounded_evolution_input_digest(
    request: &BoundedEvolutionRequest,
) -> Result<ContentHash, BoundedEvolutionError> {
    let value = serde_json::to_value(&canonical_bounded_evolution_request(request))
        .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))
}

fn canonical_bounded_evolution_request(
    request: &BoundedEvolutionRequest,
) -> BoundedEvolutionRequest {
    let mut canonical = request.clone();
    canonical.evidence_order.sort();
    for candidate in &mut canonical.candidates {
        candidate.required_evidence.sort();
    }
    canonical
        .candidates
        .sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    canonical
}

fn validate_sorted_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), BoundedEvolutionError> {
    if values.len() > max_items {
        return Err(BoundedEvolutionError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(BoundedEvolutionError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(BoundedEvolutionError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_sorted_hashes(
    values: &[ContentHash],
    field: &str,
) -> Result<(), BoundedEvolutionError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(BoundedEvolutionError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn admission_digest_payload(receipt: &BoundedEvolutionReceipt) -> serde_json::Value {
    admission_digest_payload_from_parts(
        &receipt.request_id,
        &receipt.workflow_id,
        &receipt.objective_id,
        &receipt.replay_identity,
        &receipt.candidates,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.signed_approval,
        receipt.disposition,
        &receipt.candidate_order,
        &receipt.admitted_order,
        &receipt.blocked_order,
        &receipt.evidence_order,
        &receipt.replay_order,
        receipt.budget,
        receipt.budget_remaining,
        receipt.max_concurrency,
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn admission_digest_payload_from_parts(
    request_id: &str,
    workflow_id: &str,
    objective_id: &str,
    replay_identity: &ContentHash,
    candidates: &[EvolutionCandidate],
    policy_allow: bool,
    protected_closure: bool,
    signed_approval: bool,
    disposition: EvolutionDisposition,
    candidate_order: &[String],
    admitted_order: &[String],
    blocked_order: &[String],
    evidence_order: &[ContentHash],
    replay_order: &[ContentHash],
    budget: u64,
    budget_remaining: u64,
    max_concurrency: u32,
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    effect_receipts: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "request_id": request_id,
        "workflow_id": workflow_id,
        "objective_id": objective_id,
        "replay_identity": replay_identity,
        "candidates": candidates,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
        "signed_approval": signed_approval,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "evidence_order": evidence_order,
        "replay_order": replay_order,
        "budget": budget,
        "budget_remaining": budget_remaining,
        "max_concurrency": max_concurrency,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

fn evolution_provenance(candidates: &[EvolutionCandidate]) -> Vec<ProvenanceLink> {
    candidates
        .iter()
        .map(|candidate| ProvenanceLink {
            source_id: format!("candidate:{}", candidate.candidate_id),
            relation: "bounded-evolution-artifact".into(),
            digest: candidate.artifact_digest.clone(),
        })
        .collect()
}

fn evolution_payload(receipt: &BoundedEvolutionReceipt) -> serde_json::Value {
    evolution_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.workflow_id,
        &receipt.objective_id,
        &receipt.replay_identity,
        &receipt.candidates,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.signed_approval,
        &receipt.admission_digest,
        receipt.disposition,
        &receipt.candidate_order,
        &receipt.admitted_order,
        &receipt.blocked_order,
        &receipt.evidence_order,
        &receipt.replay_order,
        receipt.budget,
        receipt.budget_remaining,
        receipt.max_concurrency,
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn evolution_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    workflow_id: &str,
    objective_id: &str,
    replay_identity: &ContentHash,
    candidates: &[EvolutionCandidate],
    policy_allow: bool,
    protected_closure: bool,
    signed_approval: bool,
    admission_digest: &ContentHash,
    disposition: EvolutionDisposition,
    candidate_order: &[String],
    admitted_order: &[String],
    blocked_order: &[String],
    evidence_order: &[ContentHash],
    replay_order: &[ContentHash],
    budget: u64,
    budget_remaining: u64,
    max_concurrency: u32,
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    effect_receipts: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request_id,
        "workflow_id": workflow_id,
        "objective_id": objective_id,
        "replay_identity": replay_identity,
        "candidates": candidates,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
        "signed_approval": signed_approval,
        "admission_digest": admission_digest,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "evidence_order": evidence_order,
        "replay_order": replay_order,
        "budget": budget,
        "budget_remaining": budget_remaining,
        "max_concurrency": max_concurrency,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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

fn admit_bounded_evolution_internal(
    request: &BoundedEvolutionRequest,
    validate_output: bool,
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
            && request.policy_allow
            && request.signed_approval
            && request.protected_closure
            && evidence_complete
            && budget_ok
            && admitted.len() < request.max_concurrency as usize
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
            if !request.policy_allow {
                omissions.insert("request:policy-denied".into());
            }
            if !request.signed_approval {
                omissions.insert("request:signed-approval-missing".into());
            }
            if !request.protected_closure {
                omissions.insert("request:protected-closure-incomplete".into());
            }
            if admitted.len() >= request.max_concurrency as usize {
                omissions.insert(format!(
                    "candidate:{}:concurrency-ceiling-exceeded",
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
    } else if !request.protected_closure || admitted_order.is_empty() {
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
    let admission_payload = admission_digest_payload_from_parts(
        &request.request_id,
        &request.workflow_id,
        &request.objective_id,
        &request.replay_identity,
        &candidates,
        request.policy_allow,
        request.protected_closure,
        request.signed_approval,
        disposition,
        &candidate_order,
        &admitted_order,
        &blocked_order,
        &evidence_order,
        &replay_order,
        request.budget,
        request.budget.saturating_sub(spent),
        request.max_concurrency,
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let admission_digest = ContentHash::of_value(&admission_payload)
        .map_err(|error| BoundedEvolutionError::Serialization(error.to_string()))?;
    let provenance = evolution_provenance(&candidates);
    let payload = evolution_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &request.workflow_id,
        &request.objective_id,
        &request.replay_identity,
        &candidates,
        request.policy_allow,
        request.protected_closure,
        request.signed_approval,
        &admission_digest,
        disposition,
        &candidate_order,
        &admitted_order,
        &blocked_order,
        &evidence_order,
        &replay_order,
        request.budget,
        request.budget.saturating_sub(spent),
        request.max_concurrency,
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-bounded-evolution:{}", request.request_id),
        "application/vnd.aurora.adapter-bounded-evolution+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| BoundedEvolutionError::Artifact(error.to_string()))?;
    let receipt = BoundedEvolutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_bounded_evolution_request(request),
        input_digest: bounded_evolution_input_digest(request)?,
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        replay_identity: request.replay_identity.clone(),
        candidates,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        signed_approval: request.signed_approval,
        admission_digest,
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
    if validate_output {
        receipt.validate()?;
    }
    Ok(receipt)
}

pub fn admit_bounded_evolution(
    request: &BoundedEvolutionRequest,
) -> Result<BoundedEvolutionReceipt, BoundedEvolutionError> {
    admit_bounded_evolution_internal(request, true)
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
        || request.evidence_order.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_concurrency == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(BoundedEvolutionError::Invalid(
            "evolution identity, candidates, evidence, replay identity, concurrency, locality, or boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("objective_id", &request.objective_id)?;
    validate_text("boundary", &request.boundary)?;
    if request.replay_identity == ContentHash::of_bytes(b"") {
        return Err(BoundedEvolutionError::Invalid(
            "replay identity cannot be empty".into(),
        ));
    }
    validate_sorted_hashes(&request.evidence_order, "evidence_order")?;
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        validate_text("candidate_id", &candidate.candidate_id)?;
        validate_text("affected_surface", &candidate.affected_surface)?;
        validate_text("candidate.boundary", &candidate.boundary)?;
        if !ids.insert(candidate.candidate_id.clone()) || candidate.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(BoundedEvolutionError::Invalid(format!(
                "candidate {} is invalid or duplicated",
                candidate.candidate_id
            )));
        }
        if candidate.artifact_digest == ContentHash::of_bytes(b"")
            || candidate.baseline_digest == ContentHash::of_bytes(b"")
            || candidate.required_evidence.is_empty()
        {
            return Err(BoundedEvolutionError::Invalid(format!(
                "candidate {} lacks bounded digest/evidence identity",
                candidate.candidate_id
            )));
        }
        validate_sorted_hashes(&candidate.required_evidence, "required_evidence")?;
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

    #[test]
    fn request_policy_gate_prevents_admission() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = admit_bounded_evolution(&value).unwrap();
        assert!(receipt.admitted_order.is_empty());
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:bounded-evolution:blocked"]
        );
    }

    #[test]
    fn concurrency_ceiling_limits_admitted_candidates() {
        let mut value = request();
        value.max_concurrency = 1;
        let receipt = admit_bounded_evolution(&value).unwrap();
        assert_eq!(receipt.admitted_order, vec!["candidate:a"]);
        assert_eq!(receipt.blocked_order, vec!["candidate:b"]);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("concurrency-ceiling")));
    }

    #[test]
    fn admission_digest_rejects_budget_tampering() {
        let mut receipt = admit_bounded_evolution(&request()).unwrap();
        receipt.budget_remaining = 8;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn empty_replay_identity_is_rejected() {
        let mut value = request();
        value.replay_identity = ContentHash::of_bytes(b"");
        assert!(admit_bounded_evolution(&value).is_err());
    }

    #[test]
    fn candidate_gate_tampering_is_rejected() {
        let mut receipt = admit_bounded_evolution(&request()).unwrap();
        receipt.candidates[0].policy_allow = false;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evolution_artifact_payload_tampering_is_rejected() {
        let mut receipt = admit_bounded_evolution(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = admit_bounded_evolution(&request()).unwrap();
        receipt.input.budget = 1;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evolution_input_order_is_canonicalized() {
        let mut reversed = request();
        reversed.candidates.reverse();
        let canonical = admit_bounded_evolution(&request()).unwrap();
        let reordered = admit_bounded_evolution(&reversed).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}
