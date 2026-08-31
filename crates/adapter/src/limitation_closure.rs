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
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

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
    pub input: LimitationClosureRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_permitted: bool,
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
    pub case_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

fn validate_text(field: &str, value: &str) -> Result<(), LimitationClosureError> {
    if value.is_empty() || value.trim() != value {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn limitation_input_digest(
    request: &LimitationClosureRequest,
) -> Result<ContentHash, LimitationClosureError> {
    let value = serde_json::to_value(&canonical_limitation_closure_request(request))
        .map_err(|error| LimitationClosureError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| LimitationClosureError::Serialization(error.to_string()))
}

fn canonical_limitation_closure_request(
    request: &LimitationClosureRequest,
) -> LimitationClosureRequest {
    let mut canonical = request.clone();
    for case in &mut canonical.cases {
        case.evidence_digests.sort();
        case.closure_criteria.sort();
    }
    canonical
        .cases
        .sort_by(|left, right| left.case_id.cmp(&right.case_id));
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), LimitationClosureError> {
    if values.len() > MAX_ITEMS {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(LimitationClosureError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), LimitationClosureError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &ContentHash) -> Result<(), LimitationClosureError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn validate_sorted_digests(
    field: &str,
    digests: &[ContentHash],
) -> Result<(), LimitationClosureError> {
    if digests.len() > MAX_ITEMS {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for digest in digests {
        validate_digest(field, digest)?;
    }
    if digests
        .windows(2)
        .any(|pair| pair[0].as_str() >= pair[1].as_str())
    {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_unique_digests(
    field: &str,
    digests: &[ContentHash],
) -> Result<(), LimitationClosureError> {
    if digests.len() > MAX_ITEMS {
        return Err(LimitationClosureError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for digest in digests {
        validate_digest(field, digest)?;
        if !unique.insert(digest.as_str().to_string()) {
            return Err(LimitationClosureError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

impl AdapterClosureReceipt {
    pub fn validate(&self) -> Result<(), LimitationClosureError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.case_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(LimitationClosureError::Contract(
                "limitation closure identity mismatch".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("case_order", &self.case_order)?;
        validate_sorted_strings("resolved_order", &self.resolved_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_digests("evidence_order", &self.evidence_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        let resolved = self.resolved_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let classified = resolved
            .union(&unresolved)
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.case_order.iter().cloned().collect::<BTreeSet<_>>()
            || resolved.intersection(&unresolved).next().is_some()
            || self.resolved_order.len() + self.unresolved_order.len() != self.case_order.len()
        {
            return Err(LimitationClosureError::InvalidRequest(
                "resolved and unresolved cases must partition the case closure".into(),
            ));
        }
        let blocked = !self.policy_allow || !self.federation_permitted;
        let expected_disposition = if blocked {
            ClosureDisposition::Blocked
        } else if !self.protected_closure {
            ClosureDisposition::Unknown
        } else if self.unresolved_order.is_empty() {
            ClosureDisposition::Closed
        } else if self.resolved_order.is_empty() {
            ClosureDisposition::Unknown
        } else {
            ClosureDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(LimitationClosureError::InvalidRequest(
                "closure disposition does not match policy and case states".into(),
            ));
        }
        if (!self.protected_closure && !blocked)
            != self
                .uncertainty
                .contains(&"protected closure is incomplete".to_string())
        {
            return Err(LimitationClosureError::InvalidRequest(
                "protected-closure uncertainty is not represented exactly".into(),
            ));
        }
        let mut expected_reasons = vec![format!(
            "{} limitation cases evaluated with explicit closure status",
            self.case_order.len()
        )];
        if blocked {
            expected_reasons
                .push("policy or federation authorization denied limitation exchange".into());
        }
        if !self.unresolved_order.is_empty() {
            expected_reasons.push(
                "unresolved limitations remain visible and cannot be promoted to closed".into(),
            );
        }
        expected_reasons.sort();
        if self.reasons != expected_reasons {
            return Err(LimitationClosureError::InvalidRequest(
                "closure reasons are not bound to disposition and case states".into(),
            ));
        }
        let expected_effect = if matches!(
            self.disposition,
            ClosureDisposition::Closed | ClosureDisposition::Partial
        ) {
            vec!["exchange:permitted-limitation-digests-only".to_string()]
        } else {
            vec![format!("block:limitation-closure:{:?}", self.disposition).to_lowercase()]
        };
        if self.effect_receipts != expected_effect {
            return Err(LimitationClosureError::InvalidRequest(
                "closure effect does not match its exchange state".into(),
            ));
        }
        validate_digest("case_digest", &self.case_digest)?;
        if self.artifact.artifact_id != format!("adapter-limitation-closure:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.adapter-closure-receipt+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(LimitationClosureError::Contract(
                "limitation closure artifact is not bound to the receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "federation_permitted": self.federation_permitted,
            "disposition": self.disposition,
            "case_order": self.case_order,
            "resolved_order": self.resolved_order,
            "unresolved_order": self.unresolved_order,
            "evidence_order": self.evidence_order,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "reasons": self.reasons,
            "effect_receipts": self.effect_receipts,
            "case_digest": self.case_digest,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| LimitationClosureError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| LimitationClosureError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != limitation_input_digest(&self.input)? {
            return Err(LimitationClosureError::Contract(
                "limitation closure retained input digest does not match the request".into(),
            ));
        }
        let expected = build_limitation_closure(&self.input)?;
        if self != &expected {
            return Err(LimitationClosureError::Contract(
                "limitation closure receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
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
    let receipt = build_limitation_closure(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_limitation_closure(
    request: &LimitationClosureRequest,
) -> Result<AdapterClosureReceipt, LimitationClosureError> {
    validate_request(request)?;
    let mut cases = request.cases.clone();
    for case in &mut cases {
        case.evidence_digests
            .sort_by(|left, right| left.as_str().cmp(right.as_str()));
        case.closure_criteria.sort();
    }
    cases.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    let case_digest = ContentHash::of_value(
        &serde_json::to_value(&cases)
            .map_err(|error| LimitationClosureError::Serialization(error.to_string()))?,
    )
    .map_err(|error| LimitationClosureError::Serialization(error.to_string()))?;
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
    omissions.sort();
    uncertainty.sort();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "policy_allow": request.policy_allow,
        "protected_closure": request.protected_closure,
        "federation_permitted": request.federation_permitted,
        "disposition": disposition,
        "case_order": case_order,
        "resolved_order": resolved_order,
        "unresolved_order": unresolved_order,
        "evidence_order": evidence_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "reasons": reasons,
        "effect_receipts": effect_receipts,
        "case_digest": case_digest,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
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
        input: canonical_limitation_closure_request(request),
        input_digest: limitation_input_digest(request)?,
        request_id: request.request_id.clone(),
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        federation_permitted: request.federation_permitted,
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
        case_digest,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(result)
}

fn validate_request(request: &LimitationClosureRequest) -> Result<(), LimitationClosureError> {
    if request.request_id.trim().is_empty()
        || request.cases.is_empty()
        || request.cases.len() > MAX_ITEMS
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LimitationClosureError::InvalidRequest(
            "request identity, cases, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("boundary", &request.boundary)?;
    let mut ids = BTreeSet::new();
    for case in &request.cases {
        if !ids.insert(case.case_id.clone()) {
            return Err(LimitationClosureError::InvalidRequest(format!(
                "limitation case {} is invalid or duplicated",
                case.case_id
            )));
        }
        validate_text("case.case_id", &case.case_id)?;
        validate_text("case.limitation", &case.limitation)?;
        validate_text("case.scope", &case.scope)?;
        validate_text("case.mitigation", &case.mitigation)?;
        validate_unique_digests("case.evidence_digests", &case.evidence_digests)?;
        validate_unique_strings("case.closure_criteria", &case.closure_criteria)?;
        if let Some(negative_result) = &case.negative_result {
            validate_text("case.negative_result", negative_result)?;
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

    #[test]
    fn case_input_order_is_canonicalized() {
        let mut reversed = request();
        reversed.cases.reverse();
        reversed.cases[0].closure_criteria.reverse();
        let first = close_adapter_limitations(&request()).unwrap();
        let second = close_adapter_limitations(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn duplicate_evidence_digest_is_rejected() {
        let mut value = request();
        let duplicate = value.cases[0].evidence_digests[0].clone();
        value.cases[0].evidence_digests.push(duplicate);
        assert!(close_adapter_limitations(&value).is_err());
    }

    #[test]
    fn artifact_payload_tampering_is_rejected() {
        let mut receipt = close_adapter_limitations(&request()).unwrap();
        receipt.case_digest = ContentHash::of_bytes(b"tampered-case");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = close_adapter_limitations(&request()).unwrap();
        receipt.input.policy_allow = false;
        assert!(receipt.validate().is_err());
    }
}
