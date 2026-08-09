//! Falsifiable Biological Contract IR — blueprint 25.07.
//!
//! A contract states an intent, the scope it holds in, the evidence obligations that must close, the
//! actions permitted while closing them, the shape of the claim, and — the part that makes it a
//! contract rather than a plan — the falsifiers that would refute it.
//!
//! # The three invariants
//!
//! - *Every success state closes required obligations.* [`Fbc::validate`] walks the terminal states
//!   and refuses a [`Termination::Success`] that leaves a required obligation open.
//! - *Unsupported scope expansion invalidates the claim.* [`Fbc::admits_claim_in`] uses
//!   `bioprism-scope`'s refinement order: a claim scope that does not refine the envelope is
//!   refused, naming the dimension that escaped.
//! - *Underdetermined is a valid terminal state.* [`Termination::Underdetermined`] is a first-class
//!   variant, not an error. A contract that can only succeed or fail forces a verdict out of
//!   evidence that does not support one, which is the failure mode `bioprism-oracle` also refuses
//!   with its set-valued combination.
//!
//! # What is deliberately not implemented
//!
//! - **No falsifier execution.** 25.07 lists a "falsifier executable check" under validation. What is
//!   checked here is that each falsifier names an oracle the contract's mesh contains; whether that
//!   oracle actually runs is `bioprism-oracle`'s question and needs an isolated grader environment
//!   this crate cannot provide.
//! - **No claim schema language.** [`ClaimSchema`] carries a digest and a prose description. Giving
//!   claims a grammar would be inventing the one thing 25.07 leaves entirely open.
//! - **No scoring.** 25.07's title mentions scoring; the module body never defines a scale, a
//!   weighting or an aggregation, so none is implemented. See the crate documentation's list of §25
//!   constructs that are named but never specified.

use crate::error::FbcError;
use crate::ids::{ActionId, FbcId, ObligationId};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What the contract is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    /// The question in one sentence.
    pub statement: String,
    /// Who is asking, as a role name.
    pub requester_role: String,
}

/// A piece of evidence that must be produced before a claim may be made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObligation {
    pub obligation_id: ObligationId,
    pub description: String,
    /// Actions that can discharge it. An obligation no action can discharge is unreachable.
    pub dischargeable_by: BTreeSet<ActionId>,
    /// False for obligations that strengthen a claim without being necessary for it.
    pub required: bool,
}

/// Something that, if observed, refutes the claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Falsifier {
    pub falsifier_id: String,
    /// What would have to be seen.
    pub condition: String,
    /// The oracle that would see it. Must be in the contract's mesh.
    pub oracle: String,
}

/// The shape of the claim the contract permits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSchema {
    /// A digest of the published schema document.
    pub schema_digest: String,
    pub description: String,
}

/// How the contract can end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "termination", rename_all = "snake_case")]
pub enum Termination {
    /// The claim is made. Requires every required obligation closed.
    Success,
    /// The claim is refuted by a falsifier.
    Refuted { falsifier: String },
    /// The evidence does not settle the question. A legitimate ending, not a failure.
    Underdetermined { reason: String },
    /// The contract could not proceed: budget, authority, or a missing precondition.
    Aborted { reason: String },
}

impl Termination {
    pub fn is_success(&self) -> bool {
        matches!(self, Termination::Success)
    }
}

/// One reachable ending, with what it leaves open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalState {
    pub label: String,
    pub termination: Termination,
    /// Obligations still open when the contract ends this way.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub open_obligations: BTreeSet<ObligationId>,
}

/// A falsifiable biological contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fbc {
    pub fbc_id: FbcId,
    pub intent: Intent,
    /// The scope envelope. A claim may be narrower; it may not be wider.
    pub scope: ScopeKey,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<String>,
    pub obligations: Vec<EvidenceObligation>,
    pub allowed_actions: BTreeSet<ActionId>,
    pub claim_schema: ClaimSchema,
    pub falsifiers: Vec<Falsifier>,
    pub oracle_mesh: BTreeSet<String>,
    /// Resource caps, by the same resource names states and actions use.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub capped_resources: BTreeSet<String>,
    pub terminal_states: Vec<TerminalState>,
}

impl Fbc {
    /// Every invariant 25.07 states that a contract can be checked against on its own.
    pub fn validate(&self) -> Result<(), FbcError> {
        if self.falsifiers.is_empty() {
            return Err(FbcError::NoFalsifier {
                contract: self.fbc_id.to_string(),
            });
        }

        for falsifier in &self.falsifiers {
            if !self.oracle_mesh.contains(&falsifier.oracle) {
                return Err(FbcError::FalsifierWithoutOracle {
                    falsifier: falsifier.falsifier_id.clone(),
                    oracle: falsifier.oracle.clone(),
                });
            }
        }

        for obligation in &self.obligations {
            if !obligation.required {
                continue;
            }
            let reachable = obligation
                .dischargeable_by
                .iter()
                .any(|action| self.allowed_actions.contains(action));
            if !reachable {
                return Err(FbcError::UnreachableObligation {
                    obligation: obligation.obligation_id.to_string(),
                });
            }
        }

        for terminal in &self.terminal_states {
            if !terminal.termination.is_success() {
                continue;
            }
            for open in &terminal.open_obligations {
                let required = self
                    .obligations
                    .iter()
                    .any(|obligation| &obligation.obligation_id == open && obligation.required);
                if required {
                    return Err(FbcError::SuccessWithOpenObligation {
                        state: terminal.label.clone(),
                        obligation: open.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Whether a claim made in `claim_scope` is inside the contract's envelope.
    pub fn admits_claim_in(&self, claim_scope: &ScopeKey) -> Result<(), FbcError> {
        if claim_scope.refines(&self.scope) {
            return Ok(());
        }
        let dimension = self
            .scope
            .iter()
            .find(|(dimension, coarse)| {
                !claim_scope
                    .get(dimension)
                    .is_some_and(|fine| fine.refines(coarse))
            })
            .map(|(dimension, _)| dimension.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Err(FbcError::UnsupportedScopeExpansion { dimension })
    }

    /// The obligations that must close for a success.
    pub fn required_obligations(&self) -> impl Iterator<Item = &EvidenceObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.required)
    }
}
