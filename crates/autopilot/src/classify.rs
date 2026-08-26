//! Classifying recorded step outcomes into 40.36 retry classes, from evidence only.
//!
//! The mission executor records exactly four per-step statuses — `succeeded`, `refused`,
//! `blocked`, `cancelled` — and lands *every* dispatched failure on `refused`, whether the cause
//! was its own policy (a binding that would not materialize, a schema refusal, an output-budget
//! breach) or a nested tool returning an error envelope. The 40.36 retry decision is therefore
//! not recoverable from the status alone, and this module refuses to guess it. The mapping below
//! uses only what the recorded [`MissionStepResult`] actually contains:
//!
//! | recorded evidence | class | why |
//! |---|---|---|
//! | status `succeeded` | [`StepClass::Succeeded`] | not a failure; never re-dispatched |
//! | status `blocked` | [`StepClass::Blocked`] | the step never ran; it is rescheduled exactly when its failed prerequisites are, and carries no failure class of its own |
//! | any failure whose recorded evidence declares a 40.36 decision | that declared class | the tool published its own retry decision; honouring it is the whole point of the taxonomy |
//! | status `refused` with **no** retained tool envelope | [`RetryClass::Terminal`] | the executor itself refused before or around dispatch — 40.36 invariant 3 says policy behaving correctly is not a transient fault, and re-sending identical bytes is dishonest |
//! | status `refused` **with** a retained tool envelope and no declared decision | [`RetryClass::Unknown`] | the tool failed without publishing a decision; unknown is recorded as unknown, never coerced toward retryable |
//! | status `cancelled` | [`RetryClass::Unknown`] | cancellation is an authority outside the report; the drive additionally refuses to retry past a cancellation at all |
//! | any other status string | [`RetryClass::Unknown`] | a future status must not silently become retryable |
//!
//! A "declared decision" is recognised only in these places, and only as the exact 40.36 strings
//! `terminal`, `retryable_after_change`, `retryable_as_is`:
//!
//! 1. the retained wire envelope at `/result/structuredContent/retryability` or
//!    `/result/structuredContent/error/retryability`;
//! 2. the recorded error text, when that text parses as a JSON object carrying `retryability` or
//!    `error.retryability` — the shape the CLI's `--json` failure envelope publishes.
//!
//! Anything else — a different spelling, a bare boolean, prose that mentions retrying — is not a
//! signal. In practice most nested tool errors are plain text, so the default drive is
//! conservative: it re-dispatches only what was explicitly declared re-dispatchable.

use bioprism_devplat::MissionStepResult;
use serde_json::Value;

/// The 40.36 retry decision as recovered from recorded evidence, plus the honest fourth state
/// for evidence that declares none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RetryClass {
    /// Dead as written; no grant option re-dispatches it.
    Terminal,
    /// A different request may succeed; re-dispatched only under an explicit grant option, and
    /// the only change this drive can make is re-materializing bindings.
    RetryableAfterChange,
    /// The identical step may succeed on a re-send.
    RetryableAsIs,
    /// No decision was declared. Never treated as retryable by default.
    Unknown,
}

impl RetryClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RetryClass::Terminal => "terminal",
            RetryClass::RetryableAfterChange => "retryable_after_change",
            RetryClass::RetryableAsIs => "retryable_as_is",
            RetryClass::Unknown => "unknown",
        }
    }
}

/// What one recorded step result is, for planning purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepClass {
    /// The step completed; its retained payload may serve later bindings.
    Succeeded,
    /// The step was never dispatched because a prerequisite failed; it has no failure class.
    Blocked,
    /// The step was dispatched (or refused around dispatch) and did not complete.
    Failed(RetryClass),
}

impl StepClass {
    pub fn as_str(self) -> &'static str {
        match self {
            StepClass::Succeeded => "succeeded",
            StepClass::Blocked => "blocked",
            StepClass::Failed(class) => class.as_str(),
        }
    }
}

/// One classified step, retaining where the decision came from so a report reader can audit it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepClassification {
    pub step_id: String,
    /// The executor's recorded status, verbatim.
    pub status: String,
    pub class: StepClass,
    /// `declared_structured_content`, `declared_error_text`, `executor_refusal`,
    /// `undeclared_tool_error`, `cancelled`, `succeeded`, `blocked`, or `unrecognised_status`.
    pub signal: &'static str,
    pub reason: String,
}

