//! Prospective high-throughput evidence-surveillance researcher workbench.
//! Atlas feature `AFA-adapter-P01-F19`: a bounded A1 interaction surface over the
//! EvidenceFeed3 copilot that keeps queue capacity and checkpoint state visible.

use crate::throughput_evidence_surveillance_research_copilot::{
    canonical_throughput_evidence_surveillance_research_copilot_request,
    run_throughput_evidence_surveillance_research_copilot,
    ThroughputEvidenceSurveillanceResearchCopilotRequest, ThroughputResearchCopilotDisposition,
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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F19";
pub const CONTRACT_VERSION: &str =
    "adapter-throughput-evidence-surveillance-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet5@1";
const VIEWS: [&str; 4] = [
    "view:queue",
    "view:capacity",
    "view:checkpoint",
    "view:provenance",
];
const PANELS: [&str; 4] = [
    "panel:overflow",
    "panel:negative",
    "panel:qualified",
    "panel:unknown",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchWorkbenchRequest {
    pub copilot_request: ThroughputEvidenceSurveillanceResearchCopilotRequest,
    pub workbench_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workbench_id: String,
    pub scope: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ThroughputResearchCopilotDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceResearchWorkbenchError {
    #[error("invalid throughput workbench request: {0}")]
    Invalid(String),
    #[error("throughput workbench artifact failed: {0}")]
    Artifact(String),
    #[error("throughput workbench copilot failed: {0}")]
    Copilot(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    if value.is_empty() || value.trim() != value {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} must be non-empty and trimmed"
            )),
        );
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} is outside its bounded text contract"
            )),
        );
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    if values.len() > MAX_ITEMS {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} exceeds its item bound"
            )),
        );
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
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
) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} ordering is not canonical"
            )),
        );
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(format!(
                "{field} must be a 64-character hex digest"
            )),
        );
    }
    Ok(())
}

impl ThroughputEvidenceSurveillanceResearchWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workbench_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.capacity > MAX_ITEMS
            || self.budget_units == 0
            || u64::from(self.budget_units) > MAX_ITEMS as u64
            || self.view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid("throughput workbench identity, checkpoint, canonical views, capacity, locality, candidates, or effects are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workbench_id", &self.workbench_id)?;
        validate_text("scope", &self.scope)?;
        validate_text("batch_id", &self.batch_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("qualified_order", &self.qualified_order)?;
        validate_sorted_strings("unknown_order", &self.unknown_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
        validate_sorted_strings("overflow_order", &self.overflow_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        if self.scope != format!("batch:{}", self.batch_id) {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench scope must bind to its batch".into(),
                ),
            );
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench states do not partition candidates".into(),
                ),
            );
        }
        let expected_overflow_len =
            if self.disposition == ThroughputResearchCopilotDisposition::Blocked {
                0
            } else {
                self.candidate_order.len().saturating_sub(self.capacity)
            };
        if self.overflow_order.len() != expected_overflow_len {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput overflow does not match queue capacity".into(),
                ),
            );
        }
        if self.disposition == ThroughputResearchCopilotDisposition::Completed
            && (!self.unknown_order.is_empty()
                || !self.blocked_order.is_empty()
                || !self.overflow_order.is_empty())
        {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "completed throughput workbench cannot retain unresolved, denied, or overflow states".into(),
                ),
            );
        }
        if matches!(
            self.disposition,
            ThroughputResearchCopilotDisposition::Unknown
                | ThroughputResearchCopilotDisposition::Blocked
        ) && !self.qualified_order.is_empty()
        {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "unknown or blocked throughput workbench cannot retain qualified evidence"
                        .into(),
                ),
            );
        }
        for value in [
            &self.replay_identity,
            &self.copilot_replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("throughput workbench receipt digest", value)?;
        }
        let expected_effect = format!("view:throughput-evidence-workbench:{}", self.workbench_id);
        if self.effect_receipts != vec![expected_effect] {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench effect is not the declared read-only view".into(),
                ),
            );
        }
        let expected_queue = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "capacity": self.capacity,
            "candidate_order": self.candidate_order,
            "checkpoint_seq": self.checkpoint_seq,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        if self.queue_digest != expected_queue {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench queue digest does not match capacity state".into(),
                ),
            );
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "replay_identity": self.copilot_replay_identity,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench checkpoint digest does not match replay identity".into(),
                ),
            );
        }
        let expected_workbench = ContentHash::of_value(&json!({
            "workbench_id": self.workbench_id,
            "scope": self.scope,
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "capacity": self.capacity,
            "budget_units": self.budget_units,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "views": self.view_order,
            "panels": self.panel_order,
            "candidate": self.candidate_order,
            "qualified": self.qualified_order,
            "unknown": self.unknown_order,
            "blocked": self.blocked_order,
            "overflow": self.overflow_order,
            "replay_identity": self.replay_identity,
            "copilot_replay_identity": self.copilot_replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "queue_digest": self.queue_digest,
            "checkpoint_digest": self.checkpoint_digest,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        if self.workbench_digest != expected_workbench {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench digest does not match its rendered state".into(),
                ),
            );
        }
        if self.artifact.artifact_id
            != format!(
                "adapter-throughput-evidence-workbench:{}",
                self.workbench_id
            )
            || self.artifact.content_type
                != "application/vnd.aurora.throughput-evidence-workbench+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(
                    "throughput workbench artifact is not bound to its rendered state".into(),
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
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "capacity": self.capacity,
            "budget_units": self.budget_units,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "unknown_order": self.unknown_order,
            "blocked_order": self.blocked_order,
            "overflow_order": self.overflow_order,
            "replay_identity": self.replay_identity,
            "copilot_replay_identity": self.copilot_replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "workbench_digest": self.workbench_digest,
            "queue_digest": self.queue_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|e| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
        })?;
        if self.input_digest != workbench_input_digest(&self.input)? {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench retained input digest mismatch".into(),
                ),
            );
        }
        let expected = build_throughput_evidence_surveillance_research_workbench(&self.input)?;
        if self != &expected {
            return Err(
                ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "throughput workbench receipt does not match its retained input".into(),
                ),
            );
        }
        Ok(())
    }
}

