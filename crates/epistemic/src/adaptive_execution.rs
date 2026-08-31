//! Authorized execution, observation receipts, and replay for adaptive policies.
//!
//! The adaptive planner intentionally stops at a policy tree: a likelihood profile says what
//! outcomes are possible under the caller's models, but it does not say that a provider was
//! contacted. This module is the narrow bridge between those two planes. It accepts an explicit
//! grant, asks an explicitly named provider for one observation at a time, validates the response
//! against the selected policy branch, and returns a receipt that can be replayed without a live
//! provider.
//!
//! The provider trait is deliberately small and domain-neutral. A laboratory adapter, a software
//! test runner, a literature retriever, a CI system, or an operations system can implement the
//! same seam. The epistemic kernel never opens a socket, touches a specimen, edits a repository,
//! or sends a message. Those effects remain in the provider and in its own authorization system;
//! this module records only the fact that the provider returned a bounded, digest-addressed
//! outcome.
//!
//! A receipt is not a scientific, clinical, security, or release approval. `Observed` means the
//! provider declared that it observed the result; provider authentication, consent, chain of
//! custody, and domain policy remain separate checks. `Simulated` and `Replayed` are kept as
//! distinct provenance variants so a caller cannot count a deterministic rehearsal as an external
//! observation.

use crate::adaptive::{adaptive_policy, AdaptiveNode, AdaptivePolicy};
use crate::decision::{Belief, DecisionProblem};
use crate::evidence::Acquisition;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

/// Stable schema identifier for the execution boundary.
pub const ADAPTIVE_EXECUTION_SCHEMA: &str = "bioprism-epistemic/adaptive-execution/0.1";
/// The epsilon used for receipt-level budget and probability reconciliation.
pub const EXECUTION_RECONCILIATION_EPSILON: f64 = 1e-9;

/// The source class attached to one provider outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationProvenance {
    /// The named provider declares that it obtained the result from its external or local world.
    Observed,
    /// A deterministic provider generated the result without claiming an external observation.
    Simulated,
    /// A replay provider copied the result from an existing receipt and has no live source.
    Replayed,
}

/// A request handed to a domain provider at one policy node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionRequest {
    pub plan_digest: String,
    pub sequence: u64,
    pub acquisition_id: String,
    pub declared_cost: f64,
}

impl AcquisitionRequest {
    fn new(plan_digest: &str, sequence: u64, acquisition: &Acquisition) -> Self {
        AcquisitionRequest {
            plan_digest: plan_digest.to_string(),
            sequence,
            acquisition_id: acquisition.id.clone(),
            declared_cost: acquisition.cost,
        }
    }
}

/// A provider's answer to an [`AcquisitionRequest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionObservation {
    pub provider: String,
    pub acquisition_id: String,
    pub outcome_label: String,
    /// A content hash for the provider's raw evidence or normalized result.
    pub evidence_digest: String,
    pub provenance: ObservationProvenance,
}

impl AcquisitionObservation {
    /// Creates a provider-declared external observation after checking its digest identity.
    pub fn observed(
        provider: impl Into<String>,
        acquisition_id: impl Into<String>,
        outcome_label: impl Into<String>,
        evidence_digest: impl Into<String>,
    ) -> Result<Self, AdaptiveExecutionError> {
        Self::new(
            provider,
            acquisition_id,
            outcome_label,
            evidence_digest,
            ObservationProvenance::Observed,
        )
    }

    /// Creates a deterministic result without claiming that a world was contacted.
    pub fn simulated(
        provider: impl Into<String>,
        acquisition_id: impl Into<String>,
        outcome_label: impl Into<String>,
        evidence_digest: impl Into<String>,
    ) -> Result<Self, AdaptiveExecutionError> {
        Self::new(
            provider,
            acquisition_id,
            outcome_label,
            evidence_digest,
            ObservationProvenance::Simulated,
        )
    }

    fn replayed(
        provider: impl Into<String>,
        acquisition_id: impl Into<String>,
        outcome_label: impl Into<String>,
        evidence_digest: impl Into<String>,
    ) -> Result<Self, AdaptiveExecutionError> {
        Self::new(
            provider,
            acquisition_id,
            outcome_label,
            evidence_digest,
            ObservationProvenance::Replayed,
        )
    }

