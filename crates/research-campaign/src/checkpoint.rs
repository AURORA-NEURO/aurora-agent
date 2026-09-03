use crate::adapters::{ReceiptProjection, VerifiedCampaignReceipt};
use crate::coordination::CampaignCheckpointHead;
use crate::error::{invalid_checkpoint, CampaignError};
use crate::kernel::{
    authorization_digest, digest_value, empty_event_chain_digest, ActiveAction, CampaignEvent,
    CampaignEventKind, ResearchCampaign, StageProgress,
};
use crate::model::{
    CampaignActionKind, CampaignReceiptDisposition, CampaignStatus, ResearchCampaignSpec,
    MAX_CAMPAIGN_ACTIONS, MAX_CAMPAIGN_EVENTS, MAX_CAMPAIGN_ID_BYTES, MAX_CAMPAIGN_STAGES,
    MAX_STAGE_DEPENDENCIES, MAX_STAGE_ID_BYTES,
};
use bioprism_ids::{to_canonical_string, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA: &str = "bioprism-research-campaign-checkpoint/0.1";
pub const RESEARCH_CAMPAIGN_CHECKPOINT_RETENTION: &str =
    "metadata_only_campaign;objectives_prompts_arguments_artifacts_provider_output_evidence_credentials_not_retained";
pub const MAX_CAMPAIGN_CHECKPOINT_BYTES: usize = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StageCheckpointState {
    Pending,
    InFlight,
    Uncertain,
    Settled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageCheckpoint {
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    depends_on: Vec<String>,
    state: StageCheckpointState,
    action_ordinal: Option<u16>,
    authorization_digest: Option<String>,
    receipt: Option<ReceiptProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointEnvelope {
    schema: String,
    campaign_id: String,
    spec_digest: String,
    generation: u64,
    previous_snapshot_digest: Option<String>,
    status: CampaignStatus,
    max_actions: u16,
    actions_used: u16,
    stages: Vec<StageCheckpoint>,
    active_stage_id: Option<String>,
    events: Vec<CampaignEvent>,
    event_chain_digest: String,
    retention: String,
    secret_material: String,
    snapshot_digest: String,
}

/// Strictly validated checkpoint. Construction is available only through sealing or validation.
#[derive(Debug, Clone)]
pub struct ValidatedCampaignCheckpoint {
    envelope: CheckpointEnvelope,
    value: Value,
}

impl ValidatedCampaignCheckpoint {
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub fn snapshot_digest(&self) -> &str {
        &self.envelope.snapshot_digest
    }

    pub fn generation(&self) -> u64 {
        self.envelope.generation
    }

    /// Durable head callers keep independently from checkpoint payloads to reject rollback.
    pub fn head(&self) -> CampaignCheckpointHead {
        CampaignCheckpointHead::new(
            self.envelope.campaign_id.clone(),
            self.envelope.spec_digest.clone(),
            self.envelope.generation,
            self.envelope.snapshot_digest.clone(),
        )
    }
}

/// Seal the current state at the next contiguous generation and advance its persistence cursor.
pub fn seal_campaign_checkpoint(
    campaign: &mut ResearchCampaign,
) -> Result<ValidatedCampaignCheckpoint, CampaignError> {
    let generation = campaign
        .last_generation
        .checked_add(1)
        .ok_or_else(|| invalid_checkpoint("checkpoint generation overflowed"))?;
    let stages = campaign
        .spec
        .stages()
        .map(|stage| {
            let progress = campaign
                .stage_states
                .get(stage.stage_id())
                .expect("every validated stage has runtime state");
            let (state, action_ordinal, authorization_digest, receipt) = match progress {
                StageProgress::Pending => (StageCheckpointState::Pending, None, None, None),
                StageProgress::InFlight {
                    action_ordinal,
                    authorization_digest,
                } => (
                    StageCheckpointState::InFlight,
                    Some(*action_ordinal),
                    Some(authorization_digest.clone()),
                    None,
                ),
                StageProgress::Uncertain {
                    action_ordinal,
                    authorization_digest,
                    observation,
                } => (
                    StageCheckpointState::Uncertain,
                    Some(*action_ordinal),
                    Some(authorization_digest.clone()),
                    Some(observation.clone()),
                ),
                StageProgress::Settled {
                    action_ordinal,
                    authorization_digest,
                    receipt,
                } => (
                    StageCheckpointState::Settled,
                    Some(*action_ordinal),
                    Some(authorization_digest.clone()),
                    Some(receipt.projection().clone()),
                ),
            };
            StageCheckpoint {
                stage_id: stage.stage_id().to_owned(),
                kind: stage.kind(),
                input_digest: stage.input_digest().to_owned(),
                depends_on: stage.depends_on().to_vec(),
                state,
                action_ordinal,
                authorization_digest,
                receipt,
            }
        })
        .collect();
    let envelope = CheckpointEnvelope {
        schema: RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA.to_owned(),
        campaign_id: campaign.spec.campaign_id().to_owned(),
        spec_digest: campaign.spec.spec_digest().to_owned(),
        generation,
        previous_snapshot_digest: campaign.last_snapshot_digest.clone(),
        status: campaign.status,
        max_actions: campaign.spec.max_actions(),
        actions_used: campaign.actions_used,
        stages,
        active_stage_id: campaign
            .active
            .as_ref()
            .map(|active| active.stage_id.clone()),
        events: campaign.events.clone(),
        event_chain_digest: campaign.event_chain_digest.clone(),
        retention: RESEARCH_CAMPAIGN_CHECKPOINT_RETENTION.to_owned(),
        secret_material: "never_returned".to_owned(),
        snapshot_digest: String::new(),
    };
    let mut value =
        serde_json::to_value(envelope).map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })?;
    value
        .as_object_mut()
        .expect("checkpoint envelope serializes as an object")
        .remove("snapshot_digest");
    let snapshot_digest = digest_value(&value)?;
    value
        .as_object_mut()
        .expect("checkpoint body is an object")
        .insert(
            "snapshot_digest".to_owned(),
            Value::String(snapshot_digest.clone()),
        );
    let checkpoint = validate_campaign_checkpoint(&value)?;
    campaign.last_generation = generation;
    campaign.last_snapshot_digest = Some(snapshot_digest);
    Ok(checkpoint)
}

/// Validate exact schema, bounds, DAG, transition replay, event chain, and snapshot digest.
pub fn validate_campaign_checkpoint(
    value: &Value,
) -> Result<ValidatedCampaignCheckpoint, CampaignError> {
    let canonical =
        to_canonical_string(value).map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })?;
    if canonical.len() > MAX_CAMPAIGN_CHECKPOINT_BYTES {
        return Err(invalid_checkpoint("checkpoint exceeds its byte ceiling"));
    }
    let envelope: CheckpointEnvelope = serde_json::from_value(value.clone()).map_err(|error| {
        invalid_checkpoint(format!(
            "checkpoint does not match the exact schema: {error}"
        ))
    })?;
    if envelope.schema != RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA
        || envelope.retention != RESEARCH_CAMPAIGN_CHECKPOINT_RETENTION
        || envelope.secret_material != "never_returned"
    {
        return Err(invalid_checkpoint(
            "schema or metadata-retention markers are invalid",
        ));
    }
    bounded_text(&envelope.campaign_id, "campaign_id", MAX_CAMPAIGN_ID_BYTES)?;
    require_digest(&envelope.spec_digest, "spec_digest")?;
    require_digest(&envelope.event_chain_digest, "event_chain_digest")?;
    require_digest(&envelope.snapshot_digest, "snapshot_digest")?;
    if envelope.generation == 0
        || (envelope.generation == 1 && envelope.previous_snapshot_digest.is_some())
        || (envelope.generation > 1 && envelope.previous_snapshot_digest.is_none())
    {
        return Err(invalid_checkpoint(
            "checkpoint generation and predecessor presence are inconsistent",
        ));
    }
    if let Some(previous) = &envelope.previous_snapshot_digest {
        require_digest(previous, "previous_snapshot_digest")?;
    }
    if envelope.stages.is_empty()
        || envelope.stages.len() > MAX_CAMPAIGN_STAGES
        || envelope.max_actions == 0
        || envelope.max_actions > MAX_CAMPAIGN_ACTIONS
        || usize::from(envelope.max_actions) < envelope.stages.len()
        || envelope.actions_used > envelope.max_actions
    {
        return Err(invalid_checkpoint(
            "stage or action accounting is outside its ceiling",
        ));
    }

    let mut stage_by_id = BTreeMap::new();
    for stage in &envelope.stages {
        bounded_text(&stage.stage_id, "stage_id", MAX_STAGE_ID_BYTES)?;
        require_digest(&stage.input_digest, "stage input_digest")?;
        if stage.depends_on.len() > MAX_STAGE_DEPENDENCIES {
            return Err(invalid_checkpoint(
                "stage dependency count exceeds its ceiling",
            ));
        }
        let mut dependencies = BTreeSet::new();
        for dependency in &stage.depends_on {
            bounded_text(dependency, "dependency", MAX_STAGE_ID_BYTES)?;
            if dependency == &stage.stage_id || !dependencies.insert(dependency.clone()) {
                return Err(invalid_checkpoint(
                    "stage has a self-dependency or duplicate dependency",
                ));
            }
        }
        if stage_by_id.insert(stage.stage_id.clone(), stage).is_some() {
            return Err(invalid_checkpoint("stage identifiers are not unique"));
        }
        validate_stage_projection(stage)?;
    }
    for stage in &envelope.stages {
        if stage
            .depends_on
            .iter()
            .any(|dependency| !stage_by_id.contains_key(dependency))
        {
            return Err(invalid_checkpoint("stage depends on an unknown stage"));
        }
    }
    let expected_order = checkpoint_topological_order(&envelope.stages)?;
    if envelope
        .stages
        .iter()
        .map(|stage| stage.stage_id.as_str())
        .ne(expected_order.iter().map(String::as_str))
    {
        return Err(invalid_checkpoint(
            "checkpoint stages are not in canonical topological order",
        ));
    }

    replay_and_validate(&envelope, &stage_by_id)?;

    let mut without_digest = value.clone();
    without_digest
        .as_object_mut()
        .ok_or_else(|| invalid_checkpoint("checkpoint must be an object"))?
        .remove("snapshot_digest");
    if digest_value(&without_digest)? != envelope.snapshot_digest {
        return Err(invalid_checkpoint(
            "snapshot_digest does not match the checkpoint body",
        ));
    }
    Ok(ValidatedCampaignCheckpoint {
        envelope,
        value: value.clone(),
    })
}

