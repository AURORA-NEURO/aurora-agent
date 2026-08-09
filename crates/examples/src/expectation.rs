//! What a slice asserts about its own outcome.
//!
//! This is the half that turns a reference example into evidence. Blueprint 38.01's
//! vertical-slice acceptance list ends with "the limitations card states what the world cannot
//! establish", which is only meaningful if the things it *does* establish are pinned. An example
//! whose expected outcome lives in a README drifts silently; an example whose expected outcome is
//! a field on the example fails a test the moment it stops holding.
//!
//! Every field is optional and `None` means *unasserted*, never *any value is fine*. The
//! distinction matters because [`crate::SliceReport`] records the observed value regardless, so a
//! contributor tightening a slice can read what the value already was rather than guess. Fields
//! left unasserted are visible in the report and in the coverage output; they are a known gap,
//! not a hidden one.

use crate::report::{
    CompiledObservation, GraphWalkObservation, Observations, RefusalCode, RefusalObservation,
};
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// The expected end state of a compile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expectation {
    /// The compiler must succeed, and every asserted field must match.
    Compiles(Box<Compiled>),
    /// The compiler must refuse, with this typed reason. 43.25 forbids the alternative — a
    /// smaller context returned as if it were the answer.
    Refuses(Refusal),
}

impl Expectation {
    pub fn compiles(expected: Compiled) -> Self {
        Expectation::Compiles(Box::new(expected))
    }

    pub fn refuses(expected: Refusal) -> Self {
        Expectation::Refuses(expected)
    }

    /// Checks the expectation against what a run observed, returning one line per mismatch.
    pub fn check(&self, observations: &Observations) -> Vec<String> {
        let mut failures = Vec::new();
        match self {
            Expectation::Compiles(expected) => match &observations.compiled {
                Some(observed) => expected.check(observed, &mut failures),
                None => failures.push(format!(
                    "expected the slice to compile, but it refused: {}",
                    observations
                        .refused
                        .as_ref()
                        .map(|r| r.message.clone())
                        .unwrap_or_else(|| "no refusal recorded".into())
                )),
            },
            Expectation::Refuses(expected) => match &observations.refused {
                Some(observed) => expected.check(observed, &mut failures),
                None => failures.push(
                    "expected the slice to refuse, but it compiled and delivered a section".into(),
                ),
            },
        }
        failures
    }
}

/// Assertions over a successful compile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Compiled {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<OracleStatus>,
    /// Witness kinds in the order the oracle emits them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub witness_kinds: Option<Vec<String>>,
    /// The exact compiled selection. Asserting the set, not the count, is what makes a selection
    /// claim falsifiable — two strategies can agree on eleven and disagree on which eleven.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_facts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_fact_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_closure: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_closure_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_closure_satisfied: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped_protected: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmatched_protected_tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_obligation_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refinement_frontier_actions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omission_influence_classes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omitted_fact_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_sufficiency_claim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_verifies: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Deferred passes the slice expects to see named. A subset check: the compiler may defer
    /// more than a slice knows about, but never fewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_passes_include: Option<Vec<String>>,
}

macro_rules! builder {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(
            pub fn $name(mut self, value: $ty) -> Self {
                self.$name = Some(value);
                self
            }
        )*
    };
}

impl Compiled {
    pub fn new() -> Self {
        Self::default()
    }

    builder! {
        status: OracleStatus,
        selected_fact_count: usize,
        protected_closure_size: usize,
        protected_closure_satisfied: bool,
        unresolved_obligation_count: usize,
        omitted_fact_count: usize,
        supports_sufficiency_claim: bool,
        certificate_verifies: bool,
        backend: String,
    }

    pub fn witness_kinds<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.witness_kinds = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn selected_facts<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selected_facts = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn protected_closure<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.protected_closure = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn dropped_protected<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dropped_protected = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn unmatched_protected_tags<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.unmatched_protected_tags = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn refinement_frontier_actions<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.refinement_frontier_actions = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn omission_influence_classes<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.omission_influence_classes = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn deferred_passes_include<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.deferred_passes_include = Some(values.into_iter().map(Into::into).collect());
        self
    }

    fn check(&self, observed: &CompiledObservation, failures: &mut Vec<String>) {
        compare(
            "oracle status",
            self.status,
            Some(observed.status),
            failures,
        );
        compare(
            "witness kinds",
            self.witness_kinds.as_ref(),
            Some(&observed.witness_kinds),
            failures,
        );
        compare(
            "selected facts",
            self.selected_facts.as_ref(),
            Some(&observed.selected_facts),
            failures,
        );
        compare(
            "selected fact count",
            self.selected_fact_count,
            Some(observed.selected_facts.len()),
            failures,
        );
        compare(
            "protected closure",
            self.protected_closure.as_ref(),
            Some(&observed.protected_closure),
            failures,
        );
        compare(
            "protected closure size",
            self.protected_closure_size,
            Some(observed.protected_closure.len()),
            failures,
        );
        compare(
            "protected closure satisfied",
            self.protected_closure_satisfied,
            Some(observed.protected_closure_satisfied),
            failures,
        );
        compare(
            "dropped protected facts",
            self.dropped_protected.as_ref(),
            Some(&observed.dropped_protected),
            failures,
        );
        compare(
            "unmatched protected tags",
            self.unmatched_protected_tags.as_ref(),
            Some(&observed.unmatched_protected_tags),
            failures,
        );
        compare(
            "unresolved obligation count",
            self.unresolved_obligation_count,
            Some(observed.unresolved_obligations.len()),
            failures,
        );
        compare(
            "refinement frontier actions",
            self.refinement_frontier_actions.as_ref(),
            Some(&observed.refinement_frontier_actions),
            failures,
        );
        compare(
            "omission influence classes",
            self.omission_influence_classes.as_ref(),
            Some(&observed.omission_influence_classes),
            failures,
        );
        compare(
            "omitted fact count",
            self.omitted_fact_count,
            Some(observed.omitted_fact_count),
            failures,
        );
        compare(
            "supports sufficiency claim",
            self.supports_sufficiency_claim,
            Some(observed.supports_sufficiency_claim),
            failures,
        );
        compare(
            "certificate verifies",
            self.certificate_verifies,
            Some(observed.certificate_verifies),
            failures,
        );
        compare(
            "backend",
            self.backend.as_ref(),
            Some(&observed.backend),
            failures,
        );

        if let Some(expected) = &self.deferred_passes_include {
            for pass in expected {
                if !observed.deferred_passes.iter().any(|d| &d.pass == pass) {
                    let declared: Vec<&str> = observed
                        .deferred_passes
                        .iter()
                        .map(|d| d.pass.as_str())
                        .collect();
                    failures.push(format!(
                        "expected deferred pass {pass:?} to be declared, but the compiler declared {declared:?}"
                    ));
                }
            }
        }
    }
}

