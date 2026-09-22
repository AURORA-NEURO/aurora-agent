//! Evidence-to-workflow transport gate for preclinical glioma research.
//!
//! Cross-study fusion is not itself permission to generalize a result to a new model system.
//! This feature converts a fused protocol evidence surface into endpoint-level transport
//! decisions using independent-study support, site diversity, model coverage, information, and
//! heterogeneity gates. It preserves negative and blocked findings and emits the next bounded
//! research action for an autonomous engine.

use super::multistudy_fusion::{ProtocolEvidenceFusion, ProtocolFusionDisposition};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolTransportGate1@1";
pub const MAX_ENDPOINTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTransportGateRequest {
    pub objective: String,
    pub fusion: ProtocolEvidenceFusion,
    pub target_model_system: GliomaModelSystem,
    pub min_studies: u16,
    pub min_sites: u16,
    pub min_model_systems: u16,
    pub min_information_milli: u32,
    pub max_heterogeneity_milli: u32,
    pub require_positive_signal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolTransportEndpointDisposition {
    Ready,
    Negative,
    Replicate,
    Heterogeneous,
    Contradictory,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTransportEndpoint {
    pub endpoint_id: String,
    pub study_order: Vec<String>,
    pub site_order: Vec<String>,
    pub source_model_order: Vec<GliomaModelSystem>,
    pub target_model_system: GliomaModelSystem,
    pub measured_study_count: u16,
    pub information_milli: u32,
    pub heterogeneity_milli: u32,
    pub sign_consistency_milli: u16,
    pub source_disposition: ProtocolFusionDisposition,
    pub disposition: ProtocolTransportEndpointDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolTransportGateDisposition {
    Ready,
    Negative,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTransportGate {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub target_model_system: GliomaModelSystem,
    pub source_fusion_digest: ContentHash,
    pub endpoints: Vec<ProtocolTransportEndpoint>,
    pub ready_endpoint_order: Vec<String>,
    pub negative_endpoint_order: Vec<String>,
    pub replicate_endpoint_order: Vec<String>,
    pub heterogeneous_endpoint_order: Vec<String>,
    pub contradictory_endpoint_order: Vec<String>,
    pub unresolved_endpoint_order: Vec<String>,
    pub next_actions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProtocolTransportGateDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolTransportGateError {
    #[error("protocol transport gate request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol transport gate input is invalid: {0}")]
    InvalidInput(String),
    #[error("protocol transport gate output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol transport gate digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(gate: &ProtocolTransportGate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": gate.feature_id,
        "output_schema": gate.output_schema,
        "objective": gate.objective,
        "target_model_system": gate.target_model_system,
        "source_fusion_digest": gate.source_fusion_digest,
        "endpoints": gate.endpoints,
        "ready_endpoint_order": gate.ready_endpoint_order,
        "negative_endpoint_order": gate.negative_endpoint_order,
        "replicate_endpoint_order": gate.replicate_endpoint_order,
        "heterogeneous_endpoint_order": gate.heterogeneous_endpoint_order,
        "contradictory_endpoint_order": gate.contradictory_endpoint_order,
        "unresolved_endpoint_order": gate.unresolved_endpoint_order,
        "next_actions": gate.next_actions,
        "negative_evidence": gate.negative_evidence,
        "uncertainty": gate.uncertainty,
        "disposition": gate.disposition,
    })
}

fn validate_request(
    request: &ProtocolTransportGateRequest,
) -> Result<(), ProtocolTransportGateError> {
    if request.objective.trim().is_empty()
        || request.min_studies == 0
        || request.min_sites == 0
        || request.min_model_systems == 0
        || request.min_information_milli > 1_000_000
    {
        return Err(ProtocolTransportGateError::InvalidRequest(
            "objective, independent-study/site/model floors, and bounded information/heterogeneity gates are required".into(),
        ));
    }
    request
        .fusion
        .validate()
        .map_err(|error| ProtocolTransportGateError::InvalidInput(error.to_string()))?;
    if request.fusion.objective != request.objective {
        return Err(ProtocolTransportGateError::InvalidInput(
            "fusion objective does not match transport-gate objective".into(),
        ));
    }
    if request.fusion.cells.is_empty() || request.fusion.cells.len() > MAX_ENDPOINTS {
        return Err(ProtocolTransportGateError::InvalidInput(
            "fusion endpoint count is outside the bounded transport-gate range".into(),
        ));
    }
    Ok(())
}

fn validate_gate(gate: &ProtocolTransportGate) -> Result<(), ProtocolTransportGateError> {
    if gate.feature_id != FEATURE_ID
        || gate.output_schema != OUTPUT_SCHEMA
        || gate.objective.trim().is_empty()
        || gate.source_fusion_digest.as_str().len() != 64
        || gate.endpoints.is_empty()
        || gate.endpoints.len() > MAX_ENDPOINTS
        || !canonical(&gate.ready_endpoint_order)
        || !canonical(&gate.negative_endpoint_order)
        || !canonical(&gate.replicate_endpoint_order)
        || !canonical(&gate.heterogeneous_endpoint_order)
        || !canonical(&gate.contradictory_endpoint_order)
        || !canonical(&gate.unresolved_endpoint_order)
        || !canonical(&gate.next_actions)
        || !canonical(&gate.negative_evidence)
        || !canonical(&gate.uncertainty)
        || gate
            .endpoints
            .windows(2)
            .any(|pair| pair[0].endpoint_id >= pair[1].endpoint_id)
        || gate.endpoints.iter().any(|endpoint| {
            endpoint.endpoint_id.trim().is_empty()
                || endpoint.study_order.is_empty()
                || !canonical(&endpoint.study_order)
                || endpoint.site_order.is_empty()
                || !canonical(&endpoint.site_order)
                || endpoint.source_model_order.is_empty()
                || !canonical(&endpoint.source_model_order)
                || endpoint.measured_study_count == 0
                || endpoint.information_milli > 1_000_000
                || endpoint.sign_consistency_milli > 1_000
                || endpoint.next_action.trim().is_empty()
                || !canonical(&endpoint.negative_evidence)
                || !canonical(&endpoint.uncertainty)
        })
    {
        return Err(ProtocolTransportGateError::InvalidOutput(
            "identity, ordering, support, score, or next-action invariants are invalid".into(),
        ));
    }
    let endpoint_ids = gate
        .endpoints
        .iter()
        .map(|endpoint| endpoint.endpoint_id.clone())
        .collect::<BTreeSet<_>>();
    let mut classified = BTreeSet::new();
    for order in [
        &gate.ready_endpoint_order,
        &gate.negative_endpoint_order,
        &gate.replicate_endpoint_order,
        &gate.heterogeneous_endpoint_order,
        &gate.contradictory_endpoint_order,
        &gate.unresolved_endpoint_order,
    ] {
        for endpoint_id in order.iter() {
            if !endpoint_ids.contains(endpoint_id) || !classified.insert(endpoint_id) {
                return Err(ProtocolTransportGateError::InvalidOutput(
                    "endpoint disposition orders do not partition the endpoint set".into(),
                ));
            }
        }
    }
    if classified.len() != endpoint_ids.len() {
        return Err(ProtocolTransportGateError::InvalidOutput(
            "endpoint disposition orders omit one or more endpoints".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(gate))
        .map_err(|error| ProtocolTransportGateError::Digest(error.to_string()))?;
    if expected != gate.digest {
        return Err(ProtocolTransportGateError::InvalidOutput(
            "digest is not bound to the transport gate".into(),
        ));
    }
    Ok(())
}

impl ProtocolTransportGate {
    pub fn validate(&self) -> Result<(), ProtocolTransportGateError> {
        validate_gate(self)
    }
}

/// Decide which fused endpoints may be handed to a target-model workflow.
pub fn gate_glioma_protocol_transport(
    request: &ProtocolTransportGateRequest,
) -> Result<ProtocolTransportGate, ProtocolTransportGateError> {
    validate_request(request)?;
    let mut endpoints = Vec::with_capacity(request.fusion.cells.len());
    let mut ready = Vec::new();
    let mut negative = Vec::new();
    let mut replicate = Vec::new();
    let mut heterogeneous = Vec::new();
    let mut contradictory = Vec::new();
    let mut unresolved = Vec::new();
    let mut next_actions = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();

    for cell in &request.fusion.cells {
        let target_observed = cell.model_order.contains(&request.target_model_system);
        let support_sufficient = cell.measured_study_count >= request.min_studies
            && cell.site_order.len() >= usize::from(request.min_sites)
            && cell.model_order.len() >= usize::from(request.min_model_systems)
            && cell.information_milli >= request.min_information_milli;
        let positive_signal = cell
            .robust_value_milli
            .map(|value| value > 0)
            .unwrap_or(false);
        let (disposition, next_action) =
            if cell.disposition == ProtocolFusionDisposition::Contradictory {
                contradictory.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Contradictory,
                    format!(
                        "reconcile contradictory endpoint {} before target-model transport",
                        cell.endpoint_id
                    ),
                )
            } else if cell.disposition == ProtocolFusionDisposition::Heterogeneous
                || cell.heterogeneity_milli > request.max_heterogeneity_milli
            {
                heterogeneous.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Heterogeneous,
                    format!(
                        "stratify endpoint {} by model and modality before transport",
                        cell.endpoint_id
                    ),
                )
            } else if cell.disposition == ProtocolFusionDisposition::Negative {
                negative.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Negative,
                    format!(
                        "retain replicated negative endpoint {} and test alternatives",
                        cell.endpoint_id
                    ),
                )
            } else if cell.disposition == ProtocolFusionDisposition::Unresolved {
                unresolved.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Unresolved,
                    format!(
                        "resolve missing or unresolved evidence for endpoint {}",
                        cell.endpoint_id
                    ),
                )
            } else if !support_sufficient
                || !target_observed
                || (request.require_positive_signal && !positive_signal)
            {
                replicate.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Replicate,
                    format!(
                        "add independent target-model/site support for endpoint {}",
                        cell.endpoint_id
                    ),
                )
            } else {
                ready.push(cell.endpoint_id.clone());
                (
                    ProtocolTransportEndpointDisposition::Ready,
                    format!(
                        "handoff endpoint {} to target-model mechanism and experiment workflows",
                        cell.endpoint_id
                    ),
                )
            };
        let mut endpoint_negative = cell.negative_evidence.clone();
        if matches!(
            disposition,
            ProtocolTransportEndpointDisposition::Negative
                | ProtocolTransportEndpointDisposition::Contradictory
        ) {
            endpoint_negative.push(format!("{}:transport-boundary", cell.endpoint_id));
        }
        endpoint_negative.sort();
        endpoint_negative.dedup();
        let mut endpoint_uncertainty = cell.uncertainty.clone();
        if !target_observed {
            endpoint_uncertainty.push(format!("{}:target-model-unobserved", cell.endpoint_id));
        }
        if cell.site_order.len() < usize::from(request.min_sites) {
            endpoint_uncertainty.push(format!("{}:site-support-below-gate", cell.endpoint_id));
        }
        if cell.model_order.len() < usize::from(request.min_model_systems) {
            endpoint_uncertainty.push(format!("{}:model-support-below-gate", cell.endpoint_id));
        }
        endpoint_uncertainty.sort();
        endpoint_uncertainty.dedup();
        negative_evidence.extend(endpoint_negative.clone());
        uncertainty.extend(endpoint_uncertainty.clone());
        next_actions.push(next_action.clone());
        endpoints.push(ProtocolTransportEndpoint {
            endpoint_id: cell.endpoint_id.clone(),
            study_order: cell.study_order.clone(),
            site_order: cell.site_order.clone(),
            source_model_order: cell.model_order.clone(),
            target_model_system: request.target_model_system,
            measured_study_count: cell.measured_study_count,
            information_milli: cell.information_milli,
            heterogeneity_milli: cell.heterogeneity_milli,
            sign_consistency_milli: cell.sign_consistency_milli,
            source_disposition: cell.disposition,
            disposition,
            negative_evidence: endpoint_negative,
            uncertainty: endpoint_uncertainty,
            next_action,
        });
    }
    endpoints.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    ready.sort();
    negative.sort();
    replicate.sort();
    heterogeneous.sort();
    contradictory.sort();
    unresolved.sort();
    next_actions.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if !contradictory.is_empty() || !heterogeneous.is_empty() {
        ProtocolTransportGateDisposition::Blocked
    } else if !ready.is_empty()
        && replicate.is_empty()
        && negative.is_empty()
        && unresolved.is_empty()
    {
        ProtocolTransportGateDisposition::Ready
    } else if !negative.is_empty()
        && ready.is_empty()
        && replicate.is_empty()
        && unresolved.is_empty()
    {
        ProtocolTransportGateDisposition::Negative
    } else if !ready.is_empty() || !negative.is_empty() || !replicate.is_empty() {
        ProtocolTransportGateDisposition::Partial
    } else {
        ProtocolTransportGateDisposition::Blocked
    };
    let mut gate = ProtocolTransportGate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        target_model_system: request.target_model_system,
        source_fusion_digest: request.fusion.digest.clone(),
        endpoints,
        ready_endpoint_order: ready,
        negative_endpoint_order: negative,
        replicate_endpoint_order: replicate,
        heterogeneous_endpoint_order: heterogeneous,
        contradictory_endpoint_order: contradictory,
        unresolved_endpoint_order: unresolved,
        next_actions,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-transport-gate"),
    };
    gate.digest = ContentHash::of_value(&digest_input(&gate))
        .map_err(|error| ProtocolTransportGateError::Digest(error.to_string()))?;
    validate_gate(&gate)?;
    Ok(gate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::evidence_surface::{
        ProtocolEvidenceCell, ProtocolEvidenceDisposition, ProtocolEvidenceSurface,
        ProtocolEvidenceSurfaceDisposition,
    };
    use crate::glioma::programs::p07_protocol_simulation::multistudy_fusion::{
        fuse_glioma_protocol_evidence, ProtocolEvidenceFusionRequest, ProtocolEvidenceStudySurface,
    };

    fn surface(
        study: &str,
        model_system: GliomaModelSystem,
        value: i32,
    ) -> ProtocolEvidenceSurface {
        let mut surface = ProtocolEvidenceSurface {
            feature_id: super::super::evidence_surface::FEATURE_ID.into(),
            output_schema: super::super::evidence_surface::OUTPUT_SCHEMA.into(),
            objective: "transport invasion evidence".into(),
            protocol_digest: ContentHash::of_bytes(format!("protocol-{study}").as_bytes()),
            execution_digest: ContentHash::of_bytes(format!("execution-{study}").as_bytes()),
            cells: vec![ProtocolEvidenceCell {
                endpoint_id: "invasion".into(),
                measurement_order: vec![format!("{study}-m1"), format!("{study}-m2")],
                task_order: vec!["assay".into()],
                modality_order: vec!["imaging".into()],
                measurement_count: 2,
                replicate_count: 2,
                robust_value_milli: Some(value),
                spread_milli: 20,
                quality_milli: 900,
                max_uncertainty_milli: 50,
                disposition: if value == 0 {
                    ProtocolEvidenceDisposition::Negative
                } else {
                    ProtocolEvidenceDisposition::Qualified
                },
                information_milli: 800,
                negative_evidence: Vec::new(),
                uncertainty: Vec::new(),
                next_action: "handoff".into(),
            }],
            qualified_endpoint_order: if value == 0 {
                Vec::new()
            } else {
                vec!["invasion".into()]
            },
            negative_endpoint_order: if value == 0 {
                vec!["invasion".into()]
            } else {
                Vec::new()
            },
            partial_endpoint_order: Vec::new(),
            unresolved_endpoint_order: Vec::new(),
            contradictory_endpoint_order: Vec::new(),
            overall_information_milli: 800,
            next_actions: vec!["handoff".into()],
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: ProtocolEvidenceSurfaceDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        surface.digest =
            ContentHash::of_value(&super::super::evidence_surface::digest_input(&surface)).unwrap();
        let _ = model_system;
        surface
    }

    fn request(values: &[(GliomaModelSystem, &str, i32)]) -> ProtocolTransportGateRequest {
        let studies = values
            .iter()
            .enumerate()
            .map(
                |(index, (model_system, site_id, value))| ProtocolEvidenceStudySurface {
                    study_id: format!("study-{index}"),
                    site_id: (*site_id).into(),
                    model_system: *model_system,
                    surface: surface(&format!("{index}"), *model_system, *value),
                },
            )
            .collect::<Vec<_>>();
        let fusion_request = ProtocolEvidenceFusionRequest {
            objective: "transport invasion evidence".into(),
            studies,
            min_studies: 2,
            min_quality_milli: 700,
            max_heterogeneity_milli: 100,
            contradiction_threshold_milli: 100,
        };
        let fusion = fuse_glioma_protocol_evidence(&fusion_request).unwrap();
        ProtocolTransportGateRequest {
            objective: "transport invasion evidence".into(),
            fusion,
            target_model_system: GliomaModelSystem::MouseModel,
            min_studies: 2,
            min_sites: 2,
            min_model_systems: 2,
            min_information_milli: 700,
            max_heterogeneity_milli: 100,
            require_positive_signal: true,
        }
    }

    #[test]
    fn transport_gate_releases_target_model_supported_endpoint() {
        let gate = gate_glioma_protocol_transport(&request(&[
            (GliomaModelSystem::Organoid, "site-a", 400),
            (GliomaModelSystem::MouseModel, "site-b", 430),
        ]))
        .unwrap();
        assert_eq!(gate.disposition, ProtocolTransportGateDisposition::Ready);
        assert_eq!(gate.ready_endpoint_order, vec!["invasion"]);
        assert!(gate.uncertainty.is_empty());
        gate.validate().unwrap();
    }

    #[test]
    fn transport_gate_blocks_target_model_absence_and_preserves_negative() {
        let gate = gate_glioma_protocol_transport(&request(&[
            (GliomaModelSystem::Organoid, "site-a", 0),
            (GliomaModelSystem::Organoid, "site-b", 0),
        ]))
        .unwrap();
        assert_eq!(gate.disposition, ProtocolTransportGateDisposition::Negative);
        assert_eq!(gate.negative_endpoint_order, vec!["invasion"]);
        assert!(gate.endpoints.iter().any(|endpoint| endpoint
            .uncertainty
            .iter()
            .any(|item| item.contains("target-model-unobserved"))));
    }
}
