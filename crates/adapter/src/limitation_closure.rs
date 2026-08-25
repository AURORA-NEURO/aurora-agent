//! Federated adapter limitation-closure gateway.
//!
//! Atlas feature: `AFA-adapter-P26-F24`.
//! Compiles open, measured, resolved, blocked, and unknown limitation cases into a
//! digest-only closure receipt. A limitation is never silently promoted to a pass.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P26-F24";
pub const CONTRACT_VERSION: &str = "adapter-limitation-closure/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitationStatus {
    Open,
    Measured,
    Resolved,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterLimitationCase {
    pub case_id: String,
    pub limitation: String,
    pub scope: String,
    pub status: LimitationStatus,
    pub evidence_digests: Vec<ContentHash>,
    pub closure_criteria: Vec<String>,
    pub mitigation: String,
    pub negative_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitationClosureRequest {
    pub request_id: String,
    pub cases: Vec<AdapterLimitationCase>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_permitted: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureDisposition {
    Closed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterClosureReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub disposition: ClosureDisposition,
    pub case_order: Vec<String>,
    pub resolved_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub reasons: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl AdapterClosureReceipt {
    pub fn validate(&self) -> Result<(), LimitationClosureError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(LimitationClosureError::Contract(
                "limitation closure identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.case_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(LimitationClosureError::InvalidRequest(
                "closure identity, cases, reasons, effects, locality, and boundary are required"
                    .into(),
            ));
        }
        for values in [
            &self.case_order,
            &self.resolved_order,
            &self.unresolved_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(LimitationClosureError::InvalidRequest(
                    "closure output ordering is not canonical".into(),
                ));
            }
        }
        if self
            .evidence_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(LimitationClosureError::InvalidRequest(
                "closure evidence ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LimitationClosureError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, LimitationClosureError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| LimitationClosureError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| LimitationClosureError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum LimitationClosureError {
    #[error("invalid limitation closure request: {0}")]
    InvalidRequest(String),
    #[error("limitation closure contract rejected: {0}")]
    Contract(String),
    #[error("limitation closure serialization failed: {0}")]
    Serialization(String),
}

pub fn close_adapter_limitations(
    request: &LimitationClosureRequest,
) -> Result<AdapterClosureReceipt, LimitationClosureError> {
    validate_request(request)?;
    let mut cases = request.cases.clone();
    cases.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    let case_order = cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut resolved_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative_evidence = Vec::new();
    for case in &cases {
        for digest in &case.evidence_digests {
            evidence.insert(digest.clone());
        }
        if let Some(result) = &case.negative_result {
            negative_evidence.push(format!("{}:{}", case.case_id, result));
        }
        match case.status {
            LimitationStatus::Resolved
                if !case.evidence_digests.is_empty() && !case.closure_criteria.is_empty() =>
            {
                resolved_order.push(case.case_id.clone())
            }
            LimitationStatus::Resolved => {
                unresolved_order.push(case.case_id.clone());
                omissions.push(format!(
                    "{}:resolved-without-evidence-or-criteria",
                    case.case_id
                ));
            }
            LimitationStatus::Measured => {
                unresolved_order.push(case.case_id.clone());
                uncertainty.push(format!("{}:measured-but-not-closed", case.case_id));
            }
            LimitationStatus::Open => {
                unresolved_order.push(case.case_id.clone());
                omissions.push(format!("{}:limitation-open", case.case_id));
            }
            LimitationStatus::Blocked => {
                unresolved_order.push(case.case_id.clone());
                omissions.push(format!("{}:limitation-blocked", case.case_id));
            }
            LimitationStatus::Unknown => {
                unresolved_order.push(case.case_id.clone());
                uncertainty.push(format!("{}:limitation-unknown", case.case_id));
            }
        }
    }
    let disposition = if !request.policy_allow || !request.federation_permitted {
        ClosureDisposition::Blocked
    } else if !request.protected_closure {
        uncertainty.push("protected closure is incomplete".into());
        ClosureDisposition::Unknown
    } else if unresolved_order.is_empty() {
        ClosureDisposition::Closed
    } else if resolved_order.is_empty() {
        ClosureDisposition::Unknown
    } else {
        ClosureDisposition::Partial
    };
    let mut reasons = vec![format!(
        "{} limitation cases evaluated with explicit closure status",
        case_order.len()
    )];
    if matches!(disposition, ClosureDisposition::Blocked) {
        reasons.push("policy or federation authorization denied limitation exchange".into());
    }
    if !unresolved_order.is_empty() {
        reasons
            .push("unresolved limitations remain visible and cannot be promoted to closed".into());
    }
    reasons.sort();
    negative_evidence.sort();
    let mut effect_receipts = if matches!(
        disposition,
        ClosureDisposition::Closed | ClosureDisposition::Partial
    ) {
        vec!["exchange:permitted-limitation-digests-only".into()]
    } else {
        vec![format!("block:limitation-closure:{:?}", disposition).to_lowercase()]
    };
    effect_receipts.sort();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "disposition": disposition, "case_order": case_order, "resolved_order": resolved_order, "unresolved_order": unresolved_order, "evidence_order": evidence_order, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "reasons": reasons, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-limitation-closure:{}", request.request_id),
        "application/vnd.aurora.adapter-closure-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LimitationClosureError::Contract(error.to_string()))?;
    let result = AdapterClosureReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        disposition,
        case_order,
        resolved_order,
        unresolved_order,
        evidence_order,
        omissions,
        uncertainty,
        negative_evidence,
        reasons,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_request(request: &LimitationClosureRequest) -> Result<(), LimitationClosureError> {
    if request.request_id.trim().is_empty()
        || request.cases.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LimitationClosureError::InvalidRequest(
            "request identity, cases, locality, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for case in &request.cases {
        if case.case_id.trim().is_empty()
            || !ids.insert(case.case_id.clone())
            || case.limitation.trim().is_empty()
            || case.scope.trim().is_empty()
            || case.mitigation.trim().is_empty()
            || case
                .closure_criteria
                .iter()
                .any(|criterion| criterion.trim().is_empty())
        {
            return Err(LimitationClosureError::InvalidRequest(format!(
                "limitation case {} is invalid or duplicated",
                case.case_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> LimitationClosureRequest {
        LimitationClosureRequest {
            request_id: "request:limitations".into(),
            cases: vec![
                AdapterLimitationCase {
                    case_id: "case:drift".into(),
                    limitation: "batch drift".into(),
                    scope: "multi-study".into(),
                    status: LimitationStatus::Resolved,
                    evidence_digests: vec![ContentHash::of_bytes(b"drift")],
                    closure_criteria: vec!["replication fixture passed".into()],
                    mitigation: "recalibrate baseline".into(),
                    negative_result: None,
                },
                AdapterLimitationCase {
                    case_id: "case:coverage".into(),
                    limitation: "missing modality".into(),
                    scope: "omics".into(),
                    status: LimitationStatus::Open,
                    evidence_digests: Vec::new(),
                    closure_criteria: vec!["modality admitted".into()],
                    mitigation: "retain omission".into(),
                    negative_result: Some("null modality result".into()),
                },
            ],
            policy_allow: true,
            protected_closure: true,
            federation_permitted: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn partial_closure_retains_open_limitation_and_negative_result() {
        let result = close_adapter_limitations(&request()).unwrap();
        assert_eq!(result.disposition, ClosureDisposition::Partial);
        assert!(!result.unresolved_order.is_empty());
        assert!(!result.negative_evidence.is_empty());
    }
    #[test]
    fn resolved_cases_require_evidence_and_criteria() {
        let mut request = request();
        request.cases[0].evidence_digests.clear();
        let result = close_adapter_limitations(&request).unwrap();
        assert!(!result.omissions.is_empty());
    }
    #[test]
    fn incomplete_protected_closure_is_unknown() {
        let mut request = request();
        request.protected_closure = false;
        let result = close_adapter_limitations(&request).unwrap();
        assert_eq!(result.disposition, ClosureDisposition::Unknown);
    }
    #[test]
    fn denied_federation_blocks_exchange() {
        let mut request = request();
        request.federation_permitted = false;
        let result = close_adapter_limitations(&request).unwrap();
        assert_eq!(result.disposition, ClosureDisposition::Blocked);
    }
    #[test]
    fn closure_digest_is_deterministic() {
        let first = close_adapter_limitations(&request()).unwrap();
        let second = close_adapter_limitations(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
