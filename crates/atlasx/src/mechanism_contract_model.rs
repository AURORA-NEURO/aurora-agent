//! Federated continual mechanism-exploration contract model (`AFA-atlasx-P08-F08`).
//!
//! This typed data primitive normalizes competing mechanism summaries and peer attestations into
//! a digest-addressed portfolio. It deliberately models evidence and compatibility; it does not
//! infer biology, execute experiments, or turn an unknown mechanism into a conclusion.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlasx-P08-F08";
pub const CONTRACT_VERSION: &str = "atlasx-federated-continual-mechanism-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "MechanismQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "MechanismPortfolio2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlasx-mechanism-portfolio-2+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CANDIDATES: usize = 4096;
pub const MAX_PEERS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasxMechanismCandidate5 {
    pub mechanism_id: String,
    pub study_id: String,
    pub modality: String,
    pub statement: String,
    pub support_milli: u32,
    pub novelty_milli: u32,
    pub evidence_state: MechanismEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasxMechanismPeer5 {
    pub peer_id: String,
    pub mechanism_id: String,
    pub semantic_profile: String,
    pub evidence_state: MechanismEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub authorized: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasxMechanismQuestion4 {
    pub request_id: String,
    pub question_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_mechanism_order: Vec<String>,
    pub candidates: Vec<AtlasxMechanismCandidate5>,
    pub peers: Vec<AtlasxMechanismPeer5>,
    pub replay_identity: ContentHash,
    pub minimum_peer_quorum: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasxMechanismPortfolio2Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasxMechanismPortfolio2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub question_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_mechanism_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub selected_mechanism_order: Vec<String>,
    pub unresolved_mechanism_order: Vec<String>,
    pub blocked_mechanism_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub support_milli_order: Vec<u32>,
    pub novelty_milli_order: Vec<u32>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub artifact: AtlasxMechanismPortfolio2Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismContractModelError {
    #[error("invalid atlasx mechanism contract request: {0}")]
    Invalid(String),
    #[error("atlasx mechanism portfolio failed validation: {0}")]
    Report(String),
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

pub fn mechanism_contract_model_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID,
        "version":CONTRACT_VERSION, "owner_crate":"atlasx",
        "consumers":["imaging core scientist","mechanism scientist","federation steward"],
        "behavior":"normalize mechanism candidates and signed peer attestations into a typed, digest-addressed portfolio with deterministic compatibility and evidence states",
        "value":"keeps competing explanations, omissions, and negative evidence visible while enabling safe cross-institution exchange of minimal summaries",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA, "effects":[],
        "permissions":["read:local-research-artifacts"], "autonomy_tier":"A1", "boundary":PRECLINICAL_BOUNDARY
    })
}

impl AtlasxMechanismPortfolio2 {
    pub fn validate(&self) -> Result<(), MechanismContractModelError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || [
                &self.request_id,
                &self.question_id,
                &self.purpose,
                &self.semantic_profile,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
            || self.required_mechanism_order.is_empty()
            || self.mechanism_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MechanismContractModelError::Report("mechanism identity, requirements, candidates, peers, effects, locality, or disposition is incomplete".into()));
        }
        for values in [
            &self.required_mechanism_order,
            &self.mechanism_order,
            &self.selected_mechanism_order,
            &self.unresolved_mechanism_order,
            &self.blocked_mechanism_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(MechanismContractModelError::Report(
                    "mechanism ordering is not canonical".into(),
                ));
            }
        }
        let mids = BTreeSet::from_iter(self.mechanism_order.iter().cloned());
        let parts = self
            .selected_mechanism_order
            .iter()
            .chain(&self.unresolved_mechanism_order)
            .chain(&self.blocked_mechanism_order)
            .cloned()
            .collect::<Vec<_>>();
        let pids = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let pparts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if mids.len() != self.mechanism_order.len()
            || parts.len() != mids.len()
            || BTreeSet::from_iter(parts) != mids
            || pids.len() != self.peer_order.len()
            || pparts.len() != pids.len()
            || BTreeSet::from_iter(pparts) != pids
            || self.support_milli_order.len() != self.mechanism_order.len()
            || self.novelty_milli_order.len() != self.mechanism_order.len()
        {
            return Err(MechanismContractModelError::Report(
                "mechanism or peer states do not partition".into(),
            ));
        }
        if [
            &self.replay_identity,
            &self.portfolio_digest,
            &self.artifact.content_hash,
        ]
        .iter()
        .any(|d| !valid_digest(d))
            || self.artifact.content_hash != self.portfolio_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(MechanismContractModelError::Report(
                "mechanism digest or artifact metadata is invalid".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("view:atlasx-mechanism-portfolio:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(MechanismContractModelError::Report(
                "effect is outside the atlasx mechanism contract gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MechanismContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| MechanismContractModelError::Report(e.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|e| MechanismContractModelError::Report(e.to_string()))
    }
}

fn validate_request(request: &AtlasxMechanismQuestion4) -> Result<(), MechanismContractModelError> {
    if [
        &request.request_id,
        &request.question_id,
        &request.purpose,
        &request.semantic_profile,
    ]
    .iter()
    .any(|v| v.trim().is_empty())
        || request.required_mechanism_order.is_empty()
        || request.candidates.is_empty()
        || request.peers.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.peers.len() > MAX_PEERS
        || request.minimum_peer_quorum == 0
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(MechanismContractModelError::Invalid(
            "mechanism identity, closure, bounds, quorum, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    for values in [&request.required_mechanism_order] {
        let set = BTreeSet::from_iter(values.iter().cloned());
        if set.len() != values.len() || values.iter().any(|v| v.trim().is_empty()) {
            return Err(MechanismContractModelError::Invalid(
                "required mechanisms must be unique and non-empty".into(),
            ));
        }
    }
    let mut mids = BTreeSet::new();
    for c in &request.candidates {
        if c.mechanism_id.trim().is_empty()
            || !mids.insert(c.mechanism_id.clone())
            || c.study_id.trim().is_empty()
            || c.modality.trim().is_empty()
            || c.statement.trim().is_empty()
            || c.support_milli > 1000
            || c.novelty_milli > 1000
            || !valid_digest(&c.artifact_digest)
            || !valid_digest(&c.provenance_digest)
            || !valid_digest(&c.replay_identity)
            || !c.local
            || !c.aggregate_only
        {
            return Err(MechanismContractModelError::Invalid(format!(
                "candidate {} is invalid, duplicated, non-local, or not digest-bound",
                c.mechanism_id
            )));
        }
    }
    let mut pids = BTreeSet::new();
    for p in &request.peers {
        if p.peer_id.trim().is_empty()
            || !pids.insert(p.peer_id.clone())
            || p.mechanism_id.trim().is_empty()
            || p.semantic_profile.trim().is_empty()
            || !valid_digest(&p.artifact_digest)
            || !valid_digest(&p.provenance_digest)
            || !valid_digest(&p.replay_identity)
            || !p.local
            || !p.aggregate_only
        {
            return Err(MechanismContractModelError::Invalid(format!(
                "peer {} is invalid, duplicated, non-local, or not digest-bound",
                p.peer_id
            )));
        }
    }
    Ok(())
}

pub fn admit_atlasx_mechanism_contract(
    request: &AtlasxMechanismQuestion4,
) -> Result<AtlasxMechanismPortfolio2, MechanismContractModelError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.mechanism_id.cmp(&b.mechanism_id));
    let mechanism_order = candidates
        .iter()
        .map(|c| c.mechanism_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let qualified_peer_order = peers
        .iter()
        .filter(|p| {
            p.semantic_profile == request.semantic_profile
                && p.replay_identity == request.replay_identity
                && p.authorized
                && matches!(
                    p.evidence_state,
                    MechanismEvidenceState::Proven | MechanismEvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<Vec<_>>();
    let qualified_set = BTreeSet::from_iter(qualified_peer_order.iter().cloned());
    let missing_peer_order = peer_order
        .iter()
        .filter(|p| !qualified_set.contains(*p))
        .cloned()
        .collect::<Vec<_>>();
    let quorum_ok = qualified_peer_order.len() as u32 >= request.minimum_peer_quorum;
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for c in &candidates {
        match c.evidence_state {
            MechanismEvidenceState::Contradicted => {
                blocked.insert(c.mechanism_id.clone());
                contradiction.insert(c.mechanism_id.clone());
                negative.insert(format!("{}:contradicted", c.mechanism_id));
            }
            MechanismEvidenceState::Unknown | MechanismEvidenceState::Unmeasured => {
                unresolved.insert(c.mechanism_id.clone());
                evidence.insert(c.mechanism_id.clone());
                uncertainty.insert(format!("{}:evidence-state", c.mechanism_id));
            }
            MechanismEvidenceState::Proven | MechanismEvidenceState::Supported => {
                selected.insert(c.mechanism_id.clone());
            }
        }
    }
    if !quorum_ok {
        for id in selected.clone() {
            selected.remove(&id);
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:peer-quorum"));
        }
        omissions.insert("request:peer-quorum-not-met".into());
    }
    let candidate_set = BTreeSet::from_iter(mechanism_order.iter().cloned());
    let required_missing = request
        .required_mechanism_order
        .iter()
        .any(|id| !candidate_set.contains(id));
    for id in &request.required_mechanism_order {
        if !candidate_set.contains(id) {
            unresolved.insert(id.clone());
            omissions.insert(format!("mechanism:{id}:missing"));
            negative.insert(format!("mechanism:{id}:no-candidate"));
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(mechanism_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved
        .into_iter()
        .filter(|id| candidate_set.contains(id))
        .collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global_block || !blocked_order.is_empty() {
        "blocked"
    } else if required_missing || !unresolved_order.is_empty() || !missing_peer_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:mechanism-contract-not-closed".into());
    }
    let effect_order: Vec<String> = if disposition == "qualified" {
        vec![
            "manage:local-capability".into(),
            "view:atlasx-mechanism-portfolio".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let support_order = candidates
        .iter()
        .map(|c| c.support_milli)
        .collect::<Vec<_>>();
    let novelty_order = candidates
        .iter()
        .map(|c| c.novelty_milli)
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"question_id":request.question_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"required_mechanism_order":request.required_mechanism_order,"mechanism_order":mechanism_order,"selected_mechanism_order":selected_order,"unresolved_mechanism_order":unresolved_order,"blocked_mechanism_order":blocked_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"support_milli_order":support_order,"novelty_milli_order":novelty_order,"evidence_order":evidence,"omission_order":omissions,"uncertainty_order":uncertainty,"contradiction_order":contradiction,"negative_evidence_order":negative,"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let portfolio_digest = ContentHash::of_value(&payload)
        .map_err(|e| MechanismContractModelError::Report(e.to_string()))?;
    let effect_receipts = effect_order
        .iter()
        .map(|e| {
            if e == "block:unsafe-release" {
                e.clone()
            } else {
                format!("{e}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    let mut provenance_digests = candidates
        .iter()
        .map(|c| c.provenance_digest.clone())
        .chain(peers.iter().map(|p| p.provenance_digest.clone()))
        .collect::<Vec<_>>();
    provenance_digests.sort();
    let report = AtlasxMechanismPortfolio2 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        question_id: request.question_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        required_mechanism_order: request.required_mechanism_order.clone(),
        mechanism_order: serde_json::from_value(payload["mechanism_order"].clone()).unwrap(),
        selected_mechanism_order: serde_json::from_value(
            payload["selected_mechanism_order"].clone(),
        )
        .unwrap(),
        unresolved_mechanism_order: serde_json::from_value(
            payload["unresolved_mechanism_order"].clone(),
        )
        .unwrap(),
        blocked_mechanism_order: serde_json::from_value(payload["blocked_mechanism_order"].clone())
            .unwrap(),
        peer_order: serde_json::from_value(payload["peer_order"].clone()).unwrap(),
        qualified_peer_order: serde_json::from_value(payload["qualified_peer_order"].clone())
            .unwrap(),
        missing_peer_order: serde_json::from_value(payload["missing_peer_order"].clone()).unwrap(),
        support_milli_order: support_order,
        novelty_milli_order: novelty_order,
        evidence_order: serde_json::from_value(payload["evidence_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        contradiction_order: serde_json::from_value(payload["contradiction_order"].clone())
            .unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        effect_order,
        replay_identity: request.replay_identity.clone(),
        portfolio_digest: portfolio_digest.clone(),
        artifact: AtlasxMechanismPortfolio2Artifact {
            artifact_id: format!("atlasx-mechanism-portfolio-2:{}", request.question_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: portfolio_digest,
            semantic_loss: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
            provenance_digests,
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
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
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn candidate(id: &str, state: MechanismEvidenceState) -> AtlasxMechanismCandidate5 {
        AtlasxMechanismCandidate5 {
            mechanism_id: id.into(),
            study_id: "study:a".into(),
            modality: "imaging".into(),
            statement: format!("{id} mechanism"),
            support_milli: 800,
            novelty_milli: 700,
            evidence_state: state,
            artifact_digest: h(id),
            provenance_digest: h(&format!("p{id}")),
            replay_identity: h("replay"),
            local: true,
            aggregate_only: true,
        }
    }
    fn question() -> AtlasxMechanismQuestion4 {
        AtlasxMechanismQuestion4 {
            request_id: "request:atlasx".into(),
            question_id: "question:mechanism".into(),
            purpose: "compare".into(),
            semantic_profile: "ome-v1".into(),
            required_mechanism_order: vec!["m:a".into()],
            candidates: vec![candidate("m:a", MechanismEvidenceState::Supported)],
            peers: vec![AtlasxMechanismPeer5 {
                peer_id: "peer:a".into(),
                mechanism_id: "m:a".into(),
                semantic_profile: "ome-v1".into(),
                evidence_state: MechanismEvidenceState::Supported,
                artifact_digest: h("peer"),
                provenance_digest: h("peer-prov"),
                replay_identity: h("replay"),
                authorized: true,
                local: true,
                aggregate_only: true,
            }],
            replay_identity: h("replay"),
            minimum_peer_quorum: 1,
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
        assert_eq!(mechanism_contract_model_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn complete_portfolio_is_qualified() {
        assert_eq!(
            admit_atlasx_mechanism_contract(&question())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut q = question();
        q.candidates[0].evidence_state = MechanismEvidenceState::Unknown;
        assert_eq!(
            admit_atlasx_mechanism_contract(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn contradicted_is_blocked() {
        let mut q = question();
        q.candidates[0].evidence_state = MechanismEvidenceState::Contradicted;
        assert_eq!(
            admit_atlasx_mechanism_contract(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn quorum_is_unresolved() {
        let mut q = question();
        q.minimum_peer_quorum = 2;
        assert_eq!(
            admit_atlasx_mechanism_contract(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = admit_atlasx_mechanism_contract(&question()).unwrap();
        let b = admit_atlasx_mechanism_contract(&question()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
