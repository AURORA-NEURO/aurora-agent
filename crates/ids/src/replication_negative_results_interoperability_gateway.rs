//! Multimodal multi-study replication and negative-results interoperability
//! gateway (`AFA-ids-P15-F22`).
//!
//! The gateway validates typed replication attestations and emits an
//! interoperable, digest-only record.  It does not re-run experiments, infer
//! clinical efficacy, or export raw imaging/omics observations.  Null and
//! negative outcomes are first-class evidence and are never silently dropped.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P15-F22";
pub const CONTRACT_VERSION: &str =
    "ids-multimodal-multi-study-replication-negative-results-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ClaimAndProtocol7@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationRecord9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.replication-record-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_OBSERVATIONS: usize = 8192;
pub const MAX_PEERS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol7 {
    pub claim_id: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub claim_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub minimum_replicates: usize,
    pub effect_threshold_milli: i64,
    pub protected_closure: bool,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub evidence_state: ReplicationEvidenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationObservation7 {
    pub observation_id: String,
    pub site_id: String,
    pub study_id: String,
    pub modality_ids: Vec<String>,
    pub outcome: String,
    pub effect_milli: i64,
    pub uncertainty_milli: i64,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub evidence_state: ReplicationEvidenceState,
    pub comparable: bool,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationPeer7 {
    pub peer_id: String,
    pub origin: String,
    pub claim_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub replication_digest: ContentHash,
    pub observation_count: usize,
    pub evidence_state: ReplicationEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol7Request {
    pub request_id: String,
    pub claim: ClaimAndProtocol7,
    pub observations: Vec<ReplicationObservation7>,
    pub peers: Vec<ReplicationPeer7>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub max_budget_units: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord9 {
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
    pub incomparable_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_median_milli: i64,
    pub effect_median_available: bool,
    pub positive_count: usize,
    pub null_count: usize,
    pub negative_count: usize,
    pub inconclusive_count: usize,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub record_digest: ContentHash,
    pub artifact: ReplicationRecord9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationInteroperabilityError {
    #[error("invalid replication interoperability request: {0}")]
    Invalid(String),
    #[error("replication interoperability artifact failed: {0}")]
    Artifact(String),
}

pub fn replication_interoperability_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["computational biologist", "replication coordinator", "multimodal integration gateway", "federation steward"],
        "behavior": "validates multimodal multi-study replication attestations and emits an interoperable negative-results record",
        "value": "makes positive, null, negative, inconclusive, incomparable, and contradictory replication outcomes exchangeable without erasing uncertainty",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-summaries", "manage:local-capability"],
        "permissions": ["read:local-replication-manifests", "exchange:aggregate-results"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl ReplicationRecord9 {
    pub fn validate(&self) -> Result<(), ReplicationInteroperabilityError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.claim_id,
                &self.protocol_id,
                &self.semantic_profile,
            ])
            || self.checkpoint == 0
            || self.observation_order.is_empty()
            || self.site_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ReplicationInteroperabilityError::Invalid(
                "replication identity, checkpoint, locality, observations, sites, peers, or effects are incomplete".into(),
            ));
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
            &self.incomparable_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ReplicationInteroperabilityError::Invalid(
                    "replication ordering is not canonical".into(),
                ));
            }
        }
        let observations = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let states = self
            .qualified_observation_order
            .iter()
            .chain(&self.unresolved_observation_order)
            .chain(&self.blocked_observation_order)
            .cloned()
            .collect::<Vec<_>>();
        let outcome_states = self
            .positive_order
            .iter()
            .chain(&self.null_order)
            .chain(&self.negative_order)
            .chain(&self.inconclusive_order)
            .cloned()
            .collect::<Vec<_>>();
        let sites = BTreeSet::from_iter(self.site_order.iter().cloned());
        let site_states = self
            .qualified_site_order
            .iter()
            .chain(&self.missing_site_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if observations.len() != self.observation_order.len()
            || BTreeSet::from_iter(states.iter().cloned()) != observations
            || states.len() != observations.len()
            || BTreeSet::from_iter(outcome_states.iter().cloned())
                != BTreeSet::from_iter(self.qualified_observation_order.iter().cloned())
            || outcome_states.len() != self.qualified_observation_order.len()
            || BTreeSet::from_iter(site_states.iter().cloned()) != sites
            || site_states.len() != sites.len()
            || BTreeSet::from_iter(peer_states.iter().cloned()) != peers
            || peer_states.len() != peers.len()
            || self.positive_count != self.positive_order.len()
            || self.null_count != self.null_order.len()
            || self.negative_count != self.negative_order.len()
            || self.inconclusive_count != self.inconclusive_order.len()
        {
            return Err(ReplicationInteroperabilityError::Invalid(
                "replication observation, outcome, site, peer, or count states do not partition"
                    .into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.record_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| d.as_str().len() != 64)
        {
            return Err(ReplicationInteroperabilityError::Artifact(
                "replication artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(ReplicationInteroperabilityError::Invalid(
                "effect is outside the replication interoperability gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ReplicationInteroperabilityError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ReplicationInteroperabilityError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ReplicationInteroperabilityError::Artifact(e.to_string()))
    }
}

fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

fn valid_metric(value: i64) -> bool {
    (0..=1_000).contains(&value)
}

fn outcome_allowed(outcome: &str) -> bool {
    matches!(outcome, "positive" | "null" | "negative" | "inconclusive")
}

fn median(values: &mut [i64]) -> i64 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

pub fn interoperate_replication(
    request: &ClaimAndProtocol7Request,
) -> Result<ReplicationRecord9, ReplicationInteroperabilityError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
    let observation_order = observations
        .iter()
        .map(|o| o.observation_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut positive = BTreeSet::new();
    let mut null_results = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut inconclusive = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut total_units = 0_u64;
    let mut qualified_effects = Vec::new();
    for observation in &observations {
        let id = observation.observation_id.clone();
        total_units = total_units.saturating_add(observation.estimated_units);
        if observation.negative_result {
            negative_evidence.insert(format!("{id}:negative-result"));
        }
        if observation.evidence_state == ReplicationEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative_evidence.insert(format!("{id}:contradicted"));
            continue;
        }
        let study_ok = request.claim.study_ids.contains(&observation.study_id);
        let modality_ok = request
            .claim
            .modality_ids
            .iter()
            .all(|modality| observation.modality_ids.contains(modality));
        if !study_ok || !modality_ok || !observation.raw_data_local || !observation.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:study-modality-or-locality-closure"));
            continue;
        }
        if observation.replay_identity != request.claim.replay_identity
            || !observation.signed
            || !observation.permitted
        {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:replay-or-authorization"));
            continue;
        }
        if !matches!(
            observation.evidence_state,
            ReplicationEvidenceState::Proven | ReplicationEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
            continue;
        }
        if !observation.comparable {
            unresolved.insert(id.clone());
            incomparable.insert(id.clone());
            uncertainty.insert(format!("{id}:cross-study-comparability"));
            continue;
        }
        if !outcome_allowed(&observation.outcome) {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:outcome-unmeasured"));
            continue;
        }
        if observation.outcome == "positive"
            && observation.effect_milli.abs() < request.claim.effect_threshold_milli
        {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:effect-threshold"));
            omissions.insert(format!("{id}:positive-effect-below-registered-threshold"));
            continue;
        }
        qualified.insert(id.clone());
        qualified_effects.push(observation.effect_milli);
        match observation.outcome.as_str() {
            "positive" => {
                positive.insert(id.clone());
            }
            "null" => {
                null_results.insert(id.clone());
                negative_evidence.insert(format!("{id}:null-result"));
            }
            "negative" => {
                negative.insert(id.clone());
                negative_evidence.insert(format!("{id}:negative-outcome"));
            }
            "inconclusive" => {
                inconclusive.insert(id.clone());
                uncertainty.insert(format!("{id}:inconclusive"));
            }
            _ => unreachable!(),
        }
        omissions.extend(
            observation
                .omission_reasons
                .iter()
                .map(|reason| format!("{id}:{reason}")),
        );
    }
    let site_order = observations
        .iter()
        .map(|o| o.site_id.clone())
        .collect::<BTreeSet<_>>();
    let qualified_site_order = observations
        .iter()
        .filter(|o| qualified.contains(&o.observation_id))
        .map(|o| o.site_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_site_order = site_order
        .difference(&qualified_site_order)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_site_order.is_empty() {
        omissions.insert("site:qualified-closure-incomplete".into());
    }
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let ok = peer.claim_id == request.claim.claim_id
            && peer.semantic_profile == request.claim.semantic_profile
            && peer.checkpoint == request.checkpoint
            && peer.observation_count > 0
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                ReplicationEvidenceState::Proven | ReplicationEvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if qualified.len() < request.claim.minimum_replicates {
        uncertainty.insert("replication:minimum-replicates-unmet".into());
    }
    if total_units > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total_units}"));
    }
    if !request.policy_allow {
        negative_evidence.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(observation_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        positive.clear();
        null_results.clear();
        negative.clear();
        inconclusive.clear();
        omissions.insert("request:replication-interoperability-not-authorized".into());
    }
    let disposition = if global_block || qualified.is_empty() && !blocked.is_empty() {
        "blocked"
    } else if qualified.len() < request.claim.minimum_replicates
        || qualified_peers.len() < request.minimum_peer_quorum
        || total_units > request.max_budget_units
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:replication-record-not-release-ready".into());
    }
    let qualified_order = qualified.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let median_available = !qualified_effects.is_empty();
    let effect_median_milli = if median_available {
        median(&mut qualified_effects)
    } else {
        0
    };
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "claim_id": request.claim.claim_id,
        "protocol_id": request.claim.protocol_id,
        "semantic_profile": request.claim.semantic_profile,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "observation_order": observation_order,
        "qualified_observation_order": qualified_order,
        "unresolved_observation_order": unresolved_order,
        "blocked_observation_order": blocked_order,
        "positive_order": positive,
        "null_order": null_results,
        "negative_order": negative,
        "inconclusive_order": inconclusive,
        "site_order": site_order,
        "qualified_site_order": qualified_site_order,
        "missing_site_order": missing_site_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers,
        "missing_peer_order": missing_peers,
        "incomparable_order": incomparable,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative_evidence,
        "effect_median_milli": effect_median_milli,
        "effect_median_available": median_available,
        "positive_count": positive.len(),
        "null_count": null_results.len(),
        "negative_count": negative.len(),
        "inconclusive_count": inconclusive.len(),
        "total_units": total_units,
        "replay_identity": request.claim.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ReplicationInteroperabilityError::Artifact(e.to_string()))?;
    let artifact = ReplicationRecord9Artifact {
        artifact_id: format!("replication-record-9:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: omissions.iter().cloned().collect(),
        provenance_digests: observations
            .iter()
            .map(|o| o.provenance_digest.clone())
            .chain(std::iter::once(request.claim.provenance_digest.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = ReplicationRecord9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        claim_id: request.claim.claim_id.clone(),
        protocol_id: request.claim.protocol_id.clone(),
        semantic_profile: request.claim.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        observation_order,
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
        positive_order: payload["positive_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        null_order: payload["null_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_order: payload["negative_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        inconclusive_order: payload["inconclusive_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        site_order: payload["site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_site_order: payload["qualified_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_site_order: payload["missing_site_order"]
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
        incomparable_order: payload["incomparable_order"]
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
        effect_median_milli,
        effect_median_available: median_available,
        positive_count: positive.len(),
        null_count: null_results.len(),
        negative_count: negative.len(),
        inconclusive_count: inconclusive.len(),
        total_units,
        replay_identity: request.claim.replay_identity.clone(),
        record_digest: digest,
        artifact,
        effect_receipts: if disposition == "qualified" {
            vec![
                format!("exchange:permitted-summaries:{}", request.request_id),
                format!("manage:local-capability:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ClaimAndProtocol7Request,
) -> Result<(), ReplicationInteroperabilityError> {
    if request.request_id.trim().is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.checkpoint == 0
        || request.minimum_peer_quorum == 0
        || request.max_budget_units == 0
        || request.claim.minimum_replicates == 0
        || !all_nonempty([
            &request.claim.claim_id,
            &request.claim.protocol_id,
            &request.claim.semantic_profile,
        ])
        || request.claim.study_ids.is_empty()
        || request.claim.modality_ids.is_empty()
        || request.claim.study_ids.windows(2).any(|w| w[0] >= w[1])
        || request.claim.modality_ids.windows(2).any(|w| w[0] >= w[1])
        || request.claim.claim_digest.as_str().len() != 64
        || request.claim.provenance_digest.as_str().len() != 64
        || request.claim.replay_identity.as_str().len() != 64
        || !valid_metric(request.claim.effect_threshold_milli)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(ReplicationInteroperabilityError::Invalid(
            "request, claim, study/modality closure, bounds, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut observations = BTreeSet::new();
    for observation in &request.observations {
        if !all_nonempty([
            &observation.observation_id,
            &observation.site_id,
            &observation.study_id,
            &observation.outcome,
        ]) || !observations.insert(observation.observation_id.clone())
            || observation.modality_ids.is_empty()
            || observation.modality_ids.windows(2).any(|w| w[0] >= w[1])
            || !outcome_allowed(&observation.outcome)
            || !valid_metric(observation.uncertainty_milli)
            || observation.estimated_units == 0
            || observation.artifact_digest.as_str().len() != 64
            || observation.provenance_digest.as_str().len() != 64
            || observation.replay_identity.as_str().len() != 64
            || observation
                .omission_reasons
                .windows(2)
                .any(|w| w[0] >= w[1])
        {
            return Err(ReplicationInteroperabilityError::Invalid(
                "observation identity, modalities, outcome, bounds, omissions, or digests are invalid".into(),
            ));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if !all_nonempty([
            &peer.peer_id,
            &peer.origin,
            &peer.claim_id,
            &peer.semantic_profile,
        ]) || !peers.insert(peer.peer_id.clone())
            || peer.checkpoint == 0
            || peer.observation_count == 0
            || peer.replication_digest.as_str().len() != 64
        {
            return Err(ReplicationInteroperabilityError::Invalid(
                "peer identity, claim, checkpoint, observation count, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn claim() -> ClaimAndProtocol7 {
        ClaimAndProtocol7 {
            claim_id: "claim:one".into(),
            protocol_id: "protocol:one".into(),
            semantic_profile: "neuro:replication:v1".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            modality_ids: vec!["imaging".into(), "omics".into()],
            claim_digest: h("claim"),
            provenance_digest: h("claim-provenance"),
            replay_identity: h("replay"),
            minimum_replicates: 2,
            effect_threshold_milli: 100,
            protected_closure: true,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            evidence_state: ReplicationEvidenceState::Supported,
        }
    }

    fn observation(id: &str, outcome: &str, effect: i64) -> ReplicationObservation7 {
        ReplicationObservation7 {
            observation_id: id.into(),
            site_id: format!("site:{id}"),
            study_id: if id.ends_with('a') {
                "study:a"
            } else {
                "study:b"
            }
            .into(),
            modality_ids: vec!["imaging".into(), "omics".into()],
            outcome: outcome.into(),
            effect_milli: effect,
            uncertainty_milli: 100,
            artifact_digest: h(id),
            provenance_digest: h("observation-provenance"),
            replay_identity: h("replay"),
            estimated_units: 5,
            evidence_state: ReplicationEvidenceState::Supported,
            comparable: true,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: outcome == "negative",
            omission_reasons: Vec::new(),
        }
    }

    fn request() -> ClaimAndProtocol7Request {
        ClaimAndProtocol7Request {
            request_id: "request:replication".into(),
            claim: claim(),
            observations: vec![
                observation("observation:a", "positive", 300),
                observation("observation:b", "null", 0),
            ],
            peers: vec![ReplicationPeer7 {
                peer_id: "peer:one".into(),
                origin: "site:peer".into(),
                claim_id: "claim:one".into(),
                semantic_profile: "neuro:replication:v1".into(),
                checkpoint: 2,
                replication_digest: h("peer"),
                observation_count: 2,
                evidence_state: ReplicationEvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            minimum_peer_quorum: 1,
            max_budget_units: 100,
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
    fn manifest_is_a2() {
        assert_eq!(
            replication_interoperability_manifest()["autonomy_tier"],
            "A2"
        );
    }

    #[test]
    fn nominal_record_is_qualified_with_null_evidence() {
        let report = interoperate_replication(&request()).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.null_count, 1);
        assert!(report.effect_median_available);
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }

    #[test]
    fn incomparable_observation_is_unresolved() {
        let mut request = request();
        request.observations[0].comparable = false;
        let report = interoperate_replication(&request).unwrap();
        assert!(report.incomparable_order.contains(&"observation:a".into()));
        assert!(report
            .unresolved_observation_order
            .contains(&"observation:a".into()));
    }

    #[test]
    fn contradiction_is_blocked_and_negative() {
        let mut request = request();
        request.observations[0].evidence_state = ReplicationEvidenceState::Contradicted;
        let report = interoperate_replication(&request).unwrap();
        assert!(report
            .blocked_observation_order
            .contains(&"observation:a".into()));
        assert!(report
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("contradicted")));
    }

    #[test]
    fn missing_replicate_is_unresolved() {
        let mut request = request();
        request.observations.pop();
        let report = interoperate_replication(&request).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report
            .uncertainty_order
            .contains(&"replication:minimum-replicates-unmet".into()));
    }

    #[test]
    fn federation_denial_blocks_without_exchange() {
        let mut request = request();
        request.federation_approved = false;
        let report = interoperate_replication(&request).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn negative_outcome_is_retained() {
        let mut request = request();
        request.observations[1].outcome = "negative".into();
        request.observations[1].negative_result = true;
        let report = interoperate_replication(&request).unwrap();
        assert_eq!(report.negative_count, 1);
        assert!(report
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("negative-outcome")));
    }
}
