//! Cross-module invariants for blueprint section 13.
//!
//! The unit tests hold each module's own rules. These hold the ones that only exist when the
//! modules are used together — chiefly that the `Enforced` mitigations named in
//! [`bioprism_safety::model::section_13`] correspond to type properties a test can actually
//! demonstrate, rather than to sentences somebody typed into an enum.

use bioprism_safety::attest::{
    AttestationClaim, AuditEvent, AuditLog, AuditRecord, Observation, Statement,
};
use bioprism_safety::attest::Attestation;
use bioprism_safety::boundary::{
    ArtifactKind, BoundaryModel, Channel, MovingArtifact, TenantIsolation, TrustZone,
};
use bioprism_safety::disclosure::{
    Advisory, Finding, FindingStatus, Severity, Stage, Transition, Vulnerability,
    VulnerabilityClass,
};
use bioprism_safety::incident::{
    BlastRadius, ContainmentAction, ContainmentRequest, Incident, IncidentClass, ResultDisposition,
};
use bioprism_safety::integrity::{
    check_answer_containment, check_oracle_degeneracy, DegenerateProbe, IntegrityReport,
    IntegrityStatus, TaskSpec,
};
use bioprism_safety::model::section_13;
use bioprism_safety::provenance::{
    ContextAssembly, Position, Provenance, Segment, Sink, ToolCall,
};
use bioprism_safety::supply::{Component, ComponentKind, Pin, SignatureStatus};
use bioprism_safety::threat::{Mitigation, ThreatStatus, Unrepresentable};
use bioprism_safety::SafetyError;

/// Each `Unrepresentable` the shipped model claims is demonstrated here, so the model's only
/// enforced mitigations cannot drift away from the types that justify them.
#[test]
fn every_unrepresentable_state_the_model_cites_is_demonstrated_by_a_type_property() {
    let cited: Vec<Unrepresentable> = section_13()
        .threats
        .iter()
        .flat_map(|threat| threat.mitigations.iter())
        .filter_map(|mitigation| match mitigation {
            Mitigation::Enforced { by, .. } => {
                let bioprism_safety::threat::Enforcer::Unrepresentable(state) = by;
                Some(*state)
            }
            _ => None,
        })
        .collect();

    for state in &cited {
        match state {
            Unrepresentable::NoValueClaimsASignatureVerified => {
                let component =
                    Component::new("p", ComponentKind::BenchmarkAsset, Pin::digest("a")).signed("s");
                assert_eq!(component.signature_status, SignatureStatus::NotChecked);
            }
            Unrepresentable::NoValueClaimsTenantIsolationWasApplied => {
                assert_eq!(
                    TenantIsolation::DeclaredOnly.to_string(),
                    "declared-only",
                    "the enum has one variant and it says so"
                );
            }
            Unrepresentable::NoAssertionIsFiledAsAnObservation => {
                let asserted = Statement::asserted("someone", "the sandbox held");
                assert!(!asserted.is_observed());
            }
            Unrepresentable::NoAttestationClaimsObservationWithoutOne => {
                assert!(Attestation::observed(
                    AttestationClaim::BundleClosureVerified {
                        bundle: "b".into()
                    },
                    Observation::ChainLinkRecomputed { index: 0 },
                )
                .is_err());
            }
            Unrepresentable::NoContainmentReportExistsWithoutACompleteBlastRadius => {
                assert!(Incident::open("I", IncidentClass::CompromisedKey, 1)
                    .report_contained()
                    .is_err());
            }
            Unrepresentable::NoCrossingRecordExistsThatTheModelForbids => {
                let model = BoundaryModel::evaluation_model();
                let holdout = MovingArtifact::new(
                    "h",
                    ArtifactKind::HiddenOracleAsset,
                    TrustZone::ArtifactService,
                );
                assert!(model
                    .deliver(&holdout, TrustZone::AgentSandbox, Channel::ArtifactFetch)
                    .is_err());
            }
            Unrepresentable::NoValueNamesARuntimeEnforcer
            | Unrepresentable::NoValueClaimsIsolationWasApplied => {}
        }
    }

    assert!(
        cited.contains(&Unrepresentable::NoValueNamesARuntimeEnforcer),
        "the model must cite its own honesty guarantee"
    );
}

