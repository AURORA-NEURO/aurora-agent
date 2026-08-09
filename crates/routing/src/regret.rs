//! Regret against the oracle, and the fraction of achievable gain actually captured.
//!
//! A win rate is the wrong headline. A router that picks a better architecture than the fixed
//! default on nine tasks out of ten, but only ever converts a rounding error of the available
//! improvement, is a bad router that reports 90%. The quantity that cannot be gamed that way is
//!
//! ```text
//! captured gain fraction = (router mean utility - fixed default mean utility)
//!                        / (oracle  mean utility - fixed default mean utility)
//! ```
//!
//! It is 1.0 when the router matches the retrospective bound, 0.0 when it adds nothing over the
//! default, and negative when routing actively hurt. It is **undefined** — not zero, not one —
//! when the denominator vanishes, because a fixed default that is already optimal on every task
//! leaves nothing for any router to capture and reporting a percentage of nothing is
//! meaningless. [`RegretAccount::captured_gain_fraction`] returns `None` there and the report
//! prints the reason.
//!
//! Both numbers are reported side by side for exactly this reason: the win rate is kept so a
//! reader can see the two disagree.

use crate::architecture::Architecture;
use crate::comparator::Comparator;
use serde::{Deserialize, Serialize};

/// Utility difference below which two comparators are treated as tied.
///
/// Purely a floating-point guard, not a significance threshold. Statistical significance is
/// **not** computed anywhere in this crate; with a panel of a few dozen synthetic tasks it would
/// be a made-up number.
pub const GAIN_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparatorOutcome {
    pub comparator: Comparator,
    pub tasks: usize,
    pub mean_utility: f64,
    /// Mean shortfall against the oracle. Zero for the oracle itself, never negative.
    pub mean_regret: f64,
    pub admissible_rate: f64,
    pub mean_cost_fraction: f64,
}

impl ComparatorOutcome {
    pub fn name(&self) -> &'static str {
        self.comparator.name()
    }

    pub fn fixed_architecture(&self) -> Option<&Architecture> {
        self.comparator.fixed_architecture()
    }
}

/// The four comparators, together, always.
///
/// The struct has four named fields rather than a vector so that a report cannot be constructed
/// with the oracle omitted. Dropping the bound is the failure this type is shaped to prevent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegretAccount {
    pub fixed_default: ComparatorOutcome,
    pub most_expensive_default: ComparatorOutcome,
    pub router: ComparatorOutcome,
    pub oracle: ComparatorOutcome,
}

impl RegretAccount {
    /// How much better than the fixed default the retrospective bound is. The ceiling on routing.
    pub fn achievable_gain(&self) -> f64 {
        self.oracle.mean_utility - self.fixed_default.mean_utility
    }

    /// How much better than the fixed default the router actually was. May be negative.
    pub fn captured_gain(&self) -> f64 {
        self.router.mean_utility - self.fixed_default.mean_utility
    }

    /// Captured gain as a fraction of achievable gain, or `None` when nothing was achievable.
    pub fn captured_gain_fraction(&self) -> Option<f64> {
        let achievable = self.achievable_gain();
        if achievable <= GAIN_EPSILON {
            return None;
        }
        Some(self.captured_gain() / achievable)
    }

    /// How much of the oracle's advantage the router left on the table.
    pub fn residual_regret(&self) -> f64 {
        self.oracle.mean_utility - self.router.mean_utility
    }

    pub fn all(&self) -> [&ComparatorOutcome; 4] {
        [
            &self.fixed_default,
            &self.most_expensive_default,
            &self.router,
            &self.oracle,
        ]
    }
}

/// The plain-language conclusion, computed rather than written.
///
/// Blueprint 43.41's stop rule requires reporting the inconvenient result. Deriving the verdict
/// from the numbers, and printing it unconditionally, is how this crate stops a future edit from
/// quietly describing a loss as "comparable performance".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingVerdict {
    /// Routing was worse than doing nothing. Checked first, so it cannot hide behind any other
    /// finding.
    RouterLosesToFixedDefault,
    /// The fixed default was already optimal on every task; routing had nothing to win.
    NoAchievableGain,
    RouterMatchesFixedDefault,
    RouterBeatsFixedDefault,
}

