//! The research dossier: one document chaining every measurement a run produced.
//!
//! Its digest is computed the way the workspace's other receipts compute theirs — over the
//! canonical document with the digest field removed — so any later edit is detectable by
//! recomputation alone. `limitations` is always present and always contains at least
//! [`REQUIRED_LIMITATIONS`]; a dossier without them would imply the runner can do things —
//! biology, literature, acceptance — that it deliberately cannot.
//!
//! # Artifact records and the inline cap
//!
//! Every artifact a step produced is recorded with its canonical sha256 and canonical byte
//! count. An artifact whose canonical form is at most [`INLINE_ARTIFACT_CAP_BYTES`] bytes is
//! embedded in the record under `artifact`; a larger one is recorded digest-only, and the
//! `inlined: false` flag states so. The cap is a stated design decision, not a hidden truncation:
//! nothing is ever partially embedded, because a truncated JSON copy would be a malformed
//! artifact pretending to be the real one. Worlds regenerate deterministically from the request
//! (`bioprism_worldgen::generate` is a pure function of the spec), so a digest-only world is
//! re-derivable and checkable; every figure-source artifact — certificates, comparisons, the
//! sweep table, the diversity document — is orders of magnitude below the cap and always inlines.

use crate::error::ResearchError;
use crate::findings::Finding;
use crate::protocol::ResearchProtocol;
use crate::request::ResearchRequest;
use bioprism_ids::{to_canonical_string, ContentHash};
use serde_json::{json, Map, Value};

pub const DOSSIER_SCHEMA: &str = "bioprism-research/dossier/0.1";

/// Canonical-byte ceiling for embedding an artifact's content in its dossier record.
pub const INLINE_ARTIFACT_CAP_BYTES: usize = 131_072;

/// The limitations every dossier must carry, verbatim. Verification refuses a dossier missing
/// any of them.
pub const REQUIRED_LIMITATIONS: [&str; 7] = [
    "autonomous measurement science over synthetic decision worlds: every measurement in this \
     dossier is over committed fixtures and seeded generators",
    "no biology or medicine, no literature or prior-work coverage, no external-world \
     observation, and no release-level claims from fixture evidence",
    "the question is recorded verbatim and never interpreted: the runner executes the protocol; \
     it does not understand the question",
    "oracle review is a human gate: this runner accepts nothing, approves nothing, and releases \
     nothing",
    "the sweep does not vary decision-defining knobs (skeleton, events, protected set, decision \
     time, policy): they change what the decision is, not the structure around it, and a sweep \
     that varied them would be comparing strategies across different questions",
    "negative findings are first-class results: ties and null separations are reported in the \
     same register as positive findings, and the repository's own headline finding is a tie",
    "research and developer infrastructure: it does not diagnose an individual, recommend \
     treatment, triage care, enroll participants, or claim medical-device functionality",
];

/// The only outcome an emitted dossier may contain.
///
/// One variant on purpose: a step that cannot complete aborts the run with a typed
/// [`ResearchError`] instead of producing a dossier, because a dossier of a partially-run
/// protocol would present the absence of measurements as measurements. Mirrors
/// [`crate::findings::ObservationLevel`], which makes the same move for finding levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Completed,
}

impl StepOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            StepOutcome::Completed => "completed",
        }
    }
}

fn canonicalisation(error: impl std::fmt::Display) -> ResearchError {
    ResearchError::Canonicalisation {
        reason: error.to_string(),
    }
}

/// One artifact's dossier record plus its content digest.
pub struct RecordedArtifact {
    pub record: Value,
    pub digest: String,
}

/// Builds the record for one named artifact, applying the inline cap.
pub fn artifact_record(name: &str, artifact: &Value) -> Result<RecordedArtifact, ResearchError> {
    let canonical = to_canonical_string(artifact).map_err(canonicalisation)?;
    let canonical_bytes = canonical.len();
    let digest = ContentHash::of_bytes(canonical.as_bytes()).to_string();
    let inlined = canonical_bytes <= INLINE_ARTIFACT_CAP_BYTES;
    let mut record = Map::new();
    record.insert("name".into(), json!(name));
    record.insert("sha256".into(), json!(digest));
    record.insert("canonical_bytes".into(), json!(canonical_bytes));
    record.insert("inlined".into(), json!(inlined));
    if inlined {
        record.insert("artifact".into(), artifact.clone());
    }
    Ok(RecordedArtifact {
        record: Value::Object(record),
        digest,
    })
}

/// One executed step's dossier record.
pub fn step_record(
    step_index: usize,
    step: &crate::protocol::ProtocolStep,
    inputs_digests: Value,
    outputs: Vec<Value>,
    outcome: StepOutcome,
) -> Result<Value, ResearchError> {
    let step_value = serde_json::to_value(step).map_err(canonicalisation)?;
    Ok(json!({
        "step_index": step_index,
        "step": step_value,
        "inputs_digests": inputs_digests,
        "outputs": outputs,
        "outcome": outcome.as_str(),
    }))
}

