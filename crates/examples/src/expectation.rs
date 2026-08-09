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
    BundleObservation, CompiledObservation, GraphWalkObservation, Observations, RefusalCode,
    RefusalObservation,
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
    /// Compiled factors a slice expects to see. A subset check, for the same reason
    /// [`Compiled::deferred_passes_include`] is one: what a slice claims is that a particular
    /// check reached the target, and pinning the relay factors alongside it would make the
    /// assertion fail on a change to relay bookkeeping the claim does not depend on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_factors_include: Option<Vec<String>>,
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
    /// The exact facts the temporal cut removed from the selection, by id. Asserting the set
    /// rather than a count is what separates "some evidence was withheld" from "*this* evidence
    /// was withheld", and only the second is checkable against the release schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inaccessible_selected_before_cut: Option<Vec<String>>,
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

    pub fn selected_factors_include<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.selected_factors_include = Some(values.into_iter().map(Into::into).collect());
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

    pub fn inaccessible_selected_before_cut<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inaccessible_selected_before_cut = Some(values.into_iter().map(Into::into).collect());
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
            "facts withheld at the decision cut",
            self.inaccessible_selected_before_cut.as_ref(),
            Some(&observed.inaccessible_selected_before_cut),
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

        if let Some(expected) = &self.selected_factors_include {
            for factor in expected {
                if !observed.selected_factors.contains(factor) {
                    failures.push(format!(
                        "expected factor {factor:?} in the compiled selection, but the compiler selected {:?}",
                        observed.selected_factors
                    ));
                }
            }
        }

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

/// A result bundle built from the slice's own compile and handed back to a verifier (34.14).
///
/// The bundle is not a second scenario, it is the *same* compile packaged for transport, which is
/// why it belongs to the slice in the same way [`GraphWalkProbe`] does. Blueprint 19.06 asks that a
/// result be judged from a bundle rather than from console output, and a bundle assembled from a
/// hand-written certificate — as `bioprism-bundle`'s own tests must do, having no compiler — cannot
/// say whether the compiler's certificate survives the round trip.
///
/// # The key is published in this file, and that is not a mistake
///
/// [`BundleProbe::key_bytes`] ships in the source. Under HMAC-SHA256 a published secret means every
/// reader can verify the tag *and* mint an identical one, which is exactly the property this crate
/// now claims and no more. A slice that hid the key would look like it was protecting something and
/// would be claiming, by implication, an origin guarantee the scheme does not provide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleProbe {
    pub bundle_id: String,
    /// The label a verifier looks the key up by. Not a credential, and not derived from the key.
    pub key_identity: String,
    /// The shared secret, as bytes. See the type docs for why it is in the open.
    pub key_bytes: Vec<u8>,
    /// The name the producer asserts. Authenticated in the sense that a key holder committed to
    /// the string, and corroborated by nothing.
    pub claimed_producer: String,
    #[serde(default)]
    pub expected: BundleExpectation,
}

/// Assertions over a bundle round trip. `None` means unasserted, as everywhere else here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleExpectation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recomputed_entries: Option<Vec<String>>,
    /// Entries that travelled as a digest alone. Asserted, because an entry silently dropping out
    /// of the carried set would otherwise make the bundle look smaller and cleaner than it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_recomputed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_certificate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub survives_json_round_trip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authenticated_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repudiability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub without_the_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_forgery_is_identical: Option<bool>,
}

impl BundleProbe {
    pub fn new(
        bundle_id: impl Into<String>,
        key_identity: impl Into<String>,
        key_bytes: Vec<u8>,
        claimed_producer: impl Into<String>,
    ) -> Self {
        BundleProbe {
            bundle_id: bundle_id.into(),
            key_identity: key_identity.into(),
            key_bytes,
            claimed_producer: claimed_producer.into(),
            expected: BundleExpectation::default(),
        }
    }

    pub fn expecting(mut self, expected: BundleExpectation) -> Self {
        self.expected = expected;
        self
    }

    pub fn check(&self, observed: &BundleObservation) -> Vec<String> {
        let expected = &self.expected;
        let mut failures = Vec::new();
        compare(
            "bundle entries recomputed from carried content",
            expected.recomputed_entries.as_ref(),
            Some(&observed.recomputed_entries),
            &mut failures,
        );
        compare(
            "bundle entries recorded by digest only",
            expected.not_recomputed.as_ref(),
            Some(&observed.not_recomputed),
            &mut failures,
        );
        compare(
            "embedded certificate",
            expected.embedded_certificate.as_ref(),
            Some(&observed.embedded_certificate),
            &mut failures,
        );
        compare(
            "bundle survives a JSON round trip",
            expected.survives_json_round_trip,
            Some(observed.survives_json_round_trip),
            &mut failures,
        );
        compare(
            "authenticated key",
            expected.authenticated_key.as_ref(),
            Some(&observed.authenticated_key),
            &mut failures,
        );
        compare(
            "authentication scheme",
            expected.scheme.as_ref(),
            Some(&observed.scheme),
            &mut failures,
        );
        compare(
            "repudiability",
            expected.repudiability.as_ref(),
            Some(&observed.repudiability),
            &mut failures,
        );
        compare(
            "what a reviewer without the key learns",
            expected.without_the_key.as_ref(),
            Some(&observed.without_the_key),
            &mut failures,
        );
        compare(
            "a verifier's forgery is byte-identical",
            expected.verifier_forgery_is_identical,
            Some(observed.verifier_forgery_is_identical),
            &mut failures,
        );
        failures
    }
}

impl BundleExpectation {
    pub fn new() -> Self {
        Self::default()
    }

    builder! {
        embedded_certificate: String,
        survives_json_round_trip: bool,
        authenticated_key: String,
        scheme: String,
        repudiability: String,
        without_the_key: String,
        verifier_forgery_is_identical: bool,
    }

    pub fn recomputed_entries<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.recomputed_entries = Some(values.into_iter().map(Into::into).collect());
        self
    }

    pub fn not_recomputed<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.not_recomputed = Some(values.into_iter().map(Into::into).collect());
        self
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
