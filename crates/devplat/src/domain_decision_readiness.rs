//! Fail-closed structural readiness for claims that span multiple capability domains.
//!
//! Domain reports and evidence harmonization deliberately stop short of choosing whether a
//! caller should act. That boundary is correct, but an agent still needs a repeatable answer to
//! a narrower question: does the retained evidence packet satisfy the caller's explicitly stated
//! coverage, support, contradiction, review, refusal, and lineage policy? This module answers only
//! that structural question. It never interprets the claim statement, ranks scientific evidence,
//! or upgrades a report into clinical, regulatory, publication, release, or execution authority.
//!
//! The input is the same report/link shape accepted by `domain_evidence_harmonize`, so every
//! capability group can use one gate. Policy is caller-owned and is returned in normalized form;
//! an omitted requirement is never silently treated as satisfied.

use crate::domain_evidence::{
    harmonize_domain_evidence, validate_domain_evidence_harmonization,
    DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const DOMAIN_DECISION_READINESS_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-decision-readiness/0.1";
pub const DOMAIN_DECISION_READINESS_WORKFLOW: &str = "domain_decision_readiness_audit";
pub const MAX_DOMAIN_DECISION_READINESS_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DOMAIN_DECISION_READINESS_REPORTS: usize = 64;
pub const MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS: usize = 64;
pub const MAX_DOMAIN_DECISION_READINESS_BLOCKERS: usize = 256;
pub const MAX_DOMAIN_DECISION_READINESS_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainDecisionReadinessError {
    #[error("domain decision-readiness input must be a JSON object")]
    NotObject,
    #[error("domain decision-readiness field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain decision-readiness field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("domain decision-readiness field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("domain decision-readiness input is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error("domain decision-readiness policy field {field} must be an integer between {minimum} and {maximum}")]
    InvalidPolicyInteger {
        field: String,
        minimum: usize,
        maximum: usize,
    },
    #[error("domain decision-readiness policy field {0} must be a boolean")]
    InvalidPolicyBoolean(String),
    #[error("domain decision-readiness harmonization refused: {0}")]
    Harmonization(String),
    #[error("domain decision-readiness report count is outside the supported bound")]
    InvalidReportCount,
    #[error("domain decision-readiness digest could not be canonicalised: {0}")]
    Canonicalisation(String),
    #[error("domain decision-readiness digest does not match the normalized audit")]
    DigestMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadinessPolicy {
    required_group_ids: Vec<String>,
    required_domains: Vec<String>,
    minimum_supporting_reports: usize,
    minimum_qualifying_reports: usize,
    require_all_reports_linked: bool,
    reject_contradictions: bool,
    reject_refused_reports: bool,
    allow_review_required: bool,
    require_lineage_parents: bool,
}

impl ReadinessPolicy {
    fn from_request(object: &Map<String, Value>) -> Result<Self, DomainDecisionReadinessError> {
        let policy = object
            .get("policy")
            .and_then(Value::as_object)
            .ok_or_else(|| DomainDecisionReadinessError::InvalidField("policy".into()))?;
        let required_group_ids = text_set(
            policy,
            "required_group_ids",
            MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS,
        )?;
        let required_domains = text_set(
            policy,
            "required_domains",
            MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS,
        )?;
        let minimum_supporting_reports = policy_integer(
            policy,
            "minimum_supporting_reports",
            1,
            MAX_DOMAIN_DECISION_READINESS_REPORTS,
            1,
        )?;
        let minimum_qualifying_reports = policy_integer(
            policy,
            "minimum_qualifying_reports",
            0,
            MAX_DOMAIN_DECISION_READINESS_REPORTS,
            0,
        )?;
        Ok(Self {
            required_group_ids,
            required_domains,
            minimum_supporting_reports,
            minimum_qualifying_reports,
            require_all_reports_linked: policy_boolean(policy, "require_all_reports_linked", true)?,
            reject_contradictions: policy_boolean(policy, "reject_contradictions", true)?,
            reject_refused_reports: policy_boolean(policy, "reject_refused_reports", true)?,
            allow_review_required: policy_boolean(policy, "allow_review_required", false)?,
            require_lineage_parents: policy_boolean(policy, "require_lineage_parents", false)?,
        })
    }

    fn as_value(&self) -> Value {
        json!({
            "required_group_ids": self.required_group_ids,
            "required_domains": self.required_domains,
            "minimum_supporting_reports": self.minimum_supporting_reports,
            "minimum_qualifying_reports": self.minimum_qualifying_reports,
            "require_all_reports_linked": self.require_all_reports_linked,
            "reject_contradictions": self.reject_contradictions,
            "reject_refused_reports": self.reject_refused_reports,
            "allow_review_required": self.allow_review_required,
            "require_lineage_parents": self.require_lineage_parents
        })
    }
}

/// Evaluate the caller's explicit structural policy over cross-domain reports.
pub fn audit_domain_decision_readiness(
    request: &Value,
) -> Result<Value, DomainDecisionReadinessError> {
    let object = request
        .as_object()
        .ok_or(DomainDecisionReadinessError::NotObject)?;
    ensure_size(request)?;
    let subject_id = required_text(object, "subject_id")?;
    let claim = object
        .get("claim")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("claim".into()))?;
    if claim.get("id").and_then(Value::as_str).is_none() {
        return Err(DomainDecisionReadinessError::InvalidField(
            "claim.id".into(),
        ));
    }
    let policy = ReadinessPolicy::from_request(object)?;
    let reports = object
        .get("reports")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("reports".into()))?;
    if !(1..=MAX_DOMAIN_DECISION_READINESS_REPORTS).contains(&reports.len()) {
        return Err(DomainDecisionReadinessError::InvalidReportCount);
    }
    let links = object
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("links".into()))?;

    let harmonization_request = json!({
        "subject_id": subject_id,
        "claim": claim,
        "reports": reports,
        "links": links,
        "required_group_ids": policy.required_group_ids,
        "required_domains": policy.required_domains
    });
    let harmonization = harmonize_domain_evidence(&harmonization_request)
        .map_err(|error| DomainDecisionReadinessError::Harmonization(error.to_string()))?;
    let report_rows = harmonization
        .get("reports")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DomainDecisionReadinessError::InvalidField("harmonization.reports".into())
        })?;
    let link_rows = harmonization
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("harmonization.links".into()))?;

    let mut roles_by_digest: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for link in link_rows {
        let digest = required_text_value(link, "report_digest")?;
        let role = required_text_value(link, "role")?;
        roles_by_digest.entry(digest).or_default().insert(role);
    }

    let mut assessments = Vec::with_capacity(report_rows.len());
    let mut supporting = BTreeSet::new();
    let mut qualifying = BTreeSet::new();
    let mut contradicting = BTreeSet::new();
    let mut review_required = BTreeSet::new();
    let mut refused = BTreeSet::new();
    let mut missing_lineage = BTreeSet::new();
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut report_digest_set = BTreeSet::new();

    for row in report_rows {
        let digest = required_text_value(row, "digest")?;
        report_digest_set.insert(digest.clone());
        let status = required_text_value(row, "claim_status")?;
        *status_counts.entry(status.clone()).or_default() += 1;
        let roles = roles_by_digest.get(&digest).cloned().unwrap_or_default();
        if roles.contains("supports") && matches!(status.as_str(), "observed" | "derived") {
            supporting.insert(digest.clone());
        }
        if roles.contains("qualifies") && matches!(status.as_str(), "observed" | "derived") {
            qualifying.insert(digest.clone());
        }
        if roles.contains("contradicts") {
            contradicting.insert(digest.clone());
        }
        if status == "review_required" {
            review_required.insert(digest.clone());
        }
        if status == "refused" {
            refused.insert(digest.clone());
        }
        let lineage_parent_count = row
            .get("lineage_parent_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DomainDecisionReadinessError::InvalidField(
                    "harmonization.reports.lineage_parent_count".into(),
                )
            })?;
        if lineage_parent_count == 0 {
            missing_lineage.insert(digest.clone());
        }
        assessments.push(json!({
            "digest": digest,
            "group_id": row.get("group_id"),
            "domains": row.get("domains"),
            "source_tool": row.get("source_tool"),
            "claim_status": status,
            "link_roles": roles.into_iter().collect::<Vec<_>>(),
            "lineage_parent_count": lineage_parent_count,
            "structural_contribution": contribution(row, &roles_by_digest)
        }));
    }

    let coverage = harmonization.get("coverage").cloned().ok_or_else(|| {
        DomainDecisionReadinessError::InvalidField("harmonization.coverage".into())
    })?;
    let posture = harmonization.get("posture").cloned().ok_or_else(|| {
        DomainDecisionReadinessError::InvalidField("harmonization.posture".into())
    })?;
    let all_reports_linked = coverage
        .get("all_reports_linked")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            DomainDecisionReadinessError::InvalidField("coverage.all_reports_linked".into())
        })?;
    let missing_group_ids = harmonization
        .get("missing_group_ids")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let missing_domains = harmonization
        .get("missing_domains")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let mut blockers = Vec::new();
    if !missing_group_ids.as_array().is_some_and(Vec::is_empty) {
        blockers.push(json!({
            "code": "required_groups_missing",
            "severity": "error",
            "groups": missing_group_ids,
            "message": "one or more required capability groups have no report"
        }));
    }
    if !missing_domains.as_array().is_some_and(Vec::is_empty) {
        blockers.push(json!({
            "code": "required_domains_missing",
            "severity": "error",
            "domains": missing_domains,
            "message": "one or more required domain labels have no report"
        }));
    }
    if policy.require_all_reports_linked && !all_reports_linked {
        blockers.push(json!({
            "code": "reports_unlinked",
            "severity": "error",
            "message": "every report must have an explicit evidence role before readiness can be assessed"
        }));
    }
    if supporting.len() < policy.minimum_supporting_reports {
        blockers.push(json!({
            "code": "support_floor_not_met",
            "severity": "error",
            "observed": supporting.len(),
            "required": policy.minimum_supporting_reports,
            "message": "the explicit support-link floor was not met by observed or derived reports"
        }));
    }
    if qualifying.len() < policy.minimum_qualifying_reports {
        blockers.push(json!({
            "code": "qualification_floor_not_met",
            "severity": "error",
            "observed": qualifying.len(),
            "required": policy.minimum_qualifying_reports,
            "message": "the explicit qualification-link floor was not met"
        }));
    }
    if policy.reject_contradictions && !contradicting.is_empty() {
        blockers.push(json!({
            "code": "contradictory_reports_present",
            "severity": "error",
            "report_digests": contradicting,
            "message": "an explicit contradiction is a fail-closed blocker under this policy"
        }));
    }
    if policy.reject_refused_reports && !refused.is_empty() {
        blockers.push(json!({
            "code": "refused_reports_present",
            "severity": "error",
            "report_digests": refused,
            "message": "a refused report cannot be counted as supporting evidence"
        }));
    }
    if policy.require_lineage_parents && !missing_lineage.is_empty() {
        blockers.push(json!({
            "code": "lineage_parents_missing",
            "severity": "error",
            "report_digests": missing_lineage,
            "message": "every report must declare at least one parent digest under this policy"
        }));
    }

    let review_only = !review_required.is_empty()
        || (!contradicting.is_empty() && !policy.reject_contradictions)
        || (!refused.is_empty() && !policy.reject_refused_reports)
        || (!all_reports_linked && !policy.require_all_reports_linked);
    if !policy.allow_review_required && !review_required.is_empty() {
        blockers.push(json!({
            "code": "human_review_required",
            "severity": "error",
            "report_digests": review_required,
            "message": "review_required claim posture is not admissible under this policy"
        }));
    }
    blockers.truncate(MAX_DOMAIN_DECISION_READINESS_BLOCKERS);

    let policy_blocker = blockers.iter().any(|blocker| {
        matches!(
            blocker.get("code").and_then(Value::as_str),
            Some("contradictory_reports_present")
                | Some("refused_reports_present")
                | Some("human_review_required")
        )
    });
    let decision_state = if policy_blocker {
        "blocked"
    } else if blockers
        .iter()
        .any(|blocker| blocker.get("severity").and_then(Value::as_str) == Some("error"))
    {
        if missing_group_ids
            .as_array()
            .is_some_and(|items| !items.is_empty())
            || missing_domains
                .as_array()
                .is_some_and(|items| !items.is_empty())
            || (!all_reports_linked && policy.require_all_reports_linked)
            || supporting.len() < policy.minimum_supporting_reports
            || qualifying.len() < policy.minimum_qualifying_reports
        {
            "incomplete"
        } else {
            "blocked"
        }
    } else if review_only {
        "review_required"
    } else {
        "ready_for_human_review"
    };
    let policy_satisfied = decision_state == "ready_for_human_review";
    let mut result = json!({
        "schema": DOMAIN_DECISION_READINESS_SCHEMA_VERSION,
        "workflow": DOMAIN_DECISION_READINESS_WORKFLOW,
        "subject_id": subject_id,
        "claim": claim,
        "policy": policy.as_value(),
        "harmonization_schema": DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
        "harmonization": harmonization,
        "report_count": report_rows.len(),
        "report_assessments": assessments,
        "coverage": coverage,
        "posture": posture,
        "counts": {
            "supporting_reports": supporting.len(),
            "qualifying_reports": qualifying.len(),
            "contradicting_reports": contradicting.len(),
            "review_required_reports": review_required.len(),
            "refused_reports": refused.len(),
            "reports_without_lineage_parents": missing_lineage.len(),
            "claim_statuses": status_counts
        },
        "decision_state": decision_state,
        "policy_satisfied": policy_satisfied,
        "blockers": blockers,
        "readiness_claimed": false,
        "execution": "not_started",
        "guarantees": [
            "the decision state is derived only from explicit report fields, link roles, and caller policy",
            "contradictions, refusals, review-required states, missing coverage, and missing lineage remain separately visible",
            "the same policy contract can be applied to any selected capability group or to the complete workspace catalogue"
        ],
        "does_not_claim": [
            "policy satisfaction proves a scientific, clinical, causal, regulatory, publication, or release conclusion",
            "a support link proves that the linked report is true or independently verified",
            "a complete local packet proves external provenance, execution, identity, consent, or authority"
        ]
    });
    result["digest"] = Value::String(digest_for(&result)?);
    ensure_size(&result)?;
    validate_domain_decision_readiness(&result)?;
    Ok(result)
}

