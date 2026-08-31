//! Prospective high-throughput evidence-stream operations and federation control plane.
//!
//! Atlas feature: `AFA-store-P01-F31`.
//!
//! The store owns durable admission, checkpoint, capacity, telemetry, and digest-only federation
//! semantics for evidence surveillance.  It does not retrieve papers or turn incomplete evidence
//! into a conclusion: every alert is either qualified with typed receipts or retained as an
//! explicit blocked, unknown, contradicted, unmeasured, omitted, or negative state.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-store-P01-F31";
pub const CONTRACT_VERSION: &str = "store-prospective-evidence-federated-control-plane/1.0";
pub const MAX_ALERTS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAlertCandidate {
    pub alert_id: String,
    pub study_id: String,
    pub origin_institution: String,
    pub scope: String,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub evidence_digest: Option<ContentHash>,
    pub relevance_milli: u16,
    pub state: OperationsEvidenceState,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsRequest {
    pub request_id: String,
    pub feed_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub candidates: Vec<EvidenceAlertCandidate>,
    pub required_alert_ids: Vec<String>,
    pub checkpoint_id: String,
    pub replay_identity: ContentHash,
    pub capacity_budget: u64,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub telemetry_bound: bool,
    pub recovery_checkpointed: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub feed_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub disposition: OperationsDisposition,
    pub alert_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub source_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub checkpoint_id: String,
    pub replay_identity: ContentHash,
    pub telemetry_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub federation_manifest: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceOperationsError {
    #[error("invalid evidence operations request: {0}")]
    Invalid(String),
    #[error("evidence operations artifact failed: {0}")]
    Artifact(String),
    #[error("evidence operations serialization failed: {0}")]
    Serialization(String),
}

impl EvidenceOperationsReceipt {
    pub fn validate(&self) -> Result<(), EvidenceOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.feed_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.alert_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceOperationsError::Invalid(
                "evidence operations identity, alert order, locality, checkpoint, or effects is incomplete".into(),
            ));
        }
        for values in [
            &self.alert_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceOperationsError::Invalid(
                    "evidence operations ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.source_order,
            &self.provenance_order,
            &self.evidence_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceOperationsError::Invalid(
                    "evidence operations digest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .qualified_order
            .iter()
            .any(|id| !self.alert_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.alert_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.alert_order.contains(id))
        {
            return Err(EvidenceOperationsError::Invalid(
                "evidence operations state order is not covered by alert order".into(),
            ));
        }
        self.federation_manifest
            .validate_metadata()
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceOperationsError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceOperationsError::Serialization(error.to_string()))
    }
}

pub fn evidence_operations_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "store".into(),
        consumers: [
            "platform reliability engineer".into(),
            "institution node operator".into(),
            "evidence steward".into(),
        ]
        .into(),
        behavior: "operates a bounded prospective evidence stream with deterministic admission, capacity, checkpoints, telemetry, recovery, and digest-only federation".into(),
        value: "turns high-throughput local evidence alerts into an auditable operations queue while retaining negative and incomplete evidence and never exporting raw source data".into(),
        inputs: vec![TypedPort {
            name: "evidence_operations_request".into(),
            schema: "EvidenceOperationsRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "evidence_operations_receipt".into(),
            schema: "EvidenceOperationsReceipt@1".into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::ExecuteLocalComputation,
            Effect::FederationExport,
        ]
        .into(),
        permissions: [
            "operate:institution-node".into(),
            "manage:local-capability".into(),
            "exchange:permitted-summaries".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "opentelemetry".into(),
            state: EvidenceState::Supported,
            locator: Some("https://opentelemetry.io/docs/specs/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "institution evidence operations steward".into(),
            reason: "approve local queue operation and permitted summary exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_evidence_stream(
    request: &EvidenceOperationsRequest,
) -> Result<EvidenceOperationsReceipt, EvidenceOperationsError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then_with(|| left.alert_id.cmp(&right.alert_id))
    });
    let mut alert_ids = BTreeSet::new();
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        alert_ids.insert(candidate.alert_id.clone());
        let cost = candidate.alert_id.len() as u64 + candidate.origin_institution.len() as u64 + 1;
        let capacity_ok = cost <= request.capacity_budget.saturating_sub(spent);
        let complete = candidate.scope == request.scope
            && candidate.source_digest.is_some()
            && candidate.provenance_digest.is_some()
            && candidate.evidence_digest.is_some()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && request.telemetry_bound
            && request.recovery_checkpointed
            && candidate.state == OperationsEvidenceState::Supported
            && complete
            && capacity_ok;
        if gate {
            let (Some(source_digest), Some(provenance_digest), Some(evidence_digest)) = (
                candidate.source_digest.clone(),
                candidate.provenance_digest.clone(),
                candidate.evidence_digest.clone(),
            ) else {
                return Err(EvidenceOperationsError::Invalid(
                    "qualified evidence alert is missing a required digest".into(),
                ));
            };
            spent = spent.saturating_add(cost);
            qualified.insert(candidate.alert_id.clone());
            sources.insert(source_digest);
            provenance.insert(provenance_digest);
            evidence.insert(evidence_digest);
            if candidate.negative_result {
                negative.insert(format!(
                    "alert:{}:negative-result-retained",
                    candidate.alert_id
                ));
            }
        } else {
            match candidate.state {
                OperationsEvidenceState::Unknown | OperationsEvidenceState::Unmeasured => {
                    unknown.insert(candidate.alert_id.clone());
                    uncertainty.insert(
                        format!(
                            "alert:{}:state-{:?}-not-qualified",
                            candidate.alert_id, candidate.state
                        )
                        .to_ascii_lowercase(),
                    );
                }
                OperationsEvidenceState::Contradicted => {
                    blocked.insert(candidate.alert_id.clone());
                    negative.insert(format!(
                        "alert:{}:contradicted-result-retained",
                        candidate.alert_id
                    ));
                }
                OperationsEvidenceState::Supported => {
                    blocked.insert(candidate.alert_id.clone());
                }
            }
            if candidate.scope != request.scope {
                omissions.insert(format!("alert:{}:scope-mismatch", candidate.alert_id));
            }
            if candidate.source_digest.is_none()
                || candidate.provenance_digest.is_none()
                || candidate.evidence_digest.is_none()
            {
                omissions.insert(format!(
                    "alert:{}:required-digest-missing",
                    candidate.alert_id
                ));
            }
            if !candidate.omissions.is_empty() {
                omissions.extend(
                    candidate
                        .omissions
                        .iter()
                        .map(|value| format!("alert:{}:{value}", candidate.alert_id)),
                );
            }
            if !candidate.uncertainty.is_empty() {
                uncertainty.extend(
                    candidate
                        .uncertainty
                        .iter()
                        .map(|value| format!("alert:{}:{value}", candidate.alert_id)),
                );
            }
            if !capacity_ok {
                omissions.insert(format!(
                    "alert:{}:capacity-budget-exceeded",
                    candidate.alert_id
                ));
            }
            if candidate.negative_result {
                negative.insert(format!(
                    "alert:{}:negative-result-retained",
                    candidate.alert_id
                ));
            }
        }
    }
    for required in &request.required_alert_ids {
        if !qualified.contains(required) {
            omissions.insert(format!("alert:{}:required-but-not-qualified", required));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    if !request.telemetry_bound {
        omissions.insert("request:telemetry-bound-missing".into());
    }
    if !request.recovery_checkpointed {
        omissions.insert("request:recovery-checkpoint-missing".into());
    }
    let alert_order = alert_ids.into_iter().collect::<Vec<_>>();
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let source_order = sources.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let hard_block = !request.policy_allow
        || !request.federation_allow
        || !request.signed_approval
        || !request.raw_data_local;
    let disposition = if hard_block {
        OperationsDisposition::Blocked
    } else if qualified_order.is_empty() {
        OperationsDisposition::Unknown
    } else if blocked_order.is_empty()
        && unknown_order.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
    {
        OperationsDisposition::Qualified
    } else {
        OperationsDisposition::Partial
    };
    let telemetry_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "checkpoint_id": request.checkpoint_id,
        "alert_order": alert_order,
        "qualified_order": qualified_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "disposition": disposition,
    }))
    .map_err(|error| EvidenceOperationsError::Serialization(error.to_string()))?;
    let manifest_payload = json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "qualified_order": qualified_order,
        "source_order": source_order,
        "provenance_order": provenance_order,
        "evidence_order": evidence_order,
        "telemetry_digest": telemetry_digest,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let federation_manifest = TypedResearchArtifact::from_payload(
        format!("permitted-evidence-summary:{}", request.request_id),
        "application/vnd.aurora.permitted-evidence-summary+json",
        &manifest_payload,
        vec![],
        vec![],
    )
    .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let mut effect_receipts = vec![format!(
        "checkpoint:evidence-operations:{}",
        request.checkpoint_id
    )];
    if !qualified_order.is_empty()
        && request.federation_allow
        && request.policy_allow
        && request.raw_data_local
    {
        effect_receipts.push(format!(
            "exchange:permitted-evidence-summary:{}",
            request.request_id
        ));
    }
    if disposition != OperationsDisposition::Qualified {
        effect_receipts.push(format!(
            "block:evidence-operations-release:{}",
            request.request_id
        ));
    }
    effect_receipts.sort();
    let receipt = EvidenceOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        feed_id: request.feed_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        alert_order,
        qualified_order,
        blocked_order,
        unknown_order,
        source_order,
        provenance_order,
        evidence_order,
        checkpoint_id: request.checkpoint_id.clone(),
        replay_identity: request.replay_identity.clone(),
        telemetry_digest,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        federation_manifest,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &EvidenceOperationsRequest) -> Result<(), EvidenceOperationsError> {
    if request.request_id.trim().is_empty()
        || request.feed_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_ALERTS
        || request
            .required_alert_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.capacity_budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceOperationsError::Invalid(
            "evidence operations identity, scope, candidates, required alerts, capacity, checkpoint, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.alert_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.origin_institution.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.alert_id.clone())
            || candidate
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(EvidenceOperationsError::Invalid(format!(
                "evidence alert {} is invalid or duplicated",
                candidate.alert_id
            )));
        }
    }
    if request
        .required_alert_ids
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(EvidenceOperationsError::Invalid(
            "required alert closure references an unknown candidate".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(id: &str, state: OperationsEvidenceState) -> EvidenceAlertCandidate {
        EvidenceAlertCandidate {
            alert_id: id.into(),
            study_id: "study:organoid".into(),
            origin_institution: "site:alpha".into(),
            scope: "organoid:neural".into(),
            source_digest: Some(hash(&format!("source:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            relevance_milli: if id.ends_with('a') { 950 } else { 800 },
            state,
            negative_result: false,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<EvidenceAlertCandidate>) -> EvidenceOperationsRequest {
        EvidenceOperationsRequest {
            request_id: "request:evidence-ops".into(),
            feed_id: "feed:surveillance".into(),
            workflow_id: "workflow:evidence".into(),
            federation_id: "federation:commons".into(),
            scope: "organoid:neural".into(),
            candidates,
            required_alert_ids: vec!["alert:a".into()],
            checkpoint_id: "checkpoint:17".into(),
            replay_identity: hash("replay"),
            capacity_budget: 200,
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            telemetry_bound: true,
            recovery_checkpointed: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_declares_a2_operations_and_federation() {
        let manifest = evidence_operations_manifest();
        assert_eq!(manifest.capability_id, FEATURE_ID);
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
    }

    #[test]
    fn ranks_and_qualifies_supported_alerts() {
        let receipt = operate_evidence_stream(&request(vec![
            candidate("alert:b", OperationsEvidenceState::Supported),
            candidate("alert:a", OperationsEvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, OperationsDisposition::Qualified);
        assert_eq!(receipt.qualified_order, vec!["alert:a", "alert:b"]);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:")));
    }

    #[test]
    fn unknown_and_negative_states_are_retained() {
        let receipt = operate_evidence_stream(&request(vec![
            candidate("alert:a", OperationsEvidenceState::Supported),
            candidate("alert:b", OperationsEvidenceState::Unknown),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, OperationsDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"alert:b".into()));
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown")));
    }

    #[test]
    fn missing_digest_is_an_omission_not_a_pass() {
        let mut missing = candidate("alert:a", OperationsEvidenceState::Supported);
        missing.evidence_digest = None;
        let receipt = operate_evidence_stream(&request(vec![missing])).unwrap();
        assert_eq!(receipt.disposition, OperationsDisposition::Unknown);
        assert!(receipt.omissions.iter().any(|item| item.contains("digest")));
    }

    #[test]
    fn federation_denial_blocks_exchange() {
        let mut input = request(vec![candidate(
            "alert:a",
            OperationsEvidenceState::Supported,
        )]);
        input.federation_allow = false;
        let receipt = operate_evidence_stream(&input).unwrap();
        assert_eq!(receipt.disposition, OperationsDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("federation")));
        assert!(!receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:")));
    }

    #[test]
    fn duplicate_alerts_are_rejected() {
        let result = operate_evidence_stream(&request(vec![
            candidate("alert:a", OperationsEvidenceState::Supported),
            candidate("alert:a", OperationsEvidenceState::Supported),
        ]));
        assert!(result.is_err());
    }
}
