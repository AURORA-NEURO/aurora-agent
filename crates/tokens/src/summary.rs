//! Summarisation contracts for tables, matrices, images and sequences (39.13).
//!
//! 39.13 asks for "modality-aware computed views that preserve scientific meaning while avoiding
//! raw serialization". The thing that makes such a view safe is not the statistic it computed; it
//! is the declaration of what it *did not* compute.
//!
//! # A summary with no declared loss is not a summary, it is a claim
//!
//! That is made unrepresentable here. [`SummaryContract`] cannot be constructed without a
//! [`Discarded`], and [`Discarded`] has exactly two shapes: a non-empty list of discarded aspects,
//! or [`Discarded::Nothing`] carrying an *argument* for why the view is lossless. There is no third
//! state and no default. A caller who genuinely has a lossless view writes the argument down once;
//! a caller who has not thought about it cannot get past the constructor.
//!
//! # The four invariants of 39.13
//!
//! 1. *"Never summarize across incompatible identity or coordinate systems."* [`SummaryContract`]
//!    holds its [`SourceArtifact`]s and checks that they share an identity, a coordinate system and
//!    a unit before anything else. A mean over two reference frames is a number about nothing.
//! 2. *"Preserve distribution shape when decisions depend on tails or rare states."* The obligation
//!    the view serves declares [`SummaryObligation::depends_on_tails`], and a tail-sensitive
//!    obligation with no shape-preserving aspect is
//!    [`SummaryError::TailSensitiveWithoutShape`]. This is 39.13's "summary hides multimodality"
//!    failure mode expressed as a refusal.
//! 3. *"Report missingness and QC failures."* Required preserved aspects for the tabular and array
//!    modalities.
//! 4. *"Image and sequence crops include orientation/reference context."* Required preserved
//!    aspects for those two, which is why [`Modality`] is an enum with per-variant requirements
//!    rather than a free string.
//!
//! # Recoverability
//!
//! Every discarded aspect declares how to get it back — [`Recovery::Via`] naming an expansion — or
//! declares that it cannot be recovered and why. 39.13's outputs include "expansion operations",
//! and a discard with no route back is a decision that deserves to be visible rather than a gap
//! somebody discovers later.
//!
//! # Not implemented
//!
//! No statistics, no adapters, no crops. There is no Arrow reader here, no AnnData, no DICOM, and
//! nothing computes a quantile. 39.13's `ViewOperator` SDK is a contract this module describes and
//! does not execute: a [`SummaryContract`] is a declaration about a view somebody else computed,
//! and its value is that the declaration can be checked against the obligation before the view is
//! trusted.

use crate::error::SummaryError;
use bioprism_obligation::TokenEstimate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The artifact classes 39.13 names.
///
/// An enum rather than a string because the per-modality requirements below are the module's whole
/// content, and a modality the code has never heard of would have no requirements at all — which is
/// exactly the silent pass this module exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Rows and typed columns: a results table, a clinical table.
    Table,
    /// A dense numeric array: an expression matrix, a Zarr volume.
    Matrix,
    /// A raster with a coordinate frame: a slide, a radiological series.
    Image,
    /// A biological sequence with an interval and a reference build.
    Sequence,
}

impl Modality {
    pub fn as_str(self) -> &'static str {
        match self {
            Modality::Table => "table",
            Modality::Matrix => "matrix",
            Modality::Image => "image",
            Modality::Sequence => "sequence",
        }
    }

    /// Aspects a view of this modality must preserve whatever else it does.
    fn required(self) -> &'static [&'static str] {
        match self {
            Modality::Table | Modality::Matrix => &["missingness", "qc_failures"],
            Modality::Image => &["orientation", "reference_frame"],
            Modality::Sequence => &["reference_build", "interval"],
        }
    }
}

