//! Structural conformance for provider-supplied CI artifacts, logs, and attestations.
//!
//! [`ci_provider`] converts provider-shaped payloads into canonical run evidence. This module
//! adds the next handoff layer: it binds optional artifact, log, and attestation records to the
//! normalized provider/run/check identities and computes deterministic record digests. It does
//! not fetch bytes, inspect logs, verify signatures, authenticate a provider, or turn a caller
//! declaration into external execution authority.

use std::collections::BTreeSet;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::ci_evidence::{
    audit_ci_execution_evidence, CiEvidenceFinding, CiExecutionEvidenceRequest,
};
use crate::ci_provider::{normalize_ci_provider_payload, CiProviderNormalizationRequest};
use crate::workbench::CiRequest;

pub const CI_PROVIDER_EVIDENCE_SCHEMA: &str = "bioprism-devplat-ci-provider-evidence/0.1";
const MAX_ROWS: usize = 128;
const MAX_TEXT: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CiProviderArtifact {
    pub id: String,
    pub kind: String,
    pub digest: String,
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CiProviderLog {
    pub id: String,
    pub digest: String,
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CiProviderAttestation {
    pub id: String,
    pub subject: String,
    pub issuer: String,
    pub statement_digest: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiProviderEvidenceRequest {
    pub ci: CiRequest,
    pub provider: String,
    pub payload: Value,
    #[serde(default)]
    pub source: Option<crate::ci_evidence::CiEvidenceSource>,
    #[serde(default)]
    pub artifacts: Vec<CiProviderArtifact>,
    #[serde(default)]
    pub logs: Vec<CiProviderLog>,
    #[serde(default)]
    pub attestations: Vec<CiProviderAttestation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CiProviderEvidenceAudit {
    pub schema: String,
    pub workflow: String,
    pub provider: String,
    pub source: crate::ci_evidence::CiEvidenceSource,
    pub run_id: String,
    pub payload_digest: String,
    pub plan_digest: String,
    pub evidence_digest: String,
    pub artifact_record_digest: String,
    pub log_record_digest: String,
    pub attestation_record_digest: String,
    pub artifact_count: usize,
    pub log_count: usize,
    pub attestation_count: usize,
    pub linked_artifact_count: usize,
    pub linked_log_count: usize,
    pub attestation_subject_count: usize,
    pub ci_evidence: crate::ci_evidence::CiExecutionEvidenceAudit,
    pub artifacts: Vec<CiProviderArtifact>,
    pub logs: Vec<CiProviderLog>,
    pub attestations: Vec<CiProviderAttestation>,
    pub structurally_valid: bool,
    pub conformance_ready: bool,
    pub execution: String,
    pub verification: String,
    pub findings: Vec<CiEvidenceFinding>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CiProviderEvidenceError {
    #[error("provider normalization refused: {0}")]
    Normalization(String),
    #[error("CI execution evidence refused: {0}")]
    Evidence(String),
    #[error("{field} contains {count} rows, above the bound {MAX_ROWS}")]
    TooManyRows { field: &'static str, count: usize },
    #[error("cannot canonicalize provider evidence: {0}")]
    Canonical(String),
}

fn finding(
    findings: &mut Vec<CiEvidenceFinding>,
    code: &str,
    severity: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
) {
    findings.push(CiEvidenceFinding {
        code: code.into(),
        severity: severity.into(),
        subject: subject.into(),
        detail: detail.into(),
    });
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    ContentHash::parse(value.to_owned()).is_ok()
}

fn check_known(check: Option<&str>, expected: &BTreeSet<String>) -> bool {
    check.map(|value| expected.contains(value)).unwrap_or(true)
}

fn bind_row(
    findings: &mut Vec<CiEvidenceFinding>,
    subject: &str,
    row_provider: Option<&str>,
    row_run_id: Option<&str>,
    row_check: Option<&str>,
    provider: &str,
    run_id: &str,
    expected_checks: &BTreeSet<String>,
) {
    if !valid_text(subject) {
        finding(
            findings,
            "row_id_invalid",
            "blocking",
            subject.to_owned(),
            "provider evidence row identifiers must be bounded, non-empty, and free of control characters",
        );
    }
    match row_provider {
        Some(value) if value == provider => {}
        Some(value) => finding(
            findings,
            "provider_binding_mismatch",
            "blocking",
            subject.to_owned(),
            format!("row provider {value:?} does not match normalized provider {provider:?}"),
        ),
        None => finding(
            findings,
            "provider_binding_missing",
            "blocking",
            subject.to_owned(),
            "artifact and log rows must carry the provider identity they were observed under",
        ),
    }
    match row_run_id {
        Some(value) if value == run_id => {}
        Some(value) => finding(
            findings,
            "run_binding_mismatch",
            "blocking",
            subject.to_owned(),
            format!("row run_id {value:?} does not match normalized run_id {run_id:?}"),
        ),
        None => finding(
            findings,
            "run_binding_missing",
            "blocking",
            subject.to_owned(),
            "artifact and log rows must carry the provider run identity they came from",
        ),
    }
    if !check_known(row_check, expected_checks) {
        finding(
            findings,
            "unknown_check_binding",
            "blocking",
            subject.to_owned(),
            "row references a check that is not present in the canonical CI plan",
        );
    }
}

fn validate_uri(findings: &mut Vec<CiEvidenceFinding>, subject: &str, uri: Option<&str>) {
    if let Some(uri) = uri {
        if !valid_text(uri) {
            finding(
                findings,
                "row_uri_invalid",
                "blocking",
                subject.to_owned(),
                "provider evidence URIs must be bounded and free of control characters",
            );
        }
    }
}

fn digest_rows<T: Serialize>(rows: &[T]) -> Result<String, CiProviderEvidenceError> {
    let value = serde_json::to_value(rows)
        .map_err(|error| CiProviderEvidenceError::Canonical(error.to_string()))?;
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| CiProviderEvidenceError::Canonical(error.to_string()))
}

/// Normalize a provider payload, audit its exact CI plan, and bind optional evidence rows.
pub fn audit_ci_provider_evidence(
    request: &CiProviderEvidenceRequest,
) -> Result<CiProviderEvidenceAudit, CiProviderEvidenceError> {
    if request.artifacts.len() > MAX_ROWS {
        return Err(CiProviderEvidenceError::TooManyRows {
            field: "artifacts",
            count: request.artifacts.len(),
        });
    }
    if request.logs.len() > MAX_ROWS {
        return Err(CiProviderEvidenceError::TooManyRows {
            field: "logs",
            count: request.logs.len(),
        });
    }
    if request.attestations.len() > MAX_ROWS {
        return Err(CiProviderEvidenceError::TooManyRows {
            field: "attestations",
            count: request.attestations.len(),
        });
    }

    let normalized = normalize_ci_provider_payload(&CiProviderNormalizationRequest {
        ci: request.ci.clone(),
        provider: request.provider.clone(),
        payload: request.payload.clone(),
        source: request.source,
    })
    .map_err(|error| CiProviderEvidenceError::Normalization(error.to_string()))?;
    let ci_evidence = audit_ci_execution_evidence(&CiExecutionEvidenceRequest {
        ci: request.ci.clone(),
        evidence: normalized.evidence.clone(),
    })
    .map_err(|error| CiProviderEvidenceError::Evidence(error.to_string()))?;
    let expected_checks = request
        .ci
        .checks
        .iter()
        .map(|check| check.name.clone())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();
    let mut artifact_ids = BTreeSet::new();
    let mut log_ids = BTreeSet::new();
    let mut known_subjects = BTreeSet::from([normalized.evidence.run_id.clone()]);
    let mut linked_artifact_count = 0;
    let mut linked_log_count = 0;

    for artifact in &request.artifacts {
        if !artifact_ids.insert(artifact.id.clone()) {
            finding(
                &mut findings,
                "duplicate_artifact_id",
                "blocking",
                artifact.id.clone(),
                "each provider artifact must have one canonical row",
            );
        }
        bind_row(
            &mut findings,
            &artifact.id,
            artifact.provider.as_deref(),
            artifact.run_id.as_deref(),
            artifact.check.as_deref(),
            &normalized.provider,
            &normalized.evidence.run_id,
            &expected_checks,
        );
        if !valid_text(&artifact.kind) {
            finding(
                &mut findings,
                "artifact_kind_invalid",
                "blocking",
                artifact.id.clone(),
                "artifact kind must be bounded and non-empty",
            );
        }
        if !valid_digest(&artifact.digest) {
            finding(
                &mut findings,
                "artifact_digest_invalid",
                "blocking",
                artifact.id.clone(),
                "artifact digest is not a valid content digest",
            );
        }
        validate_uri(&mut findings, &artifact.id, artifact.uri.as_deref());
        if artifact.check.is_some() {
            linked_artifact_count += 1;
        }
        known_subjects.insert(artifact.id.clone());
    }

    for log in &request.logs {
        if !log_ids.insert(log.id.clone()) {
            finding(
                &mut findings,
                "duplicate_log_id",
                "blocking",
                log.id.clone(),
                "each provider log must have one canonical row",
            );
        }
        bind_row(
            &mut findings,
            &log.id,
            log.provider.as_deref(),
            log.run_id.as_deref(),
            log.check.as_deref(),
            &normalized.provider,
            &normalized.evidence.run_id,
            &expected_checks,
        );
        if !valid_digest(&log.digest) {
            finding(
                &mut findings,
                "log_digest_invalid",
                "blocking",
                log.id.clone(),
                "log digest is not a valid content digest",
            );
        }
        validate_uri(&mut findings, &log.id, log.uri.as_deref());
        if log.check.is_some() {
            linked_log_count += 1;
        }
        known_subjects.insert(log.id.clone());
    }

    let mut attestation_ids = BTreeSet::new();
    let mut attestation_subject_count = 0;
    for attestation in &request.attestations {
        if !attestation_ids.insert(attestation.id.clone()) {
            finding(
                &mut findings,
                "duplicate_attestation_id",
                "blocking",
                attestation.id.clone(),
                "each provider attestation must have one canonical row",
            );
        }
        if !valid_text(&attestation.issuer) || !valid_text(&attestation.method) {
            finding(
                &mut findings,
                "attestation_metadata_invalid",
                "blocking",
                attestation.id.clone(),
                "attestation issuer and method must be bounded and non-empty",
            );
        }
        if !valid_digest(&attestation.statement_digest) {
            finding(
                &mut findings,
                "attestation_digest_invalid",
                "blocking",
                attestation.id.clone(),
                "attestation statement_digest is not a valid content digest",
            );
        }
        if !known_subjects.contains(&attestation.subject) {
            finding(
                &mut findings,
                "attestation_subject_unknown",
                "blocking",
                attestation.id.clone(),
                "attestation subject must identify the normalized run, artifact, or log row",
            );
        } else {
            attestation_subject_count += 1;
        }
    }

    findings.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then_with(|| left.code.cmp(&right.code))
    });
    let structurally_valid = ci_evidence.structurally_valid
        && findings
            .iter()
            .all(|finding| finding.severity != "blocking");
    let conformance_ready = structurally_valid && ci_evidence.release_candidate;
    Ok(CiProviderEvidenceAudit {
        schema: CI_PROVIDER_EVIDENCE_SCHEMA.into(),
        workflow: "ci_provider_evidence_audit".into(),
        provider: normalized.provider,
        source: normalized.source,
        run_id: normalized.evidence.run_id,
        payload_digest: normalized.payload_digest,
        plan_digest: ci_evidence.plan_digest.clone(),
        evidence_digest: ci_evidence.evidence_digest.clone(),
        artifact_record_digest: digest_rows(&request.artifacts)?,
        log_record_digest: digest_rows(&request.logs)?,
        attestation_record_digest: digest_rows(&request.attestations)?,
        artifact_count: request.artifacts.len(),
        log_count: request.logs.len(),
        attestation_count: request.attestations.len(),
        linked_artifact_count,
        linked_log_count,
        attestation_subject_count,
        ci_evidence,
        artifacts: request.artifacts.clone(),
        logs: request.logs.clone(),
        attestations: request.attestations.clone(),
        structurally_valid,
        conformance_ready,
        execution: "evidence_supplied_not_executed_here".into(),
        verification: "structural_only".into(),
        findings,
        guarantees: vec![
            "provider payloads are normalized and the canonical CI plan is regenerated before evidence rows are assessed".into(),
            "artifact, log, and attestation records remain separately digest-bound and linked to provider/run/check identities".into(),
            "duplicate, unknown, unbound, malformed, and tampered-looking rows remain blocking findings rather than being discarded".into(),
        ],
        limitations: vec![
            "the route does not fetch artifacts or logs, verify signatures, authenticate providers, or execute checks".into(),
            "row digests identify caller-supplied records and do not prove the content at a remote URI".into(),
            "attestation statements are preserved for later external verification but are not cryptographically verified here".into(),
            "conformance_ready is a structural handoff signal, not deployment, scientific, clinical, security, or production approval".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci_evidence::CiEvidenceSource;
    use crate::workbench::{CiCheck, CiRequest};

    fn digest(label: &str) -> String {
        ContentHash::of_bytes(label.as_bytes()).to_string()
    }

    fn request() -> CiProviderEvidenceRequest {
        CiProviderEvidenceRequest {
            ci: CiRequest {
                workflow: "provider-evidence".into(),
                triggers: vec!["push".into()],
                rust_toolchain: "stable".into(),
                checks: vec![CiCheck {
                    name: "unit".into(),
                    run: "cargo test -p core".into(),
                    working_directory: None,
                    required: true,
                }],
                offline: true,
            },
            provider: "github_actions".into(),
            payload: serde_json::json!({
                "run": {"id": 99, "conclusion": "success"},
                "jobs": [{"name": "unit", "conclusion": "success"}]
            }),
            source: Some(CiEvidenceSource::ProviderObserved),
            artifacts: vec![CiProviderArtifact {
                id: "artifact-unit".into(),
                kind: "junit".into(),
                digest: digest("artifact"),
                check: Some("unit".into()),
                run_id: Some("99".into()),
                provider: Some("github_actions".into()),
                uri: Some("https://example.test/artifact".into()),
            }],
            logs: vec![CiProviderLog {
                id: "log-unit".into(),
                digest: digest("log"),
                check: Some("unit".into()),
                run_id: Some("99".into()),
                provider: Some("github_actions".into()),
                uri: Some("https://example.test/log".into()),
                truncated: false,
            }],
            attestations: vec![CiProviderAttestation {
                id: "attestation-unit".into(),
                subject: "artifact-unit".into(),
                issuer: "caller".into(),
                statement_digest: digest("statement"),
                method: "declared_provider_statement".into(),
            }],
        }
    }

    #[test]
    fn complete_provider_rows_are_digest_bound_but_not_authenticated() {
        let audit = audit_ci_provider_evidence(&request()).unwrap();
        assert!(audit.structurally_valid);
        assert!(audit.conformance_ready);
        assert_eq!(audit.artifact_count, 1);
        assert_eq!(audit.linked_log_count, 1);
        assert_eq!(audit.attestation_subject_count, 1);
        assert_eq!(audit.verification, "structural_only");
        assert!(audit
            .limitations
            .iter()
            .any(|limitation| limitation.contains("not cryptographically verified")));
    }

    #[test]
    fn row_binding_and_unknown_attestation_subjects_fail_closed() {
        let mut request = request();
        request.artifacts[0].run_id = Some("wrong-run".into());
        request.logs[0].check = Some("unknown-check".into());
        request.attestations[0].subject = "not-present".into();
        let audit = audit_ci_provider_evidence(&request).unwrap();
        assert!(!audit.structurally_valid);
        assert!(!audit.conformance_ready);
        for code in [
            "run_binding_mismatch",
            "unknown_check_binding",
            "attestation_subject_unknown",
        ] {
            assert!(audit.findings.iter().any(|finding| finding.code == code));
        }
    }

    #[test]
    fn invalid_row_digest_is_reported_without_being_rewritten() {
        let mut request = request();
        request.logs[0].digest = "not-a-digest".into();
        let audit = audit_ci_provider_evidence(&request).unwrap();
        assert!(!audit.structurally_valid);
        assert_eq!(audit.logs[0].digest, "not-a-digest");
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "log_digest_invalid"));
    }
}
