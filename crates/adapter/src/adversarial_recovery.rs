//! Federated continual adversarial recovery interoperability gateway.
//!
//! Atlas feature: `AFA-adapter-P30-F24`.
//! Models hostile and failure events as replayable recovery metadata. No event is silently
//! discarded, and recovery receipts never authorize a remote effect.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P30-F24";
pub const CONTRACT_VERSION: &str = "adapter-adversarial-recovery/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_EVENTS: usize = 8192;
const MAX_NOTE_ITEMS: usize = 16384;

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
pub struct RecoveryReplayLink {
    pub event_id: String,
    pub checkpoint_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialRecoveryReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: AdversarialRecoveryRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: RecoveryDisposition,
    pub event_order: Vec<String>,
    pub payload_digest_order: Vec<ContentHash>,
    pub recovered_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub replay_order: Vec<String>,
    pub checkpoint_order: Vec<ContentHash>,
    pub replay_links: Vec<RecoveryReplayLink>,
    pub policy_allow: bool,
    pub protected_closure: bool,
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
            || self.event_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery identity, events, checks, effects, locality, or boundary are incomplete"
                    .into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_sorted_strings(&self.event_order, "event_order", MAX_EVENTS)?;
        validate_sorted_strings(&self.recovered_order, "recovered_order", MAX_EVENTS)?;
        validate_sorted_strings(&self.blocked_order, "blocked_order", MAX_EVENTS)?;
        validate_sorted_strings(&self.replay_order, "replay_order", MAX_EVENTS)?;
        validate_sorted_strings(&self.checks, "checks", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.omissions, "omissions", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.uncertainty, "uncertainty", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.negative_evidence, "negative_evidence", MAX_NOTE_ITEMS)?;
        validate_sorted_strings(&self.effect_receipts, "effect_receipts", MAX_NOTE_ITEMS)?;
        if self.payload_digest_order.len() != self.event_order.len()
            || self.payload_digest_order.len() > MAX_EVENTS
            || self
                .payload_digest_order
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery payload digests must align with every event".into(),
            ));
        }
        if self.checkpoint_order.len() > MAX_EVENTS
            || self
                .checkpoint_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .checkpoint_order
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery digest ordering is not canonical".into(),
            ));
        }
        if self.replay_links.len() > MAX_EVENTS
            || self
                .replay_links
                .windows(2)
                .any(|pair| pair[0].event_id >= pair[1].event_id)
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery replay links are not canonical".into(),
            ));
        }
        for link in &self.replay_links {
            validate_text("replay_link.event_id", &link.event_id)?;
            if link.checkpoint_digest == ContentHash::of_bytes(b"") {
                return Err(AdversarialRecoveryError::Invalid(
                    "replay link checkpoint digest cannot be empty".into(),
                ));
            }
        }
        let event_ids = self.event_order.iter().collect::<BTreeSet<_>>();
        let recovered_ids = self.recovered_order.iter().collect::<BTreeSet<_>>();
        let blocked_ids = self.blocked_order.iter().collect::<BTreeSet<_>>();
        if recovered_ids.intersection(&blocked_ids).next().is_some()
            || recovered_ids
                .union(&blocked_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != event_ids
            || self
                .replay_order
                .iter()
                .any(|event| !event_ids.contains(event))
            || self.replay_order.is_empty() != self.checkpoint_order.is_empty()
            || self.checkpoint_order.len() > self.replay_order.len()
            || self.replay_links.len() != self.replay_order.len()
            || self
                .replay_links
                .iter()
                .map(|link| link.event_id.clone())
                .collect::<Vec<_>>()
                != self.replay_order
            || self
                .replay_links
                .iter()
                .map(|link| link.checkpoint_digest.clone())
                .collect::<BTreeSet<_>>()
                != self.checkpoint_order.iter().cloned().collect()
        {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery event partition or replay/checkpoint linkage is inconsistent".into(),
            ));
        }
        let expected_disposition = if !self.policy_allow {
            RecoveryDisposition::Blocked
        } else if !self.protected_closure {
            RecoveryDisposition::Unknown
        } else if blocked_ids.is_empty() {
            RecoveryDisposition::Recovered
        } else if recovered_ids.is_empty() {
            RecoveryDisposition::Unknown
        } else {
            RecoveryDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery disposition does not match policy, closure, and event partition".into(),
            ));
        }
        if self.checks != canonical_checks() {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery checks are not canonical".into(),
            ));
        }
        let expected_effect = match self.disposition {
            RecoveryDisposition::Recovered | RecoveryDisposition::Partial => {
                "exchange:permitted-recovery-checkpoints-and-digests-only"
            }
            RecoveryDisposition::Blocked => "block:adversarial-recovery:blocked",
            RecoveryDisposition::Unknown => "block:adversarial-recovery:unknown",
        };
        if self.effect_receipts != vec![expected_effect.to_string()] {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery effect does not match the disposition".into(),
            ));
        }
        let expected_recovery_digest = ContentHash::of_value(&recovery_digest_payload(self))
            .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))?;
        if self.recovery_digest.as_ref() != Some(&expected_recovery_digest) {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery digest does not bind the receipt state".into(),
            ));
        }
        if self.artifact.artifact_id != format!("adapter-adversarial-recovery:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.adapter-adversarial-recovery+json"
            || !self.artifact.semantic_loss.is_empty()
        {
            return Err(AdversarialRecoveryError::Artifact(
                "recovery artifact is not bound to the receipt".into(),
            ));
        }
        let expected_provenance = self
            .event_order
            .iter()
            .zip(&self.payload_digest_order)
            .map(|(event_id, digest)| ProvenanceLink {
                source_id: event_id.clone(),
                relation: "observed-recovery-event".into(),
                digest: digest.clone(),
            })
            .collect::<Vec<_>>();
        if self.artifact.provenance != expected_provenance {
            return Err(AdversarialRecoveryError::Artifact(
                "recovery artifact provenance is not bound to event payloads".into(),
            ));
        }
        let payload = recovery_artifact_payload(self);
        self.artifact
            .verify_payload(&payload)
            .map_err(|e| AdversarialRecoveryError::Artifact(e.to_string()))?;
        if self.input_digest != recovery_input_digest(&self.input)? {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery retained input digest mismatch".into(),
            ));
        }
        validate_request(&self.input)?;
        let expected = build_adversarial_recovery_receipt(&self.input)?;
        if self != &expected {
            return Err(AdversarialRecoveryError::Invalid(
                "recovery receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, AdversarialRecoveryError> {
        self.validate()?;
        let v = serde_json::to_value(self)
            .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))?;
        ContentHash::of_value(&v)
            .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), AdversarialRecoveryError> {
    if value.is_empty() || value.trim() != value {
        return Err(AdversarialRecoveryError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(AdversarialRecoveryError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_sorted_strings(
    values: &[String],
    field: &str,
    max_items: usize,
) -> Result<(), AdversarialRecoveryError> {
    if values.len() > max_items {
        return Err(AdversarialRecoveryError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(AdversarialRecoveryError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AdversarialRecoveryError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn recovery_digest_payload(receipt: &AdversarialRecoveryReceipt) -> serde_json::Value {
    recovery_payload_from_parts(
        &receipt.request_id,
        &receipt.workflow_id,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.disposition,
        &receipt.event_order,
        &receipt.payload_digest_order,
        &receipt.recovered_order,
        &receipt.blocked_order,
        &receipt.replay_order,
        &receipt.checkpoint_order,
        &receipt.replay_links,
        None,
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

fn recovery_input_digest(
    request: &AdversarialRecoveryRequest,
) -> Result<ContentHash, AdversarialRecoveryError> {
    let value = serde_json::to_value(&canonical_adversarial_recovery_request(request))
        .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))
}

fn canonical_adversarial_recovery_request(
    request: &AdversarialRecoveryRequest,
) -> AdversarialRecoveryRequest {
    let mut canonical = request.clone();
    canonical
        .events
        .sort_by(|left, right| left.event_id.cmp(&right.event_id));
    canonical
}

fn canonical_checks() -> Vec<String> {
    vec![
        "adversarial event kinds fail closed without remote effects".into(),
        "checkpoints and replay identities remain content-addressed".into(),
        "events are ordered by stable id".into(),
    ]
}

fn recovery_artifact_payload(receipt: &AdversarialRecoveryReceipt) -> serde_json::Value {
    recovery_payload_from_parts(
        &receipt.request_id,
        &receipt.workflow_id,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.disposition,
        &receipt.event_order,
        &receipt.payload_digest_order,
        &receipt.recovered_order,
        &receipt.blocked_order,
        &receipt.replay_order,
        &receipt.checkpoint_order,
        &receipt.replay_links,
        receipt.recovery_digest.as_ref(),
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn recovery_payload_from_parts(
    request_id: &str,
    workflow_id: &str,
    policy_allow: bool,
    protected_closure: bool,
    disposition: RecoveryDisposition,
    event_order: &[String],
    payload_digest_order: &[ContentHash],
    recovered_order: &[String],
    blocked_order: &[String],
    replay_order: &[String],
    checkpoint_order: &[ContentHash],
    replay_links: &[RecoveryReplayLink],
    recovery_digest: Option<&ContentHash>,
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    effect_receipts: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    let mut payload_object = serde_json::Map::new();
    payload_object.insert("request_id".into(), json!(request_id));
    payload_object.insert("workflow_id".into(), json!(workflow_id));
    payload_object.insert("policy_allow".into(), json!(policy_allow));
    payload_object.insert("protected_closure".into(), json!(protected_closure));
    payload_object.insert("disposition".into(), json!(disposition));
    payload_object.insert("event_order".into(), json!(event_order));
    payload_object.insert("payload_digest_order".into(), json!(payload_digest_order));
    payload_object.insert("recovered_order".into(), json!(recovered_order));
    payload_object.insert("blocked_order".into(), json!(blocked_order));
    payload_object.insert("replay_order".into(), json!(replay_order));
    payload_object.insert("checkpoint_order".into(), json!(checkpoint_order));
    payload_object.insert("replay_links".into(), json!(replay_links));
    payload_object.insert("checks".into(), json!(checks));
    payload_object.insert("omissions".into(), json!(omissions));
    payload_object.insert("uncertainty".into(), json!(uncertainty));
    payload_object.insert("negative_evidence".into(), json!(negative_evidence));
    payload_object.insert("effect_receipts".into(), json!(effect_receipts));
    payload_object.insert("raw_data_local".into(), json!(raw_data_local));
    payload_object.insert("boundary".into(), json!(boundary));
    if let Some(recovery_digest) = recovery_digest {
        payload_object.insert("recovery_digest".into(), json!(recovery_digest));
    }
    if recovery_digest.is_some() {
        payload_object.insert(
            "schema_version".into(),
            json!(RESEARCH_CONTRACT_SCHEMA_VERSION),
        );
        payload_object.insert("contract_version".into(), json!(CONTRACT_VERSION));
        payload_object.insert("feature_id".into(), json!(FEATURE_ID));
    }
    Value::Object(payload_object)
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
    let receipt = build_adversarial_recovery_receipt(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_adversarial_recovery_receipt(
    request: &AdversarialRecoveryRequest,
) -> Result<AdversarialRecoveryReceipt, AdversarialRecoveryError> {
    let mut events = request.events.clone();
    events.sort_by(|a, b| a.event_id.cmp(&b.event_id));
    let event_order = events
        .iter()
        .map(|e| e.event_id.clone())
        .collect::<Vec<_>>();
    let payload_digest_order = events
        .iter()
        .map(|e| e.payload_digest.clone())
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
    let replay_links = events
        .iter()
        .filter_map(|event| {
            event
                .checkpoint_digest
                .clone()
                .map(|checkpoint_digest| RecoveryReplayLink {
                    event_id: event.event_id.clone(),
                    checkpoint_digest,
                })
        })
        .collect::<Vec<_>>();
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
    let checks = canonical_checks();
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
    let recovery_payload = recovery_payload_from_parts(
        &request.request_id,
        &request.workflow_id,
        request.policy_allow,
        request.protected_closure,
        disposition,
        &event_order,
        &payload_digest_order,
        &recovered_order,
        &blocked_order,
        &replay_order,
        &checkpoint_order,
        &replay_links,
        None,
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let recovery_digest = ContentHash::of_value(&recovery_payload)
        .map_err(|e| AdversarialRecoveryError::Serialization(e.to_string()))?;
    let payload = recovery_payload_from_parts(
        &request.request_id,
        &request.workflow_id,
        request.policy_allow,
        request.protected_closure,
        disposition,
        &event_order,
        &payload_digest_order,
        &recovered_order,
        &blocked_order,
        &replay_order,
        &checkpoint_order,
        &replay_links,
        Some(&recovery_digest),
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let provenance = event_order
        .iter()
        .zip(&payload_digest_order)
        .map(|(event_id, digest)| ProvenanceLink {
            source_id: event_id.clone(),
            relation: "observed-recovery-event".into(),
            digest: digest.clone(),
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-adversarial-recovery:{}", request.request_id),
        "application/vnd.aurora.adapter-adversarial-recovery+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|e| AdversarialRecoveryError::Artifact(e.to_string()))?;
    let receipt = AdversarialRecoveryReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_adversarial_recovery_request(request),
        input_digest: recovery_input_digest(request)?,
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        event_order,
        payload_digest_order,
        recovered_order,
        blocked_order,
        replay_order,
        checkpoint_order,
        replay_links,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        recovery_digest: Some(recovery_digest),
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &AdversarialRecoveryRequest) -> Result<(), AdversarialRecoveryError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(AdversarialRecoveryError::Invalid(
            "recovery identity, events, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("boundary", &request.boundary)?;
    let mut ids = BTreeSet::new();
    for e in &request.events {
        validate_text("event_id", &e.event_id)?;
        validate_text("event_kind", &e.event_kind)?;
        if !ids.insert(e.event_id.clone()) || e.boundary != PRECLINICAL_BOUNDARY {
            return Err(AdversarialRecoveryError::Invalid(format!(
                "event {} is invalid or duplicated",
                e.event_id
            )));
        }
        validate_text("event.boundary", &e.boundary)?;
        if e.payload_digest == ContentHash::of_bytes(b"")
            || e.checkpoint_digest
                .as_ref()
                .is_some_and(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(AdversarialRecoveryError::Invalid(
                "event payload and checkpoint digests cannot be empty".into(),
            ));
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

    #[test]
    fn recovery_digest_binds_the_event_partition() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.blocked_order.clear();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn replay_requires_checkpoint_evidence() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.checkpoint_order.clear();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn empty_event_digest_is_rejected() {
        let mut value = q();
        value.events[0].payload_digest = ContentHash::of_bytes(b"");
        assert!(recover_adversarial_events(&value).is_err());
    }

    #[test]
    fn forged_effect_cannot_change_recovery_authority() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.effect_receipts = vec!["external:remote-execution".into()];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn replay_links_retain_event_identity() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.replay_links[0].event_id = receipt.replay_links[1].event_id.clone();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn recovery_artifact_payload_is_verified() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn policy_state_is_bound_to_recovery_disposition() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.policy_allow = false;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn receipt_rejects_tampered_retained_request() {
        let mut receipt = recover_adversarial_events(&q()).unwrap();
        receipt.input.events[0].event_kind = "tampered-kind".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
