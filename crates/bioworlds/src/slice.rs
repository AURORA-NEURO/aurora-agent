//! Vertical slices as objects rather than prose.
//!
//! Blueprint 38 closes every reference world with a ten-point *vertical-slice acceptance* list,
//! and the same list appears verbatim in all sixteen. A list repeated sixteen times in prose is
//! not checkable by anything, which is the failure mode `crates/examples` was built against: a
//! catalogue that enumerates only what it can run makes the untested surface invisible by omitting
//! it.
//!
//! So a slice here is a value — a world, a query shape, the claim it supports, and a list of
//! [`StructuralCheck`]s a consumer can evaluate. `crates/examples` has the registry pattern this
//! matches; this crate does not depend on it, both because the dependency is not in the set and
//! because a world builder that could run the compiler would be tempted to tune worlds until the
//! compiler looked good on them.
//!
//! # The one thing a slice here never says
//!
//! Every check below is structural. None of them compiles a query, runs an oracle, or produces a
//! verdict, because `bioprism-fiber` is deliberately absent from this crate's dependencies. A
//! slice can therefore say "this world makes the property *exercisable*" and can never say "this
//! world demonstrates the property holds". [`VerticalSlice::makes_exercisable`] and
//! [`VerticalSlice::still_blocked`] are named for that difference.
//!
//! # Not implemented
//!
//! Points 3 through 9 of §38's acceptance list — sandboxed actions, architecture forks, non-LLM
//! oracle scoring, PRISM minimisation, mutation validation, signed bundles, CI regression replay.
//! Each needs a crate this one does not depend on. They are absent, and named here rather than
//! quietly dropped.

use crate::builder::BioWorld;
use crate::error::BioWorldError;
use crate::query::QueryShape;
use crate::structure::{profile, DependencyClosure, StructuralProfile};
use crate::underdetermined::{analyse, UnderdeterminationProfile};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A property of a world a consumer can check without a compiler.
///
/// Each variant is phrased as the claim it asserts, so a failing check reads as a falsified
/// sentence rather than as an assertion number.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
// `deny_unknown_fields` is deliberately absent: `serde` cannot enforce it on an internally
// tagged enum, because the tagged representation buffers the content before it knows the variant.
// The mutation battery records the resulting gap rather than carrying a no-op attribute here.
#[serde(rename_all = "snake_case", tag = "check")]
pub enum StructuralCheck {
    /// The document is accepted by the reference runtime's acceptance rules.
    WorldLoadsUnderReferenceSchema,
    /// Some factor outputs the query's target, so the query has something to compile toward.
    TargetIsProducedByAFactor,
    /// The variable is in the target's directed backward closure.
    VariableIsInTheTargetsDependencyClosure { variable: String },
    /// Some event governs the variable's availability.
    VariableIsEventManaged { variable: String },
    /// No fact providing the variable carries a protected tag.
    VariableIsNotProtected { variable: String },
    /// Every event governing the variable is released after the cut.
    VariableIsWithheldAtTheCut { variable: String },
    /// Some event governing the variable is released at or before the cut.
    VariableIsReadableAtTheCut { variable: String },
    /// No protected variable is withheld, so the cut does not double as a closure violation.
    ProtectedClosureSurvivesTheCut,
    /// No neighbourhood radius admits every decisive fact while excluding every distractor.
    NoSeparatingDepthExists,
    /// A separating radius exists and has this value. Used by controls, which are shipped
    /// precisely because they have one.
    SeparatingDepthIs { radius: usize },
    /// At least this share of distractor facts carry a tag that tokenises into the protected
    /// vocabulary without being a protected tag.
    TagCamouflageIsAtLeastPercent { percent: usize },
    /// The world's subject count sits in 38.01's 80-200 band.
    CohortIsAtBlueprintScale,
    /// At least this many hypotheses survive the available evidence.
    AtLeastThisManyLiveHypotheses { count: usize },
    /// Exactly this many survive.
    ExactlyThisManyLiveHypotheses { count: usize },
    /// A factor declares the hypotheses mutually exclusive.
    HypothesesAreDeclaredMutuallyExclusive,
    /// Every hypothesis-support input is provided by a fact or produced by a factor: the world
    /// underdetermines without being incomplete.
    NoSupportInputIsUnresolvable,
    /// Every hypothesis-support input is readable at the cut: the ambiguity is not temporal.
    NoSupportInputIsWithheldAtTheCut,
    /// The target depends on every hypothesis, so the ambiguity is on the decision path.
    EveryHypothesisIsOnTheDecisionPath,
    /// The discriminating study is present as a fact declaring it was not observed, rather than
    /// omitted from the world.
    DiscriminatingEvidenceIsDeclaredUnobserved { variable: String },
    /// The world carries at least this many facts, so the compact-selection question is not
    /// trivial.
    WorldHasAtLeastThisManyFacts { count: usize },
}

