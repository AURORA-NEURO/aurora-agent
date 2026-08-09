//! End-to-end and adversarial invariants for the policy fiber.
//!
//! Blueprint 43.33's evaluation program asks for information-flow noninterference tests, prompt
//! and cache leakage scans, declassification attack tests and retention/deletion propagation.
//! These are those tests, written against the public API only — nothing here reaches into a
//! module's internals, because a policy layer that can only be verified from inside is not one an
//! independent reviewer can check (36.01 release gate 7).

use bioprism_policy::{
    check_flow, derive, propose_transport, Authority, Channel, Classification, Clearance, Consent,
    Decision, DeclassificationRegistry, DeclassificationRule, ExecutionMode, ExportPolicy,
    Jurisdiction, Obligation, PolicyLabel, PolicyLattice, PolicyRule, Principal, Purpose,
    PurposeSet, RedactionPlan, RedactionRule, Refusal, Replacement, Request, Residency, Retention,
};
use bioprism_scope::{MappingCheck, MappingKind, ScopeKey, Timestamp};
use bioprism_section::{InfluenceClass, UnresolvedObligation};
use bioprism_world::{Fact, World};
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::json;

fn now() -> Timestamp {
    Timestamp::parse("2026-08-08T00:00:00Z").expect("fixture timestamp parses")
}

fn fact(id: &str, scope: serde_json::Value, tags: &[&str]) -> Fact {
    Fact::from_json(&json!({
        "id": id,
        "provides": "some_variable",
        "value": 1,
        "scope": scope,
        "tags": tags,
        "provenance": ["fixture"],
    }))
    .expect("fixture fact parses")
}

fn public_scope() -> ScopeKey {
    ScopeKey::new().exact("cohort", "PUBLIC").exact("residency", "eu")
}

fn controlled_scope() -> ScopeKey {
    ScopeKey::new()
        .exact("cohort", "CONTROLLED")
        .exact("residency", "eu")
}

/// A lattice with one public cohort and one controlled cohort, both resident in the EU.
fn two_cohort_lattice() -> PolicyLattice {
    PolicyLattice::new()
        .with_rule(
            PolicyRule::new(
                "rule.public-cohort",
                1,
                public_scope(),
                PolicyLabel::public()
                    .with_purposes(PurposeSet::of([
                        Purpose::ResearchAnalysis,
                        Purpose::BenchmarkPublication,
                    ]))
                    .with_residency(Residency::only(["eu"])),
            )
            .under_consent(Consent::new(
                "consent.public.v1",
                PurposeSet::of([Purpose::ResearchAnalysis, Purpose::BenchmarkPublication]),
            )),
        )
        .expect("rule registers")
        .with_rule(
            PolicyRule::new(
                "rule.controlled-cohort",
                1,
                controlled_scope(),
                PolicyLabel::public()
                    .with_classification(Classification::ControlledGenomicOrImaging)
                    .with_compartment("genomic")
                    .with_export(ExportPolicy::NoExport)
                    .with_residency(Residency::only(["eu"]))
                    .with_min_cell_size(11),
            )
            .under_consent(Consent::new(
                "consent.controlled.v2",
                PurposeSet::of([Purpose::ResearchAnalysis]),
            )),
        )
        .expect("rule registers")
}

fn low_principal() -> Principal {
    Principal::new("p.low", "analyst", "eu")
        .with_clearance(Clearance::up_to(Classification::PublicAggregate))
}

fn cleared_principal() -> Principal {
    Principal::new("p.genomics", "investigator", "eu").with_clearance(
        Clearance::up_to(Classification::ControlledGenomicOrImaging).cleared_for("genomic"),
    )
}