/// What a view keeps.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "preserved", rename_all = "snake_case")]
pub enum PreservedAspect {
    /// The identity of the thing measured survives into the view.
    Identity,
    /// The unit of the values.
    Units { unit: String },
    /// The coordinate or reference frame positions are expressed in.
    ReferenceFrame { system: String },
    /// Orientation of a crop, so left and right do not become each other.
    Orientation,
    /// Reference genome or assembly build.
    ReferenceBuild { build: String },
    /// The interval a sequence view covers.
    Interval { start: u64, end: u64 },
    /// Enough of the distribution's shape to see multimodality, named by the statistics kept.
    DistributionShape { statistics: Vec<String> },
    /// Explicit tail coverage, named by quantile.
    Tails { quantiles: Vec<String> },
    /// The proportion and pattern of missing values.
    Missingness,
    /// Which quality-control checks failed, distinct from values being absent.
    QcFailures,
    /// A rare state or population that a summary statistic would otherwise wash out.
    RareStates { states: Vec<String> },
}

impl PreservedAspect {
    fn key(&self) -> &'static str {
        match self {
            PreservedAspect::Identity => "identity",
            PreservedAspect::Units { .. } => "units",
            PreservedAspect::ReferenceFrame { .. } => "reference_frame",
            PreservedAspect::Orientation => "orientation",
            PreservedAspect::ReferenceBuild { .. } => "reference_build",
            PreservedAspect::Interval { .. } => "interval",
            PreservedAspect::DistributionShape { .. } => "distribution_shape",
            PreservedAspect::Tails { .. } => "tails",
            PreservedAspect::Missingness => "missingness",
            PreservedAspect::QcFailures => "qc_failures",
            PreservedAspect::RareStates { .. } => "rare_states",
        }
    }

    /// Whether this aspect is enough to see multimodality or a rare state.
    fn shows_shape(&self) -> bool {
        matches!(
            self,
            PreservedAspect::DistributionShape { .. }
                | PreservedAspect::Tails { .. }
                | PreservedAspect::RareStates { .. }
        )
    }
}

/// Whether a discarded aspect can be got back.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "recovery", rename_all = "snake_case")]
pub enum Recovery {
    /// A named expansion operation returns it. 39.13's "expansion operations" output.
    Via { expansion: String },
    /// It cannot be recovered from the view, and the reason is stated. Legitimate — a policy-bound
    /// raw field genuinely cannot be — and worth being visible.
    NotRecoverable { reason: String },
}

/// One thing a view does not carry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DiscardedAspect {
    /// What was dropped, in the caller's own vocabulary: "per-cell values", "pixels outside the
    /// crop", "rows failing QC".
    pub aspect: String,
    pub recovery: Recovery,
}

impl DiscardedAspect {
    pub fn recoverable(aspect: impl Into<String>, expansion: impl Into<String>) -> Self {
        DiscardedAspect {
            aspect: aspect.into(),
            recovery: Recovery::Via {
                expansion: expansion.into(),
            },
        }
    }

    pub fn unrecoverable(aspect: impl Into<String>, reason: impl Into<String>) -> Self {
        DiscardedAspect {
            aspect: aspect.into(),
            recovery: Recovery::NotRecoverable {
                reason: reason.into(),
            },
        }
    }
}

/// What a view discards. There is no way to say nothing without saying why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "loss", rename_all = "snake_case")]
pub enum Discarded {
    /// The view is lossless, and here is the argument. Rare, and the argument is what makes it
    /// checkable: "the source has 12 rows and the view lists all 12" is an argument, "it's fine" is
    /// one somebody can reject.
    Nothing { argument: String },
    /// A non-empty list of discarded aspects. Emptiness is rejected at construction, because an
    /// empty list is a lossless claim without the argument.
    Aspects { aspects: Vec<DiscardedAspect> },
}

impl Discarded {
    pub fn lossless(argument: impl Into<String>) -> Self {
        Discarded::Nothing {
            argument: argument.into(),
        }
    }