/// Restore only when the checkpoint matches a separately retained trusted head and every settled
/// private artifact has been reverified into a receipt. An in-flight checkpoint is fenced as
/// `ReconciliationRequired`; a ready restored campaign must acquire the caller's atomic
/// authorization claim before dispatch.
pub fn restore_campaign(
    spec: ResearchCampaignSpec,
    checkpoint: &ValidatedCampaignCheckpoint,
    trusted_head: &CampaignCheckpointHead,
    receipts: Vec<VerifiedCampaignReceipt>,
) -> Result<ResearchCampaign, CampaignError> {
    let envelope = &checkpoint.envelope;
    if trusted_head != &checkpoint.head() {
        return Err(invalid_checkpoint(
            "checkpoint does not match the caller-provided trusted head",
        ));
    }
    if spec.campaign_id() != envelope.campaign_id
        || spec.spec_digest() != envelope.spec_digest
        || spec.max_actions() != envelope.max_actions
        || spec.stages().count() != envelope.stages.len()
    {
        return Err(invalid_checkpoint(
            "checkpoint does not match the supplied campaign specification",
        ));
    }
    for (stage, saved) in spec.stages().zip(&envelope.stages) {
        if stage.stage_id() != saved.stage_id
            || stage.kind() != saved.kind
            || stage.input_digest() != saved.input_digest
            || stage.depends_on() != saved.depends_on
        {
            return Err(invalid_checkpoint(
                "checkpoint stage metadata does not match the supplied specification",
            ));
        }
    }

    let mut receipt_by_stage = BTreeMap::new();
    for receipt in receipts {
        let stage_id = receipt.stage_id().to_owned();
        if receipt_by_stage.insert(stage_id.clone(), receipt).is_some() {
            return Err(invalid_checkpoint(format!(
                "more than one rehydrated receipt was supplied for stage {stage_id:?}"
            )));
        }
    }
    let mut stage_states = BTreeMap::new();
    for saved in &envelope.stages {
        let state = match saved.state {
            StageCheckpointState::Pending => StageProgress::Pending,
            StageCheckpointState::InFlight => StageProgress::InFlight {
                action_ordinal: saved.action_ordinal.expect("validated in-flight ordinal"),
                authorization_digest: saved
                    .authorization_digest
                    .clone()
                    .expect("validated in-flight authorization"),
            },
            StageCheckpointState::Uncertain => StageProgress::Uncertain {
                action_ordinal: saved.action_ordinal.expect("validated uncertain ordinal"),
                authorization_digest: saved
                    .authorization_digest
                    .clone()
                    .expect("validated uncertain authorization"),
                observation: saved
                    .receipt
                    .clone()
                    .expect("validated uncertain observation"),
            },
            StageCheckpointState::Settled => {
                let receipt = receipt_by_stage.remove(&saved.stage_id).ok_or_else(|| {
                    invalid_checkpoint(format!(
                        "settled stage {:?} has no reverified artifact receipt",
                        saved.stage_id
                    ))
                })?;
                if Some(receipt.projection()) != saved.receipt.as_ref() {
                    return Err(invalid_checkpoint(format!(
                        "rehydrated receipt does not match stage {:?}",
                        saved.stage_id
                    )));
                }
                StageProgress::Settled {
                    action_ordinal: saved.action_ordinal.expect("validated settled ordinal"),
                    authorization_digest: saved
                        .authorization_digest
                        .clone()
                        .expect("validated settled authorization"),
                    receipt,
                }
            }
        };
        stage_states.insert(saved.stage_id.clone(), state);
    }
    if !receipt_by_stage.is_empty() {
        return Err(invalid_checkpoint(
            "a rehydrated receipt does not belong to a settled checkpoint stage",
        ));
    }
    let active = envelope.active_stage_id.as_ref().map(|stage_id| {
        let stage = envelope
            .stages
            .iter()
            .find(|stage| &stage.stage_id == stage_id)
            .expect("validated active stage exists");
        ActiveAction {
            stage_id: stage_id.clone(),
            action_ordinal: stage.action_ordinal.expect("validated active ordinal"),
            authorization_digest: stage
                .authorization_digest
                .clone()
                .expect("validated active authorization"),
        }
    });
    let status = if active.is_some() {
        CampaignStatus::ReconciliationRequired
    } else {
        envelope.status
    };
    Ok(ResearchCampaign {
        spec,
        stage_states,
        status,
        actions_used: envelope.actions_used,
        active,
        events: envelope.events.clone(),
        event_chain_digest: envelope.event_chain_digest.clone(),
        last_generation: envelope.generation,
        last_snapshot_digest: Some(envelope.snapshot_digest.clone()),
    })
}

