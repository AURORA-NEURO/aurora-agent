//! The seven knobs `crates/bioworlds` and `crates/examples` said were missing.
//!
//! Two things are asserted here and they pull in opposite directions, which is why they live in
//! one file. The knobs have to *do* something — a generated world must now withhold decisive
//! evidence the closure does not protect, and must withhold a fact the caller may not read — and
//! at the same time nothing about the world a default spec generates is allowed to move, because
//! `fixtures/generated/` is committed bytes and `crates/bioworlds` reads one of those files to
//! state a claim about this family.
//!
//! The parity test therefore compares against the committed files rather than against a second
//! call to the generator: a test that only checked `generate(spec) == generate(spec)` would pass
//! just as happily after the default had drifted.

use bioprism_fiber::{compile, FiberError, PolicyViolation, Query};
use bioprism_section::InfluenceClass;
use bioprism_world::{validate, World};
use bioprism_worldgen::{
    generate, DeclaredAbsence, DistractorAttachment, EventSpec, PolicySpec, Skeleton, TagStyle,
    WorldSpec, CENTRAL_LAB_CONFIRMATION, LOCAL_LAB_VALUE,
};
use std::path::PathBuf;

const CENTRAL_LAB_FACT: &str = "fact.central_lab";
const LOCAL_LAB_FACT: &str = "fact.local_lab";

fn committed(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/generated")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn pretty(value: &serde_json::Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("generated documents are finite");
    text.push('\n');
    text
}

fn build(spec: &WorldSpec) -> (World, Query) {
    let generated = generate(spec);
    let world = World::from_json(generated.world).expect("the generated world loads");
    let query = Query::from_json(generated.query).expect("the generated query loads");
    (world, query)
}

fn compiled(spec: &WorldSpec) -> bioprism_fiber::CompileOutput {
    let (world, query) = build(spec);
    compile(&world, &query).expect("the generated world compiles")
}

/// The guarantee every other test in this file is allowed to move around.
#[test]
fn a_default_spec_generates_the_world_it_generated_before() {
    for (spec, world_file, query_file) in [
        (
            WorldSpec::reference_like(750),
            "reference_like_world.json",
            "reference_like_query.json",
        ),
        (
            WorldSpec::discriminating(750),
            "discriminating_world.json",
            "discriminating_query.json",
        ),
    ] {
        let generated = generate(&spec);
        assert_eq!(
            pretty(&generated.world),
            committed(world_file),
            "{world_file} moved; every new knob must default to the behaviour it replaced"
        );
        assert_eq!(pretty(&generated.query), committed(query_file), "{query_file}");
    }
}

/// A spec serialised before the knobs existed still names the same world.
#[test]
fn a_spec_written_without_the_new_fields_still_deserialises_to_the_old_behaviour() {
    let legacy = r#"{
        "world_id": "generated-reference-like-v1",
        "subjects": 4,
        "distractors": 750,
        "relay_depth": 0,
        "attachment": "hub",
        "tag_style": "distinct",
        "leakage": ["identity", "site", "temporal", "preprocessing"],
        "seed": 20260808
    }"#;
    let spec: WorldSpec = serde_json::from_str(legacy).expect("the pre-knob shape deserialises");
    assert_eq!(pretty(&generate(&spec).world), committed("reference_like_world.json"));
}

#[test]
fn the_same_spec_and_seed_produce_the_same_world_at_every_new_corner() {
    for spec in [
        WorldSpec::external_confirmation(30),
        WorldSpec::policy_restricted(30),
        WorldSpec {
            hypotheses: vec!["split_is_sound".into(), "split_is_contaminated".into()],
            ..WorldSpec::discriminating(30)
        },
    ] {
        assert_eq!(generate(&spec).world, generate(&spec).world);
        let world = World::from_json(generate(&spec).world).expect("valid world");
        let report = validate(&world, &Default::default());
        assert!(
            !report.has_errors(),
            "{} has structural errors: {:#?}",
            spec.world_id,
            report.diagnostics
        );
    }
}

