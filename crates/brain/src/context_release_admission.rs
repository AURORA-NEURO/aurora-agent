//! Policy- and grant-bound admission for compiled research context.
//!
//! Atlas feature: `AFA-brain-P03-F06`. The admission gate sits between context
//! compilation and downstream decision-section release. It requires matching
//! replay/context/omission identities, a valid typed grant, and a policy receipt;
//! it never upgrades unresolved evidence.

use bioprism_foundation::{
    AutonomyGrant, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, PolicyDecision, PolicyReceipt, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F06";
pub const CONTRACT_VERSION: &str = "brain-context-release-admission/1.0";
pub const RELEASE_ACTION: &str = "release:local-context";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextReleaseAdmissionRequest {
    pub request_id: String,
    pub context_digest: ContentHash,
    pub omission_certificate_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_receipt: PolicyReceipt,
    pub autonomy_grant: AutonomyGrant,
    pub requested_resource: String,
    pub requested_units: f64,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextReleaseAdmissionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub disposition: String,
    pub actor: String,
    pub action: String,
    pub context_digest: ContentHash,
    pub omission_certificate_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_decision: PolicyDecision,
    pub policy_reasons: Vec<String>,
    pub grant_scope: String,
    pub grant_expiry: String,
    pub remaining_units: f64,
    pub release_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextReleaseAdmissionError {
    #[error("invalid context release admission request: {0}")]
    Invalid(String),
    #[error("context release admission artifact failed: {0}")]
    Artifact(String),
}

impl ContextReleaseAdmissionReceipt {
    pub fn validate(&self) -> Result<(), ContextReleaseAdmissionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.actor.trim().is_empty()
            || self.action != RELEASE_ACTION
            || self.grant_scope.trim().is_empty()
            || self.grant_expiry.trim().is_empty()
            || !self.remaining_units.is_finite()
            || self.remaining_units < 0.0
            || self.policy_reasons.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "admitted" | "blocked" | "approval_required" | "unresolved"
            )
        {
            return Err(ContextReleaseAdmissionError::Invalid(
                "context release identity, policy, grant, budget, disposition, or effects are incomplete".into(),
            ));
        }
        for digest in [
            &self.context_digest,
            &self.omission_certificate_digest,
            &self.replay_identity,
            &self.release_digest,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextReleaseAdmissionError::Invalid(
                    "context release digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("release:local-context:") && effect != "block:unsafe-release"
        }) {
            return Err(ContextReleaseAdmissionError::Invalid(
                "context release effect is outside admission gate".into(),
            ));
        }
        let policy_allows = matches!(
            self.policy_decision,
            PolicyDecision::Allow | PolicyDecision::LocalOnly
        ) && !self
            .policy_reasons
            .iter()
            .any(|reason| reason == "unresolved");
        if self.disposition == "admitted" && !policy_allows {
            return Err(ContextReleaseAdmissionError::Invalid(
                "admitted context release is inconsistent with policy decision".into(),
            ));
        }
        if self.disposition == "unresolved" && self.policy_decision != PolicyDecision::Unresolved {
            return Err(ContextReleaseAdmissionError::Invalid(
                "unresolved context release is inconsistent with policy decision".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == "admitted" {
            vec![format!("release:local-context:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextReleaseAdmissionError::Invalid(
                "context release effect does not match disposition".into(),
            ));
        }
        let expected_release_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "context_digest": self.context_digest,
            "omission_certificate_digest": self.omission_certificate_digest,
            "replay_identity": self.replay_identity,
            "actor": self.actor,
            "action": RELEASE_ACTION,
            "remaining_units": self.remaining_units,
            "disposition": self.disposition,
        }))
        .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))?;
        if self.release_digest != expected_release_digest {
            return Err(ContextReleaseAdmissionError::Invalid(
                "context release digest is not bound to admission state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-context-release-admission:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != "application/vnd.aurora.context-release-admission+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextReleaseAdmissionError::Invalid(
                "context release artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextReleaseAdmissionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))
    }
}

fn receipt_payload(receipt: &ContextReleaseAdmissionReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "disposition": receipt.disposition,
        "actor": receipt.actor,
        "action": receipt.action,
        "context_digest": receipt.context_digest,
        "omission_certificate_digest": receipt.omission_certificate_digest,
        "replay_identity": receipt.replay_identity,
        "policy_decision": receipt.policy_decision,
        "policy_reasons": receipt.policy_reasons,
        "grant_scope": receipt.grant_scope,
        "grant_expiry": receipt.grant_expiry,
        "remaining_units": receipt.remaining_units,
        "release_digest": receipt.release_digest,
        "boundary": receipt.boundary,
    })
}

