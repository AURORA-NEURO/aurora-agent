//! A provider-neutral autonomous brain kernel.
//!
//! This crate implements the part of an agent that must remain deterministic and inspectable
//! even when the eventual language model is stochastic: model selection under explicit resource
//! constraints, prompt assembly with omission accounting, bounded DAG planning, and a guarded
//! online bandit ledger. It is the executable kernel for the model-selection and autonomous
//! planning layers named by blueprint sections 09.08, 09.11, 11.18, and 11.20.
//!
//! The kernel deliberately does not open sockets, read environment variables, store provider
//! keys, execute tools, or claim that a model response is correct. Those effects belong at an
//! application-owned runtime boundary. A runtime may use [`bioprism_runtime::SecretBroker`] or
//! the Python SDK's in-memory credential store, then pass only an opaque credential handle and
//! the resulting value-free metadata back here. This separation makes a user-supplied key
//! possible without making the key part of an MCP argument, plan, certificate, or learning state.
//!
//! The learning implementation is UCB-style rather than a claim of reinforcement learning in
//! the statistical sense. It updates only from an explicit bounded reward supplied by an
//! evaluator, records failures separately, and never mutates hidden global state. A future
//! contextual policy can consume the same state schema without invalidating existing ledgers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const BRAIN_SCHEMA_VERSION: &str = "bioprism-autonomous-brain/0.1";
pub const MODEL_SELECTION_SCHEMA: &str = "bioprism-brain-model-selection/0.1";
pub const CONTEXTUAL_MODEL_SELECTION_SCHEMA: &str = "bioprism-brain-contextual-model-selection/0.1";
pub const PROMPT_ASSEMBLY_SCHEMA: &str = "bioprism-brain-prompt-assembly/0.1";
pub const PLAN_SCHEMA: &str = "bioprism-brain-plan/0.1";
pub const BANDIT_SCHEMA: &str = "bioprism-brain-bandit/0.1";
pub const LEARNING_EVIDENCE_SCHEMA: &str = "bioprism-brain-learning-evidence/0.1";
pub const PROVIDER_HEALTH_SCHEMA: &str = "bioprism-brain-provider-health/0.1";

const MAX_MODELS: usize = 256;
const MAX_PROMPT_CHUNKS: usize = 512;
const MAX_PLAN_STEPS: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 256;
const MAX_EVALUATOR_ID_BYTES: usize = 256;
const MAX_CONTEXT_LABEL_BYTES: usize = 256;

