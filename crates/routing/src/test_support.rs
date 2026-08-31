//! Fixtures shared by the unit tests.
//!
//! Two kinds live here and they are used for different jobs. Worldgen-backed helpers produce
//! *real* worlds so that structural claims about the fingerprint are measured rather than
//! stipulated. Hand-built helpers produce arithmetic fixtures so that claims about regret, verdict
//! selection and reporting can be checked against numbers whose right answer is known by hand.

use crate::architecture::Architecture;
use crate::comparator::Comparator;
use crate::evidence::Observation;
use crate::fingerprint::{AttachmentRegime, ChainRegime, Fingerprint, Regime, TagRegime};
use crate::lab::Task;
use crate::policy::DecisionReason;
use crate::regret::{ComparatorOutcome, RegretAccount};
use crate::report::{ComparatorPick, TaskRow};
use bioprism_fiber::Query;
use bioprism_section::OracleStatus;
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use std::collections::BTreeMap;

pub fn world_and_query(spec: &WorldSpec) -> (World, Query) {
    let generated = generate(spec);
    (
        World::from_json(generated.world).expect("worldgen emits a valid fiber-world/0.1 document"),
        Query::from_json(generated.query).expect("worldgen emits a valid fiber-query/0.1 document"),
    )
}

pub fn spec_task(task_id: &str, spec: &WorldSpec) -> Task {
    let (world, query) = world_and_query(spec);
    Task::new(task_id, world, query).expect("task identifiers in tests are non-empty")
}

/// A fingerprint whose three categorical axes are set directly, for distance and policy tests.
pub fn synthetic_fingerprint(
    tag_informativeness: f64,
    max_unary_chain: usize,
    hub_is_derived: bool,
    facts: usize,
) -> Fingerprint {
    Fingerprint {
        facts,
        factors: facts,
        protected_tag_count: 10,
        protected_fact_fraction: 0.1,
        distractor_density: 0.9,
        tag_informativeness,
        mean_factor_arity: 2.0,
        max_factor_arity: 2,
        arity_histogram: BTreeMap::from([(2, facts)]),
        max_unary_chain,
        hub_share: 0.9,
        hub_is_derived,
        target_producer_count: 1,
    }
}

pub fn observation_with(
    fingerprint: &Fingerprint,
    task_id: &str,
    architecture: Architecture,
    admissible: bool,
    facts_exposed: usize,
    total_facts: usize,
) -> Observation {
    Observation {
        task_id: task_id.to_string(),
        fingerprint: fingerprint.clone(),
        architecture,
        verdict_preserving: admissible,
        closure_complete: admissible,
        status: if admissible {
            OracleStatus::Invalid
        } else {
            OracleStatus::Underdetermined
        },
        facts_exposed,
        total_facts,
    }
}

pub fn observation(
    task_id: &str,
    architecture: Architecture,
    admissible: bool,
    facts_exposed: usize,
    total_facts: usize,
) -> Observation {
    observation_with(
        &synthetic_fingerprint(1.0, 1, false, total_facts),
        task_id,
        architecture,
        admissible,
        facts_exposed,
        total_facts,
    )
}

fn pick(architecture: Architecture, utility: f64) -> ComparatorPick {
    ComparatorPick {
        architecture,
        admissible: utility >= 0.0,
        cost_fraction: (1.0 - utility).clamp(0.0, 1.0),
        utility,
    }
}

/// A hand-built account with the four mean utilities set directly.
pub fn account(fixed: f64, most_expensive: f64, router: f64, oracle: f64) -> RegretAccount {
    let outcome = |comparator: Comparator, mean_utility: f64| ComparatorOutcome {
        comparator,
        tasks: 10,
        mean_utility,
        mean_regret: oracle - mean_utility,
        admissible_rate: 1.0,
        mean_cost_fraction: (1.0 - mean_utility).clamp(0.0, 1.0),
    };

    RegretAccount {
        fixed_default: outcome(
            Comparator::FixedDefault {
                architecture: Architecture::FiberCompiled,
            },
            fixed,
        ),
        most_expensive_default: outcome(
            Comparator::MostExpensiveDefault {
                architecture: Architecture::FullContext,
            },
            most_expensive,
        ),
        router: outcome(Comparator::EvidenceRouter, router),
        oracle: outcome(Comparator::OracleRetrospective, oracle),
    }
}

/// A hand-built task row with the four per-task utilities set directly.
pub fn task_row(
    task_id: &str,
    fixed: f64,
    most_expensive: f64,
    router: f64,
    oracle: f64,
) -> TaskRow {
    let oracle_architecture = Architecture::GraphKHop { depth: 5 };
    let router_architecture = if (router - oracle).abs() < 1e-12 {
        oracle_architecture.clone()
    } else {
        Architecture::LexicalTopK { k: 11 }
    };

    TaskRow {
        task_id: task_id.to_string(),
        regime: Regime {
            tags: TagRegime::Separable,
            chain: ChainRegime::Shallow,
            attachment: AttachmentRegime::Leaf,
        },
        facts: 100,
        abstained: false,
        confidence: 0.5,
        reason: DecisionReason::Routed {
            margin: 0.2,
            supporting_tasks: 3,
            runner_up: Architecture::FullContext,
        },
        fixed_default: pick(Architecture::FiberCompiled, fixed),
        most_expensive_default: pick(Architecture::FullContext, most_expensive),
        router: pick(router_architecture, router),
        oracle: pick(oracle_architecture, oracle),
    }
}