/// Validate a retained readiness audit, including its content-addressed digest and nested
/// harmonization shape. This is an integrity check, not a fresh claim interpretation.
pub fn validate_domain_decision_readiness(
    value: &Value,
) -> Result<(), DomainDecisionReadinessError> {
    let object = value
        .as_object()
        .ok_or(DomainDecisionReadinessError::NotObject)?;
    exact_text(object, "schema", DOMAIN_DECISION_READINESS_SCHEMA_VERSION)?;
    exact_text(object, "workflow", DOMAIN_DECISION_READINESS_WORKFLOW)?;
    required_text(object, "subject_id")?;
    if !object.get("claim").is_some_and(Value::is_object) {
        return Err(DomainDecisionReadinessError::InvalidField("claim".into()));
    }
    if object
        .get("claim")
        .and_then(|claim| claim.get("id"))
        .and_then(Value::as_str)
        .is_none()
    {
        return Err(DomainDecisionReadinessError::InvalidField(
            "claim.id".into(),
        ));
    }
    let harmonization = object
        .get("harmonization")
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("harmonization".into()))?;
    validate_domain_evidence_harmonization(harmonization)
        .map_err(|error| DomainDecisionReadinessError::Harmonization(error.to_string()))?;
    let decision_state = exact_one_of(
        object,
        "decision_state",
        &[
            "ready_for_human_review",
            "review_required",
            "incomplete",
            "blocked",
        ],
    )?;
    if object.get("policy_satisfied")
        != Some(&Value::Bool(decision_state == "ready_for_human_review"))
    {
        return Err(DomainDecisionReadinessError::InvalidField(
            "policy_satisfied".into(),
        ));
    }
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(DomainDecisionReadinessError::InvalidField(
            "readiness_claimed".into(),
        ));
    }
    exact_text(object, "execution", "not_started")?;
    let blockers = object
        .get("blockers")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField("blockers".into()))?;
    if blockers.len() > MAX_DOMAIN_DECISION_READINESS_BLOCKERS {
        return Err(DomainDecisionReadinessError::TooManyItems {
            field: "blockers".into(),
            maximum: MAX_DOMAIN_DECISION_READINESS_BLOCKERS,
        });
    }
    let digest = required_text(object, "digest")?;
    if ContentHash::parse(&digest).is_err() || digest_for(value)? != digest {
        return Err(DomainDecisionReadinessError::DigestMismatch);
    }
    ensure_size(value)
}

