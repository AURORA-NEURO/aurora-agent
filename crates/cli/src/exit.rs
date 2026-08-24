//! Exit codes.
//!
//! Blueprint 40.13 requires errors to map to documented exit codes; blueprint 40.36 requires a
//! typed error taxonomy whose retry classification survives that mapping. The second requirement
//! is the stricter one and it is what shapes this registry. A caller that sees only the process
//! status — a CI step, a shell `if`, a supervising agent — must be able to recover the retry
//! decision from it, without parsing stderr and without reading the `--json` envelope.
//!
//! # One retry decision per code
//!
//! 40.36's retry classification is three-valued: the request is dead as written, the request may
//! succeed once something changes, or the identical request may succeed on a re-send. A code that
//! carries failures drawn from two of those three answers destroys the distinction for everybody
//! downstream, because no amount of care at the branch can recover what the code did not encode.
//! Every failure code below therefore carries exactly one [`Retryability`], and
//! [`ExitCode::retryability`] is total over the registry.
//!
//! That invariant is why codes 6 to 9 exist. `compile_failed` previously absorbed every failure
//! that was neither a usage error, an input error nor an I/O error: a policy refusal, an oracle
//! abstention, a superseded snapshot and a genuine contract violation all left the process with
//! status 4, and those four want four different next actions. Splitting them changes a published
//! interface and is deliberate.
//!
//! # `stale` advertised the opposite of the truth
//!
//! A superseded precondition is the one failure here that a client may re-send *unchanged* once it
//! has re-read the thing it asserted. Routed through `compile_failed`, it was advertised as not
//! retryable, so a client that trusted the code stopped at precisely the point where re-reading
//! and re-sending would have worked. Code 9 exists so that the registry no longer states the
//! reverse of the retry decision 40.36 assigns.
//!
//! # Codes 0 and 1 publish no retry decision
//!
//! 40.36 classifies *failures*. Exit 0 and exit 1 both report a completed run — the checked
//! property held, or it did not — so a retry decision would be an answer to a question nobody
//! asked. [`ExitCode::retryability`] returns `None` for both rather than inventing a third state
//! that consumers would have to special-case anyway.

use bioprism_autopilot::{AutopilotError, GrantError};
use bioprism_baseline::CompareError;
use bioprism_fiber::{FiberError, PolicyViolation};
use bioprism_mutation::MutationError;
use bioprism_prism::MinimizeError;
use bioprism_store::StoreError;
use std::fmt;

/// Whether a caller may re-send the identical request, as blueprint 40.36 classifies it.
///
/// The variant names and slugs are 40.36's rather than this crate's, so that a consumer joining a
/// process exit status against a taxonomy published elsewhere in the workspace does not have to
/// translate between two vocabularies for the same three answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retryability {
    /// Re-sending is pointless and so is waiting: the request is dead as written.
    Terminal,
    /// Re-sending the identical request is pointless, but a different one may succeed — raise the
    /// budget, obtain the grant, widen the contract.
    RetryableAfterChange,
    /// The identical logical request may succeed on a re-send, once the caller has refreshed the
    /// precondition it asserted or the dependency has recovered.
    RetryableAsIs,
}

impl Retryability {
    pub fn as_str(self) -> &'static str {
        match self {
            Retryability::Terminal => "terminal",
            Retryability::RetryableAfterChange => "retryable_after_change",
            Retryability::RetryableAsIs => "retryable_as_is",
        }
    }

    /// Whether a client loop may re-send without a human or an agent editing the request.
    pub fn permits_automatic_retry(self) -> bool {
        self == Retryability::RetryableAsIs
    }
}

