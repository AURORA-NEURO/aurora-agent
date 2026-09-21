//! Join qualified glioma evidence into an actionable scientific frontier.
//!
//! The autonomous engine needs a decision boundary between surveillance and execution.  This
//! feature joins caller-supplied, already-local evidence by claim scope, collapses exact artifact
//! copies, and emits typed next actions: compile supported claims, preserve negative findings,
//! review contradictions, or acquire independent evidence for sparse/unknown claims.  It does
//! not infer causality, retrieve sources, or execute an assay.

use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceFrontierJoin1@1";
pub const MAX_RECORDS: usize = 8_192;
pub const MAX_CLAIMS: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFrontierVerdict {
    Supported,
    Negative,
    Contradicted,
    Unresolved,
    Sparse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFrontierAction {
    CompileKnowledge,
    PreserveNegative,
    ReviewContradiction,
    AcquireIndependentEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFrontierJoinRequest {
    pub objective: String,
    pub records: Vec<EvidenceRecord>,
    pub max_claims: usize,
    pub min_quality_milli: u16,
    pub min_reproducibility_milli: u16,
    pub min_independent_sources: usize,
    pub contradiction_floor_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFrontierClaim {
    pub frontier_id: String,
    pub claim: String,
    pub scope: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub evidence_order: Vec<String>,
    pub independent_source_count: usize,
    pub support_milli: u16,
    pub negative_milli: u16,
    pub contradiction_milli: u16,
    pub unknown_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub verdict: EvidenceFrontierVerdict,
    pub next_action: EvidenceFrontierAction,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFrontierJoin {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub claims: Vec<EvidenceFrontierClaim>,
    pub compile_order: Vec<String>,
    pub acquire_order: Vec<String>,
    pub review_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub deduplicated_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceFrontierVerdict,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceFrontierJoinError {
    #[error("evidence frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence frontier digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn frontier_key(record: &EvidenceRecord) -> String {
    format!(
        "{}|{}|{:?}|{:?}",
        record.claim, record.scope, record.modality, record.model_system
    )
}

fn frontier_id(key: &str) -> String {
    format!("frontier:{}", ContentHash::of_bytes(key.as_bytes()))
}

fn digest_input(output: &EvidenceFrontierJoin) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "compile_order": output.compile_order,
        "acquire_order": output.acquire_order,
        "review_order": output.review_order,
        "negative_order": output.negative_order,
        "rejected_order": output.rejected_order,
        "deduplicated_order": output.deduplicated_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl EvidenceFrontierJoin {
    pub fn validate(&self) -> Result<(), EvidenceFrontierJoinError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.compile_order)
            || !canonical(&self.acquire_order)
            || !canonical(&self.review_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.rejected_order)
            || !canonical(&self.deduplicated_order)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .claims
                    .iter()
                    .map(|claim| claim.frontier_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.claims.iter().any(|claim| {
                claim.frontier_id.trim().is_empty()
                    || claim.claim.trim().is_empty()
                    || claim.scope.trim().is_empty()
                    || claim.evidence_order.is_empty()
                    || !canonical(&claim.evidence_order)
                    || claim.independent_source_count == 0
                    || claim.support_milli > 1_000
                    || claim.negative_milli > 1_000
                    || claim.contradiction_milli > 1_000
                    || claim.unknown_milli > 1_000
                    || claim.quality_milli > 1_000
                    || claim.reproducibility_milli > 1_000
                    || claim.priority_milli > 1_000
                    || claim.rationale.trim().is_empty()
            })
            || self.digest.as_str().len() != 64
        {
            return Err(EvidenceFrontierJoinError::InvalidOutput(
                "identity, canonical partitions, claim metrics, rationale, or digest is invalid"
                    .into(),
            ));
        }
        let claim_ids = self
            .claims
            .iter()
            .map(|claim| claim.frontier_id.clone())
            .collect::<BTreeSet<_>>();
        let claim_order = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let action_union = self
            .compile_order
            .iter()
            .chain(self.acquire_order.iter())
            .chain(self.review_order.iter())
            .chain(self.negative_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if claim_order != claim_ids
            || self.claim_order.len() != claim_ids.len()
            || action_union.len()
                != self.compile_order.len()
                    + self.acquire_order.len()
                    + self.review_order.len()
                    + self.negative_order.len()
            || !action_union.is_subset(&claim_ids)
        {
            return Err(EvidenceFrontierJoinError::InvalidOutput(
                "claim identity or action partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceFrontierJoinError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceFrontierJoinError::Digest(
                "frontier digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn weighted_mean(records: &[&EvidenceRecord], selector: fn(&EvidenceRecord) -> u16) -> u16 {
    let mut numerator = 0_u64;
    let mut denominator = 0_u64;
    for record in records {
        let weight =
            u64::from(record.quality_milli.max(1)) * u64::from(record.reproducibility_milli.max(1));
        numerator = numerator.saturating_add(weight * u64::from(selector(record)));
        denominator = denominator.saturating_add(weight);
    }
    if denominator == 0 {
        0
    } else {
        (numerator / denominator).min(1_000) as u16
    }
}

/// Join local evidence into a deterministic, executable frontier for the autonomous research
/// director.  Exact artifact copies never increase independent support.
pub fn join_glioma_evidence_frontier(
    request: &EvidenceFrontierJoinRequest,
) -> Result<EvidenceFrontierJoin, EvidenceFrontierJoinError> {
    if request.objective.trim().is_empty()
        || request.records.is_empty()
        || request.records.len() > MAX_RECORDS
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.min_quality_milli > 1_000
        || request.min_reproducibility_milli > 1_000
        || request.min_independent_sources == 0
        || request.contradiction_floor_milli > 1_000
    {
        return Err(EvidenceFrontierJoinError::InvalidRequest(
            "objective, record bounds, quality floors, source quorum, or contradiction floor is invalid"
                .into(),
        ));
    }

    let mut groups: BTreeMap<String, Vec<&EvidenceRecord>> = BTreeMap::new();
    let mut rejected_order = BTreeSet::new();
    let mut deduplicated_order = BTreeSet::new();
    for record in &request.records {
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.source_artifact.content_hash.as_str().len() != 64
            || record.source_artifact.contains_human_data
            || record.source_artifact.contains_direct_identifiers
        {
            return Err(EvidenceFrontierJoinError::InvalidRequest(
                "records must be local, preclinical, typed, and content-addressed".into(),
            ));
        }
        if record.quality_milli < request.min_quality_milli
            || record.reproducibility_milli < request.min_reproducibility_milli
        {
            rejected_order.insert(record.evidence_id.clone());
            continue;
        }
        groups.entry(frontier_key(record)).or_default().push(record);
    }

    let mut claims = Vec::new();
    let mut uncertainty = BTreeSet::new();
    for (key, records) in groups {
        let mut by_artifact = BTreeMap::<String, &EvidenceRecord>::new();
        for record in records {
            let artifact = record.source_artifact.content_hash.as_str().to_string();
            if by_artifact.contains_key(&artifact) {
                deduplicated_order.insert(record.evidence_id.clone());
            } else {
                by_artifact.insert(artifact, record);
            }
        }
        let unique = by_artifact.into_values().collect::<Vec<_>>();
        let evidence_order = {
            let mut ids = unique
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect::<Vec<_>>();
            ids.sort();
            ids
        };
        if unique.is_empty() {
            continue;
        }
        let independent_source_count = unique.len();
        let support_milli = weighted_mean(&unique, |record| {
            if matches!(record.state, EvidenceState::Supported) {
                1_000
            } else {
                0
            }
        });
        let negative_milli = weighted_mean(&unique, |record| {
            if matches!(record.state, EvidenceState::Negative) {
                1_000
            } else {
                0
            }
        });
        let contradiction_milli = weighted_mean(&unique, |record| {
            if matches!(record.state, EvidenceState::Contradicted) {
                1_000
            } else {
                0
            }
        });
        let unknown_milli = weighted_mean(&unique, |record| {
            if matches!(
                record.state,
                EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
            ) {
                1_000
            } else {
                0
            }
        });
        let quality_milli = weighted_mean(&unique, |record| record.quality_milli);
        let reproducibility_milli = weighted_mean(&unique, |record| record.reproducibility_milli);
        let verdict = if contradiction_milli >= request.contradiction_floor_milli
            && support_milli >= request.contradiction_floor_milli
        {
            EvidenceFrontierVerdict::Contradicted
        } else if independent_source_count < request.min_independent_sources {
            EvidenceFrontierVerdict::Sparse
        } else if support_milli >= request.contradiction_floor_milli {
            EvidenceFrontierVerdict::Supported
        } else if negative_milli >= request.contradiction_floor_milli {
            EvidenceFrontierVerdict::Negative
        } else {
            EvidenceFrontierVerdict::Unresolved
        };
        let next_action = match verdict {
            EvidenceFrontierVerdict::Supported => EvidenceFrontierAction::CompileKnowledge,
            EvidenceFrontierVerdict::Negative => EvidenceFrontierAction::PreserveNegative,
            EvidenceFrontierVerdict::Contradicted => EvidenceFrontierAction::ReviewContradiction,
            EvidenceFrontierVerdict::Unresolved | EvidenceFrontierVerdict::Sparse => {
                EvidenceFrontierAction::AcquireIndependentEvidence
            }
        };
        let priority_milli = match next_action {
            EvidenceFrontierAction::ReviewContradiction => 1_000,
            EvidenceFrontierAction::AcquireIndependentEvidence => {
                850_u16.saturating_add(unknown_milli / 7).min(1_000)
            }
            EvidenceFrontierAction::PreserveNegative => 700,
            EvidenceFrontierAction::CompileKnowledge => {
                quality_milli.saturating_add(reproducibility_milli) / 2
            }
        };
        if matches!(
            verdict,
            EvidenceFrontierVerdict::Sparse | EvidenceFrontierVerdict::Unresolved
        ) {
            uncertainty.insert(format!(
                "{}: independent support is below the frontier gate",
                frontier_id(&key)
            ));
        }
        claims.push(EvidenceFrontierClaim {
            frontier_id: frontier_id(&key),
            claim: unique[0].claim.clone(),
            scope: unique[0].scope.clone(),
            modality: unique[0].modality,
            model_system: unique[0].model_system,
            evidence_order,
            independent_source_count,
            support_milli,
            negative_milli,
            contradiction_milli,
            unknown_milli,
            quality_milli,
            reproducibility_milli,
            verdict,
            next_action,
            priority_milli,
            rationale: match verdict {
                EvidenceFrontierVerdict::Supported => {
                    "independent support clears the compilation gate".into()
                }
                EvidenceFrontierVerdict::Negative => {
                    "negative evidence is retained as a first-class result".into()
                }
                EvidenceFrontierVerdict::Contradicted => {
                    "positive and contradictory states require explicit review".into()
                }
                EvidenceFrontierVerdict::Sparse => "independent source quorum is not met".into(),
                EvidenceFrontierVerdict::Unresolved => {
                    "evidence remains unknown or below the support gate".into()
                }
            },
        });
    }

    claims.sort_by(|left, right| left.frontier_id.cmp(&right.frontier_id));
    if claims.len() > request.max_claims {
        claims.truncate(request.max_claims);
        uncertainty.insert("claim frontier truncated at the configured bound".into());
    }
    let claim_order = claims
        .iter()
        .map(|claim| claim.frontier_id.clone())
        .collect::<Vec<_>>();
    let mut compile_order = Vec::new();
    let mut acquire_order = Vec::new();
    let mut review_order = Vec::new();
    let mut negative_order = Vec::new();
    for claim in &claims {
        match claim.next_action {
            EvidenceFrontierAction::CompileKnowledge => {
                compile_order.push(claim.frontier_id.clone())
            }
            EvidenceFrontierAction::AcquireIndependentEvidence => {
                acquire_order.push(claim.frontier_id.clone())
            }
            EvidenceFrontierAction::ReviewContradiction => {
                review_order.push(claim.frontier_id.clone())
            }
            EvidenceFrontierAction::PreserveNegative => {
                negative_order.push(claim.frontier_id.clone())
            }
        }
    }
    let disposition = if !review_order.is_empty() {
        EvidenceFrontierVerdict::Contradicted
    } else if !acquire_order.is_empty() {
        EvidenceFrontierVerdict::Unresolved
    } else if !negative_order.is_empty() && compile_order.is_empty() {
        EvidenceFrontierVerdict::Negative
    } else {
        EvidenceFrontierVerdict::Supported
    };
    let mut output = EvidenceFrontierJoin {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        claims,
        compile_order,
        acquire_order,
        review_order,
        negative_order,
        rejected_order: rejected_order.into_iter().collect(),
        deduplicated_order: deduplicated_order.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| EvidenceFrontierJoinError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceFrontierJoinError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::LocalArtifactRef;

    fn record(id: &str, artifact: &str, state: EvidenceState) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: artifact.into(),
                content_hash: ContentHash::of_bytes(artifact.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Assay,
            claim: "egfr resistance".into(),
            scope: "organoid".into(),
            modality: GliomaModality::FunctionalPerturbation,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request(records: Vec<EvidenceRecord>) -> EvidenceFrontierJoinRequest {
        EvidenceFrontierJoinRequest {
            objective: "route glioma evidence frontier".into(),
            records,
            max_claims: 10,
            min_quality_milli: 700,
            min_reproducibility_milli: 700,
            min_independent_sources: 2,
            contradiction_floor_milli: 500,
        }
    }

    #[test]
    fn routes_supported_claims_and_collapses_exact_copies() {
        let output = join_glioma_evidence_frontier(&request(vec![
            record("a", "artifact-a", EvidenceState::Supported),
            record("a-copy", "artifact-a", EvidenceState::Supported),
            record("b", "artifact-b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(output.compile_order.len(), 1);
        assert_eq!(output.claims[0].independent_source_count, 2);
        assert_eq!(output.deduplicated_order, vec!["a-copy"]);
        output.validate().unwrap();
    }

    #[test]
    fn routes_mixed_positive_and_contradiction_to_review() {
        let output = join_glioma_evidence_frontier(&request(vec![
            record("a", "artifact-a", EvidenceState::Supported),
            record("b", "artifact-b", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(output.review_order.len(), 1);
        assert_eq!(output.disposition, EvidenceFrontierVerdict::Contradicted);
    }

    #[test]
    fn routes_sparse_claim_to_independent_acquisition() {
        let output = join_glioma_evidence_frontier(&request(vec![record(
            "a",
            "artifact-a",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(output.acquire_order.len(), 1);
        assert_eq!(output.claims[0].verdict, EvidenceFrontierVerdict::Sparse);
    }
}
