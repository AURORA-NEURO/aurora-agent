//! Deterministic researcher-facing retrieval and synthesis workbench.
//!
//! Atlas feature: `AFA-choreography-P02-F19`.
//!
//! This is a read-only interaction surface over caller-supplied retrieval candidates. It makes
//! ranking, omissions, freshness, contradictions, provenance, and protected-closure state visible
//! to a research workflow operator; it never retrieves from the network or upgrades an unresolved
//! candidate into a scientific conclusion.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-choreography-P02-F19";
pub const CONTRACT_VERSION: &str = "choreography-prospective-retrieval-synthesis-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub candidate_id: String,
    pub source_id: String,
    pub title: String,
    pub evidence_state: EvidenceState,
    pub content_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub relevance_milli: u32,
    pub freshness_days: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub request_id: String,
    pub batch_id: String,
    pub scope: String,
    pub query: String,
    pub schema_version: String,
    pub candidates: Vec<RetrievalCandidate>,
    pub required_source_order: Vec<String>,
    pub min_independent_sources: u32,
    pub max_visible: u32,
    pub max_freshness_days: u32,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub scope: String,
    pub query: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub visible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub required_source_order: Vec<String>,
    pub observed_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub views: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkbenchError {
    #[error("invalid retrieval workbench query: {0}")]
    Invalid(String),
    #[error("retrieval workbench artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl EvidenceSynthesis {
    pub fn validate(&self) -> Result<(), WorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.query.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.rank_order.len() != self.candidate_order.len()
            || self.views.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(WorkbenchError::Invalid(
                "workbench identity, candidates, views, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.visible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.stale_order,
            &self.contradiction_order,
            &self.required_source_order,
            &self.observed_source_order,
            &self.missing_source_order,
            &self.views,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(WorkbenchError::Invalid(
                    "workbench ordering is not canonical".into(),
                ));
            }
        }
        if self.rank_order.iter().collect::<BTreeSet<_>>()
            != self.candidate_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(WorkbenchError::Invalid(
                "rank order is not a candidate permutation".into(),
            ));
        }
        let covered = self
            .visible_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if covered.iter().collect::<BTreeSet<_>>()
            != self.candidate_order.iter().collect::<BTreeSet<_>>()
            || covered.len() != self.candidate_order.len()
        {
            return Err(WorkbenchError::Invalid(
                "workbench dispositions do not partition candidates".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "view:authorized-research-state" && effect != "block:unsafe-release"
        }) {
            return Err(WorkbenchError::Invalid(
                "workbench effect is outside read-only view boundary".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| WorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, WorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| WorkbenchError::Artifact(error.to_string()))?,
        )
        .map_err(|error| WorkbenchError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "choreography".into(), consumers: BTreeSet::from(["research workflow operator".into(), "researcher workbench".into(), "retrieval reviewer".into()]), behavior: "renders ranked, omission-aware, provenance-bearing retrieval candidates as a local read-only synthesis workbench".into(), value: "makes high-throughput retrieval state auditable without network side effects or silent evidence promotion".into(), inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]), permissions: BTreeSet::from(["view:authorized-research-state".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "research-workflow-operator".into(), reason: "view authorized research state".into() }], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_query(query: &ScopedRetrievalQuery) -> Result<(), WorkbenchError> {
    if query.schema_version != INPUT_SCHEMA
        || query.request_id.trim().is_empty()
        || query.batch_id.trim().is_empty()
        || query.scope.trim().is_empty()
        || query.query.trim().is_empty()
        || query.candidates.is_empty()
        || query.required_source_order.is_empty()
        || query.min_independent_sources == 0
        || query.max_visible == 0
        || query.max_freshness_days == 0
        || query.budget_units == 0
        || query.max_budget_units == 0
        || query.budget_units > query.max_budget_units
        || !query.raw_data_local
        || query.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(WorkbenchError::Invalid(
            "query identity, bounds, candidates, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&query.required_source_order) {
        return Err(WorkbenchError::Invalid(
            "required source order is not canonical".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &query.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.source_id.trim().is_empty()
            || candidate.title.trim().is_empty()
            || candidate.content_digest.is_none()
            || candidate.provenance_digest.is_none()
        {
            return Err(WorkbenchError::Invalid(
                "candidate identity, source, title, content, or provenance is incomplete".into(),
            ));
        }
    }
    Ok(())
}

pub fn render(query: &ScopedRetrievalQuery) -> Result<EvidenceSynthesis, WorkbenchError> {
    validate_query(query)?;
    let candidates = query
        .candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut candidate_order = candidates.keys().cloned().collect::<Vec<_>>();
    candidate_order.sort();
    let mut scores = BTreeMap::new();
    let mut stale = BTreeSet::new();
    let mut contradictions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    for candidate in query.candidates.iter() {
        if candidate.freshness_days > query.max_freshness_days {
            stale.insert(candidate.candidate_id.clone());
            omissions.insert(format!("{}:stale", candidate.candidate_id));
        }
        if candidate.evidence_state == EvidenceState::Contradicted {
            contradictions.insert(candidate.candidate_id.clone());
            semantic_loss.push(SemanticLoss {
                field: format!("candidate:{}", candidate.candidate_id),
                reason: "contradicted evidence is visible but not promoted".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
        if matches!(
            candidate.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            uncertainty.insert(format!("{}:evidence-state", candidate.candidate_id));
        }
        for item in &candidate.omissions {
            omissions.insert(format!("{}:{}", candidate.candidate_id, item));
        }
        for item in &candidate.uncertainty {
            uncertainty.insert(format!("{}:{}", candidate.candidate_id, item));
        }
        negative.insert(format!(
            "{}:{}",
            candidate.candidate_id,
            if candidate.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        let evidence_bonus = match candidate.evidence_state {
            EvidenceState::Proven => 20_000,
            EvidenceState::Supported => 10_000,
            _ => 0,
        };
        scores.insert(
            candidate.candidate_id.clone(),
            candidate
                .relevance_milli
                .saturating_add(evidence_bonus)
                .saturating_sub(candidate.freshness_days.saturating_mul(10)),
        );
    }
    let rank_order = {
        let mut values = candidate_order.clone();
        values.sort_by(|left, right| {
            scores[right]
                .cmp(&scores[left])
                .then_with(|| left.cmp(right))
        });
        values
    };
    let required_sources = query
        .required_source_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed_sources = query
        .candidates
        .iter()
        .map(|candidate| candidate.source_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_sources = required_sources
        .difference(&observed_sources)
        .cloned()
        .collect::<Vec<_>>();
    omissions.extend(
        missing_sources
            .iter()
            .map(|source| format!("missing-source:{source}")),
    );
    let global_block = !query.policy_allow || !query.protected_closure;
    if !query.policy_allow {
        omissions.insert("query:policy-denied".into());
    }
    if !query.protected_closure {
        omissions.insert("query:protected-closure-incomplete".into());
    }
    let mut visible = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut spent = 0_u32;
    for candidate_id in &rank_order {
        let candidate = candidates[candidate_id];
        let hard_block = global_block || candidate.evidence_state == EvidenceState::Contradicted;
        let conditional = candidate.freshness_days > query.max_freshness_days
            || matches!(
                candidate.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
            || !candidate.omissions.is_empty()
            || !candidate.uncertainty.is_empty();
        if hard_block {
            blocked.push(candidate_id.clone());
            continue;
        }
        if conditional || visible.len() >= query.max_visible as usize {
            unresolved.push(candidate_id.clone());
            if visible.len() >= query.max_visible as usize {
                omissions.insert(format!("{}:view-capacity", candidate_id));
            }
            continue;
        }
        let cost = candidate.title.len() as u32 + 1;
        if cost > query.budget_units.saturating_sub(spent) {
            unresolved.push(candidate_id.clone());
            omissions.insert(format!("{}:budget-ceiling", candidate_id));
        } else {
            spent = spent.saturating_add(cost);
            visible.push(candidate_id.clone());
        }
    }
    visible.sort();
    unresolved.sort();
    blocked.sort();
    let source_quorum = visible
        .iter()
        .map(|id| candidates[id].source_id.clone())
        .collect::<BTreeSet<_>>()
        .len() as u32;
    if source_quorum < query.min_independent_sources {
        omissions.insert(format!(
            "source-quorum:{source_quorum}/{}",
            query.min_independent_sources
        ));
    }
    let disposition = if global_block {
        "blocked"
    } else if !blocked.is_empty()
        || !unresolved.is_empty()
        || source_quorum < query.min_independent_sources
    {
        "unresolved"
    } else {
        "qualified"
    };
    let views = vec![
        "candidate-table".to_string(),
        "omission-audit".to_string(),
        "source-lineage".to_string(),
    ];
    let payload = json!({"schema_version": OUTPUT_SCHEMA, "request_id": query.request_id, "batch_id": query.batch_id, "candidate_order": candidate_order, "rank_order": rank_order, "visible_order": visible, "unresolved_order": unresolved, "blocked_order": blocked, "replay_identity": query.replay_identity, "disposition": disposition});
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|error| WorkbenchError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("evidence-synthesis:{}", query.request_id),
        "application/vnd.aurora.evidence-synthesis+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: query.batch_id.clone(),
            relation: "retrieval-synthesis-workbench".into(),
            digest: synthesis_digest.clone(),
        }],
    )
    .map_err(|error| WorkbenchError::Artifact(error.to_string()))?;
    let receipt = EvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: query.request_id.clone(),
        batch_id: query.batch_id.clone(),
        scope: query.scope.clone(),
        query: query.query.clone(),
        disposition: disposition.into(),
        candidate_order,
        rank_order,
        visible_order: visible,
        unresolved_order: unresolved,
        blocked_order: blocked,
        stale_order: stale.into_iter().collect(),
        contradiction_order: contradictions.into_iter().collect(),
        required_source_order: query.required_source_order.clone(),
        observed_source_order: observed_sources.into_iter().collect(),
        missing_source_order: missing_sources,
        views,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: query.replay_identity.clone(),
        synthesis_digest,
        semantic_loss,
        artifact,
        effect_receipts: if disposition == "qualified" {
            vec!["view:authorized-research-state".into()]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: query.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"workbench")
    }
    fn candidate(id: &str, state: EvidenceState) -> RetrievalCandidate {
        RetrievalCandidate {
            candidate_id: id.into(),
            source_id: format!("source-{id}"),
            title: format!("candidate {id}"),
            evidence_state: state,
            content_digest: Some(hash()),
            provenance_digest: Some(hash()),
            relevance_milli: 80_000,
            freshness_days: 1,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_result: false,
        }
    }
    fn query() -> ScopedRetrievalQuery {
        ScopedRetrievalQuery {
            request_id: "request:workbench".into(),
            batch_id: "batch:1".into(),
            scope: "organoid".into(),
            query: "resilience".into(),
            schema_version: INPUT_SCHEMA.into(),
            candidates: vec![
                candidate("candidate-b", EvidenceState::Supported),
                candidate("candidate-a", EvidenceState::Proven),
            ],
            required_source_order: vec!["source-candidate-a".into(), "source-candidate-b".into()],
            min_independent_sources: 2,
            max_visible: 2,
            max_freshness_days: 10,
            replay_identity: hash(),
            budget_units: 100,
            max_budget_units: 100,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_workbench_is_ranked_and_read_only() {
        let receipt = render(&query()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.rank_order, vec!["candidate-a", "candidate-b"]);
        assert_eq!(
            receipt.effect_receipts,
            vec!["view:authorized-research-state"]
        );
    }
    #[test]
    fn stale_unknown_and_capacity_are_visible() {
        let mut value = query();
        value.candidates[0].freshness_days = 20;
        value.candidates[1].evidence_state = EvidenceState::Unknown;
        value.max_visible = 1;
        let receipt = render(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(!receipt.stale_order.is_empty());
        assert!(!receipt.uncertainty.is_empty());
    }
    #[test]
    fn contradiction_is_retained_not_promoted() {
        let mut value = query();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        let receipt = render(&value).unwrap();
        assert!(receipt.contradiction_order.contains(&"candidate-b".into()));
        assert!(receipt.blocked_order.contains(&"candidate-b".into()));
        assert!(!receipt.semantic_loss.is_empty());
    }
    #[test]
    fn policy_and_protected_closure_block_release() {
        let mut value = query();
        value.policy_allow = false;
        value.protected_closure = false;
        let receipt = render(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn missing_quorum_is_explicit() {
        let mut value = query();
        value.min_independent_sources = 3;
        let receipt = render(&value).unwrap();
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("source-quorum")));
    }
    #[test]
    fn manifest_is_a1_and_read_only() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .permissions
            .contains("view:authorized-research-state"));
    }
}
