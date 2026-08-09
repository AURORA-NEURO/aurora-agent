//! Context failure modes and recovery (39.24).
//!
//! 39.24 is about contexts that are "small but biologically unsafe or insufficient", and it names
//! six failure modes: summary polarity reversal, identity collapse, future leakage, stale cache,
//! budget-induced omission, and role over-redaction. Each is a variant of [`ContextFailure`],
//! carrying the operands a caller would need to build the typed event the section requires.
//!
//! # The four invariants, as constructor preconditions
//!
//! 1. *"Recovery preserves the failed artifact for audit."* [`RecoveryRecord`] has no constructor
//!    that omits `failed_digest`, and [`RecoveryError::FailedArtifactNotPreserved`] fires on an
//!    empty one. The failed context is the evidence; discarding it to save space would be this
//!    section's own thesis failing at the last step.
//! 2. *"Do not solve insufficiency by exposing hidden evaluator truth."* An escalation naming a
//!    holdout node is [`RecoveryError::WouldExposeHoldout`]. Note what is *not* done: there is no
//!    `Escalation::RevealHoldout` variant that is then rejected at runtime. The refusal is at
//!    construction, so a holdout-revealing recovery never exists as a value.
//! 3. *"Escalation increases resolution selectively."* [`Escalation::RaiseResolution`] must name
//!    nodes, and a whole-context recompile is a different, more expensive variant that must state
//!    why selective escalation was insufficient. Without that split, "recompile everything" is the
//!    cheapest thing to write and therefore what gets written.
//! 4. *"Repeated failures trigger policy rollback."* [`FailureStreak`] counts consecutive failures
//!    against one policy and returns [`PolicyAction::Rollback`] at the threshold. Consecutive, not
//!    cumulative: a policy that failed once a year ago and once today has not demonstrated a
//!    pattern.
//!
//! # Not implemented
//!
//! No divergence localisation. 39.24's `FirstDivergence` service and its "minimal missing subgraph"
//! output require replaying a decision against an oracle, which needs a model this crate does not
//! have. [`ContextFailure`] is therefore a diagnosis *somebody else reached*, and every variant
//! records who: [`Diagnosis::diagnosed_by`]. Nothing here infers a failure from a context.

use crate::error::RecoveryError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The six failure modes 39.24 names, with the evidence each one needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "failure", rename_all = "snake_case")]
pub enum ContextFailure {
    /// A summary reversed the sign or direction of what it summarised: "no evidence of progression"
    /// standing in for "evidence of no progression", or a fold-change that lost its direction.
    SummaryPolarityReversal {
        summary_id: String,
        source_locator: String,
    },
    /// Two distinct identities were merged: two specimens, two lesions, two timepoints of one
    /// lesion treated as one thing.
    IdentityCollapse { merged: Vec<String> },
    /// Evidence released after the decision's visibility cutoff reached the context.
    FutureLeakage {
        node_id: String,
        available_at: u64,
        cutoff: u64,
    },
    /// A cached projection outside its declared validity was used. The bridge to
    /// [`crate::staleness`].
    StaleCache { context_id: String, detail: String },
    /// A node was dropped for size, and it turned out to matter. Distinct from an ordinary
    /// omission: the ledger recorded it, and the recording was not enough.
    BudgetInducedOmission {
        node_id: String,
        obligation: String,
    },
    /// A role filter removed something the role needed. The opposite failure to a leak, and just as
    /// real: an over-redacted context produces a confident answer from a partial picture.
    RoleOverRedaction { role: String, node_id: String },
}

