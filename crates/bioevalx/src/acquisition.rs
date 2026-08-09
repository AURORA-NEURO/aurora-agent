//! What an agent went and got, what that closed, and when it should have stopped (26.05).
//!
//! 26.05 measures "whether an agent chooses the next source, assay, tool, or expert input that
//! most efficiently resolves biological uncertainty". Its metric list is built almost entirely on
//! quantities the section never defines — "information gain per token/cost", "regret versus oracle
//! acquisition policy", "retrieval stopping quality", "downstream decision improvement" — and the
//! honest response is to implement the accounting and refuse the estimators.
//!
//! What *is* checkable is the obligation ledger. An obligation is a thing that must be known
//! before the decision is admissible; an acquisition closes zero or more of them; and a trace can
//! be read for three findings that need no information-theoretic model at all:
//!
//! - [`Trace::redundant`] — acquisitions that closed nothing already open. 26.05's "semantic
//!   similarity retrieves redundant evidence".
//! - [`Trace::unnecessary`] — acquisitions after every obligation was already closed. 26.05's
//!   "agent orders every available assay".
//! - [`Trace::deferred_decisive`] — the decisive acquisition ranked behind cheaper ones that
//!   closed nothing. 26.05's "cheap evidence delays a decisive source".
//!
//! # Regret refuses rather than defaulting
//!
//! [`Trace::regret_against`] takes a reference policy and returns a cost difference; there is no
//! zero-argument `regret()`. 26.05's design detail names five comparison policies ("full-context
//! ingestion, similarity retrieval, dependency retrieval, random acquisition, and BioPRISM
//! information-value selection under identical budgets") and does not privilege one, so a regret
//! number computed against an unnamed baseline would be a number about a policy the reader cannot
//! see. [`AcquisitionError::NoReferencePolicy`] says exactly that.
//!
//! # Not implemented
//!
//! No information gain. Estimating it needs a posterior over hypotheses, which is
//! `bioprism-evalengine`'s capability posterior for a different quantity and nothing at all for
//! this one. No downstream utility: 26.05's "downstream decision improvement" needs the decision
//! replayed under the counterfactual acquisition, which is [`crate::design`]'s matched-fork
//! machinery and belongs to a caller who can actually rerun the cell.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::AcquisitionError;

/// The kinds of acquisition 26.05 lists under "Evaluation target".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionKind {
    /// "document and database retrieval"
    Retrieval,
    /// "assay selection"
    Assay,
    /// "metadata inspection"
    Metadata,
    /// "expert consultation"
    Expert,
    /// "additional analysis"
    Analysis,
}

impl AcquisitionKind {
    /// Every kind, in blueprint listing order. "Stopping decisions" is 26.05's sixth target and is
    /// not a kind here — a stop is the absence of a further action, modelled by
    /// [`Trace::stopped_after`].
    pub const ALL: [AcquisitionKind; 5] = [
        AcquisitionKind::Retrieval,
        AcquisitionKind::Assay,
        AcquisitionKind::Metadata,
        AcquisitionKind::Expert,
        AcquisitionKind::Analysis,
    ];
}

/// One thing that must be known before the decision is admissible.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Obligation {
    pub id: String,
    /// Whether the decision is inadmissible without this, as opposed to merely weaker. Carried so
    /// that a trace closing every optional obligation and no required one is not read as thorough.
    pub required: bool,
}

impl Obligation {
    /// An obligation the decision cannot be admissible without.
    pub fn required(id: impl Into<String>) -> Self {
        Obligation {
            id: id.into(),
            required: true,
        }
    }

    /// An obligation whose closure strengthens but does not gate the decision.
    pub fn optional(id: impl Into<String>) -> Self {
        Obligation {
            id: id.into(),
            required: false,
        }
    }
}

/// One acquisition the agent performed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub kind: AcquisitionKind,
    /// Cost in a caller-chosen unit. Compared only within one trace and one reference policy;
    /// this module never converts between units.
    pub cost: u64,
    /// Obligations this acquisition closed.
    pub closes: BTreeSet<String>,
}

impl Action {
    /// An acquisition that closes nothing.
    pub fn new(id: impl Into<String>, kind: AcquisitionKind, cost: u64) -> Self {
        Action {
            id: id.into(),
            kind,
            cost,
            closes: BTreeSet::new(),
        }
    }

    /// Record an obligation this acquisition closed.
    pub fn closing(mut self, obligation: impl Into<String>) -> Self {
        self.closes.insert(obligation.into());
        self
    }
}

/// A sequence of acquisitions against a declared obligation set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    obligations: Vec<Obligation>,
    actions: Vec<Action>,
    /// Whether the agent stopped of its own accord rather than exhausting a budget. 26.05's sixth
    /// target and its "test stopping under diminishing returns" step both need this distinction:
    /// a run that stopped because it ran out of budget has demonstrated nothing about stopping.
    pub stopped_after: bool,
}

impl Trace {
    /// Start a trace against a declared obligation set.
    pub fn against(obligations: Vec<Obligation>) -> Self {
        Trace {
            obligations,
            actions: Vec::new(),
            stopped_after: false,
        }
    }

    /// Append an acquisition, refusing one that claims to close an obligation nobody declared.
    ///
    /// The refusal matters: an agent credited for closing an obligation that was never open has
    /// been credited for work that had no reason to be done, and that credit is what 26.05's
    /// "unnecessary acquisition rate" is trying to catch.
    pub fn perform(&mut self, action: Action) -> Result<(), AcquisitionError> {
        if self.actions.iter().any(|a| a.id == action.id) {
            return Err(AcquisitionError::DuplicateAction(action.id));
        }
        for obligation in &action.closes {
            if !self.obligations.iter().any(|o| o.id == *obligation) {
                return Err(AcquisitionError::UnopenedObligation {
                    action: action.id.clone(),
                    obligation: obligation.clone(),
                });
            }
        }
        self.actions.push(action);
        Ok(())
    }