/// The deliverable for `non_protected_temporal_withholding`.
#[test]
fn a_world_can_withhold_a_decisive_variable_without_breaking_the_closure() {
    let out = compiled(&WorldSpec::external_confirmation(40));

    assert_eq!(
        out.certificate.omissions.inaccessible_selected_before_cut,
        vec![CENTRAL_LAB_FACT.to_string()],
        "the cut must withhold the central lab confirmation and nothing else"
    );
    assert!(
        out.protected_closure_satisfied(),
        "and the mandatory closure must survive it: {:?}",
        out.trace.dropped_protected
    );
    assert!(
        !out.certificate.protected_closure.is_empty(),
        "an empty closure would survive trivially"
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::DeferredAcquisition),
        1
    );
    assert!(!out.certificate.manifest.supports_sufficiency_claim());
}

/// The withheld variable is decisive, which is the sharper form of the recorded obstacle.
#[test]
fn the_withheld_variable_is_one_the_target_depends_on() {
    let (world, _) = build(&WorldSpec::external_confirmation(40));
    let readers: Vec<&str> = world
        .factors
        .iter()
        .filter(|factor| {
            factor
                .inputs
                .iter()
                .any(|input| input.as_str() == CENTRAL_LAB_CONFIRMATION)
        })
        .map(|factor| factor.id.as_str())
        .collect();
    assert_eq!(readers, vec!["factor.confirmation_check"]);

    let out = compiled(&WorldSpec::external_confirmation(40));
    assert!(
        out.certificate
            .selected_factors
            .iter()
            .any(|id| id == "factor.confirmation_check"),
        "the check that reads it must be in the compiled slice, or withholding it withholds nothing"
    );
}

/// The control: identically tagged, identically event-managed, released before the cut.
#[test]
fn an_event_managed_unprotected_variable_released_before_the_cut_stays_readable() {
    let out = compiled(&WorldSpec::external_confirmation(40));
    assert!(out.trace.temporal_cut.event_managed().contains(LOCAL_LAB_VALUE));
    assert!(out
        .certificate
        .selected_facts
        .iter()
        .any(|id| id == LOCAL_LAB_FACT));
}

#[test]
fn protecting_the_withheld_variable_turns_the_withholding_into_a_closure_violation() {
    let out = compiled(&WorldSpec::external_confirmation(40).protecting(CENTRAL_LAB_CONFIRMATION));
    assert_eq!(
        out.trace.dropped_protected,
        vec![CENTRAL_LAB_FACT.to_string()],
        "the same cut on the same schedule must now break the closure"
    );
    assert!(!out.protected_closure_satisfied());
}

#[test]
fn moving_the_cut_past_the_release_recovers_the_withheld_variable() {
    let out = compiled(
        &WorldSpec::external_confirmation(40).with_decision_time("2025-05-01T00:00:00Z"),
    );
    assert!(out.certificate.omissions.inaccessible_selected_before_cut.is_empty());
    assert!(out
        .certificate
        .selected_facts
        .iter()
        .any(|id| id == CENTRAL_LAB_FACT));
}

/// The temporal claim must not depend on the knobs that make the benchmark separable.
#[test]
fn the_withheld_set_is_the_same_at_both_structural_corners() {
    let discriminating = WorldSpec::external_confirmation(40);
    let reference_shaped = WorldSpec {
        relay_depth: 0,
        attachment: DistractorAttachment::Hub,
        tag_style: TagStyle::Distinct,
        ..WorldSpec::external_confirmation(40)
    };

    assert_eq!(
        compiled(&discriminating)
            .certificate
            .omissions
            .inaccessible_selected_before_cut,
        compiled(&reference_shaped)
            .certificate
            .omissions
            .inaccessible_selected_before_cut
    );
}

