//! Multimodal provenance and signing assurance (`AFA-ids-P18-F26`).
//!
//! This verifier checks a caller-supplied local provenance DAG and emits a
//! deterministic digest-only receipt. It never signs bytes, exports raw data,
//! or turns an incomplete lineage graph into a publication claim.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P18-F26";
pub const CONTRACT_VERSION: &str = "ids-multimodal-provenance-signing-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ProvenanceBundleRequest7@1";
pub const OUTPUT_SCHEMA: &str = "SignedProvenanceReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.signed-provenance-receipt-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_NODES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceNode8 {
    pub node_id: String,
    pub kind: String,
    pub parent_ids: Vec<String>,
    pub content_digest: ContentHash,
    pub actor: String,
    pub evidence_state: ProvenanceEvidenceState,
    pub local: bool,
    pub protected: bool,
    pub signature_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceBundleRequest7 {
    pub request_id: String,
    pub artifact_id: String,
    pub semantic_profile: String,
    pub nodes: Vec<ProvenanceNode8>,
    pub expected_root: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceReceipt9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub artifact_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub node_order: Vec<String>,
    pub verified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_parent_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub invalid_signature_order: Vec<String>,
    pub root_mismatch_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub root_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signature_mode: String,
    pub receipt_digest: ContentHash,
    pub artifact: SignedProvenanceReceipt9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProvenanceSigningError {
    #[error("invalid provenance/signing request: {0}")]
    Invalid(String),
    #[error("provenance/signing receipt failed validation: {0}")]
    Receipt(String),
}

pub fn provenance_signing_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["research-object steward", "provenance auditor", "federation operator", "replay auditor"],
        "behavior": "verifies local multimodal provenance DAGs, signatures, roots, and semantic closure",
        "value": "prevents missing lineage, cycles, invalid signatures, or root drift from becoming reproducible research-object claims",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:provenance-digests", "manage:local-capability"],
        "permissions": ["read:local-provenance", "request:provenance-assurance"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl SignedProvenanceReceipt9 {
    pub fn validate(&self) -> Result<(), ProvenanceSigningError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.artifact_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.signature_mode != "detached-digest-attestation"
            || self.node_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ProvenanceSigningError::Receipt(
                "identity, locality, signature mode, nodes, disposition, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.node_order,
            &self.verified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_parent_order,
            &self.cycle_order,
            &self.invalid_signature_order,
            &self.root_mismatch_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ProvenanceSigningError::Receipt(
                    "provenance receipt ordering is not canonical".into(),
                ));
            }
        }
        let nodes = BTreeSet::from_iter(self.node_order.iter().cloned());
        let states = self
            .verified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if nodes.len() != self.node_order.len()
            || states.len() != nodes.len()
            || BTreeSet::from_iter(states.iter().cloned()) != nodes
        {
            return Err(ProvenanceSigningError::Receipt(
                "provenance node states do not partition the node order".into(),
            ));
        }
        if !valid_digest(&self.root_digest)
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.receipt_digest)
            || self.artifact.content_hash != self.receipt_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(ProvenanceSigningError::Receipt(
                "provenance digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:provenance-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ProvenanceSigningError::Receipt(
                "effect is outside the governed provenance gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ProvenanceSigningError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ProvenanceSigningError::Receipt(error.to_string()))?,
        )
        .map_err(|error| ProvenanceSigningError::Receipt(error.to_string()))
    }
}