fn contribution(row: &Value, roles_by_digest: &BTreeMap<String, BTreeSet<String>>) -> &'static str {
    let digest = row
        .get("digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let roles = roles_by_digest.get(digest);
    match (
        roles.is_some_and(|roles| roles.contains("contradicts")),
        roles.is_some_and(|roles| roles.contains("supports")),
        roles.is_some_and(|roles| roles.contains("qualifies")),
    ) {
        (true, _, _) => "contradicting",
        (false, true, _) => "supporting",
        (false, false, true) => "qualifying",
        _ => "context_only",
    }
}

fn policy_integer(
    object: &Map<String, Value>,
    field: &str,
    minimum: usize,
    maximum: usize,
    default: usize,
) -> Result<usize, DomainDecisionReadinessError> {
    let Some(value) = object.get(field) else {
        return Ok(default);
    };
    let value = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| DomainDecisionReadinessError::InvalidPolicyInteger {
            field: field.into(),
            minimum,
            maximum,
        })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(DomainDecisionReadinessError::InvalidPolicyInteger {
            field: field.into(),
            minimum,
            maximum,
        });
    }
    Ok(value)
}

fn policy_boolean(
    object: &Map<String, Value>,
    field: &str,
    default: bool,
) -> Result<bool, DomainDecisionReadinessError> {
    object
        .get(field)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| DomainDecisionReadinessError::InvalidPolicyBoolean(field.into()))
        })
        .unwrap_or(Ok(default))
}