/// The deliverable for `policy_blocked_omission`.
#[test]
fn a_generated_world_can_declare_a_policy_requirement_the_caller_does_not_hold() {
    let out = compiled(&WorldSpec::policy_restricted(40));

    assert_eq!(out.trace.policy.requirements_seen, 1);
    assert_eq!(out.trace.policy.withheld, vec![LOCAL_LAB_FACT.to_string()]);
    assert_eq!(out.trace.policy.unaccepted, vec!["consent-tier-2".to_string()]);
    assert!(out.trace.policy.unverified.is_empty());
    assert!(!out.policy_released_everything());

    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::InaccessibleByPolicy),
        1,
        "the omission must be classed as policy-blocked, not as structurally irrelevant"
    );
    assert!(out.protected_closure_satisfied());
}

#[test]
fn accepting_the_clause_releases_the_fact_the_policy_screen_withheld() {
    let spec = WorldSpec {
        policy: PolicySpec::requiring(LOCAL_LAB_VALUE, "consent-tier-2").accepting("consent-tier-2"),
        ..WorldSpec::policy_restricted(40)
    };
    let out = compiled(&spec);

    assert_eq!(out.trace.policy.requirements_seen, 1);
    assert!(out.policy_released_everything());
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::InaccessibleByPolicy),
        0
    );
    assert!(out
        .certificate
        .selected_facts
        .iter()
        .any(|id| id == LOCAL_LAB_FACT));
}

/// 43.13 forbids trimming protected evidence to fit, whatever the reason for the trim.
#[test]
fn a_policy_requirement_over_a_protected_variable_is_refused_rather_than_trimmed() {
    let spec = WorldSpec {
        policy: PolicySpec::requiring("split_assignment", "consent-tier-2"),
        ..WorldSpec::discriminating(20)
    };
    let (world, query) = build(&spec);

    match compile(&world, &query) {
        Err(FiberError::Policy(PolicyViolation::ProtectedClosureWithheld { fact_ids, clauses })) => {
            assert_eq!(fact_ids, vec!["fact.split".to_string()]);
            assert_eq!(clauses, vec!["consent-tier-2".to_string()]);
        }
        other => panic!("expected a policy refusal, got {other:?}"),
    }
}

#[test]
fn a_clause_the_corpus_never_granted_is_a_conflict_rather_than_a_silent_grant() {
    let spec = WorldSpec {
        policy: PolicySpec {
            accepted: vec!["research-only".into(), "consent-tier-2".into()],
            ..PolicySpec::default()
        },
        ..WorldSpec::discriminating(20)
    };
    let (world, query) = build(&spec);

    assert!(matches!(
        compile(&world, &query),
        Err(FiberError::Policy(PolicyViolation::Conflict { .. }))
    ));
}

#[test]
fn two_mutually_exclusive_terminals_can_be_declared_and_reach_the_target() {
    let spec = WorldSpec {
        hypotheses: vec!["split_is_sound".into(), "split_is_contaminated".into()],
        ..WorldSpec::discriminating(20)
    };
    let out = compiled(&spec);

    for factor_id in [
        "factor.hypothesis.split_is_sound",
        "factor.hypothesis.split_is_contaminated",
        "factor.hypothesis_exclusion",
    ] {
        assert!(
            out.certificate.selected_factors.iter().any(|id| id == factor_id),
            "{factor_id} must be reachable from the target, or the exclusion is decorative"
        );
    }

    assert_eq!(
        out.certificate.oracle.status,
        bioprism_section::OracleStatus::Invalid,
        "declaring an exclusion must not change the verdict: no v0.1 pass reads exclusion_rule"
    );
}

#[test]
fn a_declared_absence_beyond_the_skeleton_becomes_evidence_the_claim_reads() {
    let mut declared = DeclaredAbsence::reference_records();
    declared.push(DeclaredAbsence::new(
        "fact.negative_site_audit",
        "declared_site_audit",
        "no_site_audit_performed",
        &["manifest/qc.md"],
    ));
    let spec = WorldSpec {
        declared_absent: declared,
        ..WorldSpec::discriminating(20)
    }
    .protecting("declared_site_audit");

    let (world, _) = build(&spec);
    let claim = world
        .factors
        .iter()
        .find(|factor| factor.id.as_str() == "factor.claim_support")
        .expect("the claim factor exists");
    assert!(
        claim
            .inputs
            .iter()
            .any(|input| input.as_str() == "declared_site_audit"),
        "a record no check consumes must feed the claim, or it is a fact nothing depends on"
    );

    let out = compiled(&spec);
    assert!(out
        .certificate
        .protected_closure
        .iter()
        .any(|id| id == "fact.negative_site_audit"));
}

