//! The register: modules keyed by title, each carrying at least one sourced verdict.
//!
//! # Why the key is the title
//!
//! Because the id may not be written. [`crate::module::ModuleKey`] explains the mechanism; the
//! consequence is here, and it shapes the API: [`Register::find`] takes a title, [`Register::get`]
//! takes a key built from components, and there is no lookup taking a dotted string, because a
//! caller who had one would have had to write it down.
//!
//! Titles are the blueprint's own, spelled as `docs/BACKLOG.md` spells them, so that
//! [`crate::reconcile`] can match the two files without a normalisation table that would itself
//! become a place for drift to hide.
//!
//! # Several verdicts, and what that means
//!
//! An [`Entry`] holds a list, not one. Two shapes turn up in the workspace and they are different
//! findings:
//!
//! - **Compound** — one crate reaching two classifications about one module, because the module
//!   contains both kinds of content. `crates/ops` classified the monorepo module as process *and*
//!   named the crate that holds its one checkable sentence. That is not indecision; it is the
//!   block-level split `crates/bioevalx` named, seen one module at a time.
//! - **Contested** — two crates reaching different classifications. `crates/atlasx` reports that
//!   the capability-metrics remainder defines nothing, and `bioprism-metrics` reports that it
//!   already implements the arithmetic governing all of it. Both readings are recorded and neither
//!   is adjudicated here, because picking a winner would delete the evidence that the workspace
//!   has two answers.
//!
//! [`Register::contested`] and [`Entry::is_compound`] separate them, and the report prints both.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::RegisterError;
use crate::module::ModuleKey;
use crate::verdict::{Classification, Standing, Verdict};

/// One uncovered blueprint module and every judgement recorded about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EntryFields", into = "EntryFields")]
pub struct Entry {
    key: ModuleKey,
    title: String,
    verdicts: Vec<Verdict>,
}

/// The wire form of an [`Entry`], and its only construction path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryFields {
    pub key: ModuleKey,
    pub title: String,
    pub verdicts: Vec<Verdict>,
}

