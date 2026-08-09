//! The debugger, modelled rather than served.
//!
//! Blueprint 23.32 specifies a local debugger by listing ten panes it displays and five actions a
//! user may take in development mode. This module is that specification as data: what each pane
//! would show, whether anything in this workspace can supply it, and — the part 23.32 omits — what
//! question a developer is actually asking when they open it.
//!
//! # There is no debugger here, and that is the honest form of the module
//!
//! A debugger needs a live session: a paused execution, a message queue to inject into, a grant to
//! revoke. This crate has no runtime, no process, no transport and no clock, so every
//! [`DevAction`] reports [`ActionAvailability::RequiresLiveSession`] and none of them is
//! implemented. Shipping a stub that pretends to pause would be worse than shipping the model: a
//! consumer would build against it.
//!
//! What the model is *for* is the audit. [`debugger_surface`] states, pane by pane, whether the
//! data exists in this workspace's types, needs a runtime nobody here provides, or was never
//! modelled at all. That converts 23.32's ten bullets from a wish list into a coverage report,
//! and [`SurfaceReport::gaps`] is the list a later contributor works from.
//!
//! # The finding
//!
//! Of ten panes, three are servable from types that exist in this workspace today, six need a live
//! choreography runtime, and one — the semantic-loss warning pane — is servable only in part:
//! `bioprism-sdk` carries a plugin's *declared* loss, which is what the adapter says it drops, and
//! nothing anywhere measures what it actually dropped. A pane that renders a declaration as though
//! it were a measurement is the exact failure `bioprism-docgraph`'s "an estimate is never a
//! measurement" rule exists to prevent, so the pane is reported as
//! [`PaneAvailability::PartiallyServable`] with the gap named.

use crate::diagnostic::{Certainty, Site};
use serde::{Deserialize, Serialize};

/// One pane 23.32 says the debugger displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pane {
    GlobalChoreographyState,
    ParticipantLocalState,
    ContextCapsule,
    EvidenceAndCommitmentLedgers,
    ActiveGrantsAndBudgets,
    CausalTrace,
    EnabledAndRejectedActs,
    BranchTree,
    VerifierResults,
    SemanticLossWarnings,
}

/// Whether this workspace can supply a pane's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneAvailability {
    /// The data exists in a type this workspace defines, and a renderer could read it today.
    Servable,
    /// Part of the data exists and part does not. The gap is named on the pane.
    PartiallyServable,
    /// The data only exists while something is running, and nothing here runs anything.
    RequiresLiveSession,
    /// Nothing in this workspace models it.
    NotModelled,
}

impl PaneAvailability {
    pub fn as_str(self) -> &'static str {
        match self {
            PaneAvailability::Servable => "servable",
            PaneAvailability::PartiallyServable => "partially_servable",
            PaneAvailability::RequiresLiveSession => "requires_live_session",
            PaneAvailability::NotModelled => "not_modelled",
        }
    }

    /// Whether a renderer could show anything at all.
    pub fn has_data(self) -> bool {
        matches!(
            self,
            PaneAvailability::Servable | PaneAvailability::PartiallyServable
        )
    }
}

/// A pane, with the question it answers and the evidence behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneModel {
    pub pane: Pane,
    /// The developer's actual question. 23.32 names the pane; naming the question is what makes it
    /// possible to say whether the pane answers anything.
    pub question: String,
    pub availability: PaneAvailability,
    /// The workspace type that supplies the data, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backed_by: Option<String>,
    /// What is missing, when something is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    /// How confidently the availability judgement is asserted.
    pub certainty: Certainty,
}

/// One of the five things 23.32 says a user may do in development mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevAction {
    Pause,
    InjectMessage,
    RevokeGrant,
    SwapParticipant,
    ForkBranch,
}

impl DevAction {
    pub const ALL: [DevAction; 5] = [
        DevAction::Pause,
        DevAction::InjectMessage,
        DevAction::RevokeGrant,
        DevAction::SwapParticipant,
        DevAction::ForkBranch,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DevAction::Pause => "pause",
            DevAction::InjectMessage => "inject_message",
            DevAction::RevokeGrant => "revoke_grant",
            DevAction::SwapParticipant => "swap_participant",
            DevAction::ForkBranch => "fork_branch",
        }
    }
}

/// Whether an action can be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionAvailability {
    /// Needs a paused execution to act on. Nothing in this crate provides one.
    RequiresLiveSession,
}

