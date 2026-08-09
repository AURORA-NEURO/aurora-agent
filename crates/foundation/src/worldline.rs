//! BioWorldlines: eight clocks, and the leakage they exist to prevent.
//!
//! Blueprint 24.09 separates eight times and gives the reason in one sentence: "later-recorded
//! information may describe earlier biology, and a model must not receive evidence that was
//! unavailable at the decision time." Collapse the clocks into one timestamp and that sentence
//! becomes uncheckable, because the thing that makes the evidence inadmissible (when it was
//! *recorded*) is not the thing that makes it relevant (when the biology *happened*).
//!
//! [`Stamped::admissible_at`] therefore checks the record clock and ignores the biological one.
//! A pathology report describing a tumour that existed for years is inadmissible at a decision
//! taken before the report was filed, and the test for that is the one worth writing.
//!
//! The second enforceable idea is fork honesty. 24.09: "Real-world retrospective data do not
//! justify arbitrary treatment counterfactuals. The fork metadata states whether a branch is
//! replayed, semi-synthetic, simulated, or purely hypothetical."
//! [`BranchKind::licenses_treatment_counterfactual`] answers `false` for every kind, including
//! simulated — a simulator licenses a claim about the simulator, which
//! [`worldclass::CounterfactualClaim::SimulatedIntervention`] already covers.
//!
//! Not implemented: alignment of longitudinal specimens and images, and distinguishing true
//! change from acquisition variation. Both are listed in 24.09 as temporal evaluation families,
//! and both need registration and noise models rather than types.
//!
//! [`worldclass::CounterfactualClaim::SimulatedIntervention`]: crate::worldclass::CounterfactualClaim::SimulatedIntervention

use crate::error::TemporalError;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The eight clocks blueprint 24.09 keeps apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    /// When the biology happened. Often unknown, frequently much earlier than everything else.
    BiologicalEvent,
    SpecimenCollection,
    AssayAcquisition,
    /// When the information entered a record a system could read. The leakage clock.
    Record,
    Analysis,
    PublicationOrDatabaseUpdate,
    /// When the system under evaluation had to commit.
    Decision,
    /// When the benchmark unseals a withheld outcome.
    EvaluationReveal,
}

impl Clock {
    pub const ALL: [Clock; 8] = [
        Clock::BiologicalEvent,
        Clock::SpecimenCollection,
        Clock::AssayAcquisition,
        Clock::Record,
        Clock::Analysis,
        Clock::PublicationOrDatabaseUpdate,
        Clock::Decision,
        Clock::EvaluationReveal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Clock::BiologicalEvent => "biological event",
            Clock::SpecimenCollection => "specimen collection",
            Clock::AssayAcquisition => "assay acquisition",
            Clock::Record => "record",
            Clock::Analysis => "analysis",
            Clock::PublicationOrDatabaseUpdate => "publication or database update",
            Clock::Decision => "decision",
            Clock::EvaluationReveal => "evaluation reveal",
        }
    }

    /// Whether this clock governs availability to a system under evaluation.
    ///
    /// Record time is the primary gate. Publication time gates literature-derived evidence,
    /// which is why a model must not cite a paper that did not exist yet even though the
    /// biology it describes is older than the decision.
    pub fn gates_availability(self) -> bool {
        matches!(self, Clock::Record | Clock::PublicationOrDatabaseUpdate)
    }
}

/// A piece of evidence with its clocks attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamped {
    pub evidence: String,
    pub clocks: BTreeMap<Clock, Timestamp>,
}

impl Stamped {
    pub fn new(evidence: impl Into<String>) -> Self {
        Stamped {
            evidence: evidence.into(),
            clocks: BTreeMap::new(),
        }
    }

    pub fn at(mut self, clock: Clock, timestamp: Timestamp) -> Self {
        self.clocks.insert(clock, timestamp);
        self
    }

