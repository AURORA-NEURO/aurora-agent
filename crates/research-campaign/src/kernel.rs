use crate::adapters::{ReceiptProjection, VerifiedCampaignReceipt};
use crate::checkpoint::seal_campaign_checkpoint;
use crate::coordination::{
    CampaignAuthorizationClaim, CampaignCheckpointCoordinator, CampaignCheckpointHead,
};
use crate::error::{invalid_checkpoint, invalid_reconciliation_receipt, CampaignError};
use crate::model::{
    CampaignActionKind, CampaignAdapterAvailability, CampaignReceiptDisposition, CampaignStatus,
    ResearchCampaignSpec,
};
use crate::reconciliation::{
    CampaignReconciliationDecisionDocument, CampaignReconciliationQuery,
    CampaignReconciliationResult, ValidatedCampaignReconciliationReceipt,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Authorization for exactly one campaign action. It is intentionally neither cloneable nor
/// deserializable; only [`ResearchCampaign::authorize_next_action`] can mint one.
///
/// ```compile_fail
/// use bioprism_research_campaign::CampaignActionAuthorization;
///
/// fn duplicate(token: CampaignActionAuthorization) {
///     let _second = token.clone();
/// }
/// ```
#[derive(Debug)]
pub struct CampaignActionAuthorization {
    campaign_id: String,
    spec_digest: String,
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    action_ordinal: u16,
    authorization_digest: String,
}

impl CampaignActionAuthorization {
    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn stage_id(&self) -> &str {
        &self.stage_id
    }

    pub fn kind(&self) -> CampaignActionKind {
        self.kind
    }

    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }

    pub fn action_ordinal(&self) -> u16 {
        self.action_ordinal
    }

    pub fn authorization_digest(&self) -> &str {
        &self.authorization_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StageProgress {
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
        receipt: VerifiedCampaignReceipt,
    },
}

impl StageProgress {
    pub(crate) fn allows_dependents(&self) -> bool {
        matches!(
            self,
            Self::Settled { receipt, .. } if receipt.disposition().allows_dependents()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CampaignEventKind {
    Authorized,
    CompletionUnknown,
    Settled,
    ReconciledNotExecuted,
    ReconciledSucceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CampaignEvent {
    pub(crate) ordinal: u16,
    pub(crate) action_ordinal: u16,
    pub(crate) stage_id: String,
    pub(crate) kind: CampaignActionKind,
    pub(crate) input_digest: String,
    pub(crate) transition: CampaignEventKind,
    pub(crate) authorization_digest: String,
    pub(crate) disposition: Option<CampaignReceiptDisposition>,
    pub(crate) artifact_digest: Option<String>,
    pub(crate) detail_digest: Option<String>,
    pub(crate) reconciliation_receipt_digest: Option<String>,
    pub(crate) previous_event_digest: Option<String>,
    pub(crate) event_digest: String,
}

impl CampaignEvent {
    pub(crate) fn recomputed_digest(&self) -> Result<String, CampaignError> {
        event_digest(
            self.ordinal,
            self.action_ordinal,
            &self.stage_id,
            self.kind,
            &self.input_digest,
            &self.transition,
            &self.authorization_digest,
            self.disposition,
            self.artifact_digest.as_deref(),
            self.detail_digest.as_deref(),
            self.reconciliation_receipt_digest.as_deref(),
            self.previous_event_digest.as_deref(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveAction {
    pub(crate) stage_id: String,
    pub(crate) action_ordinal: u16,
    pub(crate) authorization_digest: String,
}

#[derive(Clone)]
struct PreparedCampaignAction {
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    action_ordinal: u16,
    authorization_digest: String,
    authorization_predecessor_digest: String,
    event: CampaignEvent,
}

/// In-memory campaign state. Raw objectives and artifacts stay here or with the caller and never
/// enter the durable projection.
#[derive(Debug, Clone)]
pub struct ResearchCampaign {
    pub(crate) spec: ResearchCampaignSpec,
    pub(crate) stage_states: BTreeMap<String, StageProgress>,
    pub(crate) status: CampaignStatus,
    pub(crate) actions_used: u16,
    pub(crate) active: Option<ActiveAction>,
    pub(crate) events: Vec<CampaignEvent>,
    pub(crate) event_chain_digest: String,
    pub(crate) last_generation: u64,
    pub(crate) last_snapshot_digest: Option<String>,
}

/// Start an empty campaign. Starting performs no action and consumes no budget.
pub fn start_campaign(spec: ResearchCampaignSpec) -> Result<ResearchCampaign, CampaignError> {
    let event_chain_digest = empty_event_chain_digest(spec.campaign_id(), spec.spec_digest())?;
    let stage_states = spec
        .stages()
        .map(|stage| (stage.stage_id().to_owned(), StageProgress::Pending))
        .collect();
    Ok(ResearchCampaign {
        spec,
        stage_states,
        status: CampaignStatus::Planned,
        actions_used: 0,
        active: None,
        events: Vec::new(),
        event_chain_digest,
        last_generation: 0,
        last_snapshot_digest: None,
    })
}

impl ResearchCampaign {
    pub fn spec(&self) -> &ResearchCampaignSpec {
        &self.spec
    }

    pub fn status(&self) -> CampaignStatus {
        self.status
    }

    pub fn actions_used(&self) -> u16 {
        self.actions_used
    }

    pub fn event_chain_digest(&self) -> &str {
        &self.event_chain_digest
    }

    pub fn active_stage_id(&self) -> Option<&str> {
        self.active.as_ref().map(|active| active.stage_id.as_str())
    }

    /// Atomically persist the next in-flight checkpoint, then release its dispatch token.
    ///
    /// The caller-owned coordinator must share one durable payload/head transaction across every
    /// worker. A fresh campaign uses an atomic `None -> first head` creation; later calls compare
    /// the exact last sealed head. Local state changes only after the coordinator confirms the
    /// candidate, so rejection cannot leak an authorization or partially mutate this campaign.
    /// If a storage acknowledgement is ambiguous, the coordinator must return an error. The
    /// caller then restores the durable candidate (if present), which is fenced for
    /// reconciliation and never yields the same dispatch token again.
    pub fn authorize_next_action<C: CampaignCheckpointCoordinator>(
        &mut self,
        coordinator: &C,
    ) -> Result<CampaignActionAuthorization, CampaignError> {
        let prepared = self.prepare_next_action()?;
        let expected_head = self.current_checkpoint_head();
        let mut candidate = self.clone();
        let authorization = candidate.commit_prepared_action(prepared.clone());
        let checkpoint = seal_campaign_checkpoint(&mut candidate)?;
        let claim = CampaignAuthorizationClaim::new(
            expected_head.clone(),
            checkpoint.head(),
            prepared.stage_id,
            prepared.kind,
            prepared.input_digest,
            prepared.action_ordinal,
            prepared.authorization_digest,
            prepared.authorization_predecessor_digest,
        );
        coordinator
            .compare_and_store_authorization(expected_head.as_ref(), &checkpoint, &claim)
            .map_err(|reason| CampaignError::AuthorizationCheckpointRejected { reason })?;
        *self = candidate;
        Ok(authorization)
    }

    fn current_checkpoint_head(&self) -> Option<CampaignCheckpointHead> {
        self.last_snapshot_digest.as_ref().map(|snapshot_digest| {
            CampaignCheckpointHead::new(
                self.spec.campaign_id().to_owned(),
                self.spec.spec_digest().to_owned(),
                self.last_generation,
                snapshot_digest.clone(),
            )
        })
    }

    fn prepare_next_action(&self) -> Result<PreparedCampaignAction, CampaignError> {
        if self.active.is_some() {
            return Err(CampaignError::ActionAlreadyInFlight);
        }
        if !matches!(self.status, CampaignStatus::Planned | CampaignStatus::Ready) {
            return Err(CampaignError::ActionNotAvailable {
                status: self.status.as_str().to_owned(),
            });
        }
        if self.actions_used >= self.spec.max_actions() {
            return Err(CampaignError::ActionCeilingExhausted);
        }
        let stage_id = self
            .spec
            .topological_order()
            .iter()
            .find(|stage_id| {
                matches!(
                    self.stage_states.get(*stage_id),
                    Some(StageProgress::Pending)
                ) && self
                    .spec
                    .stage(stage_id)
                    .expect("ordered stage exists")
                    .depends_on()
                    .iter()
                    .all(|dependency| {
                        self.stage_states
                            .get(dependency)
                            .is_some_and(StageProgress::allows_dependents)
                    })
            })
            .cloned()
            .ok_or_else(|| invalid_checkpoint("campaign has no dependency-ready pending stage"))?;
        let stage = self.spec.stage(&stage_id).expect("ordered stage exists");
        if let CampaignAdapterAvailability::FeatureDisabled { required_feature } =
            stage.kind().adapter_availability()
        {
            return Err(CampaignError::AdapterUnavailable {
                kind: stage.kind().as_str().to_owned(),
                required_feature,
            });
        }
        let action_ordinal = self.actions_used + 1;
        let authorization_digest = authorization_digest(
            self.spec.campaign_id(),
            self.spec.spec_digest(),
            stage.stage_id(),
            stage.kind(),
            stage.input_digest(),
            action_ordinal,
            &self.event_chain_digest,
        )?;
        let event = build_event(
            self.events.len() as u16 + 1,
            action_ordinal,
            stage.stage_id(),
            stage.kind(),
            stage.input_digest(),
            CampaignEventKind::Authorized,
            &authorization_digest,
            None,
            None,
            None,
            None,
            self.events.last().map(|event| event.event_digest.as_str()),
        )?;

        Ok(PreparedCampaignAction {
            stage_id,
            kind: stage.kind(),
            input_digest: stage.input_digest().to_owned(),
            action_ordinal,
            authorization_digest,
            authorization_predecessor_digest: self.event_chain_digest.clone(),
            event,
        })
    }

    fn commit_prepared_action(
        &mut self,
        prepared: PreparedCampaignAction,
    ) -> CampaignActionAuthorization {
        self.actions_used = prepared.action_ordinal;
        self.stage_states.insert(
            prepared.stage_id.clone(),
            StageProgress::InFlight {
                action_ordinal: prepared.action_ordinal,
                authorization_digest: prepared.authorization_digest.clone(),
            },
        );
        self.active = Some(ActiveAction {
            stage_id: prepared.stage_id.clone(),
            action_ordinal: prepared.action_ordinal,
            authorization_digest: prepared.authorization_digest.clone(),
        });
        self.status = CampaignStatus::InFlight;
        self.event_chain_digest = prepared.event.event_digest.clone();
        self.events.push(prepared.event);

        CampaignActionAuthorization {
            campaign_id: self.spec.campaign_id().to_owned(),
            spec_digest: self.spec.spec_digest().to_owned(),
            stage_id: prepared.stage_id,
            kind: prepared.kind,
            input_digest: prepared.input_digest,
            action_ordinal: prepared.action_ordinal,
            authorization_digest: prepared.authorization_digest,
        }
    }

    /// Consume one authorization and one verifier-created receipt. A stale token or mismatched
    /// receipt fails before any campaign state changes.
    pub fn apply_receipt(
        &mut self,
        authorization: CampaignActionAuthorization,
        receipt: VerifiedCampaignReceipt,
    ) -> Result<(), CampaignError> {
        let active = self
            .active
            .as_ref()
            .ok_or(CampaignError::StaleAuthorization)?;
        if self.status != CampaignStatus::InFlight
            || authorization.campaign_id != self.spec.campaign_id()
            || authorization.spec_digest != self.spec.spec_digest()
            || authorization.stage_id != active.stage_id
            || authorization.action_ordinal != active.action_ordinal
            || authorization.authorization_digest != active.authorization_digest
            || receipt.stage_id() != authorization.stage_id
            || receipt.kind() != authorization.kind
            || receipt.input_digest() != authorization.input_digest
        {
            return Err(CampaignError::StaleAuthorization);
        }
        let authorization_predecessor = self.authorization_predecessor_digest(active)?;
        let expected = authorization_digest(
            &authorization.campaign_id,
            &authorization.spec_digest,
            &authorization.stage_id,
            authorization.kind,
            &authorization.input_digest,
            authorization.action_ordinal,
            &authorization_predecessor,
        )?;
        if expected != authorization.authorization_digest {
            return Err(CampaignError::StaleAuthorization);
        }

        let projection = receipt.projection().clone();
        let transition = if projection.disposition == CampaignReceiptDisposition::UnknownCompletion
        {
            CampaignEventKind::CompletionUnknown
        } else {
            CampaignEventKind::Settled
        };
        let event = build_event(
            self.events.len() as u16 + 1,
            authorization.action_ordinal,
            &authorization.stage_id,
            authorization.kind,
            &authorization.input_digest,
            transition,
            &authorization.authorization_digest,
            Some(projection.disposition),
            Some(&projection.artifact_digest),
            Some(&projection.detail_digest),
            None,
            self.events.last().map(|event| event.event_digest.as_str()),
        )?;
        let disposition = receipt.disposition();
        if disposition == CampaignReceiptDisposition::UnknownCompletion {
            self.stage_states.insert(
                authorization.stage_id,
                StageProgress::Uncertain {
                    action_ordinal: authorization.action_ordinal,
                    authorization_digest: authorization.authorization_digest,
                    observation: projection,
                },
            );
        } else {
            self.stage_states.insert(
                authorization.stage_id,
                StageProgress::Settled {
                    action_ordinal: authorization.action_ordinal,
                    authorization_digest: authorization.authorization_digest,
                    receipt,
                },
            );
            self.active = None;
        }
        self.event_chain_digest = event.event_digest.clone();
        self.events.push(event);
        self.status = self.status_after_disposition(disposition);
        Ok(())
    }

    /// Describe the one fenced authorization a configured execution journal must reconcile.
    pub fn reconciliation_query(&self) -> Result<CampaignReconciliationQuery, CampaignError> {
        if self.status != CampaignStatus::ReconciliationRequired {
            return Err(CampaignError::ReconciliationNotAvailable {
                status: self.status.as_str().to_owned(),
            });
        }
        let active = self
            .active
            .as_ref()
            .ok_or_else(|| invalid_checkpoint("reconciliation status has no active action"))?;
        let stage = self
            .spec
            .stage(&active.stage_id)
            .ok_or_else(|| invalid_checkpoint("active action names an unknown stage"))?;
        if !self.active_progress_matches(active) {
            return Err(invalid_checkpoint(
                "active action does not match its stage progress",
            ));
        }
        let predecessor = self.authorization_predecessor_digest(active)?;
        let expected = authorization_digest(
            self.spec.campaign_id(),
            self.spec.spec_digest(),
            stage.stage_id(),
            stage.kind(),
            stage.input_digest(),
            active.action_ordinal,
            &predecessor,
        )?;
        if expected != active.authorization_digest {
            return Err(invalid_checkpoint(
                "active authorization digest does not match its predecessor",
            ));
        }
        Ok(CampaignReconciliationQuery::new(
            self.spec.campaign_id().to_owned(),
            self.spec.spec_digest().to_owned(),
            stage.stage_id().to_owned(),
            stage.kind(),
            stage.input_digest().to_owned(),
            active.action_ordinal,
            active.authorization_digest.clone(),
            predecessor,
            self.spec.reconciliation_authority().clone(),
        ))
    }

    /// Consume a journal-verified reconciliation decision for the currently fenced action.
    /// Unknown is a no-op, proved non-execution requeues without refunding the action ordinal,
    /// and known execution requires the exact native verified receipt named by the journal.
    pub fn reconcile_active_action(
        &mut self,
        reconciliation: ValidatedCampaignReconciliationReceipt,
        native_receipt: Option<VerifiedCampaignReceipt>,
    ) -> Result<CampaignReconciliationResult, CampaignError> {
        let query = self.reconciliation_query()?;
        let document = reconciliation.document();
        if document.campaign_id != query.campaign_id()
            || document.spec_digest != query.spec_digest()
            || document.stage_id != query.stage_id()
            || document.kind != query.kind()
            || document.input_digest != query.input_digest()
            || document.action_ordinal != query.action_ordinal()
            || document.authorization_digest != query.authorization_digest()
            || document.authorization_predecessor_digest != query.authorization_predecessor_digest()
            || &document.authority != query.authority()
        {
            return Err(CampaignError::StaleReconciliationReceipt);
        }
        let decision = document.decision.clone();
        let reconciliation_receipt_digest = document.receipt_digest.clone();
        let previous_event_digest = self.events.last().map(|event| event.event_digest.clone());

        match decision {
            CampaignReconciliationDecisionDocument::Unknown { .. } => {
                if native_receipt.is_some() {
                    return Err(invalid_reconciliation_receipt(
                        "unknown reconciliation cannot carry a native receipt",
                    ));
                }
                Ok(CampaignReconciliationResult::Unresolved)
            }
            CampaignReconciliationDecisionDocument::NotExecuted {
                absence_evidence_digest,
            } => {
                if native_receipt.is_some() {
                    return Err(invalid_reconciliation_receipt(
                        "not_executed reconciliation cannot carry a native receipt",
                    ));
                }
                let event = build_event(
                    self.events.len() as u16 + 1,
                    query.action_ordinal(),
                    query.stage_id(),
                    query.kind(),
                    query.input_digest(),
                    CampaignEventKind::ReconciledNotExecuted,
                    query.authorization_digest(),
                    None,
                    None,
                    Some(&absence_evidence_digest),
                    Some(&reconciliation_receipt_digest),
                    previous_event_digest.as_deref(),
                )?;
                self.stage_states
                    .insert(query.stage_id().to_owned(), StageProgress::Pending);
                self.active = None;
                self.event_chain_digest = event.event_digest.clone();
                self.events.push(event);
                if self.actions_used >= self.spec.max_actions() {
                    self.status = CampaignStatus::Exhausted;
                    Ok(CampaignReconciliationResult::Exhausted)
                } else {
                    self.status = CampaignStatus::Ready;
                    Ok(CampaignReconciliationResult::Requeued)
                }
            }
            CampaignReconciliationDecisionDocument::Succeeded {
                artifact_digest,
                native_receipt_digest,
                ..
            } => {
                let native_receipt = native_receipt.ok_or_else(|| {
                    invalid_reconciliation_receipt(
                        "succeeded reconciliation requires its native verified receipt",
                    )
                })?;
                if native_receipt.stage_id() != query.stage_id()
                    || native_receipt.kind() != query.kind()
                    || native_receipt.input_digest() != query.input_digest()
                    || native_receipt.artifact_digest() != artifact_digest
                    || native_receipt.projection_digest()? != native_receipt_digest
                    || native_receipt.disposition() == CampaignReceiptDisposition::UnknownCompletion
                {
                    return Err(invalid_reconciliation_receipt(
                        "native verified receipt does not exactly match succeeded reconciliation",
                    ));
                }
                let projection = native_receipt.projection().clone();
                let event = build_event(
                    self.events.len() as u16 + 1,
                    query.action_ordinal(),
                    query.stage_id(),
                    query.kind(),
                    query.input_digest(),
                    CampaignEventKind::ReconciledSucceeded,
                    query.authorization_digest(),
                    Some(projection.disposition),
                    Some(&projection.artifact_digest),
                    Some(&projection.detail_digest),
                    Some(&reconciliation_receipt_digest),
                    previous_event_digest.as_deref(),
                )?;
                let disposition = native_receipt.disposition();
                self.stage_states.insert(
                    query.stage_id().to_owned(),
                    StageProgress::Settled {
                        action_ordinal: query.action_ordinal(),
                        authorization_digest: query.authorization_digest().to_owned(),
                        receipt: native_receipt,
                    },
                );
                self.active = None;
                self.event_chain_digest = event.event_digest.clone();
                self.events.push(event);
                self.status = self.status_after_disposition(disposition);
                Ok(CampaignReconciliationResult::Settled(self.status))
            }
        }
    }

    fn active_progress_matches(&self, active: &ActiveAction) -> bool {
        matches!(
            self.stage_states.get(&active.stage_id),
            Some(
                StageProgress::InFlight {
                    action_ordinal,
                    authorization_digest,
                }
                | StageProgress::Uncertain {
                    action_ordinal,
                    authorization_digest,
                    ..
                }
            ) if *action_ordinal == active.action_ordinal
                && authorization_digest == &active.authorization_digest
        )
    }

    fn authorization_predecessor_digest(
        &self,
        active: &ActiveAction,
    ) -> Result<String, CampaignError> {
        let authorization_event = self
            .events
            .iter()
            .rev()
            .find(|event| {
                event.transition == CampaignEventKind::Authorized
                    && event.stage_id == active.stage_id
                    && event.action_ordinal == active.action_ordinal
                    && event.authorization_digest == active.authorization_digest
            })
            .ok_or_else(|| invalid_checkpoint("active action has no authorization event"))?;
        authorization_event
            .previous_event_digest
            .clone()
            .map_or_else(
                || empty_event_chain_digest(self.spec.campaign_id(), self.spec.spec_digest()),
                Ok,
            )
    }

    fn status_after_disposition(&self, disposition: CampaignReceiptDisposition) -> CampaignStatus {
        match disposition {
            CampaignReceiptDisposition::Succeeded
            | CampaignReceiptDisposition::CompletedWithNegativeFindings => {
                if self
                    .stage_states
                    .values()
                    .all(StageProgress::allows_dependents)
                {
                    if self
                        .spec
                        .stages()
                        .any(|stage| stage.kind() == CampaignActionKind::NeurosurgeryResearch)
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
}

pub(crate) fn empty_event_chain_digest(
    campaign_id: &str,
    spec_digest: &str,
) -> Result<String, CampaignError> {
    digest_value(&json!({
        "campaign_id": campaign_id,
        "spec_digest": spec_digest,
        "events": [],
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authorization_digest(
    campaign_id: &str,
    spec_digest: &str,
    stage_id: &str,
    kind: CampaignActionKind,
    input_digest: &str,
    action_ordinal: u16,
    preceding_event_chain_digest: &str,
) -> Result<String, CampaignError> {
    digest_value(&json!({
        "campaign_id": campaign_id,
        "spec_digest": spec_digest,
        "stage_id": stage_id,
        "kind": kind,
        "input_digest": input_digest,
        "action_ordinal": action_ordinal,
        "preceding_event_chain_digest": preceding_event_chain_digest,
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_event(
    ordinal: u16,
    action_ordinal: u16,
    stage_id: &str,
    kind: CampaignActionKind,
    input_digest: &str,
    transition: CampaignEventKind,
    authorization_digest: &str,
    disposition: Option<CampaignReceiptDisposition>,
    artifact_digest: Option<&str>,
    detail_digest: Option<&str>,
    reconciliation_receipt_digest: Option<&str>,
    previous_event_digest: Option<&str>,
) -> Result<CampaignEvent, CampaignError> {
    let event_digest = event_digest(
        ordinal,
        action_ordinal,
        stage_id,
        kind,
        input_digest,
        &transition,
        authorization_digest,
        disposition,
        artifact_digest,
        detail_digest,
        reconciliation_receipt_digest,
        previous_event_digest,
    )?;
    Ok(CampaignEvent {
        ordinal,
        action_ordinal,
        stage_id: stage_id.to_owned(),
        kind,
        input_digest: input_digest.to_owned(),
        transition,
        authorization_digest: authorization_digest.to_owned(),
        disposition,
        artifact_digest: artifact_digest.map(str::to_owned),
        detail_digest: detail_digest.map(str::to_owned),
        reconciliation_receipt_digest: reconciliation_receipt_digest.map(str::to_owned),
        previous_event_digest: previous_event_digest.map(str::to_owned),
        event_digest,
    })
}

#[allow(clippy::too_many_arguments)]
fn event_digest(
    ordinal: u16,
    action_ordinal: u16,
    stage_id: &str,
    kind: CampaignActionKind,
    input_digest: &str,
    transition: &CampaignEventKind,
    authorization_digest: &str,
    disposition: Option<CampaignReceiptDisposition>,
    artifact_digest: Option<&str>,
    detail_digest: Option<&str>,
    reconciliation_receipt_digest: Option<&str>,
    previous_event_digest: Option<&str>,
) -> Result<String, CampaignError> {
    digest_value(&json!({
        "ordinal": ordinal,
        "action_ordinal": action_ordinal,
        "stage_id": stage_id,
        "kind": kind,
        "input_digest": input_digest,
        "transition": transition,
        "authorization_digest": authorization_digest,
        "disposition": disposition,
        "artifact_digest": artifact_digest,
        "detail_digest": detail_digest,
        "reconciliation_receipt_digest": reconciliation_receipt_digest,
        "previous_event_digest": previous_event_digest,
    }))
}

pub(crate) fn digest_value(value: &Value) -> Result<String, CampaignError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })
}
