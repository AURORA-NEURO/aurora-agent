//! The atlas, the unmeasured/measured-poor distinction, and the claim ladder.
//!
//! Blueprint 33 (BioCapability Atlas), 33.01 (score eligibility), 33.19 (benchmark health) and
//! 43.40 (claim ladder). The central claim under test is the one section 33 is built around: a
//! capability with no evidence is a categorically different object from a capability that was
//! measured and performed badly, and no route through this crate turns the first into the second.

use bioprism_atlas::{
    composite, Atlas, AtlasError, CapabilityCell, CapabilityDimension, CapabilityFamily,
    CapabilityId, CapabilityNode, CapabilityOntology, CausalChain, CellRendering, ClaimConstraint,
    ClaimTier, CoverageReport, Detectability, EvidenceRecord, EvidenceStatus, EvidenceTier,
    FailureAxes, FailureLabel, FailureMechanism, FailureRecord, Inconsistency, Inducement,
    LabelDistribution, Measurement, MeasurementDepth, MeasurementFields, OracleTier, RelationKind,
    Reversibility, Severity, TrialOutcome, UnmeasuredReason, WeightingPolicy,
};
use bioprism_ids::{RunId, WorldId};
use bioprism_section::InfluenceClass;
use serde_json::json;

const VERSION: &str = "capability-ontology/2026-08-07";

fn cap(id: &str) -> CapabilityId {
    CapabilityId::parse(id).expect("valid capability identifier")
}

fn world(id: &str) -> WorldId {
    WorldId::parse(id).expect("valid world identifier")
}

fn node(id: &str, family: CapabilityFamily) -> CapabilityNode {
    CapabilityNode::new(cap(id), id, family, CapabilityDimension::Competence)
}

/// `agent` is an interior aggregate over two measurable leaves.
fn ontology() -> CapabilityOntology {
    CapabilityOntology::from_nodes(
        VERSION,
        [
            node("agent", CapabilityFamily::DomainReasoning),
            node("literature", CapabilityFamily::EvidenceAcquisition).with_parent(cap("agent")),
            node("analysis", CapabilityFamily::ToolUse).with_parent(cap("agent")),
        ],
    )
    .unwrap()
}

/// The same hierarchy with a safety capability that constrains the whole agent.
fn guarded_ontology() -> CapabilityOntology {
    CapabilityOntology::from_nodes(
        VERSION,
        [
            node("agent", CapabilityFamily::DomainReasoning),
            node("literature", CapabilityFamily::EvidenceAcquisition).with_parent(cap("agent")),
            node("analysis", CapabilityFamily::ToolUse).with_parent(cap("agent")),
            CapabilityNode::new(
                cap("boundary"),
                "dual-use boundary",
                CapabilityFamily::PrivacyAndSafety,
                CapabilityDimension::Safety,
            )
            .with_relation(RelationKind::SafetyConstraintOn, cap("agent")),
        ],
    )
    .unwrap()
}

fn trial(
    id: &str,
    capability: &str,
    outcome: TrialOutcome,
    tier: EvidenceTier,
    oracle: OracleTier,
) -> EvidenceRecord {
    EvidenceRecord::new(id, cap(capability), VERSION, tier, oracle, outcome)
}

/// Two independent parent worlds, one site, one domain — a public-observed-world measurement.
fn public_world_trials(capability: &str, outcomes: [TrialOutcome; 2]) -> Vec<EvidenceRecord> {
    outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| {
            trial(
                &format!("{capability}-t{index}"),
                capability,
                outcome,
                EvidenceTier::PublicObservedWorld,
                OracleTier::Deterministic,
            )
            .with_parent_world(world(&format!("w{index}")))
            .with_site("site-a")
            .with_domain("oncology")
        })
        .collect()
}

