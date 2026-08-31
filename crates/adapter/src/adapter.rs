//! The adapter contract itself.
//!
//! Blueprint 40.17 names five interfaces — `probe`, `ingest`, `validate`, `views`, `export`.
//! Three of them are deliberately not in this trait.
//!
//! `validate` is not a separate method because a separable validation step is one that can be
//! skipped: [`Ingestion::new`](crate::Ingestion::new) validates unconditionally, so an adapter
//! cannot publish an unvalidated fragment even by accident. `views` and `export` belong to the
//! projection layer, which reads a world rather than producing one, and putting them here
//! would give every adapter author two unrelated jobs.
//!
//! `probe` is present as [`Adapter::manifest`] plus [`crate::probe::field_inventory`], split
//! in two because they answer to different parties. The manifest is the adapter's own
//! declaration of what it does; the field inventory is an *independent* reading of the source
//! that the adapter does not participate in. Conformance compares them, and it can only do
//! that if the adapter did not supply both sides of the comparison.

use crate::error::AdapterError;
use crate::ingestion::Ingestion;
use crate::loss::LossKind;
use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The conformance ladder of 04.06: an adapter advertises only the level whose suite it passes.
///
/// Only the first two are meaningful in this crate. `Execute`, `Stream` and `Replay` describe
/// adapters that drive a running system, and nothing here does; they exist in the enum so that
/// an adapter which later gains those capabilities does not need a schema change to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceLevel {
    /// Reads the native format and reports structure without producing facts.
    Parse,
    /// Produces validated facts with a complete loss audit.
    Normalize,
    /// Additionally drives execution against the source system.
    Execute,
    /// Additionally emits an event stream.
    Stream,
    /// Additionally reproduces a historical ingest byte for byte.
    Replay,
}

/// What an adapter says about itself, before it is handed any bytes.
///
/// `declared_loss_kinds` is the part that earns its place: it turns "which of the forbidden
/// silent coercions does this adapter commit" from a code-reading exercise into a field, and
/// conformance checks the declaration against what the adapter actually emitted. An adapter
/// that emits an undeclared loss kind has surprised its own author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterManifest {
    pub name: String,
    pub version: String,
    /// Media types or profile names this adapter accepts, e.g. `text/csv`.
    pub accepted_formats: Vec<String>,
    pub conformance_level: ConformanceLevel,
    /// Every loss kind this adapter may emit. Emitting one not listed here is a conformance
    /// failure, not a runtime error: the facts are still correct, the documentation is not.
    pub declared_loss_kinds: BTreeSet<LossKind>,
    /// Scope dimensions the emitted facts may bind. Lets a world builder check for dimension
    /// collisions between adapters before running either of them.
    pub scope_dimensions: BTreeSet<String>,
}

impl AdapterManifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        conformance_level: ConformanceLevel,
    ) -> Self {
        AdapterManifest {
            name: name.into(),
            version: version.into(),
            accepted_formats: Vec::new(),
            conformance_level,
            declared_loss_kinds: BTreeSet::new(),
            scope_dimensions: BTreeSet::new(),
        }
    }

    pub fn accepting<I, S>(mut self, formats: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.accepted_formats
            .extend(formats.into_iter().map(Into::into));
        self
    }

    pub fn declaring<I>(mut self, kinds: I) -> Self
    where
        I: IntoIterator<Item = LossKind>,
    {
        self.declared_loss_kinds.extend(kinds);
        self
    }

    pub fn binding<I, S>(mut self, dimensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scope_dimensions
            .extend(dimensions.into_iter().map(Into::into));
        self
    }

    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.name.trim().is_empty() || self.version.trim().is_empty() {
            return Err(AdapterError::Conformance(
                "adapter manifest identity is incomplete".into(),
            ));
        }
        for value in [&self.name, &self.version] {
            if value.len() > 512 || value.chars().any(char::is_control) || value.trim() != value {
                return Err(AdapterError::Conformance(
                    "adapter manifest identity is outside its bounded text contract".into(),
                ));
            }
        }
        for format in &self.accepted_formats {
            if format.trim().is_empty()
                || format.len() > 256
                || format.trim() != format
                || format.chars().any(char::is_control)
                || format != &format.to_ascii_lowercase()
            {
                return Err(AdapterError::Conformance(
                    "adapter manifest formats must be normalized and bounded".into(),
                ));
            }
        }
        if self
            .accepted_formats
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(AdapterError::Conformance(
                "adapter manifest formats must be strictly sorted".into(),
            ));
        }
        for dimension in &self.scope_dimensions {
            if dimension.trim().is_empty()
                || dimension.len() > 256
                || dimension.trim() != dimension
                || dimension.chars().any(char::is_control)
            {
                return Err(AdapterError::Conformance(
                    "adapter manifest scope dimensions are outside their bound".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Normalizes one class of source into validated facts plus a mandatory loss audit.
///
/// Implementations must be pure with respect to the source: given the same bytes and the same
/// mapping policy they must produce the same [`Ingestion`], including the same loss report.
/// Anything that would vary between runs — a wall clock, a random id, a filesystem timestamp —
/// belongs in the caller's provenance, not in the facts. [`crate::conformance`] checks this
/// rather than trusting it.
pub trait Adapter {
    /// Stable name, used in manifests, fact provenance and conformance reports.
    fn name(&self) -> &str;

    /// The adapter's self-description. Must not depend on any source.
    fn manifest(&self) -> AdapterManifest;

    /// Reads `source` into facts and a loss audit.
    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError>;
}

impl<T: Adapter + ?Sized> Adapter for &T {
    fn name(&self) -> &str {
        (**self).name()
    }

    fn manifest(&self) -> AdapterManifest {
        (**self).manifest()
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        (**self).ingest(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conformance_levels_are_ordered_from_weakest_to_strongest() {
        assert!(ConformanceLevel::Parse < ConformanceLevel::Normalize);
        assert!(ConformanceLevel::Normalize < ConformanceLevel::Replay);
    }

    #[test]
    fn a_manifest_declares_its_loss_surface_as_a_set_not_a_list() {
        let manifest = AdapterManifest::new("t", "0.1.0", ConformanceLevel::Normalize)
            .declaring([LossKind::UnmappedColumn, LossKind::UnmappedColumn]);
        assert_eq!(manifest.declared_loss_kinds.len(), 1);
    }

    #[test]
    fn a_manifest_rejects_noncanonical_formats() {
        let manifest =
            AdapterManifest::new("t", "0.1.0", ConformanceLevel::Parse).accepting(["TEXT/CSV"]);
        assert!(manifest.validate().is_err());
    }
}
