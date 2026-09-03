//! Federated replication and negative-result control plane for `AFA-atlashub-P15-F29`.
//!
//! This product turns independent, caller-supplied replication summaries into a deterministic
//! `ReplicationRecord8`. Null and negative outcomes are retained as scientific evidence rather
//! than averaged away. The module never reads raw measurements, runs a protocol, or exports
//! anything except an aggregate digest after explicit policy and locality gates.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P15-F29";
pub const CONTRACT_VERSION: &str =
    "atlashub-local-single-study-replication-negative-results-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ClaimAndProtocol1@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationRecord8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.replication-record-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_OBSERVATIONS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome {
    Positive,
    Null,
    Negative,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol1 {
    pub claim_id: String,
    pub protocol_id: String,
    pub claim_text: String,
    pub semantic_profile: String,
    pub expected_direction: String,
    pub minimum_replicates: usize,
    pub protocol_digest: ContentHash,
    pub baseline_digest: ContentHash,
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
pub struct ReplicationObservation4 {
    pub observation_id: String,
    pub site_id: String,
    pub origin: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub outcome: ReplicationOutcome,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub comparable: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerReplicationSummary4 {
    pub peer_id: String,
    pub origin: String,
    pub claim_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub claim_id: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub qualified_observation_order: Vec<String>,
    pub unresolved_observation_order: Vec<String>,
    pub blocked_observation_order: Vec<String>,
    pub positive_order: Vec<String>,
    pub null_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub inconclusive_order: Vec<String>,
    pub site_order: Vec<String>,
    pub qualified_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_median_milli: i64,
    pub positive_count: usize,
    pub null_count: usize,
    pub negative_count: usize,
    pub replay_identity: ContentHash,
    pub record_digest: ContentHash,
    pub artifact: ReplicationArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationControlError {
    #[error("invalid replication control request: {0}")]
    Invalid(String),
    #[error("replication control artifact failed: {0}")]
    Artifact(String),
}

pub fn replication_control_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"atlashub","consumers":["integration engineer","replication scientist","federation steward"],"behavior":"classifies independent replication observations and negative results under typed protocol, provenance, replay, policy, and federation gates","value":"prevents null, negative, contradictory, or incomparable replication evidence from being hidden in a positive claim","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability","exchange:permitted-summaries"],"permissions":["operate:institution-node"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl ReplicationRecord8 {
    pub fn validate(&self) -> Result<(), ReplicationControlError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.claim_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.observation_order.is_empty()
            || self.site_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ReplicationControlError::Invalid("replication identity, checkpoint, locality, observations, sites, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.observation_order,
            &self.qualified_observation_order,
            &self.unresolved_observation_order,
            &self.blocked_observation_order,
            &self.positive_order,
            &self.null_order,
            &self.negative_order,
            &self.inconclusive_order,
            &self.site_order,
            &self.qualified_site_order,
            &self.missing_site_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ReplicationControlError::Invalid(
                    "replication ordering is not canonical".into(),
                ));
            }
        }
        let observations = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let classified = self
            .qualified_observation_order
            .iter()
            .chain(&self.unresolved_observation_order)
            .chain(&self.blocked_observation_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if observations != classified || observations.len() != self.observation_order.len() {
            return Err(ReplicationControlError::Invalid(
                "observation states do not partition".into(),
            ));
        }
        let sites = BTreeSet::from_iter(self.site_order.iter().cloned());
        let site_parts = self
            .qualified_site_order
            .iter()
            .chain(&self.missing_site_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if sites != site_parts || sites.len() != self.site_order.len() {
            return Err(ReplicationControlError::Invalid(
                "site states do not partition".into(),
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
            return Err(ReplicationControlError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.record_digest
        {
            return Err(ReplicationControlError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ReplicationControlError::Invalid(
                "effect is outside the replication gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ReplicationControlError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ReplicationControlError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ReplicationControlError::Artifact(error.to_string()))
    }
}

pub fn operate_replication_control(
    request_id: &str,
    claim: &ClaimAndProtocol1,
    observations: &[ReplicationObservation4],
    peers: &[PeerReplicationSummary4],
) -> Result<ReplicationRecord8, ReplicationControlError> {
    validate_claim(claim, observations, peers)?;
    let mut rows = observations.to_vec();
    rows.sort_by(|a, b| {
        a.site_id
            .cmp(&b.site_id)
            .then(a.observation_id.cmp(&b.observation_id))
    });
    let observation_order = rows
        .iter()
        .map(|r| r.observation_id.clone())
        .collect::<Vec<_>>();
    let site_order = rows
        .iter()
        .map(|r| r.site_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut positive = BTreeSet::new();
    let mut nulls = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut inconclusive = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut effect_values = Vec::new();
    for row in &rows {
        for reason in &row.omission_reasons {
            omissions.insert(format!("{}:{}", row.observation_id, reason));
        }
        if row.negative_result
            || matches!(
                row.outcome,
                ReplicationOutcome::Negative | ReplicationOutcome::Null
            )
        {
            negative_evidence.insert(format!("{}:negative-or-null", row.observation_id));
        }
        if row.outcome == ReplicationOutcome::Positive {
            positive.insert(row.observation_id.clone());
            effect_values.push(row.effect_milli);
        }
        if row.outcome == ReplicationOutcome::Null {
            nulls.insert(row.observation_id.clone());
        }
        if row.outcome == ReplicationOutcome::Negative {
            negative.insert(row.observation_id.clone());
        }
        if row.outcome == ReplicationOutcome::Inconclusive {
            inconclusive.insert(row.observation_id.clone());
        }
        let compatible = row.protocol_id == claim.protocol_id
            && row.semantic_profile == claim.semantic_profile
            && row.protocol_id == claim.protocol_id
            && row.replay_identity == claim.replay_identity
            && row.signed
            && row.comparable
            && row.raw_data_local
            && row.aggregate_only;
        if row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.observation_id.clone());
            negative_evidence.insert(format!("{}:contradicted", row.observation_id));
        } else if !compatible
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(row.observation_id.clone());
            uncertainty.insert(format!("{}:unresolved", row.observation_id));
        } else {
            qualified.insert(row.observation_id.clone());
        }
    }
    let mut peers_sorted = peers.to_vec();
    peers_sorted.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers_sorted
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers_sorted {
        let ok = peer.claim_id == claim.claim_id
            && peer.semantic_profile == claim.semantic_profile
            && peer.checkpoint == claim.minimum_replicates as u64
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    let global_block = !claim.policy_allow
        || !claim.protected_closure
        || !claim.signed_approval
        || !claim.federation_approved
        || !claim.raw_data_local
        || !claim.aggregate_only;
    if !claim.policy_allow {
        negative_evidence.insert("request:policy-denied".into());
    }
    if !claim.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !claim.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !claim.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if qualified.len() < claim.minimum_replicates
        || qualified_peers.is_empty()
        || !negative.is_empty()
        || !nulls.is_empty()
        || !inconclusive.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:replication-gates-incomplete".into());
    }
    if global_block {
        blocked.extend(observation_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
    }
    let effect_median = if effect_values.is_empty() {
        0
    } else {
        effect_values.sort();
        effect_values[effect_values.len() / 2]
    };
    let qualified_observation_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_observation_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_observation_order = blocked.into_iter().collect::<Vec<_>>();
    let qualified_site_set = rows
        .iter()
        .filter(|row| qualified_observation_order.contains(&row.observation_id))
        .map(|row| row.site_id.clone())
        .collect::<BTreeSet<_>>();
    let qualified_site_order = qualified_site_set.iter().cloned().collect::<Vec<_>>();
    let missing_site_order = site_order
        .iter()
        .filter(|site| !qualified_site_set.contains(*site))
        .cloned()
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request_id,"claim_id":claim.claim_id,"protocol_id":claim.protocol_id,"semantic_profile":claim.semantic_profile,"checkpoint":claim.minimum_replicates,"disposition":disposition,"observation_order":observation_order,"qualified_observation_order":qualified_observation_order,"unresolved_observation_order":unresolved_observation_order,"blocked_observation_order":blocked_observation_order,"positive_order":positive,"null_order":nulls,"negative_order":negative,"inconclusive_order":inconclusive,"site_order":site_order,"qualified_site_order":qualified_site_order,"missing_site_order":missing_site_order,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative_evidence,"effect_median_milli":effect_median,"positive_count":positive.len(),"null_count":nulls.len(),"negative_count":negative.len(),"replay_identity":claim.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let record_digest = ContentHash::of_value(&payload)
        .map_err(|error| ReplicationControlError::Artifact(error.to_string()))?;
    let artifact = ReplicationArtifact8 {
        artifact_id: format!("replication-record-8:{}", request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: record_digest.clone(),
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
            format!("exchange:permitted-summaries:{}", request_id),
            format!("manage:local-capability:{}", request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ReplicationRecord8 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request_id.into(),
        claim_id: claim.claim_id.clone(),
        protocol_id: claim.protocol_id.clone(),
        semantic_profile: claim.semantic_profile.clone(),
        checkpoint: claim.minimum_replicates as u64,
        disposition: disposition.into(),
        observation_order: payload["observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_observation_order: payload["qualified_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_observation_order: payload["unresolved_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_observation_order: payload["blocked_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        positive_order: positive.into_iter().collect(),
        null_order: nulls.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        inconclusive_order: inconclusive.into_iter().collect(),
        site_order: site_order.clone(),
        qualified_site_order,
        missing_site_order,
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative_evidence.into_iter().collect(),
        effect_median_milli: effect_median,
        positive_count: 0,
        null_count: 0,
        negative_count: 0,
        replay_identity: claim.replay_identity.clone(),
        record_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: claim.raw_data_local,
        aggregate_only: claim.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let mut receipt = receipt;
    receipt.positive_count = receipt.positive_order.len();
    receipt.null_count = receipt.null_order.len();
    receipt.negative_count = receipt.negative_order.len();
    receipt.validate()?;
    Ok(receipt)
}

fn validate_claim(
    claim: &ClaimAndProtocol1,
    observations: &[ReplicationObservation4],
    peers: &[PeerReplicationSummary4],
) -> Result<(), ReplicationControlError> {
    if claim.claim_id.trim().is_empty()
        || claim.protocol_id.trim().is_empty()
        || claim.claim_text.trim().is_empty()
        || claim.semantic_profile.trim().is_empty()
        || claim.expected_direction.trim().is_empty()
        || claim.minimum_replicates == 0
        || claim.protocol_digest.as_str().len() != 64
        || claim.baseline_digest.as_str().len() != 64
        || claim.replay_identity.as_str().len() != 64
        || claim.boundary != PRECLINICAL_BOUNDARY
        || !claim.raw_data_local
        || !claim.aggregate_only
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
        || peers.is_empty()
    {
        return Err(ReplicationControlError::Invalid(
            "claim identity, digests, bounds, locality, observations, or peers are invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for row in observations {
        if row.observation_id.trim().is_empty()
            || row.site_id.trim().is_empty()
            || row.origin.trim().is_empty()
            || !ids.insert(row.observation_id.clone())
            || row.artifact_digest.as_str().len() != 64
            || row.provenance_digest.as_str().len() != 64
            || row.replay_identity.as_str().len() != 64
        {
            return Err(ReplicationControlError::Invalid(
                "observation identity, uniqueness, origin, or digest is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.report_digest.as_str().len() != 64
        {
            return Err(ReplicationControlError::Invalid(
                "peer identity, uniqueness, origin, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn claim() -> ClaimAndProtocol1 {
        ClaimAndProtocol1 {
            claim_id: "claim:1".into(),
            protocol_id: "protocol:1".into(),
            claim_text: "effect is reproducible".into(),
            semantic_profile: "neuro:v1".into(),
            expected_direction: "positive".into(),
            minimum_replicates: 1,
            protocol_digest: hash("protocol"),
            baseline_digest: hash("baseline"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn observation(
        id: &str,
        outcome: ReplicationOutcome,
        state: EvidenceState,
    ) -> ReplicationObservation4 {
        ReplicationObservation4 {
            observation_id: id.into(),
            site_id: format!("site:{id}"),
            origin: "site-a".into(),
            protocol_id: "protocol:1".into(),
            semantic_profile: "neuro:v1".into(),
            outcome,
            effect_milli: 100,
            uncertainty_milli: 5,
            evidence_state: state,
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("p:{id}")),
            replay_identity: hash("replay"),
            signed: true,
            comparable: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    fn peer() -> PeerReplicationSummary4 {
        PeerReplicationSummary4 {
            peer_id: "peer:a".into(),
            origin: "site-a".into(),
            claim_id: "claim:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 1,
            report_digest: hash("peer"),
            evidence_state: EvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn manifest_is_a1_and_effects_are_bounded() {
        let m = replication_control_manifest();
        assert_eq!(m["autonomy_tier"], "A1");
        assert_eq!(m["effects"][0], "manage:local-capability");
    }
    #[test]
    fn positive_replication_is_qualified() {
        let r = operate_replication_control(
            "request:replication",
            &claim(),
            &[observation(
                "a",
                ReplicationOutcome::Positive,
                EvidenceState::Supported,
            )],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(
            r.effect_receipts,
            vec![
                "exchange:permitted-summaries:request:replication",
                "manage:local-capability:request:replication"
            ]
        );
    }
    #[test]
    fn null_result_is_retained_and_unresolved() {
        let r = operate_replication_control(
            "request:replication",
            &claim(),
            &[observation(
                "a",
                ReplicationOutcome::Null,
                EvidenceState::Supported,
            )],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert_eq!(r.null_count, 1);
        assert!(r
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("negative-or-null")));
    }
    #[test]
    fn contradiction_blocks() {
        let r = operate_replication_control(
            "request:replication",
            &claim(),
            &[observation(
                "a",
                ReplicationOutcome::Positive,
                EvidenceState::Contradicted,
            )],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(r
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("contradicted")));
    }
    #[test]
    fn policy_denial_fails_closed() {
        let mut c = claim();
        c.policy_allow = false;
        let r = operate_replication_control(
            "request:replication",
            &c,
            &[observation(
                "a",
                ReplicationOutcome::Positive,
                EvidenceState::Supported,
            )],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_observation_is_rejected() {
        let r = operate_replication_control(
            "request:replication",
            &claim(),
            &[
                observation("a", ReplicationOutcome::Positive, EvidenceState::Supported),
                observation("a", ReplicationOutcome::Positive, EvidenceState::Supported),
            ],
            &[peer()],
        );
        assert!(r.is_err());
    }
}