impl ContextFailure {
    pub fn kind(&self) -> &'static str {
        match self {
            ContextFailure::SummaryPolarityReversal { .. } => "summary_polarity_reversal",
            ContextFailure::IdentityCollapse { .. } => "identity_collapse",
            ContextFailure::FutureLeakage { .. } => "future_leakage",
            ContextFailure::StaleCache { .. } => "stale_cache",
            ContextFailure::BudgetInducedOmission { .. } => "budget_induced_omission",
            ContextFailure::RoleOverRedaction { .. } => "role_over_redaction",
        }
    }

    /// Whether the failure invalidates the underlying result or only the current projection.
    ///
    /// Every module of section 39 requires a failure event to state this. A leak and an identity
    /// collapse contaminate the result itself — recompiling does not undo a decision made on merged
    /// identities — while an over-redaction or a budget omission means the projection was wrong and
    /// the underlying evidence is fine.
    pub fn invalidates_underlying_result(&self) -> bool {
        matches!(
            self,
            ContextFailure::FutureLeakage { .. }
                | ContextFailure::IdentityCollapse { .. }
                | ContextFailure::SummaryPolarityReversal { .. }
        )
    }

    /// Nodes this failure implicates, for checking a proposed escalation against it.
    pub fn implicated_nodes(&self) -> BTreeSet<String> {
        match self {
            ContextFailure::SummaryPolarityReversal { summary_id, .. } => {
                BTreeSet::from([summary_id.clone()])
            }
            ContextFailure::IdentityCollapse { merged } => merged.iter().cloned().collect(),
            ContextFailure::FutureLeakage { node_id, .. }
            | ContextFailure::BudgetInducedOmission { node_id, .. }
            | ContextFailure::RoleOverRedaction { node_id, .. } => {
                BTreeSet::from([node_id.clone()])
            }
            ContextFailure::StaleCache { context_id, .. } => BTreeSet::from([context_id.clone()]),
        }
    }
}

/// A failure as diagnosed, with who diagnosed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub failure_id: String,
    pub failure: ContextFailure,
    /// The oracle, reviewer or check that reached this conclusion. This crate never fills it in
    /// from its own inference, because it performs none.
    pub diagnosed_by: String,
    /// The policy in force when the failure occurred, so a streak can be attributed.
    pub policy_id: String,
}

impl Diagnosis {
    pub fn new(
        failure_id: impl Into<String>,
        failure: ContextFailure,
        diagnosed_by: impl Into<String>,
        policy_id: impl Into<String>,
    ) -> Self {
        Diagnosis {
            failure_id: failure_id.into(),
            failure,
            diagnosed_by: diagnosed_by.into(),
            policy_id: policy_id.into(),
        }
    }
}

/// A change of resolution on a named set of nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionChange {
    pub from: String,
    pub to: String,
}

impl ResolutionChange {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        ResolutionChange {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// What a recovery proposes to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "escalation", rename_all = "snake_case")]
pub enum Escalation {
    /// Raise resolution on specific nodes. The cheap, selective repair 39.24 wants.
    RaiseResolution {
        nodes: BTreeSet<String>,
        change: ResolutionChange,
    },
    /// Recompile under a different context policy without changing resolution.
    AlternatePolicy { policy_id: String },
    /// Recompile the whole context. Expensive and blunt, so it must say why the selective route was
    /// not enough.
    RecompileWholeContext { why_selective_insufficient: String },
}

impl Escalation {
    /// Nodes the escalation would newly expose.
    pub fn exposes(&self) -> BTreeSet<String> {
        match self {
            Escalation::RaiseResolution { nodes, .. } => nodes.clone(),
            Escalation::AlternatePolicy { .. } | Escalation::RecompileWholeContext { .. } => {
                BTreeSet::new()
            }
        }
    }