impl StructuralCheck {
    /// The claim, as a sentence.
    pub fn claim(&self) -> String {
        match self {
            StructuralCheck::WorldLoadsUnderReferenceSchema => {
                "the world loads under fiber-world/0.1 and the reference acceptance checks".into()
            }
            StructuralCheck::TargetIsProducedByAFactor => {
                "some factor produces the query's target".into()
            }
            StructuralCheck::VariableIsInTheTargetsDependencyClosure { variable } => {
                format!("the target depends on {variable}")
            }
            StructuralCheck::VariableIsEventManaged { variable } => {
                format!("an event governs the availability of {variable}")
            }
            StructuralCheck::VariableIsNotProtected { variable } => {
                format!("{variable} is not in the protected closure")
            }
            StructuralCheck::VariableIsWithheldAtTheCut { variable } => {
                format!("{variable} is unreadable at the decision cut")
            }
            StructuralCheck::VariableIsReadableAtTheCut { variable } => {
                format!("{variable} is readable at the decision cut")
            }
            StructuralCheck::ProtectedClosureSurvivesTheCut => {
                "no protected variable is withheld by the cut".into()
            }
            StructuralCheck::NoSeparatingDepthExists => {
                "no neighbourhood radius is both sound and compact".into()
            }
            StructuralCheck::SeparatingDepthIs { radius } => {
                format!("a neighbourhood radius of {radius} is both sound and compact")
            }
            StructuralCheck::TagCamouflageIsAtLeastPercent { percent } => {
                format!("at least {percent}% of distractors carry a camouflaged tag")
            }
            StructuralCheck::CohortIsAtBlueprintScale => {
                "the subject count sits in 38.01's 80-200 band".into()
            }
            StructuralCheck::AtLeastThisManyLiveHypotheses { count } => {
                format!("at least {count} hypotheses survive the available evidence")
            }
            StructuralCheck::ExactlyThisManyLiveHypotheses { count } => {
                format!("exactly {count} hypotheses survive the available evidence")
            }
            StructuralCheck::HypothesesAreDeclaredMutuallyExclusive => {
                "a factor declares the hypotheses mutually exclusive".into()
            }
            StructuralCheck::NoSupportInputIsUnresolvable => {
                "every hypothesis-support input is provided or produced".into()
            }
            StructuralCheck::NoSupportInputIsWithheldAtTheCut => {
                "every hypothesis-support input is readable at the cut".into()
            }
            StructuralCheck::EveryHypothesisIsOnTheDecisionPath => {
                "the target depends on every hypothesis".into()
            }
            StructuralCheck::DiscriminatingEvidenceIsDeclaredUnobserved { variable } => {
                format!("{variable} is present as a declared absence rather than omitted")
            }
            StructuralCheck::WorldHasAtLeastThisManyFacts { count } => {
                format!("the world carries at least {count} facts")
            }
        }
    }
}

/// A check's result, with what was actually measured.
///
/// `observed` carries the measurement even when the check passes, because a passing check whose
/// number nobody can see is indistinguishable from a check that measured nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckOutcome {
    pub check: StructuralCheck,
    pub claim: String,
    pub holds: bool,
    pub observed: String,
}

/// A property `crates/examples` records as blocked, and why it stays blocked.
///
/// Property ids are strings rather than a borrowed enum: `bioprism-examples` is not in this
/// crate's dependency set. `tests/backlog_ids.rs` asserts they are the exact snake_case ids that
/// crate's catalogue uses, so a rename there fails loudly here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockedProperty {
    pub property_id: String,
    pub reason: String,
}

