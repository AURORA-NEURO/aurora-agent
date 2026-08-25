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

fn non_empty_text(value: &Value, field: &str) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty() && text.len() <= MAX_RECEIPT_ID)
        .map(str::to_string)
        .or_else(|| {
            if value.is_string() {
                None
            } else {
                Some(format!("{field} is not text"))
            }
        })
}

fn evidence_present(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Object(object)) => !object.is_empty(),
        Some(_) => true,
    }
}

fn evidence_rows(delivery: &Value) -> Result<Vec<DeliveryReceiptEvidence>, String> {
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
            let ready = readiness
                .get(readiness_name)
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(DeliveryReceiptEvidence {
                name: name.into(),
                present,
                ready,
                digest: if present {
                    Some(digest(value.expect("present evidence has a value"), name)?)
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
            .and_then(|value| non_empty_text(value, "target"))
            .ok_or_else(|| {
                format!("delivery.release_request.targets[{index}].target is invalid")
            })?;
        if !seen.insert(target.clone()) {
            finding(
                findings,
                "duplicate_release_target",
                target.clone(),
                "each requested delivery target must occur exactly once",
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
        let blockers = object
            .get("blockers")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("delivery target {target}.blockers must be an array"))?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    format!("delivery target {target}.blockers must contain strings")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let notes = object
            .get("notes")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("delivery target {target}.notes must be an array"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("delivery target {target}.notes must contain strings"))
            })
            .collect::<Result<Vec<_>, _>>()?;
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
    if request.receipt_id.trim().is_empty()
        || request.receipt_id.len() > MAX_RECEIPT_ID
        || request.receipt_id.chars().any(char::is_control)
    {
        return Err(format!(
            "receipt_id must be non-empty, free of control characters, and at most {MAX_RECEIPT_ID} bytes"
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
    let evidence = evidence_rows(&request.delivery)?;
    let target_value = serde_json::to_value(&targets)
        .map_err(|error| format!("cannot encode receipt targets: {error}"))?;
    let target_digest = digest(&target_value, "receipt targets")?;
    let receipt_seed = json!({
        "schema": DELIVERY_RECEIPT_SCHEMA,
        "workflow": "developer_delivery_receipt",
        "receipt_id": request.receipt_id,
        "delivery_digest": delivery_digest,
        "target_digest": target_digest,
        "targets": targets,
        "evidence": evidence,
        "release_request_ready": release_request_ready,
    });
    let receipt_digest = digest(&receipt_seed, "delivery receipt")?;
    let available_target_count = targets.iter().filter(|row| row.available).count();
    let ready_target_count = targets.iter().filter(|row| row.ready).count();
    let blocked_target_count = targets.len().saturating_sub(ready_target_count);
    let ready_evidence_count = evidence.iter().filter(|row| row.ready).count();
    let structurally_valid = findings.is_empty();
    let release_candidate = structurally_valid && release_request_ready && !targets.is_empty();

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

/// The fields whose comparison carries its own finding code, because a caller needs to tell those
/// mismatches apart. Everything else the recomputation produces is compared under
/// `receipt_projection_mismatch`.
const SEPARATELY_REPORTED_FIELDS: [&str; 6] = [
    "delivery_digest",
    "target_digest",
    "receipt_digest",
    "targets",
    "evidence",
    "release_candidate",
];

/// Recompute a receipt from a stored delivery audit and detect tampering in its projection.
///
/// Every field the recomputation produces is compared against the stored receipt, not only the
/// digests and the target and evidence rows. `receipt_digest` is taken over the receipt's
/// identity, digests, targets, evidence, and readiness flag; it deliberately does not cover the
/// derived counts, the structural verdict, the findings, or the guarantee and limitation text, so
/// a digest that matched would say nothing about whether those had been edited. Comparing the
/// whole projection is what makes an edit to them detectable.
///
/// The comparison is one-directional: a stored receipt may carry fields the recomputation does
/// not, and those are ignored. The shipped MCP surface returns a receipt with transport fields
/// added to the same object, so treating an unrecognised field as tampering would reject every
/// receipt the server hands out. An unrecognised field is therefore not checked at all, which is
/// a bound on what this function proves rather than a judgement that such a field is harmless.
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
    let delivery_digest_match = compare_value(
        &mut findings,
        "delivery_digest_mismatch",
        "delivery_digest",
        receipt.get("delivery_digest"),
        expected_value
            .get("delivery_digest")
            .expect("serialized receipt has delivery digest"),
    );
    let target_digest_match = compare_value(
        &mut findings,
        "target_digest_mismatch",
        "target_digest",
        receipt.get("target_digest"),
        expected_value
            .get("target_digest")
            .expect("serialized receipt has target digest"),
    );
    let supplied_receipt_digest = receipt
        .get("receipt_digest")
        .and_then(Value::as_str)
        .map(str::to_string);
    let receipt_digest_shape_broken = supplied_receipt_digest
        .as_ref()
        .is_some_and(|digest| ContentHash::parse(digest.clone()).is_err());
    let receipt_digest_match = if receipt_digest_shape_broken {
        finding(
            &mut findings,
            "receipt_digest_malformed",
            "receipt_digest",
            "the stored receipt digest is not a lowercase 64-character SHA-256 digest, which is a \
             defect in the claimed digest rather than evidence that the projection moved",
        );
        false
    } else {
        compare_value(
            &mut findings,
            "receipt_digest_mismatch",
            "receipt_digest",
            receipt.get("receipt_digest"),
            expected_value
                .get("receipt_digest")
                .expect("serialized receipt has receipt digest"),
        )
    };
    let targets_match = compare_value(
        &mut findings,
        "targets_mismatch",
        "targets",
        receipt.get("targets"),
        expected_value
            .get("targets")
            .expect("serialized receipt has targets"),
    );
    let evidence_match = compare_value(
        &mut findings,
        "evidence_mismatch",
        "evidence",
        receipt.get("evidence"),
        expected_value
            .get("evidence")
            .expect("serialized receipt has evidence"),
    );
    compare_value(
        &mut findings,
        "readiness_projection_mismatch",
        "release_candidate",
        receipt.get("release_candidate"),
        expected_value
            .get("release_candidate")
            .expect("serialized receipt has release candidate"),
    );
    for (field, recomputed) in expected_value
        .as_object()
        .expect("serialized receipt is an object")
    {
        if SEPARATELY_REPORTED_FIELDS.contains(&field.as_str()) {
            continue;
        }
        compare_value(
            &mut findings,
            "receipt_projection_mismatch",
            field,
            receipt.get(field),
            recomputed,
        );
    }
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

        let mut tampered = stored;
        tampered["targets"][0]["ready"] = json!(false);
        let rejected = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: tampered,
            delivery,
        })
        .unwrap();
        assert!(!rejected.valid);
        assert!(rejected
            .findings
            .iter()
            .any(|finding| finding.code == "targets_mismatch"));
    }

    #[test]
    fn a_field_the_receipt_digest_does_not_cover_is_still_compared_against_the_recomputation() {
        let delivery = delivery(true);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-projection".into(),
            delivery: delivery.clone(),
        })
        .unwrap();
        let stored = serde_json::to_value(&receipt).unwrap();
        for field in [
            "ready_target_count",
            "structurally_valid",
            "verification",
            "limitations",
            "guarantees",
            "findings",
            "schema",
        ] {
            let mut edited = stored.clone();
            edited[field] = json!(match field {
                "ready_target_count" => json!(99),
                "structurally_valid" => json!(false),
                _ => json!("edited after the receipt was built"),
            });
            let verified = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
                receipt: edited,
                delivery: delivery.clone(),
            })
            .unwrap();
            assert!(
                !verified.valid,
                "an edit to {field} survived verification; the receipt digest does not cover it, \
                 so the projection comparison is the only thing that can catch it"
            );
            assert!(verified
                .findings
                .iter()
                .any(|finding| finding.code == "receipt_projection_mismatch"
                    && finding.subject == field));
            assert_eq!(
                verified.recomputed_receipt_digest, receipt.receipt_digest,
                "the digest still matches, which is exactly why comparing it alone was not enough"
            );
        }
    }

    #[test]
    fn a_field_the_recomputation_does_not_produce_is_tolerated_rather_than_checked() {
        let delivery = delivery(true);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-transport".into(),
            delivery: delivery.clone(),
        })
        .unwrap();
        let mut carried = serde_json::to_value(&receipt).unwrap();
        carried["receipt_ready"] = json!(true);
        carried["delivery"] = delivery.clone();
        let verified = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: carried,
            delivery,
        })
        .unwrap();
        assert!(
            verified.valid,
            "a transport that adds fields to the receipt object must not be read as tampering"
        );
    }

    #[test]
    fn a_shape_broken_receipt_digest_is_reported_as_malformed_rather_than_as_a_mismatch() {
        let delivery = delivery(true);
        let receipt = build_delivery_receipt(&DeliveryReceiptRequest {
            receipt_id: "receipt-malformed".into(),
            delivery: delivery.clone(),
        })
        .unwrap();
        let mut broken = serde_json::to_value(&receipt).unwrap();
        broken["receipt_digest"] = json!("NOT-A-DIGEST");
        let verified = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: broken,
            delivery: delivery.clone(),
        })
        .unwrap();
        assert!(!verified.valid);
        assert!(!verified.receipt_digest_match);
        let codes: Vec<&str> = verified
            .findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect();
        assert!(codes.contains(&"receipt_digest_malformed"), "{codes:?}");
        assert!(
            !codes.contains(&"receipt_digest_mismatch"),
            "a digest of the wrong shape is a defect in the claimed digest, not evidence that the \
             projection moved: {codes:?}"
        );

        let mut wrong = serde_json::to_value(&receipt).unwrap();
        wrong["receipt_digest"] = json!("0".repeat(64));
        let verified = verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
            receipt: wrong,
            delivery,
        })
        .unwrap();
        let codes: Vec<&str> = verified
            .findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect();
        assert!(codes.contains(&"receipt_digest_mismatch"), "{codes:?}");
        assert!(!codes.contains(&"receipt_digest_malformed"), "{codes:?}");
    }
}