/// The threat the model records as unanalysed is the path the boundary model actually finds.
#[test]
fn the_unanalysed_threat_and_the_boundary_models_feedback_loop_describe_the_same_route() {
    let model = section_13();
    let unanalysed = model.unanalysed();
    assert_eq!(unanalysed.len(), 1);
    assert_eq!(unanalysed[0].id, "T-13.05-grader-steers-next-trial");

    let loops = BoundaryModel::evaluation_model().feedback_loops();
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].first(), Some(&TrustZone::EvaluatorSandbox));
    assert_eq!(loops[0].last(), Some(&TrustZone::AgentSandbox));
    assert_eq!(unanalysed[0].surface, "control_plane");
    assert!(loops[0].contains(&TrustZone::ControlPlane));
}

/// A pack review runs every check, files the witnesses as observations, and the chain verifies.
#[test]
fn a_pack_review_files_witnesses_as_observations_and_the_audit_chain_verifies() {
    let tasks = vec![
        TaskSpec::new("T-1", "BRAF V600E").with_context("ctx", "prior note: BRAF V600E present"),
        TaskSpec::new("T-2", "KRAS G12C").with_context("ctx", "no prior molecular data"),
    ];
    let mut report = IntegrityReport::default();
    report.push(check_answer_containment(&tasks));
    report.push(check_oracle_degeneracy(
        "exact-match",
        &[DegenerateProbe::new("unknown")
            .accepted_by("T-1")
            .accepted_by("T-2")],
        2,
    ));
    assert!(!report.is_clean());
    assert_eq!(report.witnesses().len(), 2);

    let mut log = AuditLog::new();
    for (index, witness) in report.witnesses().iter().enumerate() {
        log.append(AuditRecord::new(
            AuditEvent::ReviewerDecision,
            "reviewer:pack-gate",
            "onco-v1",
            Statement::observed(Observation::WitnessProduced {
                check: "integrity".into(),
                kind: witness.kind().into(),
            }),
            index as u64 + 1,
        ))
        .expect("epochs advance");
    }
    log.verify().expect("nothing tampered");
    assert!(
        log.assertions().is_empty(),
        "witnesses are computed, so nothing in this log is somebody's word"
    );
}

/// A check with no evidence keeps the report from being clean even when everything else passed.
#[test]
fn an_unprobed_oracle_keeps_a_pack_from_reporting_clean() {
    let mut report = IntegrityReport::default();
    report.push(check_answer_containment(&[TaskSpec::new("T-1", "yes")
        .with_context("ctx", "unrelated background")]));
    assert!(report.is_clean(), "the one check that ran found nothing");
    report.push(check_oracle_degeneracy("exact-match", &[], 2));
    assert!(!report.is_clean());
    assert_eq!(
        report.underdetermined_checks()[0].status,
        IntegrityStatus::Underdetermined
    );
}

/// An injection path is a structural finding, and it is recorded as an observation.
#[test]
fn an_injection_path_reaching_a_tool_argument_is_recordable_as_an_observation() {
    let mut assembly = ContextAssembly::new();
    assembly
        .add(Segment::new("page", Provenance::RetrievedContent))
        .expect("root");
    assembly.record_tool_call(ToolCall::new("shell").argument_from("command", "page"));
    let paths = assembly.injection_paths();
    assert_eq!(paths.len(), 1);
    assert!(matches!(paths[0].sink, Sink::ToolArgument { .. }));

    let mut log = AuditLog::new();
    log.append(AuditRecord::new(
        AuditEvent::SensitiveArtifactAccess,
        "assembler",
        "run-1",
        Statement::observed(Observation::InjectionPathFound {
            origin: paths[0].origin.clone(),
            sink: paths[0].sink.as_str().into(),
        }),
        1,
    ))
    .expect("appended");
    log.verify().expect("intact");
}

/// A laundered segment is refused before it can become a finding, so the assembly never contains it.
#[test]
fn an_assembly_that_refuses_laundering_never_reports_the_laundered_segment() {
    let mut assembly = ContextAssembly::new();
    assembly
        .add(Segment::new("page", Provenance::RetrievedContent))
        .expect("root");
    let refused = assembly.add(
        Segment::new("preamble", Provenance::System)
            .derived_from("page")
            .positioned(Position::Instruction),
    );
    assert!(matches!(
        refused.expect_err("laundering"),
        SafetyError::AuthorityLaundering { .. }
    ));
    assert_eq!(assembly.segments().len(), 1);
    assert!(assembly.injection_paths().is_empty());
}