impl fmt::Display for Retryability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// The command completed and its assertion held.
    Ok = 0,
    /// The command completed but the thing it checked did not hold: an invalid split, a
    /// certificate that does not verify. Distinct from an error, and scriptable.
    AssertionFailed = 1,
    /// Bad invocation.
    Usage = 2,
    /// Input did not satisfy its schema or could not be parsed.
    InvalidInput = 3,
    /// No result satisfies the tool's own declared invariants under this request, for example a
    /// budget smaller than the protected closure 43.13 forbids trimming.
    ///
    /// Narrower than it once was: this code now means a contract violation and nothing else.
    CompileFailed = 4,
    /// A declared dependency could not be read or written.
    Io = 5,
    /// The request contradicts state already committed under the same identity.
    ///
    /// Not to be confused with 40.25's `policy conflict`, which is a refusal by policy and exits
    /// 7. The distinction is which side has to move: a policy conflict clears when a grant is
    /// obtained, whereas this code says the identity is already spoken for and the caller must
    /// name a different one.
    Conflict = 6,
    /// Policy refused the request. 40.36 invariant 3: the platform behaved correctly, so this is
    /// not a system failure and not something to page on.
    PolicyDenied = 7,
    /// The tool ran correctly and the answer is not determined by the evidence. 40.36 invariant 4
    /// exists to stop an oracle abstention being reported as a compile failure.
    Indeterminate = 8,
    /// A precondition the request asserted has been superseded since it was read.
    ///
    /// The caller re-reads what it asserted and re-sends the identical request; this is the only
    /// failure here whose remedy touches nothing in the request itself.
    Stale = 9,
}

impl ExitCode {
    /// Every code in the registry, in numeric order.
    ///
    /// Exposed so that a test can quantify over the registry rather than restate it, which is the
    /// only way the "one retry decision per code" invariant stays true as codes are added.
    pub const ALL: [ExitCode; 10] = [
        ExitCode::Ok,
        ExitCode::AssertionFailed,
        ExitCode::Usage,
        ExitCode::InvalidInput,
        ExitCode::CompileFailed,
        ExitCode::Io,
        ExitCode::Conflict,
        ExitCode::PolicyDenied,
        ExitCode::Indeterminate,
        ExitCode::Stale,
    ];

    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// The 40.36 retry decision this code carries, or `None` for the two codes that report a
    /// verdict rather than a failure.
    pub fn retryability(self) -> Option<Retryability> {
        match self {
            ExitCode::Ok | ExitCode::AssertionFailed => None,
            ExitCode::Usage | ExitCode::InvalidInput | ExitCode::Conflict => {
                Some(Retryability::Terminal)
            }
            ExitCode::CompileFailed | ExitCode::PolicyDenied | ExitCode::Indeterminate => {
                Some(Retryability::RetryableAfterChange)
            }
            ExitCode::Io | ExitCode::Stale => Some(Retryability::RetryableAsIs),
        }
    }

    /// Whether a client may re-send the identical request.
    ///
    /// Kept as a boolean because it is already on the wire in the `--json` envelope, and derived
    /// from [`ExitCode::retryability`] so the two can no longer disagree. It is deliberately the
    /// weaker of the two accessors: `false` covers both "dead as written" and "retry once you
    /// change something", which is exactly why a caller wanting to act on the difference must read
    /// the code or the `retryability` field instead.
    pub fn is_retryable(self) -> bool {
        matches!(self.retryability(), Some(decision) if decision.permits_automatic_retry())
    }

    /// The one line `--help` prints for this code.
    ///
    /// This is the registry's statement of what the code means, and the only statement of it a
    /// user ever reads; the doc comments above say why each code exists, which is a different
    /// question. `bioprism-devx` transcribes these strings and judges each against the diagnostic
    /// class routed to the code, so a summary that drifts from the code's real meaning is a
    /// finding there rather than a matter of taste here.
    pub fn summary(self) -> &'static str {
        match self {
            ExitCode::Ok => "the command completed and its assertion held",
            ExitCode::AssertionFailed => "completed, but the checked property does not hold",
            ExitCode::Usage => "bad invocation",
            ExitCode::InvalidInput => "input failed its schema or could not be parsed",
            ExitCode::CompileFailed => "no result satisfies the declared contract",
            ExitCode::Io => "a declared dependency could not be read or written",
            ExitCode::Conflict => "contradicts state already committed under this id",
            ExitCode::PolicyDenied => "policy refused; the platform behaved correctly",
            ExitCode::Indeterminate => "ran correctly; the evidence does not decide",
            ExitCode::Stale => "a precondition was superseded; re-read and re-send",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ExitCode::Ok => "ok",
            ExitCode::AssertionFailed => "assertion_failed",
            ExitCode::Usage => "usage",
            ExitCode::InvalidInput => "invalid_input",
            ExitCode::CompileFailed => "compile_failed",
            ExitCode::Io => "io",
            ExitCode::Conflict => "conflict",
            ExitCode::PolicyDenied => "policy_denied",
            ExitCode::Indeterminate => "indeterminate",
            ExitCode::Stale => "stale",
        }
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.as_i32(), self.slug())
    }
}

