//! Causal component attribution over matched forks.
//!
//! Blueprint 00.01 claims that a matched fork isolates which component explains a difference, and
//! 07.07 spells out the conditions: the same cell revision, state, visible evidence, budget policy
//! and environment seed across variants, with the differences enumerated from the architecture IR.
//! `bioprism-prism`'s `matched_fork` builds such forks over context policies; this module scores
//! them, and its main job is knowing when it must not.
//!
//! # The refusal is the feature
//!
//! A fork that changed the planner *and* the model, then observed an improvement, supports no
//! statement about either. Every honest thing that can be said is "something in {planner, model}
//! did it", which is not attribution — it is the same end-to-end comparison the fork was built to
//! replace. So [`attribute`] returns [`Attribution::Refused`] whenever more than one component
//! varied, and the refusal names the varied set so the caller can go and run the two forks that
//! would have answered the question.
//!
//! Three more refusals fall out of the same discipline:
//!
//! - **nothing varied.** Two arms with identical settings that reached different conclusions have
//!   demonstrated run-to-run instability, not a component effect. Reporting an effect here would
//!   attribute noise to whichever component was named last.
//! - **a held-fixed component moved.** The declared control was violated, so the match is broken
//!   whatever the varied count says. This is checked against the declaration rather than inferred,
//!   because a component nobody declared is a component nobody controlled.
//! - **the arms describe different component sets.** If one arm declares a `verifier` and the
//!   other does not mention one, "varied" is undefined: the second arm may have had no verifier or
//!   may simply not have said. Absence of a declaration is not a value.
//!
//! # Causal versus descriptive
//!
//! 07.07's causal caution — "the claim is causal only for randomized or otherwise controlled
//! interventions under the evaluated distribution" — is carried as [`AttributionClaim`]. A fork
//! that does not declare itself controlled still yields an attribution, but a descriptive one, and
//! the label travels with the report rather than living in a footnote.
//!
//! # Not implemented here
//!
//! Interaction effects and factorial designs (07.07 "support factorial and sequential designs"),
//! hidden-confounder detection, and sign-reversal search across cells. Those need many forks and a
//! design description this crate does not model; [`ComponentEffect`] aggregates single-factor forks
//! and stops there, and pooling forks that varied *different* components is refused rather than
//! averaged.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::ladder::ScoreTier;
use crate::score::Conclusion;

/// One side of a fork: the component settings that were in force, and what it concluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmSpec {
    pub arm: String,
    /// Declared architecture factors: `model`, `planner`, `context_selector`, `memory`,
    /// `tool_router`, `verifier`, `branch_count`, `budget`, `provider`, and so on. A component
    /// that is not a key here was not declared, and is treated as unknown rather than absent.
    pub components: BTreeMap<String, String>,
    pub conclusion: Conclusion,
    /// The tier the arm's conclusion rests on. An attribution is only as grounded as its weaker
    /// side.
    pub tier: ScoreTier,
}

impl ArmSpec {
    pub fn new(
        arm: impl Into<String>,
        components: BTreeMap<String, String>,
        conclusion: Conclusion,
        tier: ScoreTier,
    ) -> Self {
        ArmSpec {
            arm: arm.into(),
            components,
            conclusion,
            tier,
        }
    }
}

/// Two arms resumed from the same frozen state, plus the control declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedFork {
    pub fork_id: String,
    /// The decision cell both arms resumed from. Two arms with different cells are not a fork,
    /// and the caller is expected to have refused earlier.
    pub cell_id: String,
    pub baseline: ArmSpec,
    pub variant: ArmSpec,
    /// Components the fork *declared* it was holding fixed. Checked, not trusted.
    #[serde(default)]
    pub held_fixed: BTreeSet<String>,
    /// Whether the intervention was randomized or otherwise controlled under the evaluated
    /// distribution. Only a controlled fork yields a causal claim.
    #[serde(default)]
    pub controlled: bool,
}

impl MatchedFork {
    pub fn new(
        fork_id: impl Into<String>,
        cell_id: impl Into<String>,
        baseline: ArmSpec,
        variant: ArmSpec,
    ) -> Self {
        MatchedFork {
            fork_id: fork_id.into(),
            cell_id: cell_id.into(),
            baseline,
            variant,
            held_fixed: BTreeSet::new(),
            controlled: false,
        }
    }

