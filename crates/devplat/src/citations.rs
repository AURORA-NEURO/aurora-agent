//! Grepping your own citation set, as a test rather than as a discipline.
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears
//! anywhere under `crates/` or `docs/`. The script says so, and says it is a weak criterion on
//! purpose. The failure mode that follows is not hypothetical: a crate that classifies a module as
//! process, writes the id while doing so, and thereby raises the coverage figure by exactly as much
//! as a crate that implemented it. Coverage then measures attention while reading as capability.
//!
//! [`scan`] reproduces the script's token rule — the same `\b(0[1-9]|[1-4][0-9])\.[0-9]{2}\b`
//! shape, without a regex engine — and [`audit`] compares what a directory tree actually contains
//! against a declared set. Pointed at this crate with
//! [`classify::implemented_module_ids`](crate::classify::implemented_module_ids), it fails if this
//! crate ever cites a module it did not implement.
//!
//! # Two things it deliberately does not do
//!
//! It does not distinguish a citation from a decimal number, because the coverage script does not
//! either. A percentage written to two decimal places, whose integer part falls in the section
//! range, is a citation as far as the tool is concerned. The honest response is to notice that and
//! round the percentage, not to teach the audit an exception the tool it mirrors does not have —
//! so every measurement reported in this crate is written to one decimal place.
//!
//! It does not read `docs/`. The rule this enforces is about what a crate claims; the workspace's
//! own documentation is a different question with a different owner.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::CitationError;

/// Every blueprint-module-shaped token in a string, in the coverage script's sense.
///
/// `NN.MM` where `NN` is 01 to 49, `MM` is any two digits, and neither side abuts an identifier
/// character. Returns them sorted and deduplicated.
pub fn scan(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
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

/// What a directory tree cites, against what it declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationAudit {
    /// Every token found, whatever its provenance.
    pub found: BTreeSet<String>,
    /// What the caller said it implemented.
    pub declared: BTreeSet<String>,
    /// Cited but not declared. Each one silently moves the coverage figure.
    pub undeclared: BTreeSet<String>,
    /// Declared but never written down. Coverage will not see it, so the work is invisible.
    pub uncited: BTreeSet<String>,
    /// Files read, so a caller can tell an empty result from an empty scan.
    pub files_scanned: usize,
}

impl CitationAudit {
    /// Whether the tree cites exactly what it declared.
    pub fn is_clean(&self) -> bool {
        self.undeclared.is_empty() && self.uncited.is_empty()
    }

    /// One line per defect, in the order a reader would fix them.
    pub fn defects(&self) -> Vec<String> {
        let mut defects = Vec::new();
        for token in &self.undeclared {
            defects.push(format!(
                "`{token}` appears in the source but is not a module this crate implemented: it \
                 raises coverage without raising capability"
            ));
        }
        for token in &self.uncited {
            defects.push(format!(
                "`{token}` is declared as implemented but appears nowhere in the source: coverage \
                 will not see it"
            ));
        }
        defects
    }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), CitationError> {
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
            collect(&path, out)?;
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs") | Some("toml") | Some("md")
        ) {
            out.push(path);
        }
    }
    Ok(())
}

/// Scan a directory tree and compare its citations against a declared set.
///
/// Reads `.rs`, `.toml` and `.md` files, which is what a crate directory contains and what the
/// coverage script would read.
pub fn audit<'a>(
    root: &Path,
    declared: impl IntoIterator<Item = &'a str>,
) -> Result<CitationAudit, CitationError> {
    if !root.is_dir() {
        return Err(CitationError::NotADirectory {
            path: root.display().to_string(),
        });
    }
    let declared: BTreeSet<String> = declared.into_iter().map(str::to_string).collect();
    let mut files = Vec::new();
    collect(root, &mut files)?;
    let mut found = BTreeSet::new();
    for file in &files {
        let text = fs::read_to_string(file).map_err(|error| CitationError::Unreadable {
            path: file.display().to_string(),
            reason: error.to_string(),
        })?;
        found.extend(scan(&text));
    }
    let undeclared = found.difference(&declared).cloned().collect();
    let uncited = declared.difference(&found).cloned().collect();
    Ok(CitationAudit {
        found,
        declared,
        undeclared,
        uncited,
        files_scanned: files.len(),
    })
}
