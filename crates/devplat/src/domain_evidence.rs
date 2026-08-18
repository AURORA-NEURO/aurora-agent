//! Cross-domain evidence harmonization for explicit domain-report projections.
//!
//! This module joins reports without pretending that a join is an experiment, a causal analysis,
//! a clinical interpretation, or a release gate. Every report is validated as a canonical
//! `domain_report`, every report must be linked at least once by an explicit caller role, and
//! subject identity must match exactly. The result is therefore a traceability artifact: it tells
//! a reviewer which report bytes were considered, which capability group/source produced them,
//! and where the caller declared support, qualification, contradiction, or context.

use crate::domain_report::{
    classify_domain_report_bridge, validate_domain_report, DOMAIN_REPORT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-evidence-harmonization/0.1";
pub const DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW: &str = "domain_evidence_harmonize";
pub const MAX_DOMAIN_EVIDENCE_HARMONIZATION_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DOMAIN_EVIDENCE_REPORTS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_LINKS: usize = 256;
pub const MAX_DOMAIN_EVIDENCE_REQUIREMENTS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_TEXT_BYTES: usize = 512;

const LINK_ROLES: &[&str] = &["supports", "qualifies", "contradicts", "context"];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainEvidenceError {
    #[error("domain evidence harmonization input must be a JSON object")]
    NotObject,
    #[error("domain evidence field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain evidence field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("domain evidence field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("domain evidence input is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error("domain evidence report {index} is invalid: {error}")]
    InvalidReport { index: usize, error: String },
    #[error("domain evidence report {index} has subject_id {actual:?}, expected {expected:?}")]
    SubjectMismatch {
        index: usize,
        actual: String,
        expected: String,
    },
    #[error("domain evidence report {index} registry digest does not match its canonical report")]
    ReportDigestMismatch { index: usize },
    #[error("domain evidence link {index} points to report index {report_index}, but only {report_count} reports exist")]
    LinkReportOutOfRange {
        index: usize,
        report_index: usize,
        report_count: usize,
    },
    #[error("domain evidence link {index} has unsupported role {role:?}")]
    InvalidRole { index: usize, role: String },
    #[error("domain evidence link {index} must include a note for {role} links")]
    LinkNoteRequired { index: usize, role: String },
    #[error("domain evidence link {index} is duplicated")]
    DuplicateLink { index: usize },
    #[error("domain evidence claim id is required")]
    ClaimIdRequired,
    #[error("domain evidence could not be canonicalised: {0}")]
    Canonicalisation(String),
}

#[derive(Debug, Clone)]
struct CanonicalReport {
    report: Value,
    digest: String,
}

/// Build a deterministic traceability artifact from explicit domain reports and caller links.
pub fn harmonize_domain_evidence(request: &Value) -> Result<Value, DomainEvidenceError> {
    let object = request.as_object().ok_or(DomainEvidenceError::NotObject)?;
    ensure_size(request)?;
    let subject_id = required_text(object, "subject_id")?;
    let claim = claim(object.get("claim"))?;
    let reports = canonical_reports(object.get("reports"), &subject_id)?;
    let links = links(object.get("links"), reports.len(), &reports)?;
    let required_group_ids = optional_text_set(object, "required_group_ids")?;
    let required_domains = optional_text_set(object, "required_domains")?;

    let mut linked_roles: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for link in &links {
        linked_roles
            .entry(link.report_index)
            .or_default()
            .push(link.role.clone());
    }
    let all_reports_linked = (0..reports.len()).all(|index| linked_roles.contains_key(&index));
    let observed_groups = reports
        .iter()
        .filter_map(|report| report.report.get("group_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let observed_domains = reports
        .iter()
        .flat_map(|report| {
            report
                .report
                .get("domains")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    let missing_group_ids = required_group_ids
        .iter()
        .filter(|group| !observed_groups.contains(group.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let missing_domains = required_domains
        .iter()
        .filter(|domain| !observed_domains.contains(domain.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let mut support_link_count = 0usize;
    let mut qualification_link_count = 0usize;
    let mut contradiction_link_count = 0usize;
    let mut context_link_count = 0usize;
    let mut report_class_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut bridge_mode_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut lineage_parent_digest_count = 0usize;
    let mut reports_with_lineage_parents = 0usize;
    let mut reports_without_lineage_parents = 0usize;
    let mut link_rows = Vec::with_capacity(links.len());
    for link in &links {
        match link.role.as_str() {
            "supports" => support_link_count += 1,
            "qualifies" => qualification_link_count += 1,
            "contradicts" => contradiction_link_count += 1,
            "context" => context_link_count += 1,
            _ => unreachable!("links are validated above"),
        }
        link_rows.push(json!({
            "report_index": link.report_index,
            "report_digest": reports[link.report_index].digest,
            "role": link.role,
            "note": link.note
        }));
    }
    let report_rows = reports
        .iter()
        .enumerate()
        .map(|(index, report)| {
            let object = report
                .report
                .as_object()
                .expect("canonical domain report is an object");
            let roles = linked_roles.get(&index).cloned().unwrap_or_default();
            let bridge_metadata = classify_domain_report_bridge(&report.report);
            let report_class = bridge_metadata.report_class;
            let bridge_mode = bridge_metadata.mode;
            *report_class_counts
                .entry(report_class.to_string())
                .or_default() += 1;
            if let Some(mode) = bridge_mode.as_ref() {
                *bridge_mode_counts.entry(mode.to_string()).or_default() += 1;
            }
            let parent_digests = object
                .get("parent_digests")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            lineage_parent_digest_count += parent_digests;
            if parent_digests > 0 {
                reports_with_lineage_parents += 1;
            } else {
                reports_without_lineage_parents += 1;
            }
            json!({
                "index": index,
                "digest": report.digest,
                "group_id": object.get("group_id"),
                "domains": object.get("domains"),
                "subject_id": object.get("subject_id"),
                "source_tool": object.get("source_tool"),
                "claim_status": object
                    .get("claim_posture")
                    .and_then(Value::as_object)
                    .and_then(|posture| posture.get("status")),
                "parent_digests": object.get("parent_digests"),
                "report_class": report_class,
                "bridge_mode": bridge_mode,
                "lineage_parent_count": parent_digests,
                "link_roles": roles,
                "link_count": linked_roles.get(&index).map_or(0, Vec::len)
            })
        })
        .collect::<Vec<_>>();
    let claim_statuses = reports
        .iter()
        .filter_map(|report| {
            report
                .report
                .get("claim_posture")
                .and_then(Value::as_object)
                .and_then(|posture| posture.get("status"))
                .and_then(Value::as_str)
        })
        .collect::<BTreeSet<_>>();
    let requirements_complete = missing_group_ids.is_empty() && missing_domains.is_empty();
    let traceability_state = if all_reports_linked && requirements_complete {
        "complete"
    } else if all_reports_linked {
        "requirements_missing"
    } else {
        "links_missing"
    };
    let mut result = json!({
        "schema": DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
        "workflow": DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW,
        "subject_id": subject_id,
        "claim": claim,
        "report_count": reports.len(),
        "reports": report_rows,
        "links": link_rows,
        "required_group_ids": required_group_ids,
        "required_domains": required_domains,
        "missing_group_ids": missing_group_ids,
        "missing_domains": missing_domains,
            "coverage": {
                "all_reports_linked": all_reports_linked,
                "requirements_complete": requirements_complete,
                "traceability_state": traceability_state,
                "observed_group_count": observed_groups.len(),
                "observed_domain_count": observed_domains.len(),
                "bridge_summary": {
                    "report_classes": report_class_counts,
                    "modes": bridge_mode_counts,
                    "lineage": {
                        "parent_digest_count": lineage_parent_digest_count,
                        "reports_with_lineage_parents": reports_with_lineage_parents,
                        "reports_without_lineage_parents": reports_without_lineage_parents
                    }
                }
            },
        "posture": {
            "support_link_count": support_link_count,
            "qualification_link_count": qualification_link_count,
            "contradiction_link_count": contradiction_link_count,
            "context_link_count": context_link_count,
            "explicit_contradiction_declared": contradiction_link_count > 0,
            "qualification_declared": qualification_link_count > 0,
            "claim_statuses": claim_statuses,
            "interpretation": "not_run",
            "requires_human_review": true
        },
        "readiness_claimed": false,
        "execution": "not_started",
        "guarantees": [
            "every retained report row identifies its exact canonical report digest, capability group, domains, source tool, and link roles",
            "subject identity is joined by exact string equality and mismatches are refused",
            "caller-declared support, qualification, contradiction, and context remain distinct",
            "the harmonizer records traceability and does not execute tools or interpret the claim"
        ],
        "does_not_claim": [
            "a support link proves the claim is true or causally identified",
            "a contradiction link proves which report is correct",
            "complete traceability proves scientific, clinical, regulatory, publication, or release validity",
            "report presence or harmonization proves provenance completeness or external effect completion"
        ]
    });
    let digest = ContentHash::of_value(&result)
        .map_err(|error| DomainEvidenceError::Canonicalisation(error.to_string()))?;
    result["harmonization_digest"] = json!(digest.to_string());
    ensure_size(&result)?;
    validate_domain_evidence_harmonization(&result)?;
    Ok(result)
}

/// Validate an already harmonized report before durable artifact indexing or restore.
pub fn validate_domain_evidence_harmonization(value: &Value) -> Result<(), DomainEvidenceError> {
    let object = value.as_object().ok_or(DomainEvidenceError::NotObject)?;
    exact_text(
        object,
        "schema",
        DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
    )?;
    exact_text(object, "workflow", DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW)?;
    let subject_id = required_text(object, "subject_id")?;
    let _ = claim(object.get("claim"))?;
    let harmonization_digest = required_text(object, "harmonization_digest")?;
    if !is_sha256_digest(&harmonization_digest) {
        return Err(DomainEvidenceError::InvalidField(
            "harmonization_digest".into(),
        ));
    }
    let report_count = object
        .get("report_count")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| DomainEvidenceError::InvalidField("report_count".into()))?;
    if !(1..=MAX_DOMAIN_EVIDENCE_REPORTS).contains(&report_count) {
        return Err(DomainEvidenceError::InvalidField("report_count".into()));
    }
    let report_rows = object
        .get("reports")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceError::InvalidField("reports".into()))?;
    if report_rows.len() != report_count {
        return Err(DomainEvidenceError::InvalidField("reports".into()));
    }
    let mut report_digests = Vec::with_capacity(report_rows.len());
    let mut observed_groups = BTreeSet::new();
    let mut observed_domains = BTreeSet::new();
    let mut report_class_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut bridge_mode_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut lineage_parent_digest_count = 0usize;
    let mut reports_with_lineage_parents = 0usize;
    let mut reports_without_lineage_parents = 0usize;
    for (index, row) in report_rows.iter().enumerate() {
        let row = row
            .as_object()
            .ok_or_else(|| DomainEvidenceError::InvalidField(format!("reports[{index}]")))?;
        if row.get("index").and_then(Value::as_u64) != Some(index as u64) {
            return Err(DomainEvidenceError::InvalidField(format!(
                "reports[{index}].index"
            )));
        }
        let digest = required_text(row, "digest")?;
        if !is_sha256_digest(&digest) {
            return Err(DomainEvidenceError::InvalidField(format!(
                "reports[{index}].digest"
            )));
        }
        report_digests.push(digest);
        required_text(row, "group_id")?;
        let group_id = required_text(row, "group_id")?;
        observed_groups.insert(group_id);
        let row_subject_id = required_text(row, "subject_id")?;
        if row_subject_id != subject_id {
            return Err(DomainEvidenceError::SubjectMismatch {
                index,
                actual: row_subject_id,
                expected: subject_id.clone(),
            });
        }
        required_text(row, "source_tool")?;
        for domain in text_array(row, "domains", MAX_DOMAIN_EVIDENCE_REQUIREMENTS)? {
            observed_domains.insert(domain);
        }
        let report_class = required_text(row, "report_class")?;
        *report_class_counts.entry(report_class).or_default() += 1;
        let bridge_mode = match row.get("bridge_mode") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
            Some(_) => {
                return Err(DomainEvidenceError::InvalidField(format!(
                    "reports[{index}].bridge_mode"
                )))
            }
        };
        if let Some(mode) = bridge_mode {
            *bridge_mode_counts.entry(mode).or_default() += 1;
        }
        let parent_digests = row
            .get("parent_digests")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DomainEvidenceError::InvalidField(format!("reports[{index}].parent_digests"))
            })?;
        let parent_count = row
            .get("lineage_parent_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DomainEvidenceError::InvalidField(format!("reports[{index}].lineage_parent_count"))
            })?;
        if parent_count != parent_digests.len() {
            return Err(DomainEvidenceError::InvalidField(format!(
                "reports[{index}].lineage_parent_count"
            )));
        }
        lineage_parent_digest_count += parent_count;
        if parent_count > 0 {
            reports_with_lineage_parents += 1;
        } else {
            reports_without_lineage_parents += 1;
        }
        let _ = text_array(row, "link_roles", MAX_DOMAIN_EVIDENCE_LINKS)?;
    }
    let links = object
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceError::InvalidField("links".into()))?;
    if links.is_empty() || links.len() > MAX_DOMAIN_EVIDENCE_LINKS {
        return Err(DomainEvidenceError::TooManyItems {
            field: "links".into(),
            maximum: MAX_DOMAIN_EVIDENCE_LINKS,
        });
    }
    let mut linked_reports = BTreeSet::new();
    let mut role_counts = BTreeMap::new();
    for (index, link) in links.iter().enumerate() {
        let link = link
            .as_object()
            .ok_or_else(|| DomainEvidenceError::InvalidField(format!("links[{index}]")))?;
        let report_index = link
            .get("report_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DomainEvidenceError::InvalidField(format!("links[{index}].report_index"))
            })?;
        if report_index >= report_count {
            return Err(DomainEvidenceError::LinkReportOutOfRange {
                index,
                report_index,
                report_count,
            });
        }
        let role = required_text(link, "role")?;
        if !LINK_ROLES.contains(&role.as_str()) {
            return Err(DomainEvidenceError::InvalidRole { index, role });
        }
        let report_digest = required_text(link, "report_digest")?;
        if !is_sha256_digest(&report_digest) || report_digest != report_digests[report_index] {
            return Err(DomainEvidenceError::ReportDigestMismatch { index });
        }
        let note = match link.get("note") {
            None => "",
            Some(Value::String(value)) if value.len() <= MAX_DOMAIN_EVIDENCE_TEXT_BYTES => value,
            Some(_) => {
                return Err(DomainEvidenceError::InvalidField(format!(
                    "links[{index}].note"
                )))
            }
        };
        if note.is_empty() && matches!(role.as_str(), "qualifies" | "contradicts") {
            return Err(DomainEvidenceError::LinkNoteRequired { index, role });
        }
        linked_reports.insert(report_index);
        *role_counts.entry(role).or_insert(0usize) += 1;
    }
    let required_group_ids = text_array(
        object,
        "required_group_ids",
        MAX_DOMAIN_EVIDENCE_REQUIREMENTS,
    )?;
    let required_domains =
        text_array(object, "required_domains", MAX_DOMAIN_EVIDENCE_REQUIREMENTS)?;
    let missing_group_ids = text_array(
        object,
        "missing_group_ids",
        MAX_DOMAIN_EVIDENCE_REQUIREMENTS,
    )?;
    let missing_domains = text_array(object, "missing_domains", MAX_DOMAIN_EVIDENCE_REQUIREMENTS)?;
    let expected_missing_groups = required_group_ids
        .iter()
        .filter(|group| !observed_groups.contains(group.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let expected_missing_domains = required_domains
        .iter()
        .filter(|domain| !observed_domains.contains(domain.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing_group_ids != expected_missing_groups || missing_domains != expected_missing_domains {
        return Err(DomainEvidenceError::InvalidField(
            "missing_requirements".into(),
        ));
    }
    let coverage = object
        .get("coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| DomainEvidenceError::InvalidField("coverage".into()))?;
    let all_reports_linked = linked_reports.len() == report_count;
    if coverage.get("all_reports_linked") != Some(&Value::Bool(all_reports_linked)) {
        return Err(DomainEvidenceError::InvalidField(
            "coverage.all_reports_linked".into(),
        ));
    }
    let requirements_complete = missing_group_ids.is_empty() && missing_domains.is_empty();
    if coverage.get("requirements_complete") != Some(&Value::Bool(requirements_complete)) {
        return Err(DomainEvidenceError::InvalidField(
            "coverage.requirements_complete".into(),
        ));
    }
    let expected_state = if all_reports_linked && requirements_complete {
        "complete"
    } else if all_reports_linked {
        "requirements_missing"
    } else {
        "links_missing"
    };
    exact_text(coverage, "traceability_state", expected_state)?;
    let bridge_summary = coverage
        .get("bridge_summary")
        .and_then(Value::as_object)
        .ok_or_else(|| DomainEvidenceError::InvalidField("coverage.bridge_summary".into()))?;
    let expected_report_classes = json!(report_class_counts);
    if bridge_summary.get("report_classes") != Some(&expected_report_classes) {
        return Err(DomainEvidenceError::InvalidField(
            "coverage.bridge_summary.report_classes".into(),
        ));
    }
    let expected_modes = json!(bridge_mode_counts);
    if bridge_summary.get("modes") != Some(&expected_modes) {
        return Err(DomainEvidenceError::InvalidField(
            "coverage.bridge_summary.modes".into(),
        ));
    }
    let lineage = bridge_summary
        .get("lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainEvidenceError::InvalidField("coverage.bridge_summary.lineage".into())
        })?;
    for (field, expected) in [
        ("parent_digest_count", lineage_parent_digest_count),
        ("reports_with_lineage_parents", reports_with_lineage_parents),
        (
            "reports_without_lineage_parents",
            reports_without_lineage_parents,
        ),
    ] {
        if lineage.get(field).and_then(Value::as_u64) != Some(expected as u64) {
            return Err(DomainEvidenceError::InvalidField(format!(
                "coverage.bridge_summary.lineage.{field}"
            )));
        }
    }
    let posture = object
        .get("posture")
        .and_then(Value::as_object)
        .ok_or_else(|| DomainEvidenceError::InvalidField("posture".into()))?;
    for (field, role) in [
        ("support_link_count", "supports"),
        ("qualification_link_count", "qualifies"),
        ("contradiction_link_count", "contradicts"),
        ("context_link_count", "context"),
    ] {
        if posture.get(field).and_then(Value::as_u64)
            != Some(*role_counts.get(role).unwrap_or(&0) as u64)
        {
            return Err(DomainEvidenceError::InvalidField(format!(
                "posture.{field}"
            )));
        }
    }
    if posture.get("explicit_contradiction_declared")
        != Some(&Value::Bool(
            role_counts.get("contradicts").copied().unwrap_or(0) > 0,
        ))
    {
        return Err(DomainEvidenceError::InvalidField(
            "posture.explicit_contradiction_declared".into(),
        ));
    }
    if posture.get("qualification_declared")
        != Some(&Value::Bool(
            role_counts.get("qualifies").copied().unwrap_or(0) > 0,
        ))
    {
        return Err(DomainEvidenceError::InvalidField(
            "posture.qualification_declared".into(),
        ));
    }
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(DomainEvidenceError::InvalidField(
            "readiness_claimed".into(),
        ));
    }
    exact_text(object, "execution", "not_started")?;
    let mut digest_input = object.clone();
    digest_input.remove("harmonization_digest");
    let recomputed = ContentHash::of_value(&Value::Object(digest_input))
        .map_err(|error| DomainEvidenceError::Canonicalisation(error.to_string()))?;
    if recomputed.to_string() != harmonization_digest {
        return Err(DomainEvidenceError::InvalidField(
            "harmonization_digest".into(),
        ));
    }
    ensure_size(value)
}

#[derive(Debug, Clone)]
struct Link {
    report_index: usize,
    role: String,
    note: String,
}

fn canonical_reports(
    value: Option<&Value>,
    subject_id: &str,
) -> Result<Vec<CanonicalReport>, DomainEvidenceError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceError::InvalidField("reports".into()))?;
    if values.is_empty() || values.len() > MAX_DOMAIN_EVIDENCE_REPORTS {
        return Err(DomainEvidenceError::InvalidField("reports".into()));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let object = value
                .as_object()
                .ok_or_else(|| DomainEvidenceError::InvalidReport {
                    index,
                    error: "report must be an object".into(),
                })?;
            let report = if object.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_REPORT_SCHEMA_VERSION)
            {
                value.clone()
            } else {
                object
                    .get("report")
                    .filter(|report| {
                        report.get("schema").and_then(Value::as_str)
                            == Some(DOMAIN_REPORT_SCHEMA_VERSION)
                    })
                    .cloned()
                    .ok_or_else(|| DomainEvidenceError::InvalidReport {
                        index,
                        error: "expected a canonical domain_report or a projection wrapper".into(),
                    })?
            };
            validate_domain_report(&report).map_err(|error| {
                DomainEvidenceError::InvalidReport {
                    index,
                    error: error.to_string(),
                }
            })?;
            let actual_subject = report
                .get("subject_id")
                .and_then(Value::as_str)
                .ok_or_else(|| DomainEvidenceError::InvalidReport {
                    index,
                    error: "subject_id is missing".into(),
                })?;
            if actual_subject != subject_id {
                return Err(DomainEvidenceError::SubjectMismatch {
                    index,
                    actual: actual_subject.to_string(),
                    expected: subject_id.to_string(),
                });
            }
            let digest = ContentHash::of_value(&report)
                .map_err(|error| DomainEvidenceError::Canonicalisation(error.to_string()))?
                .to_string();
            if let Some(declared) = object
                .get("artifact_registry")
                .and_then(Value::as_object)
                .and_then(|registry| registry.get("content_digest"))
                .and_then(Value::as_str)
            {
                if declared != digest {
                    return Err(DomainEvidenceError::ReportDigestMismatch { index });
                }
            }
            Ok(CanonicalReport { report, digest })
        })
        .collect()
}

