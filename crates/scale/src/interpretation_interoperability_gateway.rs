//! Multimodal interpretation interoperability gateway (`AFA-scale-P14-F22`).
//!
//! Negotiates versioned, aggregate-only interpretation exchange. The gateway never transfers raw
//! experimental payloads and refuses semantic drift, missing provenance, or incomplete policy
//! closure instead of silently coercing a representation.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-scale-P14-F22";
pub const CONTRACT_VERSION: &str =
    "scale-multimodal-interpretation-visualization-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult2@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.scale-interactive-interpretation-6+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationEndpoint2 {
    pub endpoint_id: String,
    pub protocol: String,
    pub schema_version: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub policy_allowed: bool,
    pub authorized: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub comparable: bool,
    pub negative_result: bool,
    pub semantic_loss_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult2 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub required_protocol: String,
    pub required_schema_version: String,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub endpoints: Vec<InterpretationEndpoint2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretationArtifact6 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretation6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub required_protocol: String,
    pub required_schema_version: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub endpoint_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub interpretation_digest: ContentHash,
    pub artifact: InteractiveInterpretationArtifact6,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationInteroperabilityError {
    #[error("invalid interpretation interoperability request or receipt: {0}")]
    Invalid(String),
    #[error("interpretation interoperability artifact failed: {0}")]
    Artifact(String),
}

fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn valid_digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn interpretation_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "scale".into(),
        consumers: ["research workflow operator".into(), "federation steward".into(), "visualization client".into()].into(),
        behavior: "negotiate and validate versioned multimodal interpretation exchange with semantic-loss and policy witnesses".into(),
        value: "lets independent research sites exchange compatible interpretation artifacts without exporting raw data or hiding semantic loss".into(),
        inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["connect:approved-endpoints".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(r: &EvidenceBackedResult2) -> Result<(), InterpretationInteroperabilityError> {
    if r.schema_version != INPUT_SCHEMA
        || [
            &r.request_id,
            &r.consumer,
            &r.purpose,
            &r.required_protocol,
            &r.required_schema_version,
            &r.semantic_profile,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        || !valid_digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.endpoints.is_empty()
    {
        return Err(InterpretationInteroperabilityError::Invalid(
            "identity, policy, locality, replay, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for e in &r.endpoints {
        if e.endpoint_id.trim().is_empty()
            || !ids.insert(e.endpoint_id.clone())
            || e.protocol.trim().is_empty()
            || e.schema_version.trim().is_empty()
            || e.semantic_profile.trim().is_empty()
            || !valid_digest(&e.artifact_digest)
            || !valid_digest(&e.provenance_digest)
            || e.replay_identity != r.replay_identity
            || !e.local_only
            || !e.aggregate_only
            || !ordered(&e.semantic_loss_order)
        {
            return Err(InterpretationInteroperabilityError::Invalid(
                "endpoint identity, digest, replay, locality, or ordering is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl InteractiveInterpretation6 {
    pub fn validate(&self) -> Result<(), InterpretationInteroperabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.endpoint_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(InterpretationInteroperabilityError::Invalid(
                "identity, locality, endpoints, disposition, or effects are incomplete".into(),
            ));
        }
        for v in [
            &self.endpoint_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.migration_order,
            &self.semantic_loss_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(InterpretationInteroperabilityError::Invalid(
                    "interpretation interoperability ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.endpoint_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.endpoint_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(InterpretationInteroperabilityError::Invalid(
                "endpoint states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.interpretation_digest)
            || self.artifact.content_hash != self.interpretation_digest
            || !self.artifact.provenance_digests.iter().all(valid_digest)
        {
            return Err(InterpretationInteroperabilityError::Artifact(
                "interpretation digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("exchange:permitted-artifacts:"))
        {
            return Err(InterpretationInteroperabilityError::Invalid(
                "exchange effect is outside gateway bounds".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("exchange:permitted-artifacts:{}", self.request_id)]
        {
            return Err(InterpretationInteroperabilityError::Invalid(
                "qualified exchange effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(InterpretationInteroperabilityError::Invalid(
                "non-qualified exchange must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn interoperate_interpretations(
    r: &EvidenceBackedResult2,
) -> Result<InteractiveInterpretation6, InterpretationInteroperabilityError> {
    validate_request(r)?;
    let endpoint_order = r
        .endpoints
        .iter()
        .map(|e| e.endpoint_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut compatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for e in &r.endpoints {
        provenance.insert(e.provenance_digest.clone());
        semantic_loss.extend(
            e.semantic_loss_order
                .iter()
                .map(|x| format!("{}:{}", e.endpoint_id, x)),
        );
        if e.negative_result {
            negative.insert(e.endpoint_id.clone());
        }
        let exact = e.protocol == r.required_protocol
            && e.schema_version == r.required_schema_version
            && e.semantic_profile == r.semantic_profile
            && e.comparable
            && e.policy_allowed
            && e.authorized;
        let additive = e.protocol == r.required_protocol
            && e.semantic_profile == r.semantic_profile
            && e.comparable
            && e.policy_allowed
            && e.authorized;
        if exact
            && matches!(
                e.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            compatible.insert(e.endpoint_id.clone());
        } else if additive
            && matches!(
                e.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            compatible.insert(e.endpoint_id.clone());
            migration.insert(format!("{}:schema-migration", e.endpoint_id));
        } else if !e.policy_allowed
            || !e.authorized
            || !e.local_only
            || !e.aggregate_only
            || e.replay_identity != r.replay_identity
        {
            blocked.insert(e.endpoint_id.clone());
            semantic_loss.insert(format!("{}:policy-locality-or-replay", e.endpoint_id));
        } else {
            unresolved.insert(e.endpoint_id.clone());
            semantic_loss.insert(format!("{}:incompatible-or-uncertain", e.endpoint_id));
        }
    }
    let global_block =
        !r.policy_allowed || !r.protected_closure || !r.raw_data_local || !r.aggregate_only;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || compatible.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(endpoint_order.iter().cloned());
        compatible.clear();
        unresolved.clear();
        semantic_loss.insert("request:global-gate-blocked".into());
    }
    if disposition != "qualified" {
        semantic_loss.insert("request:exchange-closure-not-ready".into());
    }
    let payload = json!({"endpoint_order":endpoint_order,"compatible_order":compatible,"unresolved_order":unresolved,"blocked_order":blocked,"migration_order":migration,"semantic_loss_order":semantic_loss,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| InterpretationInteroperabilityError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = InteractiveInterpretation6 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        required_protocol: r.required_protocol.clone(),
        required_schema_version: r.required_schema_version.clone(),
        semantic_profile: r.semantic_profile.clone(),
        disposition: disposition.into(),
        endpoint_order: strings("endpoint_order"),
        compatible_order: strings("compatible_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        migration_order: strings("migration_order"),
        semantic_loss_order: strings("semantic_loss_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        interpretation_digest: digest.clone(),
        artifact: InteractiveInterpretationArtifact6 {
            artifact_id: format!("scale-interpretation-interchange:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: vec!["raw-data-local-and-aggregate-only".into()],
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("exchange:permitted-artifacts:{}", r.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn interoperate_interpretations_json(
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request: EvidenceBackedResult2 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid interpretation interoperability request: {e}"))?;
    serde_json::to_value(interoperate_interpretations(&request).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_interpretation_interoperability_json(
    value: &serde_json::Value,
) -> Result<InteractiveInterpretation6, String> {
    let receipt: InteractiveInterpretation6 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid interpretation interoperability receipt: {e}"))?;
    receipt.validate().map_err(|e| e.to_string())?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> EvidenceBackedResult2 {
        EvidenceBackedResult2 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "interop-1".into(),
            consumer: "operator".into(),
            purpose: "exchange interpretation".into(),
            required_protocol: "mcp".into(),
            required_schema_version: "1.0".into(),
            semantic_profile: "ome-ngff".into(),
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            endpoints: vec![InterpretationEndpoint2 {
                endpoint_id: "e1".into(),
                protocol: "mcp".into(),
                schema_version: "1.0".into(),
                semantic_profile: "ome-ngff".into(),
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("r"),
                evidence_state: EvidenceState::Supported,
                policy_allowed: true,
                authorized: true,
                local_only: true,
                aggregate_only: true,
                comparable: true,
                negative_result: false,
                semantic_loss_order: vec![],
            }],
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            interpretation_interoperability_gateway_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn exact_exchange_qualifies() {
        assert_eq!(
            interoperate_interpretations(&req()).unwrap().disposition,
            "qualified"
        );
    }
    #[test]
    fn schema_migration_is_explicit() {
        let mut r = req();
        r.endpoints[0].schema_version = "1.1".into();
        assert!(interoperate_interpretations(&r)
            .unwrap()
            .migration_order
            .contains(&"e1:schema-migration".into()));
    }
    #[test]
    fn policy_blocks_exchange() {
        let mut r = req();
        r.policy_allowed = false;
        assert_eq!(
            interoperate_interpretations(&r).unwrap().disposition,
            "blocked"
        );
    }
}
