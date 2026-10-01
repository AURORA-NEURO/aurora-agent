//! Bounded release-artifact integrity scanning for preclinical glioma research objects.
//!
//! A local archive worker supplies observed digest/size and format metadata for each candidate;
//! this reducer turns those observations into a deterministic verified/quarantined partition. It
//! catches digest mismatch, truncation, unsupported formats, malformed metadata, executable
//! payloads, path escapes, missing required members, and budget violations without ever treating a
//! declaration as verified or moving the underlying bytes.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaArtifactIntegrityReport1@1";
pub const MAX_ARTIFACTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityFindingKind {
    DigestMismatch,
    Truncated,
    UnsupportedFormat,
    MalformedMetadata,
    ExecutablePayload,
    PathEscape,
    MissingRequiredArtifact,
    Oversized,
    NotRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIntegrityCandidate {
    pub artifact_id: String,
    pub relative_path: String,
    pub declared_content_hash: ContentHash,
    pub observed_content_hash: ContentHash,
    pub declared_bytes: u64,
    pub observed_bytes: u64,
    pub content_type: String,
    pub metadata_valid: bool,
    pub executable_payload: bool,
    pub link_target: Option<String>,
    pub requested_export: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIntegrityRequest {
    pub candidate_manifest_digest: ContentHash,
    pub release_root: String,
    pub required_artifact_order: Vec<String>,
    pub allowed_content_types: Vec<String>,
    pub candidates: Vec<ArtifactIntegrityCandidate>,
    pub max_total_bytes: u64,
    pub max_memory_bytes: u64,
    pub stream_chunk_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityFinding {
    pub artifact_id: String,
    pub kind: IntegrityFindingKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityDisposition {
    Clear,
    Warnings,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIntegrityReport {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_manifest_digest: ContentHash,
    pub release_root: String,
    pub verified_order: Vec<String>,
    pub quarantined_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub findings: Vec<IntegrityFinding>,
    pub scanned_bytes: u64,
    pub stream_chunk_bytes: u64,
    pub disposition: IntegrityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactIntegrityError {
    #[error("artifact integrity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("artifact integrity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("artifact integrity digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn valid_path(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 1024
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !value.contains('\\')
}

fn digest_input(report: &ArtifactIntegrityReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "candidate_manifest_digest": report.candidate_manifest_digest,
        "release_root": report.release_root,
        "verified_order": report.verified_order,
        "quarantined_order": report.quarantined_order,
        "warning_order": report.warning_order,
        "blocking_order": report.blocking_order,
        "findings": report.findings,
        "scanned_bytes": report.scanned_bytes,
        "stream_chunk_bytes": report.stream_chunk_bytes,
        "disposition": report.disposition,
    })
}

impl ArtifactIntegrityReport {
    pub fn validate(&self) -> Result<(), ArtifactIntegrityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.candidate_manifest_digest.as_str().len() != 64
            || self.release_root.trim().is_empty()
            || !canonical(&self.verified_order)
            || !canonical(&self.quarantined_order)
            || !canonical(&self.warning_order)
            || !canonical(&self.blocking_order)
            || self.stream_chunk_bytes == 0
            || self
                .verified_order
                .iter()
                .chain(self.quarantined_order.iter())
                .any(|id| !valid_identifier(id))
        {
            return Err(ArtifactIntegrityError::InvalidOutput(
                "identity, digest, ordering, or stream invariants are invalid".into(),
            ));
        }
        let ids = self
            .verified_order
            .iter()
            .chain(self.quarantined_order.iter())
            .collect::<BTreeSet<_>>();
        if ids.len() != self.verified_order.len() + self.quarantined_order.len()
            || self.findings.iter().any(|finding| {
                !ids.contains(&finding.artifact_id)
                    || !valid_identifier(&finding.artifact_id)
                    || finding.detail.trim().is_empty()
            })
        {
            return Err(ArtifactIntegrityError::InvalidOutput(
                "artifact partitions and findings are not closed".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ArtifactIntegrityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ArtifactIntegrityError::InvalidOutput(
                "integrity report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ArtifactIntegrityRequest) -> Result<(), ArtifactIntegrityError> {
    if request.candidate_manifest_digest.as_str().len() != 64
        || !valid_path(&request.release_root)
        || request
            .required_artifact_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.allowed_content_types.is_empty()
        || request
            .allowed_content_types
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_ARTIFACTS
        || request.max_total_bytes == 0
        || request.max_memory_bytes == 0
        || request.stream_chunk_bytes == 0
        || request.stream_chunk_bytes > request.max_memory_bytes
    {
        return Err(ArtifactIntegrityError::InvalidRequest(
            "manifest/root, canonical policy lists, bounded candidates, and stream limits are required".into(),
        ));
    }
    let allowed = request
        .allowed_content_types
        .iter()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_artifact_order.len()
        || request
            .required_artifact_order
            .iter()
            .any(|id| !valid_identifier(id))
        || allowed.len() != request.allowed_content_types.len()
    {
        return Err(ArtifactIntegrityError::InvalidRequest(
            "required artifact and format policy lists must be unique and valid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !valid_identifier(&candidate.artifact_id)
            || !ids.insert(candidate.artifact_id.clone())
            || !valid_path(&candidate.relative_path)
            || candidate.declared_content_hash.as_str().len() != 64
            || candidate.observed_content_hash.as_str().len() != 64
            || candidate.content_type.trim().is_empty()
            || candidate.declared_bytes > request.max_total_bytes
            || candidate.observed_bytes > request.max_total_bytes
        {
            return Err(ArtifactIntegrityError::InvalidRequest(
                "candidate identity, safe relative path, digests, format, and size bounds are required".into(),
            ));
        }
        if let Some(target) = &candidate.link_target {
            if !valid_path(target) {
                return Err(ArtifactIntegrityError::InvalidRequest(
                    "link targets must remain relative to the release root".into(),
                ));
            }
        }
    }
    Ok(())
}

/// Reduce local scanner observations into a fail-closed artifact integrity report.
pub fn scan_glioma_artifact_integrity(
    request: &ArtifactIntegrityRequest,
) -> Result<ArtifactIntegrityReport, ArtifactIntegrityError> {
    validate_request(request)?;
    let allowed = request
        .allowed_content_types
        .iter()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates = request.candidates.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let mut findings = Vec::new();
    let mut verified = BTreeSet::new();
    let mut quarantined = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    let mut blocking = BTreeSet::new();
    let mut seen_required = BTreeSet::new();
    let mut scanned_bytes = 0u64;
    for candidate in candidates {
        scanned_bytes = scanned_bytes.saturating_add(candidate.observed_bytes);
        let mut candidate_blocked = false;
        if required.contains(&candidate.artifact_id) {
            seen_required.insert(candidate.artifact_id.clone());
        }
        if candidate.declared_content_hash != candidate.observed_content_hash {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::DigestMismatch,
                detail: "observed content hash differs from the manifest declaration".into(),
            });
        }
        if candidate.observed_bytes < candidate.declared_bytes {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::Truncated,
                detail: "observed byte length is smaller than the declared length".into(),
            });
        }
        if candidate.observed_bytes > candidate.declared_bytes {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::Oversized,
                detail: "observed byte length exceeds the declared length".into(),
            });
        }
        if !allowed.contains(&candidate.content_type) {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::UnsupportedFormat,
                detail: format!(
                    "content type {} is outside the pinned allow-list",
                    candidate.content_type
                ),
            });
        }
        if !candidate.metadata_valid {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::MalformedMetadata,
                detail: "declared metadata failed local schema validation".into(),
            });
        }
        if candidate.executable_payload {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::ExecutablePayload,
                detail: "executable payloads are not allowed in a research-object release".into(),
            });
        }
        if candidate.link_target.is_some() {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::PathEscape,
                detail: "linked release members cannot be represented as verified archive content"
                    .into(),
            });
        }
        if !candidate.requested_export {
            warnings.insert(candidate.artifact_id.clone());
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::NotRequested,
                detail: "candidate is present but not requested for export".into(),
            });
        }
        if scanned_bytes > request.max_total_bytes {
            candidate_blocked = true;
            findings.push(IntegrityFinding {
                artifact_id: candidate.artifact_id.clone(),
                kind: IntegrityFindingKind::Oversized,
                detail: "aggregate scan exceeds the declared byte budget".into(),
            });
        }
        if candidate_blocked || !candidate.requested_export {
            quarantined.insert(candidate.artifact_id.clone());
            if candidate_blocked {
                blocking.insert(candidate.artifact_id.clone());
            }
        } else {
            verified.insert(candidate.artifact_id.clone());
        }
    }
    for missing in required.difference(&seen_required) {
        blocking.insert((*missing).clone());
        quarantined.insert((*missing).clone());
        findings.push(IntegrityFinding {
            artifact_id: (*missing).clone(),
            kind: IntegrityFindingKind::MissingRequiredArtifact,
            detail: "required artifact is absent from scanner observations".into(),
        });
    }
    findings.sort_by(|left, right| {
        (
            left.artifact_id.clone(),
            left.kind as u8,
            left.detail.clone(),
        )
            .cmp(&(
                right.artifact_id.clone(),
                right.kind as u8,
                right.detail.clone(),
            ))
    });
    let disposition = if !blocking.is_empty() {
        IntegrityDisposition::Blocked
    } else if !warnings.is_empty() {
        IntegrityDisposition::Warnings
    } else {
        IntegrityDisposition::Clear
    };
    let mut report = ArtifactIntegrityReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_manifest_digest: request.candidate_manifest_digest.clone(),
        release_root: request.release_root.clone(),
        verified_order: verified.into_iter().collect(),
        quarantined_order: quarantined.into_iter().collect(),
        warning_order: warnings.into_iter().collect(),
        blocking_order: blocking.into_iter().collect(),
        findings,
        scanned_bytes,
        stream_chunk_bytes: request.stream_chunk_bytes,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-artifact-integrity"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ArtifactIntegrityError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn candidate(id: &str) -> ArtifactIntegrityCandidate {
        ArtifactIntegrityCandidate {
            artifact_id: id.into(),
            relative_path: format!("{id}.json"),
            declared_content_hash: hash(id),
            observed_content_hash: hash(id),
            declared_bytes: 12,
            observed_bytes: 12,
            content_type: "application/json".into(),
            metadata_valid: true,
            executable_payload: false,
            link_target: None,
            requested_export: true,
        }
    }

    fn request() -> ArtifactIntegrityRequest {
        ArtifactIntegrityRequest {
            candidate_manifest_digest: hash("manifest"),
            release_root: "release".into(),
            required_artifact_order: vec!["a".into(), "b".into()],
            allowed_content_types: vec!["application/json".into()],
            candidates: vec![candidate("a"), candidate("b")],
            max_total_bytes: 1024,
            max_memory_bytes: 64,
            stream_chunk_bytes: 32,
        }
    }

    #[test]
    fn clean_candidates_are_verified_deterministically() {
        let left = scan_glioma_artifact_integrity(&request()).unwrap();
        let right = scan_glioma_artifact_integrity(&request()).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.disposition, IntegrityDisposition::Clear);
        assert_eq!(left.verified_order, vec!["a", "b"]);
        left.validate().unwrap();
    }

    #[test]
    fn digest_truncation_and_executable_payloads_are_quarantined() {
        let mut request = request();
        request.candidates[0].observed_content_hash = hash("wrong");
        request.candidates[1].observed_bytes = 4;
        request.candidates[1].executable_payload = true;
        let output = scan_glioma_artifact_integrity(&request).unwrap();
        assert_eq!(output.disposition, IntegrityDisposition::Blocked);
        assert!(output.verified_order.is_empty());
        assert_eq!(output.quarantined_order, vec!["a", "b"]);
    }

    #[test]
    fn path_escape_and_unsupported_formats_never_verify() {
        let mut request = request();
        request.candidates[0].content_type = "application/x-sharedlib".into();
        request.candidates[1].link_target = Some("linked.json".into());
        let output = scan_glioma_artifact_integrity(&request).unwrap();
        assert!(output.verified_order.is_empty());
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == IntegrityFindingKind::UnsupportedFormat));
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == IntegrityFindingKind::PathEscape));
    }

    #[test]
    fn missing_required_artifacts_block_export() {
        let mut request = request();
        request.candidates.pop();
        let output = scan_glioma_artifact_integrity(&request).unwrap();
        assert_eq!(output.disposition, IntegrityDisposition::Blocked);
        assert_eq!(output.blocking_order, vec!["b"]);
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == IntegrityFindingKind::MissingRequiredArtifact));
    }

    #[test]
    fn stream_chunk_cannot_exceed_memory_budget() {
        let mut request = request();
        request.stream_chunk_bytes = 65;
        assert!(matches!(
            scan_glioma_artifact_integrity(&request),
            Err(ArtifactIntegrityError::InvalidRequest(_))
        ));
    }
}