#[test]
fn an_unmeasured_capability_is_never_reported_as_a_low_score() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Fail, TrialOutcome::Fail],
        ))
        .build()
        .unwrap();

    let measured_and_poor = atlas.cell(&cap("literature")).unwrap();
    let never_looked_at = atlas.cell(&cap("analysis")).unwrap();

    assert_eq!(measured_and_poor.score(), Some(0.0));
    assert_eq!(never_looked_at.score(), None);
    assert_ne!(measured_and_poor, never_looked_at);

    assert!(matches!(
        never_looked_at.render(),
        CellRendering::Hole {
            reason: UnmeasuredReason::NotAttempted
        }
    ));
    assert!(matches!(
        measured_and_poor.render(),
        CellRendering::Score { value, .. } if value == 0.0
    ));

    let report = CoverageReport::of(&atlas);
    assert!(report
        .measured
        .iter()
        .all(|entry| entry.capability != cap("analysis")));
    assert!(report
        .holes
        .iter()
        .any(|hole| hole.capability == cap("analysis")));
}

#[test]
fn an_unmeasured_capability_serialises_without_any_score_field() {
    let cell = CapabilityCell::unmeasured(UnmeasuredReason::NotAttempted);
    let value = serde_json::to_value(&cell).unwrap();
    let object = value.as_object().expect("a JSON object");

    assert_eq!(object.get("state").and_then(|v| v.as_str()), Some("unmeasured"));
    assert_eq!(object.len(), 2, "state and reason only: {object:?}");
    for forbidden in ["score", "value", "passes", "failures", "measurement"] {
        assert!(
            !object.contains_key(forbidden),
            "an unmeasured cell must not carry {forbidden}"
        );
    }
}

#[test]
fn a_capability_whose_every_trial_failed_is_measured_and_poor_not_unmeasured() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Fail, TrialOutcome::Fail],
        ))
        .build()
        .unwrap();

    let cell = atlas.cell(&cap("analysis")).unwrap();
    assert!(cell.is_measured());
    assert_eq!(cell.score(), Some(0.0));
    assert_eq!(cell.measurement().unwrap().failures(), 2);
    assert_eq!(cell.unmeasured_reason(), None);
}

#[test]
fn a_capability_whose_every_trial_was_non_evaluable_is_unmeasured_not_zero() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::NonEvaluable, TrialOutcome::NonEvaluable],
        ))
        .build()
        .unwrap();

    let cell = atlas.cell(&cap("analysis")).unwrap();
    assert_eq!(cell.score(), None);
    assert_eq!(
        cell.unmeasured_reason(),
        Some(UnmeasuredReason::AllTrialsNonEvaluable)
    );
}

#[test]
fn a_capability_that_only_ever_abstained_is_a_hole_and_not_a_run_of_failures() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Abstained, TrialOutcome::Abstained],
        ))
        .build()
        .unwrap();

    assert_eq!(
        atlas.cell(&cap("analysis")).unwrap().unmeasured_reason(),
        Some(UnmeasuredReason::AllTrialsAbstained)
    );
}

#[test]
fn abstained_and_non_evaluable_trials_survive_alongside_a_measurement() {
    let mut records = public_world_trials("analysis", [TrialOutcome::Pass, TrialOutcome::Fail]);
    records.push(
        trial(
            "analysis-abstain",
            "analysis",
            TrialOutcome::Abstained,
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
        )
        .with_parent_world(world("w0")),
    );
    records.push(
        trial(
            "analysis-crash",
            "analysis",
            TrialOutcome::NonEvaluable,
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
        )
        .with_parent_world(world("w0")),
    );

    let atlas = Atlas::builder(ontology())
        .evidence_all(records)
        .build()
        .unwrap();
    let measurement = atlas.cell(&cap("analysis")).unwrap().measurement().unwrap();

    assert_eq!(measurement.evaluable(), 2);
    assert_eq!(measurement.score(), 0.5);
    assert_eq!(measurement.abstained(), 1);
    assert_eq!(measurement.non_evaluable(), 1);
    assert_eq!(measurement.excluded(), 2);
}