fn validate_stage_projection(stage: &StageCheckpoint) -> Result<(), CampaignError> {
    let shape_ok = match stage.state {
        StageCheckpointState::Pending => {
            stage.action_ordinal.is_none()
                && stage.authorization_digest.is_none()
                && stage.receipt.is_none()
        }
        StageCheckpointState::InFlight => {
            stage.action_ordinal.is_some()
                && stage.authorization_digest.is_some()
                && stage.receipt.is_none()
        }
        StageCheckpointState::Uncertain => {
            stage.action_ordinal.is_some()
                && stage.authorization_digest.is_some()
                && stage.receipt.as_ref().is_some_and(|receipt| {
                    receipt.disposition == CampaignReceiptDisposition::UnknownCompletion
                })
        }
        StageCheckpointState::Settled => {
            stage.action_ordinal.is_some()
                && stage.authorization_digest.is_some()
                && stage.receipt.as_ref().is_some_and(|receipt| {
                    receipt.disposition != CampaignReceiptDisposition::UnknownCompletion
                })
        }
    };
    if !shape_ok {
        return Err(invalid_checkpoint(
            "stage state does not match its authorization and receipt fields",
        ));
    }
    if let Some(ordinal) = stage.action_ordinal {
        if ordinal == 0 {
            return Err(invalid_checkpoint("action ordinal must be positive"));
        }
    }
    if let Some(digest) = &stage.authorization_digest {
        require_digest(digest, "authorization_digest")?;
    }
    if let Some(receipt) = &stage.receipt {
        if receipt.stage_id != stage.stage_id
            || receipt.kind != stage.kind
            || receipt.input_digest != stage.input_digest
        {
            return Err(invalid_checkpoint(
                "receipt projection is not bound to its stage",
            ));
        }
        require_digest(&receipt.artifact_digest, "receipt artifact_digest")?;
        require_digest(&receipt.detail_digest, "receipt detail_digest")?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReplayedStage {
    Pending,
    InFlight {
        action_ordinal: u16,
        authorization_digest: String,
    },
    Uncertain {
        action_ordinal: u16,
        authorization_digest: String,
        observation: ReceiptProjection,
    },
    Settled {
        action_ordinal: u16,
        authorization_digest: String,
        receipt: ReceiptProjection,
    },
}

fn replay_and_validate(
    envelope: &CheckpointEnvelope,
    stage_by_id: &BTreeMap<String, &StageCheckpoint>,
) -> Result<(), CampaignError> {
    if envelope.events.len() > MAX_CAMPAIGN_EVENTS {
        return Err(invalid_checkpoint(
            "event count exceeds the campaign action ceiling",
        ));
    }
    let anchor = empty_event_chain_digest(&envelope.campaign_id, &envelope.spec_digest)?;
    let mut chain = anchor.clone();
    let mut previous_event: Option<String> = None;
    let mut actions_used = 0_u16;
    let mut active: Option<String> = None;
    let mut logical_status = CampaignStatus::Planned;
    let mut replayed = envelope
        .stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), ReplayedStage::Pending))
        .collect::<BTreeMap<_, _>>();

    for (index, event) in envelope.events.iter().enumerate() {
        if event.ordinal as usize != index + 1
            || event.previous_event_digest != previous_event
            || event.event_digest != event.recomputed_digest()?
        {
            return Err(invalid_checkpoint(
                "event ordinals, predecessor links, or event digests are invalid",
            ));
        }
        require_digest(&event.input_digest, "event input_digest")?;
        require_digest(&event.authorization_digest, "event authorization_digest")?;
        require_digest(&event.event_digest, "event_digest")?;
        if let Some(digest) = &event.reconciliation_receipt_digest {
            require_digest(digest, "event reconciliation_receipt_digest")?;
        }
        let stage = stage_by_id
            .get(&event.stage_id)
            .ok_or_else(|| invalid_checkpoint("event names an unknown stage"))?;
        if stage.kind != event.kind || stage.input_digest != event.input_digest {
            return Err(invalid_checkpoint(
                "event does not match its stage metadata",
            ));
        }
        match event.transition {
            CampaignEventKind::Authorized => {
                if event.disposition.is_some()
                    || event.artifact_digest.is_some()
                    || event.detail_digest.is_some()
                    || event.reconciliation_receipt_digest.is_some()
                    || active.is_some()
                    || !matches!(
                        logical_status,
                        CampaignStatus::Planned | CampaignStatus::Ready
                    )
                    || !matches!(replayed.get(&event.stage_id), Some(ReplayedStage::Pending))
                {
                    return Err(invalid_checkpoint("authorization event shape is invalid"));
                }
                if stage.depends_on.iter().any(|dependency| {
                    !matches!(
                        replayed.get(dependency),
                        Some(ReplayedStage::Settled { receipt, .. })
                            if receipt.disposition.allows_dependents()
                    )
                }) {
                    return Err(invalid_checkpoint(
                        "an action was authorized before its dependencies settled",
                    ));
                }
                actions_used = actions_used
                    .checked_add(1)
                    .ok_or_else(|| invalid_checkpoint("action count overflowed"))?;
                if event.action_ordinal != actions_used || actions_used > envelope.max_actions {
                    return Err(invalid_checkpoint(
                        "action ordinals are not contiguous or exceed the ceiling",
                    ));
                }
                let expected = authorization_digest(
                    &envelope.campaign_id,
                    &envelope.spec_digest,
                    &event.stage_id,
                    event.kind,
                    &event.input_digest,
                    event.action_ordinal,
                    &chain,
                )?;
                if expected != event.authorization_digest {
                    return Err(invalid_checkpoint(
                        "authorization digest does not match its campaign boundary",
                    ));
                }
                replayed.insert(
                    event.stage_id.clone(),
                    ReplayedStage::InFlight {
                        action_ordinal: event.action_ordinal,
                        authorization_digest: event.authorization_digest.clone(),
                    },
                );
                active = Some(event.stage_id.clone());
                logical_status = CampaignStatus::InFlight;
            }
            CampaignEventKind::CompletionUnknown => {
                let (Some(artifact_digest), Some(detail_digest)) =
                    (event.artifact_digest.as_ref(), event.detail_digest.as_ref())
                else {
                    return Err(invalid_checkpoint("unknown-completion event is incomplete"));
                };
                if event.disposition != Some(CampaignReceiptDisposition::UnknownCompletion)
                    || event.reconciliation_receipt_digest.is_some()
                {
                    return Err(invalid_checkpoint(
                        "unknown-completion event shape is invalid",
                    ));
                }
                require_digest(artifact_digest, "event artifact_digest")?;
                require_digest(detail_digest, "event detail_digest")?;
                let Some(ReplayedStage::InFlight {
                    action_ordinal,
                    authorization_digest: active_digest,
                }) = replayed.get(&event.stage_id)
                else {
                    return Err(invalid_checkpoint(
                        "unknown-completion event has no matching in-flight action",
                    ));
                };
                if active.as_deref() != Some(event.stage_id.as_str())
                    || *action_ordinal != event.action_ordinal
                    || active_digest != &event.authorization_digest
                {
                    return Err(invalid_checkpoint(
                        "unknown-completion event does not match its authorization",
                    ));
                }
                replayed.insert(
                    event.stage_id.clone(),
                    ReplayedStage::Uncertain {
                        action_ordinal: event.action_ordinal,
                        authorization_digest: event.authorization_digest.clone(),
                        observation: ReceiptProjection {
                            stage_id: event.stage_id.clone(),
                            kind: event.kind,
                            input_digest: event.input_digest.clone(),
                            artifact_digest: artifact_digest.clone(),
                            detail_digest: detail_digest.clone(),
                            disposition: CampaignReceiptDisposition::UnknownCompletion,
                        },
                    },
                );
                logical_status = CampaignStatus::ReconciliationRequired;
            }
            CampaignEventKind::Settled => {
                let (Some(disposition), Some(artifact_digest), Some(detail_digest)) = (
                    event.disposition,
                    event.artifact_digest.as_ref(),
                    event.detail_digest.as_ref(),
                ) else {
                    return Err(invalid_checkpoint("settlement event is incomplete"));
                };
                if logical_status != CampaignStatus::InFlight
                    || disposition == CampaignReceiptDisposition::UnknownCompletion
                    || event.reconciliation_receipt_digest.is_some()
                {
                    return Err(invalid_checkpoint("settlement event shape is invalid"));
                }
                require_digest(artifact_digest, "event artifact_digest")?;
                require_digest(detail_digest, "event detail_digest")?;
                let Some(ReplayedStage::InFlight {
                    action_ordinal,
                    authorization_digest: active_digest,
                }) = replayed.get(&event.stage_id)
                else {
                    return Err(invalid_checkpoint(
                        "settlement event has no matching in-flight action",
                    ));
                };
                if active.as_deref() != Some(event.stage_id.as_str())
                    || *action_ordinal != event.action_ordinal
                    || active_digest != &event.authorization_digest
                {
                    return Err(invalid_checkpoint(
                        "settlement event does not match its authorization",
                    ));
                }
                replayed.insert(
                    event.stage_id.clone(),
                    ReplayedStage::Settled {
                        action_ordinal: event.action_ordinal,
                        authorization_digest: event.authorization_digest.clone(),
                        receipt: ReceiptProjection {
                            stage_id: event.stage_id.clone(),
                            kind: event.kind,
                            input_digest: event.input_digest.clone(),
                            artifact_digest: artifact_digest.clone(),
                            detail_digest: detail_digest.clone(),
                            disposition,
                        },
                    },
                );
                active = None;
                logical_status =
                    status_after_replayed_disposition(envelope, &replayed, disposition);
            }
            CampaignEventKind::ReconciledNotExecuted => {
                let (Some(absence_evidence_digest), Some(reconciliation_receipt_digest)) = (
                    event.detail_digest.as_ref(),
                    event.reconciliation_receipt_digest.as_ref(),
                ) else {
                    return Err(invalid_checkpoint(
                        "not-executed reconciliation event is incomplete",
                    ));
                };
                if !matches!(
                    logical_status,
                    CampaignStatus::InFlight | CampaignStatus::ReconciliationRequired
                ) || event.disposition.is_some()
                    || event.artifact_digest.is_some()
                {
                    return Err(invalid_checkpoint(
                        "not-executed reconciliation event shape is invalid",
                    ));
                }
                require_digest(absence_evidence_digest, "event absence_evidence_digest")?;
                require_digest(
                    reconciliation_receipt_digest,
                    "event reconciliation_receipt_digest",
                )?;
                let (action_ordinal, active_digest) = match replayed.get(&event.stage_id) {
                    Some(
                        ReplayedStage::InFlight {
                            action_ordinal,
                            authorization_digest,
                        }
                        | ReplayedStage::Uncertain {
                            action_ordinal,
                            authorization_digest,
                            ..
                        },
                    ) => (*action_ordinal, authorization_digest),
                    _ => {
                        return Err(invalid_checkpoint(
                            "not-executed reconciliation has no matching uncertain action",
                        ));
                    }
                };
                if active.as_deref() != Some(event.stage_id.as_str())
                    || action_ordinal != event.action_ordinal
                    || active_digest != &event.authorization_digest
                {
                    return Err(invalid_checkpoint(
                        "not-executed reconciliation does not match its authorization",
                    ));
                }
                replayed.insert(event.stage_id.clone(), ReplayedStage::Pending);
                active = None;
                logical_status = if actions_used >= envelope.max_actions {
                    CampaignStatus::Exhausted
                } else {
                    CampaignStatus::Ready
                };
            }
            CampaignEventKind::ReconciledSucceeded => {
                let (
                    Some(disposition),
                    Some(artifact_digest),
                    Some(detail_digest),
                    Some(reconciliation_receipt_digest),
                ) = (
                    event.disposition,
                    event.artifact_digest.as_ref(),
                    event.detail_digest.as_ref(),
                    event.reconciliation_receipt_digest.as_ref(),
                )
                else {
                    return Err(invalid_checkpoint(
                        "succeeded reconciliation event is incomplete",
                    ));
                };
                if !matches!(
                    logical_status,
                    CampaignStatus::InFlight | CampaignStatus::ReconciliationRequired
                ) || disposition == CampaignReceiptDisposition::UnknownCompletion
                {
                    return Err(invalid_checkpoint(
                        "succeeded reconciliation cannot retain unknown completion",
                    ));
                }
                require_digest(artifact_digest, "event artifact_digest")?;
                require_digest(detail_digest, "event detail_digest")?;
                require_digest(
                    reconciliation_receipt_digest,
                    "event reconciliation_receipt_digest",
                )?;
                let (action_ordinal, active_digest) = match replayed.get(&event.stage_id) {
                    Some(
                        ReplayedStage::InFlight {
                            action_ordinal,
                            authorization_digest,
                        }
                        | ReplayedStage::Uncertain {
                            action_ordinal,
                            authorization_digest,
                            ..
                        },
                    ) => (*action_ordinal, authorization_digest),
                    _ => {
                        return Err(invalid_checkpoint(
                            "succeeded reconciliation has no matching uncertain action",
                        ));
                    }
                };
                if active.as_deref() != Some(event.stage_id.as_str())
                    || action_ordinal != event.action_ordinal
                    || active_digest != &event.authorization_digest
                {
                    return Err(invalid_checkpoint(
                        "succeeded reconciliation does not match its authorization",
                    ));
                }
                replayed.insert(
                    event.stage_id.clone(),
                    ReplayedStage::Settled {
                        action_ordinal: event.action_ordinal,
                        authorization_digest: event.authorization_digest.clone(),
                        receipt: ReceiptProjection {
                            stage_id: event.stage_id.clone(),
                            kind: event.kind,
                            input_digest: event.input_digest.clone(),
                            artifact_digest: artifact_digest.clone(),
                            detail_digest: detail_digest.clone(),
                            disposition,
                        },
                    },
                );
                active = None;
                logical_status =
                    status_after_replayed_disposition(envelope, &replayed, disposition);
            }
        }
        chain = event.event_digest.clone();
        previous_event = Some(event.event_digest.clone());
    }
    if actions_used != envelope.actions_used
        || chain != envelope.event_chain_digest
        || active.as_deref() != envelope.active_stage_id.as_deref()
    {
        return Err(invalid_checkpoint(
            "event replay does not match action accounting, chain head, or active stage",
        ));
    }
    for saved in &envelope.stages {
        let replayed = replayed
            .get(&saved.stage_id)
            .expect("replay map covers every stage");
        let matches = match (saved.state, replayed) {
            (StageCheckpointState::Pending, ReplayedStage::Pending) => true,
            (
                StageCheckpointState::InFlight,
                ReplayedStage::InFlight {
                    action_ordinal,
                    authorization_digest,
                },
            ) => {
                saved.action_ordinal == Some(*action_ordinal)
                    && saved.authorization_digest.as_ref() == Some(authorization_digest)
            }
            (
                StageCheckpointState::Uncertain,
                ReplayedStage::Uncertain {
                    action_ordinal,
                    authorization_digest,
                    observation,
                },
            ) => {
                saved.action_ordinal == Some(*action_ordinal)
                    && saved.authorization_digest.as_ref() == Some(authorization_digest)
                    && saved.receipt.as_ref() == Some(observation)
            }
            (
                StageCheckpointState::Settled,
                ReplayedStage::Settled {
                    action_ordinal,
                    authorization_digest,
                    receipt,
                },
            ) => {
                saved.action_ordinal == Some(*action_ordinal)
                    && saved.authorization_digest.as_ref() == Some(authorization_digest)
                    && saved.receipt.as_ref() == Some(receipt)
            }
            _ => false,
        };
        if !matches {
            return Err(invalid_checkpoint(
                "stage projection does not match replayed events",
            ));
        }
    }
    if envelope.status != logical_status
        && !(active.is_some()
            && logical_status == CampaignStatus::InFlight
            && envelope.status == CampaignStatus::ReconciliationRequired)
    {
        return Err(invalid_checkpoint(
            "campaign status does not match its replayed state",
        ));
    }
    Ok(())
}