    /// Record that the agent chose to stop.
    pub fn stopping(mut self) -> Self {
        self.stopped_after = true;
        self
    }

    /// Obligations still open at the end of the trace.
    pub fn open(&self) -> Vec<&Obligation> {
        let closed: BTreeSet<&str> = self
            .actions
            .iter()
            .flat_map(|a| a.closes.iter().map(String::as_str))
            .collect();
        self.obligations
            .iter()
            .filter(|o| !closed.contains(o.id.as_str()))
            .collect()
    }

    /// Whether every *required* obligation was closed.
    ///
    /// The optional ones deliberately do not count. A trace that closed every required obligation
    /// and stopped is a good trace even if optional evidence remains unread; a trace that read
    /// everything optional and missed a required obligation is not, and one predicate that mixed
    /// them would rank the second above the first.
    pub fn admissible(&self) -> bool {
        self.open().iter().all(|o| !o.required)
    }

    /// Acquisitions that closed nothing that was still open when they ran.
    pub fn redundant(&self) -> Vec<&Action> {
        let mut closed: BTreeSet<&str> = BTreeSet::new();
        let mut out = Vec::new();
        for action in &self.actions {
            let newly: Vec<&str> = action
                .closes
                .iter()
                .map(String::as_str)
                .filter(|id| !closed.contains(id))
                .collect();
            if newly.is_empty() {
                out.push(action);
            }
            for id in newly {
                closed.insert(id);
            }
        }
        out
    }

    /// Acquisitions performed after every required obligation was already closed.
    pub fn unnecessary(&self) -> Vec<&Action> {
        let required: BTreeSet<&str> = self
            .obligations
            .iter()
            .filter(|o| o.required)
            .map(|o| o.id.as_str())
            .collect();
        let mut outstanding = required.clone();
        let mut out = Vec::new();
        for action in &self.actions {
            if outstanding.is_empty() {
                out.push(action);
                continue;
            }
            for id in &action.closes {
                outstanding.remove(id.as_str());
            }
        }
        out
    }

    /// The cost spent before the first acquisition that closed a required obligation.
    ///
    /// 26.05's "cheap evidence delays a decisive source", made concrete: this is what the agent
    /// spent on things that did not move the admissibility question. Returns `None` when no
    /// acquisition ever closed a required obligation, because in that case the phrase has no
    /// referent — the run never reached a decisive source at all.
    pub fn deferred_decisive(&self) -> Option<u64> {
        let required: BTreeSet<&str> = self
            .obligations
            .iter()
            .filter(|o| o.required)
            .map(|o| o.id.as_str())
            .collect();
        let mut spent = 0u64;
        for action in &self.actions {
            if action.closes.iter().any(|id| required.contains(id.as_str())) {
                return Some(spent);
            }
            spent = spent.saturating_add(action.cost);
        }
        None
    }

    /// Total cost of the trace.
    pub fn cost(&self) -> u64 {
        self.actions.iter().map(|a| a.cost).sum()
    }

    /// Cost difference against a named reference policy, or a refusal if none is named.
    ///
    /// Positive means this trace spent more than the reference to reach the same admissibility.
    /// The reference's own admissibility is reported beside the number rather than folded into it:
    /// spending less than a policy that never became admissible is not an achievement.
    pub fn regret_against(
        &self,
        reference: Option<&ReferencePolicy>,
    ) -> Result<Regret, AcquisitionError> {
        let reference = reference.ok_or(AcquisitionError::NoReferencePolicy)?;
        Ok(Regret {
            policy: reference.name.clone(),
            cost_difference: self.cost() as i128 - reference.cost as i128,
            this_admissible: self.admissible(),
            reference_admissible: reference.admissible,
        })
    }

    /// The obligations, in declaration order.
    pub fn obligations(&self) -> &[Obligation] {
        &self.obligations
    }

    /// The actions, in performance order.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Cost by acquisition kind.
    pub fn cost_by_kind(&self) -> BTreeMap<AcquisitionKind, u64> {
        let mut out = BTreeMap::new();
        for action in &self.actions {
            *out.entry(action.kind).or_insert(0) += action.cost;
        }
        out
    }
}

/// A named acquisition policy a trace can be compared against.
///
/// Named, because 26.05 offers five candidate baselines and a regret figure whose baseline is not
/// stated is uninterpretable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferencePolicy {
    pub name: String,
    pub cost: u64,
    pub admissible: bool,
}

impl ReferencePolicy {
    /// Declare a reference policy and what it achieved.
    pub fn new(name: impl Into<String>, cost: u64, admissible: bool) -> Self {
        ReferencePolicy {
            name: name.into(),
            cost,
            admissible,
        }
    }
}

/// The comparison against a named policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Regret {
    pub policy: String,
    pub cost_difference: i128,
    pub this_admissible: bool,
    pub reference_admissible: bool,
}

impl Regret {
    /// Whether this comparison is between two runs that both reached an admissible decision.
    ///
    /// When it is false the cost difference still exists but means something else, and a caller
    /// that reports the number without this flag has compared a finished job to an unfinished one.
    pub fn like_for_like(&self) -> bool {
        self.this_admissible && self.reference_admissible
    }
}
