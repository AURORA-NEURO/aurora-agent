//! Federated continual context compilation with aggregate-only exchange.
//!
//! Atlas feature: `AFA-brain-P03-F04`. The compiler proves local context
//! qualification before producing digest-only federation envelopes; raw study
//! observations never become an export side effect.

use crate::context_compilation::ContextCompilationDisposition;
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F04";
pub const CONTRACT_VERSION: &str = "brain-federated-context-compilation/1.0";
const CONTEXT_CONTENT_TYPE: &str = "application/vnd.aurora.federated-context+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextCandidate {
    pub context_id: String,
    pub study_id: String,
    pub modality: String,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextCompilationRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub required_context_ids: Vec<String>,
    pub minimum_support_milli: u16,
    pub candidates: Vec<FederatedContextCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextCompilationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: ContextCompilationDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContextCompilationError {
    #[error("invalid federated context compilation request: {0}")]
    Invalid(String),
    #[error("federated context compilation artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), FederatedContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context identity, closure, aggregate-only locality, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.study_order, "study_order"),
            (&self.modality_order, "modality_order"),
            (&self.candidate_order, "candidate_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.aggregate_order, "aggregate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != candidates
            || !identity_keys(&self.qualified_order)
                .is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.qualified_order)
                .is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.blocked_order).is_disjoint(&identity_keys(&self.unknown_order))
        {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context candidate states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.context_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextCompilationError::Invalid(
                    "federated context digest is invalid".into(),
                ));
            }
        }
        if self.aggregate_order.iter().any(|digest| digest.len() != 64) {
            return Err(FederatedContextCompilationError::Invalid(
                "federated aggregate order must contain digest-only entries".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("manage:local-federated-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context effect is outside local management gate".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
        ) {
            vec![format!(
                "manage:local-federated-context:{}",
                self.federation_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context receipts must declare local emitted data".into(),
            ));
        }
        if !self.aggregate_only
            && (self.disposition != ContextCompilationDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "request:aggregate-only-required"))
        {
            return Err(FederatedContextCompilationError::Invalid(
                "non-aggregate federated context must be blocked and retain release evidence"
                    .into(),
            ));
        }
        let expected_comparability_digest = ContentHash::of_value(&json!({
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "semantic_profile": self.semantic_profile,
        }))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
        if self.comparability_digest != expected_comparability_digest {
            return Err(FederatedContextCompilationError::Invalid(
                "federated comparability digest is not bound to study and modality closure".into(),
            ));
        }
        let expected_context_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "comparability_digest": self.comparability_digest,
            "disposition": self.disposition,
            "replay_identity": self.replay_identity,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
        }))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
        if self.context_digest != expected_context_digest {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context digest is not bound to candidate outcomes".into(),
            ));
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "endpoint": self.endpoint,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "aggregate_order": self.aggregate_order,
            "comparability_digest": self.comparability_digest,
            "context_digest": self.context_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
            "boundary": self.boundary,
        }))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope_digest {
            return Err(FederatedContextCompilationError::Invalid(
                "federated envelope digest is not bound to release metadata".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-context:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTEXT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))
    }
}

