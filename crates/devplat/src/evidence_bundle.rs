//! Portable verification for bounded mission evidence bundles.
//!
//! Exporting a JSON bundle is useful only when a caller can independently check that it was not
//! modified after export. This module verifies the bundle's canonical digest, validates the
//! retention contract, and checks the optional retained-result digest. It deliberately does not
//! execute a mission, rerun an evaluator, contact a provider, or turn a valid digest into a
//! scientific, clinical, operational, or release claim.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use thiserror::Error;

pub const MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION: &str = "bioprism-api/mission-evidence-bundle/0.1";
pub const MISSION_EVIDENCE_BUNDLE_VERIFY_SCHEMA_VERSION: &str =
    "bioprism-devplat-mission-evidence-bundle-verify/0.1";
pub const MAX_EVIDENCE_BUNDLE_VERIFY_BYTES: usize = 2 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_TRACE_ROWS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceBundleError {
    #[error("evidence bundle is not an object")]
    NotObject,
    #[error("evidence bundle exceeds the {maximum}-byte verification bound ({actual} bytes)")]
    TooLarge { actual: usize, maximum: usize },
    #[error("evidence bundle is invalid: {reason}")]
    Invalid { reason: String },
    #[error("evidence bundle could not be canonicalised: {0}")]
    Canonicalisation(String),
}

fn text_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, EvidenceBundleError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| {
            let trimmed = value.trim();
            !trimmed.is_empty()
                && value.len() <= MAX_TEXT_BYTES
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| EvidenceBundleError::Invalid {
            reason: format!(
                "{field} must be non-empty, at most {MAX_TEXT_BYTES} bytes, and contain no control characters"
            ),
        })
}

fn identifier_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, EvidenceBundleError> {
    let value = text_field(object, field)?;
    if value != value.trim() {
        return Err(EvidenceBundleError::Invalid {
            reason: format!("{field} must not contain surrounding whitespace"),
        });
    }
    Ok(value)
}

fn bool_field(object: &Map<String, Value>, field: &str) -> Result<bool, EvidenceBundleError> {
    object
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| EvidenceBundleError::Invalid {
            reason: format!("{field} must be a boolean"),
        })
}

fn digest_field(object: &Map<String, Value>, field: &str) -> Result<String, EvidenceBundleError> {
    let value = text_field(object, field)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvidenceBundleError::Invalid {
            reason: format!("{field} must be a lowercase 64-character SHA-256 digest"),
        });
    }
    ContentHash::parse(value.to_string()).map_err(|_| EvidenceBundleError::Invalid {
        reason: format!("{field} must be a lowercase 64-character SHA-256 digest"),
    })?;
    Ok(value.to_string())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn validate_trace_rows(trace: Option<&Value>, trace_included: bool) -> bool {
    match trace {
        Some(Value::Null) => !trace_included,
        Some(Value::Array(rows)) => {
            if rows.len() > MAX_TRACE_ROWS || !trace_included && !rows.is_empty() {
                return false;
            }
            let mut sequences = std::collections::BTreeSet::new();
            let mut previous_sequence = None;
            rows.iter().all(|row| {
                let object = match row.as_object() {
                    Some(object) => object,
                    None => return false,
                };
                let sequence = match object.get("sequence").and_then(Value::as_u64) {
                    Some(sequence) => sequence,
                    None => return false,
                };
                let event = match object.get("event").and_then(Value::as_str) {
                    Some(event) => event,
                    None => return false,
                };
                let trimmed = event.trim();
                let ordered = previous_sequence.is_none_or(|previous| sequence > previous);
                previous_sequence = Some(sequence);
                ordered
                    && sequences.insert(sequence)
                    && !trimmed.is_empty()
                    && event.len() <= MAX_TEXT_BYTES
                    && !event.chars().any(char::is_control)
            })
        }
        _ => false,
    }
}

