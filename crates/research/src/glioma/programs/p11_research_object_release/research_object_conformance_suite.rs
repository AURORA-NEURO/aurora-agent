//! Pinned standards conformance for preclinical glioma research objects.
//!
//! A conformance result is more than schema validation: it checks the scientific qualification
//! surface that makes an object safe to consume (provenance, uncertainty, negative evidence,
//! artifact coverage, and an independently verified signature when required). Unsupported
//! extensions are reported explicitly and never silently treated as compatible.

use super::archive_migration_adapter::ArchiveObject;
use super::release_signature_verifier::{
    ReleaseVerificationDisposition, ReleaseVerificationReport,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectConformance1@1";
pub const MAX_FIELDS: usize = 2_048;
pub const MAX_ARTIFACTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceProfile {
    pub profile_id: String,
    pub schema_id: String,
    pub schema_version: String,
    pub required_field_order: Vec<String>,
    pub allowed_field_order: Vec<String>,
    pub forbidden_field_order: Vec<String>,
    pub required_artifact_order: Vec<String>,
    pub extension_allowlist_order: Vec<String>,
    pub require_provenance: bool,
    pub require_uncertainty: bool,
    pub require_negative_evidence: bool,
    pub require_verified_signature: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceRequest {
    pub object: ArchiveObject,
    pub profile: ConformanceProfile,
    pub signature: Option<ReleaseVerificationReport>,
    pub migration_route: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceSeverity {
    Pass,
    Warning,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceFinding {
    pub finding_id: String,
    pub severity: ConformanceSeverity,
    pub rationale: String,
    pub field_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceDisposition {
    Conformant,
    Inconclusive,
    NonConformant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub feature_id: String,
    pub output_schema: String,
    pub profile_id: String,
    pub object_id: String,
    pub schema_id: String,
    pub schema_version: String,
    pub findings: Vec<ConformanceFinding>,
    pub pass_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub migration_route: Option<String>,
    pub disposition: ConformanceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConformanceError {
    #[error("research-object conformance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("research-object conformance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("research-object conformance digest failed: {0}")]
    Digest(String),
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn valid_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|major| !major.is_empty() && major.bytes().all(|byte| byte.is_ascii_digit()))
        && parts.all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_profile(profile: &ConformanceProfile) -> bool {
    valid_text(&profile.profile_id)
        && valid_text(&profile.schema_id)
        && valid_version(&profile.schema_version)
        && profile.required_field_order.len() <= MAX_FIELDS
        && profile.allowed_field_order.len() <= MAX_FIELDS
        && profile.forbidden_field_order.len() <= MAX_FIELDS
        && profile.required_artifact_order.len() <= MAX_ARTIFACTS
        && profile.extension_allowlist_order.len() <= MAX_FIELDS
        && [
            &profile.required_field_order,
            &profile.allowed_field_order,
            &profile.forbidden_field_order,
            &profile.required_artifact_order,
            &profile.extension_allowlist_order,
        ]
        .iter()
        .all(|values| canonical(values) && values.iter().all(|value| valid_text(value)))
        && profile
            .required_field_order
            .iter()
            .all(|field| profile.allowed_field_order.binary_search(field).is_ok())
        && profile
            .forbidden_field_order
            .iter()
            .all(|field| profile.allowed_field_order.binary_search(field).is_err())
}

fn valid_object(object: &ArchiveObject) -> bool {
    valid_text(&object.object_id)
        && valid_text(&object.schema_id)
        && valid_version(&object.schema_version)
        && object.fields.len() <= MAX_FIELDS
        && object.fields.keys().all(|field| valid_text(field))
        && object.fields.values().all(|value| valid_text(value))
        && object.artifacts.len() <= MAX_ARTIFACTS
        && object
            .artifacts
            .windows(2)
            .all(|pair| pair[0].artifact_id < pair[1].artifact_id)
        && object.artifacts.iter().all(|artifact| {
            valid_text(&artifact.artifact_id) && artifact.content_hash.as_str().len() == 64
        })
        && object.provenance_digest.as_str().len() == 64
        && canonical(&object.uncertainty_order)
        && canonical(&object.negative_evidence_order)
}

fn digest_input(report: &ConformanceReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "profile_id": report.profile_id,
        "object_id": report.object_id,
        "schema_id": report.schema_id,
        "schema_version": report.schema_version,
        "findings": report.findings,
        "pass_order": report.pass_order,
        "warning_order": report.warning_order,
        "blocking_order": report.blocking_order,
        "migration_route": report.migration_route,
        "disposition": report.disposition,
    })
}

fn nonzero_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64 && hash.as_str().chars().any(|byte| byte != '0')
}

fn add_finding(
    findings: &mut Vec<ConformanceFinding>,
    finding_id: &str,
    severity: ConformanceSeverity,
    rationale: &str,
    field_id: Option<&str>,
) {
    findings.push(ConformanceFinding {
        finding_id: finding_id.into(),
        severity,
        rationale: rationale.into(),
        field_id: field_id.map(str::to_string),
    });
}

impl ConformanceReport {
    pub fn validate(&self) -> Result<(), ConformanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_text(&self.profile_id)
            || !valid_text(&self.object_id)
            || !valid_text(&self.schema_id)
            || !valid_version(&self.schema_version)
            || self.findings.len() > MAX_FIELDS
            || self
                .findings
                .windows(2)
                .any(|pair| pair[0].finding_id >= pair[1].finding_id)
            || self.findings.iter().any(|finding| {
                !valid_text(&finding.finding_id)
                    || !valid_text(&finding.rationale)
                    || finding
                        .field_id
                        .as_deref()
                        .is_some_and(|field| !valid_text(field))
            })
            || !canonical(&self.pass_order)
            || !canonical(&self.warning_order)
            || !canonical(&self.blocking_order)
        {
            return Err(ConformanceError::InvalidOutput(
                "conformance identity, ordering, bounds, or finding fields are invalid".into(),
            ));
        }
        let ids = self
            .findings
            .iter()
            .map(|finding| finding.finding_id.as_str())
            .collect::<BTreeSet<_>>();
        if self
            .pass_order
            .iter()
            .chain(self.warning_order.iter())
            .chain(self.blocking_order.iter())
            .any(|finding| !ids.contains(finding.as_str()))
        {
            return Err(ConformanceError::InvalidOutput(
                "finding partitions reference an unknown finding".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ConformanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ConformanceError::InvalidOutput(
                "conformance report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ConformanceRequest) -> Result<(), ConformanceError> {
    if !valid_object(&request.object) || !valid_profile(&request.profile) {
        return Err(ConformanceError::InvalidRequest(
            "object and pinned conformance profile are invalid".into(),
        ));
    }
    if let Some(route) = &request.migration_route {
        if !valid_text(route) {
            return Err(ConformanceError::InvalidRequest(
                "migration route must be a bounded non-empty identifier".into(),
            ));
        }
    }
    if request.profile.require_verified_signature {
        let Some(signature) = &request.signature else {
            return Err(ConformanceError::InvalidRequest(
                "profile requires a verification report".into(),
            ));
        };
        signature
            .validate()
            .map_err(|error| ConformanceError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

/// Evaluate a research object against one frozen standards/scientific qualification profile.
pub fn evaluate_glioma_research_object_conformance(
    request: &ConformanceRequest,
) -> Result<ConformanceReport, ConformanceError> {
    validate_request(request)?;
    let profile = &request.profile;
    let object = &request.object;
    let mut findings = Vec::new();
    if object.schema_id != profile.schema_id {
        add_finding(
            &mut findings,
            "schema-id",
            ConformanceSeverity::Block,
            "object schema identity differs from the pinned profile",
            None,
        );
    } else {
        add_finding(
            &mut findings,
            "schema-id",
            ConformanceSeverity::Pass,
            "object schema identity matches the pinned profile",
            None,
        );
    }
    if object.schema_version != profile.schema_version {
        add_finding(
            &mut findings,
            "schema-version",
            ConformanceSeverity::Block,
            "object schema version differs from the pinned profile; migrate before import",
            None,
        );
    } else {
        add_finding(
            &mut findings,
            "schema-version",
            ConformanceSeverity::Pass,
            "object schema version is pinned and compatible",
            None,
        );
    }
    for field in &profile.required_field_order {
        if object.fields.contains_key(field) {
            add_finding(
                &mut findings,
                &format!("required-field:{field}"),
                ConformanceSeverity::Pass,
                "required typed field is present",
                Some(field),
            );
        } else {
            add_finding(
                &mut findings,
                &format!("required-field:{field}"),
                ConformanceSeverity::Block,
                "required typed field is missing and cannot be synthesized by conformance",
                Some(field),
            );
        }
    }
    for field in object.fields.keys() {
        if profile.forbidden_field_order.binary_search(field).is_ok() {
            add_finding(
                &mut findings,
                &format!("forbidden-field:{field}"),
                ConformanceSeverity::Block,
                "forbidden field is present in the object",
                Some(field),
            );
        } else if profile.allowed_field_order.binary_search(field).is_ok() {
            add_finding(
                &mut findings,
                &format!("allowed-field:{field}"),
                ConformanceSeverity::Pass,
                "field belongs to the pinned profile",
                Some(field),
            );
        } else if profile
            .extension_allowlist_order
            .binary_search(field)
            .is_ok()
        {
            add_finding(
                &mut findings,
                &format!("extension-field:{field}"),
                ConformanceSeverity::Warning,
                "declared local extension is retained but not part of the frozen core profile",
                Some(field),
            );
        } else {
            add_finding(
                &mut findings,
                &format!("unsupported-field:{field}"),
                ConformanceSeverity::Block,
                "unsupported field cannot be silently accepted; use a versioned migration route",
                Some(field),
            );
        }
    }
    let artifact_ids = object
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.as_str())
        .collect::<BTreeSet<_>>();
    for artifact_id in &profile.required_artifact_order {
        if artifact_ids.contains(artifact_id.as_str()) {
            add_finding(
                &mut findings,
                &format!("required-artifact:{artifact_id}"),
                ConformanceSeverity::Pass,
                "required content-addressed artifact is present",
                Some(artifact_id),
            );
        } else {
            add_finding(
                &mut findings,
                &format!("required-artifact:{artifact_id}"),
                ConformanceSeverity::Block,
                "required artifact is absent from the object",
                Some(artifact_id),
            );
        }
    }
    if profile.require_provenance {
        add_finding(
            &mut findings,
            "provenance",
            if nonzero_hash(&object.provenance_digest) {
                ConformanceSeverity::Pass
            } else {
                ConformanceSeverity::Block
            },
            if nonzero_hash(&object.provenance_digest) {
                "provenance digest is present"
            } else {
                "provenance digest is absent or an unbound zero placeholder"
            },
            None,
        );
    }
    if profile.require_uncertainty {
        add_finding(
            &mut findings,
            "uncertainty",
            if object.uncertainty_order.is_empty() {
                ConformanceSeverity::Block
            } else {
                ConformanceSeverity::Pass
            },
            if object.uncertainty_order.is_empty() {
                "uncertainty qualification is missing"
            } else {
                "uncertainty qualification is retained"
            },
            None,
        );
    }
    if profile.require_negative_evidence {
        add_finding(
            &mut findings,
            "negative-evidence",
            if object.negative_evidence_order.is_empty() {
                ConformanceSeverity::Block
            } else {
                ConformanceSeverity::Pass
            },
            if object.negative_evidence_order.is_empty() {
                "negative/null evidence declaration is missing"
            } else {
                "negative/null evidence declaration is retained"
            },
            None,
        );
    }
    if profile.require_verified_signature {
        let verified = request.signature.as_ref().is_some_and(|signature| {
            signature.disposition == ReleaseVerificationDisposition::Verified
        });
        add_finding(
            &mut findings,
            "verified-signature",
            if verified {
                ConformanceSeverity::Pass
            } else {
                ConformanceSeverity::Block
            },
            if verified {
                "offline signature and provenance verification passed"
            } else {
                "signature is absent or not verified against the pinned trust roots"
            },
            None,
        );
    }
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    let mut pass_order = Vec::new();
    let mut warning_order = Vec::new();
    let mut blocking_order = Vec::new();
    for finding in &findings {
        match finding.severity {
            ConformanceSeverity::Pass => pass_order.push(finding.finding_id.clone()),
            ConformanceSeverity::Warning => warning_order.push(finding.finding_id.clone()),
            ConformanceSeverity::Block => blocking_order.push(finding.finding_id.clone()),
        }
    }
    let disposition = if !blocking_order.is_empty() {
        ConformanceDisposition::NonConformant
    } else if !warning_order.is_empty() {
        ConformanceDisposition::Inconclusive
    } else {
        ConformanceDisposition::Conformant
    };
    let mut report = ConformanceReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        profile_id: profile.profile_id.clone(),
        object_id: object.object_id.clone(),
        schema_id: object.schema_id.clone(),
        schema_version: object.schema_version.clone(),
        findings,
        pass_order,
        warning_order,
        blocking_order,
        migration_route: request.migration_route.clone(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-conformance"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ConformanceError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::archive_migration_adapter::ArchiveArtifactRef;
    use std::collections::BTreeMap;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn object() -> ArchiveObject {
        ArchiveObject {
            object_id: "object-1".into(),
            schema_id: "aurora.glioma.result".into(),
            schema_version: "2.0".into(),
            fields: BTreeMap::from([("effect".into(), "0.42".into())]),
            artifacts: vec![ArchiveArtifactRef {
                artifact_id: "result".into(),
                content_hash: hash("result"),
                required: true,
                local_only: true,
            }],
            provenance_digest: hash("provenance"),
            uncertainty_order: vec!["tail".into()],
            negative_evidence_order: vec!["null".into()],
        }
    }

    fn profile() -> ConformanceProfile {
        ConformanceProfile {
            profile_id: "ro-crate-glioma".into(),
            schema_id: "aurora.glioma.result".into(),
            schema_version: "2.0".into(),
            required_field_order: vec!["effect".into()],
            allowed_field_order: vec!["effect".into()],
            forbidden_field_order: vec!["protected".into()],
            required_artifact_order: vec!["result".into()],
            extension_allowlist_order: vec!["local_extension".into()],
            require_provenance: true,
            require_uncertainty: true,
            require_negative_evidence: true,
            require_verified_signature: false,
        }
    }

    #[test]
    fn conformant_profile_is_deterministic() {
        let request = ConformanceRequest {
            object: object(),
            profile: profile(),
            signature: None,
            migration_route: None,
        };
        let report = evaluate_glioma_research_object_conformance(&request).unwrap();
        assert_eq!(report.disposition, ConformanceDisposition::Conformant);
        report.validate().unwrap();
    }

    #[test]
    fn unsupported_field_blocks_and_preserves_migration_route() {
        let mut request = ConformanceRequest {
            object: object(),
            profile: profile(),
            signature: None,
            migration_route: Some("GAF-GLIOMA-P11-F22".into()),
        };
        request
            .object
            .fields
            .insert("unknown".into(), "value".into());
        let report = evaluate_glioma_research_object_conformance(&request).unwrap();
        assert_eq!(report.disposition, ConformanceDisposition::NonConformant);
        assert!(report
            .blocking_order
            .iter()
            .any(|finding| finding.contains("unsupported-field")));
        assert_eq!(
            report.migration_route.as_deref(),
            Some("GAF-GLIOMA-P11-F22")
        );
    }

    #[test]
    fn extension_is_warning_and_missing_scientific_qualification_blocks() {
        let mut request = ConformanceRequest {
            object: object(),
            profile: profile(),
            signature: None,
            migration_route: None,
        };
        request
            .object
            .fields
            .insert("local_extension".into(), "value".into());
        request.object.uncertainty_order.clear();
        let report = evaluate_glioma_research_object_conformance(&request).unwrap();
        assert_eq!(report.disposition, ConformanceDisposition::NonConformant);
        assert!(report
            .warning_order
            .iter()
            .any(|finding| finding.contains("extension-field")));
        assert!(report
            .blocking_order
            .iter()
            .any(|finding| finding == "uncertainty"));
    }
}
