//! Shared typed retrieval-and-synthesis inference kernel for Worldgen P02 F01–F04.
//!
//! The kernel ranks caller-supplied evidence summaries only. It never fetches literature or
//! exports raw data, and every outcome retains uncertainty, omissions, negative evidence, and a
//! replay identity so that a researcher can audit the decision.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub candidate_id: String,
    pub source_id: String,
    pub title: String,
    pub study_id: String,
    pub modality: String,
    pub relevance_milli: u16,
    pub freshness_milli: u16,
    pub evidence_state: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub permitted: bool,
    pub comparable: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub request_id: String,
    pub researcher: String,
    pub corpus_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub query_terms: Vec<String>,
    pub candidates: Vec<RetrievalCandidate>,
    pub minimum_relevance_milli: u16,
    pub minimum_freshness_milli: u16,
    pub max_budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub corpus_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    /// Candidate IDs in canonical lexical order.
    pub candidate_order: Vec<String>,
    /// Candidate IDs in descending relevance/freshness ranking order.
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub source_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub ranked_scores_milli: Vec<u32>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub artifact: serde_json::Value,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalError {
    #[error("invalid retrieval request: {0}")]
    Invalid(String),
    #[error("invalid retrieval receipt: {0}")]
    Receipt(String),
    #[error("retrieval artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted(value: &serde_json::Value) -> Vec<String> {
    let mut output = value
        .as_array()
        .expect("set values serialize as arrays")
        .iter()
        .map(|item| item.as_str().expect("set values are strings").to_owned())
        .collect::<Vec<_>>();
    output.sort();
    output.dedup();
    output
}

impl RetrievalReceipt {
    pub fn validate(&self) -> Result<(), RetrievalError> {
        if self.schema_version != SCHEMA_VERSION
            || self.boundary != BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.corpus_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_scores_milli.len() != self.ranked_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalError::Receipt(
                "retrieval identity, locality, candidates, ranking, scores, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.source_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(RetrievalError::Receipt(
                    "retrieval ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(RetrievalError::Receipt(
                "retrieval states do not partition candidates".into(),
            ));
        }
        if self.ranked_order.iter().any(|id| !ids.contains(id))
            || self.ranked_order.iter().cloned().collect::<BTreeSet<_>>() != ids
            || !digest(&self.replay_identity)
            || !digest(&self.synthesis_digest)
        {
            return Err(RetrievalError::Receipt(
                "retrieval ranking or digest is invalid".into(),
            ));
        }
        if self.artifact.get("content_hash").and_then(|value| value.as_str())
            != Some(self.synthesis_digest.as_str())
            || self.artifact.get("boundary").and_then(|value| value.as_str()) != Some(BOUNDARY)
        {
            return Err(RetrievalError::Receipt(
                "retrieval artifact is inconsistent".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalError::Receipt(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| RetrievalError::Receipt(error.to_string()))
    }
}

pub fn manifest(
    feature_id: &str,
    version: &str,
    input_schema: &str,
    scale: &str,
    autonomy: &str,
) -> serde_json::Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": version,
        "owner_crate": "worldgen",
        "consumers": ["imaging core scientist", "benchmark curator", "research program lead", "preclinical neuroscientist"],
        "behavior": format!("rank typed evidence summaries for {scale} retrieval and synthesis without network retrieval"),
        "value": "turns bounded evidence candidates into auditable, omission-aware synthesis receipts",
        "input_schema": input_schema,
        "output_schema": "EvidenceSynthesis1@1",
        "effects": ["retain:local-evidence-synthesis", "block:unsafe-release"],
        "permissions": ["read:local-research-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": BOUNDARY,
    })
}