/// A failure carrying the exit code it should produce.
#[derive(Debug)]
pub struct CliError {
    pub code: ExitCode,
    pub message: String,
    pub subject: Option<String>,
}

impl CliError {
    pub fn new(code: ExitCode, message: impl Into<String>) -> Self {
        CliError {
            code,
            message: message.into(),
            subject: None,
        }
    }

    pub fn about(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    pub fn io(path: &std::path::Path, source: std::io::Error) -> Self {
        CliError::new(ExitCode::Io, source.to_string()).about(path.display().to_string())
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        CliError::new(ExitCode::InvalidInput, message)
    }

    /// The code an unclassified failure of this binary lands on.
    ///
    /// 40.36's `Internal` class has no code of its own in this registry: it shares `io` with
    /// `Unavailable`. Both are retryable as-is, so the retry decision survives the sharing, which
    /// is why `bioprism-devx`'s exit-code audit reports it as an open imprecision rather than a
    /// defect. What does not survive is the ability to tell a dependency that could not be read
    /// from a bug in this binary, and this constructor is the one place that costs anything.
    pub fn internal(message: impl Into<String>) -> Self {
        CliError::new(ExitCode::Io, message)
    }

    /// Routes a compiler failure to the code carrying its 40.36 class.
    ///
    /// [`FiberError`] already separates the phases 43.16 asks it to separate, and collapsing all of
    /// them onto `compile_failed` threw that work away at the process boundary: a query missing a
    /// field, a corpus released under clauses the query did not accept, and an oracle that cannot
    /// order two split groups are three different conversations with the caller. The classification
    /// is written here, against the error type, rather than at each call site, so that a new
    /// variant fails to compile until somebody decides which code it belongs under.
    ///
    /// `UnknownQueryFields` is `invalid_input` and not `compile_failed`: nothing was compiled, and
    /// re-sending the identical command cannot succeed. The operator has to edit the document,
    /// which is the same conversation as a missing field, one key over.
    pub fn from_compile(error: FiberError) -> Self {
        let code = match &error {
            FiberError::UnsupportedQuerySchema { .. }
            | FiberError::QueryNotAnObject
            | FiberError::UnknownQueryFields { .. }
            | FiberError::MissingQueryField(_)
            | FiberError::WrongQueryFieldType { .. }
            | FiberError::InvalidIdentifier(_)
            | FiberError::InvalidDecisionTime(_)
            | FiberError::InvalidDecisionContract(_)
            | FiberError::InvalidRateDistortionContract(_)
            | FiberError::InvalidAdaptiveAcquisitionContract(_)
            | FiberError::World(_) => ExitCode::InvalidInput,
            FiberError::BudgetExceeded { .. } => ExitCode::CompileFailed,
            FiberError::UnorderableSplitGroups { .. } => ExitCode::Indeterminate,
            FiberError::Policy(violation) => policy_code(violation),
        };
        CliError::new(code, error.to_string())
    }

    /// Routes a store failure to the code carrying its 40.36 class.
    ///
    /// The store is a dependency of the compile rather than part of the query, so a store whose
    /// schema the binary does not recognise is a superseded precondition and not malformed input:
    /// the caller re-indexes and re-sends the identical command. Reporting it as `invalid_input`
    /// told an operator their world was broken when in fact their index was old.
    pub fn from_store(path: &std::path::Path, error: StoreError) -> Self {
        let code = match &error {
            StoreError::Io(_) | StoreError::CorruptIndex(_) => ExitCode::Io,
            StoreError::UnsupportedSchema { .. } => ExitCode::Stale,
            StoreError::Json(_)
            | StoreError::UnsupportedKey(_)
            | StoreError::UnsupportedValue(_)
            | StoreError::MalformedWorld => ExitCode::InvalidInput,
        };
        CliError::new(code, error.to_string()).about(path.display().to_string())
    }

    /// Routes a metamorphic-family failure to the code carrying its 40.36 class.
    ///
    /// [`MutationError`] already separates the parent world not loading from the oracle refusing to
    /// judge it from the world having no content digest, and those are an input, an evidence and a
    /// tool failure respectively. Reporting all three the same way would tell a caller whose world
    /// simply failed its schema that the mutation contract could not be satisfied.
    ///
    /// The oracle case defers to [`CliError::from_compile`] rather than naming a code, so the two
    /// paths that surface the same [`FiberError`] cannot drift apart. The unaddressable case is
    /// this binary's fault rather than the caller's: a JSON document cannot carry a non-finite
    /// number, so a world that arrived intact and then failed to canonicalise was made
    /// unrepresentable here, not supplied that way.
    pub fn from_mutation(error: MutationError) -> Self {
        let message = error.to_string();
        match error {
            MutationError::DoesNotLoad { .. } => CliError::invalid(message),
            MutationError::NotEvaluable { source, .. } => {
                CliError::new(CliError::from_compile(source).code, message)
            }
            MutationError::NotAddressable { .. } => CliError::internal(message),
        }
    }

    /// Routes a minimization failure to the code carrying its 40.36 class.
    ///
    /// [`MinimizeError`] has one variant and it is an oracle that declined to judge the candidate,
    /// which lands where [`FiberError::UnorderableSplitGroups`] lands: the binary ran correctly and
    /// the evidence does not decide. Not `compile_failed` — nothing failed to compile — and not
    /// `invalid_input`, which would tell an operator their world was malformed when the world is
    /// exactly as supplied and the oracle simply cannot order what it holds. Written against the
    /// error type rather than at the call site so a second variant fails to compile until somebody
    /// decides which code it belongs under.
    pub fn from_minimize(error: MinimizeError) -> Self {
        let code = match &error {
            MinimizeError::OracleRefusedCandidate { .. } => ExitCode::Indeterminate,
        };
        CliError::new(code, error.to_string())
    }

    /// Routes a comparison failure to the code carrying its 40.36 class.
    ///
    /// [`CompareError`] has one variant and it is the oracle declining to judge the full-context
    /// reference, which lands where [`CliError::from_minimize`] lands for the same reason: the
    /// binary ran correctly and the evidence does not decide. Not `compile_failed` — every strategy
    /// compiled — and not `invalid_input`, which would tell an operator their world was malformed
    /// when the world is exactly as supplied and the oracle simply cannot order what it holds.
    ///
    /// The classification defers to [`CliError::from_compile`] rather than naming a code, so the
    /// paths that surface the same [`FiberError`] cannot drift apart. Written against the error type
    /// rather than at the call site so a second variant fails to compile until somebody decides
    /// which code it belongs under.
    pub fn from_compare(error: CompareError) -> Self {
        let message = error.to_string();
        match error {
            CompareError::OracleRefusedReference { source, .. } => {
                CliError::new(CliError::from_compile(source).code, message)
            }
        }
    }

    /// Routes an autopilot failure to the code carrying its 40.36 class.
    ///
    /// [`AutopilotError`] keeps the one distinction this registry cannot recover after the fact:
    /// a grant that does not authorise the mission it was asked to drive is a policy refusal —
    /// the mission is well-formed and the platform behaved correctly, so the caller's next
    /// conversation is with whoever issues grants, which is exit 7. A mission, mission report,
    /// workflow instantiation, or autopilot report that does not satisfy its contract is exit 3:
    /// the document must change before any re-send can succeed. Canonicalisation is the one
    /// failure that is this binary's fault rather than the caller's, so it lands where
    /// [`CliError::internal`] lands. Written against the error type rather than at each call
    /// site so a new variant fails to compile until somebody decides which code it belongs
    /// under.
    pub fn from_autopilot(error: AutopilotError) -> Self {
        let message = error.to_string();
        match error {
            AutopilotError::GrantDoesNotAuthorise { .. } => {
                CliError::new(ExitCode::PolicyDenied, message)
            }
            AutopilotError::InvalidMission { .. }
            | AutopilotError::InvalidReport { .. }
            | AutopilotError::InvalidInstantiation { .. }
            | AutopilotError::InvalidAutopilotReport { .. }
            | AutopilotError::InvalidCheckpoint { .. } => CliError::invalid(message),
            AutopilotError::Canonicalisation { .. }
            | AutopilotError::Persistence { .. }
            | AutopilotError::CompareAndSwapConflict
            | AutopilotError::Scheduling { .. } => CliError::internal(message),
        }
    }

    /// Routes an autonomy-grant refusal to `invalid_input`.
    ///
    /// Every [`GrantError`] names a field of the grant document that must change — an empty
    /// allow-list, a recursive tool, an attempt budget outside its bounds — so all of them are
    /// exit 3 rather than exit 7: nothing was refused *by* a policy here, the authority document
    /// itself failed its schema, and sending the operator to obtain a grant would send them to
    /// fix the wrong document. The match is exhaustive so a new refusal fails to compile until
    /// somebody decides whether it still describes a document defect.
    pub fn from_grant(error: GrantError) -> Self {
        match &error {
            GrantError::NoTools
            | GrantError::TooManyTools { .. }
            | GrantError::InvalidToolName { .. }
            | GrantError::DuplicateTool { .. }
            | GrantError::RecursiveTool
            | GrantError::InvalidAttemptBudget { .. }
            | GrantError::InvalidRetrySchedule { .. }
            | GrantError::UnsupportedStopOption => CliError::invalid(error.to_string()),
        }
    }

    /// The failure as the `--json` envelope publishes it.
    ///
    /// `retryable` is the boolean that was already on the wire and keeps its meaning. `retryability`
    /// is the three-valued decision beside it, because a consumer reading the envelope should not
    /// have to re-derive from the numeric code what the process could simply have said.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": self.code.as_i32(),
                "kind": self.code.slug(),
                "retryable": self.code.is_retryable(),
                "retryability": self.code.retryability().map(Retryability::as_str),
                "message": self.message,
                "subject": self.subject,
            }
        })
    }
}

