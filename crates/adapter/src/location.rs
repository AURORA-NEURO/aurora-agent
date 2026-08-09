//! Where in a source something was found — or lost.
//!
//! Blueprint 40.17 requires that semantic loss be *reported*, not merely counted. A count is
//! not actionable: an engineer who reads "3 columns dropped" cannot fix anything. Every loss
//! entry therefore names the exact place in the source it refers to, and the same type is used
//! to name the places that were successfully mapped, so that conformance (40.32) can compare
//! the two sets against an independent inventory of the source.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A stable locator for one addressable place inside a source.
///
/// The four components form a coarse-to-fine path: which source, which artifact inside it,
/// which record inside that, which field inside that. Adapters fill in only the components
/// they can honestly determine; a `None` means "this granularity does not apply here", never
/// "unknown". The ordering derived here is the one the loss report and the field inventory are
/// sorted by, which is what makes both byte-stable across runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Identifier of the source as declared by the caller, e.g. `cohort-2024`.
    pub source: String,
    /// Path of the artifact within the source, slash-separated, relative to the source root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    /// One-based record ordinal within the artifact. Record 1 of a CSV is the first data row,
    /// not the header; the header is addressed with `record: None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<u64>,
    /// Field name within the record — a column name for tabular sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl SourceLocation {
    /// The source as a whole. Used for losses that are properties of the source rather than of
    /// any one field, such as an absent upstream accession.
    pub fn source(source: impl Into<String>) -> Self {
        SourceLocation {
            source: source.into(),
            artifact: None,
            record: None,
            field: None,
        }
    }

    /// One artifact within a repository source.
    pub fn artifact(source: impl Into<String>, artifact: impl Into<String>) -> Self {
        SourceLocation {
            source: source.into(),
            artifact: Some(artifact.into()),
            record: None,
            field: None,
        }
    }

    /// One column of a tabular source, addressed independently of any row.
    ///
    /// This is the granularity at which loss completeness is decided for tables: a column is
    /// either mapped or lost, and the answer may not vary row by row, because a per-row answer
    /// would let an adapter drop a field in the rows nobody inspected.
    pub fn column(source: impl Into<String>, field: impl Into<String>) -> Self {
        SourceLocation {
            source: source.into(),
            artifact: None,
            record: None,
            field: Some(field.into()),
        }
    }

    /// One cell: a column within a specific record.
    pub fn cell(source: impl Into<String>, record: u64, field: impl Into<String>) -> Self {
        SourceLocation {
            source: source.into(),
            artifact: None,
            record: Some(record),
            field: Some(field.into()),
        }
    }

    /// One record of a tabular source.
    pub fn record(source: impl Into<String>, record: u64) -> Self {
        SourceLocation {
            source: source.into(),
            artifact: None,
            record: Some(record),
            field: None,
        }
    }

    /// Narrows this location to a specific artifact, keeping record and field.
    pub fn within(mut self, artifact: impl Into<String>) -> Self {
        self.artifact = Some(artifact.into());
        self
    }

    /// The coarser location that owns this one: a cell's column, a column's source.
    ///
    /// Conformance uses this to accept a loss declared at column granularity as covering the
    /// cells beneath it, without accepting the reverse.
    pub fn generalized(&self) -> Option<SourceLocation> {
        if self.record.is_some() {
            return Some(SourceLocation {
                record: None,
                ..self.clone()
            });
        }
        if self.field.is_some() {
            return Some(SourceLocation {
                field: None,
                ..self.clone()
            });
        }
        if self.artifact.is_some() {
            return Some(SourceLocation::source(self.source.clone()));
        }
        None
    }

    /// True when `self` is this location or any location that owns it.
    pub fn covers(&self, finer: &SourceLocation) -> bool {
        let mut candidate = Some(finer.clone());
        while let Some(current) = candidate {
            if &current == self {
                return true;
            }
            candidate = current.generalized();
        }
        false
    }

    /// A stable single-line rendering, used in fact provenance and in error messages.
    pub fn locator(&self) -> String {
        let mut out = self.source.clone();
        if let Some(artifact) = &self.artifact {
            out.push('/');
            out.push_str(artifact);
        }
        let mut sep = '#';
        if let Some(record) = self.record {
            out.push(sep);
            out.push_str(&format!("record={record}"));
            sep = '&';
        }
        if let Some(field) = &self.field {
            out.push(sep);
            out.push_str(&format!("field={field}"));
        }
        out
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.locator())
    }
}

/// An ordered, de-duplicated set of locations.
///
/// A `BTreeSet` rather than a `Vec` because both the mapped set and the source inventory are
/// compared for equality and printed in reports; insertion order would otherwise leak into
/// content hashes and make a re-run of the same ingest produce different bytes.
pub type LocationSet = BTreeSet<SourceLocation>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_location_covers_every_cell_in_that_column() {
        let column = SourceLocation::column("s", "age");
        assert!(column.covers(&SourceLocation::cell("s", 7, "age")));
        assert!(column.covers(&column));
    }

    #[test]
    fn a_cell_location_does_not_cover_its_whole_column() {
        let cell = SourceLocation::cell("s", 7, "age");
        assert!(!cell.covers(&SourceLocation::column("s", "age")));
    }

    #[test]
    fn locations_in_different_sources_never_cover_each_other() {
        let a = SourceLocation::column("a", "age");
        let b = SourceLocation::cell("b", 1, "age");
        assert!(!a.covers(&b));
        assert!(!b.covers(&a));
    }

    #[test]
    fn the_locator_rendering_is_stable_and_names_every_component() {
        let location = SourceLocation::cell("cohort", 3, "tumor_volume").within("labs/a.csv");
        assert_eq!(
            location.locator(),
            "cohort/labs/a.csv#record=3&field=tumor_volume"
        );
    }
}
