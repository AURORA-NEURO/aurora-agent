//! Prospective continuous release-candidate compilation for preclinical glioma programs.
//!
//! The pipeline consumes an ordered local event stream and produces an immutable candidate
//! snapshot. It is deliberately evidence-preserving: negative findings, omissions, schema/policy
//! changes, and missing artifacts become explicit regressions or holds rather than disappearing
//! during promotion. It never signs, uploads, or publishes a candidate.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaContinuousReleaseCandidate1@1";
pub const MAX_EVENTS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePipelineEventKind {
    CandidateSnapshot,
    ExplicitOmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePipelineEvent {
    pub sequence: u64,
    pub event_id: String,
    pub candidate_id: String,
    pub version: String,
    pub event_epoch: u64,
    pub kind: ReleasePipelineEventKind,
    pub program_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub schema_version: String,
    pub policy_digest: ContentHash,
    pub content_digest: ContentHash,
    pub omission_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousReleaseRules {
    pub required_program_order: Vec<String>,
    pub required_artifact_order: Vec<String>,
    pub schema_version: String,
    pub policy_digest: ContentHash,
    pub max_event_age: u64,
    pub require_negative_evidence_field: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePipelinePrior {
    pub version: String,
    pub candidate_digest: ContentHash,
    pub program_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub schema_version: String,
    pub policy_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousReleaseRequest {
    pub candidate_id: String,
    pub now_epoch: u64,
    pub events: Vec<ReleasePipelineEvent>,
    pub rules: ContinuousReleaseRules,
    pub prior: Option<ReleasePipelinePrior>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuousReleaseDisposition {
    ReadyForReview,
    Hold,
    Blocked,
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousReleaseCandidate {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_id: String,
    pub version: String,
    pub event_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub program_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub schema_version: String,
    pub policy_digest: ContentHash,
    pub semantic_diff_order: Vec<String>,
    pub regression_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub disposition: ContinuousReleaseDisposition,
    pub candidate_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContinuousReleaseError {
    #[error("continuous release request is invalid: {0}")]
    InvalidRequest(String),
    #[error("continuous release candidate is invalid: {0}")]
    InvalidOutput(String),
    #[error("continuous release digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn digest_input(candidate: &ContinuousReleaseCandidate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": candidate.feature_id,
        "output_schema": candidate.output_schema,
        "candidate_id": candidate.candidate_id,
        "version": candidate.version,
        "event_order": candidate.event_order,
        "omission_order": candidate.omission_order,
        "program_order": candidate.program_order,
        "artifact_order": candidate.artifact_order,
        "negative_evidence": candidate.negative_evidence,
        "schema_version": candidate.schema_version,
        "policy_digest": candidate.policy_digest,
        "semantic_diff_order": candidate.semantic_diff_order,
        "regression_order": candidate.regression_order,
        "approval_order": candidate.approval_order,
        "disposition": candidate.disposition,
    })
}

impl ContinuousReleaseCandidate {
    pub fn validate(&self) -> Result<(), ContinuousReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.candidate_id)
            || !identifier(&self.version)
            || !canonical(&self.event_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.program_order)
            || !canonical(&self.artifact_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.semantic_diff_order)
            || !canonical(&self.regression_order)
            || !canonical(&self.approval_order)
            || self.schema_version.trim().is_empty()
            || self.policy_digest.as_str().len() != 64
        {
            return Err(ContinuousReleaseError::InvalidOutput(
                "candidate identity, ordering, schema, policy, or digest fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ContinuousReleaseError::Digest(error.to_string()))?;
        if expected != self.candidate_digest {
            return Err(ContinuousReleaseError::InvalidOutput(
                "candidate digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_set(name: &str, values: &[String]) -> Result<(), ContinuousReleaseError> {
    if !canonical(values) || values.iter().any(|value| !identifier(value)) {
        return Err(ContinuousReleaseError::InvalidRequest(format!(
            "{name} must be canonical, unique, and identifier-safe"
        )));
    }
    Ok(())
}

fn validate_request(request: &ContinuousReleaseRequest) -> Result<(), ContinuousReleaseError> {
    if !identifier(&request.candidate_id)
        || request.now_epoch == 0
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || request.rules.schema_version.trim().is_empty()
        || request.rules.policy_digest.as_str().len() != 64
        || request.rules.max_event_age == 0
    {
        return Err(ContinuousReleaseError::InvalidRequest(
            "candidate identity, bounded events, schema, policy, and age limit are required".into(),
        ));
    }
    validate_set(
        "required_program_order",
        &request.rules.required_program_order,
    )?;
    validate_set(
        "required_artifact_order",
        &request.rules.required_artifact_order,
    )?;
    let mut event_ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for event in &request.events {
        if !identifier(&event.event_id)
            || !event_ids.insert(event.event_id.clone())
            || !sequences.insert(event.sequence)
            || event.sequence == 0
            || event.candidate_id != request.candidate_id
            || !identifier(&event.version)
            || event.event_epoch == 0
            || event.schema_version.trim().is_empty()
            || event.policy_digest.as_str().len() != 64
            || event.content_digest.as_str().len() != 64
        {
            return Err(ContinuousReleaseError::InvalidRequest(
                "events must have unique ordered identity, candidate binding, and content digests"
                    .into(),
            ));
        }
        validate_set("event.program_order", &event.program_order)?;
        validate_set("event.artifact_order", &event.artifact_order)?;
        validate_set("event.negative_evidence", &event.negative_evidence)?;
        if matches!(event.kind, ReleasePipelineEventKind::ExplicitOmission)
            && event
                .omission_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(ContinuousReleaseError::InvalidRequest(
                "explicit omission events require a reason".into(),
            ));
        }
    }
    Ok(())
}

/// Compile the latest event-sourced candidate and compare it with the prior immutable edition.
pub fn compile_glioma_continuous_release(
    request: &ContinuousReleaseRequest,
) -> Result<ContinuousReleaseCandidate, ContinuousReleaseError> {
    validate_request(request)?;
    let mut events = request.events.clone();
    events.sort_by_key(|event| event.sequence);
    if events
        .iter()
        .enumerate()
        .any(|(index, event)| event.sequence != (index as u64) + 1)
    {
        return Err(ContinuousReleaseError::InvalidRequest(
            "event sequences must be contiguous from one".into(),
        ));
    }
    let snapshots = events
        .iter()
        .filter(|event| matches!(event.kind, ReleasePipelineEventKind::CandidateSnapshot))
        .collect::<Vec<_>>();
    let latest = snapshots.last().ok_or_else(|| {
        ContinuousReleaseError::InvalidRequest("at least one candidate snapshot is required".into())
    })?;
    let mut event_order = events
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    event_order.sort();
    let omission_order = events
        .iter()
        .filter_map(|event| event.omission_reason.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut semantic_diff = BTreeSet::new();
    let mut regressions = BTreeSet::new();
    let mut approvals = BTreeSet::new();
    if latest.schema_version != request.rules.schema_version {
        regressions.insert("schema-version-regression".into());
    }
    if latest.policy_digest != request.rules.policy_digest {
        regressions.insert("policy-version-regression".into());
    }
    if request.now_epoch.saturating_sub(latest.event_epoch) > request.rules.max_event_age {
        regressions.insert("stale-candidate".into());
    }
    let required_programs = request
        .rules
        .required_program_order
        .iter()
        .collect::<BTreeSet<_>>();
    let present_programs = latest.program_order.iter().collect::<BTreeSet<_>>();
    for missing in required_programs.difference(&present_programs) {
        regressions.insert(format!("missing-program:{missing}"));
    }
    let required_artifacts = request
        .rules
        .required_artifact_order
        .iter()
        .collect::<BTreeSet<_>>();
    let present_artifacts = latest.artifact_order.iter().collect::<BTreeSet<_>>();
    for missing in required_artifacts.difference(&present_artifacts) {
        regressions.insert(format!("missing-artifact:{missing}"));
    }
    if request.rules.require_negative_evidence_field && latest.negative_evidence.is_empty() {
        regressions.insert("negative-evidence-field-empty".into());
    }
    if let Some(prior) = &request.prior {
        if prior.version >= latest.version {
            regressions.insert("version-not-monotonic".into());
        } else {
            semantic_diff.insert(format!("version:{}->{}", prior.version, latest.version));
        }
        if prior.schema_version != latest.schema_version {
            semantic_diff.insert("schema-version-changed".into());
        }
        if prior.policy_digest != latest.policy_digest {
            semantic_diff.insert("policy-digest-changed".into());
        }
        if prior.program_order != latest.program_order {
            semantic_diff.insert("program-coverage-changed".into());
        }
        if prior.artifact_order != latest.artifact_order {
            semantic_diff.insert("artifact-closure-changed".into());
        }
        let old_negative = prior.negative_evidence.iter().collect::<BTreeSet<_>>();
        let new_negative = latest.negative_evidence.iter().collect::<BTreeSet<_>>();
        for dropped in old_negative.difference(&new_negative) {
            regressions.insert(format!("negative-evidence-dropped:{dropped}"));
        }
        if prior.candidate_digest != latest.content_digest {
            semantic_diff.insert("candidate-content-changed".into());
        }
    } else {
        semantic_diff.insert("initial-edition".into());
    }
    if !regressions.is_empty() {
        approvals.insert("accountable-review-required".into());
    } else {
        approvals.insert("independent-release-review-required".into());
    }
    let disposition = if !regressions.is_empty() {
        ContinuousReleaseDisposition::Blocked
    } else if request
        .prior
        .as_ref()
        .is_some_and(|prior| prior.candidate_digest == latest.content_digest)
    {
        ContinuousReleaseDisposition::NoChange
    } else {
        ContinuousReleaseDisposition::ReadyForReview
    };
    let mut output = ContinuousReleaseCandidate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_id: request.candidate_id.clone(),
        version: latest.version.clone(),
        event_order,
        omission_order,
        program_order: latest.program_order.clone(),
        artifact_order: latest.artifact_order.clone(),
        negative_evidence: latest.negative_evidence.clone(),
        schema_version: latest.schema_version.clone(),
        policy_digest: latest.policy_digest.clone(),
        semantic_diff_order: semantic_diff.into_iter().collect(),
        regression_order: regressions.into_iter().collect(),
        approval_order: approvals.into_iter().collect(),
        disposition,
        candidate_digest: ContentHash::of_bytes(b"unsealed-glioma-continuous-release"),
    };
    output.candidate_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ContinuousReleaseError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn event(sequence: u64, kind: ReleasePipelineEventKind) -> ReleasePipelineEvent {
        ReleasePipelineEvent {
            sequence,
            event_id: format!("event-{sequence}"),
            candidate_id: "candidate-a".into(),
            version: "v2".into(),
            event_epoch: 100,
            kind,
            program_order: vec!["p01".into(), "p07".into()],
            artifact_order: vec!["result".into()],
            negative_evidence: vec!["null-effect".into()],
            schema_version: "schema-1".into(),
            policy_digest: hash("policy"),
            content_digest: hash("content"),
            omission_reason: None,
        }
    }

    fn request() -> ContinuousReleaseRequest {
        ContinuousReleaseRequest {
            candidate_id: "candidate-a".into(),
            now_epoch: 100,
            events: vec![event(1, ReleasePipelineEventKind::CandidateSnapshot)],
            rules: ContinuousReleaseRules {
                required_program_order: vec!["p01".into(), "p07".into()],
                required_artifact_order: vec!["result".into()],
                schema_version: "schema-1".into(),
                policy_digest: hash("policy"),
                max_event_age: 10,
                require_negative_evidence_field: true,
            },
            prior: None,
        }
    }

    #[test]
    fn initial_candidate_is_ready_for_review() {
        let output = compile_glioma_continuous_release(&request()).unwrap();
        assert_eq!(
            output.disposition,
            ContinuousReleaseDisposition::ReadyForReview
        );
        assert_eq!(output.semantic_diff_order, vec!["initial-edition"]);
        output.validate().unwrap();
    }

    #[test]
    fn dropped_negative_evidence_blocks_promotion() {
        let mut request = request();
        let mut prior_event = event(1, ReleasePipelineEventKind::CandidateSnapshot);
        prior_event.version = "v1".into();
        prior_event.content_digest = hash("old-content");
        let prior = compile_glioma_continuous_release(&ContinuousReleaseRequest {
            events: vec![prior_event],
            ..request.clone()
        })
        .unwrap();
        request.events[0].version = "v2".into();
        request.events[0].negative_evidence.clear();
        request.events[0].content_digest = hash("new-content");
        request.rules.require_negative_evidence_field = false;
        request.prior = Some(ReleasePipelinePrior {
            version: prior.version,
            candidate_digest: hash("old-content"),
            program_order: prior.program_order,
            artifact_order: prior.artifact_order,
            negative_evidence: vec!["null-effect".into()],
            schema_version: prior.schema_version,
            policy_digest: prior.policy_digest,
        });
        let output = compile_glioma_continuous_release(&request).unwrap();
        assert_eq!(output.disposition, ContinuousReleaseDisposition::Blocked);
        assert!(output
            .regression_order
            .iter()
            .any(|reason| reason.contains("negative-evidence-dropped")));
    }

    #[test]
    fn missing_required_artifact_blocks_candidate() {
        let mut request = request();
        request.rules.required_artifact_order = vec!["figure".into(), "result".into()];
        let output = compile_glioma_continuous_release(&request).unwrap();
        assert_eq!(output.disposition, ContinuousReleaseDisposition::Blocked);
        assert!(output
            .regression_order
            .iter()
            .any(|reason| reason == "missing-artifact:figure"));
    }

    #[test]
    fn explicit_omission_is_retained_and_not_silent() {
        let mut request = request();
        let mut omission = event(2, ReleasePipelineEventKind::ExplicitOmission);
        omission.omission_reason = Some("local-raw-data-remains-local".into());
        request.events.push(omission);
        let output = compile_glioma_continuous_release(&request).unwrap();
        assert_eq!(output.omission_order, vec!["local-raw-data-remains-local"]);
        assert_eq!(output.event_order.len(), 2);
    }

    #[test]
    fn stale_candidate_is_blocked() {
        let mut request = request();
        request.now_epoch = 200;
        let output = compile_glioma_continuous_release(&request).unwrap();
        assert!(output
            .regression_order
            .iter()
            .any(|reason| reason == "stale-candidate"));
    }
}
