//! Obstruction markers, shared by every projection.
//!
//! Blueprint 43.25 orders a Decision Section so that "conflicts and unresolved obligations appear
//! *before* any narrative rendering", and 43.01 forbids treating a visualisation as proof of
//! completeness. Those two together mean an obstruction must survive into *every* view — a
//! timeline that shows only what happened, or a table that shows only what was delivered, would
//! read as a complete account of a region that is in fact blocked.
//!
//! Rather than let each projection re-derive obstructions its own way, all four build them here,
//! from one place, with one naming rule. [`crate::FidelityLedger`] then checks that what the view
//! carries matches what the section holds, so "I forgot to render the obligations" is a typed
//! error rather than a silently smaller picture.

use crate::identity::{conflict_id, obligation_id, obligation_kind};
use bioprism_section::{DecisionSection, LeakageWitness, UnresolvedObligation};
use serde::{Deserialize, Serialize};

/// An obligation the compiler could not discharge, in view form.
///
/// `handles` names the objects a reader must go and look at — the fact that was withheld, for
/// instance — so the marker is actionable rather than merely present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationMarker {
    pub id: String,
    pub kind: String,
    pub detail: String,
    pub handles: Vec<String>,
}

/// One oracle witness, in view form.
///
/// 43.41 makes a witness a concrete checkable object rather than a score, so the detail here is
/// the witness's own description and never a severity number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictMarker {
    pub id: String,
    pub witness_kind: String,
    pub detail: String,
}

/// Every unresolved obligation in the section, named and described.
pub fn obligations(section: &DecisionSection) -> Vec<ObligationMarker> {
    section
        .unresolved_obligations
        .iter()
        .enumerate()
        .map(|(index, obligation)| ObligationMarker {
            id: obligation_id(index, obligation),
            kind: obligation_kind(obligation).to_string(),
            detail: describe_obligation(obligation),
            handles: obligation_handles(obligation),
        })
        .collect()
}

/// Every oracle witness in the section, named and described.
pub fn conflicts(section: &DecisionSection) -> Vec<ConflictMarker> {
    section
        .oracle
        .witnesses
        .iter()
        .enumerate()
        .map(|(index, witness)| ConflictMarker {
            id: conflict_id(index, witness),
            witness_kind: witness.kind().to_string(),
            detail: describe_witness(witness),
        })
        .collect()
}

/// The objects an obligation points at, so a view can link to them.
pub fn obligation_handles(obligation: &UnresolvedObligation) -> Vec<String> {
    match obligation {
        UnresolvedObligation::InaccessibleAtCut { fact_id } => vec![fact_id.clone()],
        UnresolvedObligation::Obstructed { .. } | UnresolvedObligation::PolicyBlocked { .. } => {
            Vec::new()
        }
    }
}

fn describe_obligation(obligation: &UnresolvedObligation) -> String {
    match obligation {
        UnresolvedObligation::InaccessibleAtCut { fact_id } => {
            format!("{fact_id} is required by the slice but not readable at the decision cut")
        }
        UnresolvedObligation::Obstructed { detail } => detail.clone(),
        UnresolvedObligation::PolicyBlocked { detail } => detail.clone(),
    }
}

/// Renders a witness as text without inventing a magnitude for it.
fn describe_witness(witness: &LeakageWitness) -> String {
    match witness {
        LeakageWitness::IdentityLeakage {
            alias,
            subjects,
            splits,
        } => format!(
            "alias {alias} resolves to subjects [{}] which land in splits [{}]",
            subjects.join(", "),
            splits.join(", ")
        ),
        LeakageWitness::SiteLeakage { site_by_split } => {
            let pairs: Vec<String> = site_by_split
                .iter()
                .map(|(split, sites)| format!("{split}: [{}]", sites.join(", ")))
                .collect();
            format!("site is determined by split — {}", pairs.join("; "))
        }
        LeakageWitness::TemporalLeakage {
            decision_time,
            future_label_sources,
        } => {
            let pairs: Vec<String> = future_label_sources
                .iter()
                .map(|(source, time)| format!("{source} available {time}"))
                .collect();
            format!(
                "label sources postdate the decision time {decision_time} — {}",
                pairs.join("; ")
            )
        }
        LeakageWitness::PreprocessingLeakage { detail } => detail.clone(),
        LeakageWitness::DomainCheck {
            check,
            observed,
            detail,
        } => {
            let pairs: Vec<String> = observed
                .iter()
                .map(|(variable, value)| format!("{variable}={value}"))
                .collect();
            format!("check {check} fired on [{}] — {detail}", pairs.join(", "))
        }
    }
}
