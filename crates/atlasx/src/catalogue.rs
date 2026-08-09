//! Every metric this crate's scope names, and the one it defines.
//!
//! The scope was ten modules of the capability-metrics section and twelve of the public-hub
//! section. Between them they name **118 metrics**. Not one carries a formula, an estimator, or a
//! stated denominator, except for five that name a denominator in passing and still say nothing
//! about the numerator. This module is that fact as data rather than as a complaint, so a test can
//! assert it and a reader can check it against the blueprint line by line.
//!
//! `bioprism-metrics` found the same thing across the whole capability-metrics section — 222 names
//! over nineteen modules, none defined — and drew the right conclusion: implement the *discipline*
//! the section does define, repeatedly and identically, and refuse to invent the estimators. This
//! crate inherits that conclusion. [`NAMED_NEVER_DEFINED`] is what the refusal costs, itemised.
//!
//! # Modules named here, and modules cited
//!
//! Entries are keyed by module **title**, not by blueprint id, and that is deliberate.
//! `tools/coverage.sh` counts any `NN.MM` token under `crates/` as a position taken on that module.
//! Writing twenty-two ids into a table of things this crate did not implement would move a coverage
//! percentage by transcribing a bulleted list, which is the one thing this workspace must not do.
//!
//! The two modules this crate does implement are cited by id in [`crate::debt`] and
//! [`crate::browse`], and they are the only two ids in the crate.
//!
//! # The one blueprint metric this crate defines
//!
//! *Profile coverage*, from the BioCapability Atlas module, as
//! [`crate::DebtStatement::profile_coverage`]: measured capabilities over capabilities in the grid,
//! refusing on an empty grid. The definition is stated in that method's documentation, travels with
//! its subject, and is *a* definition rather than *the* definition — the module names the metric and
//! stops, so any denominator is a choice somebody has to make and say out loud.
//!
//! Two further numbers leave this crate with stated denominators and are **not** blueprint names:
//! share-of-aggregated and failures-per-independent-unit. They are listed in [`DEFINED_HERE`] so the
//! distinction between "the blueprint asked for this" and "this crate needed it" survives contact
//! with a reader.

use serde::{Deserialize, Serialize};

/// Which section of the blueprint a metric name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// The BioCapability Atlas and Metrics section.
    CapabilityMetrics,
    /// The BioAtlas Public Hub and Ecosystem section.
    PublicHub,
}

/// A metric the blueprint names and never defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NamedMetric {
    pub origin: Origin,
    /// The module's title. Not its id; see this module's documentation.
    pub module_title: &'static str,
    pub metric: &'static str,
    /// The denominator the name itself implies, where it implies one. Never a numerator, and never
    /// an estimator: "obligations closed per 1,000 tokens" says what to divide by and nothing at
    /// all about what counts as closing an obligation.
    pub denominator: Option<&'static str>,
}

/// A number this crate emits with a stated denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DefinedMetric {
    pub metric: &'static str,
    /// True when the blueprint names this metric; false when the name is this crate's own.
    pub blueprint_name: bool,
    pub numerator: &'static str,
    pub denominator: &'static str,
    /// What the number refuses to do rather than what it does.
    pub refuses: &'static str,
}

const M: Origin = Origin::CapabilityMetrics;
const H: Origin = Origin::PublicHub;

const T_GROUNDING: &str = "Evidence Grounding, Provenance, and Claim Support";
const T_ACQUISITION: &str = "Information Acquisition and Context Value";
const T_VOE: &str = "Value of Experiment, Assay Selection, and Active Discovery";
const T_RESOURCE: &str = "Tissue, Sample, Time, and Resource Efficiency";
const T_TEMPORAL: &str = "Temporal Validity and Evidence Firewall Metrics";
const T_CROSSMODAL: &str = "Cross-Modal Consistency and Contradiction Metrics";
const T_CAUSAL: &str = "Causal Identification, Intervention, and Mechanism Metrics";
const T_REPRO: &str = "Reproducibility, Reexecution, and Claim Stability";
const T_SPINE: &str = "Translation Spine and Evidence Maturity Metrics";
const T_COORD: &str = "Multi-Agent Coordination and Molecule Value";

const T_PERSONAS: &str = "Users, Personas, and Jobs to be Done";
const T_IA: &str = "Information Architecture and Navigation";
const T_WORLDLINE: &str = "Worldline Timeline and State Explorer";
const T_MICROSCOPE: &str = "BioDecision Cell Inference Microscope";
const T_FORK: &str = "Fork Compare and Counterfactual Lab";
const T_ORACLE: &str = "Oracle Evidence and Disagreement Explorer";
const T_ATLAS: &str = "BioCapability Atlas";
const T_FAILURE: &str = "Failure Atlas and First-Divergence Browser";
const T_REGISTRY: &str = "Architecture and Agent Molecule Registry";
const T_INTEGRATIONS: &str = "Notebook, IDE, MCP, and Agent Integrations";
const T_ONBOARDING: &str = "No-Key Demos and Onboarding";
const T_COMMUNITY: &str = "Open Source Community and Star Flywheel";