/// A world, a query shape, and the claim they support.
#[derive(Debug, Clone)]
pub struct VerticalSlice {
    pub id: String,
    pub title: String,
    pub blueprint_modules: Vec<String>,
    /// What a reader is being asked to accept.
    pub claim: String,
    /// Property ids this world makes *exercisable* — not ones it demonstrates.
    pub makes_exercisable: Vec<String>,
    /// Property ids still blocked after this world exists, with the remaining obstacle.
    pub still_blocked: Vec<BlockedProperty>,
    /// Measurements worth reporting, including unfavourable ones.
    pub findings: Vec<String>,
    pub checks: Vec<StructuralCheck>,
    pub distractor_tag: String,
    world: BioWorld,
    query: QueryShape,
}

impl VerticalSlice {
    /// A slice with nothing asserted yet.
    ///
    /// The claim, the blueprint citation and the checks arrive through the methods below rather
    /// than as constructor arguments, so a slice that asserts nothing is visibly a slice that
    /// asserts nothing.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        world: BioWorld,
        query: QueryShape,
        distractor_tag: impl Into<String>,
    ) -> Self {
        VerticalSlice {
            id: id.into(),
            title: title.into(),
            blueprint_modules: Vec::new(),
            claim: String::new(),
            makes_exercisable: Vec::new(),
            still_blocked: Vec::new(),
            findings: Vec::new(),
            checks: Vec::new(),
            distractor_tag: distractor_tag.into(),
            world,
            query,
        }
    }

    /// The blueprint modules the slice answers to, and the sentence a reader must evaluate.
    pub fn about(mut self, blueprint_modules: &[&str], claim: impl Into<String>) -> Self {
        self.blueprint_modules = blueprint_modules.iter().map(|m| (*m).to_string()).collect();
        self.claim = claim.into();
        self
    }

    pub fn checking(mut self, checks: Vec<StructuralCheck>) -> Self {
        self.checks = checks;
        self
    }

    pub fn makes_exercisable(mut self, ids: &[&str]) -> Self {
        self.makes_exercisable = ids.iter().map(|id| (*id).to_string()).collect();
        self
    }

    pub fn still_blocked(mut self, blocked: Vec<BlockedProperty>) -> Self {
        self.still_blocked = blocked;
        self
    }

    pub fn with_findings(mut self, findings: &[&str]) -> Self {
        self.findings = findings.iter().map(|f| (*f).to_string()).collect();
        self
    }

    pub fn world(&self) -> &BioWorld {
        &self.world
    }

    pub fn query(&self) -> &QueryShape {
        &self.query
    }

    /// Measures the world and evaluates every check.
    pub fn run(&self) -> Result<SliceReport, BioWorldError> {
        let structure = profile(&self.world, &self.query, &self.distractor_tag)?;
        let hypotheses = analyse(&self.world, &self.query)?;

        let outcomes: Vec<CheckOutcome> = self
            .checks
            .iter()
            .map(|check| self.evaluate(check, &structure, &hypotheses))
            .collect();
        let failures = outcomes
            .iter()
            .filter(|outcome| !outcome.holds)
            .map(|outcome| format!("{} — observed: {}", outcome.claim, outcome.observed))
            .collect();

        let mut report = SliceReport {
            slice_id: self.id.clone(),
            title: self.title.clone(),
            blueprint_modules: self.blueprint_modules.clone(),
            claim: self.claim.clone(),
            world_id: self.world.id().to_string(),
            world_digest: self.world.digest()?,
            query_id: self.query.query_id.clone(),
            structure,
            hypotheses,
            checks: outcomes,
            failures,
            makes_exercisable: self.makes_exercisable.clone(),
            still_blocked: self.still_blocked.clone(),
            findings: self.findings.clone(),
            digest: String::new(),
        };
        report.digest = report.recompute_digest()?;
        Ok(report)
    }

    fn evaluate(
        &self,
        check: &StructuralCheck,
        structure: &StructuralProfile,
        hypotheses: &UnderdeterminationProfile,
    ) -> CheckOutcome {
        let inner = self.world.world();
        let (holds, observed) = match check {
            StructuralCheck::WorldLoadsUnderReferenceSchema => (
                inner.validate_reference_compat().is_ok(),
                format!("{} facts, {} factors", structure.facts, structure.factors),
            ),
            StructuralCheck::TargetIsProducedByAFactor => {
                let producers: Vec<String> = inner
                    .producers_of(&structure.target)
                    .map(|f| f.id.as_str().to_string())
                    .collect();
                (!producers.is_empty(), producers.join(", "))
            }
            StructuralCheck::VariableIsInTheTargetsDependencyClosure { variable } => {
                let closure = DependencyClosure::of_target(inner, &structure.target);
                (
                    closure.depends_on(variable),
                    format!("{} variables in the closure", closure.variables.len()),
                )
            }
            StructuralCheck::VariableIsEventManaged { variable } => {
                let managed = structure.temporal.event_managed.contains(variable);
                (
                    managed,
                    format!("event-managed: {:?}", structure.temporal.event_managed),
                )
            }
            StructuralCheck::VariableIsNotProtected { variable } => {
                let protected = inner
                    .facts
                    .iter()
                    .filter(|fact| fact.provides.as_str() == variable)
                    .flat_map(|fact| fact.tags.iter())
                    .filter(|tag| self.query.protects(tag))
                    .cloned()
                    .collect::<Vec<_>>();
                (
                    protected.is_empty(),
                    format!("protected tags carried: {protected:?}"),
                )
            }
            StructuralCheck::VariableIsWithheldAtTheCut { variable } => (
                structure.temporal.withheld.contains(variable),
                format!("withheld at the cut: {:?}", structure.temporal.withheld),
            ),
            StructuralCheck::VariableIsReadableAtTheCut { variable } => (
                !structure.temporal.withheld.contains(variable),
                format!("withheld at the cut: {:?}", structure.temporal.withheld),
            ),
            StructuralCheck::ProtectedClosureSurvivesTheCut => (
                structure.temporal.protected_closure_survives_the_cut(),
                format!(
                    "withheld and protected: {:?}",
                    structure.temporal.withheld_and_protected
                ),
            ),
            StructuralCheck::NoSeparatingDepthExists => (
                structure.separating_depth.is_none(),
                format!("separating depth: {:?}", structure.separating_depth),
            ),
            StructuralCheck::SeparatingDepthIs { radius } => (
                structure.separating_depth == Some(*radius),
                format!("separating depth: {:?}", structure.separating_depth),
            ),
            StructuralCheck::TagCamouflageIsAtLeastPercent { percent } => {
                let measured = (structure.tag_camouflage_fraction * 100.0).round() as usize;
                (measured >= *percent, format!("{measured}%"))
            }
            StructuralCheck::CohortIsAtBlueprintScale => {
                let subjects = subject_count(&self.world);
                (
                    (80..=200).contains(&subjects),
                    format!("{subjects} subjects"),
                )
            }
            StructuralCheck::AtLeastThisManyLiveHypotheses { count } => (
                hypotheses.live_hypotheses.len() >= *count,
                format!("live: {:?}", hypotheses.live_hypotheses),
            ),
            StructuralCheck::ExactlyThisManyLiveHypotheses { count } => (
                hypotheses.live_hypotheses.len() == *count,
                format!("live: {:?}", hypotheses.live_hypotheses),
            ),
            StructuralCheck::HypothesesAreDeclaredMutuallyExclusive => (
                !hypotheses.exclusion_factors.is_empty(),
                format!("exclusion factors: {:?}", hypotheses.exclusion_factors),
            ),
            StructuralCheck::NoSupportInputIsUnresolvable => (
                hypotheses.unresolvable_support_inputs.is_empty(),
                format!("unresolvable: {:?}", hypotheses.unresolvable_support_inputs),
            ),
            StructuralCheck::NoSupportInputIsWithheldAtTheCut => (
                hypotheses.support_inputs_withheld_at_cut.is_empty(),
                format!("withheld: {:?}", hypotheses.support_inputs_withheld_at_cut),
            ),
            StructuralCheck::EveryHypothesisIsOnTheDecisionPath => {
                let closure = DependencyClosure::of_target(inner, &structure.target);
                let missing: Vec<&String> = hypotheses
                    .hypothesis_variables
                    .iter()
                    .filter(|variable| !closure.depends_on(variable))
                    .collect();
                (missing.is_empty(), format!("off the path: {missing:?}"))
            }
            StructuralCheck::DiscriminatingEvidenceIsDeclaredUnobserved { variable } => (
                hypotheses
                    .declared_unobserved_discriminating_evidence
                    .contains(variable),
                format!(
                    "declared unobserved: {:?}",
                    hypotheses.declared_unobserved_discriminating_evidence
                ),
            ),
            StructuralCheck::WorldHasAtLeastThisManyFacts { count } => (
                structure.facts >= *count,
                format!("{} facts", structure.facts),
            ),
        };

        CheckOutcome {
            check: check.clone(),
            claim: check.claim(),
            holds,
            observed,
        }
    }
}

