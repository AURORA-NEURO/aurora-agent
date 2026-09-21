//! Evidence-cluster indexing for local preclinical glioma surveillance.
//!
//! A literature or assay record is not an independent observation merely because it has a new
//! identifier. This feature groups records by claim scope and modality, collapses exact artifact
//! copies, scores independent source families, and preserves positive, negative, contradictory,
//! and unresolved states for downstream knowledge compilation. It does not fetch sources or
//! infer a biological conclusion.

use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceClusterIndex1@1";
pub const MAX_RECORDS: usize = 16_384;
pub const MAX_CLUSTERS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClusterRequest {
    pub objective: String,
    pub records: Vec<EvidenceRecord>,
    pub max_clusters: usize,
    pub min_quality_milli: u16,
    pub min_independent_artifacts: usize,
    pub min_support_milli: u16,
    pub min_negative_milli: u16,
    pub preserve_exact_duplicates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClusterMember {
    pub evidence_id: String,
    pub artifact_digest: ContentHash,
    pub source_kind: EvidenceSourceKind,
    pub state: EvidenceState,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClusterVerdict {
    IndependentSupport,
    Negative,
    Contradicted,
    DuplicateOnly,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCluster {
    pub cluster_id: String,
    pub claim: String,
    pub scope: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub member_order: Vec<String>,
    pub independent_artifact_order: Vec<ContentHash>,
    pub source_kind_order: Vec<EvidenceSourceKind>,
    pub members: Vec<EvidenceClusterMember>,
    pub duplicate_artifact_count: usize,
    pub support_score_milli: u16,
    pub negative_score_milli: u16,
    pub contradiction_score_milli: u16,
    pub unknown_score_milli: u16,
    pub independence_milli: u16,
    pub verdict: EvidenceClusterVerdict,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClusterDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClusterIndex {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub cluster_order: Vec<String>,
    pub clusters: Vec<EvidenceCluster>,
    pub qualified_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub duplicate_evidence_order: Vec<String>,
    pub omitted_cluster_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceClusterDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceClusterError {
    #[error("evidence cluster request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence cluster record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence cluster output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence cluster digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn evidence_weight(record: &EvidenceRecord) -> u16 {
    let quality = u32::from(record.quality_milli);
    let relevance = u32::from(record.relevance_milli);
    let reproducibility = u32::from(record.reproducibility_milli);
    ((quality * relevance * reproducibility) / 1_000_000).min(1_000) as u16
}

fn cluster_key(record: &EvidenceRecord) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{:?}\u{1f}{:?}",
        record.claim, record.scope, record.modality, record.model_system
    )
}

fn cluster_id(key: &str) -> String {
    format!("cluster:{}", ContentHash::of_bytes(key.as_bytes()))
}

fn digest_input(output: &EvidenceClusterIndex) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "cluster_order": output.cluster_order,
        "clusters": output.clusters,
        "qualified_order": output.qualified_order,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "unresolved_order": output.unresolved_order,
        "duplicate_evidence_order": output.duplicate_evidence_order,
        "omitted_cluster_order": output.omitted_cluster_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl EvidenceClusterIndex {
    pub fn validate(&self) -> Result<(), EvidenceClusterError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.cluster_order)
            || !canonical(&self.qualified_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.duplicate_evidence_order)
            || !canonical(&self.omitted_cluster_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.clusters.iter().any(|cluster| {
                cluster.cluster_id.trim().is_empty()
                    || cluster.claim.trim().is_empty()
                    || cluster.scope.trim().is_empty()
                    || !canonical(&cluster.member_order)
                    || !canonical(&cluster.independent_artifact_order)
                    || !canonical(&cluster.source_kind_order)
                    || cluster.members.iter().any(|member| {
                        member.evidence_id.trim().is_empty() || member.weight_milli > 1_000
                    })
                    || cluster.member_order
                        != cluster
                            .members
                            .iter()
                            .map(|member| member.evidence_id.clone())
                            .collect::<Vec<_>>()
                    || cluster.support_score_milli > 1_000
                    || cluster.negative_score_milli > 1_000
                    || cluster.contradiction_score_milli > 1_000
                    || cluster.unknown_score_milli > 1_000
                    || cluster.independence_milli > 1_000
                    || cluster.next_action.trim().is_empty()
            })
        {
            return Err(EvidenceClusterError::InvalidOutput(
                "identity, ordering, score, member, or rationale fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceClusterError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceClusterError::InvalidOutput(
                "evidence cluster digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn cluster_glioma_evidence(
    request: &EvidenceClusterRequest,
) -> Result<EvidenceClusterIndex, EvidenceClusterError> {
    if request.objective.trim().is_empty()
        || request.records.is_empty()
        || request.records.len() > MAX_RECORDS
        || request.max_clusters == 0
        || request.max_clusters > MAX_CLUSTERS
        || request.min_quality_milli > 1_000
        || request.min_support_milli > 1_000
        || request.min_negative_milli > 1_000
    {
        return Err(EvidenceClusterError::InvalidRequest(
            "objective, records, cluster bound, and score thresholds are invalid".into(),
        ));
    }
    let mut record_ids = BTreeSet::new();
    let mut groups = BTreeMap::<String, Vec<&EvidenceRecord>>::new();
    for record in &request.records {
        record
            .source_artifact
            .validate()
            .map_err(|error| EvidenceClusterError::InvalidRecord(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !record_ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceClusterError::InvalidRecord(
                "evidence ids, claims, scopes, and score bounds must be unique and valid".into(),
            ));
        }
        if record.quality_milli >= request.min_quality_milli || request.preserve_exact_duplicates {
            let key = cluster_key(record);
            groups.entry(key).or_default().push(record);
        }
    }
    if groups.is_empty() {
        return Err(EvidenceClusterError::InvalidRequest(
            "no records clear the requested quality floor".into(),
        ));
    }

    let mut built = groups
        .into_iter()
        .map(|(key, mut records)| {
            records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
            let first = records[0];
            let cluster_id = cluster_id(&key);
            let members = records
                .iter()
                .map(|record| EvidenceClusterMember {
                    evidence_id: record.evidence_id.clone(),
                    artifact_digest: record.source_artifact.content_hash.clone(),
                    source_kind: record.source_kind,
                    state: record.state,
                    weight_milli: evidence_weight(record),
                })
                .collect::<Vec<_>>();
            let artifacts = members
                .iter()
                .map(|member| member.artifact_digest.clone())
                .collect::<BTreeSet<_>>();
            let source_kinds = members
                .iter()
                .map(|member| member.source_kind)
                .collect::<BTreeSet<_>>();
            let support_score = members
                .iter()
                .filter(|member| member.state == EvidenceState::Supported)
                .map(|member| u32::from(member.weight_milli))
                .sum::<u32>()
                .min(1_000) as u16;
            let negative_score = members
                .iter()
                .filter(|member| member.state == EvidenceState::Negative)
                .map(|member| u32::from(member.weight_milli))
                .sum::<u32>()
                .min(1_000) as u16;
            let contradiction_score = members
                .iter()
                .filter(|member| member.state == EvidenceState::Contradicted)
                .map(|member| u32::from(member.weight_milli))
                .sum::<u32>()
                .min(1_000) as u16;
            let unknown_score = members
                .iter()
                .filter(|member| {
                    matches!(
                        member.state,
                        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
                    )
                })
                .map(|member| u32::from(member.weight_milli))
                .sum::<u32>()
                .min(1_000) as u16;
            let independence_milli = ((artifacts.len().min(4) * 250)
                .max(source_kinds.len().min(4) * 250))
            .min(1_000) as u16;
            let duplicate_artifact_count = members.len().saturating_sub(artifacts.len());
            let verdict = if support_score >= request.min_support_milli
                && artifacts.len() >= request.min_independent_artifacts
                && contradiction_score < support_score
            {
                EvidenceClusterVerdict::IndependentSupport
            } else if contradiction_score >= request.min_negative_milli
                && contradiction_score > support_score
                && contradiction_score >= negative_score
            {
                EvidenceClusterVerdict::Contradicted
            } else if negative_score >= request.min_negative_milli && negative_score > support_score
            {
                EvidenceClusterVerdict::Negative
            } else if artifacts.len() < request.min_independent_artifacts
                && duplicate_artifact_count > 0
            {
                EvidenceClusterVerdict::DuplicateOnly
            } else {
                EvidenceClusterVerdict::Unresolved
            };
            let next_action = match verdict {
                EvidenceClusterVerdict::IndependentSupport => {
                    "compile_into_typed_knowledge_with_source_independence"
                }
                EvidenceClusterVerdict::Negative => "preserve_negative_result_and_test_boundary",
                EvidenceClusterVerdict::Contradicted => {
                    "route_to_claim_adjudication_and_replication"
                }
                EvidenceClusterVerdict::DuplicateOnly => "seek_an_independent_source_or_artifact",
                EvidenceClusterVerdict::Unresolved => "acquire_or_calibrate_missing_evidence",
            };
            EvidenceCluster {
                cluster_id,
                claim: first.claim.clone(),
                scope: first.scope.clone(),
                modality: first.modality,
                model_system: first.model_system,
                member_order: members
                    .iter()
                    .map(|member| member.evidence_id.clone())
                    .collect(),
                independent_artifact_order: artifacts.iter().cloned().collect(),
                source_kind_order: source_kinds.iter().copied().collect(),
                members,
                duplicate_artifact_count,
                support_score_milli: support_score,
                negative_score_milli: negative_score,
                contradiction_score_milli: contradiction_score,
                unknown_score_milli: unknown_score,
                independence_milli,
                verdict,
                next_action: next_action.into(),
            }
        })
        .collect::<Vec<_>>();
    built.sort_by(|left, right| {
        right
            .support_score_milli
            .cmp(&left.support_score_milli)
            .then_with(|| left.cluster_id.cmp(&right.cluster_id))
    });
    let omitted_cluster_order = built
        .iter()
        .skip(request.max_clusters)
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    built.truncate(request.max_clusters);
    built.sort_by(|left, right| left.cluster_id.cmp(&right.cluster_id));
    let cluster_order = built
        .iter()
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let qualified_order = built
        .iter()
        .filter(|cluster| cluster.verdict == EvidenceClusterVerdict::IndependentSupport)
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let negative_order = built
        .iter()
        .filter(|cluster| cluster.verdict == EvidenceClusterVerdict::Negative)
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let contradicted_order = built
        .iter()
        .filter(|cluster| cluster.verdict == EvidenceClusterVerdict::Contradicted)
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let unresolved_order = built
        .iter()
        .filter(|cluster| {
            matches!(
                cluster.verdict,
                EvidenceClusterVerdict::DuplicateOnly | EvidenceClusterVerdict::Unresolved
            )
        })
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let duplicate_evidence_order = built
        .iter()
        .flat_map(|cluster| cluster.members.iter().map(move |member| (cluster, member)))
        .filter(|(cluster, member)| {
            cluster
                .members
                .iter()
                .filter(|candidate| candidate.artifact_digest == member.artifact_digest)
                .count()
                > 1
        })
        .map(|(_, member)| member.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let negative_evidence = built
        .iter()
        .filter(|cluster| cluster.verdict == EvidenceClusterVerdict::Negative)
        .map(|cluster| cluster.cluster_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = omitted_cluster_order
        .iter()
        .map(|cluster| format!("{cluster}: cluster omitted by max_clusters bound"))
        .collect::<Vec<_>>();
    uncertainty.extend(
        built
            .iter()
            .filter(|cluster| cluster.verdict == EvidenceClusterVerdict::Unresolved)
            .map(|cluster| format!("{}: unresolved support or independence", cluster.cluster_id)),
    );
    uncertainty.sort();
    let disposition = if !qualified_order.is_empty() && contradicted_order.is_empty() {
        EvidenceClusterDisposition::Qualified
    } else if !negative_order.is_empty() && qualified_order.is_empty() {
        EvidenceClusterDisposition::Negative
    } else if !cluster_order.is_empty() {
        EvidenceClusterDisposition::Partial
    } else {
        EvidenceClusterDisposition::Unresolved
    };
    let mut output = EvidenceClusterIndex {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        cluster_order,
        clusters: built,
        qualified_order,
        negative_order,
        contradicted_order,
        unresolved_order,
        duplicate_evidence_order,
        omitted_cluster_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| EvidenceClusterError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceClusterError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn record(
        evidence_id: &str,
        artifact: &str,
        state: EvidenceState,
        source_kind: EvidenceSourceKind,
    ) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: evidence_id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{artifact}"),
                content_hash: ContentHash::of_bytes(artifact.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind,
            claim: "egfr drives invasion".into(),
            scope: "organoid preclinical".into(),
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 950,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request(records: Vec<EvidenceRecord>) -> EvidenceClusterRequest {
        EvidenceClusterRequest {
            objective: "cluster glioma evidence".into(),
            records,
            max_clusters: 32,
            min_quality_milli: 700,
            min_independent_artifacts: 2,
            min_support_milli: 700,
            min_negative_milli: 600,
            preserve_exact_duplicates: true,
        }
    }

    #[test]
    fn independent_artifacts_become_a_supported_cluster() {
        let output = cluster_glioma_evidence(&request(vec![
            record(
                "e1",
                "a",
                EvidenceState::Supported,
                EvidenceSourceKind::Literature,
            ),
            record(
                "e2",
                "b",
                EvidenceState::Supported,
                EvidenceSourceKind::Assay,
            ),
        ]))
        .expect("cluster");
        assert_eq!(output.disposition, EvidenceClusterDisposition::Qualified);
        assert_eq!(output.qualified_order.len(), 1);
        output.validate().expect("digest validates");
    }

    #[test]
    fn exact_artifact_copies_are_not_counted_as_independent_support() {
        let output = cluster_glioma_evidence(&request(vec![
            record(
                "e1",
                "same",
                EvidenceState::Supported,
                EvidenceSourceKind::Literature,
            ),
            record(
                "e2",
                "same",
                EvidenceState::Supported,
                EvidenceSourceKind::Literature,
            ),
        ]))
        .expect("cluster");
        assert_eq!(
            output.clusters[0].verdict,
            EvidenceClusterVerdict::DuplicateOnly
        );
        assert_eq!(output.duplicate_evidence_order, vec!["e1", "e2"]);
    }

    #[test]
    fn negative_and_contradictory_states_remain_visible() {
        let output = cluster_glioma_evidence(&request(vec![
            record(
                "e1",
                "negative",
                EvidenceState::Negative,
                EvidenceSourceKind::Assay,
            ),
            record(
                "e2",
                "contradiction",
                EvidenceState::Contradicted,
                EvidenceSourceKind::Model,
            ),
        ]))
        .expect("cluster");
        assert!(output
            .clusters
            .iter()
            .any(|cluster| cluster.verdict == EvidenceClusterVerdict::Contradicted));
        assert!(output.clusters.iter().any(|cluster| {
            cluster
                .members
                .iter()
                .any(|member| member.state == EvidenceState::Negative)
        }));
    }
}