fn validate_request(request: &ProvenanceBundleRequest7) -> Result<(), ProvenanceSigningError> {
    if request.request_id.trim().is_empty()
        || request.artifact_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.nodes.is_empty()
        || request.nodes.len() > MAX_NODES
        || !valid_digest(&request.expected_root)
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(ProvenanceSigningError::Invalid(
            "request identity, nodes, bounds, roots, replay, or locality is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty()
            || node.kind.trim().is_empty()
            || node.actor.trim().is_empty()
            || !valid_digest(&node.content_digest)
            || !ids.insert(node.node_id.clone())
        {
            return Err(ProvenanceSigningError::Invalid(
                "node identity, actor, digest, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn assure_provenance_signing(
    request: &ProvenanceBundleRequest7,
) -> Result<SignedProvenanceReceipt9, ProvenanceSigningError> {
    validate_request(request)?;
    let mut nodes = request.nodes.clone();
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let node_order = nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let by_id = nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut missing_parent = BTreeSet::new();
    let mut invalid_signature = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for node in &nodes {
        for parent in &node.parent_ids {
            if !by_id.contains_key(parent) {
                missing_parent.insert(format!("{}:{}", node.node_id, parent));
            }
        }
        if !node.signature_valid {
            invalid_signature.insert(node.node_id.clone());
        }
        if node.evidence_state == ProvenanceEvidenceState::Contradicted {
            negative_evidence.insert(format!("{}:contradicted", node.node_id));
        } else if !matches!(
            node.evidence_state,
            ProvenanceEvidenceState::Proven | ProvenanceEvidenceState::Supported
        ) {
            uncertainty.insert(format!("{}:evidence-state", node.node_id));
        }
    }
    let mut indegree = nodes
        .iter()
        .map(|node| {
            (
                node.node_id.clone(),
                node.parent_ids
                    .iter()
                    .filter(|parent| by_id.contains_key(*parent))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in &nodes {
        for parent in &node.parent_ids {
            if by_id.contains_key(parent) {
                children
                    .entry(parent.clone())
                    .or_default()
                    .push(node.node_id.clone());
            }
        }
    }
    let mut queue = VecDeque::from_iter(
        indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone()),
    );
    let mut topo = Vec::new();
    while let Some(id) = queue.pop_front() {
        topo.push(id.clone());
        if let Some(next) = children.get(&id) {
            for child in next {
                let degree = indegree.get_mut(child).expect("child exists");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    let cycle = node_order
        .iter()
        .filter(|id| !topo.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut verified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut root_mismatch = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    for node in &nodes {
        let id = node.node_id.clone();
        if cycle.contains(&id) {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:provenance-cycle"));
        } else if node.content_digest == request.expected_root && node.parent_ids.is_empty() {
            // The root is still subject to the same signature and evidence gates.
            if !node.local || !node.protected || !node.signature_valid {
                blocked.insert(id.clone());
            } else if !matches!(
                node.evidence_state,
                ProvenanceEvidenceState::Proven | ProvenanceEvidenceState::Supported
            ) {
                unresolved.insert(id.clone());
            } else {
                verified.insert(id.clone());
            }
        } else if node
            .parent_ids
            .iter()
            .any(|parent| !by_id.contains_key(parent))
        {
            unresolved.insert(id.clone());
        } else if !node.local || !node.protected || !node.signature_valid {
            blocked.insert(id.clone());
        } else if !matches!(
            node.evidence_state,
            ProvenanceEvidenceState::Proven | ProvenanceEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
        } else {
            verified.insert(id.clone());
        }
        if node.content_digest == request.expected_root && !node.parent_ids.is_empty() {
            root_mismatch.insert(id.clone());
        }
    }
    if !node_order.iter().any(|id| {
        by_id[id].content_digest == request.expected_root && by_id[id].parent_ids.is_empty()
    }) {
        root_mismatch.insert(request.artifact_id.clone());
        omissions.insert("request:expected-root-not-found".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !root_mismatch.is_empty();
    if global_block {
        blocked.extend(node_order.iter().cloned());
        verified.clear();
        unresolved.clear();
        omissions.insert("request:provenance-governance-or-root-denied".into());
    }
    let verified_order = verified.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let disposition = if global_block || (verified_order.is_empty() && unresolved_order.is_empty())
    {
        "blocked"
    } else if !unresolved_order.is_empty() || !blocked_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:provenance-closure-incomplete".into());
    }
    let mut payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "artifact_id": request.artifact_id,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "node_order": node_order,
        "verified_order": verified_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_parent_order": missing_parent.iter().cloned().collect::<Vec<_>>(),
        "cycle_order": cycle.iter().cloned().collect::<Vec<_>>(),
        "invalid_signature_order": invalid_signature.iter().cloned().collect::<Vec<_>>(),
        "root_mismatch_order": root_mismatch.iter().cloned().collect::<Vec<_>>(),
        "omission_order": omissions.iter().cloned().collect::<Vec<_>>(),
        "uncertainty_order": uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order": negative_evidence.iter().cloned().collect::<Vec<_>>(),
        "root_digest": request.expected_root,
        "replay_identity": request.replay_identity,
        "signature_mode": "detached-digest-attestation",
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let receipt_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProvenanceSigningError::Receipt(error.to_string()))?;
    payload["receipt_digest"] = json!(receipt_digest);
    payload["artifact"] = json!({
        "artifact_id": format!("signed-provenance-receipt-9:{}", request.artifact_id),
        "content_type": CONTENT_TYPE,
        "content_hash": receipt_digest,
        "semantic_loss": omissions.iter().cloned().collect::<Vec<_>>(),
        "provenance_digests": nodes.iter().map(|node| node.content_digest.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
        "boundary": PRECLINICAL_BOUNDARY,
    });
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:provenance-digests:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: SignedProvenanceReceipt9 = serde_json::from_value(payload)
        .map_err(|error| ProvenanceSigningError::Receipt(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request(nodes: Vec<ProvenanceNode8>) -> ProvenanceBundleRequest7 {
        ProvenanceBundleRequest7 {
            request_id: "prov:req".into(),
            artifact_id: "artifact:1".into(),
            semantic_profile: "ome-ngff".into(),
            expected_root: hash("a"),
            replay_identity: hash("b"),
            nodes,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn node(id: &str, parents: Vec<&str>) -> ProvenanceNode8 {
        ProvenanceNode8 {
            node_id: id.into(),
            kind: "transform".into(),
            parent_ids: parents.into_iter().map(str::to_string).collect(),
            content_digest: hash(if id == "root" { "a" } else { id }),
            actor: "lab".into(),
            evidence_state: ProvenanceEvidenceState::Supported,
            local: true,
            protected: true,
            signature_valid: true,
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            provenance_signing_assurance_manifest()["autonomy_tier"],
            "A1"
        );
    }
    #[test]
    fn nominal_dag_is_qualified() {
        let r = assure_provenance_signing(&request(vec![
            node("root", vec![]),
            node("child", vec!["root"]),
        ]))
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.verified_order, vec!["child", "root"]);
    }
    #[test]
    fn missing_parent_is_unresolved() {
        let r = assure_provenance_signing(&request(vec![
            node("root", vec![]),
            node("child", vec!["missing"]),
        ]))
        .unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.missing_parent_order.is_empty());
    }
    #[test]
    fn cycle_is_blocked() {
        let r = assure_provenance_signing(&request(vec![
            node("root", vec!["child"]),
            node("child", vec!["root"]),
        ]))
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.cycle_order, vec!["child", "root"]);
    }
    #[test]
    fn invalid_signature_is_blocked() {
        let mut n = node("root", vec![]);
        n.signature_valid = false;
        let r = assure_provenance_signing(&request(vec![n])).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.invalid_signature_order, vec!["root"]);
    }
    #[test]
    fn root_drift_is_blocked() {
        let mut q = request(vec![node("root", vec![])]);
        q.expected_root = hash("z");
        let r = assure_provenance_signing(&q).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(!r.root_mismatch_order.is_empty());
    }
    #[test]
    fn policy_denial_has_no_exchange_effect() {
        let mut q = request(vec![node("root", vec![])]);
        q.policy_allow = false;
        let r = assure_provenance_signing(&q).unwrap();
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
}