/// Verify one exported mission evidence bundle without executing any contained workflow.
pub fn verify_mission_evidence_bundle(bundle: &Value) -> Result<Value, EvidenceBundleError> {
    let encoded = serde_json::to_vec(bundle)
        .map_err(|error| EvidenceBundleError::Canonicalisation(error.to_string()))?;
    if encoded.len() > MAX_EVIDENCE_BUNDLE_VERIFY_BYTES {
        return Err(EvidenceBundleError::TooLarge {
            actual: encoded.len(),
            maximum: MAX_EVIDENCE_BUNDLE_VERIFY_BYTES,
        });
    }
    let object = bundle.as_object().ok_or(EvidenceBundleError::NotObject)?;
    if object.get("schema").and_then(Value::as_str) != Some(MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION)
    {
        return Err(EvidenceBundleError::Invalid {
            reason: "schema is not the mission evidence bundle schema".into(),
        });
    }
    if object.get("workflow").and_then(Value::as_str) != Some("mission_evidence_bundle_export") {
        return Err(EvidenceBundleError::Invalid {
            reason: "workflow must be mission_evidence_bundle_export".into(),
        });
    }
    identifier_field(object, "mission_id")?;

    let claimed_bundle_digest = digest_field(object, "bundle_digest")?;
    let mut without_digest = bundle.clone();
    let Some(unsigned_object) = without_digest.as_object_mut() else {
        return Err(EvidenceBundleError::Invalid {
            reason: "bundle is not an object after cloning".into(),
        });
    };
    unsigned_object.remove("bundle_digest");
    let recomputed_bundle_digest = ContentHash::of_value(&without_digest)
        .map_err(|error| EvidenceBundleError::Canonicalisation(error.to_string()))?
        .to_string();
    let bundle_digest_match = claimed_bundle_digest == recomputed_bundle_digest;

    let retention = object
        .get("retention")
        .and_then(Value::as_object)
        .ok_or_else(|| EvidenceBundleError::Invalid {
            reason: "retention must be an object".into(),
        })?;
    let retention_mode = text_field(retention, "mode")?;
    let mode_valid = matches!(retention_mode, "full" | "summary_only");
    let result_retained = bool_field(retention, "result_retained")?;
    let result_included = bool_field(retention, "result_included")?;
    let result = object
        .get("result")
        .ok_or_else(|| EvidenceBundleError::Invalid {
            reason: "result must be present, including when null".into(),
        })?;
    let result_present = !result.is_null();
    let retention_contract_valid = mode_valid
        && result_included == result_present
        && (!result_included || result_retained)
        && (retention_mode != "summary_only" || (!result_retained && !result_present));
    let omission_metadata_valid = match retention.get("result_omitted") {
        None | Some(Value::Null) => true,
        Some(Value::Object(omitted)) => {
            omitted.get("bytes").and_then(Value::as_u64).is_some()
                && omitted
                    .get("sha256")
                    .and_then(Value::as_str)
                    .is_some_and(valid_digest)
        }
        Some(_) => false,
    };

    let claimed_result_digest = match object.get("result_digest") {
        None | Some(Value::Null) => None,
        Some(_) => Some(digest_field(object, "result_digest")?),
    };
    let recomputed_result_digest = if result_present {
        Some(
            ContentHash::of_value(result)
                .map_err(|error| EvidenceBundleError::Canonicalisation(error.to_string()))?
                .to_string(),
        )
    } else {
        None
    };
    let result_digest_match = match (&claimed_result_digest, &recomputed_result_digest) {
        (Some(claimed), Some(recomputed)) => Some(claimed == recomputed),
        (None, Some(_)) => Some(false),
        _ => None,
    };

    let export = object
        .get("export")
        .and_then(Value::as_object)
        .ok_or_else(|| EvidenceBundleError::Invalid {
            reason: "export must be an object".into(),
        })?;
    let export_include_result = bool_field(export, "include_result")?;
    let export_include_trace = bool_field(export, "include_trace")?;
    let trace_included = match export.get("trace_included") {
        None => export_include_trace,
        Some(_) => bool_field(export, "trace_included")?,
    };
    let trace_contract_valid = trace_included == export_include_trace
        && validate_trace_rows(object.get("trace"), trace_included);
    let export_contract_valid = export.get("format").and_then(Value::as_str) == Some("json")
        && export.get("digest_algorithm").and_then(Value::as_str) == Some("sha256")
        && export.get("execution").and_then(Value::as_str) == Some("not_started")
        && export_include_result == result_included
        && trace_included == export_include_trace;

    let mut failures = Vec::new();
    if !bundle_digest_match {
        failures.push("bundle_digest_mismatch");
    }
    if !retention_contract_valid {
        failures.push("retention_contract_invalid");
    }
    if !omission_metadata_valid {
        failures.push("retention_omission_metadata_invalid");
    }
    if result_digest_match == Some(false) {
        failures.push("result_digest_mismatch");
    }
    if !trace_contract_valid {
        failures.push("trace_contract_invalid");
    }
    if !export_contract_valid {
        failures.push("export_contract_invalid");
    }
    let valid = failures.is_empty();
    Ok(json!({
        "ok": true,
        "schema": MISSION_EVIDENCE_BUNDLE_VERIFY_SCHEMA_VERSION,
        "workflow": "mission_evidence_bundle_verify",
        "valid": valid,
        "verification_status": if valid { "verified" } else { "failed" },
        "bundle_digest": claimed_bundle_digest,
        "recomputed_bundle_digest": recomputed_bundle_digest,
        "result_digest": claimed_result_digest,
        "recomputed_result_digest": recomputed_result_digest,
        "checks": {
            "schema": true,
            "bundle_digest": bundle_digest_match,
            "retention": retention_contract_valid,
            "result_digest": result_digest_match,
            "trace": trace_contract_valid,
            "export": export_contract_valid
        },
        "failures": failures,
        "execution": "not_started",
        "guarantees": [
            "canonical bundle and retained-result digests are independently recomputed",
            "verification does not execute a mission, evaluator, domain tool, or external effect",
            "retention and omission claims are checked separately from digest integrity"
        ],
        "limitations": [
            "a valid content digest proves byte-level canonical integrity, not provenance or external storage",
            "verification does not establish scientific, clinical, causal, operational, regulatory, or release truth"
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Value {
        let mut value = json!({
            "schema": MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "workflow": "mission_evidence_bundle_export",
            "mission_id": "mission-verifier",
            "retention": {
                "mode": "summary_only",
                "result_retained": false,
                "result_included": false,
                "summary_retained": true,
                "result_omitted": {"bytes": 300000, "sha256": "a".repeat(64)}
            },
            "result": Value::Null,
            "result_digest": "b".repeat(64),
            "evaluator_replay": {"workflow": "mission_evaluator_replay_summary"},
            "catalog_drift": {"status": "not_recorded"},
            "trace": [{"sequence": 1, "event": "mission_succeeded"}],
            "export": {
                "format": "json",
                "include_result": false,
                "include_trace": true,
                "trace_included": true,
                "digest_algorithm": "sha256",
                "execution": "not_started"
            }
        });
        let digest = ContentHash::of_value(&value).unwrap().to_string();
        value["bundle_digest"] = Value::String(digest);
        value
    }

    #[test]
    fn verifies_canonical_bundle_and_reports_non_executing_posture() {
        let result = verify_mission_evidence_bundle(&bundle()).unwrap();
        assert_eq!(result["workflow"], "mission_evidence_bundle_verify");
        assert_eq!(result["valid"], true);
        assert_eq!(result["checks"]["bundle_digest"], true);
        assert_eq!(result["execution"], "not_started");
    }

    #[test]
    fn detects_tampering_without_rejecting_the_verification_report() {
        let mut tampered = bundle();
        tampered["catalog_drift"]["status"] = Value::String("drifted".into());
        let result = verify_mission_evidence_bundle(&tampered).unwrap();
        assert_eq!(result["valid"], false);
        assert!(result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "bundle_digest_mismatch"));
    }

    #[test]
    fn accepts_legacy_trace_omission_only_when_the_export_flag_agrees() {
        let mut legacy = bundle();
        legacy["trace"] = Value::Null;
        legacy["export"]["include_trace"] = Value::Bool(false);
        legacy["export"]
            .as_object_mut()
            .unwrap()
            .remove("trace_included");
        legacy.as_object_mut().unwrap().remove("bundle_digest");
        let digest = ContentHash::of_value(&legacy).unwrap().to_string();
        legacy["bundle_digest"] = Value::String(digest);
        let result = verify_mission_evidence_bundle(&legacy).unwrap();
        assert_eq!(result["valid"], true);
        assert_eq!(result["checks"]["trace"], true);
    }

    #[test]
    fn rejects_export_flags_that_disagree_with_retention() {
        let mut invalid = bundle();
        invalid["export"]["include_result"] = Value::Bool(true);
        invalid.as_object_mut().unwrap().remove("bundle_digest");
        let digest = ContentHash::of_value(&invalid).unwrap().to_string();
        invalid["bundle_digest"] = Value::String(digest);
        let result = verify_mission_evidence_bundle(&invalid).unwrap();
        assert_eq!(result["valid"], false);
        assert!(result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "export_contract_invalid"));
    }

    #[test]
    fn rejects_noncanonical_identity_and_digest_metadata() {
        let mut invalid = bundle();
        invalid["mission_id"] = Value::String(" mission-verifier".into());
        let error = verify_mission_evidence_bundle(&invalid)
            .expect_err("surrounding identity whitespace must be rejected");
        assert!(error.to_string().contains("surrounding whitespace"));

        let mut invalid_digest = bundle();
        invalid_digest["bundle_digest"] = Value::String("A".repeat(64));
        let error = verify_mission_evidence_bundle(&invalid_digest)
            .expect_err("uppercase digests must be rejected");
        assert!(error.to_string().contains("lowercase 64-character"));
    }

    #[test]
    fn rejects_malformed_present_trace_inclusion_metadata() {
        let mut invalid = bundle();
        invalid["export"]["trace_included"] = Value::String("true".into());
        let error = verify_mission_evidence_bundle(&invalid)
            .expect_err("present trace_included metadata must be boolean");
        assert!(error
            .to_string()
            .contains("trace_included must be a boolean"));
    }

    #[test]
    fn reports_invalid_trace_rows_and_omission_metadata_after_rehash() {
        let mut invalid = bundle();
        invalid["trace"] = json!([
            {"sequence": 1, "event": "mission_succeeded"},
            {"sequence": 1, "event": "mission\ncontinued"}
        ]);
        invalid["retention"]["result_omitted"]["sha256"] = Value::String("A".repeat(64));
        invalid.as_object_mut().unwrap().remove("bundle_digest");
        let digest = ContentHash::of_value(&invalid).unwrap().to_string();
        invalid["bundle_digest"] = Value::String(digest);
        let result = verify_mission_evidence_bundle(&invalid).unwrap();
        assert_eq!(result["valid"], false);
        assert!(result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "trace_contract_invalid"));
        assert!(result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "retention_omission_metadata_invalid"));
    }

    #[test]
    fn rejects_descending_trace_sequences_after_rehash() {
        let mut invalid = bundle();
        invalid["trace"] = json!([
            {"sequence": 2, "event": "mission.completed"},
            {"sequence": 1, "event": "mission.started"}
        ]);
        invalid.as_object_mut().unwrap().remove("bundle_digest");
        let digest = ContentHash::of_value(&invalid).unwrap().to_string();
        invalid["bundle_digest"] = Value::String(digest);
        let result = verify_mission_evidence_bundle(&invalid).unwrap();
        assert_eq!(result["valid"], false);
        assert!(result["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure == "trace_contract_invalid"));
    }
}