    fn new(
        provider: impl Into<String>,
        acquisition_id: impl Into<String>,
        outcome_label: impl Into<String>,
        evidence_digest: impl Into<String>,
        provenance: ObservationProvenance,
    ) -> Result<Self, AdaptiveExecutionError> {
        let provider = provider.into();
        let acquisition_id = acquisition_id.into();
        let outcome_label = outcome_label.into();
        let evidence_digest = evidence_digest.into();
        non_empty_identifier("provider", &provider)?;
        non_empty_identifier("acquisition_id", &acquisition_id)?;
        non_empty_identifier("outcome_label", &outcome_label)?;
        ContentHash::parse(evidence_digest.clone()).map_err(|_| {
            AdaptiveExecutionError::InvalidObservation(format!(
                "evidence_digest must be a lowercase 64-character SHA-256 digest, got {evidence_digest:?}"
            ))
        })?;
        Ok(AcquisitionObservation {
            provider,
            acquisition_id,
            outcome_label,
            evidence_digest,
            provenance,
        })
    }
}

/// A receipt row for one validated request/observation pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationReceipt {
    pub sequence: u64,
    pub request: AcquisitionRequest,
    pub observation: AcquisitionObservation,
}

/// Why an execution stopped before reaching a terminal policy action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRefusal {
    AuthorizationRequired,
    AuthorizationMismatch,
    ProviderFailure,
    ProviderIdentityMismatch,
    AcquisitionIdentityMismatch,
    OutcomeNotDeclared,
    DuplicateAcquisition,
    ReceiptMismatch,
    InvalidObservation,
    BudgetExceeded,
    PolicyMalformed,
}

/// Whether a run reached the terminal action in the selected policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Completed,
    Partial,
    Refused,
}

/// The non-secret authorization summary carried by a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationSummary {
    pub granted: bool,
    pub grant_id: Option<String>,
    pub provider: Option<String>,
}

/// An explicit, plan-scoped grant for a provider handoff.
///
/// The kernel can validate scope and identity, but it cannot manufacture human consent or
/// provider authentication. Callers should mint this value only after their domain gate has
/// recorded those facts. The private fields prevent a receipt or a JSON payload from being used as
/// a grant accidentally; the public constructor is intentionally named `issue` to make the
/// authority boundary visible at call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionGrant {
    grant_id: String,
    plan_digest: String,
    provider: String,
}

impl ExecutionGrant {
    /// Issues a grant scoped to one plan digest and one provider identity.
    pub fn issue(
        grant_id: impl Into<String>,
        plan_digest: impl Into<String>,
        provider: impl Into<String>,
    ) -> Result<Self, AdaptiveExecutionError> {
        let grant_id = grant_id.into();
        let plan_digest = plan_digest.into();
        let provider = provider.into();
        non_empty_identifier("grant_id", &grant_id)?;
        ContentHash::parse(plan_digest.clone()).map_err(|_| {
            AdaptiveExecutionError::InvalidAuthorization(
                "plan_digest must be a lowercase 64-character SHA-256 digest".into(),
            )
        })?;
        non_empty_identifier("provider", &provider)?;
        Ok(ExecutionGrant {
            grant_id,
            plan_digest,
            provider,
        })
    }

    pub fn summary(&self) -> AuthorizationSummary {
        AuthorizationSummary {
            granted: true,
            grant_id: Some(self.grant_id.clone()),
            provider: Some(self.provider.clone()),
        }
    }
}

/// The complete audit result of one adaptive run or refusal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveExecutionReceipt {
    pub schema: String,
    pub plan_digest: String,
    pub provider: String,
    pub status: ExecutionStatus,
    pub authorization: AuthorizationSummary,
    pub observations: Vec<ObservationReceipt>,
    pub actual_acquisition_cost: f64,
    pub terminal_action: Option<usize>,
    pub terminal_risk: Option<f64>,
    pub refusal: Option<ExecutionRefusal>,
    pub refusal_detail: Option<String>,
}

impl AdaptiveExecutionReceipt {
    /// Whether the receipt claims a complete, externally observed run.
    pub fn is_completed(&self) -> bool {
        self.status == ExecutionStatus::Completed
    }

    /// Number of observations with each provenance class.
    pub fn provenance_counts(&self) -> (usize, usize, usize) {
        let mut observed = 0;
        let mut simulated = 0;
        let mut replayed = 0;
        for row in &self.observations {
            match row.observation.provenance {
                ObservationProvenance::Observed => observed += 1,
                ObservationProvenance::Simulated => simulated += 1,
                ObservationProvenance::Replayed => replayed += 1,
            }
        }
        (observed, simulated, replayed)
    }

