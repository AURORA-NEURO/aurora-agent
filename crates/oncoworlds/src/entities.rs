//! The entity worlds (30.20–30.24), one checkable distinction each.
//!
//! Blueprint modules 30.20 through 30.24 define five families of disease world: paediatric
//! high-grade glioma and DMG, paediatric low-grade glioma and the BRAF pathway, other paediatric
//! and rare CNS entities, adult diffuse glioma and glioblastoma, and brain metastases with
//! meningioma. Read side by side they are largely the same document — see the crate header's
//! boilerplate measurement — and the parts that differ are each module's characteristic failure
//! list. That is what this module implements: the one structural distinction per world that a type
//! can check, and nothing else.
//!
//! What is deliberately absent is a per-entity criteria table. `bioprism_onco::taxonomy` already
//! carries one worked instantiation and documents it as invented; five more here would be five
//! more inventions, and the entity-specific molecular content of these modules
//! (30.20's "H3, TP53, ACVR1, PDGFRA", 30.23's "IDH, 1p/19q, MGMT, TERT, EGFR, chromosome 7/10,
//! CDKN2A/B") appears in the blueprint explicitly qualified — "and other relevant alteration
//! evidence without assuming completeness", "and other evidence as available". A closed enum built
//! from a list the source says is open would misrepresent it.
//!
//! # The five distinctions
//!
//! | Module | Failure named | What is checked here |
//! | --- | --- | --- |
//! | 30.20 | "mixing diagnosis and autopsy tissue without modeling selection" | [`pool_provenance`] |
//! | 30.21 | "treating all BRAF alterations as equivalent" | [`pool_alterations`] |
//! | 30.22 | "reporting macro performance without case counts" | [`RarePerformanceReport::publish`] |
//! | 30.23 | "ignoring recurrence selection" | [`pool_provenance`], reused |
//! | 30.24 | "treating lesions as independent patients", "ignoring systemic death" | [`declare_cluster`], [`handle_event`] |
//!
//! 30.20 and 30.23 share a check because they name the same failure: material that reached a
//! biobank through a different route is a differently selected sample of tumours, whether the route
//! was an autopsy or a second surgery.
//!
//! # Not implemented
//!
//! No entity criteria, no molecular tables, no grading, no outcome model, no competing-risk
//! estimator. [`handle_event`] decides whether a declared handling is admissible for an endpoint;
//! it does not analyse anything.

use crate::error::EntityWorldRefusal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How material reached the study (30.20, 30.23).
///
/// The three routes are the ones those modules name: "biopsy, autopsy, liquid, and model-system
/// lineage" and "diagnosis and recurrence specimens". Each is a different selection of tumours,
/// which is why they are not interchangeable even for one participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TissueProvenance {
    DiagnosticBiopsy,
    RecurrenceResection,
    Postmortem,
}

impl TissueProvenance {
    pub const fn as_str(self) -> &'static str {
        match self {
            TissueProvenance::DiagnosticBiopsy => "diagnostic biopsy",
            TissueProvenance::RecurrenceResection => "recurrence resection",
            TissueProvenance::Postmortem => "postmortem",
        }
    }
}

/// Whether material from two routes may be analysed as one group.
///
/// Same route: yes. Different routes: only with the selection between them modelled. There is no
/// ranking of routes here and no assertion that one is better — postmortem multi-region material
/// is often the *richer* evidence, which is exactly why 30.20's microbenchmark is about a target
/// strong there and weak in diagnostic biopsies.
pub fn pool_provenance(
    left: TissueProvenance,
    right: TissueProvenance,
    selection_modelled: bool,
) -> Result<(), EntityWorldRefusal> {
    if left == right || selection_modelled {
        return Ok(());
    }
    Err(EntityWorldRefusal::UnmodelledProvenanceSelection {
        left: left.as_str().to_string(),
        right: right.as_str().to_string(),
    })
}

/// The alteration mechanisms 30.21 lists: "fusion, variant, copy-number, and pathway evidence".
///
/// Mechanism, not gene. 30.21's microbenchmark is "two tumors share pathway activation but differ
/// in alteration mechanism and natural history", so the distinction that matters is how the
/// pathway came to be activated, and naming the genes would add nothing checkable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlterationMechanism {
    Fusion,
    SequenceVariant,
    CopyNumberChange,
}

impl AlterationMechanism {
    pub const fn as_str(self) -> &'static str {
        match self {
            AlterationMechanism::Fusion => "fusion",
            AlterationMechanism::SequenceVariant => "sequence variant",
            AlterationMechanism::CopyNumberChange => "copy-number change",
        }
    }
}

