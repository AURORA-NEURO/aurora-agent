//! One report that states the unflattering things first.
//!
//! Blueprint 09.01's learning loop ends with "promotion requires holdout and safety gates", and the
//! purpose of a report is to make that promotion checkable by a reader rather than by the system
//! that wants it. So the ordering here is fixed and is not a presentation choice:
//!
//! 1. **Contaminated measurement attempts.** If the system tried to measure on a burned holdout,
//!    that is the first thing a reader needs, because everything below it is conditional on the
//!    holdout policy actually being obeyed.
//! 2. **Remaining holdout budget.** A release that spends the last query of the last certification
//!    surface has bought its number by retiring the instrument.
//! 3. **What branching cost**, before what it caught.
//! 4. **Whether the front resolved**, with "it did not" as a first-class outcome.
//! 5. **Improvements**, last, each carrying its own defeaters.
//!
//! `bioprism-routing`'s report does the same thing one level down: it checks "the router lost"
//! before every other case, and on the shipped reference world that check fires — the router
//! captures **0% of available gain** under regime holdout and abstains on every task. This crate
//! does not improve that number and does not try to; it is the sibling result that the lab's own
//! self-improvement machinery has to be measured against. An evolution card claiming a routing gain
//! would be a card whose surface is the same regime holdout that produced the zero.
//!
//! Not implemented: no rendering beyond markdown, no HTML, no plots, and no aggregate score. There
//! is deliberately no single number at the top of this report.

use crate::evolution::EvolutionArchive;
use crate::holdout::HoldoutLedger;
use crate::hypothesis::SeparationVerdict;
use crate::pareto::{Direction, Selection};
use crate::risk::{BranchVerdict, BranchYield};
use serde::{Deserialize, Serialize};

/// The assembled lab report.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LabReport {
    /// Cards whose measurement surface was burned, with the refusal each one hit.
    pub contaminated_attempts: Vec<String>,
    /// Certification surfaces with queries left, and how many.
    pub remaining_budget: Vec<(String, u32)>,
    /// Certification surfaces that repeated optimization has retired.
    pub retired_surfaces: Vec<String>,
    pub branching: Option<BranchYield>,
    pub selection: Option<Selection>,
    pub separation: Option<SeparationVerdict>,
    /// One sentence per improvement, each carrying its surface and its defeaters.
    pub improvements: Vec<String>,
}

impl LabReport {
    /// Assembles a report from the parts a lab run produces.
    ///
    /// Takes the archive and the ledger rather than pre-computed counts, so the contaminated-card
    /// section cannot be assembled without the archive that would populate it. A report built from
    /// summary numbers is a report whose worst section can be omitted by passing zero.
    pub fn assemble(
        archive: &EvolutionArchive,
        ledger: &HoldoutLedger,
        direction: Direction,
    ) -> Self {
        let contaminated_attempts = archive
            .contaminated()
            .iter()
            .filter_map(|card| match card.surface() {
                crate::evolution::MeasurementSurface::Contaminated(record) => {
                    Some(format!("`{}`: {}", card.id, record.refusal))
                }
                crate::evolution::MeasurementSurface::Clean { .. } => None,
            })
            .collect();
        let remaining_budget = ledger
            .remaining_certification_budget()
            .into_iter()
            .map(|(id, left)| (id.to_string(), left))
            .collect();
        let retired_surfaces = ledger
            .iter()
            .filter(|holdout| holdout.partition.certifies() && holdout.is_retired())
            .map(|holdout| holdout.id.to_string())
            .collect();
        LabReport {
            contaminated_attempts,
            remaining_budget,
            retired_surfaces,
            branching: None,
            selection: None,
            separation: None,
            improvements: archive
                .improvements(direction)
                .iter()
                .map(|claim| claim.to_sentence())
                .collect(),
        }
    }

