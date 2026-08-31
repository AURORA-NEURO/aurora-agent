//! Federated continual retrieval/synthesis workflow fabric (`AFA-backends-P02-F16`).
//!
//! The fabric coordinates digest-only peer summaries locally. It is deliberately a planner and
//! verifier: it does not retrieve source documents, execute backends, or export raw observations.
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-backends-P02-F16";
pub const CONTRACT_VERSION: &str =
    "backends-federated-continual-retrieval-synthesis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "FederatedRetrievalSynthesisRequest6@1";
pub const OUTPUT_SCHEMA: &str = "FederatedRetrievalSynthesisRun8@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.backends-federated-retrieval-synthesis-run-8+json";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalPeer5 {
    pub peer_id: String,
    pub institution_id: String,
    pub candidate_order: Vec<String>,
    pub semantic_profile: String,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalSynthesisRequest6 {
    pub schema_version: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub required_peer_order: Vec<String>,
    pub peers: Vec<RetrievalPeer5>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_authorized: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedRetrievalDisposition {
    Qualified,
    Partial,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalSynthesisRun8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: FederatedRetrievalDisposition,
    pub stage_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_candidate_order: Vec<String>,
    pub unresolved_candidate_order: Vec<String>,
    pub blocked_candidate_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub workflow_digest: ContentHash,
    pub artifact: FederatedRetrievalArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedRetrievalError {
    #[error("invalid federated retrieval request: {0}")]
    Invalid(String),
    #[error("federated retrieval artifact failed: {0}")]
    Artifact(String),
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn federated_retrieval_synthesis_workflow_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "backends".into(),
        consumers: ["federated workflow operator".into(), "retrieval synthesis steward".into(), "backend planner".into()].into(),
        behavior: "coordinate digest-only peer retrieval and synthesis summaries through a checkpointed continual workflow fabric".into(),
        value: "makes federation closure, peer quorum, omissions, replay, provenance, and negative evidence auditable before any backend executes".into(),
        inputs: vec![TypedPort { name: "federated_retrieval_synthesis_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "federated_retrieval_synthesis_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:federated-aggregate-summaries".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}
impl FederatedRetrievalSynthesisRun8 {
    pub fn validate(&self) -> Result<(), FederatedRetrievalError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.peer_order.is_empty()
            || self.stage_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalError::Invalid(
                "federated retrieval identity, axes, locality, stages, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.stage_order,
            &self.candidate_order,
            &self.selected_candidate_order,
            &self.unresolved_candidate_order,
            &self.blocked_candidate_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(FederatedRetrievalError::Invalid(
                    "federated retrieval ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_candidate_order
            .iter()
            .chain(&self.unresolved_candidate_order)
            .chain(&self.blocked_candidate_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.candidate_order.len() || parts != ids {
            return Err(FederatedRetrievalError::Invalid(
                "candidate states do not partition".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers.len() != self.peer_order.len() || pp != peers {
            return Err(FederatedRetrievalError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.workflow_digest)
            || self.artifact.content_hash != self.workflow_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(FederatedRetrievalError::Artifact(
                "federated retrieval digest is invalid".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(FederatedRetrievalError::Artifact(
                "federated retrieval content type is invalid".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            e != "block:unsafe-release" && !e.starts_with("coordinate:federated-retrieval:")
        }) {
            return Err(FederatedRetrievalError::Invalid(
                "effect is outside governed gate".into(),
            ));
        }
        if self.disposition == FederatedRetrievalDisposition::Qualified
            && self.effect_receipts
                != [format!(
                    "coordinate:federated-retrieval:{}",
                    self.request_id
                )]
        {
            return Err(FederatedRetrievalError::Invalid(
                "qualified effect is invalid".into(),
            ));
        }
        if self.disposition != FederatedRetrievalDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(FederatedRetrievalError::Invalid(
                "non-qualified run must block".into(),
            ));
        }
        Ok(())
    }
}
pub fn run_federated_retrieval_synthesis(
    request: &FederatedRetrievalSynthesisRequest6,
) -> Result<FederatedRetrievalSynthesisRun8, FederatedRetrievalError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_candidate_order.is_empty()
        || request.required_peer_order.is_empty()
        || request.peers.is_empty()
        || !canonical(&request.required_candidate_order)
        || !canonical(&request.required_peer_order)
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalError::Invalid(
            "request identity, closure, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut rows = request.peers.clone();
    rows.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = request
        .required_peer_order
        .iter()
        .cloned()
        .chain(rows.iter().map(|p| p.peer_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_order = request
        .required_candidate_order
        .iter()
        .cloned()
        .chain(rows.iter().flat_map(|p| p.candidate_order.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    for p in &rows {
        if p.peer_id.trim().is_empty()
            || p.institution_id.trim().is_empty()
            || p.semantic_profile.trim().is_empty()
            || !ids.insert(p.peer_id.clone())
            || !canonical(&p.candidate_order)
            || !canonical(&p.omission_order)
            || !digest(&p.evidence_digest)
            || !digest(&p.provenance_digest)
            || !digest(&p.replay_identity)
        {
            return Err(FederatedRetrievalError::Invalid(
                "peer identity, order, or digest is invalid".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for p in &rows {
        omission.extend(
            p.omission_order
                .iter()
                .map(|x| format!("{}:{x}", p.peer_id)),
        );
        if p.semantic_profile == request.semantic_profile
            && p.replay_identity == request.replay_identity
            && p.signed
            && p.permitted
            && p.local_only
            && p.aggregate_only
            && p.policy_allow
            && p.protected_closure
            && matches!(
                p.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            qualified.insert(p.peer_id.clone());
        } else {
            missing.insert(p.peer_id.clone());
            uncertainty.insert(format!("{}:peer-closure", p.peer_id));
        }
        if p.negative_result {
            negative.insert(format!("{}:negative-result", p.peer_id));
        }
    }
    for p in &request.required_peer_order {
        if !rows.iter().any(|x| &x.peer_id == p) {
            missing.insert(p.clone());
            omission.insert(format!("peer:{p}:missing"));
        }
    }
    let selected_candidates = rows
        .iter()
        .filter(|p| qualified.contains(&p.peer_id))
        .flat_map(|p| p.candidate_order.clone())
        .collect::<BTreeSet<_>>();
    let mut selected = selected_candidates.clone();
    let unresolved = candidate_order
        .iter()
        .filter(|c| !selected_candidates.contains(*c))
        .cloned()
        .collect::<BTreeSet<_>>();
    for c in &request.required_candidate_order {
        if !selected_candidates.contains(c) {
            omission.insert(format!("candidate:{c}:missing"));
            selected.remove(c);
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_authorized
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global {
        selected.clear();
        omission.insert("workflow:federation-gate-blocked".into());
    }
    let disposition = if global {
        FederatedRetrievalDisposition::Blocked
    } else if missing.len() > 0 || unresolved.len() > 0 || selected.is_empty() {
        FederatedRetrievalDisposition::Partial
    } else {
        FederatedRetrievalDisposition::Qualified
    };
    if disposition != FederatedRetrievalDisposition::Qualified {
        omission.insert("workflow:not-release-ready".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = if global {
        candidate_order.iter().cloned().collect()
    } else {
        Vec::new()
    };
    let stage_order: Vec<String> = vec![
        "stage:candidate-merge".into(),
        "stage:checkpoint".into(),
        "stage:peer-qualify".into(),
        "stage:seal-envelope".into(),
    ];
    let effect_receipts = if disposition == FederatedRetrievalDisposition::Qualified {
        vec![format!(
            "coordinate:federated-retrieval:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"stage_order":stage_order,"candidate_order":candidate_order,"selected_candidate_order":selected_order,"unresolved_candidate_order":unresolved_order,"blocked_candidate_order":blocked_order,"peer_order":peer_order,"qualified_peer_order":qualified,"missing_peer_order":missing,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"effect_receipts":effect_receipts,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let d = ContentHash::of_value(&payload)
        .map_err(|e| FederatedRetrievalError::Artifact(e.to_string()))?;
    let artifact = FederatedRetrievalArtifact8 {
        artifact_id: format!("backends-federated-retrieval:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: d.clone(),
        semantic_loss: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        provenance_digests: rows.iter().map(|p| p.provenance_digest.clone()).collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let out = FederatedRetrievalSynthesisRun8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        stage_order: payload["stage_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_candidate_order: payload["selected_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_candidate_order: payload["unresolved_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_candidate_order: payload["blocked_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        workflow_digest: d,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn p(id: &str) -> RetrievalPeer5 {
        RetrievalPeer5 {
            peer_id: id.into(),
            institution_id: format!("inst:{id}"),
            candidate_order: vec!["candidate:a".into()],
            semantic_profile: "retrieval:v1".into(),
            evidence_digest: h("evidence"),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            evidence_state: EvidenceState::Supported,
            signed: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            policy_allow: true,
            protected_closure: true,
            omission_order: Vec::new(),
            negative_result: false,
        }
    }
    fn q() -> FederatedRetrievalSynthesisRequest6 {
        FederatedRetrievalSynthesisRequest6 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req".into(),
            workflow_id: "wf".into(),
            federation_id: "fed".into(),
            requester: "operator".into(),
            purpose: "synthesis".into(),
            semantic_profile: "retrieval:v1".into(),
            required_candidate_order: vec!["candidate:a".into()],
            required_peer_order: vec!["peer:a".into()],
            peers: vec![p("peer:a")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_authorized: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_retrieval_synthesis_workflow_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            run_federated_retrieval_synthesis(&q()).unwrap().disposition,
            FederatedRetrievalDisposition::Qualified
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = q();
        r.policy_allow = false;
        assert_eq!(
            run_federated_retrieval_synthesis(&r).unwrap().disposition,
            FederatedRetrievalDisposition::Blocked
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            run_federated_retrieval_synthesis(&q()).unwrap(),
            run_federated_retrieval_synthesis(&q()).unwrap()
        )
    }
}
