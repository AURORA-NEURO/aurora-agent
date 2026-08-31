//! A ranking over partially-ordered capability vectors is allowed to refuse to totally order them.

mod common;

use bioprism_atlas::UnmeasuredReason;
use bioprism_metrics::{
    breakdown, compare, CapabilityGrid, CapabilityVector, DeclaredWeighting, Dominance, GridCell,
    Instability, MetricsError, PartialRanking, RankInstability, SystemId, TotalRanking,
    Unorderable,
};
use common::{cap, grid_of, lower_is_better, point_cell, recorded, unrecorded};

fn system(name: &str, grid: CapabilityGrid) -> CapabilityVector {
    CapabilityVector::new(SystemId::parse(name).expect("named system"), grid)
}

fn two_cell_grid(label: &str, verify: f64, safety: f64) -> CapabilityGrid {
    grid_of(
        label,
        recorded("comparison"),
        vec![
            ("verify.oracle", point_cell(verify, 12)),
            ("safety.escalation", point_cell(safety, 12)),
        ],
    )
}

#[test]
fn a_trade_off_is_incomparable_and_no_tiebreak_collapses_it() {
    let left = system("a", two_cell_grid("a", 0.9, 0.4));
    let right = system("b", two_cell_grid("b", 0.4, 0.9));

    match compare(&left, &right) {
        Dominance::Incomparable {
            because:
                Unorderable::TradeOff {
                    left_better_on,
                    right_better_on,
                },
        } => {
            assert_eq!(left_better_on, vec![cap("verify.oracle")]);
            assert_eq!(right_better_on, vec![cap("safety.escalation")]);
        }
        other => panic!("expected a trade-off, got {other:?}"),
    }
}

#[test]
fn dominance_requires_being_at_least_as_good_everywhere() {
    let strong = system("a", two_cell_grid("a", 0.9, 0.9));
    let weak = system("b", two_cell_grid("b", 0.4, 0.4));
    assert!(matches!(compare(&strong, &weak), Dominance::LeftDominates));
    assert!(matches!(compare(&weak, &strong), Dominance::RightDominates));
}

#[test]
fn two_identical_vectors_are_equivalent_rather_than_arbitrarily_ordered() {
    let left = system("a", two_cell_grid("a", 0.7, 0.7));
    let right = system("b", two_cell_grid("b", 0.7, 0.7));
    assert!(matches!(compare(&left, &right), Dominance::Equivalent));
}

#[test]
fn dominance_respects_a_lower_is_better_direction() {
    let fast = system(
        "a",
        grid_of(
            "a",
            lower_is_better("latency"),
            vec![("tooluse.route", point_cell(100.0, 6))],
        ),
    );
    let slow = system(
        "b",
        grid_of(
            "b",
            lower_is_better("latency"),
            vec![("tooluse.route", point_cell(900.0, 6))],
        ),
    );
    assert!(matches!(compare(&fast, &slow), Dominance::LeftDominates));
}

#[test]
fn an_unmeasured_axis_makes_a_system_incomparable_however_good_its_other_cells() {
    let complete = system("a", two_cell_grid("a", 0.4, 0.4));
    let holed = system(
        "b",
        grid_of(
            "b",
            recorded("comparison"),
            vec![
                ("verify.oracle", point_cell(0.99, 12)),
                (
                    "safety.escalation",
                    GridCell::unmeasured(UnmeasuredReason::NotAttempted),
                ),
            ],
        ),
    );

    match compare(&complete, &holed) {
        Dominance::Incomparable {
            because:
                Unorderable::UnmeasuredAxis {
                    system, capability, ..
                },
        } => {
            assert_eq!(system.as_str(), "b");
            assert_eq!(capability, cap("safety.escalation"));
        }
        other => panic!("a hole cannot be dominated, got {other:?}"),
    }
}

#[test]
fn a_system_with_a_hole_stays_in_the_maximal_set() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.9)),
        system(
            "b",
            grid_of(
                "b",
                recorded("comparison"),
                vec![
                    ("verify.oracle", point_cell(0.1, 12)),
                    (
                        "safety.escalation",
                        GridCell::unmeasured(UnmeasuredReason::NotAttempted),
                    ),
                ],
            ),
        ),
    ])
    .expect("two systems");

    let maximal = ranking.maximal();
    assert_eq!(maximal.len(), 2);
    assert!(!ranking.is_total());
    assert_eq!(ranking.unresolved().len(), 1);
}

