//! Adversarial-recovery research workbench (`AFA-ids-P30-F18`).
//!
//! Turns hostile or failed workflow events into deterministic, replayable recovery metadata.
//! The workbench never retries jobs, invokes connectors, moves raw data, or grants authority.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P30-F18";
pub const CONTRACT_VERSION: &str = "ids-adversarial-recovery-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "IdsAdversarialRecoveryRequest8@1";
pub const OUTPUT_SCHEMA: &str = "IdsAdversarialRecoveryReceipt10@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-adversarial-recovery-receipt-10+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_EVENTS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsRecoveryEvent7 {
    pub event_id: String,
    pub event_kind: String,
    pub payload_digest: ContentHash,
    pub checkpoint_digest: Option<ContentHash>,
    pub evidence_state: RecoveryEvidenceState,
    pub authorized: bool,
    pub recoverable: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsAdversarialRecoveryRequest8 {
    pub request_id: String,
    pub workflow_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub events: Vec<IdsRecoveryEvent7>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsAdversarialRecoveryReceipt10Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsAdversarialRecoveryReceipt10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub event_order: Vec<String>,
    pub recovered_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub hostile_order: Vec<String>,
    pub replay_order: Vec<String>,
    pub checkpoint_digest_order: Vec<ContentHash>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub recovery_digest: ContentHash,
    pub artifact: IdsAdversarialRecoveryReceipt10Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdversarialRecoveryWorkbenchError {
    #[error("invalid IDS adversarial-recovery request: {0}")]
    Invalid(String),
    #[error("IDS adversarial-recovery report failed validation: {0}")]
    Report(String),
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
fn hostile(kind: &str) -> bool {
    matches!(
        kind,
        "revoked_key"
            | "poisoned_artifact"
            | "prompt_injection"
            | "compromised_connector"
            | "resource_exhaustion"
            | "unauthorized_data_movement"
            | "instrument_preflight_failure"
    )
}

pub fn adversarial_recovery_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION,
        "owner_crate":"ids", "consumers":["researcher", "recovery operator", "security engineer"],
        "behavior":"classify hostile and failed workflow events into deterministic replayable recovery states with counterexamples and omission witnesses",
        "value":"makes recovery posture auditable before any retry, connector, instrument, or federation effect is considered",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA,
        "effects":["preview:adversarial-recovery", "manage:local-capability"],
        "permissions":["read:local-recovery-summaries", "request:adversarial-recovery-preview"],
        "autonomy_tier":"A1", "boundary":PRECLINICAL_BOUNDARY
    })
}

