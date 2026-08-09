//! 23.29: the threat-control relation, zone-bounded discharge, dimensional trust, release gates.

use bioprism_interweave::adapter::Grade;
use bioprism_interweave::threat::{
    discharged_by, Actor, AgreementClause, Assessment, Control, CrossOrgAgreement, DischargeRefusal,
    OutputTrust, RelianceRefusal, ReleaseCandidate, ReleaseGate, Threat, TrustDimension, TrustZone,
    RED_TEAM_EXERCISES,
};
use std::collections::BTreeSet;

#[test]
fn every_threat_is_answered_by_at_least_one_control() {
    for threat in Threat::ALL {
        assert!(
            !threat.controls().is_empty(),
            "{threat:?} has no control and is therefore an unrecorded accepted risk"
        );
    }
}

#[test]
fn every_control_answers_at_least_one_threat() {
    for control in Control::ALL {
        assert!(
            !control.threats().is_empty(),
            "{control:?} is a checklist item no threat needs"
        );
    }
}

#[test]
fn the_relation_is_consistent_in_both_directions() {
    for threat in Threat::ALL {
        for control in threat.controls() {
            assert!(
                control.threats().contains(&threat),
                "{control:?} does not name {threat:?} back"
            );
        }
    }
}

#[test]
fn transitive_revocation_is_the_control_capability_escalation_specifically_needs() {
    assert!(Threat::CapabilityEscalation
        .controls()
        .contains(&Control::TransitiveRevocation));
    assert_eq!(
        Control::TransitiveRevocation.threats(),
        BTreeSet::from([Threat::CapabilityEscalation])
    );
}

#[test]
fn every_threat_carries_the_blueprints_own_description() {
    for threat in Threat::ALL {
        assert!(!threat.description().is_empty());
    }
    assert_eq!(Threat::ALL.len(), 11);
    assert_eq!(Control::ALL.len(), 15);
}

#[test]
fn an_untrusted_actor_cannot_discharge_a_trusted_core_control() {
    let agent = Actor {
        name: "patcher".into(),
        zone: TrustZone::Untrusted,
    };
    let refusal = discharged_by(&agent, Control::AppendOnlyAuditHistory, "bundle-1").unwrap_err();
    assert_eq!(
        refusal,
        DischargeRefusal::ZoneTooLow {
            actor: "patcher".into(),
            control: Control::AppendOnlyAuditHistory,
            actual: TrustZone::Untrusted,
            required: TrustZone::TrustedCore,
        }
    );
}

#[test]
fn a_conditionally_trusted_actor_may_discharge_a_conditionally_trusted_control() {
    let runtime = Actor {
        name: "local-runtime".into(),
        zone: TrustZone::ConditionallyTrusted,
    };
    assert!(discharged_by(&runtime, Control::IsolatedExecution, "component-3").is_ok());
}

#[test]
fn a_participant_cannot_run_the_independent_evaluator_control_on_its_own_output() {
    let core = Actor {
        name: "molecule-a".into(),
        zone: TrustZone::TrustedCore,
    };
    let refusal =
        discharged_by(&core, Control::IndependentEvaluatorProcess, "molecule-a").unwrap_err();
    assert_eq!(
        refusal,
        DischargeRefusal::SelfEvaluation {
            actor: "molecule-a".into(),
            control: Control::IndependentEvaluatorProcess,
        }
    );
}

#[test]
fn the_same_evaluator_control_is_fine_on_somebody_elses_output() {
    let core = Actor {
        name: "evaluator".into(),
        zone: TrustZone::TrustedCore,
    };
    assert!(discharged_by(&core, Control::IndependentEvaluatorProcess, "molecule-a").is_ok());
}

#[test]
fn the_three_trust_zones_partition_the_named_members_without_overlap() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for zone in TrustZone::ALL {
        for member in zone.members() {
            assert!(seen.insert(member), "{member} appears in two zones");
        }
    }
    assert_eq!(seen.len(), 16);
}

#[test]
fn a_new_output_trust_record_is_unassessed_on_every_dimension() {
    let trust = OutputTrust::new("bundle-1");
    assert_eq!(trust.unassessed().len(), TrustDimension::ALL.len());
    assert_eq!(
        trust.assessment(TrustDimension::Freshness),
        &Assessment::Unassessed
    );
}