#[test]
fn grids_under_different_conditions_are_incomparable_rather_than_ordered() {
    let left = system("a", two_cell_grid("a", 0.9, 0.9));
    let right = system(
        "b",
        grid_of(
            "b",
            unrecorded("comparison"),
            vec![
                ("verify.oracle", point_cell(0.1, 12)),
                ("safety.escalation", point_cell(0.1, 12)),
            ],
        ),
    );

    assert!(matches!(
        compare(&left, &right),
        Dominance::Incomparable {
            because: Unorderable::ConditionsDiffer { .. }
        }
    ));
}

#[test]
fn vectors_over_different_capability_sets_are_incomparable() {
    let left = system("a", two_cell_grid("a", 0.9, 0.9));
    let right = system(
        "b",
        grid_of(
            "b",
            recorded("comparison"),
            vec![("verify.oracle", point_cell(0.9, 12))],
        ),
    );

    match compare(&left, &right) {
        Dominance::Incomparable {
            because: Unorderable::DifferentCapabilitySets { only_left, .. },
        } => assert_eq!(only_left, vec![cap("safety.escalation")]),
        other => panic!("expected a set mismatch, got {other:?}"),
    }
}

#[test]
fn a_ranking_needs_two_systems_and_refuses_a_duplicated_identifier() {
    assert!(matches!(
        PartialRanking::over(vec![system("a", two_cell_grid("a", 0.5, 0.5))]),
        Err(MetricsError::RankingTooSmall(1))
    ));
    assert!(matches!(
        PartialRanking::over(vec![
            system("a", two_cell_grid("a", 0.5, 0.5)),
            system("a", two_cell_grid("a", 0.6, 0.6)),
        ]),
        Err(MetricsError::DuplicateSystem(_))
    ));
}

#[test]
fn totalising_records_every_incomparability_it_overwrote() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.4)),
        system("b", two_cell_grid("b", 0.4, 0.9)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "verification-led triage",
        vec![(cap("verify.oracle"), 3.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let total = ranking.totalise(&weighting).expect("both cells measured");
    assert_eq!(total.leaders(), vec![&SystemId::parse("a").unwrap()]);
    assert!(total.overwrote_a_refusal());
    assert_eq!(total.collapsed.len(), 1);
    assert!(matches!(
        total.collapsed[0].dominance,
        Dominance::Incomparable {
            because: Unorderable::TradeOff { .. }
        }
    ));
    assert!(total.headline().contains("incomparable before weighting"));
}

#[test]
fn a_total_ranking_records_the_weighting_and_its_digest() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.4)),
        system("b", two_cell_grid("b", 0.4, 0.9)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "verification-led triage",
        vec![(cap("verify.oracle"), 3.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let total = ranking.totalise(&weighting).expect("measured");
    assert_eq!(total.weighting_digest, weighting.digest().as_str());
    assert_eq!(total.weighting.intended_use(), "verification-led triage");
}

#[test]
fn every_row_of_a_total_ranking_carries_its_coverage_rather_than_a_bare_number() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.4)),
        system("b", two_cell_grid("b", 0.4, 0.9)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let total = ranking.totalise(&weighting).expect("measured");
    let encoded = serde_json::to_value(&total).expect("serializable");
    for row in encoded["order"].as_array().expect("rows") {
        assert!(row["aggregate"]["coverage"]["contributed"].is_array());
        assert!(row["aggregate"]["coverage"].get("blocking_holes").is_some());
    }
}

#[test]
fn deserialized_partial_rankings_must_reference_each_pair_exactly_once() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.4)),
        system("b", two_cell_grid("b", 0.4, 0.9)),
    ])
    .expect("two systems");
    let mut document = serde_json::to_value(&ranking).expect("serializable");
    document["relations"][0]["left"] = serde_json::Value::String("ghost".into());

    assert!(serde_json::from_value::<PartialRanking>(document).is_err());
}

