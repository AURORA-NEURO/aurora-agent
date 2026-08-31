//! Federated continual resource-discovery research copilot.
//!
//! Atlas feature: `AFA-bioworlds-P05-F12`.
//!
//! This bioworlds-owned product qualifies resource metadata for a researcher without fetching
//! resources or moving raw data. It makes capability, site, evidence, trust, provenance, replay,
//! and locality closure a typed admission decision that downstream workbenches can replay.

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

pub const FEATURE_ID: &str = "AFA-bioworlds-P05-F12";
pub const CONTRACT_VERSION: &str =
    "bioworlds-federated-continual-resource-discovery-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ResourceNeed5@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedResourceSet6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioworlds-qualified-resource-set-6+json";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCandidate6 {
    pub resource_id: String,
    pub capability_id: String,
    pub site_id: String,
    pub institution_id: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub availability_milli: u16,
    pub trust_milli: u16,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNeed5 {
    pub schema_version: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_resource_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub required_site_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_resource_count: u32,
    pub minimum_site_count: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub candidates: Vec<ResourceCandidate6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDiscoveryDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResourceSet6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: ResourceDiscoveryDisposition,
    pub ranked_resource_order: Vec<String>,
    pub selected_resource_order: Vec<String>,
    pub unresolved_resource_order: Vec<String>,
    pub blocked_resource_order: Vec<String>,
    pub missing_resource_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub blocked_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub resource_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResourceDiscoveryError {
    #[error("invalid bioworlds resource-discovery request or receipt: {0}")]
    Invalid(String),
    #[error("bioworlds resource-discovery artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ResourceDiscoveryError {
    ResourceDiscoveryError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), ResourceDiscoveryError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flat = Vec::new();
    for part in parts {
        if !canonical(part) || part.iter().any(|id| !expected.contains(id)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flat.extend_from_slice(part);
    }
    if flat.len() != expected.len() || flat.iter().collect::<BTreeSet<_>>().len() != flat.len() {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl QualifiedResourceSet6 {
    pub fn validate(&self) -> Result<(), ResourceDiscoveryError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A2
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_resource_order.is_empty()
            || self.capability_order.is_empty()
            || self.site_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "resource identity, closure, locality, autonomy, or effects are incomplete",
            ));
        }
        for values in [
            &self.ranked_resource_order,
            &self.selected_resource_order,
            &self.unresolved_resource_order,
            &self.blocked_resource_order,
            &self.missing_resource_order,
            &self.capability_order,
            &self.selected_capability_order,
            &self.unresolved_capability_order,
            &self.blocked_capability_order,
            &self.missing_capability_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("resource receipt ordering is not canonical"));
            }
        }
        partition(
            &self.ranked_resource_order,
            &[
                &self.selected_resource_order,
                &self.unresolved_resource_order,
                &self.blocked_resource_order,
                &self.missing_resource_order,
            ],
            "resource",
        )?;
        partition(
            &self.capability_order,
            &[
                &self.selected_capability_order,
                &self.unresolved_capability_order,
                &self.blocked_capability_order,
                &self.missing_capability_order,
            ],
            "capability",
        )?;
        partition(
            &self.site_order,
            &[
                &self.selected_site_order,
                &self.unresolved_site_order,
                &self.blocked_site_order,
                &self.missing_site_order,
            ],
            "site",
        )?;
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.resource_digest)
            || self.artifact.content_hash != self.resource_digest
        {
            return Err(invalid("resource digest or replay identity is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:authorized-resource-state:")
                && effect != "block:unsafe-release"
        }) {
            return Err(invalid(
                "resource effect is outside the bounded discovery gate",
            ));
        }
        if self.disposition == ResourceDiscoveryDisposition::Qualified
            && self.effect_receipts
                != vec![format!(
                    "read:authorized-resource-state:{}",
                    self.request_id
                )]
        {
            return Err(invalid("qualified resource effect is invalid"));
        }
        if self.disposition != ResourceDiscoveryDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified resource discovery must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ResourceDiscoveryError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ResourceDiscoveryError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| ResourceDiscoveryError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| ResourceDiscoveryError::Artifact(e.to_string()))
            })
    }
}