#[test]
fn a_low_request_compiles_the_same_content_whether_or_not_controlled_evidence_exists() {
    let lattice = two_cohort_lattice();
    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::ModelPrompt,
        now(),
    );

    let open = fact("fact.open", json!({"cohort": "PUBLIC", "residency": "eu"}), &[]);
    let secret = fact(
        "fact.secret",
        json!({"cohort": "CONTROLLED", "residency": "eu"}),
        &[],
    );

    let without_high = lattice.screen([&open], &request);
    let with_high = lattice.screen([&open, &secret], &request);

    let ids_without: Vec<&str> = without_high.facts().map(|f| f.id.as_str()).collect();
    let ids_with: Vec<&str> = with_high.facts().map(|f| f.id.as_str()).collect();

    assert_eq!(ids_without, ids_with, "high evidence must not change the low view");
    assert_eq!(without_high.derived_label(), with_high.derived_label());
    assert_eq!(without_high.obligations(), with_high.obligations());

    assert!(without_high.is_complete());
    assert!(!with_high.is_complete());
}

#[test]
fn the_existence_of_a_refusal_is_reported_because_silent_omission_is_the_failure_mode() {
    let lattice = two_cohort_lattice();
    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::ModelPrompt,
        now(),
    );
    let secret = fact(
        "fact.secret",
        json!({"cohort": "CONTROLLED", "residency": "eu"}),
        &[],
    );

    let screening = lattice.screen([&secret], &request);

    let group = screening.trace.omission_group();
    assert_eq!(group.influence, InfluenceClass::InaccessibleByPolicy);
    assert!(!group.influence.supports_sufficiency());
    assert_eq!(group.count, 1);
    assert!(!screening.trace.supports_sufficiency_claim());

    match screening.trace.unresolved_obligations().as_slice() {
        [UnresolvedObligation::PolicyBlocked { detail }] => {
            assert!(detail.contains("fact.secret"));
        }
        other => panic!("expected exactly one policy-blocked obligation, got {other:?}"),
    }
}

#[test]
fn an_untrusted_world_tag_cannot_widen_the_policy_that_governs_a_fact() {
    let lattice = two_cohort_lattice();
    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::PublicArtifact,
        now(),
    );

    let hostile = fact(
        "fact.claims-to-be-public",
        json!({"cohort": "CONTROLLED", "residency": "eu"}),
        &["public", "declassified", "approved_for_export", "no_phi"],
    );

    let decision = lattice.admits_fact(&hostile, &request);

    assert!(!decision.is_admitted());
    assert_eq!(
        lattice.resolve_fact(&hostile).label.classification,
        Classification::ControlledGenomicOrImaging
    );
    assert_eq!(lattice.unregistered_tags(&hostile).len(), 4);
}

#[test]
fn a_tag_registered_as_restrictive_tightens_a_fact_that_its_scope_rule_left_open() {
    let mut lattice = two_cohort_lattice();
    lattice.register_tag(
        "pediatric",
        PolicyLabel::public()
            .with_compartment("pediatric")
            .with_min_cell_size(20),
    );
    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::ModelPrompt,
        now(),
    );

    let plain = fact("fact.plain", json!({"cohort": "PUBLIC", "residency": "eu"}), &[]);
    let peds = fact(
        "fact.peds",
        json!({"cohort": "PUBLIC", "residency": "eu"}),
        &["pediatric"],
    );

    assert!(lattice.admits_fact(&plain, &request).is_admitted());
    assert!(matches!(
        lattice.admits_fact(&peds, &request).refusal(),
        Some(Refusal::CompartmentNotCleared { .. })
    ));
}

#[test]
fn two_separately_admissible_facts_can_combine_into_something_no_site_may_hold() {
    let lattice = PolicyLattice::new()
        .with_rule(PolicyRule::new(
            "rule.eu-only",
            1,
            ScopeKey::new().exact("cohort", "EU"),
            PolicyLabel::public().with_residency(Residency::only(["eu"])),
        ))
        .expect("registers")
        .with_rule(PolicyRule::new(
            "rule.us-only",
            1,
            ScopeKey::new().exact("cohort", "US"),
            PolicyLabel::public().with_residency(Residency::only(["us"])),
        ))
        .expect("registers");

    let eu = fact("fact.eu", json!({"cohort": "EU"}), &[]);
    let us = fact("fact.us", json!({"cohort": "US"}), &[]);
    let request = Request::new(
        Principal::new("p", "analyst", "eu"),
        Purpose::ResearchAnalysis,
        Channel::LocalCompute,
        now(),
    );

    let both = lattice.screen([&eu, &us], &request);
    assert!(
        both.is_complete(),
        "each fact is individually admissible: one is read locally, the other computed at its site"
    );
    let modes: Vec<&ExecutionMode> = both
        .admitted
        .iter()
        .map(|item| &item.admission.mode)
        .collect();
    assert_eq!(
        modes,
        vec![
            &ExecutionMode::Central,
            &ExecutionMode::Local {
                site: Jurisdiction::new("us")
            }
        ]
    );

    assert!(
        both.derived_label().residency.is_nowhere(),
        "any artifact joining the two has no legal home, which is what a compiler must see \
         before it selects both rather than after it has rendered them"
    );
    assert_eq!(
        both.derived_label(),
        derive([
            &lattice.resolve(&ScopeKey::new().exact("cohort", "EU")).label,
            &lattice.resolve(&ScopeKey::new().exact("cohort", "US")).label,
        ])
    );
}