    /// Performs inexpensive receipt-internal checks before a caller stores or forwards it.
    pub fn validate_shape(&self) -> Result<(), AdaptiveExecutionError> {
        if self.schema != ADAPTIVE_EXECUTION_SCHEMA {
            return Err(AdaptiveExecutionError::InvalidReceipt(
                "receipt schema is not the adaptive execution schema".into(),
            ));
        }
        ContentHash::parse(self.plan_digest.clone()).map_err(|_| {
            AdaptiveExecutionError::InvalidReceipt("receipt plan_digest is malformed".into())
        })?;
        non_empty_identifier("provider", &self.provider)?;
        if !self.actual_acquisition_cost.is_finite() || self.actual_acquisition_cost < 0.0 {
            return Err(AdaptiveExecutionError::InvalidReceipt(
                "actual_acquisition_cost must be finite and non-negative".into(),
            ));
        }
        if self.observations.len() > 16 {
            return Err(AdaptiveExecutionError::InvalidReceipt(
                "receipt contains more than the exact 16-step bound".into(),
            ));
        }
        let mut sequences = BTreeSet::new();
        for (index, row) in self.observations.iter().enumerate() {
            if row.sequence != index as u64 || !sequences.insert(row.sequence) {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "receipt sequences must be contiguous and unique".into(),
                ));
            }
            if row.request.plan_digest != self.plan_digest
                || row.request.sequence != row.sequence
                || row.request.acquisition_id != row.observation.acquisition_id
            {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "receipt request and observation identities do not reconcile".into(),
                ));
            }
            receipt_identifier("request acquisition_id", &row.request.acquisition_id)?;
            if !row.request.declared_cost.is_finite() || row.request.declared_cost < 0.0 {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "receipt request declared_cost must be finite and non-negative".into(),
                ));
            }
            receipt_identifier("observation provider", &row.observation.provider)?;
            if row.observation.provider != self.provider {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "receipt observation provider does not match receipt provider".into(),
                ));
            }
            receipt_identifier(
                "observation acquisition_id",
                &row.observation.acquisition_id,
            )?;
            receipt_identifier("observation outcome_label", &row.observation.outcome_label)?;
            ContentHash::parse(row.observation.evidence_digest.clone()).map_err(|_| {
                AdaptiveExecutionError::InvalidReceipt(
                    "receipt contains a malformed evidence digest".into(),
                )
            })?;
        }

        match (
            self.authorization.granted,
            self.authorization.grant_id.as_deref(),
            self.authorization.provider.as_deref(),
        ) {
            (true, Some(grant_id), Some(provider)) => {
                receipt_identifier("authorization grant_id", grant_id)?;
                receipt_identifier("authorization provider", provider)?;
            }
            (false, None, None) => {}
            _ => {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "receipt authorization summary is internally inconsistent".into(),
                ));
            }
        }
        if self.status == ExecutionStatus::Completed {
            if self.terminal_action.is_none()
                || self.terminal_risk.is_none()
                || !self.authorization.granted
                || self.authorization.provider.as_deref() != Some(self.provider.as_str())
                || self.refusal.is_some()
                || self.refusal_detail.is_some()
            {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "completed receipt must have a terminal action and no refusal".into(),
                ));
            }
        } else {
            if self.terminal_action.is_some() || self.terminal_risk.is_some() {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "partial or refused receipt cannot claim a terminal action".into(),
                ));
            }
            if self.refusal.is_none()
                || self
                    .refusal_detail
                    .as_deref()
                    .is_none_or(|detail| detail.trim().is_empty())
            {
                return Err(AdaptiveExecutionError::InvalidReceipt(
                    "partial or refused receipt must explain why execution stopped".into(),
                ));
            }
        }
        Ok(())
    }
}

/// A fully bound adaptive policy ready for a provider handoff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptivePlan {
    pub problem: DecisionProblem,
    pub belief: Belief,
    pub acquisitions: Vec<Acquisition>,
    pub budget: f64,
    pub max_steps: usize,
    pub policy: AdaptivePolicy,
}

