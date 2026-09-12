//! Signed pathway-activity inference for preclinical glioma research.
//!
//! The analyzer converts local, value-only molecular observations into ranked pathway states
//! that can feed mechanism discrimination and experiment selection.  It does not call a pathway
//! database, infer a diagnosis, or impute missing nodes: every missing feature, modality conflict,
//! and low-confidence bottleneck is retained in the typed result.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaPathwayActivity1@1";
const SCALE: i64 = 1_000;
const MAX_PATHWAYS: usize = 4_096;
const MAX_NODES: usize = 32_768;
const MAX_OBSERVATIONS: usize = 131_072;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub min_pathway_nodes: usize,
    pub min_observed_nodes: usize,
    pub min_modalities: usize,
    pub min_confidence_milli: u16,
    pub max_pathways: usize,
    pub require_cross_modal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityNode {
    pub node_id: String,
    pub label: String,
    pub modality: GliomaModality,
    pub expected_direction: i8,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityEdge {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relation: i8,
    pub confidence_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityDefinition {
    pub pathway_id: String,
    pub label: String,
    pub nodes: Vec<PathwayActivityNode>,
    pub edges: Vec<PathwayActivityEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityObservation {
    pub observation_id: String,
    pub study_id: String,
    pub sample_lineage: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub artifact: LocalArtifactRef,
    pub feature_id: String,
    pub value_milli: i64,
    pub reliability_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathwayActivityDirection {
    Activated,
    Suppressed,
    Neutral,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityRecord {
    pub pathway_id: String,
    pub label: String,
    pub node_order: Vec<String>,
    pub observed_node_order: Vec<String>,
    pub missing_node_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub signed_activity_milli: i64,
    pub coverage_milli: u16,
    pub cross_modal_agreement_milli: u16,
    pub confidence_milli: u16,
    pub bottleneck_order: Vec<String>,
    pub direction: PathwayActivityDirection,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathwayActivityDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathwayActivityAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub pathway_order: Vec<String>,
    pub ranking_order: Vec<String>,
    pub pathways: Vec<PathwayActivityRecord>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: PathwayActivityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathwayActivityError {
    #[error("pathway activity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("pathway activity definition or observation is invalid: {0}")]
    InvalidInput(String),
    #[error("pathway activity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("pathway activity digest failed: {0}")]
    Digest(String),
}

fn ordered<T: Ord>(items: &[T]) -> bool {
    items.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &PathwayActivityAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "pathway_order": output.pathway_order,
        "ranking_order": output.ranking_order,
        "pathways": output.pathways,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl PathwayActivityAnalysis {
    pub fn validate(&self) -> Result<(), PathwayActivityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || !ordered(&self.pathway_order)
            || !ordered(&self.ranking_order)
            || !ordered(&self.negative_evidence)
            || !ordered(&self.uncertainty)
            || self.pathways.iter().any(|pathway| {
                !ordered(&pathway.node_order)
                    || !ordered(&pathway.observed_node_order)
                    || !ordered(&pathway.missing_node_order)
                    || !ordered(&pathway.modality_order)
                    || !ordered(&pathway.bottleneck_order)
                    || pathway.coverage_milli > 1_000
                    || pathway.cross_modal_agreement_milli > 1_000
                    || pathway.confidence_milli > 1_000
            })
        {
            return Err(PathwayActivityError::InvalidOutput(
                "identity, ordering, or bounded activity partitions are invalid".into(),
            ));
        }
        if self.pathways.len() != self.pathway_order.len()
            || self
                .pathways
                .iter()
                .map(|pathway| pathway.pathway_id.clone())
                .collect::<Vec<_>>()
                != self.pathway_order
            || self
                .ranking_order
                .iter()
                .any(|id| !self.pathway_order.contains(id))
        {
            return Err(PathwayActivityError::InvalidOutput(
                "pathway partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| PathwayActivityError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(PathwayActivityError::InvalidOutput(
                "digest is not bound to pathway activity analysis".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &PathwayActivityRequest) -> Result<(), PathwayActivityError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.min_pathway_nodes == 0
        || request.min_observed_nodes == 0
        || request.min_observed_nodes > request.min_pathway_nodes
        || request.min_modalities == 0
        || request.min_confidence_milli > 1_000
        || request.max_pathways == 0
        || request.max_pathways > MAX_PATHWAYS
    {
        return Err(PathwayActivityError::InvalidRequest(
            "objective, study, pathway floors, confidence, or pathway bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_definition(definition: &PathwayActivityDefinition) -> Result<(), PathwayActivityError> {
    if definition.pathway_id.trim().is_empty()
        || definition.label.trim().is_empty()
        || definition.nodes.is_empty()
        || definition.nodes.len() > MAX_NODES
        || definition
            .nodes
            .windows(2)
            .any(|pair| pair[0].node_id >= pair[1].node_id)
        || definition.nodes.iter().any(|node| {
            node.node_id.trim().is_empty()
                || node.label.trim().is_empty()
                || ![-1, 1].contains(&node.expected_direction)
                || node.weight_milli == 0
                || node.weight_milli > 1_000
        })
        || definition.edges.iter().any(|edge| {
            edge.source_node_id >= edge.target_node_id
                || ![-1, 1].contains(&edge.relation)
                || edge.confidence_milli > 1_000
        })
    {
        return Err(PathwayActivityError::InvalidInput(format!(
            "pathway definition {} is not canonical or bounded",
            definition.pathway_id
        )));
    }
    let node_ids = definition
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<BTreeSet<_>>();
    if definition.edges.iter().any(|edge| {
        !node_ids.contains(edge.source_node_id.as_str())
            || !node_ids.contains(edge.target_node_id.as_str())
    }) {
        return Err(PathwayActivityError::InvalidInput(format!(
            "pathway {} contains an edge to an unknown node",
            definition.pathway_id
        )));
    }
    Ok(())
}

fn normalize(value: i64) -> i64 {
    value.clamp(-SCALE, SCALE)
}

/// Infer signed pathway activity from local molecular observations. The result is deliberately a
/// research-priority artifact: it supplies interpretable activity, coverage, confidence, and
/// bottleneck gates for the next mechanism or experiment workflow.
pub fn analyze_glioma_pathway_activity(
    request: &PathwayActivityRequest,
    definitions: &[PathwayActivityDefinition],
    observations: &[PathwayActivityObservation],
) -> Result<PathwayActivityAnalysis, PathwayActivityError> {
    validate_request(request)?;
    if definitions.is_empty()
        || definitions.len() > request.max_pathways
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(PathwayActivityError::InvalidInput(
            "pathway or observation count is outside the bounded analysis budget".into(),
        ));
    }
    let mut pathways = BTreeMap::new();
    let mut total_nodes = 0usize;
    for definition in definitions {
        validate_definition(definition)?;
        total_nodes += definition.nodes.len();
        if pathways
            .insert(definition.pathway_id.clone(), definition)
            .is_some()
        {
            return Err(PathwayActivityError::InvalidInput(
                "pathway identifiers must be unique".into(),
            ));
        }
    }
    if total_nodes > MAX_NODES {
        return Err(PathwayActivityError::InvalidInput(
            "total pathway nodes exceed bounded analysis budget".into(),
        ));
    }
    let mut values: BTreeMap<(GliomaModality, String), (i64, u16)> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.study_id != request.study_id
            || observation.sample_lineage.trim().is_empty()
            || observation.model_system != request.model_system
            || observation.feature_id.trim().is_empty()
            || observation.reliability_milli > 1_000
            || !seen.insert((
                observation.sample_lineage.clone(),
                observation.modality,
                observation.feature_id.clone(),
            ))
        {
            return Err(PathwayActivityError::InvalidInput(format!(
                "observation {} has invalid identity, binding, reliability, or duplicate feature",
                observation.observation_id
            )));
        }
        observation
            .artifact
            .validate()
            .map_err(|e| PathwayActivityError::InvalidInput(e.to_string()))?;
        let key = (observation.modality, observation.feature_id.clone());
        values
            .entry(key)
            .and_modify(|current| {
                if observation.reliability_milli > current.1 {
                    *current = (observation.value_milli, observation.reliability_milli);
                }
            })
            .or_insert((observation.value_milli, observation.reliability_milli));
    }
    let pathway_order = pathways.keys().cloned().collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for pathway_id in &pathway_order {
        let definition = pathways[pathway_id];
        let node_order = definition
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();
        let mut observed = Vec::new();
        let mut missing = Vec::new();
        let mut modalities = BTreeSet::new();
        let mut weighted_sum = 0i128;
        let mut weight_total = 0i128;
        let mut confidence_sum = 0i128;
        let mut bottleneck = Vec::new();
        let mut per_modality: BTreeMap<GliomaModality, Vec<i64>> = BTreeMap::new();
        for node in &definition.nodes {
            let key = (node.modality, node.node_id.clone());
            if let Some((value, reliability)) = values.get(&key) {
                let signed = normalize(*value) * i64::from(node.expected_direction);
                let weight = i128::from(node.weight_milli) * i128::from(*reliability);
                weighted_sum += i128::from(signed) * weight;
                weight_total += weight;
                confidence_sum += i128::from(*reliability) * i128::from(node.weight_milli);
                observed.push(node.node_id.clone());
                modalities.insert(node.modality);
                per_modality.entry(node.modality).or_default().push(signed);
                if *reliability < request.min_confidence_milli {
                    bottleneck.push(node.node_id.clone());
                }
            } else {
                missing.push(node.node_id.clone());
                bottleneck.push(node.node_id.clone());
            }
        }
        let coverage =
            ((observed.len() as i128 * i128::from(SCALE)) / definition.nodes.len() as i128) as u16;
        let activity = if weight_total == 0 {
            0
        } else {
            (weighted_sum / weight_total).clamp(-i128::from(SCALE), i128::from(SCALE)) as i64
        };
        let confidence = if weight_total == 0 {
            0
        } else {
            ((confidence_sum * i128::from(SCALE)) / weight_total).clamp(0, i128::from(SCALE)) as u16
        };
        let modality_means = per_modality
            .values()
            .filter_map(|items| {
                if items.is_empty() {
                    None
                } else {
                    Some(items.iter().sum::<i64>() / items.len() as i64)
                }
            })
            .collect::<Vec<_>>();
        let agreement = if modality_means.is_empty() {
            0
        } else {
            let min = *modality_means.iter().min().unwrap_or(&0);
            let max = *modality_means.iter().max().unwrap_or(&0);
            (SCALE - (max - min).unsigned_abs().min(SCALE as u64) as i64) as u16
        };
        let direction = if observed.len() < request.min_observed_nodes
            || modalities.len() < request.min_modalities
            || confidence < request.min_confidence_milli
            || (request.require_cross_modal && modalities.len() < 2)
        {
            PathwayActivityDirection::Unresolved
        } else if activity > 100 {
            PathwayActivityDirection::Activated
        } else if activity < -100 {
            PathwayActivityDirection::Suppressed
        } else {
            PathwayActivityDirection::Neutral
        };
        let mut record_negative = BTreeSet::new();
        let mut record_uncertainty = BTreeSet::new();
        if !missing.is_empty() {
            record_negative.insert("pathway nodes are missing and were not imputed".into());
        }
        if agreement < request.min_confidence_milli {
            record_negative.insert("modalities disagree on signed pathway activity".into());
        }
        if confidence < request.min_confidence_milli {
            record_uncertainty
                .insert("observed node reliability is below the release floor".into());
        }
        if direction == PathwayActivityDirection::Unresolved {
            record_uncertainty.insert(
                "pathway activity did not meet observed-node, modality, or confidence gates".into(),
            );
        }
        negative.extend(record_negative.iter().cloned());
        uncertainty.extend(record_uncertainty.iter().cloned());
        records.push(PathwayActivityRecord {
            pathway_id: pathway_id.clone(),
            label: definition.label.clone(),
            node_order,
            observed_node_order: observed,
            missing_node_order: missing,
            modality_order: modalities.into_iter().collect(),
            signed_activity_milli: activity,
            coverage_milli: coverage,
            cross_modal_agreement_milli: agreement,
            confidence_milli: confidence,
            bottleneck_order: bottleneck,
            direction,
            negative_evidence: record_negative.into_iter().collect(),
            uncertainty: record_uncertainty.into_iter().collect(),
        });
    }
    let mut ranking = records
        .iter()
        .map(|record| {
            (
                record.pathway_id.clone(),
                (record.signed_activity_milli.unsigned_abs()
                    * u64::from(record.coverage_milli)
                    * u64::from(record.confidence_milli)),
            )
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|a, b| (b.1, a.0.clone()).cmp(&(a.1, b.0.clone())));
    let ranking_order = ranking.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
    let qualified = records
        .iter()
        .filter(|record| record.direction != PathwayActivityDirection::Unresolved)
        .count();
    let disposition = if qualified == records.len() && negative.is_empty() {
        PathwayActivityDisposition::Qualified
    } else if qualified == 0 {
        PathwayActivityDisposition::Unresolved
    } else {
        PathwayActivityDisposition::Partial
    };
    let mut output = PathwayActivityAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        pathway_order,
        ranking_order,
        pathways: records,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| PathwayActivityError::Digest(e.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|e| PathwayActivityError::Digest(e.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }
    fn request() -> PathwayActivityRequest {
        PathwayActivityRequest {
            objective: "rank glioma invasion pathways".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            min_pathway_nodes: 2,
            min_observed_nodes: 2,
            min_modalities: 2,
            min_confidence_milli: 700,
            max_pathways: 8,
            require_cross_modal: true,
        }
    }
    fn definition() -> PathwayActivityDefinition {
        PathwayActivityDefinition {
            pathway_id: "invasion".into(),
            label: "invasion programme".into(),
            nodes: vec![
                PathwayActivityNode {
                    node_id: "egfr".into(),
                    label: "EGFR".into(),
                    modality: GliomaModality::Genomics,
                    expected_direction: 1,
                    weight_milli: 1_000,
                },
                PathwayActivityNode {
                    node_id: "vimentin".into(),
                    label: "VIM".into(),
                    modality: GliomaModality::Transcriptomics,
                    expected_direction: 1,
                    weight_milli: 1_000,
                },
            ],
            edges: vec![PathwayActivityEdge {
                source_node_id: "egfr".into(),
                target_node_id: "vimentin".into(),
                relation: 1,
                confidence_milli: 900,
            }],
        }
    }
    fn observation(
        id: &str,
        modality: GliomaModality,
        feature: &str,
        value: i64,
    ) -> PathwayActivityObservation {
        PathwayActivityObservation {
            observation_id: id.into(),
            study_id: "study-1".into(),
            sample_lineage: "sample-1".into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            artifact: artifact(id),
            feature_id: feature.into(),
            value_milli: value,
            reliability_milli: 900,
        }
    }
    #[test]
    fn ranks_cross_modal_activity_and_replays() {
        let observations = vec![
            observation("g", GliomaModality::Genomics, "egfr", 700),
            observation("t", GliomaModality::Transcriptomics, "vimentin", 800),
        ];
        let first = analyze_glioma_pathway_activity(&request(), &[definition()], &observations)
            .expect("analysis");
        let second = analyze_glioma_pathway_activity(
            &request(),
            &[definition()],
            &observations.into_iter().rev().collect::<Vec<_>>(),
        )
        .expect("analysis");
        assert_eq!(first, second);
        assert_eq!(first.disposition, PathwayActivityDisposition::Qualified);
        assert_eq!(
            first.pathways[0].direction,
            PathwayActivityDirection::Activated
        );
    }
    #[test]
    fn missing_node_is_partial_and_not_imputed() {
        let output = analyze_glioma_pathway_activity(
            &request(),
            &[definition()],
            &[observation("g", GliomaModality::Genomics, "egfr", 700)],
        )
        .expect("analysis");
        assert_eq!(output.disposition, PathwayActivityDisposition::Unresolved);
        assert_eq!(output.pathways[0].missing_node_order, vec!["vimentin"]);
        assert!(!output.negative_evidence.is_empty());
    }
}
