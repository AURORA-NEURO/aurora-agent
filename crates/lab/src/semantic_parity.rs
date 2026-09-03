//! Federated lab semantic-parity assurance for protocol simulation summaries.
//!
//! Atlas feature: `AFA-lab-P28-F12`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P28-F12";
pub const CONTRACT_VERSION: &str = "lab-semantic-parity/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabInstitutionReport {
    pub institution_id: String,
    pub report_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub scenario_ids: Vec<String>,
    pub passed_scenarios: usize,
    pub failed_closed_scenarios: usize,
    pub approval_scenarios: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabSemanticParityRequest {
    pub request_id: String,
    pub federation_id: String,
    pub protocol_id: String,
    pub benchmark_id: String,
    pub institutions: Vec<LabInstitutionReport>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticParityDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabSemanticParityReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub protocol_id: String,
    pub benchmark_id: String,
    pub institution_ids: Vec<String>,
    pub disposition: SemanticParityDisposition,
    pub semantic_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl LabSemanticParityReceipt {
    pub fn validate(&self) -> Result<(), SemanticParityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.institution_ids.len() < 2
            || self
                .institution_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.checks.is_empty()
        {
            return Err(SemanticParityError::InvalidField(
                "semantic parity identity, ordering, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| SemanticParityError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, SemanticParityError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| SemanticParityError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| SemanticParityError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum SemanticParityError {
    #[error("invalid lab semantic parity field: {0}")]
    InvalidField(String),
    #[error("lab semantic parity artifact error: {0}")]
    Artifact(String),
    #[error("lab semantic parity serialization error: {0}")]
    Serialization(String),
}

pub fn evaluate_semantic_parity(
    request: &LabSemanticParityRequest,
) -> Result<LabSemanticParityReceipt, SemanticParityError> {
    validate_request(request)?;
    let mut institutions = request.institutions.clone();
    institutions.sort_by(|left, right| left.institution_id.cmp(&right.institution_id));
    let institution_ids = institutions
        .iter()
        .map(|institution| institution.institution_id.clone())
        .collect::<Vec<_>>();
    let semantic_digest = institutions
        .first()
        .map(|institution| institution.semantic_digest.clone());
    let mut checks = vec![
        "institution reports are ordered by stable id".to_string(),
        "scenario counts partition each institution report".to_string(),
        "raw protocol outputs remain institution-local".to_string(),
    ];
    let mut omissions = Vec::new();
    let parity_match = institutions.windows(2).all(|pair| {
        pair[0].semantic_digest == pair[1].semantic_digest
            && canonical_scenarios(&pair[0].scenario_ids)
                == canonical_scenarios(&pair[1].scenario_ids)
    });
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented parity admission".into());
        SemanticParityDisposition::Blocked
    } else if !parity_match {
        omissions.push("institution semantic or scenario identities disagree".into());
        checks.push("semantic disagreement remains unknown rather than a consensus".into());
        SemanticParityDisposition::Unknown
    } else {
        checks.push("institution semantic digests and scenario identities agree".into());
        SemanticParityDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "protocol_id": request.protocol_id,
        "benchmark_id": request.benchmark_id,
        "institution_ids": institution_ids,
        "disposition": disposition,
        "semantic_digest": semantic_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("lab-semantic-parity:{}", request.request_id),
        "application/vnd.aurora.lab-semantic-parity+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
    let receipt = LabSemanticParityReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        protocol_id: request.protocol_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        institution_ids,
        disposition,
        semantic_digest,
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn canonical_scenarios(scenarios: &[String]) -> Vec<String> {
    let mut values = scenarios.to_vec();
    values.sort();
    values.dedup();
    values
}

fn validate_request(request: &LabSemanticParityRequest) -> Result<(), SemanticParityError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.protocol_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.institutions.len() < 2
    {
        return Err(SemanticParityError::InvalidField(
            "semantic parity identity, institutions, and boundary are required".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for institution in &request.institutions {
        if institution.institution_id.trim().is_empty()
            || !ids.insert(institution.institution_id.clone())
            || institution.scenario_ids.is_empty()
            || institution.passed_scenarios
                + institution.failed_closed_scenarios
                + institution.approval_scenarios
                != institution.scenario_ids.len()
        {
            return Err(SemanticParityError::InvalidField(
                "institution ids, scenarios, and status counts must be complete".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(site: &str, semantic: &[u8]) -> LabInstitutionReport {
        LabInstitutionReport {
            institution_id: site.into(),
            report_digest: ContentHash::of_bytes(site.as_bytes()),
            semantic_digest: ContentHash::of_bytes(semantic),
            scenario_ids: vec!["s1".into(), "s2".into()],
            passed_scenarios: 1,
            failed_closed_scenarios: 1,
            approval_scenarios: 0,
        }
    }

    fn request() -> LabSemanticParityRequest {
        LabSemanticParityRequest {
            request_id: "request:parity".into(),
            federation_id: "federation:lab".into(),
            protocol_id: "protocol:organoid".into(),
            benchmark_id: "benchmark:lab".into(),
            institutions: vec![report("site:b", b"semantic"), report("site:a", b"semantic")],
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn semantic_disagreement_is_unknown() {
        let mut input = request();
        input.institutions[1].semantic_digest = ContentHash::of_bytes(b"different");
        let receipt = evaluate_semantic_parity(&input).unwrap();
        assert_eq!(receipt.disposition, SemanticParityDisposition::Unknown);
    }

    #[test]
    fn matching_semantics_pass() {
        let receipt = evaluate_semantic_parity(&request()).unwrap();
        assert_eq!(receipt.disposition, SemanticParityDisposition::Passed);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