fn parse_declared(value: &str) -> Option<RetryClass> {
    match value {
        "terminal" => Some(RetryClass::Terminal),
        "retryable_after_change" => Some(RetryClass::RetryableAfterChange),
        "retryable_as_is" => Some(RetryClass::RetryableAsIs),
        _ => None,
    }
}

/// Look for an explicitly declared 40.36 decision in the retained evidence.
///
/// Returns the class and which surface declared it. An unrecognised value in a `retryability`
/// slot is *not* a signal: coercing near-miss spellings would turn a typo into an authority.
fn declared_retryability(result: &MissionStepResult) -> Option<(RetryClass, &'static str)> {
    if let Some(wire) = result.wire.as_ref() {
        for pointer in [
            "/result/structuredContent/retryability",
            "/result/structuredContent/error/retryability",
        ] {
            if let Some(declared) = wire
                .pointer(pointer)
                .and_then(Value::as_str)
                .and_then(parse_declared)
            {
                return Some((declared, "declared_structured_content"));
            }
        }
    }
    let text = result.error.as_deref()?;
    let parsed: Value = serde_json::from_str(text).ok()?;
    if !parsed.is_object() {
        return None;
    }
    for pointer in ["/retryability", "/error/retryability"] {
        if let Some(declared) = parsed
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(parse_declared)
        {
            return Some((declared, "declared_error_text"));
        }
    }
    None
}

/// Classify a step an attempt dispatched but whose report carries no result row for it.
///
/// The absence is evidence in its own right: the drive asked for the step, so it may have run and
/// may have had effects, while the report declares nothing about it. Treating that as `unknown`
/// keeps it out of every default retry path and out of any success claim. Both the planner and
/// the report reader use this so a dispatched step is never silently missing from either.
pub fn classify_missing_step_result(step_id: &str) -> StepClassification {
    StepClassification {
        step_id: step_id.to_string(),
        status: "missing".into(),
        class: StepClass::Failed(RetryClass::Unknown),
        signal: "unrecognised_status",
        reason: "the attempt dispatched this step but its report holds no result row for it".into(),
    }
}

/// Classify one recorded step result. Pure, total, and deterministic; the full mapping is the
/// table in the module documentation.
pub fn classify_step_result(result: &MissionStepResult) -> StepClassification {
    let (class, signal, reason) = match result.status.as_str() {
        "succeeded" => (
            StepClass::Succeeded,
            "succeeded",
            "the executor recorded a completed step".to_string(),
        ),
        "blocked" => (
            StepClass::Blocked,
            "blocked",
            "the step was never dispatched because a prerequisite refused or was blocked"
                .to_string(),
        ),
        "cancelled" => (
            StepClass::Failed(RetryClass::Unknown),
            "cancelled",
            "cancellation is an authority outside the mission report; the outcome class is not \
             recoverable and the drive never retries past a cancellation"
                .to_string(),
        ),
        "refused" => match declared_retryability(result) {
            Some((declared, signal)) => (
                StepClass::Failed(declared),
                signal,
                format!(
                    "the recorded evidence declares the 40.36 decision `{}`",
                    declared.as_str()
                ),
            ),
            None if result.wire.is_none() => (
                StepClass::Failed(RetryClass::Terminal),
                "executor_refusal",
                "the mission executor refused this step itself (binding, schema, or output \
                 budget); policy behaving correctly is terminal for identical bytes"
                    .to_string(),
            ),
            None => (
                StepClass::Failed(RetryClass::Unknown),
                "undeclared_tool_error",
                "the nested tool returned an error envelope that declares no 40.36 decision"
                    .to_string(),
            ),
        },
        other => (
            StepClass::Failed(RetryClass::Unknown),
            "unrecognised_status",
            format!("status `{other}` is outside the executor's recorded vocabulary"),
        ),
    };
    StepClassification {
        step_id: result.id.clone(),
        status: result.status.clone(),
        class,
        signal,
        reason,
    }
}
