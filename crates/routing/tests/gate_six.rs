//! Gate 6, end to end, on a structural panel.
//!
//! `00_START_HERE/04_CRITICAL_PATH.md` sets the bar: choose between at least three architecture
//! configurations on unseen tasks, and compare against a fixed default, a largest-model default,
//! and an oracle retrospective selector. These tests build a 48-task panel by varying the four
//! structural knobs of `bioprism-worldgen` (blueprint 43.39) independently, run the whole thing
//! leave-one-out, and pin the properties that make the resulting numbers meaningful.
//!
//! The panel is chosen so the answer is not known in advance. Distractors attach either to a leaf
//! hub or next to the targets; decisive evidence sits either adjacent to the targets or behind a
//! relay chain; distractor tags are either lexically distinct or camouflaged into the protected
//! vocabulary; and the split either leaks or does not. Those choices are exactly the ones that
//! decide whether a depth-limited walk or a BM25 retriever can work at all.
//!
//! Three of these tests pin results that do *not* flatter the router, and they are the reason the
//! rest can be believed:
//! `the_compiler_wins_every_task_so_this_panel_alone_cannot_test_conditioning` records that with
//! the compiler approved there is no conditioning to demonstrate at all;
//! `with_the_whole_regime_withheld_the_router_abstains_everywhere_and_captures_nothing` records
//! that this router interpolates and does not extrapolate; and
//! `the_confidence_score_is_underconfident_and_the_calibration_curve_shows_it` records that its
//! confidence number is not yet a usable probability.
//!
//! Run with `--nocapture` to read the full markdown reports.

use bioprism_fiber::Query;
use bioprism_routing::{
    lab, ApprovedSet, Architecture, Comparator, EvidenceLedger, Holdout, LabSettings, RoutingError,
    RoutingPolicy, RoutingReport, RoutingVerdict, Task,
};
use bioprism_world::World;
use bioprism_worldgen::{generate, DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};
use std::collections::BTreeSet;
use std::sync::OnceLock;

fn task(task_id: &str, spec: &WorldSpec) -> Task {
    let generated = generate(spec);
    Task::new(
        task_id,
        World::from_json(generated.world).expect("worldgen emits a valid world"),
        Query::from_json(generated.query).expect("worldgen emits a valid query"),
    )
    .expect("task identifier is non-empty")
}

/// Every combination of the structural knobs, at three scales.
fn panel() -> &'static [Task] {
    static PANEL: OnceLock<Vec<Task>> = OnceLock::new();
    PANEL.get_or_init(|| {
        let mut tasks = Vec::new();
        for distractors in [24usize, 96, 240] {
            for relay_depth in [0usize, 3] {
                for (attachment_name, attachment) in [
                    ("leafhub", DistractorAttachment::Hub),
                    ("neartarget", DistractorAttachment::NearTarget),
                ] {
                    for (tag_name, tag_style) in [
                        ("distinct", TagStyle::Distinct),
                        ("camouflaged", TagStyle::Camouflaged),
                    ] {
                        for (leak_name, leakage) in [
                            ("leaky", LeakageMechanism::ALL.to_vec()),
                            ("clean", Vec::new()),
                        ] {
                            let task_id = format!(
                                "d{distractors}-r{relay_depth}-{attachment_name}-{tag_name}-{leak_name}"
                            );
                            let spec = WorldSpec {
                                world_id: task_id.clone(),
                                subjects: 4,
                                distractors,
                                relay_depth,
                                attachment,
                                tag_style,
                                leakage,
                                seed: 20_260_808,
                            };
                            tasks.push(task(&task_id, &spec));
                        }
                    }
                }
            }
        }
        tasks
    })
}

fn approved_with_compiler() -> ApprovedSet {
    ApprovedSet::reference_panel()
}

/// The compiler removed, so the router must choose among architectures whose best setting really
/// does depend on the world's shape.
fn approved_without_compiler() -> ApprovedSet {
    ApprovedSet::new([
        Architecture::FullContext,
        Architecture::GraphKHop { depth: 4 },
        Architecture::GraphKHop { depth: 5 },
        Architecture::GraphKHop { depth: 7 },
        Architecture::HypergraphComponent,
        Architecture::QueryGraph,
        Architecture::LexicalTopK { k: 11 },
        Architecture::LexicalTopK { k: 50 },
    ])
    .expect("panel is non-empty")
}

