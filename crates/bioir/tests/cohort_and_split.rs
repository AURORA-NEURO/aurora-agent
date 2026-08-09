//! Invariants of blueprint 25.13, cohort, eligibility and split.

use bioprism_bioir::{
    ChronologicalBoundary, CohortDefinition, CohortError, CohortId, EligibilityRule, Estimand,
    GroupingKey, LeakageFinding, LineageGraph, Observation, ObservationId, Predicate, ProcessKind,
    Quantity, RepeatedMeasures, Specimen, SpecimenId, SplitPlan, SplitUnit, SubjectId, TimeAnchor,
    Truth, UnitOfAnalysis,
};
use bioprism_scope::Timestamp;
use serde_json::json;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("well-formed timestamp")
}

fn oid(text: &str) -> ObservationId {
    ObservationId::parse(text).expect("well-formed observation id")
}

fn subject(text: &str) -> SubjectId {
    SubjectId::parse(text).expect("well-formed subject id")
}

fn sid(text: &str) -> SpecimenId {
    SpecimenId::parse(text).expect("well-formed specimen id")
}

fn cohort(unit: UnitOfAnalysis, grouping: GroupingKey) -> CohortDefinition {
    CohortDefinition {
        id: CohortId::parse("gbm-resection-2026").expect("well-formed cohort id"),
        population: "adults with newly diagnosed glioblastoma".to_string(),
        source_datasets: vec!["site-registry-v3".to_string()],
        rules: vec![EligibilityRule::include(
            "adult",
            Predicate::AttributeAtLeast {
                key: "age".to_string(),
                threshold: 18.0,
            },
        )
        .because("the protocol enrolls adults")],
        time_anchor: TimeAnchor {
            event: "resection".to_string(),
            horizon_days: Some(365),
            censoring_rule: "administrative censoring at last contact".to_string(),
        },
        unit: unit.clone(),
        grouping,
        estimand: Estimand {
            target: "overall survival at 12 months".to_string(),
            unit,
            population: "adults with newly diagnosed glioblastoma".to_string(),
            contrast: None,
            summary: "risk difference".to_string(),
        },
    }
}

fn adult(id: &str, who: &str, site: &str, index: &str) -> Observation {
    Observation::new(oid(id), subject(who), site, ts(index))
        .with_attribute("age", json!(54))
}

#[test]
fn a_split_separating_repeated_measures_of_one_subject_is_invalid() {
    let definition = cohort(
        UnitOfAnalysis::Observation,
        GroupingKey::by([SplitUnit::Subject])
            .with_repeated_measures(RepeatedMeasures::GroupedBySubject),
    );
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-1", "site-a", "2026-02-05T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test");

    let findings = plan.validate(&definition, &assembly, &frame, None);
    assert!(findings.iter().any(|finding| matches!(
        finding,
        LeakageFinding::RepeatedMeasuresSeparated { subject, observations, .. }
            if subject == &self::subject("pt-1") && *observations == 2
    )));
}

#[test]
fn a_split_by_site_that_separates_one_subjects_repeated_measures_is_still_invalid() {
    let definition = cohort(
        UnitOfAnalysis::Observation,
        GroupingKey::by([SplitUnit::Site])
            .with_repeated_measures(RepeatedMeasures::GroupedBySubject),
    );
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-1", "site-b", "2026-02-05T00:00:00Z"),
        adult("obs-3", "pt-2", "site-b", "2026-02-08T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Site)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test")
        .assign(oid("obs-3"), "test");

    let findings = plan.validate(&definition, &assembly, &frame, None);
    assert!(
        findings.iter().all(|finding| !matches!(
            finding,
            LeakageFinding::GroupSeparated { .. }
        )),
        "the site split honours every site"
    );
    assert!(
        findings.iter().any(|finding| matches!(
            finding,
            LeakageFinding::RepeatedMeasuresSeparated { subject, .. }
                if subject == &self::subject("pt-1")
        )),
        "a patient treated at two sites is split by a site-keyed plan"
    );
}