fn text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainDecisionReadinessError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(DomainDecisionReadinessError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| DomainDecisionReadinessError::InvalidField(field.into()))?;
        if text.len() > MAX_DOMAIN_DECISION_READINESS_TEXT_BYTES {
            return Err(DomainDecisionReadinessError::TextTooLarge {
                field: field.into(),
                maximum: MAX_DOMAIN_DECISION_READINESS_TEXT_BYTES,
            });
        }
        result.insert(text.to_string());
    }
    Ok(result.into_iter().collect())
}

fn required_text_value(value: &Value, field: &str) -> Result<String, DomainDecisionReadinessError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField(field.into()))
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainDecisionReadinessError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField(field.into()))?;
    if value.len() > MAX_DOMAIN_DECISION_READINESS_TEXT_BYTES {
        return Err(DomainDecisionReadinessError::TextTooLarge {
            field: field.into(),
            maximum: MAX_DOMAIN_DECISION_READINESS_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), DomainDecisionReadinessError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(DomainDecisionReadinessError::InvalidField(field.into()));
    }
    Ok(())
}

fn exact_one_of<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    values: &[&str],
) -> Result<&'a str, DomainDecisionReadinessError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| values.contains(value))
        .ok_or_else(|| DomainDecisionReadinessError::InvalidField(field.into()))?;
    Ok(value)
}

