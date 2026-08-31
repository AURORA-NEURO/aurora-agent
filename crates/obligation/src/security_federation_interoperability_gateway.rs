//! Federated continual security and interoperability gateway (`AFA-obligation-P20-F24`).
//!
//! The gateway is deliberately a metadata-only boundary.  It negotiates versioned capability
//! manifests, verifies policy and replay identity, and emits a deterministic envelope describing
//! what may be exchanged.  Institutions retain raw data locally; a `qualified` result authorizes
//! only the explicitly permitted aggregate artifacts.  Missing, stale, contradictory, negative,
//! and semantically incompatible inputs remain visible in the receipt.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-obligation-P20-F24";
pub const CONTRACT_VERSION: &str = "obligation-security-federation-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "FederationRequest4@1";
pub const OUTPUT_SCHEMA: &str = "FederationEnvelope6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.federation-envelope-6+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationEvidenceState {
    Proven,
    Supported,
    Speculative,
    Contradicted,
    Unknown,
    Negative,
}

/// A signed, policy-scoped capability that a peer is willing to expose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationCapability6 {
    pub capability_id: String,
    pub provider_id: String,
    pub protocol: String,
    pub schema_version: String,
    pub semantic_profile: String,
    pub purpose: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: FederationEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub revoked: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub semantic_loss_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
}

