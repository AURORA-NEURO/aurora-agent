//! Local single-study evidence-surveillance inference engine (`AFA-ids-P01-F01`).
//!
//! This module turns caller-supplied, institution-local evidence observations into a
//! deterministic, omission-aware researcher artifact. It never retrieves sources, executes
//! tools, exports raw data, or makes a clinical decision.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P01-F01";
pub const CONTRACT_VERSION: &str =
    "ids-local-single-study-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-qualified-evidence-set-1+json";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState1 {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObservation1 {
    pub evidence_id: String,
    pub study_id: String,
    pub scope: String,
    pub source_id: String,
    pub relevance_milli: u16,
    pub evidence_state: EvidenceState1,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub fresh: bool,
    pub authorized: bool,
    pub local_only: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeed1 {
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub scope: String,
    pub semantic_profile: String,
    pub observations: Vec<EvidenceObservation1>,
    pub minimum_relevance_milli: u16,
    pub max_items: usize,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet1Artifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub evidence_digest: ContentHash,
    pub artifact: QualifiedEvidenceSet1Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceInferenceError {
    #[error("invalid local evidence feed or qualified set: {0}")]
    Invalid(String),
    #[error("qualified evidence artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl QualifiedEvidenceSet1 {
    pub fn validate(&self) -> Result<(), EvidenceInferenceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceInferenceError::Invalid(
                "identity, locality, candidate closure, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(EvidenceInferenceError::Invalid(
                    "evidence ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.candidate_order.len() {
            return Err(EvidenceInferenceError::Invalid(
                "candidate ids are duplicated".into(),
            ));
        }
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.blocked_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(EvidenceInferenceError::Invalid(
                "candidate states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.evidence_digest)
            || self.artifact.content_hash != self.evidence_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(EvidenceInferenceError::Artifact(
                "evidence or provenance digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release"
                && !effect.starts_with("read:local-research-artifacts:")
        }) {
            return Err(EvidenceInferenceError::Invalid(
                "effect is outside read-only evidence gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("read:local-research-artifacts:{}", self.study_id)]
        {
            return Err(EvidenceInferenceError::Invalid(
                "qualified read effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(EvidenceInferenceError::Invalid(
                "non-qualified evidence must block".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceInferenceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| EvidenceInferenceError::Artifact(e.to_string()))?,
        )
        .map_err(|e| EvidenceInferenceError::Artifact(e.to_string()))
    }
}

pub fn local_evidence_surveillance_manifest() -> serde_json::Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumer":"computational biologist","behavior":"compute deterministic evidence alerts for one institution-local preclinical study from typed EvidenceFeed1 observations","value":"preserves qualified, unknown, unmeasured, contradicted, omitted, and negative evidence in a replayable researcher artifact","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["read:local-research-artifacts"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY})
}

pub fn infer_local_evidence_surveillance(
    feed: &EvidenceFeed1,
) -> Result<QualifiedEvidenceSet1, EvidenceInferenceError> {
    if feed.request_id.trim().is_empty()
        || feed.study_id.trim().is_empty()
        || feed.intent.trim().is_empty()
        || feed.scope.trim().is_empty()
        || feed.semantic_profile.trim().is_empty()
        || feed.observations.is_empty()
        || feed.max_items == 0
        || !digest(&feed.replay_identity)
        || !feed.raw_data_local
        || feed.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceInferenceError::Invalid(
            "feed identity, locality, replay, bounds, or boundary are invalid".into(),
        ));
    }
    let mut rows = feed.observations.clone();
    rows.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then(a.evidence_id.cmp(&b.evidence_id))
    });
    let candidate_order = rows
        .iter()
        .map(|r| r.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        if row.negative_result {
            negative.insert(format!("{}:negative-result", row.evidence_id));
        }
        let hard = !feed.policy_allow
            || !feed.protected_closure
            || !row.authorized
            || !row.local_only
            || row.study_id != feed.study_id
            || row.scope != feed.scope
            || !digest(&row.content_digest)
            || !digest(&row.provenance_digest);
        let soft = row.replay_identity != feed.replay_identity
            || !row.fresh
            || row.relevance_milli < feed.minimum_relevance_milli
            || !matches!(
                row.evidence_state,
                EvidenceState1::Proven | EvidenceState1::Supported
            );
        if hard || row.evidence_state == EvidenceState1::Contradicted {
            blocked.insert(row.evidence_id.clone());
        } else if soft || index >= feed.max_items {
            unknown.insert(row.evidence_id.clone());
            if index >= feed.max_items {
                omissions.insert(format!("{}:capacity", row.evidence_id));
            }
            uncertainty.insert(format!("{}:unresolved", row.evidence_id));
        } else {
            qualified.insert(row.evidence_id.clone());
        }
    }
    if !feed.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !feed.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    let disposition = if !blocked.is_empty() {
        "blocked"
    } else if !unknown.is_empty() || qualified.is_empty() || !negative.is_empty() {
        "unknown"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("workflow:closure-incomplete".into());
    }
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order":candidate_order,"qualified_order":qualified,"blocked_order":blocked,"unknown_order":unknown,"replay_identity":feed.replay_identity})).map_err(|e| EvidenceInferenceError::Artifact(e.to_string()))?;
    let out = QualifiedEvidenceSet1 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: feed.request_id.clone(),
        study_id: feed.study_id.clone(),
        intent: feed.intent.clone(),
        scope: feed.scope.clone(),
        semantic_profile: feed.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order: candidate_order.clone(),
        qualified_order: qualified.iter().cloned().collect(),
        blocked_order: blocked.iter().cloned().collect(),
        unknown_order: unknown.iter().cloned().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: feed.replay_identity.clone(),
        evidence_digest: evidence_digest.clone(),
        artifact: QualifiedEvidenceSet1Artifact {
            schema_version: SCHEMA_VERSION.into(),
            artifact_id: format!("ids-qualified-evidence:{}", feed.study_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: evidence_digest,
            semantic_loss: Vec::new(),
            provenance_digests: rows.iter().map(|r| r.provenance_digest.clone()).collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("read:local-research-artifacts:{}", feed.study_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn feed() -> EvidenceFeed1 {
        EvidenceFeed1 {
            request_id: "r".into(),
            study_id: "study".into(),
            intent: "evidence".into(),
            scope: "preclinical".into(),
            semantic_profile: "profile:v1".into(),
            observations: vec![EvidenceObservation1 {
                evidence_id: "e1".into(),
                study_id: "study".into(),
                scope: "preclinical".into(),
                source_id: "s1".into(),
                relevance_milli: 900,
                evidence_state: EvidenceState1::Supported,
                content_digest: hash("content"),
                provenance_digest: hash("prov"),
                replay_identity: hash("replay"),
                fresh: true,
                authorized: true,
                local_only: true,
                negative_result: false,
            }],
            minimum_relevance_milli: 500,
            max_items: 4,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            local_evidence_surveillance_manifest()["autonomy_tier"],
            "A0"
        );
    }
    #[test]
    fn qualified() {
        assert_eq!(
            infer_local_evidence_surveillance(&feed())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn unknown_negative() {
        let mut f = feed();
        f.observations[0].negative_result = true;
        assert_eq!(
            infer_local_evidence_surveillance(&f).unwrap().disposition,
            "unknown"
        );
    }
    #[test]
    fn policy_blocks() {
        let mut f = feed();
        f.policy_allow = false;
        assert_eq!(
            infer_local_evidence_surveillance(&f).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            infer_local_evidence_surveillance(&feed()).unwrap(),
            infer_local_evidence_surveillance(&feed()).unwrap()
        );
    }
}