impl AdaptivePlan {
    /// Computes and binds an exact policy to its decision inputs.
    pub fn new(
        problem: DecisionProblem,
        belief: Belief,
        acquisitions: Vec<Acquisition>,
        budget: f64,
        max_steps: usize,
    ) -> Result<Self, AdaptiveExecutionError> {
        let policy = adaptive_policy(&problem, &belief, &acquisitions, budget, max_steps)
            .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))?;
        let plan = AdaptivePlan {
            problem,
            belief,
            acquisitions,
            budget,
            max_steps,
            policy,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Binds a policy produced by another layer, such as the FIBER compiler, after rechecking the
    /// tree's identities, path caps, and objective decomposition.
    pub fn from_policy(
        problem: DecisionProblem,
        belief: Belief,
        acquisitions: Vec<Acquisition>,
        budget: f64,
        max_steps: usize,
        policy: AdaptivePolicy,
    ) -> Result<Self, AdaptiveExecutionError> {
        let plan = AdaptivePlan {
            problem,
            belief,
            acquisitions,
            budget,
            max_steps,
            policy,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// The canonical digest binding inputs and selected policy, not merely the tree by itself.
    pub fn digest(&self) -> Result<String, AdaptiveExecutionError> {
        let value = serde_json::to_value(self)
            .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))?;
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), AdaptiveExecutionError> {
        self.problem
            .validate()
            .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))?;
        self.belief
            .check_against(&self.problem)
            .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))?;
        if !self.budget.is_finite() || self.budget < 0.0 || self.max_steps > 16 {
            return Err(AdaptiveExecutionError::InvalidPlan(
                "budget or horizon is outside the exact execution bound".into(),
            ));
        }
        if self.acquisitions.is_empty() || self.acquisitions.len() > 16 {
            return Err(AdaptiveExecutionError::InvalidPlan(
                "acquisition count is outside the exact execution bound".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for acquisition in &self.acquisitions {
            acquisition
                .check_against(&self.problem)
                .map_err(|error| AdaptiveExecutionError::InvalidPlan(error.to_string()))?;
            if !ids.insert(acquisition.id.clone()) {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "acquisition identifiers must be unique".into(),
                ));
            }
        }
        if !self.policy.expected_total.is_finite()
            || !self.policy.expected_terminal_risk.is_finite()
            || !self.policy.expected_acquisition_cost.is_finite()
            || self.policy.expected_acquisition_cost < -EXECUTION_RECONCILIATION_EPSILON
            || self.policy.selected_depth > self.max_steps
            || self.policy.nodes_evaluated == 0
            || self.policy.nodes_evaluated > 65_536
        {
            return Err(AdaptiveExecutionError::InvalidPlan(
                "policy objective or enumeration metadata is malformed".into(),
            ));
        }
        let mut path = Vec::new();
        validate_node(
            &self.policy.root,
            &self.problem,
            &self.acquisitions,
            self.budget,
            self.max_steps,
            0,
            &mut path,
        )?;
        Ok(())
    }

    /// Executes only after `grant` scopes the exact plan digest and provider identity.
    pub fn execute<E: AcquisitionExecutor>(
        &self,
        grant: Option<&ExecutionGrant>,
        executor: &mut E,
    ) -> Result<AdaptiveExecutionReceipt, AdaptiveExecutionError> {
        self.validate()?;
        let plan_digest = self.digest()?;
        let provider = executor.provider_id().to_string();
        let authorization = match grant {
            None => return Ok(refused_receipt(
                &plan_digest,
                &provider,
                AuthorizationSummary {
                    granted: false,
                    grant_id: None,
                    provider: None,
                },
                ExecutionRefusal::AuthorizationRequired,
                "an explicit plan-scoped execution grant is required; no provider call was made",
            )),
            Some(grant) if grant.plan_digest != plan_digest || grant.provider != provider => {
                return Ok(refused_receipt(
                    &plan_digest,
                    &provider,
                    grant.summary(),
                    ExecutionRefusal::AuthorizationMismatch,
                    "grant scope does not match the bound plan digest and provider identity",
                ))
            }
            Some(grant) => grant.summary(),
        };

        let mut observations = Vec::new();
        let mut actual_cost = 0.0;
        let terminal = execute_node(
            &self.policy.root,
            &self.acquisitions,
            &plan_digest,
            executor,
            &mut observations,
            &mut actual_cost,
            self.budget,
        );
        match terminal {
            Ok((action, risk)) => {
                let receipt = AdaptiveExecutionReceipt {
                    schema: ADAPTIVE_EXECUTION_SCHEMA.into(),
                    plan_digest,
                    provider,
                    status: ExecutionStatus::Completed,
                    authorization,
                    observations,
                    actual_acquisition_cost: actual_cost,
                    terminal_action: Some(action),
                    terminal_risk: Some(risk),
                    refusal: None,
                    refusal_detail: None,
                };
                receipt.validate_shape()?;
                Ok(receipt)
            }
            Err(failure) => {
                let receipt = AdaptiveExecutionReceipt {
                    schema: ADAPTIVE_EXECUTION_SCHEMA.into(),
                    plan_digest,
                    provider,
                    status: if observations.is_empty() {
                        ExecutionStatus::Refused
                    } else {
                        ExecutionStatus::Partial
                    },
                    authorization,
                    observations,
                    actual_acquisition_cost: actual_cost,
                    terminal_action: None,
                    terminal_risk: None,
                    refusal: Some(failure.0),
                    refusal_detail: Some(failure.1),
                };
                receipt.validate_shape()?;
                Ok(receipt)
            }
        }
    }

    /// Replays a receipt through a provider that owns no live source.
    pub fn replay(
        &self,
        receipt: &AdaptiveExecutionReceipt,
    ) -> Result<AdaptiveExecutionReceipt, AdaptiveExecutionError> {
        receipt.validate_shape()?;
        let mut replay = ReceiptReplayExecutor::from_receipt(receipt)?;
        let grant = ExecutionGrant::issue(
            format!("replay:{}", receipt.plan_digest),
            self.digest()?,
            replay.provider_id(),
        )?;
        self.execute(Some(&grant), &mut replay)
    }
}