impl Entry {
    /// An entry, or a refusal naming what it lacked.
    ///
    /// An entry with no verdict is refused rather than allowed and flagged later, because that
    /// value is exactly a `docs/BACKLOG.md` line — a module id and a title with no explanation —
    /// and this crate exists to be the thing that is not that.
    pub fn new(key: ModuleKey, title: &str, verdicts: Vec<Verdict>) -> Result<Self, RegisterError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(RegisterError::TitleMissing);
        }
        if verdicts.is_empty() {
            return Err(RegisterError::NoVerdict {
                title: title.to_string(),
            });
        }
        Ok(Entry {
            key,
            title: title.to_string(),
            verdicts,
        })
    }

    pub fn key(&self) -> ModuleKey {
        self.key
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn verdicts(&self) -> &[Verdict] {
        &self.verdicts
    }

    /// The verdict a reader should read first: the one the section's own classifying crate gave.
    ///
    /// Position, not precedence — the register is written with the primary verdict first, and the
    /// alternatives after it. A function returning "the strongest" verdict would be adjudicating,
    /// which is the one thing this crate must not do.
    pub fn primary(&self) -> &Verdict {
        &self.verdicts[0]
    }

    /// The distinct classifications recorded, in the order first seen.
    pub fn classifications(&self) -> Vec<&Classification> {
        let mut seen: Vec<&Classification> = Vec::new();
        for verdict in &self.verdicts {
            if !seen.iter().any(|held| *held == verdict.classification()) {
                seen.push(verdict.classification());
            }
        }
        seen
    }

    /// Whether one crate reached more than one classification about this module.
    pub fn is_compound(&self) -> bool {
        self.crates_with_more_than_one_classification()
            .next()
            .is_some()
    }

    fn crates_with_more_than_one_classification(&self) -> impl Iterator<Item = String> + '_ {
        let mut by_crate: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
        for verdict in &self.verdicts {
            by_crate
                .entry(verdict.recorded_by().to_string())
                .or_default()
                .insert(verdict.classification().as_str());
        }
        by_crate
            .into_iter()
            .filter(|(_, kinds)| kinds.len() > 1)
            .map(|(name, _)| name)
    }

    /// Whether two *different* crates classified this module differently.
    ///
    /// Deliberately not the same question as [`Entry::is_compound`]. One crate holding two
    /// readings of one module is a finding about the module; two crates holding two readings is a
    /// finding about the workspace.
    pub fn is_contested(&self) -> bool {
        self.contest().is_some()
    }

    /// The two sides of a contest, if there is one: `(crate, verdict)` pairs that differ.
    pub fn contest(&self) -> Option<Vec<(String, &'static str)>> {
        let mut positions: Vec<(String, &'static str)> = Vec::new();
        for verdict in &self.verdicts {
            let position = (
                verdict.recorded_by().to_string(),
                verdict.classification().as_str(),
            );
            if !positions.contains(&position) {
                positions.push(position);
            }
        }
        let differing: BTreeSet<&'static str> = positions.iter().map(|(_, kind)| *kind).collect();
        if differing.len() < 2 {
            return None;
        }
        let crates: BTreeSet<&String> = positions.iter().map(|(name, _)| name).collect();
        if crates.len() < 2 {
            return None;
        }
        Some(positions)
    }

    /// Whether any recorded verdict says work remains.
    ///
    /// Any, not all: if one crate says the content is discharged and another says the distributed
    /// half is absent, a reader planning work needs to see the second.
    pub fn has_work_remaining(&self) -> bool {
        self.verdicts
            .iter()
            .any(|verdict| verdict.classification().is_work_remaining())
    }

    /// Whether every verdict on this module is this register's own reading rather than a
    /// classifying crate's stated one.
    ///
    /// The count of these is the honest ceiling on how much of the register is transcription. A
    /// reader who trusts the classifying crates and not this one should read this set first.
    pub fn is_entirely_inferred(&self) -> bool {
        self.verdicts
            .iter()
            .all(|verdict| verdict.standing() == Standing::InferredHere)
    }

    /// A one-line row for a report.
    pub fn describe(&self) -> String {
        format!(
            "{} {} — {}",
            self.key.section_label(),
            self.title,
            self.verdicts
                .iter()
                .map(Verdict::describe)
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

impl TryFrom<EntryFields> for Entry {
    type Error = RegisterError;

    fn try_from(fields: EntryFields) -> Result<Self, Self::Error> {
        Entry::new(fields.key, &fields.title, fields.verdicts)
    }
}

impl From<Entry> for EntryFields {
    fn from(entry: Entry) -> Self {
        EntryFields {
            key: entry.key,
            title: entry.title,
            verdicts: entry.verdicts,
        }
    }
}

/// Every uncovered module, with the reason no crate implements it.
///
/// Ordered by key, so two runs of a report over the same register produce the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Entry>", into = "Vec<Entry>")]
pub struct Register {
    entries: Vec<Entry>,
}

impl Register {
    /// A register, or a refusal naming the duplicate.
    ///
    /// Sorts by key on the way in. Callers therefore cannot express an ordering, which is what
    /// makes a rendered report a function of the content alone.
    pub fn new(mut entries: Vec<Entry>) -> Result<Self, RegisterError> {
        entries.sort_by_key(Entry::key);
        for pair in entries.windows(2) {
            if pair[0].key() == pair[1].key() {
                return Err(RegisterError::DuplicateKey);
            }
            if pair[0].title().eq_ignore_ascii_case(pair[1].title()) {
                return Err(RegisterError::DuplicateTitle {
                    title: pair[0].title().to_string(),
                });
            }
        }
        let mut titles: BTreeSet<String> = BTreeSet::new();
        for entry in &entries {
            if !titles.insert(entry.title().to_ascii_lowercase()) {
                return Err(RegisterError::DuplicateTitle {
                    title: entry.title().to_string(),
                });
            }
        }
        Ok(Register { entries })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look a module up by the blueprint's title, case-insensitively.
    pub fn find(&self, title: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.title().eq_ignore_ascii_case(title.trim()))
    }

    /// Look a module up by a key the caller assembled from components.
    pub fn get(&self, key: ModuleKey) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.key() == key)
    }

    /// The modules of one section, in blueprint order.
    pub fn section(&self, section: u8) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.key().section() == section)
            .collect()
    }

    /// Every section present, in order.
    pub fn sections(&self) -> Vec<u8> {
        let mut sections: Vec<u8> = self
            .entries
            .iter()
            .map(|entry| entry.key().section())
            .collect();
        sections.sort_unstable();
        sections.dedup();
        sections
    }

    /// Modules two different crates classified differently.
    pub fn contested(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_contested())
            .collect()
    }

    /// Modules where one crate reached more than one classification.
    pub fn compound(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_compound())
            .collect()
    }

    /// Modules any verdict says still carry work.
    pub fn work_remaining(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.has_work_remaining())
            .collect()
    }

    /// Modules explained only by this register's own reading of somebody else's text.
    pub fn only_inferred(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_entirely_inferred())
            .collect()
    }

    /// Every distinct crate named anywhere in the register: as an author of a judgement, as a
    /// discharger, or as a crate that was searched.
    ///
    /// [`crate::reconcile`] checks the whole set against the workspace's actual member list, so a
    /// renamed or deleted crate turns the register red rather than leaving it pointing at nothing.
    pub fn named_crates(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for entry in &self.entries {
            for verdict in entry.verdicts() {
                names.insert(verdict.recorded_by().to_string());
                for named in verdict.classification().named_crates() {
                    names.insert(named.to_string());
                }
            }
        }
        names
    }

    /// Drop a module that has left the backlog.
    ///
    /// A module leaving is the normal case, not a rewrite: one of the four crates being written
    /// alongside this one cites a module, `tools/backlog.sh` stops listing it, and the entry is
    /// removed here. Returns whether anything was removed, so a regeneration script can tell a
    /// no-op from a change.
    pub fn without(&mut self, key: ModuleKey) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.key() != key);
        self.entries.len() != before
    }
}

impl TryFrom<Vec<Entry>> for Register {
    type Error = RegisterError;

    fn try_from(entries: Vec<Entry>) -> Result<Self, Self::Error> {
        Register::new(entries)
    }
}

impl From<Register> for Vec<Entry> {
    fn from(register: Register) -> Self {
        register.entries
    }
}