const fn n(origin: Origin, module_title: &'static str, metric: &'static str) -> NamedMetric {
    NamedMetric {
        origin,
        module_title,
        metric,
        denominator: None,
    }
}

const fn d(
    origin: Origin,
    module_title: &'static str,
    metric: &'static str,
    denominator: &'static str,
) -> NamedMetric {
    NamedMetric {
        origin,
        module_title,
        metric,
        denominator: Some(denominator),
    }
}

/// The 117 metric names in this crate's scope that nothing anywhere defines.
///
/// The scope names 118; the missing one is *profile coverage*, which [`DEFINED_HERE`] gives a
/// denominator. Transcribed from the "Primary metrics" list of each capability-metrics module and
/// the "Product metrics" list of each public-hub module, in blueprint order within each module.
pub const NAMED_NEVER_DEFINED: &[NamedMetric] = &[
    n(M, T_GROUNDING, "grounded claim precision"),
    n(M, T_GROUNDING, "evidence obligation completion"),
    n(M, T_GROUNDING, "unsupported-claim rate"),
    n(M, T_GROUNDING, "citation entailment"),
    n(M, T_GROUNDING, "provenance replay success"),
    d(
        M,
        T_ACQUISITION,
        "obligations closed per 1,000 tokens",
        "tokens",
    ),
    n(M, T_ACQUISITION, "posterior entropy reduction"),
    n(M, T_ACQUISITION, "decision-regret reduction"),
    n(M, T_ACQUISITION, "unnecessary-access rate"),
    n(M, T_ACQUISITION, "stopping efficiency"),
    n(M, T_VOE, "realized value of information"),
    n(M, T_VOE, "oracle regret"),
    n(M, T_VOE, "tissue-adjusted discovery yield"),
    n(M, T_VOE, "falsification rate"),
    n(M, T_VOE, "budget feasibility"),
    n(M, T_VOE, "adaptive policy gain"),
    d(M, T_RESOURCE, "utility per tissue unit", "tissue unit"),
    n(M, T_RESOURCE, "wasted irreversible resource"),
    n(M, T_RESOURCE, "time-to-supported-claim"),
    d(
        M,
        T_RESOURCE,
        "expert-minutes per resolved case",
        "resolved case",
    ),
    d(M, T_RESOURCE, "compute-to-information ratio", "information"),
    n(M, T_TEMPORAL, "temporal relation accuracy"),
    d(M, T_TEMPORAL, "leakage events per run", "run"),
    n(M, T_TEMPORAL, "baseline error"),
    n(M, T_TEMPORAL, "time-origin validity"),
    n(M, T_TEMPORAL, "stale-evidence use"),
    n(M, T_TEMPORAL, "revision auditability"),
    n(M, T_CROSSMODAL, "compatible-join precision"),
    n(M, T_CROSSMODAL, "contradiction recall"),
    n(M, T_CROSSMODAL, "false-conflict rate"),
    n(M, T_CROSSMODAL, "cross-modal calibration"),
    n(M, T_CROSSMODAL, "circularity rate"),
    n(M, T_CROSSMODAL, "resolution value"),
    n(M, T_CAUSAL, "identification accuracy"),
    n(M, T_CAUSAL, "effect bias and coverage"),
    n(M, T_CAUSAL, "negative-control behavior"),
    n(M, T_CAUSAL, "counterfactual regret"),
    n(M, T_CAUSAL, "causal-language precision"),
    n(M, T_CAUSAL, "mechanism maturity"),
    n(M, T_REPRO, "reexecution rate"),
    n(M, T_REPRO, "numerical tolerance pass"),
    n(M, T_REPRO, "figure agreement"),
    n(M, T_REPRO, "claim-flip rate"),
    n(M, T_REPRO, "dependency fragility"),
    n(M, T_REPRO, "independent reproduction"),
    n(M, T_SPINE, "supported-edge fraction"),
    n(M, T_SPINE, "weakest-link maturity"),
    n(M, T_SPINE, "translation debt"),
    n(M, T_SPINE, "transport calibration"),
    n(M, T_SPINE, "unjustified scale-jump rate"),
    n(M, T_COORD, "team gain"),
    n(M, T_COORD, "coordination overhead"),
    n(M, T_COORD, "duplicate-work rate"),
    n(M, T_COORD, "dissent preservation"),
    n(M, T_COORD, "authority violations"),
    n(M, T_COORD, "failure recovery"),
    n(H, T_PERSONAS, "time to first meaningful replay"),
    n(H, T_PERSONAS, "successful parent-world authoring"),
    n(H, T_PERSONAS, "result reproduction"),
    n(H, T_PERSONAS, "private-site participation"),
    n(H, T_PERSONAS, "expert review burden"),
    n(H, T_PERSONAS, "repeat usage"),
    n(H, T_IA, "findability"),
    n(H, T_IA, "cross-object traceability"),
    n(H, T_IA, "time to provenance"),
    n(H, T_IA, "broken-link rate"),
    n(H, T_IA, "user comprehension"),
    n(H, T_WORLDLINE, "temporal comprehension"),
    n(H, T_WORLDLINE, "cutoff correctness"),
    n(H, T_WORLDLINE, "event trace latency"),
    n(H, T_WORLDLINE, "user error rate"),
    n(H, T_WORLDLINE, "provenance resolution"),
    n(H, T_MICROSCOPE, "decision replay rate"),
    n(H, T_MICROSCOPE, "fork setup time"),
    n(H, T_MICROSCOPE, "attribution usefulness"),
    n(H, T_MICROSCOPE, "cell reproducibility"),
    n(H, T_MICROSCOPE, "failure-minimization success"),
    n(H, T_FORK, "paired effect precision"),
    n(H, T_FORK, "cache correctness"),
    n(H, T_FORK, "state equivalence"),
    n(H, T_FORK, "experiment setup time"),
    n(H, T_FORK, "result interpretability"),
    n(H, T_ORACLE, "oracle transparency"),
    n(H, T_ORACLE, "disagreement resolution time"),
    n(H, T_ORACLE, "expert trust"),
    n(H, T_ORACLE, "appeal turnaround"),
    n(H, T_ORACLE, "circularity detection"),
    n(H, T_ATLAS, "confidence"),
    n(H, T_ATLAS, "rank stability"),
    n(H, T_ATLAS, "failure discoverability"),
    n(H, T_ATLAS, "user decision value"),
    n(H, T_FAILURE, "failure localization precision"),
    n(H, T_FAILURE, "download-to-replay success"),
    n(H, T_FAILURE, "duplicate failure reduction"),
    n(H, T_FAILURE, "regression closure"),
    n(H, T_FAILURE, "cross-system transfer"),
    n(H, T_REGISTRY, "reproducible instantiation"),
    n(H, T_REGISTRY, "component coverage"),
    n(H, T_REGISTRY, "substitution safety"),
    n(H, T_REGISTRY, "registry reuse"),
    n(H, T_REGISTRY, "architecture drift"),
    n(H, T_INTEGRATIONS, "integration setup time"),
    n(H, T_INTEGRATIONS, "trace completeness"),
    n(H, T_INTEGRATIONS, "round-trip fidelity"),
    n(H, T_INTEGRATIONS, "developer retention"),
    n(H, T_INTEGRATIONS, "tool compatibility"),
    n(H, T_ONBOARDING, "install success"),
    n(H, T_ONBOARDING, "time to first replay"),
    n(H, T_ONBOARDING, "demo completion"),
    n(H, T_ONBOARDING, "conversion to own-world import"),
    n(H, T_ONBOARDING, "documentation search success"),
    n(H, T_COMMUNITY, "contributors"),
    n(H, T_COMMUNITY, "maintained packs"),
    n(H, T_COMMUNITY, "repeat contributors"),
    n(H, T_COMMUNITY, "world reproductions"),
    n(H, T_COMMUNITY, "external integrations"),
    n(H, T_COMMUNITY, "stars-to-active-use ratio"),
];

