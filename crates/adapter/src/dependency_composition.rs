//! Deterministic adapter-dependency composition inference engine.
//!
//! Atlas feature: `AFA-adapter-P27-F18`.
//!
//! This module composes declared adapter capabilities for a multimodal, multi-study objective.
//! It never invokes adapter code or moves raw data: composition is a typed, local planning
//! receipt that retains missing capabilities, ambiguous providers, protected-closure gaps, and
//! policy denials as first-class states.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P27-F18";
pub const CONTRACT_VERSION: &str = "adapter-dependency-composition/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_COMPONENTS: usize = 8192;
const MAX_ITEMS: usize = 16384;

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
    pub required_capabilities: Vec<String>,
    pub components: Vec<AdapterDependencyComponent>,
    pub policy_allow: bool,
    pub protected_closure: bool,
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
            || self.component_order.is_empty()
            || self.required_capabilities.is_empty()
            || self.components.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition identity, components, reasons, effects, locality, and boundary are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("objective_id", &self.objective_id)?;
        validate_sorted_strings("required_capabilities", &self.required_capabilities)?;
        if self.components.len() > MAX_COMPONENTS
            || self
                .components
                .windows(2)
                .any(|pair| pair[0].component_id >= pair[1].component_id)
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition components are not in canonical order".into(),
            ));
        }
        let request = AdapterCompositionRequest {
            request_id: self.request_id.clone(),
            objective_id: self.objective_id.clone(),
            required_capabilities: self.required_capabilities.clone(),
            components: self.components.clone(),
            policy_allow: self.policy_allow,
            protected_closure: self.protected_closure,
            raw_data_local: self.raw_data_local,
            boundary: self.boundary.clone(),
        };
        let expected = infer_adapter_dependency_composition_internal(&request, false)?;
        if self != &expected {
            return Err(DependencyCompositionError::Contract(
                "composition receipt is not derived from its retained dependency declaration"
                    .into(),
            ));
        }
        for (field, values) in [
            ("component_order", &self.component_order),
            ("selected_order", &self.selected_order),
            ("missing_capability_order", &self.missing_capability_order),
            ("dependency_order", &self.dependency_order),
            ("modality_order", &self.modality_order),
            ("omissions", &self.omissions),
            ("uncertainty", &self.uncertainty),
            ("negative_evidence", &self.negative_evidence),
            ("reasons", &self.reasons),
            ("effect_receipts", &self.effect_receipts),
        ] {
            validate_sorted_strings(field, values)?;
        }
        if self.artifact_order.len() > MAX_ITEMS
            || self
                .artifact_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition artifact ordering is not canonical".into(),
            ));
        }
        let component_ids = self.component_order.iter().collect::<BTreeSet<_>>();
        if self
            .selected_order
            .iter()
            .any(|component| !component_ids.contains(component))
        {
            return Err(DependencyCompositionError::InvalidRequest(
                "selected components must be declared components".into(),
            ));
        }
        let expected_effect = match self.disposition {
            CompositionDisposition::Composed | CompositionDisposition::Partial => {
                "exchange:permitted-composition-manifest-and-digests"
            }
            CompositionDisposition::Blocked => "block:adapter-composition:blocked",
            CompositionDisposition::Unknown => "block:adapter-composition:unknown",
        };
        if self.effect_receipts != vec![expected_effect.to_string()] {
            return Err(DependencyCompositionError::InvalidRequest(
                "composition effect does not match its disposition".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-dependency-composition:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.adapter-composition-receipt+json"
            || self.artifact.provenance
                != composition_provenance(&self.components, &self.selected_order)
        {
            return Err(DependencyCompositionError::Contract(
                "composition artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DependencyCompositionError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&composition_payload(self))
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

fn validate_text(field: &str, value: &str) -> Result<(), DependencyCompositionError> {
    if value.is_empty() || value.trim() != value {
        return Err(DependencyCompositionError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(DependencyCompositionError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), DependencyCompositionError> {
    if values.len() > MAX_ITEMS {
        return Err(DependencyCompositionError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(DependencyCompositionError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DependencyCompositionError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn graph_has_cycle(graph: &BTreeMap<String, Vec<String>>) -> bool {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visiting.contains(node) {
            return true;
        }
        if !visited.insert(node.to_string()) {
            return false;
        }
        visiting.insert(node.to_string());
        if graph
            .get(node)
            .into_iter()
            .flatten()
            .any(|dependency| visit(dependency, graph, visiting, visited))
        {
            return true;
        }
        visiting.remove(node);
        false
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    graph
        .keys()
        .any(|node| visit(node, graph, &mut visiting, &mut visited))
}

fn composition_provenance(
    components: &[AdapterDependencyComponent],
    selected_order: &[String],
) -> Vec<ProvenanceLink> {
    let by_id = components
        .iter()
        .map(|component| (component.component_id.as_str(), component))
        .collect::<BTreeMap<_, _>>();
    selected_order
        .iter()
        .flat_map(|component_id| {
            by_id[component_id.as_str()]
                .artifact_digests
                .iter()
                .enumerate()
                .map(|(index, digest)| ProvenanceLink {
                    source_id: format!("component:{component_id}:artifact:{index}"),
                    relation: "dependency-composition-component-artifact".into(),
                    digest: digest.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn composition_payload(receipt: &AdapterCompositionReceipt) -> serde_json::Value {
    composition_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.objective_id,
        &receipt.required_capabilities,
        &receipt.components,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.disposition,
        &receipt.component_order,
        &receipt.selected_order,
        &receipt.missing_capability_order,
        &receipt.dependency_order,
        &receipt.modality_order,
        &receipt.artifact_order,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.reasons,
        &receipt.effect_receipts,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn composition_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    objective_id: &str,
    required_capabilities: &[String],
    components: &[AdapterDependencyComponent],
    policy_allow: bool,
    protected_closure: bool,
    disposition: CompositionDisposition,
    component_order: &[String],
    selected_order: &[String],
    missing_capability_order: &[String],
    dependency_order: &[String],
    modality_order: &[String],
    artifact_order: &[ContentHash],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    reasons: &[String],
    effect_receipts: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request_id,
        "objective_id": objective_id,
        "required_capabilities": required_capabilities,
        "components": components,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
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
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    infer_adapter_dependency_composition_internal(request, true)
}

fn infer_adapter_dependency_composition_internal(
    request: &AdapterCompositionRequest,
    validate_output: bool,
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
    let mut component_closure_gap = false;
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
            component_closure_gap = true;
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
    let mut dependency_graph = BTreeMap::new();
    for component_id in &selected {
        let component = by_id[component_id.as_str()];
        let dependencies = component
            .requires
            .iter()
            .filter_map(|dependency| {
                providers
                    .get(dependency)
                    .and_then(|provider_ids| provider_ids.first())
                    .map(|provider| (*provider).to_string())
            })
            .collect::<Vec<_>>();
        dependency_graph.insert(component_id.clone(), dependencies);
    }
    let cycle_detected = graph_has_cycle(&dependency_graph);
    if cycle_detected {
        uncertainty.insert("composition:dependency-cycle-detected".into());
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
    } else if !request.protected_closure || component_closure_gap || cycle_detected {
        CompositionDisposition::Unknown
    } else if missing_capability_order.is_empty() && uncertainty.is_empty() {
        CompositionDisposition::Composed
    } else if selected_order.is_empty() {
        CompositionDisposition::Unknown
    } else if !missing_capability_order.is_empty() {
        CompositionDisposition::Partial
    } else {
        CompositionDisposition::Unknown
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
    if cycle_detected {
        reasons.push("dependency cycles cannot be executed as a linear composition".into());
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
    let provenance = composition_provenance(&components, &selected_order);
    let payload = composition_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &request.objective_id,
        &request.required_capabilities,
        &components,
        request.policy_allow,
        request.protected_closure,
        disposition,
        &component_order,
        &selected_order,
        &missing_capability_order,
        &dependency_order,
        &modality_order,
        &artifact_order,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &reasons,
        &effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-dependency-composition:{}", request.request_id),
        "application/vnd.aurora.adapter-composition-receipt+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| DependencyCompositionError::Contract(error.to_string()))?;
    let result = AdapterCompositionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective_id.clone(),
        required_capabilities: request.required_capabilities.clone(),
        components,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        component_order,
        selected_order,
        missing_capability_order,
        dependency_order,
        modality_order,
        artifact_order,
        omissions,
        uncertainty,
        negative_evidence,
        reasons,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        result.validate()?;
    }
    Ok(result)
}

fn validate_request(request: &AdapterCompositionRequest) -> Result<(), DependencyCompositionError> {
    if request.request_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.required_capabilities.is_empty()
        || request.components.is_empty()
        || request.components.len() > MAX_COMPONENTS
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DependencyCompositionError::InvalidRequest(
            "request identity, objective, required capabilities, components, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("objective_id", &request.objective_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_sorted_strings("required_capabilities", &request.required_capabilities)?;
    let mut ids = BTreeSet::new();
    for component in &request.components {
        validate_text("component_id", &component.component_id)?;
        validate_text("capability_id", &component.capability_id)?;
        validate_text("input_schema", &component.input_schema)?;
        validate_text("output_schema", &component.output_schema)?;
        validate_text("component.boundary", &component.boundary)?;
        if !ids.insert(component.component_id.clone())
            || component.modality_order.is_empty()
            || component.artifact_digests.is_empty()
            || !component.raw_data_local
            || component.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(DependencyCompositionError::InvalidRequest(format!(
                "component {} is invalid or duplicated",
                component.component_id
            )));
        }
        validate_sorted_strings("modality_order", &component.modality_order)?;
        validate_sorted_strings("requires", &component.requires)?;
        validate_sorted_strings("provides", &component.provides)?;
        if component.artifact_digests.len() > MAX_ITEMS
            || component
                .artifact_digests
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || component
                .artifact_digests
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(DependencyCompositionError::InvalidRequest(format!(
                "component {} has invalid artifact digests",
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
        assert_eq!(receipt.disposition, CompositionDisposition::Unknown);
    }

    #[test]
    fn dependency_cycle_cannot_be_composed() {
        let mut value = request();
        value.components[1].requires = vec!["capability:final".into()];
        let receipt = infer_adapter_dependency_composition(&value).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("dependency-cycle")));
    }

    #[test]
    fn component_closure_gap_cannot_be_composed() {
        let mut value = request();
        value.components[0].protected_closure = false;
        let receipt = infer_adapter_dependency_composition(&value).unwrap();
        assert_eq!(receipt.disposition, CompositionDisposition::Unknown);
    }

    #[test]
    fn forged_composition_effect_is_rejected() {
        let mut receipt = infer_adapter_dependency_composition(&request()).unwrap();
        receipt.effect_receipts = vec!["block:adapter-composition:unknown".into()];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn duplicate_dependency_declaration_is_rejected() {
        let mut value = request();
        value.components[0].requires =
            vec!["capability:features".into(), "capability:features".into()];
        assert!(infer_adapter_dependency_composition(&value).is_err());
    }

    #[test]
    fn retained_component_gate_tampering_is_rejected() {
        let mut receipt = infer_adapter_dependency_composition(&request()).unwrap();
        receipt.components[0].policy_allow = false;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn composition_artifact_provenance_tampering_is_rejected() {
        let mut receipt = infer_adapter_dependency_composition(&request()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn component_input_order_is_canonicalized() {
        let mut reordered = request();
        reordered.components.reverse();
        let canonical = infer_adapter_dependency_composition(&request()).unwrap();
        let reordered = infer_adapter_dependency_composition(&reordered).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}