    pub fn get(&self, clock: Clock) -> Option<Timestamp> {
        self.clocks.get(&clock).copied()
    }

    /// Whether this evidence may be handed to a system deciding at `decision_time`.
    ///
    /// Unstamped is refused, not waved through: evidence that cannot be dated cannot be shown
    /// to have been available, and the whole leakage guarantee rests on it. Availability is
    /// judged only on gating clocks — the biological event clock may sit anywhere.
    pub fn admissible_at(&self, decision_time: Timestamp) -> Result<(), TemporalError> {
        let mut gated = false;
        for clock in Clock::ALL.into_iter().filter(|c| c.gates_availability()) {
            let Some(stamp) = self.get(clock) else {
                continue;
            };
            gated = true;
            if stamp > decision_time {
                return Err(TemporalError::Leakage {
                    evidence: self.evidence.clone(),
                    recorded: stamp.to_rfc3339(),
                    decided: decision_time.to_rfc3339(),
                });
            }
        }
        if gated {
            Ok(())
        } else {
            Err(TemporalError::UnstampedClock {
                evidence: self.evidence.clone(),
                clock: Clock::Record.as_str(),
            })
        }
    }
}

/// What kind of thing a worldline branch is (24.09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    /// The same evidence, a different analysis.
    Replayed,
    /// Real observations with an injected known change.
    SemiSynthetic,
    /// Produced by a declared model.
    Simulated,
    /// Asserted, with nothing behind it. Legal to explore, never legal to score as evidence.
    Hypothetical,
}

impl BranchKind {
    pub const ALL: [BranchKind; 4] = [
        BranchKind::Replayed,
        BranchKind::SemiSynthetic,
        BranchKind::Simulated,
        BranchKind::Hypothetical,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BranchKind::Replayed => "replayed",
            BranchKind::SemiSynthetic => "semi-synthetic",
            BranchKind::Simulated => "simulated",
            BranchKind::Hypothetical => "hypothetical",
        }
    }

    /// No branch kind licenses a real-world treatment counterfactual.
    ///
    /// This is a constant function, and it is deliberately written as one rather than omitted:
    /// the question gets asked, and the answer being uniformly `false` is the finding.
    pub fn licenses_treatment_counterfactual(self) -> bool {
        false
    }

    /// Whether results from this branch may be scored as evidence about the world at all.
    pub fn is_evidential(self) -> bool {
        !matches!(self, BranchKind::Hypothetical)
    }
}

/// A fork of a worldline, with the metadata 24.09 requires it to state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub kind: BranchKind,
    /// The model a simulated branch was produced by. Required for `Simulated`, since a claim
    /// from a simulator is only ever as strong as the named simulator.
    pub model: Option<String>,
    pub forked_at: Timestamp,
}

impl Branch {
    /// Refuses a treatment counterfactual on any branch, and names the branch kind that was
    /// hoped to license it.
    pub fn claim_treatment_counterfactual(&self) -> Result<(), TemporalError> {
        Err(TemporalError::UnjustifiedCounterfactualBranch {
            branch: self.id.clone(),
            kind: self.kind.as_str(),
        })
    }