/// How many metrics this crate's twenty-two blueprint modules name in total.
pub const METRICS_NAMED_IN_SCOPE: usize = 118;

/// The numbers this crate emits, with their denominators written down.
pub const DEFINED_HERE: &[DefinedMetric] = &[
    DefinedMetric {
        metric: "profile coverage",
        blueprint_name: true,
        numerator: "capabilities with a measured cell",
        denominator: "capabilities in the grid, which is a reading and not an ontology",
        refuses: "an empty grid, where zero over zero is the absence of a measurement",
    },
    DefinedMetric {
        metric: "share of aggregated",
        blueprint_name: false,
        numerator: "visible failures in one bucket",
        denominator: "every record browsed, withheld ones included",
        refuses: "shrinking when a record is withheld, so the visible shares stop summing to one",
    },
    DefinedMetric {
        metric: "failures per independent unit",
        blueprint_name: false,
        numerator: "visible, system-charged failures implicating one capability",
        denominator: "the effective size of that capability's grid cell",
        refuses: "a grid that is a reading of a different subject, and a cell that is a hole",
    },
];

/// The metric names in scope, counting the one this crate defines.
pub fn named_in_scope() -> usize {
    NAMED_NEVER_DEFINED.len()
        + DEFINED_HERE
            .iter()
            .filter(|metric| metric.blueprint_name)
            .count()
}

/// The metrics of one module, by title.
pub fn by_module(module_title: &str) -> Vec<&'static NamedMetric> {
    NAMED_NEVER_DEFINED
        .iter()
        .filter(|metric| metric.module_title == module_title)
        .collect()
}
