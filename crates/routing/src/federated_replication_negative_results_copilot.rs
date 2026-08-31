//! Federated continual replication and negative-results research copilot.
//!
//! Atlas feature `AFA-routing-P15-F12`. It compares typed replication summaries without
//! pooling raw observations, preserving null, negative, contradictory, and unmeasured outcomes.

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-routing-P15-F12";
pub const CONTRACT_VERSION: &str =
    "routing-federated-continual-replication-negative-results-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ReplicationQuestion6@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationCopilotReceipt8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.replication-copilot-receipt-8+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome {
    Positive,
    Null,
    Negative,
    Contradicted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationObservation6 {
    pub replication_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub effect_milli: i32,
    pub interval_low_milli: i32,
    pub interval_high_milli: i32,
    pub expected_direction: String,
    pub outcome: ReplicationOutcome,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub independent_site: bool,
    pub local_data: bool,
    pub policy_allowed: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationPeer5 {
    pub peer_id: String,
    pub origin: String,
    pub replication_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationQuestion6 {
    pub request_id: String,
    pub federation_id: String,
    pub claim_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub observations: Vec<ReplicationObservation6>,
    pub peers: Vec<ReplicationPeer5>,
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
pub struct ReplicationCopilotArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationCopilotReceipt8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub claim_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub reproducible_order: Vec<String>,
    pub null_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub replication_digest: ContentHash,
    pub artifact: ReplicationCopilotArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationCopilotError {
    #[error("invalid replication copilot request: {0}")]
    Invalid(String),
    #[error("replication copilot artifact failed: {0}")]
    Artifact(String),
}

pub fn federated_replication_negative_results_manifest() -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"routing","consumers":["replication scientist","evidence steward","federation operator"],"behavior":"compares typed independent replication outcomes and publishes negative-result evidence without pooling raw data","value":"makes null, negative, contradictory, and missing replication evidence first-class release constraints","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["retain:replication-receipt","exchange:aggregate-replication-summary"],"permissions":["retain:replication-evidence","exchange:aggregate-replication"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl ReplicationCopilotReceipt8 {
    pub fn validate(&self) -> Result<(), ReplicationCopilotError> {
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
            || self.claim_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.observation_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ReplicationCopilotError::Invalid("replication identity, checkpoint, locality, observations, peers, disposition, or effects are incomplete".into()));
        }
        for v in [
            &self.observation_order,
            &self.reproducible_order,
            &self.null_order,
            &self.negative_order,
            &self.contradicted_order,
            &self.unknown_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.contradiction_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if v.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ReplicationCopilotError::Invalid(
                    "replication ordering is not canonical".into(),
                ));
            }
        }
        let all = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let parts = self
            .reproducible_order
            .iter()
            .chain(&self.null_order)
            .chain(&self.negative_order)
            .chain(&self.contradicted_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all != parts || all.len() != self.observation_order.len() {
            return Err(ReplicationCopilotError::Invalid(
                "replication outcomes do not partition".into(),
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
            return Err(ReplicationCopilotError::Invalid(
                "replication peers do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.replication_digest
        {
            return Err(ReplicationCopilotError::Artifact(
                "replication artifact digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_federated_replication(
    q: &ReplicationQuestion6,
) -> Result<ReplicationCopilotReceipt8, ReplicationCopilotError> {
    validate_request(q)?;
    let mut rows = q.observations.clone();
    rows.sort_by(|a, b| a.replication_id.cmp(&b.replication_id));
    let observation_order = rows
        .iter()
        .map(|r| r.replication_id.clone())
        .collect::<Vec<_>>();
    let required_studies = q
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = q
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut reproducible = BTreeSet::new();
    let mut nulls = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut neg_evidence = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    for r in &rows {
        studies.insert(r.study_id.clone());
        modalities.insert(r.modality.clone());
        if r.negative_result || r.outcome == ReplicationOutcome::Negative {
            negative.insert(r.replication_id.clone());
            neg_evidence.insert(format!("{}:negative-result", r.replication_id));
        }
        match r.evidence_state {
            EvidenceState::Contradicted => {
                contradicted.insert(r.replication_id.clone());
                contradiction.insert(format!("{}:contradicted-evidence", r.replication_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unknown.insert(r.replication_id.clone());
                uncertainty.insert(format!("{}:evidence-unmeasured", r.replication_id));
            }
            EvidenceState::Proven | EvidenceState::Supported
                if r.independent_site && r.local_data && r.policy_allowed =>
            {
                match r.outcome {
                    ReplicationOutcome::Null => {
                        nulls.insert(r.replication_id.clone());
                    }
                    ReplicationOutcome::Negative => {}
                    ReplicationOutcome::Contradicted => {
                        contradicted.insert(r.replication_id.clone());
                    }
                    ReplicationOutcome::Unknown => {
                        unknown.insert(r.replication_id.clone());
                    }
                    ReplicationOutcome::Positive => {
                        reproducible.insert(r.replication_id.clone());
                    }
                }
            }
            EvidenceState::Proven | EvidenceState::Supported => {
                uncertainty.insert(format!("{}:independence-locality-policy", r.replication_id));
                unknown.insert(r.replication_id.clone());
            }
        }
    }
    let missing_study = required_studies
        .difference(&studies)
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_modality = required_modalities
        .difference(&modalities)
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
            p.replication_id == q.claim_id
                && p.semantic_profile == q.semantic_profile
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
        neg_evidence.insert("request:policy-denied".into());
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
    let disposition = if global || !contradicted.is_empty() {
        "blocked"
    } else if !missing_study.is_empty()
        || !missing_modality.is_empty()
        || !unknown.is_empty()
        || qualified_peers.len() < q.minimum_peer_quorum
        || reproducible.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:replication-not-release-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"federation_id":q.federation_id,"claim_id":q.claim_id,"researcher":q.researcher,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"checkpoint":q.checkpoint,"disposition":disposition,"observation_order":observation_order,"reproducible_order":reproducible,"null_order":nulls,"negative_order":negative,"contradicted_order":contradicted,"unknown_order":unknown,"missing_study_order":missing_study,"missing_modality_order":missing_modality,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"contradiction_order":contradiction,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":neg_evidence,"replay_identity":q.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ReplicationCopilotError::Artifact(e.to_string()))?;
    let artifact = ReplicationCopilotArtifact8 {
        artifact_id: format!("replication-copilot-receipt-8:{}", q.request_id),
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
            format!("retain:replication-receipt:{}", q.request_id),
            format!("exchange:aggregate-replication-summary:{}", q.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ReplicationCopilotReceipt8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        federation_id: q.federation_id.clone(),
        claim_id: q.claim_id.clone(),
        researcher: q.researcher.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        checkpoint: q.checkpoint,
        disposition: disposition.into(),
        observation_order: rows.iter().map(|r| r.replication_id.clone()).collect(),
        reproducible_order: reproducible.into_iter().collect(),
        null_order: nulls.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        contradicted_order: contradicted.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        missing_study_order: missing_study.into_iter().collect(),
        missing_modality_order: missing_modality.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: neg_evidence.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        replication_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(q: &ReplicationQuestion6) -> Result<(), ReplicationCopilotError> {
    if ![
        &q.request_id,
        &q.federation_id,
        &q.claim_id,
        &q.researcher,
        &q.purpose,
        &q.semantic_profile,
    ]
    .iter()
    .all(|v| !v.trim().is_empty())
        || q.required_study_order.is_empty()
        || q.required_modality_order.is_empty()
        || q.observations.is_empty()
        || q.peers.is_empty()
        || q.checkpoint == 0
        || q.minimum_peer_quorum == 0
        || !q.raw_data_local
        || !q.aggregate_only
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.replay_identity.as_str().len() != 64
    {
        return Err(ReplicationCopilotError::Invalid("replication identity, bounds, peers, observations, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for r in &q.observations {
        if r.replication_id.trim().is_empty()
            || !ids.insert(r.replication_id.clone())
            || r.study_id.trim().is_empty()
            || r.modality.trim().is_empty()
            || r.semantic_profile != q.semantic_profile
            || r.artifact_digest.as_str().len() != 64
            || r.provenance_digest.as_str().len() != 64
            || r.replay_identity != q.replay_identity
            || r.interval_low_milli > r.interval_high_milli
            || !r.local_data
        {
            return Err(ReplicationCopilotError::Invalid("replication observation identity, profile, digests, interval, replay, or locality is invalid".into()));
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
    fn q() -> ReplicationQuestion6 {
        let r = h("replay");
        ReplicationQuestion6 {
            request_id: "request:replication".into(),
            federation_id: "fed:replication".into(),
            claim_id: "claim:1".into(),
            researcher: "replication-scientist".into(),
            purpose: "replicate-organoid-effect".into(),
            semantic_profile: "neuro:v1".into(),
            required_study_order: vec!["study:a".into()],
            required_modality_order: vec!["imaging".into()],
            observations: vec![ReplicationObservation6 {
                replication_id: "rep:a".into(),
                study_id: "study:a".into(),
                modality: "imaging".into(),
                semantic_profile: "neuro:v1".into(),
                effect_milli: 80,
                interval_low_milli: 20,
                interval_high_milli: 120,
                expected_direction: "positive".into(),
                outcome: ReplicationOutcome::Positive,
                evidence_state: EvidenceState::Supported,
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: r.clone(),
                independent_site: true,
                local_data: true,
                policy_allowed: true,
                negative_result: false,
            }],
            peers: vec![ReplicationPeer5 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                replication_id: "claim:1".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 2,
                report_digest: h("report"),
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
    fn positive_is_qualified() {
        assert_eq!(
            assure_federated_replication(&q()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn null_and_negative_are_first_class() {
        let mut x = q();
        x.observations[0].outcome = ReplicationOutcome::Null;
        let r = assure_federated_replication(&x).unwrap();
        assert_eq!(r.null_order, vec!["rep:a"]);
        let mut x = q();
        x.observations[0].outcome = ReplicationOutcome::Negative;
        assert!(!assure_federated_replication(&x)
            .unwrap()
            .negative_order
            .is_empty())
    }
    #[test]
    fn contradiction_blocks() {
        let mut x = q();
        x.observations[0].evidence_state = EvidenceState::Contradicted;
        assert_eq!(
            assure_federated_replication(&x).unwrap().disposition,
            "blocked"
        )
    }
}
