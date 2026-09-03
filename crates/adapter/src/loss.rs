//! Semantic loss: what an adapter did not carry, and why.
//!
//! Blueprint 40.17 invariant 3 ("semantic loss and unsupported fields are reported") and the
//! "no silent coercion" list in 28.00 are the whole reason this crate exists. An adapter that
//! returns facts and nothing else is indistinguishable from an adapter that returns facts
//! having thrown away the units, the reference genome build and the ontology version — and the
//! resulting world would then be *wrong in a way no downstream check can detect*, because the
//! evidence it dropped is exactly the evidence needed to notice.
//!
//! Two design decisions here are load-bearing.
//!
//! First, "nothing was lost" is a distinct variant from "nobody checked". An empty vector of
//! loss entries conflates the two, and the conflation always resolves in the adapter author's
//! favour: a stub that never audits anything looks perfect. [`SemanticLoss::Unaudited`] makes
//! the absence of an audit a fact about the ingestion rather than a silence.
//!
//! Second, [`SemanticLoss::Lossy`] cannot be constructed with zero entries, because
//! [`LossReport`] enforces non-emptiness at construction. Otherwise `Lossy { entries: vec![] }`
//! would quietly re-introduce the same conflation the enum was built to remove.

use crate::error::AdapterError;
use crate::location::{LocationSet, SourceLocation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The categories of loss an adapter may declare.
///
/// The first six are the blueprint's own list of forbidden silent behaviours (28.00, "No
/// silent coercion"), turned from prose into a closed enum so that a reviewer can ask an
/// adapter which of them it commits and get an answer rather than an essay. The last two exist
/// because the adapters in this crate need them: a tabular reader that refuses to guess a
/// column's type, and an inventory walker that hashes bytes it cannot interpret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossKind {
    /// A source column exists but no mapping rule claims it, so its content is not in the world.
    UnmappedColumn,
    /// A magnitude was carried without the unit it was measured in.
    UnpreservedUnit,
    /// Coordinates were carried without the frame they are expressed in — a reference genome
    /// build, a patient-space imaging frame, a slide coordinate system.
    CoordinateFrameNotCarried,
    /// A numeric value does not survive the round trip through the emitted representation.
    PrecisionReduced,
    /// The chain back to an upstream accession, version or retrieval time is not available.
    ProvenanceUnavailable,
    /// A term that belongs to a controlled vocabulary was carried as free text, or was mapped
    /// without recording which version of the vocabulary performed the mapping.
    OntologyTermUnmapped,
    /// The adapter could not determine a field's type and declined to guess one.
    TypeUndetermined,
    /// Bytes were addressed and hashed but never interpreted.
    ContentUninterpreted,
}

impl LossKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LossKind::UnmappedColumn => "unmapped_column",
            LossKind::UnpreservedUnit => "unpreserved_unit",
            LossKind::CoordinateFrameNotCarried => "coordinate_frame_not_carried",
            LossKind::PrecisionReduced => "precision_reduced",
            LossKind::ProvenanceUnavailable => "provenance_unavailable",
            LossKind::OntologyTermUnmapped => "ontology_term_unmapped",
            LossKind::TypeUndetermined => "type_undetermined",
            LossKind::ContentUninterpreted => "content_uninterpreted",
        }
    }

    /// Every kind, in declaration order. Used by manifests that declare a full loss surface and
    /// by conformance when reporting which kinds an adapter emitted without declaring.
    pub const ALL: [LossKind; 8] = [
        LossKind::UnmappedColumn,
        LossKind::UnpreservedUnit,
        LossKind::CoordinateFrameNotCarried,
        LossKind::PrecisionReduced,
        LossKind::ProvenanceUnavailable,
        LossKind::OntologyTermUnmapped,
        LossKind::TypeUndetermined,
        LossKind::ContentUninterpreted,
    ];
}

impl fmt::Display for LossKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How much a loss costs the consumer of the resulting world.
///
/// 40.17 lists "semantic-loss severity" as an operational metric, which only means something
/// if severity is assigned at the point of loss rather than inferred later from the kind. The
/// same kind can be either: an unmapped free-text comment column is advisory, an unmapped
/// treatment-arm column is blocking, and only the adapter author knows which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossSeverity {
    /// The world is complete for its declared purpose; the dropped material is out of scope.
    Advisory,
    /// Some downstream question that the source could have answered now cannot be.
    Degrading,
    /// The world would be misleading if published as-is.
    Blocking,
}