/// The debugger surface: ten panes and five actions, with their availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceReport {
    pub panes: Vec<PaneModel>,
    pub actions: Vec<(DevAction, ActionAvailability)>,
}

impl SurfaceReport {
    pub fn pane(&self, pane: Pane) -> Option<&PaneModel> {
        self.panes.iter().find(|model| model.pane == pane)
    }

    /// Panes a renderer could show something for.
    pub fn servable(&self) -> Vec<&PaneModel> {
        self.panes
            .iter()
            .filter(|model| model.availability.has_data())
            .collect()
    }

    /// Panes with a named gap, which is the contributor's work list.
    pub fn gaps(&self) -> Vec<(Pane, &str)> {
        self.panes
            .iter()
            .filter_map(|model| model.gap.as_deref().map(|gap| (model.pane, gap)))
            .collect()
    }

    /// The site to cite when reporting that a pane cannot be served.
    pub fn site_for(&self, pane: Pane) -> Site {
        match self.pane(pane) {
            Some(model) => match &model.backed_by {
                Some(type_name) => Site::Artifact {
                    node_kind: "type".to_string(),
                    id: type_name.clone(),
                },
                None => Site::Unlocated {
                    because: format!(
                        "no type in this workspace supplies the {} pane, so there is nothing to \
                         point at",
                        model.pane.as_str()
                    ),
                },
            },
            None => Site::Unlocated {
                because: "the pane is not in the 23.32 list".to_string(),
            },
        }
    }
}

impl Pane {
    pub const ALL: [Pane; 10] = [
        Pane::GlobalChoreographyState,
        Pane::ParticipantLocalState,
        Pane::ContextCapsule,
        Pane::EvidenceAndCommitmentLedgers,
        Pane::ActiveGrantsAndBudgets,
        Pane::CausalTrace,
        Pane::EnabledAndRejectedActs,
        Pane::BranchTree,
        Pane::VerifierResults,
        Pane::SemanticLossWarnings,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Pane::GlobalChoreographyState => "global_choreography_state",
            Pane::ParticipantLocalState => "participant_local_state",
            Pane::ContextCapsule => "context_capsule",
            Pane::EvidenceAndCommitmentLedgers => "evidence_and_commitment_ledgers",
            Pane::ActiveGrantsAndBudgets => "active_grants_and_budgets",
            Pane::CausalTrace => "causal_trace",
            Pane::EnabledAndRejectedActs => "enabled_and_rejected_acts",
            Pane::BranchTree => "branch_tree",
            Pane::VerifierResults => "verifier_results",
            Pane::SemanticLossWarnings => "semantic_loss_warnings",
        }
    }
}

