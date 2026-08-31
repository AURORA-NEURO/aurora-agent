//! Federated continual dependency-composition contract model for Interweave.
//!
//! Atlas feature: `AFA-interweave-P27-F08`.
//!
//! The model resolves typed capability declarations and their dependency closure into a
//! deterministic, digest-only composition receipt. It does not load components, invoke tools,
//! contact a network, or move raw preclinical data.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-interweave-P27-F08";
pub const CONTRACT_VERSION: &str =
    "interweave-federated-continual-dependency-composition-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "InterweaveDependencyContract5@1";
pub const OUTPUT_SCHEMA: &str = "CapabilityComposition6@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDeclaration {
    pub capability_id: String,
    pub version: String,
    pub input_schema: String,
    pub output_schema: String,
    pub semantic_profile: String,
    pub dependency_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub determinism: String,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyCompositionRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub requested_capability_order: Vec<String>,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub declarations: Vec<CapabilityDeclaration>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCompositionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub disposition: String,
    pub requested_capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub incompatible_capability_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub composition_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DependencyCompositionError {
    #[error("invalid federated dependency composition request: {0}")]
    Invalid(String),
    #[error("federated dependency composition artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl CapabilityCompositionReceipt {
    pub fn validate(&self) -> Result<(), DependencyCompositionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.protocol_version.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.requested_capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(DependencyCompositionError::Invalid(
                "composition identity, schema, capabilities, locality, aggregate boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.requested_capability_order,
            &self.selected_capability_order,
            &self.missing_capability_order,
            &self.incompatible_capability_order,
            &self.cycle_order,
            &self.unresolved_capability_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(DependencyCompositionError::Invalid(
                    "composition orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        if self.requested_capability_order.iter().collect::<BTreeSet<_>>().len()
            != self.requested_capability_order.len()
        {
            return Err(DependencyCompositionError::Invalid(
                "requested capabilities are duplicated".into(),
            ));
        }
        let requested = self
            .requested_capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let represented = self
            .selected_capability_order
            .iter()
            .chain(self.missing_capability_order.iter())
            .chain(self.incompatible_capability_order.iter())
            .chain(self.unresolved_capability_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if self
            .selected_capability_order
            .iter()
            .chain(self.missing_capability_order.iter())
            .chain(self.incompatible_capability_order.iter())
            .chain(self.unresolved_capability_order.iter())
            .collect::<Vec<_>>()
            .len()
            != represented.len()
            || !requested.is_subset(&represented)
        {
            return Err(DependencyCompositionError::Invalid(
                "composition outcomes do not cover requested capabilities".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("compose:capability-contract:") && effect != "block:unsafe-release"
        }) {
            return Err(DependencyCompositionError::Invalid(
                "effect is outside the no-execution composition gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| DependencyCompositionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, DependencyCompositionError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| DependencyCompositionError::Artifact(error.to_string()))?,
        )
        .map_err(|error| DependencyCompositionError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "interweave".into(),
        consumers: BTreeSet::from([
            "laboratory automation engineer".into(),
            "component composition reviewer".into(),
            "federation capability operator".into(),
        ]),
        behavior: "resolves typed capability declarations and dependency closure into a deterministic digest-only composition receipt without loading components".into(),
        value: "prevents incompatible, cyclic, unproven, or unauthorized capabilities from being composed across institutions".into(),
        inputs: vec![TypedPort {
            name: "dependency_composition_request".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "capability_composition_receipt".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]),
        permissions: BTreeSet::from([
            "compose:declared-capabilities".into(),
            "exchange:aggregate-capability-manifests".into(),
        ]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "mcp-2025-06-18".into(),
                state: EvidenceState::Supported,
                locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()),
            },
            EvidenceReference {
                source_id: "slsa-provenance-1.2".into(),
                state: EvidenceState::Supported,
                locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "federation capability operator".into(),
            reason: "approve aggregate-only capability composition across institutions".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure(
    request: &DependencyCompositionRequest,
) -> Result<CapabilityCompositionReceipt, DependencyCompositionError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.protocol_version.trim().is_empty()
        || request.requested_capability_order.is_empty()
        || !canonical(&request.requested_capability_order)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DependencyCompositionError::Invalid(
            "request identity, capability order, locality, aggregate boundary, budget, or schema is invalid".into(),
        ));
    }
    let requested = request
        .requested_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if requested.len() != request.requested_capability_order.len() {
        return Err(DependencyCompositionError::Invalid(
            "requested capabilities must be unique".into(),
        ));
    }
    let mut declarations = BTreeMap::new();
    for declaration in &request.declarations {
        if declaration.capability_id.trim().is_empty()
            || !canonical(&declaration.dependency_order)
            || !canonical(&declaration.effect_order)
            || declaration.semantic_profile != request.semantic_profile
            || declaration.input_schema.trim().is_empty()
            || declaration.output_schema.trim().is_empty()
            || declaration.version.trim().is_empty()
            || declaration.determinism != "byte-stable"
            || declaration.artifact_digest == ContentHash::of_bytes(&[])
            || declaration.provenance_digest.is_none()
            || !declaration.permitted
            || !declaration.raw_data_local
            || declarations.insert(declaration.capability_id.clone(), declaration).is_some()
        {
            return Err(DependencyCompositionError::Invalid(
                "capability declarations are duplicated, non-canonical, unbound, unsigned, or non-local".into(),
            ));
        }
    }
    let mut missing = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut state = BTreeMap::<String, u8>::new();
    let mut selected = BTreeSet::new();
    fn visit(
        id: &str,
        declarations: &BTreeMap<String, &CapabilityDeclaration>,
        selected: &mut BTreeSet<String>,
        missing: &mut BTreeSet<String>,
        incompatible: &mut BTreeSet<String>,
        unresolved: &mut BTreeSet<String>,
        cycles: &mut BTreeSet<String>,
        state: &mut BTreeMap<String, u8>,
    ) {
        match state.get(id).copied() {
            Some(1) => {
                cycles.insert(id.into());
                return;
            }
            Some(2) => return,
            _ => {}
        }
        let Some(declaration) = declarations.get(id).copied() else {
            missing.insert(id.into());
            return;
        };
        state.insert(id.into(), 1);
        for dependency in &declaration.dependency_order {
            visit(dependency, declarations, selected, missing, incompatible, unresolved, cycles, state);
        }
        state.insert(id.into(), 2);
        if cycles.contains(id) || declaration.evidence_state == EvidenceState::Contradicted {
            incompatible.insert(id.into());
        } else if matches!(declaration.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative)
            || declaration.uncertainty.iter().any(|_| true)
        {
            unresolved.insert(id.into());
        } else if declaration.omissions.is_empty() {
            selected.insert(id.into());
        } else {
            unresolved.insert(id.into());
        }
    }
    for id in &request.requested_capability_order {
        visit(id, &declarations, &mut selected, &mut missing, &mut incompatible, &mut unresolved, &mut cycles, &mut state);
    }
    let selected_snapshot = selected.clone();
    for id in selected_snapshot {
        if declarations
            .get(&id)
            .map(|declaration| {
                declaration.dependency_order.iter().any(|dependency| {
                    missing.contains(dependency)
                        || cycles.contains(dependency)
                        || incompatible.contains(dependency)
                        || unresolved.contains(dependency)
                })
            })
            .unwrap_or(true)
        {
            selected.remove(&id);
            unresolved.insert(id);
        }
    }
    for declaration in declarations.values() {
        for item in &declaration.omissions {
            omissions.insert(format!("capability:{}:{item}", declaration.capability_id));
        }
        for item in &declaration.uncertainty {
            uncertainty.insert(format!("capability:{}:{item}", declaration.capability_id));
        }
        if declaration.negative_result {
            negative.insert(format!("capability:{}:negative-result", declaration.capability_id));
        }
    }
    for id in &missing {
        omissions.insert(format!("missing-capability:{id}"));
        uncertainty.insert(format!("missing-capability:{id}"));
    }
    for id in &cycles {
        omissions.insert(format!("dependency-cycle:{id}"));
    }
    if !request.policy_allow {
        incompatible.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        incompatible.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        incompatible.insert("workflow:signed-approval-missing".into());
    }
    if !request.federation_approved {
        incompatible.insert("workflow:federation-approval-missing".into());
    }
    for event in &request.adversarial_events {
        incompatible.insert(format!("adversarial:{event}"));
        omissions.insert(format!("workflow:adversarial:{event}"));
    }
    let global_block = !incompatible.is_empty() || !cycles.is_empty() || !request.adversarial_events.is_empty();
    let disposition = if global_block { "blocked" } else if !missing.is_empty() || !unresolved.is_empty() || !uncertainty.is_empty() { "unresolved" } else { "qualified" };
    let selected_capability_order = selected.into_iter().collect::<Vec<_>>();
    let missing_capability_order = missing.into_iter().collect::<Vec<_>>();
    let incompatible_capability_order = incompatible.into_iter().collect::<Vec<_>>();
    let cycle_order = cycles.into_iter().collect::<Vec<_>>();
    let unresolved_capability_order = unresolved.into_iter().collect::<Vec<_>>();
    let composition_payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "protocol_version": request.protocol_version,
        "requested_capability_order": request.requested_capability_order,
        "selected_capability_order": selected_capability_order,
        "missing_capability_order": missing_capability_order,
        "incompatible_capability_order": incompatible_capability_order,
        "cycle_order": cycle_order,
        "unresolved_capability_order": unresolved_capability_order,
        "disposition": disposition,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let composition_digest = ContentHash::of_value(&composition_payload)
        .map_err(|error| DependencyCompositionError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("interweave-capability-composition:{}", request.request_id),
        "application/vnd.aurora.capability-composition+json",
        &composition_payload,
        Vec::<SemanticLoss>::new(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "federated-capability-composition".into(),
            digest: composition_digest.clone(),
        }],
    )
    .map_err(|error| DependencyCompositionError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "qualified" {
        vec![format!("compose:capability-contract:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = CapabilityCompositionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        protocol_version: request.protocol_version.clone(),
        disposition: disposition.into(),
        requested_capability_order: request.requested_capability_order.clone(),
        selected_capability_order,
        missing_capability_order,
        incompatible_capability_order,
        cycle_order,
        unresolved_capability_order,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        composition_digest,
        replay_identity: request
            .declarations
            .first()
            .map(|declaration| declaration.artifact_digest.clone())
            .unwrap_or_else(|| ContentHash::of_bytes(b"empty-composition")),
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"interweave-dependency-contract")
    }

    fn declaration(id: &str, dependencies: Vec<&str>, state: EvidenceState) -> CapabilityDeclaration {
        CapabilityDeclaration {
            capability_id: id.into(), version: "1.0.0".into(), input_schema: "Input@1".into(), output_schema: "Output@1".into(), semantic_profile: "composition:v1".into(), dependency_order: dependencies.into_iter().map(str::to_string).collect(), effect_order: vec!["read-local".into()], determinism: "byte-stable".into(), evidence_state: state, artifact_digest: hash(), provenance_digest: Some(hash()), permitted: true, raw_data_local: true, omissions: Vec::new(), uncertainty: Vec::new(), negative_result: false,
        }
    }

    fn request() -> DependencyCompositionRequest {
        DependencyCompositionRequest {
            request_id: "request:interweave-composition".into(), federation_id: "federation:composition".into(), purpose: "research-workflow".into(), requested_capability_order: vec!["capability-a".into(), "capability-b".into()], semantic_profile: "composition:v1".into(), protocol_version: "mcp:2025-06-18".into(), declarations: vec![declaration("capability-a", vec![], EvidenceState::Supported), declaration("capability-b", vec!["capability-a"], EvidenceState::Proven)], policy_allow: true, protected_closure: true, signed_approval: true, federation_approved: true, raw_data_local: true, aggregate_only: true, budget_units: 10, max_budget_units: 10, adversarial_events: Vec::new(), boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualified_dependency_closure_emits_composition() { let receipt = assure(&request()).unwrap(); assert_eq!(receipt.disposition, "qualified"); assert_eq!(receipt.selected_capability_order, vec!["capability-a", "capability-b"]); assert!(receipt.effect_receipts[0].starts_with("compose:capability-contract:")); }
    #[test]
    fn missing_dependency_is_unresolved() { let mut value = request(); value.declarations[1].dependency_order = vec!["missing".into()]; let receipt = assure(&value).unwrap(); assert_eq!(receipt.disposition, "unresolved"); assert!(receipt.missing_capability_order.contains(&"missing".into())); }
    #[test]
    fn cycle_is_blocked() { let mut value = request(); value.declarations[0].dependency_order = vec!["capability-b".into()]; let receipt = assure(&value).unwrap(); assert_eq!(receipt.disposition, "blocked"); assert!(!receipt.cycle_order.is_empty()); }
    #[test]
    fn unknown_evidence_is_unresolved() { let mut value = request(); value.declarations[0].evidence_state = EvidenceState::Unknown; let receipt = assure(&value).unwrap(); assert_eq!(receipt.disposition, "unresolved"); assert!(!receipt.unresolved_capability_order.is_empty()); }
    #[test]
    fn policy_and_adversarial_inputs_block() { let mut value = request(); value.policy_allow = false; value.adversarial_events = vec!["poisoned-manifest".into()]; let receipt = assure(&value).unwrap(); assert_eq!(receipt.disposition, "blocked"); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test]
    fn manifest_is_a2_and_federated() { let manifest = capability_manifest(); assert_eq!(manifest.autonomy_tier, AutonomyTier::A2); assert!(manifest.effects.contains(&Effect::FederationExport)); }
}
