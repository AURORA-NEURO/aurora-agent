//! Gate 4, end to end, against a synthetic registry whose truth is known.
//!
//! The critical path states the gate as: "Demonstrate that a 500–2,000-instance adaptive panel
//! estimates the same meaningful capability differences as a much larger exhaustive run while
//! preserving coverage constraints and confidence guarantees."
//!
//! Every clause is checked here. The registry below is deliberately built the way Gate 3 builds
//! one — a few audited parents, many descendants each, and outcomes that depend strongly on which
//! parent an instance came from. That parent effect is the whole difficulty: it is what makes the
//! exhaustive run's four thousand trials worth far fewer than four thousand, and it is what a
//! panel that samples greedily inside two parent families would get catastrophically wrong.
//!
//! What this does **not** show: that the selection policy beats a well-tuned stratified fixed
//! panel. Blueprint 08.08 requires exactly that comparison before adaptive scheduling may control
//! a release, and the baseline here is a convenience panel — registry order, no stratification —
//! which is a low bar chosen to isolate one failure mode, not to settle the question. The outcome
//! surface is also synthetic and generated from the very exchangeability assumption the estimator
//! makes, so this exercises the arithmetic, not the modelling assumptions.

use bioprism_adaptive::{
    rng::SplitMix64, AdaptivePanel, CapabilityId, Candidate, CoveragePolicy, InstanceId, Outcome,
    PanelConfig, ParentId, Question, SelectionConfig, StoppingRule, Trial,
};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const CAPABILITIES: [(&str, f64); 3] = [
    ("retrieval", 0.85),
    ("planning", 0.60),
    ("leakage-detection", 0.57),
];
const PARENTS: usize = 24;
const INSTANCES_PER_PARENT: usize = 60;
/// Half-width of the uniform per-parent shift in success probability. This is the parent effect;
/// set it to zero and the clustering correction correctly becomes a no-op.
const PARENT_SPREAD: f64 = 0.22;
/// The adaptive panel's total trial budget. Inside Gate 4's stated 500–2,000 band.
const ADAPTIVE_BUDGET: usize = 540;
const EXHAUSTIVE_TRIALS: usize = CAPABILITIES.len() * PARENTS * INSTANCES_PER_PARENT;

struct Registry {
    candidates: Vec<Candidate>,
    outcomes: BTreeMap<InstanceId, bool>,
}

impl Registry {
    fn build(seed: u64) -> Self {
        let mut rng = SplitMix64::new(seed);
        let mut candidates = Vec::with_capacity(EXHAUSTIVE_TRIALS);
        let mut outcomes = BTreeMap::new();
        for (c, (capability, ability)) in CAPABILITIES.iter().enumerate() {
            let capability = CapabilityId::parse(*capability).unwrap();
            for p in 0..PARENTS {
                let parent = ParentId::parse(format!("c{c}-p{p:02}")).unwrap();
                let shift = (rng.unit() - 0.5) * 2.0 * PARENT_SPREAD;
                let rate = (ability + shift).clamp(0.03, 0.97);
                for i in 0..INSTANCES_PER_PARENT {
                    let instance = InstanceId::parse(format!("c{c}-p{p:02}-i{i:03}")).unwrap();
                    outcomes.insert(instance.clone(), rng.unit() < rate);
                    candidates.push(
                        Candidate::new(instance, capability.clone(), parent.clone(), 1.0).unwrap(),
                    );
                }
            }
        }
        Registry {
            candidates,
            outcomes,
        }
    }

    fn trial(&self, candidate: &Candidate) -> Trial {
        let pass = self.outcomes[&candidate.instance];
        Trial::new(
            candidate.capability.clone(),
            candidate.instance.clone(),
            candidate.parent.clone(),
            if pass { Outcome::Pass } else { Outcome::Fail },
            candidate.cost,
        )
        .unwrap()
    }
}

fn config(budget: usize) -> PanelConfig {
    PanelConfig {
        coverage: CoveragePolicy {
            min_trials_per_capability: 40,
            min_parents_per_capability: 10,
            min_trials_per_parent: 3,
            max_parent_share: None,
            sentinels: BTreeMap::new(),
        },
        stopping: StoppingRule {
            question: Question::IntervalWidth { target: 0.10 },
            budget,
            min_effective_trials: 10.0,
            credibility: 0.95,
        },
        selection: SelectionConfig::default(),
        ..PanelConfig::default()
    }
}

fn exhaustive_panel(registry: &Registry) -> AdaptivePanel {
    let mut panel = AdaptivePanel::new(config(EXHAUSTIVE_TRIALS));
    for candidate in &registry.candidates {
        panel.record(registry.trial(candidate)).unwrap();
    }
    panel
}