/// Domain adapter seam for one adaptive acquisition.
pub trait AcquisitionExecutor {
    /// Stable provider identity, included in the grant and every receipt.
    fn provider_id(&self) -> &str;

    /// Returns exactly one outcome for the requested acquisition, or a refusal/error string.
    fn acquire(&mut self, request: &AcquisitionRequest) -> Result<AcquisitionObservation, String>;
}

/// A replay-only executor built from receipt rows.
///
/// It has no field for a live source. A sequence or identity mismatch is a hard error, so replay
/// cannot silently continue with a provider response from the current world.
#[derive(Debug, Clone)]
pub struct ReceiptReplayExecutor {
    provider: String,
    observations: Vec<ObservationReceipt>,
    cursor: usize,
}

impl ReceiptReplayExecutor {
    pub fn from_receipt(
        receipt: &AdaptiveExecutionReceipt,
    ) -> Result<Self, AdaptiveExecutionError> {
        receipt.validate_shape()?;
        Ok(ReceiptReplayExecutor {
            provider: format!("replay:{}", receipt.provider),
            observations: receipt.observations.clone(),
            cursor: 0,
        })
    }
}

impl AcquisitionExecutor for ReceiptReplayExecutor {
    fn provider_id(&self) -> &str {
        &self.provider
    }

    fn acquire(&mut self, request: &AcquisitionRequest) -> Result<AcquisitionObservation, String> {
        let row = self
            .observations
            .get(self.cursor)
            .ok_or_else(|| "replay receipt has no row for this request".to_string())?;
        if row.sequence != request.sequence
            || row.request.acquisition_id != request.acquisition_id
            || row.request.plan_digest != request.plan_digest
        {
            return Err("replay request does not match the next recorded receipt row".into());
        }
        self.cursor += 1;
        AcquisitionObservation::replayed(
            self.provider.clone(),
            row.observation.acquisition_id.clone(),
            row.observation.outcome_label.clone(),
            row.observation.evidence_digest.clone(),
        )
        .map_err(|error| error.to_string())
    }
}

/// Typed failures for plan, grant, provider, and receipt boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdaptiveExecutionError {
    #[error("invalid adaptive execution plan: {0}")]
    InvalidPlan(String),
    #[error("invalid adaptive execution authorization: {0}")]
    InvalidAuthorization(String),
    #[error("invalid provider observation: {0}")]
    InvalidObservation(String),
    #[error("invalid adaptive execution receipt: {0}")]
    InvalidReceipt(String),
}

fn non_empty_identifier(field: &str, value: &str) -> Result<(), AdaptiveExecutionError> {
    if value.trim().is_empty()
        || value.len() > 256
        || value != value.trim()
        || value.chars().any(char::is_control)
    {
        return Err(AdaptiveExecutionError::InvalidObservation(format!(
            "{field} must contain between 1 and 256 bytes without surrounding whitespace or control characters"
        )));
    }
    Ok(())
}

fn receipt_identifier(field: &str, value: &str) -> Result<(), AdaptiveExecutionError> {
    non_empty_identifier(field, value).map_err(|_| {
        AdaptiveExecutionError::InvalidReceipt(format!(
            "receipt {field} must be a bounded identifier without surrounding whitespace or control characters"
        ))
    })
}