    pub fn of(aspects: Vec<DiscardedAspect>) -> Self {
        Discarded::Aspects { aspects }
    }

    pub fn is_lossless(&self) -> bool {
        matches!(self, Discarded::Nothing { .. })
    }

    pub fn aspects(&self) -> &[DiscardedAspect] {
        match self {
            Discarded::Nothing { .. } => &[],
            Discarded::Aspects { aspects } => aspects,
        }
    }

    /// Discarded aspects with no route back.
    pub fn unrecoverable(&self) -> impl Iterator<Item = &DiscardedAspect> {
        self.aspects()
            .iter()
            .filter(|aspect| matches!(aspect.recovery, Recovery::NotRecoverable { .. }))
    }
}

/// One artifact a view was computed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtifact {
    pub artifact_id: String,
    /// The identity of the thing measured: a specimen, a subject, a lesion. Views must not span
    /// two.
    pub identity: String,
    /// The coordinate or reference frame. Two frames do not aggregate.
    pub coordinate_system: String,
    /// The unit of the values.
    pub unit: String,
    /// A stable locator back into the source region, row range or interval.
    pub locator: String,
}

impl SourceArtifact {
    pub fn new(
        artifact_id: impl Into<String>,
        identity: impl Into<String>,
        coordinate_system: impl Into<String>,
        unit: impl Into<String>,
        locator: impl Into<String>,
    ) -> Self {
        SourceArtifact {
            artifact_id: artifact_id.into(),
            identity: identity.into(),
            coordinate_system: coordinate_system.into(),
            unit: unit.into(),
            locator: locator.into(),
        }
    }
}

/// What the view is for, and what that use is sensitive to.
///
/// The obligation is the reason a particular summarisation is or is not adequate. A mean is a fine
/// view of an expression matrix for one obligation and a catastrophe for another, and the only way
/// to tell is to have the obligation state which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryObligation {
    pub obligation_id: String,
    /// The decision turns on tails, rare subpopulations or multimodality.
    #[serde(default)]
    pub depends_on_tails: bool,
    /// The decision turns on which values are missing, not only on the present ones.
    #[serde(default)]
    pub depends_on_missingness: bool,
}

impl SummaryObligation {
    pub fn new(obligation_id: impl Into<String>) -> Self {
        SummaryObligation {
            obligation_id: obligation_id.into(),
            depends_on_tails: false,
            depends_on_missingness: false,
        }
    }

    pub fn tail_sensitive(mut self) -> Self {
        self.depends_on_tails = true;
        self
    }

    pub fn missingness_sensitive(mut self) -> Self {
        self.depends_on_missingness = true;
        self
    }
}

/// A declared summarisation: what it is of, what it keeps, what it loses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryContract {
    pub summary_id: String,
    pub modality: Modality,
    pub obligation: SummaryObligation,
    pub sources: Vec<SourceArtifact>,
    pub preserved: Vec<PreservedAspect>,
    pub discarded: Discarded,
    /// The estimated token cost of the view. Never a measurement.
    pub estimate: TokenEstimate,
    /// The view operator and version that produced it, so a view can be reproduced or invalidated.
    pub operator: String,
}