/// Splits 40.25's policy failures between a refusal and a malformed authority.
///
/// A refusal is a decision the platform made correctly and clears when a grant is obtained. A
/// governing policy that does not parse, or a scope binding that names no clause, is a defect in
/// the world document, and telling the caller to obtain a grant for it would send them to the
/// wrong party.
fn policy_code(violation: &PolicyViolation) -> ExitCode {
    match violation {
        PolicyViolation::Conflict { .. } | PolicyViolation::ProtectedClosureWithheld { .. } => {
            ExitCode::PolicyDenied
        }
        PolicyViolation::MalformedDataPolicy { .. }
        | PolicyViolation::UninterpretableRequirement { .. } => ExitCode::InvalidInput,
    }
}

pub type CliResult<T> = Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caller_can_recover_the_retry_decision_from_the_exit_code_alone() {
        for code in ExitCode::ALL {
            let decision = code.retryability();
            match code {
                ExitCode::Ok | ExitCode::AssertionFailed => assert!(
                    decision.is_none(),
                    "{code} reports a verdict and must not publish a retry decision"
                ),
                _ => assert!(
                    decision.is_some(),
                    "{code} is a failure code that publishes no retry decision, so a caller \
                     branching on the status cannot decide whether to re-send"
                ),
            }
        }
    }

    #[test]
    fn no_code_advertises_the_opposite_of_the_retry_decision_it_carries() {
        for code in ExitCode::ALL {
            let permits = code
                .retryability()
                .is_some_and(Retryability::permits_automatic_retry);
            assert_eq!(
                code.is_retryable(),
                permits,
                "{code} advertises is_retryable()={} against a {:?} decision",
                code.is_retryable(),
                code.retryability()
            );
        }
    }

    #[test]
    fn stale_is_retryable_as_is_and_compile_failed_is_not() {
        assert_eq!(
            ExitCode::Stale.retryability(),
            Some(Retryability::RetryableAsIs)
        );
        assert!(ExitCode::Stale.is_retryable());
        assert_eq!(
            ExitCode::CompileFailed.retryability(),
            Some(Retryability::RetryableAfterChange)
        );
        assert!(!ExitCode::CompileFailed.is_retryable());
    }

    #[test]
    fn every_code_has_a_distinct_number_and_a_distinct_slug() {
        let mut numbers: Vec<i32> = ExitCode::ALL.iter().map(|c| c.as_i32()).collect();
        let mut slugs: Vec<&str> = ExitCode::ALL.iter().map(|c| c.slug()).collect();
        numbers.dedup();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(numbers.len(), 10);
        assert_eq!(slugs.len(), 10);
        assert_eq!(numbers, (0..10).collect::<Vec<i32>>());
    }

    #[test]
    fn a_policy_refusal_and_a_malformed_policy_do_not_share_a_code() {
        assert_eq!(
            policy_code(&PolicyViolation::Conflict {
                clauses: vec!["c".into()],
                governing: vec!["d".into()],
            }),
            ExitCode::PolicyDenied
        );
        assert_eq!(
            policy_code(&PolicyViolation::MalformedDataPolicy {
                fact_id: "f".into(),
                variable: "data_policy",
                found: "7".into(),
            }),
            ExitCode::InvalidInput
        );
    }

    #[test]
    fn an_oracle_that_cannot_decide_does_not_report_a_compile_failure() {
        let error = CliError::from_compile(FiberError::UnorderableSplitGroups {
            alias: "subject-1".into(),
            present: vec!["train".into()],
        });
        assert_eq!(error.code, ExitCode::Indeterminate);

        let contract = CliError::from_compile(FiberError::BudgetExceeded {
            selected: 12,
            max_facts: 4,
        });
        assert_eq!(contract.code, ExitCode::CompileFailed);
    }

    #[test]
    fn an_index_built_under_another_schema_is_stale_rather_than_invalid() {
        let error = CliError::from_store(
            std::path::Path::new("store"),
            StoreError::UnsupportedSchema {
                expected: "bioprism-store/0.1",
                actual: "bioprism-store/0.0".into(),
            },
        );
        assert_eq!(error.code, ExitCode::Stale);
        assert!(error.code.is_retryable());
    }

    #[test]
    fn an_unauthorised_mission_is_policy_denied_and_a_malformed_grant_is_invalid_input() {
        let refused = CliError::from_autopilot(AutopilotError::GrantDoesNotAuthorise {
            reason: "step `a` uses tool `b`, which the grant does not allow".into(),
        });
        assert_eq!(refused.code, ExitCode::PolicyDenied);

        let malformed_instantiation = CliError::from_autopilot(AutopilotError::InvalidInstantiation {
            reason: "workflow must be domain_workflow_instantiate".into(),
        });
        assert_eq!(malformed_instantiation.code, ExitCode::InvalidInput);

        let malformed_grant = CliError::from_grant(GrantError::NoTools);
        assert_eq!(malformed_grant.code, ExitCode::InvalidInput);
        let recursive_grant = CliError::from_grant(GrantError::RecursiveTool);
        assert_eq!(
            recursive_grant.code,
            ExitCode::InvalidInput,
            "a grant naming agent_mission is a defect in the authority document, not a refusal \
             a further grant could clear"
        );
    }

    #[test]
    fn the_json_envelope_publishes_the_three_valued_decision_beside_the_boolean() {
        let document = CliError::new(ExitCode::PolicyDenied, "refused").to_json();
        assert_eq!(document["error"]["code"], 7);
        assert_eq!(document["error"]["retryable"], false);
        assert_eq!(document["error"]["retryability"], "retryable_after_change");

        let verdict = CliError::new(ExitCode::Io, "gone").to_json();
        assert_eq!(verdict["error"]["retryable"], true);
        assert_eq!(verdict["error"]["retryability"], "retryable_as_is");
    }
}