#[test]
fn a_measurement_cannot_be_deserialised_with_an_empty_denominator() {
    let error = serde_json::from_value::<Measurement>(json!({
        "capability": "analysis",
        "passes": 0,
        "failures": 0,
        "highest_tier": "public_observed_world",
        "strongest_oracle": "deterministic"
    }))
    .expect_err("an empty denominator is not a score of zero");
    assert!(
        error.to_string().contains("zero evaluable trials"),
        "unexpected message: {error}"
    );

    assert!(serde_json::from_value::<CapabilityCell>(json!({
        "state": "measured",
        "measurement": {
            "capability": "analysis",
            "passes": 0,
            "failures": 0,
            "highest_tier": "public_observed_world",
            "strongest_oracle": "deterministic"
        }
    }))
    .is_err());
}

#[test]
fn a_measurement_claiming_more_independent_parents_than_trials_is_refused() {
    let built = Measurement::try_from(MeasurementFields {
        capability: cap("analysis"),
        passes: 1,
        failures: 0,
        non_evaluable: 0,
        abstained: 0,
        superseded: 0,
        independent_parents: 9,
        generated_instances: 0,
        independent_sites: 0,
        domains: 0,
        highest_tier: EvidenceTier::PublicObservedWorld,
        strongest_oracle: OracleTier::Deterministic,
        deterministic_failures: 0,
    });
    assert!(matches!(
        built,
        Err(AtlasError::ImpossibleClusterCount { .. })
    ));
}

#[test]
fn generated_scale_never_inflates_the_effective_size() {
    let mut records = vec![trial(
        "parent",
        "analysis",
        TrialOutcome::Pass,
        EvidenceTier::PublicObservedWorld,
        OracleTier::Deterministic,
    )
    .with_parent_world(world("w0"))];
    for index in 0..50 {
        records.push(
            trial(
                &format!("gen-{index}"),
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::PublicObservedWorld,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0"))
            .generated_instance(),
        );
    }

    let atlas = Atlas::builder(ontology())
        .evidence_all(records)
        .build()
        .unwrap();
    let measurement = atlas.cell(&cap("analysis")).unwrap().measurement().unwrap();

    assert_eq!(measurement.evaluable(), 51);
    assert_eq!(measurement.generated_instances(), 50);
    assert_eq!(measurement.independent_parents(), 1);
    assert_eq!(measurement.effective_size(), 1);
    assert_eq!(measurement.depth(), MeasurementDepth::Clustered);
}

#[test]
fn a_model_judge_cannot_override_a_deterministic_failure() {
    let atlas = Atlas::builder(ontology())
        .evidence(
            trial(
                "shared",
                "analysis",
                TrialOutcome::Fail,
                EvidenceTier::PublicObservedWorld,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0")),
        )
        .evidence(
            trial(
                "shared",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::PublicObservedWorld,
                OracleTier::ModelJudge,
            )
            .with_parent_world(world("w0")),
        )
        .build()
        .unwrap();

    let measurement = atlas.cell(&cap("analysis")).unwrap().measurement().unwrap();
    assert_eq!(measurement.passes(), 0);
    assert_eq!(measurement.failures(), 1);
    assert_eq!(measurement.deterministic_failures(), 1);
    assert_eq!(measurement.superseded(), 1);
    assert_eq!(measurement.strongest_oracle(), OracleTier::Deterministic);
}

#[test]
fn two_oracles_of_equal_authority_that_disagree_are_surfaced_not_arbitrated() {
    let built = Atlas::builder(ontology())
        .evidence(trial(
            "shared",
            "analysis",
            TrialOutcome::Fail,
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
        ))
        .evidence(trial(
            "shared",
            "analysis",
            TrialOutcome::Pass,
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
        ))
        .build();
    assert!(matches!(built, Err(AtlasError::ConflictingEvidence { .. })));
}

#[test]
fn evidence_compiled_under_a_different_ontology_version_is_refused_not_reprojected() {
    let stale = EvidenceRecord::new(
        "t1",
        cap("analysis"),
        "capability-ontology/2025-01-01",
        EvidenceTier::PublicObservedWorld,
        OracleTier::Deterministic,
        TrialOutcome::Pass,
    );
    assert!(matches!(
        Atlas::builder(ontology()).evidence(stale).build(),
        Err(AtlasError::OntologyVersionMismatch { .. })
    ));
}

#[test]
fn a_claim_is_refused_above_the_tier_its_evidence_supports() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Fail],
        ))
        .build()
        .unwrap();

    let permitted = bioprism_atlas::permitted_claim(&atlas, &cap("agent"));
    assert_eq!(permitted, ClaimTier::PublicObservedWorlds);

    assert!(bioprism_atlas::license_claim(&atlas, &cap("agent"), ClaimTier::PublicObservedWorlds)
        .is_ok());
    match bioprism_atlas::license_claim(&atlas, &cap("agent"), ClaimTier::ControlledHiddenMultiSite)
    {
        Err(AtlasError::ClaimAboveEvidence {
            requested,
            permitted,
            ..
        }) => {
            assert_eq!(requested, ClaimTier::ControlledHiddenMultiSite);
            assert_eq!(permitted, ClaimTier::PublicObservedWorlds);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_claim_at_the_no_claim_tier_is_vacuous_and_is_not_licensed() {
    let atlas = Atlas::builder(ontology()).build().unwrap();
    assert!(matches!(
        bioprism_atlas::license_claim(&atlas, &cap("agent"), ClaimTier::NoClaim),
        Err(AtlasError::VacuousClaim { .. })
    ));
}

#[test]
fn an_unmeasured_leaf_blocks_every_claim_about_its_parent() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("literature")),
        ClaimTier::PublicObservedWorlds
    );
    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("agent")),
        ClaimTier::NoClaim,
        "excellent literature synthesis with no executable-analysis coverage earns no overall rank"
    );

    let assessment = bioprism_atlas::assess_claim(&atlas, &cap("agent"));
    assert!(assessment.constraints.iter().any(|constraint| matches!(
        constraint,
        ClaimConstraint::UnmeasuredInSubtree { capability, .. } if capability == "analysis"
    )));
}

