//! Prospective high-throughput decision-context compilation assurance.
//!
//! Atlas feature: `AFA-adapter-P03-F27`.
//!
//! This adapter-side harness certifies the closure of a typed decision query before a
//! high-throughput research workflow can consume it.  It is a verification product, not a
//! compiler: missing facts, absent derivation, policy denial, or incomplete protected closure
//! remain explicit and cannot be promoted by an agent.

use bioprism_foundation::{
    PolicyDecision, ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P03-F27";
pub const CONTRACT_VERSION: &str = "prospective-context-compilation-assurance/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub query_id: String,
    pub requester: String,
    pub intent: String,
    pub required_fact_ids: Vec<String>,
    pub resolved_fact_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationRequest {
    pub request_id: String,
    pub query: DecisionQuery,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub admission_reference: Option<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompilationDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: ContextCompilationRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub query_id: String,
    pub requester: String,
    pub intent: String,
    pub required_fact_ids: Vec<String>,
    pub resolved_fact_ids: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub admission_reference: Option<String>,
    pub disposition: ContextCompilationDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedContext {
    resolved_fact_ids: Vec<String>,
    disposition: ContextCompilationDisposition,
    checks: Vec<String>,
    omissions: Vec<String>,
}

fn derive_context(request: &ContextCompilationRequest) -> DerivedContext {
    let mut resolved = request.query.resolved_fact_ids.clone();
    resolved.sort();
    let mut required = request.query.required_fact_ids.clone();
    required.sort();
    let missing = required
        .iter()
        .filter(|fact| !resolved.contains(fact))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = missing
        .iter()
        .map(|fact| format!("required decision fact unavailable: {fact}"))
        .collect::<Vec<_>>();
    let admission_ready = request
        .admission_reference
        .as_ref()
        .is_some_and(|reference| !reference.trim().is_empty());
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !admission_ready;
    if blocked {
        omissions.push(
            "policy, admission, or protected-closure gate blocked context certification".into(),
        );
    }
    if request.query.evidence_receipt_digest.is_none() {
        omissions.push("decision-context derivation receipt is absent".into());
    }
    let disposition = if blocked {
        ContextCompilationDisposition::Blocked
    } else if !missing.is_empty() || request.query.evidence_receipt_digest.is_none() {
        ContextCompilationDisposition::Unknown
    } else {
        ContextCompilationDisposition::Passed
    };
    let mut checks = vec![
        "decision fact identities are canonicalized".into(),
        "protected closure and derivation are checked before admission".into(),
        "local research context remains outside any federation envelope".into(),
    ];
    checks.push(match disposition {
        ContextCompilationDisposition::Passed => {
            "required facts, derivation, and admission reference passed".into()
        }
        ContextCompilationDisposition::Blocked => {
            "policy, admission, or protected closure blocked certification".into()
        }
        ContextCompilationDisposition::Unknown => {
            "incomplete decision context remains unknown rather than certified".into()
        }
    });
    checks.sort();
    checks.dedup();
    omissions.sort();
    omissions.dedup();
    DerivedContext {
        resolved_fact_ids: resolved,
        disposition,
        checks,
        omissions,
    }
}

fn context_provenance(evidence_receipt_digest: &Option<ContentHash>) -> Vec<ProvenanceLink> {
    evidence_receipt_digest
        .iter()
        .map(|digest| ProvenanceLink {
            source_id: "evidence-receipt".into(),
            relation: "context-derivation-receipt".into(),
            digest: digest.clone(),
        })
        .collect()
}

impl ContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.required_fact_ids.is_empty()
            || self.checks.is_empty()
        {
            return Err(ContextCompilationError::InvalidField(
                "context compilation identity, boundary, or checks are incomplete".into(),
            ));
        }
        if self
            .resolved_fact_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.resolved_fact_ids.len()
        {
            return Err(ContextCompilationError::InvalidField(
                "resolved fact identities are not unique".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("query_id", &self.query_id)?;
        validate_text("requester", &self.requester)?;
        validate_text("intent", &self.intent)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("required_fact_ids", &self.required_fact_ids)?;
        validate_sorted_strings("resolved_fact_ids", &self.resolved_fact_ids)?;
        validate_sorted_strings("checks", &self.checks)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        if self.artifact.artifact_id != format!("certified-decision-section:{}", self.query_id)
            || self.artifact.content_type
                != "application/vnd.aurora.certified-decision-section+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance != context_provenance(&self.evidence_receipt_digest)
        {
            return Err(ContextCompilationError::Artifact(
                "context artifact is not bound to the retained query and evidence".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&context_payload(self))
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != context_input_digest(&self.input)? {
            return Err(ContextCompilationError::InvalidField(
                "context compilation retained input digest does not match the request".into(),
            ));
        }
        let expected = build_context_compilation(&self.input)?;
        if self != &expected {
            return Err(ContextCompilationError::InvalidField(
                "context compilation receipt is not derived from its retained request".into(),
            ));
        }
        if self.disposition == ContextCompilationDisposition::Passed && !self.omissions.is_empty() {
            return Err(ContextCompilationError::InvalidField(
                "passed context compilation cannot retain omissions".into(),
            ));
        }
        if self.disposition != ContextCompilationDisposition::Passed && self.omissions.is_empty() {
            return Err(ContextCompilationError::InvalidField(
                "non-passed context compilation must explain its omissions".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextCompilationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextCompilationError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), ContextCompilationError> {
    if value.is_empty() || value.trim() != value {
        return Err(ContextCompilationError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ContextCompilationError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn context_input_digest(
    request: &ContextCompilationRequest,
) -> Result<ContentHash, ContextCompilationError> {
    let value = serde_json::to_value(&canonical_context_request(request))
        .map_err(|error| ContextCompilationError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ContextCompilationError::Serialization(error.to_string()))
}

fn canonical_context_request(request: &ContextCompilationRequest) -> ContextCompilationRequest {
    let mut canonical = request.clone();
    canonical.query.required_fact_ids.sort();
    canonical.query.resolved_fact_ids.sort();
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), ContextCompilationError> {
    if values.len() > MAX_ITEMS {
        return Err(ContextCompilationError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = std::collections::BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ContextCompilationError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ContextCompilationError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextCompilationError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn context_payload(receipt: &ContextCompilationReceipt) -> serde_json::Value {
    context_payload_from_parts(
        &receipt.schema_version,
        &receipt.feature_id,
        &receipt.contract_version,
        &receipt.request_id,
        &receipt.query_id,
        &receipt.requester,
        &receipt.intent,
        &receipt.required_fact_ids,
        &receipt.resolved_fact_ids,
        receipt.policy_decision,
        receipt.protected_closure_satisfied,
        receipt.admission_reference.as_ref(),
        receipt.disposition,
        &receipt.evidence_receipt_digest,
        &receipt.checks,
        &receipt.omissions,
        &receipt.artifact.provenance,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn context_payload_from_parts(
    schema_version: &str,
    feature_id: &str,
    contract_version: &str,
    request_id: &str,
    query_id: &str,
    requester: &str,
    intent: &str,
    required_fact_ids: &[String],
    resolved_fact_ids: &[String],
    policy_decision: PolicyDecision,
    protected_closure_satisfied: bool,
    admission_reference: Option<&String>,
    disposition: ContextCompilationDisposition,
    evidence_receipt_digest: &Option<ContentHash>,
    checks: &[String],
    omissions: &[String],
    provenance: &[ProvenanceLink],
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "feature_id": feature_id,
        "contract_version": contract_version,
        "request_id": request_id,
        "query_id": query_id,
        "requester": requester,
        "intent": intent,
        "required_fact_ids": required_fact_ids,
        "resolved_fact_ids": resolved_fact_ids,
        "policy_decision": policy_decision,
        "protected_closure_satisfied": protected_closure_satisfied,
        "admission_reference": admission_reference,
        "disposition": disposition,
        "evidence_receipt_digest": evidence_receipt_digest,
        "checks": checks,
        "omissions": omissions,
        "provenance": provenance,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum ContextCompilationError {
    #[error("invalid context compilation field: {0}")]
    InvalidField(String),
    #[error("context compilation artifact error: {0}")]
    Artifact(String),
    #[error("context compilation serialization error: {0}")]
    Serialization(String),
}

pub fn assure_context_compilation(
    request: &ContextCompilationRequest,
) -> Result<ContextCompilationReceipt, ContextCompilationError> {
    let receipt = build_context_compilation(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_context_compilation(
    request: &ContextCompilationRequest,
) -> Result<ContextCompilationReceipt, ContextCompilationError> {
    validate_request(request)?;
    let canonical = canonical_context_request(request);
    let derived = derive_context(&canonical);
    let provenance = context_provenance(&canonical.query.evidence_receipt_digest);
    let payload = context_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        FEATURE_ID,
        CONTRACT_VERSION,
        &canonical.request_id,
        &canonical.query.query_id,
        &canonical.query.requester,
        &canonical.query.intent,
        &canonical.query.required_fact_ids,
        &derived.resolved_fact_ids,
        canonical.policy_decision,
        canonical.protected_closure_satisfied,
        canonical.admission_reference.as_ref(),
        derived.disposition,
        &canonical.query.evidence_receipt_digest,
        &derived.checks,
        &derived.omissions,
        &provenance,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("certified-decision-section:{}", canonical.query.query_id),
        "application/vnd.aurora.certified-decision-section+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let receipt = ContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input: canonical.clone(),
        input_digest: context_input_digest(request)?,
        request_id: canonical.request_id,
        query_id: canonical.query.query_id,
        requester: canonical.query.requester,
        intent: canonical.query.intent,
        required_fact_ids: canonical.query.required_fact_ids,
        resolved_fact_ids: derived.resolved_fact_ids,
        policy_decision: canonical.policy_decision,
        protected_closure_satisfied: canonical.protected_closure_satisfied,
        admission_reference: canonical.admission_reference,
        disposition: derived.disposition,
        evidence_receipt_digest: canonical.query.evidence_receipt_digest,
        checks: derived.checks,
        omissions: derived.omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &ContextCompilationRequest) -> Result<(), ContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.query.query_id.trim().is_empty()
        || request.query.requester.trim().is_empty()
        || request.query.intent.trim().is_empty()
        || request.query.required_fact_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextCompilationError::InvalidField(
            "decision query identity, required facts, requester, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("query_id", &request.query.query_id)?;
    validate_text("requester", &request.query.requester)?;
    validate_text("intent", &request.query.intent)?;
    validate_text("boundary", &request.boundary)?;
    if request.query.required_fact_ids.len() > MAX_ITEMS
        || request.query.resolved_fact_ids.len() > MAX_ITEMS
    {
        return Err(ContextCompilationError::InvalidField(
            "context fact count exceeds its bound".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for fact in &request.query.required_fact_ids {
        validate_text("required_fact_id", fact)?;
        if !seen.insert(fact.clone()) {
            return Err(ContextCompilationError::InvalidField(
                "required fact identities must be non-empty and unique".into(),
            ));
        }
    }
    let required = seen;
    let mut resolved = std::collections::BTreeSet::new();
    for fact in &request.query.resolved_fact_ids {
        validate_text("resolved_fact_id", fact)?;
        if !resolved.insert(fact.clone()) {
            return Err(ContextCompilationError::InvalidField(
                "resolved fact identities must be unique".into(),
            ));
        }
        if !required.contains(fact) {
            return Err(ContextCompilationError::InvalidField(
                "resolved facts must be required by the decision query".into(),
            ));
        }
    }
    if let Some(reference) = &request.admission_reference {
        validate_text("admission_reference", reference)?;
    }
    if request.policy_decision == PolicyDecision::Allow
        && !request
            .admission_reference
            .as_ref()
            .is_some_and(|reference| !reference.trim().is_empty())
    {
        return Err(ContextCompilationError::InvalidField(
            "prospective context admission requires a reference".into(),
        ));
    }
    if request
        .query
        .evidence_receipt_digest
        .as_ref()
        .is_some_and(|digest| *digest == ContentHash::of_bytes(b""))
    {
        return Err(ContextCompilationError::InvalidField(
            "evidence receipt digest must be non-empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fact_stays_unknown() {
        let receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context".into(),
            query: DecisionQuery {
                query_id: "query:mechanism".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into(), "fact:b".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: None,
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:context".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn complete_context_passes_with_admission_and_evidence() {
        let digest = ContentHash::of_bytes(b"context-evidence");
        let receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context-pass".into(),
            query: DecisionQuery {
                query_id: "query:mechanism-pass".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:b".into(), "fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into(), "fact:b".into()],
                evidence_receipt_digest: Some(digest),
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:context-pass".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Passed);
        assert!(receipt.omissions.is_empty());
    }

    #[test]
    fn whitespace_admission_reference_is_rejected() {
        let mut request = ContextCompilationRequest {
            request_id: "request:context".into(),
            query: DecisionQuery {
                query_id: "query:mechanism".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: Some(ContentHash::of_bytes(b"evidence")),
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("   ".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(assure_context_compilation(&request).is_err());
        request.admission_reference = None;
        assert!(assure_context_compilation(&request).is_err());
    }

    #[test]
    fn resolved_fact_outside_query_scope_is_rejected() {
        let request = ContextCompilationRequest {
            request_id: "request:context".into(),
            query: DecisionQuery {
                query_id: "query:mechanism".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into(), "fact:extra".into()],
                evidence_receipt_digest: None,
            },
            policy_decision: PolicyDecision::Deny,
            protected_closure_satisfied: false,
            admission_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(assure_context_compilation(&request).is_err());
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context".into(),
            query: DecisionQuery {
                query_id: "query:mechanism".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into(), "fact:b".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: None,
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:context".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.resolved_fact_ids[0] = "fact:z".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn retained_query_scope_tampering_is_rejected() {
        let digest = ContentHash::of_bytes(b"context-evidence");
        let mut receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context-scope".into(),
            query: DecisionQuery {
                query_id: "query:scope".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: Some(digest),
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:scope".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.required_fact_ids[0] = "fact:forged".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:retained".into(),
            query: DecisionQuery {
                query_id: "query:retained".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: Some(ContentHash::of_bytes(b"context-evidence")),
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:retained".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.input.query.intent = "tampered intent".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn derivation_receipt_provenance_tampering_is_rejected() {
        let digest = ContentHash::of_bytes(b"context-evidence");
        let mut receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context-provenance".into(),
            query: DecisionQuery {
                query_id: "query:provenance".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: Some(digest),
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:provenance".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
