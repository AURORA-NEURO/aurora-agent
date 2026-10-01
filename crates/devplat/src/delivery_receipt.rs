//! Content-addressed handoff receipts for cross-domain developer delivery audits.
//!
//! `developer_delivery_audit` is intentionally a live composition of bounded local checks. It
//! is useful to inspect directly, but downstream systems also need a stable object they can join
//! across MCP, REST, SDK, event, and webhook records. This module canonicalizes that audit into a
//! receipt without adding time, randomness, external execution, signatures, or release authority.
//! A receipt is a digest-bound structural handoff, not proof that any provider ran or that a
//! deployment is approved.

use std::collections::BTreeSet;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DELIVERY_RECEIPT_SCHEMA: &str = "bioprism-devplat-delivery-receipt/0.1";
const MAX_RECEIPT_ID: usize = 128;
const MAX_TARGETS: usize = 16;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_TARGET_MESSAGES: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryReceiptRequest {
    pub receipt_id: String,
    pub delivery: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryReceiptVerificationRequest {
    pub receipt: Value,
    pub delivery: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceiptTarget {
    pub target: String,
    pub available: bool,
    pub eligible: bool,
    pub blockers: Vec<String>,
    pub notes: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceiptEvidence {
    pub name: String,
    pub present: bool,
    pub ready: bool,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceiptFinding {
    pub code: String,
    pub severity: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceiptAudit {
    pub schema: String,
    pub workflow: String,
    pub receipt_id: String,
    pub delivery_digest: String,
    pub target_digest: String,
    pub receipt_digest: String,
    pub target_count: usize,
    pub available_target_count: usize,
    pub ready_target_count: usize,
    pub blocked_target_count: usize,
    pub ready_evidence_count: usize,
    pub release_request_ready: bool,
    pub structurally_valid: bool,
    pub release_candidate: bool,
    pub verification: String,
    pub targets: Vec<DeliveryReceiptTarget>,
    pub evidence: Vec<DeliveryReceiptEvidence>,
    pub findings: Vec<DeliveryReceiptFinding>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceiptVerification {
    pub schema: String,
    pub workflow: String,
    pub receipt_id: String,
    pub supplied_receipt_digest: Option<String>,
    pub recomputed_receipt_digest: String,
    pub delivery_digest_match: bool,
    pub target_digest_match: bool,
    pub receipt_digest_match: bool,
    pub targets_match: bool,
    pub evidence_match: bool,
    pub valid: bool,
    pub structurally_valid: bool,
    pub findings: Vec<DeliveryReceiptFinding>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

fn finding(
    findings: &mut Vec<DeliveryReceiptFinding>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
) {
    findings.push(DeliveryReceiptFinding {
        code: code.into(),
        severity: "blocking".into(),
        subject: subject.into(),
        detail: detail.into(),
    });
}

fn digest(value: &Value, label: &str) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("cannot hash {label}: {error}"))
}

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value) && value.len() <= MAX_RECEIPT_ID && value == value.trim()
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn required_text(value: &Value, field: &str) -> Result<String, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("{field} must be text"))?;
    if !valid_text(text) {
        return Err(format!(
            "{field} must be non-empty, at most {MAX_TEXT_BYTES} bytes, and contain no control characters"
        ));
    }
    Ok(text.to_owned())
}

fn evidence_present(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Object(object)) => !object.is_empty(),
        Some(_) => true,
    }
}