impl LossSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            LossSeverity::Advisory => "advisory",
            LossSeverity::Degrading => "degrading",
            LossSeverity::Blocking => "blocking",
        }
    }
}

impl fmt::Display for LossSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One thing that was not carried, and where it was.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LossEntry {
    pub kind: LossKind,
    pub severity: LossSeverity,
    /// The place in the source this loss refers to. Never synthesized: an adapter that cannot
    /// name the location has not understood the source well enough to declare the loss.
    pub location: SourceLocation,
    /// What was dropped and why, in terms a reviewer can act on.
    pub detail: String,
}

impl LossEntry {
    pub fn new(
        kind: LossKind,
        severity: LossSeverity,
        location: SourceLocation,
        detail: impl Into<String>,
    ) -> Self {
        LossEntry {
            kind,
            severity,
            location,
            detail: detail.into(),
        }
    }

    fn validate(&self) -> Result<(), AdapterError> {
        if self.detail.is_empty() || self.detail.trim() != self.detail {
            return Err(AdapterError::InvalidLoss(
                "loss detail must be non-empty and trimmed".into(),
            ));
        }
        if self.detail.len() > 1024 || self.detail.chars().any(char::is_control) {
            return Err(AdapterError::InvalidLoss(
                "loss detail is outside its bounded text contract".into(),
            ));
        }
        self.location.validate().map_err(AdapterError::InvalidLoss)
    }
}

/// A non-empty, sorted, de-duplicated collection of loss entries.
///
/// Non-emptiness is the invariant that keeps [`SemanticLoss::Lossy`] honest. Sorting is what
/// makes the report content-hashable: two runs of the same ingest that discover the same
/// losses in different orders must produce the same bytes, or conformance's determinism check
/// would fail for a reason that has nothing to do with the adapter's determinism.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LossReport {
    entries: Vec<LossEntry>,
}

impl LossReport {
    /// Builds a report, or `None` when there is nothing to report.
    ///
    /// Returning `Option` rather than an empty report is what forces the caller to decide
    /// between [`SemanticLoss::Lossless`] and [`SemanticLoss::Lossy`] explicitly.
    pub fn new(entries: Vec<LossEntry>) -> Option<Self> {
        if entries.is_empty() {
            return None;
        }
        let mut entries = entries;
        entries.sort();
        entries.dedup();
        Some(LossReport { entries })
    }

    pub fn entries(&self) -> &[LossEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Always false. Present because clippy asks for it next to `len`, and because its
    /// constant answer is the invariant this type exists to guarantee.
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn kinds(&self) -> BTreeSet<LossKind> {
        self.entries.iter().map(|entry| entry.kind).collect()
    }

    /// The worst severity present, or `None` for an invalid/deserialized empty report.
    pub fn max_severity(&self) -> Option<LossSeverity> {
        self.entries.iter().map(|entry| entry.severity).max()
    }

    pub fn locations(&self) -> LocationSet {
        self.entries
            .iter()
            .map(|entry| entry.location.clone())
            .collect()
    }

    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.entries.is_empty() {
            return Err(AdapterError::InvalidLoss(
                "a lossy audit must contain at least one entry".into(),
            ));
        }
        for entry in &self.entries {
            entry.validate()?;
        }
        if self.entries.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(AdapterError::InvalidLoss(
                "loss entries must be strictly sorted and unique".into(),
            ));
        }
        Ok(())
    }
}

/// The mandatory outcome of a semantic-loss audit.
///
/// Every [`crate::Ingestion`] carries one. The three variants are mutually exclusive claims
/// about what the adapter knows, and they are deliberately not orderable by "goodness":
/// `Unaudited` is not a worse `Lossless`, it is a different statement — the adapter is
/// asserting ignorance rather than fidelity, and a publisher should treat it as unreviewable
/// rather than as slightly lossy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "audit", rename_all = "snake_case")]
pub enum SemanticLoss {
    /// No audit was performed. Facts may still be present and may still be correct; nobody
    /// checked whether anything was dropped on the way, so nothing about completeness is known.
    Unaudited {
        /// Why the audit was skipped — an unsupported format, a truncated read, a stub adapter.
        reason: String,
    },
    /// Every location in `mapped` was examined and carried into the emitted facts, and the
    /// adapter asserts that the source contains nothing else.
    Lossless { mapped: LocationSet },
    /// The audit completed and found losses. `mapped` is what survived; `lost` is what did not.
    Lossy {
        mapped: LocationSet,
        lost: LossReport,
    },
}