#[test]
fn an_out_of_scope_declaration_is_the_only_hole_that_does_not_block_a_claim() {
    let deferred = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .declare_unmeasured(cap("analysis"), UnmeasuredReason::DeferredAcquisition)
        .build()
        .unwrap();
    assert_eq!(
        bioprism_atlas::permitted_claim(&deferred, &cap("agent")),
        ClaimTier::NoClaim
    );

    let scoped = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .declare_unmeasured(cap("analysis"), UnmeasuredReason::OutOfScopeByDeclaredUse)
        .build()
        .unwrap();
    assert_eq!(
        bioprism_atlas::permitted_claim(&scoped, &cap("agent")),
        ClaimTier::PublicObservedWorlds
    );
}

#[test]
fn a_single_parent_world_cannot_license_a_cross_world_claim() {
    let atlas = Atlas::builder(ontology())
        .evidence_all([
            trial(
                "t0",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::CrossDomainPublic,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0")),
            trial(
                "t1",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::CrossDomainPublic,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0")),
        ])
        .declare_unmeasured(cap("literature"), UnmeasuredReason::OutOfScopeByDeclaredUse)
        .build()
        .unwrap();

    let assessment = bioprism_atlas::assess_claim(&atlas, &cap("analysis"));
    assert_eq!(assessment.permitted, ClaimTier::SyntheticStructural);
    assert!(assessment.constraints.iter().any(|constraint| matches!(
        constraint,
        ClaimConstraint::LimitedByMeasurement { detail, .. }
            if detail.contains("independent parent worlds")
    )));
}

#[test]
fn a_measurement_judged_only_by_a_model_judge_cannot_license_a_public_world_claim() {
    let atlas = Atlas::builder(ontology())
        .evidence_all([
            trial(
                "t0",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::PublicObservedWorld,
                OracleTier::ModelJudge,
            )
            .with_parent_world(world("w0")),
            trial(
                "t1",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::PublicObservedWorld,
                OracleTier::ModelJudge,
            )
            .with_parent_world(world("w1")),
        ])
        .declare_unmeasured(cap("literature"), UnmeasuredReason::OutOfScopeByDeclaredUse)
        .build()
        .unwrap();

    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("analysis")),
        ClaimTier::SyntheticStructural
    );
}

