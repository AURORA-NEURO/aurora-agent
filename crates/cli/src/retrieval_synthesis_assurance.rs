//! Prospective high-throughput retrieval and synthesis assurance.
//!
//! Atlas feature: `AFA-cli-P02-F27`.
//!
//! This module is deliberately a verification boundary.  It does not retrieve papers, contact a
//! provider, or infer a biological conclusion.  A caller supplies a typed, content-addressed
//! retrieval summary and this gate decides whether the summary is complete enough to become an
//! auditable `EvidenceSynthesis7` artifact.  Missing, stale, contradictory, negative, and
//! unmeasured evidence remain explicit rather than being silently discarded.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P02-F27";
pub const CONTRACT_VERSION: &str = "cli-prospective-retrieval-synthesis-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis7@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalEvidenceCandidate {
    pub candidate_id: String,
    pub source_id: String,
    pub title: String,
    pub relevance_milli: u32,
    pub evidence_state: EvidenceState,
    pub content_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub freshness_days: u32,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
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
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalSynthesisAssuranceReceipt {
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
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub evidence_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalSynthesisAssuranceError {
    #[error("invalid retrieval-synthesis assurance request: {0}")]
    Invalid(String),
    #[error("retrieval-synthesis assurance artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl RetrievalSynthesisAssuranceReceipt {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisAssuranceError> {
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
            || self.effect_receipts.is_empty()
            || self.checks.is_empty()
        {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "identity, locality, query, candidates, checks, or effects are incomplete".into(),
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
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(RetrievalSynthesisAssuranceError::Invalid(
                    "retrieval orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked = self.rank_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked != candidates || self.rank_order.windows(2).any(|pair| pair[0].is_empty()) {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "rank order is not a permutation of candidates".into(),
            ));
        }
        let mut partition = BTreeSet::<String>::new();
        for id in self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
        {
            if !partition.insert(id.clone()) || !candidates.contains(id) {
                return Err(RetrievalSynthesisAssuranceError::Invalid(
                    "candidate disposition partition is duplicated or incomplete".into(),
                ));
            }
        }
        if partition != candidates {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "candidate disposition partition is incomplete".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:retrieval-synthesis:") && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalSynthesisAssuranceError::Invalid(
                "effect is outside the retrieval release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalSynthesisAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "cli".into(),
        consumers: BTreeSet::from([
            "evidence-surveillance operator".into(),
            "release-evidence reviewer".into(),
            "downstream retrieval workflow".into(),
        ]),
        behavior: "verifies a bounded prospective retrieval summary and emits an auditable synthesis release verdict".into(),
        value: "prevents stale, contradictory, unproven, incomplete, or unauthorized evidence from being promoted as a qualified synthesis".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["evaluate:capability-runs".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "slsa-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "release-evidence-reviewer".into(), reason: "retrieval synthesis release verdict".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &ScopedRetrievalQuery,
) -> Result<(), RetrievalSynthesisAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.corpus_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.query_schema != INPUT_SCHEMA
        || request.candidates.is_empty()
        || request.required_source_ids.is_empty()
        || request.min_independent_sources == 0
        || request.max_selected == 0
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || request.max_freshness_days == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "identity, schema, bounded budget, corpus, sources, freshness, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&request.required_source_ids)
        || request
            .required_source_ids
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "required sources must be unique, non-empty, and canonical".into(),
        ));
    }
    if request.candidates.iter().any(|candidate| {
        candidate.candidate_id.trim().is_empty()
            || candidate.source_id.trim().is_empty()
            || candidate.title.trim().is_empty()
    }) {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "candidate identity and title are required".into(),
        ));
    }
    let mut ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "candidate identifiers must be unique".into(),
        ));
    }
    if request
        .adversarial_events
        .iter()
        .any(|event| event.trim().is_empty())
    {
        return Err(RetrievalSynthesisAssuranceError::Invalid(
            "adversarial event labels must be non-empty".into(),
        ));
    }
    Ok(())
}

