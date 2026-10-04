//! Typed-knowledge interoperability gateway for local and federated glioma research.
//!
//! The gateway negotiates capabilities and export boundaries before a knowledge artifact enters a
//! downstream workflow.  It exchanges typed claims, aggregate metrics, digests, and provenance
//! metadata only; raw data, identifiers, clinical decisions, and instrument commands are never
//! admitted.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeProtocolGateway1@1";
pub const PROTOCOL_VERSION: &str = "glioma-knowledge-protocol/1.0";
const DENIED_EXPORTS: &[&str] = &[
    "raw_data",
    "human_identifiers",
    "clinical_decision",
    "instrument_command",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeProtocolRequest {
    pub request_id: String,
    pub objective: String,
    pub offered_capability_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub requested_export_order: Vec<String>,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub allow_federation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeProtocolDisposition {
    Accepted,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeProtocolNegotiation {
    pub feature_id: String,
    pub output_schema: String,
    pub protocol_version: String,
    pub request_id: String,
    pub objective: String,
    pub negotiated_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub allowed_export_order: Vec<String>,
    pub denied_export_order: Vec<String>,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub disposition: KnowledgeProtocolDisposition,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeProtocolGatewayError {
    #[error("knowledge protocol gateway request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge protocol gateway output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge protocol gateway digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &KnowledgeProtocolNegotiation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "protocol_version": output.protocol_version,
        "request_id": output.request_id,
        "objective": output.objective,
        "negotiated_capability_order": output.negotiated_capability_order,
        "missing_capability_order": output.missing_capability_order,
        "allowed_export_order": output.allowed_export_order,
        "denied_export_order": output.denied_export_order,
        "federation_allowed": output.federation_allowed,
        "raw_data_local": output.raw_data_local,
        "disposition": output.disposition,
        "omissions": output.omissions,
        "uncertainty": output.uncertainty,
    })
}

impl KnowledgeProtocolNegotiation {
    pub fn validate(&self) -> Result<(), KnowledgeProtocolGatewayError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.protocol_version != PROTOCOL_VERSION
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.negotiated_capability_order)
            || !canonical(&self.missing_capability_order)
            || !canonical(&self.allowed_export_order)
            || !canonical(&self.denied_export_order)
            || !canonical(&self.omissions)
            || !canonical(&self.uncertainty)
            || self.denied_export_order.iter().any(|export| {
                !DENIED_EXPORTS.contains(&export.as_str())
                    && self.allowed_export_order.contains(export)
            })
            || self.digest.as_str().len() != 64
        {
            return Err(KnowledgeProtocolGatewayError::InvalidOutput(
                "protocol identity, canonical ordering, export boundary, or digest is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeProtocolGatewayError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeProtocolGatewayError::Digest(
                "protocol negotiation digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Negotiate a typed-knowledge exchange without permitting raw or clinical data movement.
pub fn negotiate_glioma_knowledge_protocol(
    request: &KnowledgeProtocolRequest,
) -> Result<KnowledgeProtocolNegotiation, KnowledgeProtocolGatewayError> {
    if request.request_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || !request.raw_data_local
        || request
            .offered_capability_order
            .iter()
            .any(|value| value.trim().is_empty())
        || request
            .required_capability_order
            .iter()
            .any(|value| value.trim().is_empty())
        || request
            .requested_export_order
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(KnowledgeProtocolGatewayError::InvalidRequest(
            "request identity, objective, local-data boundary, and capability values are invalid"
                .into(),
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
    let missing = required.difference(&offered).cloned().collect::<Vec<_>>();
    let requested = request
        .requested_export_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let denied = requested
        .iter()
        .filter(|value| DENIED_EXPORTS.contains(&value.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let allowed = requested
        .difference(&denied.iter().cloned().collect())
        .cloned()
        .collect::<Vec<_>>();
    let federation_allowed =
        request.allow_federation && request.raw_data_local && request.policy_allow;
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if !missing.is_empty() {
        omissions.insert("required-capability-missing".into());
    }
    if !denied.is_empty() {
        omissions.insert("export-policy-denied".into());
    }
    if request.allow_federation && !federation_allowed {
        uncertainty.insert("federation-policy-not-authorized".into());
    }
    let disposition = if !request.policy_allow {
        KnowledgeProtocolDisposition::Blocked
    } else if !missing.is_empty() || !denied.is_empty() {
        KnowledgeProtocolDisposition::Partial
    } else {
        KnowledgeProtocolDisposition::Accepted
    };
    let mut output = KnowledgeProtocolNegotiation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        protocol_version: PROTOCOL_VERSION.into(),
        request_id: request.request_id.clone(),
        objective: request.objective.clone(),
        negotiated_capability_order: negotiated,
        missing_capability_order: missing,
        allowed_export_order: allowed,
        denied_export_order: denied,
        federation_allowed,
        raw_data_local: request.raw_data_local,
        disposition,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeProtocolGatewayError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(policy_allow: bool, exports: Vec<&str>) -> KnowledgeProtocolRequest {
        KnowledgeProtocolRequest {
            request_id: "request-1".into(),
            objective: "exchange typed glioma knowledge".into(),
            offered_capability_order: vec!["typed_claims".into(), "provenance".into()],
            required_capability_order: vec!["typed_claims".into()],
            requested_export_order: exports.into_iter().map(str::to_string).collect(),
            policy_allow,
            raw_data_local: true,
            allow_federation: true,
        }
    }

    #[test]
    fn typed_exchange_is_accepted() {
        let output =
            negotiate_glioma_knowledge_protocol(&request(true, vec!["typed_claims"])).unwrap();
        assert_eq!(output.disposition, KnowledgeProtocolDisposition::Accepted);
        assert!(output.federation_allowed);
        output.validate().unwrap();
    }

    #[test]
    fn raw_export_is_partial_not_silently_allowed() {
        let output =
            negotiate_glioma_knowledge_protocol(&request(true, vec!["typed_claims", "raw_data"]))
                .unwrap();
        assert_eq!(output.disposition, KnowledgeProtocolDisposition::Partial);
        assert_eq!(output.denied_export_order, vec!["raw_data"]);
    }

    #[test]
    fn denied_policy_blocks_exchange() {
        let output =
            negotiate_glioma_knowledge_protocol(&request(false, vec!["typed_claims"])).unwrap();
        assert_eq!(output.disposition, KnowledgeProtocolDisposition::Blocked);
    }
}