#[derive(Debug, Error)]
pub enum BrainError {
    #[error("{field} must be non-empty")]
    Empty { field: &'static str },
    #[error("{field} is over the {max}-item bound")]
    TooMany { field: &'static str, max: usize },
    #[error("{field} must be finite and within [{min}, {max}]")]
    OutOfRange {
        field: &'static str,
        min: f64,
        max: f64,
    },
    #[error("model selection refused: no eligible model remains")]
    NoEligibleModel,
    #[error("prompt assembly refused: required content exceeds the input-token budget")]
    RequiredPromptDoesNotFit,
    #[error("plan step {step:?} references unknown dependency {dependency:?}")]
    UnknownDependency { step: String, dependency: String },
    #[error("plan contains a dependency cycle")]
    PlanCycle,
    #[error("plan step {step:?} uses a tool that is not allowed")]
    ToolNotAllowed { step: String },
    #[error("bandit arm {0:?} is not present")]
    UnknownArm(String),
    #[error("contextual observation digest does not match the selected context")]
    ContextDigestMismatch,
    #[error("contextual observations contain duplicate arm {0:?}")]
    DuplicateContextObservation(String),
    #[error("bandit state contains duplicate arm {0:?}")]
    DuplicateArm(String),
    #[error("bandit reward is outside the configured range")]
    InvalidReward,
    #[error("assessment cannot be both passed and failed")]
    ContradictoryAssessment,
    #[error("invalid provider health posture for {0:?}")]
    InvalidProviderHealth(String),
    #[error("{field} must be a lowercase SHA-256 digest")]
    InvalidDigest { field: &'static str },
    #[error("invalid JSON for digest: {0}")]
    Json(#[from] serde_json::Error),
}

fn non_empty(value: &str, field: &'static str) -> Result<(), BrainError> {
    if value.trim().is_empty() {
        Err(BrainError::Empty { field })
    } else {
        Ok(())
    }
}

fn finite_range(value: f64, field: &'static str, min: f64, max: f64) -> Result<(), BrainError> {
    if value.is_finite() && value >= min && value <= max {
        Ok(())
    } else {
        Err(BrainError::OutOfRange { field, min, max })
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String, BrainError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_digest_value(value: &str, field: &'static str) -> Result<(), BrainError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(BrainError::InvalidDigest { field });
    }
    Ok(())
}

/// Metadata describing a model that an application has made available to the brain.
///
/// This is not a credential record. `requires_credential` describes the runtime contract while
/// the actual credential remains outside the serialized model catalogue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelDescriptor {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub context_window_tokens: u64,
    pub max_output_tokens: u64,
    /// Normalized quality prior in `[0, 1]`, supplied by the application or evaluator.
    pub quality: f64,
    /// Expected end-to-end latency in milliseconds.
    pub latency_ms: u64,
    /// Cost in integer micro-units per million tokens; the unit is caller-defined but stable
    /// within one selection request.
    pub cost_per_million_tokens: u64,
    /// Availability prior in `[0, 1]`; it is not provider authentication.
    pub reliability: f64,
    #[serde(default)]
    pub requires_credential: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl ModelDescriptor {
    fn validate(&self) -> Result<(), BrainError> {
        non_empty(&self.provider, "provider")?;
        non_empty(&self.model, "model")?;
        finite_range(self.quality, "quality", 0.0, 1.0)?;
        finite_range(self.reliability, "reliability", 0.0, 1.0)
    }

    fn id(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelObservation {
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionWeights {
    #[serde(default = "default_quality_weight")]
    pub quality: f64,
    #[serde(default = "default_reliability_weight")]
    pub reliability: f64,
    #[serde(default = "default_cost_weight")]
    pub cost: f64,
    #[serde(default = "default_latency_weight")]
    pub latency: f64,
    #[serde(default = "default_exploration_weight")]
    pub exploration: f64,
}

fn default_quality_weight() -> f64 {
    0.55
}
fn default_reliability_weight() -> f64 {
    0.25
}
fn default_cost_weight() -> f64 {
    0.10
}
fn default_latency_weight() -> f64 {
    0.10
}
fn default_exploration_weight() -> f64 {
    0.15
}

impl Default for SelectionWeights {
    fn default() -> Self {
        Self {
            quality: default_quality_weight(),
            reliability: default_reliability_weight(),
            cost: default_cost_weight(),
            latency: default_latency_weight(),
            exploration: default_exploration_weight(),
        }
    }
}

impl SelectionWeights {
    fn validate(&self) -> Result<(), BrainError> {
        for (name, value) in [
            ("weights.quality", self.quality),
            ("weights.reliability", self.reliability),
            ("weights.cost", self.cost),
            ("weights.latency", self.latency),
            ("weights.exploration", self.exploration),
        ] {
            finite_range(value, name, 0.0, 100.0)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSelectionRequest {
    pub task: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub input_tokens: u64,
    pub requested_output_tokens: u64,
    #[serde(default)]
    pub max_cost_per_million_tokens: Option<u64>,
    #[serde(default)]
    pub max_latency_ms: Option<u64>,
    #[serde(default)]
    pub min_quality: Option<f64>,
    pub models: Vec<ModelDescriptor>,
    #[serde(default)]
    pub observations: Vec<ModelObservation>,
    #[serde(default)]
    pub weights: SelectionWeights,
    /// Runtime-supplied provider posture. Credentials remain outside this request; this map only
    /// carries bounded readiness/circuit metadata so the kernel can refuse unhealthy providers.
    #[serde(default)]
    pub provider_health: BTreeMap<String, ProviderHealth>,
}

fn default_provider_registered() -> bool {
    true
}

fn default_provider_circuit() -> String {
    "closed".into()
}

fn default_provider_credential_ready() -> bool {
    true
}

fn default_provider_eligible() -> bool {
    true
}

/// Value-only runtime posture for one provider. It is deliberately not a credential record and
/// never contains key material, endpoint secrets, or provider response content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHealth {
    #[serde(default = "default_provider_registered")]
    pub registered: bool,
    #[serde(default = "default_provider_circuit")]
    pub circuit: String,
    #[serde(default)]
    pub consecutive_failures: u64,
    #[serde(default = "default_provider_credential_ready")]
    pub credential_ready: bool,
    #[serde(default = "default_provider_eligible")]
    pub eligible: bool,
}

impl ProviderHealth {
    fn validate(&self, provider: &str) -> Result<(), BrainError> {
        non_empty(provider, "provider_health provider")?;
        non_empty(&self.circuit, "provider_health.circuit")?;
        if !matches!(
            self.circuit.as_str(),
            "closed" | "half_open" | "open" | "unconfigured"
        ) {
            return Err(BrainError::InvalidProviderHealth(provider.to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCandidateScore {
    pub model_id: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub base_score: f64,
    pub exploration_bonus: f64,
    pub score: f64,
    pub observed_pulls: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSelectionReport {
    pub schema: String,
    pub task: String,
    pub selected_model: Option<ModelDescriptor>,
    pub selected_model_id: Option<String>,
    pub ranking: Vec<ModelCandidateScore>,
    pub selection_status: String,
    pub decision_digest: String,
    pub does_not_claim: Vec<String>,
}

/// Select a model using hard capability/resource gates followed by a deterministic utility/UCB
/// ranking. Every candidate remains in the report, including rejected models, so the caller can
/// explain why a model was not selected.
pub fn select_model(request: &ModelSelectionRequest) -> Result<ModelSelectionReport, BrainError> {
    non_empty(&request.task, "task")?;
    if request.models.is_empty() {
        return Err(BrainError::NoEligibleModel);
    }
    if request.models.len() > MAX_MODELS {
        return Err(BrainError::TooMany {
            field: "models",
            max: MAX_MODELS,
        });
    }
    if request.provider_health.len() > MAX_MODELS {
        return Err(BrainError::TooMany {
            field: "provider_health",
            max: MAX_MODELS,
        });
    }
    for (provider, health) in &request.provider_health {
        health.validate(provider)?;
    }
    request.weights.validate()?;
    if let Some(min_quality) = request.min_quality {
        finite_range(min_quality, "min_quality", 0.0, 1.0)?;
    }

    let mut observations = BTreeMap::new();
    for observation in &request.observations {
        non_empty(&observation.arm_id, "observation.arm_id")?;
        finite_range(
            observation.reward_sum,
            "observation.reward_sum",
            -1e12,
            1e12,
        )?;
        if observations
            .insert(observation.arm_id.clone(), observation)
            .is_some()
        {
            return Err(BrainError::DuplicateArm(observation.arm_id.clone()));
        }
    }
    let max_cost = request
        .models
        .iter()
        .map(|model| model.cost_per_million_tokens)
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let max_latency = request
        .models
        .iter()
        .map(|model| model.latency_ms)
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let total_pulls = request
        .observations
        .iter()
        .map(|observation| observation.pulls)
        .sum::<u64>();
    let log_total = ((total_pulls + 1) as f64).ln();

    let mut ranking = Vec::with_capacity(request.models.len());
    for model in &request.models {
        model.validate()?;
        let model_id = model.id();
        let observation = observations.get(&model_id).copied();
        let mut reasons = Vec::new();
        if !model.enabled {
            reasons.push("disabled_by_caller".into());
        }
        if model.context_window_tokens
            < request
                .input_tokens
                .saturating_add(request.requested_output_tokens)
        {
            reasons.push("context_window_too_small".into());
        }
        if model.max_output_tokens < request.requested_output_tokens {
            reasons.push("max_output_tokens_too_small".into());
        }
        if let Some(max_cost) = request.max_cost_per_million_tokens {
            if model.cost_per_million_tokens > max_cost {
                reasons.push("cost_limit_exceeded".into());
            }
        }
        if let Some(max_latency) = request.max_latency_ms {
            if model.latency_ms > max_latency {
                reasons.push("latency_limit_exceeded".into());
            }
        }
        if let Some(min_quality) = request.min_quality {
            if model.quality < min_quality {
                reasons.push("quality_floor_not_met".into());
            }
        }
        for capability in &request.required_capabilities {
            if !model.capabilities.iter().any(|item| item == capability) {
                reasons.push(format!("missing_capability:{capability}"));
            }
        }
        if let Some(health) = request.provider_health.get(&model.provider) {
            if !health.registered {
                reasons.push("provider_unregistered".into());
            }
            if !health.credential_ready {
                reasons.push("provider_credential_unready".into());
            }
            if health.circuit == "open" {
                reasons.push("provider_circuit_open".into());
            }
            if !health.eligible {
                reasons.push("provider_health_ineligible".into());
            }
        }
        if observation.is_some_and(|item| item.disabled) {
            reasons.push("disabled_by_learning_policy".into());
        }
        let eligible = reasons.is_empty();
        let pulls = observation.map(|item| item.pulls).unwrap_or(0);
        let mean_reward = observation
            .filter(|item| item.pulls > 0)
            .map(|item| item.reward_sum / item.pulls as f64)
            .unwrap_or(0.0);
        let exploration_bonus = if pulls == 0 {
            request.weights.exploration
        } else {
            request.weights.exploration * (log_total / pulls as f64).sqrt()
        };
        let base_score = request.weights.quality * model.quality
            + request.weights.reliability * model.reliability
            + request.weights.exploration * mean_reward
            - request.weights.cost * (model.cost_per_million_tokens as f64 / max_cost)
            - request.weights.latency * (model.latency_ms as f64 / max_latency);
        ranking.push(ModelCandidateScore {
            model_id,
            eligible,
            reasons,
            base_score,
            exploration_bonus,
            score: base_score + exploration_bonus,
            observed_pulls: pulls,
        });
    }
    ranking.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.model_id.cmp(&right.model_id))
    });
    let selected_model_id = ranking
        .iter()
        .find(|candidate| candidate.eligible)
        .map(|candidate| candidate.model_id.clone());
    let selected_model = selected_model_id.as_ref().and_then(|id| {
        request
            .models
            .iter()
            .find(|model| model.id() == *id)
            .cloned()
    });
    let selection_status = if selected_model_id.is_some() {
        "selected"
    } else {
        "refused_no_eligible_model"
    };
    let mut report = ModelSelectionReport {
        schema: MODEL_SELECTION_SCHEMA.into(),
        task: request.task.clone(),
        selected_model,
        selected_model_id,
        ranking,
        selection_status: selection_status.into(),
        decision_digest: String::new(),
        does_not_claim: vec![
            "model quality priors are caller-supplied and are not an evaluation result".into(),
            "selection does not authenticate a provider or redeem a credential".into(),
            "selection does not execute a model call or verify a future answer".into(),
        ],
    };
    let digest_input = report.clone();
    report.decision_digest = digest(&digest_input)?;
    if report.selected_model_id.is_none() {
        return Ok(report);
    }
    Ok(report)
}

/// Stable, non-secret context labels used to keep online model observations scoped to a domain
/// and risk posture. The raw task remains in the base selection request; this structure is safe to
/// retain in a learning ledger and its digest is the join key for contextual observations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelectionContext {
    pub domain: String,
    pub capability: String,
    pub risk_class: String,
    #[serde(default)]
    pub task_family: Option<String>,
}

impl ModelSelectionContext {
    fn validate(&self) -> Result<(), BrainError> {
        for (field, value) in [
            ("context.domain", &self.domain),
            ("context.capability", &self.capability),
            ("context.risk_class", &self.risk_class),
        ] {
            non_empty(value, field)?;
            if value.len() > MAX_CONTEXT_LABEL_BYTES {
                return Err(BrainError::TooMany {
                    field,
                    max: MAX_CONTEXT_LABEL_BYTES,
                });
            }
        }
        if let Some(task_family) = &self.task_family {
            non_empty(task_family, "context.task_family")?;
            if task_family.len() > MAX_CONTEXT_LABEL_BYTES {
                return Err(BrainError::TooMany {
                    field: "context.task_family",
                    max: MAX_CONTEXT_LABEL_BYTES,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelObservation {
    pub context_digest: String,
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelSelectionRequest {
    pub context: ModelSelectionContext,
    pub base: ModelSelectionRequest,
    #[serde(default)]
    pub observations: Vec<ContextualModelObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelSelectionReport {
    pub schema: String,
    pub context: ModelSelectionContext,
    pub context_digest: String,
    pub selection: ModelSelectionReport,
    pub contextual_observations_used: usize,
    pub global_observation_fallbacks: usize,
    pub selection_status: String,
    pub does_not_claim: Vec<String>,
}

/// Select a model with observations scoped to one domain/capability/risk context.
///
/// Exact contextual observations override global observations for the same model arm. Missing
/// contextual observations fall back to the base request's global observation, so a new domain is
/// exploratory without erasing useful system-wide history. The server retains no hidden state.
pub fn select_model_contextual(
    request: &ContextualModelSelectionRequest,
) -> Result<ContextualModelSelectionReport, BrainError> {
    request.context.validate()?;
    let context_digest = digest(&request.context)?;
    let mut base = request.base.clone();
    let global_arm_ids = base
        .observations
        .iter()
        .map(|observation| observation.arm_id.clone())
        .collect::<BTreeSet<_>>();
    let mut contextual_by_arm = BTreeMap::new();
    for observation in &request.observations {
        if observation.context_digest != context_digest {
            return Err(BrainError::ContextDigestMismatch);
        }
        if contextual_by_arm
            .insert(observation.arm_id.clone(), observation)
            .is_some()
        {
            return Err(BrainError::DuplicateContextObservation(
                observation.arm_id.clone(),
            ));
        }
    }
    let mut merged = base.observations.clone();
    let global_fallbacks = request
        .base
        .observations
        .iter()
        .filter(|observation| !contextual_by_arm.contains_key(&observation.arm_id))
        .count();
    for observation in contextual_by_arm.values() {
        let replacement = ModelObservation {
            arm_id: observation.arm_id.clone(),
            pulls: observation.pulls,
            reward_sum: observation.reward_sum,
            failures: observation.failures,
            disabled: observation.disabled,
        };
        if let Some(existing) = merged
            .iter_mut()
            .find(|existing| existing.arm_id == replacement.arm_id)
        {
            *existing = replacement;
        } else {
            merged.push(replacement);
        }
    }
    base.observations = merged;
    let selection = select_model(&base)?;
    let contextual_observations_used = request.observations.len();
    let selection_status = if contextual_observations_used == 0 {
        "contextual_selection_global_history_only"
    } else if global_fallbacks == global_arm_ids.len() {
        "contextual_selection_exact_history"
    } else {
        "contextual_selection_mixed_history"
    };
    Ok(ContextualModelSelectionReport {
        schema: CONTEXTUAL_MODEL_SELECTION_SCHEMA.into(),
        context: request.context.clone(),
        context_digest,
        selection,
        contextual_observations_used,
        global_observation_fallbacks: global_fallbacks,
        selection_status: selection_status.into(),
        does_not_claim: vec![
            "context labels scope observations but do not prove domain similarity".into(),
            "a contextual reward remains evaluator-supplied and does not verify a model answer"
                .into(),
            "the contextual selector does not authenticate providers or redeem credentials".into(),
        ],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptChunk {
    pub id: String,
    #[serde(default = "default_user_role")]
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_user_role() -> String {
    "user".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAssemblyRequest {
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub developer: Option<String>,
    pub task: String,
    #[serde(default)]
    pub context: Vec<PromptChunk>,
    #[serde(default)]
    pub output_contract: Option<String>,
    pub max_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAssemblyReport {
    pub schema: String,
    pub messages: Vec<PromptMessage>,
    pub included_context_ids: Vec<String>,
    pub omitted_context_ids: Vec<String>,
    pub estimated_input_tokens: u64,
    pub complete: bool,
    pub prompt_digest: String,
    pub warnings: Vec<String>,
}

fn estimate_tokens(text: &str) -> u64 {
    ((text.chars().count() as u64).saturating_add(3) / 4).max(1)
}

fn validate_role(role: &str) -> Result<(), BrainError> {
    if matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
        Ok(())
    } else {
        Err(BrainError::Empty {
            field: "prompt role",
        })
    }
}

/// Assemble a bounded prompt while preserving the IDs of context that did not fit. Required
/// material fails closed; optional material is omitted explicitly rather than silently truncated.
pub fn assemble_prompt(
    request: &PromptAssemblyRequest,
) -> Result<PromptAssemblyReport, BrainError> {
    non_empty(&request.task, "task")?;
    if request.max_input_tokens == 0 {
        return Err(BrainError::OutOfRange {
            field: "max_input_tokens",
            min: 1.0,
            max: u64::MAX as f64,
        });
    }
    if request.context.len() > MAX_PROMPT_CHUNKS {
        return Err(BrainError::TooMany {
            field: "context",
            max: MAX_PROMPT_CHUNKS,
        });
    }
    for chunk in &request.context {
        non_empty(&chunk.id, "context.id")?;
        non_empty(&chunk.content, "context.content")?;
        validate_role(&chunk.role)?;
    }
    let mut messages = Vec::new();
    if let Some(system) = &request.system {
        if !system.is_empty() {
            messages.push(PromptMessage {
                role: "system".into(),
                content: system.clone(),
                source_id: "system".into(),
            });
        }
    }
    if let Some(developer) = &request.developer {
        if !developer.is_empty() {
            messages.push(PromptMessage {
                role: "developer".into(),
                content: developer.clone(),
                source_id: "developer".into(),
            });
        }
    }
    let mut base_tokens = messages
        .iter()
        .map(|message| estimate_tokens(&message.content))
        .sum::<u64>();
    if let Some(contract) = &request.output_contract {
        if !contract.is_empty() {
            base_tokens = base_tokens.saturating_add(estimate_tokens(contract));
        }
    }
    base_tokens = base_tokens.saturating_add(estimate_tokens(&request.task));
    if base_tokens > request.max_input_tokens {
        return Err(BrainError::RequiredPromptDoesNotFit);
    }
    let mut chunks = request.context.iter().collect::<Vec<_>>();
    chunks.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut included_context_ids = Vec::new();
    let mut omitted_context_ids = Vec::new();
    let mut tokens = base_tokens;
    for chunk in chunks {
        let cost = estimate_tokens(&chunk.content);
        if tokens.saturating_add(cost) <= request.max_input_tokens {
            tokens = tokens.saturating_add(cost);
            included_context_ids.push(chunk.id.clone());
            messages.push(PromptMessage {
                role: chunk.role.clone(),
                content: chunk.content.clone(),
                source_id: chunk.id.clone(),
            });
        } else if chunk.required {
            return Err(BrainError::RequiredPromptDoesNotFit);
        } else {
            omitted_context_ids.push(chunk.id.clone());
        }
    }
    let mut task_content = request.task.clone();
    if let Some(contract) = &request.output_contract {
        if !contract.is_empty() {
            task_content.push_str("\n\nOutput contract:\n");
            task_content.push_str(contract);
        }
    }
    messages.push(PromptMessage {
        role: "user".into(),
        content: task_content,
        source_id: "task".into(),
    });
    let complete = omitted_context_ids.is_empty();
    let mut report = PromptAssemblyReport {
        schema: PROMPT_ASSEMBLY_SCHEMA.into(),
        messages,
        included_context_ids,
        omitted_context_ids,
        estimated_input_tokens: tokens,
        complete,
        prompt_digest: String::new(),
        warnings: Vec::new(),
    };
    if !report.complete {
        report.warnings.push(
            "optional context was omitted to satisfy the input budget; omission is not zero influence".into(),
        );
    }
    let digest_input = report.clone();
    report.prompt_digest = digest(&digest_input)?;
    Ok(report)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanEffect {
    #[default]
    ReadOnly,
    ProviderCall,
    ExternalWrite,
    Irreversible,
}

impl PlanEffect {
    fn needs_approval(&self) -> bool {
        !matches!(self, Self::ReadOnly)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanStep {
    pub id: String,
    pub objective: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub effect: PlanEffect,
    #[serde(default)]
    pub estimated_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlanRequest {
    pub objective: String,
    pub steps: Vec<PlanStep>,
    pub allowed_tools: Vec<String>,
    pub max_cost: u64,
    #[serde(default = "default_true")]
    pub require_approval_for_effects: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlan {
    pub schema: String,
    pub objective: String,
    pub ordered_step_ids: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub estimated_cost: u64,
    pub requires_approval: bool,
    pub execution: String,
    pub plan_digest: String,
    pub does_not_claim: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlanReport {
    pub ok: bool,
    pub status: String,
    pub plan: Option<AutonomousPlan>,
    pub errors: Vec<String>,
}

/// Validate and topologically order a proposed plan. This is planning, never execution: the
/// returned `execution` field is always `not_started`, and non-read-only steps remain approval
/// gated even when the dependency graph is valid.
pub fn plan_autonomous(
    request: &AutonomousPlanRequest,
) -> Result<AutonomousPlanReport, BrainError> {
    non_empty(&request.objective, "objective")?;
    if request.steps.is_empty() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_empty_plan".into(),
            plan: None,
            errors: vec!["at least one step is required".into()],
        });
    }
    if request.steps.len() > MAX_PLAN_STEPS {
        return Err(BrainError::TooMany {
            field: "steps",
            max: MAX_PLAN_STEPS,
        });
    }
    let allowed = request
        .allowed_tools
        .iter()
        .filter(|tool| !tool.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut by_id = BTreeMap::new();
    let mut errors = Vec::new();
    for step in &request.steps {
        non_empty(&step.id, "step.id")?;
        non_empty(&step.objective, "step.objective")?;
        non_empty(&step.tool, "step.tool")?;
        if step.tool.len() > MAX_TOOL_NAME_BYTES {
            errors.push(format!("step {} tool name is too long", step.id));
        }
        if !allowed.contains(&step.tool) {
            errors.push(
                BrainError::ToolNotAllowed {
                    step: step.id.clone(),
                }
                .to_string(),
            );
        }
        if by_id.insert(step.id.clone(), step).is_some() {
            errors.push(format!("duplicate step id {:?}", step.id));
        }
    }
    for step in &request.steps {
        for dependency in &step.depends_on {
            if dependency == &step.id {
                errors.push(format!("step {:?} depends on itself", step.id));
            } else if !by_id.contains_key(dependency) {
                errors.push(
                    BrainError::UnknownDependency {
                        step: step.id.clone(),
                        dependency: dependency.clone(),
                    }
                    .to_string(),
                );
            }
        }
    }
    let estimated_cost = request
        .steps
        .iter()
        .map(|step| step.estimated_cost)
        .sum::<u64>();
    if estimated_cost > request.max_cost {
        errors.push(format!(
            "estimated cost {} exceeds max cost {}",
            estimated_cost, request.max_cost
        ));
    }
    if !errors.is_empty() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_policy_or_shape".into(),
            plan: None,
            errors,
        });
    }

    let mut indegree = request
        .steps
        .iter()
        .map(|step| (step.id.clone(), step.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for step in &request.steps {
        for dependency in &step.depends_on {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(step.id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered_step_ids = Vec::with_capacity(request.steps.len());
    while let Some(id) = ready.pop_first() {
        ordered_step_ids.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("dependent was validated against the step map");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if ordered_step_ids.len() != request.steps.len() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_dependency_cycle".into(),
            plan: None,
            errors: vec![BrainError::PlanCycle.to_string()],
        });
    }
    let steps = ordered_step_ids
        .iter()
        .map(|id| (*by_id.get(id).expect("ordered id was validated")).clone())
        .collect::<Vec<_>>();
    let requires_approval = request.require_approval_for_effects
        && steps.iter().any(|step| step.effect.needs_approval());
    let mut plan = AutonomousPlan {
        schema: PLAN_SCHEMA.into(),
        objective: request.objective.clone(),
        ordered_step_ids,
        steps,
        estimated_cost,
        requires_approval,
        execution: "not_started".into(),
        plan_digest: String::new(),
        does_not_claim: vec![
            "a valid DAG is not evidence that a tool call will succeed".into(),
            "provider calls and external effects remain outside this planning kernel".into(),
            "approval is required before non-read-only execution".into(),
        ],
    };
    let digest_input = plan.clone();
    plan.plan_digest = digest(&digest_input)?;
    Ok(AutonomousPlanReport {
        ok: true,
        status: if requires_approval {
            "planned_approval_required"
        } else {
            "planned_ready_for_caller_execution"
        }
        .into(),
        plan: Some(plan),
        errors: Vec::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditPolicy {
    #[serde(default = "default_bandit_exploration")]
    pub exploration: f64,
    #[serde(default = "default_bandit_min_reward")]
    pub min_reward: f64,
    #[serde(default = "default_bandit_max_reward")]
    pub max_reward: f64,
    #[serde(default = "default_bandit_failure_penalty")]
    pub failure_penalty: f64,
}

fn default_bandit_exploration() -> f64 {
    0.50
}
fn default_bandit_min_reward() -> f64 {
    -1.0
}
fn default_bandit_max_reward() -> f64 {
    1.0
}
fn default_bandit_failure_penalty() -> f64 {
    0.25
}

impl Default for BanditPolicy {
    fn default() -> Self {
        Self {
            exploration: default_bandit_exploration(),
            min_reward: default_bandit_min_reward(),
            max_reward: default_bandit_max_reward(),
            failure_penalty: default_bandit_failure_penalty(),
        }
    }
}

impl BanditPolicy {
    fn validate(&self) -> Result<(), BrainError> {
        finite_range(self.exploration, "bandit.exploration", 0.0, 100.0)?;
        finite_range(self.min_reward, "bandit.min_reward", -100.0, 100.0)?;
        finite_range(self.max_reward, "bandit.max_reward", -100.0, 100.0)?;
        finite_range(self.failure_penalty, "bandit.failure_penalty", 0.0, 100.0)?;
        if self.min_reward >= self.max_reward {
            return Err(BrainError::InvalidReward);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditArm {
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditState {
    pub schema: String,
    #[serde(default)]
    pub generation: u64,
    #[serde(default)]
    pub policy: BanditPolicy,
    pub arms: Vec<BanditArm>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditCandidateScore {
    pub arm_id: String,
    pub pulls: u64,
    pub mean_reward: f64,
    pub exploration_bonus: f64,
    pub failure_rate: f64,
    pub score: f64,
    pub eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditSelectionReport {
    pub schema: String,
    pub selected_arm_id: Option<String>,
    pub ranking: Vec<BanditCandidateScore>,
    pub selection_status: String,
    pub state_generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditUpdate {
    pub arm_id: String,
    pub reward: f64,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub outcome_digest: Option<String>,
}

/// Value-only identity for one provider-backed brain run.
///
/// The identity binds the evaluator's later reward to the exact selection, prompt, plan, and
/// provider outcome without retaining prompt text, provider response text, or credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainRunIdentity {
    pub run_id: String,
    pub selection_digest: String,
    pub prompt_digest: String,
    pub plan_digest: String,
    pub provider: String,
    pub model: String,
    pub outcome_digest: String,
    #[serde(default)]
    pub request_id: Option<String>,
}

/// An explicit evaluator judgment. The brain never derives this from a provider response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainEvaluatorAssessment {
    pub evaluator_id: String,
    pub evaluator_version: String,
    pub reward: f64,
    pub passed: bool,
    #[serde(default)]
    pub failed: bool,
    /// Digest of evaluator-side notes. Raw notes and provider response text never cross this API.
    #[serde(default)]
    pub feedback_digest: Option<String>,
    #[serde(default)]
    pub failure_class: Option<String>,
    #[serde(default)]
    pub evidence_digest: Option<String>,
}

/// Input to the append-only learning boundary. State is caller-owned and returned by value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainOutcomeRecordRequest {
    pub run: BrainRunIdentity,
    pub assessment: BrainEvaluatorAssessment,
    pub bandit_state: BanditState,
    pub arm_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainLearningEvidence {
    pub schema: String,
    pub run: BrainRunIdentity,
    pub assessment: BrainEvaluatorAssessment,
    pub arm_id: String,
    pub bandit_update: BanditUpdate,
    pub previous_generation: u64,
    pub next_generation: u64,
    pub next_state_digest: String,
    pub evidence_digest: String,
    pub does_not_claim: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainOutcomeRecordReport {
    pub ok: bool,
    pub status: String,
    pub next_state: BanditState,
    pub learning_evidence: BrainLearningEvidence,
}

fn validate_brain_run_identity(run: &BrainRunIdentity) -> Result<(), BrainError> {
    non_empty(&run.run_id, "run.run_id")?;
    non_empty(&run.provider, "run.provider")?;
    non_empty(&run.model, "run.model")?;
    validate_digest_value(&run.selection_digest, "run.selection_digest")?;
    validate_digest_value(&run.prompt_digest, "run.prompt_digest")?;
    validate_digest_value(&run.plan_digest, "run.plan_digest")?;
    validate_digest_value(&run.outcome_digest, "run.outcome_digest")?;
    if let Some(request_id) = &run.request_id {
        non_empty(request_id, "run.request_id")?;
    }
    Ok(())
}

fn validate_brain_assessment(assessment: &BrainEvaluatorAssessment) -> Result<(), BrainError> {
    non_empty(&assessment.evaluator_id, "assessment.evaluator_id")?;
    non_empty(
        &assessment.evaluator_version,
        "assessment.evaluator_version",
    )?;
    if assessment.evaluator_id.len() > MAX_EVALUATOR_ID_BYTES
        || assessment.evaluator_version.len() > MAX_EVALUATOR_ID_BYTES
    {
        return Err(BrainError::TooMany {
            field: "assessment evaluator metadata",
            max: MAX_EVALUATOR_ID_BYTES,
        });
    }
    if assessment.passed && assessment.failed {
        return Err(BrainError::ContradictoryAssessment);
    }
    if let Some(feedback_digest) = &assessment.feedback_digest {
        validate_digest_value(feedback_digest, "assessment.feedback_digest")?;
    }
    if let Some(failure_class) = &assessment.failure_class {
        non_empty(failure_class, "assessment.failure_class")?;
        if failure_class.len() > MAX_EVALUATOR_ID_BYTES {
            return Err(BrainError::TooMany {
                field: "assessment.failure_class",
                max: MAX_EVALUATOR_ID_BYTES,
            });
        }
    }
    if let Some(evidence_digest) = &assessment.evidence_digest {
        validate_digest_value(evidence_digest, "assessment.evidence_digest")?;
    }
    Ok(())
}

/// Bind one explicit evaluator judgment to a run and advance caller-owned bandit state.
///
/// This is the durable-learning contract's value layer: applications persist the returned
/// `learning_evidence` and `next_state` in their own store. No provider text, secret, or hidden
/// server memory participates in the update.
pub fn record_brain_outcome(
    request: &BrainOutcomeRecordRequest,
) -> Result<BrainOutcomeRecordReport, BrainError> {
    validate_brain_run_identity(&request.run)?;
    validate_brain_assessment(&request.assessment)?;
    non_empty(&request.arm_id, "arm_id")?;
    validate_bandit_state(&request.bandit_state)?;
    let bandit_update = BanditUpdate {
        arm_id: request.arm_id.clone(),
        reward: request.assessment.reward,
        failed: request.assessment.failed,
        outcome_digest: Some(request.run.outcome_digest.clone()),
    };
    let next_state = update_bandit(&request.bandit_state, &bandit_update)?;
    let next_state_digest = digest(&next_state)?;
    let mut learning_evidence = BrainLearningEvidence {
        schema: LEARNING_EVIDENCE_SCHEMA.into(),
        run: request.run.clone(),
        assessment: request.assessment.clone(),
        arm_id: request.arm_id.clone(),
        bandit_update,
        previous_generation: request.bandit_state.generation,
        next_generation: next_state.generation,
        next_state_digest,
        evidence_digest: String::new(),
        does_not_claim: vec![
            "an evaluator reward is not proof that the provider answer is true".into(),
            "online adaptation is not a claim of general intelligence or biological learning".into(),
            "the ledger contains value-free digests and judgments, not credentials or response text".into(),
            "a passed evaluator does not grant tool permission, clinical validity, or release readiness".into(),
        ],
    };
    let digest_input = learning_evidence.clone();
    learning_evidence.evidence_digest = digest(&digest_input)?;
    Ok(BrainOutcomeRecordReport {
        ok: true,
        status: "recorded_evaluator_reward".into(),
        next_state,
        learning_evidence,
    })
}

fn validate_bandit_state(state: &BanditState) -> Result<(), BrainError> {
    state.policy.validate()?;
    let mut seen = BTreeSet::new();
    for arm in &state.arms {
        non_empty(&arm.arm_id, "arm.arm_id")?;
        finite_range(arm.reward_sum, "arm.reward_sum", -1e12, 1e12)?;
        if !seen.insert(arm.arm_id.clone()) {
            return Err(BrainError::DuplicateArm(arm.arm_id.clone()));
        }
    }
    Ok(())
}

/// Select an arm using a bounded UCB score. Arms with no observations receive the full
/// exploration coefficient, so a good prior cannot permanently starve an untested model.
pub fn select_bandit_arm(state: &BanditState) -> Result<BanditSelectionReport, BrainError> {
    validate_bandit_state(state)?;
    let total_pulls = state.arms.iter().map(|arm| arm.pulls).sum::<u64>();
    let log_total = ((total_pulls + 1) as f64).ln();
    let mut ranking = state
        .arms
        .iter()
        .map(|arm| {
            let mean_reward = if arm.pulls == 0 {
                0.0
            } else {
                arm.reward_sum / arm.pulls as f64
            };
            let failure_rate = if arm.pulls == 0 {
                0.0
            } else {
                arm.failures as f64 / arm.pulls as f64
            };
            let exploration_bonus = if arm.pulls == 0 {
                state.policy.exploration
            } else {
                state.policy.exploration * (log_total / arm.pulls as f64).sqrt()
            };
            let score =
                mean_reward + exploration_bonus - state.policy.failure_penalty * failure_rate;
            BanditCandidateScore {
                arm_id: arm.arm_id.clone(),
                pulls: arm.pulls,
                mean_reward,
                exploration_bonus,
                failure_rate,
                score,
                eligible: !arm.disabled,
            }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.arm_id.cmp(&right.arm_id))
    });
    let selected_arm_id = ranking
        .iter()
        .find(|candidate| candidate.eligible)
        .map(|candidate| candidate.arm_id.clone());
    Ok(BanditSelectionReport {
        schema: BANDIT_SCHEMA.into(),
        selected_arm_id: selected_arm_id.clone(),
        ranking,
        selection_status: if selected_arm_id.is_some() {
            "selected".into()
        } else {
            "refused_no_eligible_arm".into()
        },
        state_generation: state.generation,
    })
}

/// Apply one explicit evaluator reward and return the new value-bearing state. The caller owns
/// persistence and must supply a digest of the evaluated outcome when it wants a replay link.
pub fn update_bandit(
    state: &BanditState,
    update: &BanditUpdate,
) -> Result<BanditState, BrainError> {
    validate_bandit_state(state)?;
    state.policy.validate()?;
    finite_range(
        update.reward,
        "update.reward",
        state.policy.min_reward,
        state.policy.max_reward,
    )?;
    let mut next = state.clone();
    let arm = next
        .arms
        .iter_mut()
        .find(|arm| arm.arm_id == update.arm_id)
        .ok_or_else(|| BrainError::UnknownArm(update.arm_id.clone()))?;
    if arm.disabled {
        return Err(BrainError::UnknownArm(update.arm_id.clone()));
    }
    arm.pulls = arm.pulls.saturating_add(1);
    arm.reward_sum += update.reward;
    if update.failed {
        arm.failures = arm.failures.saturating_add(1);
    }
    next.generation = next.generation.saturating_add(1);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(provider: &str, name: &str, quality: f64, cost: u64) -> ModelDescriptor {
        ModelDescriptor {
            provider: provider.into(),
            model: name.into(),
            capabilities: vec!["reasoning".into()],
            context_window_tokens: 16_000,
            max_output_tokens: 2_000,
            quality,
            latency_ms: 100,
            cost_per_million_tokens: cost,
            reliability: 0.9,
            requires_credential: true,
            enabled: true,
        }
    }

    #[test]
    fn model_selection_applies_hard_gates_before_deterministic_ranking() {
        let report = select_model(&ModelSelectionRequest {
            task: "summarize".into(),
            required_capabilities: vec!["reasoning".into(), "structured_output".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: Some(100),
            max_latency_ms: None,
            min_quality: None,
            models: vec![
                model("a", "cheap", 0.7, 1),
                model("b", "expensive", 0.99, 1000),
            ],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
        })
        .unwrap();
        assert!(report.selected_model_id.is_none());
        assert!(report.ranking.iter().all(|candidate| !candidate.eligible));
        assert_eq!(report.selection_status, "refused_no_eligible_model");
    }

    #[test]
    fn contextual_model_selection_overrides_global_history_without_hidden_state() {
        let context = ModelSelectionContext {
            domain: "oncology".into(),
            capability: "assay_fidelity".into(),
            risk_class: "high_review".into(),
            task_family: Some("evidence_summary".into()),
        };
        let context_digest = digest(&context).unwrap();
        let base = ModelSelectionRequest {
            task: "summarize assay evidence".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            models: vec![
                model("a", "global", 0.8, 10),
                model("b", "context", 0.8, 10),
            ],
            observations: vec![
                ModelObservation {
                    arm_id: "a/global".into(),
                    pulls: 10,
                    reward_sum: 8.0,
                    failures: 0,
                    disabled: false,
                },
                ModelObservation {
                    arm_id: "b/context".into(),
                    pulls: 10,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                },
            ],
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
        };
        let report = select_model_contextual(&ContextualModelSelectionRequest {
            context,
            base,
            observations: vec![ContextualModelObservation {
                context_digest,
                arm_id: "b/context".into(),
                pulls: 10,
                reward_sum: 10.0,
                failures: 0,
                disabled: false,
            }],
        })
        .unwrap();
        assert_eq!(
            report.selection.selected_model_id.as_deref(),
            Some("b/context")
        );
        assert_eq!(report.contextual_observations_used, 1);
        assert_eq!(report.global_observation_fallbacks, 1);
        assert_eq!(
            report.selection_status,
            "contextual_selection_mixed_history"
        );
    }

    #[test]
    fn provider_health_is_a_kernel_gate_and_remains_visible_in_candidate_reasons() {
        let mut provider_health = BTreeMap::new();
        provider_health.insert(
            "a".into(),
            ProviderHealth {
                registered: true,
                circuit: "open".into(),
                consecutive_failures: 3,
                credential_ready: true,
                eligible: false,
            },
        );
        provider_health.insert(
            "b".into(),
            ProviderHealth {
                registered: true,
                circuit: "closed".into(),
                consecutive_failures: 0,
                credential_ready: true,
                eligible: true,
            },
        );
        let report = select_model(&ModelSelectionRequest {
            task: "provider health test".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            models: vec![model("a", "open", 0.99, 1), model("b", "ready", 0.7, 2)],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health,
        })
        .unwrap();
        assert_eq!(report.selected_model_id.as_deref(), Some("b/ready"));
        let refused = report
            .ranking
            .iter()
            .find(|candidate| candidate.model_id == "a/open")
            .unwrap();
        assert!(!refused.eligible);
        assert!(refused
            .reasons
            .iter()
            .any(|reason| reason == "provider_circuit_open"));
        assert!(refused
            .reasons
            .iter()
            .any(|reason| reason == "provider_health_ineligible"));
    }

    #[test]
    fn prompt_assembly_reports_optional_omission_and_digest() {
        let report = assemble_prompt(&PromptAssemblyRequest {
            system: Some("be precise".into()),
            developer: None,
            task: "answer".into(),
            context: vec![
                PromptChunk {
                    id: "required".into(),
                    role: "user".into(),
                    content: "must include".into(),
                    required: true,
                    priority: 0,
                },
                PromptChunk {
                    id: "optional".into(),
                    role: "user".into(),
                    content: "this is deliberately large enough to omit".into(),
                    required: false,
                    priority: 0,
                },
            ],
            output_contract: Some("JSON".into()),
            max_input_tokens: 10,
        })
        .unwrap();
        assert_eq!(report.omitted_context_ids, vec!["optional"]);
        assert!(!report.complete);
        assert_eq!(report.prompt_digest.len(), 64);
    }

    #[test]
    fn planner_orders_dependencies_and_requires_approval_for_effects() {
        let report = plan_autonomous(&AutonomousPlanRequest {
            objective: "inspect then call model".into(),
            allowed_tools: vec!["inspect".into(), "invoke".into()],
            max_cost: 10,
            require_approval_for_effects: true,
            steps: vec![
                PlanStep {
                    id: "invoke".into(),
                    objective: "call model".into(),
                    tool: "invoke".into(),
                    arguments: json!({}),
                    depends_on: vec!["inspect".into()],
                    effect: PlanEffect::ProviderCall,
                    estimated_cost: 5,
                },
                PlanStep {
                    id: "inspect".into(),
                    objective: "inspect context".into(),
                    tool: "inspect".into(),
                    arguments: json!({}),
                    depends_on: vec![],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
            ],
        })
        .unwrap();
        let plan = report.plan.unwrap();
        assert_eq!(plan.ordered_step_ids, vec!["inspect", "invoke"]);
        assert!(plan.requires_approval);
        assert_eq!(plan.execution, "not_started");
    }

    #[test]
    fn bandit_updates_are_explicit_and_unexplored_arms_are_selected() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: vec![
                BanditArm {
                    arm_id: "known".into(),
                    pulls: 10,
                    reward_sum: 1.0,
                    failures: 0,
                    disabled: false,
                },
                BanditArm {
                    arm_id: "new".into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                },
            ],
        };
        let selected = select_bandit_arm(&state).unwrap();
        assert_eq!(selected.selected_arm_id.as_deref(), Some("new"));
        let next = update_bandit(
            &state,
            &BanditUpdate {
                arm_id: "new".into(),
                reward: 0.8,
                failed: false,
                outcome_digest: Some("a".repeat(64)),
            },
        )
        .unwrap();
        assert_eq!(next.generation, 1);
        assert_eq!(next.arms[1].pulls, 1);
        assert_eq!(next.arms[1].reward_sum, 0.8);
    }

    #[test]
    fn outcome_record_binds_evaluator_reward_without_response_text() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 4,
            policy: BanditPolicy::default(),
            arms: vec![BanditArm {
                arm_id: "openai/test-model".into(),
                pulls: 1,
                reward_sum: 0.2,
                failures: 0,
                disabled: false,
            }],
        };
        let report = record_brain_outcome(&BrainOutcomeRecordRequest {
            run: BrainRunIdentity {
                run_id: "run-1".into(),
                selection_digest: "a".repeat(64),
                prompt_digest: "b".repeat(64),
                plan_digest: "c".repeat(64),
                provider: "openai".into(),
                model: "test-model".into(),
                outcome_digest: "d".repeat(64),
                request_id: Some("request-1".into()),
            },
            assessment: BrainEvaluatorAssessment {
                evaluator_id: "json_contract".into(),
                evaluator_version: "1".into(),
                reward: 0.9,
                passed: true,
                failed: false,
                feedback_digest: Some("f".repeat(64)),
                failure_class: None,
                evidence_digest: Some("e".repeat(64)),
            },
            bandit_state: state,
            arm_id: "openai/test-model".into(),
        })
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.status, "recorded_evaluator_reward");
        assert_eq!(report.next_state.generation, 5);
        assert_eq!(report.learning_evidence.previous_generation, 4);
        assert_eq!(report.learning_evidence.next_generation, 5);
        assert_eq!(report.learning_evidence.evidence_digest.len(), 64);
        let encoded = serde_json::to_string(&report.learning_evidence).unwrap();
        assert!(!encoded.contains("provider response"));
        assert!(!encoded.contains("api_key"));
    }

    #[test]
    fn outcome_record_rejects_contradictory_assessments() {
        let error = record_brain_outcome(&BrainOutcomeRecordRequest {
            run: BrainRunIdentity {
                run_id: "run-1".into(),
                selection_digest: "a".repeat(64),
                prompt_digest: "b".repeat(64),
                plan_digest: "c".repeat(64),
                provider: "openai".into(),
                model: "test-model".into(),
                outcome_digest: "d".repeat(64),
                request_id: None,
            },
            assessment: BrainEvaluatorAssessment {
                evaluator_id: "evaluator".into(),
                evaluator_version: "1".into(),
                reward: 0.0,
                passed: true,
                failed: true,
                feedback_digest: None,
                failure_class: None,
                evidence_digest: None,
            },
            bandit_state: BanditState {
                schema: BANDIT_SCHEMA.into(),
                generation: 0,
                policy: BanditPolicy::default(),
                arms: vec![BanditArm {
                    arm_id: "openai/test-model".into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                }],
            },
            arm_id: "openai/test-model".into(),
        })
        .unwrap_err();
        assert!(matches!(error, BrainError::ContradictoryAssessment));
    }

    #[test]
    fn plan_rejects_cycles_without_partial_execution() {
        let report = plan_autonomous(&AutonomousPlanRequest {
            objective: "cycle".into(),
            allowed_tools: vec!["x".into()],
            max_cost: 10,
            require_approval_for_effects: true,
            steps: vec![
                PlanStep {
                    id: "a".into(),
                    objective: "a".into(),
                    tool: "x".into(),
                    arguments: Value::Null,
                    depends_on: vec!["b".into()],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
                PlanStep {
                    id: "b".into(),
                    objective: "b".into(),
                    tool: "x".into(),
                    arguments: Value::Null,
                    depends_on: vec!["a".into()],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
            ],
        })
        .unwrap();
        assert!(!report.ok);
        assert_eq!(report.status, "refused_dependency_cycle");
        assert!(report.plan.is_none());
    }
}
