//! Federated continual evidence-surveillance researcher workbench.
//! Atlas feature `AFA-adapter-P01-F20`: an A1 read-only view over signed,
//! permitted aggregate contributions; raw observations never enter the view artifact.

use crate::federated_continual_evidence_surveillance_research_copilot::{
    canonical_federated_continual_evidence_surveillance_research_copilot_request,
    run_federated_continual_evidence_surveillance_research_copilot,
    FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    FederatedContinualResearchCopilotDisposition,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F20";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-evidence-surveillance-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet5@1";
const VIEWS: [&str; 4] = [
    "view:peers",
    "view:aggregate",
    "view:omissions",
    "view:provenance",
];
const PANELS: [&str; 4] = [
    "panel:denied",
    "panel:negative",
    "panel:qualified",
    "panel:unknown",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
    pub copilot_request: FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    pub workbench_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workbench_id: String,
    pub scope: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: FederatedContinualResearchCopilotDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContinualEvidenceSurveillanceResearchWorkbenchError {
    #[error("invalid federated workbench request: {0}")]
    Invalid(String),
    #[error("federated workbench artifact failed: {0}")]
    Artifact(String),
    #[error("federated workbench copilot failed: {0}")]
    Copilot(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    if value.is_empty() || value.trim() != value {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} must be non-empty and trimmed"
            )),
        );
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} is outside its bounded text contract"
            )),
        );
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    if values.len() > MAX_ITEMS {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} exceeds its item bound"
            )),
        );
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                    "{field} contains duplicate values"
                )),
            );
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} ordering is not canonical"
            )),
        );
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} must be a 64-character hex digest"
            )),
        );
    }
    Ok(())
}

fn federated_workbench_input_digest(
    request: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<ContentHash, FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    let canonical =
        canonical_federated_continual_evidence_surveillance_research_workbench_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })
}