    pub fn holding_fixed<I, S>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.held_fixed
            .extend(components.into_iter().map(Into::into));
        self
    }

    pub fn controlled(mut self) -> Self {
        self.controlled = true;
        self
    }

    /// Components declared by one arm and not the other.
    pub fn undeclared_on_one_side(&self) -> BTreeSet<String> {
        let left: BTreeSet<&String> = self.baseline.components.keys().collect();
        let right: BTreeSet<&String> = self.variant.components.keys().collect();
        left.symmetric_difference(&right)
            .map(|key| (*key).clone())
            .collect()
    }

    /// Components whose settings differ between the arms.
    pub fn varied(&self) -> BTreeSet<String> {
        self.baseline
            .components
            .iter()
            .filter(|(key, value)| {
                self.variant
                    .components
                    .get(*key)
                    .is_some_and(|other| other != *value)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }
}

/// Which direction the varied component moved the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectDirection {
    Improved,
    Regressed,
    /// Both arms reached the same conclusion. A real, reportable null.
    Unchanged,
    /// At least one arm was unknown, disputed or vetoed, so the pair cannot be ordered. Distinct
    /// from `Unchanged`: nobody has shown the component does nothing.
    Indeterminate,
}

impl EffectDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            EffectDirection::Improved => "improved",
            EffectDirection::Regressed => "regressed",
            EffectDirection::Unchanged => "unchanged",
            EffectDirection::Indeterminate => "indeterminate",
        }
    }
}

/// The strength of what may be said, per 07.07's causal caution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionClaim {
    /// Controlled intervention from a matched state: the component caused the difference under the
    /// evaluated distribution.
    Causal,
    /// Matched but not controlled: the difference is associated with the component.
    Descriptive,
}

/// Why an attribution was not made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum RefusalReason {
    /// The headline case. More than one component changed, so no single component is implicated.
    MultipleComponentsVaried { varied: Vec<String> },
    /// The arms were identical. Any observed difference is instability, not an effect.
    NothingVaried { conclusions_differed: bool },
    /// A component the fork promised to hold fixed did not stay fixed.
    HeldFixedViolated { components: Vec<String> },
    /// The arms declare different component sets, so "varied" has no meaning.
    ComponentSetsDiffer { only_on_one_side: Vec<String> },
    /// Neither arm declared any component at all.
    NoComponentsDeclared,
    /// A public fork reached an internally inconsistent state while being attributed.
    InvariantViolation { detail: String },
}

impl RefusalReason {
    /// One sentence a human can act on, in the same spirit as `prism`'s fork attribution line.
    pub fn explain(&self) -> String {
        match self {
            RefusalReason::MultipleComponentsVaried { varied } => format!(
                "{} components varied at once ({}); the fork supports no attribution to any of \
                 them — run one fork per component",
                varied.len(),
                varied.join(", ")
            ),
            RefusalReason::NothingVaried {
                conclusions_differed,
            } => {
                if *conclusions_differed {
                    "no component varied, yet the arms concluded differently; this is run-to-run \
                     instability, not a component effect"
                        .to_string()
                } else {
                    "no component varied; there is nothing to attribute".to_string()
                }
            }
            RefusalReason::HeldFixedViolated { components } => format!(
                "components declared held-fixed did not stay fixed ({}); the match is broken",
                components.join(", ")
            ),
            RefusalReason::ComponentSetsDiffer { only_on_one_side } => format!(
                "the arms declare different component sets ({} appears on one side only); an \
                 undeclared component is unknown, not absent",
                only_on_one_side.join(", ")
            ),
            RefusalReason::NoComponentsDeclared => {
                "neither arm declared any architecture component; a fork with no declared factors \
                 is an end-to-end comparison"
                    .to_string()
            }
            RefusalReason::InvariantViolation { detail } => {
                format!("the matched fork is internally inconsistent: {detail}")
            }
        }
    }
}

/// The outcome of attributing one fork.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "attribution", rename_all = "snake_case")]
pub enum Attribution {
    Attributed {
        component: String,
        from: String,
        to: String,
        direction: EffectDirection,
        claim: AttributionClaim,
        /// The weaker of the two arms' tiers. An attribution built on two judge readings is a
        /// judge-tier attribution however clean the fork was.
        supporting_tier: ScoreTier,
    },
    Refused {
        reason: RefusalReason,
    },
}

impl Attribution {
    pub fn is_refused(&self) -> bool {
        matches!(self, Attribution::Refused { .. })
    }