#[test]
fn changing_only_the_purpose_changes_what_a_compilation_may_select() {
    let lattice = two_cohort_lattice();
    let open = fact("fact.open", json!({"cohort": "PUBLIC", "residency": "eu"}), &[]);

    let research = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::ModelPrompt,
        now(),
    );
    let training = Request::new(
        low_principal(),
        Purpose::ModelTraining,
        Channel::ModelPrompt,
        now(),
    );

    assert!(lattice.screen([&open], &research).is_complete());

    let refused = lattice.screen([&open], &training);
    assert!(refused.admitted.is_empty());
    assert_eq!(refused.refused[0].refusal.constraint(), "purpose_not_consented");
}

#[test]
fn withdrawing_consent_removes_evidence_an_otherwise_identical_compilation_admitted() {
    let scope = ScopeKey::new().exact("cohort", "PUBLIC").exact("residency", "eu");
    let open = fact("fact.open", json!({"cohort": "PUBLIC", "residency": "eu"}), &[]);
    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::LocalCompute,
        now(),
    );

    let build = |consent: Consent| {
        PolicyLattice::new()
            .with_rule(
                PolicyRule::new(
                    "rule.public-cohort",
                    1,
                    scope.clone(),
                    PolicyLabel::public().with_residency(Residency::only(["eu"])),
                )
                .under_consent(consent),
            )
            .expect("registers")
    };

    let active = Consent::new(
        "consent.public.v1",
        PurposeSet::of([Purpose::ResearchAnalysis]),
    );
    let withdrawn = active.clone().withdrawn_at(now());

    assert!(build(active).screen([&open], &request).is_complete());

    let after = build(withdrawn);
    let screening = after.screen([&open], &request);
    assert!(screening.admitted.is_empty());
    assert_eq!(screening.refused[0].refusal.constraint(), "consent_withdrawn");
}

#[test]
fn a_policy_change_moves_the_version_that_cache_admissions_are_keyed_to() {
    let scope = ScopeKey::new().exact("cohort", "PUBLIC").exact("residency", "eu");
    let before = PolicyLattice::new()
        .with_rule(PolicyRule::new(
            "rule.public-cohort",
            1,
            scope.clone(),
            PolicyLabel::public()
                .with_residency(Residency::only(["eu"]))
                .with_retention(Retention::Days(7)),
        ))
        .expect("registers");
    let after = PolicyLattice::new()
        .with_rule(PolicyRule::new(
            "rule.public-cohort",
            2,
            scope.clone(),
            PolicyLabel::public()
                .with_residency(Residency::only(["eu"]))
                .with_retention(Retention::Days(1)),
        ))
        .expect("registers");

    let request = Request::new(
        low_principal(),
        Purpose::ResearchAnalysis,
        Channel::Cache,
        now(),
    );

    let stamp = |lattice: &PolicyLattice| {
        lattice
            .admits(&scope, &request)
            .admission()
            .expect("public evidence is cacheable")
            .obligations
            .clone()
    };

    let old = stamp(&before);
    let new = stamp(&after);
    assert_ne!(old, new);
    assert!(old.contains(&Obligation::KeyCacheToPolicyVersion {
        version: before.version()
    }));
    assert!(new.contains(&Obligation::DeleteBy {
        at: "2026-08-09T00:00:00Z".to_string()
    }));
}

