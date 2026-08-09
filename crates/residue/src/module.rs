//! Blueprint module identity, built from components so it is never written down.
//!
//! # Why this type exists at all, when a `&'static str` would do
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears
//! anywhere under `crates/` or `docs/`. This crate's entire subject is the modules that *are not*
//! covered, so a register that spelled its keys out would mark all of them covered and drive the
//! headline to 100% — the precise dishonesty the coverage document exists to avoid. Two crates
//! have already been bitten: `docs/BACKLOG.md` emptied itself on its second run, and
//! `crates/sweep`'s citation test cited the very ids it asserted were uncited.
//!
//! So a [`ModuleKey`] is a pair of small integers and [`ModuleKey::id`] renders one at runtime.
//! There is no constructor taking a string, and no `Display` impl — a key that could be formatted
//! into a literal by a careless `format!("{key}")` in a doc example would put the token back into
//! the source, and the whole mechanism is that it cannot get there.
//!
//! The serialized form is `{"section": 11, "index": 4}` for the same reason: a stored register is a
//! file under `crates/`, and an `id` field holding the dotted form would be a citation of every
//! module in it.
//!
//! # What the type does not defend against
//!
//! Nothing here stops an author writing a dotted id in a doc comment beside a key. That is
//! [`crate::citations`]'s job, and it is a scan over this crate's own source rather than a type,
//! because "no token of this shape appears in this text" is not a property any value can hold.
//!
//! The first draft of *this paragraph* demonstrated the point by spelling an id out to show what
//! the serialized form must not look like, and the scan caught it. That is the third time a crate
//! in this workspace has cited the module it was explaining while explaining why it must not.

use serde::{Deserialize, Serialize};

use crate::error::RegisterError;

/// The lowest section number `tools/coverage.sh` will read as a citation.
pub const FIRST_SECTION: u8 = 1;

/// The highest section number `tools/coverage.sh` will read as a citation.
///
/// The blueprint ships 44 sections; the script's pattern reaches 49. The wider bound is used here
/// so that this crate refuses exactly what the script would have counted, rather than a subset of
/// it.
pub const LAST_SECTION: u8 = 49;

/// A blueprint module's identity, held as components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "ModuleKeyFields")]
pub struct ModuleKey {
    section: u8,
    index: u8,
}

/// The wire form of a [`ModuleKey`], which is also its only construction path.
///
/// Public because `serde` needs it, and because a caller building a register from JSON should be
/// able to name the shape it is building from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModuleKeyFields {
    pub section: u8,
    pub index: u8,
}

impl ModuleKey {
    /// A key, or a refusal naming which component was out of range.
    pub fn new(section: u8, index: u8) -> Result<Self, RegisterError> {
        if !(FIRST_SECTION..=LAST_SECTION).contains(&section) {
            return Err(RegisterError::SectionOutOfRange { section });
        }
        if index == 0 {
            return Err(RegisterError::IndexOutOfRange { index });
        }
        Ok(ModuleKey { section, index })
    }

    pub fn section(&self) -> u8 {
        self.section
    }

    pub fn index(&self) -> u8 {
        self.index
    }

    /// The dotted id, assembled at run time.
    ///
    /// This is the one place in the crate where an uncovered module's id exists as a string, and
    /// it exists only inside a running process. Callers rendering a report to a file under
    /// `crates/` or `docs/` are writing citations; that is a decision for the caller and it is why
    /// this function is named for what it produces rather than for what it looks like.
    pub fn id(&self) -> String {
        format!("{:02}.{:02}", self.section, self.index)
    }

    /// The section, rendered the way the workspace's prose spells it.
    pub fn section_label(&self) -> String {
        format!("§{:02}", self.section)
    }
}

impl TryFrom<ModuleKeyFields> for ModuleKey {
    type Error = RegisterError;

    fn try_from(fields: ModuleKeyFields) -> Result<Self, Self::Error> {
        ModuleKey::new(fields.section, fields.index)
    }
}
