//! Prospective assay provenance integrity auditing for preclinical glioma workflows.
//!
//! This feature is an admission gate, not a receipt store.  It evaluates whether an instrument
//! run has a continuous, policy-allowed chain from sample lineage through protocol, calibrated
//! device, operator authority, clock observation, and produced artifact.  The result is a
//! deterministic partition into verified, warning, blocked, and unresolved runs.  Only verified
//! runs may enter downstream multimodal analysis or a verified research-object release.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaAssayProvenanceAudit1@1";
pub const MAX_RUNS: usize = 512;
pub const MAX_REASONS: usize = 32;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayLifecycleStatus {
    Completed,
    Partial,
    Failed,
    Aborted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayProvenanceRun {
    pub run_id: String,
    pub sample_lineage_digest: ContentHash,
    pub expected_sample_scope_digest: ContentHash,
    pub observed_sample_scope_digest: ContentHash,
    pub approved_protocol_digest: ContentHash,
    pub observed_protocol_digest: ContentHash,
    pub device_id: String,
    pub calibration_valid_until_tick: u64,
    pub operator_authority_digest: Option<ContentHash>,
    pub observed_clock_tick: u64,
    pub artifact_manifest_digest: ContentHash,
    pub artifact_predecessor_digest: Option<ContentHash>,
    pub artifact_sequence: u64,
    pub lifecycle_status: AssayLifecycleStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayProvenanceAuditRequest {
    pub runs: Vec<AssayProvenanceRun>,
    pub current_tick: u64,
    pub max_clock_skew_ticks: u64,
    pub require_operator_authority: bool,
    pub require_artifact_continuity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayProvenanceDisposition {
    Verified,
    Warning,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayProvenanceAuditResult {
    pub run_id: String,
    pub disposition: AssayProvenanceDisposition,
    pub reason_order: Vec<String>,
    pub admit_to_analysis: bool,
    pub admit_to_verified_release: bool,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayProvenanceIntegrityAudit {
    pub feature_id: String,
    pub output_schema: String,
    pub current_tick: u64,
    pub results: Vec<AssayProvenanceAuditResult>,
    pub verified_run_order: Vec<String>,
    pub warning_run_order: Vec<String>,
    pub blocked_run_order: Vec<String>,
    pub unresolved_run_order: Vec<String>,
    pub analysis_admission_order: Vec<String>,
    pub verified_release_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssayProvenanceAuditError {
    #[error("assay provenance audit request is invalid: {0}")]
    InvalidRequest(String),
    #[error("assay provenance audit output is invalid: {0}")]
    InvalidOutput(String),
    #[error("assay provenance audit digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn valid_optional_hash(value: &Option<ContentHash>) -> bool {
    value.as_ref().is_none_or(valid_hash)
}

fn body(run: &AssayProvenanceRun) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.run_id,
        "sample_lineage_digest": run.sample_lineage_digest,
        "expected_sample_scope_digest": run.expected_sample_scope_digest,
        "observed_sample_scope_digest": run.observed_sample_scope_digest,
        "approved_protocol_digest": run.approved_protocol_digest,
        "observed_protocol_digest": run.observed_protocol_digest,
        "device_id": run.device_id,
        "calibration_valid_until_tick": run.calibration_valid_until_tick,
        "operator_authority_digest": run.operator_authority_digest,
        "observed_clock_tick": run.observed_clock_tick,
        "artifact_manifest_digest": run.artifact_manifest_digest,
        "artifact_predecessor_digest": run.artifact_predecessor_digest,
        "artifact_sequence": run.artifact_sequence,
        "lifecycle_status": run.lifecycle_status,
    })
}

fn result_body(result: &AssayProvenanceAuditResult) -> serde_json::Value {
    serde_json::json!({
        "run_id": result.run_id,
        "disposition": result.disposition,
        "reason_order": result.reason_order,
        "admit_to_analysis": result.admit_to_analysis,
        "admit_to_verified_release": result.admit_to_verified_release,
    })
}

fn audit_body(audit: &AssayProvenanceIntegrityAudit) -> serde_json::Value {
    serde_json::json!({
        "feature_id": audit.feature_id,
        "output_schema": audit.output_schema,
        "current_tick": audit.current_tick,
        "results": audit.results,
        "verified_run_order": audit.verified_run_order,
        "warning_run_order": audit.warning_run_order,
        "blocked_run_order": audit.blocked_run_order,
        "unresolved_run_order": audit.unresolved_run_order,
        "analysis_admission_order": audit.analysis_admission_order,
        "verified_release_order": audit.verified_release_order,
        "negative_evidence_order": audit.negative_evidence_order,
    })
}

fn validate_request(
    request: &AssayProvenanceAuditRequest,
) -> Result<(), AssayProvenanceAuditError> {
    if request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.current_tick == 0
        || request.max_clock_skew_ticks == 0
    {
        return Err(AssayProvenanceAuditError::InvalidRequest(
            "a bounded run set, current tick, and positive clock-skew bound are required".into(),
        ));
    }
    let run_ids = request
        .runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    if run_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || run_ids.iter().collect::<BTreeSet<_>>().len() != run_ids.len()
    {
        return Err(AssayProvenanceAuditError::InvalidRequest(
            "runs must be unique and canonically ordered by run_id".into(),
        ));
    }
    for run in &request.runs {
        if !safe_text(&run.run_id)
            || !safe_text(&run.device_id)
            || run.calibration_valid_until_tick == 0
            || run.observed_clock_tick == 0
            || !valid_hash(&run.sample_lineage_digest)
            || !valid_hash(&run.expected_sample_scope_digest)
            || !valid_hash(&run.observed_sample_scope_digest)
            || !valid_hash(&run.approved_protocol_digest)
            || !valid_hash(&run.observed_protocol_digest)
            || !valid_hash(&run.artifact_manifest_digest)
            || !valid_optional_hash(&run.operator_authority_digest)
            || !valid_optional_hash(&run.artifact_predecessor_digest)
        {
            return Err(AssayProvenanceAuditError::InvalidRequest(format!(
                "run {} has invalid identity, clock, calibration, or digest bounds",
                run.run_id
            )));
        }
        let expected = ContentHash::of_value(&body(run))
            .map_err(|error| AssayProvenanceAuditError::Digest(error.to_string()))?;
        if expected.as_str().len() != 64 {
            return Err(AssayProvenanceAuditError::Digest(
                "canonical assay provenance body did not produce a content hash".into(),
            ));
        }
    }
    Ok(())
}

impl AssayProvenanceIntegrityAudit {
    pub fn validate(&self) -> Result<(), AssayProvenanceAuditError> {
        if self.feature_id != FEATURE_ID || self.output_schema != OUTPUT_SCHEMA {
            return Err(AssayProvenanceAuditError::InvalidOutput(
                "feature or output schema identity is incorrect".into(),
            ));
        }
        if self.results.is_empty() || self.results.len() > MAX_RUNS || self.current_tick == 0 {
            return Err(AssayProvenanceAuditError::InvalidOutput(
                "output run bounds are invalid".into(),
            ));
        }
        if self
            .results
            .windows(2)
            .any(|pair| pair[0].run_id >= pair[1].run_id)
            || self
                .results
                .iter()
                .any(|result| !safe_text(&result.run_id) || result.reason_order.len() > MAX_REASONS)
        {
            return Err(AssayProvenanceAuditError::InvalidOutput(
                "results must be canonically ordered with bounded reasons".into(),
            ));
        }
        for result in &self.results {
            let expected = ContentHash::of_value(&result_body(result))
                .map_err(|error| AssayProvenanceAuditError::Digest(error.to_string()))?;
            if expected != result.result_digest {
                return Err(AssayProvenanceAuditError::InvalidOutput(format!(
                    "result digest is not bound for {}",
                    result.run_id
                )));
            }
            if result.admit_to_analysis
                != (result.disposition == AssayProvenanceDisposition::Verified)
                || result.admit_to_verified_release != result.admit_to_analysis
            {
                return Err(AssayProvenanceAuditError::InvalidOutput(format!(
                    "admission flags are inconsistent for {}",
                    result.run_id
                )));
            }
        }
        let digest = ContentHash::of_value(&audit_body(self))
            .map_err(|error| AssayProvenanceAuditError::Digest(error.to_string()))?;
        if digest != self.digest {
            return Err(AssayProvenanceAuditError::InvalidOutput(
                "audit digest is not bound to output".into(),
            ));
        }
        Ok(())
    }
}

fn classify_run(
    run: &AssayProvenanceRun,
    request: &AssayProvenanceAuditRequest,
) -> (AssayProvenanceDisposition, Vec<String>) {
    let mut blocked = Vec::new();
    let mut unresolved = Vec::new();
    let mut warning = Vec::new();

    if run
        .sample_lineage_digest
        .as_str()
        .chars()
        .all(|character| character == '0')
    {
        unresolved.push("sample-lineage-digest-is-empty".into());
    }
    if run.expected_sample_scope_digest != run.observed_sample_scope_digest {
        blocked.push("sample-scope-does-not-match-approved-scope".into());
    }
    if run.approved_protocol_digest != run.observed_protocol_digest {
        blocked.push("observed-protocol-does-not-match-approved-protocol".into());
    }
    if request.current_tick > run.calibration_valid_until_tick {
        blocked.push("device-calibration-expired-before-audit".into());
    }
    if request.require_operator_authority && run.operator_authority_digest.is_none() {
        blocked.push("operator-authority-is-missing".into());
    }
    if request.require_artifact_continuity
        && run.artifact_sequence > 0
        && run.artifact_predecessor_digest.is_none()
    {
        blocked.push("artifact-predecessor-is-missing".into());
    }
    let skew = request.current_tick.abs_diff(run.observed_clock_tick);
    if skew > request.max_clock_skew_ticks {
        blocked.push("observed-clock-exceeds-skew-bound".into());
    } else if skew > request.max_clock_skew_ticks / 2 {
        warning.push("observed-clock-is-near-skew-bound".into());
    }
    match run.lifecycle_status {
        AssayLifecycleStatus::Completed => {}
        AssayLifecycleStatus::Partial => warning.push("run-is-partial".into()),
        AssayLifecycleStatus::Failed | AssayLifecycleStatus::Aborted => {
            blocked.push("run-did-not-complete".into())
        }
        AssayLifecycleStatus::Unknown => unresolved.push("run-lifecycle-is-unknown".into()),
    }
    if blocked.is_empty() && unresolved.is_empty() {
        if warning.is_empty() {
            (AssayProvenanceDisposition::Verified, warning)
        } else {
            (AssayProvenanceDisposition::Warning, warning)
        }
    } else if !blocked.is_empty() {
        blocked.extend(unresolved);
        blocked.extend(warning);
        (AssayProvenanceDisposition::Blocked, blocked)
    } else {
        unresolved.extend(warning);
        (AssayProvenanceDisposition::Unresolved, unresolved)
    }
}

pub fn audit_glioma_assay_provenance(
    request: &AssayProvenanceAuditRequest,
) -> Result<AssayProvenanceIntegrityAudit, AssayProvenanceAuditError> {
    validate_request(request)?;
    let mut results = Vec::with_capacity(request.runs.len());
    for run in &request.runs {
        let (disposition, reason_order) = classify_run(run, request);
        let admit_to_analysis = disposition == AssayProvenanceDisposition::Verified;
        let mut result = AssayProvenanceAuditResult {
            run_id: run.run_id.clone(),
            disposition,
            reason_order,
            admit_to_analysis,
            admit_to_verified_release: admit_to_analysis,
            result_digest: ContentHash::of_bytes(b"unsealed-glioma-assay-provenance-result"),
        };
        result.result_digest = ContentHash::of_value(&result_body(&result))
            .map_err(|error| AssayProvenanceAuditError::Digest(error.to_string()))?;
        results.push(result);
    }
    let verified_run_order = results
        .iter()
        .filter(|result| result.disposition == AssayProvenanceDisposition::Verified)
        .map(|result| result.run_id.clone())
        .collect::<Vec<_>>();
    let warning_run_order = results
        .iter()
        .filter(|result| result.disposition == AssayProvenanceDisposition::Warning)
        .map(|result| result.run_id.clone())
        .collect::<Vec<_>>();
    let blocked_run_order = results
        .iter()
        .filter(|result| result.disposition == AssayProvenanceDisposition::Blocked)
        .map(|result| result.run_id.clone())
        .collect::<Vec<_>>();
    let unresolved_run_order = results
        .iter()
        .filter(|result| result.disposition == AssayProvenanceDisposition::Unresolved)
        .map(|result| result.run_id.clone())
        .collect::<Vec<_>>();
    let negative_evidence_order = results
        .iter()
        .filter(|result| result.disposition != AssayProvenanceDisposition::Verified)
        .flat_map(|result| {
            result
                .reason_order
                .iter()
                .map(move |reason| format!("{}:{}", result.run_id, reason))
        })
        .collect::<Vec<_>>();
    let audit = AssayProvenanceIntegrityAudit {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        current_tick: request.current_tick,
        analysis_admission_order: verified_run_order.clone(),
        verified_release_order: verified_run_order.clone(),
        verified_run_order,
        warning_run_order,
        blocked_run_order,
        unresolved_run_order,
        negative_evidence_order,
        results,
        digest: ContentHash::of_bytes(b"unsealed-glioma-assay-provenance-audit"),
    };
    let mut sealed = audit;
    sealed.digest = ContentHash::of_value(&audit_body(&sealed))
        .map_err(|error| AssayProvenanceAuditError::Digest(error.to_string()))?;
    sealed.validate()?;
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn run(id: &str) -> AssayProvenanceRun {
        AssayProvenanceRun {
            run_id: id.into(),
            sample_lineage_digest: hash("lineage"),
            expected_sample_scope_digest: hash("scope"),
            observed_sample_scope_digest: hash("scope"),
            approved_protocol_digest: hash("protocol"),
            observed_protocol_digest: hash("protocol"),
            device_id: "microscope-a".into(),
            calibration_valid_until_tick: 200,
            operator_authority_digest: Some(hash("operator")),
            observed_clock_tick: 100,
            artifact_manifest_digest: hash("artifact"),
            artifact_predecessor_digest: None,
            artifact_sequence: 0,
            lifecycle_status: AssayLifecycleStatus::Completed,
        }
    }

    fn request(runs: Vec<AssayProvenanceRun>) -> AssayProvenanceAuditRequest {
        AssayProvenanceAuditRequest {
            runs,
            current_tick: 100,
            max_clock_skew_ticks: 10,
            require_operator_authority: true,
            require_artifact_continuity: true,
        }
    }

    #[test]
    fn verified_runs_are_admitted_to_analysis_and_release() {
        let output = audit_glioma_assay_provenance(&request(vec![run("run-a")])).unwrap();
        assert_eq!(output.verified_run_order, vec!["run-a"]);
        assert_eq!(output.analysis_admission_order, vec!["run-a"]);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn scope_and_protocol_tampering_blocks_a_run() {
        let mut item = run("run-a");
        item.observed_sample_scope_digest = hash("other-scope");
        item.observed_protocol_digest = hash("other-protocol");
        let output = audit_glioma_assay_provenance(&request(vec![item])).unwrap();
        assert_eq!(output.blocked_run_order, vec!["run-a"]);
        assert!(output.analysis_admission_order.is_empty());
        assert!(output.results[0]
            .reason_order
            .iter()
            .any(|reason| reason.contains("scope")));
    }

    #[test]
    fn expired_calibration_and_clock_skew_block_a_run() {
        let mut item = run("run-a");
        item.calibration_valid_until_tick = 99;
        item.observed_clock_tick = 40;
        let output = audit_glioma_assay_provenance(&request(vec![item])).unwrap();
        assert_eq!(output.blocked_run_order, vec!["run-a"]);
        assert!(output.results[0]
            .reason_order
            .iter()
            .any(|reason| reason.contains("calibration")));
        assert!(output.results[0]
            .reason_order
            .iter()
            .any(|reason| reason.contains("clock")));
    }

    #[test]
    fn unknown_lifecycle_is_unresolved_and_never_admitted() {
        let mut item = run("run-a");
        item.lifecycle_status = AssayLifecycleStatus::Unknown;
        let output = audit_glioma_assay_provenance(&request(vec![item])).unwrap();
        assert_eq!(output.unresolved_run_order, vec!["run-a"]);
        assert!(!output.results[0].admit_to_analysis);
    }

    #[test]
    fn missing_continuity_and_authority_are_blocked() {
        let mut item = run("run-a");
        item.artifact_sequence = 1;
        item.operator_authority_digest = None;
        let output = audit_glioma_assay_provenance(&request(vec![item])).unwrap();
        assert_eq!(output.blocked_run_order, vec!["run-a"]);
        assert!(output.results[0]
            .reason_order
            .iter()
            .any(|reason| reason.contains("predecessor")));
        assert!(output.results[0]
            .reason_order
            .iter()
            .any(|reason| reason.contains("authority")));
    }
}