/// A request made by a downstream context compiler or federation operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub consumer: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub required_capability_order: Vec<String>,
    pub required_provider_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub network_available: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u64,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationEnvelope6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub consumer: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub denied_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub provider_order: Vec<String>,
    pub selected_provider_order: Vec<String>,
    pub missing_provider_order: Vec<String>,
    pub protocol_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityFederationGatewayError {
    #[error("invalid security/federation gateway request or receipt: {0}")]
    Invalid(String),
    #[error("security/federation gateway artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> SecurityFederationGatewayError {
    SecurityFederationGatewayError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn digest(value: &Value) -> Result<ContentHash, SecurityFederationGatewayError> {
    ContentHash::of_value(value)
        .map_err(|error| SecurityFederationGatewayError::Artifact(error.to_string()))
}

fn partition(all: &[String], parts: &[&[String]]) -> Result<(), SecurityFederationGatewayError> {
    let expected = all.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != all.len() {
        return Err(invalid("gateway identifiers are not unique"));
    }
    let combined = parts
        .iter()
        .flat_map(|part| part.iter().cloned())
        .collect::<Vec<_>>();
    let actual = combined.iter().cloned().collect::<BTreeSet<_>>();
    if combined.len() != actual.len() || actual != expected {
        return Err(invalid("gateway outcomes do not partition the request"));
    }
    Ok(())
}

pub fn security_federation_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "obligation".into(),
        consumers: [
            "context compiler engineer".into(),
            "federation security steward".into(),
            "institution operator".into(),
        ]
        .into(),
        behavior: "negotiate versioned, policy-bounded federation capabilities into deterministic aggregate-only envelopes while preserving omissions, uncertainty, semantic loss, and negative evidence".into(),
        value: "enables interoperable institutional research exchange without unauthorized data egress or silent semantic drift".into(),
        inputs: vec![TypedPort { name: "federation_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "federation_envelope".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["connect:approved-endpoints".into(), "exchange:permitted-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) },
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "federation security steward".into(), reason: "A2 exchange effects require explicit institutional approval; this capability never grants unrestricted network or raw-data authority".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl FederationEnvelope6 {
    pub fn validate(&self) -> Result<(), SecurityFederationGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.capability_order.is_empty()
            || self.provider_order.is_empty()
            || self.protocol_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(invalid(
                "gateway identity, typed closure, locality, or effect fields are incomplete",
            ));
        }
        for values in [
            &self.capability_order,
            &self.selected_capability_order,
            &self.unresolved_capability_order,
            &self.denied_capability_order,
            &self.missing_capability_order,
            &self.provider_order,
            &self.selected_provider_order,
            &self.missing_provider_order,
            &self.protocol_order,
            &self.migration_order,
            &self.semantic_loss_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.reasons,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("gateway ordering is not canonical"));
            }
        }
        partition(
            &self.capability_order,
            &[
                &self.selected_capability_order,
                &self.unresolved_capability_order,
                &self.denied_capability_order,
                &self.missing_capability_order,
            ],
        )?;
        partition(
            &self.provider_order,
            &[&self.selected_provider_order, &self.missing_provider_order],
        )?;
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.envelope_digest)
            || self.artifact.content_hash != self.envelope_digest
        {
            return Err(SecurityFederationGatewayError::Artifact(
                "gateway digest or artifact hash is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("exchange:permitted-artifacts:")
        }) {
            return Err(invalid("gateway effect is outside the exchange gate"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| SecurityFederationGatewayError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, SecurityFederationGatewayError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|error| SecurityFederationGatewayError::Artifact(error.to_string()))
            .and_then(|value| digest(&value))
    }
}

pub fn negotiate_security_federation(
    request: &FederationRequest4,
    capabilities: &[FederationCapability6],
) -> Result<FederationEnvelope6, SecurityFederationGatewayError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_capability_order.is_empty()
        || !canonical(&request.required_capability_order)
        || !canonical(&request.required_provider_order)
        || !canonical(&request.adversarial_event_order)
        || !valid_digest(&request.replay_identity)
        || request.budget_units == 0
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || capabilities.is_empty()
    {
        return Err(invalid(
            "request identity, required closure, digest, budget, locality, or boundary is invalid",
        ));
    }
    let mut rows = capabilities.to_vec();
    rows.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let capability_order = rows
        .iter()
        .map(|row| row.capability_id.clone())
        .collect::<Vec<_>>();
    if capability_order.windows(2).any(|pair| pair[0] == pair[1])
        || capability_order.iter().any(|id| id.trim().is_empty())
    {
        return Err(invalid(
            "capability identifiers must be unique and non-empty",
        ));
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut providers = BTreeSet::new();
    let mut selected_providers = BTreeSet::new();
    let mut protocols = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for row in &rows {
        let id = row.capability_id.clone();
        providers.insert(row.provider_id.clone());
        protocols.insert(format!("{}@{}", row.protocol, row.schema_version));
        provenance.insert(row.provenance_digest.clone());
        semantic_loss.extend(
            row.semantic_loss_order
                .iter()
                .map(|item| format!("{id}:{item}")),
        );
        omission.extend(row.omission_order.iter().map(|item| format!("{id}:{item}")));
        uncertainty.extend(
            row.uncertainty_order
                .iter()
                .map(|item| format!("{id}:{item}")),
        );
        if row.negative_result || matches!(row.evidence_state, FederationEvidenceState::Negative) {
            negative.insert(format!("{id}:negative-result"));
        }
        if row.revoked
            || !row.permitted
            || !row.signed
            || !row.raw_data_local
            || !row.aggregate_only
            || row.purpose != request.purpose
        {
            denied.insert(id);
            omission.insert(format!(
                "{}:permission-or-locality-denied",
                row.capability_id
            ));
        } else if row.semantic_profile != request.semantic_profile {
            unresolved.insert(id);
            migration.insert(format!(
                "{}:semantic-profile:{}->{}",
                row.capability_id, row.semantic_profile, request.semantic_profile
            ));
            uncertainty.insert(format!("{}:semantic-profile-mismatch", row.capability_id));
        } else if row.replay_identity != request.replay_identity {
            unresolved.insert(id);
            uncertainty.insert(format!("{}:replay-mismatch", row.capability_id));
        } else if !matches!(
            row.evidence_state,
            FederationEvidenceState::Proven | FederationEvidenceState::Supported
        ) {
            unresolved.insert(id);
            uncertainty.insert(format!("{}:evidence-not-supported", row.capability_id));
        } else {
            selected.insert(id);
            selected_providers.insert(row.provider_id.clone());
        }
    }
    for required in &request.required_capability_order {
        if !capability_order.contains(required) {
            missing.insert(required.clone());
            omission.insert(format!("request:missing-capability:{required}"));
        }
    }
    for required in &request.required_provider_order {
        if !selected_providers.contains(required) {
            omission.insert(format!("request:missing-provider:{required}"));
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.network_available
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty();
    if global_block {
        denied.extend(capability_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omission.insert("request:security-policy-protected-closure-or-network-blocked".into());
    }
    uncertainty.extend(
        request
            .adversarial_event_order
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let missing_order = missing.iter().cloned().collect::<Vec<_>>();
    let provider_order = providers
        .into_iter()
        .chain(request.required_provider_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_provider_order = selected_providers.into_iter().collect::<Vec<_>>();
    let missing_provider_order = request
        .required_provider_order
        .iter()
        .filter(|id| !selected_provider_order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    if global_block || selected_order.is_empty() && unresolved_order.is_empty() {
        omission.insert("request:federation-closure-not-ready".into());
    }
    let disposition = if global_block || selected_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !missing_order.is_empty()
        || !missing_provider_order.is_empty()
        || !denied_order.is_empty()
        || !unresolved_order.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    let reasons = if disposition == "qualified" {
        vec!["all-required-capabilities-qualified".to_string()]
    } else {
        vec![
            format!("disposition:{disposition}"),
            "partial-and-negative-evidence-retained".into(),
        ]
    };
    let omission_order = omission.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "consumer": request.consumer,
        "purpose": request.purpose,
        "scope": request.scope,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "capability_order": capability_order,
        "selected_capability_order": selected_order,
        "unresolved_capability_order": unresolved_order,
        "denied_capability_order": denied_order,
        "missing_capability_order": missing_order,
        "provider_order": provider_order,
        "selected_provider_order": selected_provider_order,
        "missing_provider_order": missing_provider_order,
        "protocol_order": protocols.into_iter().collect::<Vec<_>>(),
        "migration_order": migration.into_iter().collect::<Vec<_>>(),
        "semantic_loss_order": semantic_loss.into_iter().collect::<Vec<_>>(),
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative.into_iter().collect::<Vec<_>>(),
        "adversarial_event_order": request.adversarial_event_order,
        "reasons": reasons,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let envelope_digest = digest(&payload)?;
    let semantic_loss_values = payload["semantic_loss_order"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| {
            value.as_str().map(|field| SemanticLoss {
                field: field.into(),
                reason: "peer-declared semantic loss or migration boundary".into(),
                severity: bioprism_foundation::LossSeverity::Unknown,
            })
        })
        .collect::<Vec<_>>();
    let provenance_links = provenance
        .into_iter()
        .enumerate()
        .map(|(index, digest)| ProvenanceLink {
            source_id: format!("federation-capability:{index}"),
            relation: "declared-by-peer".into(),
            digest,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        artifact_id: format!("federation-envelope-6:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: envelope_digest.clone(),
        semantic_loss: semantic_loss_values,
        provenance: provenance_links,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let mut output = payload;
    output["envelope_digest"] = json!(envelope_digest);
    output["artifact"] = serde_json::to_value(artifact)
        .map_err(|error| SecurityFederationGatewayError::Artifact(error.to_string()))?;
    output["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let envelope: FederationEnvelope6 = serde_json::from_value(output)
        .map_err(|error| SecurityFederationGatewayError::Artifact(error.to_string()))?;
    envelope.validate()?;
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn capability(id: &str) -> FederationCapability6 {
        FederationCapability6 {
            capability_id: id.into(),
            provider_id: format!("site:{id}"),
            protocol: "wes".into(),
            schema_version: "1.0".into(),
            semantic_profile: "prov-v1".into(),
            purpose: "benchmark".into(),
            artifact_digest: h(id),
            provenance_digest: h("prov"),
            replay_identity: h("replay"),
            evidence_state: FederationEvidenceState::Supported,
            signed: true,
            permitted: true,
            revoked: false,
            raw_data_local: true,
            aggregate_only: true,
            semantic_loss_order: vec![],
            omission_order: vec![],
            uncertainty_order: vec![],
            negative_result: false,
        }
    }
    fn request() -> FederationRequest4 {
        FederationRequest4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req-1".into(),
            federation_id: "fed-1".into(),
            consumer: "context compiler engineer".into(),
            purpose: "benchmark".into(),
            scope: "study:preclinical".into(),
            semantic_profile: "prov-v1".into(),
            required_capability_order: vec!["cap-a".into(), "cap-b".into()],
            required_provider_order: vec!["site:cap-a".into(), "site:cap-b".into()],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            network_available: true,
            raw_data_local: true,
            aggregate_only: true,
            budget_units: 10,
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            security_federation_interoperability_gateway_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn qualified_exchange_is_explicit() {
        let output =
            negotiate_security_federation(&request(), &[capability("cap-b"), capability("cap-a")])
                .unwrap();
        assert_eq!(output.disposition, "qualified");
        assert!(output.effect_receipts[0].starts_with("exchange:permitted-artifacts:"));
    }
    #[test]
    fn schema_drift_is_unresolved_not_silent() {
        let mut row = capability("cap-a");
        row.semantic_profile = "other-v2".into();
        let output =
            negotiate_security_federation(&request(), &[row, capability("cap-b")]).unwrap();
        assert_eq!(output.disposition, "unresolved");
        assert!(!output.migration_order.is_empty());
    }
    #[test]
    fn policy_blocks_and_retains_denied_state() {
        let mut query = request();
        query.policy_allow = false;
        let output =
            negotiate_security_federation(&query, &[capability("cap-a"), capability("cap-b")])
                .unwrap();
        assert_eq!(output.disposition, "blocked");
        assert_eq!(output.effect_receipts, vec!["block:unsafe-release"]);
        assert_eq!(output.denied_capability_order.len(), 2);
    }
    #[test]
    fn negative_evidence_is_first_class() {
        let mut row = capability("cap-a");
        row.negative_result = true;
        let output =
            negotiate_security_federation(&request(), &[row, capability("cap-b")]).unwrap();
        assert_eq!(
            output.negative_evidence_order,
            vec!["cap-a:negative-result"]
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let left =
            negotiate_security_federation(&request(), &[capability("cap-b"), capability("cap-a")])
                .unwrap();
        let right =
            negotiate_security_federation(&request(), &[capability("cap-a"), capability("cap-b")])
                .unwrap();
        assert_eq!(left.envelope_digest, right.envelope_digest);
    }
}