fn digest_for(value: &Value) -> Result<String, DomainDecisionReadinessError> {
    let mut projection = value.clone();
    projection
        .as_object_mut()
        .ok_or(DomainDecisionReadinessError::NotObject)?
        .remove("digest");
    ContentHash::of_value(&projection)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainDecisionReadinessError::Canonicalisation(error.to_string()))
}

fn ensure_size(value: &Value) -> Result<(), DomainDecisionReadinessError> {
    let actual = serde_json::to_vec(value)
        .map_err(|error| DomainDecisionReadinessError::Canonicalisation(error.to_string()))?
        .len();
    if actual > MAX_DOMAIN_DECISION_READINESS_BYTES {
        return Err(DomainDecisionReadinessError::TooLarge {
            actual,
            maximum: MAX_DOMAIN_DECISION_READINESS_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(subject_id: &str, group_id: &str, domain: &str, status: &str) -> Value {
        json!({
            "schema": "bioprism-devplat-domain-report/0.1",
            "workflow": "domain_report_project",
            "group_id": group_id,
            "domains": [domain],
            "subject_id": subject_id,
            "source_tool": "modality_catalog",
            "report": {"observation": status},
            "claim_posture": {"status": status, "does_not_claim": ["truth"]},
            "parent_digests": ["a".repeat(64)],
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": ["caller supplied"],
            "does_not_claim": ["scientific validity"]
        })
    }

    fn request(reports: Vec<Value>, links: Value) -> Value {
        json!({
            "subject_id": "subject-1",
            "claim": {"id": "claim-1", "statement": "opaque"},
            "reports": reports,
            "links": links,
            "policy": {
                "required_group_ids": ["biological_domains", "biological_ir_and_query"],
                "required_domains": ["modalities", "BioQL syntax"],
                "minimum_supporting_reports": 1,
                "minimum_qualifying_reports": 1,
                "require_lineage_parents": true
            }
        })
    }

    #[test]
    fn a_complete_clean_packet_is_ready_only_for_human_review() {
        let first = report("subject-1", "biological_domains", "modalities", "observed");
        let second = report(
            "subject-1",
            "biological_ir_and_query",
            "BioQL syntax",
            "derived",
        );
        let first_digest = ContentHash::of_value(&first).unwrap().to_string();
        let second_digest = ContentHash::of_value(&second).unwrap().to_string();
        let result = audit_domain_decision_readiness(&request(
            vec![first, second],
            json!([
                {"report_index": 0, "report_digest": first_digest, "role": "supports", "note": "observed support"},
                {"report_index": 1, "report_digest": second_digest, "role": "qualifies", "note": "scope qualifier"}
            ]),
        ))
        .unwrap();
        assert_eq!(result["decision_state"], "ready_for_human_review");
        assert_eq!(result["policy_satisfied"], true);
        validate_domain_decision_readiness(&result).unwrap();
    }

    #[test]
    fn contradictions_and_refusals_never_count_as_support() {
        let refused = report("subject-1", "biological_domains", "modalities", "refused");
        let observed = report(
            "subject-1",
            "biological_ir_and_query",
            "BioQL syntax",
            "observed",
        );
        let refused_digest = ContentHash::of_value(&refused).unwrap().to_string();
        let observed_digest = ContentHash::of_value(&observed).unwrap().to_string();
        let result = audit_domain_decision_readiness(&request(
            vec![refused, observed],
            json!([
                {"report_index": 0, "report_digest": refused_digest, "role": "supports"},
                {"report_index": 1, "report_digest": observed_digest, "role": "contradicts", "note": "explicit disagreement"}
            ]),
        ))
        .unwrap();
        assert_eq!(result["decision_state"], "blocked");
        assert_eq!(result["counts"]["supporting_reports"], 0);
        assert_eq!(result["counts"]["refused_reports"], 1);
        assert_eq!(result["counts"]["contradicting_reports"], 1);
    }

    #[test]
    fn missing_required_coverage_is_incomplete_not_zero_evidence() {
        let observed = report("subject-1", "biological_domains", "modalities", "observed");
        let digest = ContentHash::of_value(&observed).unwrap().to_string();
        let result = audit_domain_decision_readiness(&request(
            vec![observed],
            json!([{"report_index": 0, "report_digest": digest, "role": "supports"}]),
        ))
        .unwrap();
        assert_eq!(result["decision_state"], "incomplete");
        assert_eq!(result["policy_satisfied"], false);
        assert!(result["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| { row["code"] == "required_groups_missing" }));
    }

    #[test]
    fn review_required_can_be_explicitly_allowed_but_stays_visible() {
        let review = report(
            "subject-1",
            "biological_domains",
            "modalities",
            "review_required",
        );
        let observed = report(
            "subject-1",
            "biological_ir_and_query",
            "BioQL syntax",
            "observed",
        );
        let review_digest = ContentHash::of_value(&review).unwrap().to_string();
        let observed_digest = ContentHash::of_value(&observed).unwrap().to_string();
        let mut request = request(
            vec![review, observed],
            json!([
                {"report_index": 0, "report_digest": review_digest, "role": "context"},
                {"report_index": 1, "report_digest": observed_digest, "role": "supports"}
            ]),
        );
        request["policy"]["allow_review_required"] = json!(true);
        request["policy"]["minimum_qualifying_reports"] = json!(0);
        let result = audit_domain_decision_readiness(&request).unwrap();
        assert_eq!(result["decision_state"], "review_required");
        assert_eq!(result["policy_satisfied"], false);
        assert_eq!(result["counts"]["review_required_reports"], 1);
    }

    #[test]
    fn tampering_with_a_retained_digest_is_refused() {
        let observed = report("subject-1", "biological_domains", "modalities", "observed");
        let digest = ContentHash::of_value(&observed).unwrap().to_string();
        let mut result = audit_domain_decision_readiness(&request(
            vec![observed],
            json!([{"report_index": 0, "report_digest": digest, "role": "supports"}]),
        ))
        .unwrap();
        result["digest"] = json!("b".repeat(64));
        assert!(matches!(
            validate_domain_decision_readiness(&result),
            Err(DomainDecisionReadinessError::DigestMismatch)
        ));
    }
}
