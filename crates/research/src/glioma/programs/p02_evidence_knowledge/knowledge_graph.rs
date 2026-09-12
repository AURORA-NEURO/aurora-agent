//! Deterministic evidence-to-typed-knowledge compilation for preclinical glioma research.
//!
//! This feature turns caller-supplied, local evidence records into a scoped claim graph that an
//! autonomous research workflow can consume.  Identical claims are coalesced only within their
//! declared scope; support, negative, contradictory, and unresolved records remain separately
//! addressable.  Coverage floors are evaluated on supporting records, so a claim cannot become
//! qualified merely because an unsupported modality or model happens to be present.  The
//! compiler never infers causality, fetches literature, moves raw data, or crosses the clinical
//! boundary.

use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaTypedKnowledge1@1";
pub const MAX_RECORDS: usize = 100_000;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRequest {
    pub objective: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_support_milli: u16,
    pub min_sources_per_claim: usize,
    pub max_claims: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeClaimDisposition {
    Supported,
    Contested,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClaim {
    pub claim_id: String,
    pub statement: String,
    pub scope: String,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub supporting_evidence_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradictory_evidence_order: Vec<String>,
    pub unresolved_evidence_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub confidence_milli: u16,
    pub disposition: KnowledgeClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claims: Vec<KnowledgeClaim>,
    pub claim_order: Vec<String>,
    pub top_claim_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: KnowledgeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeError {
    #[error("knowledge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge evidence record is invalid: {0}")]
    InvalidRecord(String),
    #[error("knowledge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct ClaimAccumulator {
    claim_id: String,
    statement: String,
    scope: String,
    modality_order: BTreeSet<GliomaModality>,
    model_system_order: BTreeSet<GliomaModelSystem>,
    supporting_modalities: BTreeSet<GliomaModality>,
    supporting_models: BTreeSet<GliomaModelSystem>,
    supporting: BTreeSet<String>,
    negative: BTreeSet<String>,
    contradictory: BTreeSet<String>,
    unresolved: BTreeSet<String>,
    support_sum: u64,
    support_count: usize,
    contradiction_sum: u64,
    contradiction_count: usize,
}

fn canonical_claim(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn claim_id(canonical: &str, scope: &str) -> Result<String, KnowledgeError> {
    let hash = ContentHash::of_value(&serde_json::json!({
        "claim": canonical,
        "scope": scope,
    }))
    .map_err(|error| KnowledgeError::Digest(error.to_string()))?;
    Ok(format!("claim-{hash}"))
}

fn evidence_weight(record: &EvidenceRecord) -> u64 {
    // A fixed-point quality/relevance/reproducibility score.  The denominator is 1_000, so all
    // downstream values stay in the public 0..=1_000 range without floating point drift.
    (45 * record.quality_milli as u64
        + 35 * record.relevance_milli as u64
        + 20 * record.reproducibility_milli as u64)
        / 100
}

fn digest_input(output: &TypedKnowledge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claims": output.claims,
        "claim_order": output.claim_order,
        "top_claim_order": output.top_claim_order,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

fn ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl TypedKnowledge {
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.claims.len() != self.claim_order.len()
            || self
                .claims
                .iter()
                .map(|claim| claim.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || !ordered_unique(&self.claim_order)
            || !ordered_unique(&self.top_claim_order)
            || !ordered_unique(&self.omission_order)
            || !ordered_unique(&self.negative_evidence_order)
            || !ordered_unique(&self.uncertainty_order)
            || self.claims.iter().any(|claim| {
                claim.claim_id.trim().is_empty()
                    || claim.statement.trim().is_empty()
                    || claim.scope.trim().is_empty()
                    || !ordered_unique(&claim.modality_order)
                    || !ordered_unique(&claim.model_system_order)
                    || !ordered_unique(&claim.supporting_evidence_order)
                    || !ordered_unique(&claim.negative_evidence_order)
                    || !ordered_unique(&claim.contradictory_evidence_order)
                    || !ordered_unique(&claim.unresolved_evidence_order)
                    || !ordered_unique(&claim.missing_modality_order)
                    || !ordered_unique(&claim.missing_model_system_order)
                    || claim.support_milli > 1_000
                    || claim.contradiction_milli > 1_000
                    || claim.confidence_milli > 1_000
            })
            || !self
                .top_claim_order
                .iter()
                .all(|claim_id| self.claim_order.binary_search(claim_id).is_ok())
        {
            return Err(KnowledgeError::InvalidOutput(
                "identity, claim partition, ordering, coverage, or score bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeError::InvalidOutput(
                "digest is not bound to typed knowledge".into(),
            ));
        }
        Ok(())
    }
}

pub fn compile_typed_knowledge(
    request: &KnowledgeRequest,
    records: &[EvidenceRecord],
) -> Result<TypedKnowledge, KnowledgeError> {
    if request.objective.trim().is_empty()
        || request.min_support_milli > 1_000
        || request.min_sources_per_claim == 0
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || records.len() > MAX_RECORDS
    {
        return Err(KnowledgeError::InvalidRequest(
            "objective, support threshold, source floor, or record bound is invalid".into(),
        ));
    }

    let mut record_ids = BTreeSet::new();
    let mut groups: BTreeMap<String, ClaimAccumulator> = BTreeMap::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| KnowledgeError::InvalidRecord(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !record_ids.insert(record.evidence_id.clone())
        {
            return Err(KnowledgeError::InvalidRecord(
                "evidence ids, claim scope, scores, or uniqueness are invalid".into(),
            ));
        }
        let canonical = canonical_claim(&record.claim);
        let scope = record
            .scope
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let key = format!("{canonical}\u{1f}{scope}");
        let id = claim_id(&canonical, &scope)?;
        let accumulator = groups.entry(key).or_insert_with(|| ClaimAccumulator {
            claim_id: id,
            statement: record.claim.trim().to_string(),
            scope,
            modality_order: BTreeSet::new(),
            model_system_order: BTreeSet::new(),
            supporting_modalities: BTreeSet::new(),
            supporting_models: BTreeSet::new(),
            supporting: BTreeSet::new(),
            negative: BTreeSet::new(),
            contradictory: BTreeSet::new(),
            unresolved: BTreeSet::new(),
            support_sum: 0,
            support_count: 0,
            contradiction_sum: 0,
            contradiction_count: 0,
        });
        if record.claim.trim() < accumulator.statement.as_str() {
            accumulator.statement = record.claim.trim().to_string();
        }
        accumulator.modality_order.insert(record.modality);
        if let Some(model) = record.model_system {
            accumulator.model_system_order.insert(model);
        }
        match record.state {
            EvidenceState::Supported => {
                accumulator.supporting.insert(record.evidence_id.clone());
                accumulator.supporting_modalities.insert(record.modality);
                if let Some(model) = record.model_system {
                    accumulator.supporting_models.insert(model);
                }
                accumulator.support_sum += evidence_weight(record);
                accumulator.support_count += 1;
            }
            EvidenceState::Negative => {
                accumulator.negative.insert(record.evidence_id.clone());
            }
            EvidenceState::Contradicted => {
                accumulator.contradictory.insert(record.evidence_id.clone());
                accumulator.contradiction_sum += evidence_weight(record);
                accumulator.contradiction_count += 1;
            }
            EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Stale => {
                accumulator.unresolved.insert(record.evidence_id.clone());
            }
        }
    }

    let mut accumulators = groups.into_values().collect::<Vec<_>>();
    accumulators.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    if accumulators.len() > request.max_claims {
        accumulators.truncate(request.max_claims);
    }

    let mut claims = Vec::with_capacity(accumulators.len());
    for accumulator in accumulators {
        let support_milli = if accumulator.support_count == 0 {
            0
        } else {
            (accumulator.support_sum / accumulator.support_count as u64).min(1_000) as u16
        };
        let contradiction_milli = if accumulator.contradiction_count == 0 {
            0
        } else {
            (accumulator.contradiction_sum / accumulator.contradiction_count as u64).min(1_000)
                as u16
        };
        let missing_modality_order = request
            .required_modalities
            .difference(&accumulator.supporting_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_model_system_order = request
            .required_model_systems
            .difference(&accumulator.supporting_models)
            .copied()
            .collect::<Vec<_>>();
        let confidence_milli = support_milli.saturating_sub(contradiction_milli);
        let disposition = if accumulator.support_count < request.min_sources_per_claim
            || support_milli < request.min_support_milli
            || !missing_modality_order.is_empty()
            || !missing_model_system_order.is_empty()
            || !accumulator.unresolved.is_empty()
        {
            if accumulator.support_count == 0 && !accumulator.negative.is_empty() {
                KnowledgeClaimDisposition::Negative
            } else {
                KnowledgeClaimDisposition::Unresolved
            }
        } else if !accumulator.contradictory.is_empty() || !accumulator.negative.is_empty() {
            KnowledgeClaimDisposition::Contested
        } else {
            KnowledgeClaimDisposition::Supported
        };
        claims.push(KnowledgeClaim {
            claim_id: accumulator.claim_id,
            statement: accumulator.statement,
            scope: accumulator.scope,
            modality_order: accumulator.modality_order.into_iter().collect(),
            model_system_order: accumulator.model_system_order.into_iter().collect(),
            supporting_evidence_order: accumulator.supporting.into_iter().collect(),
            negative_evidence_order: accumulator.negative.into_iter().collect(),
            contradictory_evidence_order: accumulator.contradictory.into_iter().collect(),
            unresolved_evidence_order: accumulator.unresolved.into_iter().collect(),
            missing_modality_order,
            missing_model_system_order,
            support_milli,
            contradiction_milli,
            confidence_milli,
            disposition,
        });
    }

    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let mut top_claims = claims
        .iter()
        .filter(|claim| claim.disposition == KnowledgeClaimDisposition::Supported)
        .map(|claim| {
            (
                claim.confidence_milli,
                claim.support_milli,
                claim.claim_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    top_claims.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let top_claim_order = top_claims
        .into_iter()
        .map(|(_, _, id)| id)
        .collect::<Vec<_>>();
    let omission_order = claims
        .iter()
        .flat_map(|claim| {
            claim
                .missing_modality_order
                .iter()
                .map(move |modality| format!("{}:missing-modality:{modality:?}", claim.claim_id))
                .chain(
                    claim
                        .missing_model_system_order
                        .iter()
                        .map(move |model| format!("{}:missing-model:{model:?}", claim.claim_id)),
                )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let negative_evidence_order = claims
        .iter()
        .flat_map(|claim| claim.negative_evidence_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let uncertainty_order = claims
        .iter()
        .flat_map(|claim| {
            claim
                .unresolved_evidence_order
                .iter()
                .map(move |evidence_id| format!("{}:unresolved:{evidence_id}", claim.claim_id))
                .chain(
                    claim
                        .contradictory_evidence_order
                        .iter()
                        .map(move |evidence_id| {
                            format!("{}:contradictory:{evidence_id}", claim.claim_id)
                        }),
                )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = if claims.is_empty() || top_claim_order.is_empty() {
        KnowledgeDisposition::Unresolved
    } else if claims
        .iter()
        .all(|claim| claim.disposition == KnowledgeClaimDisposition::Supported)
    {
        KnowledgeDisposition::Qualified
    } else {
        KnowledgeDisposition::Partial
    };
    let mut output = TypedKnowledge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claims,
        claim_order,
        top_claim_order,
        omission_order,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| KnowledgeError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn record(
        id: &str,
        claim: &str,
        state: EvidenceState,
        modality: GliomaModality,
    ) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: claim.into(),
            scope: "preclinical glioma".into(),
            modality,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request() -> KnowledgeRequest {
        KnowledgeRequest {
            objective: "rank glioma invasion mechanisms".into(),
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_support_milli: 700,
            min_sources_per_claim: 1,
            max_claims: 8,
        }
    }

    #[test]
    fn compiles_supported_claims_and_is_replay_stable() {
        let records = vec![record(
            "e1",
            "EGFR signaling increases invasion",
            EvidenceState::Supported,
            GliomaModality::Genomics,
        )];
        let first = compile_typed_knowledge(&request(), &records).unwrap();
        let second = compile_typed_knowledge(&request(), &records).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, KnowledgeDisposition::Qualified);
        assert_eq!(
            first.claims[0].disposition,
            KnowledgeClaimDisposition::Supported
        );
        first.validate().unwrap();
    }

    #[test]
    fn contradiction_and_missing_coverage_remain_contested_or_unresolved() {
        let mut req = request();
        req.required_modalities =
            BTreeSet::from([GliomaModality::Genomics, GliomaModality::Spatial]);
        let output = compile_typed_knowledge(
            &req,
            &[
                record(
                    "e-supported",
                    "EGFR signaling increases invasion",
                    EvidenceState::Supported,
                    GliomaModality::Genomics,
                ),
                record(
                    "e-contradictory",
                    "EGFR signaling increases invasion",
                    EvidenceState::Contradicted,
                    GliomaModality::Genomics,
                ),
                record(
                    "e-contradictory-spatial",
                    "EGFR signaling increases invasion",
                    EvidenceState::Contradicted,
                    GliomaModality::Spatial,
                ),
            ],
        )
        .unwrap();
        assert_eq!(output.disposition, KnowledgeDisposition::Unresolved);
        assert_eq!(
            output.claims[0].disposition,
            KnowledgeClaimDisposition::Unresolved
        );
        assert_eq!(
            output.claims[0].missing_modality_order,
            vec![GliomaModality::Spatial]
        );
        assert_eq!(
            output.claims[0].contradictory_evidence_order,
            vec!["e-contradictory", "e-contradictory-spatial"]
        );
    }

    #[test]
    fn negative_only_claim_is_first_class_negative() {
        let output = compile_typed_knowledge(
            &request(),
            &[record(
                "e-negative",
                "PDGF signaling increases invasion",
                EvidenceState::Negative,
                GliomaModality::Genomics,
            )],
        )
        .unwrap();
        assert_eq!(output.disposition, KnowledgeDisposition::Unresolved);
        assert_eq!(
            output.claims[0].disposition,
            KnowledgeClaimDisposition::Negative
        );
        assert_eq!(output.negative_evidence_order, vec!["e-negative"]);
    }
}
