//! Policy-owned high-throughput protocol simulation assurance harness.
//!
//! Atlas feature: `AFA-policy-P10-F27`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-policy-P10-F27";
pub const CONTRACT_VERSION: &str = "protocol-assurance-harness/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAssuranceRequest {
    pub request_id: String,
    pub protocol_id: String,
    pub benchmark_id: String,
    pub total_cells: usize,
    pub passed_cells: usize,
    pub blocked_cells: usize,
    pub unknown_cells: usize,
    pub protected_closure_satisfied: bool,
    pub policy_decision: PolicyDecision,
    pub simulation_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub protocol_id: String,
    pub disposition: ProtocolAssuranceDisposition,
    pub total_cells: usize,
    pub passed_cells: usize,
    pub blocked_cells: usize,
    pub unknown_cells: usize,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub simulation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ProtocolAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ProtocolAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.total_cells == 0
            || self.total_cells != self.passed_cells + self.blocked_cells + self.unknown_cells
            || self.checks.is_empty()
        {
            return Err(ProtocolAssuranceError::InvalidField(
                "protocol counts, checks, identity, or boundary".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ProtocolAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ProtocolAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ProtocolAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ProtocolAssuranceError {
    #[error("invalid protocol assurance field: {0}")]
    InvalidField(String),
    #[error("protocol assurance artifact error: {0}")]
    Artifact(String),
    #[error("protocol assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assess_protocol_assurance(
    request: &ProtocolAssuranceRequest,
) -> Result<ProtocolAssuranceReceipt, ProtocolAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.protocol_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.total_cells == 0
        || request.total_cells
            != request.passed_cells + request.blocked_cells + request.unknown_cells
    {
        return Err(ProtocolAssuranceError::InvalidField(
            "protocol identity, benchmark, boundary, and partitioned cells are required".into(),
        ));
    }
    let mut checks: Vec<String> = vec!["simulation digest is present".into()];
    let mut omissions: Vec<String> = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || request.blocked_cells > 0
        || !request.protected_closure_satisfied
    {
        checks.push("policy, blocked cells, or protected closure prevented admission".into());
        ProtocolAssuranceDisposition::Blocked
    } else if request.unknown_cells > 0 {
        omissions.push("unknown simulation cells remain unmeasured".into());
        checks.push("unknown simulation cells prevent a pass".into());
        ProtocolAssuranceDisposition::Unknown
    } else {
        checks.push("all protocol cells and policy gates passed".into());
        ProtocolAssuranceDisposition::Passed
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "protocol_id": request.protocol_id, "disposition": disposition, "total_cells": request.total_cells, "passed_cells": request.passed_cells, "blocked_cells": request.blocked_cells, "unknown_cells": request.unknown_cells, "checks": checks, "omissions": omissions, "simulation_digest": request.simulation_digest, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        "protocol-assurance-harness",
        "application/vnd.aurora.protocol-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ProtocolAssuranceError::Artifact(error.to_string()))?;
    let receipt = ProtocolAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        protocol_id: request.protocol_id.clone(),
        disposition,
        total_cells: request.total_cells,
        passed_cells: request.passed_cells,
        blocked_cells: request.blocked_cells,
        unknown_cells: request.unknown_cells,
        checks: serde_json::from_value(payload["checks"].clone())
            .map_err(|error| ProtocolAssuranceError::Serialization(error.to_string()))?,
        omissions: serde_json::from_value(payload["omissions"].clone())
            .map_err(|error| ProtocolAssuranceError::Serialization(error.to_string()))?,
        simulation_digest: request.simulation_digest.clone(),
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_cell_blocks_pass() {
        let receipt = assess_protocol_assurance(&ProtocolAssuranceRequest {
            request_id: "request:protocol".into(),
            protocol_id: "protocol:organoid".into(),
            benchmark_id: "benchmark:protocol".into(),
            total_cells: 2,
            passed_cells: 1,
            blocked_cells: 0,
            unknown_cells: 1,
            protected_closure_satisfied: true,
            policy_decision: PolicyDecision::Allow,
            simulation_digest: ContentHash::of_bytes(b"simulation"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ProtocolAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
