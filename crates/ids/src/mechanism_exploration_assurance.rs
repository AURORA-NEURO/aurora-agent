//! Multimodal mechanism-exploration assurance harness (`AFA-ids-P08-F26`).
//!
//! The harness verifies caller-supplied candidate summaries and peer attestations. It ranks
//! supported candidates, retains competing explanations and negative evidence, and refuses to
//! present incomplete closure as a scientific conclusion.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P08-F26";
pub const CONTRACT_VERSION: &str = "ids-multimodal-mechanism-exploration-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "MechanismQuestion2@1";
pub const OUTPUT_SCHEMA: &str = "MechanismPortfolio7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.mechanism-portfolio-7+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismQuestion2 {
    pub request_id: String,
    pub question_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub minimum_support_milli: i64,
    pub minimum_peer_quorum: usize,
    pub checkpoint: u64,
    pub budget_units: u64,
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
pub struct MechanismCandidate4 {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub origin: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub support_milli: i64,
    pub novelty_milli: i64,
    pub semantic_profile: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: MechanismEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub counterevidence: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerMechanismSummary4 {
    pub peer_id: String,
    pub origin: String,
    pub question_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub portfolio_digest: ContentHash,
    pub candidate_count: usize,
    pub evidence_state: MechanismEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolio7Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolio7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub question_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub competing_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub support_scores_milli: Vec<i64>,
    pub novelty_scores_milli: Vec<i64>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub artifact: MechanismPortfolio7Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismExplorationError {
    #[error("invalid mechanism exploration request: {0}")]
    Invalid(String),
    #[error("mechanism portfolio artifact failed: {0}")]
    Artifact(String),
}

pub fn mechanism_exploration_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["downstream AURORA crate maintainer","mechanism scientist","independent validation partner"],"behavior":"verifies bounded multimodal mechanism candidates and peer attestations under comparability, closure, provenance, replay, and policy gates","value":"preserves competing explanations and negative evidence while preventing incomplete mechanism portfolios from becoming conclusions","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["verify:mechanism-portfolio"],"permissions":["evaluate:capability-runs"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl MechanismPortfolio7 {
    pub fn validate(&self) -> Result<(), MechanismExplorationError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.question_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MechanismExplorationError::Invalid("mechanism identity, checkpoint, locality, candidates, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.competing_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(MechanismExplorationError::Invalid(
                    "mechanism ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.competing_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids != parts || ids.len() != self.candidate_order.len() {
            return Err(MechanismExplorationError::Invalid(
                "mechanism candidate states do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != peer_parts || peers.len() != self.peer_order.len() {
            return Err(MechanismExplorationError::Invalid(
                "mechanism peer states do not partition".into(),
            ));
        }
        if self.selected_order.len() + self.competing_order.len() != self.support_scores_milli.len()
            || self.selected_order.len() + self.competing_order.len()
                != self.novelty_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.portfolio_digest
        {
            return Err(MechanismExplorationError::Artifact(
                "mechanism artifact, score cardinality, or digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| !e.starts_with("verify:mechanism-portfolio:") && e != "block:unsafe-release")
        {
            return Err(MechanismExplorationError::Invalid(
                "effect is outside mechanism gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MechanismExplorationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| MechanismExplorationError::Artifact(e.to_string()))?,
        )
        .map_err(|e| MechanismExplorationError::Artifact(e.to_string()))
    }
}

pub fn assure_mechanism_exploration(
    request: &MechanismQuestion2,
    candidates: &[MechanismCandidate4],
    peers: &[PeerMechanismSummary4],
) -> Result<MechanismPortfolio7, MechanismExplorationError> {
    validate_request(request, candidates, peers)?;
    let mut rows = candidates.to_vec();
    rows.sort_by(|a, b| {
        b.support_milli
            .saturating_add(b.novelty_milli)
            .cmp(&a.support_milli.saturating_add(a.novelty_milli))
            .then(a.candidate_id.cmp(&b.candidate_id))
    });
    let candidate_order = rows
        .iter()
        .map(|x| x.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut ps = peers.to_vec();
    ps.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = ps.iter().map(|x| x.peer_id.clone()).collect::<Vec<_>>();
    let mut qp = BTreeSet::new();
    let mut mp = BTreeSet::new();
    let mut unc = BTreeSet::new();
    for p in &ps {
        let ok = p.question_id == request.question_id
            && p.federation_id == request.federation_id
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && matches!(
                p.evidence_state,
                MechanismEvidenceState::Proven | MechanismEvidenceState::Supported
            );
        if ok {
            qp.insert(p.peer_id.clone());
        } else {
            mp.insert(p.peer_id.clone());
            unc.insert(format!("peer:{}:not-qualified", p.peer_id));
        }
    }
    let mut selected = BTreeSet::new();
    let mut competing = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_studies = BTreeSet::new();
    let mut missing_modalities = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = Vec::new();
    let mut novelty_scores = Vec::new();
    for c in &rows {
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.candidate_id));
        }
        if c.counterevidence {
            negative.insert(format!("{}:counterevidence", c.candidate_id));
        }
        for r in &c.omission_reasons {
            omissions.insert(format!("{}:{}", c.candidate_id, r));
        }
        let missing_s = request
            .required_studies
            .iter()
            .filter(|x| !c.study_ids.contains(x))
            .count();
        let missing_m = request
            .required_modalities
            .iter()
            .filter(|x| !c.modality_ids.contains(x))
            .count();
        if missing_s > 0 {
            missing_studies.insert(format!("{}:missing:{}", c.candidate_id, missing_s));
        }
        if missing_m > 0 {
            missing_modalities.insert(format!("{}:missing:{}", c.candidate_id, missing_m));
        }
        let mut reasons = Vec::new();
        if c.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        if missing_s > 0 {
            reasons.push("study-closure-incomplete");
        }
        if missing_m > 0 {
            reasons.push("modality-closure-incomplete");
        }
        if c.support_milli < request.minimum_support_milli {
            reasons.push("support-threshold-failed");
        }
        if c.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !c.signed || !c.permitted {
            reasons.push("authorization-missing");
        }
        if !c.raw_data_local || !c.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if c.evidence_state == MechanismEvidenceState::Contradicted {
            blocked.insert(c.candidate_id.clone());
            negative.insert(format!("{}:contradicted", c.candidate_id));
        } else if !matches!(
            c.evidence_state,
            MechanismEvidenceState::Proven | MechanismEvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(c.candidate_id.clone());
            unc.insert(format!("{}:unresolved", c.candidate_id));
        } else if selected.is_empty() {
            selected.insert(c.candidate_id.clone());
            scores.push(c.support_milli);
            novelty_scores.push(c.novelty_milli);
        } else {
            competing.insert(c.candidate_id.clone());
            scores.push(c.support_milli);
            novelty_scores.push(c.novelty_milli);
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        unc.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        unc.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        unc.insert("request:federation-approval-missing".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || !missing_studies.is_empty()
        || !missing_modalities.is_empty()
        || qp.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        competing.clear();
        unresolved.clear();
        scores.clear();
        novelty_scores.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:mechanism-gates-incomplete".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"question_id":request.question_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"candidate_order":candidate_order,"selected_order":selected,"competing_order":competing,"unresolved_order":unresolved,"blocked_order":blocked,"missing_study_order":missing_studies,"missing_modality_order":missing_modalities,"peer_order":peer_order,"qualified_peer_order":qp,"missing_peer_order":mp,"omission_order":omissions,"uncertainty_order":unc,"negative_evidence_order":negative,"support_scores_milli":scores,"novelty_scores_milli":novelty_scores,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let portfolio_digest = ContentHash::of_value(&payload)
        .map_err(|e| MechanismExplorationError::Artifact(e.to_string()))?;
    let artifact = MechanismPortfolio7Artifact {
        artifact_id: format!("mechanism-portfolio-7:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: portfolio_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![format!("verify:mechanism-portfolio:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = MechanismPortfolio7 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        question_id: request.question_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        competing_order: payload["competing_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_study_order: payload["missing_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        support_scores_milli: scores,
        novelty_scores_milli: novelty_scores,
        replay_identity: request.replay_identity.clone(),
        portfolio_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &MechanismQuestion2,
    candidates: &[MechanismCandidate4],
    peers: &[PeerMechanismSummary4],
) -> Result<(), MechanismExplorationError> {
    if request.request_id.trim().is_empty()
        || request.question_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.is_empty()
        || request.required_modalities.is_empty()
        || request.minimum_support_milli < 0
        || request.minimum_support_milli > 1000
        || request.minimum_peer_quorum == 0
        || request.checkpoint == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || candidates.is_empty()
        || peers.is_empty()
    {
        return Err(MechanismExplorationError::Invalid("mechanism identity, closure, threshold, quorum, checkpoint, budget, replay, locality, candidates, peers, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for c in candidates {
        if c.candidate_id.trim().is_empty()
            || c.mechanism_id.trim().is_empty()
            || c.origin.trim().is_empty()
            || c.content_digest.as_str().len() != 64
            || c.provenance_digest.as_str().len() != 64
            || c.replay_identity.as_str().len() != 64
            || !ids.insert(c.candidate_id.clone())
        {
            return Err(MechanismExplorationError::Invalid(
                "candidate identity, uniqueness, origin, or digest is invalid".into(),
            ));
        }
    }
    let mut pids = BTreeSet::new();
    for p in peers {
        if p.peer_id.trim().is_empty()
            || p.origin.trim().is_empty()
            || p.portfolio_digest.as_str().len() != 64
            || !pids.insert(p.peer_id.clone())
        {
            return Err(MechanismExplorationError::Invalid(
                "peer identity, uniqueness, origin, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn req() -> MechanismQuestion2 {
        MechanismQuestion2 {
            request_id: "request:mechanism".into(),
            question_id: "question:1".into(),
            federation_id: "federation:1".into(),
            requester: "mechanism-scientist".into(),
            purpose: "explore".into(),
            semantic_profile: "multi:v1".into(),
            required_studies: vec!["study:1".into()],
            required_modalities: vec!["imaging".into()],
            minimum_support_milli: 70,
            minimum_peer_quorum: 1,
            checkpoint: 1,
            budget_units: 10,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn cand(id: &str, state: MechanismEvidenceState) -> MechanismCandidate4 {
        MechanismCandidate4 {
            candidate_id: id.into(),
            mechanism_id: format!("mech:{id}"),
            origin: "site-a".into(),
            study_ids: vec!["study:1".into()],
            modality_ids: vec!["imaging".into()],
            support_milli: 90,
            novelty_milli: 80,
            semantic_profile: "multi:v1".into(),
            content_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("r"),
            evidence_state: state,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            counterevidence: false,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    fn peer() -> PeerMechanismSummary4 {
        PeerMechanismSummary4 {
            peer_id: "peer:a".into(),
            origin: "site-a".into(),
            question_id: "question:1".into(),
            federation_id: "federation:1".into(),
            semantic_profile: "multi:v1".into(),
            checkpoint: 1,
            portfolio_digest: h("peer"),
            candidate_count: 1,
            evidence_state: MechanismEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(mechanism_exploration_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn qualified_is_replayable() {
        let r = assure_mechanism_exploration(
            &req(),
            &[
                cand("b", MechanismEvidenceState::Supported),
                cand("a", MechanismEvidenceState::Proven),
            ],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
        assert!(!r.competing_order.is_empty());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = assure_mechanism_exploration(
            &req(),
            &[cand("a", MechanismEvidenceState::Unknown)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn contradiction_blocks() {
        let r = assure_mechanism_exploration(
            &req(),
            &[cand("a", MechanismEvidenceState::Contradicted)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn missing_closure_is_unresolved() {
        let mut c = cand("a", MechanismEvidenceState::Supported);
        c.modality_ids.clear();
        let r = assure_mechanism_exploration(&req(), &[c], &[peer()]).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(assure_mechanism_exploration(
            &req(),
            &[
                cand("a", MechanismEvidenceState::Supported),
                cand("a", MechanismEvidenceState::Supported)
            ],
            &[peer()]
        )
        .is_err());
    }
}