/// Whether two alteration mechanisms in one pathway may be pooled.
///
/// Only under a stated estimand. 30.21's benchmark asks "whether pooling is defensible for a
/// specified estimand" — the answer depends on the question, so the question has to be present.
pub fn pool_alterations(
    left: AlterationMechanism,
    right: AlterationMechanism,
    estimand: Option<&str>,
) -> Result<(), EntityWorldRefusal> {
    if left == right || estimand.is_some_and(|estimand| !estimand.trim().is_empty()) {
        return Ok(());
    }
    Err(EntityWorldRefusal::MechanismCollapse {
        left: left.as_str().to_string(),
        right: right.as_str().to_string(),
    })
}

/// Whether a benchmark can be formed at all (30.22 ladder item 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "feasibility", rename_all = "snake_case")]
pub enum BenchmarkFeasibility {
    Feasible,
    /// Some classes have no cases. Reported rather than dropped: a class the evaluation cannot
    /// speak about is a finding about the evaluation.
    InfeasibleForClasses { classes: Vec<String> },
}

/// Classes with zero cases, if any.
///
/// No minimum-n rule. 30.22 states none, and any number chosen here would silently decide which
/// rare entities are allowed to be studied.
pub fn feasibility(counts: &BTreeMap<String, usize>) -> BenchmarkFeasibility {
    let empty: Vec<String> = counts
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(class, _)| class.clone())
        .collect();
    if empty.is_empty() {
        BenchmarkFeasibility::Feasible
    } else {
        BenchmarkFeasibility::InfeasibleForClasses { classes: empty }
    }
}

/// A macro-averaged performance figure and the counts it averaged over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RarePerformanceReport {
    pub macro_score: f64,
    #[serde(default)]
    pub per_class_counts: BTreeMap<String, usize>,
}

/// A published rare-entity report, which always carries its case counts.
///
/// No public constructor; produced only by [`RarePerformanceReport::publish`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublishedPerformance {
    report: RarePerformanceReport,
    feasibility: BenchmarkFeasibility,
}

impl PublishedPerformance {
    pub fn report(&self) -> &RarePerformanceReport {
        &self.report
    }

    pub fn feasibility(&self) -> &BenchmarkFeasibility {
        &self.feasibility
    }
}

impl RarePerformanceReport {
    /// Whether this report may be published.
    ///
    /// A macro average over classes whose sizes are not shown is the failure 30.22 names first: it
    /// weights a class of three the same as a class of three hundred and hides that it did.
    pub fn publish(self) -> Result<PublishedPerformance, EntityWorldRefusal> {
        if self.per_class_counts.is_empty() {
            return Err(EntityWorldRefusal::MacroScoreWithoutCounts);
        }
        let feasibility = feasibility(&self.per_class_counts);
        Ok(PublishedPerformance {
            report: self,
            feasibility,
        })
    }
}

/// A set of lesions and the participants they came from (30.24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LesionSet {
    pub lesions: usize,
    pub participants: usize,
}

/// Whether a lesion-level analysis has declared its participant cluster.
///
/// When there are more lesions than participants, some participant contributed several, and an
/// analysis that treats each lesion as independent has invented sample size. Equal counts need no
/// declaration; that is the one-lesion-per-participant case.
pub fn declare_cluster(
    set: LesionSet,
    cluster_declared: bool,
) -> Result<(), EntityWorldRefusal> {
    if set.lesions <= set.participants || cluster_declared {
        return Ok(());
    }
    Err(EntityWorldRefusal::UndeclaredCluster {
        lesions: set.lesions,
        participants: set.participants,
    })
}

/// The endpoints 30.24 names: "local-control and toxicity endpoints", against patient-level
/// "competing systemic progression or death".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LesionEndpoint {
    LocalControl,
    OverallSurvival,
}

/// Something that can happen to a participant during follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpEvent {
    LocalProgression,
    SystemicDeath,
    LostToFollowUp,
}

impl FollowUpEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            FollowUpEvent::LocalProgression => "local progression",
            FollowUpEvent::SystemicDeath => "death from systemic disease",
            FollowUpEvent::LostToFollowUp => "loss to follow-up",
        }
    }
}

/// How an analysis proposes to treat an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventHandling {
    Event,
    Censoring,
    CompetingRisk,
}

