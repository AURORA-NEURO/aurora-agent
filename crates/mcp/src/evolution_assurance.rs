//! Adversarial assurance for bounded research evolution receipts.
//!
//! Atlas feature: `AFA-mcp-P32-F27`.
//! This MCP-owned gate independently verifies an adapter admission receipt before
//! it can be treated as release evidence. It never executes, signs, deploys, or
//! mutates a candidate. Missing evidence is unknown; failed safety gates block.

use bioprism_adapter::{BoundedEvolutionReceipt, EvolutionDisposition};
use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mcp-P32-F27";
pub const CONTRACT_VERSION: &str = "mcp-bounded-evolution-assurance/1.0";
pub const TOOL_NAME: &str = "mcp_bounded_evolution_assurance";

pub const REQUIRED_CHECKS: [&str; 10] = [
    "adversarial-containment",
    "canonical-order",
    "locality",
    "negative-evidence",
    "policy-authority",
    "protected-closure",
    "release-boundary",
    "replay-integrity",
    "signed-approval",
    "source-receipt",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceVerdict {
    Pass,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceCheck {
    pub check_id: String,
    pub passed: bool,
    pub evidence_digest: ContentHash,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub source_receipt: BoundedEvolutionReceipt,
    pub expected_receipt_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub checks: Vec<AssuranceCheck>,
    pub policy_allow: bool,
    pub signed_approval: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub source_receipt_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub verdict: AssuranceVerdict,
    pub passed_checks: Vec<String>,
    pub failed_checks: Vec<String>,
    pub missing_checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvolutionAssuranceError {
    #[error("invalid bounded evolution assurance request: {0}")]
    Invalid(String),
    #[error("bounded evolution assurance artifact error: {0}")]
    Artifact(String),
    #[error("bounded evolution assurance serialization error: {0}")]
    Serialization(String),
}

impl EvolutionAssuranceReceipt {
    pub fn validate(&self) -> Result<(), EvolutionAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvolutionAssuranceError::Invalid(
                "assurance identity, effect receipt, locality, or preclinical boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.passed_checks,
            &self.failed_checks,
            &self.missing_checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvolutionAssuranceError::Invalid(
                    "assurance evidence ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvolutionAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvolutionAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvolutionAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvolutionAssuranceError::Serialization(error.to_string()))
    }
}

pub fn assure_bounded_evolution(
    request: &EvolutionAssuranceRequest,
) -> Result<EvolutionAssuranceReceipt, EvolutionAssuranceError> {
    validate_request(request)?;
    let source_receipt_digest = request
        .source_receipt
        .digest()
        .map_err(|error| EvolutionAssuranceError::Invalid(error.to_string()))?;
    let mut supplied = HashMap::new();
    for check in &request.checks {
        supplied.insert(check.check_id.as_str(), check);
    }

    let source_ok = source_receipt_digest == request.expected_receipt_digest;
    let replay_ok = !request.replay_identity.as_str().is_empty()
        && !request.source_receipt.replay_order.is_empty();
    let closure_ok = request.protected_closure && request.source_receipt.uncertainty.is_empty();
    let authority_ok =
        request.policy_allow && request.source_receipt.disposition != EvolutionDisposition::Blocked;
    let approval_ok = request.signed_approval;
    let locality_ok = request.raw_data_local && request.source_receipt.raw_data_local;
    let adversarial_ok = request.source_receipt.effect_receipts.iter().all(|effect| {
        let effect = effect.to_ascii_lowercase();
        (!effect.contains("deploy") || effect.contains("no-deploy"))
            && (!effect.contains("execute") || effect.contains("no-execution"))
    });
    let negative_ok = !request.source_receipt.negative_evidence.is_empty()
        || request.source_receipt.blocked_order.is_empty();
    let canonical_ok = request.source_receipt.validate().is_ok();
    let release_ok = request.source_receipt.disposition == EvolutionDisposition::Admitted
        && request.source_receipt.blocked_order.is_empty()
        && request.source_receipt.uncertainty.is_empty();
    let computed = [
        ("source-receipt", source_ok),
        ("replay-integrity", replay_ok),
        ("protected-closure", closure_ok),
        ("policy-authority", authority_ok),
        ("signed-approval", approval_ok),
        ("locality", locality_ok),
        ("adversarial-containment", adversarial_ok),
        ("negative-evidence", negative_ok),
        ("canonical-order", canonical_ok),
        ("release-boundary", release_ok),
    ];

    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omissions = BTreeSet::from_iter(request.source_receipt.omissions.iter().cloned());
    let mut uncertainty = BTreeSet::from_iter(request.source_receipt.uncertainty.iter().cloned());
    let mut negative =
        BTreeSet::from_iter(request.source_receipt.negative_evidence.iter().cloned());
    for (check_id, computed_pass) in computed {
        match supplied.get(check_id) {
            None => {
                missing.insert(check_id.to_string());
                uncertainty.insert(format!("assurance:missing-check:{check_id}"));
            }
            Some(check) if check.passed && computed_pass => {
                passed.insert(check_id.to_string());
            }
            Some(check) => {
                failed.insert(check_id.to_string());
                if !check.reason.trim().is_empty() {
                    negative.insert(format!("check:{check_id}:{}", check.reason));
                }
            }
        }
    }
    if !adversarial_ok {
        negative.insert("assurance:effect-boundary-attempted-execution-or-deployment".into());
    }
    if !negative_ok {
        omissions.insert("assurance:blocked-candidates-lack-negative-evidence".into());
    }
    let verdict = if !missing.is_empty() {
        AssuranceVerdict::Unknown
    } else if !failed.is_empty() {
        AssuranceVerdict::Blocked
    } else {
        AssuranceVerdict::Pass
    };
    let mut effect_receipts = if matches!(verdict, AssuranceVerdict::Pass) {
        vec!["effect:assurance-receipt-only-no-execution-or-deployment".to_string()]
    } else {
        vec![format!("block:unsafe-release:{verdict:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let passed_checks = passed.into_iter().collect::<Vec<_>>();
    let failed_checks = failed.into_iter().collect::<Vec<_>>();
    let missing_checks = missing.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let payload = json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "source_receipt_digest": source_receipt_digest,
        "verdict": verdict,
        "passed_checks": passed_checks,
        "failed_checks": failed_checks,
        "missing_checks": missing_checks,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("mcp-bounded-evolution-assurance:{}", request.request_id),
        "application/vnd.aurora.mcp-bounded-evolution-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvolutionAssuranceError::Artifact(error.to_string()))?;
    let receipt = EvolutionAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        source_receipt_digest,
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        verdict,
        passed_checks,
        failed_checks,
        missing_checks,
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

fn validate_request(request: &EvolutionAssuranceRequest) -> Result<(), EvolutionAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || request.checks.is_empty()
        || request.source_receipt.validate().is_err()
    {
        return Err(EvolutionAssuranceError::Invalid(
            "assurance identity, source receipt, checks, locality, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for check in &request.checks {
        if !REQUIRED_CHECKS.contains(&check.check_id.as_str())
            || check.check_id.trim().is_empty()
            || check.reason.chars().any(|character| character.is_control())
            || !ids.insert(check.check_id.clone())
        {
            return Err(EvolutionAssuranceError::Invalid(
                "checks must be known, unique, printable, and independently named".into(),
            ));
        }
    }
    if request
        .checks
        .windows(2)
        .any(|pair| pair[0].check_id >= pair[1].check_id)
    {
        return Err(EvolutionAssuranceError::Invalid(
            "checks must be in canonical order".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_adapter::{admit_bounded_evolution, BoundedEvolutionRequest, EvolutionCandidate};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn source() -> BoundedEvolutionReceipt {
        admit_bounded_evolution(&BoundedEvolutionRequest {
            request_id: "request:assurance".into(),
            workflow_id: "workflow:assurance".into(),
            objective_id: "objective:research".into(),
            candidates: vec![EvolutionCandidate {
                candidate_id: "candidate:a".into(),
                artifact_digest: hash("artifact"),
                baseline_digest: hash("baseline"),
                required_evidence: vec![hash("evidence")],
                replayable: true,
                deterministic: true,
                safety_reviewed: true,
                policy_allow: true,
                resource_cost: 2,
                affected_surface: "research:cell-system".into(),
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            evidence_order: vec![hash("evidence")],
            replay_identity: hash("replay"),
            budget: 10,
            max_concurrency: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap()
    }

    fn request() -> EvolutionAssuranceRequest {
        let source_receipt = source();
        let expected_receipt_digest = source_receipt.digest().unwrap();
        let checks = REQUIRED_CHECKS
            .iter()
            .map(|check_id| AssuranceCheck {
                check_id: (*check_id).into(),
                passed: true,
                evidence_digest: hash(check_id),
                reason: "independently replayed".into(),
            })
            .collect();
        EvolutionAssuranceRequest {
            request_id: "assurance:request".into(),
            workflow_id: "workflow:assurance".into(),
            source_receipt,
            expected_receipt_digest,
            replay_identity: hash("replay"),
            benchmark_digest: hash("benchmark"),
            checks,
            policy_allow: true,
            signed_approval: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn complete_admission_passes_only_with_all_gates() {
        let receipt = assure_bounded_evolution(&request()).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Pass);
        assert!(receipt.failed_checks.is_empty());
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn missing_adversarial_check_is_unknown_not_pass() {
        let mut request = request();
        request
            .checks
            .retain(|check| check.check_id != "adversarial-containment");
        let receipt = assure_bounded_evolution(&request).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Unknown);
        assert!(receipt
            .missing_checks
            .contains(&"adversarial-containment".into()));
    }

    #[test]
    fn failed_release_boundary_blocks_and_retains_negative_evidence() {
        let mut request = request();
        request
            .checks
            .iter_mut()
            .find(|check| check.check_id == "release-boundary")
            .unwrap()
            .passed = false;
        let receipt = assure_bounded_evolution(&request).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert!(!receipt.negative_evidence.is_empty());
        assert!(receipt.effect_receipts[0].contains("block:unsafe-release"));
    }

    #[test]
    fn source_digest_mismatch_blocks() {
        let mut request = request();
        request.expected_receipt_digest = hash("wrong");
        let receipt = assure_bounded_evolution(&request).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert!(receipt.failed_checks.contains(&"source-receipt".into()));
    }
}