/// A holdout leak runs from open incident to containment report, and cannot skip the middle.
#[test]
fn a_holdout_leak_reaches_containment_only_after_every_dependent_result_is_dispositioned() {
    let mut incident = Incident::open("INC-1", IncidentClass::HiddenHoldoutLeak, 10)
        .requesting(ContainmentRequest::new(
            ContainmentAction::QuarantineArtifacts,
            "sre:kim",
            10,
        ))
        .requesting(ContainmentRequest::new(
            ContainmentAction::FreezePublication,
            "sre:kim",
            10,
        ))
        .with_blast_radius(BlastRadius::partial(3));
    incident
        .timeline
        .push(10, "sre:kim", "holdout digest seen in a public trace")
        .expect("first");
    assert!(incident.report_contained().is_err());

    incident.blast_radius = BlastRadius::complete()
        .with("r-11", ResultDisposition::Invalidated)
        .with("r-12", ResultDisposition::UnderInvestigation);
    incident
        .timeline
        .push(12, "sre:kim", "lineage query completed")
        .expect("second");
    assert!(incident.report_contained().is_err());

    incident
        .blast_radius
        .dispose("r-12", ResultDisposition::RequiresReproduction);
    let report = incident.report_contained().expect("all resolved");
    assert_eq!(report.results_examined(), 2);
    assert_eq!(report.results_invalidated(), 1);
    assert_eq!(report.requests_issued(), 2);
    assert!(report.caveat().contains("requested, not observed"));
    assert_eq!(incident.timeline.len(), 2);
}

/// A confirmed red-team finding becomes a sentinel and a tracked vulnerability, disclosed last.
#[test]
fn a_confirmed_finding_becomes_a_sentinel_and_walks_the_disclosure_ladder() {
    let finding = Finding::new(
        "F-11",
        "hidden-test-extraction",
        "evaluator_sandbox",
        VulnerabilityClass::HiddenTestExposure,
    )
    .with_status(FindingStatus::Confirmed)
    .reproducing("read the mount path from the error message");
    let cell = finding
        .clone()
        .into_regression_cell(true)
        .expect("confirmed")
        .minimised();
    assert!(cell.minimised);
    assert!(!cell.public_summary().contains("error message"));

    let mut vulnerability =
        Vulnerability::reported("V-11", finding.class, Severity::High, 20);
    assert!(vulnerability.severity.requires_independent_verification());
    vulnerability
        .advance(Transition::to(Stage::Triaged, 21))
        .expect("triaged");
    vulnerability
        .advance(Transition::to(Stage::Fixed, 25))
        .expect("fixed");

    let mut advisory = Advisory {
        affected_versions: "0.1.0".into(),
        impact: "holdout labels readable from the task sandbox".into(),
        mitigation: "rotate the holdout".into(),
        fixed_versions: "0.1.1".into(),
        result_implications: String::new(),
        timeline: "e20 reported, e25 fixed".into(),
        credit: "external researcher".into(),
        residual_risk: "mirrors may retain the old asset".into(),
    };
    assert!(advisory.audit_for(&vulnerability).is_err());
    advisory.result_implications = "runs r-1..r-40 require reproduction".into();
    advisory.audit_for(&vulnerability).expect("complete");

    vulnerability
        .advance(Transition::to(Stage::Disclosed, 30))
        .expect("disclosed");
    assert!(!vulnerability.embargoed);
    assert_eq!(vulnerability.history.len(), 3);
}

/// Nothing in the shipped model lets a caller treat a perimeter control as applied.
#[test]
fn no_perimeter_threat_in_the_shipped_model_can_be_relied_on() {
    let model = section_13();
    for threat in &model.threats {
        match threat.rely() {
            Ok(_) => assert_eq!(
                threat.status(),
                ThreatStatus::Mitigated,
                "{} returned Ok without being mitigated",
                threat.id
            ),
            Err(SafetyError::UnenforcedReliance { .. } | SafetyError::UnmitigatedThreat { .. }) => {}
            Err(other) => panic!("{} produced an unexpected error: {other}", threat.id),
        }
    }
    let coverage = model.coverage();
    assert!(coverage.declared_only > 0);
    assert!(coverage.unmitigated > 0);
    assert!(coverage.mitigated > 0);
}

/// The whole model round-trips through JSON, so a stored threat model reloads with the same status.
#[test]
fn the_shipped_model_round_trips_through_json_without_changing_a_single_status() {
    let model = section_13();
    let json = serde_json::to_string_pretty(&model).expect("serialises");
    let back: bioprism_safety::ThreatModel = serde_json::from_str(&json).expect("deserialises");
    for (before, after) in model.threats.iter().zip(back.threats.iter()) {
        assert_eq!(before.status(), after.status(), "{}", before.id);
    }
    assert!(
        json.contains("declared_only"),
        "the serialised model must say which mitigations are only declared"
    );
}
