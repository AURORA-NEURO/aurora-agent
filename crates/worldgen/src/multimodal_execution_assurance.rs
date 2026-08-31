//! Multimodal multi-study computational-execution assurance (`AFA-worldgen-P12-F26`).
//!
//! This is a product-grade A1 preflight: it verifies a caller-supplied research workflow graph,
//! studies/modalities, evidence state, budget, replay identity, and governance posture before any
//! executor is allowed to consume the plan. It never dispatches code, reads raw data, or claims a
//! scientific result.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P12-F26";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-computational-execution-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec2@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen-execution-run-7+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_NODES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenExecutionNode6 {
    pub node_id: String,
    pub study_id: String,
    pub modality: String,
    pub depends_on: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: ExecutionEvidenceState,
    pub estimated_units: u64,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenMultimodalExecutionRequest8 {
    pub request_id: String,
    pub workflow_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub nodes: Vec<WorldgenExecutionNode6>,
    pub replay_identity: ContentHash,
    pub budget_units: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenExecutionRun7Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenExecutionRun7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub node_order: Vec<String>,
    pub planned_node_order: Vec<String>,
    pub selected_node_order: Vec<String>,
    pub unresolved_node_order: Vec<String>,
    pub blocked_node_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub budget_exceeded_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub consumed_units: u64,
    pub budget_units: u64,
    pub checkpoint_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub execution_digest: ContentHash,
    pub artifact: WorldgenExecutionRun7Artifact,
    pub effect_order: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalExecutionAssuranceError {
    #[error("invalid worldgen multimodal-execution request: {0}")]
    Invalid(String),
    #[error("worldgen multimodal-execution report failed validation: {0}")]
    Report(String),
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

pub fn multimodal_execution_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "worldgen",
        "consumers": ["research program lead", "computational execution operator", "benchmark curator"],
        "behavior": "verify multimodal multi-study execution graphs with deterministic dependency, evidence, budget, replay, provenance, policy, federation, and locality gates before executor dispatch",
        "value": "prevents incomparable, under-evidenced, over-budget, or unauthorized research runs from entering computation",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:execution-run-digests", "manage:local-capability", "block:unsafe-release"],
        "permissions": ["evaluate:capability-runs"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl WorldgenExecutionRun7 {
    pub fn validate(&self) -> Result<(), MultimodalExecutionAssuranceError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || [
                &self.request_id,
                &self.workflow_id,
                &self.purpose,
                &self.semantic_profile,
            ]
            .iter()
            .any(|value| value.trim().is_empty())
            || self.required_study_order.is_empty()
            || self.required_modality_order.is_empty()
            || self.node_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MultimodalExecutionAssuranceError::Report(
                "execution identity, requirements, nodes, effects, locality, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.required_study_order,
            &self.required_modality_order,
            &self.node_order,
            &self.selected_node_order,
            &self.unresolved_node_order,
            &self.blocked_node_order,
            &self.cycle_order,
            &self.missing_dependency_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.budget_exceeded_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(MultimodalExecutionAssuranceError::Report(
                    "execution ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.node_order.iter().cloned());
        let parts = self
            .selected_node_order
            .iter()
            .chain(&self.unresolved_node_order)
            .chain(&self.blocked_node_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.node_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || BTreeSet::from_iter(self.planned_node_order.iter().cloned()).len()
                != self.planned_node_order.len()
            || self.planned_node_order.iter().any(|id| !ids.contains(id))
            || !valid_digest(&self.checkpoint_digest)
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.execution_digest)
            || self.artifact.content_hash != self.execution_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(MultimodalExecutionAssuranceError::Report(
                "execution states, plan, digests, or artifact metadata do not validate".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:execution-run-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalExecutionAssuranceError::Report(
                "effect is outside governed execution gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalExecutionAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalExecutionAssuranceError::Report(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalExecutionAssuranceError::Report(error.to_string()))
    }
}

fn validate_request(
    request: &WorldgenMultimodalExecutionRequest8,
) -> Result<(), MultimodalExecutionAssuranceError> {
    if [
        &request.request_id,
        &request.workflow_id,
        &request.purpose,
        &request.semantic_profile,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.nodes.is_empty()
        || request.nodes.len() > MAX_NODES
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(MultimodalExecutionAssuranceError::Invalid(
            "execution identity, requirements, node bound, replay, locality, or boundary is invalid".into(),
        ));
    }
    for values in [
        &request.required_study_order,
        &request.required_modality_order,
    ] {
        let set = BTreeSet::from_iter(values.iter().cloned());
        if set.len() != values.len() || values.iter().any(|value| value.trim().is_empty()) {
            return Err(MultimodalExecutionAssuranceError::Invalid(
                "required studies and modalities must be unique and non-empty".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty()
            || !ids.insert(node.node_id.clone())
            || node.study_id.trim().is_empty()
            || node.modality.trim().is_empty()
            || !valid_digest(&node.artifact_digest)
            || !valid_digest(&node.provenance_digest)
            || !node.local
            || !node.aggregate_only
        {
            return Err(MultimodalExecutionAssuranceError::Invalid(format!(
                "node {} is invalid, duplicated, non-local, or not digest-bound",
                node.node_id
            )));
        }
        let dependency_set = BTreeSet::from_iter(node.depends_on.iter().cloned());
        if dependency_set.len() != node.depends_on.len()
            || node
                .depends_on
                .iter()
                .any(|dependency| dependency == &node.node_id)
        {
            return Err(MultimodalExecutionAssuranceError::Invalid(format!(
                "node {} has duplicate or self dependency",
                node.node_id
            )));
        }
    }
    Ok(())
}

pub fn assure_worldgen_multimodal_execution(
    request: &WorldgenMultimodalExecutionRequest8,
) -> Result<WorldgenExecutionRun7, MultimodalExecutionAssuranceError> {
    validate_request(request)?;
    let mut node_map = BTreeMap::new();
    for node in &request.nodes {
        node_map.insert(node.node_id.clone(), node);
    }
    let node_order = node_map.keys().cloned().collect::<Vec<_>>();
    let mut indegree = node_map
        .keys()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut missing_dependency = BTreeSet::new();
    let mut missing_dependency_nodes = BTreeSet::new();
    for node in node_map.values() {
        for dependency in &node.depends_on {
            if !node_map.contains_key(dependency) {
                missing_dependency.insert(format!("{}:{dependency}", node.node_id));
                missing_dependency_nodes.insert(node.node_id.clone());
            } else {
                *indegree.get_mut(&node.node_id).expect("node exists") += 1;
                dependents
                    .entry(dependency.clone())
                    .or_default()
                    .insert(node.node_id.clone());
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<BTreeSet<_>>();
    let mut planned = Vec::with_capacity(node_order.len());
    while let Some(id) = ready.pop_first() {
        planned.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("child exists");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    let planned_set = BTreeSet::from_iter(planned.iter().cloned());
    let cycle = node_order
        .iter()
        .filter(|id| !planned_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for node in node_map.values() {
        studies.insert(node.study_id.clone());
        modalities.insert(node.modality.clone());
        provenance.insert(node.provenance_digest.clone());
    }
    let missing_study = request
        .required_study_order
        .iter()
        .filter(|study| !studies.contains(*study))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality = request
        .required_modality_order
        .iter()
        .filter(|modality| !modalities.contains(*modality))
        .cloned()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for node_id in &planned {
        let node = node_map[node_id];
        if cycle.contains(node_id) {
            blocked.insert(node_id.clone());
            continue;
        }
        if missing_dependency_nodes.contains(node_id) {
            unresolved.insert(node_id.clone());
            uncertainty.insert(format!("{node_id}:missing-dependency"));
        } else if node
            .depends_on
            .iter()
            .any(|dependency| blocked.contains(dependency))
        {
            blocked.insert(node_id.clone());
            negative.insert(format!("{node_id}:blocked-dependency"));
        } else if node
            .depends_on
            .iter()
            .any(|dependency| unresolved.contains(dependency))
        {
            unresolved.insert(node_id.clone());
            uncertainty.insert(format!("{node_id}:unresolved-dependency"));
        } else {
            match node.evidence_state {
                ExecutionEvidenceState::Contradicted => {
                    blocked.insert(node_id.clone());
                    negative.insert(format!("{node_id}:contradicted"));
                }
                ExecutionEvidenceState::Unknown | ExecutionEvidenceState::Unmeasured => {
                    unresolved.insert(node_id.clone());
                    evidence.insert(node_id.clone());
                    uncertainty.insert(format!("{node_id}:evidence-state"));
                }
                ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported => {
                    selected.insert(node_id.clone());
                }
            }
        }
    }
    for id in &cycle {
        blocked.insert(id.clone());
        negative.insert(format!("{id}:cycle"));
    }
    for item in &missing_dependency {
        uncertainty.insert(format!("{item}:missing-dependency"));
    }
    for node_id in &missing_dependency_nodes {
        selected.remove(node_id);
        unresolved.insert(node_id.clone());
    }
    for study in &missing_study {
        omissions.insert(format!("study:{study}:missing"));
        negative.insert(format!("study:{study}:no-node"));
    }
    for modality in &missing_modality {
        omissions.insert(format!("modality:{modality}:missing"));
        negative.insert(format!("modality:{modality}:no-node"));
    }
    let planned_units = selected
        .iter()
        .map(|id| node_map[id].estimated_units)
        .sum::<u64>();
    let budget_exceeded = if planned_units > request.budget_units {
        selected.iter().cloned().collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if !budget_exceeded.is_empty() {
        for id in &budget_exceeded {
            selected.remove(id);
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:budget"));
            negative.insert(format!("{id}:budget-exceeded"));
        }
        omissions.insert("request:budget-exceeded".into());
    }
    let consumed_units = selected
        .iter()
        .map(|id| node_map[id].estimated_units)
        .sum::<u64>();
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(node_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global_block || !blocked_order.is_empty() {
        "blocked"
    } else if !unresolved_order.is_empty()
        || !missing_study.is_empty()
        || !missing_modality.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:multimodal-execution-not-closed".into());
    }
    let effect_order = if disposition == "qualified" {
        vec![
            "exchange:execution-run-digests".to_string(),
            "manage:local-capability".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    let checkpoint_payload = json!({
        "workflow_id": request.workflow_id,
        "planned_node_order": planned,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
    });
    let checkpoint_digest = ContentHash::of_value(&checkpoint_payload)
        .map_err(|error| MultimodalExecutionAssuranceError::Report(error.to_string()))?;
    let payload = json!({
        "schema_version":"aurora-research-contract/1.0",
        "contract_version":CONTRACT_VERSION,
        "feature_id":FEATURE_ID,
        "request_id":request.request_id,
        "workflow_id":request.workflow_id,
        "purpose":request.purpose,
        "semantic_profile":request.semantic_profile,
        "disposition":disposition,
        "required_study_order":request.required_study_order,
        "required_modality_order":request.required_modality_order,
        "node_order":node_order,
        "planned_node_order":planned,
        "selected_node_order":selected_order,
        "unresolved_node_order":unresolved_order,
        "blocked_node_order":blocked_order,
        "cycle_order":cycle,
        "missing_dependency_order":missing_dependency,
        "missing_study_order":missing_study,
        "missing_modality_order":missing_modality,
        "budget_exceeded_order":budget_exceeded,
        "evidence_order":evidence,
        "omission_order":omissions,
        "uncertainty_order":uncertainty,
        "negative_evidence_order":negative,
        "consumed_units":consumed_units,
        "budget_units":request.budget_units,
        "checkpoint_digest":checkpoint_digest,
        "replay_identity":request.replay_identity,
        "raw_data_local":true,
        "aggregate_only":true,
        "boundary":PRECLINICAL_BOUNDARY,
    });
    let execution_digest = ContentHash::of_value(&payload)
        .map_err(|error| MultimodalExecutionAssuranceError::Report(error.to_string()))?;
    let effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    let report = WorldgenExecutionRun7 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        required_study_order: request.required_study_order.clone(),
        required_modality_order: request.required_modality_order.clone(),
        node_order,
        planned_node_order: serde_json::from_value(payload["planned_node_order"].clone()).unwrap(),
        selected_node_order: selected_order,
        unresolved_node_order: unresolved_order,
        blocked_node_order: blocked_order,
        cycle_order: serde_json::from_value(payload["cycle_order"].clone()).unwrap(),
        missing_dependency_order: serde_json::from_value(
            payload["missing_dependency_order"].clone(),
        )
        .unwrap(),
        missing_study_order: serde_json::from_value(payload["missing_study_order"].clone())
            .unwrap(),
        missing_modality_order: serde_json::from_value(payload["missing_modality_order"].clone())
            .unwrap(),
        budget_exceeded_order: serde_json::from_value(payload["budget_exceeded_order"].clone())
            .unwrap(),
        evidence_order: serde_json::from_value(payload["evidence_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        consumed_units,
        budget_units: request.budget_units,
        checkpoint_digest,
        replay_identity: request.replay_identity.clone(),
        execution_digest: execution_digest.clone(),
        artifact: WorldgenExecutionRun7Artifact {
            artifact_id: format!("worldgen-execution-run-7:{}", request.workflow_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: execution_digest,
            semantic_loss: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_order,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn node(id: &str, modality: &str, depends_on: Vec<&str>) -> WorldgenExecutionNode6 {
        WorldgenExecutionNode6 {
            node_id: id.into(),
            study_id: "study:a".into(),
            modality: modality.into(),
            depends_on: depends_on.into_iter().map(String::from).collect(),
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("prov:{id}")),
            evidence_state: ExecutionEvidenceState::Supported,
            estimated_units: 3,
            local: true,
            aggregate_only: true,
        }
    }

    fn request() -> WorldgenMultimodalExecutionRequest8 {
        WorldgenMultimodalExecutionRequest8 {
            request_id: "request:execution".into(),
            workflow_id: "workflow:multimodal".into(),
            purpose: "verify".into(),
            semantic_profile: "ome-v1".into(),
            required_study_order: vec!["study:a".into()],
            required_modality_order: vec!["imaging".into(), "rna".into()],
            nodes: vec![
                node("node:b", "rna", vec!["node:a"]),
                node("node:a", "imaging", vec![]),
            ],
            replay_identity: hash("replay"),
            budget_units: 10,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_execution_assurance_manifest()["autonomy_tier"],
            "A1"
        );
    }
    #[test]
    fn complete_graph_is_qualified() {
        assert_eq!(
            assure_worldgen_multimodal_execution(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn cycle_is_blocked() {
        let mut q = request();
        q.nodes[1].depends_on = vec!["node:b".into()];
        assert_eq!(
            assure_worldgen_multimodal_execution(&q)
                .unwrap()
                .disposition,
            "blocked"
        );
    }
    #[test]
    fn unknown_evidence_is_unresolved() {
        let mut q = request();
        q.nodes[0].evidence_state = ExecutionEvidenceState::Unknown;
        assert_eq!(
            assure_worldgen_multimodal_execution(&q)
                .unwrap()
                .disposition,
            "unresolved"
        );
    }
    #[test]
    fn federation_denial_is_blocked() {
        let mut q = request();
        q.federation_approved = false;
        assert_eq!(
            assure_worldgen_multimodal_execution(&q)
                .unwrap()
                .disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = assure_worldgen_multimodal_execution(&request()).unwrap();
        let b = assure_worldgen_multimodal_execution(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