fn settings(approved: ApprovedSet, fixed_default: Architecture, holdout: Holdout) -> LabSettings {
    let policy = RoutingPolicy::defaulting_to(approved, fixed_default.clone())
        .expect("the safe default is approved");
    LabSettings::new(policy, fixed_default)
        .expect("the fixed default is approved")
        .with_holdout(holdout)
}

fn ledger_with_compiler() -> &'static EvidenceLedger {
    static LEDGER: OnceLock<EvidenceLedger> = OnceLock::new();
    LEDGER.get_or_init(|| {
        lab::observe(panel(), &approved_with_compiler()).expect("the panel is non-empty")
    })
}

fn ledger_without_compiler() -> &'static EvidenceLedger {
    static LEDGER: OnceLock<EvidenceLedger> = OnceLock::new();
    LEDGER.get_or_init(|| {
        lab::observe(panel(), &approved_without_compiler()).expect("the panel is non-empty")
    })
}

fn retrospective_winners(ledger: &EvidenceLedger) -> BTreeSet<String> {
    ledger
        .task_ids()
        .into_iter()
        .map(|task_id| {
            ledger
                .best_for_task(task_id)
                .expect("every task was observed")
                .architecture
                .label()
        })
        .collect()
}

fn assert_report_is_internally_consistent(report: &RoutingReport) {
    assert_eq!(report.tasks.len(), panel().len());
    for outcome in report.account.all() {
        assert_eq!(outcome.tasks, panel().len());
        assert!(
            outcome.mean_regret >= -1e-9,
            "{} reported negative regret against the oracle bound",
            outcome.name()
        );
        assert!(
            report.account.oracle.mean_utility >= outcome.mean_utility - 1e-9,
            "{} beat the retrospective bound, which is impossible",
            outcome.name()
        );
    }
    assert_eq!(report.verdict, RoutingVerdict::of(&report.account));
    assert_eq!(
        report.tasks_won + report.tasks_lost + report.tasks_tied,
        report.tasks.len()
    );
}

#[test]
fn the_panel_varies_structure_independently_and_at_scale() {
    let regimes: BTreeSet<String> = panel()
        .iter()
        .map(|task| task.regime().to_string())
        .collect();
    assert!(
        regimes.len() >= 6,
        "the panel collapsed into {} regimes: {regimes:?}",
        regimes.len()
    );
    assert_eq!(panel().len(), 48);
}

/// The measured null result, pinned so it cannot quietly disappear.
///
/// With the compiler in the approved set the retrospective winner is `fiber` on all 48 tasks,
/// across every structural corner and every scale. A router evaluated on this panel therefore has
/// nothing to condition on: the best answer is a constant, and any router that learns the
/// constant reports a perfect captured-gain fraction while demonstrating no sensitivity to
/// structure whatsoever. Reporting that would be flattery.
///
/// This is the same obligation `crates/baseline` discharges when it pins the world on which a
/// BM25 retriever ties the compiler: the honest reading of a result is sometimes that the
/// experiment cannot separate the hypotheses.
#[test]
fn the_compiler_wins_every_task_so_this_panel_alone_cannot_test_conditioning() {
    let winners = retrospective_winners(ledger_with_compiler());
    assert_eq!(
        winners,
        BTreeSet::from(["fiber".to_string()]),
        "the compiler stopped dominating the panel; the null result above needs rewriting"
    );
}

/// With the compiler withheld, the retrospective winner really does move with structure.
#[test]
fn with_the_compiler_withheld_the_best_architecture_depends_on_structure() {
    let winners = retrospective_winners(ledger_without_compiler());
    assert!(
        winners.len() >= 2,
        "one architecture still won every task, so conditioning is untestable here: {winners:?}"
    );
    println!("retrospective winners without the compiler: {winners:?}");
}

#[test]
fn the_most_expensive_default_is_full_context_because_it_exposes_the_whole_world() {
    assert_eq!(
        ledger_with_compiler().most_expensive_architecture(),
        Some(Architecture::FullContext)
    );
}

#[test]
fn no_comparator_except_the_oracle_is_allowed_to_see_the_answer() {
    let report = lab::run(
        panel(),
        &settings(
            approved_with_compiler(),
            Architecture::FiberCompiled,
            Holdout::Task,
        ),
    )
    .expect("the lab runs");

    let cheating: Vec<&'static str> = report
        .account
        .all()
        .iter()
        .filter(|outcome| outcome.comparator.sees_the_answer())
        .map(|outcome| outcome.name())
        .collect();
    assert_eq!(cheating, vec![Comparator::OracleRetrospective.name()]);
}