    pub fn is_selective(&self) -> bool {
        matches!(self, Escalation::RaiseResolution { .. })
    }
}

/// A recovery attempt, with the failed artifact preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub diagnosis: Diagnosis,
    /// The digest of the context that failed. Preserved, never replaced by the recompiled one.
    pub failed_digest: String,
    pub escalation: Escalation,
    /// The digest of the recompiled context, once one exists. `None` while the recovery is proposed
    /// but not performed — this crate performs nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recompiled_digest: Option<String>,
    /// The minimised case promoted to the regression suite, per 39.24's execution path step 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression_fixture: Option<String>,
}

impl RecoveryRecord {
    /// Propose a recovery, refusing the three shapes 39.24 forbids.
    ///
    /// `holdouts` is the evaluator's hidden set. It is a parameter rather than a field on the
    /// escalation because the escalation does not know what is hidden — the caller does, and making
    /// it supply the set is what forces the check to happen against real knowledge rather than
    /// against whatever the escalation happened to declare about itself.
    pub fn propose(
        diagnosis: Diagnosis,
        failed_digest: impl Into<String>,
        escalation: Escalation,
        holdouts: &BTreeSet<String>,
    ) -> Result<Self, RecoveryError> {
        let failed_digest = failed_digest.into();
        if failed_digest.trim().is_empty() {
            return Err(RecoveryError::FailedArtifactNotPreserved(
                diagnosis.failure_id,
            ));
        }
        match &escalation {
            Escalation::RaiseResolution { nodes, .. } if nodes.is_empty() => {
                return Err(RecoveryError::SelectiveEscalationNamesNoNodes(
                    diagnosis.failure_id,
                ));
            }
            Escalation::RecompileWholeContext {
                why_selective_insufficient,
            } if why_selective_insufficient.trim().is_empty() => {
                return Err(RecoveryError::WholeRecompileUnjustified(
                    diagnosis.failure_id,
                ));
            }
            _ => {}
        }
        if let Some(node) = escalation.exposes().intersection(holdouts).min() {
            return Err(RecoveryError::WouldExposeHoldout {
                failure: diagnosis.failure_id,
                node: node.clone(),
            });
        }
        Ok(RecoveryRecord {
            diagnosis,
            failed_digest,
            escalation,
            recompiled_digest: None,
            regression_fixture: None,
        })
    }

    /// Record the recompiled artifact. The failed one stays where it is.
    pub fn recompiled(mut self, digest: impl Into<String>) -> Self {
        self.recompiled_digest = Some(digest.into());
        self
    }

    pub fn promoting(mut self, fixture_id: impl Into<String>) -> Self {
        self.regression_fixture = Some(fixture_id.into());
        self
    }

    /// Whether the recovery repaired the projection or only the projection could be repaired.
    pub fn underlying_result_survives(&self) -> bool {
        !self.diagnosis.failure.invalidates_underlying_result()
    }
}

/// What to do about a policy that keeps failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "policy_action", rename_all = "snake_case")]
pub enum PolicyAction {
    /// Below the threshold. Keep the policy and keep counting.
    Continue { consecutive: usize, threshold: usize },
    /// At or past the threshold. 39.24's fourth invariant.
    Rollback {
        policy_id: String,
        consecutive: usize,
        failure_kinds: BTreeSet<String>,
    },
}

impl PolicyAction {
    pub fn is_rollback(&self) -> bool {
        matches!(self, PolicyAction::Rollback { .. })
    }
}

/// Consecutive failures against one policy.
///
/// Consecutive rather than cumulative, and a success resets it. A cumulative counter would roll
/// back every policy eventually, which trains an operator to ignore the signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureStreak {
    pub policy_id: String,
    pub threshold: usize,
    pub failures: Vec<Diagnosis>,
}

impl FailureStreak {
    pub fn new(policy_id: impl Into<String>, threshold: usize) -> Self {
        FailureStreak {
            policy_id: policy_id.into(),
            threshold,
            failures: Vec::new(),
        }
    }

    /// Record a failure. Failures under another policy are ignored, not counted: a streak is about
    /// one policy's behaviour.
    pub fn observe_failure(&mut self, diagnosis: Diagnosis) {
        if diagnosis.policy_id == self.policy_id {
            self.failures.push(diagnosis);
        }
    }

    pub fn observe_success(&mut self) {
        self.failures.clear();
    }

