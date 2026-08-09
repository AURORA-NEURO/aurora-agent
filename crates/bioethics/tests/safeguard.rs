//! The declared/enforced distinction, and the shipped §36 register.

use bioprism_bioethics::safeguard::{SafeguardDocument, DECLARED, ENFORCED};
use bioprism_bioethics::{
    section_36_remainder, BioethicsError, BlueprintModule, ControlSurface, DeclaredSafeguard,
    Impossibility, Safeguard,
};

fn claim_safeguard() -> DeclaredSafeguard {
    DeclaredSafeguard::new(
        "task risk classification",
        BlueprintModule::DualUseBiosecurityAndCapabilityRelease,
        ControlSurface::Claim,
        "36.11 required controls",
    )
}

fn perimeter_safeguard() -> DeclaredSafeguard {
    DeclaredSafeguard::new(
        "institutional safety review",
        BlueprintModule::PhysicalExperimentAndWetLabActionBoundaries,
        ControlSurface::Perimeter,
        "36.10 required controls",
    )
}

#[test]
fn a_perimeter_safeguard_cannot_be_enforced() {
    let error = perimeter_safeguard()
        .enforce(Impossibility::NoValueRepresentsAPerformedPhysicalAction)
        .expect_err("a control that needs a person cannot be enforced by a type");
    assert!(matches!(
        error,
        BioethicsError::PerimeterCannotBeEnforced {
            surface: ControlSurface::Perimeter,
            ..
        }
    ));
}

#[test]
fn a_declared_safeguard_cannot_be_recorded_as_an_enforced_one() {
    let enforced = claim_safeguard()
        .enforce(Impossibility::NoReleaseReferralExistsForAnUnassessedTask)
        .expect("a claim control may be enforced when an impossibility backs it");
    let encoded = serde_json::to_string(&Safeguard::Enforced(enforced))
        .expect("an enforced safeguard serialises");
    assert!(encoded.contains(ENFORCED));

    let error = serde_json::from_str::<Safeguard>(&encoded)
        .expect_err("enforcement is a property of this crate's types, not of bytes");
    assert!(
        error.to_string().contains("does not travel in bytes"),
        "the decode must fail loudly rather than downgrade to declared: {error}"
    );
}

#[test]
fn a_declared_safeguard_survives_a_round_trip_through_json() {
    let original = Safeguard::Declared(perimeter_safeguard());
    let encoded = serde_json::to_string(&original).expect("serialisable");
    assert!(encoded.contains(DECLARED));
    let decoded: Safeguard =
        serde_json::from_str(&encoded).expect("a declaration is transportable");
    assert_eq!(decoded, original);
}

#[test]
fn an_unknown_enforcement_word_is_refused_rather_than_defaulted() {
    let document = SafeguardDocument {
        name: "quarantine".to_string(),
        module: BlueprintModule::SandboxingUntrustedCodeAndResearchArtifacts,
        surface: ControlSurface::Perimeter,
        enforcement: "partially".to_string(),
        declared_in: Some("a runbook".to_string()),
        impossibility: None,
    };
    let error = Safeguard::try_from(document)
        .expect_err("a third enforcement state would be a state nobody defined");
    assert!(matches!(
        error,
        BioethicsError::UnknownEnforcementState { .. }
    ));
}

#[test]
fn relying_on_a_declaration_is_a_typed_error_rather_than_a_judgement_call() {
    let declared = Safeguard::Declared(perimeter_safeguard());
    let error = declared.rely().expect_err("a declaration applies nothing");
    assert!(matches!(error, BioethicsError::UnenforcedReliance { .. }));

    let enforced = Safeguard::Enforced(
        claim_safeguard()
            .enforce(Impossibility::NoReleaseReferralExistsForAnUnassessedTask)
            .expect("claim controls may be enforced"),
    );
    assert!(enforced.rely().is_ok());
}

#[test]
fn the_shipped_register_marks_no_perimeter_control_enforced() {
    let register = section_36_remainder();
    for entry in register.enforced() {
        assert_eq!(
            entry.surface(),
            ControlSurface::Claim,
            "{:?} guards a perimeter and cannot be enforced by a single-process library",
            entry.name()
        );
    }
}

#[test]
fn the_shipped_register_pins_its_two_counts_and_never_sums_them() {
    let register = section_36_remainder();
    let counts = register.counts();
    assert_eq!(counts.declared, 36);
    assert_eq!(counts.enforced, 6);
    assert_eq!(
        register.entries().len(),
        42,
        "seven modules, six required controls each"
    );
}

#[test]
fn every_module_in_scope_contributes_exactly_its_six_required_controls() {
    let register = section_36_remainder();
    for module in BlueprintModule::ALL {
        assert_eq!(
            register.for_module(module).len(),
            6,
            "{module} lists six required controls"
        );
    }
}

#[test]
fn every_enforced_entry_names_an_impossibility_and_the_module_it_is_checkable_in() {
    let register = section_36_remainder();
    let mut named: Vec<Impossibility> = register
        .enforced()
        .into_iter()
        .map(|entry| {
            entry
                .impossibility()
                .expect("an enforced entry carries its impossibility")
        })
        .collect();
    named.sort();
    let mut all = Impossibility::ALL.to_vec();
    all.sort();
    assert_eq!(
        named, all,
        "every impossibility this crate declares is used, and none is used twice"
    );
    for impossibility in Impossibility::ALL {
        assert!(
            !impossibility.checkable_in().is_empty(),
            "{impossibility} must name a module a reader can go and falsify it in"
        );
    }
}

#[test]
fn the_two_modules_this_crate_did_not_implement_carry_no_blueprint_id() {
    for module in BlueprintModule::ALL {
        if module.is_implemented_here() {
            assert!(module.module_id().is_some());
            assert!(module.deferred_to().is_none());
        } else {
            assert!(
                module.module_id().is_none(),
                "{} was read and not implemented; citing it would move a coverage number only",
                module.title()
            );
            assert!(
                module.deferred_to().is_some(),
                "an uncited module must say where the workspace's position actually lives"
            );
        }
    }
}

#[test]
fn every_deferred_control_is_declared_and_every_one_of_them_is_a_perimeter_control() {
    let register = section_36_remainder();
    let deferred = register.deferred();
    assert_eq!(deferred.len(), 12, "two uncited modules, six controls each");
    for entry in deferred {
        assert!(!entry.is_enforced());
        assert_eq!(
            entry.surface(),
            ControlSurface::Perimeter,
            "{:?} is why these two modules were classified as infrastructure",
            entry.name()
        );
    }
}