fn canonical_federated_continual_evidence_surveillance_research_workbench_request(
    request: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
    let mut canonical = request.clone();
    canonical.copilot_request =
        canonical_federated_continual_evidence_surveillance_research_copilot_request(
            &canonical.copilot_request,
        );
    canonical
}
impl FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workbench_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.allowed_artifacts.is_empty()
            || self.min_peer_quorum == 0
            || self.budget_units == 0
            || self.view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid("federated workbench identity, canonical views, locality, candidates, or effects are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workbench_id", &self.workbench_id)?;
        validate_text("scope", &self.scope)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("endpoint", &self.endpoint)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_sorted_strings("allowed_artifacts", &self.allowed_artifacts)?;
        validate_sorted_strings("peer_order", &self.peer_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("qualified_order", &self.qualified_order)?;
        validate_sorted_strings("unknown_order", &self.unknown_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
        validate_sorted_strings("aggregate_order", &self.aggregate_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        let classified = self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench states do not partition candidates".into(),
                ),
            );
        }
        if self.aggregate_order != self.qualified_order {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench aggregate view must equal qualified view".into(),
                ),
            );
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.federation_digest,
            &self.envelope_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("federated workbench receipt digest", value)?;
        }
        let expected_effect = format!("view:federated-evidence-workbench:{}", self.workbench_id);
        if self.effect_receipts != vec![expected_effect] {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench effect is not the declared read-only view".into(),
                ),
            );
        }
        let expected_federation = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "peer_order": self.peer_order,
            "min_peer_quorum": self.min_peer_quorum,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(
                error.to_string(),
            )
        })?;
        if self.federation_digest != expected_federation {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench federation digest does not match peer scope".into(),
                ),
            );
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "allowed_artifacts": self.allowed_artifacts,
            "semantic_profile": self.semantic_profile,
            "aggregate_order": self.aggregate_order,
            "raw_data_local": self.raw_data_local,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(
                error.to_string(),
            )
        })?;
        if self.envelope_digest != expected_envelope {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench envelope digest does not match aggregate policy scope"
                        .into(),
                ),
            );
        }
        let expected_workbench = ContentHash::of_value(&json!({
            "workbench_id": self.workbench_id,
            "scope": self.scope,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "semantic_profile": self.semantic_profile,
            "views": self.view_order,
            "panels": self.panel_order,
            "peer_order": self.peer_order,
            "candidate": self.candidate_order,
            "qualified": self.qualified_order,
            "unknown": self.unknown_order,
            "blocked": self.blocked_order,
            "aggregate": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(
                error.to_string(),
            )
        })?;
        if self.workbench_digest != expected_workbench {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench digest does not match its rendered state".into(),
                ),
            );
        }
        if self.artifact.artifact_id
            != format!("adapter-federated-evidence-workbench:{}", self.workbench_id)
            || self.artifact.content_type
                != "application/vnd.aurora.federated-evidence-workbench+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(
                    "federated workbench artifact is not bound to its rendered state".into(),
                ),
            );
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workbench_id": self.workbench_id,
            "scope": self.scope,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "semantic_profile": self.semantic_profile,
            "allowed_artifacts": self.allowed_artifacts,
            "min_peer_quorum": self.min_peer_quorum,
            "budget_units": self.budget_units,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "peer_order": self.peer_order,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "unknown_order": self.unknown_order,
            "blocked_order": self.blocked_order,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "workbench_digest": self.workbench_digest,
            "federation_digest": self.federation_digest,
            "envelope_digest": self.envelope_digest,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(
                error.to_string(),
            )
        })?;
        self.artifact.validate_metadata().map_err(|e| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
        })?;
        if self.input_digest != federated_workbench_input_digest(&self.input)? {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench retained input digest mismatch".into(),
                ),
            );
        }
        let expected =
            build_federated_continual_evidence_surveillance_research_workbench(&self.input)?;
        if self != &expected {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench receipt does not match its retained input".into(),
                ),
            );
        }
        Ok(())
    }
}
pub fn federated_continual_evidence_surveillance_research_workbench_manifest() -> CapabilityManifest
{
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["preclinical researcher".into(),"consortium administrator".into()].into(),behavior:"renders a deterministic federated continual EvidenceFeed4 workbench with peer, aggregate, omission, denied, unknown, negative, qualified, and provenance panels without moving raw observations".into(),value:"gives preclinical researchers an accessible policy-separated view of permitted cross-institution evidence while retaining signer, locality, quorum, and negative-result evidence".into(),inputs:vec![TypedPort{name:"federated_evidence_workbench_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"qualified_federated_evidence_workbench_set".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"json-schema".into(),state:EvidenceState::Supported,locator:Some("https://json-schema.org/specification".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_federated_continual_evidence_surveillance_research_workbench(
    request: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt,
    FederatedContinualEvidenceSurveillanceResearchWorkbenchError,
> {
    let receipt = build_federated_continual_evidence_surveillance_research_workbench(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_federated_continual_evidence_surveillance_research_workbench(
    request: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt,
    FederatedContinualEvidenceSurveillanceResearchWorkbenchError,
> {
    validate_request(request)?;
    let canonical_request =
        canonical_federated_continual_evidence_surveillance_research_workbench_request(request);
    let request = &canonical_request;
    let c =
        run_federated_continual_evidence_surveillance_research_copilot(&request.copilot_request)
            .map_err(|e| {
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Copilot(e.to_string())
            })?;
    let views = VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let panels = PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let candidate = c.candidate_order.clone();
    let qualified = c.selected_order.clone();
    let unknown = c.unresolved_order.clone();
    let blocked = c.denied_order.clone();
    let aggregate = c.aggregate_order.clone();
    let copilot_run_digest = c.run_digest.clone();
    let workbench_digest = ContentHash::of_value(&json!({
        "workbench_id": request.workbench_id,
        "scope": request.scope,
        "federation_id": request.copilot_request.federation_id,
        "purpose": request.copilot_request.purpose,
        "endpoint": request.copilot_request.endpoint,
        "semantic_profile": request.copilot_request.semantic_profile,
        "views": views,
        "panels": panels,
        "peer_order": c.peer_order,
        "candidate": candidate,
        "qualified": qualified,
        "unknown": unknown,
        "blocked": blocked,
        "aggregate": aggregate,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })?;
    let mut omissions = c.omissions.clone();
    omissions.push("workbench:read-only-federated-view".into());
    omissions.sort();
    omissions.dedup();
    let effect_receipts = vec![format!(
        "view:federated-evidence-workbench:{}",
        request.workbench_id
    )];
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.copilot_request.request_id,
        "workbench_id": request.workbench_id,
        "scope": request.scope,
        "federation_id": request.copilot_request.federation_id,
        "purpose": request.copilot_request.purpose,
        "endpoint": request.copilot_request.endpoint,
        "semantic_profile": request.copilot_request.semantic_profile,
        "allowed_artifacts": request.copilot_request.allowed_artifacts,
        "min_peer_quorum": request.copilot_request.min_peer_quorum,
        "budget_units": request.budget_units,
        "policy_allow": request.copilot_request.policy_allow,
        "protected_closure": request.copilot_request.protected_closure,
        "disposition": c.disposition,
        "view_order": views,
        "panel_order": panels,
        "peer_order": c.peer_order,
        "candidate_order": candidate,
        "qualified_order": qualified,
        "unknown_order": unknown,
        "blocked_order": blocked,
        "aggregate_order": aggregate,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "workbench_digest": workbench_digest,
        "federation_digest": c.federation_digest,
        "envelope_digest": c.envelope_digest,
        "omissions": omissions,
        "uncertainty": c.uncertainty,
        "negative_evidence": c.negative_evidence,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": true,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-evidence-workbench:{}",
            request.workbench_id
        ),
        "application/vnd.aurora.federated-evidence-workbench+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
    })?;
    let input_digest = federated_workbench_input_digest(request)?;
    let receipt = FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.copilot_request.request_id.clone(),
        workbench_id: request.workbench_id.clone(),
        scope: request.scope.clone(),
        federation_id: request.copilot_request.federation_id.clone(),
        purpose: request.copilot_request.purpose.clone(),
        endpoint: request.copilot_request.endpoint.clone(),
        semantic_profile: request.copilot_request.semantic_profile.clone(),
        allowed_artifacts: request.copilot_request.allowed_artifacts.clone(),
        min_peer_quorum: request.copilot_request.min_peer_quorum,
        budget_units: request.budget_units,
        policy_allow: request.copilot_request.policy_allow,
        protected_closure: request.copilot_request.protected_closure,
        disposition: c.disposition,
        view_order: views,
        panel_order: panels,
        peer_order: c.peer_order.clone(),
        candidate_order: candidate,
        qualified_order: qualified,
        unknown_order: unknown,
        blocked_order: blocked,
        aggregate_order: aggregate,
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        federation_digest: c.federation_digest.clone(),
        envelope_digest: c.envelope_digest.clone(),
        omissions,
        uncertainty: c.uncertainty.clone(),
        negative_evidence: c.negative_evidence.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: request.boundary.clone(),
    };
    Ok(receipt)
}
fn validate_request(
    r: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    if r.budget_units == 0
        || u64::from(r.budget_units) > MAX_ITEMS as u64
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.copilot_request.boundary != PRECLINICAL_BOUNDARY
        || !r.copilot_request.raw_data_local
        || !r.copilot_request.dry_run
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "federated workbench identity, budget, dry-run, locality, or boundary is invalid"
                    .into(),
            ),
        );
    }
    validate_text("workbench_id", &r.workbench_id)?;
    validate_text("scope", &r.scope)?;
    validate_text("boundary", &r.boundary)?;
    validate_text("copilot request_id", &r.copilot_request.request_id)?;
    validate_text("copilot federation_id", &r.copilot_request.federation_id)?;
    validate_text("copilot purpose", &r.copilot_request.purpose)?;
    validate_text("copilot endpoint", &r.copilot_request.endpoint)?;
    validate_text(
        "copilot semantic_profile",
        &r.copilot_request.semantic_profile,
    )?;
    if r.scope != format!("federation:{}", r.copilot_request.federation_id) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "federated workbench scope must bind to its federation".into(),
            ),
        );
    }
    if r.requested_view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
        || r.requested_panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "federated workbench views, panels, or replay identity is invalid".into(),
            ),
        );
    }
    validate_digest("workbench replay identity", &r.replay_identity)?;
    validate_digest(
        "copilot replay identity",
        &r.copilot_request.replay_identity,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federated_continual_evidence_surveillance_research_copilot::FederatedCopilotEvidenceContribution;
    fn request() -> FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
        let c = FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
            request_id: "req-20".into(),
            agent_id: "researcher-20".into(),
            federation_id: "federation-20".into(),
            purpose: "evidence surveillance".into(),
            endpoint: "local://federation".into(),
            semantic_profile: "profile-v1".into(),
            allowed_artifacts: vec!["qualified-evidence".into()],
            min_peer_quorum: 2,
            declared_tools: vec!["evidence.inspect".into()],
            requested_tool: "evidence.inspect".into(),
            max_tool_calls: 1,
            dry_run: true,
            approval_reference: None,
            approval_granted: false,
            contributions: vec![
                FederatedCopilotEvidenceContribution {
                    peer_id: "peer-a".into(),
                    institution_id: "inst-a".into(),
                    source_id: "source-a".into(),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(b"a")),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
                FederatedCopilotEvidenceContribution {
                    peer_id: "peer-b".into(),
                    institution_id: "inst-b".into(),
                    source_id: "source-b".into(),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(b"b")),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(b"copilot-20"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
            copilot_request: c,
            workbench_id: "wb-20".into(),
            scope: "federation:federation-20".into(),
            requested_view_order: VIEWS.iter().map(|v| (*v).to_string()).collect(),
            requested_panel_order: PANELS.iter().map(|v| (*v).to_string()).collect(),
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"wb-20"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_continual_evidence_surveillance_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn renders_view() {
        let r = render_federated_continual_evidence_surveillance_research_workbench(&request())
            .unwrap();
        assert_eq!(r.feature_id, FEATURE_ID)
    }
    #[test]
    fn policy_denial_visible() {
        let mut r = request();
        r.copilot_request.policy_allow = false;
        let receipt =
            render_federated_continual_evidence_surveillance_research_workbench(&r).unwrap();
        assert!(receipt.qualified_order.is_empty());
        assert_eq!(receipt.blocked_order, receipt.candidate_order);
        assert_eq!(
            receipt.effect_receipts,
            vec!["view:federated-evidence-workbench:wb-20"]
        );
    }
    #[test]
    fn rejects_non_dry_run() {
        let mut r = request();
        r.copilot_request.dry_run = false;
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn rejects_panels() {
        let mut r = request();
        r.requested_panel_order.reverse();
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn scope_must_bind_to_federation() {
        let mut r = request();
        r.scope = "federation:other".into();
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn tampered_workbench_digest_is_rejected() {
        let mut receipt =
            render_federated_continual_evidence_surveillance_research_workbench(&request())
                .unwrap();
        receipt.workbench_digest = ContentHash::of_bytes(b"tampered-workbench");
        assert!(receipt.validate().is_err())
    }
    #[test]
    fn tampered_envelope_digest_is_rejected() {
        let mut receipt =
            render_federated_continual_evidence_surveillance_research_workbench(&request())
                .unwrap();
        receipt.envelope_digest = ContentHash::of_bytes(b"tampered-envelope");
        assert!(receipt.validate().is_err())
    }
    #[test]
    fn tampered_retained_request_is_rejected() {
        let mut receipt =
            render_federated_continual_evidence_surveillance_research_workbench(&request())
                .unwrap();
        receipt.input.scope = "federation:tampered".into();
        assert!(receipt.validate().is_err())
    }
    #[test]
    fn replay_stable() {
        let r = request();
        assert_eq!(
            render_federated_continual_evidence_surveillance_research_workbench(&r).unwrap(),
            render_federated_continual_evidence_surveillance_research_workbench(&r).unwrap()
        )
    }

    #[test]
    fn reordered_nested_copilot_input_has_stable_identity() {
        let mut reordered = request();
        reordered.copilot_request.allowed_artifacts.reverse();
        reordered.copilot_request.declared_tools.reverse();
        reordered.copilot_request.contributions.reverse();
        let first = render_federated_continual_evidence_surveillance_research_workbench(&request())
            .unwrap();
        let second =
            render_federated_continual_evidence_surveillance_research_workbench(&reordered)
                .unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workbench_digest, second.workbench_digest);
    }
}