#[test]
fn an_unassessed_dimension_refuses_reliance_and_is_distinguishable_from_an_inadequate_one() {
    let trust = OutputTrust::new("bundle-1")
        .inadequate(TrustDimension::ParticipantIndependence, "two aliases, one operator");
    let refusals = trust
        .relied_upon_for(&BTreeSet::from([
            TrustDimension::ParticipantIndependence,
            TrustDimension::Freshness,
        ]))
        .unwrap_err();
    assert_eq!(refusals.len(), 2);
    assert!(refusals
        .iter()
        .any(|r| matches!(r, RelianceRefusal::Inadequate { .. })));
    assert!(refusals
        .iter()
        .any(|r| matches!(r, RelianceRefusal::Unassessed { .. })));
}

#[test]
fn an_output_adequate_on_exactly_the_dimensions_a_caller_needs_may_be_relied_upon() {
    let trust = OutputTrust::new("bundle-1")
        .adequate(TrustDimension::ArtifactIntegrity, "hash matched")
        .adequate(TrustDimension::ExecutionReproducibility, "replayed twice");
    assert!(trust
        .relied_upon_for(&BTreeSet::from([
            TrustDimension::ArtifactIntegrity,
            TrustDimension::ExecutionReproducibility,
        ]))
        .is_ok());
    assert_eq!(trust.unassessed().len(), 7);
}

#[test]
fn reliance_on_no_dimensions_is_vacuously_permitted_and_says_nothing_about_the_output() {
    let trust = OutputTrust::new("bundle-1");
    assert!(trust.relied_upon_for(&BTreeSet::new()).is_ok());
    assert_eq!(trust.unassessed().len(), 9);
}

#[test]
fn a_federation_agreement_missing_clauses_names_them() {
    let agreement = CrossOrgAgreement {
        parties: ("lab-a".into(), "lab-b".into()),
        clauses: BTreeSet::from([
            AgreementClause::DataHandling,
            AgreementClause::IncidentNotification,
        ]),
    };
    assert!(!agreement.complete());
    assert_eq!(agreement.gaps().len(), 6);
    assert!(agreement.gaps().contains(&AgreementClause::Revocation));
}

#[test]
fn a_complete_federation_agreement_has_no_gaps() {
    let agreement = CrossOrgAgreement {
        parties: ("lab-a".into(), "lab-b".into()),
        clauses: AgreementClause::ALL.into_iter().collect(),
    };
    assert!(agreement.complete());
}

#[test]
fn a_release_that_passes_no_gate_is_blocked_on_all_six() {
    let candidate = ReleaseCandidate::new("0.1.0");
    assert_eq!(candidate.blocking(&BTreeSet::new()).len(), 6);
    assert!(!candidate.releasable(&BTreeSet::new()));
}

#[test]
fn a_release_shipping_an_adapter_without_a_stated_grade_is_blocked_on_the_grade_gate() {
    let candidate = ReleaseGate::ALL
        .into_iter()
        .fold(ReleaseCandidate::new("1.0.0"), ReleaseCandidate::passing);
    let shipped = BTreeSet::from(["a2a-bridge".to_string()]);
    assert_eq!(
        candidate.blocking(&shipped),
        BTreeSet::from([ReleaseGate::AdapterLossAndTrustGradesVisible])
    );
}

#[test]
fn the_same_release_with_the_grade_published_is_releasable() {
    let candidate = ReleaseGate::ALL
        .into_iter()
        .fold(ReleaseCandidate::new("1.0.0"), ReleaseCandidate::passing)
        .publishing("a2a-bridge", Grade::G4);
    let shipped = BTreeSet::from(["a2a-bridge".to_string()]);
    assert!(candidate.releasable(&shipped));
}

#[test]
fn a_release_that_ships_no_adapters_is_not_blocked_by_the_grade_gate() {
    let candidate = ReleaseGate::ALL
        .into_iter()
        .fold(ReleaseCandidate::new("1.0.0"), ReleaseCandidate::passing);
    assert!(candidate.releasable(&BTreeSet::new()));
}

#[test]
fn the_red_team_programme_lists_twelve_exercises_and_this_crate_runs_none() {
    assert_eq!(RED_TEAM_EXERCISES.len(), 12);
}