#[test]
fn a_site_held_out_split_keeps_every_row_of_a_site_together() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::by([SplitUnit::Site]));
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-2", "site-a", "2026-01-06T00:00:00Z"),
        adult("obs-3", "pt-3", "site-b", "2026-01-07T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");

    let honest = SplitPlan::new(SplitUnit::Site)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "train")
        .assign(oid("obs-3"), "test");
    assert!(honest
        .validate(&definition, &assembly, &frame, None)
        .is_empty());

    let broken = SplitPlan::new(SplitUnit::Site)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test")
        .assign(oid("obs-3"), "test");
    assert!(broken
        .validate(&definition, &assembly, &frame, None)
        .iter()
        .any(|finding| matches!(
            finding,
            LeakageFinding::GroupSeparated { facet, key, .. }
                if facet == &SplitUnit::Site && key == "site-a"
        )));
}

#[test]
fn declaring_repeated_measures_independent_keeps_the_duplicate_structure_visible() {
    let definition = cohort(
        UnitOfAnalysis::Observation,
        GroupingKey::by([SplitUnit::Observation])
            .with_repeated_measures(RepeatedMeasures::Independent),
    );
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-1", "site-a", "2026-02-05T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Observation)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test");

    let findings = plan.validate(&definition, &assembly, &frame, None);
    assert!(findings.iter().any(|finding| matches!(
        finding,
        LeakageFinding::RepeatedMeasuresDeclaredIndependent { .. }
    )));
    assert!(
        findings.iter().all(|finding| !matches!(
            finding,
            LeakageFinding::RepeatedMeasuresSeparated { .. }
        )),
        "the declaration is honoured, not overruled"
    );
}

#[test]
fn a_frame_declared_one_row_per_subject_reports_the_subject_that_has_two() {
    let definition = cohort(
        UnitOfAnalysis::Subject,
        GroupingKey::by([SplitUnit::Subject])
            .with_repeated_measures(RepeatedMeasures::AtMostOnePerSubject),
    );
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-1", "site-a", "2026-02-05T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "train");

    let findings = plan.validate(&definition, &assembly, &frame, None);
    assert!(findings.iter().any(|finding| matches!(
        finding,
        LeakageFinding::UndeclaredRepeatedMeasures { observations, .. } if *observations == 2
    )));
}

#[test]
fn a_single_row_per_subject_frame_split_by_subject_has_no_findings() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::by([SplitUnit::Subject]));
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-2", "site-b", "2026-01-06T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test");
    assert!(plan
        .validate(&definition, &assembly, &frame, None)
        .is_empty());
}

#[test]
fn splitting_on_observations_while_grouping_by_subject_is_refused_before_any_data() {
    let definition = cohort(UnitOfAnalysis::Observation, GroupingKey::by([SplitUnit::Subject]));
    let plan = SplitPlan::new(SplitUnit::Observation);
    assert_eq!(
        plan.validate_declaration(&definition),
        Err(CohortError::SplitUnitFinerThanGrouping {
            cohort: "gbm-resection-2026".to_string(),
            split_unit: "observation".to_string(),
            grouping: "subject".to_string(),
        })
    );
}

#[test]
fn splitting_by_site_under_a_subject_grouping_is_not_a_declaration_error() {
    let definition = cohort(UnitOfAnalysis::Observation, GroupingKey::by([SplitUnit::Subject]));
    let plan = SplitPlan::new(SplitUnit::Site);
    assert_eq!(
        plan.validate_declaration(&definition),
        Ok(()),
        "site and subject are incomparable, so only the data can decide"
    );
}

