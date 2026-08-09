//! The crate scanning itself, because the register is the one document that must not cite.
//!
//! # The trap
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears anywhere
//! under `crates/` or `docs/`. It says so, and says the criterion is weak on purpose. This crate is
//! a list of eighty-four modules that are *not* covered, living under `crates/`. Written the
//! obvious way it would mark all eighty-four covered and take the headline to 100% — a number
//! produced entirely by the document complaining that the number is produced that way.
//!
//! This is not hypothetical. `docs/BACKLOG.md` emptied itself on its second run, because the run
//! that generated it counted the file it had just written. `crates/sweep`'s citation test cited the
//! two ids it existed to assert were uncited, and caught itself only because the check it was
//! writing happened to run over its own source. Both are recorded in the crates that made them.
//!
//! # What is enforced, and where
//!
//! Two mechanisms, because a convention is not a mechanism.
//!
//! [`crate::module::ModuleKey`] holds a section and an index as integers and has no constructor
//! taking a string, so an id cannot enter the register as data. That covers the register.
//!
//! [`scan`] reproduces the coverage script's token rule without a regex engine and [`audit`] runs
//! it over this crate's whole directory — source, tests and manifest, every extension the script
//! would read — and fails if any token found belongs to a module the register classifies. That
//! covers the prose, which is where an id would actually get written.
//!
//! # Two things it deliberately does not do
//!
//! It does not distinguish a citation from a decimal number, because the coverage script does not
//! either. `crates/devplat` nearly shipped a measured percentage written to two decimal places
//! whose integer part fell in the section range; as far as the tool is concerned that is a
//! citation. The honest response is to notice and round, not to teach the audit an exception the
//! tool it mirrors does not have. Every figure in this crate is written to one decimal place.
//!
//! It does not forbid *every* id. A module this register does not classify is one somebody already
//! cited, so naming it moves no number — but the audit reports those separately as
//! [`CitationAudit::unclassified`] rather than silently allowing them, because an id appearing in a
//! register of uncovered modules is worth a second look even when it is harmless.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::entry::Register;
use crate::error::CitationError;

/// Every blueprint-module-shaped token in a string, in the coverage script's sense.
///
/// `NN.MM` where `NN` is 01 to 49, `MM` is any two digits, and neither side abuts a word character
/// or another digit. Returned sorted and deduplicated.
///
/// Reimplemented rather than shelled out to, for the reason `crates/devplat` gives: a test that
/// invokes the script tests the script, and a test that reimplements its rule fails when the two
/// drift — which is the event worth catching.
pub fn scan(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    for start in 0..bytes.len() {
        if start + 5 > bytes.len() {
            break;
        }
        if start > 0 && is_word(bytes[start - 1]) {
            continue;
        }
        let window = &bytes[start..start + 5];
        if !window[0].is_ascii_digit()
            || !window[1].is_ascii_digit()
            || window[2] != b'.'
            || !window[3].is_ascii_digit()
            || !window[4].is_ascii_digit()
        {
            continue;
        }
        if start + 5 < bytes.len() && is_word(bytes[start + 5]) {
            continue;
        }
        let section = (window[0] - b'0') * 10 + (window[1] - b'0');
        if !(1..=49).contains(&section) {
            continue;
        }
        found.insert(String::from_utf8_lossy(window).into_owned());
    }
    found
}

/// The ids no file of this crate may contain: one per module the register classifies.
///
/// Assembled from components at run time. This function is the only place the whole forbidden set
/// exists as strings, and it exists there for exactly as long as the audit takes.
pub fn forbidden_ids(register: &Register) -> BTreeSet<String> {
    register
        .entries()
        .iter()
        .map(|entry| entry.key().id())
        .collect()
}

/// What a directory tree cites, split by whether the register explains the module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationAudit {
    /// Every token found, whatever its provenance.
    pub found: BTreeSet<String>,
    /// Tokens naming a module this register classifies as uncovered. Each one silently marks that
    /// module covered and is a defect.
    pub forbidden: BTreeSet<String>,
    /// Tokens naming some other module. Harmless for the coverage figure, reported anyway.
    pub unclassified: BTreeSet<String>,
    /// Files read, so a caller can tell an empty result from an empty scan.
    pub files_scanned: usize,
}

impl CitationAudit {
    /// Whether this crate cites no module it says is uncovered.
    pub fn is_clean(&self) -> bool {
        self.forbidden.is_empty()
    }

    /// One line per defect, in the order a reader would fix them.
    pub fn defects(&self) -> Vec<String> {
        self.forbidden
            .iter()
            .map(|token| {
                format!(
                    "`{token}` appears in this crate and names a module this register classifies \
                     as uncovered: writing it marks the module covered without covering it"
                )
            })
            .collect()
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "scanned {} files, found {} module-shaped tokens\n",
            self.files_scanned,
            self.found.len()
        );
        if self.forbidden.is_empty() {
            out.push_str("no uncovered module is cited by its own register\n");
        } else {
            out.push_str(&format!("{} forbidden:\n", self.forbidden.len()));
            for defect in self.defects() {
                out.push_str(&format!("  - {defect}\n"));
            }
        }
        if !self.unclassified.is_empty() {
            out.push_str(&format!(
                "{} token(s) name modules outside the register: {}\n",
                self.unclassified.len(),
                self.unclassified
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out
    }
}

/// This crate's own directory, resolved at compile time.
///
/// Compile-time rather than a runtime search, for the reason `bioprism-cookbook` gives about the
/// workspace root: a search would silently succeed against a different checkout, and the whole
/// value of the audit is that it checks the tree it was built from.
pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Scan a directory tree and split its citations against a register.
///
/// Reads `.rs`, `.toml` and `.md`, which is what a crate directory holds and what the coverage
/// script would read. Refuses rather than returns a clean report when nothing was read, because an
/// empty scan and a clean crate produce the same output and only one of them is good news.
pub fn audit(root: &Path, register: &Register) -> Result<CitationAudit, CitationError> {
    if !root.is_dir() {
        return Err(CitationError::NotADirectory {
            path: root.display().to_string(),
        });
    }
    let mut files = Vec::new();
    collect(root, &mut files)?;
    files.sort();
    let mut found = BTreeSet::new();
    for file in &files {
        let text = fs::read_to_string(file).map_err(|error| CitationError::Unreadable {
            path: file.display().to_string(),
            reason: error.to_string(),
        })?;
        found.extend(scan(&text));
    }
    if files.is_empty() {
        return Err(CitationError::NothingScanned {
            path: root.display().to_string(),
        });
    }
    let forbidden_set = forbidden_ids(register);
    let forbidden = found.intersection(&forbidden_set).cloned().collect();
    let unclassified = found.difference(&forbidden_set).cloned().collect();
    Ok(CitationAudit {
        found,
        forbidden,
        unclassified,
        files_scanned: files.len(),
    })
}

/// Audit this crate against the register it ships.
pub fn audit_self(register: &Register) -> Result<CitationAudit, CitationError> {
    audit(&crate_root(), register)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CitationError> {
    let entries = fs::read_dir(dir).map_err(|error| CitationError::Unreadable {
        path: dir.display().to_string(),
        reason: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| CitationError::Unreadable {
            path: dir.display().to_string(),
            reason: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("target") {
                continue;
            }
            collect(&path, out)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs") | Some("toml") | Some("md")
        ) {
            out.push(path);
        }
    }
    Ok(())
}