fn evidence_rows(
    delivery: &Value,
    findings: &mut Vec<DeliveryReceiptFinding>,
) -> Result<Vec<DeliveryReceiptEvidence>, String> {
    let readiness = delivery
        .get("readiness")
        .and_then(Value::as_object)
        .ok_or("delivery.readiness must be an object")?;
    let fields = [
        ("platform", "platform_checks_clean"),
        ("repository", "repository_scope_clean"),
        ("repository_impact", "repository_impact_clean"),
        ("sdk", "sdk_admission_clean"),
        ("conformance", "conformance_release"),
        ("provider", "provider_capability_gate_cleared"),
        ("governance", "governance_document_clean"),
        ("release", "release_audit_ready"),
        ("ci_evidence", "ci_execution_evidence_ready"),
        ("execution_provenance", "execution_provenance_ready"),
        ("ci_provider_evidence", "ci_provider_evidence_ready"),
    ];
    fields
        .into_iter()
        .map(|(name, readiness_name)| {
            let value = delivery.get(name);
            let present = evidence_present(value);
            let ready = match readiness.get(readiness_name) {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => {
                    return Err(format!(
                        "delivery.readiness.{readiness_name} must be boolean"
                    ));
                }
            };
            if ready && !present {
                finding(
                    findings,
                    "ready_evidence_missing",
                    name,
                    "readiness cannot report evidence as ready when its evidence payload is absent",
                );
            }
            Ok(DeliveryReceiptEvidence {
                name: name.into(),
                present,
                ready,
                digest: if present {
                    let value = value.ok_or_else(|| {
                        format!("delivery.{name} was marked present without a value")
                    })?;
                    Some(digest(value, name)?)
                } else {
                    None
                },
            })
        })
        .collect()
}

