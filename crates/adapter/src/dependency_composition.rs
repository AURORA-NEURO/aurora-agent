//! Deterministic adapter-dependency composition inference engine.
//!
//! Atlas feature: `AFA-adapter-P27-F18`.
//!
//! This module composes declared adapter capabilities for a multimodal, multi-study objective.
//! It never invokes adapter code or moves raw data: composition is a typed, local planning
//! receipt that retains missing capabilities, ambiguous providers, protected-closure gaps, and
//! policy denials as first-class states.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P27-F18";
pub const CONTRACT_VERSION: &str = "adapter-dependency-composition/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDependencyComponent {
    pub component_id: String,
    pub capability_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub modality_order: Vec<String>,
    pub requires: Vec<String>,
    pub provides: Vec<String>,
    pub artifact_digests: Vec<ContentHash>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCompositionRequest {
    pub request_id: String,
    pub objective_id: String,
    pub required_capabilities: Vec<String>,
    pub components: Vec<AdapterDependencyComponent>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionDisposition {
    Composed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCompositionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective_id: String,
    pub disposition: CompositionDisposition,
    pub component_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub reasons: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl AdapterCompositionReceipt {
    pub fn validate(&self) -> Result<(), DependencyCompositionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(DependencyCompositionError::Contract(
                "adapter dependency composition identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.component_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition identity, components, reasons, effects, locality, and boundary are required".into(),
            ));
        }
        for values in [
            &self.component_order,
            &self.selected_order,
            &self.missing_capability_order,
            &self.dependency_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.reasons,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(DependencyCompositionError::InvalidRequest(
                    "composition output ordering is not canonical".into(),
                ));
            }
        }
        if self
            .artifact_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition artifact ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DependencyCompositionError::Contract(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, DependencyCompositionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| DependencyCompositionError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| DependencyCompositionError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum DependencyCompositionError {
    #[error("invalid adapter dependency composition input: {0}")]
    InvalidRequest(String),
    #[error("adapter dependency composition contract rejected: {0}")]
    Contract(String),
    #[error("adapter dependency composition serialization failed: {0}")]
    Serialization(String),
}

pub fn infer_adapter_dependency_composition(
    request: &AdapterCompositionRequest,
) -> Result<AdapterCompositionReceipt, DependencyCompositionError> {
    validate_request(request)?;
    let mut components = request.components.clone();
    components.sort_by(|a, b| a.component_id.cmp(&b.component_id));
    let component_order = components
        .iter()
        .map(|component| component.component_id.clone())
        .collect::<Vec<_>>();
    let by_id = components
        .iter()
        .map(|component| (component.component_id.as_str(), component))
        .collect::<BTreeMap<_, _>>();
    let mut providers: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for component in &components {
        providers
            .entry(component.capability_id.clone())
            .or_default()
            .push(component.component_id.as_str());
        for capability in &component.provides {
            providers
                .entry(capability.clone())
                .or_default()
                .push(component.component_id.as_str());
        }
    }
    for ids in providers.values_mut() {
        ids.sort_unstable();
        ids.dedup();
    }
    let mut pending = request
        .required_capabilities
        .iter()
        .cloned()
        .collect::<VecDeque<_>>();
    let mut seen_capabilities = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut dependency_order = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut blocked_component = false;
    while let Some(capability) = pending.pop_front() {
        if !seen_capabilities.insert(capability.clone()) {
            continue;
        }
        let candidate_ids = providers.get(&capability).cloned().unwrap_or_default();
        if candidate_ids.is_empty() {
            missing.insert(capability.clone());
            omissions.insert(format!("capability:{capability}:no-compatible-provider"));
            continue;
        }
        if candidate_ids.len() > 1 {
            uncertainty.insert(format!(
                "capability:{capability}:multiple-providers-ranked-by-component-id"
            ));
        }
        let component_id = candidate_ids[0];
        let component = by_id[component_id];
        selected.insert(component_id.to_owned());
        if !component.policy_allow || !component.raw_data_local {
            blocked_component = true;
            omissions.insert(format!(
                "component:{}:policy-or-locality-denied",
                component.component_id
            ));
        }
        if !component.protected_closure {
            uncertainty.insert(format!(
                "component:{}:protected-closure-incomplete",
                component.component_id
            ));
        }
        for dependency in &component.requires {
            dependency_order.insert(format!("{}->{dependency}", component.component_id));
            pending.push_back(dependency.clone());
        }
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let missing_capability_order = missing.into_iter().collect::<Vec<_>>();
    let dependency_order = dependency_order.into_iter().collect::<Vec<_>>();
    let mut modalities = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for component_id in &selected_order {
        let component = by_id[component_id.as_str()];
        modalities.extend(component.modality_order.iter().cloned());
        artifacts.extend(component.artifact_digests.iter().cloned());
        if component.modality_order.is_empty() {
            negative_evidence.insert(format!(
                "component:{}:no-declared-modality",
                component.component_id
            ));
        }
    }
    for capability in &missing_capability_order {
        negative_evidence.insert(format!(
            "capability:{capability}:negative-provider-evidence"
        ));
    }
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let mut omissions = omissions.into_iter().collect::<Vec<_>>();
    if !request.protected_closure {
        uncertainty.push("request:protected-closure-incomplete".into());
    }
    uncertainty.sort();
    omissions.sort();
    let disposition = if !request.policy_allow || !request.raw_data_local || blocked_component {
        CompositionDisposition::Blocked
    } else if !request.protected_closure {
        CompositionDisposition::Unknown
    } else if missing_capability_order.is_empty() {
        CompositionDisposition::Composed
    } else if selected_order.is_empty() {
        CompositionDisposition::Unknown
    } else {
        CompositionDisposition::Partial
    };
    let mut reasons = vec![format!(
        "{} required capabilities evaluated across {} declared components",
        request.required_capabilities.len(),
        component_order.len()
    )];
    if !missing_capability_order.is_empty() {
        reasons.push("missing capabilities remain explicit and cannot be executed".into());
    }
    if matches!(disposition, CompositionDisposition::Blocked) {
        reasons.push(
            "policy, locality, or selected-component authorization denied composition".into(),
        );
    }
    reasons.sort();
    let negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(
        disposition,
        CompositionDisposition::Composed | CompositionDisposition::Partial
    ) {
        vec!["exchange:permitted-composition-manifest-and-digests".into()]
    } else {
        vec![format!("block:adapter-composition:{disposition:?}").to_lowercase()]
    };
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "objective_id": request.objective_id,
        "disposition": disposition,
        "component_order": component_order,
        "selected_order": selected_order,
        "missing_capability_order": missing_capability_order,
        "dependency_order": dependency_order,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "reasons": reasons,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-dependency-composition:{}", request.request_id),
        "application/vnd.aurora.adapter-composition-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| DependencyCompositionError::Contract(error.to_string()))?;
    let result = AdapterCompositionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective_id.clone(),
        disposition,
        component_order: payload["component_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_capability_order: payload["missing_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        dependency_order: payload["dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        modality_order: payload["modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        artifact_order,
        omissions: payload["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty: payload["uncertainty"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence: payload["negative_evidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        reasons: payload["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_request(request: &AdapterCompositionRequest) -> Result<(), DependencyCompositionError> {
    if request.request_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request.components.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DependencyCompositionError::InvalidRequest(
            "request identity, objective, required capabilities, components, locality, and boundary are required".into(),
        ));
    }
    if request
        .required_capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
    {
        return Err(DependencyCompositionError::InvalidRequest(
            "required capabilities cannot be empty".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for component in &request.components {
        if component.component_id.trim().is_empty()
            || !ids.insert(component.component_id.clone())
            || component.capability_id.trim().is_empty()
            || component.input_schema.trim().is_empty()
            || component.output_schema.trim().is_empty()
            || component.modality_order.is_empty()
            || component.artifact_digests.is_empty()
            || !component.raw_data_local
            || component.boundary != PRECLINICAL_BOUNDARY
            || component
                .requires
                .iter()
                .any(|value| value.trim().is_empty())
            || component
                .provides
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(DependencyCompositionError::InvalidRequest(format!(
                "component {} is invalid or duplicated",
                component.component_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(id: &str, capability: &str, requires: &[&str]) -> AdapterDependencyComponent {
        AdapterDependencyComponent {
            component_id: id.into(),
            capability_id: capability.into(),
            input_schema: format!("{capability}Input"),
            output_schema: format!("{capability}Output"),
            modality_order: vec!["imaging".into(), "omics".into()],
            requires: requires.iter().map(|value| (*value).into()).collect(),
            provides: Vec::new(),
            artifact_digests: vec![ContentHash::of_bytes(id.as_bytes())],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request() -> AdapterCompositionRequest {
        AdapterCompositionRequest {
            request_id: "composition:multimodal".into(),
            objective_id: "objective:qc".into(),
            required_capabilities: vec!["capability:final".into()],
            components: vec![
                component(
                    "component:final",
                    "capability:final",
                    &["capability:features"],
                ),
                component("component:features", "capability:features", &[]),
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn composes_dependency_closure_deterministically() {
        let receipt = infer_adapter_dependency_composition(&request()).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Composed);
        assert_eq!(receipt.selected_order.len(), 2);
        assert_eq!(
            receipt.digest().unwrap(),
            infer_adapter_dependency_composition(&request())
                .unwrap()
                .digest()
                .unwrap()
        );
    }

    #[test]
    fn missing_capability_is_partial_with_negative_evidence() {
        let mut request = request();
        request
            .required_capabilities
            .push("capability:missing".into());
        let receipt = infer_adapter_dependency_composition(&request).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Partial);
        assert!(!receipt.negative_evidence.is_empty());
        assert!(!receipt.missing_capability_order.is_empty());
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut request = request();
        request.protected_closure = false;
        let receipt = infer_adapter_dependency_composition(&request).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Unknown);
    }

    #[test]
    fn denied_component_blocks_composition() {
        let mut request = request();
        request.components[0].policy_allow = false;
        let receipt = infer_adapter_dependency_composition(&request).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Blocked);
    }

    #[test]
    fn duplicate_provider_is_explicitly_uncertain() {
        let mut request = request();
        request.components.push(component(
            "component:features-2",
            "capability:features",
            &[],
        ));
        let receipt = infer_adapter_dependency_composition(&request).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|value| value.contains("multiple-providers")));
    }
}