/// The 23.32 debugger surface, as this workspace can and cannot supply it.
pub fn debugger_surface() -> SurfaceReport {
    let live = |pane: Pane, question: &str, gap: &str| PaneModel {
        pane,
        question: question.to_string(),
        availability: PaneAvailability::RequiresLiveSession,
        backed_by: None,
        gap: Some(gap.to_string()),
        certainty: Certainty::Observed,
    };

    SurfaceReport {
        panes: vec![
            live(
                Pane::GlobalChoreographyState,
                "where in the protocol is the whole system right now",
                "no choreography runtime is reachable from this crate; the state exists only while \
                 a run is in progress",
            ),
            live(
                Pane::ParticipantLocalState,
                "what does each participant believe, and where do two of them disagree",
                "per-participant state is a runtime artefact and is not carried by any type here",
            ),
            PaneModel {
                pane: Pane::ContextCapsule,
                question: "what evidence was this decision allowed to see, and what was withheld"
                    .to_string(),
                availability: PaneAvailability::Servable,
                backed_by: Some("bioprism_devx::introspect::CompileRecord".to_string()),
                gap: None,
                certainty: Certainty::Observed,
            },
            live(
                Pane::EvidenceAndCommitmentLedgers,
                "what has been committed to, and on what evidence",
                "ledgers are append-only runtime state; nothing in this crate's dependency set \
                 holds one",
            ),
            live(
                Pane::ActiveGrantsAndBudgets,
                "what is this participant currently permitted to do, and how much of its budget is left",
                "a budget is declared in a query and consumed at runtime; only the declaration is \
                 reachable here",
            ),
            PaneModel {
                pane: Pane::CausalTrace,
                question: "which pass decided this, and what did it decide".to_string(),
                availability: PaneAvailability::Servable,
                backed_by: Some("bioprism_devx::introspect::PassRecord".to_string()),
                gap: None,
                certainty: Certainty::Observed,
            },
            live(
                Pane::EnabledAndRejectedActs,
                "what could have happened here instead, and why did it not",
                "the enabled set is computed by a runtime against a live state",
            ),
            live(
                Pane::BranchTree,
                "which forks exist from this decision point",
                "forking needs an execution to fork; bioprism-trace owns divergence analysis over \
                 traces that already exist",
            ),
            PaneModel {
                pane: Pane::VerifierResults,
                question: "does the receipt for this result verify, and against what".to_string(),
                availability: PaneAvailability::Servable,
                backed_by: Some("bioprism_section::ContextCertificate::verify".to_string()),
                gap: None,
                certainty: Certainty::Observed,
            },
            PaneModel {
                pane: Pane::SemanticLossWarnings,
                question: "what did the adapters drop on the way in".to_string(),
                availability: PaneAvailability::PartiallyServable,
                backed_by: Some("bioprism_sdk::SemanticLossDeclaration".to_string()),
                gap: Some(
                    "the declaration says what an adapter claims it drops; nothing in this \
                     workspace measures what it actually dropped, so this pane can show a claim \
                     and must not present it as an observation"
                        .to_string(),
                ),
                certainty: Certainty::Observed,
            },
        ],
        actions: DevAction::ALL
            .into_iter()
            .map(|action| (action, ActionAvailability::RequiresLiveSession))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pane_2332_lists_is_modelled_exactly_once() {
        let surface = debugger_surface();
        assert_eq!(surface.panes.len(), Pane::ALL.len());
        for pane in Pane::ALL {
            assert!(surface.pane(pane).is_some(), "{} is unmodelled", pane.as_str());
        }
    }

    #[test]
    fn the_debugger_serves_no_live_action_and_says_so_for_all_five() {
        let surface = debugger_surface();
        assert_eq!(surface.actions.len(), 5);
        for (action, availability) in &surface.actions {
            assert_eq!(
                *availability,
                ActionAvailability::RequiresLiveSession,
                "{} claims to be available",
                action.as_str()
            );
        }
    }

    #[test]
    fn three_panes_are_fully_servable_and_one_is_only_partly() {
        let surface = debugger_surface();
        let full = surface
            .panes
            .iter()
            .filter(|p| p.availability == PaneAvailability::Servable)
            .count();
        let partial = surface
            .panes
            .iter()
            .filter(|p| p.availability == PaneAvailability::PartiallyServable)
            .count();
        assert_eq!(full, 3);
        assert_eq!(partial, 1);
        assert_eq!(surface.servable().len(), 4);
    }

    #[test]
    fn the_semantic_loss_pane_declares_that_a_declaration_is_not_a_measurement() {
        let surface = debugger_surface();
        let pane = surface
            .pane(Pane::SemanticLossWarnings)
            .expect("pane is modelled");
        let gap = pane.gap.as_deref().expect("the partial pane names its gap");
        assert!(gap.contains("measures"));
        assert_eq!(pane.availability, PaneAvailability::PartiallyServable);
    }

    #[test]
    fn every_unavailable_pane_names_its_gap_and_every_servable_one_names_its_type() {
        for pane in debugger_surface().panes {
            if pane.availability.has_data() {
                assert!(pane.backed_by.is_some(), "{:?} claims data with no type", pane.pane);
            } else {
                assert!(pane.gap.is_some(), "{:?} is unavailable with no reason", pane.pane);
                assert!(pane.backed_by.is_none());
            }
            assert!(!pane.question.trim().is_empty());
        }
    }

    #[test]
    fn a_pane_with_no_backing_type_yields_an_unlocated_site_that_explains_itself() {
        let surface = debugger_surface();
        match surface.site_for(Pane::BranchTree) {
            Site::Unlocated { because } => assert!(because.contains("branch_tree")),
            other => panic!("expected an unlocated site, got {other:?}"),
        }
        assert!(surface.site_for(Pane::CausalTrace).is_addressable());
    }

    #[test]
    fn the_gap_list_is_the_contributor_work_list_and_covers_every_unserved_pane() {
        let surface = debugger_surface();
        let gaps = surface.gaps();
        assert_eq!(gaps.len(), 7);
        assert!(gaps.iter().all(|(_, gap)| gap.len() > 30));
    }

    #[test]
    fn the_surface_round_trips_through_json() {
        let surface = debugger_surface();
        let encoded = serde_json::to_string(&surface).expect("serialises");
        let decoded: SurfaceReport = serde_json::from_str(&encoded).expect("parses back");
        assert_eq!(surface, decoded);
    }
}