pub fn infer(
    query: &RetrievalQuery,
    feature_id: &str,
    contract_version: &str,
) -> Result<RetrievalReceipt, RetrievalError> {
    if query.request_id.trim().is_empty()
        || query.researcher.trim().is_empty()
        || query.corpus_id.trim().is_empty()
        || query.purpose.trim().is_empty()
        || query.semantic_profile.trim().is_empty()
        || query.query_terms.is_empty()
        || query.candidates.is_empty()
        || query.max_budget_units == 0
        || query.boundary != BOUNDARY
        || !query.raw_data_local
        || !query.aggregate_only
        || !digest(&query.replay_identity)
    {
        return Err(RetrievalError::Invalid(
            "retrieval identity, terms, candidates, budget, replay, locality, or boundary is invalid".into(),
        ));
    }

    let mut ranked = query.candidates.clone();
    ranked.sort_by(|left, right| {
        let left_score = left.relevance_milli as u32 * 7 + left.freshness_milli as u32 * 3;
        let right_score = right.relevance_milli as u32 * 7 + right.freshness_milli as u32 * 3;
        right_score
            .cmp(&left_score)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    if ranked.windows(2).any(|pair| pair[0].candidate_id == pair[1].candidate_id)
        || ranked.iter().any(|candidate| {
            candidate.candidate_id.trim().is_empty()
                || candidate.source_id.trim().is_empty()
                || candidate.title.trim().is_empty()
                || candidate.study_id.trim().is_empty()
                || candidate.modality.trim().is_empty()
                || !digest(&candidate.content_digest)
                || !digest(&candidate.provenance_digest)
                || !digest(&candidate.replay_identity)
        })
    {
        return Err(RetrievalError::Invalid(
            "candidate ids, labels, sources, or digests are invalid".into(),
        ));
    }

    let mut canonical = ranked
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    canonical.sort();
    let ranked_order = ranked
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let ranked_scores_milli = ranked
        .iter()
        .map(|candidate| candidate.relevance_milli as u32 * 7 + candidate.freshness_milli as u32 * 3)
        .collect::<Vec<_>>();

    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut total_units = 0u64;

    for candidate in &ranked {
        let fits_budget = total_units
            .checked_add(candidate.estimated_units)
            .is_some_and(|total| total <= query.max_budget_units);
        let qualified = query.policy_allow
            && query.protected_closure
            && candidate.permitted
            && candidate.comparable
            && candidate.evidence_state == "supported"
            && candidate.relevance_milli >= query.minimum_relevance_milli
            && candidate.freshness_milli >= query.minimum_freshness_milli
            && candidate.replay_identity == query.replay_identity
            && fits_budget;
        if qualified {
            selected.insert(candidate.candidate_id.clone());
            sources.insert(candidate.source_id.clone());
            total_units += candidate.estimated_units;
            continue;
        }

        let unresolved_state = matches!(candidate.evidence_state.as_str(), "unknown" | "unmeasured" | "speculative");
        if unresolved_state {
            unresolved.insert(candidate.candidate_id.clone());
            uncertainty.insert(format!("candidate:{}:evidence-unresolved", candidate.candidate_id));
        } else {
            blocked.insert(candidate.candidate_id.clone());
        }
        if candidate.negative_result {
            negative.insert(format!("candidate:{}:negative-result-retained", candidate.candidate_id));
        }
        if candidate.relevance_milli < query.minimum_relevance_milli {
            uncertainty.insert(format!("candidate:{}:low-relevance", candidate.candidate_id));
        }
        if candidate.freshness_milli < query.minimum_freshness_milli {
            uncertainty.insert(format!("candidate:{}:stale", candidate.candidate_id));
        }
        if !candidate.comparable {
            omissions.insert(format!("candidate:{}:incomparable", candidate.candidate_id));
        }
        if candidate.replay_identity != query.replay_identity {
            uncertainty.insert(format!("candidate:{}:replay-mismatch", candidate.candidate_id));
        }
        if !fits_budget {
            omissions.insert(format!("candidate:{}:budget-exceeded", candidate.candidate_id));
        }
        if !candidate.permitted {
            omissions.insert(format!("candidate:{}:permission-denied", candidate.candidate_id));
        }
        if candidate.evidence_state != "supported" {
            uncertainty.insert(format!("candidate:{}:evidence-state-{}", candidate.candidate_id, candidate.evidence_state));
        }
    }
    if !query.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !query.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }

    let disposition = if !query.policy_allow || !query.protected_closure {
        "blocked"
    } else if selected.is_empty() {
        "unknown"
    } else if blocked.is_empty() && omissions.is_empty() && uncertainty.is_empty() && negative.is_empty() {
        "qualified"
    } else {
        "partial"
    };
    let effect_receipts = if disposition == "qualified" {
        vec![format!("retain:local-evidence-synthesis:{}", query.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };

    let payload = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": query.request_id,
        "researcher": query.researcher,
        "corpus_id": query.corpus_id,
        "semantic_profile": query.semantic_profile,
        "disposition": disposition,
        "candidate_order": canonical,
        "ranked_order": ranked_order,
        "selected_order": selected,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "source_order": sources,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "ranked_scores_milli": ranked_scores_milli,
        "total_units": total_units,
        "replay_identity": query.replay_identity,
        "boundary": BOUNDARY,
    });
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|error| RetrievalError::Artifact(error.to_string()))?;
    let receipt = RetrievalReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: query.request_id.clone(),
        researcher: query.researcher.clone(),
        corpus_id: query.corpus_id.clone(),
        semantic_profile: query.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order: sorted(&payload["candidate_order"]),
        ranked_order: payload["ranked_order"]
            .as_array()
            .expect("ranked order is an array")
            .iter()
            .map(|value| value.as_str().expect("ranked ids are strings").into())
            .collect(),
        selected_order: sorted(&payload["selected_order"]),
        unresolved_order: sorted(&payload["unresolved_order"]),
        blocked_order: sorted(&payload["blocked_order"]),
        source_order: sorted(&payload["source_order"]),
        omission_order: sorted(&payload["omission_order"]),
        uncertainty_order: sorted(&payload["uncertainty_order"]),
        negative_evidence_order: sorted(&payload["negative_evidence_order"]),
        ranked_scores_milli: payload["ranked_scores_milli"]
            .as_array()
            .expect("ranked scores are an array")
            .iter()
            .map(|value| value.as_u64().expect("ranked scores are integers") as u32)
            .collect(),
        total_units,
        replay_identity: query.replay_identity.clone(),
        synthesis_digest: synthesis_digest.clone(),
        artifact: json!({
            "artifact_id": format!("evidence-synthesis:{}", query.request_id),
            "content_type": "application/vnd.aurora.worldgen.evidence-synthesis-1+json",
            "content_hash": synthesis_digest,
            "boundary": BOUNDARY,
        }),
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn candidate(id: &str, score: u16, state: &str, replay: &str) -> RetrievalCandidate {
        RetrievalCandidate {
            candidate_id: id.into(),
            source_id: format!("source:{id}"),
            title: format!("Preclinical study {id}"),
            study_id: format!("study:{id}"),
            modality: "imaging".into(),
            relevance_milli: score,
            freshness_milli: score,
            evidence_state: state.into(),
            content_digest: hash(&format!("content:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash(replay),
            estimated_units: 2,
            permitted: true,
            comparable: true,
            negative_result: false,
        }
    }

    fn query() -> RetrievalQuery {
        RetrievalQuery {
            request_id: "retrieval:req".into(),
            researcher: "scientist:imaging".into(),
            corpus_id: "corpus:local".into(),
            purpose: "compare synaptic phenotype evidence".into(),
            semantic_profile: "ome-ngff+prov".into(),
            query_terms: vec!["synapse".into(), "organoid".into()],
            candidates: vec![candidate("b", 700, "supported", "replay"), candidate("a", 900, "supported", "replay")],
            minimum_relevance_milli: 600,
            minimum_freshness_milli: 500,
            max_budget_units: 10,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn ranks_and_qualifies_supported_candidates() {
        let receipt = infer(&query(), "AFA-worldgen-P02-F01", "worldgen-local-retrieval-synthesis-inference/1.0").unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.ranked_order, vec!["a", "b"]);
        assert_eq!(receipt.candidate_order, vec!["a", "b"]);
        assert_eq!(receipt.selected_order, vec!["a", "b"]);
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn retains_unknown_and_negative_evidence_without_release() {
        let mut request = query();
        request.candidates[0].evidence_state = "unknown".into();
        request.candidates[0].negative_result = true;
        let receipt = infer(&request, "AFA-worldgen-P02-F02", "worldgen-multimodal-retrieval-synthesis-inference/1.0").unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert!(receipt.unresolved_order.contains(&"b".into()));
        assert!(receipt.negative_evidence_order.iter().any(|value| value.contains("negative-result")));
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn policy_and_protected_closure_fail_closed() {
        let mut request = query();
        request.policy_allow = false;
        request.protected_closure = false;
        let receipt = infer(&request, "AFA-worldgen-P02-F04", "worldgen-federated-continual-retrieval-synthesis-inference/1.0").unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.omission_order.contains(&"request:policy-denied".into()));
        assert!(receipt.uncertainty_order.contains(&"request:protected-closure-incomplete".into()));
    }

    #[test]
    fn budget_exhaustion_is_an_explicit_omission() {
        let mut request = query();
        request.max_budget_units = 2;
        let receipt = infer(&request, "AFA-worldgen-P02-F03", "worldgen-throughput-retrieval-synthesis-inference/1.0").unwrap();
        assert_eq!(receipt.selected_order, vec!["a"]);
        assert!(receipt.omission_order.iter().any(|value| value.contains("budget-exceeded")));
    }
}