fn adaptive_panel(registry: &Registry) -> AdaptivePanel {
    let mut panel = AdaptivePanel::new(config(ADAPTIVE_BUDGET));
    for _ in 0..ADAPTIVE_BUDGET {
        let record = panel.select_next(&registry.candidates).unwrap();
        let candidate = registry
            .candidates
            .iter()
            .find(|c| c.instance == record.chosen.instance)
            .expect("the selector only ever returns candidates it was given");
        panel.record(registry.trial(candidate)).unwrap();
    }
    panel
}

/// The convenience baseline: take the first `ADAPTIVE_BUDGET` instances in registry order.
///
/// This is not a straw man of *stratified* sampling, which would do fine. It is a straw man of
/// the thing people actually do — iterate the generated corpus — and the point is only that
/// spending a whole budget inside a handful of parent families is a coverage failure the panel
/// must be able to detect rather than a slightly noisier estimate.
fn convenience_panel(registry: &Registry) -> AdaptivePanel {
    let mut panel = AdaptivePanel::new(config(ADAPTIVE_BUDGET));
    for candidate in registry.candidates.iter().take(ADAPTIVE_BUDGET) {
        panel.record(registry.trial(candidate)).unwrap();
    }
    panel
}

struct Gate4 {
    registry: Registry,
    exhaustive: AdaptivePanel,
    adaptive: AdaptivePanel,
}

fn gate4() -> &'static Gate4 {
    static CELL: OnceLock<Gate4> = OnceLock::new();
    CELL.get_or_init(|| {
        let registry = Registry::build(0x0006_A7E4);
        let exhaustive = exhaustive_panel(&registry);
        let adaptive = adaptive_panel(&registry);
        Gate4 {
            registry,
            exhaustive,
            adaptive,
        }
    })
}

fn capability(name: &str) -> CapabilityId {
    CapabilityId::parse(name).unwrap()
}

#[test]
fn the_adaptive_panel_stays_inside_gate_fours_stated_panel_size() {
    let gate = gate4();
    assert!((500..=2000).contains(&gate.adaptive.ledger().len()));
    assert!(gate.exhaustive.ledger().len() >= 8 * gate.adaptive.ledger().len());
}

#[test]
fn the_adaptive_panel_recovers_the_exhaustive_capability_estimates() {
    let gate = gate4();
    for (name, _) in CAPABILITIES {
        let capability = capability(name);
        let full = gate.exhaustive.estimate(&capability).unwrap();
        let small = gate.adaptive.estimate(&capability).unwrap();
        let gap = (full.posterior_mean - small.posterior_mean).abs();
        assert!(
            gap < 0.06,
            "{name}: adaptive {:.3} vs exhaustive {:.3} ({} vs {} trials)",
            small.posterior_mean,
            full.posterior_mean,
            small.trials,
            full.trials
        );
    }
}

#[test]
fn the_adaptive_panel_orders_every_pair_the_exhaustive_run_can_actually_resolve() {
    // Gate 4 asks for the same *meaningful* differences, not the same ranking. Planning and
    // leakage-detection sit three points apart, and twenty-four parents cannot separate them at
    // any panel size — the two panels do in fact rank that pair differently. A test that demanded
    // a stable total ordering would be demanding the panel invent a resolution it does not have,
    // and would pass only for an estimator that ignored the parent structure.
    let gate = gate4();
    let resolved = |p: f64| !(0.10..=0.90).contains(&p);
    let mut checked = 0;
    for (i, (left, _)) in CAPABILITIES.iter().enumerate() {
        for (right, _) in CAPABILITIES.iter().skip(i + 1) {
            let full = gate
                .exhaustive
                .compare(&capability(left), &capability(right))
                .unwrap();
            if !resolved(full.probability_left_exceeds_right) {
                continue;
            }
            let small = gate
                .adaptive
                .compare(&capability(left), &capability(right))
                .unwrap();
            assert!(
                resolved(small.probability_left_exceeds_right),
                "{left} vs {right}: the panel lost a difference the sweep resolved ({})",
                small.headline()
            );
            assert_eq!(
                full.probability_left_exceeds_right > 0.5,
                small.probability_left_exceeds_right > 0.5,
                "{left} vs {right}: the panel reversed a resolved difference"
            );
            checked += 1;
        }
    }
    assert!(checked >= 2, "only {checked} pairs were resolvable at all");
}

