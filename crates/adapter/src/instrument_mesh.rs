//! Federated laboratory-integration mesh for policy-separated instrument capabilities.
//!
//! Atlas feature: `AFA-adapter-P11-F04`.
//!
//! This product selects a locally governed instrument capability for a typed preclinical action
//! without contacting hardware or moving raw experimental data. It turns capability manifests,
//! interlocks, policy, authorization, protected closure, and federation availability into a
//! deterministic action receipt. Missing capability, incomplete closure, a network partition, or
//! absent authority stays explicit; no physical effect is implied by a successful selection.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P11-F04";
pub const CONTRACT_VERSION: &str = "federated-laboratory-integration/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCapability {
    pub instrument_id: String,
    pub site_id: String,
    pub protocol_profile: String,
    pub supported_operations: Vec<String>,
    pub interlocks: BTreeSet<String>,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentActionRequest {
    pub request_id: String,
    pub federation_id: String,
    pub action_id: String,
    pub operation: String,
    pub required_capabilities: Vec<String>,
    pub required_interlocks: BTreeSet<String>,
    pub target_instrument_id: Option<String>,
    pub estimated_cost: f64,
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub protected_closure: bool,
    pub network_partition: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentMeshDecision {
    Admitted,
    ApprovalRequired,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentEffectReceipt {
    pub action_id: String,
    pub instrument_id: String,
    pub site_id: String,
    pub operation: String,
    pub authorized: bool,
    pub executed: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentMeshReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub action_id: String,
    pub decision: InstrumentMeshDecision,
    pub candidate_order: Vec<String>,
    pub selected_instrument_id: Option<String>,
    pub selected_site_id: Option<String>,
    pub selected_protocol_profile: Option<String>,
    pub satisfied_capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
    pub missing_interlocks: Vec<String>,
    pub effect: Option<InstrumentEffectReceipt>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl InstrumentMeshReceipt {
    pub fn validate(&self) -> Result<(), InstrumentMeshError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(InstrumentMeshError::Contract(
                "instrument mesh contract identity mismatch".into(),
            ));
        }
        if self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.action_id.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.reasons.is_empty()
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "identity, reasons, locality, and preclinical boundary are required".into(),
            ));
        }
        if self
            .candidate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self.candidate_order.iter().collect::<BTreeSet<_>>().len()
                != self.candidate_order.len()
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "candidate order must be canonical and unique".into(),
            ));
        }
        if self
            .missing_capabilities
            .iter()
            .any(|item| item.trim().is_empty())
            || self
                .missing_interlocks
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "missing capability and interlock names must be non-empty".into(),
            ));
        }
        if self.decision == InstrumentMeshDecision::Admitted {
            let effect = self.effect.as_ref().ok_or_else(|| {
                InstrumentMeshError::InvalidRequest(
                    "admitted action needs an effect receipt".into(),
                )
            })?;
            if !effect.authorized || effect.executed || !effect.raw_data_local {
                return Err(InstrumentMeshError::InvalidRequest(
                    "admitted mesh receipt must be authorized, not executed, and local".into(),
                ));
            }
            if self.selected_instrument_id.is_none() || self.selected_site_id.is_none() {
                return Err(InstrumentMeshError::InvalidRequest(
                    "admitted action needs a selected instrument and site".into(),
                ));
            }
        } else if self.effect.is_some() {
            return Err(InstrumentMeshError::InvalidRequest(
                "non-admitted mesh receipt cannot contain an effect".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InstrumentMeshError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, InstrumentMeshError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum InstrumentMeshError {
    #[error("invalid instrument mesh request: {0}")]
    InvalidRequest(String),
    #[error("duplicate instrument capability {0}")]
    DuplicateCapability(String),
    #[error("instrument mesh contract rejected: {0}")]
    Contract(String),
    #[error("instrument mesh serialization failed: {0}")]
    Serialization(String),
}

pub fn integrate_instrument_mesh(
    request: &InstrumentActionRequest,
    capabilities: &[InstrumentCapability],
) -> Result<InstrumentMeshReceipt, InstrumentMeshError> {
    validate_request(request, capabilities)?;
    let mut ordered = capabilities.to_vec();
    ordered.sort_by(|left, right| {
        left.instrument_id
            .cmp(&right.instrument_id)
            .then(left.site_id.cmp(&right.site_id))
    });
    let candidate_order = ordered
        .iter()
        .map(|capability| format!("{}@{}", capability.instrument_id, capability.site_id))
        .collect::<Vec<_>>();
    let mut missing_capabilities = request.required_capabilities.clone();
    missing_capabilities.sort();
    let mut missing_interlocks = request
        .required_interlocks
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let matching = ordered.iter().find(|capability| {
        request
            .target_instrument_id
            .as_ref()
            .map_or(true, |target| target == &capability.instrument_id)
            && capability
                .supported_operations
                .iter()
                .any(|operation| operation == &request.operation)
            && request.required_capabilities.iter().all(|required| {
                capability
                    .supported_operations
                    .iter()
                    .any(|supported| supported == required)
            })
            && request
                .required_interlocks
                .iter()
                .all(|required| capability.interlocks.contains(required))
    });
    let (
        selected_instrument_id,
        selected_site_id,
        selected_protocol_profile,
        satisfied_capabilities,
    ) = matching
        .map(|capability| {
            missing_capabilities.clear();
            missing_interlocks.clear();
            (
                Some(capability.instrument_id.clone()),
                Some(capability.site_id.clone()),
                Some(capability.protocol_profile.clone()),
                request.required_capabilities.clone(),
            )
        })
        .unwrap_or((None, None, None, Vec::new()));
    let mut semantic_loss = Vec::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut reasons = Vec::new();
    let decision = if !request.policy_allow || !request.protected_closure {
        reasons.push("policy allow and protected closure are both required".into());
        semantic_loss.push(SemanticLoss {
            field: "authorization_closure".into(),
            reason:
                "an unresolved policy or protected constraint cannot authorize an instrument effect"
                    .into(),
            severity: LossSeverity::DecisionRelevant,
        });
        InstrumentMeshDecision::Blocked
    } else if matching.is_none() {
        omissions.push("no candidate instrument satisfies the requested operation, capabilities, and interlocks".into());
        uncertainty.push("instrument capability absence is not evidence that the requested operation is impossible".into());
        reasons.push("capability matching returned no safe local candidate".into());
        InstrumentMeshDecision::Unknown
    } else if request.network_partition || request.authorization_reference.is_none() {
        reasons.push(if request.network_partition {
            "federation partition prevents confirmation of remote capability authority".into()
        } else {
            "independent authorization reference is required before any external effect".into()
        });
        InstrumentMeshDecision::ApprovalRequired
    } else {
        reasons.push("local capability, policy, closure, authorization, interlock, and locality gates passed; no hardware effect performed".into());
        InstrumentMeshDecision::Admitted
    };
    if !missing_capabilities.is_empty() {
        semantic_loss.push(SemanticLoss {
            field: "capability_match".into(),
            reason: "requested capability is absent from the visible local mesh".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !missing_interlocks.is_empty() {
        semantic_loss.push(SemanticLoss {
            field: "interlocks".into(),
            reason: "required instrument interlocks were not observed in a candidate manifest"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let effect = if decision == InstrumentMeshDecision::Admitted {
        Some(InstrumentEffectReceipt {
            action_id: request.action_id.clone(),
            instrument_id: selected_instrument_id.clone().expect("admitted selection"),
            site_id: selected_site_id.clone().expect("admitted selection"),
            operation: request.operation.clone(),
            authorized: true,
            executed: false,
            raw_data_local: true,
        })
    } else {
        None
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "action_id": request.action_id,
        "decision": decision,
        "candidate_order": candidate_order,
        "selected_instrument_id": selected_instrument_id,
        "selected_site_id": selected_site_id,
        "selected_protocol_profile": selected_protocol_profile,
        "satisfied_capabilities": satisfied_capabilities,
        "missing_capabilities": missing_capabilities,
        "missing_interlocks": missing_interlocks,
        "effect": effect,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = ordered
        .iter()
        .map(|capability| ProvenanceLink {
            source_id: format!("{}@{}", capability.instrument_id, capability.site_id),
            relation: "selected-from-local-instrument-capability-manifest".into(),
            digest: ContentHash::of_bytes(capability.instrument_id.as_bytes()),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("instrument-mesh:{}", request.request_id),
        "application/vnd.aurora.federated-instrument-mesh+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| InstrumentMeshError::Contract(error.to_string()))?;
    let receipt = InstrumentMeshReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        action_id: request.action_id.clone(),
        decision,
        candidate_order,
        selected_instrument_id,
        selected_site_id,
        selected_protocol_profile,
        satisfied_capabilities,
        missing_capabilities,
        missing_interlocks,
        effect,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &InstrumentActionRequest,
    capabilities: &[InstrumentCapability],
) -> Result<(), InstrumentMeshError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.action_id.trim().is_empty()
        || request.operation.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.estimated_cost.is_finite()
        || request.estimated_cost <= 0.0
    {
        return Err(InstrumentMeshError::InvalidRequest(
            "request identity, operation, required capabilities, positive cost, locality, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for capability in capabilities {
        if capability.instrument_id.trim().is_empty()
            || capability.site_id.trim().is_empty()
            || capability.protocol_profile.trim().is_empty()
            || !capability.raw_data_local
            || capability.supported_operations.is_empty()
            || !ids.insert(format!(
                "{}@{}",
                capability.instrument_id, capability.site_id
            ))
        {
            return Err(InstrumentMeshError::DuplicateCapability(
                capability.instrument_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> InstrumentActionRequest {
        InstrumentActionRequest {
            request_id: "request:mesh".into(),
            federation_id: "federation:preclinical".into(),
            action_id: "action:image".into(),
            operation: "image.acquire".into(),
            required_capabilities: vec!["image.acquire".into()],
            required_interlocks: ["preflight_signed".into()].into(),
            target_instrument_id: None,
            estimated_cost: 2.0,
            policy_allow: true,
            authorization_reference: Some("approval:operator".into()),
            protected_closure: true,
            network_partition: false,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn capability(instrument_id: &str, site_id: &str) -> InstrumentCapability {
        InstrumentCapability {
            instrument_id: instrument_id.into(),
            site_id: site_id.into(),
            protocol_profile: "ome-ngff-v0.5".into(),
            supported_operations: vec!["image.acquire".into()],
            interlocks: ["preflight_signed".into()].into(),
            raw_data_local: true,
        }
    }

    #[test]
    fn selection_is_canonical_and_replayable() {
        let capabilities = vec![
            capability("scope-b", "site-2"),
            capability("scope-a", "site-1"),
        ];
        let first = integrate_instrument_mesh(&request(), &capabilities).unwrap();
        let second = integrate_instrument_mesh(&request(), &capabilities).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_instrument_id.as_deref(), Some("scope-a"));
        assert!(first.digest().is_ok());
    }

    #[test]
    fn absent_capability_is_unknown_without_effect() {
        let mut request = request();
        request.required_capabilities = vec!["omics.sequence".into()];
        let receipt =
            integrate_instrument_mesh(&request, &[capability("scope-a", "site-1")]).unwrap();
        assert_eq!(receipt.decision, InstrumentMeshDecision::Unknown);
        assert!(receipt.effect.is_none());
        assert!(!receipt.uncertainty.is_empty());
    }

    #[test]
    fn missing_authority_requires_approval_without_effect() {
        let mut request = request();
        request.authorization_reference = None;
        let receipt =
            integrate_instrument_mesh(&request, &[capability("scope-a", "site-1")]).unwrap();
        assert_eq!(receipt.decision, InstrumentMeshDecision::ApprovalRequired);
        assert!(receipt.effect.is_none());
    }

    #[test]
    fn partition_requires_approval_and_locality_is_preserved() {
        let mut request = request();
        request.network_partition = true;
        let receipt =
            integrate_instrument_mesh(&request, &[capability("scope-a", "site-1")]).unwrap();
        assert_eq!(receipt.decision, InstrumentMeshDecision::ApprovalRequired);
        assert!(receipt.effect.is_none());
        assert!(receipt.raw_data_local);
    }
}
