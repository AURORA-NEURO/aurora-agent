//! Federated limitation-closure interoperability gateway (`AFA-ids-P26-F24`).
//!
//! This gateway turns caller-declared limitations and peer attestations into a
//! deterministic, digest-bound closure receipt. An unresolved or contradicted
//! limitation is never silently promoted to closed, and no raw research payload
//! is loaded or exchanged.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P26-F24";
pub const CONTRACT_VERSION: &str = "ids-federated-limitation-closure-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "IdsLimitationCase8@1";
pub const OUTPUT_SCHEMA: &str = "IdsClosureReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-closure-receipt-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CASES: usize = 16_384;
pub const MAX_PEERS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitationState {
    Open,
    Measured,
    Resolved,
    Blocked,
    Unknown,
    Contradicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerEvidenceState {
    Qualified,
    Unknown,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsLimitationCase8 {
    pub case_id: String,
    pub limitation: String,
    pub scope: String,
    pub status: LimitationState,
    pub evidence_digests: Vec<ContentHash>,
    pub closure_criteria: Vec<String>,
    pub mitigation: String,
    pub negative_result: Option<String>,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsClosurePeer7 {
    pub peer_id: String,
    pub semantic_profile: String,
    pub closure_digest: ContentHash,
    pub evidence_state: PeerEvidenceState,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsLimitationClosureRequest7 {
    pub request_id: String,
    pub semantic_profile: String,
    pub required_scopes: Vec<String>,
    pub cases: Vec<IdsLimitationCase8>,
    pub peers: Vec<IdsClosurePeer7>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsClosureReceipt9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsClosureReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub semantic_profile: String,
    pub required_scope_order: Vec<String>,
    pub disposition: String,
    pub case_order: Vec<String>,
    pub resolved_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub minimum_peer_quorum: u16,
    pub qualified_peer_count: u16,
    pub evidence_order: Vec<ContentHash>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub reasons: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: IdsClosureReceipt9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LimitationClosureError {
    #[error("invalid limitation closure request: {0}")]
    Invalid(String),
    #[error("limitation closure receipt failed validation: {0}")]
    Receipt(String),
}

pub fn limitation_closure_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "federation operator", "release auditor", "context compiler engineer"],
        "behavior": "close typed limitation cases across policy-separated institutions without hiding unresolved or negative evidence",
        "value": "prevents measured, open, contradictory, or peer-incomplete limitations from being represented as closed",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-limitation-digests", "manage:local-capability"],
        "permissions": ["read:local-limitation-attestations", "request:limitation-closure"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn ordered_hashes(values: &[ContentHash]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl IdsClosureReceipt9 {
    pub fn validate(&self) -> Result<(), LimitationClosureError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_scope_order.is_empty()
            || self.case_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["closed", "partial", "unknown", "blocked"].contains(&self.disposition.as_str())
            || self.qualified_peer_count as usize > self.peer_order.len()
        {
            return Err(LimitationClosureError::Receipt(
                "closure identity, scopes, cases, peers, effects, locality, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.required_scope_order,
            &self.case_order,
            &self.resolved_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(LimitationClosureError::Receipt(
                    "closure ordering is not canonical".into(),
                ));
            }
        }
        if !ordered_hashes(&self.evidence_order) {
            return Err(LimitationClosureError::Receipt(
                "closure evidence ordering is not canonical".into(),
            ));
        }
        let cases = BTreeSet::from_iter(self.case_order.iter().cloned());
        let case_parts = self
            .resolved_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if cases.len() != self.case_order.len()
            || case_parts.len() != cases.len()
            || BTreeSet::from_iter(case_parts) != cases
        {
            return Err(LimitationClosureError::Receipt(
                "limitation states do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if peers.len() != self.peer_order.len()
            || peer_parts.len() != peers.len()
            || BTreeSet::from_iter(peer_parts) != peers
        {
            return Err(LimitationClosureError::Receipt(
                "peer states do not partition".into(),
            ));
        }
        if self.minimum_peer_quorum == 0
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.closure_digest)
            || self.artifact.content_hash != self.closure_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(LimitationClosureError::Receipt(
                "closure digest, quorum, or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-limitation-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(LimitationClosureError::Receipt(
                "effect is outside the governed limitation-closure gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, LimitationClosureError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| LimitationClosureError::Receipt(error.to_string()))?,
        )
        .map_err(|error| LimitationClosureError::Receipt(error.to_string()))
    }
}

fn validate_request(request: &IdsLimitationClosureRequest7) -> Result<(), LimitationClosureError> {
    if request.request_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_scopes.is_empty()
        || request.cases.is_empty()
        || request.cases.len() > MAX_CASES
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.minimum_peer_quorum < 2
        || request.minimum_peer_quorum as usize > MAX_PEERS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(LimitationClosureError::Invalid(
            "request identity, scope, case/peer bound, quorum, replay, locality, or boundary is invalid".into(),
        ));
    }
    let scopes = BTreeSet::from_iter(request.required_scopes.iter().cloned());
    if scopes.len() != request.required_scopes.len()
        || request
            .required_scopes
            .iter()
            .any(|scope| scope.trim().is_empty())
    {
        return Err(LimitationClosureError::Invalid(
            "required scopes are not unique and non-empty".into(),
        ));
    }
    let mut case_ids = BTreeSet::new();
    for case in &request.cases {
        if case.case_id.trim().is_empty()
            || !case_ids.insert(case.case_id.clone())
            || case.limitation.trim().is_empty()
            || case.scope.trim().is_empty()
            || case
                .evidence_digests
                .iter()
                .any(|digest| !valid_digest(digest))
            || case.closure_criteria.is_empty()
            || case
                .closure_criteria
                .iter()
                .any(|criterion| criterion.trim().is_empty())
            || case.mitigation.trim().is_empty()
            || !valid_digest(&case.replay_identity)
            || !case.local
            || !case.aggregate_only
        {
            return Err(LimitationClosureError::Invalid(format!(
                "limitation case {} is invalid, duplicated, non-local, or not digest-bound",
                case.case_id
            )));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.semantic_profile.trim().is_empty()
            || !valid_digest(&peer.closure_digest)
            || !valid_digest(&peer.replay_identity)
            || !peer.local
            || !peer.aggregate_only
        {
            return Err(LimitationClosureError::Invalid(format!(
                "closure peer {} is invalid, duplicated, or non-local",
                peer.peer_id
            )));
        }
    }
    Ok(())
}

pub fn close_ids_limitations(
    request: &IdsLimitationClosureRequest7,
) -> Result<IdsClosureReceipt9, LimitationClosureError> {
    validate_request(request)?;
    let mut cases = request.cases.clone();
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let case_order = cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let required_scope_order = BTreeSet::from_iter(request.required_scopes.iter().cloned())
        .into_iter()
        .collect::<Vec<_>>();
    let mut resolved = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let required_scopes = BTreeSet::from_iter(request.required_scopes.iter().cloned());
    for case in &cases {
        evidence.extend(case.evidence_digests.iter().cloned());
        if let Some(result) = &case.negative_result {
            negative.insert(format!("{}:{}", case.case_id, result));
        }
        if !required_scopes.contains(&case.scope) {
            unresolved.insert(case.case_id.clone());
            omissions.insert(format!("{}:scope-not-requested", case.case_id));
            continue;
        }
        match case.status {
            LimitationState::Resolved
                if !case.evidence_digests.is_empty() && !case.closure_criteria.is_empty() =>
            {
                if case.replay_identity == request.replay_identity {
                    resolved.insert(case.case_id.clone());
                } else {
                    unresolved.insert(case.case_id.clone());
                    uncertainty.insert(format!("{}:replay-identity", case.case_id));
                }
            }
            LimitationState::Resolved => {
                unresolved.insert(case.case_id.clone());
                omissions.insert(format!(
                    "{}:resolved-without-evidence-or-criteria",
                    case.case_id
                ));
            }
            LimitationState::Measured => {
                unresolved.insert(case.case_id.clone());
                uncertainty.insert(format!("{}:measured-but-not-closed", case.case_id));
            }
            LimitationState::Open => {
                unresolved.insert(case.case_id.clone());
                omissions.insert(format!("{}:limitation-open", case.case_id));
            }
            LimitationState::Unknown => {
                unresolved.insert(case.case_id.clone());
                uncertainty.insert(format!("{}:limitation-unknown", case.case_id));
            }
            LimitationState::Blocked => {
                blocked.insert(case.case_id.clone());
                omissions.insert(format!("{}:limitation-blocked", case.case_id));
            }
            LimitationState::Contradicted => {
                blocked.insert(case.case_id.clone());
                negative.insert(format!("{}:contradicted", case.case_id));
            }
        }
    }
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let id = peer.peer_id.clone();
        if peer.semantic_profile != request.semantic_profile {
            missing_peers.insert(id.clone());
            omissions.insert(format!("{}:semantic-profile", id));
        } else if peer.replay_identity != request.replay_identity {
            missing_peers.insert(id.clone());
            uncertainty.insert(format!("{}:replay-identity", id));
        } else {
            match peer.evidence_state {
                PeerEvidenceState::Qualified => {
                    qualified_peers.insert(id);
                }
                PeerEvidenceState::Unknown => {
                    missing_peers.insert(id.clone());
                    uncertainty.insert(format!("{}:peer-evidence-unknown", id));
                }
                PeerEvidenceState::Contradicted => {
                    missing_peers.insert(id.clone());
                    negative.insert(format!("{}:peer-evidence-contradicted", id));
                }
            }
        }
    }
    let qualified_peer_count = qualified_peers.len() as u16;
    if qualified_peer_count < request.minimum_peer_quorum {
        uncertainty.insert(format!(
            "peer-quorum:{}/{}",
            qualified_peer_count, request.minimum_peer_quorum
        ));
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(case_order.iter().cloned());
        resolved.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let mut disposition = if global_block {
        "blocked"
    } else if resolved.is_empty() && unresolved.is_empty() {
        "blocked"
    } else if resolved.is_empty() {
        "unknown"
    } else if !unresolved.is_empty()
        || !blocked.is_empty()
        || qualified_peer_count < request.minimum_peer_quorum
    {
        "partial"
    } else {
        "closed"
    };
    if !blocked.is_empty() && resolved.is_empty() {
        disposition = "blocked";
    }
    if disposition != "closed" {
        omissions.insert("request:limitation-closure-not-complete".into());
    }
    let resolved_order = resolved.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peers.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peers.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let mut effect_order = if disposition == "closed" {
        vec![
            "exchange:permitted-limitation-digests".to_string(),
            "manage:local-capability".to_string(),
        ]
    } else if disposition == "partial" {
        vec![
            "block:unsafe-release".to_string(),
            "exchange:permitted-limitation-digests".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let mut effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    effect_receipts.sort();
    let mut reasons = vec![format!(
        "{} limitation cases and {} peer attestations evaluated with explicit closure states",
        case_order.len(),
        peer_order.len()
    )];
    if qualified_peer_count < request.minimum_peer_quorum {
        reasons.push("peer quorum is below the requested closure threshold".into());
    }
    if !unresolved_order.is_empty() {
        reasons
            .push("unresolved limitations remain visible and cannot be promoted to closed".into());
    }
    if !blocked_order.is_empty() {
        reasons.push("blocked or contradicted limitations remain visible".into());
    }
    reasons.sort();
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "semantic_profile": request.semantic_profile,
        "required_scope_order": required_scope_order,
        "disposition": disposition,
        "case_order": case_order,
        "resolved_order": resolved_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "minimum_peer_quorum": request.minimum_peer_quorum,
        "qualified_peer_count": qualified_peer_count,
        "evidence_order": evidence.into_iter().collect::<Vec<_>>(),
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "reasons": reasons,
        "effect_order": effect_order,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let closure_digest = ContentHash::of_value(&payload)
        .map_err(|error| LimitationClosureError::Receipt(error.to_string()))?;
    let artifact = IdsClosureReceipt9Artifact {
        artifact_id: format!("ids-closure-receipt-9:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        provenance_digests: Vec::new(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = IdsClosureReceipt9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        required_scope_order: required_scope_order,
        disposition: disposition.into(),
        case_order: payload["case_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        resolved_order: payload["resolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        minimum_peer_quorum: request.minimum_peer_quorum,
        qualified_peer_count,
        evidence_order: payload["evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        reasons: payload["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        effect_order: payload["effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        closure_digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> IdsLimitationClosureRequest7 {
        IdsLimitationClosureRequest7 {
            request_id: "ids:closure:req".into(),
            semantic_profile: "ids-v1".into(),
            required_scopes: vec!["imaging".into(), "omics".into()],
            cases: vec![
                IdsLimitationCase8 {
                    case_id: "case:batch-drift".into(),
                    limitation: "batch drift".into(),
                    scope: "imaging".into(),
                    status: LimitationState::Resolved,
                    evidence_digests: vec![h("drift")],
                    closure_criteria: vec!["independent fixture passed".into()],
                    mitigation: "recalibrate baseline".into(),
                    negative_result: None,
                    replay_identity: h("replay"),
                    local: true,
                    aggregate_only: true,
                },
                IdsLimitationCase8 {
                    case_id: "case:missing-modality".into(),
                    limitation: "missing modality".into(),
                    scope: "omics".into(),
                    status: LimitationState::Open,
                    evidence_digests: vec![],
                    closure_criteria: vec!["modality admitted".into()],
                    mitigation: "retain omission".into(),
                    negative_result: Some("null modality result".into()),
                    replay_identity: h("replay"),
                    local: true,
                    aggregate_only: true,
                },
            ],
            peers: vec![
                IdsClosurePeer7 {
                    peer_id: "peer:a".into(),
                    semantic_profile: "ids-v1".into(),
                    closure_digest: h("peer-a"),
                    evidence_state: PeerEvidenceState::Qualified,
                    replay_identity: h("replay"),
                    local: true,
                    aggregate_only: true,
                },
                IdsClosurePeer7 {
                    peer_id: "peer:b".into(),
                    semantic_profile: "ids-v1".into(),
                    closure_digest: h("peer-b"),
                    evidence_state: PeerEvidenceState::Qualified,
                    replay_identity: h("replay"),
                    local: true,
                    aggregate_only: true,
                },
            ],
            minimum_peer_quorum: 2,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(limitation_closure_manifest()["autonomy_tier"], "A2");
    }

    #[test]
    fn open_limitation_is_partial_and_negative_evidence_is_retained() {
        let receipt = close_ids_limitations(&request()).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert_eq!(receipt.resolved_order, vec!["case:batch-drift"]);
        assert!(!receipt.negative_evidence_order.is_empty());
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn resolved_case_requires_replay_match() {
        let mut request = request();
        request.cases[0].replay_identity = h("other-replay");
        let receipt = close_ids_limitations(&request).unwrap();
        assert_eq!(receipt.disposition, "unknown");
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|value| value.contains("replay-identity")));
    }

    #[test]
    fn peer_quorum_is_explicit() {
        let mut request = request();
        request.peers[1].evidence_state = PeerEvidenceState::Unknown;
        let receipt = close_ids_limitations(&request).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|value| value.starts_with("peer-quorum:")));
    }

    #[test]
    fn contradicted_case_is_blocked() {
        let mut request = request();
        request.cases[0].status = LimitationState::Contradicted;
        let receipt = close_ids_limitations(&request).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.blocked_order.contains(&"case:batch-drift".into()));
    }

    #[test]
    fn governance_denial_blocks_all_cases() {
        let mut request = request();
        request.policy_allow = false;
        let receipt = close_ids_limitations(&request).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.blocked_order.len(), 2);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn closure_digest_is_deterministic() {
        let first = close_ids_limitations(&request()).unwrap();
        let second = close_ids_limitations(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
