//! Reconciliation against the live backlog and the live workspace.
//!
//! A register of explanations is only worth having while it explains the modules that are actually
//! left. Two ways it stops doing that, and both are drift rather than error:
//!
//! - **A module leaves the backlog.** Four crates are being written against these sections right
//!   now; each citation deletes a line from `docs/BACKLOG.md`, and an entry here that outlives its
//!   module is an explanation for a gap that has been filled.
//! - **A module arrives.** `tools/backlog.sh` regenerates from the blueprint, so a module can enter
//!   the list without anybody touching this crate. An unexplained arrival is the register's
//!   equivalent of an uncovered module: honest only while it is visible.
//!
//! [`reconcile`] reports both directions and never repairs either. Repair is an edit somebody
//! makes, with a source; a function that silently dropped a stale entry would be manufacturing
//! agreement between two files, which is the failure `docs/BACKLOG.md` already suffered once when a
//! run counted the file it had just written.
//!
//! # It also checks the citations, not just the ids
//!
//! Each verdict names a crate, a file and a fragment of that file. All three are resolved against
//! the working tree: the crate must be a workspace member, the file must live inside that crate,
//! and the fragment must still be there. A reworded sentence in `crates/atlashub` is not a defect
//! in `crates/atlashub`; it is this register attributing to that crate something it no longer says,
//! which is exactly the failure `bioprism-cookbook`'s pinned quotations exist to make loud. The
//! mechanism is reused wholesale rather than rebuilt.
//!
//! # Text, not linkage
//!
//! Nothing here is a dependency of the crates it checks. The workspace is read as text through
//! `bioprism_cookbook::Workspace`, which already reads `Cargo.toml` for the member list and each
//! member's manifest for its package name. A register that could not be built while a classifying
//! crate was mid-edit would be unavailable exactly when somebody is changing a classification.

use std::collections::BTreeSet;

use bioprism_cookbook::{CrateName, QuoteStatus, Workspace};
use serde::{Deserialize, Serialize};

use crate::entry::Register;
use crate::error::ReconciliationError;

/// The backlog file this register reconciles against.
pub const BACKLOG_PATH: &str = "docs/BACKLOG.md";

/// One line of `docs/BACKLOG.md`, as parsed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BacklogLine {
    /// The dotted id, read out of the file rather than written into this crate.
    pub id: String,
    pub title: String,
}

/// A module the backlog and the register disagree about the existence of.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Drift {
    pub id: String,
    pub title: String,
}

/// A module both files hold under different titles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TitleDrift {
    pub id: String,
    pub in_backlog: String,
    pub in_register: String,
}

/// A verdict whose source no longer resolves.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceDefect {
    pub module: String,
    pub recorded_by: String,
    pub locus: String,
    pub problem: SourceProblem,
}

/// How a source failed to resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProblem {
    /// The crate the judgement is attributed to is not a workspace member.
    CrateNotInWorkspace,
    /// The file the judgement was recorded in could not be read.
    LocusUnreadable,
    /// The file exists but does not live inside the crate the judgement is attributed to. The
    /// judgement may well be real; it is not where this register says it is.
    LocusOutsideItsCrate,
    /// The file is there and the anchored fragment is not. Somebody reworded it, and this register
    /// is now attributing to them something they no longer say.
    AnchorReworded,
}

/// Everything the register asserts about the backlog and the workspace, resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reconciliation {
    /// Modules `docs/BACKLOG.md` lists that this register does not explain.
    pub unexplained: Vec<Drift>,
    /// Modules this register explains that the backlog no longer lists.
    pub stale: Vec<Drift>,
    /// Modules both hold, under different titles.
    pub retitled: Vec<TitleDrift>,
    /// Verdict sources that no longer resolve.
    pub source_defects: Vec<SourceDefect>,
    /// Crates named anywhere in the register that are not workspace members.
    pub unknown_crates: BTreeSet<String>,
    /// How many lines the backlog yielded, so an empty comparison is distinguishable from a
    /// backlog that parsed to nothing.
    pub backlog_entries: usize,
    pub register_entries: usize,
}

impl Reconciliation {
    /// Whether the register, the backlog and the workspace all agree.
    pub fn is_clean(&self) -> bool {
        self.unexplained.is_empty()
            && self.stale.is_empty()
            && self.retitled.is_empty()
            && self.source_defects.is_empty()
            && self.unknown_crates.is_empty()
    }

    /// One line per disagreement, in the order a reader would fix them.
    pub fn defects(&self) -> Vec<String> {
        let mut out = Vec::new();
        for drift in &self.unexplained {
            out.push(format!(
                "`{}` ({}) is in the backlog and this register does not explain it",
                drift.id, drift.title
            ));
        }
        for drift in &self.stale {
            out.push(format!(
                "`{}` ({}) is explained here and has left the backlog; delete the entry",
                drift.id, drift.title
            ));
        }
        for drift in &self.retitled {
            out.push(format!(
                "`{}` is `{}` in the backlog and `{}` here",
                drift.id, drift.in_backlog, drift.in_register
            ));
        }
        for defect in &self.source_defects {
            out.push(format!(
                "{}: the judgement attributed to `{}` in `{}` does not resolve ({:?})",
                defect.module, defect.recorded_by, defect.locus, defect.problem
            ));
        }
        for name in &self.unknown_crates {
            out.push(format!(
                "`{name}` is named in a verdict and is not a workspace member"
            ));
        }
        out
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "backlog {} modules, register {} modules\n",
            self.backlog_entries, self.register_entries
        );
        let defects = self.defects();
        if defects.is_empty() {
            out.push_str("register, backlog and workspace agree\n");
        } else {
            out.push_str(&format!("{} disagreement(s):\n", defects.len()));
            for defect in defects {
                out.push_str(&format!("  - {defect}\n"));
            }
        }
        out
    }
}