fn refused_receipt(
    plan_digest: &str,
    provider: &str,
    authorization: AuthorizationSummary,
    refusal: ExecutionRefusal,
    detail: &str,
) -> AdaptiveExecutionReceipt {
    AdaptiveExecutionReceipt {
        schema: ADAPTIVE_EXECUTION_SCHEMA.into(),
        plan_digest: plan_digest.into(),
        provider: provider.into(),
        status: ExecutionStatus::Refused,
        authorization,
        observations: Vec::new(),
        actual_acquisition_cost: 0.0,
        terminal_action: None,
        terminal_risk: None,
        refusal: Some(refusal),
        refusal_detail: Some(detail.into()),
    }
}

fn validate_node(
    node: &AdaptiveNode,
    problem: &DecisionProblem,
    acquisitions: &[Acquisition],
    budget: f64,
    max_steps: usize,
    depth: usize,
    path: &mut Vec<usize>,
) -> Result<(), AdaptiveExecutionError> {
    if depth > max_steps {
        return Err(AdaptiveExecutionError::InvalidPlan(
            "policy path exceeds max_steps".into(),
        ));
    }
    match node {
        AdaptiveNode::Stop { action, risk } => {
            if *action >= problem.action_count() || !risk.is_finite() {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "stop node action or risk is malformed".into(),
                ));
            }
        }
        AdaptiveNode::Acquire {
            acquisition,
            id,
            cost,
            expected_total,
            expected_terminal_risk,
            expected_acquisition_cost,
            outcomes,
        } => {
            let item = acquisitions.get(*acquisition).ok_or_else(|| {
                AdaptiveExecutionError::InvalidPlan(
                    "policy acquisition index is out of range".into(),
                )
            })?;
            if item.id != *id || (item.cost - *cost).abs() > EXECUTION_RECONCILIATION_EPSILON {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "policy acquisition identity or cost does not match its input".into(),
                ));
            }
            if path.contains(acquisition) {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "an acquisition repeats along one policy path".into(),
                ));
            }
            if *cost < 0.0
                || !cost.is_finite()
                || !expected_total.is_finite()
                || !expected_terminal_risk.is_finite()
                || !expected_acquisition_cost.is_finite()
            {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "acquire node carries a non-finite objective".into(),
                ));
            }
            if *cost > budget + EXECUTION_RECONCILIATION_EPSILON {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "policy path exceeds the declared budget".into(),
                ));
            }
            if outcomes.len() != item.outcomes().len() {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "policy does not enumerate every declared outcome".into(),
                ));
            }
            let mut labels = BTreeSet::new();
            let mut probability_sum = 0.0;
            for (index, outcome) in outcomes.iter().enumerate() {
                let declared = &item.outcomes()[index];
                if outcome.label != declared.label || !labels.insert(outcome.label.clone()) {
                    return Err(AdaptiveExecutionError::InvalidPlan(
                        "policy outcome labels do not match declared order".into(),
                    ));
                }
                if !outcome.probability.is_finite()
                    || outcome.probability < -EXECUTION_RECONCILIATION_EPSILON
                    || outcome.probability > 1.0 + EXECUTION_RECONCILIATION_EPSILON
                    || outcome.posterior.len() != problem.model_count()
                    || outcome
                        .posterior
                        .iter()
                        .any(|mass| !mass.is_finite() || *mass < 0.0)
                {
                    return Err(AdaptiveExecutionError::InvalidPlan(
                        "policy outcome probability or posterior is malformed".into(),
                    ));
                }
                probability_sum += outcome.probability;
                path.push(*acquisition);
                validate_node(
                    &outcome.next,
                    problem,
                    acquisitions,
                    (budget - cost).max(0.0),
                    max_steps,
                    depth + 1,
                    path,
                )?;
                path.pop();
            }
            if (probability_sum - 1.0).abs() > EXECUTION_RECONCILIATION_EPSILON {
                return Err(AdaptiveExecutionError::InvalidPlan(
                    "policy outcome probabilities do not sum to one".into(),
                ));
            }
        }
    }
    Ok(())
}