#[test]
fn declassification_is_the_only_path_that_gets_controlled_evidence_into_a_prompt() {
    let lattice = two_cohort_lattice();
    let scope = controlled_scope();
    let steward_authority = Authority::new("authority.data-steward");
    let steward = Principal::new("p.steward", "steward", "eu")
        .with_clearance(Clearance::up_to(Classification::RestrictedDualUse).cleared_for("genomic"))
        .holding(steward_authority.clone());

    let prompt = Request::new(
        cleared_principal(),
        Purpose::ResearchAnalysis,
        Channel::ModelPrompt,
        now(),
    );
    assert!(matches!(
        lattice.admits(&scope, &prompt).refusal(),
        Some(Refusal::ChannelCeilingExceeded { .. })
    ));

    let controlled = lattice.resolve(&scope).label;
    let registry = DeclassificationRegistry::new()
        .with_rule(
            DeclassificationRule::new(
                "declass.suppressed-counts",
                1,
                steward_authority,
                controlled.clone(),
                PolicyLabel::public()
                    .with_classification(Classification::PublicAggregate)
                    .with_purposes(PurposeSet::of([Purpose::ResearchAnalysis]))
                    .with_residency(Residency::only(["eu"]))
                    .with_min_cell_size(11),
                "counts below eleven are suppressed before release; residual linkage risk reviewed",
            )
            .releasing("genomic"),
        )
        .expect("rule registers");

    let (released, receipt) = registry
        .apply(
            "declass.suppressed-counts",
            1,
            &controlled,
            &steward,
            now(),
        )
        .expect("the steward may release suppressed counts");

    assert!(released.classification <= Channel::ModelPrompt.ceiling());
    assert_eq!(receipt.principal, "p.steward");
    assert!(check_flow(&released, &controlled).is_ok());
    assert!(
        check_flow(&controlled, &released).is_err(),
        "the reverse move is a downgrade and needs the rule, not an assertion"
    );
}

#[test]
fn an_admitted_fact_can_be_transported_only_as_a_declared_move_with_a_sound_ledger() {
    let lattice = two_cohort_lattice();
    let label = lattice.resolve(&public_scope()).label;

    let pooled = ScopeKey::new().exact("cohort", "POOLED").exact("residency", "eu");
    let mapping = propose_transport(&label, &public_scope(), &pooled, "approved EU pooling")
        .expect("an in-territory pooling is legal");

    assert!(matches!(mapping.kind, MappingKind::Transport { .. }));
    assert_eq!(mapping.check(), MappingCheck::Sound);
    assert!(!mapping.loss.policy_conditions.is_empty());

    let offshore = ScopeKey::new().exact("cohort", "POOLED").exact("residency", "us");
    assert!(matches!(
        propose_transport(&label, &public_scope(), &offshore, "central pooling"),
        Err(Refusal::ResidencyViolation { .. })
    ));
}

#[test]
fn a_federated_admission_carries_the_aggregate_and_certificate_duties_it_depends_on() {
    let lattice = two_cohort_lattice();
    let offshore = Principal::new("p.us", "analyst", "us")
        .with_clearance(Clearance::up_to(Classification::PublicAggregate));
    let request = Request::new(
        offshore,
        Purpose::ResearchAnalysis,
        Channel::FederatedAggregate,
        now(),
    );

    let admission = lattice
        .admits(&public_scope(), &request)
        .admission()
        .cloned()
        .expect("a federated path exists for public EU evidence");

    assert!(matches!(admission.mode, ExecutionMode::Federated { .. }));
    assert!(admission.obligations.contains(&Obligation::AggregatesOnly));
    assert!(admission
        .obligations
        .contains(&Obligation::EmitLocalCertificate));
    assert!(admission.obligations.contains(&Obligation::RecordAccess));
}

#[test]
fn screening_a_generated_world_against_an_empty_lattice_admits_nothing_at_all() {
    let generated = generate(&WorldSpec::reference_like(20));
    let world = World::from_json(generated.world).expect("generated world parses");
    let request = Request::new(
        cleared_principal(),
        Purpose::ResearchAnalysis,
        Channel::LocalCompute,
        now(),
    );

    let screening = PolicyLattice::new().screen(world.facts.iter(), &request);

    assert!(!world.facts.is_empty());
    assert!(screening.admitted.is_empty());
    assert_eq!(screening.refused.len(), world.facts.len());
    assert!(screening
        .refused
        .iter()
        .all(|item| item.refusal.constraint() == "unlabelled_evidence"));
    assert_eq!(screening.trace.omission_group().count, world.facts.len());
}

