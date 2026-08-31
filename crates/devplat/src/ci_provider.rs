//! Bounded normalization of external CI provider payloads.
//!
//! [`ci_evidence`] intentionally accepts a canonical `CiRunEvidence` object. Real providers do
//! not emit that exact shape, so this module provides the next integration boundary: it converts
//! a caller-supplied GitHub Actions or generic provider payload into the canonical evidence
//! envelope, derives result digests when a provider does not supply them, and preserves unknown
//! states for the later structural audit. It never contacts a provider, verifies signatures,
//! fetches logs, or treats a normalized payload as authenticated truth.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::ci_evidence::{
    CiCheckEvidence, CiCheckStatus, CiEvidenceSource, CiRunConclusion, CiRunEvidence,
};
use crate::workbench::{plan_ci, CiRequest};

pub const CI_PROVIDER_NORMALIZATION_SCHEMA: &str = "bioprism-devplat-ci-provider-normalization/0.1";
const MAX_TEXT: usize = 512;
const MAX_CHECKS: usize = 64;
const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiProviderNormalizationRequest {
    pub ci: CiRequest,
    pub provider: String,
    pub payload: Value,
    #[serde(default)]
    pub source: Option<CiEvidenceSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiProviderNormalization {
    pub schema: String,
    pub workflow: String,
    pub provider: String,
    pub source: CiEvidenceSource,
    pub payload_digest: String,
    pub run_id: String,
    pub conclusion: CiRunConclusion,
    pub check_count: usize,
    pub derived_result_digest_count: usize,
    pub warnings: Vec<String>,
    pub evidence: CiRunEvidence,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CiProviderNormalizationError {
    #[error("provider must be github_actions, gitlab_ci, or generic")]
    UnsupportedProvider,
    #[error("{field} must be a non-empty value no longer than {MAX_TEXT} bytes")]
    InvalidText { field: &'static str },
    #[error("{field} must be an object")]
    InvalidObject { field: &'static str },
    #[error("{field} must be an array")]
    InvalidArray { field: &'static str },
    #[error("CI provider payload has no checks")]
    EmptyChecks,
    #[error("CI provider payload has too many checks: {0}")]
    TooManyChecks(usize),
    #[error("{field} must be a string or number")]
    InvalidScalar { field: &'static str },
    #[error("{field} must be a valid content digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("CI provider payload is {actual} bytes, above the {maximum}-byte bound")]
    PayloadTooLarge { actual: usize, maximum: usize },
    #[error("cannot canonicalize CI provider payload: {0}")]
    Canonical(String),
    #[error("cannot generate canonical CI plan: {0}")]
    Plan(String),
}

fn bounded_text(field: &'static str, value: &str) -> Result<String, CiProviderNormalizationError> {
    if value.trim().is_empty()
        || value.len() > MAX_TEXT
        || value.chars().any(char::is_control)
        || value != value.trim()
    {
        return Err(CiProviderNormalizationError::InvalidText { field });
    }
    Ok(value.to_owned())
}

fn object<'a>(
    field: &'static str,
    value: &'a Value,
) -> Result<&'a Map<String, Value>, CiProviderNormalizationError> {
    value
        .as_object()
        .ok_or(CiProviderNormalizationError::InvalidObject { field })
}

fn scalar_text(
    field: &'static str,
    value: Option<&Value>,
) -> Result<String, CiProviderNormalizationError> {
    let value = value.ok_or(CiProviderNormalizationError::InvalidScalar { field })?;
    let text = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .ok_or(CiProviderNormalizationError::InvalidScalar { field })?;
    bounded_text(field, &text)
}

fn optional_text(
    field: &'static str,
    value: Option<&Value>,
) -> Result<Option<String>, CiProviderNormalizationError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => scalar_text(field, Some(value)).map(Some),
    }
}

fn parse_digest(field: &'static str, value: &str) -> Result<String, CiProviderNormalizationError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || ContentHash::parse(value.to_owned()).is_err()
    {
        return Err(CiProviderNormalizationError::InvalidDigest {
            field,
            value: value.to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn optional_digest(
    field: &'static str,
    value: Option<&Value>,
) -> Result<Option<String>, CiProviderNormalizationError> {
    optional_text(field, value)?
        .map(|value| parse_digest(field, &value))
        .transpose()
}

fn normalized_conclusion(value: Option<&Value>) -> CiRunConclusion {
    let Some(value) = value.and_then(Value::as_str) else {
        return CiRunConclusion::Unknown;
    };
    match value.to_ascii_lowercase().as_str() {
        "success" | "successful" | "passed" | "pass" => CiRunConclusion::Success,
        "failure" | "failed" | "error" | "action_required" => CiRunConclusion::Failure,
        "cancelled" | "canceled" => CiRunConclusion::Cancelled,
        "timed_out" | "timedout" | "timeout" => CiRunConclusion::TimedOut,
        "neutral" => CiRunConclusion::Neutral,
        _ => CiRunConclusion::Unknown,
    }
}

fn normalized_status(value: Option<&Value>) -> CiCheckStatus {
    let Some(value) = value.and_then(Value::as_str) else {
        return CiCheckStatus::Unknown;
    };
    match value.to_ascii_lowercase().as_str() {
        "success" | "successful" | "passed" | "pass" => CiCheckStatus::Passed,
        "failure" | "failed" | "error" | "action_required" => CiCheckStatus::Failed,
        "skipped" | "neutral" => CiCheckStatus::Skipped,
        "cancelled" | "canceled" => CiCheckStatus::Cancelled,
        _ => CiCheckStatus::Unknown,
    }
}

fn duration_ms(value: Option<&Value>) -> Option<u64> {
    value.and_then(Value::as_u64).or_else(|| {
        value
            .and_then(Value::as_i64)
            .and_then(|number| u64::try_from(number).ok())
    })
}

fn duration_seconds_ms(value: Option<&Value>) -> Option<u64> {
    value
        .and_then(Value::as_f64)
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u64)
}

fn check_payload<'a>(
    provider: &str,
    payload: &'a Map<String, Value>,
) -> Result<(&'a Map<String, Value>, &'a Vec<Value>), CiProviderNormalizationError> {
    let (run, checks) = if provider == "github_actions" {
        let run = match payload.get("run") {
            Some(run) => object("run", run)?,
            None => payload,
        };
        let checks = payload
            .get("jobs")
            .or_else(|| run.get("jobs"))
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "jobs" })?
            .as_array()
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "jobs" })?;
        (run, checks)
    } else if provider == "gitlab_ci" {
        let run = match payload.get("pipeline") {
            Some(run) => object("pipeline", run)?,
            None => payload,
        };
        let checks = payload
            .get("jobs")
            .or_else(|| run.get("jobs"))
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "jobs" })?
            .as_array()
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "jobs" })?;
        (run, checks)
    } else {
        let checks = payload
            .get("checks")
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "checks" })?
            .as_array()
            .ok_or(CiProviderNormalizationError::InvalidArray { field: "checks" })?;
        (payload, checks)
    };
    if checks.is_empty() {
        return Err(CiProviderNormalizationError::EmptyChecks);
    }
    if checks.len() > MAX_CHECKS {
        return Err(CiProviderNormalizationError::TooManyChecks(checks.len()));
    }
    Ok((run, checks))
}

