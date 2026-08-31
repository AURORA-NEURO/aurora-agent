//! Federated continual signed research-object release workbench.
//!
//! Atlas feature: `AFA-stewardship-P16-F20`.
//!
//! This surface prepares a publication queue from already validated local research objects. It
//! never signs or exports raw experimental bytes; it admits only replay-verified, provenance- and
//! evidence-complete objects and retains every blocked, stale, unknown, or negative release.

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

pub const FEATURE_ID: &str = "AFA-stewardship-P16-F20";
pub const CONTRACT_VERSION: &str = "stewardship-federated-release-workbench/1.0";
pub const MAX_OBJECTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseObjectState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseObjectCandidate {
    pub object_id: String,
    pub origin_institution: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub release_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub source_count: u32,
    pub state: ReleaseObjectState,
    pub replay_verified: bool,
    pub raw_data_local: bool,
    pub omissions: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseWorkbenchRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub objects: Vec<ReleaseObjectCandidate>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseWorkbenchDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub disposition: ReleaseWorkbenchDisposition,
    pub object_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub origin_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub release_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub federation_manifest: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseWorkbenchError {
    #[error("invalid release workbench request: {0}")]
    Invalid(String),
    #[error("release workbench artifact failed: {0}")]
    Artifact(String),
    #[error("release workbench serialization failed: {0}")]
    Serialization(String),
}

impl ReleaseWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ReleaseWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.object_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ReleaseWorkbenchError::Invalid(
                "release identity, objects, locality, effects, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.object_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.origin_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ReleaseWorkbenchError::Invalid(
                    "release workbench ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.artifact_order,
            &self.provenance_order,
            &self.evidence_order,
            &self.release_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ReleaseWorkbenchError::Invalid(
                    "release workbench digest ordering is not canonical".into(),
                ));
            }
        }
        self.federation_manifest
            .validate_metadata()
            .map_err(|error| ReleaseWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ReleaseWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReleaseWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReleaseWorkbenchError::Serialization(error.to_string()))
    }
}

