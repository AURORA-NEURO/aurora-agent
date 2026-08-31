//! Federated continual mechanism-exploration assurance for `AFA-adaptive-P08-F28`.
//!
//! The harness ranks caller-declared mechanism candidates and peer attestations. It never
//! invents mechanisms, fits a model, or moves raw experimental data; every uncertainty and
//! competing explanation remains in the receipt.

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adaptive-P08-F28";
pub const CONTRACT_VERSION: &str =
    "adaptive-federated-continual-mechanism-exploration-assurance/1.0";
pub const INPUT_SCHEMA: &str = "MechanismQuestion6@1";
pub const OUTPUT_SCHEMA: &str = "MechanismAssuranceReceipt8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.mechanism-assurance-receipt-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate6 {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub support_milli: u32,
    pub novelty_milli: u32,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub independent_source: bool,
    pub local_data: bool,
    pub policy_allowed: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPeer5 {
    pub peer_id: String,
    pub origin: String,
    pub mechanism_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub support_milli: u32,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismQuestion6 {
    pub request_id: String,
    pub federation_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub support_threshold_milli: u32,
    pub candidates: Vec<MechanismCandidate6>,
    pub peers: Vec<MechanismPeer5>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismAssuranceArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismAssuranceReceipt8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub competing_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub assurance_digest: ContentHash,
    pub artifact: MechanismAssuranceArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismAssuranceError {
    #[error("invalid mechanism assurance request: {0}")]
    Invalid(String),
    #[error("mechanism assurance artifact failed: {0}")]
    Artifact(String),
}
pub fn mechanism_exploration_assurance_manifest() -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"adaptive","consumers":["mechanism scientist","adaptive-panel operator","federation steward"],"behavior":"ranks typed mechanism candidates and peer attestations under reproducibility and policy gates","value":"exposes competing explanations and missing evidence before an adaptive research decision","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["retain:mechanism-assurance","exchange:aggregate-mechanism-summary"],"permissions":["retain:mechanism-evidence","exchange:aggregate-mechanism"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}
impl MechanismAssuranceReceipt8 {
    pub fn validate(&self) -> Result<(), MechanismAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MechanismAssuranceError::Invalid("mechanism identity, checkpoint, locality, candidates, peers, or effects are incomplete".into()));
        }
        for v in [
            &self.candidate_order,
            &self.ranked_order,
            &self.selected_order,
            &self.competing_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if v.windows(2).any(|w| w[0] >= w[1]) {
                return Err(MechanismAssuranceError::Invalid(
                    "mechanism ordering is not canonical".into(),
                ));
            }
        }
        let all = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.competing_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_candidate_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all != parts || all.len() != self.candidate_order.len() {
            return Err(MechanismAssuranceError::Invalid(
                "candidate states do not partition".into(),
            ));
        }
        if self.ranked_order.len() != all.len()
            || self.ranked_order.iter().any(|x| !all.contains(x))
        {
            return Err(MechanismAssuranceError::Invalid(
                "ranking is not a candidate permutation".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != pp || peers.len() != self.peer_order.len() {
            return Err(MechanismAssuranceError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.assurance_digest
        {
            return Err(MechanismAssuranceError::Artifact(
                "mechanism artifact digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}
pub fn assure_mechanisms(
    q: &MechanismQuestion6,
) -> Result<MechanismAssuranceReceipt8, MechanismAssuranceError> {
    validate_request(q)?;
    let mut rows = q.candidates.clone();
    rows.sort_by(|a, b| {
        b.support_milli
            .cmp(&a.support_milli)
            .then(b.novelty_milli.cmp(&a.novelty_milli))
            .then(a.candidate_id.cmp(&b.candidate_id))
    });
    let candidate_order = rows
        .iter()
        .map(|r| r.candidate_id.clone())
        .collect::<Vec<_>>();
    let ranked_order = candidate_order.clone();
    let mut selected = BTreeSet::new();
    let mut competing = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut mods = BTreeSet::new();
    for r in &rows {
        studies.insert(r.study_id.clone());
        mods.insert(r.modality.clone());
        if r.negative_result {
            negative.insert(format!("{}:negative-result", r.candidate_id));
        }
        match r.evidence_state {
            EvidenceState::Contradicted => {
                blocked.insert(r.candidate_id.clone());
                contradiction.insert(format!("{}:contradicted", r.candidate_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unresolved.insert(r.candidate_id.clone());
                uncertainty.insert(format!("{}:evidence-state", r.candidate_id));
            }
            EvidenceState::Proven | EvidenceState::Supported
                if r.support_milli >= q.support_threshold_milli
                    && r.independent_source
                    && r.local_data
                    && r.policy_allowed =>
            {
                if selected.is_empty() {
                    selected.insert(r.candidate_id.clone());
                } else {
                    competing.insert(r.candidate_id.clone());
                }
            }
            EvidenceState::Proven | EvidenceState::Supported => {
                unresolved.insert(r.candidate_id.clone());
                uncertainty.insert(format!("{}:closure-or-threshold", r.candidate_id));
            }
        }
    }
    let missing_study = q
        .required_study_order
        .iter()
        .filter(|s| !studies.contains(*s))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_modality = q
        .required_modality_order
        .iter()
        .filter(|m| !mods.contains(*m))
        .cloned()
        .collect::<BTreeSet<_>>();
    for x in &missing_study {
        omissions.insert(format!("study:{}:missing", x));
    }
    for x in &missing_modality {
        omissions.insert(format!("modality:{}:missing", x));
    }
    let mut peers = q.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let qualified_peers = peers
        .iter()
        .filter(|p| {
            p.semantic_profile == q.semantic_profile
                && p.checkpoint == q.checkpoint
                && p.signed
                && p.aggregate_only
                && p.raw_data_local
                && matches!(
                    p.evidence_state,
                    EvidenceState::Proven | EvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|p| !qualified_peers.contains(*p))
        .cloned()
        .collect::<BTreeSet<_>>();
    for p in &missing_peers {
        uncertainty.insert(format!("peer:{}:not-qualified", p));
    }
    let global = !q.policy_allow
        || !q.protected_closure
        || !q.signed_approval
        || !q.federation_approved
        || !q.raw_data_local
        || !q.aggregate_only;
    if !q.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !q.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !q.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !q.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || !missing_study.is_empty()
        || !missing_modality.is_empty()
        || !unresolved.is_empty()
        || qualified_peers.len() < q.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:mechanism-not-release-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"federation_id":q.federation_id,"researcher":q.researcher,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"checkpoint":q.checkpoint,"disposition":disposition,"candidate_order":candidate_order,"ranked_order":ranked_order,"selected_order":selected,"competing_order":competing,"unresolved_order":unresolved,"blocked_order":blocked,"missing_candidate_order":[],"missing_study_order":missing_study,"missing_modality_order":missing_modality,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"contradiction_order":contradiction,"negative_evidence_order":negative,"replay_identity":q.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| MechanismAssuranceError::Artifact(e.to_string()))?;
    let artifact = MechanismAssuranceArtifact8 {
        artifact_id: format!("mechanism-assurance-receipt-8:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|r| r.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("retain:mechanism-assurance:{}", q.request_id),
            format!("exchange:aggregate-mechanism-summary:{}", q.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let r = MechanismAssuranceReceipt8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        federation_id: q.federation_id.clone(),
        researcher: q.researcher.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        checkpoint: q.checkpoint,
        disposition: disposition.into(),
        candidate_order: rows.iter().map(|x| x.candidate_id.clone()).collect(),
        ranked_order,
        selected_order: selected.into_iter().collect(),
        competing_order: competing.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        missing_candidate_order: Vec::new(),
        missing_study_order: missing_study.into_iter().collect(),
        missing_modality_order: missing_modality.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        assurance_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    r.validate()?;
    Ok(r)
}
fn validate_request(q: &MechanismQuestion6) -> Result<(), MechanismAssuranceError> {
    if ![
        &q.request_id,
        &q.federation_id,
        &q.researcher,
        &q.purpose,
        &q.semantic_profile,
    ]
    .iter()
    .all(|v| !v.trim().is_empty())
        || q.required_study_order.is_empty()
        || q.required_modality_order.is_empty()
        || q.candidates.is_empty()
        || q.peers.is_empty()
        || q.checkpoint == 0
        || q.minimum_peer_quorum == 0
        || !q.raw_data_local
        || !q.aggregate_only
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.replay_identity.as_str().len() != 64
    {
        return Err(MechanismAssuranceError::Invalid("mechanism identity, bounds, candidates, peers, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for c in &q.candidates {
        if c.candidate_id.trim().is_empty()
            || !ids.insert(c.candidate_id.clone())
            || c.mechanism_id.trim().is_empty()
            || c.study_id.trim().is_empty()
            || c.modality.trim().is_empty()
            || c.semantic_profile != q.semantic_profile
            || c.artifact_digest.as_str().len() != 64
            || c.provenance_digest.as_str().len() != 64
            || c.replay_identity != q.replay_identity
            || !c.local_data
        {
            return Err(MechanismAssuranceError::Invalid(
                "candidate identity, profile, digests, replay, or locality is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn q() -> MechanismQuestion6 {
        let r = h("replay");
        MechanismQuestion6 {
            request_id: "request:mechanism".into(),
            federation_id: "fed:mechanism".into(),
            researcher: "mechanism-scientist".into(),
            purpose: "explore-organoid-mechanism".into(),
            semantic_profile: "neuro:v1".into(),
            required_study_order: vec!["study:a".into()],
            required_modality_order: vec!["imaging".into()],
            support_threshold_milli: 700,
            candidates: vec![MechanismCandidate6 {
                candidate_id: "candidate:a".into(),
                mechanism_id: "mech:a".into(),
                study_id: "study:a".into(),
                modality: "imaging".into(),
                semantic_profile: "neuro:v1".into(),
                support_milli: 900,
                novelty_milli: 800,
                evidence_state: EvidenceState::Supported,
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: r.clone(),
                independent_source: true,
                local_data: true,
                policy_allowed: true,
                negative_result: false,
            }],
            peers: vec![MechanismPeer5 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                mechanism_id: "mech:a".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 2,
                support_milli: 800,
                artifact_digest: h("pa"),
                provenance_digest: h("pp"),
                evidence_state: EvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: r,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_and_ranked() {
        let r = assure_mechanisms(&q()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.selected_order, vec!["candidate:a"])
    }
    #[test]
    fn contradiction_blocks() {
        let mut x = q();
        x.candidates[0].evidence_state = EvidenceState::Contradicted;
        assert_eq!(assure_mechanisms(&x).unwrap().disposition, "blocked")
    }
}
