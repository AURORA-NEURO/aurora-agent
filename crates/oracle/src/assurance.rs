//! Prospective oracle-contract verification and release admission.
//!
//! Atlas feature: `AFA-oracle-P25-F27`.
//! This module turns the oracle mesh's evidence ladder into a separately
//! deployable, omission-aware product boundary. It verifies typed oracle
//! claims, emits a content-addressed capability manifest, and fails closed
//! when policy, authority, protected closure, provenance, or locality is
//! incomplete. It does not execute an oracle or move experimental bytes.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oracle-P25-F27";
pub const CONTRACT_VERSION: &str = "oracle-contract-frontier-assurance/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleContractEvidence {
    pub oracle_id: String,
    pub state: OracleEvidenceState,
    pub result_digest: Option<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleContractInput {
    pub request_id: String,
    pub workflow_id: String,
    pub benchmark_id: String,
    pub scope: String,
    pub evidence: Vec<OracleContractEvidence>,
    pub source_receipt_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub local_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleAssuranceDisposition {
    Admitted,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleCapabilityManifest {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub manifest_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub benchmark_id: String,
    pub scope: String,
    pub disposition: OracleAssuranceDisposition,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub source_receipt_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub budget_remaining: u64,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: OracleManifestArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleManifestArtifact {
    pub content_hash: ContentHash,
    pub media_type: String,
    pub scope: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OracleAssuranceError {
    #[error("invalid oracle contract input: {0}")]
    Invalid(String),
    #[error("oracle capability manifest serialization failed: {0}")]
    Serialization(String),
}

impl OracleCapabilityManifest {
    pub fn validate(&self) -> Result<(), OracleAssuranceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.manifest_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || (self.admitted_order.is_empty()
                && self.blocked_order.is_empty()
                && self.omissions.is_empty()
                && self.uncertainty.is_empty()
                && self.negative_evidence.is_empty())
            || self.budget_remaining > self.budget
            || self.artifact.media_type.trim().is_empty()
            || self.artifact.scope.trim().is_empty()
        {
            return Err(OracleAssuranceError::Invalid(
                "manifest identity, typed admission, checks, effects, locality, budget, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.admitted_order,
            &self.blocked_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(OracleAssuranceError::Invalid(
                    "oracle assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.evidence_order, &self.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(OracleAssuranceError::Invalid(
                    "oracle assurance digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, OracleAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| OracleAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| OracleAssuranceError::Serialization(error.to_string()))
    }
}

pub fn operate_oracle_assurance(
    request: &OracleContractInput,
) -> Result<OracleCapabilityManifest, OracleAssuranceError> {
    validate_request(request)?;
    let mut evidence = request.evidence.clone();
    evidence.sort_by(|left, right| left.oracle_id.cmp(&right.oracle_id));
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut evidence_order = BTreeSet::new();
    let mut provenance_order = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for item in &evidence {
        let cost = (item.evidence_order.len() as u64).saturating_add(1);
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = item.result_digest.is_some()
            && !item.evidence_order.is_empty()
            && item.omissions.is_empty()
            && item.uncertainty.is_empty();
        let safe = item.state == OracleEvidenceState::Supported;
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.local_only
            && complete
            && safe
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            admitted.insert(item.oracle_id.clone());
            evidence_order.extend(item.evidence_order.iter().cloned());
            provenance_order.insert(item.provenance_digest.clone());
        } else {
            blocked.insert(item.oracle_id.clone());
            if item.state != OracleEvidenceState::Supported {
                negative.insert(
                    format!(
                        "oracle:{}:state-{:?}-not-admitted",
                        item.oracle_id, item.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if !complete {
                omissions.insert(format!(
                    "oracle:{}:protected-closure-or-evidence-incomplete",
                    item.oracle_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("oracle:{}:budget-ceiling-exceeded", item.oracle_id));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.local_only {
        negative.insert("request:raw-data-locality-required".into());
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.signed_approval || !request.local_only {
        OracleAssuranceDisposition::Blocked
    } else if !request.protected_closure {
        OracleAssuranceDisposition::Unknown
    } else if admitted_order.is_empty() {
        OracleAssuranceDisposition::Unknown
    } else if blocked_order.is_empty() {
        OracleAssuranceDisposition::Admitted
    } else {
        OracleAssuranceDisposition::Partial
    };
    let mut checks = vec![
        "canonical oracle ordering and content-addressed manifest".into(),
        "protected closure, provenance, authority, locality, and policy gates".into(),
        "contradicted, unknown, unmeasured, and omitted evidence remains visible".into(),
        "replay identity and benchmark family are bound to the release decision".into(),
    ];
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let mut effect_receipts = if !admitted_order.is_empty() {
        admitted_order
            .iter()
            .map(|id| format!("verify:oracle:{id}"))
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:oracle-assurance:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let evidence_order = evidence_order.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance_order.into_iter().collect::<Vec<_>>();
    let manifest_id = format!("oracle-manifest:{}", request.request_id);
    let payload = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "manifest_id": manifest_id,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "benchmark_id": request.benchmark_id,
        "scope": request.scope,
        "disposition": disposition,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "source_receipt_digest": request.source_receipt_digest,
        "benchmark_digest": request.benchmark_digest,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = OracleManifestArtifact {
        content_hash: ContentHash::of_value(&payload)
            .map_err(|error| OracleAssuranceError::Serialization(error.to_string()))?,
        media_type: "application/vnd.aurora.oracle-capability-manifest+json".into(),
        scope: format!("oracle-assurance:{}", request.request_id),
    };
    let manifest = OracleCapabilityManifest {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        manifest_id,
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        scope: request.scope.clone(),
        disposition,
        admitted_order,
        blocked_order,
        evidence_order,
        provenance_order,
        source_receipt_digest: request.source_receipt_digest.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        budget: request.budget,
        budget_remaining: request.budget.saturating_sub(spent),
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    manifest.validate()?;
    Ok(manifest)
}

fn validate_request(request: &OracleContractInput) -> Result<(), OracleAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.evidence.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(OracleAssuranceError::Invalid(
            "oracle identity, scope, evidence, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for item in &request.evidence {
        if item.oracle_id.trim().is_empty()
            || !ids.insert(item.oracle_id.clone())
            || item.boundary != PRECLINICAL_BOUNDARY
            || item
                .evidence_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || item.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || item.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || item
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(OracleAssuranceError::Invalid(format!(
                "oracle evidence {} is invalid or duplicated",
                item.oracle_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn evidence(id: &str, state: OracleEvidenceState) -> OracleContractEvidence {
        OracleContractEvidence {
            oracle_id: id.into(),
            state,
            result_digest: Some(hash(&format!("result:{id}"))),
            evidence_order: vec![hash(&format!("evidence:{id}"))],
            provenance_digest: hash(&format!("provenance:{id}")),
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(evidence: Vec<OracleContractEvidence>) -> OracleContractInput {
        OracleContractInput {
            request_id: "oracle:assurance".into(),
            workflow_id: "workflow:benchmark".into(),
            benchmark_id: "benchmark:held-out-family".into(),
            scope: "organoid:neural".into(),
            evidence,
            source_receipt_digest: hash("source"),
            benchmark_digest: hash("benchmark"),
            replay_identity: hash("replay"),
            budget: 10,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            local_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn admits_supported_oracle_with_complete_evidence() {
        let manifest = operate_oracle_assurance(&request(vec![evidence(
            "oracle:a",
            OracleEvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(manifest.disposition, OracleAssuranceDisposition::Admitted);
        assert_eq!(manifest.admitted_order, vec!["oracle:a"]);
        assert_eq!(manifest.digest(), manifest.digest());
    }

    #[test]
    fn preserves_contradiction_and_partial_admission() {
        let manifest = operate_oracle_assurance(&request(vec![
            evidence("oracle:a", OracleEvidenceState::Supported),
            evidence("oracle:b", OracleEvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(manifest.disposition, OracleAssuranceDisposition::Partial);
        assert_eq!(manifest.blocked_order, vec!["oracle:b"]);
        assert!(!manifest.negative_evidence.is_empty());
    }

    #[test]
    fn protected_closure_gap_is_unknown_not_a_pass() {
        let mut input = request(vec![evidence("oracle:a", OracleEvidenceState::Supported)]);
        input.protected_closure = false;
        let manifest = operate_oracle_assurance(&input).unwrap();
        assert_eq!(manifest.disposition, OracleAssuranceDisposition::Unknown);
        assert!(manifest
            .uncertainty
            .iter()
            .any(|value| value.contains("protected-closure")));
    }

    #[test]
    fn denied_locality_blocks_and_records_effect() {
        let mut input = request(vec![evidence("oracle:a", OracleEvidenceState::Supported)]);
        input.local_only = false;
        let manifest = operate_oracle_assurance(&input).unwrap();
        assert_eq!(manifest.disposition, OracleAssuranceDisposition::Blocked);
        assert!(manifest.effect_receipts[0].starts_with("block:"));
    }

    #[test]
    fn duplicate_oracle_ids_are_rejected() {
        let result = operate_oracle_assurance(&request(vec![
            evidence("oracle:a", OracleEvidenceState::Supported),
            evidence("oracle:a", OracleEvidenceState::Supported),
        ]));
        assert!(result.is_err());
    }
}