fn execute_node<E: AcquisitionExecutor>(
    node: &AdaptiveNode,
    acquisitions: &[Acquisition],
    plan_digest: &str,
    executor: &mut E,
    observations: &mut Vec<ObservationReceipt>,
    actual_cost: &mut f64,
    budget: f64,
) -> Result<(usize, f64), (ExecutionRefusal, String)> {
    match node {
        AdaptiveNode::Stop { action, risk } => Ok((*action, *risk)),
        AdaptiveNode::Acquire {
            acquisition,
            outcomes,
            ..
        } => {
            let item = acquisitions.get(*acquisition).ok_or_else(|| {
                (
                    ExecutionRefusal::PolicyMalformed,
                    "selected policy acquisition index is out of range".into(),
                )
            })?;
            if *actual_cost + item.cost > budget + EXECUTION_RECONCILIATION_EPSILON {
                return Err((
                    ExecutionRefusal::BudgetExceeded,
                    "actual acquisition cost would exceed the plan budget".into(),
                ));
            }
            let sequence = observations.len() as u64;
            let request = AcquisitionRequest::new(plan_digest, sequence, item);
            let response = executor
                .acquire(&request)
                .map_err(|detail| (ExecutionRefusal::ProviderFailure, detail))?;
            if response.provider != executor.provider_id() {
                return Err((
                    ExecutionRefusal::ProviderIdentityMismatch,
                    "provider observation identity does not match executor identity".into(),
                ));
            }
            if response.acquisition_id != item.id {
                return Err((
                    ExecutionRefusal::AcquisitionIdentityMismatch,
                    "provider returned an outcome for a different acquisition".into(),
                ));
            }
            let selected = outcomes
                .iter()
                .find(|outcome| outcome.label == response.outcome_label)
                .ok_or_else(|| {
                    (
                        ExecutionRefusal::OutcomeNotDeclared,
                        "provider returned an outcome label absent from the policy branch".into(),
                    )
                })?;
            ContentHash::parse(response.evidence_digest.clone()).map_err(|_| {
                (
                    ExecutionRefusal::InvalidObservation,
                    "provider returned a malformed evidence digest".into(),
                )
            })?;
            *actual_cost += item.cost;
            observations.push(ObservationReceipt {
                sequence,
                request,
                observation: response,
            });
            execute_node(
                &selected.next,
                acquisitions,
                plan_digest,
                executor,
                observations,
                actual_cost,
                budget,
            )
        }
    }
}

/// A tiny deterministic adapter useful for local tests and provider simulations.
#[derive(Debug, Clone)]
pub struct ScriptedExecutor {
    provider: String,
    outcomes: Vec<(String, String, ObservationProvenance)>,
    cursor: usize,
}

impl ScriptedExecutor {
    /// The script is marked simulated so it cannot be mistaken for an external observation.
    pub fn simulated(provider: impl Into<String>, outcomes: Vec<(String, String)>) -> Self {
        ScriptedExecutor {
            provider: provider.into(),
            outcomes: outcomes
                .into_iter()
                .map(|(acquisition, outcome)| {
                    (acquisition, outcome, ObservationProvenance::Simulated)
                })
                .collect(),
            cursor: 0,
        }
    }
}

impl AcquisitionExecutor for ScriptedExecutor {
    fn provider_id(&self) -> &str {
        &self.provider
    }

    fn acquire(&mut self, request: &AcquisitionRequest) -> Result<AcquisitionObservation, String> {
        let (acquisition_id, outcome_label, provenance) =
            self.outcomes
                .get(self.cursor)
                .cloned()
                .ok_or_else(|| "script has no outcome for this request".to_string())?;
        if acquisition_id != request.acquisition_id {
            return Err("script acquisition identity does not match the policy request".into());
        }
        self.cursor += 1;
        let payload = json!({
            "provider": self.provider,
            "sequence": request.sequence,
            "acquisition_id": acquisition_id,
            "outcome_label": outcome_label,
            "provenance": provenance,
        });
        let digest = ContentHash::of_value(&payload)
            .map_err(|error| error.to_string())?
            .as_str()
            .to_string();
        AcquisitionObservation::new(
            self.provider.clone(),
            acquisition_id,
            outcome_label,
            digest,
            provenance,
        )
        .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Acquisition, Belief, DecisionProblem};

    fn plan() -> AdaptivePlan {
        let problem = DecisionProblem::new(
            vec!["choose-m0".into(), "choose-m1".into()],
            vec!["m0".into(), "m1".into()],
            vec![0.0, 1.0, 1.0, 0.0],
        )
        .unwrap();
        AdaptivePlan::new(
            problem,
            Belief::new(vec![0.9, 0.1]).unwrap(),
            vec![
                Acquisition::binary("screen", 0.01, vec![0.9, 0.2]).unwrap(),
                Acquisition::binary("confirm", 0.1, vec![0.01, 0.99]).unwrap(),
            ],
            0.11,
            2,
        )
        .unwrap()
    }