/// Read `docs/BACKLOG.md` and pull out its module lines.
///
/// The format is one bullet per module, id in backticks, title after it. Parsed with a
/// hand-written reader rather than a regex for the reason everything in this workspace is:
/// dependencies are pinned and offline. A file that yields no lines is refused rather than
/// reported as an empty backlog — a format change and an empty backlog produce the same value and
/// only one of them means the work is done.
pub fn parse_backlog(text: &str) -> Vec<BacklogLine> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        let Some(rest) = trimmed.strip_prefix("- `") else {
            continue;
        };
        let Some(close) = rest.find('`') else {
            continue;
        };
        let id = rest[..close].trim().to_string();
        if id.len() != 5 || !id.is_char_boundary(2) || &id[2..3] != "." {
            continue;
        }
        if !id.chars().enumerate().all(|(position, character)| {
            if position == 2 {
                character == '.'
            } else {
                character.is_ascii_digit()
            }
        }) {
            continue;
        }
        let title = rest[close + 1..].trim().to_string();
        lines.push(BacklogLine { id, title });
    }
    lines
}

/// Reconcile the register against the working tree it was built from.
pub fn reconcile(register: &Register) -> Result<Reconciliation, ReconciliationError> {
    let workspace = Workspace::here().map_err(|error| ReconciliationError::Unreadable {
        path: "Cargo.toml".to_string(),
        reason: error.to_string(),
    })?;
    reconcile_in(register, &workspace)
}

/// Reconcile against a workspace the caller opened, so a test can point at a fixture tree.
pub fn reconcile_in(
    register: &Register,
    workspace: &Workspace,
) -> Result<Reconciliation, ReconciliationError> {
    let text = workspace
        .read(BACKLOG_PATH)
        .map_err(|error| ReconciliationError::Unreadable {
            path: BACKLOG_PATH.to_string(),
            reason: error.to_string(),
        })?;
    let backlog = parse_backlog(&text);
    if backlog.is_empty() {
        return Err(ReconciliationError::BacklogParsedEmpty {
            path: BACKLOG_PATH.to_string(),
        });
    }

    let mut unexplained = Vec::new();
    let mut retitled = Vec::new();
    let registered: Vec<(String, &str)> = register
        .entries()
        .iter()
        .map(|entry| (entry.key().id(), entry.title()))
        .collect();

    for line in &backlog {
        match registered.iter().find(|(id, _)| *id == line.id) {
            None => unexplained.push(Drift {
                id: line.id.clone(),
                title: line.title.clone(),
            }),
            Some((_, title)) if !title.eq_ignore_ascii_case(&line.title) => {
                retitled.push(TitleDrift {
                    id: line.id.clone(),
                    in_backlog: line.title.clone(),
                    in_register: (*title).to_string(),
                })
            }
            Some(_) => {}
        }
    }

    let listed: BTreeSet<&str> = backlog.iter().map(|line| line.id.as_str()).collect();
    let stale: Vec<Drift> = registered
        .iter()
        .filter(|(id, _)| !listed.contains(id.as_str()))
        .map(|(id, title)| Drift {
            id: id.clone(),
            title: (*title).to_string(),
        })
        .collect();

    let mut unknown_crates = BTreeSet::new();
    for name in register.named_crates() {
        let known = CrateName::parse(name.as_str())
            .ok()
            .is_some_and(|parsed| workspace.contains_package(&parsed));
        if !known {
            unknown_crates.insert(name);
        }
    }

    let mut source_defects = Vec::new();
    for entry in register.entries() {
        for verdict in entry.verdicts() {
            let source = verdict.source();
            let recorded_by = source.recorded_by().to_string();
            let locus = source.locus().to_string();
            let mut push = |problem: SourceProblem| {
                source_defects.push(SourceDefect {
                    module: entry.title().to_string(),
                    recorded_by: recorded_by.clone(),
                    locus: locus.clone(),
                    problem,
                });
            };
            let Some(directory) = workspace.directory_of(source.recorded_by()) else {
                push(SourceProblem::CrateNotInWorkspace);
                continue;
            };
            if !source.locus().starts_with(&format!("{directory}/")) {
                push(SourceProblem::LocusOutsideItsCrate);
                continue;
            }
            match workspace.resolve_quote(source.anchor()) {
                QuoteStatus::Present => {}
                QuoteStatus::SourceUnreadable => push(SourceProblem::LocusUnreadable),
                QuoteStatus::Reworded => push(SourceProblem::AnchorReworded),
            }
        }
    }

    Ok(Reconciliation {
        unexplained,
        stale,
        retitled,
        source_defects,
        unknown_crates,
        backlog_entries: backlog.len(),
        register_entries: register.len(),
    })
}