/// Subjects, read off the per-subject value maps.
///
/// Derived from the document rather than carried alongside it, so the number a report states is
/// the number the world actually has.
pub fn subject_count(world: &BioWorld) -> usize {
    world
        .world()
        .facts
        .iter()
        .filter_map(|fact| fact.value.as_object())
        .map(|map| {
            map.keys()
                .filter(|key| {
                    key.starts_with('S')
                        && key.len() == 4
                        && key[1..].chars().all(|c| c.is_ascii_digit())
                })
                .count()
        })
        .max()
        .unwrap_or(0)
}

/// What a slice measured, as one serialisable artefact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SliceReport {
    pub slice_id: String,
    pub title: String,
    pub blueprint_modules: Vec<String>,
    pub claim: String,
    pub world_id: String,
    pub world_digest: String,
    pub query_id: String,
    pub structure: StructuralProfile,
    pub hypotheses: UnderdeterminationProfile,
    pub checks: Vec<CheckOutcome>,
    /// Failed checks, as falsified sentences.
    pub failures: Vec<String>,
    pub makes_exercisable: Vec<String>,
    pub still_blocked: Vec<BlockedProperty>,
    pub findings: Vec<String>,
    pub digest: String,
}

impl SliceReport {
    pub fn holds(&self) -> bool {
        self.failures.is_empty()
    }

