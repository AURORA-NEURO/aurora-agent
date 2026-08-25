//! Federated continual adversarial recovery interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P30-F24`.
//! Models hostile and failure events as replayable recovery metadata. No event is silently
//! discarded, and recovery receipts never authorize a remote effect.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P30-F24";
pub const CONTRACT_VERSION: &str = "adapter-adversarial-recovery/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEvent {
    pub event_id: String,
    pub event_kind: String,
    pub payload_digest: ContentHash,
    pub checkpoint_digest: Option<ContentHash>,
    pub authorized: bool,
    pub recoverable: bool,
    pub local_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialRecoveryRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub events: Vec<RecoveryEvent>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    Recovered,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialRecoveryReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: RecoveryDisposition,
    pub event_order: Vec<String>,
    pub recovered_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub replay_order: Vec<String>,
    pub checkpoint_order: Vec<ContentHash>,
    pub recovery_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl AdversarialRecoveryReceipt {
    pub fn validate(&self) -> Result<(), AdversarialRecoveryError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.event_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery identity, events, checks, effects, locality, or boundary are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.event_order,
            &self.recovered_order,
            &self.blocked_order,
            &self.replay_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AdversarialRecoveryError::Invalid(
                    "recovery ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.checkpoint_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AdversarialRecoveryError::Invalid(
                    "recovery digest ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| AdversarialRecoveryError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, AdversarialRecoveryError> {
        self.validate()?;
        let v = serde_json::to_value(self)
            .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))?;
        ContentHash::of_value(&v)
            .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum AdversarialRecoveryError {
    #[error("invalid adversarial recovery request: {0}")]
    Invalid(String),
    #[error("adversarial recovery artifact error: {0}")]
    Artifact(String),
    #[error("adversarial recovery serialization error: {0}")]
    Serialization(String),
}

pub fn recover_adversarial_events(
    request: &AdversarialRecoveryRequest,
) -> Result<AdversarialRecoveryReceipt, AdversarialRecoveryError> {
    validate_request(request)?;
    let mut events = request.events.clone();
    events.sort_by(|a, b| a.event_id.cmp(&b.event_id));
    let event_order = events
        .iter()
        .map(|e| e.event_id.clone())
        .collect::<Vec<_>>();
    let mut recovered = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut replay = BTreeSet::new();
    let mut checkpoints = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for e in &events {
        if let Some(d) = &e.checkpoint_digest {
            replay.insert(e.event_id.clone());
            checkpoints.insert(d.clone());
        }
        let hostile = matches!(
            e.event_kind.as_str(),
            "revoked_key" | "poisoned_artifact" | "prompt_injection" | "resource_exhaustion"
        );
        if !e.authorized || !e.recoverable || !e.local_only || hostile {
            blocked.insert(e.event_id.clone());
            if !e.authorized {
                omissions.insert(format!("event:{}:authorization-denied", e.event_id));
            }
            if !e.recoverable {
                omissions.insert(format!("event:{}:non-recoverable", e.event_id));
            }
            if !e.local_only {
                omissions.insert(format!("event:{}:raw-data-locality-uncertain", e.event_id));
            }
            if hostile {
                negative.insert(format!(
                    "event:{}:adversarial-kind-{}",
                    e.event_id, e.event_kind
                ));
            }
        } else {
            recovered.insert(e.event_id.clone());
        }
    }
    let recovered_order = recovered.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let replay_order = replay.into_iter().collect::<Vec<_>>();
    let checkpoint_order = checkpoints.into_iter().collect::<Vec<_>>();
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    let disposition = if !request.policy_allow {
        RecoveryDisposition::Blocked
    } else if !request.protected_closure {
        RecoveryDisposition::Unknown
    } else if blocked_order.is_empty() {
        RecoveryDisposition::Recovered
    } else if recovered_order.is_empty() {
        RecoveryDisposition::Unknown
    } else {
        RecoveryDisposition::Partial
    };
    let mut checks = vec![
        "events are ordered by stable id".into(),
        "checkpoints and replay identities remain content-addressed".into(),
        "adversarial event kinds fail closed without remote effects".into(),
    ];
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(
        disposition,
        RecoveryDisposition::Recovered | RecoveryDisposition::Partial
    ) {
        vec!["exchange:permitted-recovery-checkpoints-and-digests-only".into()]
    } else {
        vec![format!("block:adversarial-recovery:{disposition:?}").to_lowercase()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"disposition":disposition,"event_order":event_order,"recovered_order":recovered_order,"blocked_order":blocked_order,"replay_order":replay_order,"checkpoint_order":checkpoint_order,"checks":checks,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative_evidence,"effect_receipts":effect_receipts,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-adversarial-recovery:{}", request.request_id),
        "application/vnd.aurora.adapter-adversarial-recovery+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| AdversarialRecoveryError::Artifact(e.to_string()))?;
    let receipt = AdversarialRecoveryReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        event_order,
        recovered_order,
        blocked_order,
        replay_order,
        checkpoint_order,
        recovery_digest: None,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &AdversarialRecoveryRequest) -> Result<(), AdversarialRecoveryError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.events.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(AdversarialRecoveryError::Invalid(
            "recovery identity, events, locality, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for e in &request.events {
        if e.event_id.trim().is_empty()
            || !ids.insert(e.event_id.clone())
            || e.event_kind.trim().is_empty()
            || e.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(AdversarialRecoveryError::Invalid(format!(
                "event {} is invalid or duplicated",
                e.event_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn e(id: &str, k: &str, auth: bool, recover: bool) -> RecoveryEvent {
        RecoveryEvent {
            event_id: id.into(),
            event_kind: k.into(),
            payload_digest: ContentHash::of_bytes(id.as_bytes()),
            checkpoint_digest: Some(ContentHash::of_bytes(format!("checkpoint:{id}").as_bytes())),
            authorized: auth,
            recoverable: recover,
            local_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn q() -> AdversarialRecoveryRequest {
        AdversarialRecoveryRequest {
            request_id: "recovery:adapter".into(),
            workflow_id: "workflow:federated".into(),
            events: vec![
                e("event:a", "crash", true, true),
                e("event:b", "poisoned_artifact", true, true),
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn partial_recovery_retains_hostile_event() {
        let r = recover_adversarial_events(&q()).unwrap();
        assert_eq!(r.disposition, RecoveryDisposition::Partial);
        assert!(!r.negative_evidence.is_empty());
    }
    #[test]
    fn full_recovery_has_recovered_state() {
        let mut q = q();
        q.events[1].event_kind = "retry".into();
        let r = recover_adversarial_events(&q).unwrap();
        assert_eq!(r.disposition, RecoveryDisposition::Recovered);
        assert_eq!(r.recovered_order.len(), 2);
    }
    #[test]
    fn protected_gap_is_unknown() {
        let mut q = q();
        q.protected_closure = false;
        assert_eq!(
            recover_adversarial_events(&q).unwrap().disposition,
            RecoveryDisposition::Unknown
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = q();
        q.policy_allow = false;
        assert_eq!(
            recover_adversarial_events(&q).unwrap().disposition,
            RecoveryDisposition::Blocked
        );
    }
    #[test]
    fn unauthorized_event_blocks() {
        let mut q = q();
        q.events[0].authorized = false;
        let r = recover_adversarial_events(&q).unwrap();
        assert!(r.blocked_order.contains(&"event:a".into()));
    }
}
