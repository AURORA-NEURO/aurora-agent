//! Longitudinal confirmation and prospective blind reveal (31.09).
//!
//! 31.09's failure containment, shared by every module in section 31: "A later event can evaluate but
//! cannot enter an earlier context snapshot." Everything here is one way of making that true by
//! construction rather than by discipline.
//!
//! # The snapshot has no way to grow
//!
//! [`Snapshot`] is built by [`Snapshot::freeze`] and exposes its evidence read-only. There is no
//! `add`, no `extend`, no `&mut` accessor, and it does not implement any trait that would let a
//! caller rebuild it with more. Contaminating a snapshot with later evidence is not forbidden here;
//! it is unrepresentable, which is the standard AGENTS.md sets: "A test that asserts a rule is good.
//! A type that makes the rule unbreakable is better."
//!
//! # The escrow opens only through a predeclared rule
//!
//! 31.09's required functions are "escrow later outcomes and artifacts" and "predeclare reveal
//! rules", in that order. [`Escrow::new`] rejects any [`RevealRule`] declared after the snapshot
//! froze — writing the reveal rule once you have seen the outcome is the thing predeclaration
//! prevents, and it fails here with [`OracleXError::RuleDeclaredAfterFreeze`]. Reading the outcome
//! needs a [`RevealToken`], and the only mint is [`Escrow::fire`].
//!
//! # Forecast and revision are scored apart
//!
//! 31.09: "score forecasts and revisions separately". [`score_forecast`] refuses a forecast whose
//! cited basis is not contained in the snapshot — a right answer from evidence that did not exist yet
//! is not a right answer, it is a leak, and it comes back as a contradiction with the offending
//! citation named. [`revision_gain`] compares a later forecast against the earlier one and is a
//! different number with a different meaning.
//!
//! # Not implemented
//!
//! No calibration, no time-to-resolution, no follow-up completeness estimator. 31.09 lists all three
//! as metrics; each needs a cohort and a survival model. [`Ascertainment`] records whether follow-up
//! was complete, because 31.09 requires it to be *recorded*, and this crate can hold a record without
//! pretending to estimate from it.

use std::collections::BTreeSet;

use bioprism_oracle::{EvidenceTier, UtcTimestamp};
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::verdict::{Determination, Witness};

/// The evidence available at one instant, closed against later addition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    frozen_at: UtcTimestamp,
    evidence: BTreeSet<String>,
}

impl Snapshot {
    pub fn freeze(
        frozen_at: UtcTimestamp,
        evidence: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Snapshot {
            frozen_at,
            evidence: evidence.into_iter().map(Into::into).collect(),
        }
    }

    pub fn frozen_at(&self) -> &UtcTimestamp {
        &self.frozen_at
    }

    /// Read-only. Returning a `&BTreeSet` rather than `&mut` is the enforcement.
    pub fn evidence(&self) -> &BTreeSet<String> {
        &self.evidence
    }

    pub fn contains(&self, item: &str) -> bool {
        self.evidence.contains(item)
    }
}

/// The condition under which escrowed material becomes visible.
///
/// Free-form because the conditions are study-specific — a fixed follow-up duration, an accrual
/// target, a pre-registered interim analysis. What matters mechanically is that the condition existed
/// before the snapshot froze, which [`Escrow::new`] checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RevealRule {
    pub name: String,
    pub declared_at: UtcTimestamp,
    pub condition: String,
}

impl RevealRule {
    pub fn new(
        name: impl Into<String>,
        declared_at: UtcTimestamp,
        condition: impl Into<String>,
    ) -> Self {
        RevealRule {
            name: name.into(),
            declared_at,
            condition: condition.into(),
        }
    }
}

/// Proof that a predeclared rule fired. Cannot be constructed outside this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealToken {
    rule: String,
}

impl RevealToken {
    pub fn rule(&self) -> &str {
        &self.rule
    }
}

/// Whether follow-up was complete when the outcome was escrowed (31.09: "record ascertainment and
/// missing follow-up").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ascertainment {
    Complete,
    /// The subject was followed and the outcome did not occur by the last contact.
    AdministrativelyCensored,
    /// The subject stopped being followed. Not the same as the outcome not occurring.
    LostToFollowUp,
}

/// Later material, sealed until a predeclared rule fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escrow<T> {
    frozen_at: UtcTimestamp,
    rules: Vec<RevealRule>,
    outcome: T,
    pub ascertainment: Ascertainment,
}