/// The alternative skeleton must not cost the oracle its diagnosis.
#[test]
fn the_external_confirmation_skeleton_keeps_the_oracle_diagnostic() {
    let leaky = compiled(&WorldSpec::external_confirmation(20));
    let clean = compiled(
        &WorldSpec::external_confirmation(20)
            .with_world_id("generated-external-confirmation-clean-v1")
            .with_leakage(vec![]),
    );

    assert_eq!(
        leaky.certificate.oracle.witness_kinds(),
        compiled(&WorldSpec::discriminating(20))
            .certificate
            .oracle
            .witness_kinds(),
        "the sixth check must not change which defects the oracle finds"
    );
    assert_eq!(
        clean.certificate.oracle.status,
        bioprism_section::OracleStatus::Valid
    );
}

/// The event knob has to be able to say "no event governs anything", not only "these events do".
#[test]
fn a_world_with_an_empty_release_schedule_withholds_nothing() {
    let spec = WorldSpec {
        events: Vec::new(),
        ..WorldSpec::external_confirmation(20)
    };
    let out = compiled(&spec);
    assert!(out.trace.temporal_cut.event_managed().is_empty());
    assert!(out.certificate.omissions.inaccessible_selected_before_cut.is_empty());
}

#[test]
fn the_default_release_schedule_is_the_one_the_generator_used_to_hard_code() {
    let schedule = EventSpec::reference_release_schedule();
    assert_eq!(schedule.len(), 2);
    assert_eq!(
        schedule[0].produces,
        vec![
            "training_decision_time".to_string(),
            "split_assignment".to_string(),
            "preprocess_fit_scope".to_string()
        ]
    );

    let protected = WorldSpec::reference_protected_variables();
    for variable in &schedule[0].produces {
        assert!(
            protected.contains(variable),
            "{variable} was event-managed and protected, which is the collapse the knobs separate"
        );
    }
    assert!(!protected.contains("future_label_value"));
}

/// The mirror image of `crates/bioworlds`' `every_missing_knob_names_a_field_worldgen_does_not_have`.
///
/// That test asserts these keys are *absent* and is meant to fail the moment one lands, so the
/// enumeration of gaps self-invalidates rather than rotting. Pinning the names here is what makes
/// the two tests talk about the same thing: a field renamed on this side would otherwise leave the
/// other side passing again and the gap list looking accurate.
#[test]
fn every_field_the_gap_list_proposed_is_the_field_that_landed() {
    let spec = serde_json::to_value(WorldSpec::discriminating(1)).expect("WorldSpec serialises");
    let map = spec.as_object().expect("an object");
    for proposed in [
        "events",
        "protected_variables",
        "decision_time",
        "skeleton",
        "hypotheses",
        "declared_absent",
        "policy",
    ] {
        assert!(map.contains_key(proposed), "WorldSpec has no `{proposed}` field");
    }
}

#[test]
fn the_skeleton_knob_selects_the_check_table_rather_than_a_second_generator() {
    assert_eq!(Skeleton::default(), Skeleton::SplitIntegrity);
    assert_eq!(Skeleton::SplitIntegrity.checks().len(), 5);
    assert_eq!(Skeleton::ExternalConfirmation.checks().len(), 6);
    for (left, right) in Skeleton::SplitIntegrity
        .checks()
        .iter()
        .zip(Skeleton::ExternalConfirmation.checks())
    {
        assert_eq!(left, right, "the alternative skeleton extends, never rewrites");
    }
    assert_eq!(
        Skeleton::SplitIntegrity.target(),
        Skeleton::ExternalConfirmation.target()
    );
}
