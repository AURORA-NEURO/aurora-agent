//! Transitive dependency-closure analysis for preclinical glioma research objects.
//!
//! Multimodal packaging validates direct artifact references, but a release also needs a bounded
//! transitive closure over every upstream object and program.  This feature computes that closure,
//! detects cycles and orphaned artifacts, and reports missing program coverage before replay or
//! signing.  It is analysis-only: no artifact is fetched, moved, rewritten, or executed.

use super::multimodal_bundle::MultimodalResearchObjectBundle;
use crate::glioma_engine::GliomaModality;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectDependencyClosure1@1";
pub const MAX_DEPTH: usize = 128;
pub const MAX_ARTIFACTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyClosureNodeStatus {
    Root,
    Closed,
    MissingUpstream,
    Cycle,
    Orphaned,
    DepthExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyClosureDisposition {
    Closed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyClosureRequest {
    pub objective: String,
    pub bundle: MultimodalResearchObjectBundle,
    pub max_depth: usize,
    pub required_programs: BTreeSet<String>,
    pub require_program_coverage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyClosureNode {
    pub artifact_id: String,
    pub modality: GliomaModality,
    pub source_program: String,
    pub upstream_order: Vec<String>,
    pub depth: usize,
    pub status: DependencyClosureNodeStatus,
    pub issue_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyClosurePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub bundle_digest: ContentHash,
    pub artifact_order: Vec<String>,
    pub root_order: Vec<String>,
    pub traversal_order: Vec<String>,
    pub nodes: Vec<DependencyClosureNode>,
    pub missing_upstream_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub orphan_order: Vec<String>,
    pub depth_exceeded_order: Vec<String>,
    pub uncovered_program_order: Vec<String>,
    pub program_coverage_order: Vec<String>,
    pub limitations: Vec<String>,
    pub disposition: DependencyClosureDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DependencyClosureError {
    #[error("dependency closure request is invalid: {0}")]
    InvalidRequest(String),
    #[error("dependency closure output is invalid: {0}")]
    InvalidOutput(String),
    #[error("dependency closure digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &DependencyClosurePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "bundle_digest": plan.bundle_digest,
        "artifact_order": plan.artifact_order,
        "root_order": plan.root_order,
        "traversal_order": plan.traversal_order,
        "nodes": plan.nodes,
        "missing_upstream_order": plan.missing_upstream_order,
        "cycle_order": plan.cycle_order,
        "orphan_order": plan.orphan_order,
        "depth_exceeded_order": plan.depth_exceeded_order,
        "uncovered_program_order": plan.uncovered_program_order,
        "program_coverage_order": plan.program_coverage_order,
        "limitations": plan.limitations,
        "disposition": plan.disposition,
    })
}

impl DependencyClosurePlan {
    pub fn validate(&self) -> Result<(), DependencyClosureError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.bundle_digest.as_str().len() != 64
            || !canonical(&self.artifact_order)
            || !canonical(&self.root_order)
            || !canonical(&self.traversal_order)
            || !canonical(&self.missing_upstream_order)
            || !canonical(&self.cycle_order)
            || !canonical(&self.orphan_order)
            || !canonical(&self.depth_exceeded_order)
            || !canonical(&self.uncovered_program_order)
            || !canonical(&self.program_coverage_order)
            || !canonical(&self.limitations)
            || self.nodes.len() != self.artifact_order.len()
            || self.nodes.iter().any(|node| {
                node.artifact_id.trim().is_empty()
                    || !canonical(&node.upstream_order)
                    || !canonical(&node.issue_order)
            })
        {
            return Err(DependencyClosureError::InvalidOutput(
                "identity, closure ordering, node cardinality, or digest shape is invalid".into(),
            ));
        }
        let ids = self
            .nodes
            .iter()
            .map(|node| node.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        if ids != self.artifact_order.iter().cloned().collect::<BTreeSet<_>>()
            || self
                .root_order
                .iter()
                .chain(self.traversal_order.iter())
                .chain(self.missing_upstream_order.iter())
                .chain(self.cycle_order.iter())
                .chain(self.orphan_order.iter())
                .chain(self.depth_exceeded_order.iter())
                .any(|id| !ids.contains(id))
        {
            return Err(DependencyClosureError::InvalidOutput(
                "closure partitions contain unknown artifacts".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DependencyClosureError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DependencyClosureError::Digest(
                "dependency closure digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &DependencyClosureRequest) -> Result<(), DependencyClosureError> {
    if request.objective.trim().is_empty()
        || request.max_depth == 0
        || request.max_depth > MAX_DEPTH
        || request.bundle.entries.is_empty()
        || request.bundle.entries.len() > MAX_ARTIFACTS
        || request
            .required_programs
            .iter()
            .any(|program| program.trim().is_empty())
    {
        return Err(DependencyClosureError::InvalidRequest(
            "objective, bounded depth/artifacts, and non-empty program identifiers are required"
                .into(),
        ));
    }
    request
        .bundle
        .validate()
        .map_err(|error| DependencyClosureError::InvalidRequest(error.to_string()))?;
    Ok(())
}

/// Compute transitive artifact/program closure and fail-closed release blockers.
pub fn analyze_glioma_research_object_dependency_closure(
    request: &DependencyClosureRequest,
) -> Result<DependencyClosurePlan, DependencyClosureError> {
    validate_request(request)?;
    let entries = request
        .bundle
        .entries
        .iter()
        .map(|entry| (entry.artifact_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let artifact_order = entries.keys().cloned().collect::<Vec<_>>();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    let mut roots = BTreeSet::new();
    for (artifact_id, entry) in &entries {
        if entry.upstream_artifact_ids.is_empty() {
            roots.insert(artifact_id.clone());
        }
        for upstream in &entry.upstream_artifact_ids {
            children
                .entry(upstream.clone())
                .or_default()
                .push(artifact_id.clone());
        }
    }
    for values in children.values_mut() {
        values.sort();
    }
    let root_order = roots.iter().cloned().collect::<Vec<_>>();
    let mut traversal = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    let mut orphans = BTreeSet::new();
    let mut depth_exceeded = BTreeSet::new();
    let mut node_states =
        BTreeMap::<String, (usize, DependencyClosureNodeStatus, BTreeSet<String>)>::new();

    fn visit(
        id: &str,
        depth: usize,
        max_depth: usize,
        entries: &BTreeMap<String, &super::multimodal_bundle::MultimodalResearchObjectEntry>,
        traversal: &mut BTreeSet<String>,
        missing: &mut BTreeSet<String>,
        cycles: &mut BTreeSet<String>,
        depth_exceeded: &mut BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
        states: &mut BTreeMap<String, (usize, DependencyClosureNodeStatus, BTreeSet<String>)>,
    ) {
        if !traversal.insert(id.to_string()) {
            if visiting.contains(id) {
                cycles.insert(id.to_string());
            }
            return;
        }
        visiting.insert(id.to_string());
        let entry = match entries.get(id) {
            Some(entry) => *entry,
            None => {
                missing.insert(id.to_string());
                visiting.remove(id);
                return;
            }
        };
        let mut issues = BTreeSet::new();
        let mut status = if entry.upstream_artifact_ids.is_empty() {
            DependencyClosureNodeStatus::Root
        } else {
            DependencyClosureNodeStatus::Closed
        };
        if depth > max_depth {
            depth_exceeded.insert(id.to_string());
            issues.insert("max-depth-exceeded".into());
            status = DependencyClosureNodeStatus::DepthExceeded;
        }
        for upstream in &entry.upstream_artifact_ids {
            if !entries.contains_key(upstream) {
                missing.insert(upstream.clone());
                issues.insert(format!("missing-upstream:{upstream}"));
                status = DependencyClosureNodeStatus::MissingUpstream;
            } else if visiting.contains(upstream) {
                cycles.insert(upstream.clone());
                cycles.insert(id.to_string());
                issues.insert(format!("cycle:{upstream}"));
                status = DependencyClosureNodeStatus::Cycle;
            } else {
                visit(
                    upstream,
                    depth.saturating_add(1),
                    max_depth,
                    entries,
                    traversal,
                    missing,
                    cycles,
                    depth_exceeded,
                    visiting,
                    states,
                );
            }
        }
        visiting.remove(id);
        states.insert(id.to_string(), (depth, status, issues));
    }

    for id in &artifact_order {
        visit(
            id,
            0,
            request.max_depth,
            &entries,
            &mut traversal,
            &mut missing,
            &mut cycles,
            &mut depth_exceeded,
            &mut BTreeSet::new(),
            &mut node_states,
        );
    }
    for id in &artifact_order {
        if !roots.contains(id) && !traversal.contains(id) {
            orphans.insert(id.clone());
        }
    }
    let covered_programs = entries
        .values()
        .map(|entry| entry.source_program.clone())
        .collect::<BTreeSet<_>>();
    let program_coverage_order = covered_programs.iter().cloned().collect::<Vec<_>>();
    let uncovered_program_order = request
        .required_programs
        .difference(&covered_programs)
        .cloned()
        .collect::<Vec<_>>();
    let mut limitations = request
        .bundle
        .limitations
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        limitations.insert("dependency-closure-missing-inputs".into());
    }
    if !cycles.is_empty() {
        limitations.insert("dependency-closure-cycle-detected".into());
    }
    if !uncovered_program_order.is_empty() {
        limitations.insert("dependency-closure-program-coverage-incomplete".into());
    }
    let mut nodes = Vec::new();
    for id in &artifact_order {
        let entry = entries.get(id).expect("entry exists");
        let (depth, mut status, mut issues) = node_states.remove(id).unwrap_or((
            0,
            DependencyClosureNodeStatus::Orphaned,
            BTreeSet::new(),
        ));
        if cycles.contains(id) {
            status = DependencyClosureNodeStatus::Cycle;
            issues.insert("cycle-detected".into());
        } else if depth_exceeded.contains(id) {
            status = DependencyClosureNodeStatus::DepthExceeded;
        } else if !missing.is_empty()
            && entry
                .upstream_artifact_ids
                .iter()
                .any(|upstream| missing.contains(upstream))
        {
            status = DependencyClosureNodeStatus::MissingUpstream;
        }
        nodes.push(DependencyClosureNode {
            artifact_id: id.clone(),
            modality: entry.modality,
            source_program: entry.source_program.clone(),
            upstream_order: entry.upstream_artifact_ids.clone(),
            depth,
            status,
            issue_order: issues.into_iter().collect(),
        });
    }
    let disposition = if !missing.is_empty()
        || !cycles.is_empty()
        || (request.require_program_coverage && !uncovered_program_order.is_empty())
        || !depth_exceeded.is_empty()
    {
        DependencyClosureDisposition::Blocked
    } else if !orphans.is_empty()
        || request.bundle.disposition
            != super::multimodal_bundle::MultimodalResearchObjectDisposition::ReadyForSigning
    {
        DependencyClosureDisposition::Partial
    } else {
        DependencyClosureDisposition::Closed
    };
    let mut plan = DependencyClosurePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        bundle_digest: request.bundle.digest.clone(),
        artifact_order,
        root_order,
        traversal_order: traversal.into_iter().collect(),
        nodes,
        missing_upstream_order: missing.into_iter().collect(),
        cycle_order: cycles.into_iter().collect(),
        orphan_order: orphans.into_iter().collect(),
        depth_exceeded_order: depth_exceeded.into_iter().collect(),
        uncovered_program_order,
        program_coverage_order,
        limitations: limitations.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| DependencyClosureError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::multimodal_bundle::{
        compile_glioma_multimodal_research_object, MultimodalResearchObjectInput,
        MultimodalResearchObjectRequest,
    };
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/vnd.aurora.glioma.closure+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn bundle(upstream_a: Vec<&str>, upstream_b: Vec<&str>) -> MultimodalResearchObjectBundle {
        let release = ResearchObjectRequest {
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            objective: "closure".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["P03".into(), "P11".into()],
            artifacts: vec![artifact("placeholder")],
            negative_evidence: Vec::new(),
            limitations: vec!["preclinical-only".into()],
            raw_data_local: true,
            aggregate_only: true,
        };
        let input = |modality: GliomaModality, id: &str, upstream: Vec<&str>| {
            MultimodalResearchObjectInput {
                modality,
                artifact: artifact(id),
                source_program: "P03".into(),
                schema_version: "1.0".into(),
                semantic_loss_milli: 0,
                provenance_digest: hash("shared"),
                upstream_artifact_ids: upstream.into_iter().map(str::to_string).collect(),
                required: true,
            }
        };
        compile_glioma_multimodal_research_object(&MultimodalResearchObjectRequest {
            release,
            inputs: vec![
                input(GliomaModality::Imaging, "a", upstream_a),
                input(GliomaModality::Genomics, "b", upstream_b),
            ],
            required_modalities: BTreeSet::from([
                GliomaModality::Imaging,
                GliomaModality::Genomics,
            ]),
            max_semantic_loss_milli: 200,
            require_cross_modal_alignment: true,
            max_inputs: 8,
        })
        .unwrap()
    }

    #[test]
    fn closure_accepts_rooted_artifacts_and_program_coverage() {
        let plan = analyze_glioma_research_object_dependency_closure(&DependencyClosureRequest {
            objective: "closure".into(),
            bundle: bundle(Vec::new(), vec!["a"]),
            max_depth: 8,
            required_programs: BTreeSet::from(["P03".into()]),
            require_program_coverage: true,
        })
        .unwrap();
        assert_eq!(plan.disposition, DependencyClosureDisposition::Closed);
        assert_eq!(plan.root_order, vec!["a"]);
        assert_eq!(plan.traversal_order, vec!["a", "b"]);
        plan.validate().unwrap();
    }

    #[test]
    fn closure_blocks_cycles_and_missing_upstream() {
        let cycle = analyze_glioma_research_object_dependency_closure(&DependencyClosureRequest {
            objective: "closure".into(),
            bundle: bundle(vec!["b"], vec!["a"]),
            max_depth: 8,
            required_programs: BTreeSet::new(),
            require_program_coverage: false,
        })
        .unwrap();
        assert_eq!(cycle.disposition, DependencyClosureDisposition::Blocked);
        assert!(!cycle.cycle_order.is_empty());

        let missing =
            analyze_glioma_research_object_dependency_closure(&DependencyClosureRequest {
                objective: "closure".into(),
                bundle: bundle(vec!["unknown"], Vec::new()),
                max_depth: 8,
                required_programs: BTreeSet::new(),
                require_program_coverage: false,
            })
            .unwrap();
        assert_eq!(missing.disposition, DependencyClosureDisposition::Blocked);
        assert_eq!(missing.missing_upstream_order, vec!["unknown"]);
    }
}