impl SummaryContract {
    /// Build and check a summarisation contract.
    ///
    /// The only constructor. Every invariant of 39.13 is checked here rather than in a separate
    /// `validate` that a caller could forget, so an existing [`SummaryContract`] value is one that
    /// has already been checked against its obligation.
    #[allow(clippy::too_many_arguments)]
    pub fn declare(
        summary_id: impl Into<String>,
        modality: Modality,
        obligation: SummaryObligation,
        sources: Vec<SourceArtifact>,
        preserved: Vec<PreservedAspect>,
        discarded: Discarded,
        estimate: TokenEstimate,
        operator: impl Into<String>,
    ) -> Result<Self, SummaryError> {
        let summary_id = summary_id.into();

        if sources.is_empty() {
            return Err(SummaryError::NoSources {
                summary: summary_id,
            });
        }
        if let Some(source) = sources.iter().find(|source| source.locator.is_empty()) {
            let _ = source;
            return Err(SummaryError::NoSourceLocator {
                summary: summary_id,
            });
        }

        match &discarded {
            Discarded::Aspects { aspects } if aspects.is_empty() => {
                return Err(SummaryError::LossNotDeclared(summary_id));
            }
            Discarded::Nothing { argument } if argument.trim().is_empty() => {
                return Err(SummaryError::LosslessWithoutArgument(summary_id));
            }
            _ => {}
        }

        let first = &sources[0];
        for source in &sources[1..] {
            if source.identity != first.identity {
                return Err(SummaryError::IncompatibleIdentity {
                    summary: summary_id,
                    left: first.identity.clone(),
                    right: source.identity.clone(),
                });
            }
            if source.coordinate_system != first.coordinate_system {
                return Err(SummaryError::IncompatibleCoordinateSystem {
                    summary: summary_id,
                    left: first.coordinate_system.clone(),
                    right: source.coordinate_system.clone(),
                });
            }
            if source.unit != first.unit {
                return Err(SummaryError::IncompatibleUnits {
                    summary: summary_id,
                    left: first.unit.clone(),
                    right: source.unit.clone(),
                });
            }
        }

        let keys: BTreeSet<&str> = preserved.iter().map(PreservedAspect::key).collect();
        for required in modality.required() {
            if !keys.contains(required) {
                return Err(SummaryError::RequiredAspectMissing {
                    summary: summary_id,
                    modality: modality.as_str().to_string(),
                    required: (*required).to_string(),
                });
            }
        }

        if obligation.depends_on_tails && !preserved.iter().any(PreservedAspect::shows_shape) {
            return Err(SummaryError::TailSensitiveWithoutShape {
                summary: summary_id,
                obligation: obligation.obligation_id,
            });
        }
        if obligation.depends_on_missingness && !keys.contains("missingness") {
            return Err(SummaryError::RequiredAspectMissing {
                summary: summary_id,
                modality: modality.as_str().to_string(),
                required: "missingness".to_string(),
            });
        }

        Ok(SummaryContract {
            summary_id,
            modality,
            obligation,
            sources,
            preserved,
            discarded,
            estimate,
            operator: operator.into(),
        })
    }

    /// The identity every source shares. Checked at construction, so this cannot disagree.
    pub fn identity(&self) -> &str {
        &self.sources[0].identity
    }

    pub fn coordinate_system(&self) -> &str {
        &self.sources[0].coordinate_system
    }

    /// Expansion operations a consumer may invoke to recover discarded content.
    pub fn expansions(&self) -> BTreeSet<String> {
        self.discarded
            .aspects()
            .iter()
            .filter_map(|aspect| match &aspect.recovery {
                Recovery::Via { expansion } => Some(expansion.clone()),
                Recovery::NotRecoverable { .. } => None,
            })
            .collect()
    }