#[test]
fn deserialized_total_rankings_must_bind_the_weighting_digest_and_row_receipts() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.4)),
        system("b", two_cell_grid("b", 0.4, 0.9)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");
    let total = ranking.totalise(&weighting).expect("both cells measured");
    let encoded = serde_json::to_value(&total).expect("serializable");
    let restored: TotalRanking = serde_json::from_value(encoded.clone()).expect("valid ranking");
    assert_eq!(restored.order.len(), 2);

    let mut bad_digest = encoded.clone();
    bad_digest["weighting_digest"] = serde_json::Value::String("0".repeat(64));
    assert!(serde_json::from_value::<TotalRanking>(bad_digest).is_err());

    let mut bad_receipt = encoded;
    bad_receipt["order"][0]["aggregate"]["weighting_digest"] =
        serde_json::Value::String("0".repeat(64));
    assert!(serde_json::from_value::<TotalRanking>(bad_receipt).is_err());
}

#[test]
fn tied_systems_share_a_rank_and_no_tiebreak_is_applied() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.7, 0.7)),
        system("b", two_cell_grid("b", 0.7, 0.7)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let total = ranking.totalise(&weighting).expect("measured");
    assert_eq!(total.leaders().len(), 2);
    assert!(total.order.iter().all(|row| row.rank == 1));
}

#[test]
fn totalising_refuses_when_a_weighted_capability_is_unmeasured_for_any_system() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.9, 0.9)),
        system(
            "b",
            grid_of(
                "b",
                recorded("comparison"),
                vec![
                    ("verify.oracle", point_cell(0.9, 12)),
                    (
                        "safety.escalation",
                        GridCell::unmeasured(UnmeasuredReason::InaccessibleByPolicy),
                    ),
                ],
            ),
        ),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    assert!(matches!(
        ranking.totalise(&weighting),
        Err(MetricsError::WeightedCapabilityUnmeasured { .. })
    ));
}

#[test]
fn rank_instability_names_the_capability_whose_removal_flips_the_leader() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.90, 0.10)),
        system("b", two_cell_grid("b", 0.20, 0.95)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "verification-led triage",
        vec![(cap("verify.oracle"), 3.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let instability = RankInstability::measure(&ranking, &weighting).expect("measurable");
    assert_eq!(instability.perturbations, 2);
    assert_eq!(
        instability.instability,
        Instability::Measured {
            top_changed_in: 1,
            evaluated: 2
        }
    );
    assert_eq!(instability.instability.fraction(), Some(0.5));
    assert!(instability
        .flipping_capabilities
        .contains(&cap("verify.oracle")));
    assert!(!instability.leader_is_robust());
}

#[test]
fn a_leader_that_wins_on_every_capability_reports_zero_instability() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.90, 0.90)),
        system("b", two_cell_grid("b", 0.10, 0.10)),
    ])
    .expect("two systems");
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");

    let instability = RankInstability::measure(&ranking, &weighting).expect("measurable");
    assert_eq!(instability.top_changed_in, 0);
    assert_eq!(instability.instability.fraction(), Some(0.0));
    assert!(instability.leader_is_robust());
}

/// The defect: `perturbations` went into the denominator whether or not it went into the numerator,
/// so a ranking with nothing to perturb published `0.0` — a leader that survived everything — and
/// `leader_is_robust()` agreed with it.
#[test]
fn nothing_evaluable_is_not_perfect_stability() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.90, 0.90)),
        system("b", two_cell_grid("b", 0.10, 0.10)),
    ])
    .expect("two systems");
    let single = DeclaredWeighting::declare("verification only", vec![(cap("verify.oracle"), 1.0)])
        .expect("valid weighting");

    let instability = RankInstability::measure(&ranking, &single).expect("measurable");

    assert_eq!(
        instability.perturbations, 0,
        "dropping the only weighted capability leaves nothing to weight"
    );
    assert_eq!(
        instability.instability,
        Instability::NotPerturbable {
            weighted_capabilities: 1
        }
    );
    assert_eq!(
        instability.instability.fraction(),
        None,
        "an empty perturbation set has no fraction, and 0.0 claimed it survived all of them"
    );
    assert!(
        !instability.leader_is_robust(),
        "zero checks cannot establish robustness"
    );
}

