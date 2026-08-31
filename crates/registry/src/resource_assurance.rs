//! Federated continual resource-discovery assurance.
//!
//! Atlas feature: `AFA-registry-P05-F28`.
//!
//! The registry does not expose a raw-data directory.  It qualifies signed, digest-only
//! resource descriptors against a researcher's typed need, preserving stale, revoked,
//! unavailable, protected, and out-of-scope resources as explicit omissions.  This makes a
//! continual consortium registry useful to downstream context compilers without turning a
//! directory listing into an authorization or scientific conclusion.

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

pub const FEATURE_ID: &str = "AFA-registry-P05-F28";
pub const CONTRACT_VERSION: &str = "registry-federated-resource-discovery-assurance/1.0";
pub const MAX_RESOURCES: usize = 4096;
pub const MAX_RESULTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Available,
    Stale,
    Protected,
    Unavailable,
    Revoked,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedResourceDescriptor {
    pub resource_id: String,
    pub institution_id: String,
    pub scope: String,
    pub capabilities: Vec<String>,
    pub modalities: Vec<String>,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub registry_epoch: u64,
    pub state: ResourceState,
    pub trust_milli: u16,
    pub signed: bool,
    pub raw_data_local: bool,
    pub export_permitted: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDiscoveryAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub scope: String,
    pub required_capabilities: Vec<String>,
    pub required_modalities: Vec<String>,
    pub minimum_registry_epoch: u64,
    pub max_results: usize,
    pub resources: Vec<FederatedResourceDescriptor>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAssuranceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedFederatedResource {
    pub resource_id: String,
    pub institution_id: String,
    pub rank: usize,
    pub capability_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceDiscoveryAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub scope: String,
    pub disposition: ResourceAssuranceDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_resources: Vec<QualifiedFederatedResource>,
    pub omitted_order: Vec<String>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub federation_manifest: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

fn federation_manifest_payload(
    request_id: &str,
    federation_id: &str,
    requester: &str,
    scope: &str,
    disposition: ResourceAssuranceDisposition,
    candidate_order: &[String],
    selected_order: &[String],
    omitted_order: &[String],
    selected_resources: &[QualifiedFederatedResource],
    semantic_order: &[ContentHash],
    artifact_order: &[ContentHash],
    provenance_order: &[ContentHash],
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    replay_identity: &ContentHash,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request_id,
        "federation_id": federation_id,
        "requester": requester,
        "scope": scope,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "omitted_order": omitted_order,
        "resources": selected_resources,
        "semantic_order": semantic_order,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": replay_identity,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceDiscoveryAssuranceError {
    #[error("invalid resource discovery assurance field: {0}")]
    Invalid(String),
    #[error("resource discovery assurance serialization failed: {0}")]
    Serialization(String),
    #[error("resource discovery assurance artifact failed: {0}")]
    Artifact(String),
}

impl ResourceDiscoveryAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ResourceDiscoveryAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "identity, candidate order, locality, checks, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.omitted_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ResourceDiscoveryAssuranceError::Invalid(
                    "resource discovery ordering is not canonical".into(),
                ));
            }
        }
        if self.candidate_order.iter().any(|id| id.trim().is_empty())
            || self
                .candidate_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self
                .selected_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "resource discovery candidate identities are not unique".into(),
            ));
        }
        let candidate_ids = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().collect::<BTreeSet<_>>();
        let omitted_ids = self.omitted_order.iter().collect::<BTreeSet<_>>();
        let classified_ids = selected_ids.union(&omitted_ids).collect::<BTreeSet<_>>();
        if selected_ids.intersection(&omitted_ids).next().is_some()
            || classified_ids.iter().any(|id| !candidate_ids.contains(*id))
            || candidate_ids.iter().any(|id| !classified_ids.contains(id))
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "resource discovery candidates are not partitioned by selection and omission"
                    .into(),
            ));
        }
        if self.selected_resources.len() != self.selected_order.len()
            || self
                .selected_resources
                .iter()
                .zip(&self.selected_order)
                .enumerate()
                .any(|(index, (resource, id))| {
                    resource.resource_id != *id
                        || resource.rank != index + 1
                        || resource.institution_id.trim().is_empty()
                        || resource.reason.trim().is_empty()
                        || resource.capability_order.is_empty()
                        || resource.modality_order.is_empty()
                        || resource
                            .capability_order
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                        || resource
                            .modality_order
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                })
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "resource discovery selected resource details do not match selection order".into(),
            ));
        }
        let expected_effects = if self.selected_order.is_empty() {
            vec!["block:federated-resource-discovery".to_string()]
        } else {
            self.selected_order
                .iter()
                .map(|id| format!("exchange:signed-resource-manifest:{id}"))
                .collect::<Vec<_>>()
        };
        if self.effect_receipts != expected_effects {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "resource discovery effects do not match the selected resources".into(),
            ));
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ResourceDiscoveryAssuranceError::Invalid(
                    "resource discovery digest ordering is not canonical".into(),
                ));
            }
        }
        if self.disposition == ResourceAssuranceDisposition::Qualified
            && self.selected_order.is_empty()
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(
                "qualified discovery requires a selected resource".into(),
            ));
        }
        if self.federation_manifest.artifact_id
            != format!("federated-resource-manifest:{}", self.request_id)
            || self.federation_manifest.content_type
                != "application/vnd.aurora.federated-resource-manifest+json"
            || !self.federation_manifest.semantic_loss.is_empty()
            || !self.federation_manifest.provenance.is_empty()
        {
            return Err(ResourceDiscoveryAssuranceError::Artifact(
                "resource discovery manifest identity or provenance is invalid".into(),
            ));
        }
        self.federation_manifest
            .validate_metadata()
            .map_err(|error| ResourceDiscoveryAssuranceError::Artifact(error.to_string()))?;
        let payload = federation_manifest_payload(
            &self.request_id,
            &self.federation_id,
            &self.requester,
            &self.scope,
            self.disposition,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.selected_resources,
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.replay_identity,
        );
        self.federation_manifest
            .verify_payload(&payload)
            .map_err(|error| ResourceDiscoveryAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ResourceDiscoveryAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResourceDiscoveryAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResourceDiscoveryAssuranceError::Serialization(error.to_string()))
    }
}

