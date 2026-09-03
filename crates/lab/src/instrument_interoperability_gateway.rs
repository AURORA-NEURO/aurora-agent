//! Prospective high-throughput laboratory-integration interoperability gateway.
//!
//! Atlas feature: `AFA-lab-P11-F23`.
//!
//! The gateway negotiates typed instrument endpoint capabilities and preflight metadata. It does
//! not connect to hardware. Every endpoint, capability, interlock, evidence, replay, and policy
//! decision is retained in a deterministic receipt that instrument operators can audit.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P11-F23";
pub const CONTRACT_VERSION: &str =
    "lab-prospective-laboratory-integration-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "LaboratoryIntegrationRequest4@1";
pub const OUTPUT_SCHEMA: &str = "LaboratoryIntegrationReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.lab-laboratory-integration-receipt-7+json";
pub const MAX_ENDPOINTS: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentEndpoint5 {
    pub endpoint_id: String,
    pub instrument_id: String,
    pub protocol_version: String,
    pub capability_order: Vec<String>,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed_identity: bool,
    pub policy_allowed: bool,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub healthy: bool,
    pub stale: bool,
    pub revoked: bool,
    pub interlock_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaboratoryIntegrationRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_endpoint_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub required_interlock_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_endpoint_count: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub endpoints: Vec<InstrumentEndpoint5>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaboratoryIntegrationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaboratoryIntegrationReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: LaboratoryIntegrationDisposition,
    pub ranked_endpoint_order: Vec<String>,
    pub selected_endpoint_order: Vec<String>,
    pub unresolved_endpoint_order: Vec<String>,
    pub blocked_endpoint_order: Vec<String>,
    pub missing_endpoint_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub blocked_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub interlock_order: Vec<String>,
    pub selected_interlock_order: Vec<String>,
    pub missing_interlock_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub integration_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LaboratoryIntegrationError {
    #[error("invalid laboratory integration request or receipt: {0}")]
    Invalid(String),
    #[error("laboratory integration artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> LaboratoryIntegrationError {
    LaboratoryIntegrationError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), LaboratoryIntegrationError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flat = Vec::new();
    for part in parts {
        if !canonical(part) || part.iter().any(|id| !expected.contains(id)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flat.extend_from_slice(part);
    }
    if flat.len() != expected.len() || flat.iter().collect::<BTreeSet<_>>().len() != flat.len() {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl LaboratoryIntegrationReceipt7 {
    pub fn validate(&self) -> Result<(), LaboratoryIntegrationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A2
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_endpoint_order.is_empty()
            || self.capability_order.is_empty()
            || self.interlock_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("laboratory integration identity, closure, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_endpoint_order,
            &self.selected_endpoint_order,
            &self.unresolved_endpoint_order,
            &self.blocked_endpoint_order,
            &self.missing_endpoint_order,
            &self.capability_order,
            &self.selected_capability_order,
            &self.unresolved_capability_order,
            &self.blocked_capability_order,
            &self.missing_capability_order,
            &self.interlock_order,
            &self.selected_interlock_order,
            &self.missing_interlock_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("laboratory integration ordering is not canonical"));
            }
        }
        partition(
            &self.ranked_endpoint_order,
            &[
                &self.selected_endpoint_order,
                &self.unresolved_endpoint_order,
                &self.blocked_endpoint_order,
                &self.missing_endpoint_order,
            ],
            "endpoint",
        )?;
        partition(
            &self.capability_order,
            &[
                &self.selected_capability_order,
                &self.unresolved_capability_order,
                &self.blocked_capability_order,
                &self.missing_capability_order,
            ],
            "capability",
        )?;
        if self
            .selected_interlock_order
            .iter()
            .any(|id| !self.interlock_order.contains(id))
            || self
                .missing_interlock_order
                .iter()
                .any(|id| !self.interlock_order.contains(id))
        {
            return Err(invalid("interlock state is outside the universe"));
        }
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.integration_digest)
            || self.artifact.content_hash != self.integration_digest
        {
            return Err(invalid("laboratory integration digest is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("manage:local-instrument-capability:")
                && !effect.starts_with("exchange:instrument-summary:")
                && effect != "block:unsafe-release"
        }) {
            return Err(invalid(
                "instrument effect is outside the interoperability gate",
            ));
        }
        if self.disposition == LaboratoryIntegrationDisposition::Qualified
            && self.effect_receipts
                != vec![
                    format!("manage:local-instrument-capability:{}", self.request_id),
                    format!("exchange:instrument-summary:{}", self.request_id),
                ]
        {
            return Err(invalid(
                "qualified laboratory integration effects are invalid",
            ));
        }
        if self.disposition != LaboratoryIntegrationDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified laboratory integration must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LaboratoryIntegrationError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, LaboratoryIntegrationError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))
            })
    }
}

