//! Federated continual retrieval-and-synthesis assurance.
//!
//! Atlas feature: `AFA-lab-P02-F28`.  This read-only gate evaluates local
//! retrieval attestations and peer summaries before an evidence synthesis can
//! enter a downstream preclinical workflow.  It never performs network search,
//! invents a synthesis, or exports raw evidence.

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

pub const FEATURE_ID: &str = "AFA-lab-P02-F28";
pub const CONTRACT_VERSION: &str =
    "lab-federated-continual-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.evidence-synthesis-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub evidence_id: String,
    pub study_id: String,
    pub source_id: String,
    pub scope: String,
    pub relevance_milli: u16,
    pub freshness_epoch: u64,
    pub semantic_profile: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSynthesisSummary {
    pub institution_id: String,
    pub evidence_digest: ContentHash,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub schema_version: String,
    pub query_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_evidence_order: Vec<String>,
    pub required_scope_order: Vec<String>,
    pub minimum_freshness_epoch: u64,
    pub candidates: Vec<RetrievalCandidate>,
    pub peers: Vec<PeerSynthesisSummary>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub query_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: SynthesisDisposition,
    pub evidence_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_evidence_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub missing_scope_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetrievalSynthesisError {
    #[error("invalid retrieval query: {0}")]
    Invalid(String),
    #[error("retrieval synthesis artifact failed: {0}")]
    Artifact(String),
}
fn invalid(value: impl Into<String>) -> RetrievalSynthesisError {
    RetrievalSynthesisError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl EvidenceSynthesis {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.query_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.evidence_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "synthesis identity, locality, evidence, peers, or effects are incomplete",
            ));
        }
        for values in [
            &self.evidence_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_evidence_order,
            &self.stale_order,
            &self.missing_scope_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("synthesis ordering is not canonical"));
            }
        }
        let ids = self.evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid(
                "synthesis evidence states do not partition candidates",
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("synthesis peer states do not partition peers"));
        }
        for value in [
            &self.replay_identity,
            &self.synthesis_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("synthesis digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("synthesis artifact type is invalid"));
        }
        if self.disposition == SynthesisDisposition::Qualified
            && self.effect_receipts != [format!("verify:evidence-synthesis:{}", self.query_id)]
        {
            return Err(invalid("qualified synthesis effect is invalid"));
        }
        if self.disposition != SynthesisDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified synthesis must block release"));
        }
        Ok(())
    }
}

