//! Federated continual retrieval-and-synthesis bundle assurance.
//!
//! Atlas feature: `AFA-bundle-P02-F28`.  This verifier checks caller-supplied
//! signed-result-bundle attestations and digest-only peer summaries before a
//! synthesis can be released. It does not fetch evidence, execute tools, or
//! move raw experimental payloads.

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

pub const FEATURE_ID: &str = "AFA-bundle-P02-F28";
pub const CONTRACT_VERSION: &str =
    "bundle-federated-continual-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.bundle-evidence-synthesis-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleEvidenceCandidate {
    pub evidence_id: String,
    pub study_id: String,
    pub source_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub relevance_milli: u16,
    pub freshness_epoch: u64,
    pub evidence_state: EvidenceState,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub manifest_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub bundle_verified: bool,
    pub raw_payload_carried: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundlePeerSummary {
    pub institution_id: String,
    pub synthesis_digest: ContentHash,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
    pub bundle_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleRetrievalQuery {
    pub schema_version: String,
    pub query_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_evidence_order: Vec<String>,
    pub required_scope_order: Vec<String>,
    pub minimum_freshness_epoch: u64,
    pub candidates: Vec<BundleEvidenceCandidate>,
    pub peers: Vec<BundlePeerSummary>,
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
pub enum BundleSynthesisDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleEvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub query_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: BundleSynthesisDisposition,
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
pub enum BundleAssuranceError {
    #[error("invalid bundle retrieval query: {0}")]
    Invalid(String),
    #[error("bundle assurance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(value: impl Into<String>) -> BundleAssuranceError {
    BundleAssuranceError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl BundleEvidenceSynthesis {
    pub fn validate(&self) -> Result<(), BundleAssuranceError> {
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
        let evidence = self.evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != evidence.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != evidence
        {
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
            .map_err(|error| BundleAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("synthesis artifact type is invalid"));
        }
        if self.disposition == BundleSynthesisDisposition::Qualified
            && self.effect_receipts
                != [format!(
                    "verify:bundle-evidence-synthesis:{}",
                    self.query_id
                )]
        {
            return Err(invalid("qualified synthesis effect is invalid"));
        }
        if self.disposition != BundleSynthesisDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified synthesis must block release"));
        }
        Ok(())
    }
}

pub fn retrieval_bundle_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bundle".into(),
        consumers: BTreeSet::from([String::from("laboratory automation engineer"), String::from("research data steward"), String::from("release governance operator")]),
        behavior: "verifies signed-result-bundle retrieval attestations and aggregate-only peer synthesis summaries under evidence, freshness, scope, provenance, replay, policy, and federation gates without fetching or executing evidence".into(),
        value: "prevents a malformed, stale, semantically drifting, or unauthorized result bundle from silently becoming federated evidence synthesis".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from([String::from("evaluate:capability-runs")]), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_retrieval_bundle(
    query: &BundleRetrievalQuery,
) -> Result<BundleEvidenceSynthesis, BundleAssuranceError> {
    validate_query(query)?;
    let mut candidates = query.candidates.clone();
    candidates.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then_with(|| a.evidence_id.cmp(&b.evidence_id))
    });
    let mut evidence_order = candidates
        .iter()
        .map(|c| c.evidence_id.clone())
        .collect::<Vec<_>>();
    evidence_order.sort();
    let required = query
        .required_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let scopes = query
        .required_scope_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let covered = candidates
        .iter()
        .filter(|c| scopes.contains(&c.scope))
        .map(|c| c.scope.clone())
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut missing_scope = scopes
        .difference(&covered)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for c in &candidates {
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.evidence_id));
        }
        omissions.extend(c.omissions.iter().map(|v| format!("{}:{v}", c.evidence_id)));
        uncertainty.extend(
            c.uncertainty
                .iter()
                .map(|v| format!("{}:{v}", c.evidence_id)),
        );
        if c.freshness_epoch < query.minimum_freshness_epoch {
            stale.insert(c.evidence_id.clone());
            unresolved.insert(c.evidence_id.clone());
        } else if c.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(c.evidence_id.clone());
            blocked.insert(c.evidence_id.clone());
        } else if !c.local_only || !c.permitted || !c.bundle_verified || c.raw_payload_carried {
            blocked.insert(c.evidence_id.clone());
        } else if c.semantic_profile != query.semantic_profile
            || c.replay_identity != query.replay_identity
            || !digest(&c.content_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.manifest_digest)
            || c.relevance_milli < 600
            || !c.omissions.is_empty()
            || !c.uncertainty.is_empty()
            || !matches!(
                c.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(c.evidence_id.clone());
        } else if !scopes.contains(&c.scope) {
            missing_scope.insert(c.scope.clone());
            unresolved.insert(c.evidence_id.clone());
        } else {
            selected.insert(c.evidence_id.clone());
        }
    }
    let missing = required
        .difference(&evidence_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omissions.insert(format!("{id}:required-evidence-missing"));
    }
    for scope in &missing_scope {
        omissions.insert(format!("required-scope-missing:{scope}"));
    }
    let mut peer_order = query
        .peers
        .iter()
        .map(|p| p.institution_id.clone())
        .collect::<Vec<_>>();
    peer_order.sort();
    let mut qualified_peer = BTreeSet::new();
    let mut missing_peer = BTreeSet::new();
    for p in &query.peers {
        if p.signed
            && p.permitted
            && p.aggregate_only
            && p.bundle_verified
            && p.semantic_profile == query.semantic_profile
            && p.replay_identity == query.replay_identity
            && digest(&p.synthesis_digest)
        {
            qualified_peer.insert(p.institution_id.clone());
        } else {
            missing_peer.insert(p.institution_id.clone());
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
            .map(|v| format!("adversarial:{v}")),
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
        omissions.insert("request:bundle-release-gate-blocked".into());
    }
    let required_block = required.iter().any(|id| blocked.contains(id));
    let disposition = if global_block || required_block {
        BundleSynthesisDisposition::Blocked
    } else if required.is_subset(&selected)
        && missing.is_empty()
        && missing_scope.is_empty()
        && qualified_peer.len() >= query.minimum_peer_quorum as usize
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        BundleSynthesisDisposition::Qualified
    } else {
        BundleSynthesisDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_evidence_order = missing.into_iter().collect::<Vec<_>>();
    let stale_order = stale.into_iter().collect::<Vec<_>>();
    let missing_scope_order = missing_scope.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peer.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peer.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let contradiction_order = contradiction.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == BundleSynthesisDisposition::Qualified {
        vec![format!(
            "verify:bundle-evidence-synthesis:{}",
            query.query_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"query_id":query.query_id,"federation_id":query.federation_id,"semantic_profile":query.semantic_profile,"disposition":disposition,"evidence_order":evidence_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_evidence_order":missing_evidence_order,"stale_order":stale_order,"missing_scope_order":missing_scope_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"contradiction_order":contradiction_order,"negative_evidence_order":negative_evidence_order,"replay_identity":query.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":query.raw_data_local,"aggregate_only":query.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|e| BundleAssuranceError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bundle-evidence-synthesis:{}", query.query_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| BundleAssuranceError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect::<Vec<String>>()
    };
    let result = BundleEvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        query_id: query.query_id.clone(),
        federation_id: query.federation_id.clone(),
        semantic_profile: query.semantic_profile.clone(),
        disposition,
        evidence_order: strings("evidence_order"),
        selected_order: strings("selected_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        missing_evidence_order: strings("missing_evidence_order"),
        stale_order: strings("stale_order"),
        missing_scope_order: strings("missing_scope_order"),
        peer_order: strings("peer_order"),
        qualified_peer_order: strings("qualified_peer_order"),
        missing_peer_order: strings("missing_peer_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        contradiction_order: strings("contradiction_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: query.replay_identity.clone(),
        synthesis_digest,
        artifact,
        effect_receipts: strings("effect_receipts"),
        raw_data_local: query.raw_data_local,
        aggregate_only: query.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_query(query: &BundleRetrievalQuery) -> Result<(), BundleAssuranceError> {
    if query.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || query.query_id.trim().is_empty()
        || query.federation_id.trim().is_empty()
        || query.semantic_profile.trim().is_empty()
        || query.required_evidence_order.is_empty()
        || !canonical(&query.required_evidence_order)
        || query.required_scope_order.is_empty()
        || !canonical(&query.required_scope_order)
        || query.minimum_freshness_epoch == 0
        || query.candidates.is_empty()
        || query.peers.is_empty()
        || query.minimum_peer_quorum == 0
        || query.minimum_peer_quorum as usize > query.peers.len()
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
    for c in &query.candidates {
        if c.evidence_id.trim().is_empty()
            || !ids.insert(c.evidence_id.clone())
            || c.study_id.trim().is_empty()
            || c.source_id.trim().is_empty()
            || c.scope.trim().is_empty()
            || c.semantic_profile.trim().is_empty()
            || c.relevance_milli > 1000
            || c.freshness_epoch == 0
            || !digest(&c.content_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.manifest_digest)
            || !digest(&c.replay_identity)
            || !canonical(&c.omissions)
            || !canonical(&c.uncertainty)
        {
            return Err(invalid(format!(
                "candidate {} is malformed or duplicated",
                c.evidence_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for p in &query.peers {
        if p.institution_id.trim().is_empty()
            || !peers.insert(p.institution_id.clone())
            || !digest(&p.synthesis_digest)
            || !digest(&p.replay_identity)
            || p.semantic_profile.trim().is_empty()
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                p.institution_id
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
    fn query() -> BundleRetrievalQuery {
        let d = hash("bundle");
        let c = |id: &str| BundleEvidenceCandidate {
            evidence_id: id.into(),
            study_id: format!("study:{id}"),
            source_id: format!("source:{id}"),
            scope: "organoid-study".into(),
            semantic_profile: "bundle-v1".into(),
            relevance_milli: 900,
            freshness_epoch: 10,
            evidence_state: EvidenceState::Supported,
            content_digest: d.clone(),
            provenance_digest: d.clone(),
            manifest_digest: d.clone(),
            replay_identity: d.clone(),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
            bundle_verified: true,
            raw_payload_carried: false,
        };
        let p = |id: &str| BundlePeerSummary {
            institution_id: id.into(),
            synthesis_digest: d.clone(),
            semantic_profile: "bundle-v1".into(),
            replay_identity: d.clone(),
            signed: true,
            permitted: true,
            aggregate_only: true,
            bundle_verified: true,
        };
        BundleRetrievalQuery {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            query_id: "query:bundle".into(),
            federation_id: "fed:bundle".into(),
            semantic_profile: "bundle-v1".into(),
            required_evidence_order: vec!["evidence:a".into()],
            required_scope_order: vec!["organoid-study".into()],
            minimum_freshness_epoch: 5,
            candidates: vec![c("evidence:a")],
            peers: vec![p("inst:a"), p("inst:b")],
            minimum_peer_quorum: 2,
            replay_identity: d,
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
            retrieval_bundle_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_retrieval_bundle(&query()).unwrap().disposition,
            BundleSynthesisDisposition::Qualified
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            assure_retrieval_bundle(&query()).unwrap().synthesis_digest,
            assure_retrieval_bundle(&query()).unwrap().synthesis_digest
        )
    }
    #[test]
    fn stale_unresolved() {
        let mut q = query();
        q.candidates[0].freshness_epoch = 2;
        assert_eq!(
            assure_retrieval_bundle(&q).unwrap().disposition,
            BundleSynthesisDisposition::Unresolved
        )
    }
    #[test]
    fn unverified_bundle_blocks() {
        let mut q = query();
        q.candidates[0].bundle_verified = false;
        assert_eq!(
            assure_retrieval_bundle(&q).unwrap().disposition,
            BundleSynthesisDisposition::Blocked
        )
    }
    #[test]
    fn quorum_unresolved() {
        let mut q = query();
        q.peers[1].signed = false;
        assert_eq!(
            assure_retrieval_bundle(&q).unwrap().disposition,
            BundleSynthesisDisposition::Unresolved
        )
    }
    #[test]
    fn adversarial_blocks() {
        let mut q = query();
        q.adversarial_events.push("poisoned-bundle".into());
        assert_eq!(
            assure_retrieval_bundle(&q).unwrap().disposition,
            BundleSynthesisDisposition::Blocked
        )
    }
}
