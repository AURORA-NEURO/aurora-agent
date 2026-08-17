//! Structural reconciliation for caller-supplied CI execution evidence.
//!
//! [`workbench::plan_ci`] deliberately stops at a deterministic, not-executed workflow artifact.
//! This module is the next boundary: it regenerates that plan, binds a later run report to the
//! plan digest and exact check set, and keeps provider/caller attestation separate from what the
//! repository can verify locally. It never contacts GitHub, trusts a URL, or turns a successful
//! caller claim into deployment or scientific validity.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::workbench::{plan_ci, CiRequest};

pub const CI_EXECUTION_EVIDENCE_SCHEMA: &str = "bioprism-devplat-ci-execution-evidence/0.1";
const MAX_RUN_TEXT: usize = 512;
const MAX_CHECKS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiEvidenceSource {
    CallerAttested,
    ProviderObserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiRunConclusion {
    Success,
    Failure,
    Cancelled,
    TimedOut,
    Neutral,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiCheckStatus {
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCheckEvidence {
    pub name: String,
    pub status: CiCheckStatus,
    pub result_digest: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub detail: Option<String>,
}

impl CiCheckEvidence {
    fn validate(&self) -> Result<(), CiEvidenceError> {
        bounded_text("evidence check name", &self.name)?;
        validate_digest("evidence result_digest", &self.result_digest)?;
        if let Some(detail) = &self.detail {
            bounded_text("evidence check detail", detail)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiRunEvidence {
    pub run_id: String,
    pub provider: String,
    pub source: CiEvidenceSource,
    pub plan_digest: String,
    pub conclusion: CiRunConclusion,
    pub checks: Vec<CiCheckEvidence>,
    #[serde(default)]
    pub environment_digest: Option<String>,
    #[serde(default)]
    pub run_url: Option<String>,
}

impl CiRunEvidence {
    fn validate(&self) -> Result<(), CiEvidenceError> {
        bounded_text("run_id", &self.run_id)?;
        bounded_text("provider", &self.provider)?;
        validate_digest("plan_digest", &self.plan_digest)?;
        if self.checks.is_empty() || self.checks.len() > MAX_CHECKS {
            return Err(CiEvidenceError::InvalidCheckCount(self.checks.len()));
        }
        if let Some(environment_digest) = &self.environment_digest {
            validate_digest("environment_digest", environment_digest)?;
        }
        if let Some(run_url) = &self.run_url {
            bounded_text("run_url", run_url)?;
            if run_url
                .chars()
                .any(|character| character == '\n' || character == '\r')
            {
                return Err(CiEvidenceError::ControlCharacter { field: "run_url" });
            }
        }
        for check in &self.checks {
            check.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiExecutionEvidenceRequest {
    pub ci: CiRequest,
    pub evidence: CiRunEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiEvidenceFinding {
    pub code: String,
    pub severity: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiExecutionEvidenceAudit {
    pub schema: String,
    pub workflow: String,
    pub plan_digest: String,
    pub evidence_digest: String,
    pub run_id: String,
    pub provider: String,
    pub source: CiEvidenceSource,
    pub conclusion: CiRunConclusion,
    pub expected_check_count: usize,
    pub observed_check_count: usize,
    pub passed_check_count: usize,
    pub failed_check_count: usize,
    pub skipped_check_count: usize,
    pub unknown_check_count: usize,
    pub required_missing: Vec<String>,
    pub required_failed: Vec<String>,
    pub optional_nonpassing: Vec<String>,
    pub complete: bool,
    pub structurally_valid: bool,
    pub release_candidate: bool,
    pub execution: String,
    pub verification: String,
    pub findings: Vec<CiEvidenceFinding>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CiEvidenceError {
    #[error("{field} must be a non-empty value no longer than {MAX_RUN_TEXT} bytes")]
    InvalidText { field: &'static str },
    #[error("{field} contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} is not a valid content digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("CI evidence check count {0} is outside 1..={MAX_CHECKS}")]
    InvalidCheckCount(usize),
    #[error("cannot canonicalize CI evidence: {0}")]
    Canonical(String),
    #[error("cannot generate canonical CI plan: {0}")]
    Plan(String),
}

fn bounded_text(field: &'static str, value: &str) -> Result<(), CiEvidenceError> {
    if value.trim().is_empty() || value.len() > MAX_RUN_TEXT {
        return Err(CiEvidenceError::InvalidText { field });
    }
    if value.chars().any(char::is_control) {
        return Err(CiEvidenceError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), CiEvidenceError> {
    ContentHash::parse(value.to_string())
        .map(|_| ())
        .map_err(|_| CiEvidenceError::InvalidDigest {
            field,
            value: value.to_string(),
        })
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

/// Reconcile a caller-supplied run against a freshly generated canonical CI plan.
pub fn audit_ci_execution_evidence(
    request: &CiExecutionEvidenceRequest,
) -> Result<CiExecutionEvidenceAudit, CiEvidenceError> {
    let plan = plan_ci(&request.ci).map_err(|error| CiEvidenceError::Plan(error.to_string()))?;
    request.evidence.validate()?;
    let evidence_value = serde_json::to_value((&plan.digest, &request.evidence))
        .map_err(|error| CiEvidenceError::Canonical(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&evidence_value)
        .map_err(|error| CiEvidenceError::Canonical(error.to_string()))?
        .to_string();

    let expected = request
        .ci
        .checks
        .iter()
        .map(|check| (check.name.as_str(), check.required))
        .collect::<BTreeMap<_, _>>();
    let mut findings = Vec::new();
    if request.evidence.plan_digest != plan.digest {
        finding(
            &mut findings,
            "plan_digest_mismatch",
            "blocking",
            "plan_digest",
            "run evidence is not bound to the canonical plan generated from the supplied CI request",
        );
    }

    let mut seen = BTreeSet::new();
    let mut observed = BTreeMap::new();
    for check in &request.evidence.checks {
        if !seen.insert(check.name.as_str()) {
            finding(
                &mut findings,
                "duplicate_check_evidence",
                "blocking",
                check.name.clone(),
                "each canonical CI check may have exactly one evidence row",
            );
            continue;
        }
        if !expected.contains_key(check.name.as_str()) {
            finding(
                &mut findings,
                "unknown_check_evidence",
                "blocking",
                check.name.clone(),
                "evidence names a check that is not present in the canonical CI plan",
            );
            continue;
        }
        observed.insert(check.name.clone(), check);
    }

    let mut required_missing = Vec::new();
    let mut required_failed = Vec::new();
    let mut optional_nonpassing = Vec::new();
    for (name, required) in &expected {
        let Some(check) = observed.get(*name) else {
            finding(
                &mut findings,
                "missing_check_evidence",
                "blocking",
                (*name).to_string(),
                "the canonical CI plan has no corresponding run evidence",
            );
            if *required {
                required_missing.push((*name).to_string());
            }
            continue;
        };
        if check.status != CiCheckStatus::Passed {
            let detail = "the check did not produce a passing status";
            finding(
                &mut findings,
                "check_not_passing",
                "blocking",
                (*name).to_string(),
                detail,
            );
            if *required {
                required_failed.push((*name).to_string());
            } else {
                optional_nonpassing.push((*name).to_string());
            }
        }
    }

    let passed_check_count = observed
        .values()
        .filter(|check| check.status == CiCheckStatus::Passed)
        .count();
    let failed_check_count = observed
        .values()
        .filter(|check| check.status == CiCheckStatus::Failed)
        .count();
    let skipped_check_count = observed
        .values()
        .filter(|check| check.status == CiCheckStatus::Skipped)
        .count();
    let unknown_check_count = observed
        .values()
        .filter(|check| {
            matches!(
                check.status,
                CiCheckStatus::Cancelled | CiCheckStatus::Unknown
            )
        })
        .count();
    let complete = required_missing.is_empty() && observed.len() == expected.len();
    let structurally_valid = findings.iter().all(|finding| {
        !matches!(
            finding.code.as_str(),
            "plan_digest_mismatch"
                | "duplicate_check_evidence"
                | "unknown_check_evidence"
                | "missing_check_evidence"
        )
    });
    let release_candidate = structurally_valid
        && complete
        && request.evidence.conclusion == CiRunConclusion::Success
        && passed_check_count == expected.len();
    required_missing.sort();
    required_failed.sort();
    optional_nonpassing.sort();
    findings.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then_with(|| left.code.cmp(&right.code))
    });

    Ok(CiExecutionEvidenceAudit {
        schema: CI_EXECUTION_EVIDENCE_SCHEMA.into(),
        workflow: request.ci.workflow.clone(),
        plan_digest: plan.digest,
        evidence_digest,
        run_id: request.evidence.run_id.clone(),
        provider: request.evidence.provider.clone(),
        source: request.evidence.source,
        conclusion: request.evidence.conclusion,
        expected_check_count: expected.len(),
        observed_check_count: observed.len(),
        passed_check_count,
        failed_check_count,
        skipped_check_count,
        unknown_check_count,
        required_missing,
        required_failed,
        optional_nonpassing,
        complete,
        structurally_valid,
        release_candidate,
        execution: "evidence_supplied_not_executed_here".into(),
        verification: "structural_only".into(),
        findings,
        guarantees: vec![
            "the plan digest is regenerated from the supplied CI request rather than trusted from the run payload".into(),
            "check names, requiredness, status, and per-check result digests remain separately inspectable".into(),
            "missing, unknown, duplicate, failed, skipped, cancelled, and unknown checks cannot be silently treated as passes".into(),
        ],
        limitations: vec![
            "the route does not contact GitHub, verify a provider signature, fetch logs, or execute a command".into(),
            "provider_observed and caller_attested are provenance labels, not cryptographic trust decisions".into(),
            "release_candidate is a structural handoff signal and is not deployment, security, scientific, clinical, or production approval".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::{CiCheck, CiRequest};

    fn digest(label: &str) -> String {
        ContentHash::of_bytes(label.as_bytes()).to_string()
    }

    fn request() -> CiExecutionEvidenceRequest {
        CiExecutionEvidenceRequest {
            ci: CiRequest {
                workflow: "contracts".into(),
                triggers: vec!["push".into()],
                rust_toolchain: "stable".into(),
                checks: vec![
                    CiCheck {
                        name: "unit".into(),
                        run: "cargo test -p core".into(),
                        working_directory: None,
                        required: true,
                    },
                    CiCheck {
                        name: "lint".into(),
                        run: "cargo clippy -p core".into(),
                        working_directory: None,
                        required: false,
                    },
                ],
                offline: true,
            },
            evidence: CiRunEvidence {
                run_id: "run-1".into(),
                provider: "github_actions".into(),
                source: CiEvidenceSource::ProviderObserved,
                plan_digest: digest("placeholder"),
                conclusion: CiRunConclusion::Success,
                checks: vec![
                    CiCheckEvidence {
                        name: "unit".into(),
                        status: CiCheckStatus::Passed,
                        result_digest: digest("unit"),
                        duration_ms: Some(100),
                        detail: None,
                    },
                    CiCheckEvidence {
                        name: "lint".into(),
                        status: CiCheckStatus::Passed,
                        result_digest: digest("lint"),
                        duration_ms: Some(50),
                        detail: None,
                    },
                ],
                environment_digest: Some(digest("environment")),
                run_url: Some("https://example.test/run/1".into()),
            },
        }
    }

    #[test]
    fn evidence_is_bound_to_the_regenerated_plan_and_passes_only_when_complete() {
        let mut request = request();
        let plan = plan_ci(&request.ci).unwrap();
        request.evidence.plan_digest = plan.digest;
        let audit = audit_ci_execution_evidence(&request).unwrap();
        assert!(audit.structurally_valid);
        assert!(audit.complete);
        assert!(audit.release_candidate);
        assert_eq!(audit.passed_check_count, 2);
        assert_eq!(audit.verification, "structural_only");

        request.evidence.checks[0].status = CiCheckStatus::Failed;
        let failed_run = audit_ci_execution_evidence(&request).unwrap();
        assert!(failed_run.structurally_valid);
        assert!(failed_run.complete);
        assert!(!failed_run.release_candidate);
        assert_eq!(failed_run.required_failed, vec!["unit"]);
    }

    #[test]
    fn missing_failed_and_unknown_evidence_remain_blocking_and_digest_bound() {
        let mut request = request();
        let plan = plan_ci(&request.ci).unwrap();
        request.evidence.plan_digest = plan.digest;
        request.evidence.checks = vec![CiCheckEvidence {
            name: "unit".into(),
            status: CiCheckStatus::Failed,
            result_digest: digest("failed-unit"),
            duration_ms: None,
            detail: Some("assertion failed".into()),
        }];
        let audit = audit_ci_execution_evidence(&request).unwrap();
        assert!(!audit.structurally_valid);
        assert!(!audit.release_candidate);
        assert_eq!(audit.required_failed, vec!["unit"]);
        assert_eq!(audit.required_missing, Vec::<String>::new());
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "missing_check_evidence" && finding.subject == "lint"));

        request.evidence.plan_digest = digest("wrong-plan");
        let mismatch = audit_ci_execution_evidence(&request).unwrap();
        assert!(mismatch
            .findings
            .iter()
            .any(|finding| finding.code == "plan_digest_mismatch"));
        assert_ne!(audit.evidence_digest, mismatch.evidence_digest);
    }
}
