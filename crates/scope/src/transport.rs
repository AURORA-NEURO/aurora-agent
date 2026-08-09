//! Restriction, transport, aggregation and extension.
//!
//! Blueprint 43.05 is emphatic that these are four *distinct* operations and must not collapse
//! into a single "join". Narrowing a section to a sub-scope is always safe; carrying a section
//! across to an unrelated scope is not, and must declare what it lost. Every cross-scope move
//! therefore carries a [`LossLedger`], and an aggregation must say which operator it used
//! rather than leaving the reader to guess whether a value is a sum, a max or an estimate.

use crate::key::ScopeKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MappingKind {
    /// Narrowing along a refinement morphism. Always semantically safe.
    Restriction,
    /// Carrying evidence to a scope that is not a refinement. Requires justification.
    Transport { justification: String },
    /// Summarising many sub-scopes into one. Must name the operator.
    Aggregation { operator: AggregationOperator },
    /// Asserting a section holds on a strictly wider scope. The most dangerous move.
    Extension { warrant: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationOperator {
    Sum,
    Max,
    Min,
    Mean,
    Existential,
    Universal,
    StatisticalEstimate,
    DomainSpecific,
}

/// What a cross-scope move discarded, added, or made conditional.
///
/// An empty ledger on a [`MappingKind::Transport`] is a defect, not a clean result: it claims
/// evidence crossed scopes losslessly, which 43.05 treats as a modelling error to be surfaced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossLedger {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discarded: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncertainty_added: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_conditions: Vec<String>,
}

impl LossLedger {
    pub fn is_empty(&self) -> bool {
        self.discarded.is_empty()
            && self.uncertainty_added.is_empty()
            && self.policy_conditions.is_empty()
    }

    pub fn discarding(mut self, what: impl Into<String>) -> Self {
        self.discarded.push(what.into());
        self
    }

    pub fn adding_uncertainty(mut self, what: impl Into<String>) -> Self {
        self.uncertainty_added.push(what.into());
        self
    }

    pub fn conditioned_on(mut self, policy: impl Into<String>) -> Self {
        self.policy_conditions.push(policy.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeMapping {
    pub from: ScopeKey,
    pub to: ScopeKey,
    pub kind: MappingKind,
    #[serde(default)]
    pub loss: LossLedger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingCheck {
    Sound,
    /// The declared kind understates the move: it was labelled a restriction but `to` is not a
    /// refinement of `from`.
    MisdeclaredRestriction,
    /// A transport or extension declared no loss at all.
    UndeclaredLoss,
}

impl ScopeMapping {
    pub fn check(&self) -> MappingCheck {
        match &self.kind {
            MappingKind::Restriction => {
                if self.to.refines(&self.from) {
                    MappingCheck::Sound
                } else {
                    MappingCheck::MisdeclaredRestriction
                }
            }
            MappingKind::Transport { .. } | MappingKind::Extension { .. } => {
                if self.loss.is_empty() {
                    MappingCheck::UndeclaredLoss
                } else {
                    MappingCheck::Sound
                }
            }
            MappingKind::Aggregation { .. } => MappingCheck::Sound,
        }
    }
}