impl SemanticLoss {
    pub fn validate(&self) -> Result<(), AdapterError> {
        match self {
            SemanticLoss::Unaudited { reason } => {
                if reason.is_empty() || reason.trim() != reason {
                    return Err(AdapterError::InvalidLoss(
                        "unaudited reason must be non-empty and trimmed".into(),
                    ));
                }
                if reason.len() > 1024 || reason.chars().any(char::is_control) {
                    return Err(AdapterError::InvalidLoss(
                        "unaudited reason is outside its bounded text contract".into(),
                    ));
                }
            }
            SemanticLoss::Lossless { mapped } => {
                for location in mapped {
                    location.validate().map_err(AdapterError::InvalidLoss)?;
                }
            }
            SemanticLoss::Lossy { mapped, lost } => {
                for location in mapped {
                    location.validate().map_err(AdapterError::InvalidLoss)?;
                }
                lost.validate()?;
            }
        }
        Ok(())
    }

    pub fn validate_for_source(&self, source_id: &str) -> Result<(), AdapterError> {
        self.validate()?;
        for location in self
            .mapped()
            .into_iter()
            .chain(self.entries().iter().map(|entry| entry.location.clone()))
        {
            if location.source != source_id {
                return Err(AdapterError::InvalidLoss(format!(
                    "loss location {} belongs to a different source",
                    location.locator()
                )));
            }
        }
        Ok(())
    }

    /// True when an audit was performed at all, whichever way it came out.
    pub fn is_audited(&self) -> bool {
        !matches!(self, SemanticLoss::Unaudited { .. })
    }

    /// True only for an audit that completed and found nothing. An unaudited ingestion is not
    /// lossless; it is unknown.
    pub fn is_lossless(&self) -> bool {
        matches!(self, SemanticLoss::Lossless { .. })
    }

    /// Locations the adapter claims to have carried into the world. Empty for `Unaudited`,
    /// because an adapter that did not audit has made no claim about any location.
    pub fn mapped(&self) -> LocationSet {
        match self {
            SemanticLoss::Unaudited { .. } => LocationSet::new(),
            SemanticLoss::Lossless { mapped } => mapped.clone(),
            SemanticLoss::Lossy { mapped, .. } => mapped.clone(),
        }
    }

    pub fn report(&self) -> Option<&LossReport> {
        match self {
            SemanticLoss::Lossy { lost, .. } => Some(lost),
            _ => None,
        }
    }

    pub fn entries(&self) -> &[LossEntry] {
        match self {
            SemanticLoss::Lossy { lost, .. } => lost.entries(),
            _ => &[],
        }
    }

    pub fn kinds(&self) -> BTreeSet<LossKind> {
        self.report().map(LossReport::kinds).unwrap_or_default()
    }

    /// The worst severity found, or `None` when nothing was lost or nothing was checked.
    pub fn max_severity(&self) -> Option<LossSeverity> {
        self.report().and_then(LossReport::max_severity)
    }

    /// Every entry at or beneath `field`.
    ///
    /// Reporting, not accounting: a reviewer looking at one column wants the per-cell precision
    /// entries underneath it as well as the column-level ones. Conformance deliberately does
    /// *not* use this — a per-cell entry never discharges a whole column — but a human reading
    /// a loss report does.
    pub fn entries_for(&self, field: &SourceLocation) -> Vec<&LossEntry> {
        self.entries()
            .iter()
            .filter(|entry| field.covers(&entry.location))
            .collect()
    }
}

/// Accumulates an audit while an adapter walks a source, then closes it into a [`SemanticLoss`].
///
/// The builder exists so that "mapped" and "lost" are recorded at the moment the adapter makes
/// the decision, in the same code path, rather than reconstructed afterwards from the emitted
/// facts. A reconstruction can only report what was carried; it can never notice what was
/// dropped, which is precisely the information the audit is for.
#[derive(Debug, Clone, Default)]
pub struct LossAudit {
    mapped: LocationSet,
    lost: Vec<LossEntry>,
}

impl LossAudit {
    pub fn new() -> Self {
        LossAudit::default()
    }

    /// Records that `location` was examined and its content carried into the emitted facts.
    pub fn mapped(&mut self, location: SourceLocation) -> &mut Self {
        self.mapped.insert(location);
        self
    }