/// An unevaluable perturbation may not enter a denominator it can never enter the numerator of.
/// Nothing in this crate produces one today — every perturbation is a subset of a weighting that
/// already totalised — so the claim is pinned against the type, which is where it has to hold if a
/// future refusal is ever to reach it.
#[test]
fn an_unevaluable_perturbation_stays_out_of_the_denominator() {
    let partly = Instability::Measured {
        top_changed_in: 1,
        evaluated: 2,
    };
    assert_eq!(
        partly.fraction(),
        Some(0.5),
        "one flip in two evaluable perturbations is one half, not one third of three attempted"
    );

    let none = Instability::NothingEvaluable { attempted: 3 };
    assert_eq!(none.fraction(), None);
    assert!(!none.is_measured());
    assert!(none.headline().contains("no evidence either way"));
}

/// The three states are three JSON shapes. A renderer cannot coerce an absent denominator into a
/// zero, because there is no number in the document to coerce.
#[test]
fn an_instability_without_a_denominator_does_not_serialize_as_a_number() {
    let measured = serde_json::to_value(Instability::Measured {
        top_changed_in: 0,
        evaluated: 4,
    })
    .unwrap();
    let nothing = serde_json::to_value(Instability::NothingEvaluable { attempted: 4 }).unwrap();
    let none = serde_json::to_value(Instability::NotPerturbable {
        weighted_capabilities: 1,
    })
    .unwrap();

    assert_eq!(measured["instability"], "measured");
    assert_eq!(nothing["instability"], "nothing_evaluable");
    assert_eq!(none["instability"], "not_perturbable");
    for absent in ["top_changed_in", "evaluated"] {
        assert!(
            nothing.get(absent).is_none() && none.get(absent).is_none(),
            "an instability with no denominator must not carry {absent}"
        );
    }
    assert_ne!(measured, nothing);
    assert_ne!(nothing, none);

    for state in [measured, nothing, none] {
        let decoded: Instability = serde_json::from_value(state.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), state);
    }
}

#[test]
fn a_deserialized_instability_cannot_claim_an_impossible_denominator() {
    for document in [
        serde_json::json!({
            "instability": "measured",
            "top_changed_in": 3,
            "evaluated": 2
        }),
        serde_json::json!({
            "instability": "nothing_evaluable",
            "attempted": 0
        }),
        serde_json::json!({
            "instability": "not_perturbable",
            "weighted_capabilities": 0
        }),
    ] {
        assert!(serde_json::from_value::<Instability>(document).is_err());
    }
}

#[test]
fn leave_one_out_perturbations_are_deterministic_and_reproducible_from_the_declaration() {
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![
            (cap("verify.oracle"), 1.0),
            (cap("safety.escalation"), 1.0),
            (cap("memory.recall"), 1.0),
        ],
    )
    .expect("valid weighting");

    let first = weighting.leave_one_out().expect("perturbable");
    let second = weighting.leave_one_out().expect("perturbable");
    assert_eq!(first.len(), 3);
    assert_eq!(
        first.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
        second.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>()
    );
    assert!(first
        .iter()
        .all(|(dropped, perturbed)| !perturbed.capabilities().any(|c| c == dropped)));
}

#[test]
fn a_single_capability_weighting_has_no_leave_one_out_perturbation_left() {
    let weighting =
        DeclaredWeighting::declare("verification only", vec![(cap("verify.oracle"), 1.0)])
            .expect("valid weighting");
    assert!(weighting.leave_one_out().expect("perturbable").is_empty());
    assert!(weighting
        .without(&cap("verify.oracle"))
        .expect("no error")
        .is_none());
}