impl RoutingVerdict {
    pub fn of(account: &RegretAccount) -> Self {
        let captured = account.captured_gain();
        if captured < -GAIN_EPSILON {
            return RoutingVerdict::RouterLosesToFixedDefault;
        }
        if account.achievable_gain() <= GAIN_EPSILON {
            return RoutingVerdict::NoAchievableGain;
        }
        if captured > GAIN_EPSILON {
            RoutingVerdict::RouterBeatsFixedDefault
        } else {
            RoutingVerdict::RouterMatchesFixedDefault
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RoutingVerdict::RouterLosesToFixedDefault => "router_loses_to_fixed_default",
            RoutingVerdict::NoAchievableGain => "no_achievable_gain",
            RoutingVerdict::RouterMatchesFixedDefault => "router_matches_fixed_default",
            RoutingVerdict::RouterBeatsFixedDefault => "router_beats_fixed_default",
        }
    }

    /// True when the honest reading is that routing did not help.
    pub fn is_negative_result(self) -> bool {
        !matches!(self, RoutingVerdict::RouterBeatsFixedDefault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::account;

    #[test]
    fn regret_against_the_oracle_is_never_negative() {
        let account = account(0.50, 0.00, 0.62, 0.80);
        for outcome in account.all() {
            assert!(
                outcome.mean_regret >= -GAIN_EPSILON,
                "{} reported negative regret against the bound",
                outcome.name()
            );
        }
        assert!(account.oracle.mean_regret.abs() < GAIN_EPSILON);
    }

    #[test]
    fn captured_gain_fraction_is_undefined_when_the_default_is_already_optimal() {
        let account = account(0.80, 0.00, 0.80, 0.80);
        assert_eq!(account.captured_gain_fraction(), None);
        assert_eq!(
            RoutingVerdict::of(&account),
            RoutingVerdict::NoAchievableGain
        );
    }

    #[test]
    fn a_router_that_wins_often_but_captures_little_gain_reports_a_small_fraction() {
        let account = account(0.50, 0.00, 0.53, 0.80);
        let fraction = account.captured_gain_fraction().unwrap();
        assert!(
            (fraction - 0.10).abs() < 1e-9,
            "captured fraction was {fraction}"
        );
        assert_eq!(
            RoutingVerdict::of(&account),
            RoutingVerdict::RouterBeatsFixedDefault,
            "beating the default is still true, which is exactly why the fraction is reported \
             next to it"
        );
    }

    #[test]
    fn a_router_that_hurts_is_reported_as_losing_even_when_gain_was_available() {
        let account = account(0.50, 0.00, 0.40, 0.80);
        assert_eq!(
            RoutingVerdict::of(&account),
            RoutingVerdict::RouterLosesToFixedDefault
        );
        assert!(account.captured_gain_fraction().unwrap() < 0.0);
        assert!(RoutingVerdict::of(&account).is_negative_result());
    }

    #[test]
    fn a_router_that_hurts_where_no_gain_was_available_still_reports_the_loss() {
        let account = account(0.80, 0.00, 0.70, 0.80);
        assert_eq!(
            RoutingVerdict::of(&account),
            RoutingVerdict::RouterLosesToFixedDefault,
            "a loss must not be absorbed into the no-achievable-gain verdict"
        );
    }

    #[test]
    fn matching_the_oracle_captures_the_whole_available_gain() {
        let account = account(0.50, 0.00, 0.80, 0.80);
        assert!((account.captured_gain_fraction().unwrap() - 1.0).abs() < 1e-9);
        assert!(account.residual_regret().abs() < GAIN_EPSILON);
    }

    #[test]
    fn the_account_always_carries_all_four_comparators() {
        let account = account(0.50, 0.00, 0.62, 0.80);
        let names: Vec<&'static str> = account.all().iter().map(|o| o.name()).collect();
        assert_eq!(
            names,
            vec![
                "fixed-default",
                "most-expensive-default",
                "evidence-router",
                "oracle-retrospective"
            ]
        );
    }

    #[test]
    fn an_account_round_trips_through_json() {
        let account = account(0.50, 0.00, 0.62, 0.80);
        let text = serde_json::to_string(&account).unwrap();
        assert_eq!(
            serde_json::from_str::<RegretAccount>(&text).unwrap(),
            account
        );
    }
}
