//! Typed evidence-closure analysis for autonomous preclinical glioma knowledge.
//!
//! A claim graph is not action-ready merely because it has a confidence score. This capability
//! binds each compiled claim to the local evidence records that actually support, contradict,
//! negate, or leave it unresolved, then measures independent artifact, modality, and model
//! coverage. Missing closure is an explicit state; it is never converted into zero evidence or a
//! confident downstream action.

use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeClosure1@1";
pub const MAX_CLAIMS: usize = 16_384;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClosureRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub records: Vec<EvidenceRecord>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_independent_artifacts: usize,
    pub min_support_milli: u16,
    pub max_claims: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeClosureClaimDisposition {
    Closed,
    Partial,
    Unresolved,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClaimClosure {
    pub claim_id: String,
    pub evidence_order: Vec<String>,
    pub source_kind_order: Vec<EvidenceSourceKind>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub independent_artifact_count: usize,
    pub support_milli: u16,
    pub closure_milli: u16,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub omission_order: Vec<String>,
    pub disposition: KnowledgeClosureClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeClosureDisposition {
    Qualified,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClosure {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub closed_claim_order: Vec<String>,
    pub partial_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub negative_claim_order: Vec<String>,
    pub claims: Vec<KnowledgeClaimClosure>,
    pub orphan_evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeClosureDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeClosureError {
    #[error("knowledge closure request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge closure evidence record is invalid: {0}")]
    InvalidRecord(String),
    #[error("knowledge closure output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge closure digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &KnowledgeClosure) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "closed_claim_order": output.closed_claim_order,
        "partial_claim_order": output.partial_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "negative_claim_order": output.negative_claim_order,
        "claims": output.claims,
        "orphan_evidence_order": output.orphan_evidence_order,
        "omission_order": output.omission_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl KnowledgeClosure {
    pub fn validate(&self) -> Result<(), KnowledgeClosureError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.closed_claim_order)
            || !canonical(&self.partial_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.negative_claim_order)
            || !canonical(&self.orphan_evidence_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.claims.len() != self.claim_order.len()
            || self
                .claims
                .iter()
                .map(|claim| claim.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.claims.iter().any(|claim| {
                claim.claim_id.trim().is_empty()
                    || !canonical(&claim.evidence_order)
                    || !canonical(&claim.source_kind_order)
                    || !canonical(&claim.modality_order)
                    || !canonical(&claim.model_system_order)
                    || !canonical(&claim.missing_modality_order)
                    || !canonical(&claim.missing_model_system_order)
                    || !canonical(&claim.omission_order)
                    || claim.support_milli > 1_000
                    || claim.closure_milli > 1_000
            })
        {
            return Err(KnowledgeClosureError::InvalidOutput(
                "identity, claim partitions, ordering, or score bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeClosureError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeClosureError::InvalidOutput(
                "knowledge closure digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn support_score(records: &[&EvidenceRecord]) -> u16 {
    let positive = records
        .iter()
        .filter(|record| matches!(record.state, EvidenceState::Supported))
        .collect::<Vec<_>>();
    if positive.is_empty() {
        return 0;
    }
    (positive
        .iter()
        .map(|record| {
            (45 * u64::from(record.quality_milli)
                + 35 * u64::from(record.relevance_milli)
                + 20 * u64::from(record.reproducibility_milli))
                / 100
        })
        .sum::<u64>()
        / positive.len() as u64) as u16
}

fn validate_request(request: &KnowledgeClosureRequest) -> Result<(), KnowledgeClosureError> {
    if request.objective.trim().is_empty()
        || request.knowledge.objective != request.objective
        || request.min_independent_artifacts == 0
        || request.min_support_milli > 1_000
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.records.len() > MAX_RECORDS
    {
        return Err(KnowledgeClosureError::InvalidRequest(
            "objective binding, independent-artifact floor, support floor, claim bound, or record bound is invalid".into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| KnowledgeClosureError::InvalidRequest(error.to_string()))?;
    if request.knowledge.claims.len() > request.max_claims {
        return Err(KnowledgeClosureError::InvalidRequest(
            "typed knowledge exceeds the requested claim capacity".into(),
        ));
    }
    Ok(())
}

pub fn compile_glioma_knowledge_closure(
    request: &KnowledgeClosureRequest,
) -> Result<KnowledgeClosure, KnowledgeClosureError> {
    validate_request(request)?;
    let mut records_by_id = BTreeMap::<String, &EvidenceRecord>::new();
    for record in &request.records {
        record
            .source_artifact
            .validate()
            .map_err(|error| KnowledgeClosureError::InvalidRecord(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || records_by_id
                .insert(record.evidence_id.clone(), record)
                .is_some()
        {
            return Err(KnowledgeClosureError::InvalidRecord(
                "evidence ids, claim scope, scores, or uniqueness are invalid".into(),
            ));
        }
    }
    let mut referenced = BTreeSet::new();
    let mut claims = Vec::new();
    let mut closed_claim_order = Vec::new();
    let mut partial_claim_order = Vec::new();
    let mut unresolved_claim_order = Vec::new();
    let mut negative_claim_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for claim in &request.knowledge.claims {
        let evidence_ids = claim
            .supporting_evidence_order
            .iter()
            .chain(claim.negative_evidence_order.iter())
            .chain(claim.contradictory_evidence_order.iter())
            .chain(claim.unresolved_evidence_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let evidence_order = evidence_ids.iter().cloned().collect::<Vec<_>>();
        referenced.extend(evidence_ids.iter().cloned());
        let found = evidence_ids
            .iter()
            .filter_map(|evidence_id| records_by_id.get(evidence_id).copied())
            .collect::<Vec<_>>();
        let missing = evidence_ids
            .iter()
            .filter(|evidence_id| !records_by_id.contains_key(*evidence_id))
            .map(|evidence_id| {
                format!(
                    "{claim_id}: missing evidence {evidence_id}",
                    claim_id = claim.claim_id
                )
            })
            .collect::<Vec<_>>();
        let source_kind_order = found
            .iter()
            .map(|record| record.source_kind)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let modality_order = found
            .iter()
            .map(|record| record.modality)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let model_system_order = found
            .iter()
            .filter_map(|record| record.model_system)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let independent_artifact_count = found
            .iter()
            .map(|record| record.source_artifact.artifact_id.clone())
            .collect::<BTreeSet<_>>()
            .len();
        let support_milli = support_score(&found);
        let observed_modalities = modality_order.iter().copied().collect::<BTreeSet<_>>();
        let observed_models = model_system_order.iter().copied().collect::<BTreeSet<_>>();
        let missing_modality_order = request
            .required_modalities
            .difference(&observed_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_model_system_order = request
            .required_model_systems
            .difference(&observed_models)
            .copied()
            .collect::<Vec<_>>();
        let missing_count = missing.len();
        let mut claim_omissions = missing;
        if independent_artifact_count < request.min_independent_artifacts {
            claim_omissions.push(format!(
                "{}: independent artifact floor is unmet",
                claim.claim_id
            ));
        }
        if support_milli < request.min_support_milli {
            claim_omissions.push(format!("{}: support floor is unmet", claim.claim_id));
        }
        claim_omissions.extend(
            missing_modality_order
                .iter()
                .map(|modality| format!("{}: missing modality {modality:?}", claim.claim_id)),
        );
        claim_omissions.extend(
            missing_model_system_order
                .iter()
                .map(|model| format!("{}: missing model {model:?}", claim.claim_id)),
        );
        claim_omissions.sort();
        claim_omissions.dedup();
        let disposition = if matches!(claim.disposition, KnowledgeClaimDisposition::Negative) {
            KnowledgeClosureClaimDisposition::Negative
        } else if matches!(claim.disposition, KnowledgeClaimDisposition::Unresolved)
            || missing_count > 0
        {
            KnowledgeClosureClaimDisposition::Unresolved
        } else if claim_omissions.is_empty()
            && matches!(claim.disposition, KnowledgeClaimDisposition::Supported)
        {
            KnowledgeClosureClaimDisposition::Closed
        } else {
            KnowledgeClosureClaimDisposition::Partial
        };
        let closure_milli = if claim_omissions.is_empty() {
            support_milli
        } else {
            support_milli.min(500)
        };
        match disposition {
            KnowledgeClosureClaimDisposition::Closed => {
                closed_claim_order.push(claim.claim_id.clone())
            }
            KnowledgeClosureClaimDisposition::Partial => {
                partial_claim_order.push(claim.claim_id.clone())
            }
            KnowledgeClosureClaimDisposition::Unresolved => {
                unresolved_claim_order.push(claim.claim_id.clone())
            }
            KnowledgeClosureClaimDisposition::Negative => {
                negative_claim_order.push(claim.claim_id.clone())
            }
        }
        omission_order.extend(claim_omissions.iter().cloned());
        if matches!(disposition, KnowledgeClosureClaimDisposition::Negative) {
            negative_evidence.push(format!("{}: claim is explicitly negative", claim.claim_id));
        }
        if !claim_omissions.is_empty() {
            uncertainty.push(format!(
                "{}: protected evidence closure is incomplete",
                claim.claim_id
            ));
        }
        claims.push(KnowledgeClaimClosure {
            claim_id: claim.claim_id.clone(),
            evidence_order,
            source_kind_order,
            modality_order,
            model_system_order,
            independent_artifact_count,
            support_milli,
            closure_milli,
            missing_modality_order,
            missing_model_system_order,
            omission_order: claim_omissions,
            disposition,
        });
    }
    let orphan_evidence_order = records_by_id
        .keys()
        .filter(|evidence_id| !referenced.contains(*evidence_id))
        .cloned()
        .collect::<Vec<_>>();
    if !orphan_evidence_order.is_empty() {
        uncertainty.push(
            "one or more supplied evidence records are not referenced by typed claims".into(),
        );
    }
    closed_claim_order.sort();
    partial_claim_order.sort();
    unresolved_claim_order.sort();
    negative_claim_order.sort();
    omission_order.sort();
    omission_order.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let claim_order = request.knowledge.claim_order.clone();
    let disposition = if claim_order.is_empty() {
        KnowledgeClosureDisposition::Blocked
    } else if !closed_claim_order.is_empty()
        && partial_claim_order.is_empty()
        && unresolved_claim_order.is_empty()
    {
        KnowledgeClosureDisposition::Qualified
    } else if !unresolved_claim_order.is_empty() {
        KnowledgeClosureDisposition::Unresolved
    } else {
        KnowledgeClosureDisposition::Partial
    };
    let next_step = match disposition {
        KnowledgeClosureDisposition::Qualified => {
            "release the closed claims to consistency closure or bounded downstream planning"
        }
        KnowledgeClosureDisposition::Partial => {
            "acquire missing modality, model, support, or independent-artifact coverage before promotion"
        }
        KnowledgeClosureDisposition::Unresolved => {
            "resolve missing evidence references and protected closure before autonomous execution"
        }
        KnowledgeClosureDisposition::Blocked => {
            "compile a non-empty typed knowledge snapshot before evaluating closure"
        }
    };
    let mut output = KnowledgeClosure {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: request.knowledge.digest.clone(),
        claim_order,
        closed_claim_order,
        partial_claim_order,
        unresolved_claim_order,
        negative_claim_order,
        claims,
        orphan_evidence_order,
        omission_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| KnowledgeClosureError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeClosureError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::LocalArtifactRef;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "aggregate-evidence".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn record(id: &str, artifact_id: &str) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(artifact_id),
            source_kind: EvidenceSourceKind::Assay,
            claim: "egfr activation increases invasion".into(),
            scope: "organoid".into(),
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn knowledge() -> TypedKnowledge {
        super::super::knowledge_graph::compile_typed_knowledge(
            &super::super::knowledge_graph::KnowledgeRequest {
                objective: "knowledge closure".into(),
                required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 500,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[
                record("evidence-a", "artifact-a"),
                record("evidence-b", "artifact-b"),
            ],
        )
        .unwrap()
    }

    fn request(knowledge: TypedKnowledge, records: Vec<EvidenceRecord>) -> KnowledgeClosureRequest {
        KnowledgeClosureRequest {
            objective: "knowledge closure".into(),
            knowledge,
            records,
            required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_independent_artifacts: 2,
            min_support_milli: 500,
            max_claims: 8,
        }
    }

    #[test]
    fn closes_a_supported_claim_with_independent_artifacts() {
        let knowledge = knowledge();
        let output = compile_glioma_knowledge_closure(&request(
            knowledge,
            vec![
                record("evidence-a", "artifact-a"),
                record("evidence-b", "artifact-b"),
            ],
        ))
        .expect("closure");
        assert_eq!(output.disposition, KnowledgeClosureDisposition::Qualified);
        assert_eq!(output.closed_claim_order.len(), 1);
        output.validate().expect("digest validates");
    }

    #[test]
    fn missing_referenced_evidence_is_unresolved_not_zero_support() {
        let knowledge = knowledge();
        let output = compile_glioma_knowledge_closure(&request(
            knowledge,
            vec![record("evidence-a", "artifact-a")],
        ))
        .expect("closure");
        assert_eq!(output.disposition, KnowledgeClosureDisposition::Unresolved);
        assert!(!output.omission_order.is_empty());
        assert!(!output.uncertainty.is_empty());
    }
}
