//! The factorial design that makes an attribution askable (26.18).
//!
//! `bioprism-evalengine::attribution` already answers the attribution question, and its answer is
//! mostly a refusal: it will not attribute a difference to any component when more than one
//! component varied. That refusal is correct and this module does not weaken it. What it does is
//! supply the missing input — 26.18's step 2, "change one component or predefined factorial set" —
//! by turning a set of arms into the list of contrasts that *are* single-factor, so a caller stops
//! handing the attributor pairs it is bound to refuse.
//!
//! # A design is a lattice, not a list
//!
//! [`FactorialDesign`] holds a declared factor set, and every arm must assign every factor:
//! [`DesignError::UnassignedFactor`] refuses a partially specified arm, because an arm that leaves
//! a factor unstated is not "the same as the baseline there", it is unknown there — the same
//! zero-versus-unknown distinction the rest of this crate turns on. From a complete design,
//! [`FactorialDesign::single_factor_contrasts`] enumerates exactly the pairs that differ in one
//! coordinate, and those are the only pairs that can carry a component claim.
//!
//! # Interaction is a claim about a cell that must exist
//!
//! 26.18's metric list includes "interaction effect". An interaction between factors `a` and `b`
//! is only estimable when all four cells of the `a × b` sub-lattice are present.
//! [`FactorialDesign::estimable_interactions`] returns the pairs for which they are, and — the
//! load-bearing half — [`FactorialDesign::missing_for_interaction`] names the cells a design would
//! need to add. A design that reports an interaction it could not have estimated is a fabricated
//! finding, and the missing-cell list is what stops the question from being asked at all.
//!
//! # Not implemented
//!
//! No effect estimate. [`FactorialDesign::contrast_forks`] emits
//! [`bioprism_evalengine::MatchedFork`] values for [`bioprism_evalengine::attribute`]; the
//! direction, the refusal reasons and the claim language are that crate's, and there is no second
//! copy here. No paired-seed machinery (26.18 step 4) — this crate has no RNG and no runner. No
//! cost or latency delta: 26.18 lists it and `bioprism-metrics` owns cost-conditioned
//! comparability. No deterministic tool fixtures (step 3), which are a runtime concern.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_evalengine::{ArmSpec, Conclusion, MatchedFork, ScoreTier};
use serde::{Deserialize, Serialize};

use crate::error::DesignError;

/// One arm of a factorial design: a complete assignment of every declared factor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arm {
    pub id: String,
    /// Factor name to level. Every declared factor must appear.
    pub levels: BTreeMap<String, String>,
    /// The arm's outcome, as the evidence ladder classified it.
    pub conclusion: Conclusion,
    /// The tier that conclusion rests on. Carried through to the fork so an attribution inherits
    /// the weaker side's grounding rather than the stronger one's.
    pub tier: ScoreTier,
}

impl Arm {
    /// Declare an arm.
    pub fn new(
        id: impl Into<String>,
        levels: BTreeMap<String, String>,
        conclusion: Conclusion,
        tier: ScoreTier,
    ) -> Self {
        Arm {
            id: id.into(),
            levels,
            conclusion,
            tier,
        }
    }
}

/// A pair of arms differing in exactly one factor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contrast {
    pub factor: String,
    pub baseline: String,
    pub variant: String,
    pub from_level: String,
    pub to_level: String,
}

/// A declared factor set and the arms that populate it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactorialDesign {
    /// The decision cell all arms resume from. 26.18 step 1: "freeze world and decision state".
    pub cell_id: String,
    factors: BTreeSet<String>,
    arms: Vec<Arm>,
    baseline: String,
}

impl FactorialDesign {
    /// Declare a design over a factor set, naming the baseline arm.
    ///
    /// The baseline is named at construction rather than inferred. 26.18 estimates "which
    /// architectural component caused improvement", and improvement is relative to something; a
    /// design that picks its baseline after seeing the outcomes has chosen the comparison that
    /// flatters it.
    pub fn declare(
        cell_id: impl Into<String>,
        factors: impl IntoIterator<Item = String>,
        baseline: impl Into<String>,
    ) -> Self {
        FactorialDesign {
            cell_id: cell_id.into(),
            factors: factors.into_iter().collect(),
            arms: Vec::new(),
            baseline: baseline.into(),
        }
    }

    /// Add an arm, refusing an incomplete or duplicated assignment.
    pub fn add(&mut self, arm: Arm) -> Result<(), DesignError> {
        if self.arms.iter().any(|a| a.id == arm.id) {
            return Err(DesignError::DuplicateArm(arm.id));
        }
        for factor in &self.factors {
            if !arm.levels.contains_key(factor) {
                return Err(DesignError::UnassignedFactor {
                    arm: arm.id.clone(),
                    factor: factor.clone(),
                });
            }
        }
        for factor in arm.levels.keys() {
            if !self.factors.contains(factor) {
                return Err(DesignError::UndeclaredFactor {
                    arm: arm.id.clone(),
                    factor: factor.clone(),
                });
            }
        }
        if let Some(other) = self.arms.iter().find(|a| a.levels == arm.levels) {
            return Err(DesignError::DuplicateCell {
                arm: arm.id.clone(),
                other: other.id.clone(),
            });
        }
        self.arms.push(arm);
        Ok(())
    }

    /// Check the design is usable at all.
    pub fn validate(&self) -> Result<(), DesignError> {
        if self.arms.len() < 2 {
            return Err(DesignError::TooFewArms);
        }
        if !self.arms.iter().any(|a| a.id == self.baseline) {
            return Err(DesignError::DuplicateArm(self.baseline.clone()));
        }
        Ok(())
    }