pub fn resource_discovery_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioworlds".into(), consumers: ["resource researcher".into(), "workbench operator".into(), "federation verifier".into()].into(), behavior: "ranks declared institution-local resource capabilities and emits an omission-aware qualified resource set without fetching resources".into(), value: "makes multi-site resource discovery reproducible while preventing unknown or unauthorized capabilities from appearing available".into(), inputs: vec![TypedPort { name: "resource_need".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_resource_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::FederationExport].into(), permissions: ["read:authorized-resource-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ga4gh-drs".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "resource workbench operator".into(), reason: "resource capability state may expose governed federation metadata and requires explicit authority".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(request: &ResourceNeed5) -> Result<(), ResourceDiscoveryError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_resource_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.required_site_order.is_empty()
        || !canonical(&request.required_resource_order)
        || !canonical(&request.required_capability_order)
        || !canonical(&request.required_site_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_resource_count == 0
        || request.minimum_site_count == 0
        || !digest_valid(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
    {
        return Err(invalid(
            "resource identity, closure, bounds, replay, boundary, or candidates are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.resource_id.trim().is_empty()
            || candidate.capability_id.trim().is_empty()
            || candidate.site_id.trim().is_empty()
            || candidate.institution_id.trim().is_empty()
            || candidate.semantic_profile != request.semantic_profile
            || candidate.availability_milli > 1000
            || candidate.trust_milli > 1000
            || !digest_valid(&candidate.provenance_digest)
            || !digest_valid(&candidate.replay_identity)
            || !canonical(&candidate.omission_order)
            || !canonical(&candidate.uncertainty_order)
            || !ids.insert(candidate.resource_id.clone())
        {
            return Err(invalid(
                "resource candidate identity, profile, ranges, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

fn evidence_rank(state: EvidenceState) -> u8 {
    match state {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    }
}

pub fn qualify_resources(
    request: &ResourceNeed5,
) -> Result<QualifiedResourceSet6, ResourceDiscoveryError> {
    validate_request(request)?;
    let mut rows = request.candidates.clone();
    rows.sort_by(|left, right| {
        (
            evidence_rank(left.evidence_state),
            std::cmp::Reverse(left.trust_milli),
            std::cmp::Reverse(left.availability_milli),
            left.resource_id.as_str(),
        )
            .cmp(&(
                evidence_rank(right.evidence_state),
                std::cmp::Reverse(right.trust_milli),
                std::cmp::Reverse(right.availability_milli),
                right.resource_id.as_str(),
            ))
    });
    let ranked = rows
        .iter()
        .map(|row| row.resource_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_resource_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    for row in &rows {
        omissions.extend(row.omission_order.iter().cloned());
        uncertainty.extend(row.uncertainty_order.iter().cloned());
        if row.negative_result {
            negative.insert(row.resource_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.resource_id.clone());
        }
        let hard = row.revoked
            || !row.policy_allowed
            || !row.federation_allowed
            || !row.raw_data_local
            || !row.aggregate_only
            || row.availability_milli == 0;
        let soft = row.stale
            || row.replay_identity != request.replay_identity
            || row.availability_milli < 500
            || row.trust_milli < 500
            || !row.omission_order.is_empty()
            || !row.uncertainty_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.resource_id.clone());
        } else if soft {
            unresolved.insert(row.resource_id.clone());
        } else {
            selected.insert(row.resource_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omissions.insert(format!("missing required resource: {id}"));
    }
    let mut capabilities = request
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    capabilities.extend(rows.iter().map(|row| row.capability_id.clone()));
    let selected_capabilities = capabilities
        .iter()
        .filter(|id| {
            rows.iter()
                .any(|row| row.capability_id == **id && selected.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && rows
                    .iter()
                    .any(|row| row.capability_id == **id && unresolved.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let blocked_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && !unresolved_capabilities.contains(*id)
                && rows
                    .iter()
                    .any(|row| row.capability_id == **id && blocked.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && !unresolved_capabilities.contains(*id)
                && !blocked_capabilities.contains(*id)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut sites = request
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    sites.extend(rows.iter().map(|row| row.site_id.clone()));
    let selected_sites = sites
        .iter()
        .filter(|id| {
            rows.iter()
                .any(|row| row.site_id == **id && selected.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved_sites = sites
        .iter()
        .filter(|id| {
            !selected_sites.contains(*id)
                && rows
                    .iter()
                    .any(|row| row.site_id == **id && unresolved.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let blocked_sites = sites
        .iter()
        .filter(|id| {
            !selected_sites.contains(*id)
                && !unresolved_sites.contains(*id)
                && rows
                    .iter()
                    .any(|row| row.site_id == **id && blocked.contains(&row.resource_id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_sites = sites
        .iter()
        .filter(|id| {
            !selected_sites.contains(*id)
                && !unresolved_sites.contains(*id)
                && !blocked_sites.contains(*id)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let globally_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.federation_allow
        && request.raw_data_local
        && request.aggregate_only
        && request.adversarial_event_order.is_empty();
    let disposition = if !globally_open
        || !blocked.is_empty()
        || !missing.is_empty()
        || !blocked_capabilities.is_empty()
        || !missing_capabilities.is_empty()
        || !blocked_sites.is_empty()
        || !missing_sites.is_empty()
        || selected.len() < request.minimum_resource_count as usize
        || selected_sites.len() < request.minimum_site_count as usize
    {
        ResourceDiscoveryDisposition::Blocked
    } else if !unresolved.is_empty()
        || !unresolved_capabilities.is_empty()
        || !unresolved_sites.is_empty()
    {
        ResourceDiscoveryDisposition::Unresolved
    } else {
        ResourceDiscoveryDisposition::Qualified
    };
    let effect_receipts = if disposition == ResourceDiscoveryDisposition::Qualified {
        vec![format!(
            "read:authorized-resource-state:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons = vec![match disposition { ResourceDiscoveryDisposition::Qualified => "all resource, capability, site, policy, replay, provenance, and locality gates passed".into(), ResourceDiscoveryDisposition::Unresolved => "stale, uncertain, low-trust, or unknown resource evidence remains unresolved".into(), ResourceDiscoveryDisposition::Blocked => "policy, closure, resource, capability, site, authorization, or adversarial gates blocked discovery".into() }];
    let provenance_digest = ContentHash::of_bytes(
        rows.iter()
            .map(|row| row.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "requester": request.requester, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "disposition": disposition, "ranked_resource_order": ranked, "selected_resource_order": selected, "unresolved_resource_order": unresolved, "blocked_resource_order": blocked, "missing_resource_order": missing, "capability_order": capabilities, "selected_capability_order": selected_capabilities, "unresolved_capability_order": unresolved_capabilities, "blocked_capability_order": blocked_capabilities, "missing_capability_order": missing_capabilities, "site_order": sites, "selected_site_order": selected_sites, "unresolved_site_order": unresolved_sites, "blocked_site_order": blocked_sites, "missing_site_order": missing_sites, "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "contradiction_order": contradiction, "adversarial_event_order": request.adversarial_event_order, "replay_identity": request.replay_identity, "provenance_digest": provenance_digest, "reasons": reasons, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "autonomy_tier": AutonomyTier::A2, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-resource-set:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ResourceDiscoveryError::Artifact(error.to_string()))?;
    let resource_digest = artifact.content_hash.clone();
    let receipt = QualifiedResourceSet6 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        ranked_resource_order: ranked,
        selected_resource_order: selected.into_iter().collect(),
        unresolved_resource_order: unresolved.into_iter().collect(),
        blocked_resource_order: blocked.into_iter().collect(),
        missing_resource_order: missing.into_iter().collect(),
        capability_order: capabilities.into_iter().collect(),
        selected_capability_order: selected_capabilities.into_iter().collect(),
        unresolved_capability_order: unresolved_capabilities.into_iter().collect(),
        blocked_capability_order: blocked_capabilities.into_iter().collect(),
        missing_capability_order: missing_capabilities.into_iter().collect(),
        site_order: sites.into_iter().collect(),
        selected_site_order: selected_sites.into_iter().collect(),
        unresolved_site_order: unresolved_sites.into_iter().collect(),
        blocked_site_order: blocked_sites.into_iter().collect(),
        missing_site_order: missing_sites.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest,
        reasons,
        resource_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        autonomy_tier: AutonomyTier::A2,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, state: EvidenceState) -> ResourceCandidate6 {
        ResourceCandidate6 {
            resource_id: id.into(),
            capability_id: format!("capability:{id}"),
            site_id: format!("site:{id}"),
            institution_id: format!("institution:{id}"),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            availability_milli: 900,
            trust_milli: 900,
            provenance_digest: hash(id),
            replay_identity: hash("replay"),
            policy_allowed: true,
            federation_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn request(candidates: Vec<ResourceCandidate6>) -> ResourceNeed5 {
        ResourceNeed5 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "resource:1".into(),
            requester: "operator".into(),
            purpose: "find resource".into(),
            semantic_profile: "imaging-omics".into(),
            required_resource_order: vec!["resource:1".into()],
            required_capability_order: vec!["capability:resource:1".into()],
            required_site_order: vec!["site:resource:1".into()],
            replay_identity: hash("replay"),
            minimum_resource_count: 1,
            minimum_site_count: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates,
        }
    }
    #[test]
    fn qualified_is_deterministic() {
        let result = qualify_resources(&request(vec![candidate(
            "resource:1",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Qualified);
    }
    #[test]
    fn unknown_is_unresolved() {
        let result = qualify_resources(&request(vec![candidate(
            "resource:1",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Unresolved);
    }
    #[test]
    fn contradiction_is_blocked() {
        let result = qualify_resources(&request(vec![candidate(
            "resource:1",
            EvidenceState::Contradicted,
        )]))
        .unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Blocked);
    }
    #[test]
    fn missing_resource_is_blocked() {
        let value = request(vec![candidate("resource:other", EvidenceState::Supported)]);
        let result = qualify_resources(&value).unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Blocked);
        assert_eq!(result.missing_resource_order, vec!["resource:1"]);
    }
    #[test]
    fn revoked_resource_is_blocked() {
        let mut item = candidate("resource:1", EvidenceState::Supported);
        item.revoked = true;
        let result = qualify_resources(&request(vec![item])).unwrap();
        assert_eq!(result.disposition, ResourceDiscoveryDisposition::Blocked);
    }
    #[test]
    fn manifest_is_valid() {
        resource_discovery_manifest().validate().unwrap();
    }
}