    #[test]
    fn no_grant_refuses_without_calling_provider() {
        let plan = plan();
        let mut executor =
            ScriptedExecutor::simulated("lab", vec![("screen".into(), "negative".into())]);
        let receipt = plan.execute(None, &mut executor).unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Refused);
        assert_eq!(
            receipt.refusal,
            Some(ExecutionRefusal::AuthorizationRequired)
        );
        assert!(receipt.observations.is_empty());
    }

    #[test]
    fn grant_executes_only_declared_branches_and_preserves_simulation_provenance() {
        let plan = plan();
        let digest = plan.digest().unwrap();
        let grant = ExecutionGrant::issue("grant-1", digest, "lab").unwrap();
        let mut executor = ScriptedExecutor::simulated(
            "lab",
            vec![
                ("screen".into(), "negative".into()),
                ("confirm".into(), "negative".into()),
            ],
        );
        let receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Completed);
        assert_eq!(receipt.observations.len(), 2);
        assert!(receipt
            .observations
            .iter()
            .all(|row| row.observation.provenance == ObservationProvenance::Simulated));
        assert_eq!(receipt.provenance_counts(), (0, 2, 0));
        receipt.validate_shape().unwrap();
    }

    #[test]
    fn unexpected_outcome_is_a_partial_fail_closed_receipt() {
        let plan = plan();
        let grant = ExecutionGrant::issue("grant-1", plan.digest().unwrap(), "lab").unwrap();
        let mut executor =
            ScriptedExecutor::simulated("lab", vec![("screen".into(), "invented".into())]);
        let receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Refused);
        assert_eq!(receipt.refusal, Some(ExecutionRefusal::OutcomeNotDeclared));
        assert!(receipt.observations.is_empty());
    }

    #[test]
    fn incomplete_failure_receipts_are_rejected_at_the_boundary() {
        let plan = plan();
        let grant = ExecutionGrant::issue("grant-1", plan.digest().unwrap(), "lab").unwrap();
        let mut executor =
            ScriptedExecutor::simulated("lab", vec![("screen".into(), "invented".into())]);
        let mut receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        receipt.refusal = None;
        assert!(matches!(
            receipt.validate_shape(),
            Err(AdaptiveExecutionError::InvalidReceipt(_))
        ));

        let mut executor =
            ScriptedExecutor::simulated("lab", vec![("screen".into(), "invented".into())]);
        let mut receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        receipt.refusal_detail = Some("  \n".into());
        assert!(matches!(
            receipt.validate_shape(),
            Err(AdaptiveExecutionError::InvalidReceipt(_))
        ));
    }

    #[test]
    fn receipt_authorization_and_observation_provenance_must_reconcile() {
        let plan = plan();
        let grant = ExecutionGrant::issue("grant-1", plan.digest().unwrap(), "lab").unwrap();
        let mut executor = ScriptedExecutor::simulated(
            "lab",
            vec![
                ("screen".into(), "negative".into()),
                ("confirm".into(), "negative".into()),
            ],
        );
        let mut receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        receipt.authorization.granted = false;
        receipt.authorization.grant_id = None;
        receipt.authorization.provider = None;
        assert!(matches!(
            receipt.validate_shape(),
            Err(AdaptiveExecutionError::InvalidReceipt(_))
        ));

        let mut executor = ScriptedExecutor::simulated(
            "lab",
            vec![
                ("screen".into(), "negative".into()),
                ("confirm".into(), "negative".into()),
            ],
        );
        let mut receipt = plan.execute(Some(&grant), &mut executor).unwrap();
        receipt.observations[0].observation.provider = "other-provider".into();
        assert!(matches!(
            receipt.validate_shape(),
            Err(AdaptiveExecutionError::InvalidReceipt(_))
        ));
    }

    #[test]
    fn replay_has_no_live_source_and_changes_provenance_to_replayed() {
        let plan = plan();
        let grant = ExecutionGrant::issue("grant-1", plan.digest().unwrap(), "lab").unwrap();
        let mut executor = ScriptedExecutor::simulated(
            "lab",
            vec![
                ("screen".into(), "negative".into()),
                ("confirm".into(), "negative".into()),
            ],
        );
        let original = plan.execute(Some(&grant), &mut executor).unwrap();
        let replayed = plan.replay(&original).unwrap();
        assert_eq!(replayed.status, ExecutionStatus::Completed);
        assert_eq!(replayed.observations.len(), original.observations.len());
        assert!(replayed
            .observations
            .iter()
            .all(|row| row.observation.provenance == ObservationProvenance::Replayed));
        assert_eq!(
            replayed.observations[0].observation.outcome_label,
            "negative"
        );
    }

    #[test]
    fn plan_digest_binds_inputs_and_policy() {
        let first = plan();
        let mut second = first.clone();
        second.budget = 0.10;
        assert_ne!(first.digest().unwrap(), second.digest().unwrap());
    }
}