#[test]
fn exclusion_counts_partition_the_screened_frame() {
    let mut definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    definition.rules.push(
        EligibilityRule::exclude(
            "prior-radiation",
            Predicate::AttributeEquals {
                key: "prior_radiation".to_string(),
                value: json!(true),
            },
        )
        .because("prior radiation confounds the outcome"),
    );
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z")
            .with_attribute("prior_radiation", json!(false)),
        adult("obs-2", "pt-2", "site-a", "2026-01-06T00:00:00Z")
            .with_attribute("prior_radiation", json!(true)),
        Observation::new(oid("obs-3"), subject("pt-3"), "site-b", ts("2026-01-07T00:00:00Z"))
            .with_attribute("age", json!(11))
            .with_attribute("prior_radiation", json!(false)),
    ];

    let assembly = definition.assemble(&frame).expect("rules execute");
    assert_eq!(assembly.screened, 3);
    assert_eq!(assembly.included, vec![oid("obs-1")]);
    assert_eq!(assembly.exclusion_counts().get("adult"), Some(&1));
    assert_eq!(assembly.exclusion_counts().get("prior-radiation"), Some(&1));
    assert!(assembly.reconciles());
}

#[test]
fn a_missing_attribute_makes_eligibility_undecidable_rather_than_false() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    let frame = vec![Observation::new(
        oid("obs-1"),
        subject("pt-1"),
        "site-a",
        ts("2026-01-05T00:00:00Z"),
    )];

    let assembly = definition.assemble(&frame).expect("rules execute");
    assert!(assembly.included.is_empty());
    assert!(assembly.excluded.is_empty(), "an unknown age is not an exclusion");
    assert_eq!(assembly.undecidable.get(&oid("obs-1")), Some(&"adult".to_string()));
    assert!(assembly.reconciles());
}

#[test]
fn kleene_conjunction_is_false_when_one_operand_is_false_even_if_another_is_unknown() {
    let observation = Observation::new(
        oid("obs-1"),
        subject("pt-1"),
        "site-a",
        ts("2026-01-05T00:00:00Z"),
    )
    .with_attribute("age", json!(11));

    let mixed = Predicate::All {
        of: vec![
            Predicate::AttributeAtLeast {
                key: "age".to_string(),
                threshold: 18.0,
            },
            Predicate::AttributeEquals {
                key: "absent".to_string(),
                value: json!(true),
            },
        ],
    };
    assert_eq!(mixed.evaluate(&observation), Truth::False);

    let only_unknown = Predicate::All {
        of: vec![Predicate::AttributeEquals {
            key: "absent".to_string(),
            value: json!(true),
        }],
    };
    assert_eq!(only_unknown.evaluate(&observation), Truth::Unknown);
    assert_eq!(only_unknown.evaluate(&observation).negate(), Truth::Unknown);
    assert_eq!(
        Predicate::AttributePresent {
            key: "absent".to_string()
        }
        .evaluate(&observation),
        Truth::False,
        "presence is the one test that decides on missingness itself"
    );
}

#[test]
fn an_estimand_targeting_a_different_unit_than_the_cohort_is_refused() {
    let mut definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    definition.estimand.unit = UnitOfAnalysis::Lesion;
    assert_eq!(
        definition.validate(),
        Err(CohortError::EstimandUnitMismatch {
            cohort: "gbm-resection-2026".to_string(),
            cohort_unit: "subject".to_string(),
            estimand_unit: "lesion".to_string(),
        })
    );
}

#[test]
fn a_cohort_with_no_executable_rules_is_refused() {
    let mut definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    definition.rules.clear();
    assert_eq!(
        definition.validate(),
        Err(CohortError::NoRules {
            cohort: "gbm-resection-2026".to_string()
        })
    );
}