impl<T> Escrow<T> {
    /// Rejects any rule written after the snapshot froze.
    pub fn new(
        snapshot: &Snapshot,
        rules: impl IntoIterator<Item = RevealRule>,
        outcome: T,
        ascertainment: Ascertainment,
    ) -> Result<Self, OracleXError> {
        let rules: Vec<RevealRule> = rules.into_iter().collect();
        for rule in &rules {
            if rule.declared_at.as_str() > snapshot.frozen_at().as_str() {
                return Err(OracleXError::RuleDeclaredAfterFreeze {
                    rule: rule.name.clone(),
                    declared_at: rule.declared_at.as_str().to_string(),
                    frozen_at: snapshot.frozen_at().as_str().to_string(),
                });
            }
        }
        Ok(Escrow {
            frozen_at: snapshot.frozen_at().clone(),
            rules,
            outcome,
            ascertainment,
        })
    }

    pub fn frozen_at(&self) -> &UtcTimestamp {
        &self.frozen_at
    }

    pub fn rules(&self) -> &[RevealRule] {
        &self.rules
    }

    /// Mints a token when the named rule exists and the caller declares its condition met.
    ///
    /// `condition_met` is the caller's assertion because the conditions are study-specific and this
    /// crate reads no clock. What it cannot do is invent a rule: a name not among the predeclared
    /// rules fails, which is the half that matters.
    pub fn fire(&self, rule: &str, condition_met: bool) -> Result<RevealToken, OracleXError> {
        let known = self.rules.iter().any(|candidate| candidate.name == rule);
        if !known || !condition_met {
            return Err(OracleXError::EscrowSealed {
                escrow: rule.to_string(),
            });
        }
        Ok(RevealToken {
            rule: rule.to_string(),
        })
    }

    /// The only reader. Takes a token, so no call site can reach the outcome by accident.
    pub fn open(&self, token: &RevealToken) -> Result<&T, OracleXError> {
        if self
            .rules
            .iter()
            .any(|rule| rule.name == token.rule)
        {
            Ok(&self.outcome)
        } else {
            Err(OracleXError::EscrowSealed {
                escrow: token.rule.clone(),
            })
        }
    }
}

/// A prediction, with the evidence it says it used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forecast {
    pub claim: String,
    /// What the forecaster cited. Every item must be in the snapshot.
    pub basis: BTreeSet<String>,
}

impl Forecast {
    pub fn new(claim: impl Into<String>, basis: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Forecast {
            claim: claim.into(),
            basis: basis.into_iter().map(Into::into).collect(),
        }
    }
}

/// Scores a forecast against a revealed outcome, refusing any forecast built on evidence the
/// snapshot did not contain.
///
/// The basis check runs first and unconditionally. A forecast that cited later evidence is
/// contradicted whether or not it was correct: correctness computed from a leak is the failure mode,
/// not a mitigating factor.
pub fn score_forecast(
    snapshot: &Snapshot,
    forecast: &Forecast,
    outcome: &str,
    token: &RevealToken,
) -> Determination {
    let leaked: Vec<&String> = forecast
        .basis
        .iter()
        .filter(|item| !snapshot.contains(item))
        .collect();
    if let Some(item) = leaked.first() {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "forecast basis is contained in the frozen snapshot".to_string(),
                expected: format!("every cited item present at {}", snapshot.frozen_at().as_str()),
                observed: format!("'{item}' was cited and is not in the snapshot"),
            },
        );
    }
    if forecast.basis.is_empty() {
        return Determination::unresolved(
            "a declared forecast basis",
            "the forecast cites nothing, so containment in the snapshot cannot be checked",
        );
    }
    if forecast.claim == outcome {
        Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "forecast matched the outcome revealed under rule '{}', from snapshot evidence only",
                token.rule()
            ),
        )
    } else {
        Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "forecast matches the revealed outcome".to_string(),
                expected: outcome.to_string(),
                observed: forecast.claim.clone(),
            },
        )
    }
}

/// Whether a later forecast moved toward the outcome relative to an earlier one.
///
/// Three-valued and separate from [`score_forecast`], because 31.09 scores forecasts and revisions
/// apart. `None` means both were right or both were wrong: no gain, no loss, and reporting either as
/// a number would invent a comparison.
pub fn revision_gain(earlier: &Forecast, later: &Forecast, outcome: &str) -> Option<bool> {
    let before = earlier.claim == outcome;
    let after = later.claim == outcome;
    if before == after {
        None
    } else {
        Some(after)
    }
}