fn links(
    value: Option<&Value>,
    report_count: usize,
    reports: &[CanonicalReport],
) -> Result<Vec<Link>, DomainEvidenceError> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceError::InvalidField("links".into()))?;
    if values.is_empty() || values.len() > MAX_DOMAIN_EVIDENCE_LINKS {
        return Err(DomainEvidenceError::InvalidField("links".into()));
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| DomainEvidenceError::InvalidField(format!("links[{index}")))?;
        let report_index = object
            .get("report_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DomainEvidenceError::InvalidField(format!("links[{index}].report_index"))
            })?;
        if report_index >= report_count {
            return Err(DomainEvidenceError::LinkReportOutOfRange {
                index,
                report_index,
                report_count,
            });
        }
        let role = required_text(object, "role")?;
        if !LINK_ROLES.contains(&role.as_str()) {
            return Err(DomainEvidenceError::InvalidRole { index, role });
        }
        let note = object
            .get("note")
            .map(|_| required_text(object, "note"))
            .transpose()?
            .unwrap_or_default();
        if note.is_empty() && matches!(role.as_str(), "qualifies" | "contradicts") {
            return Err(DomainEvidenceError::LinkNoteRequired { index, role });
        }
        let declared_digest = object
            .get("report_digest")
            .map(|_| required_text(object, "report_digest"))
            .transpose()?;
        if declared_digest
            .as_deref()
            .is_some_and(|digest| digest != reports[report_index].digest)
        {
            return Err(DomainEvidenceError::ReportDigestMismatch { index });
        }
        let key = (report_index, role.clone(), note.clone());
        if !seen.insert(key) {
            return Err(DomainEvidenceError::DuplicateLink { index });
        }
        result.push(Link {
            report_index,
            role,
            note,
        });
    }
    result.sort_by(|left, right| {
        left.report_index
            .cmp(&right.report_index)
            .then_with(|| left.role.cmp(&right.role))
            .then_with(|| left.note.cmp(&right.note))
    });
    Ok(result)
}