    pub fn with_branching(mut self, branching: BranchYield) -> Self {
        self.branching = Some(branching);
        self
    }

    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// Whether anything in the report should stop a release.
    ///
    /// True on a contaminated attempt, on a retired certification surface, and on branching that
    /// spent budget and caught nothing. Each of those is a statement about the *instrument* rather
    /// than about the system's quality, which is why they gate rather than merely inform.
    pub fn blocks_release(&self) -> bool {
        !self.contaminated_attempts.is_empty()
            || !self.retired_surfaces.is_empty()
            || matches!(
                self.branching.as_ref().map(BranchYield::verdict),
                Some(BranchVerdict::PaidAndCaughtNothing { .. })
            )
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Inference lab report\n\n");

        out.push_str("## Measurement surface\n\n");
        if self.contaminated_attempts.is_empty() {
            out.push_str("No card was measured on a burned holdout.\n\n");
        } else {
            out.push_str(&format!(
                "**{} card(s) were measured on a burned holdout and cannot be reported as improvements:**\n\n",
                self.contaminated_attempts.len()
            ));
            for attempt in &self.contaminated_attempts {
                out.push_str(&format!("- {attempt}\n"));
            }
            out.push('\n');
        }
        if !self.retired_surfaces.is_empty() {
            out.push_str(&format!(
                "**Retired by repeated optimization: {}.** A score on any of these is not a certification.\n\n",
                self.retired_surfaces.join(", ")
            ));
        }
        if self.remaining_budget.is_empty() {
            out.push_str("No certification surface has queries left.\n\n");
        } else {
            out.push_str("Remaining certification budget:\n\n");
            for (id, left) in &self.remaining_budget {
                out.push_str(&format!("- `{id}`: {left} quer(y/ies)\n"));
            }
            out.push('\n');
        }

        out.push_str("## Risk-triggered branching\n\n");
        match &self.branching {
            None => out.push_str("Not exercised.\n\n"),
            Some(report) => {
                out.push_str(&format!(
                    "{} decision(s), {} escalation(s) ({} of them on an unmeasured feature), \
                     {} branch(es) and {} verifier call(s) spent.\n\n",
                    report.decisions,
                    report.escalations,
                    report.escalations_on_undetermined,
                    report.spent.branches,
                    report.spent.verifier_calls
                ));
                out.push_str(&match report.verdict() {
                    BranchVerdict::NothingTriggered => {
                        "**Nothing triggered.** The controller spent nothing and proved nothing.\n\n"
                            .to_string()
                    }
                    BranchVerdict::PaidAndCaughtNothing { spent, escalations } => format!(
                        "**Paid and caught nothing.** {escalations} escalation(s) spent {} branch(es) and {} verifier call(s) and found no problem.\n\n",
                        spent.branches, spent.verifier_calls
                    ),
                    BranchVerdict::Mixed {
                        catches,
                        wasted_escalations,
                        ..
                    } => format!(
                        "{wasted_escalations} escalation(s) caught nothing; {catches} caught something.\n\n"
                    ),
                    BranchVerdict::EveryEscalationCaughtSomething { catches, .. } => {
                        format!("Every escalation caught something ({catches}).\n\n")
                    }
                });
                if report.escaped_without_escalation > 0 {
                    out.push_str(&format!(
                        "{} harm(s) got through where no rule fired: the trigger set has false negatives.\n\n",
                        report.escaped_without_escalation
                    ));
                }
            }
        }

        out.push_str("## Pareto front\n\n");
        match &self.selection {
            None => out.push_str("No front was built.\n\n"),
            Some(Selection::Empty) => out.push_str("The front is empty.\n\n"),
            Some(Selection::Unique { candidate }) => {
                out.push_str(&format!("One candidate survives: `{candidate}`.\n\n"));
            }
            Some(Selection::Ambiguous { front, unresolved }) => {
                out.push_str(&format!(
                    "{} candidates are mutually non-dominated. This is the answer, not a failure to choose:\n\n",
                    front.len()
                ));
                for candidate in front {
                    out.push_str(&format!("- `{candidate}`\n"));
                }
                out.push('\n');
                for entry in unresolved {
                    out.push_str(&format!(
                        "`{}` is unplaceable because {} was never measured.\n",
                        entry.candidate,
                        entry.axes.join(", ")
                    ));
                }
                if !unresolved.is_empty() {
                    out.push('\n');
                }
            }
        }

        out.push_str("## Hypothesis separation\n\n");
        match &self.separation {
            None => out.push_str("No hypothesis set was separated.\n\n"),
            Some(SeparationVerdict::NotSeparable { reason, .. }) => out.push_str(&format!(
                "**Not separable.** {}\n\n",
                match reason {
                    crate::hypothesis::NotSeparableReason::NoDisagreement =>
                        "the live hypotheses commit to the same things.".to_string(),
                    crate::hypothesis::NotSeparableReason::NoObligationCovers { loci } => format!(
                        "nothing in the obligation graph settles {}.",
                        loci.iter().map(|l| l.id()).collect::<Vec<_>>().join(", ")
                    ),
                    crate::hypothesis::NotSeparableReason::EveryDiscriminatorUnresolvable {
                        loci,
                    } => format!(
                        "every discriminator is unresolvable: {}.",
                        loci.iter().map(|l| l.id()).collect::<Vec<_>>().join(", ")
                    ),
                    crate::hypothesis::NotSeparableReason::EveryDiscriminatorInadmissible {
                        loci,
                    } => format!(
                        "every discriminator was closed by decision rather than evidence: {}.",
                        loci.iter().map(|l| l.id()).collect::<Vec<_>>().join(", ")
                    ),
                }
            )),
            Some(SeparationVerdict::Separable { obligations }) => out.push_str(&format!(
                "Not yet separated. Acquiring {} would separate them.\n\n",
                obligations
                    .iter()
                    .map(|entry| format!("`{}`", entry.locus.id()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Some(SeparationVerdict::Separated {
                surviving,
                retired,
                remaining,
                ..
            }) => {
                out.push_str(&format!(
                    "{} hypothesis(es) retired by evidence; {} surviving.\n\n",
                    retired.len(),
                    surviving.len()
                ));
                if !remaining.is_empty() {
                    out.push_str(&format!(
                        "{} disagreement(s) remain open among the survivors.\n\n",
                        remaining.len()
                    ));
                }
            }
        }

        out.push_str("## Improvements\n\n");
        if self.improvements.is_empty() {
            out.push_str("None reportable.\n\n");
        } else {
            for sentence in &self.improvements {
                out.push_str(&format!("- {sentence}\n"));
            }
            out.push('\n');
        }

        out.push_str(&format!(
            "**Release gate: {}.**\n",
            if self.blocks_release() {
                "blocked"
            } else {
                "nothing in this report blocks it"
            }
        ));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::BranchCost;

    fn empty_yield() -> BranchYield {
        BranchYield {
            decisions: 4,
            escalations: 2,
            escalations_on_undetermined: 0,
            spent: BranchCost {
                branches: 6,
                verifier_calls: 2,
            },
            catches: 0,
            wasted_escalations: 2,
            escaped_after_escalation: 0,
            escaped_without_escalation: 1,
            branches_per_catch: None,
        }
    }

    #[test]
    fn a_report_with_a_contaminated_attempt_blocks_the_release() {
        let report = LabReport {
            contaminated_attempts: vec!["`card-2`: holdout was already selected on".to_string()],
            ..LabReport::default()
        };
        assert!(report.blocks_release());
        assert!(report.to_markdown().contains("burned holdout"));
    }

    #[test]
    fn branching_that_caught_nothing_blocks_the_release_and_says_so_first() {
        let report = LabReport::default().with_branching(empty_yield());
        assert!(report.blocks_release());
        assert!(report.to_markdown().contains("Paid and caught nothing"));
    }

    #[test]
    fn an_ambiguous_front_is_rendered_as_the_answer_rather_than_as_an_error() {
        use crate::space::ConfigurationId;
        let report = LabReport::default().with_selection(Selection::Ambiguous {
            front: vec![ConfigurationId::new("cheap"), ConfigurationId::new("safe")],
            unresolved: Vec::new(),
        });
        let markdown = report.to_markdown();
        assert!(markdown.contains("This is the answer, not a failure to choose"));
        assert!(!report.blocks_release());
    }

    #[test]
    fn a_retired_certification_surface_blocks_the_release() {
        let report = LabReport {
            retired_surfaces: vec!["private-a".to_string()],
            ..LabReport::default()
        };
        assert!(report.blocks_release());
        assert!(report
            .to_markdown()
            .contains("Retired by repeated optimization"));
    }

    #[test]
    fn a_report_with_nothing_in_it_still_renders_every_section() {
        let markdown = LabReport::default().to_markdown();
        for heading in [
            "## Measurement surface",
            "## Risk-triggered branching",
            "## Pareto front",
            "## Hypothesis separation",
            "## Improvements",
        ] {
            assert!(markdown.contains(heading), "missing {heading}");
        }
    }

    #[test]
    fn a_report_carries_no_single_aggregate_score() {
        let markdown = LabReport::default().to_markdown();
        assert!(!markdown.contains("overall score"));
        assert!(!markdown.contains("Overall"));
    }
}
