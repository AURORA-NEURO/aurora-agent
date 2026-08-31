//! Institution-local operations and federated control plane for context compilation.
//!
//! Atlas feature: `AFA-brain-P03-F29`. Operational completion is deliberately
//! separate from scientific qualification: checkpoints, retries, telemetry,
//! recovery dispositions, and permitted-summary exchange are all typed.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F29";
pub const CONTRACT_VERSION: &str = "brain-local-context-compilation-federated-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContextControlStage {
    pub stage_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub ready: bool,
    pub retry_count: u16,
    pub telemetry_digest: Option<ContentHash>,
    pub cost_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContextControlRequest {
    pub request_id: String,
    pub node_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub goal: String,
    pub stage_order: Vec<String>,
    pub stages: Vec<LocalContextControlStage>,
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
pub enum LocalContextControlDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContextControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub node_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub goal: String,
    pub disposition: LocalContextControlDisposition,
    pub stage_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
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
pub enum LocalContextControlError {
    #[error("invalid local context control request: {0}")]
    Invalid(String),
    #[error("local context control artifact failed: {0}")]
    Artifact(String),
}

impl LocalContextControlReceipt {
    pub fn validate(&self) -> Result<(), LocalContextControlError> {
        let stage_count = u64::try_from(self.stage_order.len()).map_err(|_| {
            LocalContextControlError::Invalid(
                "local control stage count exceeds checkpoint sequence width".into(),
            )
        })?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.node_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.stage_order.is_empty()
            || self.checkpoint_seq != stage_count
            || self.effect_receipts.is_empty()
        {
            return Err(LocalContextControlError::Invalid(
                "local control identity, checkpoint, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.stage_order,
            &self.completed_order,
            &self.degraded_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(LocalContextControlError::Invalid(
                    "local control ordering is not canonical".into(),
                ));
            }
        }
        if self
            .exchange_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(LocalContextControlError::Invalid(
                "local control exchange ordering is not canonical".into(),
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
        if classified.len() != self.stage_order.len()
            || classified
                .iter()
                .any(|stage| !self.stage_order.contains(stage))
        {
            return Err(LocalContextControlError::Invalid(
                "local control dispositions do not partition stages".into(),
            ));
        }
        if self.exchange_order.len() != self.completed_order.len() {
            return Err(LocalContextControlError::Invalid(
                "local control exchange or retry accounting is invalid".into(),
            ));
        }
        for digest in self.exchange_order.iter().chain([
            &self.run_digest,
            &self.telemetry_digest,
            &self.federation_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(LocalContextControlError::Invalid(
                    "local control digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-context-summary:")
                && !effect.starts_with("manage:local-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(LocalContextControlError::Invalid(
                "local control effect is outside the governed operations gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LocalContextControlError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, LocalContextControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| LocalContextControlError::Artifact(error.to_string()))
    }
}

pub fn local_context_compilation_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: [
            "platform reliability engineer".into(),
            "institution node operator".into(),
            "federation administrator".into(),
        ]
        .into(),
        behavior: "operates institution-local context compilation with durable checkpoints, bounded retries, telemetry, recovery dispositions, and permitted summary exchange".into(),
        value: "makes operational progress and failure auditable without upgrading execution completion into a scientific conclusion".into(),
        inputs: vec![TypedPort {
            name: "local_context_control_request".into(),
            schema: "LocalContextControlRequest1@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "local_context_control_receipt".into(),
            schema: "LocalContextControlResponse1@1".into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: ["operate:institution-node".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "opentelemetry".into(),
            state: EvidenceState::Supported,
            locator: Some("https://opentelemetry.io/docs/specs/".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_local_context_compilation(
    request: &LocalContextControlRequest,
) -> Result<LocalContextControlReceipt, LocalContextControlError> {
    if request.request_id.trim().is_empty()
        || request.node_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.stage_order.is_empty()
        || request.stages.is_empty()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LocalContextControlError::Invalid(
            "local control identity, stages, budget, replay, or boundary is invalid".into(),
        ));
    }
    let stage_order = request.stage_order.iter().cloned().collect::<BTreeSet<_>>();
    if stage_order.len() != request.stage_order.len()
        || stage_order.iter().any(|stage| stage.trim().is_empty())
    {
        return Err(LocalContextControlError::Invalid(
            "local control stage identifiers must be unique and non-empty".into(),
        ));
    }
    let mut stage_map = std::collections::BTreeMap::new();
    for stage in &request.stages {
        if stage_map.insert(stage.stage_id.clone(), stage).is_some() {
            return Err(LocalContextControlError::Invalid(
                "local control stages must be unique".into(),
            ));
        }
    }
    let ordered = stage_order.iter().cloned().collect::<Vec<_>>();
    let checkpoint_seq = u64::try_from(ordered.len()).map_err(|_| {
        LocalContextControlError::Invalid(
            "local control stage count exceeds checkpoint sequence width".into(),
        )
    })?;
    let mut completed = BTreeSet::new();
    let mut degraded = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut exchange_order = Vec::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-local-control-contract".to_string(),
        "gate:checkpoint".to_string(),
        "gate:bounded-retry".to_string(),
        "gate:telemetry".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:permitted-summary".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.signed_approval;
    let mut consumed_budget = 0u32;
    let mut retries = 0u64;
    for stage_id in &ordered {
        let Some(stage) = stage_map.get(stage_id) else {
            unresolved.insert(stage_id.clone());
            omissions.insert(format!("stage:{}:missing-checkpoint", stage_id));
            continue;
        };
        retries = retries.saturating_add(u64::from(stage.retry_count));
        if !global_open || !stage.raw_data_local || stage.boundary != PRECLINICAL_BOUNDARY {
            denied.insert(stage_id.clone());
            counterexamples.insert(format!(
                "counterexample:{}:policy-approval-locality",
                stage_id
            ));
        } else if stage.retry_count > request.max_retries {
            degraded.insert(stage_id.clone());
            omissions.insert(format!("stage:{}:retry-budget-exhausted", stage_id));
        } else if consumed_budget.saturating_add(stage.cost_units) > request.budget_units {
            denied.insert(stage_id.clone());
            omissions.insert(format!("stage:{}:resource-budget-exhausted", stage_id));
        } else if !stage.ready {
            unresolved.insert(stage_id.clone());
            uncertainty.insert(format!("stage:{}:not-ready", stage_id));
        } else if stage.replay_identity != request.replay_identity {
            unresolved.insert(stage_id.clone());
            uncertainty.insert(format!("stage:{}:replay-mismatch", stage_id));
        } else if stage.telemetry_digest.is_none() {
            unresolved.insert(stage_id.clone());
            omissions.insert(format!("stage:{}:telemetry-missing", stage_id));
        } else if stage.evidence_digest.is_none() || stage.provenance_digest.is_none() {
            unresolved.insert(stage_id.clone());
            omissions.insert(format!("stage:{}:evidence-or-provenance-missing", stage_id));
        } else if matches!(
            stage.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(stage_id.clone());
            uncertainty.insert(format!("stage:{}:evidence-uncertain", stage_id));
        } else if matches!(stage.state, EvidenceState::Contradicted) {
            denied.insert(stage_id.clone());
            negative.insert(format!("stage:{}:contradicted", stage_id));
        } else {
            completed.insert(stage_id.clone());
            consumed_budget = consumed_budget.saturating_add(stage.cost_units);
            exchange_order.push(
                ContentHash::of_value(&json!({
                    "stage_id": stage.stage_id,
                    "context_digest": stage.context_digest,
                    "section_digest": stage.section_digest,
                    "evidence_digest": stage.evidence_digest,
                    "provenance_digest": stage.provenance_digest,
                    "telemetry_digest": stage.telemetry_digest,
                }))
                .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?,
            );
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
    exchange_order.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let disposition = if !global_open || !denied.is_empty() {
        LocalContextControlDisposition::Denied
    } else if !unresolved.is_empty() {
        LocalContextControlDisposition::Unresolved
    } else if !degraded.is_empty() {
        LocalContextControlDisposition::Degraded
    } else {
        LocalContextControlDisposition::Completed
    };
    let telemetry_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "workflow_id": request.workflow_id,
        "stage_order": ordered,
        "retry_count": retries,
        "exchange_order": exchange_order,
    }))
    .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?;
    let raw_data_local = true;
    let federation_digest = ContentHash::of_value(&json!({
        "node_id": request.node_id,
        "workflow_id": request.workflow_id,
        "exchange_order": exchange_order,
        "raw_data_local": raw_data_local,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?;
    let run_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "disposition": disposition,
        "completed_order": completed,
        "degraded_order": degraded,
        "unresolved_order": unresolved,
        "denied_order": denied,
        "checkpoint_seq": checkpoint_seq,
        "consumed_budget_units": consumed_budget,
        "telemetry_digest": telemetry_digest,
        "federation_digest": federation_digest,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "node_id": request.node_id,
        "workflow_id": request.workflow_id,
        "scope": request.scope,
        "goal": request.goal,
        "disposition": disposition,
        "stage_order": ordered,
        "completed_order": completed,
        "degraded_order": degraded,
        "unresolved_order": unresolved,
        "denied_order": denied,
        "exchange_order": exchange_order,
        "checkpoint_seq": checkpoint_seq,
        "retry_count": retries,
        "consumed_budget_units": consumed_budget,
        "run_digest": run_digest,
        "telemetry_digest": telemetry_digest,
        "federation_digest": federation_digest,
        "replay_identity": request.replay_identity,
        "witness_order": witnesses,
        "counterexample_order": counterexamples,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-local-context-compilation-federated-control-plane:{}",
            request.request_id
        ),
        "application/vnd.aurora.local-context-control+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LocalContextControlError::Artifact(error.to_string()))?;
    let receipt = LocalContextControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        node_id: request.node_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        goal: request.goal.clone(),
        disposition,
        stage_order: ordered.clone(),
        completed_order: completed.into_iter().collect(),
        degraded_order: degraded.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        exchange_order,
        checkpoint_seq,
        retry_count: retries,
        consumed_budget_units: consumed_budget,
        run_digest,
        telemetry_digest,
        federation_digest,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(disposition, LocalContextControlDisposition::Completed) {
            vec![
                format!("exchange:permitted-context-summary:{}", request.request_id),
                format!("manage:local-context:{}", request.request_id),
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

    fn request() -> LocalContextControlRequest {
        let replay = hash("local-context-control");
        let stage = |id: &str| LocalContextControlStage {
            stage_id: id.into(),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            evidence_digest: Some(replay.clone()),
            provenance_digest: Some(replay.clone()),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            ready: true,
            retry_count: 0,
            telemetry_digest: Some(replay.clone()),
            cost_units: 1,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        LocalContextControlRequest {
            request_id: "request:local-context-control".into(),
            node_id: "node:alpha".into(),
            workflow_id: "workflow:context".into(),
            scope: "organoid:neural-circuit".into(),
            goal: "compile-context".into(),
            stage_order: vec!["stage:a".into(), "stage:b".into()],
            stages: vec![stage("stage:a"), stage("stage:b")],
            max_retries: 2,
            budget_units: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            local_context_compilation_federated_control_plane_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_is_completed() {
        assert_eq!(
            operate_local_context_compilation(&request())
                .unwrap()
                .disposition,
            LocalContextControlDisposition::Completed
        );
    }
    #[test]
    fn not_ready_is_unresolved() {
        let mut value = request();
        value.stages[0].ready = false;
        assert_eq!(
            operate_local_context_compilation(&value)
                .unwrap()
                .disposition,
            LocalContextControlDisposition::Unresolved
        );
    }
    #[test]
    fn retry_limit_is_degraded() {
        let mut value = request();
        value.stages[0].retry_count = 3;
        assert_eq!(
            operate_local_context_compilation(&value)
                .unwrap()
                .disposition,
            LocalContextControlDisposition::Degraded
        );
    }
    #[test]
    fn budget_is_denied() {
        let mut value = request();
        value.budget_units = 1;
        assert_eq!(
            operate_local_context_compilation(&value)
                .unwrap()
                .disposition,
            LocalContextControlDisposition::Denied
        );
    }
    #[test]
    fn policy_is_denied() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            operate_local_context_compilation(&value)
                .unwrap()
                .disposition,
            LocalContextControlDisposition::Denied
        );
    }
    #[test]
    fn non_local_input_returns_denied_metadata_receipt() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = operate_local_context_compilation(&value).unwrap();
        assert_eq!(receipt.disposition, LocalContextControlDisposition::Denied);
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = operate_local_context_compilation(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
