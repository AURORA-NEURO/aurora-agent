//! What the register adds up to, counted in a way that does not hide the one number that matters.
//!
//! # Why there is no single percentage here
//!
//! `docs/COVERAGE.md` reports one figure and spends four sections explaining what it does not mean.
//! A residue register invites the same collapse — *eighty-four left, seventy explained, that is
//! most of it* — and the collapse destroys the distinction the whole crate is built on. Four of the
//! five verdicts mean the module will **never** move; one means it still can. Averaging them gives
//! a number that improves when somebody reclassifies work as prose.
//!
//! So [`Distribution`] counts the five separately and [`Distribution::work_remaining`] is reported
//! beside them rather than folded in. `bioprism_safety::threat::Coverage` refuses to add its three
//! counts into a percentage for the same reason, and `crates/ops` follows it; this is the third
//! instance of one rule.
//!
//! # Counted twice, on purpose
//!
//! A module can carry several verdicts, so "how many modules are process" and "how many process
//! verdicts were recorded" are different questions with different answers. [`Distribution::modules`]
//! counts each module once, under the verdict its section's own classifying crate gave first;
//! [`Distribution::verdicts`] counts every recorded judgement. Where the two differ, a module is
//! compound or contested, and [`Report`] lists both sets rather than summarising them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::entry::{Entry, Register};
use crate::verdict::{Classification, Standing};

/// The five verdicts, as counting buckets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub process: usize,
    pub foreign_artifact: usize,
    pub discharged_elsewhere: usize,
    pub block_level_split: usize,
    pub genuinely_uncovered: usize,
}

impl Counts {
    pub fn total(&self) -> usize {
        self.process
            + self.foreign_artifact
            + self.discharged_elsewhere
            + self.block_level_split
            + self.genuinely_uncovered
    }

    fn add(&mut self, classification: &Classification) {
        match classification {
            Classification::Process => self.process += 1,
            Classification::ForeignArtifact { .. } => self.foreign_artifact += 1,
            Classification::DischargedElsewhere { .. } => self.discharged_elsewhere += 1,
            Classification::BlockLevelSplit { .. } => self.block_level_split += 1,
            Classification::GenuinelyUncovered { .. } => self.genuinely_uncovered += 1,
        }
    }

    /// The five rows, labelled, in the order the vocabulary was established in.
    pub fn rows(&self) -> [(&'static str, usize); 5] {
        [
            ("process", self.process),
            ("foreign artifact", self.foreign_artifact),
            ("discharged elsewhere", self.discharged_elsewhere),
            ("block-level split", self.block_level_split),
            ("genuinely uncovered", self.genuinely_uncovered),
        ]
    }
}

/// How the register distributes, counted per module and per verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Distribution {
    /// One count per module, under its primary verdict.
    pub modules: Counts,
    /// One count per recorded judgement.
    pub verdicts: Counts,
    /// Modules any verdict says still carry work. Never folded into the counts above, because a
    /// module can be discharged on one reading and unfinished on another and both are true.
    pub work_remaining: usize,
    /// Verdicts a classifying crate stated about the module named.
    pub transcribed: usize,
    /// Verdicts this register drew from a crate's text about something else.
    pub inferred_here: usize,
}

impl Distribution {
    pub fn of(register: &Register) -> Self {
        let mut modules = Counts::default();
        let mut verdicts = Counts::default();
        let mut transcribed = 0;
        let mut inferred_here = 0;
        for entry in register.entries() {
            modules.add(entry.primary().classification());
            for verdict in entry.verdicts() {
                verdicts.add(verdict.classification());
                match verdict.standing() {
                    Standing::Transcribed => transcribed += 1,
                    Standing::InferredHere => inferred_here += 1,
                }
            }
        }
        Distribution {
            modules,
            verdicts,
            work_remaining: register.work_remaining().len(),
            transcribed,
            inferred_here,
        }
    }
}

/// One module a reader should look at, with why it was singled out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Highlight {
    pub section: String,
    pub title: String,
    pub detail: String,
}

