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
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 8192;

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
    pub input: InstrumentActionRequest,
    pub input_digest: ContentHash,
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
    pub capabilities: Vec<InstrumentCapability>,
    pub decision: InstrumentMeshDecision,
    pub candidate_order: Vec<String>,
    pub selected_instrument_id: Option<String>,
    pub selected_site_id: Option<String>,
    pub selected_protocol_profile: Option<String>,
    pub satisfied_capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
    pub missing_interlocks: Vec<String>,
    pub effect: Option<InstrumentEffectReceipt>,
    pub mesh_digest: ContentHash,
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
            || self.operation.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.reasons.is_empty()
            || !self.estimated_cost.is_finite()
            || self.estimated_cost <= 0.0
            || self.mesh_digest == ContentHash::of_bytes(b"")
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "identity, reasons, locality, and preclinical boundary are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("action_id", &self.action_id)?;
        validate_text("operation", &self.operation)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("required_capabilities", &self.required_capabilities)?;
        for interlock in &self.required_interlocks {
            validate_text("required_interlock", interlock)?;
        }
        if let Some(target) = &self.target_instrument_id {
            validate_text("target_instrument_id", target)?;
        }
        if let Some(reference) = &self.authorization_reference {
            validate_text("authorization_reference", reference)?;
        }
        if self.capabilities.len() > MAX_ITEMS
            || self
                .capabilities
                .windows(2)
                .any(|pair| capability_key(&pair[0]) >= capability_key(&pair[1]))
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "instrument capability order must be canonical and unique".into(),
            ));
        }
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("satisfied_capabilities", &self.satisfied_capabilities)?;
        validate_sorted_strings("missing_capabilities", &self.missing_capabilities)?;
        validate_sorted_strings("missing_interlocks", &self.missing_interlocks)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self
            .semantic_loss
            .windows(2)
            .any(|pair| pair[0].field >= pair[1].field)
        {
            return Err(InstrumentMeshError::InvalidRequest(
                "instrument semantic-loss ordering is not canonical".into(),
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
            if self.selected_instrument_id.is_none()
                || self.selected_site_id.is_none()
                || self.selected_protocol_profile.is_none()
            {
                return Err(InstrumentMeshError::InvalidRequest(
                    "admitted action needs a selected instrument and site".into(),
                ));
            }
            if effect.action_id != self.action_id
                || self.selected_instrument_id.as_deref() != Some(effect.instrument_id.as_str())
                || self.selected_site_id.as_deref() != Some(effect.site_id.as_str())
                || effect.raw_data_local != self.raw_data_local
            {
                return Err(InstrumentMeshError::InvalidRequest(
                    "instrument effect is not bound to the admitted selection".into(),
                ));
            }
        } else if self.effect.is_some() {
            return Err(InstrumentMeshError::InvalidRequest(
                "non-admitted mesh receipt cannot contain an effect".into(),
            ));
        }
        let expected_provenance = mesh_provenance(&self.capabilities)?;
        if self.artifact.artifact_id != format!("instrument-mesh:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.federated-instrument-mesh+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(InstrumentMeshError::Contract(
                "instrument mesh artifact is not bound to candidates and semantic loss".into(),
            ));
        }
        let expected_mesh_digest = ContentHash::of_value(&mesh_digest_payload(self))
            .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
        if self.mesh_digest != expected_mesh_digest {
            return Err(InstrumentMeshError::InvalidRequest(
                "instrument mesh digest does not bind the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InstrumentMeshError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&mesh_payload(self))
            .map_err(|error| InstrumentMeshError::Contract(error.to_string()))?;
        validate_request(&self.input, &self.capabilities)?;
        if self.input_digest != instrument_input_digest(&self.input)? {
            return Err(InstrumentMeshError::Contract(
                "instrument mesh retained input digest does not match the request".into(),
            ));
        }
        let expected = integrate_instrument_mesh_internal(&self.input, &self.capabilities, false)?;
        if self != &expected {
            return Err(InstrumentMeshError::Contract(
                "instrument mesh receipt is not derived from its retained request and capability manifests".into(),
            ));
        }
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

fn validate_text(field: &str, value: &str) -> Result<(), InstrumentMeshError> {
    if value.is_empty() || value.trim() != value {
        return Err(InstrumentMeshError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(InstrumentMeshError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn instrument_input_digest(
    request: &InstrumentActionRequest,
) -> Result<ContentHash, InstrumentMeshError> {
    let canonical = canonical_instrument_action_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))
}

fn canonical_instrument_action_request(
    request: &InstrumentActionRequest,
) -> InstrumentActionRequest {
    let mut canonical = request.clone();
    canonical.required_capabilities.sort();
    canonical
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), InstrumentMeshError> {
    if values.len() > MAX_ITEMS {
        return Err(InstrumentMeshError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(InstrumentMeshError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InstrumentMeshError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn capability_key(capability: &InstrumentCapability) -> String {
    format!("{}@{}", capability.instrument_id, capability.site_id)
}

fn mesh_provenance(
    capabilities: &[InstrumentCapability],
) -> Result<Vec<ProvenanceLink>, InstrumentMeshError> {
    capabilities
        .iter()
        .map(|capability| {
            let value = serde_json::to_value(capability)
                .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
            let digest = ContentHash::of_value(&value)
                .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
            Ok(ProvenanceLink {
                source_id: capability_key(capability),
                relation: "selected-from-local-instrument-capability-manifest".into(),
                digest,
            })
        })
        .collect()
}

fn mesh_payload(receipt: &InstrumentMeshReceipt) -> serde_json::Value {
    mesh_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.federation_id,
        &receipt.action_id,
        &receipt.operation,
        &receipt.required_capabilities,
        &receipt.required_interlocks,
        &receipt.target_instrument_id,
        receipt.estimated_cost,
        receipt.policy_allow,
        &receipt.authorization_reference,
        receipt.protected_closure,
        receipt.network_partition,
        &receipt.capabilities,
        receipt.decision,
        &receipt.candidate_order,
        &receipt.selected_instrument_id,
        &receipt.selected_site_id,
        &receipt.selected_protocol_profile,
        &receipt.satisfied_capabilities,
        &receipt.missing_capabilities,
        &receipt.missing_interlocks,
        &receipt.effect,
        &receipt.mesh_digest,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.semantic_loss,
        &receipt.reasons,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn mesh_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    federation_id: &str,
    action_id: &str,
    operation: &str,
    required_capabilities: &[String],
    required_interlocks: &BTreeSet<String>,
    target_instrument_id: &Option<String>,
    estimated_cost: f64,
    policy_allow: bool,
    authorization_reference: &Option<String>,
    protected_closure: bool,
    network_partition: bool,
    capabilities: &[InstrumentCapability],
    decision: InstrumentMeshDecision,
    candidate_order: &[String],
    selected_instrument_id: &Option<String>,
    selected_site_id: &Option<String>,
    selected_protocol_profile: &Option<String>,
    satisfied_capabilities: &[String],
    missing_capabilities: &[String],
    missing_interlocks: &[String],
    effect: &Option<InstrumentEffectReceipt>,
    mesh_digest: &ContentHash,
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request_id,
        "federation_id": federation_id,
        "action_id": action_id,
        "operation": operation,
        "required_capabilities": required_capabilities,
        "required_interlocks": required_interlocks,
        "target_instrument_id": target_instrument_id,
        "estimated_cost": estimated_cost,
        "policy_allow": policy_allow,
        "authorization_reference": authorization_reference,
        "protected_closure": protected_closure,
        "network_partition": network_partition,
        "capabilities": capabilities,
        "decision": decision,
        "candidate_order": candidate_order,
        "selected_instrument_id": selected_instrument_id,
        "selected_site_id": selected_site_id,
        "selected_protocol_profile": selected_protocol_profile,
        "satisfied_capabilities": satisfied_capabilities,
        "missing_capabilities": missing_capabilities,
        "missing_interlocks": missing_interlocks,
        "effect": effect,
        "mesh_digest": mesh_digest,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

fn mesh_digest_payload(receipt: &InstrumentMeshReceipt) -> serde_json::Value {
    mesh_digest_payload_from_parts(
        &receipt.request_id,
        &receipt.federation_id,
        &receipt.action_id,
        receipt.estimated_cost,
        receipt.decision,
        &receipt.candidate_order,
        &receipt.selected_instrument_id,
        &receipt.selected_site_id,
        &receipt.selected_protocol_profile,
        &receipt.satisfied_capabilities,
        &receipt.missing_capabilities,
        &receipt.missing_interlocks,
        &receipt.effect,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.semantic_loss,
        &receipt.reasons,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn mesh_digest_payload_from_parts(
    request_id: &str,
    federation_id: &str,
    action_id: &str,
    estimated_cost: f64,
    decision: InstrumentMeshDecision,
    candidate_order: &[String],
    selected_instrument_id: &Option<String>,
    selected_site_id: &Option<String>,
    selected_protocol_profile: &Option<String>,
    satisfied_capabilities: &[String],
    missing_capabilities: &[String],
    missing_interlocks: &[String],
    effect: &Option<InstrumentEffectReceipt>,
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "request_id": request_id,
        "federation_id": federation_id,
        "action_id": action_id,
        "estimated_cost": estimated_cost,
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
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    integrate_instrument_mesh_internal(request, capabilities, true)
}

fn integrate_instrument_mesh_internal(
    request: &InstrumentActionRequest,
    capabilities: &[InstrumentCapability],
    validate_output: bool,
) -> Result<InstrumentMeshReceipt, InstrumentMeshError> {
    let input = canonical_instrument_action_request(request);
    let request = &input;
    validate_request(request, capabilities)?;
    let request = request.clone();
    let mut ordered = capabilities.to_vec();
    for capability in &mut ordered {
        capability.supported_operations.sort();
    }
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
            .is_none_or(|target| target == &capability.instrument_id)
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
    let mut satisfied_capabilities = satisfied_capabilities;
    satisfied_capabilities.sort();
    missing_capabilities.sort();
    missing_interlocks.sort();
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
    omissions.sort();
    uncertainty.sort();
    reasons.sort();
    semantic_loss.sort_by(|left, right| left.field.cmp(&right.field));
    let effect = if decision == InstrumentMeshDecision::Admitted {
        let instrument_id = selected_instrument_id.clone().ok_or_else(|| {
            InstrumentMeshError::InvalidRequest(
                "admitted action is missing the selected instrument".into(),
            )
        })?;
        let site_id = selected_site_id.clone().ok_or_else(|| {
            InstrumentMeshError::InvalidRequest(
                "admitted action is missing the selected site".into(),
            )
        })?;
        Some(InstrumentEffectReceipt {
            action_id: request.action_id.clone(),
            instrument_id,
            site_id,
            operation: request.operation.clone(),
            authorized: true,
            executed: false,
            raw_data_local: true,
        })
    } else {
        None
    };
    let mesh_digest_payload = mesh_digest_payload_from_parts(
        &request.request_id,
        &request.federation_id,
        &request.action_id,
        request.estimated_cost,
        decision,
        &candidate_order,
        &selected_instrument_id,
        &selected_site_id,
        &selected_protocol_profile,
        &satisfied_capabilities,
        &missing_capabilities,
        &missing_interlocks,
        &effect,
        &omissions,
        &uncertainty,
        &semantic_loss,
        &reasons,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let mesh_digest = ContentHash::of_value(&mesh_digest_payload)
        .map_err(|error| InstrumentMeshError::Serialization(error.to_string()))?;
    let provenance = mesh_provenance(&ordered)?;
    let payload = mesh_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &request.federation_id,
        &request.action_id,
        &request.operation,
        &request.required_capabilities,
        &request.required_interlocks,
        &request.target_instrument_id,
        request.estimated_cost,
        request.policy_allow,
        &request.authorization_reference,
        request.protected_closure,
        request.network_partition,
        &ordered,
        decision,
        &candidate_order,
        &selected_instrument_id,
        &selected_site_id,
        &selected_protocol_profile,
        &satisfied_capabilities,
        &missing_capabilities,
        &missing_interlocks,
        &effect,
        &mesh_digest,
        &omissions,
        &uncertainty,
        &semantic_loss,
        &reasons,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("instrument-mesh:{}", request.request_id),
        "application/vnd.aurora.federated-instrument-mesh+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| InstrumentMeshError::Contract(error.to_string()))?;
    let input_digest = instrument_input_digest(&input)?;
    let receipt = InstrumentMeshReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input,
        input_digest,
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        action_id: request.action_id.clone(),
        operation: request.operation.clone(),
        required_capabilities: request.required_capabilities.clone(),
        required_interlocks: request.required_interlocks.clone(),
        target_instrument_id: request.target_instrument_id.clone(),
        estimated_cost: request.estimated_cost,
        policy_allow: request.policy_allow,
        authorization_reference: request.authorization_reference.clone(),
        protected_closure: request.protected_closure,
        network_partition: request.network_partition,
        capabilities: ordered,
        decision,
        candidate_order,
        selected_instrument_id,
        selected_site_id,
        selected_protocol_profile,
        satisfied_capabilities,
        missing_capabilities,
        missing_interlocks,
        effect,
        mesh_digest,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        receipt.validate()?;
    }
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
        || request.required_capabilities.len() > MAX_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.estimated_cost.is_finite()
        || request.estimated_cost <= 0.0
    {
        return Err(InstrumentMeshError::InvalidRequest(
            "request identity, operation, required capabilities, positive cost, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("action_id", &request.action_id)?;
    validate_text("operation", &request.operation)?;
    validate_text("boundary", &request.boundary)?;
    let mut required = BTreeSet::new();
    for capability in &request.required_capabilities {
        validate_text("required_capability", capability)?;
        if !required.insert(capability) {
            return Err(InstrumentMeshError::InvalidRequest(
                "required capabilities cannot contain duplicates".into(),
            ));
        }
    }
    if let Some(target) = &request.target_instrument_id {
        validate_text("target_instrument_id", target)?;
    }
    if let Some(reference) = &request.authorization_reference {
        validate_text("authorization_reference", reference)?;
    }
    for interlock in &request.required_interlocks {
        validate_text("required_interlock", interlock)?;
    }
    let mut ids = BTreeSet::new();
    for capability in capabilities {
        validate_text("instrument_id", &capability.instrument_id)?;
        validate_text("site_id", &capability.site_id)?;
        validate_text("protocol_profile", &capability.protocol_profile)?;
        if !capability.raw_data_local
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
        let mut operations = BTreeSet::new();
        for operation in &capability.supported_operations {
            validate_text("supported_operation", operation)?;
            if !operations.insert(operation) {
                return Err(InstrumentMeshError::InvalidRequest(
                    "supported operations cannot contain duplicates".into(),
                ));
            }
        }
        for interlock in &capability.interlocks {
            validate_text("capability_interlock", interlock)?;
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
    fn required_capability_order_does_not_change_receipt_identity() {
        let mut first_request = request();
        first_request.required_capabilities = vec!["image.acquire".into(), "image.focus".into()];
        let mut second_request = first_request.clone();
        second_request.required_capabilities.reverse();
        let capabilities = [capability("scope-a", "site-1")];

        let first = integrate_instrument_mesh(&first_request, &capabilities).unwrap();
        let second = integrate_instrument_mesh(&second_request, &capabilities).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.input_digest, second.input_digest);
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

    #[test]
    fn empty_authorization_reference_is_rejected() {
        let mut value = request();
        value.authorization_reference = Some(String::new());
        assert!(integrate_instrument_mesh(&value, &[capability("scope-a", "site-1")]).is_err());
    }

    #[test]
    fn mesh_digest_rejects_selection_tampering() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.selected_protocol_profile = Some("forged-profile".into());
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn admitted_effect_cannot_be_marked_executed() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.effect.as_mut().unwrap().executed = true;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn duplicate_supported_operation_is_rejected() {
        let mut value = capability("scope-a", "site-1");
        value.supported_operations.push("image.acquire".into());
        assert!(integrate_instrument_mesh(&request(), &[value]).is_err());
    }

    #[test]
    fn retained_capability_manifest_tampering_is_rejected() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.capabilities[0].protocol_profile = "forged-profile".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn mesh_capability_provenance_tampering_is_rejected() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_network_gate_tampering_is_rejected() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.network_partition = true;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt =
            integrate_instrument_mesh(&request(), &[capability("scope-a", "site-1")]).unwrap();
        receipt.input.operation = "tampered-operation".into();
        assert!(receipt.validate().is_err());
    }
}