    pub fn action(&self) -> PolicyAction {
        if self.threshold > 0 && self.failures.len() >= self.threshold {
            PolicyAction::Rollback {
                policy_id: self.policy_id.clone(),
                consecutive: self.failures.len(),
                failure_kinds: self
                    .failures
                    .iter()
                    .map(|diagnosis| diagnosis.failure.kind().to_string())
                    .collect(),
            }
        } else {
            PolicyAction::Continue {
                consecutive: self.failures.len(),
                threshold: self.threshold,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holdouts() -> BTreeSet<String> {
        BTreeSet::from(["n/oracle-answer".to_string()])
    }

    fn omission_failure() -> Diagnosis {
        Diagnosis::new(
            "fail/1",
            ContextFailure::BudgetInducedOmission {
                node_id: "n/mgmt".to_string(),
                obligation: "o/mgmt-status".to_string(),
            },
            "oracle/deterministic",
            "policy/aggressive",
        )
    }

    fn selective(nodes: &[&str]) -> Escalation {
        Escalation::RaiseResolution {
            nodes: nodes.iter().map(|node| node.to_string()).collect(),
            change: ResolutionChange::new("l1", "l3"),
        }
    }

    #[test]
    fn a_recovery_preserves_the_failed_artifact_alongside_the_recompiled_one() {
        let record = RecoveryRecord::propose(
            omission_failure(),
            "digest/failed",
            selective(&["n/mgmt"]),
            &holdouts(),
        )
        .expect("proposes")
        .recompiled("digest/repaired");
        assert_eq!(record.failed_digest, "digest/failed");
        assert_eq!(record.recompiled_digest.as_deref(), Some("digest/repaired"));
    }

    #[test]
    fn a_recovery_that_discards_the_failed_artifact_is_refused() {
        assert!(matches!(
            RecoveryRecord::propose(omission_failure(), "  ", selective(&["n/mgmt"]), &holdouts()),
            Err(RecoveryError::FailedArtifactNotPreserved(_))
        ));
    }

    #[test]
    fn an_escalation_that_would_reveal_the_evaluators_answer_is_refused() {
        assert!(matches!(
            RecoveryRecord::propose(
                omission_failure(),
                "digest/failed",
                selective(&["n/mgmt", "n/oracle-answer"]),
                &holdouts()
            ),
            Err(RecoveryError::WouldExposeHoldout { ref node, .. }) if node == "n/oracle-answer"
        ));
    }

    #[test]
    fn a_selective_escalation_that_names_no_nodes_is_refused_as_a_disguised_full_recompile() {
        assert!(matches!(
            RecoveryRecord::propose(
                omission_failure(),
                "digest/failed",
                selective(&[]),
                &holdouts()
            ),
            Err(RecoveryError::SelectiveEscalationNamesNoNodes(_))
        ));
    }

    #[test]
    fn a_whole_context_recompile_must_say_why_selective_escalation_was_not_enough() {
        assert!(matches!(
            RecoveryRecord::propose(
                omission_failure(),
                "digest/failed",
                Escalation::RecompileWholeContext {
                    why_selective_insufficient: String::new()
                },
                &holdouts()
            ),
            Err(RecoveryError::WholeRecompileUnjustified(_))
        ));
        assert!(RecoveryRecord::propose(
            omission_failure(),
            "digest/failed",
            Escalation::RecompileWholeContext {
                why_selective_insufficient:
                    "the identity collapse spans every node, so no subset is minimal".to_string()
            },
            &holdouts()
        )
        .is_ok());
    }

    #[test]
    fn a_leak_invalidates_the_underlying_result_and_an_over_redaction_only_the_projection() {
        let leak = ContextFailure::FutureLeakage {
            node_id: "n/followup".to_string(),
            available_at: 12,
            cutoff: 8,
        };
        let redaction = ContextFailure::RoleOverRedaction {
            role: "stats".to_string(),
            node_id: "n/cohort-rule".to_string(),
        };
        assert!(leak.invalidates_underlying_result());
        assert!(!redaction.invalidates_underlying_result());
    }

    #[test]
    fn a_recovery_record_reports_whether_recompiling_can_save_the_result_at_all() {
        let leak = Diagnosis::new(
            "fail/leak",
            ContextFailure::FutureLeakage {
                node_id: "n/followup".to_string(),
                available_at: 12,
                cutoff: 8,
            },
            "oracle/temporal",
            "policy/a",
        );
        let record = RecoveryRecord::propose(
            leak,
            "digest/failed",
            Escalation::AlternatePolicy {
                policy_id: "policy/strict-cutoff".to_string(),
            },
            &holdouts(),
        )
        .expect("proposes");
        assert!(!record.underlying_result_survives());
    }

    #[test]
    fn every_failure_mode_named_in_the_specification_has_a_distinct_kind() {
        let kinds: BTreeSet<&str> = [
            ContextFailure::SummaryPolarityReversal {
                summary_id: "s".to_string(),
                source_locator: "l".to_string(),
            },
            ContextFailure::IdentityCollapse {
                merged: vec!["a".to_string(), "b".to_string()],
            },
            ContextFailure::FutureLeakage {
                node_id: "n".to_string(),
                available_at: 2,
                cutoff: 1,
            },
            ContextFailure::StaleCache {
                context_id: "c".to_string(),
                detail: "d".to_string(),
            },
            ContextFailure::BudgetInducedOmission {
                node_id: "n".to_string(),
                obligation: "o".to_string(),
            },
            ContextFailure::RoleOverRedaction {
                role: "r".to_string(),
                node_id: "n".to_string(),
            },
        ]
        .iter()
        .map(ContextFailure::kind)
        .collect();
        assert_eq!(kinds.len(), 6);
    }

    #[test]
    fn a_policy_rolls_back_only_after_consecutive_failures_reach_the_threshold() {
        let mut streak = FailureStreak::new("policy/aggressive", 3);
        streak.observe_failure(omission_failure());
        streak.observe_failure(omission_failure());
        assert!(!streak.action().is_rollback());
        streak.observe_failure(omission_failure());
        assert!(streak.action().is_rollback());
    }

    #[test]
    fn a_success_resets_the_streak_so_scattered_failures_do_not_accumulate_into_a_rollback() {
        let mut streak = FailureStreak::new("policy/aggressive", 3);
        streak.observe_failure(omission_failure());
        streak.observe_failure(omission_failure());
        streak.observe_success();
        streak.observe_failure(omission_failure());
        assert!(matches!(
            streak.action(),
            PolicyAction::Continue { consecutive: 1, .. }
        ));
    }

    #[test]
    fn a_failure_under_another_policy_does_not_count_against_this_one() {
        let mut streak = FailureStreak::new("policy/aggressive", 2);
        streak.observe_failure(Diagnosis::new(
            "fail/other",
            ContextFailure::StaleCache {
                context_id: "c".to_string(),
                detail: "expired".to_string(),
            },
            "check/staleness",
            "policy/other",
        ));
        streak.observe_failure(omission_failure());
        assert!(!streak.action().is_rollback());
    }

    #[test]
    fn a_rollback_names_which_failure_kinds_drove_it() {
        let mut streak = FailureStreak::new("policy/aggressive", 2);
        streak.observe_failure(omission_failure());
        streak.observe_failure(Diagnosis::new(
            "fail/2",
            ContextFailure::RoleOverRedaction {
                role: "molecular".to_string(),
                node_id: "n/x".to_string(),
            },
            "reviewer/blinded",
            "policy/aggressive",
        ));
        let PolicyAction::Rollback { failure_kinds, .. } = streak.action() else {
            panic!("expected a rollback");
        };
        assert!(failure_kinds.contains("budget_induced_omission"));
        assert!(failure_kinds.contains("role_over_redaction"));
    }

    #[test]
    fn a_recovery_record_survives_a_json_round_trip() {
        let record = RecoveryRecord::propose(
            omission_failure(),
            "digest/failed",
            selective(&["n/mgmt"]),
            &holdouts(),
        )
        .expect("proposes")
        .promoting("fx/regression-mgmt");
        let text = serde_json::to_string(&record).expect("serialises");
        let back: RecoveryRecord = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, record);
    }
}