fn canonical_check_values(checks: &[Value]) -> Result<Vec<Value>, CiProviderNormalizationError> {
    let mut keyed = checks
        .iter()
        .map(|check| {
            serde_json::to_string(check)
                .map(|key| (key, check))
                .map_err(|error| CiProviderNormalizationError::Canonical(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, check)| check.clone()).collect())
}

fn canonicalize_normalized_checks(
    checks: Vec<CiCheckEvidence>,
    request: &CiRequest,
) -> Result<Vec<CiCheckEvidence>, CiProviderNormalizationError> {
    let plan_order = request
        .checks
        .iter()
        .enumerate()
        .map(|(index, check)| (check.name.to_ascii_lowercase(), index))
        .collect::<BTreeMap<_, _>>();
    let mut keyed = checks
        .into_iter()
        .map(|check| {
            let name = check.name.to_ascii_lowercase();
            let plan_index = plan_order
                .get(&name)
                .copied()
                .unwrap_or(request.checks.len());
            let payload_key = serde_json::to_string(&check)
                .map_err(|error| CiProviderNormalizationError::Canonical(error.to_string()))?;
            Ok((plan_index, name, payload_key, check))
        })
        .collect::<Result<Vec<_>, CiProviderNormalizationError>>()?;
    keyed.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    Ok(keyed.into_iter().map(|(_, _, _, check)| check).collect())
}

fn normalize_check(
    provider: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<CiCheckEvidence, CiProviderNormalizationError> {
    let check = object("check", value)?;
    let name = scalar_text(
        "check.name",
        check
            .get("name")
            .or_else(|| check.get("job_name"))
            .or_else(|| check.get("id")),
    )?;
    let status = normalized_status(
        check
            .get("status")
            .or_else(|| check.get("conclusion"))
            .or_else(|| check.get("result")),
    );
    let result_digest = match check.get("result_digest") {
        Some(value) => parse_digest(
            "check.result_digest",
            &scalar_text("check.result_digest", Some(value))?,
        )?,
        None => {
            let digest = ContentHash::of_value(value)
                .map_err(|error| CiProviderNormalizationError::Canonical(error.to_string()))?
                .to_string();
            warnings.push(format!("derived_result_digest:{name}"));
            digest
        }
    };
    let detail = optional_text("check.detail", check.get("detail"))?;
    let duration_ms = duration_ms(check.get("duration_ms")).or_else(|| {
        (provider == "gitlab_ci")
            .then(|| duration_seconds_ms(check.get("duration")))
            .flatten()
    });
    if provider != "generic" && check.get("result_digest").is_none() {
        warnings.push(format!("provider_result_digest_absent:{name}"));
    }
    Ok(CiCheckEvidence {
        name,
        status,
        result_digest,
        duration_ms,
        detail,
    })
}

/// Normalize a bounded provider payload into the canonical CI evidence contract.
pub fn normalize_ci_provider_payload(
    request: &CiProviderNormalizationRequest,
) -> Result<CiProviderNormalization, CiProviderNormalizationError> {
    let provider = bounded_text("provider", request.provider.trim())?.to_ascii_lowercase();
    if provider != "github_actions" && provider != "gitlab_ci" && provider != "generic" {
        return Err(CiProviderNormalizationError::UnsupportedProvider);
    }
    let payload = object("payload", &request.payload)?;
    let payload_bytes = serde_json::to_vec(&request.payload)
        .map_err(|error| CiProviderNormalizationError::Canonical(error.to_string()))?;
    if payload_bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(CiProviderNormalizationError::PayloadTooLarge {
            actual: payload_bytes.len(),
            maximum: MAX_PAYLOAD_BYTES,
        });
    }
    let payload_digest = ContentHash::of_value(&request.payload)
        .map_err(|error| CiProviderNormalizationError::Canonical(error.to_string()))?
        .to_string();
    let plan = plan_ci(&request.ci)
        .map_err(|error| CiProviderNormalizationError::Plan(error.to_string()))?;
    let (run, checks) = check_payload(&provider, payload)?;
    let run_id = scalar_text(
        "run_id",
        run.get("run_id")
            .or_else(|| run.get("id"))
            .or_else(|| run.get("workflow_run_id")),
    )?;
    let conclusion = normalized_conclusion(run.get("conclusion").or_else(|| run.get("result")));
    let source = request.source.unwrap_or(if provider != "generic" {
        CiEvidenceSource::ProviderObserved
    } else {
        CiEvidenceSource::CallerAttested
    });
    let mut warnings = Vec::new();
    if conclusion == CiRunConclusion::Unknown {
        warnings.push("run_conclusion_unknown_or_missing".into());
    }
    let checks = canonical_check_values(checks)?;
    let checks = checks
        .iter()
        .map(|check| normalize_check(&provider, check, &mut warnings))
        .collect::<Result<Vec<_>, _>>()?;
    let checks = canonicalize_normalized_checks(checks, &request.ci)?;
    let environment_digest = optional_digest("environment_digest", run.get("environment_digest"))?;
    let run_url = optional_text(
        "run_url",
        run.get("run_url")
            .or_else(|| run.get("html_url"))
            .or_else(|| run.get("web_url"))
            .or_else(|| run.get("url")),
    )?;
    let derived_result_digest_count = warnings
        .iter()
        .filter(|warning| warning.starts_with("derived_result_digest:"))
        .count();
    let evidence = CiRunEvidence {
        run_id: run_id.clone(),
        provider: provider.clone(),
        source,
        plan_digest: plan.digest,
        conclusion,
        checks,
        environment_digest,
        run_url,
    };
    Ok(CiProviderNormalization {
        schema: CI_PROVIDER_NORMALIZATION_SCHEMA.into(),
        workflow: "ci_provider_normalize".into(),
        provider,
        source,
        payload_digest,
        run_id,
        conclusion,
        check_count: evidence.checks.len(),
        derived_result_digest_count,
        warnings,
        evidence,
        guarantees: vec![
            "the provider payload is converted into the canonical CiRunEvidence shape before auditing".into(),
            "missing provider result digests are deterministically derived from each supplied check object".into(),
            "unknown, failed, skipped, cancelled, duplicate, and extra checks remain visible to ci_execution_evidence_audit".into(),
        ],
        limitations: vec![
            "the route accepts caller-supplied payloads and does not contact or authenticate a provider".into(),
            "derived digests identify the supplied payload object; they are not log-content or signature proofs".into(),
            "normalization is not execution, log retrieval, deployment approval, or release authority".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::{CiCheck, CiRequest};

    fn request(provider: &str, payload: Value) -> CiProviderNormalizationRequest {
        CiProviderNormalizationRequest {
            ci: CiRequest {
                workflow: "provider-normalization".into(),
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
            provider: provider.into(),
            payload,
            source: None,
        }
    }

    #[test]
    fn github_actions_payload_becomes_audit_ready_evidence_envelope() {
        let normalized = normalize_ci_provider_payload(&request(
            "github_actions",
            serde_json::json!({
                "run": {"id": 42, "conclusion": "success", "html_url": "https://example.test/run/42"},
                "jobs": [
                    {"name": "unit", "conclusion": "success"},
                    {"name": "lint", "conclusion": "success"}
                ]
            }),
        ))
        .unwrap();
        assert_eq!(normalized.evidence.run_id, "42");
        assert_eq!(
            normalized.evidence.source,
            CiEvidenceSource::ProviderObserved
        );
        assert_eq!(normalized.evidence.conclusion, CiRunConclusion::Success);
        assert_eq!(normalized.evidence.checks[0].status, CiCheckStatus::Passed);
        assert_eq!(normalized.derived_result_digest_count, 2);
        assert_eq!(normalized.warnings.len(), 4);
        assert_eq!(normalized.evidence.plan_digest.len(), 64);
    }

    #[test]
    fn provider_check_order_is_canonical_but_raw_payload_digest_remains_bound() {
        let first = normalize_ci_provider_payload(&request(
            "github_actions",
            serde_json::json!({
                "run": {"id": 43, "conclusion": "success"},
                "jobs": [
                    {"name": "unit", "conclusion": "success"},
                    {"name": "lint", "conclusion": "success"}
                ]
            }),
        ))
        .unwrap();
        let second = normalize_ci_provider_payload(&request(
            "github_actions",
            serde_json::json!({
                "run": {"id": 43, "conclusion": "success"},
                "jobs": [
                    {"name": "lint", "conclusion": "success"},
                    {"name": "unit", "conclusion": "success"}
                ]
            }),
        ))
        .unwrap();
        assert_eq!(first.evidence, second.evidence);
        assert_ne!(first.payload_digest, second.payload_digest);
    }

    #[test]
    fn generic_payload_preserves_unknown_provider_states_and_explicit_digests() {
        let digest = ContentHash::of_bytes(b"provided").to_string();
        let normalized = normalize_ci_provider_payload(&request(
            "generic",
            serde_json::json!({
                "run_id": "generic-1",
                "conclusion": "in_progress",
                "checks": [
                    {"name": "unit", "status": "in_progress", "result_digest": digest},
                    {"name": "lint", "status": "skipped"}
                ]
            }),
        ))
        .unwrap();
        assert_eq!(normalized.evidence.source, CiEvidenceSource::CallerAttested);
        assert_eq!(normalized.evidence.conclusion, CiRunConclusion::Unknown);
        assert_eq!(normalized.evidence.checks[0].status, CiCheckStatus::Unknown);
        assert_eq!(normalized.derived_result_digest_count, 1);
        assert!(normalized
            .warnings
            .contains(&"run_conclusion_unknown_or_missing".into()));
    }

    #[test]
    fn gitlab_pipeline_payload_maps_duration_and_pipeline_metadata() {
        let normalized = normalize_ci_provider_payload(&request(
            "gitlab_ci",
            serde_json::json!({
                "pipeline": {"id": 77, "status": "success", "web_url": "https://gitlab.example/pipelines/77"},
                "jobs": [
                    {"name": "unit", "status": "success", "duration": 1.25},
                    {"name": "lint", "status": "skipped", "duration_ms": 10}
                ]
            }),
        ))
        .unwrap();
        assert_eq!(normalized.evidence.run_id, "77");
        assert_eq!(
            normalized.evidence.source,
            CiEvidenceSource::ProviderObserved
        );
        assert_eq!(
            normalized.evidence.run_url.as_deref(),
            Some("https://gitlab.example/pipelines/77")
        );
        assert_eq!(normalized.evidence.checks[0].duration_ms, Some(1250));
        assert_eq!(normalized.evidence.checks[1].duration_ms, Some(10));
        assert_eq!(normalized.derived_result_digest_count, 2);
    }

    #[test]
    fn invalid_provider_digest_is_refused_instead_of_silently_coerced() {
        let error = normalize_ci_provider_payload(&request(
            "generic",
            serde_json::json!({
                "run_id": "generic-2",
                "conclusion": "success",
                "checks": [
                    {"name": "unit", "status": "success", "result_digest": "not-a-digest"}
                ]
            }),
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            CiProviderNormalizationError::InvalidDigest { .. }
        ));

        let error = normalize_ci_provider_payload(&request(
            "generic",
            serde_json::json!({
                "run_id": "generic-3",
                "conclusion": "success",
                "checks": [
                    {"name": "unit", "status": "success", "result_digest": "A".repeat(64)}
                ]
            }),
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            CiProviderNormalizationError::InvalidDigest { .. }
        ));
    }

    #[test]
    fn provider_normalization_rejects_metadata_aliases_and_oversized_payloads() {
        let error = normalize_ci_provider_payload(&request(
            "generic",
            serde_json::json!({
                "run_id": " generic-run ",
                "conclusion": "success",
                "checks": [{"name": "unit", "status": "success"}]
            }),
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            CiProviderNormalizationError::InvalidText { field: "run_id" }
        ));

        let oversized = serde_json::json!({
            "run_id": "generic-large",
            "conclusion": "success",
            "checks": [{"name": "unit", "status": "success"}],
            "payload": "x".repeat(MAX_PAYLOAD_BYTES + 1)
        });
        let error = normalize_ci_provider_payload(&request("generic", oversized)).unwrap_err();
        assert!(matches!(
            error,
            CiProviderNormalizationError::PayloadTooLarge { .. }
        ));
    }
}
