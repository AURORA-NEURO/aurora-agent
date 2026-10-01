//! World P32 frontier: validate causal closure, provenance lineage, and omission-safe replay.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-world-P32-F01";
pub const CONTRACT_VERSION: &str = "world-local-causal-integrity/1.0";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.world.causal-integrity-card-1+json";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalEdge4 {
    pub edge_id: String,
    pub cause_id: String,
    pub effect_id: String,
    pub relation: String,
    pub evidence_digest: ContentHash,
    pub policy_epoch: u64,
    pub local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalIntegrityRequest4 {
    pub request_id: String,
    pub purpose: String,
    pub edges: Vec<CausalEdge4>,
    pub required_node_order: Vec<String>,
    pub required_relation_order: Vec<String>,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_events: Vec<String>,
    pub action_budget: u32,
    pub action_count: u32,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub edge_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub edge_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub node_order: Vec<String>,
    pub relation_order: Vec<String>,
    pub epoch_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: CausalIntegrityArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CausalIntegrityError {
    #[error("invalid causal-integrity request: {0}")]
    Invalid(String),
    #[error("causal-integrity card failed validation: {0}")]
    Output(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered_digests(v: &[ContentHash]) -> bool {
    v.windows(2).all(|pair| pair[0] < pair[1])
}
fn nonempty(v: &str) -> bool {
    !v.trim().is_empty()
}

fn receipt_digest(value: &serde_json::Value) -> Result<ContentHash, CausalIntegrityError> {
    let mut basis = value.clone();
    let Some(fields) = basis.as_object_mut() else {
        return Err(CausalIntegrityError::Output(
            "causal receipt is not a JSON object".into(),
        ));
    };
    fields.remove("closure_digest");
    let Some(artifact) = fields
        .get_mut("artifact")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Err(CausalIntegrityError::Output(
            "causal receipt artifact is not a JSON object".into(),
        ));
    };
    artifact.remove("content_hash");
    ContentHash::of_value(&basis).map_err(|error| CausalIntegrityError::Output(error.to_string()))
}

/// Finds every edge in a strongly connected component; each such edge participates in a directed
/// cycle and cannot be part of a qualified causal closure.
fn cyclic_edge_ids(edges: &[CausalEdge4]) -> BTreeSet<String> {
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    let mut incoming = BTreeMap::<String, Vec<String>>::new();
    let mut nodes = BTreeSet::new();
    for edge in edges {
        nodes.insert(edge.cause_id.clone());
        nodes.insert(edge.effect_id.clone());
        outgoing
            .entry(edge.cause_id.clone())
            .or_default()
            .push(edge.effect_id.clone());
        incoming
            .entry(edge.effect_id.clone())
            .or_default()
            .push(edge.cause_id.clone());
    }
    for neighbors in outgoing.values_mut().chain(incoming.values_mut()) {
        neighbors.sort();
        neighbors.dedup();
    }

    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::with_capacity(nodes.len());
    for root in &nodes {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut stack = vec![(root.clone(), 0usize)];
        while !stack.is_empty() {
            let next = {
                let (node, next_index) = stack.last_mut().expect("nonempty DFS stack");
                let neighbors = outgoing.get(node).map(Vec::as_slice).unwrap_or_default();
                if *next_index < neighbors.len() {
                    let next = neighbors[*next_index].clone();
                    *next_index += 1;
                    Some(next)
                } else {
                    None
                }
            };
            if let Some(next) = next {
                if visited.insert(next.clone()) {
                    stack.push((next, 0));
                }
            } else {
                finish_order.push(stack.pop().expect("nonempty DFS stack").0);
            }
        }
    }

    let mut component_of = BTreeMap::new();
    let mut component_sizes = Vec::new();
    for root in finish_order.into_iter().rev() {
        if component_of.contains_key(&root) {
            continue;
        }
        let component_id = component_sizes.len();
        component_of.insert(root.clone(), component_id);
        let mut component_size = 0;
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            component_size += 1;
            for predecessor in incoming.get(&node).into_iter().flatten() {
                if !component_of.contains_key(predecessor) {
                    component_of.insert(predecessor.clone(), component_id);
                    stack.push(predecessor.clone());
                }
            }
        }
        component_sizes.push(component_size);
    }

    let mut cyclic_edges = BTreeSet::new();
    for edge in edges {
        let cause_component = component_of[&edge.cause_id];
        if component_sizes[cause_component] > 1 && cause_component == component_of[&edge.effect_id]
        {
            cyclic_edges.insert(edge.edge_id.clone());
        }
    }
    cyclic_edges
}

pub fn manifest(id: &str, version: &str, scale: &str, mode: &str) -> serde_json::Value {
    json!({"schema_version":"1.0.0","capability_id":id,"version":version,"owner_crate":"world","consumers":["causal compiler","mechanism explorer","provenance ledger","release auditor"],"behavior":format!("qualify causal-world closure at {scale} ({mode})"),"value":"prevents cyclic, stale, or silently incomplete causal explanations from entering a replayable research object","input_schema":"CausalIntegrityRequest4@1","output_schema":"CausalIntegrityCard7@1","effects":["emit:causal-card","retain:causal-omissions","block:unsafe-release"],"permissions":["read:local-causal-edges"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}
impl CausalIntegrityCard7 {
    pub fn validate(&self) -> Result<(), CausalIntegrityError> {
        if self.schema_version != "1.0.0"
            || !nonempty(&self.contract_version)
            || !nonempty(&self.feature_id)
            || !nonempty(&self.mode)
            || !nonempty(&self.scale)
            || !nonempty(&self.request_id)
            || !nonempty(&self.purpose)
            || !["qualified", "partial", "unknown", "blocked"].contains(&self.disposition.as_str())
            || self.boundary != BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.edge_order.is_empty()
            || !ordered(&self.edge_order)
            || !ordered(&self.accepted_order)
            || !ordered(&self.rejected_order)
            || !ordered(&self.unknown_order)
            || !ordered(&self.omitted_order)
            || !ordered(&self.node_order)
            || !ordered(&self.relation_order)
            || !ordered(&self.epoch_order)
            || !ordered(&self.negative_evidence_order)
            || !digest(&self.replay_identity)
            || !digest(&self.closure_digest)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.artifact_id != format!("world-causal:{}", self.request_id)
            || self.artifact.semantic_loss != self.omitted_order
            || !ordered_digests(&self.artifact.edge_digests)
            || !self.artifact.edge_digests.iter().all(digest)
            || self.artifact.content_hash != self.closure_digest
            || self.artifact.boundary != BOUNDARY
        {
            return Err(CausalIntegrityError::Output(
                "causal identity, ordering, locality, digest, or artifact is invalid".into(),
            ));
        }
        let value = serde_json::to_value(self)
            .map_err(|error| CausalIntegrityError::Output(error.to_string()))?;
        if receipt_digest(&value)? != self.closure_digest {
            return Err(CausalIntegrityError::Output(
                "causal receipt digest does not match its contents".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.edge_order.iter().cloned());
        let states = self
            .accepted_order
            .iter()
            .chain(&self.rejected_order)
            .chain(&self.unknown_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.edge_order.len()
            || states.len() != ids.len()
            || BTreeSet::from_iter(states) != ids
        {
            return Err(CausalIntegrityError::Output(
                "edge states do not partition".into(),
            ));
        }
        let expected_effects = if self.disposition == "qualified" {
            vec![format!("emit:causal-card:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".to_string()]
        };
        if self.effect_receipts != expected_effects
            || (self.disposition == "qualified"
                && (self.accepted_order != self.edge_order
                    || !self.rejected_order.is_empty()
                    || !self.unknown_order.is_empty()
                    || !self.omitted_order.is_empty()))
            || (self.disposition == "blocked"
                && (self.omitted_order != self.edge_order
                    || !self.accepted_order.is_empty()
                    || !self.rejected_order.is_empty()
                    || !self.unknown_order.is_empty()))
        {
            return Err(CausalIntegrityError::Output(
                "causal disposition and effect receipt disagree".into(),
            ));
        }
        Ok(())
    }
}
pub fn qualify(
    request: &CausalIntegrityRequest4,
    id: &str,
    version: &str,
    scale: &str,
    mode: &str,
) -> Result<CausalIntegrityCard7, CausalIntegrityError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.purpose)
        || request.edges.is_empty()
        || request.required_node_order.is_empty()
        || request.required_relation_order.is_empty()
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !ordered(&request.required_node_order)
        || !ordered(&request.required_relation_order)
        || !ordered(&request.adversarial_events)
    {
        return Err(CausalIntegrityError::Invalid(
            "causal identity, requirements, digest, ordering, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut rows = request.edges.clone();
    rows.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut accepted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    let mut relations = BTreeSet::new();
    let mut epochs = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for e in &rows {
        if !seen.insert(e.edge_id.clone())
            || !nonempty(&e.edge_id)
            || !nonempty(&e.cause_id)
            || !nonempty(&e.effect_id)
            || e.cause_id == e.effect_id
            || !nonempty(&e.relation)
            || !digest(&e.evidence_digest)
            || !e.local
            || !e.aggregate_only
        {
            return Err(CausalIntegrityError::Invalid(
                "edge identity, endpoints, evidence, or locality is invalid".into(),
            ));
        }
        order.push(e.edge_id.clone());
        nodes.insert(e.cause_id.clone());
        nodes.insert(e.effect_id.clone());
        relations.insert(e.relation.clone());
        epochs.insert(format!("{}:{}", e.relation, e.policy_epoch));
        digests.insert(e.evidence_digest.clone());
        if e.negative_result {
            negative.insert(format!("{}:negative-result", e.edge_id));
        }
        if e.policy_epoch == 0 {
            unknown.insert(e.edge_id.clone());
        } else if !request.required_relation_order.contains(&e.relation)
            || !request.required_node_order.contains(&e.cause_id)
            || !request.required_node_order.contains(&e.effect_id)
        {
            rejected.insert(e.edge_id.clone());
        } else if e.evidence_digest == request.replay_identity {
            omitted.insert(e.edge_id.clone());
        } else {
            accepted.insert(e.edge_id.clone());
        }
    }
    for edge_id in cyclic_edge_ids(&rows) {
        accepted.remove(&edge_id);
        rejected.insert(edge_id.clone());
        unknown.remove(&edge_id);
        omitted.remove(&edge_id);
        negative.insert(format!("{edge_id}:causal-cycle"));
    }
    let global = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || request.action_count > request.action_budget;
    if global {
        omitted.extend(order.iter().cloned());
        accepted.clear();
        rejected.clear();
        unknown.clear();
    }
    let missing = !request
        .required_node_order
        .iter()
        .all(|n| nodes.contains(n))
        || !request
            .required_relation_order
            .iter()
            .all(|r| relations.contains(r));
    let disposition = if global {
        "blocked"
    } else if missing {
        "unknown"
    } else if !rejected.is_empty() || !unknown.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let mut payload = json!({"schema_version":"1.0.0","contract_version":version,"feature_id":id,"mode":mode,"scale":scale,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"edge_order":order,"accepted_order":accepted.into_iter().collect::<Vec<_>>(),"rejected_order":rejected.into_iter().collect::<Vec<_>>(),"unknown_order":unknown.into_iter().collect::<Vec<_>>(),"omitted_order":omitted.into_iter().collect::<Vec<_>>(),"node_order":nodes.into_iter().collect::<Vec<_>>(),"relation_order":relations.into_iter().collect::<Vec<_>>(),"epoch_order":epochs.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    payload["artifact"] = json!({"artifact_id":format!("world-causal:{}",request.request_id),"content_type":CONTENT_TYPE,"semantic_loss":payload["omitted_order"],"edge_digests":digests.into_iter().collect::<Vec<_>>(),"boundary":BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("emit:causal-card:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let hash = receipt_digest(&payload)?;
    payload["closure_digest"] = json!(hash);
    payload["artifact"]["content_hash"] = json!(hash);
    let out: CausalIntegrityCard7 =
        serde_json::from_value(payload).map_err(|e| CausalIntegrityError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn edge(id: &str, cause: &str, effect: &str) -> CausalEdge4 {
        CausalEdge4 {
            edge_id: id.into(),
            cause_id: cause.into(),
            effect_id: effect.into(),
            relation: "causes".into(),
            evidence_digest: hash(&format!("evidence:{id}")),
            policy_epoch: 1,
            local: true,
            aggregate_only: true,
            negative_result: false,
        }
    }

    fn request(edges: Vec<CausalEdge4>) -> CausalIntegrityRequest4 {
        CausalIntegrityRequest4 {
            request_id: "request:causal-integrity".into(),
            purpose: "verify a bounded causal closure".into(),
            edges,
            required_node_order: vec!["node:a".into(), "node:b".into()],
            required_relation_order: vec!["causes".into()],
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash("replay"),
            adversarial_events: Vec::new(),
            action_budget: 1,
            action_count: 0,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn a_acyclic_complete_closure_qualifies_with_a_replayable_receipt() {
        let result = qualify(
            &request(vec![edge("edge:a-b", "node:a", "node:b")]),
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("valid DAG qualifies");

        assert_eq!(result.disposition, "qualified");
        assert_eq!(result.accepted_order, vec!["edge:a-b"]);
        assert_eq!(
            result.effect_receipts,
            vec!["emit:causal-card:request:causal-integrity"]
        );
        result.validate().expect("receipt is internally consistent");
    }

    #[test]
    fn every_edge_in_a_causal_cycle_is_rejected_and_release_is_blocked() {
        let result = qualify(
            &request(vec![
                edge("edge:a-b", "node:a", "node:b"),
                edge("edge:b-a", "node:b", "node:a"),
            ]),
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("cycle is reported as a non-qualified receipt");

        assert_eq!(result.disposition, "partial");
        assert_eq!(result.accepted_order, Vec::<String>::new());
        assert_eq!(result.rejected_order, vec!["edge:a-b", "edge:b-a"]);
        assert_eq!(
            result.negative_evidence_order,
            vec!["edge:a-b:causal-cycle", "edge:b-a:causal-cycle"]
        );
        assert_eq!(result.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn every_disjoint_cyclic_component_is_found_without_rejecting_a_dag_component() {
        let mut request = request(vec![
            edge("edge:a-b", "node:a", "node:b"),
            edge("edge:b-a", "node:b", "node:a"),
            edge("edge:c-d", "node:c", "node:d"),
            edge("edge:d-c", "node:d", "node:c"),
            edge("edge:e-f", "node:e", "node:f"),
        ]);
        request.required_node_order = ["node:a", "node:b", "node:c", "node:d", "node:e", "node:f"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let result = qualify(
            &request,
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("all cycle components produce a non-qualified receipt");

        assert_eq!(result.disposition, "partial");
        assert_eq!(result.accepted_order, vec!["edge:e-f"]);
        assert_eq!(
            result.rejected_order,
            vec!["edge:a-b", "edge:b-a", "edge:c-d", "edge:d-c"]
        );
    }

    #[test]
    fn a_missing_required_node_is_unknown_and_never_qualified() {
        let mut request = request(vec![edge("edge:a-b", "node:a", "node:b")]);
        request.required_node_order.push("node:c".into());

        let result = qualify(
            &request,
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("incomplete closure is reported");

        assert_eq!(result.disposition, "unknown");
        assert_eq!(result.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn policy_denial_clears_accepted_edges_before_any_release_effect() {
        let mut request = request(vec![edge("edge:a-b", "node:a", "node:b")]);
        request.policy_allowed = false;

        let result = qualify(
            &request,
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("policy hold is a typed blocked receipt");

        assert_eq!(result.disposition, "blocked");
        assert!(result.accepted_order.is_empty());
        assert_eq!(result.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn a_receipt_digest_binds_context_and_provenance_fields() {
        let receipt = qualify(
            &request(vec![edge("edge:a-b", "node:a", "node:b")]),
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .expect("valid DAG qualifies");

        let mut changed_context = receipt.clone();
        changed_context.purpose.push_str(" after review");
        assert!(changed_context.validate().is_err());

        let mut changed_provenance = receipt;
        changed_provenance.artifact.edge_digests[0] = hash("replacement evidence");
        assert!(changed_provenance.validate().is_err());
    }
}