pub fn release_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: "0.1.0".into(), owner_crate: "stewardship".into(), consumers: ["publication steward".into(), "federation release verifier".into(), "researcher".into()].into(), behavior: "qualifies replay-verified signed research-object candidates for continual federated publication while retaining omission, policy, provenance, evidence, and localization witnesses".into(), value: "turns local validated runs into an auditable publication queue without exporting raw experimental data or hiding failed releases".into(), inputs: vec![TypedPort { name: "release_workbench_request".into(), schema: "ReleaseWorkbenchRequest@1".into(), required: true }], outputs: vec![TypedPort { name: "release_workbench_receipt".into(), schema: "ReleaseWorkbenchReceipt@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation, Effect::FederationExport].into(), permissions: ["read:validated-research-objects".into(), "exchange:signed-research-object-manifest".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ro-crate-1.1".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification/1.1/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "consortium release steward".into(), reason: "approve cross-institution signed research-object manifest exchange".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn prepare_release_workbench(
    request: &ReleaseWorkbenchRequest,
) -> Result<ReleaseWorkbenchReceipt, ReleaseWorkbenchError> {
    validate_request(request)?;
    let mut objects = request.objects.clone();
    objects.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let object_order = objects
        .iter()
        .map(|object| object.object_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut origins = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut releases = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for object in &objects {
        let cost = object.object_id.len() as u64 + object.source_count as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = object.state == ReleaseObjectState::Supported
            && object.source_count > 0
            && object.replay_verified
            && object.raw_data_local
            && object.omissions.is_empty()
            && budget_ok;
        let allow = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && complete;
        if allow {
            spent = spent.saturating_add(cost);
            admitted.push(object.object_id.clone());
            origins.insert(object.origin_institution.clone());
            artifacts.insert(object.artifact_digest.clone());
            provenance.insert(object.provenance_digest.clone());
            evidence.insert(object.evidence_digest.clone());
            releases.insert(object.release_digest.clone());
        } else {
            blocked.insert(object.object_id.clone());
            if matches!(
                object.state,
                ReleaseObjectState::Unknown | ReleaseObjectState::Unmeasured
            ) {
                unknown.insert(object.object_id.clone());
                uncertainty.insert(
                    format!(
                        "object:{}:state-{:?}-not-admitted",
                        object.object_id, object.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if matches!(
                object.state,
                ReleaseObjectState::Contradicted | ReleaseObjectState::Revoked
            ) {
                negative.insert(
                    format!(
                        "object:{}:state-{:?}-negative-evidence",
                        object.object_id, object.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if object.source_count == 0 {
                omissions.insert(format!(
                    "object:{}:source-evidence-missing",
                    object.object_id
                ));
            }
            if !object.replay_verified {
                uncertainty.insert(format!("object:{}:replay-unverified", object.object_id));
            }
            if !object.omissions.is_empty() {
                uncertainty.insert(format!(
                    "object:{}:protected-closure-incomplete",
                    object.object_id
                ));
            }
            if !object.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "object:{}:raw-data-locality-failed",
                    object.object_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("object:{}:budget-exhausted", object.object_id));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let disposition = if !request.policy_allow
        || !request.federation_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        ReleaseWorkbenchDisposition::Blocked
    } else if admitted.is_empty() {
        ReleaseWorkbenchDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        ReleaseWorkbenchDisposition::Qualified
    } else {
        ReleaseWorkbenchDisposition::Partial
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "federation_id": request.federation_id, "disposition": disposition, "object_order": object_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "origin_order": origins, "artifact_order": artifacts, "provenance_order": provenance, "evidence_order": evidence, "release_order": releases, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let federation_manifest = TypedResearchArtifact::from_payload(
        format!("federated-release-manifest:{}", request.request_id),
        "application/vnd.aurora.federated-release-manifest+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReleaseWorkbenchError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:release-workbench-publish".into()]
    } else {
        vec![format!(
            "exchange:signed-research-object-manifest:{}",
            request.request_id
        )]
    };
    let receipt = ReleaseWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        object_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        origin_order: origins.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        release_order: releases.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        federation_manifest,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ReleaseWorkbenchRequest) -> Result<(), ReleaseWorkbenchError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.objects.is_empty()
        || request.objects.len() > MAX_OBJECTS
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ReleaseWorkbenchError::Invalid(
            "release identity, purpose, objects, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for object in &request.objects {
        if object.object_id.trim().is_empty()
            || object.origin_institution.trim().is_empty()
            || object.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(object.object_id.clone())
        {
            return Err(ReleaseWorkbenchError::Invalid(format!(
                "object {} is invalid or duplicated",
                object.object_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn object(id: &str, state: ReleaseObjectState) -> ReleaseObjectCandidate {
        ReleaseObjectCandidate {
            object_id: id.into(),
            origin_institution: format!("institution:{id}"),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            release_digest: hash(&format!("release:{id}")),
            replay_identity: hash(&format!("replay:{id}")),
            source_count: 3,
            state,
            replay_verified: true,
            raw_data_local: true,
            omissions: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(objects: Vec<ReleaseObjectCandidate>) -> ReleaseWorkbenchRequest {
        ReleaseWorkbenchRequest {
            request_id: "request:release".into(),
            workflow_id: "workflow:publish".into(),
            federation_id: "federation:commons".into(),
            purpose: "reproducibility".into(),
            objects,
            replay_identity: hash("replay"),
            budget: 1000,
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a2() {
        let manifest = release_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn supported_objects_are_qualified() {
        let receipt = prepare_release_workbench(&request(vec![
            object("object:b", ReleaseObjectState::Supported),
            object("object:a", ReleaseObjectState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ReleaseWorkbenchDisposition::Qualified);
        assert_eq!(receipt.admitted_order, vec!["object:a", "object:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_object_is_retained() {
        let receipt = prepare_release_workbench(&request(vec![
            object("object:a", ReleaseObjectState::Supported),
            object("object:b", ReleaseObjectState::Unknown),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ReleaseWorkbenchDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"object:b".into()));
    }
    #[test]
    fn revoked_object_is_negative() {
        let receipt = prepare_release_workbench(&request(vec![object(
            "object:a",
            ReleaseObjectState::Revoked,
        )]))
        .unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("revoked")));
    }
    #[test]
    fn federation_denial_blocks_publish() {
        let mut input = request(vec![object("object:a", ReleaseObjectState::Supported)]);
        input.federation_allow = false;
        let receipt = prepare_release_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, ReleaseWorkbenchDisposition::Blocked);
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:release-workbench-publish"]
        );
    }
    #[test]
    fn duplicate_objects_are_rejected() {
        let result = prepare_release_workbench(&request(vec![
            object("object:a", ReleaseObjectState::Supported),
            object("object:a", ReleaseObjectState::Supported),
        ]));
        assert!(result.is_err());
    }
}