#[test]
fn the_per_capability_breakdown_separates_leading_from_never_measured() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.90, 0.10)),
        system(
            "b",
            grid_of(
                "b",
                recorded("comparison"),
                vec![
                    ("verify.oracle", point_cell(0.20, 12)),
                    (
                        "safety.escalation",
                        GridCell::unmeasured(UnmeasuredReason::NotAttempted),
                    ),
                ],
            ),
        ),
    ])
    .expect("two systems");

    let rows = breakdown(&ranking);
    let safety = rows
        .iter()
        .find(|row| row.capability == cap("safety.escalation"))
        .expect("present");
    assert_eq!(safety.best, vec![SystemId::parse("a").unwrap()]);
    assert_eq!(safety.unmeasured_for, vec![SystemId::parse("b").unwrap()]);

    let verify = rows
        .iter()
        .find(|row| row.capability == cap("verify.oracle"))
        .expect("present");
    assert!(verify.unmeasured_for.is_empty());
}

/// The second reading of the same defect: `best` is a claim about strength, and the row it ships in
/// could not say whether the leader beat the field or stood in it alone. A system that measured the
/// capability and lost is in neither `best` nor `unmeasured_for`, so the two lists cannot be
/// differenced against anything.
#[test]
fn a_lead_over_a_capability_nobody_else_measured_cannot_read_as_beating_the_field() {
    let ranking = PartialRanking::over(vec![
        system("a", two_cell_grid("a", 0.90, 0.10)),
        system(
            "b",
            grid_of(
                "b",
                recorded("comparison"),
                vec![
                    ("verify.oracle", point_cell(0.20, 12)),
                    (
                        "safety.escalation",
                        GridCell::unmeasured(UnmeasuredReason::NotAttempted),
                    ),
                ],
            ),
        ),
    ])
    .expect("two systems");

    let rows = breakdown(&ranking);
    let safety = rows
        .iter()
        .find(|row| row.capability == cap("safety.escalation"))
        .expect("present");
    assert_eq!(
        safety.measured_for,
        vec![SystemId::parse("a").unwrap()],
        "only one system carried a value, so the lead was taken over a field of one"
    );
    assert!(
        safety.lead_is_uncontested(),
        "a leader nobody could be compared against has not beaten anyone"
    );

    let verify = rows
        .iter()
        .find(|row| row.capability == cap("verify.oracle"))
        .expect("present");
    assert_eq!(
        verify.measured_for,
        vec![SystemId::parse("a").unwrap(), SystemId::parse("b").unwrap()],
        "the field includes the system that measured the capability and lost"
    );
    assert!(!verify.lead_is_uncontested());
    assert_eq!(
        verify.best,
        vec![SystemId::parse("a").unwrap()],
        "the loser is in the field and not in the lead"
    );
}

/// `best` on a capability every system left unmeasured names nobody, rather than promoting an
/// arbitrary system to the best of nothing — the rule `crate`-wide that unmeasured is not a score.
#[test]
fn a_capability_nobody_measured_has_no_leader_and_an_empty_field() {
    let unmeasured_everywhere = |label: &str| {
        grid_of(
            label,
            recorded("comparison"),
            vec![
                ("verify.oracle", point_cell(0.5, 12)),
                (
                    "safety.escalation",
                    GridCell::unmeasured(UnmeasuredReason::NotAttempted),
                ),
            ],
        )
    };
    let ranking = PartialRanking::over(vec![
        system("a", unmeasured_everywhere("a")),
        system("b", unmeasured_everywhere("b")),
    ])
    .expect("two systems");

    let rows = breakdown(&ranking);
    let safety = rows
        .iter()
        .find(|row| row.capability == cap("safety.escalation"))
        .expect("present");
    assert!(safety.best.is_empty(), "nobody led a capability nobody ran");
    assert!(safety.measured_for.is_empty());
    assert_eq!(safety.unmeasured_for.len(), 2);
    assert!(safety.lead_is_uncontested());
}

#[test]
fn a_deserialized_weighting_cannot_claim_a_digest_it_does_not_have() {
    let weighting = DeclaredWeighting::declare(
        "balanced",
        vec![(cap("verify.oracle"), 1.0), (cap("safety.escalation"), 1.0)],
    )
    .expect("valid weighting");
    let mut document = serde_json::to_value(&weighting).expect("serializable");
    document["digest"] = serde_json::Value::String("0".repeat(64));

    assert!(serde_json::from_value::<DeclaredWeighting>(document).is_err());
}
