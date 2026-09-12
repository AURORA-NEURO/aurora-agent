use crate::error::{invalid_receipt, CampaignError};
use crate::model::{CampaignActionKind, CampaignReceiptDisposition, CampaignStageSpec};
use bioprism_autopilot::{
    build_autopilot_report, plan_next_action, AutonomyGrant, DriveHistory, FinalDisposition,
    NextAction,
};
use bioprism_brain::{plan_autonomous, AutonomousPlanRequest};
use bioprism_ids::ContentHash;
use bioprism_research::{run_research, ResearchRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A receipt that passed its native verifier and is bound to one validated campaign stage.
///
/// Fields are private and this type is not deserializable. Restored callers must re-run the
/// appropriate constructor over the private artifact rather than trusting checkpoint metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCampaignReceipt {
    projection: ReceiptProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReceiptProjection {
    pub(crate) stage_id: String,
    pub(crate) kind: CampaignActionKind,
    pub(crate) input_digest: String,
    pub(crate) artifact_digest: String,
    pub(crate) detail_digest: String,
    pub(crate) disposition: CampaignReceiptDisposition,
}

impl VerifiedCampaignReceipt {
    /// Replay a synthetic research dossier from its embedded validated request and preserve the
    /// presence of negative findings. A self-consistent digest or non-empty result shape is not a
    /// substitute for reproducing the deterministic native run exactly.
    pub fn from_research_dossier(
        stage: &CampaignStageSpec,
        dossier: &Value,
    ) -> Result<Self, CampaignError> {
        require_kind(stage, CampaignActionKind::SyntheticResearch)?;
        let verification = bioprism_research::verify_dossier(dossier).map_err(|error| {
            CampaignError::Upstream {
                component: "research",
                reason: error.to_string(),
            }
        })?;
        if verification.get("valid").and_then(Value::as_bool) != Some(true) {
            return Err(invalid_receipt(
                "the research dossier verifier did not accept the dossier",
            ));
        }
        let request_value = dossier
            .get("request")
            .ok_or_else(|| invalid_receipt("verified dossier has no embedded request"))?;
        let request: ResearchRequest =
            serde_json::from_value(request_value.clone()).map_err(|error| {
                invalid_receipt(format!(
                    "verified dossier request does not match the native research schema: {error}"
                ))
            })?;
        let input_digest = request.digest().map_err(|error| CampaignError::Upstream {
            component: "research",
            reason: error.to_string(),
        })?;
        require_input(stage, &input_digest)?;
        let replayed = run_research(&request).map_err(|error| CampaignError::Upstream {
            component: "research",
            reason: error.to_string(),
        })?;
        if &replayed != dossier {
            return Err(invalid_receipt(
                "research dossier does not exactly match deterministic native replay",
            ));
        }
        let artifact_digest = required_digest(dossier, "dossier_sha256")?;
        let findings = dossier
            .get("findings")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_receipt("verified dossier has no findings array"))?;
        if findings.is_empty() {
            return Err(invalid_receipt(
                "a successful research dossier requires at least one finding",
            ));
        }
        let mut negative_findings = 0_u16;
        for finding in findings {
            match finding.get("negative").and_then(Value::as_bool) {
                Some(true) => {
                    negative_findings = negative_findings
                        .checked_add(1)
                        .ok_or_else(|| invalid_receipt("negative finding count exceeds u16"))?;
                }
                Some(false) => {}
                None => {
                    return Err(invalid_receipt(
                        "every verified research finding must label negative explicitly",
                    ));
                }
            }
        }
        let disposition = if negative_findings == 0 {
            CampaignReceiptDisposition::Succeeded
        } else {
            CampaignReceiptDisposition::CompletedWithNegativeFindings
        };
        let detail_digest = digest_value(&json!({
            "finding_count": findings.len(),
            "negative_finding_count": negative_findings,
            "verification": verification,
        }))?;
        Self::new(stage, artifact_digest, detail_digest, disposition)
    }

    /// Re-run the brain's pure planner. A refused plan remains a refusal, and an effectful plan
    /// stops for approval rather than being treated as executed.
    pub fn from_brain_plan(
        stage: &CampaignStageSpec,
        request: &AutonomousPlanRequest,
    ) -> Result<Self, CampaignError> {
        require_kind(stage, CampaignActionKind::BrainPlan)?;
        let request_value =
            serde_json::to_value(request).map_err(|error| CampaignError::Canonicalisation {
                reason: error.to_string(),
            })?;
        let input_digest = digest_value(&request_value)?;
        require_input(stage, &input_digest)?;
        let report = plan_autonomous(request).map_err(|error| CampaignError::Upstream {
            component: "brain",
            reason: error.to_string(),
        })?;
        let report_value =
            serde_json::to_value(&report).map_err(|error| CampaignError::Canonicalisation {
                reason: error.to_string(),
            })?;
        let detail_digest = digest_value(&report_value)?;
        let (artifact_digest, disposition) = match (report.ok, report.plan) {
            (true, Some(plan)) => {
                validate_digest(&plan.plan_digest, "brain plan_digest")?;
                let disposition = if plan.requires_approval {
                    CampaignReceiptDisposition::AwaitingHumanReview
                } else {
                    CampaignReceiptDisposition::Succeeded
                };
                (plan.plan_digest, disposition)
            }
            (false, None) => (detail_digest.clone(), CampaignReceiptDisposition::Refused),
            _ => {
                return Err(invalid_receipt(
                    "brain planning report has inconsistent ok/plan fields",
                ));
            }
        };
        Self::new(stage, artifact_digest, detail_digest, disposition)
    }

    /// Integrity-check a raw autopilot report and preserve only a non-success terminal status.
    ///
    /// A raw, self-digested report cannot demonstrate the grant, attempt history, or planner stop
    /// that produced it. This compatibility path may therefore stop a campaign as exhausted or
    /// refused, but it can never mint a receipt that allows dependents to run.
    pub fn from_autopilot_report(
        stage: &CampaignStageSpec,
        report: &Value,
    ) -> Result<Self, CampaignError> {
        require_kind(stage, CampaignActionKind::AutopilotDrive)?;
        let verification =
            bioprism_autopilot::verify_autopilot_report(report).map_err(|error| {
                CampaignError::Upstream {
                    component: "autopilot",
                    reason: error.to_string(),
                }
            })?;
        if verification.get("valid").and_then(Value::as_bool) != Some(true) {
            return Err(invalid_receipt(
                "the autopilot report verifier did not accept the report",
            ));
        }
        let input_digest = required_digest(report, "base_mission_digest")?;
        require_input(stage, &input_digest)?;
        let artifact_digest = required_digest(report, "report_sha256")?;
        let disposition = match report.get("final_status").and_then(Value::as_str) {
            Some("succeeded") => {
                return Err(invalid_receipt(
                    "a raw autopilot report cannot prove success; supply terminal grant and history",
                ));
            }
            Some("exhausted") => CampaignReceiptDisposition::Exhausted,
            Some("refused") => CampaignReceiptDisposition::Refused,
            _ => return Err(invalid_receipt("autopilot final_status is not recognized")),
        };
        let detail_digest = digest_value(&verification)?;
        Self::new(stage, artifact_digest, detail_digest, disposition)
    }

    /// Re-run the autopilot's pure terminal planner over caller-rehydrated private history and
    /// rebuild its canonical report. This is the only autopilot adapter path that may mint a
    /// successful campaign receipt.
    pub fn from_autopilot_terminal_history(
        stage: &CampaignStageSpec,
        grant: &AutonomyGrant,
        history: &DriveHistory,
    ) -> Result<Self, CampaignError> {
        require_kind(stage, CampaignActionKind::AutopilotDrive)?;
        let (terminal, disposition) =
            match plan_next_action(grant, history).map_err(|error| CampaignError::Upstream {
                component: "autopilot",
                reason: error.to_string(),
            })? {
                NextAction::StopSuccess { evidence } => (
                    FinalDisposition::Succeeded { evidence },
                    CampaignReceiptDisposition::Succeeded,
                ),
                NextAction::StopExhausted { accounting } => (
                    FinalDisposition::Exhausted { accounting },
                    CampaignReceiptDisposition::Exhausted,
                ),
                NextAction::StopRefused {
                    first_terminal_refusal,
                } => (
                    FinalDisposition::Refused {
                        first_terminal_refusal,
                    },
                    CampaignReceiptDisposition::Refused,
                ),
                NextAction::DispatchFull { .. } | NextAction::DispatchRepair { .. } => {
                    return Err(invalid_receipt(
                        "autopilot history is not terminal and cannot settle a campaign stage",
                    ));
                }
            };
        let report = build_autopilot_report(grant, history, &terminal).map_err(|error| {
            CampaignError::Upstream {
                component: "autopilot",
                reason: error.to_string(),
            }
        })?;
        let verification =
            bioprism_autopilot::verify_autopilot_report(&report).map_err(|error| {
                CampaignError::Upstream {
                    component: "autopilot",
                    reason: error.to_string(),
                }
            })?;
        if verification.get("valid").and_then(Value::as_bool) != Some(true) {
            return Err(invalid_receipt(
                "the rebuilt autopilot report did not pass its native integrity verifier",
            ));
        }
        let input_digest = required_digest(&report, "base_mission_digest")?;
        require_input(stage, &input_digest)?;
        let artifact_digest = required_digest(&report, "report_sha256")?;
        let detail_digest = digest_value(&verification)?;
        Self::new(stage, artifact_digest, detail_digest, disposition)
    }

    /// Record caller-observed missing input without converting it into a negative measurement.
    /// The observation digest is structurally validated but is not a truth attestation.
    pub fn missing_input(
        stage: &CampaignStageSpec,
        observation_digest: impl Into<String>,
    ) -> Result<Self, CampaignError> {
        let observation_digest = observation_digest.into();
        validate_digest(&observation_digest, "missing-input observation digest")?;
        let detail_digest = digest_value(&json!({
            "classification": "missing_input",
            "observation_digest": observation_digest,
        }))?;
        Self::new(
            stage,
            observation_digest,
            detail_digest,
            CampaignReceiptDisposition::MissingInput,
        )
    }

    /// Record an unknown completion boundary. Applying this receipt requires reconciliation and
    /// can never authorize a blind retry.
    pub fn unknown_completion(
        stage: &CampaignStageSpec,
        observation_digest: impl Into<String>,
    ) -> Result<Self, CampaignError> {
        let observation_digest = observation_digest.into();
        validate_digest(&observation_digest, "unknown-completion observation digest")?;
        let detail_digest = digest_value(&json!({
            "classification": "unknown_completion",
            "observation_digest": observation_digest,
        }))?;
        Self::new(
            stage,
            observation_digest,
            detail_digest,
            CampaignReceiptDisposition::UnknownCompletion,
        )
    }

    #[cfg(feature = "neurosurgery-adapter")]
    /// Verify a neurosurgical session. Even a finished route settles only as awaiting human
    /// review; this adapter has no success branch by design.
    pub fn from_neurosurgery_session(
        stage: &CampaignStageSpec,
        agent: &bioprism_neurosurgery::NeurosurgicalAgent,
        session: &bioprism_neurosurgery::NeurosurgicalSession,
    ) -> Result<Self, CampaignError> {
        use bioprism_neurosurgery::SessionStatus;

        require_kind(stage, CampaignActionKind::NeurosurgeryResearch)?;
        agent
            .validate_session_integrity(session)
            .map_err(|error| CampaignError::Upstream {
                component: "neurosurgery",
                reason: error.to_string(),
            })?;
        validate_digest(&session.request_digest, "neurosurgery request_digest")?;
        require_input(stage, &session.request_digest)?;
        validate_digest(
            &session.event_chain_digest,
            "neurosurgery event_chain_digest",
        )?;
        let disposition = match session.status {
            SessionStatus::NeedsInput => CampaignReceiptDisposition::MissingInput,
            SessionStatus::AwaitingHumanReview => CampaignReceiptDisposition::AwaitingHumanReview,
            SessionStatus::Planned | SessionStatus::Running => {
                return Err(invalid_receipt(
                    "a planned or running neurosurgical session is not a settlement receipt",
                ));
            }
        };
        let detail_value =
            serde_json::to_value(session).map_err(|error| CampaignError::Canonicalisation {
                reason: error.to_string(),
            })?;
        let detail_digest = digest_value(&detail_value)?;
        Self::new(
            stage,
            session.event_chain_digest.clone(),
            detail_digest,
            disposition,
        )
    }

    pub fn stage_id(&self) -> &str {
        &self.projection.stage_id
    }

    pub fn kind(&self) -> CampaignActionKind {
        self.projection.kind
    }

    pub fn input_digest(&self) -> &str {
        &self.projection.input_digest
    }

    pub fn artifact_digest(&self) -> &str {
        &self.projection.artifact_digest
    }

    pub fn disposition(&self) -> CampaignReceiptDisposition {
        self.projection.disposition
    }

    /// Digest of the native verifier projection used by a succeeded reconciliation receipt.
    pub fn projection_digest(&self) -> Result<String, CampaignError> {
        let value = serde_json::to_value(&self.projection).map_err(|error| {
            CampaignError::Canonicalisation {
                reason: error.to_string(),
            }
        })?;
        digest_value(&value)
    }

    pub(crate) fn projection(&self) -> &ReceiptProjection {
        &self.projection
    }

    fn new(
        stage: &CampaignStageSpec,
        artifact_digest: String,
        detail_digest: String,
        disposition: CampaignReceiptDisposition,
    ) -> Result<Self, CampaignError> {
        validate_digest(&artifact_digest, "artifact_digest")?;
        validate_digest(&detail_digest, "detail_digest")?;
        Ok(Self {
            projection: ReceiptProjection {
                stage_id: stage.stage_id().to_owned(),
                kind: stage.kind(),
                input_digest: stage.input_digest().to_owned(),
                artifact_digest,
                detail_digest,
                disposition,
            },
        })
    }
}

fn require_kind(
    stage: &CampaignStageSpec,
    expected: CampaignActionKind,
) -> Result<(), CampaignError> {
    if stage.kind() != expected {
        return Err(invalid_receipt(format!(
            "stage {:?} is {}, not {}",
            stage.stage_id(),
            stage.kind().as_str(),
            expected.as_str()
        )));
    }
    Ok(())
}

fn require_input(stage: &CampaignStageSpec, actual: &str) -> Result<(), CampaignError> {
    if stage.input_digest() != actual {
        return Err(invalid_receipt(format!(
            "artifact input digest does not match stage {:?}",
            stage.stage_id()
        )));
    }
    Ok(())
}

fn required_digest(value: &Value, field: &str) -> Result<String, CampaignError> {
    let digest = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_receipt(format!("{field} must be a digest string")))?
        .to_owned();
    validate_digest(&digest, field)?;
    Ok(digest)
}

fn validate_digest(value: &str, field: &str) -> Result<(), CampaignError> {
    ContentHash::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| invalid_receipt(format!("{field} must be a lowercase SHA-256 digest")))
}

fn digest_value(value: &Value) -> Result<String, CampaignError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })
}