pub fn context_release_admission_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["decision-section compiler".into(), "research workflow operator".into(), "policy engine".into()].into(),
        behavior: "admits a compiled local research context only when policy, autonomy grant, replay, omission certificate, and budget identities close".into(),
        value: "prevents downstream automation from releasing stale, unresolved, unbudgeted, or unauthorized context".into(),
        inputs: vec![TypedPort { name: "context_release_admission_request".into(), schema: "ContextReleaseAdmissionRequest1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "context_release_admission_receipt".into(), schema: "ContextReleaseAdmissionReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: [RELEASE_ACTION.into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn admit_context_release(
    request: &ContextReleaseAdmissionRequest,
) -> Result<ContextReleaseAdmissionReceipt, ContextReleaseAdmissionError> {
    if request.request_id.trim().is_empty()
        || request.requested_resource.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.context_digest.as_str().len() != 64
        || request.omission_certificate_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || !request.requested_units.is_finite()
        || request.requested_units <= 0.0
    {
        return Err(ContextReleaseAdmissionError::Invalid(
            "context release request identity, resource, budget, or boundary is invalid".into(),
        ));
    }
    request.policy_receipt.validate().map_err(|error| {
        ContextReleaseAdmissionError::Invalid(format!("policy receipt: {error}"))
    })?;
    request.autonomy_grant.validate().map_err(|error| {
        ContextReleaseAdmissionError::Invalid(format!("autonomy grant: {error}"))
    })?;
    let permitted = request
        .autonomy_grant
        .permitted_actions
        .contains(RELEASE_ACTION);
    let budget = request
        .autonomy_grant
        .resource_budget
        .get(&request.requested_resource)
        .copied()
        .unwrap_or(0.0);
    let remaining_units = (budget - request.requested_units).max(0.0);
    let identity_match = request.replay_identity.as_str().len() == 64
        && request
            .policy_receipt
            .evaluated_artifacts
            .contains(&request.context_digest)
        && request
            .policy_receipt
            .evaluated_artifacts
            .contains(&request.omission_certificate_digest);
    let policy_ok = matches!(
        request.policy_receipt.decision,
        PolicyDecision::Allow | PolicyDecision::LocalOnly
    ) && request
        .policy_receipt
        .reasons
        .iter()
        .all(|reason| reason != "unresolved");
    let disposition = if request.autonomy_grant.revoked
        || !permitted
        || budget < request.requested_units
        || !identity_match
    {
        "blocked"
    } else if !policy_ok
        || matches!(
            request.policy_receipt.decision,
            PolicyDecision::ApprovalRequired
        )
    {
        "approval_required"
    } else if matches!(request.policy_receipt.decision, PolicyDecision::Unresolved) {
        "unresolved"
    } else {
        "admitted"
    };
    let release_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "context_digest": request.context_digest,
        "omission_certificate_digest": request.omission_certificate_digest,
        "replay_identity": request.replay_identity,
        "actor": request.autonomy_grant.actor,
        "action": RELEASE_ACTION,
        "remaining_units": remaining_units,
        "disposition": disposition,
    }))
    .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "admitted" {
        vec![format!("release:local-context:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "disposition": disposition,
        "actor": request.autonomy_grant.actor,
        "action": RELEASE_ACTION,
        "context_digest": request.context_digest,
        "omission_certificate_digest": request.omission_certificate_digest,
        "replay_identity": request.replay_identity,
        "policy_decision": request.policy_receipt.decision,
        "policy_reasons": request.policy_receipt.reasons,
        "grant_scope": request.autonomy_grant.scope,
        "grant_expiry": request.autonomy_grant.expires_at,
        "remaining_units": remaining_units,
        "release_digest": release_digest,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-release-admission:{}", request.request_id),
        "application/vnd.aurora.context-release-admission+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextReleaseAdmissionError::Artifact(error.to_string()))?;
    let receipt = ContextReleaseAdmissionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        disposition: disposition.into(),
        actor: request.autonomy_grant.actor.clone(),
        action: RELEASE_ACTION.into(),
        context_digest: request.context_digest.clone(),
        omission_certificate_digest: request.omission_certificate_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        policy_decision: request.policy_receipt.decision,
        policy_reasons: request.policy_receipt.reasons.clone(),
        grant_scope: request.autonomy_grant.scope.clone(),
        grant_expiry: request.autonomy_grant.expires_at.clone(),
        remaining_units,
        release_digest,
        effect_receipts,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ContextReleaseAdmissionRequest {
        let mut actions = BTreeSet::new();
        actions.insert(RELEASE_ACTION.into());
        let mut budget = BTreeMap::new();
        budget.insert("context_units".into(), 10.0);
        ContextReleaseAdmissionRequest {
            request_id: "request:release".into(),
            context_digest: hash("context"),
            omission_certificate_digest: hash("omissions"),
            replay_identity: hash("replay"),
            policy_receipt: PolicyReceipt {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                receipt_id: "policy:release".into(),
                decision: PolicyDecision::LocalOnly,
                reasons: vec!["local-context-release".into()],
                evaluated_artifacts: vec![hash("context"), hash("omissions")],
                authority_reference: None,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            autonomy_grant: AutonomyGrant {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                actor: "researcher".into(),
                permitted_actions: actions,
                resource_budget: budget,
                scope: "study:preclinical".into(),
                expires_at: "2027-01-01T00:00:00Z".into(),
                revoked: false,
                autonomy_tier: AutonomyTier::A1,
                approval_reference: None,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            requested_resource: "context_units".into(),
            requested_units: 2.0,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            context_release_admission_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn local_policy_is_admitted() {
        let receipt = admit_context_release(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.remaining_units, 8.0);
    }
    #[test]
    fn revoked_grant_blocks() {
        let mut value = request();
        value.autonomy_grant.revoked = true;
        let receipt = admit_context_release(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
    }
    #[test]
    fn digest_is_stable() {
        let receipt = admit_context_release(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