/// Whether an event may be handled that way under this endpoint.
///
/// Death precludes local progression, so under a local-control endpoint it is a competing risk and
/// censoring it assumes the participant could still have progressed. `bioprism_onco::outcome`
/// already draws the censoring-is-not-an-event line for patient-level endpoints; this is the
/// lesion-level corner 30.24 adds, where the competing event belongs to the participant and the
/// endpoint belongs to the lesion.
pub fn handle_event(
    endpoint: LesionEndpoint,
    event: FollowUpEvent,
    handling: EventHandling,
) -> Result<(), EntityWorldRefusal> {
    let refused = matches!(
        (endpoint, event, handling),
        (
            LesionEndpoint::LocalControl,
            FollowUpEvent::SystemicDeath,
            EventHandling::Censoring | EventHandling::Event
        )
    );
    if refused {
        return Err(EntityWorldRefusal::CompetingEventAsCensoring {
            event: event.as_str().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_and_postmortem_material_are_differently_selected_samples() {
        let refusal = pool_provenance(
            TissueProvenance::DiagnosticBiopsy,
            TissueProvenance::Postmortem,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            EntityWorldRefusal::UnmodelledProvenanceSelection { .. }
        ));
        assert!(pool_provenance(
            TissueProvenance::DiagnosticBiopsy,
            TissueProvenance::Postmortem,
            true
        )
        .is_ok());
    }

    #[test]
    fn recurrence_material_is_selected_the_same_way_autopsy_material_is() {
        assert!(pool_provenance(
            TissueProvenance::DiagnosticBiopsy,
            TissueProvenance::RecurrenceResection,
            false
        )
        .is_err());
        assert!(pool_provenance(
            TissueProvenance::RecurrenceResection,
            TissueProvenance::RecurrenceResection,
            false
        )
        .is_ok());
    }

    #[test]
    fn two_alteration_mechanisms_in_one_pathway_are_not_one_group_by_default() {
        let refusal = pool_alterations(
            AlterationMechanism::Fusion,
            AlterationMechanism::SequenceVariant,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            EntityWorldRefusal::MechanismCollapse { .. }
        ));
    }

    #[test]
    fn pooling_mechanisms_becomes_available_once_the_estimand_is_stated() {
        assert!(pool_alterations(
            AlterationMechanism::Fusion,
            AlterationMechanism::SequenceVariant,
            Some("time to next systemic therapy under either mechanism"),
        )
        .is_ok());
        assert!(pool_alterations(
            AlterationMechanism::Fusion,
            AlterationMechanism::SequenceVariant,
            Some("   "),
        )
        .is_err());
    }

    #[test]
    fn a_macro_average_without_case_counts_cannot_be_published() {
        let report = RarePerformanceReport {
            macro_score: 0.88,
            per_class_counts: BTreeMap::new(),
        };
        assert_eq!(
            report.publish().unwrap_err(),
            EntityWorldRefusal::MacroScoreWithoutCounts
        );
    }

    #[test]
    fn a_class_with_no_cases_makes_the_benchmark_infeasible_for_that_class() {
        let counts: BTreeMap<String, usize> = [
            ("common group".to_string(), 300),
            ("rare subgroup".to_string(), 0),
        ]
        .into_iter()
        .collect();
        let published = RarePerformanceReport {
            macro_score: 0.88,
            per_class_counts: counts,
        }
        .publish()
        .expect("counts are present");
        assert_eq!(
            published.feasibility(),
            &BenchmarkFeasibility::InfeasibleForClasses {
                classes: vec!["rare subgroup".to_string()]
            }
        );
    }

    #[test]
    fn a_benchmark_with_cases_in_every_class_is_feasible_however_small_the_classes() {
        let counts: BTreeMap<String, usize> = [
            ("common group".to_string(), 300),
            ("rare subgroup".to_string(), 3),
        ]
        .into_iter()
        .collect();
        assert_eq!(feasibility(&counts), BenchmarkFeasibility::Feasible);
    }

    #[test]
    fn lesions_are_not_independent_patients() {
        let set = LesionSet {
            lesions: 41,
            participants: 12,
        };
        assert_eq!(
            declare_cluster(set, false).unwrap_err(),
            EntityWorldRefusal::UndeclaredCluster {
                lesions: 41,
                participants: 12
            }
        );
        assert!(declare_cluster(set, true).is_ok());
    }

    #[test]
    fn one_lesion_per_participant_needs_no_cluster_declaration() {
        let set = LesionSet {
            lesions: 12,
            participants: 12,
        };
        assert!(declare_cluster(set, false).is_ok());
    }

    #[test]
    fn systemic_death_is_a_competing_risk_for_local_control_not_censoring() {
        assert!(matches!(
            handle_event(
                LesionEndpoint::LocalControl,
                FollowUpEvent::SystemicDeath,
                EventHandling::Censoring
            )
            .unwrap_err(),
            EntityWorldRefusal::CompetingEventAsCensoring { .. }
        ));
        assert!(handle_event(
            LesionEndpoint::LocalControl,
            FollowUpEvent::SystemicDeath,
            EventHandling::CompetingRisk
        )
        .is_ok());
    }

    #[test]
    fn systemic_death_is_the_event_itself_under_overall_survival() {
        assert!(handle_event(
            LesionEndpoint::OverallSurvival,
            FollowUpEvent::SystemicDeath,
            EventHandling::Event
        )
        .is_ok());
        assert!(handle_event(
            LesionEndpoint::LocalControl,
            FollowUpEvent::LostToFollowUp,
            EventHandling::Censoring
        )
        .is_ok());
    }
}
