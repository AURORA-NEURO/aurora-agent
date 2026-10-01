//! Multimodal typed-knowledge interoperability for preclinical glioma studies.
//!
//! This gateway negotiates whether a multimodal study can exchange typed aggregate knowledge
//! across local or federated runtimes.  It reports missing modalities and capabilities explicitly,
//! preserving a degraded branch instead of pretending the study is complete.

use crate::glioma_engine::GliomaModality;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalKnowledgeProtocolGateway1@1";
pub const PROTOCOL_VERSION: &str = "glioma-multimodal-knowledge-protocol/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalKnowledgeProtocolRequest {
    pub request_id: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub required_modality_order: Vec<GliomaModality>,
    pub available_modality_order: Vec<GliomaModality>,
    pub offered_capability_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub policy_allow: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalProtocolDisposition {
    Qualified,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalProtocolNegotiation {
    pub feature_id: String,
    pub output_schema: String,
    pub protocol_version: String,
    pub request_id: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub negotiated_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub qualified_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub degraded_branch: bool,
    pub raw_data_local: bool,
    pub disposition: MultimodalProtocolDisposition,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalProtocolGatewayError {
    #[error("multimodal protocol gateway request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal protocol gateway output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal protocol gateway digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalProtocolNegotiation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "protocol_version": output.protocol_version,
        "request_id": output.request_id,
        "objective": output.objective,
        "study_order": output.study_order,
        "negotiated_capability_order": output.negotiated_capability_order,
        "missing_capability_order": output.missing_capability_order,
        "qualified_modality_order": output.qualified_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "degraded_branch": output.degraded_branch,
        "raw_data_local": output.raw_data_local,
        "disposition": output.disposition,
        "omissions": output.omissions,
        "uncertainty": output.uncertainty,
    })
}

impl MultimodalProtocolNegotiation {
    pub fn validate(&self) -> Result<(), MultimodalProtocolGatewayError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.protocol_version != PROTOCOL_VERSION
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.study_order.is_empty()
            || !canonical(&self.study_order)
            || !canonical(&self.negotiated_capability_order)
            || !canonical(&self.missing_capability_order)
            || !canonical(&self.qualified_modality_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.omissions)
            || !canonical(&self.uncertainty)
            || self.digest.as_str().len() != 64
        {
            return Err(MultimodalProtocolGatewayError::InvalidOutput(
                "protocol identity, study/modality ordering, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalProtocolGatewayError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalProtocolGatewayError::Digest(
                "multimodal protocol digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Negotiate multimodal typed-knowledge exchange while making coverage loss explicit.
pub fn negotiate_glioma_multimodal_knowledge_protocol(
    request: &MultimodalKnowledgeProtocolRequest,
) -> Result<MultimodalProtocolNegotiation, MultimodalProtocolGatewayError> {
    if request.request_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.study_order.is_empty()
        || request
            .study_order
            .iter()
            .any(|study| study.trim().is_empty())
        || !request.raw_data_local
        || request
            .offered_capability_order
            .iter()
            .any(|value| value.trim().is_empty())
        || request
            .required_capability_order
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(MultimodalProtocolGatewayError::InvalidRequest(
            "request identity, studies, local-data boundary, or capability values are invalid"
                .into(),
        ));
    }
    let studies = request.study_order.iter().cloned().collect::<BTreeSet<_>>();
    if studies.len() != request.study_order.len() {
        return Err(MultimodalProtocolGatewayError::InvalidRequest(
            "study identifiers must be unique".into(),
        ));
    }
    let offered = request
        .offered_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negotiated = offered.intersection(&required).cloned().collect::<Vec<_>>();
    let missing_capabilities = required.difference(&offered).cloned().collect::<Vec<_>>();
    let required_modalities = request
        .required_modality_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let available_modalities = request
        .available_modality_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let qualified = required_modalities
        .intersection(&available_modalities)
        .copied()
        .collect::<Vec<_>>();
    let missing_modalities = required_modalities
        .difference(&available_modalities)
        .copied()
        .collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if !missing_capabilities.is_empty() {
        omissions.insert("required-capability-missing".into());
    }
    if !missing_modalities.is_empty() {
        omissions.insert("required-modality-missing".into());
        uncertainty.insert("degraded-multimodal-branch".into());
    }
    let degraded = !missing_capabilities.is_empty() || !missing_modalities.is_empty();
    let disposition = if !request.policy_allow {
        MultimodalProtocolDisposition::Blocked
    } else if degraded {
        MultimodalProtocolDisposition::Partial
    } else {
        MultimodalProtocolDisposition::Qualified
    };
    let mut output = MultimodalProtocolNegotiation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        protocol_version: PROTOCOL_VERSION.into(),
        request_id: request.request_id.clone(),
        objective: request.objective.clone(),
        study_order: studies.into_iter().collect(),
        negotiated_capability_order: negotiated,
        missing_capability_order: missing_capabilities,
        qualified_modality_order: qualified,
        missing_modality_order: missing_modalities,
        degraded_branch: degraded,
        raw_data_local: request.raw_data_local,
        disposition,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalProtocolGatewayError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(policy_allow: bool, missing_modality: bool) -> MultimodalKnowledgeProtocolRequest {
        MultimodalKnowledgeProtocolRequest {
            request_id: "request-mm-1".into(),
            objective: "exchange multimodal glioma knowledge".into(),
            study_order: vec!["study-a".into(), "study-b".into()],
            required_modality_order: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            available_modality_order: if missing_modality {
                vec![GliomaModality::Genomics]
            } else {
                vec![GliomaModality::Genomics, GliomaModality::Imaging]
            },
            offered_capability_order: vec!["typed_claims".into(), "modality_alignment".into()],
            required_capability_order: vec!["typed_claims".into()],
            policy_allow,
            raw_data_local: true,
        }
    }

    #[test]
    fn complete_multimodal_exchange_is_qualified() {
        let output = negotiate_glioma_multimodal_knowledge_protocol(&request(true, false)).unwrap();
        assert_eq!(output.disposition, MultimodalProtocolDisposition::Qualified);
        assert!(!output.degraded_branch);
        output.validate().unwrap();
    }

    #[test]
    fn missing_modality_selects_degraded_branch() {
        let output = negotiate_glioma_multimodal_knowledge_protocol(&request(true, true)).unwrap();
        assert_eq!(output.disposition, MultimodalProtocolDisposition::Partial);
        assert!(output.degraded_branch);
        assert_eq!(output.missing_modality_order, vec![GliomaModality::Imaging]);
    }

    #[test]
    fn policy_denial_blocks_exchange() {
        let output =
            negotiate_glioma_multimodal_knowledge_protocol(&request(false, false)).unwrap();
        assert_eq!(output.disposition, MultimodalProtocolDisposition::Blocked);
    }
}
