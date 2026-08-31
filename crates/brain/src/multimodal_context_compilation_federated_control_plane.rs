//! Multimodal multi-study operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P03-F30`. Study×modality closure and semantic
//! comparability are operational gates, not inferred scientific conclusions.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F30";
pub const CONTRACT_VERSION: &str =
    "brain-multimodal-context-compilation-federated-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextControlCell {
    pub study_id: String,
    pub modality: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub comparable: bool,
    pub ready: bool,
    pub retry_count: u16,
    pub telemetry_digest: Option<ContentHash>,
    pub cost_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextControlRequest {
    pub request_id: String,
    pub workspace_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub goal: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cells: Vec<MultimodalContextControlCell>,
    pub max_retries: u16,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub signed_approval: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalContextControlDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub goal: String,
    pub disposition: MultimodalContextControlDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cell_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub exchange_order: Vec<ContentHash>,
    pub checkpoint_seq: u64,
    pub retry_count: u64,
    pub consumed_budget_units: u32,
    pub run_digest: ContentHash,
    pub telemetry_digest: ContentHash,
    pub federation_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalContextControlError {
    #[error("invalid multimodal context control request: {0}")]
    Invalid(String),
    #[error("multimodal context control artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalContextControlReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContextControlError> {
        let cell_count = u64::try_from(self.cell_order.len()).map_err(|_| {
            MultimodalContextControlError::Invalid(
                "multimodal cell count exceeds checkpoint sequence width".into(),
            )
        })?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.cell_order.is_empty()
            || self.checkpoint_seq != cell_count
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalContextControlError::Invalid("multimodal control identity, matrix closure, checkpoint, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.cell_order,
            &self.completed_order,
            &self.degraded_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.incomparable_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalContextControlError::Invalid(
                    "multimodal control ordering is not canonical".into(),
                ));
            }
        }
        if self
            .exchange_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(MultimodalContextControlError::Invalid(
                "multimodal control exchange ordering is not canonical".into(),
            ));
        }
        let classified = self
            .completed_order
            .iter()
            .chain(self.degraded_order.iter())
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.cell_order.len()
            || classified
                .iter()
                .any(|cell| !self.cell_order.contains(cell))
        {
            return Err(MultimodalContextControlError::Invalid(
                "multimodal control dispositions do not partition cells".into(),
            ));
        }
        if self.exchange_order.len() != self.completed_order.len() {
            return Err(MultimodalContextControlError::Invalid(
                "multimodal control exchange does not match completed cells".into(),
            ));
        }
        for digest in self.exchange_order.iter().chain([
            &self.run_digest,
            &self.telemetry_digest,
            &self.federation_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(MultimodalContextControlError::Invalid(
                    "multimodal control digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-multimodal-summary:")
                && !effect.starts_with("manage:multimodal-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalContextControlError::Invalid(
                "multimodal control effect is outside the governed operations gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalContextControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))
    }
}

pub fn multimodal_context_compilation_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "multimodal workflow operator".into(), "federation administrator".into()].into(), behavior: "operates multimodal multi-study context compilation with matrix closure, comparability, checkpoints, retries, telemetry, budgets, and permitted summary exchange".into(), value: "prevents incomplete or incomparable imaging and omics operations from being presented as a completed research workflow".into(), inputs: vec![TypedPort { name: "multimodal_context_control_request".into(), schema: "MultimodalContextControlRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_context_control_receipt".into(), schema: "MultimodalContextControlResponse1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["operate:institution-node".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-0.5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_multimodal_context_compilation(
    request: &MultimodalContextControlRequest,
) -> Result<MultimodalContextControlReceipt, MultimodalContextControlError> {
    if request.request_id.trim().is_empty()
        || request.workspace_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.study_order.len() < 2
        || request.modality_order.len() < 2
        || request.cells.is_empty()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalContextControlError::Invalid(
            "multimodal control identity, matrix, budget, replay, or boundary is invalid".into(),
        ));
    }
    let studies = request.study_order.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if studies.len() != request.study_order.len()
        || modalities.len() != request.modality_order.len()
        || studies.iter().any(|value| value.trim().is_empty())
        || modalities.iter().any(|value| value.trim().is_empty())
    {
        return Err(MultimodalContextControlError::Invalid(
            "study and modality identifiers must be unique and non-empty".into(),
        ));
    }
    let cell_order = studies
        .iter()
        .flat_map(|study| {
            modalities
                .iter()
                .map(move |modality| format!("{}|{}", study, modality))
        })
        .collect::<Vec<_>>();
    let checkpoint_seq = u64::try_from(cell_order.len()).map_err(|_| {
        MultimodalContextControlError::Invalid(
            "multimodal cell count exceeds checkpoint sequence width".into(),
        )
    })?;
    let mut cell_map = BTreeMap::new();
    for cell in &request.cells {
        let key = format!("{}|{}", cell.study_id, cell.modality);
        if cell_map.insert(key, cell).is_some() {
            return Err(MultimodalContextControlError::Invalid(
                "multimodal control cells must be unique".into(),
            ));
        }
    }
    let mut completed = BTreeSet::new();
    let mut degraded = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut exchanges = Vec::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-multimodal-control-contract".to_string(),
        "gate:study-modality-closure".to_string(),
        "gate:comparability".to_string(),
        "gate:checkpoint".to_string(),
        "gate:bounded-retry".to_string(),
        "gate:telemetry".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.signed_approval;
    let mut consumed = 0u32;
    let mut retries = 0u64;
    for key in &cell_order {
        let Some(cell) = cell_map.get(key) else {
            unresolved.insert(key.clone());
            omissions.insert(format!("cell:{}:missing-checkpoint", key));
            continue;
        };
        retries = retries.saturating_add(u64::from(cell.retry_count));
        if !global_open || !cell.raw_data_local || cell.boundary != PRECLINICAL_BOUNDARY {
            denied.insert(key.clone());
            counterexamples.insert(format!("counterexample:{}:policy-approval-locality", key));
        } else if !cell.comparable {
            denied.insert(key.clone());
            incomparable.insert(key.clone());
            negative.insert(format!("cell:{}:incomparable", key));
        } else if cell.retry_count > request.max_retries {
            degraded.insert(key.clone());
            omissions.insert(format!("cell:{}:retry-budget-exhausted", key));
        } else if consumed.saturating_add(cell.cost_units) > request.budget_units {
            denied.insert(key.clone());
            omissions.insert(format!("cell:{}:resource-budget-exhausted", key));
        } else if !cell.ready {
            unresolved.insert(key.clone());
            uncertainty.insert(format!("cell:{}:not-ready", key));
        } else if cell.replay_identity != request.replay_identity {
            unresolved.insert(key.clone());
            uncertainty.insert(format!("cell:{}:replay-mismatch", key));
        } else if cell.telemetry_digest.is_none() {
            unresolved.insert(key.clone());
            omissions.insert(format!("cell:{}:telemetry-missing", key));
        } else if cell.evidence_digest.is_none() || cell.provenance_digest.is_none() {
            unresolved.insert(key.clone());
            omissions.insert(format!("cell:{}:evidence-or-provenance-missing", key));
        } else if matches!(
            cell.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(key.clone());
            uncertainty.insert(format!("cell:{}:evidence-uncertain", key));
        } else if matches!(cell.state, EvidenceState::Contradicted) {
            denied.insert(key.clone());
            negative.insert(format!("cell:{}:contradicted", key));
        } else {
            completed.insert(key.clone());
            consumed = consumed.saturating_add(cell.cost_units);
            exchanges.push(ContentHash::of_value(&json!({"cell_id": key, "context_digest": cell.context_digest, "section_digest": cell.section_digest, "evidence_digest": cell.evidence_digest, "provenance_digest": cell.provenance_digest, "telemetry_digest": cell.telemetry_digest})).map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?);
        }
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        counterexamples.insert("counterexample:signed-approval-missing".into());
        omissions.insert("control:signed-approval-missing".into());
    }
    if !unresolved.is_empty() || !degraded.is_empty() {
        witnesses.insert("gate:degraded-or-unresolved-retained".into());
    }
    exchanges.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let disposition = if !global_open || !denied.is_empty() {
        MultimodalContextControlDisposition::Denied
    } else if !unresolved.is_empty() {
        MultimodalContextControlDisposition::Unresolved
    } else if !degraded.is_empty() {
        MultimodalContextControlDisposition::Degraded
    } else {
        MultimodalContextControlDisposition::Completed
    };
    let telemetry = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "workflow_id": request.workflow_id, "cell_order": cell_order, "retry_count": retries, "exchange_order": exchanges})).map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?;
    let raw_data_local = true;
    let federation = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "workflow_id": request.workflow_id, "exchange_order": exchanges, "raw_data_local": raw_data_local, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?;
    let run = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "disposition": disposition, "completed_order": completed, "degraded_order": degraded, "unresolved_order": unresolved, "denied_order": denied, "checkpoint_seq": checkpoint_seq, "consumed_budget_units": consumed, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workspace_id": request.workspace_id, "workflow_id": request.workflow_id, "scope": request.scope, "goal": request.goal, "disposition": disposition, "study_order": studies, "modality_order": modalities, "cell_order": cell_order, "completed_order": completed, "degraded_order": degraded, "unresolved_order": unresolved, "denied_order": denied, "incomparable_order": incomparable, "exchange_order": exchanges, "checkpoint_seq": checkpoint_seq, "retry_count": retries, "consumed_budget_units": consumed, "run_digest": run, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity, "witness_order": witnesses, "counterexample_order": counterexamples, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-context-compilation-federated-control-plane:{}",
            request.request_id
        ),
        "application/vnd.aurora.multimodal-context-control+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContextControlError::Artifact(error.to_string()))?;
    let receipt = MultimodalContextControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        goal: request.goal.clone(),
        disposition,
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        cell_order,
        completed_order: completed.into_iter().collect(),
        degraded_order: degraded.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        incomparable_order: incomparable.into_iter().collect(),
        exchange_order: exchanges,
        checkpoint_seq,
        retry_count: retries,
        consumed_budget_units: consumed,
        run_digest: run,
        telemetry_digest: telemetry,
        federation_digest: federation,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(disposition, MultimodalContextControlDisposition::Completed) {
            vec![
                format!(
                    "exchange:permitted-multimodal-summary:{}",
                    request.request_id
                ),
                format!("manage:multimodal-context:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalContextControlRequest {
        let replay = hash("multimodal-control");
        let cell = |study: &str, modality: &str| MultimodalContextControlCell {
            study_id: study.into(),
            modality: modality.into(),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            evidence_digest: Some(replay.clone()),
            provenance_digest: Some(replay.clone()),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            comparable: true,
            ready: true,
            retry_count: 0,
            telemetry_digest: Some(replay.clone()),
            cost_units: 1,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        MultimodalContextControlRequest {
            request_id: "request:multimodal-control".into(),
            workspace_id: "workspace:alpha".into(),
            workflow_id: "workflow:multimodal-context".into(),
            scope: "organoid:neural-circuit".into(),
            goal: "operate-multimodal-context".into(),
            study_order: vec!["study:a".into(), "study:b".into()],
            modality_order: vec!["imaging".into(), "omics".into()],
            cells: vec![
                cell("study:a", "imaging"),
                cell("study:a", "omics"),
                cell("study:b", "imaging"),
                cell("study:b", "omics"),
            ],
            max_retries: 2,
            budget_units: 4,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            multimodal_context_compilation_federated_control_plane_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn complete_is_completed() {
        assert_eq!(
            operate_multimodal_context_compilation(&request())
                .unwrap()
                .disposition,
            MultimodalContextControlDisposition::Completed
        );
    }
    #[test]
    fn missing_cell_is_unresolved() {
        let mut value = request();
        value.cells.pop();
        assert_eq!(
            operate_multimodal_context_compilation(&value)
                .unwrap()
                .disposition,
            MultimodalContextControlDisposition::Unresolved
        );
    }
    #[test]
    fn incomparable_is_denied() {
        let mut value = request();
        value.cells[0].comparable = false;
        assert_eq!(
            operate_multimodal_context_compilation(&value)
                .unwrap()
                .disposition,
            MultimodalContextControlDisposition::Denied
        );
    }
    #[test]
    fn retry_is_degraded() {
        let mut value = request();
        value.cells[0].retry_count = 3;
        assert_eq!(
            operate_multimodal_context_compilation(&value)
                .unwrap()
                .disposition,
            MultimodalContextControlDisposition::Degraded
        );
    }
    #[test]
    fn policy_is_denied() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            operate_multimodal_context_compilation(&value)
                .unwrap()
                .disposition,
            MultimodalContextControlDisposition::Denied
        );
    }
    #[test]
    fn non_local_input_returns_denied_metadata_receipt() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = operate_multimodal_context_compilation(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalContextControlDisposition::Denied
        );
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = operate_multimodal_context_compilation(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