    /// One line per discarded aspect, saying what it was and whether it can be got back.
    ///
    /// This is what a reader of a compact context needs and almost never gets: not the summary, but
    /// the shape of the hole it left.
    pub fn declared_loss(&self) -> Vec<String> {
        match &self.discarded {
            Discarded::Nothing { argument } => {
                vec![format!("nothing discarded: {argument}")]
            }
            Discarded::Aspects { aspects } => aspects
                .iter()
                .map(|aspect| match &aspect.recovery {
                    Recovery::Via { expansion } => {
                        format!("discarded {} (recover with `{expansion}`)", aspect.aspect)
                    }
                    Recovery::NotRecoverable { reason } => {
                        format!("discarded {} (unrecoverable: {reason})", aspect.aspect)
                    }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn source(id: &str, identity: &str) -> SourceArtifact {
        SourceArtifact::new(
            id,
            identity,
            "sample_x_gene",
            "log2_tpm",
            format!("world://matrix/{id}#rows=0..2000"),
        )
    }

    fn tabular_preserved() -> Vec<PreservedAspect> {
        vec![
            PreservedAspect::Identity,
            PreservedAspect::Units {
                unit: "log2_tpm".to_string(),
            },
            PreservedAspect::Missingness,
            PreservedAspect::QcFailures,
        ]
    }

    fn discarded_rows() -> Discarded {
        Discarded::of(vec![DiscardedAspect::recoverable(
            "per-cell values outside the reported quantiles",
            "expand:matrix_slice",
        )])
    }

    fn declare(
        obligation: SummaryObligation,
        preserved: Vec<PreservedAspect>,
        discarded: Discarded,
    ) -> Result<SummaryContract, SummaryError> {
        SummaryContract::declare(
            "view/expr",
            Modality::Matrix,
            obligation,
            vec![source("m1", "specimen/1")],
            preserved,
            discarded,
            est(180),
            "operator/quantile-profile@2",
        )
    }

    #[test]
    fn a_summary_declaring_no_loss_and_giving_no_argument_cannot_be_constructed() {
        assert!(matches!(
            declare(
                SummaryObligation::new("o/x"),
                tabular_preserved(),
                Discarded::of(vec![])
            ),
            Err(SummaryError::LossNotDeclared(_))
        ));
        assert!(matches!(
            declare(
                SummaryObligation::new("o/x"),
                tabular_preserved(),
                Discarded::lossless("   ")
            ),
            Err(SummaryError::LosslessWithoutArgument(_))
        ));
    }

    #[test]
    fn a_genuinely_lossless_view_is_representable_when_it_argues_for_itself() {
        let contract = declare(
            SummaryObligation::new("o/x"),
            tabular_preserved(),
            Discarded::lossless("the source has 12 rows and the view lists all 12 verbatim"),
        )
        .expect("declares");
        assert!(contract.discarded.is_lossless());
        assert_eq!(contract.declared_loss().len(), 1);
        assert!(contract.declared_loss()[0].contains("all 12"));
    }

    #[test]
    fn a_view_spanning_two_identities_is_refused_because_the_number_would_be_about_nothing() {
        let result = SummaryContract::declare(
            "view/mixed",
            Modality::Matrix,
            SummaryObligation::new("o/x"),
            vec![source("m1", "specimen/1"), source("m2", "specimen/2")],
            tabular_preserved(),
            discarded_rows(),
            est(10),
            "operator/mean",
        );
        assert!(matches!(
            result,
            Err(SummaryError::IncompatibleIdentity { .. })
        ));
    }

    #[test]
    fn a_view_spanning_two_coordinate_systems_is_refused() {
        let mut other = source("m2", "specimen/1");
        other.coordinate_system = "gene_x_sample".to_string();
        let result = SummaryContract::declare(
            "view/mixed",
            Modality::Matrix,
            SummaryObligation::new("o/x"),
            vec![source("m1", "specimen/1"), other],
            tabular_preserved(),
            discarded_rows(),
            est(10),
            "operator/mean",
        );
        assert!(matches!(
            result,
            Err(SummaryError::IncompatibleCoordinateSystem { .. })
        ));
    }

    #[test]
    fn a_view_spanning_two_units_is_refused() {
        let mut other = source("m2", "specimen/1");
        other.unit = "tpm".to_string();
        let result = SummaryContract::declare(
            "view/mixed",
            Modality::Matrix,
            SummaryObligation::new("o/x"),
            vec![source("m1", "specimen/1"), other],
            tabular_preserved(),
            discarded_rows(),
            est(10),
            "operator/mean",
        );
        assert!(matches!(
            result,
            Err(SummaryError::IncompatibleUnits { .. })
        ));
    }

    #[test]
    fn a_tail_sensitive_obligation_refuses_a_summary_that_keeps_no_distribution_shape() {
        assert!(matches!(
            declare(
                SummaryObligation::new("o/rare-clone").tail_sensitive(),
                tabular_preserved(),
                discarded_rows()
            ),
            Err(SummaryError::TailSensitiveWithoutShape { .. })
        ));
    }

    #[test]
    fn a_tail_sensitive_obligation_accepts_a_summary_that_names_its_quantiles() {
        let mut preserved = tabular_preserved();
        preserved.push(PreservedAspect::Tails {
            quantiles: vec!["p01".to_string(), "p99".to_string()],
        });
        let contract = declare(
            SummaryObligation::new("o/rare-clone").tail_sensitive(),
            preserved,
            discarded_rows(),
        )
        .expect("declares");
        assert_eq!(contract.obligation.obligation_id, "o/rare-clone");
    }

    #[test]
    fn a_rare_state_declaration_also_satisfies_tail_sensitivity() {
        let mut preserved = tabular_preserved();
        preserved.push(PreservedAspect::RareStates {
            states: vec!["subclone/EGFRvIII".to_string()],
        });
        assert!(declare(
            SummaryObligation::new("o/rare").tail_sensitive(),
            preserved,
            discarded_rows()
        )
        .is_ok());
    }

    #[test]
    fn a_matrix_view_that_reports_no_missingness_is_refused() {
        let preserved = vec![
            PreservedAspect::Identity,
            PreservedAspect::QcFailures,
        ];
        assert!(matches!(
            declare(SummaryObligation::new("o/x"), preserved, discarded_rows()),
            Err(SummaryError::RequiredAspectMissing { ref required, .. }) if required == "missingness"
        ));
    }

    #[test]
    fn a_matrix_view_that_reports_no_qc_failures_is_refused() {
        let preserved = vec![PreservedAspect::Identity, PreservedAspect::Missingness];
        assert!(matches!(
            declare(SummaryObligation::new("o/x"), preserved, discarded_rows()),
            Err(SummaryError::RequiredAspectMissing { ref required, .. }) if required == "qc_failures"
        ));
    }

    #[test]
    fn an_image_crop_without_orientation_is_refused() {
        let result = SummaryContract::declare(
            "view/crop",
            Modality::Image,
            SummaryObligation::new("o/enhancement"),
            vec![SourceArtifact::new(
                "series/1",
                "subject/1",
                "RAS",
                "signal_intensity",
                "world://series/1#z=40..44",
            )],
            vec![PreservedAspect::ReferenceFrame {
                system: "RAS".to_string(),
            }],
            Discarded::of(vec![DiscardedAspect::recoverable(
                "voxels outside the crop",
                "expand:image_region",
            )]),
            est(60),
            "operator/crop@1",
        );
        assert!(matches!(
            result,
            Err(SummaryError::RequiredAspectMissing { ref required, .. }) if required == "orientation"
        ));
    }

    #[test]
    fn a_sequence_view_without_a_reference_build_is_refused() {
        let result = SummaryContract::declare(
            "view/interval",
            Modality::Sequence,
            SummaryObligation::new("o/variant"),
            vec![SourceArtifact::new(
                "cram/1",
                "specimen/1",
                "chr7",
                "base",
                "world://cram/1#chr7:55019000-55211000",
            )],
            vec![PreservedAspect::Interval {
                start: 55_019_000,
                end: 55_211_000,
            }],
            Discarded::of(vec![DiscardedAspect::recoverable(
                "reads outside the interval",
                "expand:interval",
            )]),
            est(30),
            "operator/interval@1",
        );
        assert!(matches!(
            result,
            Err(SummaryError::RequiredAspectMissing { ref required, .. }) if required == "reference_build"
        ));
    }

    #[test]
    fn a_sequence_view_naming_its_build_and_interval_is_accepted() {
        let contract = SummaryContract::declare(
            "view/interval",
            Modality::Sequence,
            SummaryObligation::new("o/variant"),
            vec![SourceArtifact::new(
                "cram/1",
                "specimen/1",
                "chr7",
                "base",
                "world://cram/1#chr7:55019000-55211000",
            )],
            vec![
                PreservedAspect::ReferenceBuild {
                    build: "GRCh38".to_string(),
                },
                PreservedAspect::Interval {
                    start: 55_019_000,
                    end: 55_211_000,
                },
            ],
            Discarded::of(vec![DiscardedAspect::unrecoverable(
                "reads failing the consent filter",
                "consent does not permit release of the raw reads",
            )]),
            est(30),
            "operator/interval@1",
        )
        .expect("declares");
        assert_eq!(contract.identity(), "specimen/1");
        assert!(contract.expansions().is_empty());
        assert_eq!(contract.discarded.unrecoverable().count(), 1);
    }

    #[test]
    fn a_missingness_sensitive_obligation_requires_missingness_even_on_a_modality_that_would_not() {
        let result = SummaryContract::declare(
            "view/crop",
            Modality::Image,
            SummaryObligation::new("o/coverage").missingness_sensitive(),
            vec![SourceArtifact::new(
                "series/1",
                "subject/1",
                "RAS",
                "signal_intensity",
                "world://series/1",
            )],
            vec![
                PreservedAspect::Orientation,
                PreservedAspect::ReferenceFrame {
                    system: "RAS".to_string(),
                },
            ],
            Discarded::of(vec![DiscardedAspect::recoverable("voxels", "expand:region")]),
            est(10),
            "operator/crop@1",
        );
        assert!(matches!(
            result,
            Err(SummaryError::RequiredAspectMissing { ref required, .. }) if required == "missingness"
        ));
    }

    #[test]
    fn a_view_with_no_sources_is_refused() {
        let result = SummaryContract::declare(
            "view/nothing",
            Modality::Table,
            SummaryObligation::new("o/x"),
            vec![],
            tabular_preserved(),
            discarded_rows(),
            est(1),
            "operator/none",
        );
        assert!(matches!(result, Err(SummaryError::NoSources { .. })));
    }

    #[test]
    fn a_source_with_no_locator_is_refused_because_nothing_could_be_expanded_back() {
        let mut unlocatable = source("m1", "specimen/1");
        unlocatable.locator = String::new();
        let result = SummaryContract::declare(
            "view/expr",
            Modality::Matrix,
            SummaryObligation::new("o/x"),
            vec![unlocatable],
            tabular_preserved(),
            discarded_rows(),
            est(10),
            "operator/mean",
        );
        assert!(matches!(result, Err(SummaryError::NoSourceLocator { .. })));
    }

    #[test]
    fn every_declared_loss_says_whether_it_can_be_recovered() {
        let contract = declare(
            SummaryObligation::new("o/x"),
            tabular_preserved(),
            Discarded::of(vec![
                DiscardedAspect::recoverable("per-cell values", "expand:matrix_slice"),
                DiscardedAspect::unrecoverable("donor identifiers", "policy forbids release"),
            ]),
        )
        .expect("declares");
        let loss = contract.declared_loss();
        assert!(loss.iter().any(|line| line.contains("recover with")));
        assert!(loss.iter().any(|line| line.contains("unrecoverable")));
        assert_eq!(contract.expansions().len(), 1);
    }

    #[test]
    fn a_summary_cost_is_an_estimate_and_never_a_measurement() {
        let contract = declare(
            SummaryObligation::new("o/x"),
            tabular_preserved(),
            discarded_rows(),
        )
        .expect("declares");
        assert!(!contract.estimate.method.is_measured());
    }

    #[test]
    fn a_summary_contract_survives_a_json_round_trip() {
        let contract = declare(
            SummaryObligation::new("o/x"),
            tabular_preserved(),
            discarded_rows(),
        )
        .expect("declares");
        let text = serde_json::to_string(&contract).expect("serialises");
        let back: SummaryContract = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, contract);
    }
}