#[test]
fn a_failing_safety_constraint_blocks_every_claim_about_the_capability_it_guards() {
    let atlas = Atlas::builder(guarded_ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "boundary",
            [TrialOutcome::Pass, TrialOutcome::Fail],
        ))
        .build()
        .unwrap();

    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("analysis")),
        ClaimTier::NoClaim
    );
    let assessment = bioprism_atlas::assess_claim(&atlas, &cap("agent"));
    assert!(assessment
        .constraints
        .iter()
        .any(|constraint| matches!(constraint, ClaimConstraint::SafetyGate { .. })));
}

#[test]
fn an_unmeasured_safety_constraint_blocks_just_as_hard_as_a_failing_one() {
    let atlas = Atlas::builder(guarded_ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("agent")),
        ClaimTier::NoClaim,
        "an unrun safety check is not a passed safety check"
    );
}

#[test]
fn a_coverage_report_always_carries_a_holes_field_even_when_there_are_none() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "agent",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    let report = CoverageReport::of(&atlas);
    assert!(!report.has_holes());

    let value = serde_json::to_value(&report).unwrap();
    assert!(
        value.as_object().unwrap().contains_key("holes"),
        "the holes field is never elided"
    );
    assert_eq!(value["holes"].as_array().unwrap().len(), 0);
}

#[test]
fn a_hole_names_the_ancestor_claims_it_blocks() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    let report = CoverageReport::of(&atlas);
    let hole = report
        .holes
        .iter()
        .find(|h| h.capability == cap("analysis"))
        .expect("the unmeasured leaf is reported");
    assert!(hole.blocks_claims_for.contains(&cap("agent")));
    assert!(!hole.aggregate);

    let aggregate = report
        .holes
        .iter()
        .find(|h| h.capability == cap("agent"))
        .expect("the interior node has no direct evidence either");
    assert!(aggregate.aggregate);
    assert!(aggregate.blocks_claims_for.is_empty());
}

#[test]
fn coverage_holes_become_unknown_influence_groups_in_the_omission_manifest() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    let manifest = CoverageReport::of(&atlas).omission_manifest();
    assert_eq!(manifest.count_in(InfluenceClass::Unknown), 1);
    assert!(!manifest.supports_sufficiency_claim());
    assert!(!CoverageReport::of(&atlas).coverage_supports_aggregation());
}

#[test]
fn a_hole_closed_by_a_declared_scope_supports_a_sufficiency_claim() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "agent",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .declare_unmeasured(cap("analysis"), UnmeasuredReason::OutOfScopeByDeclaredUse)
        .build()
        .unwrap();

    let report = CoverageReport::of(&atlas);
    let manifest = report.omission_manifest();
    assert_eq!(manifest.count_in(InfluenceClass::Zero), 1);
    assert!(manifest.supports_sufficiency_claim());
    assert!(report.coverage_supports_aggregation());
    assert_eq!(report.debt.closed_by_declaration, 1);
}

#[test]
fn coverage_debt_names_the_families_with_no_evidence_at_all() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    let debt = CoverageReport::of(&atlas).debt;
    assert_eq!(debt.total_capabilities, 3);
    assert_eq!(debt.measured, 1);
    assert_eq!(debt.unmeasured, 2);
    assert!(debt.dark_families.contains(&CapabilityFamily::ToolUse));
    assert!(!debt.dark_families.contains(&CapabilityFamily::EvidenceAcquisition));
    assert!((debt.ratio() - 2.0 / 3.0).abs() < 1e-12);
}