pub fn verify(
    request: &ScopedRetrievalQuery,
) -> Result<RetrievalSynthesisAssuranceReceipt, RetrievalSynthesisAssuranceError> {
    validate_request(request)?;
    let required_sources = request
        .required_source_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let observed_sources = candidates
        .iter()
        .map(|candidate| candidate.source_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_sources = required_sources
        .difference(&observed_sources)
        .cloned()
        .collect::<Vec<_>>();
    let global_failed = [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("raw-data-locality", !request.raw_data_local),
        ("adversarial-input", !request.adversarial_events.is_empty()),
    ]
    .into_iter()
    .filter_map(|(gate, failed)| failed.then_some(gate.to_string()))
    .collect::<BTreeSet<_>>();
    let mut scores = BTreeMap::new();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut stale = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut conditional = BTreeSet::<String>::new();
        if !required_sources.contains(&candidate.source_id) {
            failed.insert("source-not-authorized".into());
            omissions.insert(format!("{}:source-not-authorized", candidate.candidate_id));
        }
        if candidate.content_digest.is_none() {
            conditional.insert("content-digest-missing".into());
            omissions.insert(format!("{}:content-digest-missing", candidate.candidate_id));
        }
        if candidate.provenance_digest.is_none() {
            conditional.insert("provenance-missing".into());
            omissions.insert(format!("{}:provenance-missing", candidate.candidate_id));
        }
        if candidate.freshness_days > request.max_freshness_days {
            conditional.insert("stale-evidence".into());
            stale.insert(candidate.candidate_id.clone());
            omissions.insert(format!("{}:stale", candidate.candidate_id));
        }
        if !candidate.omissions.is_empty() {
            conditional.insert("candidate-omissions".into());
            omissions.extend(
                candidate
                    .omissions
                    .iter()
                    .map(|item| format!("{}:{item}", candidate.candidate_id)),
            );
        }
        if !candidate.uncertainty.is_empty() {
            conditional.insert("candidate-uncertainty".into());
            uncertainty.extend(
                candidate
                    .uncertainty
                    .iter()
                    .map(|item| format!("{}:{item}", candidate.candidate_id)),
            );
        }
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
                contradiction.insert(candidate.candidate_id.clone());
                negative.insert(format!("{}:contradicted", candidate.candidate_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                conditional.insert("evidence-state-not-qualified".into());
                uncertainty.insert(format!("{}:evidence-state", candidate.candidate_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if candidate.negative_result {
            negative.insert(format!("{}:negative-result", candidate.candidate_id));
        } else {
            negative.insert(format!(
                "{}:negative-result-not-observed",
                candidate.candidate_id
            ));
        }
        let score = candidate.relevance_milli as i64
            + match candidate.evidence_state {
                EvidenceState::Proven => 20_000,
                EvidenceState::Supported => 10_000,
                _ => 0,
            }
            - i64::from(candidate.freshness_days) * 10
            - (conditional.len() as i64 * 500);
        scores.insert(candidate.candidate_id.clone(), score);
        let disposition = if !failed.is_empty() {
            blocked.push(candidate.candidate_id.clone());
            "blocked"
        } else if !conditional.is_empty() {
            unresolved.push(candidate.candidate_id.clone());
            "unresolved"
        } else {
            "eligible"
        };
        decisions.push(json!({
            "candidate_id": candidate.candidate_id,
            "source_id": candidate.source_id,
            "score_milli": score,
            "disposition": disposition,
            "failed_gates": failed.clone().into_iter().collect::<Vec<_>>(),
            "conditional_gates": conditional.into_iter().collect::<Vec<_>>(),
            "negative_result": candidate.negative_result,
        }));
        if !failed.is_empty() {
            semantic_loss.push(SemanticLoss {
                field: format!("candidate:{}", candidate.candidate_id),
                reason: "candidate cannot enter a qualified synthesis after a failed release gate"
                    .into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    let rank_order = candidate_order.iter().cloned().sorted_by(|left, right| {
        scores[right]
            .cmp(&scores[left])
            .then_with(|| left.cmp(right))
    });
    let mut spent = 0_u32;
    for candidate_id in &rank_order {
        if blocked.contains(candidate_id) || unresolved.contains(candidate_id) {
            continue;
        }
        if selected.len() >= request.max_selected {
            unresolved.push(candidate_id.clone());
            omissions.insert(format!("{candidate_id}:selection-capacity"));
            continue;
        }
        let cost = candidates
            .iter()
            .find(|candidate| &candidate.candidate_id == candidate_id)
            .map(|candidate| (candidate.title.len() as u32).saturating_add(1))
            .unwrap_or(1);
        if cost > request.budget_units.saturating_sub(spent) {
            unresolved.push(candidate_id.clone());
            omissions.insert(format!("{candidate_id}:budget-ceiling"));
        } else {
            spent = spent.saturating_add(cost);
            selected.push(candidate_id.clone());
        }
    }
    selected.sort();
    unresolved.sort();
    unresolved.dedup();
    let candidate_set = candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();
    let unresolved_set = unresolved.iter().cloned().collect::<BTreeSet<_>>();
    let blocked_set = blocked.iter().cloned().collect::<BTreeSet<_>>();
    let assigned = selected_set
        .union(&unresolved_set)
        .chain(blocked_set.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in candidate_set.difference(&assigned) {
        unresolved.push(id.clone());
    }
    unresolved.sort();
    unresolved.dedup();
    let selected_sources = candidates
        .iter()
        .filter(|candidate| selected_set.contains(&candidate.candidate_id))
        .map(|candidate| candidate.source_id.clone())
        .collect::<BTreeSet<_>>();
    if selected_sources.len() < request.min_independent_sources as usize {
        omissions.insert(format!(
            "independent-source-quorum:{}/{}",
            selected_sources.len(),
            request.min_independent_sources
        ));
    }
    for source in &missing_sources {
        omissions.insert(format!("missing-source:{source}"));
    }
    let disposition = if !global_failed.is_empty() || !blocked.is_empty() {
        RetrievalDisposition::Blocked
    } else if !unresolved.is_empty()
        || !missing_sources.is_empty()
        || selected_sources.len() < request.min_independent_sources as usize
    {
        RetrievalDisposition::Unresolved
    } else {
        RetrievalDisposition::Qualified
    };
    let mut checks = [
        "schema-version",
        "candidate-identity",
        "content-addressed-evidence",
        "provenance-closure",
        "freshness-window",
        "negative-evidence-retention",
        "policy-boundary",
        "replay-identity",
        "source-quorum",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    checks.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "corpus_id": request.corpus_id,
        "scope": request.scope,
        "query": request.query,
        "candidate_order": candidate_order,
        "rank_order": rank_order,
        "selected_order": selected,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "missing_source_order": missing_sources,
        "decisions": decisions,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let evidence_digest = ContentHash::of_value(&payload)
        .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("evidence-synthesis:{}", request.request_id),
        "application/vnd.aurora.evidence-synthesis+json",
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.corpus_id.clone(),
            relation: "retrieval-synthesis-assurance".into(),
            digest: evidence_digest.clone(),
        }],
    )
    .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(disposition, RetrievalDisposition::Qualified) {
        vec![format!("verify:retrieval-synthesis:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = RetrievalSynthesisAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        corpus_id: request.corpus_id.clone(),
        scope: request.scope.clone(),
        query: request.query.clone(),
        disposition,
        candidate_order,
        rank_order,
        selected_order: selected,
        unresolved_order: unresolved,
        blocked_order: blocked,
        required_source_order: request.required_source_ids.clone(),
        observed_source_order: observed_sources.into_iter().collect(),
        missing_source_order: missing_sources,
        stale_order: stale.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        checks,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        evidence_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn verify_json(value: &Value) -> Result<Value, RetrievalSynthesisAssuranceError> {
    let request: ScopedRetrievalQuery = serde_json::from_value(value.clone())
        .map_err(|error| RetrievalSynthesisAssuranceError::Invalid(error.to_string()))?;
    serde_json::to_value(verify(&request)?)
        .map_err(|error| RetrievalSynthesisAssuranceError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"retrieval-synthesis-assurance")
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

    fn request() -> ScopedRetrievalQuery {
        ScopedRetrievalQuery {
            request_id: "request:retrieval".into(),
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
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn complete_retrieval_qualifies_and_is_replayable() {
        let receipt = verify(&request()).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Qualified);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
        assert_eq!(receipt.selected_order.len(), 2);
    }

    #[test]
    fn stale_and_unknown_evidence_remain_unresolved() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        value.candidates[0].freshness_days = 90;
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Unresolved);
        assert!(receipt.stale_order.contains(&"c-1".into()));
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.starts_with("c-1:")));
    }

    #[test]
    fn contradiction_and_adversarial_input_block_release() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        value.adversarial_events = vec!["poisoned-artifact".into()];
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.starts_with("c-1:")));
    }

    #[test]
    fn missing_source_and_budget_are_explicit() {
        let mut value = request();
        value.required_source_ids.push("source-c".into());
        value.budget_units = 1;
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Unresolved);
        assert!(receipt.missing_source_order.contains(&"source-c".into()));
        assert!(receipt.omissions.iter().any(|item| item.contains("budget")));
    }

    #[test]
    fn manifest_is_a1_and_cli_facing() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Cli));
    }
}

// Iterator sorting is kept local to avoid making the canonical ordering depend on caller locale.
trait SortedBy: Iterator {
    fn sorted_by<F>(self, compare: F) -> Vec<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> std::cmp::Ordering;
}

impl<I: Iterator> SortedBy for I {
    fn sorted_by<F>(self, mut compare: F) -> Vec<Self::Item>
    where
        F: FnMut(&Self::Item, &Self::Item) -> std::cmp::Ordering,
    {
        let mut values = self.collect::<Vec<_>>();
        values.sort_by(|left, right| compare(left, right));
        values
    }
}