#[test]
fn labelling_a_generated_world_admits_it_under_the_right_purpose_and_refuses_under_another() {
    let generated = generate(&WorldSpec::reference_like(20));
    let world = World::from_json(generated.world).expect("generated world parses");
    let cohort = ScopeKey::new().exact("cohort", "RG-GEN-001");

    let lattice = PolicyLattice::new()
        .with_rule(
            PolicyRule::new(
                "rule.generated-cohort",
                1,
                cohort,
                PolicyLabel::public()
                    .with_classification(Classification::InstitutionalConfidential)
                    .with_residency(Residency::only(["eu"]))
                    .with_export(ExportPolicy::AggregatesOnly),
            )
            .under_consent(Consent::new(
                "consent.generated.v1",
                PurposeSet::of([Purpose::MethodDevelopment]),
            )),
        )
        .expect("registers");

    let method_dev = Request::new(
        cleared_principal(),
        Purpose::MethodDevelopment,
        Channel::LocalCompute,
        now(),
    );
    let publication = Request::new(
        cleared_principal(),
        Purpose::BenchmarkPublication,
        Channel::PublicArtifact,
        now(),
    );

    let permitted = lattice.screen(world.facts.iter(), &method_dev);
    assert_eq!(permitted.admitted.len(), world.facts.len());
    assert!(permitted.trace.supports_sufficiency_claim());

    let refused = lattice.screen(world.facts.iter(), &publication);
    assert!(refused.admitted.is_empty());
    assert!(!refused.trace.supports_sufficiency_claim());
}

#[test]
fn a_redacted_view_of_admitted_evidence_stays_distinguishable_from_an_unredacted_one() {
    use bioprism_section::EvidenceCapsule;

    let capsules = [EvidenceCapsule::from_raw_fact(&json!({
        "id": "fact.dob",
        "provides": "date_of_birth",
        "value": "2019-04-11",
        "scope": {"cohort": "PEDS"},
        "tags": ["pediatric"],
        "provenance": ["ehr"],
    }))];
    let label = PolicyLabel::public().with_compartment("pediatric");

    let plan = RedactionPlan::new()
        .with_rule(RedactionRule::new(
            "redact.peds-dob",
            1,
            ["pediatric"],
            Replacement::Generalized {
                to: "age_band_5y".to_string(),
            },
            "exact dates of birth identify individuals in a paediatric cohort",
        ))
        .expect("rule registers");

    let redacted = plan.apply(&capsules, &label);
    let untouched = RedactionPlan::new().apply(&capsules, &label);

    assert!(redacted.is_redacted());
    assert!(redacted.was_evaluated());
    assert!(!untouched.was_evaluated());
    assert_ne!(redacted.items[0].value, untouched.items[0].value);
    assert_eq!(redacted.items.len(), untouched.items.len());
    assert_eq!(redacted.receipts[0].rule_id, "redact.peds-dob");
}

#[test]
fn every_channel_is_judged_independently_for_the_same_principal_and_purpose() {
    let lattice = two_cohort_lattice();
    let base = Request::new(
        cleared_principal(),
        Purpose::ResearchAnalysis,
        Channel::LocalCompute,
        now(),
    );

    let outcomes: Vec<(Channel, bool)> = Channel::ALL
        .into_iter()
        .map(|channel| {
            (
                channel,
                matches!(
                    lattice.admits(&controlled_scope(), &base.through(channel)),
                    Decision::Admit(_)
                ),
            )
        })
        .collect();

    assert_eq!(
        outcomes,
        vec![
            (Channel::LocalCompute, true),
            (Channel::ModelPrompt, false),
            (Channel::Cache, false),
            (Channel::FederatedAggregate, false),
            (Channel::PublicArtifact, false),
        ],
        "controlled evidence may be computed on locally and nowhere else"
    );
}