fn status_after_replayed_disposition(
    envelope: &CheckpointEnvelope,
    replayed: &BTreeMap<String, ReplayedStage>,
    disposition: CampaignReceiptDisposition,
) -> CampaignStatus {
    match disposition {
        CampaignReceiptDisposition::Succeeded
        | CampaignReceiptDisposition::CompletedWithNegativeFindings => {
            let all_settled = replayed.values().all(|state| {
                matches!(
                    state,
                    ReplayedStage::Settled { receipt, .. }
                        if receipt.disposition.allows_dependents()
                )
            });
            if all_settled {
                if envelope
                    .stages
                    .iter()
                    .any(|stage| stage.kind == CampaignActionKind::NeurosurgeryResearch)
                {
                    CampaignStatus::AwaitingHumanReview
                } else {
                    CampaignStatus::Completed
                }
            } else {
                CampaignStatus::Ready
            }
        }
        CampaignReceiptDisposition::MissingInput => CampaignStatus::NeedsInput,
        CampaignReceiptDisposition::UnknownCompletion => CampaignStatus::ReconciliationRequired,
        CampaignReceiptDisposition::AwaitingHumanReview => CampaignStatus::AwaitingHumanReview,
        CampaignReceiptDisposition::Exhausted => CampaignStatus::Exhausted,
        CampaignReceiptDisposition::Refused => CampaignStatus::Refused,
    }
}

fn checkpoint_topological_order(stages: &[StageCheckpoint]) -> Result<Vec<String>, CampaignError> {
    let mut indegree = stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), stage.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for stage in stages {
        for dependency in &stage.depends_on {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(stage.stage_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(stages.len());
    while let Some(id) = ready.pop_first() {
        ordered.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("known dependent has indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if ordered.len() != stages.len() {
        return Err(invalid_checkpoint(
            "checkpoint stage graph contains a cycle",
        ));
    }
    Ok(ordered)
}

fn require_digest(value: &str, field: &str) -> Result<(), CampaignError> {
    ContentHash::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| invalid_checkpoint(format!("{field} is not a lowercase SHA-256 digest")))
}

fn bounded_text(value: &str, field: &str, maximum: usize) -> Result<(), CampaignError> {
    if value.trim().is_empty() || value.contains('\0') || value.len() > maximum {
        return Err(invalid_checkpoint(format!(
            "{field} must be non-empty, NUL-free text within {maximum} bytes"
        )));
    }
    Ok(())
}