fn claim(value: Option<&Value>) -> Result<Value, DomainEvidenceError> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| DomainEvidenceError::InvalidField("claim".into()))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(DomainEvidenceError::ClaimIdRequired)?;
    if id.len() > MAX_DOMAIN_EVIDENCE_TEXT_BYTES {
        return Err(DomainEvidenceError::TextTooLarge {
            field: "claim.id".into(),
            maximum: MAX_DOMAIN_EVIDENCE_TEXT_BYTES,
        });
    }
    let mut result = Map::new();
    result.insert("id".into(), Value::String(id.to_string()));
    for field in ["statement", "scope"] {
        if let Some(value) = object.get(field) {
            match field {
                "statement" => {
                    let value = value
                        .as_str()
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            DomainEvidenceError::InvalidField(format!("claim.{field}"))
                        })?;
                    if value.len() > MAX_DOMAIN_EVIDENCE_TEXT_BYTES {
                        return Err(DomainEvidenceError::TextTooLarge {
                            field: format!("claim.{field}"),
                            maximum: MAX_DOMAIN_EVIDENCE_TEXT_BYTES,
                        });
                    }
                    result.insert(field.into(), Value::String(value.to_string()));
                }
                "scope" => {
                    result.insert(field.into(), Value::String(required_text(object, field)?));
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(Value::Object(result))
}

fn optional_text_set(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, DomainEvidenceError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(value) => text_array_value(value, field, MAX_DOMAIN_EVIDENCE_REQUIREMENTS),
    }
}

