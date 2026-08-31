//! Reading a source's structure without any mapping policy.
//!
//! This module is what makes the loss-completeness check in [`crate::conformance`] worth
//! anything. If the adapter told conformance both what the source contains and what it mapped,
//! the check would reduce to "the adapter agrees with itself", which every adapter passes,
//! including one that never looks at half the file. The probe reads the source independently,
//! knows nothing about any mapping, and cannot be influenced by the adapter under test.
//!
//! It is correspondingly narrow: it can enumerate the columns of a table and the files of a
//! directory, and it refuses to guess for anything else. An [`Inventory::Unverifiable`] result
//! is honest — conformance then reports loss completeness as not applicable rather than as
//! passed, because an unverifiable claim is not a verified one.

use crate::csv::Table;
use crate::error::AdapterError;
use crate::location::{LocationSet, SourceLocation};
use crate::source::{Locator, Source};
use std::path::{Path, PathBuf};

/// Media types the probe can enumerate fields for.
pub const CSV_FORMATS: [&str; 2] = ["text/csv", "text/tab-separated-values"];
pub const MAX_DIRECTORY_ENTRIES: usize = 1_000_000;
pub const MAX_WALK_DEPTH: usize = 1024;
pub const MAX_RELATIVE_PATH_BYTES: usize = 4096;

/// An independent reading of what a source contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inventory {
    /// Every addressable field in the source, at the granularity conformance decides at:
    /// columns for tables, artifacts for repositories.
    Known(LocationSet),
    /// The probe cannot enumerate this source, and says so instead of returning an empty set.
    /// An empty `Known` would mean "this source has no fields", which is a different claim.
    Unverifiable { reason: String },
}

impl Inventory {
    pub fn locations(&self) -> Option<&LocationSet> {
        match self {
            Inventory::Known(locations) => Some(locations),
            Inventory::Unverifiable { .. } => None,
        }
    }
}

/// Enumerates the fields of `source`.
///
/// Dispatch is on the caller's *declared* format, never on the bytes. Content sniffing here
/// would defeat the purpose: a probe that guesses the format could disagree with the adapter
/// about what the source even is, and conformance would report a loss-completeness failure
/// that is really a disagreement between two guesses.
pub fn field_inventory(source: &Source) -> Result<Inventory, AdapterError> {
    source.validate()?;
    match &source.locator {
        Locator::Directory(root) => {
            let mut locations = LocationSet::new();
            for entry in walk(root)? {
                locations.insert(SourceLocation::artifact(&source.id, entry.relative));
            }
            Ok(Inventory::Known(locations))
        }
        Locator::Bytes(bytes) => match source.declared_format.as_deref() {
            Some(format) if CSV_FORMATS.contains(&format) => {
                let delimiter = if format == "text/tab-separated-values" {
                    b'\t'
                } else {
                    crate::csv::COMMA
                };
                let table = Table::parse_with(&source.id, bytes, delimiter)?;
                Ok(Inventory::Known(
                    table
                        .headers()
                        .iter()
                        .map(|name| SourceLocation::column(&source.id, name))
                        .collect(),
                ))
            }
            Some(format) => Ok(Inventory::Unverifiable {
                reason: format!("no structural probe is implemented for format {format:?}"),
            }),
            None => Ok(Inventory::Unverifiable {
                reason: "source declares no format, and the probe does not sniff content".into(),
            }),
        },
    }
}

/// One filesystem entry found by [`walk`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileEntry {
    /// Path relative to the walk root, always slash-separated so that a repository walked on
    /// Windows and the same repository walked on Linux produce identical facts.
    pub relative: String,
    pub absolute: PathBuf,
    /// Byte length, absent for entries whose length is not a meaningful property.
    pub length: Option<u64>,
    /// Symbolic links are recorded but never followed. Following them would make the walk
    /// non-terminating on a cyclic tree and would let a link outside the root pull unrelated
    /// bytes into the world under a path that misdescribes where they came from.
    pub is_symlink: bool,
}