pub fn throughput_evidence_surveillance_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["AURORA extension developer".into(),"preclinical researcher".into()].into(),behavior:"renders a deterministic prospective high-throughput EvidenceFeed3 workbench with queue, capacity, checkpoint, overflow, negative, unknown, qualified, and provenance panels without external effects".into(),value:"gives extension developers an accessible replayable view of bounded evidence admission while retaining overflow and omission receipts".into(),inputs:vec![TypedPort{name:"throughput_evidence_workbench_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"qualified_throughput_evidence_workbench_set".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"json-schema".into(),state:EvidenceState::Supported,locator:Some("https://json-schema.org/specification".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

pub fn render_throughput_evidence_surveillance_research_workbench(
    request: &ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    ThroughputEvidenceSurveillanceResearchWorkbenchReceipt,
    ThroughputEvidenceSurveillanceResearchWorkbenchError,
> {
    let receipt = build_throughput_evidence_surveillance_research_workbench(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn workbench_input_digest(
    request: &ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<ContentHash, ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    let canonical = canonical_throughput_evidence_surveillance_research_workbench_request(request);
    let value = serde_json::to_value(canonical).map_err(|e| {
        ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
    })?;
    ContentHash::of_value(&value)
        .map_err(|e| ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string()))
}

fn canonical_throughput_evidence_surveillance_research_workbench_request(
    request: &ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
) -> ThroughputEvidenceSurveillanceResearchWorkbenchRequest {
    let mut canonical = request.clone();
    canonical.copilot_request = canonical_throughput_evidence_surveillance_research_copilot_request(
        &canonical.copilot_request,
    );
    canonical
}

fn build_throughput_evidence_surveillance_research_workbench(
    request: &ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    ThroughputEvidenceSurveillanceResearchWorkbenchReceipt,
    ThroughputEvidenceSurveillanceResearchWorkbenchError,
> {
    validate_request(request)?;
    let c = run_throughput_evidence_surveillance_research_copilot(&request.copilot_request)
        .map_err(|e| {
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Copilot(e.to_string())
        })?;
    let views = VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let panels = PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let candidate = c.candidate_order.clone();
    let qualified = c.selected_order.clone();
    let unknown = c.unresolved_order.clone();
    let blocked = c.denied_order.clone();
    let overflow = c.overflow_order.clone();
    let copilot_run_digest = c.run_digest.clone();
    let queue_digest = c.queue_digest.clone();
    let checkpoint_digest = c.checkpoint_digest.clone();
    let workbench_digest = ContentHash::of_value(&json!({
        "workbench_id": request.workbench_id,
        "scope": request.scope,
        "batch_id": request.copilot_request.batch_id,
        "checkpoint_seq": request.copilot_request.checkpoint_seq,
        "capacity": request.copilot_request.capacity,
        "budget_units": request.budget_units,
        "policy_allow": request.copilot_request.policy_allow,
        "protected_closure": request.copilot_request.protected_closure,
        "disposition": c.disposition,
        "views": views,
        "panels": panels,
        "candidate": candidate,
        "qualified": qualified,
        "unknown": unknown,
        "blocked": blocked,
        "overflow": overflow,
        "replay_identity": request.replay_identity,
        "copilot_replay_identity": request.copilot_request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "queue_digest": queue_digest,
        "checkpoint_digest": checkpoint_digest,
    }))
    .map_err(|e| ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string()))?;
    let mut omissions = c.omissions.clone();
    omissions.push("workbench:read-only-throughput-view".into());
    omissions.sort();
    omissions.dedup();
    let effect_receipts = vec![format!(
        "view:throughput-evidence-workbench:{}",
        request.workbench_id
    )];
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.copilot_request.request_id,
        "workbench_id": request.workbench_id,
        "scope": request.scope,
        "batch_id": request.copilot_request.batch_id,
        "checkpoint_seq": request.copilot_request.checkpoint_seq,
        "capacity": request.copilot_request.capacity,
        "budget_units": request.budget_units,
        "policy_allow": request.copilot_request.policy_allow,
        "protected_closure": request.copilot_request.protected_closure,
        "disposition": c.disposition,
        "view_order": views,
        "panel_order": panels,
        "candidate_order": candidate,
        "qualified_order": qualified,
        "unknown_order": unknown,
        "blocked_order": blocked,
        "overflow_order": overflow,
        "replay_identity": request.replay_identity,
        "copilot_replay_identity": request.copilot_request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "workbench_digest": workbench_digest,
        "queue_digest": queue_digest,
        "checkpoint_digest": checkpoint_digest,
        "omissions": omissions,
        "uncertainty": c.uncertainty,
        "negative_evidence": c.negative_evidence,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": true,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-evidence-workbench:{}",
            request.workbench_id
        ),
        "application/vnd.aurora.throughput-evidence-workbench+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| ThroughputEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string()))?;
    let canonical_request =
        canonical_throughput_evidence_surveillance_research_workbench_request(request);
    let receipt = ThroughputEvidenceSurveillanceResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: workbench_input_digest(request)?,
        request_id: request.copilot_request.request_id.clone(),
        workbench_id: request.workbench_id.clone(),
        scope: request.scope.clone(),
        batch_id: request.copilot_request.batch_id.clone(),
        checkpoint_seq: request.copilot_request.checkpoint_seq,
        capacity: request.copilot_request.capacity,
        budget_units: request.budget_units,
        policy_allow: request.copilot_request.policy_allow,
        protected_closure: request.copilot_request.protected_closure,
        disposition: c.disposition,
        view_order: views,
        panel_order: panels,
        candidate_order: candidate,
        qualified_order: qualified,
        unknown_order: unknown,
        blocked_order: blocked,
        overflow_order: overflow,
        replay_identity: request.replay_identity.clone(),
        copilot_replay_identity: request.copilot_request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        queue_digest,
        checkpoint_digest,
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
    r: &ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<(), ThroughputEvidenceSurveillanceResearchWorkbenchError> {
    if r.budget_units == 0
        || u64::from(r.budget_units) > MAX_ITEMS as u64
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.copilot_request.boundary != PRECLINICAL_BOUNDARY
        || !r.copilot_request.raw_data_local
        || !r.copilot_request.dry_run
    {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "throughput workbench identity, budget, dry-run, locality, or boundary is invalid"
                    .into(),
            ),
        );
    }
    validate_text("workbench_id", &r.workbench_id)?;
    validate_text("scope", &r.scope)?;
    validate_text("boundary", &r.boundary)?;
    validate_text("copilot request_id", &r.copilot_request.request_id)?;
    validate_text("copilot batch_id", &r.copilot_request.batch_id)?;
    if r.scope != format!("batch:{}", r.copilot_request.batch_id) {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "throughput workbench scope must bind to its batch".into(),
            ),
        );
    }
    if r.requested_view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
        || r.requested_panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
    {
        return Err(
            ThroughputEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "throughput workbench views, panels, or replay identity is invalid".into(),
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
    use crate::throughput_evidence_surveillance_research_copilot::ThroughputCopilotEvidenceObservation;
    use bioprism_foundation::{EvidenceAvailability, EvidenceState};
    fn request() -> ThroughputEvidenceSurveillanceResearchWorkbenchRequest {
        let c = ThroughputEvidenceSurveillanceResearchCopilotRequest {
            request_id: "req-19".into(),
            agent_id: "extension-19".into(),
            batch_id: "batch-19".into(),
            checkpoint_seq: 1,
            capacity: 2,
            declared_tools: vec!["evidence.inspect".into()],
            requested_tool: "evidence.inspect".into(),
            max_tool_calls: 1,
            dry_run: true,
            approval_reference: None,
            approval_granted: false,
            observations: vec![ThroughputCopilotEvidenceObservation {
                source_id: "source-a".into(),
                sequence: 1,
                digest: Some(ContentHash::of_bytes(b"a")),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            }],
            min_relevance_score: 50,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(b"copilot-19"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ThroughputEvidenceSurveillanceResearchWorkbenchRequest {
            copilot_request: c,
            workbench_id: "wb-19".into(),
            scope: "batch:batch-19".into(),
            requested_view_order: VIEWS.iter().map(|v| (*v).to_string()).collect(),
            requested_panel_order: PANELS.iter().map(|v| (*v).to_string()).collect(),
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"wb-19"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_evidence_surveillance_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn renders_view() {
        let r = render_throughput_evidence_surveillance_research_workbench(&request()).unwrap();
        assert_eq!(r.feature_id, FEATURE_ID)
    }
    #[test]
    fn policy_denial_visible() {
        let mut r = request();
        r.copilot_request.policy_allow = false;
        let receipt = render_throughput_evidence_surveillance_research_workbench(&r).unwrap();
        assert!(receipt.qualified_order.is_empty());
        assert!(receipt.overflow_order.is_empty());
        assert_eq!(
            receipt.effect_receipts,
            vec!["view:throughput-evidence-workbench:wb-19"]
        );
    }
    #[test]
    fn rejects_non_dry_run() {
        let mut r = request();
        r.copilot_request.dry_run = false;
        assert!(render_throughput_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn rejects_panels() {
        let mut r = request();
        r.requested_panel_order.reverse();
        assert!(render_throughput_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn scope_must_bind_to_batch() {
        let mut r = request();
        r.scope = "batch:other".into();
        assert!(render_throughput_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn overflow_is_bound_to_capacity() {
        let mut r = request();
        r.copilot_request.observations.extend([
            ThroughputCopilotEvidenceObservation {
                source_id: "source-b".into(),
                sequence: 2,
                digest: Some(ContentHash::of_bytes(b"b")),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            },
            ThroughputCopilotEvidenceObservation {
                source_id: "source-c".into(),
                sequence: 3,
                digest: Some(ContentHash::of_bytes(b"c")),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            },
        ]);
        let receipt = render_throughput_evidence_surveillance_research_workbench(&r).unwrap();
        assert_eq!(receipt.overflow_order, vec!["source-c"]);
    }
    #[test]
    fn tampered_workbench_digest_is_rejected() {
        let mut receipt =
            render_throughput_evidence_surveillance_research_workbench(&request()).unwrap();
        receipt.workbench_digest = ContentHash::of_bytes(b"tampered-workbench");
        assert!(receipt.validate().is_err())
    }
    #[test]
    fn tampered_queue_digest_is_rejected() {
        let mut receipt =
            render_throughput_evidence_surveillance_research_workbench(&request()).unwrap();
        receipt.queue_digest = ContentHash::of_bytes(b"tampered-queue");
        assert!(receipt.validate().is_err())
    }
    #[test]
    fn replay_stable() {
        let r = request();
        assert_eq!(
            render_throughput_evidence_surveillance_research_workbench(&r).unwrap(),
            render_throughput_evidence_surveillance_research_workbench(&r).unwrap()
        )
    }

    #[test]
    fn reordered_nested_copilot_input_has_stable_identity() {
        let mut reordered = request();
        reordered.copilot_request.declared_tools.reverse();
        reordered.copilot_request.observations.reverse();
        let first = render_throughput_evidence_surveillance_research_workbench(&request()).unwrap();
        let second =
            render_throughput_evidence_surveillance_research_workbench(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workbench_digest, second.workbench_digest);
    }

    #[test]
    fn receipt_rejects_tampered_retained_queue_request() {
        let mut receipt =
            render_throughput_evidence_surveillance_research_workbench(&request()).unwrap();
        receipt.input.scope = "batch:tampered".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