#[test]
fn aliquots_of_one_block_split_across_folds_are_found_through_the_lineage_graph() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("blk-1"),
            subject("pt-1"),
            ts("2026-01-01T00:00:00Z"),
            "left temporal lobe",
            "FFPE block",
            Quantity::new(10.0, "mL"),
        ))
        .expect("root inserts");
    for name in ["blk-1.s1", "blk-1.s2"] {
        graph
            .insert(Specimen::derived(
                sid(name),
                sid("blk-1"),
                ProcessKind::Split,
                ts("2026-01-02T00:00:00Z"),
                "FFPE block",
                Quantity::new(2.0, "mL"),
            ))
            .expect("section inserts");
    }

    let definition = cohort(UnitOfAnalysis::Specimen, GroupingKey::default());
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z").with_specimen(sid("blk-1.s1")),
        adult("obs-2", "pt-9", "site-a", "2026-01-06T00:00:00Z").with_specimen(sid("blk-1.s2")),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Specimen)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test");

    let findings = plan.validate(&definition, &assembly, &frame, Some(&graph));
    assert!(
        findings.iter().any(|finding| matches!(
            finding,
            LeakageFinding::SharedMaterialAcrossFolds { ancestor, .. } if ancestor == &sid("blk-1")
        )),
        "two subject identifiers over one block still leak"
    );
}

#[test]
fn omitting_the_lineage_graph_is_reported_rather_than_passed_over() {
    let definition = cohort(UnitOfAnalysis::Specimen, GroupingKey::default());
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z").with_specimen(sid("blk-1.s1")),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Specimen).assign(oid("obs-1"), "train");

    assert!(plan
        .validate(&definition, &assembly, &frame, None)
        .contains(&LeakageFinding::LineageUnavailable {
            material_backed_observations: 1
        }));
}

#[test]
fn a_chronological_split_rejects_a_late_record_placed_in_the_earlier_fold() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-2", "site-a", "2026-09-05T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "train")
        .with_boundary(ChronologicalBoundary {
            earlier: bioprism_bioir::Fold::new("train"),
            later: bioprism_bioir::Fold::new("test"),
            at: ts("2026-06-01T00:00:00Z"),
        });

    assert!(plan
        .validate(&definition, &assembly, &frame, None)
        .iter()
        .any(|finding| matches!(
            finding,
            LeakageFinding::ChronologicalBoundaryViolated { observation, .. }
                if observation == &oid("obs-2")
        )));
}

#[test]
fn a_cohort_member_with_no_fold_assignment_is_reported() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        adult("obs-2", "pt-2", "site-a", "2026-01-06T00:00:00Z"),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject).assign(oid("obs-1"), "train");

    let findings = plan.validate(&definition, &assembly, &frame, None);
    assert!(findings.contains(&LeakageFinding::UnassignedObservation {
        observation: oid("obs-2")
    }));
    assert!(plan.fold_for(&oid("obs-2")).is_err());
}

#[test]
fn a_fold_assignment_for_an_excluded_record_is_reported() {
    let definition = cohort(UnitOfAnalysis::Subject, GroupingKey::default());
    let frame = vec![
        adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z"),
        Observation::new(oid("obs-2"), subject("pt-2"), "site-a", ts("2026-01-06T00:00:00Z"))
            .with_attribute("age", json!(9)),
    ];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Subject)
        .assign(oid("obs-1"), "train")
        .assign(oid("obs-2"), "test");

    assert!(plan
        .validate(&definition, &assembly, &frame, None)
        .contains(&LeakageFinding::AssignmentOutsideCohort {
            observation: oid("obs-2")
        }));
}

#[test]
fn a_record_that_cannot_be_keyed_by_the_split_unit_is_reported_not_skipped() {
    let definition = cohort(UnitOfAnalysis::Specimen, GroupingKey::default());
    let frame = vec![adult("obs-1", "pt-1", "site-a", "2026-01-05T00:00:00Z")];
    let assembly = definition.assemble(&frame).expect("rules execute");
    let plan = SplitPlan::new(SplitUnit::Specimen).assign(oid("obs-1"), "train");

    assert!(plan
        .validate(&definition, &assembly, &frame, None)
        .contains(&LeakageFinding::UnkeyableObservation {
            observation: oid("obs-1"),
            facet: SplitUnit::Specimen,
        }));
}