fn text_array(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceError> {
    let value = object
        .get(field)
        .ok_or_else(|| DomainEvidenceError::InvalidField(field.into()))?;
    text_array_value(value, field, maximum)
}

fn text_array_value(
    value: &Value,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceError> {
    let values = value
        .as_array()
        .ok_or_else(|| DomainEvidenceError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(DomainEvidenceError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| DomainEvidenceError::InvalidField(field.into()))?;
        if value.len() > MAX_DOMAIN_EVIDENCE_TEXT_BYTES {
            return Err(DomainEvidenceError::TextTooLarge {
                field: field.into(),
                maximum: MAX_DOMAIN_EVIDENCE_TEXT_BYTES,
            });
        }
        result.insert(value.to_string());
    }
    Ok(result.into_iter().collect())
}

fn required_text(object: &Map<String, Value>, field: &str) -> Result<String, DomainEvidenceError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DomainEvidenceError::InvalidField(field.into()))?;
    if value.len() > MAX_DOMAIN_EVIDENCE_TEXT_BYTES {
        return Err(DomainEvidenceError::TextTooLarge {
            field: field.into(),
            maximum: MAX_DOMAIN_EVIDENCE_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), DomainEvidenceError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(DomainEvidenceError::InvalidField(field.into()));
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn ensure_size(value: &Value) -> Result<(), DomainEvidenceError> {
    let actual = serde_json::to_vec(value)
        .map_err(|error| DomainEvidenceError::Canonicalisation(error.to_string()))?
        .len();
    if actual > MAX_DOMAIN_EVIDENCE_HARMONIZATION_BYTES {
        return Err(DomainEvidenceError::TooLarge {
            actual,
            maximum: MAX_DOMAIN_EVIDENCE_HARMONIZATION_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_report::project_domain_report;

    fn report(subject_id: &str, source_tool: &str, group_id: &str, domain: &str) -> Value {
        project_domain_report(&json!({
            "group_id": group_id,
            "domains": [domain],
            "subject_id": subject_id,
            "source_tool": source_tool,
            "report": {"observations": [group_id]},
            "claim_posture": {"status": "review_required", "does_not_claim": ["truth"]}
        }))
        .unwrap()
    }

    #[test]
    fn harmonization_preserves_roles_and_exact_digests() {
        let first = report(
            "subject-1",
            "modality_catalog",
            "biological_domains",
            "modalities",
        );
        let second = report(
            "subject-1",
            "bioql_compile",
            "biological_ir_and_query",
            "BioQL syntax",
        );
        let result = harmonize_domain_evidence(&json!({
            "subject_id": "subject-1",
            "claim": {"id": "claim-1", "statement": "opaque caller statement"},
            "reports": [first, second],
            "required_group_ids": ["biological_domains", "biological_ir_and_query"],
            "required_domains": ["modalities", "BioQL syntax"],
            "links": [
                {"report_index": 0, "role": "supports", "note": "caller link"},
                {"report_index": 1, "role": "qualifies", "note": "scope differs"}
            ]
        }))
        .unwrap();
        assert_eq!(result["coverage"]["traceability_state"], "complete");
        assert_eq!(result["posture"]["support_link_count"], 1);
        assert_eq!(result["posture"]["qualification_link_count"], 1);
        assert_eq!(result["reports"][0]["digest"].as_str().unwrap().len(), 64);
        assert_eq!(result["reports"][0]["report_class"], "ordinary");
        assert_eq!(result["reports"][0]["lineage_parent_count"], 0);
        assert_eq!(
            result["coverage"]["bridge_summary"]["report_classes"]["ordinary"],
            2
        );
        assert_eq!(
            result["coverage"]["bridge_summary"]["lineage"]["reports_without_lineage_parents"],
            2
        );
        assert_eq!(result["readiness_claimed"], false);
        validate_domain_evidence_harmonization(&result).unwrap();
    }

    #[test]
    fn harmonization_classifies_provider_bridges_and_lineage() {
        let mut bridged = report(
            "subject-1",
            "literature_bind_check",
            "biological_domains",
            "oncology",
        );
        bridged["report"] = json!({
            "kind": "provider_normalization",
            "mode": "external_payload",
            "payload_digest": "a".repeat(64)
        });
        bridged["parent_digests"] = json!(["b".repeat(64), "c".repeat(64)]);
        let result = harmonize_domain_evidence(&json!({
            "subject_id": "subject-1",
            "claim": {"id": "claim-bridge"},
            "reports": [bridged],
            "links": [{"report_index": 0, "role": "context"}]
        }))
        .unwrap();
        assert_eq!(
            result["reports"][0]["report_class"],
            "provider_normalization_external_payload"
        );
        assert_eq!(result["reports"][0]["bridge_mode"], "external_payload");
        assert_eq!(result["reports"][0]["lineage_parent_count"], 2);
        assert_eq!(
            result["coverage"]["bridge_summary"]["modes"]["external_payload"],
            1
        );
        assert_eq!(
            result["coverage"]["bridge_summary"]["lineage"]["parent_digest_count"],
            2
        );
        validate_domain_evidence_harmonization(&result).unwrap();
    }

    #[test]
    fn refuses_subject_mismatch_and_unlinked_report() {
        let mismatched = harmonize_domain_evidence(&json!({
            "subject_id": "subject-1",
            "claim": {"id": "claim-1"},
            "reports": [report("subject-2", "modality_catalog", "biological_domains", "modalities")],
            "links": [{"report_index": 0, "role": "context"}]
        }));
        assert!(matches!(
            mismatched,
            Err(DomainEvidenceError::SubjectMismatch { .. })
        ));

        let unlinked = harmonize_domain_evidence(&json!({
            "subject_id": "subject-1",
            "claim": {"id": "claim-1"},
            "reports": [
                report("subject-1", "modality_catalog", "biological_domains", "modalities"),
                report("subject-1", "bioql_compile", "biological_ir_and_query", "BioQL syntax")
            ],
            "links": [{"report_index": 0, "role": "supports"}]
        }))
        .unwrap();
        assert_eq!(unlinked["coverage"]["traceability_state"], "links_missing");
        assert_eq!(unlinked["reports"][1]["link_count"], 0);
    }

    #[test]
    fn refuses_digest_mismatched_projection_wrapper_and_duplicate_links() {
        let wrapped_report = report(
            "subject-1",
            "modality_catalog",
            "biological_domains",
            "modalities",
        );
        let wrapped = json!({
            "report": wrapped_report,
            "artifact_registry": {"content_digest": "f".repeat(64)}
        });
        assert!(matches!(
            harmonize_domain_evidence(&json!({
                "subject_id": "subject-1",
                "claim": {"id": "claim-1"},
                "reports": [wrapped],
                "links": [{"report_index": 0, "role": "context"}]
            })),
            Err(DomainEvidenceError::ReportDigestMismatch { .. })
        ));

        let duplicate_report = report(
            "subject-1",
            "modality_catalog",
            "biological_domains",
            "modalities",
        );
        assert!(matches!(
            harmonize_domain_evidence(&json!({
                "subject_id": "subject-1",
                "claim": {"id": "claim-1"},
                "reports": [duplicate_report],
                "links": [
                    {"report_index": 0, "role": "context"},
                    {"report_index": 0, "role": "context"}
                ]
            })),
            Err(DomainEvidenceError::DuplicateLink { .. })
        ));
    }
}