    /// Records a loss. Note that a location may be both mapped and lost: a magnitude carried
    /// without its unit is a partial carry, and reporting only one half would misdescribe it.
    pub fn lost(&mut self, entry: LossEntry) -> &mut Self {
        self.lost.push(entry);
        self
    }

    pub fn record(
        &mut self,
        kind: LossKind,
        severity: LossSeverity,
        location: SourceLocation,
        detail: impl Into<String>,
    ) -> &mut Self {
        self.lost(LossEntry::new(kind, severity, location, detail))
    }

    /// Closes the audit. This is the only way to produce a `Lossless` or `Lossy` value from a
    /// walk, and it picks between them on evidence rather than on the author's intent.
    pub fn finish(self) -> SemanticLoss {
        match LossReport::new(self.lost) {
            Some(lost) => SemanticLoss::Lossy {
                mapped: self.mapped,
                lost,
            },
            None => SemanticLoss::Lossless {
                mapped: self.mapped,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(field: &str) -> LossEntry {
        LossEntry::new(
            LossKind::UnmappedColumn,
            LossSeverity::Degrading,
            SourceLocation::column("s", field),
            "no mapping rule",
        )
    }

    #[test]
    fn an_empty_loss_report_cannot_be_constructed() {
        assert!(LossReport::new(Vec::new()).is_none());
    }

    #[test]
    fn an_audit_that_found_nothing_is_lossless_not_unaudited() {
        let mut audit = LossAudit::new();
        audit.mapped(SourceLocation::column("s", "age"));
        let loss = audit.finish();
        assert!(loss.is_lossless());
        assert!(loss.is_audited());
    }

    #[test]
    fn an_unaudited_ingestion_is_not_lossless() {
        let loss = SemanticLoss::Unaudited {
            reason: "format not supported".into(),
        };
        assert!(!loss.is_lossless());
        assert!(!loss.is_audited());
        assert!(loss.mapped().is_empty());
    }

    #[test]
    fn a_report_sorts_and_deduplicates_so_discovery_order_cannot_change_the_bytes() {
        let forward = LossReport::new(vec![entry("b"), entry("a"), entry("a")]).unwrap();
        let backward = LossReport::new(vec![entry("a"), entry("b"), entry("b")]).unwrap();
        assert_eq!(forward, backward);
        assert_eq!(forward.len(), 2);
    }

    #[test]
    fn a_cell_level_entry_is_reported_under_its_column_but_a_sibling_column_is_not() {
        let mut audit = LossAudit::new();
        audit.record(
            LossKind::PrecisionReduced,
            LossSeverity::Degrading,
            SourceLocation::cell("s", 4, "dose"),
            "does not round trip",
        );
        audit.record(
            LossKind::UnmappedColumn,
            LossSeverity::Advisory,
            SourceLocation::column("s", "note"),
            "no rule",
        );
        let loss = audit.finish();

        let under_dose = loss.entries_for(&SourceLocation::column("s", "dose"));
        assert_eq!(under_dose.len(), 1);
        assert_eq!(under_dose[0].kind, LossKind::PrecisionReduced);
    }

    #[test]
    fn max_severity_is_the_worst_entry_not_the_last_one() {
        let mut audit = LossAudit::new();
        audit.record(
            LossKind::UnmappedColumn,
            LossSeverity::Blocking,
            SourceLocation::column("s", "arm"),
            "treatment arm dropped",
        );
        audit.record(
            LossKind::UnmappedColumn,
            LossSeverity::Advisory,
            SourceLocation::column("s", "note"),
            "free text dropped",
        );
        assert_eq!(audit.finish().max_severity(), Some(LossSeverity::Blocking));
    }

    #[test]
    fn deserialized_empty_loss_reports_are_rejected_by_validation() {
        let loss = SemanticLoss::Lossy {
            mapped: LocationSet::new(),
            lost: LossReport {
                entries: Vec::new(),
            },
        };
        assert!(matches!(loss.validate(), Err(AdapterError::InvalidLoss(_))));
    }

    #[test]
    fn loss_locations_must_belong_to_the_ingested_source() {
        let mut audit = LossAudit::new();
        audit.record(
            LossKind::UnmappedColumn,
            LossSeverity::Degrading,
            SourceLocation::column("other", "age"),
            "not from this source",
        );
        assert!(matches!(
            audit.finish().validate_for_source("source"),
            Err(AdapterError::InvalidLoss(_))
        ));
    }
}