#[test]
fn the_adaptive_panel_agrees_with_the_exhaustive_run_about_which_differences_are_real() {
    let gate = gate4();
    let resolved = |p: f64| !(0.10..=0.90).contains(&p);
    let mut compared = 0;
    for (i, (left, _)) in CAPABILITIES.iter().enumerate() {
        for (right, _) in CAPABILITIES.iter().skip(i + 1) {
            let full = gate
                .exhaustive
                .compare(&capability(left), &capability(right))
                .unwrap();
            let small = gate
                .adaptive
                .compare(&capability(left), &capability(right))
                .unwrap();
            assert_eq!(
                resolved(full.probability_left_exceeds_right),
                resolved(small.probability_left_exceeds_right),
                "{left} vs {right}: exhaustive P={:.3}, adaptive P={:.3}",
                full.probability_left_exceeds_right,
                small.probability_left_exceeds_right
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 3);
}

#[test]
fn the_wide_capability_gap_resolves_and_the_narrow_one_does_not() {
    // The gate's substance: a panel that resolved *everything* would be doing so by ignoring the
    // parent structure, and one that resolved nothing would be useless. Retrieval sits 0.25 above
    // planning and must separate; planning sits 0.03 above leakage-detection, which twenty-four
    // parents cannot resolve, and the honest answer is that it cannot.
    let gate = gate4();
    let wide = gate
        .adaptive
        .compare(&capability("retrieval"), &capability("planning"))
        .unwrap();
    let narrow = gate
        .adaptive
        .compare(&capability("planning"), &capability("leakage-detection"))
        .unwrap();
    assert!(
        wide.probability_left_exceeds_right > 0.95,
        "{}",
        wide.headline()
    );
    assert!(
        (0.10..=0.90).contains(&narrow.probability_left_exceeds_right),
        "{}",
        narrow.headline()
    );
    // And the naive read of the narrow comparison is materially more confident than the honest
    // one, which is the failure this crate exists to prevent.
    assert!(
        (narrow.naive_probability_left_exceeds_right - 0.5).abs()
            > (narrow.probability_left_exceeds_right - 0.5).abs()
    );
}

#[test]
fn the_adaptive_panel_meets_every_coverage_floor() {
    let gate = gate4();
    let audit = gate.adaptive.audit().unwrap();
    assert_eq!(audit.reported(), CAPABILITIES.len());
    assert_eq!(audit.withheld(), 0);
    for entry in &audit.capabilities {
        assert!(entry.coverage.met(), "{}", entry.coverage.describe());
        assert!(entry.coverage.qualifying_parents >= 10);
    }
}

#[test]
fn both_panels_report_far_fewer_effective_trials_than_raw_ones() {
    let gate = gate4();
    for (name, _) in CAPABILITIES {
        for panel in [&gate.exhaustive, &gate.adaptive] {
            let estimate = panel.estimate(&capability(name)).unwrap();
            assert!(
                estimate.effective_trials < estimate.trials as f64,
                "{name}: {} effective from {} raw",
                estimate.effective_trials,
                estimate.trials
            );
            assert!(estimate.inflation > 1.0, "{}", estimate.headline());
        }
    }
    // The exhaustive run buys a great deal less than its trial count suggests: nearly two
    // thousand more trials per capability, but the parents are the same twenty-four.
    let full = gate.exhaustive.estimate(&capability("planning")).unwrap();
    let small = gate.adaptive.estimate(&capability("planning")).unwrap();
    assert!(full.trials > 8 * small.trials);
    assert!(
        full.effective_trials < 4.0 * small.effective_trials,
        "exhaustive bought {:.1} effective trials against the panel's {:.1}",
        full.effective_trials,
        small.effective_trials
    );
}

#[test]
fn the_adaptive_selection_is_reproducible_from_the_same_registry() {
    let registry = Registry::build(0x0006_A7E4);
    let first = adaptive_panel(&registry);
    let second = adaptive_panel(&registry);
    assert_eq!(
        first.audit().unwrap().digest().unwrap(),
        second.audit().unwrap().digest().unwrap()
    );
    assert_eq!(first.ledger().trials(), second.ledger().trials());
}

#[test]
fn a_convenience_panel_of_the_same_size_fails_the_coverage_floor_the_adaptive_one_meets() {
    let gate = gate4();
    let convenience = convenience_panel(&gate.registry);
    let audit = convenience.audit().unwrap();
    assert!(
        audit.withheld() > 0,
        "a panel drawn in registry order somehow met every floor: {}",
        audit.headline()
    );
    // Same budget, an order of magnitude fewer parents touched.
    let convenience_parents: usize = audit.capabilities.iter().map(|c| c.coverage.parents).sum();
    let adaptive_parents: usize = gate
        .adaptive
        .audit()
        .unwrap()
        .capabilities
        .iter()
        .map(|c| c.coverage.parents)
        .sum();
    assert!(
        adaptive_parents > 3 * convenience_parents,
        "adaptive touched {adaptive_parents} parents, convenience touched {convenience_parents}"
    );
}

#[test]
fn the_panel_spreads_its_budget_across_capabilities_rather_than_chasing_the_easiest() {
    let gate = gate4();
    let audit = gate.adaptive.audit().unwrap();
    let counts: Vec<usize> = audit.capabilities.iter().map(|c| c.coverage.trials).collect();
    let smallest = *counts.iter().min().unwrap();
    let largest = *counts.iter().max().unwrap();
    assert!(
        smallest * 3 >= largest,
        "budget split was {counts:?}, which abandons a capability"
    );
}