pub fn federated_retrieval_synthesis_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "lab".into(), consumers: BTreeSet::from([String::from("bioinformatician"), String::from("research data steward"), String::from("consortium operator")]), behavior: "verifies local retrieval attestations and aggregate-only peer synthesis summaries under explicit evidence, freshness, scope, provenance, replay, and policy gates without performing retrieval".into(), value: "prevents stale, incomparable, contradictory, or unauthorized evidence from silently becoming a federated synthesis".into(), inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: BTreeSet::from([String::from("evaluate:capability-runs")]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_federated_retrieval_synthesis(
    query: &ScopedRetrievalQuery,
) -> Result<EvidenceSynthesis, RetrievalSynthesisError> {
    validate_query(query)?;
    let mut candidates = query.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let mut evidence_order = candidates
        .iter()
        .map(|row| row.evidence_id.clone())
        .collect::<Vec<_>>();
    evidence_order.sort();
    let required = query
        .required_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut missing_scope = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let scopes = query
        .required_scope_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let covered_scopes = candidates
        .iter()
        .filter(|row| scopes.contains(&row.scope))
        .map(|row| row.scope.clone())
        .collect::<BTreeSet<_>>();
    missing_scope.extend(scopes.difference(&covered_scopes).cloned());
    for row in &candidates {
        if row.negative_result {
            negative.insert(format!("{}:negative-result", row.evidence_id));
        }
        omissions.extend(
            row.omissions
                .iter()
                .map(|item| format!("{}:{item}", row.evidence_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", row.evidence_id)),
        );
        if row.freshness_epoch < query.minimum_freshness_epoch {
            stale.insert(row.evidence_id.clone());
            unresolved.insert(row.evidence_id.clone());
        } else if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.evidence_id.clone());
            blocked.insert(row.evidence_id.clone());
        } else if !row.local_only || !row.permitted {
            blocked.insert(row.evidence_id.clone());
        } else if row.semantic_profile != query.semantic_profile
            || row.replay_identity != query.replay_identity
            || row.relevance_milli < 600
            || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(row.evidence_id.clone());
        } else if !scopes.contains(&row.scope) {
            missing_scope.insert(row.scope.clone());
            unresolved.insert(row.evidence_id.clone());
        } else {
            selected.insert(row.evidence_id.clone());
        }
    }
    let missing_evidence = required
        .difference(&evidence_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing_evidence {
        omissions.insert(format!("{id}:required-evidence-missing"));
    }
    for scope in &missing_scope {
        omissions.insert(format!("required-scope-missing:{scope}"));
    }
    let mut peer_order = query
        .peers
        .iter()
        .map(|peer| peer.institution_id.clone())
        .collect::<Vec<_>>();
    peer_order.sort();
    let mut qualified_peer = BTreeSet::new();
    let mut missing_peer = BTreeSet::new();
    for peer in &query.peers {
        if peer.signed
            && peer.permitted
            && peer.aggregate_only
            && peer.semantic_profile == query.semantic_profile
            && peer.replay_identity == query.replay_identity
            && digest(&peer.evidence_digest)
        {
            qualified_peer.insert(peer.institution_id.clone());
        } else {
            missing_peer.insert(peer.institution_id.clone());
        }
    }
    if qualified_peer.len() < query.minimum_peer_quorum as usize {
        uncertainty.insert("request:peer-quorum-incomplete".into());
    }
    if !query.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !query.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !query.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !query.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        query
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !query.policy_allow
        || !query.protected_closure
        || !query.signed_approval
        || !query.federation_approved
        || !query.raw_data_local
        || !query.aggregate_only
        || !query.adversarial_events.is_empty();
    if global_block {
        blocked.extend(evidence_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:synthesis-release-gate-blocked".into());
    }
    let disposition = if global_block {
        SynthesisDisposition::Blocked
    } else if required.is_subset(&selected)
        && missing_scope.is_empty()
        && qualified_peer.len() >= query.minimum_peer_quorum as usize
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        SynthesisDisposition::Qualified
    } else {
        SynthesisDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_evidence_order = missing_evidence.into_iter().collect::<Vec<_>>();
    let stale_order = stale.into_iter().collect::<Vec<_>>();
    let missing_scope_order = missing_scope.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peer.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peer.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let contradiction_order = contradiction.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == SynthesisDisposition::Qualified {
        vec![format!("verify:evidence-synthesis:{}", query.query_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"query_id":query.query_id,"federation_id":query.federation_id,"semantic_profile":query.semantic_profile,"disposition":disposition,"evidence_order":evidence_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_evidence_order":missing_evidence_order,"stale_order":stale_order,"missing_scope_order":missing_scope_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"contradiction_order":contradiction_order,"negative_evidence_order":negative_evidence_order,"replay_identity":query.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":query.raw_data_local,"aggregate_only":query.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("evidence-synthesis:{}", query.query_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
    let synthesis = EvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        query_id: query.query_id.clone(),
        federation_id: query.federation_id.clone(),
        semantic_profile: query.semantic_profile.clone(),
        disposition,
        evidence_order,
        selected_order,
        unresolved_order,
        blocked_order,
        missing_evidence_order,
        stale_order,
        missing_scope_order,
        peer_order,
        qualified_peer_order,
        missing_peer_order,
        omission_order,
        uncertainty_order,
        contradiction_order,
        negative_evidence_order,
        replay_identity: query.replay_identity.clone(),
        synthesis_digest,
        artifact,
        effect_receipts,
        raw_data_local: query.raw_data_local,
        aggregate_only: query.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    synthesis.validate()?;
    Ok(synthesis)
}

fn validate_query(query: &ScopedRetrievalQuery) -> Result<(), RetrievalSynthesisError> {
    if query.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || query.query_id.trim().is_empty()
        || query.federation_id.trim().is_empty()
        || query.semantic_profile.trim().is_empty()
        || query.required_evidence_order.is_empty()
        || query
            .required_evidence_order
            .iter()
            .any(|evidence_id| evidence_id.trim().is_empty())
        || !canonical(&query.required_evidence_order)
        || query.required_scope_order.is_empty()
        || query
            .required_scope_order
            .iter()
            .any(|scope| scope.trim().is_empty())
        || !canonical(&query.required_scope_order)
        || query.candidates.is_empty()
        || query.peers.is_empty()
        || query.minimum_peer_quorum == 0
        || !digest(&query.replay_identity)
        || !canonical(&query.adversarial_events)
        || !query.raw_data_local
        || !query.aggregate_only
        || query.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "query identity, requirements, peers, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &query.candidates {
        if row.evidence_id.trim().is_empty()
            || !ids.insert(row.evidence_id.clone())
            || row.study_id.trim().is_empty()
            || row.source_id.trim().is_empty()
            || row.scope.trim().is_empty()
            || row.relevance_milli > 1000
            || row.freshness_epoch == 0
            || row.semantic_profile.trim().is_empty()
            || !digest(&row.content_digest)
            || !digest(&row.provenance_digest)
            || !digest(&row.replay_identity)
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "evidence {} is malformed or duplicated",
                row.evidence_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &query.peers {
        if peer.institution_id.trim().is_empty()
            || !peers.insert(peer.institution_id.clone())
            || !digest(&peer.evidence_digest)
            || peer.semantic_profile.trim().is_empty()
            || !digest(&peer.replay_identity)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.institution_id
            )));
        }
    }
    if query.minimum_peer_quorum as usize > query.peers.len() {
        return Err(invalid("peer quorum exceeds peer count"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn query() -> ScopedRetrievalQuery {
        let d = hash("retrieval");
        let candidate = |id: &str| RetrievalCandidate {
            evidence_id: id.into(),
            study_id: format!("study:{id}"),
            source_id: format!("source:{id}"),
            scope: "organoid-study".into(),
            relevance_milli: 900,
            freshness_epoch: 10,
            semantic_profile: "preclinical-neural".into(),
            content_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            evidence_state: EvidenceState::Supported,
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
        };
        let peer = |id: &str| PeerSynthesisSummary {
            institution_id: id.into(),
            evidence_digest: d.clone(),
            semantic_profile: "preclinical-neural".into(),
            replay_identity: d.clone(),
            signed: true,
            permitted: true,
            aggregate_only: true,
        };
        ScopedRetrievalQuery {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            query_id: "query:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "preclinical-neural".into(),
            required_evidence_order: vec!["evidence:a".into()],
            required_scope_order: vec!["organoid-study".into()],
            minimum_freshness_epoch: 5,
            candidates: vec![candidate("evidence:a")],
            peers: vec![peer("inst:a"), peer("inst:b")],
            minimum_peer_quorum: 2,
            replay_identity: d.clone(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_retrieval_synthesis_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_synthesis() {
        assert_eq!(
            assure_federated_retrieval_synthesis(&query())
                .unwrap()
                .disposition,
            SynthesisDisposition::Qualified
        );
    }
    #[test]
    fn deterministic_synthesis() {
        let a = assure_federated_retrieval_synthesis(&query()).unwrap();
        let b = assure_federated_retrieval_synthesis(&query()).unwrap();
        assert_eq!(a.synthesis_digest, b.synthesis_digest);
    }
    #[test]
    fn stale_is_unresolved() {
        let mut value = query();
        value.candidates[0].freshness_epoch = 2;
        assert_eq!(
            assure_federated_retrieval_synthesis(&value)
                .unwrap()
                .disposition,
            SynthesisDisposition::Unresolved
        );
    }
    #[test]
    fn peer_quorum_is_required() {
        let mut value = query();
        value.peers[1].signed = false;
        assert_eq!(
            assure_federated_retrieval_synthesis(&value)
                .unwrap()
                .disposition,
            SynthesisDisposition::Unresolved
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = query();
        value.policy_allow = false;
        assert_eq!(
            assure_federated_retrieval_synthesis(&value)
                .unwrap()
                .disposition,
            SynthesisDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks() {
        let mut value = query();
        value.adversarial_events.push("poisoned-source".into());
        assert_eq!(
            assure_federated_retrieval_synthesis(&value)
                .unwrap()
                .disposition,
            SynthesisDisposition::Blocked
        );
    }
}
