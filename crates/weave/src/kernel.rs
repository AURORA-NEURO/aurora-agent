//! The Weave microkernel.
//!
//! Blueprint 23.49 lists exactly what the trusted computing base must do — identity binding,
//! schema validation, role-state enforcement, act transitions, commitment transitions, grant
//! lifecycle, affine budget accounting, label checks, continuation integrity, causal ordering and
//! hashing — and, just as importantly, what it must *not*: "it should not decide scientific truth,
//! write patches, plan tasks, summarize evidence, or choose a model."
//!
//! This kernel holds to that. It never inspects an act's payload for meaning. It checks that the
//! move is legal, that the mover holds the authority, and that the budget covers it, then records
//! the transition. Everything about *what is true* lives in the epistemic ledger as claims and
//! challenges, for participants and oracles to argue over.

use crate::act::{Act, ActKind};
use crate::authority::{AuthorityError, AuthorityTable, Capability};
use crate::budget::{Budget, BudgetError, Resource};
use crate::ledger::{commitments, epistemic_state, ChainStatus, Ledger, LedgerEvent};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KernelError {
    #[error("unknown participant: {0}")]
    UnknownParticipant(String),

    #[error("{kind} requires an antecedent act")]
    MissingAntecedent { kind: &'static str },

    #[error("{kind} refers to unknown act {referenced}")]
    UnknownAntecedent {
        kind: &'static str,
        referenced: String,
    },

    #[error("{kind} may not follow {found}; expected one of {expected:?}")]
    IllegalTransition {
        kind: &'static str,
        found: &'static str,
        expected: Vec<&'static str>,
    },

    #[error("{actor} already discharged commitment {commitment}")]
    AlreadyDischarged { actor: String, commitment: String },

    #[error(transparent)]
    Authority(#[from] AuthorityError),

    #[error(transparent)]
    Budget(#[from] BudgetError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub name: String,
    pub role: String,
    /// The grant this participant acts under.
    pub grant: String,
    /// Cognitive ABI portability grade (23.49). Recorded so the kernel never asks a P0 adapter to
    /// resume a continuation it cannot hold.
    pub abi_grade: u8,
}

/// What each act costs and requires. The kernel's whole policy table, deliberately small.
fn requirement(kind: ActKind) -> (Option<Capability>, u64) {
    match kind {
        ActKind::Ask => (None, 1),
        ActKind::Claim => (Some(Capability::ReadEvidence), 2),
        ActKind::Propose => (None, 1),
        ActKind::Accept => (None, 1),
        ActKind::Reject => (None, 1),
        ActKind::Challenge => (Some(Capability::ReadEvidence), 2),
        ActKind::Discharge => (Some(Capability::ExecuteTests), 3),
        ActKind::Delegate => (None, 1),
        ActKind::Revoke => (None, 1),
        ActKind::Attest => (Some(Capability::PublishResult), 2),
    }
}

pub struct Kernel {
    participants: BTreeMap<String, Participant>,
    authority: AuthorityTable,
    budgets: BTreeMap<String, Budget>,
    ledger: Ledger,
    rejected: usize,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel {
            participants: BTreeMap::new(),
            authority: AuthorityTable::new(),
            budgets: BTreeMap::new(),
            ledger: Ledger::new(),
            rejected: 0,
        }
    }

    pub fn authority_mut(&mut self) -> &mut AuthorityTable {
        &mut self.authority
    }

    pub fn authority(&self) -> &AuthorityTable {
        &self.authority
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Acts the kernel refused. Counted for every rejection reason — unknown participant,
    /// illegal transition, insufficient authority, exhausted budget — so an operator can see
    /// pressure against the policy rather than only its successes.
    pub fn rejected_acts(&self) -> usize {
        self.rejected
    }

    pub fn admit(&mut self, participant: Participant, budget: Budget) {
        self.budgets.insert(participant.name.clone(), budget);
        self.participants
            .insert(participant.name.clone(), participant);
    }

    pub fn participant(&self, name: &str) -> Option<&Participant> {
        self.participants.get(name)
    }

    pub fn remaining(&self, participant: &str, resource: Resource) -> u64 {
        self.budgets
            .get(participant)
            .map(|budget| budget.remaining(resource))
            .unwrap_or(0)
    }

    /// Validates and records one act.
    ///
    /// Order matters and is the security property: identity, then protocol legality, then
    /// authority, then budget. A rejected act is *not* recorded — an unauthorised move must not be
    /// able to grow the ledger, or a participant with no budget could still write history.
    pub fn submit(&mut self, act: Act) -> Result<LedgerEvent, KernelError> {
        match self.try_submit(act) {
            Ok(event) => Ok(event),
            Err(error) => {
                self.rejected += 1;
                Err(error)
            }
        }
    }

    fn try_submit(&mut self, act: Act) -> Result<LedgerEvent, KernelError> {
        let actor = self
            .participants
            .get(&act.from)
            .ok_or_else(|| KernelError::UnknownParticipant(act.from.clone()))?
            .clone();

        if let Some(expected) = act.kind.requires_antecedent() {
            let referenced = act
                .in_reply_to
                .clone()
                .ok_or(KernelError::MissingAntecedent {
                    kind: act.kind.as_str(),
                })?;
            let antecedent =
                self.ledger
                    .get(&referenced)
                    .ok_or_else(|| KernelError::UnknownAntecedent {
                        kind: act.kind.as_str(),
                        referenced: referenced.clone(),
                    })?;
            if !expected.contains(&antecedent.act.kind) {
                return Err(KernelError::IllegalTransition {
                    kind: act.kind.as_str(),
                    found: antecedent.act.kind.as_str(),
                    expected: expected.iter().map(|kind| kind.as_str()).collect(),
                });
            }

            if act.kind == ActKind::Discharge {
                let already = commitments(&self.ledger)
                    .get(&referenced)
                    .map(|commitment| {
                        commitment.state == crate::ledger::CommitmentState::Discharged
                    })
                    .unwrap_or(false);
                if already {
                    return Err(KernelError::AlreadyDischarged {
                        actor: act.from.clone(),
                        commitment: referenced,
                    });
                }
            }
        }

        let (capability, cost) = requirement(act.kind);
        if let Some(required) = capability {
            self.authority.check(&actor.grant, required)?;
        }

        let budget = self
            .budgets
            .entry(actor.name.clone())
            .or_insert_with(|| Budget::new().with(Resource::ToolCalls, 0));
        budget.spend(Resource::ToolCalls, cost)?;

        Ok(self.ledger.append(act))
    }

    pub fn open_commitments(&self) -> Vec<String> {
        commitments(&self.ledger)
            .into_iter()
            .filter(|(_, commitment)| commitment.state == crate::ledger::CommitmentState::Open)
            .map(|(id, _)| id)
            .collect()
    }

    pub fn contested_claims(&self) -> BTreeSet<String> {
        epistemic_state(&self.ledger)
            .into_iter()
            .filter(|(_, entry)| entry.is_contested())
            .map(|(id, _)| id)
            .collect()
    }

    pub fn verify_chain(&self) -> ChainStatus {
        self.ledger.verify_chain()
    }

    /// The kernel's view of its own state, for a PRISM decision hook (23.26).
    pub fn checkpoint(&self) -> serde_json::Value {
        serde_json::json!({
            "head": self.ledger.head(),
            "events": self.ledger.len(),
            "rejected_acts": self.rejected,
            "open_commitments": self.open_commitments(),
            "contested_claims": self.contested_claims().into_iter().collect::<Vec<_>>(),
            "participants": self.participants.keys().collect::<Vec<_>>(),
        })
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}