    pub fn component(&self) -> Option<&str> {
        match self {
            Attribution::Attributed { component, .. } => Some(component.as_str()),
            Attribution::Refused { .. } => None,
        }
    }

    pub fn direction(&self) -> Option<EffectDirection> {
        match self {
            Attribution::Attributed { direction, .. } => Some(*direction),
            Attribution::Refused { .. } => None,
        }
    }

    /// Whether this attribution may be quoted as causal.
    pub fn is_causal(&self) -> bool {
        matches!(
            self,
            Attribution::Attributed {
                claim: AttributionClaim::Causal,
                ..
            }
        )
    }

    pub fn explain(&self) -> String {
        match self {
            Attribution::Attributed {
                component,
                from,
                to,
                direction,
                claim,
                supporting_tier,
            } => format!(
                "{} {} the result when changed from `{}` to `{}` ({} claim, {} evidence)",
                component,
                direction.as_str(),
                from,
                to,
                match claim {
                    AttributionClaim::Causal => "causal",
                    AttributionClaim::Descriptive => "descriptive",
                },
                supporting_tier
            ),
            Attribution::Refused { reason } => reason.explain(),
        }
    }
}

/// Attribute one matched fork, refusing whenever the fork does not license a single-component
/// statement.
///
/// The checks run in a fixed order and the first that fires wins. Component-set mismatch comes
/// before the varied count because a mismatched declaration makes the count meaningless, and
/// held-fixed violation comes before it for the same reason: both are failures of the match, and
/// reporting "two components varied" for a fork whose control was broken would understate the
/// problem.
pub fn attribute(fork: &MatchedFork) -> Attribution {
    if fork.baseline.components.is_empty() && fork.variant.components.is_empty() {
        return Attribution::Refused {
            reason: RefusalReason::NoComponentsDeclared,
        };
    }

    let mismatched = fork.undeclared_on_one_side();
    if !mismatched.is_empty() {
        return Attribution::Refused {
            reason: RefusalReason::ComponentSetsDiffer {
                only_on_one_side: mismatched.into_iter().collect(),
            },
        };
    }

    let varied = fork.varied();

    let broken: Vec<String> = fork
        .held_fixed
        .iter()
        .filter(|component| varied.contains(*component))
        .cloned()
        .collect();
    if !broken.is_empty() {
        return Attribution::Refused {
            reason: RefusalReason::HeldFixedViolated { components: broken },
        };
    }

    match varied.len() {
        0 => Attribution::Refused {
            reason: RefusalReason::NothingVaried {
                conclusions_differed: fork.baseline.conclusion != fork.variant.conclusion,
            },
        },
        1 => {
            let Some(component) = varied.into_iter().next() else {
                return Attribution::Refused {
                    reason: RefusalReason::InvariantViolation {
                        detail: "varied component count was one but no component was available"
                            .to_string(),
                    },
                };
            };
            let Some(from) = fork.baseline.components.get(&component).cloned() else {
                return Attribution::Refused {
                    reason: RefusalReason::InvariantViolation {
                        detail: format!("varied component `{component}` is absent from baseline"),
                    },
                };
            };
            let Some(to) = fork.variant.components.get(&component).cloned() else {
                return Attribution::Refused {
                    reason: RefusalReason::InvariantViolation {
                        detail: format!("varied component `{component}` is absent from variant"),
                    },
                };
            };
            Attribution::Attributed {
                component,
                from,
                to,
                direction: direction_of(fork.baseline.conclusion, fork.variant.conclusion),
                claim: if fork.controlled {
                    AttributionClaim::Causal
                } else {
                    AttributionClaim::Descriptive
                },
                supporting_tier: fork.baseline.tier.min(fork.variant.tier),
            }
        }
        _ => Attribution::Refused {
            reason: RefusalReason::MultipleComponentsVaried {
                varied: varied.into_iter().collect(),
            },
        },
    }
}