#[test]
fn a_composite_is_refused_while_a_weighted_capability_is_unmeasured() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .build()
        .unwrap();

    let policy = WeightingPolicy::declare(
        "research triage",
        [(cap("literature"), 0.5), (cap("analysis"), 0.5)],
    )
    .unwrap();

    match composite(&atlas, &policy) {
        Err(AtlasError::CompositeIneligible { reason }) => {
            assert!(reason.contains("analysis"), "unexpected reason: {reason}");
            assert!(reason.contains("unmeasured"), "unexpected reason: {reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_composite_refuses_to_weight_two_confounded_capabilities_as_independent() {
    let ontology = CapabilityOntology::from_nodes(
        VERSION,
        [
            node("agent", CapabilityFamily::DomainReasoning),
            node("literature", CapabilityFamily::EvidenceAcquisition)
                .with_parent(cap("agent"))
                .with_relation(RelationKind::ConfoundsWith, cap("analysis")),
            node("analysis", CapabilityFamily::ToolUse).with_parent(cap("agent")),
        ],
    )
    .unwrap();

    let atlas = Atlas::builder(ontology)
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Fail],
        ))
        .build()
        .unwrap();

    let policy = WeightingPolicy::declare(
        "research triage",
        [(cap("literature"), 0.5), (cap("analysis"), 0.5)],
    )
    .unwrap();
    assert!(matches!(
        composite(&atlas, &policy),
        Err(AtlasError::ConfoundedAggregation { .. })
    ));
}

#[test]
fn an_eligible_composite_cannot_outrank_its_weakest_component() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .evidence_all([
            trial(
                "a0",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::PublicObservedWorld,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0")),
            trial(
                "a1",
                "analysis",
                TrialOutcome::Fail,
                EvidenceTier::PublicObservedWorld,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0")),
        ])
        .build()
        .unwrap();

    let policy = WeightingPolicy::declare(
        "research triage",
        [(cap("literature"), 1.0), (cap("analysis"), 1.0)],
    )
    .unwrap();
    let composed = composite(&atlas, &policy).unwrap();

    assert!((composed.value - 0.75).abs() < 1e-12);
    assert_eq!(composed.tier, ClaimTier::SyntheticStructural);
    assert_eq!(composed.weighted_capabilities, 2);
}

#[test]
fn a_weighting_policy_with_a_non_positive_weight_is_refused() {
    assert!(matches!(
        WeightingPolicy::declare("triage", [(cap("literature"), 0.0)]),
        Err(AtlasError::MalformedWeightingPolicy { .. })
    ));
    assert!(matches!(
        WeightingPolicy::declare("triage", []),
        Err(AtlasError::MalformedWeightingPolicy { .. })
    ));
}

#[test]
fn measurement_depth_reflects_independent_structure_rather_than_instance_count() {
    let multi_site = Measurement::try_from(MeasurementFields {
        capability: cap("analysis"),
        passes: 4,
        failures: 0,
        non_evaluable: 0,
        abstained: 0,
        superseded: 0,
        independent_parents: 4,
        generated_instances: 0,
        independent_sites: 3,
        domains: 2,
        highest_tier: EvidenceTier::ControlledHiddenMultiSite,
        strongest_oracle: OracleTier::Deterministic,
        deterministic_failures: 0,
    })
    .unwrap();
    assert_eq!(multi_site.depth(), MeasurementDepth::MultiSiteCrossDomain);

    let single = Measurement::try_from(MeasurementFields {
        capability: cap("analysis"),
        passes: 1,
        failures: 0,
        non_evaluable: 0,
        abstained: 0,
        superseded: 0,
        independent_parents: 1,
        generated_instances: 0,
        independent_sites: 1,
        domains: 1,
        highest_tier: EvidenceTier::ProspectiveWorkflow,
        strongest_oracle: OracleTier::Deterministic,
        deterministic_failures: 0,
    })
    .unwrap();
    assert_eq!(single.depth(), MeasurementDepth::Single);
    assert_eq!(single.score(), 1.0);
}

#[test]
fn a_perfect_score_on_one_trial_still_licenses_only_a_conformance_claim() {
    let atlas = Atlas::builder(ontology())
        .evidence(
            trial(
                "only",
                "analysis",
                TrialOutcome::Pass,
                EvidenceTier::ProspectiveWorkflow,
                OracleTier::Deterministic,
            )
            .with_parent_world(world("w0"))
            .with_site("site-a")
            .with_domain("oncology"),
        )
        .declare_unmeasured(cap("literature"), UnmeasuredReason::OutOfScopeByDeclaredUse)
        .build()
        .unwrap();

    assert_eq!(atlas.cell(&cap("analysis")).unwrap().score(), Some(1.0));
    assert_eq!(
        bioprism_atlas::permitted_claim(&atlas, &cap("analysis")),
        ClaimTier::UnitConformance
    );
}

fn diagnosed_failure(id: &str, capability: &str) -> FailureRecord {
    FailureRecord::new(
        id,
        RunId::parse("run-1").unwrap(),
        cap(capability),
        VERSION,
        CausalChain::new(
            id,
            FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
            FailureLabel::new(FailureMechanism::HypothesisCollapsedTooEarly, 3),
            Vec::new(),
            FailureLabel::new(FailureMechanism::VerifierAcceptedHackedReward, 8),
        )
        .unwrap(),
        FailureAxes::new(
            EvidenceStatus::Preserved,
            Reversibility::Reversible,
            Detectability::DetectedByDeterministicCheck,
            Severity::WrongConclusion,
            Inducement::ModelInduced,
        ),
        LabelDistribution::certain(
            FailureMechanism::HypothesisCollapsedTooEarly,
            "the trace shows a single hypothesis after step three",
        ),
    )
}

#[test]
fn a_failure_recorded_against_an_unmeasured_capability_is_reported_as_an_inconsistency() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Pass],
        ))
        .failure(diagnosed_failure("f1", "analysis"))
        .build()
        .unwrap();

    assert!(!atlas.cell(&cap("analysis")).unwrap().is_measured());
    assert!(atlas.inconsistencies().iter().any(|issue| matches!(
        issue,
        Inconsistency::FailureAgainstUnmeasuredCapability { capability, .. }
            if capability == "analysis"
    )));
    assert!(CoverageReport::of(&atlas)
        .inconsistencies
        .iter()
        .any(|issue| matches!(
            issue,
            Inconsistency::FailureAgainstUnmeasuredCapability { .. }
        )));
}