    /// Every ordered pair of arms differing in exactly one factor.
    ///
    /// Ordered from the arm nearer the baseline: a contrast is stated as a change *from* the
    /// baseline's level *to* the variant's wherever the baseline participates, and in arm order
    /// otherwise. Pairs differing in two or more factors are simply absent — that is the whole
    /// service this function performs.
    pub fn single_factor_contrasts(&self) -> Vec<Contrast> {
        let mut out = Vec::new();
        for (i, left) in self.arms.iter().enumerate() {
            for right in self.arms.iter().skip(i + 1) {
                let differing: Vec<&String> = self
                    .factors
                    .iter()
                    .filter(|f| left.levels.get(*f) != right.levels.get(*f))
                    .collect();
                if differing.len() != 1 {
                    continue;
                }
                let factor = differing[0];
                let (base, var) = if right.id == self.baseline {
                    (right, left)
                } else {
                    (left, right)
                };
                out.push(Contrast {
                    factor: factor.clone(),
                    baseline: base.id.clone(),
                    variant: var.id.clone(),
                    from_level: base.levels[factor].clone(),
                    to_level: var.levels[factor].clone(),
                });
            }
        }
        out
    }

    /// Factor pairs whose full two-by-two sub-lattice is present.
    ///
    /// Only these admit an interaction claim. The check is over the levels actually used by the
    /// design's arms, not over all conceivable levels, because a design is entitled to study two
    /// levels of a factor that has ten.
    pub fn estimable_interactions(&self) -> Vec<(String, String)> {
        let factors: Vec<&String> = self.factors.iter().collect();
        let mut out = Vec::new();
        for (i, a) in factors.iter().enumerate() {
            for b in factors.iter().skip(i + 1) {
                if self.missing_for_interaction(a, b).is_empty() {
                    out.push(((*a).clone(), (*b).clone()));
                }
            }
        }
        out
    }

    /// The `(level_a, level_b)` cells a design would need in order to estimate an `a × b`
    /// interaction, and does not have.
    ///
    /// Empty means estimable. A non-empty list is the actionable form of the refusal: it says what
    /// to run, rather than only that the question cannot be answered.
    pub fn missing_for_interaction(&self, a: &str, b: &str) -> Vec<(String, String)> {
        let levels_a: BTreeSet<&String> = self.arms.iter().filter_map(|x| x.levels.get(a)).collect();
        let levels_b: BTreeSet<&String> = self.arms.iter().filter_map(|x| x.levels.get(b)).collect();
        if levels_a.len() < 2 || levels_b.len() < 2 {
            return Vec::new();
        }
        let mut missing = Vec::new();
        for la in &levels_a {
            for lb in &levels_b {
                let present = self
                    .arms
                    .iter()
                    .any(|x| x.levels.get(a) == Some(*la) && x.levels.get(b) == Some(*lb));
                if !present {
                    missing.push(((*la).clone(), (*lb).clone()));
                }
            }
        }
        missing
    }

    /// Turn the single-factor contrasts into forks for [`bioprism_evalengine::attribute`].
    ///
    /// `held_fixed` is populated with every factor except the varying one, and `controlled` is
    /// taken from the caller rather than assumed: replaying from one frozen cell makes a fork
    /// matched, and matched is not the same as randomised.
    pub fn contrast_forks(&self, controlled: bool) -> Result<Vec<MatchedFork>, DesignError> {
        self.validate()?;
        let mut out = Vec::new();
        for contrast in self.single_factor_contrasts() {
            let base = self.arm(&contrast.baseline).expect("contrast names known arms");
            let var = self.arm(&contrast.variant).expect("contrast names known arms");
            let mut fork = MatchedFork::new(
                format!("{}::{}", self.cell_id, contrast.factor),
                &self.cell_id,
                ArmSpec::new(&base.id, base.levels.clone(), base.conclusion, base.tier),
                ArmSpec::new(&var.id, var.levels.clone(), var.conclusion, var.tier),
            );
            fork.held_fixed = self
                .factors
                .iter()
                .filter(|f| **f != contrast.factor)
                .cloned()
                .collect();
            fork.controlled = controlled;
            out.push(fork);
        }
        Ok(out)
    }

    /// An arm by id.
    pub fn arm(&self, id: &str) -> Option<&Arm> {
        self.arms.iter().find(|a| a.id == id)
    }

    /// The declared factors.
    pub fn factors(&self) -> &BTreeSet<String> {
        &self.factors
    }

    /// The arms, in declaration order.
    pub fn arms(&self) -> &[Arm] {
        &self.arms
    }

    /// Arms that differ from the baseline in more than one factor.
    ///
    /// These are the arms whose comparison to the baseline can never carry a component claim.
    /// Reported so a design's authors see the wasted arms before they run them, which is 26.18's
    /// practical point: a factorial design is expensive and an unmatched arm buys nothing.
    pub fn unattributable(&self) -> Vec<&str> {
        let Some(base) = self.arm(&self.baseline) else {
            return Vec::new();
        };
        self.arms
            .iter()
            .filter(|a| a.id != self.baseline)
            .filter(|a| {
                self.factors
                    .iter()
                    .filter(|f| a.levels.get(*f) != base.levels.get(*f))
                    .count()
                    != 1
            })
            .map(|a| a.id.as_str())
            .collect()
    }
}