/// Order two conclusions, refusing to order anything uninformative.
///
/// The ordering used here is the reporting convention of [`crate::ladder`]: a supported pass beats
/// an unsupported one, which beats a contradicted one. `Vetoed`, `Unknown`, `Disputed`,
/// `Abstained` and `JustificationUnexamined` are never ordered — a fork whose variant was vetoed
/// has not shown the component to be worse at the task, it has shown a different thing entirely.
fn direction_of(baseline: Conclusion, variant: Conclusion) -> EffectDirection {
    fn rank(conclusion: Conclusion) -> Option<u8> {
        match conclusion {
            Conclusion::Pass => Some(4),
            Conclusion::UnsupportedPass => Some(3),
            Conclusion::PartialCredit => Some(2),
            Conclusion::ContradictedPass => Some(1),
            Conclusion::Fail => Some(0),
            Conclusion::Vetoed
            | Conclusion::Unknown
            | Conclusion::Disputed
            | Conclusion::Abstained
            | Conclusion::JustificationUnexamined => None,
        }
    }
    match (rank(baseline), rank(variant)) {
        (Some(before), Some(after)) if after > before => EffectDirection::Improved,
        (Some(before), Some(after)) if after < before => EffectDirection::Regressed,
        (Some(_), Some(_)) => EffectDirection::Unchanged,
        _ => EffectDirection::Indeterminate,
    }
}

/// Forks that all varied the same component, tallied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentEffect {
    pub component: String,
    pub improved: usize,
    pub regressed: usize,
    pub unchanged: usize,
    pub indeterminate: usize,
    /// Forks contributing, so a reader can see the base this rests on.
    pub forks: Vec<String>,
    /// True when every contributing fork was controlled. One uncontrolled fork downgrades the
    /// whole tally, because pooling a controlled and an observational comparison produces neither.
    pub all_controlled: bool,
    /// The weakest evidence any contributing fork rested on.
    pub weakest_tier: ScoreTier,
}

impl ComponentEffect {
    pub fn observations(&self) -> usize {
        self.improved + self.regressed + self.unchanged + self.indeterminate
    }

    /// Forks that could be ordered at all.
    pub fn decisive(&self) -> usize {
        self.improved + self.regressed
    }

    pub fn claim(&self) -> AttributionClaim {
        if self.all_controlled {
            AttributionClaim::Causal
        } else {
            AttributionClaim::Descriptive
        }
    }

    /// Whether the tally points one way without contradiction. A component that improved four
    /// cells and regressed one has a sign reversal, and 07.07 asks for those to be surfaced rather
    /// than netted out.
    pub fn is_consistent(&self) -> bool {
        self.decisive() > 0 && (self.improved == 0 || self.regressed == 0)
    }
}

/// Attribution over a set of forks: the per-fork verdicts, the refusals, and per-component tallies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributionReport {
    pub forks: Vec<(String, Attribution)>,
    pub effects: Vec<ComponentEffect>,
}

impl AttributionReport {
    /// Attribute each fork independently, then tally by component.
    ///
    /// Refused forks contribute to no tally. That is deliberate: a fork that varied two components
    /// cannot even be counted as weak evidence for either, because a tally that includes it would
    /// be exactly the "something in this set did it" statement the refusal exists to prevent.
    pub fn build(forks: &[MatchedFork]) -> Self {
        let mut per_fork = Vec::new();
        let mut by_component: BTreeMap<String, ComponentEffect> = BTreeMap::new();

        for fork in forks {
            let attribution = attribute(fork);
            if let Attribution::Attributed {
                component,
                direction,
                claim,
                supporting_tier,
                ..
            } = &attribution
            {
                let entry =
                    by_component
                        .entry(component.clone())
                        .or_insert_with(|| ComponentEffect {
                            component: component.clone(),
                            improved: 0,
                            regressed: 0,
                            unchanged: 0,
                            indeterminate: 0,
                            forks: Vec::new(),
                            all_controlled: true,
                            weakest_tier: ScoreTier::Deterministic,
                        });
                match direction {
                    EffectDirection::Improved => entry.improved += 1,
                    EffectDirection::Regressed => entry.regressed += 1,
                    EffectDirection::Unchanged => entry.unchanged += 1,
                    EffectDirection::Indeterminate => entry.indeterminate += 1,
                }
                entry.forks.push(fork.fork_id.clone());
                entry.all_controlled &= matches!(claim, AttributionClaim::Causal);
                entry.weakest_tier = entry.weakest_tier.min(*supporting_tier);
            }
            per_fork.push((fork.fork_id.clone(), attribution));
        }

        AttributionReport {
            forks: per_fork,
            effects: by_component.into_values().collect(),
        }
    }

    pub fn refusals(&self) -> impl Iterator<Item = (&str, &RefusalReason)> {
        self.forks
            .iter()
            .filter_map(|(id, attribution)| match attribution {
                Attribution::Refused { reason } => Some((id.as_str(), reason)),
                Attribution::Attributed { .. } => None,
            })
    }