pub fn federated_context_compilation_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["federation steward".into(), "context compiler".into(), "research consortium".into()].into(),
        behavior: "compiles comparable local context and emits digest-only federated envelope metadata with explicit authority and omission evidence".into(),
        value: "enables continual consortium context exchange without moving raw preclinical observations or hiding site-local uncertainty".into(),
        inputs: vec![TypedPort { name: "federated_context_compilation_request".into(), schema: "FederatedContextCompilationRequest1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_context_compilation_receipt".into(), schema: "FederatedContextCompilationReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::FederationExport, Effect::WriteLocalArtifact].into(),
        permissions: ["manage:local-federated-context".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ro-crate-specification".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federation context approver".into(), reason: "authorize purpose-bound aggregate-only exchange after comparability, signer, policy, closure, and locality gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_federated_context(
    request: &FederatedContextCompilationRequest,
) -> Result<FederatedContextCompilationReceipt, FederatedContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.required_context_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(FederatedContextCompilationError::Invalid(
            "federated context identity, closure, replay, or boundary is invalid".into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.federation_id, "federation_id"),
        (&request.institution_id, "institution_id"),
        (&request.purpose, "purpose"),
        (&request.semantic_profile, "semantic_profile"),
        (&request.endpoint, "endpoint"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.study_ids, "study_ids")?;
    validate_unique(&request.required_modalities, "required_modalities")?;
    validate_unique(&request.required_context_ids, "required_context_ids")?;
    let studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_context_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if studies.len() != request.study_ids.len()
        || modalities.len() != request.required_modalities.len()
        || required.len() != request.required_context_ids.len()
    {
        return Err(FederatedContextCompilationError::Invalid(
            "federated context identities must be unique and non-empty".into(),
        ));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.context_id.cmp(&right.context_id));
    let mut candidate_map = std::collections::BTreeMap::new();
    let mut candidate_keys = BTreeSet::new();
    for candidate in candidates {
        for (value, field) in [
            (&candidate.context_id, "candidate.context_id"),
            (&candidate.study_id, "candidate.study_id"),
            (&candidate.modality, "candidate.modality"),
            (&candidate.boundary, "candidate.boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (digest, field) in [
            (&candidate.evidence_digest, "candidate.evidence_digest"),
            (&candidate.provenance_digest, "candidate.provenance_digest"),
            (&candidate.replay_identity, "candidate.replay_identity"),
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextCompilationError::Invalid(format!(
                    "{field} must be a 64-character content hash"
                )));
            }
        }
        if !candidate_keys.insert(candidate.context_id.to_ascii_lowercase()) {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context candidates must be unique and case-distinct".into(),
            ));
        }
        if candidate_map
            .insert(candidate.context_id.clone(), candidate)
            .is_some()
        {
            return Err(FederatedContextCompilationError::Invalid(
                "federated context candidates must be unique".into(),
            ));
        }
    }
    let candidate_order = required.iter().cloned().collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut aggregate_order = BTreeSet::new();
    for id in &required {
        match candidate_map.get(id) {
            None => {
                unknown.insert(id.clone());
                omissions.insert(format!("context:{}:missing-at-institution", id));
            }
            Some(candidate)
                if !request.policy_allow
                    || !request.protected_closure
                    || !request.signed_approval
                    || !request.raw_data_local
                    || !request.aggregate_only
                    || !candidate.raw_data_local
                    || candidate.boundary != PRECLINICAL_BOUNDARY
                    || !studies.contains(&candidate.study_id)
                    || !modalities.contains(&candidate.modality) =>
            {
                blocked.insert(id.clone());
                omissions.insert(format!("context:{}:federation-gate-blocked", id));
            }
            Some(candidate) if candidate.replay_identity != request.replay_identity => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("context:{}:replay-mismatch", id));
            }
            Some(candidate)
                if candidate.state == EvidenceState::Supported
                    && candidate.support_milli >= request.minimum_support_milli =>
            {
                qualified.insert(id.clone());
                let digest = ContentHash::of_value(&json!({"context_id": id, "study_id": candidate.study_id, "modality": candidate.modality, "support_milli": candidate.support_milli, "evidence_digest": candidate.evidence_digest, "provenance_digest": candidate.provenance_digest}))
                    .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
                aggregate_order.insert(digest.to_string());
            }
            Some(candidate)
                if matches!(
                    candidate.state,
                    EvidenceState::Unknown | EvidenceState::Speculative
                ) =>
            {
                unknown.insert(id.clone());
                uncertainty.insert(format!("context:{}:evidence-state-unknown", id));
            }
            Some(candidate) => {
                blocked.insert(id.clone());
                omissions.insert(format!(
                    "context:{}:unsupported-or-below-threshold",
                    candidate.context_id
                ));
            }
        }
    }
    let locality_failure = !request.raw_data_local
        || required
            .iter()
            .filter_map(|id| candidate_map.get(id))
            .any(|candidate| !candidate.raw_data_local);
    if locality_failure {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    if !request.aggregate_only {
        omissions.insert("request:aggregate-only-required".into());
    }
    let raw_data_local = true;
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || locality_failure
        || !request.aggregate_only
    {
        ContextCompilationDisposition::Blocked
    } else if qualified.is_empty() {
        ContextCompilationDisposition::Unknown
    } else if qualified.len() == required.len() && omissions.is_empty() && uncertainty.is_empty() {
        ContextCompilationDisposition::Qualified
    } else {
        ContextCompilationDisposition::Partial
    };
    let study_order = studies.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let aggregate_order = aggregate_order.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "semantic_profile": request.semantic_profile}))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "comparability_digest": comparability_digest, "disposition": disposition, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "raw_data_local": raw_data_local, "aggregate_only": request.aggregate_only}))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "study_order": study_order, "modality_order": modality_order, "aggregate_order": aggregate_order, "comparability_digest": comparability_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY}))
        .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
    ) {
        vec![format!(
            "manage:local-federated-context:{}",
            request.federation_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "candidate_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "aggregate_order": aggregate_order, "comparability_digest": comparability_digest, "envelope_digest": envelope_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": [], "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-context:{}", request.request_id),
        CONTEXT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContextCompilationError::Artifact(error.to_string()))?;
    let receipt = FederatedContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        endpoint: request.endpoint.clone(),
        study_order,
        modality_order,
        disposition,
        candidate_order,
        qualified_order,
        blocked_order,
        unknown_order,
        aggregate_order,
        comparability_digest,
        envelope_digest,
        context_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence: Vec::new(),
        effect_receipts,
        artifact,
        raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedContextCompilationError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedContextCompilationError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedContextCompilationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedContextCompilationError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedContextCompilationError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedContextCompilationError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedContextCompilationReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "context_digest": receipt.context_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "aggregate_only": receipt.aggregate_only,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedContextCompilationRequest {
        FederatedContextCompilationRequest {
            request_id: "request:federated-context".into(),
            federation_id: "federation:one".into(),
            institution_id: "institution:a".into(),
            purpose: "compare preclinical context".into(),
            semantic_profile: "aurora:context:v1".into(),
            endpoint: "https://institution-a.invalid/context".into(),
            study_ids: vec!["study:one".into(), "study:two".into()],
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            required_context_ids: vec!["context:one".into()],
            minimum_support_milli: 700,
            candidates: vec![FederatedContextCandidate {
                context_id: "context:one".into(),
                study_id: "study:one".into(),
                modality: "imaging".into(),
                support_milli: 900,
                state: EvidenceState::Supported,
                evidence_digest: hash("evidence"),
                provenance_digest: hash("provenance"),
                replay_identity: hash("replay"),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            federated_context_compilation_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert_eq!(
            federated_context_compilation_manifest()
                .authority_requirements
                .len(),
            1
        );
    }
    #[test]
    fn qualified_context_emits_digest_only_aggregate() {
        let receipt = compile_federated_context(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            ContextCompilationDisposition::Qualified
        );
        assert!(receipt
            .aggregate_order
            .iter()
            .all(|value| value.len() == 64));
    }
    #[test]
    fn approval_denial_blocks() {
        let mut value = request();
        value.signed_approval = false;
        let receipt = compile_federated_context(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_federated_context(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request();
        input.raw_data_local = false;
        let receipt = compile_federated_context(&input).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn non_aggregate_release_is_blocked_and_retained() {
        let mut input = request();
        input.aggregate_only = false;
        let receipt = compile_federated_context(&input).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert!(!receipt.aggregate_only);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "request:aggregate-only-required"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn envelope_artifact_payload_is_bound() {
        let mut receipt = compile_federated_context(&request()).unwrap();
        receipt.endpoint = "https://tampered.invalid/context".into();
        assert!(receipt.validate().is_err());
    }
}
