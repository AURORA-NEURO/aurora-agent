//! Federated continual resource-discovery interoperability for `AFA-ids-P05-F24`.
//!
//! This module is deliberately below the foundation crate: `ids` is the canonicalization root and
//! cannot depend on the higher-level contract graph.  It therefore owns a small, fully typed
//! interoperability envelope and uses its own byte-stable `ContentHash` primitive.  It never
//! fetches an endpoint, moves a resource, or treats a missing/unknown resource as evidence.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P05-F24";
pub const CONTRACT_VERSION: &str =
    "ids-federated-continual-resource-discovery-interoperability/1.0";
pub const INPUT_SCHEMA: &str = "ResourceNeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedResourceSet6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.qualified-resource-set-6+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_ENDPOINTS: usize = 4096;
pub const MAX_RESULTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Proven,
    Supported,
    Speculative,
    Contradicted,
    Unknown,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointStatus {
    Available,
    Stale,
    Protected,
    Unavailable,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNeed4 {
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capabilities: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub required_protocol_version: String,
    pub max_results: usize,
    pub minimum_peer_quorum: usize,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceEndpoint4 {
    pub resource_id: String,
    pub endpoint_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub protocol_versions: Vec<String>,
    pub capabilities: Vec<String>,
    pub fitness_milli: i64,
    pub status: EndpointStatus,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerResourceSummary4 {
    pub peer_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub summary_digest: ContentHash,
    pub state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteroperabilityManifest {
    pub schema_version: String,
    pub capability_id: String,
    pub version: String,
    pub input_schema: String,
    pub output_schema: String,
    pub effects: Vec<String>,
    pub permissions: Vec<String>,
    pub autonomy_tier: String,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResource6 {
    pub resource_id: String,
    pub endpoint_id: String,
    pub origin: String,
    pub protocol_version: String,
    pub fitness_milli: i64,
    pub compatibility: String,
    pub migration_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceArtifact6 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedResourceSet6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub negotiated_protocol_version: String,
    pub disposition: ResourceDisposition,
    pub endpoint_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub resources: Vec<QualifiedResource6>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub migration_notes: Vec<String>,
    pub replay_identity: ContentHash,
    pub selection_digest: ContentHash,
    pub artifact: ResourceArtifact6,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResourceInteroperabilityError {
    #[error("invalid resource interoperability request: {0}")]
    Invalid(String),
    #[error("resource interoperability artifact failed: {0}")]
    Artifact(String),
}

pub fn interoperability_manifest() -> InteroperabilityManifest {
    InteroperabilityManifest {
        schema_version: "aurora-research-contract/1.0".into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        effects: vec!["exchange:permitted-artifacts".into()],
        permissions: vec!["connect:approved-endpoints".into()],
        autonomy_tier: "A2".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl QualifiedResourceSet6 {
    pub fn validate(&self) -> Result<(), ResourceInteroperabilityError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.negotiated_protocol_version.trim().is_empty()
            || self.endpoint_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ResourceInteroperabilityError::Invalid(
                "identity, locality, protocol, endpoints, peers, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.endpoint_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_capability_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.migration_notes,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|window| window[0] >= window[1]) {
                return Err(ResourceInteroperabilityError::Invalid(
                    "resource interoperability ordering is not canonical".into(),
                ));
            }
        }
        let endpoint_set = BTreeSet::from_iter(self.endpoint_order.iter().cloned());
        let classified = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if endpoint_set != classified || endpoint_set.len() != self.endpoint_order.len() {
            return Err(ResourceInteroperabilityError::Invalid(
                "endpoint dispositions do not partition candidates".into(),
            ));
        }
        let peer_set = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let qualified_peers = BTreeSet::from_iter(self.qualified_peer_order.iter().cloned());
        let missing_peers = BTreeSet::from_iter(self.missing_peer_order.iter().cloned());
        if peer_set
            != qualified_peers
                .union(&missing_peers)
                .cloned()
                .collect::<BTreeSet<_>>()
            || qualified_peers.len() + missing_peers.len() != peer_set.len()
        {
            return Err(ResourceInteroperabilityError::Invalid(
                "peer dispositions do not partition peers".into(),
            ));
        }
        if self.resources.len() != self.qualified_order.len()
            || self
                .resources
                .iter()
                .zip(&self.qualified_order)
                .any(|(resource, resource_id)| &resource.resource_id != resource_id)
        {
            return Err(ResourceInteroperabilityError::Invalid(
                "qualified resources do not match qualified order".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.selection_digest
        {
            return Err(ResourceInteroperabilityError::Artifact(
                "artifact content type or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(ResourceInteroperabilityError::Invalid(
                "effect is outside the interoperability gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ResourceInteroperabilityError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ResourceInteroperabilityError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ResourceInteroperabilityError::Artifact(error.to_string()))
    }
}

pub fn interoperate_resources(
    request: &ResourceNeed4,
    endpoints: &[ResourceEndpoint4],
    peers: &[PeerResourceSummary4],
) -> Result<QualifiedResourceSet6, ResourceInteroperabilityError> {
    validate_request(request, endpoints, peers)?;
    let mut endpoint_rows = endpoints.to_vec();
    endpoint_rows.sort_by(|left, right| {
        right
            .fitness_milli
            .cmp(&left.fitness_milli)
            .then(left.resource_id.cmp(&right.resource_id))
            .then(left.endpoint_id.cmp(&right.endpoint_id))
    });
    let endpoint_order = endpoint_rows
        .iter()
        .map(|endpoint| endpoint.resource_id.clone())
        .collect::<Vec<_>>();
    let mut peer_rows = peers.to_vec();
    peer_rows.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let peer_order = peer_rows
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for peer in &peer_rows {
        let qualified = peer.semantic_profile == request.semantic_profile
            && peer.protocol_version == request.required_protocol_version
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(peer.state, EvidenceState::Proven | EvidenceState::Supported);
        if qualified {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
        if peer.state == EvidenceState::Contradicted {
            uncertainty.insert(format!("peer:{}:contradicted", peer.peer_id));
        }
    }
    let allowed_origins = request
        .allowed_origins
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_capabilities = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut migration_notes = BTreeSet::new();
    for endpoint in &endpoint_rows {
        let id = endpoint.resource_id.clone();
        if endpoint.negative_result {
            negative.insert(format!("{}:negative-result", id));
        }
        for reason in &endpoint.omission_reasons {
            omissions.insert(format!("{}:{}", id, reason));
        }
        let endpoint_capabilities: BTreeSet<String> =
            endpoint.capabilities.iter().cloned().collect();
        let missing = required
            .difference(&endpoint_capabilities)
            .cloned()
            .collect::<Vec<_>>();
        let mut reasons: Vec<String> = Vec::new();
        if endpoint.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch".into());
        }
        if !allowed_origins.is_empty() && !allowed_origins.contains(&endpoint.origin) {
            reasons.push("origin-out-of-scope".into());
        }
        if missing.is_empty() {
            // no-op; the explicit branch keeps missing-capability evidence separate below
        } else {
            for capability in missing {
                missing_capabilities.insert(format!("{}:{}", id, capability));
            }
            reasons.push("required-capability-missing".into());
        }
        if !endpoint
            .protocol_versions
            .contains(&request.required_protocol_version)
        {
            reasons.push("protocol-version-unavailable".into());
        }
        if endpoint.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch".into());
        }
        if !endpoint.signed {
            reasons.push("endpoint-signature-missing".into());
        }
        if !endpoint.permitted {
            reasons.push("endpoint-policy-denied".into());
        }
        if !endpoint.raw_data_local || !endpoint.aggregate_only {
            reasons.push("endpoint-locality-or-aggregate-only-failed".into());
        }
        match endpoint.status {
            EndpointStatus::Available => {}
            EndpointStatus::Stale => {
                reasons.push("stale-endpoint".into());
            }
            EndpointStatus::Protected | EndpointStatus::Revoked => {
                reasons.push("protected-or-revoked-endpoint".into());
            }
            EndpointStatus::Unavailable => reasons.push("endpoint-unavailable".into()),
        }
        match endpoint.evidence_state {
            EvidenceState::Proven | EvidenceState::Supported => {}
            EvidenceState::Contradicted => {
                reasons.push("contradicted-evidence".into());
                negative.insert(format!("{}:contradicted", id));
            }
            EvidenceState::Speculative | EvidenceState::Unknown | EvidenceState::Unmeasured => {
                reasons.push("evidence-state-unresolved".into());
                uncertainty.insert(format!("{}:evidence-state", id));
            }
        }
        if reasons.iter().any(|reason| {
            reason == "protected-or-revoked-endpoint"
                || reason == "replay-identity-mismatch"
                || reason == "contradicted-evidence"
                || reason == "endpoint-policy-denied"
                || reason == "endpoint-locality-or-aggregate-only-failed"
        }) {
            blocked.insert(id);
        } else if reasons.is_empty() && qualified.len() < request.max_results {
            qualified.push(QualifiedResource6 {
                resource_id: id,
                endpoint_id: endpoint.endpoint_id.clone(),
                origin: endpoint.origin.clone(),
                protocol_version: request.required_protocol_version.clone(),
                fitness_milli: endpoint.fitness_milli,
                compatibility: "native".into(),
                migration_notes: Vec::new(),
            });
        } else {
            if qualified.len() >= request.max_results {
                omissions.insert(format!("{}:result-limit", id));
            }
            unresolved.insert(id);
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    if global_block {
        blocked.extend(endpoint_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:global-interoperability-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        ResourceDisposition::Blocked
    } else if qualified_peers.len() < request.minimum_peer_quorum || qualified.is_empty() {
        ResourceDisposition::Unresolved
    } else {
        ResourceDisposition::Qualified
    };
    if disposition != ResourceDisposition::Qualified && qualified.is_empty() {
        omissions.insert("request:no-qualified-resource".into());
    }
    qualified.sort_by(|left, right| left.resource_id.cmp(&right.resource_id));
    let qualified_order = qualified
        .iter()
        .map(|resource| resource.resource_id.clone())
        .collect::<Vec<_>>();
    for endpoint in &endpoint_rows {
        if endpoint
            .protocol_versions
            .contains(&request.required_protocol_version)
        {
            continue;
        }
        if endpoint
            .protocol_versions
            .iter()
            .any(|version| version.starts_with("1."))
            && request.required_protocol_version.starts_with("1.")
        {
            migration_notes.insert(format!(
                "{}:protocol-major-compatible-minor-migration",
                endpoint.resource_id
            ));
        }
    }
    let mut payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "requester": request.requester,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "negotiated_protocol_version": request.required_protocol_version,
        "disposition": disposition,
        "endpoint_order": endpoint_order,
        "qualified_order": qualified_order,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers,
        "missing_peer_order": missing_peers,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "migration_notes": migration_notes,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let selection_digest = ContentHash::of_value(&payload)
        .map_err(|error| ResourceInteroperabilityError::Artifact(error.to_string()))?;
    payload["selection_digest"] = json!(selection_digest);
    let provenance_digests = endpoint_rows
        .iter()
        .map(|endpoint| endpoint.provenance_digest.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let artifact = ResourceArtifact6 {
        artifact_id: format!("qualified-resource-set-6:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: selection_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == ResourceDisposition::Qualified {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = QualifiedResourceSet6 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        negotiated_protocol_version: request.required_protocol_version.clone(),
        disposition,
        endpoint_order,
        qualified_order,
        unresolved_order: unresolved.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        missing_capability_order: missing_capabilities.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        resources: qualified,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        migration_notes: migration_notes.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        selection_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ResourceNeed4,
    endpoints: &[ResourceEndpoint4],
    peers: &[PeerResourceSummary4],
) -> Result<(), ResourceInteroperabilityError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_protocol_version.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request.max_results == 0
        || request.max_results > MAX_RESULTS
        || request.minimum_peer_quorum == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || endpoints.is_empty()
        || peers.is_empty()
        || endpoints.len() > MAX_ENDPOINTS
    {
        return Err(ResourceInteroperabilityError::Invalid(
            "request identity, capabilities, bounds, locality, peers, endpoints, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for endpoint in endpoints {
        if endpoint.resource_id.trim().is_empty()
            || endpoint.endpoint_id.trim().is_empty()
            || endpoint.origin.trim().is_empty()
            || !ids.insert(endpoint.resource_id.clone())
            || endpoint.artifact_digest.as_str().len() != 64
            || endpoint.provenance_digest.as_str().len() != 64
            || endpoint.replay_identity.as_str().len() != 64
        {
            return Err(ResourceInteroperabilityError::Invalid(
                "endpoint identity, uniqueness, or digests are invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in peers {
        if peer.peer_id.trim().is_empty()
            || peer.origin.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.summary_digest.as_str().len() != 64
        {
            return Err(ResourceInteroperabilityError::Invalid(
                "peer identity, uniqueness, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }
    fn endpoint(id: &str, fitness: i64, state: EvidenceState) -> ResourceEndpoint4 {
        ResourceEndpoint4 {
            resource_id: id.into(),
            endpoint_id: format!("endpoint:{id}"),
            origin: "site-a".into(),
            semantic_profile: "resource:v1".into(),
            protocol_versions: vec!["1.0".into()],
            capabilities: vec!["imaging".into(), "rna".into()],
            fitness_milli: fitness,
            status: EndpointStatus::Available,
            evidence_state: state,
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    fn peer(id: &str, state: EvidenceState) -> PeerResourceSummary4 {
        PeerResourceSummary4 {
            peer_id: id.into(),
            origin: id.into(),
            semantic_profile: "resource:v1".into(),
            protocol_version: "1.0".into(),
            summary_digest: hash(id),
            state,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    fn request() -> ResourceNeed4 {
        ResourceNeed4 {
            request_id: "request:resource".into(),
            federation_id: "federation:resource".into(),
            requester: "computational-biologist".into(),
            purpose: "resource-discovery".into(),
            semantic_profile: "resource:v1".into(),
            required_capabilities: vec!["imaging".into(), "rna".into()],
            allowed_origins: vec!["site-a".into()],
            required_protocol_version: "1.0".into(),
            max_results: 2,
            minimum_peer_quorum: 1,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a2_and_exchange_only() {
        let manifest = interoperability_manifest();
        assert_eq!(manifest.autonomy_tier, "A2");
        assert_eq!(manifest.effects, vec!["exchange:permitted-artifacts"]);
    }
    #[test]
    fn deterministic_fitness_order_and_qualified_exchange() {
        let result = interoperate_resources(
            &request(),
            &[
                endpoint("resource:b", 700, EvidenceState::Supported),
                endpoint("resource:a", 900, EvidenceState::Proven),
            ],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDisposition::Qualified);
        assert_eq!(result.qualified_order, vec!["resource:a", "resource:b"]);
        assert_eq!(
            result.effect_receipts,
            vec!["exchange:permitted-artifacts:request:resource"]
        );
        assert_eq!(result.digest().unwrap(), result.digest().unwrap());
    }
    #[test]
    fn unknown_evidence_is_unresolved_and_blocked_from_exchange() {
        let result = interoperate_resources(
            &request(),
            &[endpoint("resource:a", 900, EvidenceState::Unknown)],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDisposition::Unresolved);
        assert!(result
            .uncertainty
            .iter()
            .any(|value| value.contains("evidence-state")));
        assert_eq!(result.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn contradiction_is_negative_and_blocked() {
        let result = interoperate_resources(
            &request(),
            &[endpoint("resource:a", 900, EvidenceState::Contradicted)],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDisposition::Blocked);
        assert!(result
            .negative_evidence
            .iter()
            .any(|value| value.contains("contradicted")));
    }
    #[test]
    fn missing_capability_is_retained() {
        let mut value = endpoint("resource:a", 900, EvidenceState::Supported);
        value.capabilities = vec!["imaging".into()];
        let result = interoperate_resources(
            &request(),
            &[value],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert!(result
            .missing_capability_order
            .iter()
            .any(|value| value.contains("rna")));
        assert_eq!(result.disposition, ResourceDisposition::Unresolved);
    }
    #[test]
    fn protocol_and_peer_quorum_are_explicit() {
        let mut value = request();
        value.minimum_peer_quorum = 2;
        let mut candidate = endpoint("resource:a", 900, EvidenceState::Supported);
        candidate.protocol_versions = vec!["2.0".into()];
        let result = interoperate_resources(
            &value,
            &[candidate],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDisposition::Unresolved);
        assert!(result
            .uncertainty
            .iter()
            .any(|item| item.contains("quorum")));
    }
    #[test]
    fn policy_denial_fails_closed() {
        let mut value = request();
        value.policy_allow = false;
        let result = interoperate_resources(
            &value,
            &[endpoint("resource:a", 900, EvidenceState::Supported)],
            &[peer("peer:a", EvidenceState::Supported)],
        )
        .unwrap();
        assert_eq!(result.disposition, ResourceDisposition::Blocked);
        assert_eq!(result.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_endpoint_is_rejected() {
        let result = interoperate_resources(
            &request(),
            &[
                endpoint("resource:a", 900, EvidenceState::Supported),
                endpoint("resource:a", 800, EvidenceState::Supported),
            ],
            &[peer("peer:a", EvidenceState::Supported)],
        );
        assert!(result.is_err());
    }
}
