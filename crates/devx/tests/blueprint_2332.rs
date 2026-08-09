//! 23.32 held against what this crate actually provides.
//!
//! 23.32 specifies a diagnostics clause with seven requirements and a debugger with ten panes and
//! five actions. These tests check the mapping in both directions: that every requirement is
//! discharged by something in the catalogue, and that every pane and action is either supplied or
//! declared unsupplied with a named gap.

use bioprism_devx::catalogue::catalogue;
use bioprism_devx::debugger::{
    debugger_surface, ActionAvailability, DevAction, Pane, PaneAvailability,
};
use bioprism_devx::diagnostic::ExplanationRequirement;

#[test]
fn every_one_of_the_seven_things_2332_requires_an_error_to_explain_is_demonstrated() {
    let entries = catalogue();
    for requirement in ExplanationRequirement::ALL {
        let demonstrated = entries.iter().any(|entry| requirement.satisfied_by(entry));
        assert!(
            demonstrated,
            "no catalogue entry demonstrates {:?} (field `{}`)",
            requirement.phrase(),
            requirement.field()
        );
    }
}

#[test]
fn the_three_universal_requirements_are_discharged_by_every_single_entry() {
    for entry in catalogue() {
        for requirement in ExplanationRequirement::ALL {
            if requirement.is_universal() {
                assert!(
                    requirement.satisfied_by(&entry),
                    "{} leaves {:?} undischarged",
                    entry.code,
                    requirement.phrase()
                );
            }
        }
    }
}

#[test]
fn the_optional_requirements_are_genuinely_optional_and_not_padded_into_every_entry() {
    let entries = catalogue();
    for requirement in ExplanationRequirement::ALL {
        if requirement.is_universal() {
            continue;
        }
        let satisfied = entries
            .iter()
            .filter(|entry| requirement.satisfied_by(entry))
            .count();
        assert!(satisfied > 0, "{:?} is never demonstrated", requirement.phrase());
        assert!(
            satisfied < entries.len(),
            "{:?} is claimed by every entry, which means the field is being padded rather than \
             used",
            requirement.phrase()
        );
    }
}

#[test]
fn each_requirement_maps_to_a_distinct_diagnostic_field() {
    let mut fields: Vec<&str> = ExplanationRequirement::ALL
        .iter()
        .map(|r| r.field())
        .collect();
    let before = fields.len();
    fields.sort_unstable();
    fields.dedup();
    assert_eq!(
        before,
        fields.len(),
        "two 23.32 requirements share a field, so one of them is not really discharged"
    );
}

#[test]
fn the_debugger_models_all_ten_panes_and_serves_none_of_the_five_actions() {
    let surface = debugger_surface();
    assert_eq!(surface.panes.len(), 10);
    for pane in Pane::ALL {
        assert!(surface.pane(pane).is_some());
    }
    for action in DevAction::ALL {
        let (_, availability) = surface
            .actions
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .expect("every action is modelled");
        assert_eq!(*availability, ActionAvailability::RequiresLiveSession);
    }
}

#[test]
fn every_pane_this_workspace_cannot_serve_names_the_reason_it_cannot() {
    for pane in debugger_surface().panes {
        if pane.availability == PaneAvailability::Servable {
            assert!(pane.gap.is_none());
            assert!(pane.backed_by.is_some());
        } else {
            let gap = pane.gap.as_deref().expect("an unserved pane names its gap");
            assert!(gap.len() > 30, "{:?} gives a token reason", pane.pane);
        }
    }
}

#[test]
fn the_semantic_loss_pane_is_the_only_partially_servable_one_and_says_why() {
    let surface = debugger_surface();
    let partial: Vec<Pane> = surface
        .panes
        .iter()
        .filter(|p| p.availability == PaneAvailability::PartiallyServable)
        .map(|p| p.pane)
        .collect();
    assert_eq!(partial, vec![Pane::SemanticLossWarnings]);
    let pane = surface
        .pane(Pane::SemanticLossWarnings)
        .expect("modelled");
    let gap = pane.gap.as_deref().expect("gap named");
    assert!(gap.contains("claims") && gap.contains("measures"));
}

#[test]
fn no_pane_is_marked_not_modelled_because_every_one_has_been_judged() {
    for pane in debugger_surface().panes {
        assert_ne!(
            pane.availability,
            PaneAvailability::NotModelled,
            "{:?} was never judged; the variant exists for panes nobody assessed",
            pane.pane
        );
    }
}

#[test]
fn every_pane_states_the_question_a_developer_opens_it_to_answer() {
    for pane in debugger_surface().panes {
        assert!(
            pane.question.len() > 25,
            "{:?} names a pane without naming a question",
            pane.pane
        );
    }
}
