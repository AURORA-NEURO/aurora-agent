//! Federated continual interpretation and visualisation interoperability.
//!
//! Atlas feature `AFA-influence-P14-F24` turns the sound influence methods in this crate into an
//! independently deployable, digest-only federation boundary.  The gateway owns no remote
//! transport and never moves a factor table: a caller supplies a local, typed region and peer
//! capability envelopes, and receives a deterministic interpretation projection plus receipts for
//! every omission, migration, policy decision, and influence method that was attempted.
//!
//! The implementation is intentionally stricter than a generic protocol adapter.  A result is
//! not qualified unless the requested peer quorum, pinned contract, evidence closure, authority,
//! and every local influence bound are present.  Missing tables are reported as unknown, not as a
//! vacuous bound, and negative or contradictory evidence remains visible in the projection.

use crate::{InfluenceAnalyzer, InfluenceEstimate, Perturbation};
use bioprism_backends::{QueryRegion, RegionFactor};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-influence-P14-F24";
pub const CONTRACT_VERSION: &str = "influence-federated-continual-interpretation-gateway/1.0";
pub const TARGET_CONTRACT_VERSION: &str = "1.0.0";
pub const COMPATIBLE_CONTRACT_VERSION: &str = "0.9.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult4@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation6@1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationVariable {
    pub name: String,
    pub cardinality: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationFactor {
    pub factor_id: String,
    pub scope: Vec<String>,
    #[serde(default)]
    pub table: Option<Vec<f64>>,
    pub modality: String,
    pub evidence_state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    #[serde(default)]
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationClaim {
    pub claim_id: String,
    pub modality: String,
    pub statement: String,
    pub supporting_evidence: Vec<ContentHash>,
    pub uncertainty: String,
    #[serde(default)]
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerCapability {
    pub endpoint_id: String,
    pub contract_version: String,
    #[serde(default)]
    pub supported_contract_versions: Vec<String>,
    pub capabilities: Vec<String>,
    pub capability_digest: ContentHash,
    pub permitted_export: bool,
    pub healthy: bool,
    pub signed_capability: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

/// Input contract for a continual interpretation run.  Factor tables are local-only payloads;
/// peer rows carry hashes and capability metadata, never raw measurements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBackedResult4 {
    pub result_id: String,
    pub consumer: String,
    pub scope: String,
    pub institution_id: String,
    pub federation_id: String,
    pub epoch: u64,
    #[serde(default)]
    pub previous_receipt: Option<ContentHash>,
    pub target_contract_version: String,
    pub required_capabilities: Vec<String>,
    pub quorum: usize,
    pub peer_capabilities: Vec<PeerCapability>,
    pub evidence_digests: Vec<ContentHash>,
    pub required_modalities: Vec<String>,
    pub variables: Vec<InterpretationVariable>,
    pub factors: Vec<InterpretationFactor>,
    pub free_variables: Vec<String>,
    pub claims: Vec<InterpretationClaim>,
    /// `removal` replaces a factor by an all-ones potential; `multiplicative_range` uses the
    /// caller-declared relative tolerance.  No untyped perturbation class is accepted.
    pub perturbation_class: String,
    #[serde(default)]
    pub relative_tolerance: Option<f64>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodObservation {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declined: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluenceObservation {
    pub factor_id: String,
    pub modality: String,
    pub evidence_state: EvidenceState,
    pub estimate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_method: Option<String>,
    pub attempted: Vec<MethodObservation>,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationView {
    pub claim_id: String,
    pub modality: String,
    pub statement: String,
    pub bound_factor_order: Vec<String>,
    pub uncertainty: String,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayDisposition {
    Accepted,
    Migrated,
    ApprovalRequired,
    Blocked,
    Incompatible,
    Unknown,
}

/// Versioned output consumed by a researcher workbench, HTTP/event gateway, SDK, or MCP tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractiveInterpretation {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub result_id: String,
    pub consumer: String,
    pub scope: String,
    pub institution_id: String,
    pub federation_id: String,
    pub epoch: u64,
    pub negotiated_version: String,
    pub disposition: GatewayDisposition,
    pub verdict: String,
    pub peer_order: Vec<String>,
    pub accepted_peer_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub interpretation_order: Vec<InterpretationView>,
    pub influence_order: Vec<InfluenceObservation>,
    pub covered_modalities: Vec<String>,
    pub omitted_modalities: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub checks: Vec<String>,
    pub passed_checks: Vec<String>,
    pub counterexamples: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub interpretation_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedInterpretationError {
    #[error("invalid federated interpretation request: {0}")]
    InvalidRequest(String),
    #[error("influence analysis failed: {0}")]
    Influence(String),
    #[error("federated interpretation artifact failed: {0}")]
    Artifact(String),
    #[error("federated interpretation serialization failed: {0}")]
    Serialization(String),
}

fn invalid(message: impl Into<String>) -> FederatedInterpretationError {
    FederatedInterpretationError::InvalidRequest(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn nonempty(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| !value.trim().is_empty())
}

impl InteractiveInterpretation {
    pub fn validate(&self) -> Result<(), FederatedInterpretationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.result_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.negotiated_version.trim().is_empty()
            || !matches!(
                self.disposition,
                GatewayDisposition::Accepted
                    | GatewayDisposition::Migrated
                    | GatewayDisposition::ApprovalRequired
                    | GatewayDisposition::Blocked
                    | GatewayDisposition::Incompatible
                    | GatewayDisposition::Unknown
            )
            || !matches!(
                self.verdict.as_str(),
                "qualified" | "conditional" | "unknown" | "blocked"
            )
            || self.peer_order.is_empty()
            || self.claim_order.is_empty()
            || self.interpretation_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "identity, peers, claims, checks, locality, verdict, or effects are incomplete",
            ));
        }
        if !canonical(&self.peer_order)
            || !canonical(&self.accepted_peer_order)
            || !canonical(&self.capability_order)
            || !canonical(&self.claim_order)
            || !canonical(&self.covered_modalities)
            || !canonical(&self.omitted_modalities)
            || !canonical(&self.omissions)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.checks)
            || !canonical(&self.passed_checks)
            || !canonical(&self.counterexamples)
            || !canonical(&self.effect_receipts)
        {
            return Err(invalid("federated interpretation output is not canonical"));
        }
        if self
            .accepted_peer_order
            .iter()
            .any(|id| !self.peer_order.contains(id))
            || self
                .interpretation_order
                .windows(2)
                .any(|pair| pair[0].claim_id >= pair[1].claim_id)
            || self.interpretation_order.iter().any(|view| {
                view.claim_id.trim().is_empty()
                    || view.modality.trim().is_empty()
                    || view.statement.trim().is_empty()
                    || !canonical(&view.bound_factor_order)
                    || !canonical(&view.negative_evidence)
            })
        {
            return Err(invalid(
                "interpretation ordering or claim references are invalid",
            ));
        }
        for observation in &self.influence_order {
            if observation.factor_id.trim().is_empty()
                || !digest(&observation.evidence_digest)
                || !digest(&observation.provenance_digest)
                || !canonical(
                    &observation
                        .attempted
                        .iter()
                        .map(|entry| entry.method.clone())
                        .collect::<Vec<_>>(),
                )
            {
                return Err(invalid(
                    "influence observation is incomplete or not canonical",
                ));
            }
            if let Some(bound) = observation.bound {
                if !bound.is_finite() || !(0.0..=1.0).contains(&bound) {
                    return Err(invalid("influence observation bound is outside [0,1]"));
                }
            }
        }
        for value in [
            &self.replay_identity,
            &self.federation_digest,
            &self.interpretation_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("federated interpretation digest is invalid"));
            }
        }
        if self.verdict == "qualified"
            && self.disposition != GatewayDisposition::Accepted
            && self.disposition != GatewayDisposition::Migrated
        {
            return Err(invalid(
                "only accepted or migrated integrations can be qualified",
            ));
        }
        if self.verdict != "qualified"
            && self
                .effect_receipts
                .iter()
                .all(|effect| effect == "interpret:qualified")
        {
            return Err(invalid(
                "non-qualified interpretation cannot emit a qualified effect",
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedInterpretationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedInterpretationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedInterpretationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedInterpretationError::Serialization(error.to_string()))
    }
}

pub fn federated_continual_interpretation_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "influence".into(),
        consumers: [
            "formal methods researcher".into(),
            "research interpretation workbench".into(),
            "federated consortium operator".into(),
        ]
        .into(),
        behavior: "negotiates a pinned federated continual interpretation protocol and emits deterministic omission-aware influence views from institution-local typed regions".into(),
        value: "makes influence bounds and multimodal interpretations independently checkable across policy-separated institutions without moving raw preclinical data".into(),
        inputs: vec![TypedPort {
            name: "evidence_backed_result".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "interactive_interpretation".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: ["connect:approved-endpoints".into(), "exchange:permitted-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "mcp-2025-06-18".into(),
                state: EvidenceState::Supported,
                locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()),
            },
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated research authority".into(),
            reason: "A2 federation export and continual interpretation require institution-approved scope".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn build_region(
    request: &EvidenceBackedResult4,
) -> Result<QueryRegion, FederatedInterpretationError> {
    let mut builder =
        QueryRegion::builder(format!("federated-interpretation:{}", request.result_id));
    for variable in &request.variables {
        builder = builder.variable(variable.name.clone(), variable.cardinality);
    }
    for factor in &request.factors {
        let region_factor = match &factor.table {
            Some(table) => RegionFactor::with_table(
                factor.factor_id.clone(),
                factor.scope.clone(),
                table.clone(),
            ),
            None => RegionFactor::structural(factor.factor_id.clone(), factor.scope.clone()),
        };
        builder = builder.factor(region_factor);
    }
    for variable in &request.free_variables {
        builder = builder.free(variable.clone());
    }
    builder
        .assumption("factor tables are institution-local and peer exchange is digest-only")
        .build()
        .map_err(|error| invalid(format!("typed region rejected: {error}")))
}

fn validate_request(request: &EvidenceBackedResult4) -> Result<(), FederatedInterpretationError> {
    if request.result_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.target_contract_version.trim().is_empty()
        || request.epoch == 0
        || request.quorum == 0
        || request.quorum > request.peer_capabilities.len()
        || !nonempty(&request.required_capabilities)
        || !nonempty(&request.required_modalities)
        || request.variables.is_empty()
        || request.factors.is_empty()
        || request.free_variables.is_empty()
        || request.claims.is_empty()
        || request.evidence_digests.is_empty()
        || !matches!(
            request.perturbation_class.as_str(),
            "removal" | "multiplicative_range"
        )
        || request.perturbation_class == "multiplicative_range"
            && request.relative_tolerance.is_none()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
    {
        return Err(invalid("identity, scope, contract, quorum, region, evidence, perturbation, locality, replay, or boundary is invalid"));
    }
    if request.target_contract_version != TARGET_CONTRACT_VERSION {
        return Err(invalid(
            "target contract version is outside the pinned gateway window",
        ));
    }
    if let Some(tolerance) = request.relative_tolerance {
        if !tolerance.is_finite() || !(0.0..1.0).contains(&tolerance) {
            return Err(invalid("relative tolerance must be finite and in [0,1)"));
        }
    }
    if !canonical(&request.required_capabilities)
        || !canonical(&request.required_modalities)
        || !canonical(&request.free_variables)
    {
        return Err(invalid(
            "required capabilities, modalities, and free variables must be canonical",
        ));
    }
    let mut variable_ids = BTreeSet::new();
    for variable in &request.variables {
        if variable.name.trim().is_empty()
            || variable.cardinality == 0
            || !variable_ids.insert(variable.name.clone())
        {
            return Err(invalid(
                "variables must have unique non-empty names and positive cardinalities",
            ));
        }
    }
    let evidence = request.evidence_digests.iter().collect::<BTreeSet<_>>();
    if request.evidence_digests.iter().any(|value| !digest(value)) {
        return Err(invalid("evidence digest is invalid"));
    }
    let mut factor_ids = BTreeSet::new();
    for factor in &request.factors {
        if factor.factor_id.trim().is_empty()
            || factor.scope.is_empty()
            || factor.modality.trim().is_empty()
            || !factor_ids.insert(factor.factor_id.clone())
            || !digest(&factor.evidence_digest)
            || !digest(&factor.provenance_digest)
            || factor.scope.iter().any(|name| !variable_ids.contains(name))
            || !evidence.contains(&factor.evidence_digest)
        {
            return Err(invalid(
                "factors must have unique typed scopes and evidence coverage",
            ));
        }
        if let Some(table) = &factor.table {
            if table.iter().any(|value| !value.is_finite() || *value < 0.0) {
                return Err(invalid(
                    "factor tables must contain finite non-negative values",
                ));
            }
        }
    }
    if request
        .free_variables
        .iter()
        .any(|name| !variable_ids.contains(name))
    {
        return Err(invalid("free variables must be declared variables"));
    }
    let mut claim_ids = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || claim.modality.trim().is_empty()
            || claim.statement.trim().is_empty()
            || claim.uncertainty.trim().is_empty()
            || claim.supporting_evidence.is_empty()
            || !claim_ids.insert(claim.claim_id.clone())
            || claim
                .supporting_evidence
                .iter()
                .any(|value| !evidence.contains(value))
        {
            return Err(invalid(
                "claims must be unique, typed, uncertain, and evidence-backed",
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peer_capabilities {
        if peer.endpoint_id.trim().is_empty()
            || peer.contract_version.trim().is_empty()
            || !nonempty(&peer.capabilities)
            || !digest(&peer.capability_digest)
            || !peer.raw_data_local
            || peer.boundary != PRECLINICAL_BOUNDARY
            || !peer_ids.insert(peer.endpoint_id.clone())
            || peer
                .capabilities
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(invalid(
                "peer capability envelopes are incomplete, duplicated, or outside the boundary",
            ));
        }
    }
    Ok(())
}

fn selected_perturbation(
    request: &EvidenceBackedResult4,
) -> Result<Perturbation, FederatedInterpretationError> {
    match request.perturbation_class.as_str() {
        "removal" => Ok(Perturbation::Removal),
        "multiplicative_range" => {
            Perturbation::relative_tolerance(request.relative_tolerance.unwrap_or(0.0))
                .map_err(|error| FederatedInterpretationError::Influence(error.to_string()))
        }
        _ => Err(invalid("unsupported perturbation class")),
    }
}

fn peer_negotiation(
    request: &EvidenceBackedResult4,
) -> (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    GatewayDisposition,
    Vec<SemanticLoss>,
    Vec<String>,
    Vec<String>,
) {
    let mut peers = request.peer_capabilities.clone();
    peers.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.endpoint_id.clone())
        .collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let mut capabilities = BTreeSet::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut losses = Vec::new();
    let mut migrated = false;
    for peer in peers {
        let version_ok = peer.contract_version == TARGET_CONTRACT_VERSION;
        let version_migrated = peer.contract_version == COMPATIBLE_CONTRACT_VERSION
            && peer
                .supported_contract_versions
                .iter()
                .any(|version| version == TARGET_CONTRACT_VERSION);
        let capability_ok = request
            .required_capabilities
            .iter()
            .all(|required| peer.capabilities.contains(required));
        if !version_ok && !version_migrated {
            omissions.push(format!("{}:incompatible-contract", peer.endpoint_id));
            continue;
        }
        if !peer.permitted_export {
            omissions.push(format!("{}:export-denied", peer.endpoint_id));
            continue;
        }
        if !peer.healthy || !peer.signed_capability {
            uncertainty.push(format!(
                "{}:peer-health-or-signature-unresolved",
                peer.endpoint_id
            ));
            continue;
        }
        if !capability_ok {
            omissions.push(format!("{}:required-capability-missing", peer.endpoint_id));
            continue;
        }
        accepted.push(peer.endpoint_id);
        capabilities.extend(peer.capabilities);
        if version_migrated {
            migrated = true;
            losses.push(SemanticLoss {
                field: "legacy_peer_fields".into(),
                reason: "compatible migration cannot infer fields absent from the pinned target contract".into(),
                severity: LossSeverity::Unknown,
            });
        }
    }
    accepted.sort();
    let mut disposition = if accepted.len() < request.quorum {
        uncertainty.push(format!(
            "peer quorum not met: {} of {} required",
            accepted.len(),
            request.quorum
        ));
        GatewayDisposition::Unknown
    } else if migrated {
        GatewayDisposition::Migrated
    } else {
        GatewayDisposition::Accepted
    };
    if !request.policy_allow {
        omissions.push("federation policy denied artifact exchange".into());
        disposition = GatewayDisposition::Blocked;
    } else if !request.protected_closure || !request.signed_approval {
        uncertainty.push("protected closure or signed A2 approval is incomplete".into());
        disposition = GatewayDisposition::ApprovalRequired;
    }
    (
        peer_order,
        accepted,
        capabilities.into_iter().collect(),
        disposition,
        losses,
        omissions,
        uncertainty,
    )
}

pub fn run_federated_continual_interpretation(
    request: &EvidenceBackedResult4,
) -> Result<InteractiveInterpretation, FederatedInterpretationError> {
    validate_request(request)?;
    let perturbation = selected_perturbation(request)?;
    let region = build_region(request)?;
    let (
        peer_order,
        accepted_peer_order,
        capability_order,
        mut disposition,
        mut semantic_loss,
        mut omissions,
        mut uncertainty,
    ) = peer_negotiation(request);
    let mut factors = request.factors.clone();
    factors.sort_by(|left, right| left.factor_id.cmp(&right.factor_id));
    let analyzer = InfluenceAnalyzer::default();
    let mut influence_order = Vec::with_capacity(factors.len());
    let mut all_bounded = true;
    let mut negative_evidence = Vec::new();
    for factor in &factors {
        let analysis = analyzer
            .analyse_factor(&region, &factor.factor_id, &perturbation)
            .map_err(|error| FederatedInterpretationError::Influence(error.to_string()))?;
        let (estimate, bound, selected_method) = match &analysis.estimate {
            InfluenceEstimate::Bounded(value) => (
                "bounded".into(),
                Some(value.value()),
                Some(value.method().as_str().into()),
            ),
            InfluenceEstimate::Unknown(reason) => {
                all_bounded = false;
                uncertainty.push(format!("{}:influence-unknown:{reason}", factor.factor_id));
                ("unknown".into(), None, None)
            }
        };
        if !matches!(
            factor.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            all_bounded = false;
            uncertainty.push(format!("{}:evidence-state-not-supported", factor.factor_id));
        }
        if factor.negative_result {
            negative_evidence.push(format!("{}:negative-result", factor.factor_id));
        }
        let mut attempted = analysis
            .attempted
            .iter()
            .map(|entry| MethodObservation {
                method: entry.method.as_str().into(),
                value: entry.value,
                declined: entry.declined.as_ref().map(ToString::to_string),
            })
            .collect::<Vec<_>>();
        attempted.sort_by(|left, right| left.method.cmp(&right.method));
        influence_order.push(InfluenceObservation {
            factor_id: factor.factor_id.clone(),
            modality: factor.modality.clone(),
            evidence_state: factor.evidence_state,
            estimate,
            bound,
            selected_method,
            attempted,
            evidence_digest: factor.evidence_digest.clone(),
            provenance_digest: factor.provenance_digest.clone(),
            negative_result: factor.negative_result,
        });
    }
    influence_order.sort_by(|left, right| left.factor_id.cmp(&right.factor_id));
    let covered_modalities = request
        .claims
        .iter()
        .map(|claim| claim.modality.clone())
        .collect::<BTreeSet<_>>();
    let mut required_modalities = request.required_modalities.clone();
    required_modalities.sort();
    required_modalities.dedup();
    let omitted_modalities = required_modalities
        .iter()
        .filter(|modality| !covered_modalities.contains(*modality))
        .cloned()
        .collect::<Vec<_>>();
    if !omitted_modalities.is_empty() {
        omissions.extend(
            omitted_modalities
                .iter()
                .map(|modality| format!("required modality unavailable: {modality}")),
        );
        semantic_loss.push(SemanticLoss {
            field: "required_modalities".into(),
            reason: "interactive interpretation cannot claim a view that no local claim covers"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
        all_bounded = false;
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let all_factors = influence_order
        .iter()
        .map(|entry| entry.factor_id.clone())
        .collect::<Vec<_>>();
    let interpretation_order = claims
        .iter()
        .map(|claim| InterpretationView {
            claim_id: claim.claim_id.clone(),
            modality: claim.modality.clone(),
            statement: claim.statement.clone(),
            bound_factor_order: all_factors.clone(),
            uncertainty: claim.uncertainty.clone(),
            negative_evidence: {
                let mut values = claim.negative_evidence.clone();
                values.sort();
                values.dedup();
                values
            },
        })
        .collect::<Vec<_>>();
    if request.previous_receipt.is_none() {
        uncertainty
            .push("continual chain has no previous receipt; this is an initial epoch".into());
    }
    let mut counterexamples = Vec::new();
    if accepted_peer_order.len() < request.quorum {
        counterexamples.push("peer quorum not met".into());
    }
    if !all_bounded {
        counterexamples
            .push("one or more required influence or evidence gates are unresolved".into());
    }
    if disposition == GatewayDisposition::Accepted || disposition == GatewayDisposition::Migrated {
        if !all_bounded || !omitted_modalities.is_empty() {
            disposition = GatewayDisposition::Unknown;
        }
    }
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    semantic_loss.sort_by(|left, right| left.field.cmp(&right.field));
    let verdict = match disposition {
        GatewayDisposition::Accepted | GatewayDisposition::Migrated => "qualified",
        GatewayDisposition::ApprovalRequired => "conditional",
        GatewayDisposition::Blocked | GatewayDisposition::Incompatible => "blocked",
        GatewayDisposition::Unknown => "unknown",
    };
    let mut checks = vec![
        "canonical peer and capability negotiation".into(),
        "pinned contract version and migration loss".into(),
        "typed local region and sound influence methods".into(),
        "evidence, provenance, uncertainty, and negative-result retention".into(),
        "protected closure, policy, and signed A2 approval".into(),
        "digest-only federation and continual replay identity".into(),
    ];
    checks.sort();
    let passed_checks = if verdict == "qualified" {
        checks.clone()
    } else {
        Vec::new()
    };
    let effect_receipts = if verdict == "qualified" {
        vec![
            "exchange:permitted-artifact-digests-only".into(),
            "interpret:qualified".into(),
        ]
    } else if disposition == GatewayDisposition::ApprovalRequired {
        vec!["approval-required:protected-closure-or-signed-authority".into()]
    } else if disposition == GatewayDisposition::Blocked {
        vec!["blocked:policy-or-boundary".into()]
    } else {
        vec!["partial:retain-unknown-and-omissions".into()]
    };
    let peer_payload = json!({
        "federation_id": request.federation_id,
        "epoch": request.epoch,
        "target_contract_version": request.target_contract_version,
        "peer_order": peer_order,
        "accepted_peer_order": accepted_peer_order,
        "capability_order": capability_order,
    });
    let federation_digest = ContentHash::of_value(&peer_payload)
        .map_err(|error| FederatedInterpretationError::Serialization(error.to_string()))?;
    let interpretation_payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "result_id": request.result_id,
        "consumer": request.consumer,
        "scope": request.scope,
        "institution_id": request.institution_id,
        "federation_id": request.federation_id,
        "epoch": request.epoch,
        "negotiated_version": TARGET_CONTRACT_VERSION,
        "disposition": disposition,
        "verdict": verdict,
        "claim_order": claim_order,
        "interpretation_order": interpretation_order,
        "influence_order": influence_order,
        "covered_modalities": covered_modalities,
        "omitted_modalities": omitted_modalities,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "checks": checks,
        "passed_checks": passed_checks,
        "counterexamples": counterexamples,
        "replay_identity": request.replay_identity,
        "federation_digest": federation_digest,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let interpretation_digest = ContentHash::of_value(&interpretation_payload)
        .map_err(|error| FederatedInterpretationError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "interactive-interpretation:{}:{}",
            request.federation_id, request.epoch
        ),
        "application/vnd.aurora.interactive-interpretation+json",
        &interpretation_payload,
        semantic_loss.clone(),
        request
            .factors
            .iter()
            .map(|factor| bioprism_foundation::ProvenanceLink {
                source_id: factor.factor_id.clone(),
                relation: "derived-from-local-factor".into(),
                digest: factor.provenance_digest.clone(),
            })
            .collect(),
    )
    .map_err(|error| FederatedInterpretationError::Artifact(error.to_string()))?;
    let receipt = InteractiveInterpretation {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        result_id: request.result_id.clone(),
        consumer: request.consumer.clone(),
        scope: request.scope.clone(),
        institution_id: request.institution_id.clone(),
        federation_id: request.federation_id.clone(),
        epoch: request.epoch,
        negotiated_version: TARGET_CONTRACT_VERSION.into(),
        disposition,
        verdict: verdict.into(),
        peer_order,
        accepted_peer_order,
        capability_order,
        claim_order,
        interpretation_order,
        influence_order,
        covered_modalities: covered_modalities.into_iter().collect(),
        omitted_modalities,
        omissions,
        uncertainty,
        negative_evidence,
        semantic_loss,
        checks,
        passed_checks,
        counterexamples,
        replay_identity: request.replay_identity.clone(),
        federation_digest,
        interpretation_digest,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EvidenceBackedResult4 {
        let evidence = ContentHash::of_bytes(b"evidence");
        EvidenceBackedResult4 {
            result_id: "result:interpretation".into(),
            consumer: "formal methods researcher".into(),
            scope: "study:alpha".into(),
            institution_id: "site:a".into(),
            federation_id: "federation:preclinical".into(),
            epoch: 1,
            previous_receipt: None,
            target_contract_version: TARGET_CONTRACT_VERSION.into(),
            required_capabilities: vec!["influence-bounds".into(), "interpretation-view".into()],
            quorum: 1,
            peer_capabilities: vec![PeerCapability {
                endpoint_id: "site:b".into(),
                contract_version: TARGET_CONTRACT_VERSION.into(),
                supported_contract_versions: vec![TARGET_CONTRACT_VERSION.into()],
                capabilities: vec!["influence-bounds".into(), "interpretation-view".into()],
                capability_digest: ContentHash::of_bytes(b"capability"),
                permitted_export: true,
                healthy: true,
                signed_capability: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            evidence_digests: vec![evidence.clone()],
            required_modalities: vec!["imaging".into()],
            variables: vec![InterpretationVariable {
                name: "signal".into(),
                cardinality: 2,
            }],
            factors: vec![InterpretationFactor {
                factor_id: "factor:signal".into(),
                scope: vec!["signal".into()],
                table: Some(vec![1.0, 2.0]),
                modality: "imaging".into(),
                evidence_state: EvidenceState::Supported,
                evidence_digest: evidence.clone(),
                provenance_digest: ContentHash::of_bytes(b"provenance"),
                negative_result: true,
            }],
            free_variables: vec!["signal".into()],
            claims: vec![InterpretationClaim {
                claim_id: "claim:signal".into(),
                modality: "imaging".into(),
                statement: "signal remains bounded".into(),
                supporting_evidence: vec![evidence],
                uncertainty: "replication remains required".into(),
                negative_evidence: vec!["null replicate".into()],
            }],
            perturbation_class: "removal".into(),
            relative_tolerance: None,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(b"replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn gateway_is_deterministic_and_qualified() {
        let first = run_federated_continual_interpretation(&request()).unwrap();
        let second = run_federated_continual_interpretation(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.verdict, "qualified");
        assert_eq!(first.disposition, GatewayDisposition::Accepted);
        assert!(!first.negative_evidence.is_empty());
    }

    #[test]
    fn missing_factor_table_remains_unknown() {
        let mut request = request();
        request.factors[0].table = None;
        let receipt = run_federated_continual_interpretation(&request).unwrap();
        assert_eq!(receipt.verdict, "unknown");
        assert!(receipt.influence_order[0].bound.is_none());
        assert!(!receipt.uncertainty.is_empty());
    }

    #[test]
    fn denied_policy_blocks_digest_exchange() {
        let mut request = request();
        request.policy_allow = false;
        let receipt = run_federated_continual_interpretation(&request).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Blocked);
        assert_eq!(receipt.verdict, "blocked");
        assert!(receipt.effect_receipts[0].starts_with("blocked:"));
    }

    #[test]
    fn incomplete_closure_requires_approval() {
        let mut request = request();
        request.protected_closure = false;
        let receipt = run_federated_continual_interpretation(&request).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::ApprovalRequired);
        assert_eq!(receipt.verdict, "conditional");
    }
}