pub fn laboratory_integration_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"lab".into(),consumers:["instrument operator".into(),"laboratory integration engineer".into(),"federation verifier".into()].into(),behavior:"negotiates typed institution-local instrument endpoint capabilities and emits an auditable interoperability receipt without contacting hardware".into(),value:"prevents incompatible, stale, revoked, or unauthorized instrument endpoints from entering a high-throughput research workflow".into(),inputs:vec![TypedPort{name:"laboratory_integration_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"laboratory_integration_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::FederationExport].into(),permissions:["negotiate:instrument-capability".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"mcp-2025-06-18".into(),state:EvidenceState::Supported,locator:Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into())},EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:vec![AuthorityRequirement{role:"instrument operator".into(),reason:"endpoint capability exchange is a governed laboratory integration boundary".into()}],autonomy_tier:AutonomyTier::A2,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

fn validate_request(
    request: &LaboratoryIntegrationRequest4,
) -> Result<(), LaboratoryIntegrationError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_endpoint_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.required_interlock_order.is_empty()
        || !canonical(&request.required_endpoint_order)
        || !canonical(&request.required_capability_order)
        || !canonical(&request.required_interlock_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_endpoint_count == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.endpoints.is_empty()
        || request.endpoints.len() > MAX_ENDPOINTS
        || !digest_valid(&request.replay_identity)
    {
        return Err(invalid("laboratory integration identity, closure, capacity, replay, boundary, or bounds are invalid"));
    }
    let mut ids = BTreeSet::new();
    for endpoint in &request.endpoints {
        if endpoint.endpoint_id.trim().is_empty()
            || endpoint.instrument_id.trim().is_empty()
            || endpoint.protocol_version.trim().is_empty()
            || endpoint.semantic_profile != request.semantic_profile
            || endpoint.capability_order.is_empty()
            || !canonical(&endpoint.capability_order)
            || !canonical(&endpoint.interlock_order)
            || !canonical(&endpoint.omission_order)
            || !canonical(&endpoint.uncertainty_order)
            || !digest_valid(&endpoint.provenance_digest)
            || !digest_valid(&endpoint.replay_identity)
            || !ids.insert(endpoint.endpoint_id.clone())
        {
            return Err(invalid(
                "instrument endpoint identity, profile, capability, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn negotiate_laboratory_integration(
    request: &LaboratoryIntegrationRequest4,
) -> Result<LaboratoryIntegrationReceipt7, LaboratoryIntegrationError> {
    validate_request(request)?;
    let mut rows = request.endpoints.clone();
    rows.sort_by(|a, b| {
        (a.evidence_state, a.stale, a.endpoint_id.as_str()).cmp(&(
            b.evidence_state,
            b.stale,
            b.endpoint_id.as_str(),
        ))
    });
    let ranked = rows
        .iter()
        .map(|x| x.endpoint_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_endpoint_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    for row in &rows {
        omissions.extend(row.omission_order.iter().cloned());
        uncertainty.extend(row.uncertainty_order.iter().cloned());
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.endpoint_id.clone());
        }
        let hard = row.revoked
            || !row.signed_identity
            || !row.policy_allowed
            || !row.federation_allowed
            || !row.raw_data_local
            || !row.aggregate_only
            || !row.healthy;
        let soft = row.stale
            || row.replay_identity != request.replay_identity
            || !row.omission_order.is_empty()
            || !row.uncertainty_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.endpoint_id.clone());
        } else if soft {
            unresolved.insert(row.endpoint_id.clone());
        } else {
            selected.insert(row.endpoint_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omissions.insert(format!("missing required endpoint: {id}"));
    }
    let mut capabilities = request
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    capabilities.extend(rows.iter().flat_map(|x| x.capability_order.iter().cloned()));
    let selected_capabilities = capabilities
        .iter()
        .filter(|id| {
            rows.iter()
                .any(|x| selected.contains(&x.endpoint_id) && x.capability_order.contains(id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let unresolved_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && rows
                    .iter()
                    .any(|x| unresolved.contains(&x.endpoint_id) && x.capability_order.contains(id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let blocked_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && !unresolved_capabilities.contains(*id)
                && rows
                    .iter()
                    .any(|x| blocked.contains(&x.endpoint_id) && x.capability_order.contains(id))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_capabilities = capabilities
        .iter()
        .filter(|id| {
            !selected_capabilities.contains(*id)
                && !unresolved_capabilities.contains(*id)
                && !blocked_capabilities.contains(*id)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let interlocks = request
        .required_interlock_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected_interlocks = rows
        .iter()
        .filter(|x| selected.contains(&x.endpoint_id))
        .flat_map(|x| x.interlock_order.iter().cloned())
        .filter(|id| interlocks.contains(id))
        .collect::<BTreeSet<_>>();
    let missing_interlocks = interlocks
        .difference(&selected_interlocks)
        .cloned()
        .collect::<BTreeSet<_>>();
    let open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.federation_allow
        && request.raw_data_local
        && request.aggregate_only
        && request.adversarial_event_order.is_empty();
    let disposition = if !open
        || !blocked.is_empty()
        || !missing.is_empty()
        || !blocked_capabilities.is_empty()
        || !missing_capabilities.is_empty()
        || !missing_interlocks.is_empty()
        || selected.len() < request.minimum_endpoint_count as usize
    {
        LaboratoryIntegrationDisposition::Blocked
    } else if !unresolved.is_empty() || !unresolved_capabilities.is_empty() {
        LaboratoryIntegrationDisposition::Unresolved
    } else {
        LaboratoryIntegrationDisposition::Qualified
    };
    let effects = if disposition == LaboratoryIntegrationDisposition::Qualified {
        vec![
            format!("manage:local-instrument-capability:{}", request.request_id),
            format!("exchange:instrument-summary:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons=vec![match disposition{LaboratoryIntegrationDisposition::Qualified=>"all endpoint, capability, interlock, policy, replay, provenance, and locality gates passed".into(),LaboratoryIntegrationDisposition::Unresolved=>"stale, unknown, omitted, or uncertain endpoint evidence remains unresolved".into(),LaboratoryIntegrationDisposition::Blocked=>"policy, closure, capability, interlock, authorization, health, or adversarial gates blocked integration".into()}];
    let provenance_digest = ContentHash::of_bytes(
        rows.iter()
            .map(|x| x.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"ranked_endpoint_order":ranked,"selected_endpoint_order":selected,"unresolved_endpoint_order":unresolved,"blocked_endpoint_order":blocked,"missing_endpoint_order":missing,"capability_order":capabilities,"selected_capability_order":selected_capabilities,"unresolved_capability_order":unresolved_capabilities,"blocked_capability_order":blocked_capabilities,"missing_capability_order":missing_capabilities,"interlock_order":interlocks,"selected_interlock_order":selected_interlocks,"missing_interlock_order":missing_interlocks,"omission_order":omissions,"uncertainty_order":uncertainty,"contradiction_order":contradiction,"adversarial_event_order":request.adversarial_event_order,"replay_identity":request.replay_identity,"provenance_digest":provenance_digest,"reasons":reasons,"effect_receipts":effects,"raw_data_local":request.raw_data_local,"aggregate_only":request.aggregate_only,"autonomy_tier":AutonomyTier::A2,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("laboratory-integration:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))?;
    let integration_digest = artifact.content_hash.clone();
    let receipt = LaboratoryIntegrationReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        ranked_endpoint_order: ranked,
        selected_endpoint_order: selected.into_iter().collect(),
        unresolved_endpoint_order: unresolved.into_iter().collect(),
        blocked_endpoint_order: blocked.into_iter().collect(),
        missing_endpoint_order: missing.into_iter().collect(),
        capability_order: capabilities.into_iter().collect(),
        selected_capability_order: selected_capabilities.into_iter().collect(),
        unresolved_capability_order: unresolved_capabilities.into_iter().collect(),
        blocked_capability_order: blocked_capabilities.into_iter().collect(),
        missing_capability_order: missing_capabilities.into_iter().collect(),
        interlock_order: interlocks.into_iter().collect(),
        selected_interlock_order: selected_interlocks.into_iter().collect(),
        missing_interlock_order: missing_interlocks.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest,
        reasons,
        integration_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        autonomy_tier: AutonomyTier::A2,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn endpoint(id: &str, state: EvidenceState) -> InstrumentEndpoint5 {
        InstrumentEndpoint5 {
            endpoint_id: id.into(),
            instrument_id: format!("instrument:{id}"),
            protocol_version: "1.0".into(),
            capability_order: vec![format!("capability:{id}")],
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            provenance_digest: hash(id),
            replay_identity: hash("replay"),
            signed_identity: true,
            policy_allowed: true,
            federation_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            healthy: true,
            stale: false,
            revoked: false,
            interlock_order: vec!["interlock:door".into()],
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn request(items: Vec<InstrumentEndpoint5>) -> LaboratoryIntegrationRequest4 {
        LaboratoryIntegrationRequest4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "lab:1".into(),
            workflow_id: "workflow:1".into(),
            requester: "operator".into(),
            purpose: "integration".into(),
            semantic_profile: "imaging-omics".into(),
            required_endpoint_order: vec!["endpoint:1".into()],
            required_capability_order: vec!["capability:endpoint:1".into()],
            required_interlock_order: vec!["interlock:door".into()],
            replay_identity: hash("replay"),
            minimum_endpoint_count: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            endpoints: items,
        }
    }
    #[test]
    fn qualified_is_deterministic() {
        let r = negotiate_laboratory_integration(&request(vec![endpoint(
            "endpoint:1",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.disposition, LaboratoryIntegrationDisposition::Qualified);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = negotiate_laboratory_integration(&request(vec![endpoint(
            "endpoint:1",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(r.disposition, LaboratoryIntegrationDisposition::Unresolved);
    }
    #[test]
    fn contradiction_is_blocked() {
        let r = negotiate_laboratory_integration(&request(vec![endpoint(
            "endpoint:1",
            EvidenceState::Contradicted,
        )]))
        .unwrap();
        assert_eq!(r.disposition, LaboratoryIntegrationDisposition::Blocked);
    }
    #[test]
    fn missing_endpoint_is_blocked() {
        let r = negotiate_laboratory_integration(&request(vec![endpoint(
            "endpoint:other",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.disposition, LaboratoryIntegrationDisposition::Blocked);
    }
    #[test]
    fn adversarial_event_blocks() {
        let mut q = request(vec![endpoint("endpoint:1", EvidenceState::Supported)]);
        q.adversarial_event_order = vec!["tampered-endpoint".into()];
        assert_eq!(
            negotiate_laboratory_integration(&q).unwrap().disposition,
            LaboratoryIntegrationDisposition::Blocked
        );
    }
    #[test]
    fn manifest_is_valid() {
        laboratory_integration_manifest().validate().unwrap();
    }
}
