//! Multimodal adapter semantic-parity contract model.
//!
//! Atlas feature: `AFA-adapter-P28-F06`.
//!
//! Compares independently produced adapter summaries without importing raw outputs. Schema,
//! modality, and semantic digests must agree before parity is admitted; disagreement and missing
//! modalities remain explicit unknown evidence.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P28-F06";
pub const CONTRACT_VERSION: &str = "adapter-semantic-parity/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

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
    pub required_modality_order: Vec<String>,
    pub reports: Vec<AdapterSemanticReport>,
    pub policy_allow: bool,
    pub protected_closure: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedParity {
    disposition: SemanticParityDisposition,
    adapter_order: Vec<String>,
    study_order: Vec<String>,
    schema_order: Vec<ContentHash>,
    semantic_digest: Option<ContentHash>,
    modality_order: Vec<String>,
    artifact_order: Vec<ContentHash>,
    checks: Vec<String>,
    omissions: Vec<String>,
    uncertainty: Vec<String>,
    negative_evidence: Vec<String>,
    effect_receipts: Vec<String>,
}

fn canonical_report(report: &AdapterSemanticReport) -> AdapterSemanticReport {
    let mut canonical = report.clone();
    canonical.modality_order.sort();
    canonical.modality_order.dedup();
    canonical.artifact_order.sort();
    canonical.artifact_order.dedup();
    canonical
}

fn canonical_reports(reports: &[AdapterSemanticReport]) -> Vec<AdapterSemanticReport> {
    let mut canonical = reports.iter().map(canonical_report).collect::<Vec<_>>();
    canonical.sort_by(|left, right| left.adapter_id.cmp(&right.adapter_id));
    canonical
}

fn report_provenance(
    reports: &[AdapterSemanticReport],
) -> Result<Vec<ProvenanceLink>, SemanticParityError> {
    reports
        .iter()
        .map(|report| {
            let value = serde_json::to_value(report)
                .map_err(|error| SemanticParityError::Serialization(error.to_string()))?;
            let digest = ContentHash::of_value(&value)
                .map_err(|error| SemanticParityError::Serialization(error.to_string()))?;
            Ok(ProvenanceLink {
                source_id: format!("adapter-report:{}", report.adapter_id),
                relation: "semantic-parity-report".into(),
                digest,
            })
        })
        .collect()
}

