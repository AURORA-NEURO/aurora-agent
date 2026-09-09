//! Cross-family evidence triangulation for preclinical glioma claims.
//!
//! Qualification answers whether individual records clear local quality gates. Triangulation
//! answers the harder autonomous-research question: does a scoped claim survive independent
//! source families, independent artifacts, contradiction pressure, incomplete states, and
//! leave-one-source-out fragility? The input remains local and value-only; this module does not
//! fetch literature or turn correlation into a mechanistic or clinical conclusion.

use super::super::super::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceTriangulation1@1";
pub const MAX_RECORDS: usize = 16_384;
pub const MAX_CLAIMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriangulationRequest {
    pub objective: String,
    pub min_source_kinds: usize,
    pub min_independent_artifacts: usize,
    pub min_support_milli: u16,
    pub max_contradiction_milli: u16,
    pub min_diversity_milli: u16,
    pub max_leave_one_artifact_shift_milli: u16,
    pub max_claims: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriangulatedClaimVerdict {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriangulatedClaim {
    pub claim_id: String,
    pub claim: String,
    pub scope: String,
    pub evidence_order: Vec<String>,
    pub supporting_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub source_kind_order: Vec<EvidenceSourceKind>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub independent_artifact_count: usize,
    pub support_score_milli: u16,
    pub negative_score_milli: u16,
    pub contradiction_score_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub diversity_milli: u16,
    pub leave_one_artifact_shift_milli: u16,
    pub rationale_order: Vec<String>,
    pub verdict: TriangulatedClaimVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTriangulationDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTriangulation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub claims: Vec<TriangulatedClaim>,
    pub qualified_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceTriangulationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceTriangulationError {
    #[error("evidence triangulation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence triangulation record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence triangulation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence triangulation digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn claim_id(claim: &str, scope: &str) -> String {
    let key = format!("{claim}\u{1f}{scope}");
    format!("claim:{}", ContentHash::of_bytes(key.as_bytes()))
}

fn confidence_milli(record: &EvidenceRecord) -> u64 {
    u64::from(record.quality_milli)
        .saturating_mul(u64::from(record.relevance_milli))
        .checked_div(1_000)
        .unwrap_or(0)
        .saturating_mul(u64::from(record.reproducibility_milli))
        .checked_div(1_000)
        .unwrap_or(0)
}

fn mean_milli(values: &[u64]) -> u16 {
    if values.is_empty() {
        0
    } else {
        (values.iter().sum::<u64>() / values.len() as u64).min(1_000) as u16
    }
}

fn support_score(records: &[&EvidenceRecord], excluded_artifact: Option<&str>) -> u16 {
    let values = records
        .iter()
        .filter(|record| {
            excluded_artifact != Some(record.source_artifact.artifact_id.as_str())
                && record.state == EvidenceState::Supported
        })
        .map(|record| confidence_milli(record))
        .collect::<Vec<_>>();
    mean_milli(&values)
}

fn state_score(
    records: &[&EvidenceRecord],
    states: &[EvidenceState],
    excluded_artifact: Option<&str>,
) -> u16 {
    let values = records
        .iter()
        .filter(|record| {
            excluded_artifact != Some(record.source_artifact.artifact_id.as_str())
                && states.contains(&record.state)
        })
        .map(|record| confidence_milli(record))
        .collect::<Vec<_>>();
    mean_milli(&values)
}

fn digest_input(output: &EvidenceTriangulation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "qualified_order": output.qualified_order,
        "partial_order": output.partial_order,
        "negative_order": output.negative_order,
        "unresolved_order": output.unresolved_order,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl TriangulatedClaim {
    pub fn validate(&self) -> Result<(), EvidenceTriangulationError> {
        if self.claim_id.trim().is_empty()
            || self.claim.trim().is_empty()
            || self.scope.trim().is_empty()
            || !canonical(&self.evidence_order)
            || !canonical(&self.supporting_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradictory_order)
            || !canonical(&self.uncertain_order)
            || !canonical(&self.source_kind_order)
            || !canonical(&self.model_system_order)
            || !canonical(&self.rationale_order)
            || self.independent_artifact_count == 0
            || self.support_score_milli > 1_000
            || self.negative_score_milli > 1_000
            || self.contradiction_score_milli > 1_000
            || self.quality_milli > 1_000
            || self.reproducibility_milli > 1_000
            || self.diversity_milli > 1_000
            || self.leave_one_artifact_shift_milli > 1_000
        {
            return Err(EvidenceTriangulationError::InvalidOutput(
                "claim identity, ordering, diversity, or score bounds are invalid".into(),
            ));
        }
        let all = self.evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .supporting_order
            .iter()
            .chain(&self.negative_order)
            .chain(&self.contradictory_order)
            .chain(&self.uncertain_order)
            .cloned()
            .collect::<Vec<_>>();
        if all.len() != self.evidence_order.len()
            || parts.len() != all.len()
            || BTreeSet::from_iter(parts) != all
        {
            return Err(EvidenceTriangulationError::InvalidOutput(
                "claim evidence states do not partition records".into(),
            ));
        }
        Ok(())
    }
}

impl EvidenceTriangulation {
    pub fn validate(&self) -> Result<(), EvidenceTriangulationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.qualified_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.claims.len() != self.claim_order.len()
            || self.claims.iter().any(|claim| claim.validate().is_err())
        {
            return Err(EvidenceTriangulationError::InvalidOutput(
                "identity, ordering, claim count, or claim invariants are invalid".into(),
            ));
        }
        let claims = self
            .claims
            .iter()
            .map(|claim| claim.claim_id.clone())
            .collect::<BTreeSet<_>>();
        let claim_order = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partial = self.partial_order.iter().cloned().collect::<BTreeSet<_>>();
        let negative = self.negative_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if claims != claim_order
            || claims.len() != self.claims.len()
            || qualified.intersection(&partial).next().is_some()
            || qualified.intersection(&negative).next().is_some()
            || qualified.intersection(&unresolved).next().is_some()
            || partial.intersection(&negative).next().is_some()
            || partial.intersection(&unresolved).next().is_some()
            || negative.intersection(&unresolved).next().is_some()
            || qualified
                .union(&partial)
                .chain(negative.iter())
                .chain(unresolved.iter())
                .any(|id| !claims.contains(id))
        {
            return Err(EvidenceTriangulationError::InvalidOutput(
                "claim verdict partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceTriangulationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceTriangulationError::InvalidOutput(
                "triangulation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &EvidenceTriangulationRequest,
) -> Result<(), EvidenceTriangulationError> {
    if request.objective.trim().is_empty()
        || request.min_source_kinds == 0
        || request.min_independent_artifacts == 0
        || request.min_support_milli == 0
        || request.max_contradiction_milli > 1_000
        || request.min_diversity_milli > 1_000
        || request.max_leave_one_artifact_shift_milli > 1_000
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
    {
        return Err(EvidenceTriangulationError::InvalidRequest(
            "objective, independence floors, score gates, diversity, and claim bounds are required"
                .into(),
        ));
    }
    Ok(())
}

/// Triangulate scoped claims across independent source families and artifacts.
pub fn triangulate_glioma_evidence(
    request: &EvidenceTriangulationRequest,
    records: &[EvidenceRecord],
) -> Result<EvidenceTriangulation, EvidenceTriangulationError> {
    validate_request(request)?;
    if records.is_empty() || records.len() > MAX_RECORDS {
        return Err(EvidenceTriangulationError::InvalidRecord(
            "a bounded, non-empty evidence set is required".into(),
        ));
    }
    let mut record_ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| EvidenceTriangulationError::InvalidRecord(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !record_ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceTriangulationError::InvalidRecord(
                "evidence identity, scope, score bounds, and uniqueness are required".into(),
            ));
        }
    }

    let mut groups = BTreeMap::<(String, String), Vec<&EvidenceRecord>>::new();
    for record in records {
        groups
            .entry((record.claim.clone(), record.scope.clone()))
            .or_default()
            .push(record);
    }
    if groups.len() > request.max_claims {
        return Err(EvidenceTriangulationError::InvalidRecord(
            "claim count exceeds the configured bound".into(),
        ));
    }

    let mut claims = Vec::with_capacity(groups.len());
    let mut next_actions = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for ((claim, scope), mut group) in groups {
        group.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
        let claim_id = claim_id(&claim, &scope);
        let evidence_order = group
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        let supporting_order = group
            .iter()
            .filter(|record| record.state == EvidenceState::Supported)
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        let negative_order = group
            .iter()
            .filter(|record| record.state == EvidenceState::Negative)
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        let contradictory_order = group
            .iter()
            .filter(|record| record.state == EvidenceState::Contradicted)
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        let uncertain_order = group
            .iter()
            .filter(|record| {
                matches!(
                    record.state,
                    EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
                )
            })
            .map(|record| record.evidence_id.clone())
            .collect::<Vec<_>>();
        let source_kind_order = group
            .iter()
            .map(|record| record.source_kind)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let model_system_order = group
            .iter()
            .filter_map(|record| record.model_system)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let independent_artifacts = group
            .iter()
            .map(|record| record.source_artifact.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        let full_support_score = support_score(&group, None);
        let negative_score = state_score(&group, &[EvidenceState::Negative], None);
        let contradiction_score = state_score(&group, &[EvidenceState::Contradicted], None);
        let quality_milli = mean_milli(
            &group
                .iter()
                .map(|record| u64::from(record.quality_milli))
                .collect::<Vec<_>>(),
        );
        let reproducibility_milli = mean_milli(
            &group
                .iter()
                .map(|record| u64::from(record.reproducibility_milli))
                .collect::<Vec<_>>(),
        );
        let diversity_milli = (u64::from(source_kind_order.len() as u16)
            .saturating_mul(1_000)
            .checked_div(request.min_source_kinds as u64)
            .unwrap_or(0)
            .min(1_000)) as u16;
        let leave_one_artifact_shift = independent_artifacts
            .iter()
            .map(|artifact| {
                support_score(&group, Some(artifact.as_str())).abs_diff(full_support_score)
            })
            .max()
            .unwrap_or(0);
        let has_incomplete = !uncertain_order.is_empty();
        let structure_ok = source_kind_order.len() >= request.min_source_kinds
            && independent_artifacts.len() >= request.min_independent_artifacts
            && diversity_milli >= request.min_diversity_milli;
        let support_ok = full_support_score >= request.min_support_milli;
        let contradiction_ok = contradiction_score <= request.max_contradiction_milli;
        let stability_ok = leave_one_artifact_shift <= request.max_leave_one_artifact_shift_milli;
        let verdict = if structure_ok
            && support_ok
            && contradiction_ok
            && stability_ok
            && !has_incomplete
            && negative_order.is_empty()
        {
            TriangulatedClaimVerdict::Qualified
        } else if full_support_score == 0
            && (negative_score >= request.min_support_milli
                || contradiction_score > request.max_contradiction_milli)
        {
            TriangulatedClaimVerdict::Negative
        } else if full_support_score > 0
            && structure_ok
            && (has_incomplete || !contradiction_ok || !stability_ok || !negative_order.is_empty())
        {
            TriangulatedClaimVerdict::Partial
        } else {
            TriangulatedClaimVerdict::Unresolved
        };
        let mut rationale = BTreeSet::from([
            format!("support-score:{full_support_score}"),
            format!("contradiction-score:{contradiction_score}"),
            format!("negative-score:{negative_score}"),
            format!("source-kinds:{}", source_kind_order.len()),
            format!("independent-artifacts:{}", independent_artifacts.len()),
            format!("diversity-score:{diversity_milli}"),
            format!("leave-one-artifact-shift:{leave_one_artifact_shift}"),
        ]);
        if has_incomplete {
            rationale.insert("incomplete-state-present".into());
            uncertainty.insert(format!("incomplete-claim:{claim_id}"));
        }
        if !contradictory_order.is_empty() {
            negative_evidence.insert(format!("contradictory-claim:{claim_id}"));
            next_actions.insert(format!("resolve-contradiction:{claim_id}"));
        }
        if !structure_ok {
            next_actions.insert(format!("add-independent-source:{claim_id}"));
        }
        if !stability_ok {
            next_actions.insert(format!("replicate-independent-source:{claim_id}"));
        }
        if !support_ok && full_support_score > 0 {
            next_actions.insert(format!("acquire-supporting-evidence:{claim_id}"));
        }
        if !uncertain_order.is_empty() {
            next_actions.insert(format!("resolve-unknown-state:{claim_id}"));
        }
        if !negative_order.is_empty() {
            negative_evidence.insert(format!("negative-claim:{claim_id}"));
            next_actions.insert(format!("revalidate-negative:{claim_id}"));
        }
        if leave_one_artifact_shift > request.max_leave_one_artifact_shift_milli {
            rationale.insert("source-dominance-gate-failed".into());
        }
        claims.push(TriangulatedClaim {
            claim_id,
            claim,
            scope,
            evidence_order,
            supporting_order,
            negative_order,
            contradictory_order,
            uncertain_order,
            source_kind_order,
            model_system_order,
            independent_artifact_count: independent_artifacts.len(),
            support_score_milli: full_support_score,
            negative_score_milli: negative_score,
            contradiction_score_milli: contradiction_score,
            quality_milli,
            reproducibility_milli,
            diversity_milli,
            leave_one_artifact_shift_milli: leave_one_artifact_shift,
            rationale_order: rationale.into_iter().collect(),
            verdict,
        });
    }
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let qualified_order = claims
        .iter()
        .filter(|claim| claim.verdict == TriangulatedClaimVerdict::Qualified)
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let partial_order = claims
        .iter()
        .filter(|claim| claim.verdict == TriangulatedClaimVerdict::Partial)
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let negative_order = claims
        .iter()
        .filter(|claim| claim.verdict == TriangulatedClaimVerdict::Negative)
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let unresolved_order = claims
        .iter()
        .filter(|claim| claim.verdict == TriangulatedClaimVerdict::Unresolved)
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !claims.is_empty() && qualified_order.len() == claims.len() {
        EvidenceTriangulationDisposition::Qualified
    } else if !partial_order.is_empty() || !qualified_order.is_empty() {
        EvidenceTriangulationDisposition::Partial
    } else if !negative_order.is_empty() && unresolved_order.is_empty() {
        EvidenceTriangulationDisposition::Negative
    } else {
        EvidenceTriangulationDisposition::Unresolved
    };
    let mut output = EvidenceTriangulation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        claims,
        qualified_order,
        partial_order,
        negative_order,
        unresolved_order,
        next_action_order: next_actions.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-triangulation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceTriangulationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

    fn record(
        id: &str,
        artifact_id: &str,
        kind: EvidenceSourceKind,
        state: EvidenceState,
    ) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: artifact_id.into(),
                content_hash: ContentHash::of_bytes(artifact_id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: kind,
            claim: "EGFR signaling increases organoid invasion".into(),
            scope: "organoid:invasion".into(),
            modality: GliomaModality::FunctionalPerturbation,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request() -> EvidenceTriangulationRequest {
        EvidenceTriangulationRequest {
            objective: "triangulate EGFR invasion evidence".into(),
            min_source_kinds: 3,
            min_independent_artifacts: 3,
            min_support_milli: 600,
            max_contradiction_milli: 200,
            min_diversity_milli: 1_000,
            max_leave_one_artifact_shift_milli: 100,
            max_claims: 8,
        }
    }

    #[test]
    fn independent_source_families_qualify_and_replay() {
        let records = vec![
            record(
                "e1",
                "literature-1",
                EvidenceSourceKind::Literature,
                EvidenceState::Supported,
            ),
            record(
                "e2",
                "assay-1",
                EvidenceSourceKind::Assay,
                EvidenceState::Supported,
            ),
            record(
                "e3",
                "replication-1",
                EvidenceSourceKind::Replication,
                EvidenceState::Supported,
            ),
        ];
        let first = triangulate_glioma_evidence(&request(), &records).unwrap();
        let mut permuted = records.clone();
        permuted.reverse();
        let second = triangulate_glioma_evidence(&request(), &permuted).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            EvidenceTriangulationDisposition::Qualified
        );
        assert_eq!(first.qualified_order.len(), 1);
        assert_eq!(first.claims[0].independent_artifact_count, 3);
        first.validate().unwrap();
    }

    #[test]
    fn contradiction_and_negative_records_prevent_false_qualification() {
        let records = vec![
            record(
                "e1",
                "literature-1",
                EvidenceSourceKind::Literature,
                EvidenceState::Supported,
            ),
            record(
                "e2",
                "assay-1",
                EvidenceSourceKind::Assay,
                EvidenceState::Supported,
            ),
            record(
                "e3",
                "replication-1",
                EvidenceSourceKind::Replication,
                EvidenceState::Supported,
            ),
            record(
                "e4",
                "dataset-1",
                EvidenceSourceKind::Dataset,
                EvidenceState::Contradicted,
            ),
            record(
                "e5",
                "model-1",
                EvidenceSourceKind::Model,
                EvidenceState::Unknown,
            ),
        ];
        let output = triangulate_glioma_evidence(&request(), &records).unwrap();
        assert_eq!(
            output.disposition,
            EvidenceTriangulationDisposition::Partial
        );
        assert!(output.partial_order.len() == 1);
        assert!(output
            .next_action_order
            .iter()
            .any(|action| action.starts_with("resolve-contradiction:")));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.starts_with("incomplete-claim:")));
    }

    #[test]
    fn insufficient_independence_stays_unresolved() {
        let records = vec![record(
            "e1",
            "literature-1",
            EvidenceSourceKind::Literature,
            EvidenceState::Supported,
        )];
        let output = triangulate_glioma_evidence(&request(), &records).unwrap();
        assert_eq!(
            output.disposition,
            EvidenceTriangulationDisposition::Unresolved
        );
        assert_eq!(output.unresolved_order.len(), 1);
        assert!(output
            .next_action_order
            .iter()
            .any(|action| action.starts_with("add-independent-source:")));
    }

    #[test]
    fn source_dominance_is_partial_and_requests_replication() {
        let mut records = vec![
            record(
                "e1",
                "dominant-source",
                EvidenceSourceKind::Literature,
                EvidenceState::Supported,
            ),
            record(
                "e2",
                "independent-assay",
                EvidenceSourceKind::Assay,
                EvidenceState::Supported,
            ),
            record(
                "e3",
                "independent-replication",
                EvidenceSourceKind::Replication,
                EvidenceState::Supported,
            ),
        ];
        records[0].quality_milli = 1_000;
        records[0].reproducibility_milli = 1_000;
        records[1].quality_milli = 600;
        records[1].reproducibility_milli = 600;
        records[2].quality_milli = 600;
        records[2].reproducibility_milli = 600;
        let output = triangulate_glioma_evidence(&request(), &records).unwrap();
        assert_eq!(
            output.disposition,
            EvidenceTriangulationDisposition::Partial
        );
        assert_eq!(output.claims[0].verdict, TriangulatedClaimVerdict::Partial);
        assert!(output
            .next_action_order
            .iter()
            .any(|action| action.starts_with("replicate-independent-source:")));
        assert!(output.claims[0]
            .rationale_order
            .iter()
            .any(|item| item == "source-dominance-gate-failed"));
    }
}
