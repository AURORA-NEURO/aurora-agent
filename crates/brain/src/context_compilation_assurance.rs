//! Local context-compilation verification and safety assurance harness.
//!
//! Atlas feature: `AFA-brain-P03-F25`. This product gate verifies typed context
//! candidates and emits witnesses/counterexamples without executing a compiler or
//! making a scientific conclusion.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F25";
pub const CONTRACT_VERSION: &str = "brain-context-compilation-assurance/1.0";
const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceCandidate {
    pub context_id: String,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceRequest {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub candidates: Vec<ContextAssuranceCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub verdict: ContextAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub verification_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextCompilationAssuranceError {
    #[error("invalid context assurance request: {0}")]
    Invalid(String),
    #[error("context assurance artifact failed: {0}")]
    Artifact(String),
}

impl ContextCompilationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ContextCompilationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance identity, witnesses, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(ContextCompilationAssuranceError::Invalid(
                    "context assurance ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified.iter().any(|v| !self.candidate_order.contains(v))
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance outcomes do not partition candidates".into(),
            ));
        }
        for digest in [&self.verification_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextCompilationAssuranceError::Invalid(
                    "context assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("assurance:local-context-compilation:") && e != "block:unsafe-release"
        }) {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance effect is outside the local release gate".into(),
            ));
        }
        let expected_verdict = if !self.blocked_order.is_empty() {
            ContextAssuranceVerdict::Blocked
        } else if !self.unknown_order.is_empty() {
            ContextAssuranceVerdict::Unresolved
        } else {
            ContextAssuranceVerdict::Qualified
        };
        if self.verdict != expected_verdict {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance verdict does not match candidate outcomes".into(),
            ));
        }
        let expected_effect_receipts = if self.verdict == ContextAssuranceVerdict::Qualified {
            vec![format!(
                "assurance:local-context-compilation:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance effect does not match verdict".into(),
            ));
        }
        let expected_verification_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "witness_order": self.witness_order,
            "counterexample_order": self.counterexample_order,
            "verdict": self.verdict,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| ContextCompilationAssuranceError::Artifact(error.to_string()))?;
        if self.verification_digest != expected_verification_digest {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance digest is not bound to candidate outcomes".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-context-compilation-assurance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type
                != "application/vnd.aurora.context-compilation-assurance+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))
    }
}

