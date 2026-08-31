//! Content-addressed identity for bounded, high-throughput research evolution.
//!
//! Atlas feature: `AFA-ids-P32-F31`.
//! This primitive gives adapters, APIs, and federated control planes one stable
//! identity for a candidate, its baseline, replay lineage, and generation. It
//! does not authorize execution or deployment.

use crate::hash::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P32-F31";
pub const CONTRACT_VERSION: &str = "ids-bounded-evolution/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvolutionIdentity {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workflow_id: String,
    pub candidate_id: String,
    pub generation: u32,
    pub parent_digest: Option<ContentHash>,
    pub baseline_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvolutionIdentityError {
    #[error("evolution identity is invalid: {0}")]
    Invalid(String),
    #[error("evolution identity serialization failed: {0}")]
    Serialization(String),
}

impl EvolutionIdentity {
    pub fn new(
        workflow_id: impl Into<String>,
        candidate_id: impl Into<String>,
        generation: u32,
        parent_digest: Option<ContentHash>,
        baseline_digest: ContentHash,
        artifact_digest: ContentHash,
        replay_identity: ContentHash,
    ) -> Result<Self, EvolutionIdentityError> {
        let identity = Self {
            schema_version: "aurora-research-contract/1.0".into(),
            contract_version: CONTRACT_VERSION.into(),
            feature_id: FEATURE_ID.into(),
            workflow_id: workflow_id.into(),
            candidate_id: candidate_id.into(),
            generation,
            parent_digest,
            baseline_digest,
            artifact_digest,
            replay_identity,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        identity.validate()?;
        Ok(identity)
    }

    pub fn validate(&self) -> Result<(), EvolutionIdentityError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.workflow_id.trim().is_empty()
            || self.candidate_id.trim().is_empty()
            || self.generation == 0
            || (self.generation > 1 && self.parent_digest.is_none())
        {
            return Err(EvolutionIdentityError::Invalid(
                "schema, contract, feature, boundary, identity, generation, or parent lineage is incomplete".into(),
            ));
        }
        if self
            .workflow_id
            .chars()
            .chain(self.candidate_id.chars())
            .any(|character| character.is_control())
        {
            return Err(EvolutionIdentityError::Invalid(
                "workflow and candidate identities cannot contain control characters".into(),
            ));
        }
        let forbidden = ["clinical", "diagnosis", "treatment", "triage", "enrollment"];
        let joined = format!("{}:{}", self.workflow_id, self.candidate_id).to_ascii_lowercase();
        if forbidden.iter().any(|term| joined.contains(term)) {
            return Err(EvolutionIdentityError::Invalid(
                "clinical decision surfaces are outside the research identity boundary".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvolutionIdentityError> {
        self.validate()?;
        ContentHash::of_value(&json!(self))
            .map_err(|error| EvolutionIdentityError::Serialization(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    #[test]
    fn identity_is_stable_and_content_addressed() {
        let identity = EvolutionIdentity::new(
            "workflow:high-throughput",
            "candidate:a",
            1,
            None,
            hash("baseline"),
            hash("artifact"),
            hash("replay"),
        )
        .unwrap();
        assert_eq!(identity.digest(), identity.digest());
        assert_eq!(identity.feature_id, FEATURE_ID);
    }

    #[test]
    fn generation_requires_parent_lineage() {
        let result = EvolutionIdentity::new(
            "workflow:high-throughput",
            "candidate:b",
            2,
            None,
            hash("baseline"),
            hash("artifact"),
            hash("replay"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn generation_with_parent_is_valid() {
        let identity = EvolutionIdentity::new(
            "workflow:high-throughput",
            "candidate:b",
            2,
            Some(hash("parent")),
            hash("baseline"),
            hash("artifact"),
            hash("replay"),
        )
        .unwrap();
        assert!(identity.parent_digest.is_some());
    }

    #[test]
    fn clinical_identity_is_rejected() {
        let result = EvolutionIdentity::new(
            "workflow:clinical-decision",
            "candidate:a",
            1,
            None,
            hash("baseline"),
            hash("artifact"),
            hash("replay"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn mutation_of_lineage_changes_digest() {
        let first = EvolutionIdentity::new(
            "workflow:high-throughput",
            "candidate:a",
            1,
            None,
            hash("baseline"),
            hash("artifact"),
            hash("replay"),
        )
        .unwrap();
        let second = EvolutionIdentity::new(
            "workflow:high-throughput",
            "candidate:a",
            1,
            None,
            hash("baseline"),
            hash("artifact-2"),
            hash("replay"),
        )
        .unwrap();
        assert_ne!(first.digest(), second.digest());
    }
}