pub fn resource_discovery_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "registry".into(),
        consumers: [
            "research consortium operator".into(),
            "context compiler".into(),
            "preclinical researcher".into(),
        ]
        .into(),
        behavior: "qualifies signed digest-only resource descriptors against a typed need with deterministic ranking, freshness, trust, capability, modality, locality, policy, and federation gates while retaining every omission".into(),
        value: "turns a continually changing consortium registry into a replayable research-resource workbench without exporting protected raw data or treating availability as scientific validity".into(),
        inputs: vec![TypedPort {
            name: "resource_need_and_descriptors".into(),
            schema: "ResourceDiscoveryAssuranceRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "resource_discovery_assurance_receipt".into(),
            schema: "ResourceDiscoveryAssuranceReceipt@1".into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::ExecuteLocalComputation,
            Effect::FederationExport,
        ]
        .into(),
        permissions: ["view:authorized-research-state".into(), "exchange:permitted-summaries".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "ga4gh-drs-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.1".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification/1.1/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "institutional federation steward".into(),
            reason: "approve digest-only cross-institution resource-manifest exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_resource_discovery(
    request: &ResourceDiscoveryAssuranceRequest,
) -> Result<ResourceDiscoveryAssuranceReceipt, ResourceDiscoveryAssuranceError> {
    validate_request(request)?;
    let mut resources = request.resources.clone();
    resources.sort_by(|left, right| {
        right
            .trust_milli
            .cmp(&left.trust_milli)
            .then(left.resource_id.cmp(&right.resource_id))
    });
    let candidate_order = resources
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect::<Vec<_>>();
    let required_capabilities = request
        .required_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for resource in &resources {
        let cost = resource
            .resource_id
            .len()
            .checked_add(resource.capabilities.len())
            .and_then(|total| total.checked_add(resource.modalities.len()))
            .and_then(|total| u64::try_from(total).ok())
            .and_then(|total| total.checked_add(1))
            .ok_or_else(|| {
                ResourceDiscoveryAssuranceError::Invalid(
                    "resource cost exceeds the representable budget range".into(),
                )
            })?;
        let next_spent = spent.checked_add(cost);
        let budget_ok = next_spent.is_some_and(|total| total <= request.budget);
        let state_ok = resource.state == ResourceState::Available;
        let scope_ok = resource.scope == request.scope;
        let capability_ok = required_capabilities
            .iter()
            .all(|required| resource.capabilities.contains(required));
        let modality_ok = required_modalities
            .iter()
            .all(|required| resource.modalities.contains(required));
        let complete = state_ok
            && scope_ok
            && capability_ok
            && modality_ok
            && resource.registry_epoch >= request.minimum_registry_epoch
            && resource.signed
            && resource.raw_data_local
            && resource.export_permitted
            && budget_ok;
        let admitted = request.policy_allow
            && request.protected_closure
            && request.federation_allow
            && request.signed_approval
            && request.raw_data_local
            && complete
            && selected.len() < request.max_results;
        if admitted {
            spent = next_spent.ok_or_else(|| {
                ResourceDiscoveryAssuranceError::Invalid(
                    "resource budget accounting overflowed before admission".into(),
                )
            })?;
            let mut capabilities = resource.capabilities.clone();
            capabilities.sort();
            capabilities.dedup();
            let mut modalities = resource.modalities.clone();
            modalities.sort();
            modalities.dedup();
            selected.push(QualifiedFederatedResource {
                resource_id: resource.resource_id.clone(),
                institution_id: resource.institution_id.clone(),
                rank: selected.len() + 1,
                capability_order: capabilities,
                modality_order: modalities,
                reason: "signed, fresh, trusted, scoped, capability-complete, modality-complete, local, and policy-permitted".into(),
            });
            semantics.insert(resource.semantic_digest.clone());
            artifacts.insert(resource.artifact_digest.clone());
            provenance.insert(resource.provenance_digest.clone());
        } else {
            omitted.insert(resource.resource_id.clone());
            if !state_ok {
                let reason = format!(
                    "resource:{}:state-{:?}",
                    resource.resource_id, resource.state
                )
                .to_ascii_lowercase();
                if matches!(resource.state, ResourceState::Stale) {
                    uncertainty.insert(reason);
                } else {
                    negative.insert(reason);
                }
            }
            if !scope_ok {
                omissions.insert(format!("resource:{}:scope-mismatch", resource.resource_id));
            }
            if !capability_ok {
                omissions.insert(format!(
                    "resource:{}:missing-capability",
                    resource.resource_id
                ));
            }
            if !modality_ok {
                omissions.insert(format!(
                    "resource:{}:missing-modality",
                    resource.resource_id
                ));
            }
            if resource.registry_epoch < request.minimum_registry_epoch {
                uncertainty.insert(format!(
                    "resource:{}:stale-registry-epoch",
                    resource.resource_id
                ));
            }
            if !resource.signed {
                negative.insert(format!(
                    "resource:{}:unsigned-descriptor",
                    resource.resource_id
                ));
            }
            if !resource.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "resource:{}:raw-data-locality-failed",
                    resource.resource_id
                ));
            }
            if !resource.export_permitted {
                omissions.insert(format!(
                    "resource:{}:export-not-permitted",
                    resource.resource_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "resource:{}:budget-exhausted",
                    resource.resource_id
                ));
            }
            if selected.len() >= request.max_results {
                omissions.insert(format!("resource:{}:result-limit", resource.resource_id));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.federation_allow
        || !request.raw_data_local
    {
        ResourceAssuranceDisposition::Blocked
    } else if selected.is_empty() {
        ResourceAssuranceDisposition::Unknown
    } else if omitted.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        ResourceAssuranceDisposition::Qualified
    } else {
        ResourceAssuranceDisposition::Partial
    };
    let mut checks = vec![
        "candidate ranking is deterministic by trust then resource identity".into(),
        "scope, capability, modality, freshness, signature, locality, policy, federation, approval, and budget gates are explicit".into(),
        "stale, revoked, protected, unavailable, contradicted, missing, and denied resources remain unresolved".into(),
        "federation exchanges digest-only manifests; raw research data remains institution-local".into(),
    ];
    checks.sort();
    let selected_order = selected
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let payload = federation_manifest_payload(
        &request.request_id,
        &request.federation_id,
        &request.requester,
        &request.scope,
        disposition,
        &candidate_order,
        &selected_order,
        &omitted_order,
        &selected,
        &semantics.iter().cloned().collect::<Vec<_>>(),
        &artifacts.iter().cloned().collect::<Vec<_>>(),
        &provenance.iter().cloned().collect::<Vec<_>>(),
        &checks,
        &omissions.iter().cloned().collect::<Vec<_>>(),
        &uncertainty.iter().cloned().collect::<Vec<_>>(),
        &negative.iter().cloned().collect::<Vec<_>>(),
        &request.replay_identity,
    );
    let federation_manifest = TypedResearchArtifact::from_payload(
        format!("federated-resource-manifest:{}", request.request_id),
        "application/vnd.aurora.federated-resource-manifest+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceDiscoveryAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if selected.is_empty() {
        vec!["block:federated-resource-discovery".into()]
    } else {
        selected
            .iter()
            .map(|resource| format!("exchange:signed-resource-manifest:{}", resource.resource_id))
            .collect::<Vec<_>>()
    };
    let receipt = ResourceDiscoveryAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        selected_order,
        selected_resources: selected,
        omitted_order,
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        checks,
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

fn validate_request(
    request: &ResourceDiscoveryAssuranceRequest,
) -> Result<(), ResourceDiscoveryAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request
            .required_capabilities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.max_results == 0
        || request.max_results > MAX_RESULTS
        || request.resources.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ResourceDiscoveryAssuranceError::Invalid(
            "request identity, sorted need, result limit, resources, budget, or boundary is incomplete".into(),
        ));
    }
    if request.resources.len() > MAX_RESOURCES {
        return Err(ResourceDiscoveryAssuranceError::Invalid(format!(
            "resource count exceeds {MAX_RESOURCES}"
        )));
    }
    let mut ids = BTreeSet::new();
    for resource in &request.resources {
        if resource.resource_id.trim().is_empty()
            || resource.institution_id.trim().is_empty()
            || resource.scope.trim().is_empty()
            || resource.capabilities.is_empty()
            || resource.modalities.is_empty()
            || resource.trust_milli > 1000
            || resource.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(resource.resource_id.clone())
        {
            return Err(ResourceDiscoveryAssuranceError::Invalid(format!(
                "resource {} is invalid or duplicated",
                resource.resource_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn resource(id: &str, trust_milli: u16, state: ResourceState) -> FederatedResourceDescriptor {
        FederatedResourceDescriptor {
            resource_id: id.into(),
            institution_id: format!("institution:{id}"),
            scope: "organoid:neural".into(),
            capabilities: vec!["imaging".into(), "segmentation".into()],
            modalities: vec!["ome-ngff".into()],
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            registry_epoch: 12,
            state,
            trust_milli,
            signed: true,
            raw_data_local: true,
            export_permitted: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(resources: Vec<FederatedResourceDescriptor>) -> ResourceDiscoveryAssuranceRequest {
        ResourceDiscoveryAssuranceRequest {
            request_id: "request:resources".into(),
            federation_id: "federation:organoids".into(),
            requester: "researcher:alice".into(),
            scope: "organoid:neural".into(),
            required_capabilities: vec!["imaging".into(), "segmentation".into()],
            required_modalities: vec!["ome-ngff".into()],
            minimum_registry_epoch: 10,
            max_results: 4,
            resources,
            replay_identity: hash("replay:resources"),
            budget: 1000,
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_a2_and_byte_stable() {
        let manifest = resource_discovery_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }

    #[test]
    fn qualifies_by_trust_then_identity() {
        let receipt = assure_resource_discovery(&request(vec![
            resource("resource:b", 700, ResourceState::Available),
            resource("resource:a", 900, ResourceState::Available),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ResourceAssuranceDisposition::Qualified);
        assert_eq!(receipt.selected_order, vec!["resource:a", "resource:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn a_higher_trust_resource_may_precede_lexically_smaller_identity() {
        let receipt = assure_resource_discovery(&request(vec![
            resource("resource:a", 700, ResourceState::Available),
            resource("resource:b", 900, ResourceState::Available),
        ]))
        .unwrap();
        assert_eq!(receipt.candidate_order, vec!["resource:b", "resource:a"]);
        assert_eq!(receipt.selected_order, vec!["resource:b", "resource:a"]);
    }

    #[test]
    fn tampered_resource_manifest_payload_is_rejected() {
        let mut receipt = assure_resource_discovery(&request(vec![resource(
            "resource:a",
            900,
            ResourceState::Available,
        )]))
        .unwrap();
        receipt.selected_resources[0].institution_id = "institution:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn stale_resource_is_uncertain_and_retained() {
        let receipt = assure_resource_discovery(&request(vec![
            resource("resource:a", 900, ResourceState::Stale),
            resource("resource:b", 700, ResourceState::Available),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ResourceAssuranceDisposition::Partial);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("stale")));
        assert!(receipt.omitted_order.contains(&"resource:a".into()));
    }

    #[test]
    fn federation_denial_blocks_exchange() {
        let mut input = request(vec![resource("resource:a", 900, ResourceState::Available)]);
        input.federation_allow = false;
        let receipt = assure_resource_discovery(&input).unwrap();
        assert_eq!(receipt.disposition, ResourceAssuranceDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("federation")));
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:federated-resource-discovery"]
        );
    }

    #[test]
    fn protected_resource_is_negative_not_selected() {
        let receipt = assure_resource_discovery(&request(vec![resource(
            "resource:a",
            900,
            ResourceState::Protected,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, ResourceAssuranceDisposition::Unknown);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("protected")));
    }

    #[test]
    fn duplicate_resource_ids_are_rejected() {
        let result = assure_resource_discovery(&request(vec![
            resource("resource:a", 900, ResourceState::Available),
            resource("resource:a", 800, ResourceState::Available),
        ]));
        assert!(result.is_err());
    }
}
