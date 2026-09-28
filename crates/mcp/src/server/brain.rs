//! MCP brain-control request handlers.
//!
//! Kept beside the dispatcher so planning, provider selection, and job-control routes can be reviewed independently.

use super::*;

impl Server {
    /// Select a provider/model from caller-supplied metadata. No credential is accepted here:
    /// applications invoke the provider through their own secret boundary and pass only the
    /// selected model id and value-free outcome metadata back to this server.
    pub(super) fn brain_model_select(&self, arguments: &Value) -> Result<Value, String> {
        let request: ModelSelectionRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid brain model-selection request: {error}"))?;
        let report = select_model(&request)
            .map_err(|error| format!("brain model selection refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode brain model-selection report: {error}"))
    }

    /// Select a model with domain/capability/risk-scoped observations. Exact contextual history
    /// overrides global history for an arm; the caller still owns all state and credentials.
    pub(super) fn brain_model_select_contextual(&self, arguments: &Value) -> Result<Value, String> {
        let request: ContextualModelSelectionRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| {
                format!("invalid contextual brain model-selection request: {error}")
            })?;
        let report = select_model_contextual(&request)
            .map_err(|error| format!("contextual brain model selection refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode contextual brain model-selection report: {error}")
        })
    }

    /// Assemble a bounded prompt with explicit omission accounting. The server does not invoke a
    /// model; the returned digest is the stable input identity a provider runtime can record.
    pub(super) fn brain_prompt_assemble(&self, arguments: &Value) -> Result<Value, String> {
        let request: PromptAssemblyRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid brain prompt request: {error}"))?;
        let report = assemble_prompt(&request)
            .map_err(|error| format!("brain prompt assembly refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode brain prompt report: {error}"))
    }

    /// Validate and order a bounded autonomous plan. Plans are returned as not-started artifacts;
    /// effectful steps remain approval-gated and no tool is executed by this handler.
    pub(super) fn brain_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: AutonomousPlanRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid brain plan request: {error}"))?;
        let report =
            plan_autonomous(&request).map_err(|error| format!("brain plan refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode brain plan report: {error}"))
    }

    /// Select a model/tool arm from caller-persisted online-learning state. State is supplied and
    /// returned by value so the MCP server has no hidden mutable learning memory.
    pub(super) fn brain_bandit_select(&self, arguments: &Value) -> Result<Value, String> {
        let state_value = arguments
            .get("state")
            .cloned()
            .unwrap_or_else(|| arguments.clone());
        let state: BanditState = serde_json::from_value(state_value)
            .map_err(|error| format!("invalid brain bandit state: {error}"))?;
        let report = match (
            arguments.get("context_digest").and_then(Value::as_str),
            arguments.get("context"),
        ) {
            (None, None) => select_bandit_arm(&state),
            (Some(context_digest), Some(context_value)) => {
                let context: bioprism_brain::ModelSelectionContext =
                    serde_json::from_value(context_value.clone())
                        .map_err(|error| format!("invalid brain bandit context: {error}"))?;
                select_bandit_arm_contextual(&state, context_digest, &context)
            }
            _ => Err(bioprism_brain::BrainError::ContextRequired),
        }
        .map_err(|error| format!("brain bandit selection refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode brain bandit selection: {error}"))
    }

    /// Apply one bounded evaluator reward to caller-owned bandit state and return the next state.
    /// Reward updates are explicit; no provider response is treated as a reward implicitly.
    pub(super) fn brain_bandit_update(&self, arguments: &Value) -> Result<Value, String> {
        let state: BanditState = serde_json::from_value(
            arguments
                .get("state")
                .cloned()
                .ok_or_else(|| "brain_bandit_update requires state".to_string())?,
        )
        .map_err(|error| format!("invalid brain bandit state: {error}"))?;
        let update: BanditUpdate = serde_json::from_value(
            arguments
                .get("update")
                .cloned()
                .ok_or_else(|| "brain_bandit_update requires update".to_string())?,
        )
        .map_err(|error| format!("invalid brain bandit update: {error}"))?;
        let next = update_bandit(&state, &update)
            .map_err(|error| format!("brain bandit update refused: {error}"))?;
        serde_json::to_value(next)
            .map_err(|error| format!("cannot encode brain bandit state: {error}"))
    }

    /// Bind one explicit evaluator judgment to a value-only run identity and advance the
    /// caller-owned bandit state. Provider text and credentials are never accepted here.
    pub(super) fn brain_outcome_record(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .outcome_record(arguments)
    }

    pub(super) fn brain_job_submit(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .submit_job(arguments)
    }

    pub(super) fn brain_job_status(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_status(arguments)
    }

    pub(super) fn brain_job_events(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_events(arguments)
    }

    pub(super) fn brain_job_approval(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_approval(arguments)
    }

    pub(super) fn brain_job_claim(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_claim(arguments)
    }

    pub(super) fn brain_job_claim_next(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_claim_next(arguments)
    }

    pub(super) fn brain_job_renew(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_renew(arguments)
    }

    pub(super) fn brain_job_checkpoint(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_checkpoint(arguments)
    }

    pub(super) fn brain_job_complete(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_complete(arguments)
    }

    pub(super) fn brain_job_fail(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_fail(arguments)
    }

    pub(super) fn brain_job_reconcile(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_reconcile(arguments)
    }

    pub(super) fn brain_job_cancel(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .job_cancel(arguments)
    }

    pub(super) fn brain_model_health(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .model_health(arguments)
    }

    pub(super) fn brain_replay_evaluate(&self, arguments: &Value) -> Result<Value, String> {
        self.brain_control_state
            .lock()
            .map_err(|_| "brain control-plane state is unavailable".to_string())?
            .replay_evaluate(arguments)
    }
}
