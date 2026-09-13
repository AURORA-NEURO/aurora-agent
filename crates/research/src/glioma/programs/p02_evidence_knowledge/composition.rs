//! Evidence-network composition for autonomous preclinical glioma research.
//!
//! Typed knowledge contains independently qualified claims, but an autonomous engine also needs
//! to know whether a *chain* of claims is usable: which claim is a prerequisite, where support is
//! weakest, and whether a contradiction cuts across the chain. This module composes only explicit
//! caller-declared relations. It never infers causality, invents edges, or turns a supported path
//! into a clinical or therapeutic conclusion.

use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeComposition1@1";
pub const MAX_RELATIONS: usize = 32_768;
pub const MAX_PATHS: usize = 4_096;
pub const MAX_PATH_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeRelationKind {
    Supports,
    Requires,
    Contradicts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRelation {
    pub relation_id: String,
    pub from_claim_id: String,
    pub to_claim_id: String,
    pub kind: KnowledgeRelationKind,
    pub strength_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCompositionRequest {
    pub objective: String,
    pub min_path_length: usize,
    pub max_paths: usize,
    pub min_strength_milli: u16,
    pub max_contradiction_milli: u16,
    pub require_supported_root: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgePathDisposition {
    Qualified,
    Contested,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCompositionPath {
    pub path_id: String,
    pub claim_order: Vec<String>,
    pub relation_order: Vec<String>,
    pub strength_milli: u16,
    pub contradiction_milli: u16,
    pub bottleneck_claim_id: String,
    pub disposition: KnowledgePathDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeComponentDisposition {
    Qualified,
    Contested,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeCompositionComponent {
    pub component_id: String,
    pub claim_order: Vec<String>,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub unresolved_claim_order: Vec<String>,
    pub disposition: KnowledgeComponentDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeCompositionDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeComposition {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub relation_order: Vec<String>,
    pub components: Vec<KnowledgeCompositionComponent>,
    pub paths: Vec<KnowledgeCompositionPath>,
    pub selected_path_order: Vec<String>,
    pub bottleneck_claim_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: KnowledgeCompositionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeCompositionError {
    #[error("knowledge composition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge composition input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge composition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge composition digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique<T: Ord>(values: &[T]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &KnowledgeComposition) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "relation_order": output.relation_order,
        "components": output.components,
        "paths": output.paths,
        "selected_path_order": output.selected_path_order,
        "bottleneck_claim_order": output.bottleneck_claim_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

fn claim_score(claim: &super::knowledge_graph::KnowledgeClaim) -> u16 {
    match claim.disposition {
        KnowledgeClaimDisposition::Supported => claim.confidence_milli,
        KnowledgeClaimDisposition::Contested => claim.confidence_milli.min(claim.support_milli),
        KnowledgeClaimDisposition::Negative => 0,
        KnowledgeClaimDisposition::Unresolved => claim.confidence_milli / 2,
    }
}

fn relation_adjacency(relations: &[KnowledgeRelation]) -> BTreeMap<String, Vec<KnowledgeRelation>> {
    let mut adjacency = BTreeMap::<String, Vec<KnowledgeRelation>>::new();
    for relation in relations {
        if matches!(
            relation.kind,
            KnowledgeRelationKind::Supports | KnowledgeRelationKind::Requires
        ) {
            adjacency
                .entry(relation.from_claim_id.clone())
                .or_default()
                .push(relation.clone());
        }
    }
    for edges in adjacency.values_mut() {
        edges.sort_by(|left, right| {
            left.to_claim_id
                .cmp(&right.to_claim_id)
                .then(left.relation_id.cmp(&right.relation_id))
        });
    }
    adjacency
}

fn contradiction_score(claim_order: &[String], relations: &[KnowledgeRelation]) -> u16 {
    let claims = claim_order.iter().collect::<BTreeSet<_>>();
    relations
        .iter()
        .filter(|relation| {
            relation.kind == KnowledgeRelationKind::Contradicts
                && claims.contains(&relation.from_claim_id)
                && claims.contains(&relation.to_claim_id)
        })
        .map(|relation| relation.strength_milli)
        .max()
        .unwrap_or(0)
}

fn path_id(
    claim_order: &[String],
    relation_order: &[String],
) -> Result<String, KnowledgeCompositionError> {
    let digest = ContentHash::of_value(&serde_json::json!({
        "claims": claim_order,
        "relations": relation_order,
    }))
    .map_err(|error| KnowledgeCompositionError::Digest(error.to_string()))?;
    Ok(format!("path-{digest}"))
}

fn component_id(claim_order: &[String]) -> Result<String, KnowledgeCompositionError> {
    let digest = ContentHash::of_value(&serde_json::json!({"claims": claim_order}))
        .map_err(|error| KnowledgeCompositionError::Digest(error.to_string()))?;
    Ok(format!("component-{digest}"))
}

fn validate_request(
    request: &KnowledgeCompositionRequest,
) -> Result<(), KnowledgeCompositionError> {
    if request.objective.trim().is_empty()
        || request.min_path_length < 2
        || request.min_path_length > MAX_PATH_LENGTH
        || request.max_paths == 0
        || request.max_paths > MAX_PATHS
        || request.min_strength_milli > 1_000
        || request.max_contradiction_milli > 1_000
    {
        return Err(KnowledgeCompositionError::InvalidRequest(
            "objective, path bounds, path count, and score thresholds are invalid".into(),
        ));
    }
    Ok(())
}

impl KnowledgeComposition {
    pub fn validate(&self) -> Result<(), KnowledgeCompositionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.relation_order)
            || !canonical(&self.selected_path_order)
            || !canonical(&self.bottleneck_claim_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self
                .components
                .windows(2)
                .any(|pair| pair[0].component_id >= pair[1].component_id)
            || self
                .paths
                .windows(2)
                .any(|pair| pair[0].path_id >= pair[1].path_id)
            || self.paths.iter().any(|path| {
                path.path_id.trim().is_empty()
                    || path.claim_order.len() < 2
                    || path.claim_order.len() > MAX_PATH_LENGTH
                    || !unique(&path.claim_order)
                    || !unique(&path.relation_order)
                    || path.strength_milli > 1_000
                    || path.contradiction_milli > 1_000
                    || path.bottleneck_claim_id.trim().is_empty()
                    || !path.claim_order.contains(&path.bottleneck_claim_id)
            })
            || self.components.iter().any(|component| {
                component.component_id.trim().is_empty()
                    || !canonical(&component.claim_order)
                    || !canonical(&component.unresolved_claim_order)
                    || component.support_milli > 1_000
                    || component.contradiction_milli > 1_000
            })
        {
            return Err(KnowledgeCompositionError::InvalidOutput(
                "identity, graph ordering, path, component, or score invariants are invalid".into(),
            ));
        }
        let claim_set = self.claim_order.iter().collect::<BTreeSet<_>>();
        let relation_set = self.relation_order.iter().collect::<BTreeSet<_>>();
        if self
            .paths
            .iter()
            .flat_map(|path| path.claim_order.iter())
            .any(|claim| !claim_set.contains(claim))
            || self
                .paths
                .iter()
                .flat_map(|path| path.relation_order.iter())
                .any(|relation| !relation_set.contains(relation))
            || self.selected_path_order.iter().any(|path| {
                !self
                    .paths
                    .iter()
                    .any(|candidate| &candidate.path_id == path)
            })
        {
            return Err(KnowledgeCompositionError::InvalidOutput(
                "paths or selected paths reference unknown graph identities".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeCompositionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeCompositionError::InvalidOutput(
                "composition digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn build_components(
    knowledge: &TypedKnowledge,
    relations: &[KnowledgeRelation],
) -> Result<Vec<KnowledgeCompositionComponent>, KnowledgeCompositionError> {
    let claim_ids = knowledge
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for claim in &claim_ids {
        adjacency.entry(claim.clone()).or_default();
    }
    for relation in relations {
        adjacency
            .entry(relation.from_claim_id.clone())
            .or_default()
            .insert(relation.to_claim_id.clone());
        adjacency
            .entry(relation.to_claim_id.clone())
            .or_default()
            .insert(relation.from_claim_id.clone());
    }
    let mut visited = BTreeSet::new();
    let mut components = Vec::new();
    let claim_map = knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_id.as_str(), claim))
        .collect::<BTreeMap<_, _>>();
    for root in claim_ids {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut queue = VecDeque::from([root]);
        let mut claims = BTreeSet::new();
        while let Some(claim) = queue.pop_front() {
            claims.insert(claim.clone());
            for neighbour in adjacency.get(&claim).into_iter().flatten() {
                if visited.insert(neighbour.clone()) {
                    queue.push_back(neighbour.clone());
                }
            }
        }
        let claim_order = claims.into_iter().collect::<Vec<_>>();
        let support_milli = claim_order
            .iter()
            .filter_map(|claim| claim_map.get(claim.as_str()))
            .map(|claim| claim_score(claim))
            .max()
            .unwrap_or(0);
        let contradiction_milli = contradiction_score(&claim_order, relations);
        let unresolved_claim_order = claim_order
            .iter()
            .filter(|claim| {
                claim_map
                    .get(claim.as_str())
                    .is_some_and(|claim| claim.disposition == KnowledgeClaimDisposition::Unresolved)
            })
            .cloned()
            .collect::<Vec<_>>();
        let disposition = if !unresolved_claim_order.is_empty() {
            KnowledgeComponentDisposition::Unresolved
        } else if contradiction_milli > 0 {
            KnowledgeComponentDisposition::Contested
        } else {
            KnowledgeComponentDisposition::Qualified
        };
        components.push(KnowledgeCompositionComponent {
            component_id: component_id(&claim_order)?,
            claim_order,
            support_milli,
            contradiction_milli,
            unresolved_claim_order,
            disposition,
        });
    }
    components.sort_by(|left, right| left.component_id.cmp(&right.component_id));
    Ok(components)
}

struct PathSearch<'a> {
    adjacency: &'a BTreeMap<String, Vec<KnowledgeRelation>>,
    claim_scores: &'a BTreeMap<String, u16>,
    relations: &'a [KnowledgeRelation],
    request: &'a KnowledgeCompositionRequest,
    paths: Vec<KnowledgeCompositionPath>,
}

impl<'a> PathSearch<'a> {
    fn visit(
        &mut self,
        current: String,
        claims: Vec<String>,
        edge_ids: Vec<String>,
        strength: u16,
    ) {
        if self.paths.len() >= self.request.max_paths || claims.len() > MAX_PATH_LENGTH {
            return;
        }
        let outgoing = self.adjacency.get(&current).cloned().unwrap_or_default();
        let mut advanced = false;
        for edge in outgoing {
            if claims.contains(&edge.to_claim_id) {
                continue;
            }
            let Some(score) = self.claim_scores.get(&edge.to_claim_id).copied() else {
                continue;
            };
            let mut next_claims = claims.clone();
            next_claims.push(edge.to_claim_id.clone());
            let mut next_edges = edge_ids.clone();
            next_edges.push(edge.relation_id.clone());
            let next_strength = strength.min(edge.strength_milli).min(score);
            self.visit(edge.to_claim_id, next_claims, next_edges, next_strength);
            advanced = true;
        }
        if !advanced && claims.len() >= self.request.min_path_length {
            let contradiction = contradiction_score(&claims, self.relations);
            let bottleneck_claim_id = claims
                .iter()
                .min_by_key(|claim| self.claim_scores.get(*claim).copied().unwrap_or(0))
                .cloned()
                .unwrap_or_else(|| current.clone());
            let disposition = if claims
                .iter()
                .any(|claim| self.claim_scores.get(claim).copied().unwrap_or(0) == 0)
            {
                KnowledgePathDisposition::Unresolved
            } else if contradiction > 0 {
                KnowledgePathDisposition::Contested
            } else {
                KnowledgePathDisposition::Qualified
            };
            if strength >= self.request.min_strength_milli
                && contradiction <= self.request.max_contradiction_milli
            {
                if let Ok(path_id) = path_id(&claims, &edge_ids) {
                    self.paths.push(KnowledgeCompositionPath {
                        path_id,
                        claim_order: claims.clone(),
                        relation_order: edge_ids,
                        strength_milli: strength,
                        contradiction_milli: contradiction,
                        bottleneck_claim_id,
                        disposition,
                    });
                }
            }
        }
        if !advanced && claims.len() < self.request.min_path_length {
            // The branch is retained as uncertainty by the caller through the claim graph; no
            // short path is emitted as if it met the declared scientific minimum.
        }
    }
}

/// Compose a typed claim graph into deterministic mechanistic evidence paths and component gates.
pub fn compose_knowledge_graph(
    request: &KnowledgeCompositionRequest,
    knowledge: &TypedKnowledge,
    input_relations: &[KnowledgeRelation],
) -> Result<KnowledgeComposition, KnowledgeCompositionError> {
    validate_request(request)?;
    knowledge
        .validate()
        .map_err(|error| KnowledgeCompositionError::InvalidInput(error.to_string()))?;
    if input_relations.len() > MAX_RELATIONS {
        return Err(KnowledgeCompositionError::InvalidInput(
            "relation count exceeds the bounded graph limit".into(),
        ));
    }
    let claim_ids = knowledge
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let mut relation_ids = BTreeSet::new();
    let mut relations = input_relations.to_vec();
    for relation in &relations {
        if relation.relation_id.trim().is_empty()
            || relation.from_claim_id == relation.to_claim_id
            || relation.strength_milli > 1_000
            || !claim_ids.contains(&relation.from_claim_id)
            || !claim_ids.contains(&relation.to_claim_id)
            || !relation_ids.insert(relation.relation_id.clone())
        {
            return Err(KnowledgeCompositionError::InvalidInput(
                "relation identities, endpoints, strength, or uniqueness are invalid".into(),
            ));
        }
    }
    relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    let claim_order = claim_ids.into_iter().collect::<Vec<_>>();
    let relation_order = relations
        .iter()
        .map(|relation| relation.relation_id.clone())
        .collect::<Vec<_>>();
    let claim_map = knowledge
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let claim_scores = claim_map
        .iter()
        .map(|(id, claim)| (id.clone(), claim_score(claim)))
        .collect::<BTreeMap<_, _>>();
    let adjacency = relation_adjacency(&relations);
    let roots = claim_order
        .iter()
        .filter(|id| {
            !request.require_supported_root
                || claim_map
                    .get(*id)
                    .is_some_and(|claim| claim.disposition == KnowledgeClaimDisposition::Supported)
        })
        .cloned()
        .collect::<Vec<_>>();
    let roots = if roots.is_empty() && !request.require_supported_root {
        claim_order.clone()
    } else {
        roots
    };
    let mut search = PathSearch {
        adjacency: &adjacency,
        claim_scores: &claim_scores,
        relations: &relations,
        request,
        paths: Vec::new(),
    };
    for root in roots {
        if search.paths.len() >= request.max_paths {
            break;
        }
        let score = claim_scores.get(&root).copied().unwrap_or(0);
        search.visit(root.clone(), vec![root], Vec::new(), score);
    }
    search.paths.sort_by(|left, right| {
        right
            .strength_milli
            .cmp(&left.strength_milli)
            .then(left.path_id.cmp(&right.path_id))
    });
    search
        .paths
        .dedup_by(|left, right| left.path_id == right.path_id);
    let selected_path_order = search
        .paths
        .iter()
        .filter(|path| path.disposition == KnowledgePathDisposition::Qualified)
        .take(request.max_paths)
        .map(|path| path.path_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut paths = search.paths;
    paths.sort_by(|left, right| left.path_id.cmp(&right.path_id));
    let bottleneck_claim_order = paths
        .iter()
        .filter(|path| selected_path_order.binary_search(&path.path_id).is_ok())
        .map(|path| path.bottleneck_claim_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let components = build_components(knowledge, &relations)?;
    let negative_evidence_order = knowledge
        .claims
        .iter()
        .filter(|claim| claim.disposition == KnowledgeClaimDisposition::Negative)
        .map(|claim| claim.claim_id.clone())
        .chain(
            relations
                .iter()
                .filter(|relation| relation.kind == KnowledgeRelationKind::Contradicts)
                .map(|relation| relation.relation_id.clone()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let uncertainty_order = knowledge
        .claims
        .iter()
        .filter(|claim| claim.disposition == KnowledgeClaimDisposition::Unresolved)
        .map(|claim| claim.claim_id.clone())
        .chain(
            knowledge
                .claims
                .iter()
                .filter(|claim| {
                    claim.missing_modality_order.len() + claim.missing_model_system_order.len() > 0
                })
                .map(|claim| format!("coverage:{}", claim.claim_id)),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = if selected_path_order.is_empty() {
        KnowledgeCompositionDisposition::Unresolved
    } else if !negative_evidence_order.is_empty() || !uncertainty_order.is_empty() {
        KnowledgeCompositionDisposition::Partial
    } else {
        KnowledgeCompositionDisposition::Qualified
    };
    let mut output = KnowledgeComposition {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: knowledge.digest.clone(),
        claim_order,
        relation_order,
        components,
        paths,
        selected_path_order,
        bottleneck_claim_order,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-composition"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeCompositionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

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

    fn knowledge() -> TypedKnowledge {
        let record = |id: &str, claim: &str, state| EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(id),
            source_kind: EvidenceSourceKind::Dataset,
            claim: claim.into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "compose glioma invasion mechanism".into(),
                required_modalities: BTreeSet::new(),
                required_model_systems: BTreeSet::new(),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[
                record(
                    "a",
                    "EGFR signaling increases invasion",
                    EvidenceState::Supported,
                ),
                record(
                    "b",
                    "invasion increases dissemination",
                    EvidenceState::Supported,
                ),
            ],
        )
        .unwrap()
    }

    fn request() -> KnowledgeCompositionRequest {
        KnowledgeCompositionRequest {
            objective: "compose glioma invasion mechanism".into(),
            min_path_length: 2,
            max_paths: 8,
            min_strength_milli: 500,
            max_contradiction_milli: 500,
            require_supported_root: true,
        }
    }

    #[test]
    fn composes_replay_stable_supported_path_and_bottleneck() {
        let knowledge = knowledge();
        let ids = knowledge.claim_order.clone();
        let relations = vec![KnowledgeRelation {
            relation_id: "r1".into(),
            from_claim_id: ids[0].clone(),
            to_claim_id: ids[1].clone(),
            kind: KnowledgeRelationKind::Supports,
            strength_milli: 900,
        }];
        let first = compose_knowledge_graph(&request(), &knowledge, &relations).unwrap();
        let second = compose_knowledge_graph(&request(), &knowledge, &relations).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            KnowledgeCompositionDisposition::Qualified
        );
        assert_eq!(first.selected_path_order.len(), 1);
        assert_eq!(first.bottleneck_claim_order.len(), 1);
        first.validate().unwrap();
    }

    #[test]
    fn contradiction_remains_contested_and_cannot_be_averaged_away() {
        let knowledge = knowledge();
        let ids = knowledge.claim_order.clone();
        let relations = vec![
            KnowledgeRelation {
                relation_id: "r-support".into(),
                from_claim_id: ids[0].clone(),
                to_claim_id: ids[1].clone(),
                kind: KnowledgeRelationKind::Supports,
                strength_milli: 900,
            },
            KnowledgeRelation {
                relation_id: "r-contradiction".into(),
                from_claim_id: ids[1].clone(),
                to_claim_id: ids[0].clone(),
                kind: KnowledgeRelationKind::Contradicts,
                strength_milli: 700,
            },
        ];
        let mut composition_request = request();
        composition_request.max_contradiction_milli = 1_000;
        let output = compose_knowledge_graph(&composition_request, &knowledge, &relations).unwrap();
        assert_eq!(output.paths[0].contradiction_milli, 700);
        assert_eq!(
            output.paths[0].disposition,
            KnowledgePathDisposition::Contested
        );
        assert!(output
            .negative_evidence_order
            .contains(&"r-contradiction".into()));
        assert_eq!(
            output.disposition,
            KnowledgeCompositionDisposition::Unresolved
        );
    }

    #[test]
    fn unknown_endpoint_and_short_path_are_refused() {
        let knowledge = knowledge();
        let ids = knowledge.claim_order.clone();
        let mut composition_request = request();
        composition_request.min_path_length = 3;
        let short = compose_knowledge_graph(
            &composition_request,
            &knowledge,
            &[KnowledgeRelation {
                relation_id: "r1".into(),
                from_claim_id: ids[0].clone(),
                to_claim_id: ids[1].clone(),
                kind: KnowledgeRelationKind::Supports,
                strength_milli: 900,
            }],
        )
        .unwrap();
        assert!(short.paths.is_empty());
        let error = compose_knowledge_graph(
            &request(),
            &knowledge,
            &[KnowledgeRelation {
                relation_id: "bad".into(),
                from_claim_id: "missing".into(),
                to_claim_id: ids[0].clone(),
                kind: KnowledgeRelationKind::Supports,
                strength_milli: 900,
            }],
        )
        .unwrap_err();
        assert!(error.to_string().contains("endpoints"));
    }
}
