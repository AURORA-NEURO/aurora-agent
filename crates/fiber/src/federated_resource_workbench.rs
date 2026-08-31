//! Federated continual resource-discovery workbench for `AFA-fiber-P05-F20`.
//!
//! The existing FIBER resource workbench ranks local candidates. This surface adds typed peer
//! attestations, replay binding, capability closure, and a digest-only researcher view while
//! keeping protected data local and making every omission visible.

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

pub const FEATURE_ID: &str = "AFA-fiber-P05-F20";
pub const CONTRACT_VERSION: &str =
    "fiber-federated-continual-resource-discovery-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ResourceNeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedResourceSet5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.fiber-qualified-resource-set-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedResourceCandidate5 {
    pub resource_id: String,
    pub site_id: String,
    pub capabilities: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub semantic_profile: String,
    pub trust_score_milli: u16,
    pub evidence_state: EvidenceState,
    pub available: bool,
    pub raw_data_local: bool,
    pub permitted: bool,
    pub revoked: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedResourceDiscoveryRequest7 {
    pub schema_version: String,
    pub request_id: String,
    pub need_id: String,
    pub requester: String,
    pub intent: String,
    pub semantic_profile: String,
    pub required_capabilities: Vec<String>,
    pub required_site_order: Vec<String>,
    pub minimum_site_count: usize,
    pub max_results: usize,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
    pub candidates: Vec<FederatedResourceCandidate5>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedResourceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedResourceWorkbenchReceipt8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub need_id: String,
    pub requester: String,
    pub intent: String,
    pub semantic_profile: String,
    pub disposition: FederatedResourceDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub resource_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedResourceWorkbenchError {
    #[error("invalid federated resource workbench request or receipt: {0}")]
    Invalid(String),
    #[error("federated resource workbench artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> FederatedResourceWorkbenchError {
    FederatedResourceWorkbenchError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl FederatedResourceWorkbenchReceipt8 {
    pub fn validate(&self) -> Result<(), FederatedResourceWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.need_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.site_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "resource identity, candidates, sites, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.site_order,
            &self.selected_site_order,
            &self.missing_site_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("resource ordering is not canonical"));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_candidate_order)
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len()
            || self.ranked_order.iter().cloned().collect::<BTreeSet<_>>() != candidates
            || parts.len() != candidates.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != candidates
        {
            return Err(invalid(
                "resource states or ranking do not form a complete partition",
            ));
        }
        let sites = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let site_parts = self
            .selected_site_order
            .iter()
            .chain(&self.missing_site_order)
            .cloned()
            .collect::<Vec<_>>();
        if sites.len() != self.site_order.len()
            || site_parts.len() != sites.len()
            || site_parts.iter().cloned().collect::<BTreeSet<_>>() != sites
        {
            return Err(invalid("site states do not form a complete partition"));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.resource_digest)
            || self.artifact.content_hash != self.resource_digest
            || self.artifact.content_type != CONTENT_TYPE
        {
            return Err(invalid("resource or replay digest is inconsistent"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:authorized-resource-state:")
                && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside resource workbench gate"));
        }
        if self.disposition == FederatedResourceDisposition::Qualified
            && self.effect_receipts != [format!("read:authorized-resource-state:{}", self.need_id)]
        {
            return Err(invalid("qualified resource effect is invalid"));
        }
        if self.disposition != FederatedResourceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified resource workbench must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedResourceWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedResourceWorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| FederatedResourceWorkbenchError::Artifact(error.to_string()))?,
        )
        .map_err(|error| FederatedResourceWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn federated_resource_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "fiber".into(), consumers: ["context compiler engineer".into(), "preclinical researcher".into(), "resource registry operator".into()].into(), behavior: "qualifies typed local and federated resource attestations against capability, site, evidence, trust, replay, and locality constraints with deterministic researcher-facing ranking".into(), value: "turns continual resource discovery into an auditable omission-aware workbench result without exposing protected raw data or hiding unavailable peers".into(), inputs: vec![TypedPort { name: "resource_need_and_federated_candidates".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_resource_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(), permissions: ["view:authorized-research-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn qualify_federated_resources(
    request: &FederatedResourceDiscoveryRequest7,
) -> Result<FederatedResourceWorkbenchReceipt8, FederatedResourceWorkbenchError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.need_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request.required_site_order.is_empty()
        || !canonical(&request.required_site_order)
        || request.minimum_site_count == 0
        || request.max_results == 0
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local_required()
        || request.candidates.is_empty()
    {
        return Err(invalid("resource request identity, capability closure, replay, locality, boundary, or candidates are invalid"));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .trust_score_milli
            .cmp(&left.trust_score_milli)
            .then(left.resource_id.cmp(&right.resource_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.resource_id.clone())
        .collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("resource candidate identities must be unique"));
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &candidates {
        let mut state = "selected";
        if !request.policy_allow
            || !request.protected_closure
            || !request.federation_allow
            || !request.signed_approval
            || !request.aggregate_only
            || candidate.revoked
            || !candidate.permitted
            || !candidate.raw_data_local
        {
            state = "blocked";
            omissions.insert(format!(
                "resource:{}:policy-or-locality",
                candidate.resource_id
            ));
        } else if !candidate.available || candidate.semantic_profile != request.semantic_profile {
            state = "unresolved";
            uncertainty.insert(format!(
                "resource:{}:availability-or-semantic",
                candidate.resource_id
            ));
        } else if !digest(&candidate.artifact_digest) || !digest(&candidate.provenance_digest) {
            state = "unresolved";
            omissions.insert(format!("resource:{}:digest-missing", candidate.resource_id));
        } else if !request
            .required_capabilities
            .iter()
            .all(|capability| candidate.capabilities.contains(capability))
        {
            state = "unresolved";
            omissions.insert(format!(
                "resource:{}:capability-missing",
                candidate.resource_id
            ));
        } else if candidate.evidence_state == EvidenceState::Contradicted {
            state = "blocked";
            negative.insert(format!("resource:{}:contradicted", candidate.resource_id));
        } else if matches!(
            candidate.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            state = "unresolved";
            uncertainty.insert(format!(
                "resource:{}:evidence-not-asserted",
                candidate.resource_id
            ));
        }
        if candidate.negative_result {
            negative.insert(format!(
                "resource:{}:negative-result",
                candidate.resource_id
            ));
        }
        match state {
            "selected" => {
                if selected.len() < request.max_results {
                    selected.insert(candidate.resource_id.clone());
                } else {
                    missing.insert(candidate.resource_id.clone());
                    omissions.insert(format!("resource:{}:result-limit", candidate.resource_id));
                }
            }
            "unresolved" => {
                unresolved.insert(candidate.resource_id.clone());
            }
            _ => {
                blocked.insert(candidate.resource_id.clone());
            }
        }
    }
    let mut sites = request
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    sites.extend(candidates.iter().map(|candidate| candidate.site_id.clone()));
    let selected_sites = sites
        .iter()
        .filter(|site| {
            candidates.iter().any(|candidate| {
                candidate.site_id == **site && selected.contains(&candidate.resource_id)
            })
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_sites = sites
        .iter()
        .filter(|site| {
            request.required_site_order.contains(site) && !selected_sites.contains(*site)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(
        missing_sites
            .iter()
            .map(|site| format!("site:{}:missing-qualified-resource", site)),
    );
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.federation_allow {
        omissions.insert("control:federation-denied".into());
    }
    if !request.signed_approval {
        omissions.insert("control:signed-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{}", event)),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.federation_allow
        || !request.signed_approval
        || !request.aggregate_only
        || !request.raw_data_local_required()
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("control:resource-release-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        FederatedResourceDisposition::Blocked
    } else if selected.is_empty()
        || !unresolved.is_empty()
        || !missing.is_empty()
        || selected_sites.len() < request.minimum_site_count
        || !missing_sites.is_empty()
    {
        FederatedResourceDisposition::Unresolved
    } else {
        FederatedResourceDisposition::Qualified
    };
    if disposition != FederatedResourceDisposition::Qualified {
        omissions.insert("control:resource-set-not-qualified".into());
    }
    let effects = if disposition == FederatedResourceDisposition::Qualified {
        vec![format!(
            "read:authorized-resource-state:{}",
            request.need_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "need_id": request.need_id, "requester": request.requester, "intent": request.intent, "semantic_profile": request.semantic_profile, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "selected_order": selected, "unresolved_order": unresolved, "blocked_order": blocked, "missing_candidate_order": missing, "site_order": sites, "selected_site_order": selected_sites, "missing_site_order": missing_sites, "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "replay_identity": request.replay_identity, "effect_receipts": effects, "raw_data_local": true, "aggregate_only": true, "boundary": PRECLINICAL_BOUNDARY});
    let resource_digest = ContentHash::of_value(&payload)
        .map_err(|error| FederatedResourceWorkbenchError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-resource-set-5:{}", request.need_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedResourceWorkbenchError::Artifact(error.to_string()))?;
    let receipt = FederatedResourceWorkbenchReceipt8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        need_id: request.need_id.clone(),
        requester: request.requester.clone(),
        intent: request.intent.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        ranked_order: payload["ranked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_candidate_order: payload["missing_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        site_order: payload["site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_site_order: payload["selected_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_site_order: payload["missing_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        resource_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

trait LocalityRequired {
    fn raw_data_local_required(&self) -> bool;
}
impl LocalityRequired for FederatedResourceDiscoveryRequest7 {
    fn raw_data_local_required(&self) -> bool {
        self.candidates
            .iter()
            .all(|candidate| candidate.raw_data_local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedResourceDiscoveryRequest7 {
        let candidate = |id: &str, site: &str| FederatedResourceCandidate5 {
            resource_id: id.into(),
            site_id: site.into(),
            capabilities: vec!["imaging".into(), "omics".into()],
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("p-{id}")),
            semantic_profile: "resource-v1".into(),
            trust_score_milli: 900,
            evidence_state: EvidenceState::Supported,
            available: true,
            raw_data_local: true,
            permitted: true,
            revoked: false,
            negative_result: false,
        };
        FederatedResourceDiscoveryRequest7 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request-1".into(),
            need_id: "need-1".into(),
            requester: "context compiler engineer".into(),
            intent: "find resources".into(),
            semantic_profile: "resource-v1".into(),
            required_capabilities: vec!["imaging".into(), "omics".into()],
            required_site_order: vec!["site-a".into(), "site-b".into()],
            minimum_site_count: 2,
            max_results: 2,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates: vec![candidate("r-b", "site-b"), candidate("r-a", "site-a")],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_resource_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_resources_qualify() {
        let receipt = qualify_federated_resources(&request()).unwrap();
        assert_eq!(receipt.disposition, FederatedResourceDisposition::Qualified);
    }
    #[test]
    fn unavailable_is_unresolved() {
        let mut value = request();
        value.candidates[0].available = false;
        let receipt = qualify_federated_resources(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedResourceDisposition::Unresolved
        );
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        let receipt = qualify_federated_resources(&value).unwrap();
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|item| item.contains("evidence-not-asserted")));
    }
    #[test]
    fn revoked_blocks() {
        let mut value = request();
        value.candidates[0].revoked = true;
        let receipt = qualify_federated_resources(&value).unwrap();
        assert_eq!(receipt.disposition, FederatedResourceDisposition::Blocked);
    }
    #[test]
    fn federation_denial_blocks() {
        let mut value = request();
        value.federation_allow = false;
        let receipt = qualify_federated_resources(&value).unwrap();
        assert_eq!(receipt.disposition, FederatedResourceDisposition::Blocked);
    }
    #[test]
    fn ranking_is_deterministic() {
        let first = qualify_federated_resources(&request()).unwrap();
        let second = qualify_federated_resources(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