fn derive_parity(
    required_modalities: &[String],
    reports: &[AdapterSemanticReport],
    policy_allow: bool,
    protected_closure: bool,
    raw_data_local: bool,
) -> DerivedParity {
    let adapter_order = reports
        .iter()
        .map(|report| report.adapter_id.clone())
        .collect::<Vec<_>>();
    let study_order = reports
        .iter()
        .map(|report| report.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let schema_order = reports
        .iter()
        .map(|report| report.schema_fingerprint.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let artifact_order = reports
        .iter()
        .flat_map(|report| report.artifact_order.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = reports
        .iter()
        .flat_map(|report| report.modality_order.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let semantic_digest = reports.first().map(|report| report.semantic_digest.clone());
    let mut checks = vec![
        "adapter reports are ordered by stable adapter id".into(),
        "schema and semantic digests are compared without moving raw outputs".into(),
        "modality and artifact identities remain content-addressed".into(),
    ];
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative_evidence = Vec::new();
    let required = required_modalities.iter().cloned().collect::<BTreeSet<_>>();
    let observed = modality_order.iter().cloned().collect::<BTreeSet<_>>();
    for modality in required.difference(&observed) {
        omissions.push(format!("modality:{modality}:missing"));
        negative_evidence.push(format!("modality:{modality}:no-admitted-adapter-evidence"));
    }
    let parity_match = reports.windows(2).all(|pair| {
        pair[0].schema_fingerprint == pair[1].schema_fingerprint
            && pair[0].semantic_digest == pair[1].semantic_digest
            && pair[0].modality_order == pair[1].modality_order
    });
    if !parity_match {
        uncertainty.push("adapter schema, semantic, or modality digests disagree".into());
    }
    let disposition = if !policy_allow || !raw_data_local {
        checks.push("policy or locality denied parity admission".into());
        SemanticParityDisposition::Blocked
    } else if !protected_closure {
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
    checks.dedup();
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let effect_receipts = if disposition == SemanticParityDisposition::Passed {
        vec!["exchange:permitted-semantic-parity-digests-only".into()]
    } else {
        vec![format!("block:adapter-semantic-parity:{disposition:?}").to_lowercase()]
    };
    DerivedParity {
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
    }
}

impl AdapterSemanticParityReceipt {
    pub fn validate(&self) -> Result<(), SemanticParityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(SemanticParityError::Invalid(
                "semantic parity contract identity is incomplete".into(),
            ));
        }
        let request = AdapterSemanticParityRequest {
            request_id: self.request_id.clone(),
            objective_id: self.objective_id.clone(),
            required_modality_order: self.required_modality_order.clone(),
            reports: self.reports.clone(),
            policy_allow: self.policy_allow,
            protected_closure: self.protected_closure,
            raw_data_local: self.raw_data_local,
            boundary: self.boundary.clone(),
        };
        validate_request(&request)?;
        let canonical_reports = canonical_reports(&self.reports);
        if self.reports != canonical_reports {
            return Err(SemanticParityError::Invalid(
                "semantic parity reports are not canonically ordered".into(),
            ));
        }
        let mut canonical_required = self.required_modality_order.clone();
        canonical_required.sort();
        if self.required_modality_order != canonical_required {
            return Err(SemanticParityError::Invalid(
                "required modality order is not canonical".into(),
            ));
        }
        for (field, values) in [
            ("adapter_order", &self.adapter_order),
            ("study_order", &self.study_order),
            ("modality_order", &self.modality_order),
            ("checks", &self.checks),
            ("omissions", &self.omissions),
            ("uncertainty", &self.uncertainty),
            ("negative_evidence", &self.negative_evidence),
            ("effect_receipts", &self.effect_receipts),
        ] {
            validate_sorted_strings(field, values)?;
        }
        for (field, values) in [
            ("schema_order", &self.schema_order),
            ("artifact_order", &self.artifact_order),
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1])
                || values
                    .iter()
                    .any(|digest| *digest == ContentHash::of_bytes(b""))
            {
                return Err(SemanticParityError::Invalid(format!(
                    "{field} ordering or digest content is invalid"
                )));
            }
        }
        if self.semantic_digest.as_ref() == Some(&ContentHash::of_bytes(b"")) {
            return Err(SemanticParityError::Invalid(
                "semantic digest must be non-empty when present".into(),
            ));
        }
        let derived = derive_parity(
            &self.required_modality_order,
            &self.reports,
            self.policy_allow,
            self.protected_closure,
            self.raw_data_local,
        );
        if self.disposition != derived.disposition
            || self.adapter_order != derived.adapter_order
            || self.study_order != derived.study_order
            || self.schema_order != derived.schema_order
            || self.semantic_digest != derived.semantic_digest
            || self.modality_order != derived.modality_order
            || self.artifact_order != derived.artifact_order
            || self.checks != derived.checks
            || self.omissions != derived.omissions
            || self.uncertainty != derived.uncertainty
            || self.negative_evidence != derived.negative_evidence
            || self.effect_receipts != derived.effect_receipts
        {
            return Err(SemanticParityError::Invalid(
                "semantic parity receipt is not derived from its reports and gates".into(),
            ));
        }
        let provenance = report_provenance(&self.reports)?;
        if self.artifact.artifact_id != format!("adapter-semantic-parity:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.adapter-semantic-parity+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance != provenance
        {
            return Err(SemanticParityError::Artifact(
                "semantic parity artifact is not bound to its reports".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&semantic_parity_payload(self))
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

fn semantic_parity_payload(receipt: &AdapterSemanticParityReceipt) -> serde_json::Value {
    semantic_parity_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.objective_id,
        &receipt.required_modality_order,
        &receipt.reports,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.disposition,
        &receipt.adapter_order,
        &receipt.study_order,
        &receipt.schema_order,
        &receipt.semantic_digest,
        &receipt.modality_order,
        &receipt.artifact_order,
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
fn semantic_parity_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    objective_id: &str,
    required_modality_order: &[String],
    reports: &[AdapterSemanticReport],
    policy_allow: bool,
    protected_closure: bool,
    disposition: SemanticParityDisposition,
    adapter_order: &[String],
    study_order: &[String],
    schema_order: &[ContentHash],
    semantic_digest: &Option<ContentHash>,
    modality_order: &[String],
    artifact_order: &[ContentHash],
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
        "objective_id": objective_id,
        "required_modality_order": required_modality_order,
        "reports": reports,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
        "disposition": disposition,
        "adapter_order": adapter_order,
        "study_order": study_order,
        "schema_order": schema_order,
        "semantic_digest": semantic_digest,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
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
    let mut required_modalities = request.required_modality_order.clone();
    required_modalities.sort();
    let reports = canonical_reports(&request.reports);
    let derived = derive_parity(
        &required_modalities,
        &reports,
        request.policy_allow,
        request.protected_closure,
        request.raw_data_local,
    );
    let provenance = report_provenance(&reports)?;
    let payload = semantic_parity_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &request.objective_id,
        &required_modalities,
        &reports,
        request.policy_allow,
        request.protected_closure,
        derived.disposition,
        &derived.adapter_order,
        &derived.study_order,
        &derived.schema_order,
        &derived.semantic_digest,
        &derived.modality_order,
        &derived.artifact_order,
        &derived.checks,
        &derived.omissions,
        &derived.uncertainty,
        &derived.negative_evidence,
        &derived.effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-semantic-parity:{}", request.request_id),
        "application/vnd.aurora.adapter-semantic-parity+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
    let receipt = AdapterSemanticParityReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective_id.clone(),
        required_modality_order: required_modalities,
        reports,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition: derived.disposition,
        adapter_order: derived.adapter_order,
        study_order: derived.study_order,
        schema_order: derived.schema_order,
        semantic_digest: derived.semantic_digest,
        modality_order: derived.modality_order,
        artifact_order: derived.artifact_order,
        checks: derived.checks,
        omissions: derived.omissions,
        uncertainty: derived.uncertainty,
        negative_evidence: derived.negative_evidence,
        effect_receipts: derived.effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_text(field: &str, value: &str) -> Result<(), SemanticParityError> {
    if value.is_empty() || value.trim() != value {
        return Err(SemanticParityError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(SemanticParityError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), SemanticParityError> {
    if values.len() > MAX_ITEMS {
        return Err(SemanticParityError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(SemanticParityError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), SemanticParityError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SemanticParityError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
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
    validate_text("request_id", &request.request_id)?;
    validate_text("objective_id", &request.objective_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_unique_strings("required_modality_order", &request.required_modality_order)?;
    if request.reports.len() > MAX_ITEMS {
        return Err(SemanticParityError::Invalid(
            "semantic parity report count exceeds its bound".into(),
        ));
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
        validate_text("report.adapter_id", &report.adapter_id)?;
        validate_text("report.study_id", &report.study_id)?;
        validate_text("report.boundary", &report.boundary)?;
        if report.schema_fingerprint == ContentHash::of_bytes(b"")
            || report.semantic_digest == ContentHash::of_bytes(b"")
        {
            return Err(SemanticParityError::Invalid(
                "adapter report content digests are required".into(),
            ));
        }
        validate_unique_strings("report.modality_order", &report.modality_order)?;
        if report.artifact_order.len() > MAX_ITEMS
            || report
                .artifact_order
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(SemanticParityError::Invalid(
                "adapter report artifact digests are invalid".into(),
            ));
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

    #[test]
    fn report_tampering_is_rejected() {
        let mut receipt = evaluate_adapter_semantic_parity(&request()).unwrap();
        receipt.reports[0].semantic_digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn policy_state_tampering_is_rejected() {
        let mut receipt = evaluate_adapter_semantic_parity(&request()).unwrap();
        receipt.policy_allow = false;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn parity_artifact_payload_tampering_is_rejected() {
        let mut receipt = evaluate_adapter_semantic_parity(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
