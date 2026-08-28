//! Federated continual retrieval-and-synthesis assurance harness
//! (`AFA-ids-P02-F28`).
//!
//! This is a release-gate capability for institution-local preclinical
//! evidence retrieval.  It verifies caller-supplied evidence declarations,
//! source closure, and aggregate peer attestations; it never performs network
//! retrieval, exports raw text, or turns an incomplete corpus into a claim.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P02-F28";
pub const CONTRACT_VERSION: &str =
    "ids-federated-continual-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery6@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis11@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.evidence-synthesis-11+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CANDIDATES: usize = 8192;
pub const MAX_PEERS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalEvidence7 {
    pub evidence_id: String,
    pub source_id: String,
    pub origin: String,
    pub title: String,
    pub terms: Vec<String>,
    pub relevance_milli: i64,
    pub freshness_milli: i64,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub evidence_state: RetrievalEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub comparable: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalPeer6 {
    pub peer_id: String,
    pub origin: String,
    pub corpus_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub synthesis_digest: ContentHash,
    pub source_count: usize,
    pub evidence_state: RetrievalEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery6 {
    pub request_id: String,
    pub corpus_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub query_terms: Vec<String>,
    pub candidates: Vec<RetrievalEvidence7>,
    pub peers: Vec<RetrievalPeer6>,
    pub checkpoint: u64,
    pub minimum_relevance_milli: i64,
    pub minimum_freshness_milli: i64,
    pub minimum_peer_quorum: usize,
    pub max_budget_units: u64,
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
pub struct EvidenceSynthesis11Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis11 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub corpus_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub source_order: Vec<String>,
    pub qualified_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub low_relevance_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub ranked_scores_milli: Vec<i64>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub artifact: EvidenceSynthesis11Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalSynthesisAssuranceError {
    #[error("invalid retrieval synthesis assurance request: {0}")]
    Invalid(String),
    #[error("retrieval synthesis assurance artifact failed: {0}")]
    Artifact(String),
}

pub fn retrieval_synthesis_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["downstream AURORA crate maintainer", "retrieval scientist", "federation steward", "release-gate operator"],
        "behavior": "verifies a bounded federated retrieval corpus and synthesis closure before research-object release",
        "value": "prevents unsupported, stale, incomparable, contradictory, or policy-denied evidence from becoming an apparently complete synthesis",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-summaries", "manage:local-capability"],
        "permissions": ["read:local-retrieval-manifests", "evaluate:research-evidence"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl EvidenceSynthesis11 {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisAssuranceError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.corpus_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
            ])
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.source_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "retrieval identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.source_order,
            &self.qualified_source_order,
            &self.missing_source_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.low_relevance_order,
            &self.stale_order,
            &self.incomparable_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(RetrievalSynthesisAssuranceError::Invalid(
                    "retrieval assurance ordering is not canonical".into(),
                ));
            }
        }
        let candidates = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len()
            || BTreeSet::from_iter(parts.iter().cloned()) != candidates
            || parts.len() != candidates.len()
        {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "candidate assurance states do not partition".into(),
            ));
        }
        let sources = BTreeSet::from_iter(self.source_order.iter().cloned());
        let source_parts = self
            .qualified_source_order
            .iter()
            .chain(&self.missing_source_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if sources != BTreeSet::from_iter(source_parts.iter().cloned())
            || source_parts.len() != sources.len()
            || peers != BTreeSet::from_iter(peer_parts.iter().cloned())
            || peer_parts.len() != peers.len()
            || self.ranked_scores_milli.len() != self.qualified_order.len()
        {
            return Err(RetrievalSynthesisAssuranceError::Artifact(
                "source, peer, or score cardinality is inconsistent".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.synthesis_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| d.as_str().len() != 64)
        {
            return Err(RetrievalSynthesisAssuranceError::Artifact(
                "retrieval artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "effect is outside the retrieval assurance gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalSynthesisAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| RetrievalSynthesisAssuranceError::Artifact(e.to_string()))?,
        )
        .map_err(|e| RetrievalSynthesisAssuranceError::Artifact(e.to_string()))
    }
}

fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

fn score(candidate: &RetrievalEvidence7) -> i64 {
    candidate
        .relevance_milli
        .saturating_mul(3)
        .saturating_add(candidate.freshness_milli.saturating_mul(2))
        .saturating_add(candidate.terms.len().min(1_000) as i64)
}

pub fn assure_retrieval_synthesis(
    request: &ScopedRetrievalQuery6,
) -> Result<EvidenceSynthesis11, RetrievalSynthesisAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.evidence_id.cmp(&b.evidence_id));
    let candidate_order = candidates
        .iter()
        .map(|c| c.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut low_relevance = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut ranked = BTreeMap::new();
    let mut total_units = 0_u64;
    for candidate in &candidates {
        let id = candidate.evidence_id.clone();
        total_units = total_units.saturating_add(candidate.estimated_units);
        if candidate.negative_result {
            negative.insert(format!("{id}:negative-result"));
        }
        if candidate.evidence_state == RetrievalEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted"));
            continue;
        }
        if !candidate.local_only || !candidate.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-not-local-or-aggregate"));
            continue;
        }
        if candidate.replay_identity != request.replay_identity
            || !candidate.signed
            || !candidate.permitted
        {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:replay-or-authorization"));
            continue;
        }
        if !matches!(
            candidate.evidence_state,
            RetrievalEvidenceState::Proven | RetrievalEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
            continue;
        }
        let mut threshold_failure = false;
        if candidate.relevance_milli < request.minimum_relevance_milli {
            low_relevance.insert(id.clone());
            threshold_failure = true;
        }
        if candidate.freshness_milli < request.minimum_freshness_milli {
            stale.insert(id.clone());
            threshold_failure = true;
        }
        if !candidate.comparable {
            incomparable.insert(id.clone());
            threshold_failure = true;
        }
        if threshold_failure {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:retrieval-threshold-or-comparability"));
        } else {
            ranked.insert(id, score(candidate));
            qualified.insert(candidate.evidence_id.clone());
        }
        if !candidate.omission_reasons.is_empty() {
            omissions.extend(
                candidate
                    .omission_reasons
                    .iter()
                    .map(|reason| format!("{}:{reason}", candidate.evidence_id)),
            );
        }
    }
    let source_order = candidates
        .iter()
        .map(|c| c.source_id.clone())
        .collect::<BTreeSet<_>>();
    let qualified_source_order = candidates
        .iter()
        .filter(|c| qualified.contains(&c.evidence_id))
        .map(|c| c.source_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_source_order = source_order
        .difference(&qualified_source_order)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_source_order.is_empty() {
        omissions.insert("source:qualified-closure-incomplete".into());
    }
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let ok = peer.corpus_id == request.corpus_id
            && peer.semantic_profile == request.semantic_profile
            && peer.checkpoint == request.checkpoint
            && peer.source_count > 0
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                RetrievalEvidenceState::Proven | RetrievalEvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if total_units > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total_units}"));
    }
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
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:retrieval-synthesis-not-authorized".into());
    }
    let disposition = if global_block || qualified.is_empty() && !blocked.is_empty() {
        "blocked"
    } else if qualified.is_empty()
        || qualified_peers.len() < request.minimum_peer_quorum
        || total_units > request.max_budget_units
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:synthesis-not-release-ready".into());
    }
    let qualified_order = qualified.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let ranked_scores_milli = qualified_order
        .iter()
        .map(|id| ranked.get(id).copied().unwrap_or_default())
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "corpus_id": request.corpus_id,
        "requester": request.requester,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "qualified_order": qualified_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "source_order": source_order,
        "qualified_source_order": qualified_source_order,
        "missing_source_order": missing_source_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers,
        "missing_peer_order": missing_peers,
        "low_relevance_order": low_relevance,
        "stale_order": stale,
        "incomparable_order": incomparable,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "ranked_scores_milli": ranked_scores_milli,
        "total_units": total_units,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| RetrievalSynthesisAssuranceError::Artifact(e.to_string()))?;
    let artifact = EvidenceSynthesis11Artifact {
        artifact_id: format!("evidence-synthesis-11:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: omissions.iter().cloned().collect(),
        provenance_digests: candidates
            .iter()
            .map(|c| c.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = EvidenceSynthesis11 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        corpus_id: request.corpus_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order,
        qualified_order,
        unresolved_order,
        blocked_order,
        source_order: source_order.into_iter().collect(),
        qualified_source_order: qualified_source_order.into_iter().collect(),
        missing_source_order: missing_source_order.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        low_relevance_order: low_relevance.into_iter().collect(),
        stale_order: stale.into_iter().collect(),
        incomparable_order: incomparable.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        ranked_scores_milli,
        total_units,
        replay_identity: request.replay_identity.clone(),
        synthesis_digest: digest,
        artifact,
        effect_receipts: if disposition == "qualified" {
            vec![
                format!("exchange:permitted-summaries:{}", request.request_id),
                format!("manage:local-capability:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ScopedRetrievalQuery6,
) -> Result<(), RetrievalSynthesisAssuranceError> {
    if !all_nonempty([
        &request.request_id,
        &request.corpus_id,
        &request.requester,
        &request.purpose,
        &request.semantic_profile,
    ]) || request.query_terms.is_empty()
        || request.query_terms.windows(2).any(|w| w[0] >= w[1])
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.checkpoint == 0
        || request.max_budget_units == 0
        || request.minimum_peer_quorum == 0
        || !valid_metric(request.minimum_relevance_milli)
        || !valid_metric(request.minimum_freshness_milli)
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "query identity, terms, candidates, peers, thresholds, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !all_nonempty([
            &candidate.evidence_id,
            &candidate.source_id,
            &candidate.origin,
            &candidate.title,
        ]) || !candidate_ids.insert(candidate.evidence_id.clone())
            || candidate.terms.is_empty()
            || candidate.terms.windows(2).any(|w| w[0] >= w[1])
            || candidate.estimated_units == 0
            || !valid_metric(candidate.relevance_milli)
            || !valid_metric(candidate.freshness_milli)
            || candidate.content_digest.as_str().len() != 64
            || candidate.provenance_digest.as_str().len() != 64
            || candidate.replay_identity.as_str().len() != 64
            || candidate.omission_reasons.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "candidate identity, terms, scores, omission ordering, or digests are invalid"
                    .into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if !all_nonempty([
            &peer.peer_id,
            &peer.origin,
            &peer.corpus_id,
            &peer.semantic_profile,
        ]) || !peer_ids.insert(peer.peer_id.clone())
            || peer.checkpoint == 0
            || peer.source_count == 0
            || peer.synthesis_digest.as_str().len() != 64
        {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "peer identity, corpus, checkpoint, source count, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn valid_metric(value: i64) -> bool {
    (0..=1_000).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn evidence(id: &str, relevance: i64) -> RetrievalEvidence7 {
        RetrievalEvidence7 {
            evidence_id: id.into(),
            source_id: format!("source:{id}"),
            origin: "site:one".into(),
            title: format!("Preclinical result {id}"),
            terms: vec!["mechanism".into(), "neuron".into()],
            relevance_milli: relevance,
            freshness_milli: 900,
            content_digest: h(id),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            estimated_units: 5,
            evidence_state: RetrievalEvidenceState::Supported,
            signed: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            comparable: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }

    fn request() -> ScopedRetrievalQuery6 {
        let peer = RetrievalPeer6 {
            peer_id: "peer:one".into(),
            origin: "site:one".into(),
            corpus_id: "corpus:one".into(),
            semantic_profile: "neuro:evidence:v1".into(),
            checkpoint: 2,
            synthesis_digest: h("peer"),
            source_count: 1,
            evidence_state: RetrievalEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        };
        ScopedRetrievalQuery6 {
            request_id: "request:retrieval".into(),
            corpus_id: "corpus:one".into(),
            requester: "retrieval-scientist".into(),
            purpose: "preclinical-evidence-synthesis".into(),
            semantic_profile: "neuro:evidence:v1".into(),
            query_terms: vec!["mechanism".into(), "neuron".into()],
            candidates: vec![evidence("evidence:a", 900), evidence("evidence:b", 700)],
            peers: vec![peer],
            checkpoint: 2,
            minimum_relevance_milli: 600,
            minimum_freshness_milli: 500,
            minimum_peer_quorum: 1,
            max_budget_units: 100,
            replay_identity: h("replay"),
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
    fn manifest_is_a2() {
        assert_eq!(
            retrieval_synthesis_assurance_manifest()["autonomy_tier"],
            "A2"
        );
    }

    #[test]
    fn nominal_is_qualified_and_digest_stable() {
        let report = assure_retrieval_synthesis(&request()).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.qualified_order.len(), 2);
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }

    #[test]
    fn stale_candidate_is_unresolved() {
        let mut request = request();
        request.candidates[0].freshness_milli = 100;
        let report = assure_retrieval_synthesis(&request).unwrap();
        assert!(report.stale_order.contains(&"evidence:a".into()));
        assert!(report.unresolved_order.contains(&"evidence:a".into()));
    }

    #[test]
    fn contradictory_candidate_is_blocked_and_negative() {
        let mut request = request();
        request.candidates[0].evidence_state = RetrievalEvidenceState::Contradicted;
        let report = assure_retrieval_synthesis(&request).unwrap();
        assert!(report.blocked_order.contains(&"evidence:a".into()));
        assert!(report
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("contradicted")));
    }

    #[test]
    fn peer_quorum_gap_is_unresolved() {
        let mut request = request();
        request.minimum_peer_quorum = 2;
        let report = assure_retrieval_synthesis(&request).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report
            .uncertainty_order
            .contains(&"peer:minimum-quorum-unmet".into()));
    }

    #[test]
    fn federation_denial_blocks_all_candidates() {
        let mut request = request();
        request.federation_approved = false;
        let report = assure_retrieval_synthesis(&request).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert!(report.qualified_order.is_empty());
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn low_relevance_is_explicit_and_fallback_remains() {
        let mut request = request();
        request.candidates[0].relevance_milli = 100;
        let report = assure_retrieval_synthesis(&request).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert!(report.low_relevance_order.contains(&"evidence:a".into()));
        assert_eq!(report.qualified_order, vec!["evidence:b"]);
    }
}
