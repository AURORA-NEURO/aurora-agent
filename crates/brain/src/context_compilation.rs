//! Local typed research-context compilation with omission certificates.
//!
//! Atlas feature: `AFA-brain-P03-F01`. This compiler produces a deterministic context artifact
//! only from caller-supplied preclinical facts; missing, unsupported, stale, or protected facts
//! remain explicit and are never inferred into closure.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F01";
pub const CONTRACT_VERSION: &str = "brain-research-context-compilation/1.0";
const CONTEXT_CONTENT_TYPE: &str = "application/vnd.aurora.research-context+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact {
    pub fact_id: String,
    pub statement: String,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchContextCompilationRequest {
    pub request_id: String,
    pub objective: String,
    pub scope: String,
    pub required_fact_ids: Vec<String>,
    pub minimum_support_milli: u16,
    pub facts: Vec<ContextFact>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompilationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchContextCompilationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub scope: String,
    pub disposition: ContextCompilationDisposition,
    pub required_fact_order: Vec<String>,
    pub resolved_fact_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub unknown_fact_order: Vec<String>,
    pub context_digest: ContentHash,
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
pub enum ContextCompilationError {
    #[error("invalid research context compilation request: {0}")]
    Invalid(String),
    #[error("research context compilation artifact failed: {0}")]
    Artifact(String),
}

impl ResearchContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.required_fact_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextCompilationError::Invalid(
                "context identity, boundary, required facts, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.objective, "objective"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.required_fact_order, "required_fact_order"),
            (&self.resolved_fact_order, "resolved_fact_order"),
            (&self.missing_fact_order, "missing_fact_order"),
            (&self.blocked_fact_order, "blocked_fact_order"),
            (&self.unknown_fact_order, "unknown_fact_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let required = self
            .required_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let resolved = self
            .resolved_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing = self
            .missing_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked = self
            .blocked_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unknown = self
            .unknown_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = resolved.clone();
        classified.extend(missing.iter().cloned());
        classified.extend(blocked.iter().cloned());
        classified.extend(unknown.iter().cloned());
        if classified != required
            || !resolved.is_disjoint(&missing)
            || !resolved.is_disjoint(&blocked)
            || !resolved.is_disjoint(&unknown)
            || !missing.is_disjoint(&blocked)
            || !missing.is_disjoint(&unknown)
            || !blocked.is_disjoint(&unknown)
        {
            return Err(ContextCompilationError::Invalid(
                "context fact states do not partition required facts".into(),
            ));
        }
        for digest in [&self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextCompilationError::Invalid(
                    "context digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
        ) {
            vec![format!(
                "compile:local-research-context:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextCompilationError::Invalid(
                "context effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ContextCompilationError::Invalid(
                "context compilation receipts must declare local emitted data".into(),
            ));
        }
        let expected_context_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "objective": self.objective,
            "scope": self.scope,
            "disposition": self.disposition,
            "required_fact_order": self.required_fact_order,
            "resolved_fact_order": self.resolved_fact_order,
            "missing_fact_order": self.missing_fact_order,
            "blocked_fact_order": self.blocked_fact_order,
            "unknown_fact_order": self.unknown_fact_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        if self.context_digest != expected_context_digest {
            return Err(ContextCompilationError::Invalid(
                "context digest is not bound to compiled fact state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-research-context:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTEXT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextCompilationError::Invalid(
                "context artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))
    }
}

pub fn context_compilation_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["researcher".into(), "decision-section compiler".into(), "downstream crate context consumer".into()].into(), behavior: "compiles caller-supplied typed preclinical facts into a deterministic local context artifact with explicit omission and evidence-state certificates".into(), value: "turns bounded research intent into reusable context without inventing missing facts or silently closing protected evidence".into(), inputs: vec![TypedPort { name: "research_context_compilation_request".into(), schema: "ResearchContextCompilationRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "research_context_compilation_receipt".into(), schema: "ResearchContextCompilationReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-research-context".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_research_context(
    request: &ResearchContextCompilationRequest,
) -> Result<ResearchContextCompilationReceipt, ContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_fact_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ContextCompilationError::Invalid(
            "context request identity, required facts, replay, or boundary is invalid".into(),
        ));
    }
    let required = request
        .required_fact_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_fact_ids.len()
        || request
            .required_fact_ids
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(ContextCompilationError::Invalid(
            "required fact identities must be non-empty and unique".into(),
        ));
    }
    let mut facts = request.facts.clone();
    facts.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    let mut resolved = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let negative = BTreeSet::new();
    for id in &required {
        match facts.iter().find(|fact| fact.fact_id == *id) {
            None => {
                missing.insert(id.clone());
                omissions.insert(format!("fact:{}:missing", id));
            }
            Some(fact)
                if !request.policy_allow
                    || !request.protected_closure
                    || !request.raw_data_local
                    || !fact.raw_data_local
                    || fact.boundary != PRECLINICAL_BOUNDARY =>
            {
                blocked.insert(id.clone());
                omissions.insert(format!("fact:{}:policy-or-locality-blocked", id));
            }
            Some(fact) if fact.replay_identity != request.replay_identity => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("fact:{}:replay-mismatch", id));
            }
            Some(fact)
                if fact.state == EvidenceState::Supported
                    && fact.support_milli >= request.minimum_support_milli =>
            {
                resolved.insert(id.clone());
            }
            Some(fact)
                if matches!(
                    fact.state,
                    EvidenceState::Unknown | EvidenceState::Speculative
                ) =>
            {
                unknown.insert(id.clone());
                uncertainty.insert(format!("fact:{}:state-unknown", id));
            }
            Some(fact) => {
                blocked.insert(id.clone());
                omissions.insert(format!(
                    "fact:{}:unsupported-or-below-threshold",
                    fact.fact_id
                ));
            }
        }
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            ContextCompilationDisposition::Blocked
        } else if resolved.is_empty() {
            ContextCompilationDisposition::Unknown
        } else if resolved.len() == required.len() {
            ContextCompilationDisposition::Qualified
        } else {
            ContextCompilationDisposition::Partial
        };
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    let effect_receipts = if matches!(
        disposition,
        ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
    ) {
        vec![format!(
            "compile:local-research-context:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let raw_data_local = true;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "scope": request.scope, "disposition": disposition, "required_fact_order": required, "resolved_fact_order": resolved, "missing_fact_order": missing, "blocked_fact_order": blocked, "unknown_fact_order": unknown, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "scope": request.scope, "disposition": disposition, "required_fact_order": required, "resolved_fact_order": resolved, "missing_fact_order": missing, "blocked_fact_order": blocked, "unknown_fact_order": unknown, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-research-context:{}", request.request_id),
        CONTEXT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let receipt = ResearchContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective: request.objective.clone(),
        scope: request.scope.clone(),
        disposition,
        required_fact_order: required.into_iter().collect(),
        resolved_fact_order: resolved.into_iter().collect(),
        missing_fact_order: missing.into_iter().collect(),
        blocked_fact_order: blocked.into_iter().collect(),
        unknown_fact_order: unknown.into_iter().collect(),
        context_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_text(value: &str, field: &str) -> Result<(), ContextCompilationError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ContextCompilationError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ContextCompilationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextCompilationError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), ContextCompilationError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextCompilationError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ResearchContextCompilationReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "objective": receipt.objective,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "required_fact_order": receipt.required_fact_order,
        "resolved_fact_order": receipt.resolved_fact_order,
        "missing_fact_order": receipt.missing_fact_order,
        "blocked_fact_order": receipt.blocked_fact_order,
        "unknown_fact_order": receipt.unknown_fact_order,
        "context_digest": receipt.context_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ResearchContextCompilationRequest {
        ResearchContextCompilationRequest {
            request_id: "request:context".into(),
            objective: "compile mechanism context".into(),
            scope: "organoid:neural".into(),
            required_fact_ids: vec!["fact:a".into(), "fact:b".into()],
            minimum_support_milli: 700,
            facts: vec![
                ContextFact {
                    fact_id: "fact:a".into(),
                    statement: "supported fact".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    evidence_digest: hash("evidence"),
                    provenance_digest: hash("provenance"),
                    artifact_digest: hash("artifact"),
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                ContextFact {
                    fact_id: "fact:b".into(),
                    statement: "supported fact".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    evidence_digest: hash("evidence-b"),
                    provenance_digest: hash("provenance-b"),
                    artifact_digest: hash("artifact-b"),
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            context_compilation_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn compilation_qualifies_complete_context() {
        let receipt = compile_research_context(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            ContextCompilationDisposition::Qualified
        );
        assert_eq!(receipt.resolved_fact_order.len(), 2);
    }
    #[test]
    fn missing_fact_is_unknown() {
        let mut value = request();
        value.facts.pop();
        let receipt = compile_research_context(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial);
        assert_eq!(receipt.missing_fact_order, vec!["fact:b".to_string()]);
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_research_context(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:unsafe-release".to_string()]
        );
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_research_context(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = compile_research_context(&input).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "fact:fact:a:policy-or-locality-blocked"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn non_local_input_emits_metadata_only_receipt() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = compile_research_context(&input).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn context_artifact_payload_is_bound() {
        let mut receipt = compile_research_context(&request()).unwrap();
        receipt.scope = "scope:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
