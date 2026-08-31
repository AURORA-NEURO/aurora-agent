//! Local single-study evidence-surveillance researcher workbench.
//!
//! Atlas feature `AFA-adapter-P01-F17`.  This is an A0 interaction surface:
//! it renders a typed, institution-local evidence state without scheduling
//! work, invoking external tools, or turning unresolved evidence into a
//! conclusion.  The panels are deterministic and retain omissions, negative
//! evidence, and provenance digests for replay.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::local_evidence_surveillance_research_copilot::{
    run_local_evidence_surveillance_research_copilot,
    LocalEvidenceSurveillanceResearchCopilotRequest, ResearchCopilotDisposition,
};

pub const FEATURE_ID: &str = "AFA-adapter-P01-F17";
pub const CONTRACT_VERSION: &str = "adapter-local-evidence-surveillance-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet5@1";
const CANONICAL_VIEWS: [&str; 4] = [
    "view:overview",
    "view:evidence",
    "view:omissions",
    "view:provenance",
];
const CANONICAL_PANELS: [&str; 4] = [
    "panel:negative",
    "panel:provenance",
    "panel:qualified",
    "panel:unknown",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceResearchWorkbenchRequest {
    pub copilot_request: LocalEvidenceSurveillanceResearchCopilotRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: LocalEvidenceSurveillanceResearchWorkbenchRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workspace_id: String,
    pub study_id: String,
    pub scope: String,
    pub intent: String,
    pub disposition: ResearchCopilotDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocalEvidenceSurveillanceResearchWorkbenchError {
    #[error("invalid local evidence workbench request: {0}")]
    Invalid(String),
    #[error("local evidence workbench artifact failed: {0}")]
    Artifact(String),
    #[error("local evidence workbench copilot failed: {0}")]
    Copilot(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
    if value.is_empty() || value.trim() != value {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            format!("{field} must be non-empty and trimmed"),
        ));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            format!("{field} is outside its bounded text contract"),
        ));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
    if values.len() > MAX_ITEMS {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            format!("{field} exceeds its item bound"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                format!("{field} contains duplicate values"),
            ));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            format!("{field} ordering is not canonical"),
        ));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            format!("{field} must be a 64-character hex digest"),
        ));
    }
    Ok(())
}

impl LocalEvidenceSurveillanceResearchWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.view_order.is_empty()
            || self.panel_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench identity, views, candidates, locality, or effects are incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workspace_id", &self.workspace_id)?;
        validate_text("study_id", &self.study_id)?;
        validate_text("scope", &self.scope)?;
        validate_text("intent", &self.intent)?;
        if self.view_order
            != CANONICAL_VIEWS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
            || self.panel_order
                != CANONICAL_PANELS
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect::<Vec<_>>()
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench view or panel order is not canonical".into(),
            ));
        }
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("qualified_order", &self.qualified_order)?;
        validate_sorted_strings("unknown_order", &self.unknown_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
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
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench evidence states do not partition candidates".into(),
            ));
        }
        if self.disposition == ResearchCopilotDisposition::Completed
            && (!self.unknown_order.is_empty() || !self.blocked_order.is_empty())
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "completed workbench cannot retain unknown or blocked evidence".into(),
            ));
        }
        if matches!(
            self.disposition,
            ResearchCopilotDisposition::Unknown | ResearchCopilotDisposition::Blocked
        ) && !self.qualified_order.is_empty()
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "unknown or blocked workbench cannot retain qualified evidence".into(),
            ));
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("workbench receipt digest", value)?;
        }
        if self.effect_receipts
            != vec![format!(
                "view:local-evidence-workbench:{}",
                self.workspace_id
            )]
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench effect is not the declared read-only view".into(),
            ));
        }
        let expected_workbench_digest = ContentHash::of_value(&json!({
            "workspace_id": self.workspace_id,
            "study_id": self.study_id,
            "scope": self.scope,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "unknown_order": self.unknown_order,
            "blocked_order": self.blocked_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
        }))
        .map_err(|error| {
            LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        if self.workbench_digest != expected_workbench_digest {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench digest does not match its rendered state".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-local-evidence-workbench:{}", self.workspace_id)
            || self.artifact.content_type != "application/vnd.aurora.local-evidence-workbench+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(
                "workbench artifact is not bound to its rendered state".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workspace_id": self.workspace_id,
            "study_id": self.study_id,
            "scope": self.scope,
            "intent": self.intent,
            "disposition": self.disposition,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "unknown_order": self.unknown_order,
            "blocked_order": self.blocked_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "workbench_digest": self.workbench_digest,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        if self.input_digest != workbench_input_digest(&self.input)? {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench retained input digest mismatch".into(),
            ));
        }
        let expected = build_local_evidence_surveillance_research_workbench(&self.input)?;
        if self != &expected {
            return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "workbench receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn local_evidence_surveillance_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["consortium administrator".into(), "preclinical researcher".into()].into(),
        behavior: "renders a deterministic local single-study EvidenceFeed1 workbench with qualified, unknown, negative, omission, uncertainty, and provenance panels without scheduling external effects".into(),
        value: "gives researchers an accessible, replayable view of evidence state while preserving the distinction between zero, unknown, unmeasured, and denied evidence".into(),
        inputs: vec![TypedPort { name: "local_evidence_workbench_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_local_evidence_workbench_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(),
        permissions: ["view:authorized-research-state".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn render_local_evidence_surveillance_research_workbench(
    request: &LocalEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    LocalEvidenceSurveillanceResearchWorkbenchReceipt,
    LocalEvidenceSurveillanceResearchWorkbenchError,
> {
    let receipt = build_local_evidence_surveillance_research_workbench(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn workbench_input_digest(
    request: &LocalEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<ContentHash, LocalEvidenceSurveillanceResearchWorkbenchError> {
    let canonical = canonical_local_evidence_surveillance_research_workbench_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })
}

fn canonical_local_evidence_surveillance_research_workbench_request(
    request: &LocalEvidenceSurveillanceResearchWorkbenchRequest,
) -> LocalEvidenceSurveillanceResearchWorkbenchRequest {
    let mut canonical = request.clone();
    canonical.copilot_request = crate::local_evidence_surveillance_research_copilot::
        canonical_local_evidence_surveillance_research_copilot_request(
            &canonical.copilot_request,
        );
    canonical
}

fn build_local_evidence_surveillance_research_workbench(
    request: &LocalEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    LocalEvidenceSurveillanceResearchWorkbenchReceipt,
    LocalEvidenceSurveillanceResearchWorkbenchError,
> {
    validate_request(request)?;
    let copilot = run_local_evidence_surveillance_research_copilot(&request.copilot_request)
        .map_err(|error| {
            LocalEvidenceSurveillanceResearchWorkbenchError::Copilot(error.to_string())
        })?;
    let view_order = CANONICAL_VIEWS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let panel_order = CANONICAL_PANELS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let candidate_order = copilot.candidate_order.clone();
    let qualified_order = copilot.selected_order.clone();
    let unknown_order = copilot.unresolved_order.clone();
    let blocked_order = copilot.denied_order.clone();
    let copilot_run_digest = copilot.run_digest.clone();
    let workbench_digest = ContentHash::of_value(&json!({
        "workspace_id": request.workspace_id,
        "study_id": request.copilot_request.study_id,
        "scope": request.scope,
        "view_order": view_order,
        "panel_order": panel_order,
        "candidate_order": candidate_order,
        "qualified_order": qualified_order,
        "unknown_order": unknown_order,
        "blocked_order": blocked_order,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
    }))
    .map_err(|error| {
        LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })?;
    let mut omissions = copilot.omissions.clone();
    omissions.push("workbench:read-only-local-view".into());
    omissions.sort();
    omissions.dedup();
    let effect_receipts = vec![format!(
        "view:local-evidence-workbench:{}",
        request.workspace_id
    )];
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.copilot_request.request_id,
        "workspace_id": request.workspace_id,
        "study_id": request.copilot_request.study_id,
        "scope": request.scope,
        "intent": request.copilot_request.intent,
        "disposition": copilot.disposition,
        "view_order": view_order,
        "panel_order": panel_order,
        "candidate_order": candidate_order,
        "qualified_order": qualified_order,
        "unknown_order": unknown_order,
        "blocked_order": blocked_order,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "workbench_digest": workbench_digest,
        "omissions": omissions,
        "uncertainty": copilot.uncertainty,
        "negative_evidence": copilot.negative_evidence,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": true,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-local-evidence-workbench:{}", request.workspace_id),
        "application/vnd.aurora.local-evidence-workbench+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| {
        LocalEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
    })?;
    let canonical_request =
        canonical_local_evidence_surveillance_research_workbench_request(request);
    let receipt = LocalEvidenceSurveillanceResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: workbench_input_digest(request)?,
        request_id: request.copilot_request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        study_id: request.copilot_request.study_id.clone(),
        scope: request.scope.clone(),
        intent: request.copilot_request.intent.clone(),
        disposition: copilot.disposition,
        view_order,
        panel_order,
        candidate_order,
        qualified_order,
        unknown_order,
        blocked_order,
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        omissions,
        uncertainty: copilot.uncertainty.clone(),
        negative_evidence: copilot.negative_evidence.clone(),
        effect_receipts,
        artifact,
        raw_data_local: request.copilot_request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    Ok(receipt)
}

fn validate_request(
    request: &LocalEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<(), LocalEvidenceSurveillanceResearchWorkbenchError> {
    if request.budget_units == 0
        || u64::from(request.budget_units) > MAX_ITEMS as u64
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.copilot_request.boundary != PRECLINICAL_BOUNDARY
        || !request.copilot_request.raw_data_local
        || !request.copilot_request.dry_run
    {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            "workbench identity, budget, dry-run, locality, or preclinical boundary is invalid"
                .into(),
        ));
    }
    validate_text("workspace_id", &request.workspace_id)?;
    validate_text("scope", &request.scope)?;
    validate_text("boundary", &request.boundary)?;
    validate_text("copilot.study_id", &request.copilot_request.study_id)?;
    if request.scope != format!("study:{}", request.copilot_request.study_id) {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            "workbench scope must name exactly its single study".into(),
        ));
    }
    let expected_views = CANONICAL_VIEWS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let expected_panels = CANONICAL_PANELS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if request.requested_view_order != expected_views
        || request.requested_panel_order != expected_panels
    {
        return Err(LocalEvidenceSurveillanceResearchWorkbenchError::Invalid(
            "workbench views or panels are not canonical".into(),
        ));
    }
    validate_digest("workbench.replay_identity", &request.replay_identity)?;
    validate_digest(
        "copilot.replay_identity",
        &request.copilot_request.replay_identity,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_evidence_surveillance_research_copilot::CopilotEvidenceObservation;
    use bioprism_foundation::{EvidenceAvailability, EvidenceState};

    fn request() -> LocalEvidenceSurveillanceResearchWorkbenchRequest {
        LocalEvidenceSurveillanceResearchWorkbenchRequest {
            copilot_request: LocalEvidenceSurveillanceResearchCopilotRequest {
                request_id: "req-17".into(),
                agent_id: "researcher-17".into(),
                study_id: "study-17".into(),
                intent: "inspect preclinical evidence".into(),
                declared_tools: vec!["evidence.inspect".into()],
                requested_tool: "evidence.inspect".into(),
                max_tool_calls: 1,
                dry_run: true,
                required_source_ids: vec!["source-a".into()],
                observations: vec![CopilotEvidenceObservation {
                    source_id: "source-a".into(),
                    study_id: "study-17".into(),
                    source_type: "paper".into(),
                    locator: "local://source-a".into(),
                    digest: Some(ContentHash::of_bytes(b"source-a")),
                    availability: EvidenceAvailability::Available,
                    evidence_state: EvidenceState::Supported,
                    relevance_score: 90,
                    negative_result: false,
                }],
                min_relevance_score: 50,
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                replay_identity: ContentHash::of_bytes(b"copilot-17"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace-17".into(),
            scope: "study:study-17".into(),
            requested_view_order: CANONICAL_VIEWS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: CANONICAL_PANELS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"workbench-17"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_read_only() {
        let m = local_evidence_surveillance_research_workbench_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A0);
        assert!(m.authority_requirements.is_empty());
        assert!(m.validate().is_ok());
    }
    #[test]
    fn renders_qualified_read_only_view() {
        let receipt = render_local_evidence_surveillance_research_workbench(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert!(receipt.effect_receipts[0].starts_with("view:local-evidence-workbench:"));
    }
    #[test]
    fn policy_denial_remains_visible() {
        let mut r = request();
        r.copilot_request.policy_allow = false;
        let receipt = render_local_evidence_surveillance_research_workbench(&r).unwrap();
        assert_eq!(receipt.disposition, ResearchCopilotDisposition::Blocked);
        assert!(receipt.omissions.iter().any(|item| item.contains("policy")));
    }
    #[test]
    fn rejects_non_dry_run() {
        let mut r = request();
        r.copilot_request.dry_run = false;
        assert!(render_local_evidence_surveillance_research_workbench(&r).is_err());
    }
    #[test]
    fn rejects_noncanonical_panels() {
        let mut r = request();
        r.requested_panel_order.reverse();
        assert!(render_local_evidence_surveillance_research_workbench(&r).is_err());
    }
    #[test]
    fn replay_is_stable() {
        let r = request();
        assert_eq!(
            render_local_evidence_surveillance_research_workbench(&r).unwrap(),
            render_local_evidence_surveillance_research_workbench(&r).unwrap()
        );
    }

    #[test]
    fn reordered_nested_copilot_input_has_stable_identity() {
        let mut reordered = request();
        reordered.copilot_request.declared_tools.reverse();
        reordered.copilot_request.required_source_ids.reverse();
        reordered.copilot_request.observations.reverse();
        let first = render_local_evidence_surveillance_research_workbench(&request()).unwrap();
        let second = render_local_evidence_surveillance_research_workbench(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workbench_digest, second.workbench_digest);
    }

    #[test]
    fn tampered_workbench_digest_is_rejected() {
        let mut receipt =
            render_local_evidence_surveillance_research_workbench(&request()).unwrap();
        receipt.workbench_digest = ContentHash::of_bytes(b"tampered-workbench");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn scope_must_bind_to_single_study() {
        let mut value = request();
        value.scope = "study:other".into();
        assert!(render_local_evidence_surveillance_research_workbench(&value).is_err());
    }

    #[test]
    fn blocked_view_remains_read_only_and_unqualified() {
        let mut value = request();
        value.copilot_request.policy_allow = false;
        let receipt = render_local_evidence_surveillance_research_workbench(&value).unwrap();
        assert!(receipt.qualified_order.is_empty());
        assert_eq!(
            receipt.effect_receipts,
            vec!["view:local-evidence-workbench:workspace-17"]
        );
    }

    #[test]
    fn receipt_rejects_tampered_retained_view_request() {
        let mut receipt =
            render_local_evidence_surveillance_research_workbench(&request()).unwrap();
        receipt.input.scope = "study:tampered".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