/// Assembles the canonical dossier and stamps its digest.
pub fn build_dossier(
    request: &ResearchRequest,
    protocol: &ResearchProtocol,
    steps: Vec<Value>,
    findings: &[Finding],
) -> Result<Value, ResearchError> {
    let request_value = request.to_document_value()?;
    let request_digest = request.digest()?;
    let protocol_value = serde_json::to_value(protocol).map_err(canonicalisation)?;
    let findings_value = serde_json::to_value(findings).map_err(canonicalisation)?;
    let limitations: Vec<Value> = REQUIRED_LIMITATIONS
        .iter()
        .map(|text| Value::String((*text).into()))
        .collect();
    let mut dossier = json!({
        "schema": DOSSIER_SCHEMA,
        "request": request_value,
        "request_digest": request_digest,
        "protocol": protocol_value,
        "steps": steps,
        "findings": findings_value,
        "limitations": limitations,
        "inline_artifact_cap_bytes": INLINE_ARTIFACT_CAP_BYTES,
    });
    let digest = ContentHash::of_value(&dossier)
        .map_err(canonicalisation)?
        .to_string();
    dossier["dossier_sha256"] = Value::String(digest);
    Ok(dossier)
}

fn artifact_digests(dossier: &Value) -> Vec<String> {
    let mut digests = Vec::new();
    if let Some(steps) = dossier.get("steps").and_then(Value::as_array) {
        for step in steps {
            if let Some(outputs) = step.get("outputs").and_then(Value::as_array) {
                for output in outputs {
                    if let Some(sha) = output.get("sha256").and_then(Value::as_str) {
                        digests.push(sha.to_string());
                    }
                }
            }
        }
    }
    digests
}

/// Recompute the digest and check the structural contract of one research dossier.
///
/// Returns a verification projection rather than a bare boolean so a caller can print exactly
/// which check failed; `valid` is the conjunction. A document that is not even an object, or
/// that claims a different schema, is an error rather than an invalid verification, because
/// there is no research dossier to verify.
///
/// A claimed `dossier_sha256` that is not a 64-character lowercase hex digest fails as
/// `digest_malformed`, distinctly from `digest_match`: a shape defect in the claimed digest is
/// not evidence of tampering, and the projection never reports it as one.
pub fn verify_dossier(dossier: &Value) -> Result<Value, ResearchError> {
    let object = dossier
        .as_object()
        .ok_or_else(|| ResearchError::InvalidDossier {
            reason: "dossier must be a JSON object".into(),
        })?;
    let schema = object.get("schema").and_then(Value::as_str).unwrap_or("");
    if schema != DOSSIER_SCHEMA {
        return Err(ResearchError::InvalidDossier {
            reason: format!("schema is {schema:?}, expected {DOSSIER_SCHEMA:?}"),
        });
    }
    let claimed = object
        .get("dossier_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| ResearchError::InvalidDossier {
            reason: "dossier_sha256 must be a string".into(),
        })?
        .to_string();
    let mut without_digest = dossier.clone();
    without_digest
        .as_object_mut()
        .expect("object checked above")
        .remove("dossier_sha256");
    let recomputed = ContentHash::of_value(&without_digest)
        .map_err(canonicalisation)?
        .to_string();
    let digest_malformed = ContentHash::parse(claimed.clone()).is_err();
    let digest_match = !digest_malformed && claimed == recomputed;

    let request_digest_match = match (
        object.get("request"),
        object.get("request_digest").and_then(Value::as_str),
    ) {
        (Some(request), Some(claimed_request)) => ContentHash::of_value(request)
            .map(|recomputed| recomputed.as_str() == claimed_request)
            .unwrap_or(false),
        _ => false,
    };

    let limitation_texts = object
        .get("limitations")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let missing_limitations = REQUIRED_LIMITATIONS
        .iter()
        .filter(|required| !limitation_texts.iter().any(|text| text == *required))
        .map(|required| Value::String((*required).into()))
        .collect::<Vec<_>>();
    let limitations_present = !limitation_texts.is_empty() && missing_limitations.is_empty();

    let steps_present = object.get("steps").is_some_and(Value::is_array);
    let outcomes_known = object
        .get("steps")
        .and_then(Value::as_array)
        .map(|steps| {
            steps.iter().all(|step| {
                step.get("outcome").and_then(Value::as_str) == Some(StepOutcome::Completed.as_str())
            })
        })
        .unwrap_or(false);

    let findings_array = object.get("findings").and_then(Value::as_array);
    let findings_present = findings_array.is_some();
    let finding_levels_valid = findings_array
        .map(|findings| {
            findings
                .iter()
                .all(|entry| entry.get("level").and_then(Value::as_str) == Some("observation"))
        })
        .unwrap_or(false);
    let known_digests = artifact_digests(dossier);
    let findings_supported = findings_array
        .map(|findings| {
            findings.iter().all(|entry| {
                entry
                    .get("supported_by")
                    .and_then(Value::as_array)
                    .is_some_and(|digests| {
                        !digests.is_empty()
                            && digests.iter().all(|digest| {
                                digest
                                    .as_str()
                                    .is_some_and(|d| known_digests.iter().any(|k| k == d))
                            })
                    })
            })
        })
        .unwrap_or(false);

    let valid = !digest_malformed
        && digest_match
        && request_digest_match
        && limitations_present
        && steps_present
        && outcomes_known
        && findings_present
        && finding_levels_valid
        && findings_supported;
    Ok(json!({
        "schema": DOSSIER_SCHEMA,
        "valid": valid,
        "digest_malformed": digest_malformed,
        "digest_match": digest_match,
        "claimed_dossier_sha256": claimed,
        "recomputed_dossier_sha256": recomputed,
        "request_digest_match": request_digest_match,
        "limitations_present": limitations_present,
        "missing_limitations": missing_limitations,
        "steps_present": steps_present,
        "outcomes_known": outcomes_known,
        "findings_present": findings_present,
        "finding_levels_valid": finding_levels_valid,
        "findings_supported": findings_supported,
    }))
}