#[test]
fn evaluating_a_router_on_its_own_task_is_refused_rather_than_scored() {
    let policy = RoutingPolicy::defaulting_to(approved_with_compiler(), Architecture::FullContext)
        .expect("the safe default is approved");
    let leaked = panel().first().expect("the panel is non-empty");

    let error = policy
        .route_unseen(
            &leaked.task_id,
            &leaked.fingerprint(),
            ledger_with_compiler(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        RoutingError::EvidenceLeak {
            task: leaked.task_id.clone()
        }
    );
}

/// The honest null: against the compiler as the shipped default there is nothing to win.
///
/// `fiber` is the retrospective winner on all 48 tasks, so the fixed default already sits exactly
/// on the oracle bound. The router matches it and captures nothing, because nothing was available.
/// The report refuses to express that as a percentage — a fraction of zero achievable gain would
/// read as either 0% or 100% depending on rounding, and neither means anything.
#[test]
fn against_the_compiler_as_the_fixed_default_there_is_no_gain_to_capture() {
    let report = lab::run(
        panel(),
        &settings(
            approved_with_compiler(),
            Architecture::FiberCompiled,
            Holdout::Task,
        ),
    )
    .expect("the lab runs");
    assert_report_is_internally_consistent(&report);

    println!("\n=== scenario 1: fixed default = fiber, compiler approved ===\n");
    println!("{}", report.to_markdown());

    assert_eq!(report.verdict, RoutingVerdict::NoAchievableGain);
    assert_eq!(report.account.captured_gain_fraction(), None);
    assert!(report.account.achievable_gain().abs() < 1e-9);
    assert!(report.headline().contains("cannot demonstrate routing"));
}

/// The fragile default: a graph walk tuned for one structural corner.
///
/// This is the configuration where a router looks best, and it is worth being explicit about why:
/// the default was chosen to be brittle. `graph-5-hop` is the depth that works on the reference
/// world's corner and is inadmissible on half the panel, so beating it is easy and the interesting
/// number is not the win but the distance to the bound — zero, because the compiler is approved
/// and dominates, so the router only has to find one global winner.
#[test]
fn against_a_tuned_graph_walk_the_router_recovers_the_compiler() {
    let report = lab::run(
        panel(),
        &settings(
            approved_with_compiler(),
            Architecture::GraphKHop { depth: 5 },
            Holdout::Task,
        ),
    )
    .expect("the lab runs");
    assert_report_is_internally_consistent(&report);

    println!("\n=== scenario 2: fixed default = graph-5-hop, compiler approved ===\n");
    println!("{}", report.to_markdown());

    assert_eq!(report.verdict, RoutingVerdict::RouterBeatsFixedDefault);
    assert!(report.account.residual_regret().abs() < 1e-9);
    assert_eq!(report.tasks_lost, 0);
}

fn without_compiler(holdout: Holdout) -> RoutingReport {
    lab::run(
        panel(),
        &settings(
            approved_without_compiler(),
            Architecture::GraphKHop { depth: 5 },
            holdout,
        ),
    )
    .expect("the lab runs")
}

/// The conditioning test: no compiler in the approved set, so no architecture wins everywhere.
///
/// Here routing does work. The fixed default `graph-5-hop` is admissible on only half the panel,
/// the router recovers almost all of the retrospective bound, and it never loses a task. The
/// number that carries the claim is the captured-gain fraction, not the win rate: the router wins
/// 30 of 48 tasks and captures 99.8% of what was available, and those two numbers are reported
/// beside each other precisely because they can disagree.
#[test]
fn with_a_familiar_structure_in_evidence_routing_captures_almost_all_of_the_bound() {
    let report = without_compiler(Holdout::Task);
    assert_report_is_internally_consistent(&report);

    println!("\n=== scenario 3a: fixed default = graph-5-hop, compiler withheld, leave-one-task-out ===\n");
    println!("{}", report.to_markdown());

    assert_eq!(report.verdict, RoutingVerdict::RouterBeatsFixedDefault);
    assert_eq!(report.tasks_lost, 0);
    let captured = report
        .account
        .captured_gain_fraction()
        .expect("the tuned graph walk leaves gain on the table");
    assert!(
        captured > 0.9,
        "captured gain fraction fell to {captured}; the headline claim no longer holds"
    );
    assert!(
        report.win_rate() < captured,
        "the win rate happens to understate the capture here; if that reverses, the report's \
         insistence on printing both is doing even more work"
    );
}

/// The negative result, pinned. Under the stricter holdout this router does nothing at all.
///
/// Removing the routed task's whole structural regime removes every observation inside its
/// neighbourhood, because the neighbourhood radius was deliberately set so that a single
/// categorical flip falls outside it. The policy therefore has no eligible architectures, abstains
/// on all 48 tasks, and captures exactly none of the 0.8169 of achievable gain.
///
/// The honest reading is that this is a *nearest-neighbour* router and not a model of structure:
/// it interpolates within regimes it has seen and extrapolates to new ones not at all. Blueprint
/// 43.41's stop rule requires reporting that, so it is a test rather than a footnote, and the
/// report's own headline states it without prompting.
#[test]
fn with_the_whole_regime_withheld_the_router_abstains_everywhere_and_captures_nothing() {
    let report = without_compiler(Holdout::Regime);
    assert_report_is_internally_consistent(&report);

    println!("\n=== scenario 3b: fixed default = graph-5-hop, compiler withheld, leave-one-regime-out ===\n");
    println!("{}", report.to_markdown());

    assert_eq!(report.verdict, RoutingVerdict::RouterMatchesFixedDefault);
    assert_eq!(report.abstention_rate, 1.0);
    assert_eq!(report.oracle_agreement_rate, None);
    assert!(report.account.captured_gain().abs() < 1e-9);
    assert!(report.account.achievable_gain() > 0.5);

    let headline = report.headline();
    assert!(
        headline.contains("did not beat the fixed default"),
        "the report must state the negative result plainly, got `{headline}`"
    );
    assert!(report
        .to_markdown()
        .contains("did not beat the fixed default"));
}

/// The confidence score is measurably *under*-confident on this panel, and the report says so.
///
/// Routed decisions agree with the oracle far more often than their stated confidence claims. That
/// is a real defect of the heuristic in `policy::RoutingPolicy::confidence`, and the right
/// response is to measure and publish it rather than to retune the constants against this panel
/// until the number looks good.
#[test]
fn the_confidence_score_is_underconfident_and_the_calibration_curve_shows_it() {
    let report = without_compiler(Holdout::Task);
    let error = report
        .calibration
        .expected_calibration_error()
        .expect("some tasks were routed");
    assert!(
        error > 0.2,
        "calibration improved to {error}; the honest caveat above should be revisited"
    );

    let agreement = report
        .oracle_agreement_rate
        .expect("some tasks were routed");
    let mean_confidence: f64 = report
        .calibration
        .populated_bins()
        .map(|bin| bin.mean_confidence * bin.decisions as f64)
        .sum::<f64>()
        / report.calibration.decisions as f64;
    assert!(
        mean_confidence < agreement,
        "the direction of the miscalibration flipped: stated {mean_confidence}, observed \
         {agreement}"
    );
}

#[test]
fn every_decision_names_an_approved_architecture_and_abstentions_carry_zero_confidence() {
    let approved = approved_without_compiler();
    let report = lab::run(
        panel(),
        &settings(
            approved.clone(),
            Architecture::GraphKHop { depth: 5 },
            Holdout::Task,
        ),
    )
    .expect("the lab runs");

    for row in &report.tasks {
        assert!(
            approved.contains(&row.router.architecture),
            "task {} routed to unapproved `{}`",
            row.task_id,
            row.router.architecture.label()
        );
        assert_eq!(row.abstained, row.reason.is_abstention());
        if row.abstained {
            assert_eq!(
                row.confidence, 0.0,
                "task {} abstained but claimed confidence",
                row.task_id
            );
            assert_eq!(
                row.router.architecture,
                Architecture::GraphKHop { depth: 5 },
                "abstention must land on the configured safe default"
            );
        }
    }
}

#[test]
fn the_report_serialises_with_the_oracle_bound_attached() {
    let report = lab::run(
        panel(),
        &settings(
            approved_without_compiler(),
            Architecture::GraphKHop { depth: 5 },
            Holdout::Task,
        ),
    )
    .expect("the lab runs");

    let document = report.to_json();
    let comparators: Vec<String> = document["comparators"]
        .as_array()
        .expect("comparators is an array")
        .iter()
        .map(|entry| entry["name"].as_str().expect("named").to_string())
        .collect();
    assert_eq!(
        comparators,
        vec![
            "fixed-default",
            "most-expensive-default",
            "evidence-router",
            "oracle-retrospective"
        ]
    );
    assert!(
        document["captured_gain_fraction"].is_number()
            || document["captured_gain_fraction"].is_null()
    );
    assert!(document["headline"].as_str().expect("a headline").len() > 40);
}
