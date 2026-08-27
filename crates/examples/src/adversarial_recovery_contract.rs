//! Typed adversarial-recovery records for reference examples.
//!
//! Atlas feature: `AFA-examples-P30-F08`.
//!
//! The recovery primitive classifies hostile or failed example-world events and records a
//! deterministic, replayable compensation plan. It does not execute retries, open network
//! connections, or move research data; it supplies the typed evidence a governed operator can
//! use to decide whether a later local recovery run is safe.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-examples-P30-F08";
pub const CONTRACT_VERSION: &str = "examples-federated-continual-adversarial-recovery-contract/1.0";
pub const INPUT_SCHEMA: &str = "ExamplesAdversarialCase4@1";
pub const OUTPUT_SCHEMA: &str = "ExamplesRecoveryRecord2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryEvent {
    pub event_id: String,
    pub class: String,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub recoverable: bool,
    pub retry_cost: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExamplesAdversarialCase {
    pub case_id: String,
    pub scenario_id: String,
    pub scope: String,
    pub schema_version: String,
    pub events: Vec<RecoveryEvent>,
    pub replay_identity: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExamplesRecoveryRecord {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub case_id: String,
    pub scenario_id: String,
    pub scope: String,
    pub disposition: String,
    pub event_order: Vec<String>,
    pub recovered_order: Vec<String>,
    pub pending_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensated_order: Vec<String>,
    pub class_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub recovery_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoveryError {
    #[error("invalid adversarial recovery case: {0}")]
    Invalid(String),
    #[error("recovery artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ExamplesRecoveryRecord {
    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.case_id.trim().is_empty()
            || self.scenario_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.event_order.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.effect_receipts.is_empty()
        {
            return Err(RecoveryError::Invalid(
                "record identity, events, locality, boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.event_order,
            &self.recovered_order,
            &self.pending_order,
            &self.blocked_order,
            &self.compensated_order,
            &self.class_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(RecoveryError::Invalid(
                    "recovery ordering is not canonical".into(),
                ));
            }
        }
        let partition = self
            .recovered_order
            .iter()
            .chain(self.pending_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.event_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.event_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(RecoveryError::Invalid(
                "recovery dispositions do not partition events".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| effect != "retain:recovery-record" && effect != "block:unsafe-release")
        {
            return Err(RecoveryError::Invalid(
                "recovery effect is outside typed-record retention".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RecoveryError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RecoveryError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| RecoveryError::Artifact(error.to_string()))?,
        )
        .map_err(|error| RecoveryError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "examples".into(),
        consumers: BTreeSet::from(["research program lead".into(), "reference-world evaluator".into(), "recovery operator".into()]),
        behavior: "classifies adversarial example-world events into deterministic recovery and compensation records".into(), value: "makes hostile inputs, replay mismatches, omissions, and negative outcomes inspectable without silent recovery".into(),
        inputs: vec![TypedPort { name: "adversarial_case".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "recovery_record".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]), permissions: BTreeSet::from(["read:local-research-artifacts".into()]), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }, EvidenceReference { source_id: "anndata-format".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "recovery-reviewer".into(), reason: "review pending or blocked recovery before any retry".into() }], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Protocol, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_case(case: &ExamplesAdversarialCase) -> Result<(), RecoveryError> {
    if case.schema_version != INPUT_SCHEMA
        || case.case_id.trim().is_empty()
        || case.scenario_id.trim().is_empty()
        || case.scope.trim().is_empty()
        || case.events.is_empty()
        || case.artifact_digest.is_none()
        || case.provenance_digest.is_none()
        || case.budget_units == 0
        || case.max_budget_units == 0
        || case.budget_units > case.max_budget_units
        || !case.raw_data_local
        || case.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RecoveryError::Invalid(
            "case identity, events, artifacts, budget, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for event in &case.events {
        if event.event_id.trim().is_empty()
            || !ids.insert(event.event_id.clone())
            || event.class.trim().is_empty()
            || event.source_digest.is_none()
            || event.provenance_digest.is_none()
            || event.retry_cost == 0
        {
            return Err(RecoveryError::Invalid(
                "event identity, class, digests, or retry cost is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn classify(case: &ExamplesAdversarialCase) -> Result<ExamplesRecoveryRecord, RecoveryError> {
    validate_case(case)?;
    let mut event_order = case
        .events
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    event_order.sort();
    let events = case
        .events
        .iter()
        .map(|event| (event.event_id.clone(), event))
        .collect::<BTreeMap<_, _>>();
    let mut recovered = Vec::new();
    let mut pending = Vec::new();
    let mut blocked = Vec::new();
    let mut compensated = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut spent = 0_u32;
    for event_id in &event_order {
        let event = events[event_id];
        classes.insert(event.class.clone());
        negative.insert(format!(
            "{}:{}",
            event_id,
            if event.recoverable {
                "negative-result-not-observed"
            } else {
                "recovery-unavailable"
            }
        ));
        for item in &event.omissions {
            omissions.insert(format!("{}:{}", event_id, item));
        }
        for item in &event.uncertainty {
            uncertainty.insert(format!("{}:{}", event_id, item));
        }
        if event.evidence_state == EvidenceState::Contradicted {
            blocked.push(event_id.clone());
            compensated.insert(event_id.clone());
            semantic_loss.push(SemanticLoss {
                field: format!("event:{event_id}"),
                reason: "contradicted recovery evidence cannot be replayed".into(),
                severity: LossSeverity::DecisionRelevant,
            });
            continue;
        }
        if matches!(
            event.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            pending.push(event_id.clone());
            uncertainty.insert(format!("{}:evidence-state", event_id));
            compensated.insert(event_id.clone());
            continue;
        }
        if !case.policy_allow || !case.protected_closure {
            blocked.push(event_id.clone());
            compensated.insert(event_id.clone());
            continue;
        }
        if !event.recoverable || event.retry_cost > case.budget_units.saturating_sub(spent) {
            pending.push(event_id.clone());
            compensated.insert(event_id.clone());
            omissions.insert(format!("{}:recovery-budget-or-capability", event_id));
            continue;
        }
        spent = spent.saturating_add(event.retry_cost);
        recovered.push(event_id.clone());
    }
    let global_blocked = !case.policy_allow || !case.protected_closure;
    if global_blocked {
        omissions.insert("case:policy-or-protected-closure".into());
    }
    if !case.raw_data_local {
        omissions.insert("case:raw-data-locality".into());
    }
    let disposition = if global_blocked {
        "blocked"
    } else if !blocked.is_empty() {
        "partial"
    } else if !pending.is_empty() {
        "unresolved"
    } else {
        "recovered"
    };
    recovered.sort();
    pending.sort();
    blocked.sort();
    let compensation_order = compensated
        .into_iter()
        .map(|id| format!("compensate:{id}"))
        .collect::<Vec<_>>();
    let payload = json!({"schema_version": OUTPUT_SCHEMA, "case_id": case.case_id, "scenario_id": case.scenario_id, "event_order": event_order, "recovered_order": recovered, "pending_order": pending, "blocked_order": blocked, "compensated_order": compensation_order, "replay_identity": case.replay_identity, "disposition": disposition});
    let recovery_digest = ContentHash::of_value(&payload)
        .map_err(|error| RecoveryError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("examples-recovery:{}", case.case_id),
        "application/vnd.aurora.examples-recovery-record+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: case.scenario_id.clone(),
            relation: "adversarial-recovery-classification".into(),
            digest: recovery_digest.clone(),
        }],
    )
    .map_err(|error| RecoveryError::Artifact(error.to_string()))?;
    let receipt = ExamplesRecoveryRecord {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        case_id: case.case_id.clone(),
        scenario_id: case.scenario_id.clone(),
        scope: case.scope.clone(),
        disposition: disposition.into(),
        event_order,
        recovered_order: payload["recovered_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        pending_order: payload["pending_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        compensated_order: payload["compensated_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        class_order: classes.into_iter().collect(),
        replay_identity: case.replay_identity.clone(),
        recovery_digest,
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts: if disposition == "recovered" {
            vec!["retain:recovery-record".into()]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: case.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"examples-recovery")
    }
    fn event(id: &str, state: EvidenceState, recoverable: bool) -> RecoveryEvent {
        RecoveryEvent {
            event_id: id.into(),
            class: "prompt-injection".into(),
            source_digest: Some(hash()),
            provenance_digest: Some(hash()),
            replay_identity: hash(),
            evidence_state: state,
            recoverable,
            retry_cost: 2,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn case() -> ExamplesAdversarialCase {
        ExamplesAdversarialCase {
            case_id: "case:recovery".into(),
            scenario_id: "scenario:failure-world".into(),
            scope: "organoid".into(),
            schema_version: INPUT_SCHEMA.into(),
            events: vec![
                event("event-b", EvidenceState::Proven, true),
                event("event-a", EvidenceState::Supported, true),
            ],
            replay_identity: hash(),
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            budget_units: 10,
            max_budget_units: 10,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn recovered_case_is_deterministic_and_local() {
        let value = classify(&case()).unwrap();
        assert_eq!(value.disposition, "recovered");
        assert_eq!(value.event_order, vec!["event-a", "event-b"]);
        assert_eq!(value.digest().unwrap(), value.digest().unwrap());
    }
    #[test]
    fn unknown_case_remains_unresolved() {
        let mut value = case();
        value.events[0].evidence_state = EvidenceState::Unknown;
        let receipt = classify(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt.pending_order.contains(&"event-b".into()));
        assert!(!receipt.uncertainty.is_empty());
    }
    #[test]
    fn contradiction_is_blocked_and_compensated() {
        let mut value = case();
        value.events[0].evidence_state = EvidenceState::Contradicted;
        let receipt = classify(&value).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert!(receipt.blocked_order.contains(&"event-b".into()));
        assert!(receipt
            .compensated_order
            .iter()
            .any(|item| item == "compensate:event-b"));
    }
    #[test]
    fn policy_and_protected_closure_fail_closed() {
        let mut value = case();
        value.policy_allow = false;
        value.protected_closure = false;
        let receipt = classify(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn nonrecoverable_event_preserves_negative_evidence() {
        let mut value = case();
        value.events[1].recoverable = false;
        let receipt = classify(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("recovery-unavailable")));
    }
    #[test]
    fn manifest_is_a1_typed_and_read_only() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Api));
    }
}