    pub fn body(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("slice report is serialisable");
        if let Some(map) = value.as_object_mut() {
            map.remove("digest");
        }
        value
    }

    pub fn recompute_digest(&self) -> Result<String, BioWorldError> {
        ContentHash::of_value(&self.body())
            .map(|hash| hash.as_str().to_string())
            .map_err(|source| BioWorldError::Digest {
                subject: format!("slice {}", self.slice_id),
                message: source.to_string(),
            })
    }

    pub fn digest_is_intact(&self) -> bool {
        self.recompute_digest()
            .is_ok_and(|recomputed| recomputed == self.digest)
    }

    /// A summary a reader can evaluate without opening the JSON.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{} {} [{}]\n  {}\n  world {} ({} facts, {} factors, {} events) digest {}\n",
            if self.holds() { "PASS" } else { "FAIL" },
            self.slice_id,
            self.blueprint_modules.join(", "),
            self.claim,
            self.world_id,
            self.structure.facts,
            self.structure.factors,
            self.structure.events,
            self.world_digest
        ));
        out.push_str(&format!(
            "  separating depth {:?}, elimination width {} (decisive {}), camouflage {:.0}%\n",
            self.structure.separating_depth,
            self.structure.elimination_width,
            self.structure.decisive_elimination_width,
            self.structure.tag_camouflage_fraction * 100.0
        ));
        out.push_str(&format!(
            "  protected {} / unprotected {} facts; withheld and unprotected and decisive: {:?}\n",
            self.structure.protected_facts,
            self.structure.unprotected_facts,
            self.structure.temporal.withheld_not_protected_and_decisive
        ));
        for outcome in &self.checks {
            out.push_str(&format!(
                "    {} {} — {}\n",
                if outcome.holds { "ok  " } else { "FAIL" },
                outcome.claim,
                outcome.observed
            ));
        }
        if !self.makes_exercisable.is_empty() {
            out.push_str(&format!(
                "  makes exercisable: {}\n",
                self.makes_exercisable.join(", ")
            ));
        }
        for blocked in &self.still_blocked {
            out.push_str(&format!(
                "  still blocked: {} — {}\n",
                blocked.property_id, blocked.reason
            ));
        }
        for finding in &self.findings {
            out.push_str(&format!("  finding: {finding}\n"));
        }
        out
    }
}