    /// Components whose forks disagreed in sign.
    pub fn sign_reversals(&self) -> impl Iterator<Item = &ComponentEffect> {
        self.effects
            .iter()
            .filter(|effect| effect.decisive() > 0 && !effect.is_consistent())
    }

    /// A compact table for a human reading a CI failure, in the shape `prism::fork` uses.
    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut text = String::new();
        let _ = writeln!(text, "| Fork | Attribution |");
        let _ = writeln!(text, "|---|---|");
        for (id, attribution) in &self.forks {
            let _ = writeln!(text, "| `{}` | {} |", id, attribution.explain());
        }
        if !self.effects.is_empty() {
            let _ = writeln!(
                text,
                "\n| Component | Improved | Regressed | Unchanged | Indeterminate | Claim | Evidence |"
            );
            let _ = writeln!(text, "|---|---:|---:|---:|---:|---|---|");
            for effect in &self.effects {
                let _ = writeln!(
                    text,
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    effect.component,
                    effect.improved,
                    effect.regressed,
                    effect.unchanged,
                    effect.indeterminate,
                    match effect.claim() {
                        AttributionClaim::Causal => "causal",
                        AttributionClaim::Descriptive => "descriptive",
                    },
                    effect.weakest_tier
                );
            }
        }
        for effect in self.sign_reversals() {
            let _ = writeln!(
                text,
                "\n- `{}` reversed sign across cells ({} improved, {} regressed). A net effect \
                 would hide this.",
                effect.component, effect.improved, effect.regressed
            );
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn fork(
        id: &str,
        baseline: &[(&str, &str)],
        variant: &[(&str, &str)],
        before: Conclusion,
        after: Conclusion,
    ) -> MatchedFork {
        MatchedFork::new(
            id,
            "cell-1",
            ArmSpec::new(
                "baseline",
                components(baseline),
                before,
                ScoreTier::Deterministic,
            ),
            ArmSpec::new(
                "variant",
                components(variant),
                after,
                ScoreTier::Deterministic,
            ),
        )
    }

    #[test]
    fn a_fork_that_varied_two_components_supports_no_attribution() {
        let fork = fork(
            "f1",
            &[("model", "a"), ("planner", "p1")],
            &[("model", "b"), ("planner", "p2")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        let attribution = attribute(&fork);
        assert!(attribution.is_refused());
        assert_eq!(attribution.component(), None);
        match attribution {
            Attribution::Refused {
                reason: RefusalReason::MultipleComponentsVaried { varied },
            } => assert_eq!(varied, vec!["model".to_string(), "planner".to_string()]),
            other => panic!("expected a multi-component refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_single_varied_component_is_attributed_with_its_direction() {
        let fork = fork(
            "f1",
            &[("model", "a"), ("planner", "p1")],
            &[("model", "a"), ("planner", "p2")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        let attribution = attribute(&fork);
        assert_eq!(attribution.component(), Some("planner"));
        assert_eq!(attribution.direction(), Some(EffectDirection::Improved));
    }

    #[test]
    fn an_uncontrolled_fork_yields_a_descriptive_claim_not_a_causal_one() {
        let fork = fork(
            "f1",
            &[("model", "a")],
            &[("model", "b")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        assert!(!attribute(&fork).is_causal());
        assert!(attribute(&fork.clone().controlled()).is_causal());
    }

    #[test]
    fn identical_arms_that_disagree_are_instability_rather_than_a_component_effect() {
        let fork = fork(
            "f1",
            &[("model", "a")],
            &[("model", "a")],
            Conclusion::Pass,
            Conclusion::Fail,
        );
        match attribute(&fork) {
            Attribution::Refused {
                reason:
                    RefusalReason::NothingVaried {
                        conclusions_differed,
                    },
            } => assert!(conclusions_differed),
            other => panic!("expected a nothing-varied refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_component_declared_held_fixed_that_moved_breaks_the_match() {
        let fork = fork(
            "f1",
            &[("model", "a"), ("planner", "p1")],
            &[("model", "a"), ("planner", "p2")],
            Conclusion::Fail,
            Conclusion::Pass,
        )
        .holding_fixed(["planner"]);
        match attribute(&fork) {
            Attribution::Refused {
                reason: RefusalReason::HeldFixedViolated { components },
            } => assert_eq!(components, vec!["planner".to_string()]),
            other => panic!("expected a held-fixed refusal, got {other:?}"),
        }
    }

    #[test]
    fn arms_declaring_different_component_sets_cannot_be_compared() {
        let fork = fork(
            "f1",
            &[("model", "a"), ("verifier", "v1")],
            &[("model", "b")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        match attribute(&fork) {
            Attribution::Refused {
                reason: RefusalReason::ComponentSetsDiffer { only_on_one_side },
            } => assert_eq!(only_on_one_side, vec!["verifier".to_string()]),
            other => panic!("expected a component-set refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_fork_with_no_declared_components_is_an_end_to_end_comparison() {
        let fork = fork("f1", &[], &[], Conclusion::Fail, Conclusion::Pass);
        assert!(matches!(
            attribute(&fork),
            Attribution::Refused {
                reason: RefusalReason::NoComponentsDeclared
            }
        ));
    }

    #[test]
    fn an_unknown_arm_makes_the_direction_indeterminate_not_unchanged() {
        let fork = fork(
            "f1",
            &[("model", "a")],
            &[("model", "b")],
            Conclusion::Pass,
            Conclusion::Unknown,
        );
        assert_eq!(
            attribute(&fork).direction(),
            Some(EffectDirection::Indeterminate)
        );
    }

    #[test]
    fn a_vetoed_variant_is_not_reported_as_a_regression_in_capability() {
        let fork = fork(
            "f1",
            &[("model", "a")],
            &[("model", "b")],
            Conclusion::Pass,
            Conclusion::Vetoed,
        );
        assert_eq!(
            attribute(&fork).direction(),
            Some(EffectDirection::Indeterminate)
        );
    }

    #[test]
    fn the_supporting_tier_is_the_weaker_of_the_two_arms() {
        let mut fork = fork(
            "f1",
            &[("model", "a")],
            &[("model", "b")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        fork.variant.tier = ScoreTier::Judge;
        match attribute(&fork) {
            Attribution::Attributed {
                supporting_tier, ..
            } => assert_eq!(supporting_tier, ScoreTier::Judge),
            other => panic!("expected an attribution, got {other:?}"),
        }
    }

    #[test]
    fn a_refused_fork_contributes_to_no_component_tally() {
        let report = AttributionReport::build(&[
            fork(
                "clean",
                &[("model", "a"), ("planner", "p1")],
                &[("model", "a"), ("planner", "p2")],
                Conclusion::Fail,
                Conclusion::Pass,
            ),
            fork(
                "muddled",
                &[("model", "a"), ("planner", "p1")],
                &[("model", "b"), ("planner", "p2")],
                Conclusion::Fail,
                Conclusion::Pass,
            ),
        ]);
        assert_eq!(report.effects.len(), 1);
        assert_eq!(report.effects[0].component, "planner");
        assert_eq!(report.effects[0].observations(), 1);
        assert_eq!(report.refusals().count(), 1);
    }

    #[test]
    fn one_uncontrolled_fork_downgrades_a_pooled_component_claim_to_descriptive() {
        let controlled = fork(
            "f1",
            &[("planner", "p1")],
            &[("planner", "p2")],
            Conclusion::Fail,
            Conclusion::Pass,
        )
        .controlled();
        let observational = fork(
            "f2",
            &[("planner", "p1")],
            &[("planner", "p2")],
            Conclusion::Fail,
            Conclusion::Pass,
        );
        let report = AttributionReport::build(&[controlled, observational]);
        assert_eq!(report.effects[0].claim(), AttributionClaim::Descriptive);
    }

    #[test]
    fn a_component_that_helps_one_cell_and_hurts_another_is_flagged_as_a_sign_reversal() {
        let report = AttributionReport::build(&[
            fork(
                "f1",
                &[("planner", "p1")],
                &[("planner", "p2")],
                Conclusion::Fail,
                Conclusion::Pass,
            ),
            fork(
                "f2",
                &[("planner", "p1")],
                &[("planner", "p2")],
                Conclusion::Pass,
                Conclusion::Fail,
            ),
        ]);
        assert_eq!(report.sign_reversals().count(), 1);
        assert!(!report.effects[0].is_consistent());
    }

    #[test]
    fn an_attribution_round_trips_through_json() {
        let attribution = attribute(&fork(
            "f1",
            &[("model", "a")],
            &[("model", "b")],
            Conclusion::Fail,
            Conclusion::Pass,
        ));
        let text = serde_json::to_string(&attribution).expect("serialize");
        let back: Attribution = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(attribution, back);
    }
}