    /// A simulated branch without a named model is untraceable to any validity argument.
    pub fn check(&self) -> Result<(), TemporalError> {
        if self.kind == BranchKind::Simulated
            && self
                .model
                .as_ref()
                .is_none_or(|model| model.trim().is_empty())
        {
            return Err(TemporalError::UnjustifiedCounterfactualBranch {
                branch: self.id.clone(),
                kind: "simulated without a declared model",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(text: &str) -> Timestamp {
        Timestamp::parse(text).unwrap()
    }

    #[test]
    fn evidence_recorded_after_the_decision_leaks_even_when_it_describes_earlier_biology() {
        let report = Stamped::new("pathology-report:0001")
            .at(Clock::BiologicalEvent, time("2019-03-01T00:00:00Z"))
            .at(Clock::Record, time("2021-06-01T00:00:00Z"));
        let err = report
            .admissible_at(time("2020-01-01T00:00:00Z"))
            .unwrap_err();
        assert_eq!(
            err,
            TemporalError::Leakage {
                evidence: "pathology-report:0001".to_string(),
                recorded: "2021-06-01T00:00:00Z".to_string(),
                decided: "2020-01-01T00:00:00Z".to_string()
            }
        );
    }

    #[test]
    fn old_biology_recorded_before_the_decision_is_admissible() {
        let report = Stamped::new("pathology-report:0001")
            .at(Clock::BiologicalEvent, time("2015-01-01T00:00:00Z"))
            .at(Clock::Record, time("2019-11-01T00:00:00Z"));
        assert!(report.admissible_at(time("2020-01-01T00:00:00Z")).is_ok());
    }

    #[test]
    fn a_paper_published_after_the_decision_leaks_even_if_its_subject_is_older() {
        let paper = Stamped::new("doi:10.1000/x")
            .at(Clock::BiologicalEvent, time("2001-01-01T00:00:00Z"))
            .at(Clock::Record, time("2019-01-01T00:00:00Z"))
            .at(
                Clock::PublicationOrDatabaseUpdate,
                time("2022-01-01T00:00:00Z"),
            );
        assert!(matches!(
            paper.admissible_at(time("2020-01-01T00:00:00Z")).unwrap_err(),
            TemporalError::Leakage { .. }
        ));
    }

    #[test]
    fn evidence_with_no_gating_clock_is_refused_rather_than_assumed_available() {
        let orphan = Stamped::new("mystery-file")
            .at(Clock::BiologicalEvent, time("2018-01-01T00:00:00Z"))
            .at(Clock::AssayAcquisition, time("2018-02-01T00:00:00Z"));
        assert!(matches!(
            orphan
                .admissible_at(time("2020-01-01T00:00:00Z"))
                .unwrap_err(),
            TemporalError::UnstampedClock { .. }
        ));
    }

    #[test]
    fn only_record_and_publication_clocks_gate_availability() {
        let gating: Vec<Clock> = Clock::ALL
            .into_iter()
            .filter(|c| c.gates_availability())
            .collect();
        assert_eq!(
            gating,
            vec![Clock::Record, Clock::PublicationOrDatabaseUpdate]
        );
    }

    #[test]
    fn no_branch_kind_licenses_a_real_world_treatment_counterfactual() {
        for kind in BranchKind::ALL {
            assert!(!kind.licenses_treatment_counterfactual());
        }
    }

    #[test]
    fn a_hypothetical_branch_may_be_explored_but_is_not_evidential() {
        assert!(!BranchKind::Hypothetical.is_evidential());
        assert!(BranchKind::Replayed.is_evidential());
        assert!(BranchKind::Simulated.is_evidential());
    }

    #[test]
    fn a_simulated_branch_without_a_named_model_is_refused() {
        let branch = Branch {
            id: "branch:0007".to_string(),
            kind: BranchKind::Simulated,
            model: None,
            forked_at: time("2020-01-01T00:00:00Z"),
        };
        assert!(branch.check().is_err());
        let named = Branch {
            model: Some("gbm-growth-ode@0.3".to_string()),
            ..branch
        };
        assert!(named.check().is_ok());
    }

    #[test]
    fn asking_a_replayed_branch_for_a_treatment_counterfactual_names_the_branch_and_refuses() {
        let branch = Branch {
            id: "branch:0001".to_string(),
            kind: BranchKind::Replayed,
            model: None,
            forked_at: time("2020-01-01T00:00:00Z"),
        };
        assert_eq!(
            branch.claim_treatment_counterfactual().unwrap_err(),
            TemporalError::UnjustifiedCounterfactualBranch {
                branch: "branch:0001".to_string(),
                kind: "replayed"
            }
        );
    }
}