impl Highlight {
    fn of(entry: &Entry, detail: String) -> Self {
        Highlight {
            section: entry.key().section_label(),
            title: entry.title().to_string(),
            detail,
        }
    }
}

/// The register, summarised without being flattened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub distribution: Distribution,
    /// Modules per section, so a reader can see which sections the residue concentrates in.
    pub by_section: BTreeMap<String, usize>,
    /// Modules two different crates classified differently. Recorded, never adjudicated.
    pub contested: Vec<Highlight>,
    /// Modules one crate gave more than one classification, because the module contains both kinds
    /// of content.
    pub compound: Vec<Highlight>,
    /// Modules where every verdict is this register's reading rather than a crate's stated one.
    pub inferred_only: Vec<Highlight>,
    /// Modules no crate has taken any position on. The honest floor of the whole exercise.
    pub nobody_has_read: Vec<Highlight>,
}

impl Report {
    pub fn of(register: &Register) -> Self {
        let mut by_section = BTreeMap::new();
        for entry in register.entries() {
            *by_section.entry(entry.key().section_label()).or_insert(0) += 1;
        }

        let contested = register
            .contested()
            .into_iter()
            .map(|entry| {
                let positions = entry
                    .contest()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, verdict)| format!("{name} says {verdict}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                Highlight::of(entry, positions)
            })
            .collect();

        let compound = register
            .compound()
            .into_iter()
            .map(|entry| {
                let kinds = entry
                    .classifications()
                    .into_iter()
                    .map(Classification::as_str)
                    .collect::<Vec<_>>()
                    .join(" + ");
                Highlight::of(entry, kinds)
            })
            .collect();

        let inferred_only = register
            .only_inferred()
            .into_iter()
            .map(|entry| Highlight::of(entry, entry.primary().describe()))
            .collect();

        let nobody_has_read = register
            .entries()
            .iter()
            .filter(|entry| {
                entry.verdicts().iter().any(|verdict| {
                    matches!(
                        verdict.classification(),
                        Classification::GenuinelyUncovered {
                            standing: crate::verdict::UncoveredStanding::NobodyHasRead { .. }
                        }
                    )
                })
            })
            .map(|entry| {
                let surveyed = entry
                    .primary()
                    .classification()
                    .named_crates()
                    .into_iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                Highlight::of(entry, format!("searched: {surveyed}"))
            })
            .collect();

        Report {
            distribution: Distribution::of(register),
            by_section,
            contested,
            compound,
            inferred_only,
            nobody_has_read,
        }
    }

    /// A plain-text rendering, stable for the same register.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{} uncovered modules across {} sections\n\n",
            self.distribution.modules.total(),
            self.by_section.len()
        ));

        out.push_str("by verdict (module, then recorded judgement)\n");
        for ((label, modules), (_, verdicts)) in self
            .distribution
            .modules
            .rows()
            .into_iter()
            .zip(self.distribution.verdicts.rows())
        {
            out.push_str(&format!("  {label:<22} {modules:>3}  {verdicts:>3}\n"));
        }
        out.push_str(&format!(
            "\n  work remaining on some reading: {}\n  transcribed {} · inferred here {}\n\n",
            self.distribution.work_remaining,
            self.distribution.transcribed,
            self.distribution.inferred_here
        ));

        out.push_str("by section\n");
        for (section, count) in &self.by_section {
            out.push_str(&format!("  {section} {count}\n"));
        }

        for (heading, group) in [
            ("contested — two crates, two verdicts", &self.contested),
            ("compound — one crate, two verdicts", &self.compound),
            (
                "explained only by this register's reading",
                &self.inferred_only,
            ),
            ("no crate has taken any position", &self.nobody_has_read),
        ] {
            out.push_str(&format!("\n{heading} ({})\n", group.len()));
            for highlight in group {
                out.push_str(&format!(
                    "  {} {} — {}\n",
                    highlight.section, highlight.title, highlight.detail
                ));
            }
        }
        out
    }
}
