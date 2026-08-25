//! Multimodal adapter semantic-parity contract model.
//!
//! Atlas feature: `AFA-adapter-P28-F06`.
//!
//! Compares independently produced adapter summaries without importing raw outputs. Schema,
//! modality, and semantic digests must agree before parity is admitted; disagreement and missing
//! modalities remain explicit unknown evidence.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P28-F06";
pub const CONTRACT_VERSION: &str = "adapter-semantic-parity/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSemanticReport {
    pub adapter_id: String,
    pub study_id: String,
    pub schema_fingerprint: ContentHash,
    pub semantic_digest: ContentHash,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSemanticParityRequest {
    pub request_id: String,
    pub objective_id: String,
    pub required_modality_order: Vec<String>,
    pub reports: Vec<AdapterSemanticReport>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticParityDisposition {
    Passed,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterSemanticParityReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective_id: String,
    pub disposition: SemanticParityDisposition,
    pub adapter_order: Vec<String>,
    pub study_order: Vec<String>,
    pub schema_order: Vec<ContentHash>,
    pub semantic_digest: Option<ContentHash>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl AdapterSemanticParityReceipt {
    pub fn validate(&self) -> Result<(), SemanticParityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.adapter_order.len() < 2
            || self.study_order.len() < 2
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(SemanticParityError::Invalid("semantic parity identity, reports, checks, effects, locality, or boundary are incomplete".into()));
        }
        for values in [
            &self.adapter_order,
            &self.study_order,
            &self.modality_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(SemanticParityError::Invalid(
                    "semantic parity ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.schema_order, &self.artifact_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(SemanticParityError::Invalid(
                    "semantic parity digest ordering is not canonical".into(),
                ));
            }
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
    #[error("invalid adapter semantic parity request: {0}")]
    Invalid(String),
    #[error("adapter semantic parity artifact error: {0}")]
    Artifact(String),
    #[error("adapter semantic parity serialization error: {0}")]
    Serialization(String),
}

pub fn evaluate_adapter_semantic_parity(
    request: &AdapterSemanticParityRequest,
) -> Result<AdapterSemanticParityReceipt, SemanticParityError> {
    validate_request(request)?;
    let mut reports = request.reports.clone();
    reports.sort_by(|a, b| a.adapter_id.cmp(&b.adapter_id));
    let adapter_order = reports
        .iter()
        .map(|r| r.adapter_id.clone())
        .collect::<Vec<_>>();
    let study_order = reports
        .iter()
        .map(|r| r.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let schema_order = reports
        .iter()
        .map(|r| r.schema_fingerprint.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let artifact_order = reports
        .iter()
        .flat_map(|r| r.artifact_order.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = reports
        .iter()
        .flat_map(|r| r.modality_order.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let semantic_digest = reports.first().map(|r| r.semantic_digest.clone());
    let mut checks = vec![
        "adapter reports are ordered by stable adapter id".into(),
        "schema and semantic digests are compared without moving raw outputs".into(),
        "modality and artifact identities remain content-addressed".into(),
    ];
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative_evidence = Vec::new();
    let required = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed = modality_order.iter().cloned().collect::<BTreeSet<_>>();
    for modality in required.difference(&observed) {
        omissions.push(format!("modality:{modality}:missing"));
        negative_evidence.push(format!("modality:{modality}:no-admitted-adapter-evidence"));
    }
    let parity_match = reports.windows(2).all(|pair| {
        pair[0].schema_fingerprint == pair[1].schema_fingerprint
            && pair[0].semantic_digest == pair[1].semantic_digest
            && canonical(&pair[0].modality_order) == canonical(&pair[1].modality_order)
    });
    if !parity_match {
        uncertainty.push("adapter schema, semantic, or modality digests disagree".into());
    }
    let disposition = if !request.policy_allow || !request.raw_data_local {
        checks.push("policy or locality denied parity admission".into());
        SemanticParityDisposition::Blocked
    } else if !request.protected_closure {
        uncertainty.push("protected closure is incomplete".into());
        SemanticParityDisposition::Unknown
    } else if !parity_match || !omissions.is_empty() {
        checks.push("semantic disagreement or missing modality remains unknown".into());
        SemanticParityDisposition::Unknown
    } else {
        checks.push("adapter schema, semantic, and modality digests agree".into());
        SemanticParityDisposition::Passed
    };
    checks.sort();
    omissions.sort();
    uncertainty.sort();
    negative_evidence.sort();
    let effect_receipts = if disposition == SemanticParityDisposition::Passed {
        vec!["exchange:permitted-semantic-parity-digests-only".into()]
    } else {
        vec![format!("block:adapter-semantic-parity:{disposition:?}").to_lowercase()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"objective_id":request.objective_id,"disposition":disposition,"adapter_order":adapter_order,"study_order":study_order,"schema_order":schema_order,"semantic_digest":semantic_digest,"modality_order":modality_order,"artifact_order":artifact_order,"checks":checks,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative_evidence,"effect_receipts":effect_receipts,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-semantic-parity:{}", request.request_id),
        "application/vnd.aurora.adapter-semantic-parity+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
    let receipt = AdapterSemanticParityReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective_id.clone(),
        disposition,
        adapter_order,
        study_order,
        schema_order,
        semantic_digest,
        modality_order,
        artifact_order,
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

fn canonical(values: &[String]) -> Vec<String> {
    let mut result = values.to_vec();
    result.sort();
    result.dedup();
    result
}
fn validate_request(request: &AdapterSemanticParityRequest) -> Result<(), SemanticParityError> {
    if request.request_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.required_modality_order.is_empty()
        || request.reports.len() < 2
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(SemanticParityError::Invalid("semantic parity identity, required modalities, reports, locality, and boundary are required".into()));
    }
    let mut ids = BTreeSet::new();
    for report in &request.reports {
        if report.adapter_id.trim().is_empty()
            || report.study_id.trim().is_empty()
            || !ids.insert(report.adapter_id.clone())
            || report.modality_order.is_empty()
            || report.artifact_order.is_empty()
            || report.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(SemanticParityError::Invalid("adapter report identity, modalities, artifacts, uniqueness, and boundary are required".into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(id: &str, semantic: &[u8]) -> AdapterSemanticReport {
        AdapterSemanticReport {
            adapter_id: id.into(),
            study_id: format!("study:{id}"),
            schema_fingerprint: ContentHash::of_bytes(b"schema"),
            semantic_digest: ContentHash::of_bytes(semantic),
            modality_order: vec!["imaging".into(), "omics".into()],
            artifact_order: vec![ContentHash::of_bytes(id.as_bytes())],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request() -> AdapterSemanticParityRequest {
        AdapterSemanticParityRequest {
            request_id: "parity:adapter".into(),
            objective_id: "objective:qc".into(),
            required_modality_order: vec!["imaging".into(), "omics".into()],
            reports: vec![report("adapter:b", b"same"), report("adapter:a", b"same")],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn matching_reports_pass() {
        let r = evaluate_adapter_semantic_parity(&request()).unwrap();
        assert_eq!(r.disposition, SemanticParityDisposition::Passed);
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn disagreement_is_unknown() {
        let mut q = request();
        q.reports[1].semantic_digest = ContentHash::of_bytes(b"different");
        assert_eq!(
            evaluate_adapter_semantic_parity(&q).unwrap().disposition,
            SemanticParityDisposition::Unknown
        );
    }
    #[test]
    fn missing_modality_is_unknown() {
        let mut q = request();
        q.required_modality_order.push("spatial".into());
        let r = evaluate_adapter_semantic_parity(&q).unwrap();
        assert_eq!(r.disposition, SemanticParityDisposition::Unknown);
        assert!(!r.negative_evidence.is_empty());
    }
    #[test]
    fn protected_gap_is_unknown() {
        let mut q = request();
        q.protected_closure = false;
        assert_eq!(
            evaluate_adapter_semantic_parity(&q).unwrap().disposition,
            SemanticParityDisposition::Unknown
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            evaluate_adapter_semantic_parity(&q).unwrap().disposition,
            SemanticParityDisposition::Blocked
        );
    }
}