#[test]
fn more_diagnosed_failures_than_failed_trials_is_reported_as_a_contradiction() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Pass, TrialOutcome::Fail],
        ))
        .failure(diagnosed_failure("f1", "analysis"))
        .failure(diagnosed_failure("f2", "analysis"))
        .build()
        .unwrap();

    assert!(atlas.inconsistencies().iter().any(|issue| matches!(
        issue,
        Inconsistency::MoreFailuresThanFailedTrials {
            failures_recorded: 2,
            failed_trials: 1,
            ..
        }
    )));
}

#[test]
fn a_coherent_atlas_reports_no_inconsistencies() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "analysis",
            [TrialOutcome::Fail, TrialOutcome::Fail],
        ))
        .failure(diagnosed_failure("f1", "analysis"))
        .build()
        .unwrap();

    assert!(atlas.inconsistencies().is_empty());
    assert!(atlas.failures_for(&cap("analysis")).count() == 1);
    assert!(atlas.failures()[0].is_diagnosed());
}

#[test]
fn an_atlas_round_trips_through_json_without_losing_a_hole() {
    let atlas = Atlas::builder(ontology())
        .evidence_all(public_world_trials(
            "literature",
            [TrialOutcome::Pass, TrialOutcome::Fail],
        ))
        .declare_unmeasured(cap("analysis"), UnmeasuredReason::InaccessibleByPolicy)
        .build()
        .unwrap();

    let encoded = serde_json::to_string(&atlas).unwrap();
    let decoded: Atlas = serde_json::from_str(&encoded).unwrap();
    assert_eq!(atlas, decoded);
    assert_eq!(
        decoded.cell(&cap("analysis")).unwrap().unmeasured_reason(),
        Some(UnmeasuredReason::InaccessibleByPolicy)
    );
    assert_eq!(decoded.holes().count(), 2);
}
