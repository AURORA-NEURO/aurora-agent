//! Federated continual retrieval-and-synthesis assurance.
//!
//! Atlas feature: `AFA-cli-P02-F28`.
//!
//! This is a federation boundary around the local retrieval verifier.  It consumes peer-supplied
//! signed summaries, never opens a network connection, and only emits an aggregate-only exchange
//! receipt when peer quorum, semantic-profile, replay, artifact, policy, and approval gates close.
//! Raw evidence and peer payloads remain institution-local.

use super::retrieval_synthesis_assurance::{
    verify as verify_local, RetrievalDisposition, RetrievalEvidenceCandidate, ScopedRetrievalQuery,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P02-F28";
pub const CONTRACT_VERSION: &str = "cli-federated-continual-retrieval-synthesis-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerEvidenceSummary {
    pub institution_id: String,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub permitted_artifact: String,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedScopedRetrievalQuery {
    pub request_id: String,
    pub corpus_id: String,
    pub scope: String,
    pub query: String,
    pub query_schema: String,
    pub candidates: Vec<RetrievalEvidenceCandidate>,
    pub required_source_ids: Vec<String>,
    pub min_independent_sources: u32,
    pub max_selected: usize,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub max_freshness_days: u32,
    pub replay_identity: ContentHash,
    pub federation_id: String,
    pub purpose: String,
    pub origin_institution: String,
    pub peer_institution_order: Vec<String>,
    pub required_peer_quorum: u32,
    pub peer_evidence: Vec<PeerEvidenceSummary>,
    pub semantic_profile: String,
    pub permitted_artifact_order: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub corpus_id: String,
    pub scope: String,
    pub query: String,
    pub disposition: RetrievalDisposition,
    pub candidate_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub required_source_order: Vec<String>,
    pub observed_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub blocked_peer_order: Vec<String>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub local_evidence_digest: ContentHash,
    pub federation_envelope_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedRetrievalAssuranceError {
    #[error("invalid federated retrieval assurance request: {0}")]
    Invalid(String),
    #[error("federated retrieval assurance artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedRetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.corpus_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.query.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.rank_order.len() != self.candidate_order.len()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.checks.is_empty()
        {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "identity, locality, query, candidate, peer, checks, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.required_source_order,
            &self.observed_source_order,
            &self.missing_source_order,
            &self.stale_order,
            &self.contradiction_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.blocked_peer_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(FederatedRetrievalAssuranceError::Invalid(
                    "federated retrieval orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if self.rank_order.iter().cloned().collect::<BTreeSet<_>>() != candidates {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "rank order is not a permutation of candidates".into(),
            ));
        }
        let assigned = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if assigned.len() != candidates.len()
            || assigned.iter().collect::<BTreeSet<_>>().len() != assigned.len()
            || assigned.iter().cloned().collect::<BTreeSet<_>>() != candidates
        {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "candidate disposition partition is incomplete".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_partition = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .chain(self.blocked_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_partition.len() != peers.len()
            || peer_partition.iter().collect::<BTreeSet<_>>().len() != peer_partition.len()
            || peer_partition.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "peer disposition partition is incomplete".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:aggregate-evidence:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "effect is outside aggregate-only federation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "cli".into(),
        consumers: BTreeSet::from([
            "federation operator".into(),
            "AURORA extension developer".into(),
            "release-evidence reviewer".into(),
        ]),
        behavior: "verifies local and peer retrieval summaries and emits aggregate-only evidence exchange verdicts without network or raw-data effects".into(),
        value: "makes continual federation useful while retaining peer uncertainty, semantic disagreement, replay identity, provenance, and policy denial".into(),
        inputs: vec![TypedPort { name: "federated_scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "federated_evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]),
        permissions: BTreeSet::from(["evaluate:capability-runs".into(), "exchange:aggregate-evidence".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "slsa-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "federation-evidence-reviewer".into(), reason: "approve aggregate-only retrieval evidence exchange".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &FederatedScopedRetrievalQuery,
) -> Result<(), FederatedRetrievalAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.corpus_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.query_schema != INPUT_SCHEMA
        || request.candidates.is_empty()
        || request.required_source_ids.is_empty()
        || request.peer_institution_order.is_empty()
        || request.required_peer_quorum == 0
        || request.required_peer_quorum as usize > request.peer_institution_order.len()
        || request.peer_evidence.is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.permitted_artifact_order.is_empty()
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || request.max_freshness_days == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalAssuranceError::Invalid(
            "identity, schema, peer quorum, semantic profile, bounds, locality, or boundary is invalid".into(),
        ));
    }
    for values in [
        &request.required_source_ids,
        &request.peer_institution_order,
        &request.permitted_artifact_order,
    ] {
        if !canonical(values) || values.iter().any(|value| value.trim().is_empty()) {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "federation declarations must be unique, non-empty, and canonical".into(),
            ));
        }
    }
    if request.origin_institution.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || !request
            .permitted_artifact_order
            .iter()
            .any(|item| item == "evidence-synthesis")
    {
        return Err(FederatedRetrievalAssuranceError::Invalid(
            "federation identity, purpose, or evidence-synthesis allow-list is incomplete".into(),
        ));
    }
    let peer_ids = request
        .peer_evidence
        .iter()
        .map(|peer| peer.institution_id.clone())
        .collect::<BTreeSet<_>>();
    if peer_ids.len() != request.peer_evidence.len()
        || request
            .peer_institution_order
            .iter()
            .any(|peer| !peer_ids.contains(peer))
        || request.peer_evidence.iter().any(|peer| {
            peer.institution_id.trim().is_empty()
                || peer.semantic_profile.trim().is_empty()
                || peer.permitted_artifact.trim().is_empty()
                || peer.omissions.iter().any(|item| item.trim().is_empty())
                || peer.uncertainty.iter().any(|item| item.trim().is_empty())
        })
    {
        return Err(FederatedRetrievalAssuranceError::Invalid(
            "peer identity, coverage, or annotations are invalid".into(),
        ));
    }
    if request
        .adversarial_events
        .iter()
        .any(|event| event.trim().is_empty())
    {
        return Err(FederatedRetrievalAssuranceError::Invalid(
            "adversarial event labels must be non-empty".into(),
        ));
    }
    Ok(())
}

pub fn verify(
    request: &FederatedScopedRetrievalQuery,
) -> Result<FederatedRetrievalAssuranceReceipt, FederatedRetrievalAssuranceError> {
    validate_request(request)?;
    let peer_ids = request
        .peer_institution_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified_peer = BTreeSet::new();
    let mut blocked_peer = BTreeSet::new();
    let mut missing_peer = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for peer in &request.peer_evidence {
        let mut hard = peer.permitted_artifact != "evidence-synthesis"
            || !request
                .permitted_artifact_order
                .contains(&peer.permitted_artifact)
            || peer.semantic_profile != request.semantic_profile
            || peer.replay_identity != request.replay_identity
            || peer.artifact_digest.is_none()
            || peer.provenance_digest.is_none()
            || matches!(peer.evidence_state, EvidenceState::Contradicted);
        if !peer_ids.contains(&peer.institution_id) {
            hard = true;
            omissions.insert(format!("peer:{}:not-declared", peer.institution_id));
        }
        if peer.artifact_digest.is_none() {
            omissions.insert(format!(
                "peer:{}:artifact-digest-missing",
                peer.institution_id
            ));
        }
        if peer.provenance_digest.is_none() {
            omissions.insert(format!(
                "peer:{}:provenance-digest-missing",
                peer.institution_id
            ));
        }
        if peer.semantic_profile != request.semantic_profile {
            hard = true;
            omissions.insert(format!(
                "peer:{}:semantic-profile-mismatch",
                peer.institution_id
            ));
        }
        if peer.replay_identity != request.replay_identity {
            hard = true;
            omissions.insert(format!("peer:{}:replay-mismatch", peer.institution_id));
        }
        if matches!(
            peer.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            uncertainty.insert(format!("peer:{}:evidence-state", peer.institution_id));
        }
        for item in &peer.omissions {
            omissions.insert(format!("peer:{}:{item}", peer.institution_id));
        }
        for item in &peer.uncertainty {
            uncertainty.insert(format!("peer:{}:{item}", peer.institution_id));
        }
        if peer.negative_result {
            negative.insert(format!("peer:{}:negative-result", peer.institution_id));
        } else {
            negative.insert(format!(
                "peer:{}:negative-result-not-observed",
                peer.institution_id
            ));
        }
        if hard {
            blocked_peer.insert(peer.institution_id.clone());
        } else if matches!(
            peer.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            qualified_peer.insert(peer.institution_id.clone());
        } else {
            missing_peer.insert(peer.institution_id.clone());
        }
    }
    for peer in peer_ids
        .difference(&qualified_peer)
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        if !blocked_peer.contains(&peer) {
            missing_peer.insert(peer);
        }
    }
    let quorum_ok = qualified_peer.len() >= request.required_peer_quorum as usize;
    if !quorum_ok {
        omissions.insert(format!(
            "peer-quorum:{}/{}",
            qualified_peer.len(),
            request.required_peer_quorum
        ));
        uncertainty.insert("federation:peer-quorum-incomplete".into());
    }
    if !request.policy_allow {
        omissions.insert("federation:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("federation:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("federation:signed-approval-missing".into());
    }
    for event in &request.adversarial_events {
        omissions.insert(format!("federation:adversarial:{event}"));
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.adversarial_events.is_empty()
        || !blocked_peer.is_empty();
    let local_request = ScopedRetrievalQuery {
        request_id: request.request_id.clone(),
        corpus_id: request.corpus_id.clone(),
        scope: request.scope.clone(),
        query: request.query.clone(),
        query_schema: "ScopedRetrievalQuery3@1".into(),
        candidates: request.candidates.clone(),
        required_source_ids: request.required_source_ids.clone(),
        min_independent_sources: request.min_independent_sources,
        max_selected: request.max_selected,
        budget_units: request.budget_units,
        max_budget_units: request.max_budget_units,
        max_freshness_days: request.max_freshness_days,
        replay_identity: request.replay_identity.clone(),
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        raw_data_local: request.raw_data_local,
        adversarial_events: request.adversarial_events.clone(),
        boundary: request.boundary.clone(),
    };
    let local = verify_local(&local_request)
        .map_err(|error| FederatedRetrievalAssuranceError::Invalid(error.to_string()))?;
    let mut semantic_loss = local.artifact.semantic_loss.clone();
    if global_block {
        semantic_loss.push(SemanticLoss {
            field: "federation".into(),
            reason: "peer, policy, approval, or adversarial gate blocks aggregate exchange".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let disposition = if global_block || local.disposition == RetrievalDisposition::Blocked {
        RetrievalDisposition::Blocked
    } else if local.disposition == RetrievalDisposition::Unresolved || !quorum_ok {
        RetrievalDisposition::Unresolved
    } else {
        RetrievalDisposition::Qualified
    };
    let peer_order = request.peer_institution_order.clone();
    let qualified_peer_order = qualified_peer.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peer.into_iter().collect::<Vec<_>>();
    let blocked_peer_order = blocked_peer.into_iter().collect::<Vec<_>>();
    let exchange_payload = json!({
        "schema_version": OUTPUT_SCHEMA,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "origin_institution": request.origin_institution,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "local_evidence_digest": local.evidence_digest,
        "replay_identity": request.replay_identity,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "raw_data_local": true,
    });
    let federation_envelope_digest = ContentHash::of_value(&exchange_payload)
        .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "corpus_id": request.corpus_id,
        "scope": request.scope,
        "query": request.query,
        "candidate_order": local.candidate_order,
        "rank_order": local.rank_order,
        "selected_order": local.selected_order,
        "unresolved_order": local.unresolved_order,
        "blocked_order": local.blocked_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "blocked_peer_order": blocked_peer_order,
        "local_evidence_digest": local.evidence_digest,
        "federation_envelope_digest": federation_envelope_digest,
        "replay_identity": request.replay_identity,
        "disposition": disposition,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact_digest = ContentHash::of_value(&payload)
        .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-evidence-synthesis:{}", request.request_id),
        "application/vnd.aurora.federated-evidence-synthesis+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "federated-retrieval-synthesis-assurance".into(),
            digest: artifact_digest.clone(),
        }],
    )
    .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == RetrievalDisposition::Qualified {
        vec![format!(
            "exchange:aggregate-evidence:{}",
            request.federation_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let mut checks = vec![
        "schema-version".into(),
        "local-retrieval-verdict".into(),
        "peer-artifact-allow-list".into(),
        "peer-provenance-closure".into(),
        "peer-semantic-profile".into(),
        "peer-replay-identity".into(),
        "peer-quorum".into(),
        "aggregate-only-locality".into(),
        "signed-approval".into(),
        "negative-evidence-retention".into(),
    ];
    checks.sort();
    let all_omissions = omissions
        .into_iter()
        .chain(local.omissions)
        .collect::<BTreeSet<_>>();
    let all_uncertainty = uncertainty
        .into_iter()
        .chain(local.uncertainty)
        .collect::<BTreeSet<_>>();
    let all_negative = negative
        .into_iter()
        .chain(local.negative_evidence)
        .collect::<BTreeSet<_>>();
    let receipt = FederatedRetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        corpus_id: request.corpus_id.clone(),
        scope: request.scope.clone(),
        query: request.query.clone(),
        disposition,
        candidate_order: local.candidate_order,
        rank_order: local.rank_order,
        selected_order: local.selected_order,
        unresolved_order: local.unresolved_order,
        blocked_order: local.blocked_order,
        required_source_order: local.required_source_order,
        observed_source_order: local.observed_source_order,
        missing_source_order: local.missing_source_order,
        stale_order: local.stale_order,
        contradiction_order: local.contradiction_order,
        peer_order,
        qualified_peer_order,
        missing_peer_order,
        blocked_peer_order,
        checks,
        omissions: all_omissions.into_iter().collect(),
        uncertainty: all_uncertainty.into_iter().collect(),
        negative_evidence: all_negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        local_evidence_digest: local.evidence_digest,
        federation_envelope_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn verify_json(value: &Value) -> Result<Value, FederatedRetrievalAssuranceError> {
    let request: FederatedScopedRetrievalQuery = serde_json::from_value(value.clone())
        .map_err(|error| FederatedRetrievalAssuranceError::Invalid(error.to_string()))?;
    serde_json::to_value(verify(&request)?)
        .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"federated-retrieval-assurance")
    }
    fn candidate(id: &str, source: &str, state: EvidenceState) -> RetrievalEvidenceCandidate {
        RetrievalEvidenceCandidate {
            candidate_id: id.into(),
            source_id: source.into(),
            title: format!("study {id}"),
            relevance_milli: 90_000,
            evidence_state: state,
            content_digest: Some(hash()),
            provenance_digest: Some(hash()),
            freshness_days: 2,
            negative_result: false,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn peer(id: &str, state: EvidenceState) -> PeerEvidenceSummary {
        PeerEvidenceSummary {
            institution_id: id.into(),
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            replay_identity: hash(),
            semantic_profile: "prov-o+ro-crate".into(),
            evidence_state: state,
            permitted_artifact: "evidence-synthesis".into(),
            negative_result: false,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn request() -> FederatedScopedRetrievalQuery {
        FederatedScopedRetrievalQuery {
            request_id: "request:federated-retrieval".into(),
            corpus_id: "corpus:preclinical".into(),
            scope: "organoid-neuroscience".into(),
            query: "mechanism of synaptic resilience".into(),
            query_schema: INPUT_SCHEMA.into(),
            candidates: vec![
                candidate("c-1", "source-a", EvidenceState::Supported),
                candidate("c-2", "source-b", EvidenceState::Proven),
            ],
            required_source_ids: vec!["source-a".into(), "source-b".into()],
            min_independent_sources: 2,
            max_selected: 4,
            budget_units: 100,
            max_budget_units: 100,
            max_freshness_days: 30,
            replay_identity: hash(),
            federation_id: "federation:preclinical".into(),
            purpose: "replication-benchmark".into(),
            origin_institution: "institution-a".into(),
            peer_institution_order: vec!["institution-b".into(), "institution-c".into()],
            required_peer_quorum: 2,
            peer_evidence: vec![
                peer("institution-b", EvidenceState::Supported),
                peer("institution-c", EvidenceState::Proven),
            ],
            semantic_profile: "prov-o+ro-crate".into(),
            permitted_artifact_order: vec!["evidence-synthesis".into()],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_peer_quorum_emits_aggregate_exchange() {
        let receipt = verify(&request()).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Qualified);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
        assert!(receipt.effect_receipts[0].starts_with("exchange:aggregate-evidence:"));
    }
    #[test]
    fn missing_peer_quorum_is_unresolved() {
        let mut value = request();
        value.peer_evidence[1].evidence_state = EvidenceState::Unknown;
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Unresolved);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("peer-quorum")));
    }
    #[test]
    fn semantic_profile_or_artifact_mismatch_blocks() {
        let mut value = request();
        value.peer_evidence[0].semantic_profile = "other-profile".into();
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Blocked);
        assert!(receipt.blocked_peer_order.contains(&"institution-b".into()));
    }
    #[test]
    fn local_contradiction_and_negative_evidence_remain_visible() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        value.candidates[0].negative_result = true;
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-result")));
    }
    #[test]
    fn approval_and_adversarial_gates_fail_closed() {
        let mut value = request();
        value.signed_approval = false;
        value.adversarial_events = vec!["poisoned-peer-artifact".into()];
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn manifest_is_a1_and_federated() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .effects
            .contains(&Effect::FederationExport));
    }
}