fn target_rows(
    delivery: &Value,
    findings: &mut Vec<DeliveryReceiptFinding>,
) -> Result<(Vec<DeliveryReceiptTarget>, bool), String> {
    let request = delivery
        .get("release_request")
        .ok_or("delivery.release_request is required")?;
    let object = request
        .as_object()
        .ok_or("delivery.release_request must be an object")?;
    if object.get("present").and_then(Value::as_bool) != Some(true) {
        finding(
            findings,
            "release_request_not_present",
            "release_request",
            "a receipt requires an explicit delivery release request",
        );
    }
    let raw_targets = object
        .get("targets")
        .and_then(Value::as_array)
        .ok_or("delivery.release_request.targets must be an array")?;
    if raw_targets.is_empty() || raw_targets.len() > MAX_TARGETS {
        return Err(format!(
            "delivery.release_request.targets must contain between 1 and {MAX_TARGETS} entries"
        ));
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::with_capacity(raw_targets.len());
    for (index, raw) in raw_targets.iter().enumerate() {
        let object = raw.as_object().ok_or_else(|| {
            format!("delivery.release_request.targets[{index}] must be an object")
        })?;
        let target = object
            .get("target")
            .ok_or_else(|| format!("delivery.release_request.targets[{index}].target is missing"))
            .and_then(|value| {
                required_text(value, "target")
                    .and_then(|text| {
                        if valid_identifier(&text) {
                            Ok(text)
                        } else {
                            Err("target must be a bounded identifier without surrounding whitespace".into())
                        }
                    })
                    .map_err(|error| {
                        format!("delivery.release_request.targets[{index}].target {error}")
                    })
            })?;
        if !seen.insert(target.to_ascii_lowercase()) {
            finding(
                findings,
                "duplicate_release_target",
                target.clone(),
                "each requested delivery target must occur exactly once, case-insensitively",
            );
        }
        let available = object
            .get("available")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("delivery target {target}.available must be boolean"))?;
        let eligible = object
            .get("eligible")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("delivery target {target}.eligible must be boolean"))?;
        let raw_blockers = object
            .get("blockers")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("delivery target {target}.blockers must be an array"))?;
        if raw_blockers.len() > MAX_TARGET_MESSAGES {
            return Err(format!(
                "delivery target {target}.blockers must contain at most {MAX_TARGET_MESSAGES} entries"
            ));
        }
        let mut blockers = raw_blockers
            .iter()
            .enumerate()
            .map(|(message_index, value)| {
                required_text(
                    value,
                    &format!("delivery target {target}.blockers[{message_index}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        blockers.sort();
        let raw_notes = object
            .get("notes")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("delivery target {target}.notes must be an array"))?;
        if raw_notes.len() > MAX_TARGET_MESSAGES {
            return Err(format!(
                "delivery target {target}.notes must contain at most {MAX_TARGET_MESSAGES} entries"
            ));
        }
        let mut notes = raw_notes
            .iter()
            .enumerate()
            .map(|(message_index, value)| {
                required_text(
                    value,
                    &format!("delivery target {target}.notes[{message_index}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        notes.sort();
        let ready = available && eligible && blockers.is_empty();
        if eligible && !available {
            finding(
                findings,
                "eligible_target_unavailable",
                target.clone(),
                "a delivery target cannot be eligible when its evidence is unavailable",
            );
        }
        if eligible && !blockers.is_empty() {
            finding(
                findings,
                "eligible_target_has_blockers",
                target.clone(),
                "an eligible delivery target cannot retain blockers",
            );
        }
        rows.push(DeliveryReceiptTarget {
            target,
            available,
            eligible,
            blockers,
            notes,
            ready,
        });
    }
    rows.sort_by(|left, right| left.target.cmp(&right.target));
    let computed_ready = rows.iter().all(|row| row.ready);
    let reported_ready = object
        .get("ready")
        .and_then(Value::as_bool)
        .ok_or("delivery.release_request.ready must be boolean")?;
    if computed_ready != reported_ready {
        finding(
            findings,
            "release_readiness_mismatch",
            "release_request.ready",
            "receipt recomputation disagrees with the delivery audit readiness flag",
        );
    }
    Ok((rows, reported_ready))
}

/// Build a deterministic structural receipt from a completed developer-delivery audit.
pub fn build_delivery_receipt(
    request: &DeliveryReceiptRequest,
) -> Result<DeliveryReceiptAudit, String> {
    if !valid_identifier(&request.receipt_id) {
        return Err(format!(
            "receipt_id must be a bounded identifier without surrounding whitespace or control characters (at most {MAX_RECEIPT_ID} bytes)"
        ));
    }
    let object = request
        .delivery
        .as_object()
        .ok_or("delivery must be an object")?;
    if object.get("workflow").and_then(Value::as_str) != Some("developer_delivery_audit") {
        return Err("delivery.workflow must be developer_delivery_audit".into());
    }
    if object.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err("delivery.ok must be true before a receipt can be built".into());
    }
    let delivery_digest = digest(&request.delivery, "delivery audit")?;
    let mut findings = Vec::new();
    let (targets, release_request_ready) = target_rows(&request.delivery, &mut findings)?;
    let evidence = evidence_rows(&request.delivery, &mut findings)?;
    let target_value = serde_json::to_value(&targets)
        .map_err(|error| format!("cannot encode receipt targets: {error}"))?;
    let target_digest = digest(&target_value, "receipt targets")?;
    let available_target_count = targets.iter().filter(|row| row.available).count();
    let ready_target_count = targets.iter().filter(|row| row.ready).count();
    let blocked_target_count = targets.len().saturating_sub(ready_target_count);
    let ready_evidence_count = evidence.iter().filter(|row| row.ready).count();
    let structurally_valid = findings.is_empty();
    let release_candidate = structurally_valid && release_request_ready && !targets.is_empty();
    let receipt_seed = json!({
        "schema": DELIVERY_RECEIPT_SCHEMA,
        "workflow": "developer_delivery_receipt",
        "receipt_id": request.receipt_id,
        "delivery_digest": delivery_digest,
        "target_digest": target_digest,
        "targets": targets,
        "evidence": evidence,
        "findings": findings,
        "target_count": targets.len(),
        "available_target_count": available_target_count,
        "ready_target_count": ready_target_count,
        "blocked_target_count": blocked_target_count,
        "ready_evidence_count": ready_evidence_count,
        "release_request_ready": release_request_ready,
        "structurally_valid": structurally_valid,
        "release_candidate": release_candidate,
    });
    let receipt_digest = digest(&receipt_seed, "delivery receipt")?;

    Ok(DeliveryReceiptAudit {
        schema: DELIVERY_RECEIPT_SCHEMA.into(),
        workflow: "developer_delivery_receipt".into(),
        receipt_id: request.receipt_id.clone(),
        delivery_digest,
        target_digest,
        receipt_digest,
        target_count: targets.len(),
        available_target_count,
        ready_target_count,
        blocked_target_count,
        ready_evidence_count,
        release_request_ready,
        structurally_valid,
        release_candidate,
        verification: "structural_only".into(),
        targets,
        evidence,
        findings,
        guarantees: vec![
            "the receipt digest is derived from the exact delivery audit and canonical target/evidence projections".into(),
            "requested targets, blockers, evidence presence, and readiness remain independently inspectable".into(),
            "target ordering is canonicalized by target name so equivalent delivery sets receive the same target digest".into(),
        ],
        limitations: vec![
            "the receipt does not execute checks, contact providers, verify signatures, or approve deployment".into(),
            "release_candidate is a structural handoff signal for the explicit requested targets, not release authority".into(),
            "durable storage, event publication, consumer acknowledgement, and revocation remain outside this in-memory workflow".into(),
        ],
    })
}

fn compare_value(
    findings: &mut Vec<DeliveryReceiptFinding>,
    code: &str,
    subject: &str,
    supplied: Option<&Value>,
    expected: &Value,
) -> bool {
    if supplied == Some(expected) {
        true
    } else {
        finding(
            findings,
            code,
            subject,
            "stored receipt content does not match the recomputed structural projection",
        );
        false
    }
}

fn compare_digest(
    findings: &mut Vec<DeliveryReceiptFinding>,
    code: &str,
    subject: &str,
    supplied: Option<&Value>,
    expected: &Value,
) -> bool {
    if supplied.and_then(Value::as_str).is_none_or(valid_digest) {
        compare_value(findings, code, subject, supplied, expected)
    } else {
        let malformed_code = code
            .strip_suffix("_mismatch")
            .map(|base| format!("{base}_malformed"))
            .unwrap_or_else(|| format!("{code}_malformed"));
        let noncanonical_code = code
            .strip_suffix("_mismatch")
            .map(|base| format!("{base}_noncanonical"))
            .unwrap_or_else(|| format!("{code}_noncanonical"));
        finding(
            findings,
            &noncanonical_code,
            subject,
            "stored receipt digest must be a lowercase canonical SHA-256 digest",
        );
        finding(
            findings,
            &malformed_code,
            subject,
            "the stored receipt digest has the wrong shape; this is a defect in the claimed digest, not evidence that the projection moved",
        );
        false
    }
}

fn expected_field<'a>(expected: &'a Value, field: &str) -> Result<&'a Value, String> {
    expected
        .get(field)
        .ok_or_else(|| format!("recomputed delivery receipt omitted required field {field:?}"))
}

/// Recompute a receipt from a stored delivery audit and detect tampering in its projection.
pub fn verify_delivery_receipt(
    request: &DeliveryReceiptVerificationRequest,
) -> Result<DeliveryReceiptVerification, String> {
    let receipt = request
        .receipt
        .as_object()
        .ok_or("receipt must be an object")?;
    let receipt_id = receipt
        .get("receipt_id")
        .and_then(Value::as_str)
        .ok_or("receipt.receipt_id must be a string")?;
    let expected = build_delivery_receipt(&DeliveryReceiptRequest {
        receipt_id: receipt_id.into(),
        delivery: request.delivery.clone(),
    })?;
    let expected_value = serde_json::to_value(&expected)
        .map_err(|error| format!("cannot encode recomputed delivery receipt: {error}"))?;
    let mut findings = Vec::new();
    compare_value(
        &mut findings,
        "schema_mismatch",
        "schema",
        receipt.get("schema"),
        expected_field(&expected_value, "schema")?,
    );
    compare_value(
        &mut findings,
        "workflow_mismatch",
        "workflow",
        receipt.get("workflow"),
        expected_field(&expected_value, "workflow")?,
    );
    compare_value(
        &mut findings,
        "receipt_id_mismatch",
        "receipt_id",
        receipt.get("receipt_id"),
        expected_field(&expected_value, "receipt_id")?,
    );
    let delivery_digest_match = compare_digest(
        &mut findings,
        "delivery_digest_mismatch",
        "delivery_digest",
        receipt.get("delivery_digest"),
        expected_field(&expected_value, "delivery_digest")?,
    );
    let target_digest_match = compare_digest(
        &mut findings,
        "target_digest_mismatch",
        "target_digest",
        receipt.get("target_digest"),
        expected_field(&expected_value, "target_digest")?,
    );
    let receipt_digest_match = compare_digest(
        &mut findings,
        "receipt_digest_mismatch",
        "receipt_digest",
        receipt.get("receipt_digest"),
        expected_field(&expected_value, "receipt_digest")?,
    );
    let targets_match = compare_value(
        &mut findings,
        "targets_mismatch",
        "targets",
        receipt.get("targets"),
        expected_field(&expected_value, "targets")?,
    );
    let evidence_match = compare_value(
        &mut findings,
        "evidence_mismatch",
        "evidence",
        receipt.get("evidence"),
        expected_field(&expected_value, "evidence")?,
    );
    compare_value(
        &mut findings,
        "readiness_projection_mismatch",
        "release_candidate",
        receipt.get("release_candidate"),
        expected_field(&expected_value, "release_candidate")?,
    );
    for field in [
        "target_count",
        "available_target_count",
        "ready_target_count",
        "blocked_target_count",
        "ready_evidence_count",
        "release_request_ready",
        "structurally_valid",
        "findings",
    ] {
        compare_value(
            &mut findings,
            "receipt_summary_mismatch",
            field,
            receipt.get(field),
            expected_field(&expected_value, field)?,
        );
    }
    let receipt_projection = canonical_receipt_value(
        &request.receipt,
        &expected,
        &request.delivery,
        &mut findings,
    )?;
    compare_value(
        &mut findings,
        "receipt_projection_mismatch",
        "receipt",
        Some(&receipt_projection),
        &expected_value,
    );
    let supplied_receipt_digest = receipt
        .get("receipt_digest")
        .and_then(Value::as_str)
        .map(str::to_string);
    let valid = findings.is_empty();
    Ok(DeliveryReceiptVerification {
        schema: DELIVERY_RECEIPT_SCHEMA.into(),
        workflow: "developer_delivery_receipt_verify".into(),
        receipt_id: receipt_id.into(),
        supplied_receipt_digest,
        recomputed_receipt_digest: expected.receipt_digest,
        delivery_digest_match,
        target_digest_match,
        receipt_digest_match,
        targets_match,
        evidence_match,
        valid,
        structurally_valid: valid && expected.structurally_valid,
        findings,
        guarantees: vec![
            "the delivery audit is supplied separately and the receipt projection is recomputed before comparison".into(),
            "digest, target, evidence, and readiness mismatches remain separately identifiable".into(),
            "verification is deterministic and does not depend on time, network access, or provider contact".into(),
        ],
        limitations: vec![
            "verification proves structural consistency with the supplied delivery audit, not the truth of external execution".into(),
            "the route does not verify signatures, fetch logs, execute checks, or provide durable revocation".into(),
        ],
    })
}

fn canonical_receipt_value(
    supplied: &Value,
    expected: &DeliveryReceiptAudit,
    delivery: &Value,
    findings: &mut Vec<DeliveryReceiptFinding>,
) -> Result<Value, String> {
    let mut projection = supplied.clone();
    let object = projection
        .as_object_mut()
        .ok_or("receipt must be an object")?;
    for (field, expected_value) in [
        ("ok", json!(true)),
        ("valid", json!(expected.structurally_valid)),
        ("receipt_ready", json!(expected.release_candidate)),
        ("delivery", delivery.clone()),
    ] {
        if let Some(supplied_value) = object.get(field) {
            compare_value(
                findings,
                "receipt_envelope_mismatch",
                field,
                Some(supplied_value),
                &expected_value,
            );
            object.remove(field);
        }
    }
    // `__isError` is MCP test/client transport metadata added beside the serialized payload; it
    // is not part of the receipt contract and must not change the canonical receipt projection.
    object.remove("__isError");
    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delivery(ready: bool) -> Value {
        json!({
            "ok": true,
            "workflow": "developer_delivery_audit",
            "platform": {"ok": true},
            "repository": {"ok": true},
            "repository_impact": null,
            "sdk": null,
            "conformance": {"ok": true},
            "provider": null,
            "governance": null,
            "release": null,
            "ci_evidence": null,
            "execution_provenance": {"provenance_ready": ready},
            "readiness": {
                "platform_checks_clean": true,
                "repository_scope_clean": true,
                "repository_impact_clean": false,
                "sdk_admission_clean": false,
                "conformance_release": false,
                "provider_capability_gate_cleared": false,
                "governance_document_clean": false,
                "release_audit_ready": false,
                "ci_execution_evidence_ready": false,
                "execution_provenance_ready": ready,
            },
            "release_request": {
                "present": true,
                "id": "delivery-1",
                "targets": [{"target": "execution_provenance", "available": true, "eligible": ready, "blockers": if ready { json!([]) } else { json!(["execution_provenance_not_ready"]) }, "notes": []}],
                "ready": ready,
            }
        })
    }

    #[test]
    fn receipt_is_digest_bound_and_ready_only_for_a_clean_requested_target() {
        let ready = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-1".into(),
            delivery: delivery(true),
        })
        .unwrap();
        assert!(ready.structurally_valid);
        assert!(ready.release_candidate);
        assert_eq!(ready.target_count, 1);
        assert_eq!(ready.ready_target_count, 1);
        assert_eq!(ready.evidence[9].name, "execution_provenance");
        assert!(ready.evidence[9].present);
        assert!(ready.evidence[9].ready);

        let blocked = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-1".into(),
            delivery: delivery(false),
        })
        .unwrap();
        assert!(blocked.structurally_valid);
        assert!(!blocked.release_candidate);
        assert_eq!(blocked.blocked_target_count, 1);
        assert_ne!(ready.delivery_digest, blocked.delivery_digest);
    }

    #[test]
    fn receipt_rejects_forged_target_readiness_and_duplicate_names() {
        let mut value = delivery(true);
        value["release_request"]["ready"] = json!(false);
        value["release_request"]["targets"] = json!([
            {"target": "execution_provenance", "available": true, "eligible": true, "blockers": [], "notes": []},
            {"target": "execution_provenance", "available": true, "eligible": true, "blockers": [], "notes": []}
        ]);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-2".into(),
            delivery: value,
        })
        .unwrap();
        assert!(!receipt.structurally_valid);
        assert!(!receipt.release_candidate);
        assert!(receipt
            .findings
            .iter()
            .any(|finding| finding.code == "duplicate_release_target"));
        assert!(receipt
            .findings
            .iter()
            .any(|finding| finding.code == "release_readiness_mismatch"));
    }

    #[test]
    fn receipt_rejects_case_collisions_control_text_and_ready_missing_evidence() {
        let mut ambiguous = delivery(true);
        ambiguous["release_request"]["targets"] = json!([
            {"target": "Execution_Provenance", "available": true, "eligible": true, "blockers": [], "notes": []},
            {"target": "execution_provenance", "available": true, "eligible": true, "blockers": [], "notes": []}
        ]);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-case-collision".into(),
            delivery: ambiguous,
        })
        .unwrap();
        assert!(!receipt.structurally_valid);
        assert!(receipt
            .findings
            .iter()
            .any(|finding| finding.code == "duplicate_release_target"));

        let mut control_text = delivery(true);
        control_text["release_request"]["targets"][0]["notes"] = json!(["unsafe\nannotation"]);
        let error = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-control-text".into(),
            delivery: control_text,
        })
        .expect_err("control-bearing target metadata must be rejected");
        assert!(error.contains("no control characters"));

        let mut whitespace_text = delivery(true);
        whitespace_text["release_request"]["targets"][0]["notes"] = json!([" unsafe note"]);
        let error = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-whitespace-text".into(),
            delivery: whitespace_text,
        })
        .expect_err("surrounding message whitespace must be rejected");
        assert!(error.contains("no control characters"));

        let mut missing_evidence = delivery(true);
        missing_evidence["readiness"]["ci_provider_evidence_ready"] = json!(true);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-missing-evidence".into(),
            delivery: missing_evidence,
        })
        .unwrap();
        assert!(!receipt.structurally_valid);
        assert!(receipt
            .findings
            .iter()
            .any(|finding| finding.code == "ready_evidence_missing"));
    }

    #[test]
    fn receipt_target_digest_is_independent_of_message_order() {
        let mut first = delivery(false);
        first["release_request"]["targets"][0]["notes"] = json!(["z-note", "a-note"]);
        let mut second = first.clone();
        second["release_request"]["targets"][0]["notes"] = json!(["a-note", "z-note"]);

        let first = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-message-order".into(),
            delivery: first,
        })
        .unwrap();
        let second = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-message-order".into(),
            delivery: second,
        })
        .unwrap();
        assert_eq!(first.target_digest, second.target_digest);
        assert_ne!(first.delivery_digest, second.delivery_digest);
    }

    #[test]
    fn receipt_verification_recomputes_and_detects_projection_tampering() {
        let delivery = delivery(true);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-verify".into(),
            delivery: delivery.clone(),
        })
        .unwrap();
        let stored = serde_json::to_value(&receipt).unwrap();
        let verified = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: stored.clone(),
            delivery: delivery.clone(),
        })
        .unwrap();
        assert!(verified.valid);
        assert!(verified.structurally_valid);
        assert_eq!(verified.recomputed_receipt_digest, receipt.receipt_digest);

        let mut envelope = stored.clone();
        envelope["ok"] = json!(true);
        envelope["valid"] = json!(receipt.structurally_valid);
        envelope["receipt_ready"] = json!(receipt.release_candidate);
        envelope["delivery"] = delivery.clone();
        let verified_envelope = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: envelope.clone(),
            delivery: delivery.clone(),
        })
        .unwrap();
        assert!(verified_envelope.valid);

        envelope["delivery"]["workflow"] = json!("tampered");
        let rejected_envelope = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: envelope,
            delivery: delivery.clone(),
        })
        .unwrap();
        assert!(!rejected_envelope.valid);
        assert!(rejected_envelope
            .findings
            .iter()
            .any(|finding| finding.code == "receipt_envelope_mismatch"));

        let mut tampered = stored;
        tampered["targets"][0]["ready"] = json!(false);
        let rejected = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: tampered,
            delivery: delivery.clone(),
        })
        .unwrap();
        assert!(!rejected.valid);
        assert!(rejected
            .findings
            .iter()
            .any(|finding| finding.code == "targets_mismatch"));

        let mut malformed = serde_json::to_value(&receipt).unwrap();
        malformed["schema"] = json!("wrong-schema");
        malformed["receipt_digest"] = json!(receipt.receipt_digest.to_uppercase());
        let rejected = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: malformed,
            delivery,
        })
        .unwrap();
        assert!(!rejected.valid);
        assert!(rejected
            .findings
            .iter()
            .any(|finding| finding.code == "schema_mismatch"));
        assert!(rejected
            .findings
            .iter()
            .any(|finding| finding.code == "receipt_digest_noncanonical"));
    }
}