/// Assertions over a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: RefusalCode,
    /// How many facts the compiler had already selected when it refused. Asserting this is what
    /// separates "refused because the closure was too large" from "refused for some other reason
    /// that happened to produce the same error name".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_facts: Option<usize>,
}

impl Refusal {
    pub fn new(code: RefusalCode) -> Self {
        Refusal {
            code,
            selected: None,
            max_facts: None,
        }
    }

    pub fn selected(mut self, value: usize) -> Self {
        self.selected = Some(value);
        self
    }

    pub fn max_facts(mut self, value: usize) -> Self {
        self.max_facts = Some(value);
        self
    }

    fn check(&self, observed: &RefusalObservation, failures: &mut Vec<String>) {
        compare(
            "refusal code",
            Some(self.code),
            Some(observed.code),
            failures,
        );
        compare(
            "refusal selected",
            self.selected,
            observed.selected,
            failures,
        );
        compare(
            "refusal max_facts",
            self.max_facts,
            observed.max_facts,
            failures,
        );
    }
}

/// A neighbourhood-walk sweep run alongside the compile.
///
/// Blueprint 43.38 requires equal-engineering comparison: the walk sees the same world and the
/// same query as the compiler, with no filtering and no smaller budget. What the sweep asserts is
/// a *structural* claim about the world — that a sound, closed, compact depth exists, or that
/// none does — so it belongs to the slice rather than to a benchmark script that can be rerun
/// with friendlier settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphWalkProbe {
    pub max_depth: usize,
    /// Depths that are simultaneously sound, fully closed and smaller than half the world.
    /// An empty expectation is the strong claim: no depth works at all.
    pub usable_depths: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at_depth: Vec<DepthExpectation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepthExpectation {
    pub depth: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts_selected: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict_preserving: Option<bool>,
}

impl GraphWalkProbe {
    pub fn new(max_depth: usize, usable_depths: Vec<usize>) -> Self {
        GraphWalkProbe {
            max_depth,
            usable_depths,
            at_depth: Vec::new(),
        }
    }

    pub fn at(mut self, depth: usize, facts_selected: usize, verdict_preserving: bool) -> Self {
        self.at_depth.push(DepthExpectation {
            depth,
            facts_selected: Some(facts_selected),
            verdict_preserving: Some(verdict_preserving),
        });
        self
    }

    pub fn check(&self, observed: &GraphWalkObservation) -> Vec<String> {
        let mut failures = Vec::new();
        compare(
            "usable graph-walk depths",
            Some(&self.usable_depths),
            Some(&observed.usable_depths),
            &mut failures,
        );
        for expectation in &self.at_depth {
            let Some(seen) = observed
                .depths
                .iter()
                .find(|d| d.depth == expectation.depth)
            else {
                failures.push(format!(
                    "no observation recorded at depth {}",
                    expectation.depth
                ));
                continue;
            };
            compare(
                &format!("facts selected at depth {}", expectation.depth),
                expectation.facts_selected,
                Some(seen.facts_selected),
                &mut failures,
            );
            compare(
                &format!("verdict preservation at depth {}", expectation.depth),
                expectation.verdict_preserving,
                Some(seen.verdict_preserving),
                &mut failures,
            );
        }
        failures
    }
}

/// Compares one asserted field, recording a mismatch. `None` on the expected side is unasserted.
fn compare<T: PartialEq + Debug>(
    field: &str,
    expected: Option<T>,
    observed: Option<T>,
    failures: &mut Vec<String>,
) {
    let Some(expected) = expected else { return };
    match observed {
        Some(observed) if observed == expected => {}
        Some(observed) => {
            failures.push(format!(
                "{field}: expected {expected:?}, observed {observed:?}"
            ));
        }
        None => failures.push(format!("{field}: expected {expected:?}, nothing observed")),
    }
}