/// Recursively enumerates `root` in a deterministic order.
///
/// Directory iteration order is filesystem-defined and differs between machines and even
/// between runs after a rename, so the result is sorted by relative path. Nothing is skipped:
/// no hidden-file rule, no ignore file. A walker that silently omits `.git` or `.DS_Store` is
/// making a mapping decision, and mapping decisions belong to adapters, which must declare
/// their omissions as loss.
pub fn walk(root: &Path) -> Result<Vec<FileEntry>, AdapterError> {
    let mut entries = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new(), 0usize)];

    while let Some((directory, prefix, depth)) = stack.pop() {
        if depth > MAX_WALK_DEPTH {
            return Err(AdapterError::TraversalLimit {
                path: display_path(&directory),
                maximum: MAX_WALK_DEPTH,
            });
        }
        let read = std::fs::read_dir(&directory)
            .map_err(|error| AdapterError::io(display_path(&directory), &error))?;
        for item in read {
            let item = item.map_err(|error| AdapterError::io(display_path(&directory), &error))?;
            let name = item.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if relative.len() > MAX_RELATIVE_PATH_BYTES {
                return Err(AdapterError::TraversalLimit {
                    path: relative,
                    maximum: MAX_RELATIVE_PATH_BYTES,
                });
            }
            let file_type = item
                .file_type()
                .map_err(|error| AdapterError::io(relative.clone(), &error))?;

            if file_type.is_symlink() {
                if entries.len() >= MAX_DIRECTORY_ENTRIES {
                    return Err(AdapterError::TraversalLimit {
                        path: display_path(root),
                        maximum: MAX_DIRECTORY_ENTRIES,
                    });
                }
                entries.push(FileEntry {
                    relative,
                    absolute: item.path(),
                    length: None,
                    is_symlink: true,
                });
            } else if file_type.is_dir() {
                stack.push((item.path(), relative, depth + 1));
            } else {
                let metadata = item
                    .metadata()
                    .map_err(|error| AdapterError::io(relative.clone(), &error))?;
                if entries.len() >= MAX_DIRECTORY_ENTRIES {
                    return Err(AdapterError::TraversalLimit {
                        path: display_path(root),
                        maximum: MAX_DIRECTORY_ENTRIES,
                    });
                }
                entries.push(FileEntry {
                    relative,
                    absolute: item.path(),
                    length: Some(metadata.len()),
                    is_symlink: false,
                });
            }
        }
    }

    entries.sort();
    Ok(entries)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_source_inventories_its_columns_not_its_cells() {
        let source =
            Source::bytes("s", b"subject,age\nS1,40\nS2,41\n".to_vec()).with_format("text/csv");
        let inventory = field_inventory(&source).unwrap();
        let locations = inventory.locations().unwrap();
        assert_eq!(locations.len(), 2);
        assert!(locations.contains(&SourceLocation::column("s", "age")));
    }

    #[test]
    fn a_source_with_no_declared_format_is_unverifiable_rather_than_empty() {
        let source = Source::bytes("s", b"subject,age\n".to_vec());
        assert!(matches!(
            field_inventory(&source).unwrap(),
            Inventory::Unverifiable { .. }
        ));
    }

    #[test]
    fn an_unknown_declared_format_is_unverifiable_and_says_which_format() {
        let source = Source::bytes("s", b"\x00\x01".to_vec()).with_format("application/dicom");
        match field_inventory(&source).unwrap() {
            Inventory::Unverifiable { reason } => assert!(reason.contains("application/dicom")),
            other => panic!("expected an unverifiable inventory, got {other:?}"),
        }
    }

    #[test]
    fn source_metadata_is_validated_before_independent_probing() {
        let source = Source::bytes("", b"a,b\n1,2\n".to_vec()).with_format("text/csv");
        assert!(matches!(
            field_inventory(&source),
            Err(AdapterError::InvalidSource(_))
        ));
    }
}