fn receipt_payload(receipt: &ContextCompilationAssuranceReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "verdict": receipt.verdict,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "witness_order": receipt.witness_order,
        "counterexample_order": receipt.counterexample_order,
        "verification_digest": receipt.verification_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["context compiler engineer".into(), "local release gate".into(), "research workbench".into()].into(), behavior: "verifies typed context candidates with replay, provenance, protected-closure, policy, and locality witnesses".into(), value: "prevents incomplete Decision-Section context from being promoted as a qualified research artifact".into(), inputs: vec![TypedPort { name: "context_compilation_assurance_request".into(), schema: "ContextCompilationAssuranceRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_compilation_assurance_receipt".into(), schema: "ContextCompilationAssuranceResponse1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:context-compilation".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_context_compilation(
    request: &ContextCompilationAssuranceRequest,
) -> Result<ContextCompilationAssuranceReceipt, ContextCompilationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ContextCompilationAssuranceError::Invalid(
            "context assurance request identity, candidates, replay, or boundary is invalid".into(),
        ));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.context_id.cmp(&b.context_id));
    let candidate_order = candidates
        .iter()
        .map(|c| c.context_id.clone())
        .collect::<Vec<_>>();
    if candidate_order.windows(2).any(|p| p[0] == p[1])
        || candidate_order.iter().any(|v| v.trim().is_empty())
    {
        return Err(ContextCompilationAssuranceError::Invalid(
            "context identifiers must be unique and non-empty".into(),
        ));
    }
    for candidate in &candidates {
        if candidate.section_digest.as_str().len() != 64
            || candidate.replay_identity.as_str().len() != 64
            || candidate
                .evidence_digest
                .as_ref()
                .is_some_and(|digest| digest.as_str().len() != 64)
            || candidate
                .provenance_digest
                .as_ref()
                .is_some_and(|digest| digest.as_str().len() != 64)
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context assurance candidate digests must be 64 characters".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-context-contract".to_string(),
        "gate:protected-closure".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow && request.protected_closure && request.raw_data_local;
    for c in &candidates {
        if !global_open
            || !c.policy_allow
            || !c.protected_closure
            || !c.raw_data_local
            || c.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(c.context_id.clone());
            counterexamples.insert(format!(
                "counterexample:{}:policy-protected-closure-locality",
                c.context_id
            ));
        } else if c.replay_identity != request.replay_identity {
            unknown.insert(c.context_id.clone());
            uncertainty.insert(format!("context:{}:replay-mismatch", c.context_id));
        } else if c.evidence_digest.is_none() || c.provenance_digest.is_none() {
            unknown.insert(c.context_id.clone());
            omissions.insert(format!(
                "context:{}:evidence-or-provenance-missing",
                c.context_id
            ));
        } else if matches!(c.state, EvidenceState::Unknown | EvidenceState::Speculative) {
            unknown.insert(c.context_id.clone());
            uncertainty.insert(format!("context:{}:evidence-uncertain", c.context_id));
        } else if matches!(c.state, EvidenceState::Contradicted) {
            blocked.insert(c.context_id.clone());
            negative.insert(format!("context:{}:contradicted", c.context_id));
        } else {
            qualified.insert(c.context_id.clone());
        }
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    if !unknown.is_empty() {
        witnesses.insert("gate:unresolved-context-retained".into());
    }
    let verdict = if !global_open || !blocked.is_empty() {
        ContextAssuranceVerdict::Blocked
    } else if !unknown.is_empty() {
        ContextAssuranceVerdict::Unresolved
    } else {
        ContextAssuranceVerdict::Qualified
    };
    let verification_digest=ContentHash::of_value(&json!({"feature_id":FEATURE_ID,"request_id":request.request_id,"candidate_order":candidate_order,"qualified_order":qualified,"blocked_order":blocked,"unknown_order":unknown,"witness_order":witnesses,"counterexample_order":counterexamples,"verdict":verdict,"replay_identity":request.replay_identity})).map_err(|e|ContextCompilationAssuranceError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"study_id":request.study_id,"scope":request.scope,"verdict":verdict,"candidate_order":candidate_order,"qualified_order":qualified,"blocked_order":blocked,"unknown_order":unknown,"witness_order":witnesses,"counterexample_order":counterexamples,"verification_digest":verification_digest,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-compilation-assurance:{}", request.request_id),
        "application/vnd.aurora.context-compilation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))?;
    let receipt = ContextCompilationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        verdict,
        candidate_order,
        qualified_order: qualified.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(verdict, ContextAssuranceVerdict::Qualified) {
            vec![format!(
                "assurance:local-context-compilation:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request(state: EvidenceState) -> ContextCompilationAssuranceRequest {
        let r = h("context-assurance-replay");
        let c = ContextAssuranceCandidate {
            context_id: "context:one".into(),
            section_digest: r.clone(),
            evidence_digest: Some(r.clone()),
            provenance_digest: Some(r.clone()),
            replay_identity: r.clone(),
            state,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ContextCompilationAssuranceRequest {
            request_id: "request:context-assurance".into(),
            study_id: "study:one".into(),
            scope: "preclinical:organoid".into(),
            candidates: vec![c],
            replay_identity: r,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            context_compilation_assurance_manifest().autonomy_tier,
            AutonomyTier::A0
        )
    }
    #[test]
    fn supported_is_qualified() {
        assert_eq!(
            assure_context_compilation(&request(EvidenceState::Supported))
                .unwrap()
                .verdict,
            ContextAssuranceVerdict::Qualified
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        assert_eq!(
            assure_context_compilation(&request(EvidenceState::Unknown))
                .unwrap()
                .verdict,
            ContextAssuranceVerdict::Unresolved
        )
    }
    #[test]
    fn contradiction_is_blocked() {
        assert_eq!(
            assure_context_compilation(&request(EvidenceState::Contradicted))
                .unwrap()
                .verdict,
            ContextAssuranceVerdict::Blocked
        )
    }
    #[test]
    fn missing_provenance_is_unknown() {
        let mut x = request(EvidenceState::Supported);
        x.candidates[0].provenance_digest = None;
        assert_eq!(
            assure_context_compilation(&x).unwrap().verdict,
            ContextAssuranceVerdict::Unresolved
        )
    }
    #[test]
    fn policy_is_blocked() {
        let mut x = request(EvidenceState::Supported);
        x.policy_allow = false;
        assert_eq!(
            assure_context_compilation(&x).unwrap().verdict,
            ContextAssuranceVerdict::Blocked
        )
    }
    #[test]
    fn non_local_input_still_emits_metadata_only_receipt() {
        let mut x = request(EvidenceState::Supported);
        x.raw_data_local = false;
        let receipt = assure_context_compilation(&x).unwrap();
        assert_eq!(receipt.verdict, ContextAssuranceVerdict::Blocked);
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let r = assure_context_compilation(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap())
    }
}