impl IdsAdversarialRecoveryReceipt10 {
    pub fn validate(&self) -> Result<(), AdversarialRecoveryWorkbenchError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.event_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(AdversarialRecoveryWorkbenchError::Report(
                "recovery identity, events, effects, locality, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.event_order,
            &self.recovered_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.hostile_order,
            &self.replay_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(AdversarialRecoveryWorkbenchError::Report(
                    "recovery ordering is not canonical".into(),
                ));
            }
        }
        if !ordered(&self.checkpoint_digest_order) {
            return Err(AdversarialRecoveryWorkbenchError::Report(
                "checkpoint digest ordering is not canonical".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.event_order.iter().cloned());
        let parts = self
            .recovered_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.event_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.recovery_digest)
            || self.artifact.content_hash != self.recovery_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(AdversarialRecoveryWorkbenchError::Report(
                "recovery states, digests, or artifact metadata do not partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("preview:adversarial-recovery:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(AdversarialRecoveryWorkbenchError::Report(
                "effect is outside the governed recovery gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, AdversarialRecoveryWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| AdversarialRecoveryWorkbenchError::Report(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| AdversarialRecoveryWorkbenchError::Report(error.to_string()))
    }
}

fn validate_request(
    request: &IdsAdversarialRecoveryRequest8,
) -> Result<(), AdversarialRecoveryWorkbenchError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(AdversarialRecoveryWorkbenchError::Invalid(
            "recovery identity, event bound, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for event in &request.events {
        if event.event_id.trim().is_empty()
            || !ids.insert(event.event_id.clone())
            || event.event_kind.trim().is_empty()
            || !valid_digest(&event.payload_digest)
            || event
                .checkpoint_digest
                .as_ref()
                .is_some_and(|digest| !valid_digest(digest))
            || !valid_digest(&event.replay_identity)
            || !event.local
            || !event.aggregate_only
        {
            return Err(AdversarialRecoveryWorkbenchError::Invalid(format!(
                "recovery event {} is invalid, duplicated, non-local, or not digest-bound",
                event.event_id
            )));
        }
    }
    Ok(())
}

pub fn preview_adversarial_recovery(
    request: &IdsAdversarialRecoveryRequest8,
) -> Result<IdsAdversarialRecoveryReceipt10, AdversarialRecoveryWorkbenchError> {
    validate_request(request)?;
    let mut events = request.events.clone();
    events.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    let event_order = events
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    let mut recovered = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut hostile_order = BTreeSet::new();
    let mut replay = BTreeSet::new();
    let mut checkpoints = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for event in &events {
        provenance.insert(event.payload_digest.clone());
        if let Some(checkpoint) = &event.checkpoint_digest {
            replay.insert(event.event_id.clone());
            checkpoints.insert(checkpoint.clone());
        }
        if hostile(&event.event_kind) {
            hostile_order.insert(event.event_id.clone());
            negative.insert(format!(
                "{}:adversarial-kind-{}",
                event.event_id, event.event_kind
            ));
        }
        if !event.authorized {
            blocked.insert(event.event_id.clone());
            omissions.insert(format!("{}:authorization-denied", event.event_id));
        } else if !event.recoverable {
            blocked.insert(event.event_id.clone());
            omissions.insert(format!("{}:non-recoverable", event.event_id));
        } else if event.replay_identity != request.replay_identity {
            unresolved.insert(event.event_id.clone());
            uncertainty.insert(format!("{}:replay-identity", event.event_id));
        } else if event.evidence_state == RecoveryEvidenceState::Contradicted
            || hostile(&event.event_kind)
        {
            blocked.insert(event.event_id.clone());
        } else if !matches!(
            event.evidence_state,
            RecoveryEvidenceState::Proven | RecoveryEvidenceState::Supported
        ) {
            unresolved.insert(event.event_id.clone());
            uncertainty.insert(format!("{}:evidence-state", event.event_id));
        } else {
            recovered.insert(event.event_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(event_order.iter().cloned());
        recovered.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let recovered_order = recovered.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global || recovered_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty() || !unresolved_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:adversarial-recovery-not-closed".into());
    }
    let effect_order = if disposition == "qualified" {
        vec![
            "manage:local-capability".to_string(),
            "preview:adversarial-recovery".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    let effect_order = {
        let mut values = effect_order;
        values.sort();
        values
    };
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"event_order":event_order,"recovered_order":recovered_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"hostile_order":hostile_order.into_iter().collect::<Vec<_>>(),"replay_order":replay.into_iter().collect::<Vec<_>>(),"checkpoint_digest_order":checkpoints.into_iter().collect::<Vec<_>>(),"omission_order":omissions.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| AdversarialRecoveryWorkbenchError::Report(error.to_string()))?;
    let report = IdsAdversarialRecoveryReceipt10 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        event_order: payload["event_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        recovered_order: payload["recovered_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        hostile_order: payload["hostile_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        replay_order: payload["replay_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        checkpoint_digest_order: serde_json::from_value(payload["checkpoint_digest_order"].clone())
            .unwrap(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        effect_order: payload["effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        recovery_digest: digest.clone(),
        artifact: IdsAdversarialRecoveryReceipt10Artifact {
            artifact_id: format!("ids-adversarial-recovery-receipt-10:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: payload["omission_order"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: effect_order
            .iter()
            .map(|effect| {
                if effect == "block:unsafe-release" {
                    effect.clone()
                } else {
                    format!("{effect}:{}", request.request_id)
                }
            })
            .collect(),
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
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn event(id: &str) -> IdsRecoveryEvent7 {
        IdsRecoveryEvent7 {
            event_id: id.into(),
            event_kind: "checkpoint".into(),
            payload_digest: h("payload"),
            checkpoint_digest: Some(h("checkpoint")),
            evidence_state: RecoveryEvidenceState::Supported,
            authorized: true,
            recoverable: true,
            local: true,
            aggregate_only: true,
            replay_identity: h("replay"),
        }
    }
    fn request() -> IdsAdversarialRecoveryRequest8 {
        IdsAdversarialRecoveryRequest8 {
            request_id: "request:recovery".into(),
            workflow_id: "workflow:recovery".into(),
            purpose: "recover".into(),
            semantic_profile: "ids-v1".into(),
            events: vec![event("event:b"), event("event:a")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(adversarial_recovery_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            preview_adversarial_recovery(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn hostile_is_blocked() {
        let mut q = request();
        q.events[0].event_kind = "prompt_injection".into();
        assert_eq!(
            preview_adversarial_recovery(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn replay_mismatch_is_unresolved() {
        let mut q = request();
        q.events[0].replay_identity = h("other");
        assert_eq!(
            preview_adversarial_recovery(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            preview_adversarial_recovery(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = preview_adversarial_recovery(&request()).unwrap();
        let b = preview_adversarial_recovery(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
