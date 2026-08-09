//! Reference builds: genome assemblies, template spaces, and coordinate conventions.
//!
//! Blueprint 28.00 lists "changing reference genome coordinates" among the forbidden silent
//! behaviours, and 28.01 requires "exact reference and sample context" to be maintained. 39.05
//! protects genome/reference build in the same class as units and orientation.
//!
//! A genomic coordinate is a pair of numbers and a build. `chr7:140753336` is a BRAF V600E
//! locus on GRCh38 and something else on GRCh37, where the same variant sits at
//! `chr7:140453136`. The numbers are interchangeable-looking and the builds are not, so this
//! module makes the build a required part of the position rather than an annotation beside it.
//!
//! Two traps get first-class types.
//!
//! *Coordinate convention.* A VCF record at position 1000 and a BED interval starting at 1000
//! do not refer to the same base: VCF is 1-based inclusive, BED is 0-based half-open. The same
//! two integers also describe intervals of different lengths under the two conventions. The
//! convention is therefore stored with the coordinate and blocks comparison when it differs.
//!
//! *Assembly aliasing.* `hg19` and `GRCh37` are treated as distinct builds. They share primary
//! assembly coordinates but differ in mitochondrial sequence and contig naming, so declaring
//! them equal would silently accept a chrM coordinate from the wrong assembly. Callers who know
//! their data is primary-assembly-only can say so in a transport's justification; the type
//! system will not say it for them.
//!
//! Not implemented, deliberately: no chain files, no liftover arithmetic, no atlas volumes. A
//! [`BuildTransport`] is a *declaration* that a lift happened and a ledger of what it cost; the
//! mapped coordinate is supplied by whichever tool did the lifting. Computing one here would
//! mean shipping fabricated chain data.

use crate::error::{Incomparability, ReferenceError};
use bioprism_scope::{LossLedger, MappingCheck, MappingKind, ScopeKey, ScopeMapping};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A named genome assembly.
///
/// `Grch37` and `Hg19` are separate variants on purpose; see the module documentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "build", rename_all = "snake_case")]
pub enum GenomeBuild {
    Grch37,
    Hg19,
    Grch38,
    Hg38,
    T2tChm13,
    /// Anything the crate does not know by name, kept verbatim rather than coerced.
    Other {
        name: String,
    },
}

impl GenomeBuild {
    pub fn label(&self) -> String {
        match self {
            GenomeBuild::Grch37 => "GRCh37".to_string(),
            GenomeBuild::Hg19 => "hg19".to_string(),
            GenomeBuild::Grch38 => "GRCh38".to_string(),
            GenomeBuild::Hg38 => "hg38".to_string(),
            GenomeBuild::T2tChm13 => "T2T-CHM13".to_string(),
            GenomeBuild::Other { name } => name.clone(),
        }
    }

    /// Recognises the exact spellings above; everything else becomes [`GenomeBuild::Other`].
    ///
    /// There is no fuzzy matching. `"GRCh38.p13"` is not silently folded into `Grch38`, because
    /// patch releases change contig content and a caller that meant the patch should keep it.
    pub fn parse(name: &str) -> GenomeBuild {
        match name {
            "GRCh37" => GenomeBuild::Grch37,
            "hg19" => GenomeBuild::Hg19,
            "GRCh38" => GenomeBuild::Grch38,
            "hg38" => GenomeBuild::Hg38,
            "T2T-CHM13" => GenomeBuild::T2tChm13,
            other => GenomeBuild::Other {
                name: other.to_string(),
            },
        }
    }
}

impl fmt::Display for GenomeBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// The spatial reference an imaging frame is expressed in.
///
/// No atlas data ships with this crate. A template is a name and a version string; the
/// millimetre grid it denotes lives wherever the pack stores its reference assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reference", rename_all = "snake_case")]
pub enum ReferenceSpace {
    /// The subject's own scanner space. Two subjects' native spaces are never the same space,
    /// so the subject identifier is part of the reference.
    SubjectNative { subject: String },
    Template { name: String, version: String },
}

impl ReferenceSpace {
    pub fn label(&self) -> String {
        match self {
            ReferenceSpace::SubjectNative { subject } => format!("native:{subject}"),
            ReferenceSpace::Template { name, version } => format!("{name}@{version}"),
        }
    }
}

impl fmt::Display for ReferenceSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// How integer coordinates index a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateConvention {
    /// VCF, GFF, SAM: the first base is 1, and an interval includes both endpoints.
    OneBasedInclusive,
    /// BED, BAM records, most APIs: the first base is 0, and the end is exclusive.
    ZeroBasedHalfOpen,
}

impl CoordinateConvention {
    pub fn as_str(self) -> &'static str {
        match self {
            CoordinateConvention::OneBasedInclusive => "1-based inclusive",
            CoordinateConvention::ZeroBasedHalfOpen => "0-based half-open",
        }
    }
}

impl fmt::Display for CoordinateConvention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A build, or the admission that none was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BuildBinding {
    Declared(GenomeBuild),
    Unstated,
}

impl BuildBinding {
    pub fn declared(&self) -> Option<&GenomeBuild> {
        match self {
            BuildBinding::Declared(build) => Some(build),
            BuildBinding::Unstated => None,
        }
    }

    pub fn is_stated(&self) -> bool {
        matches!(self, BuildBinding::Declared(_))
    }

    pub fn require(&self, context: &str) -> Result<&GenomeBuild, ReferenceError> {
        self.declared().ok_or_else(|| ReferenceError::BuildNotDeclared {
            context: context.to_string(),
        })
    }
}

/// A single genomic locus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicPosition {
    pub build: BuildBinding,
    pub contig: String,
    pub position: u64,
    pub convention: CoordinateConvention,
}

impl GenomicPosition {
    pub fn new(
        build: BuildBinding,
        contig: impl Into<String>,
        position: u64,
        convention: CoordinateConvention,
    ) -> Self {
        GenomicPosition {
            build,
            contig: contig.into(),
            position,
            convention,
        }
    }

    /// Returns the first blocking reason two loci cannot be compared.
    ///
    /// Build is checked before contig and convention: when the assembly is unknown, agreement
    /// on `chr7` says nothing, because `chr7` is a different sequence in each build.
    pub fn comparable_with(&self, other: &GenomicPosition) -> Result<(), Incomparability> {
        let (Some(mine), Some(theirs)) = (self.build.declared(), other.build.declared()) else {
            let side = match (self.build.is_stated(), other.build.is_stated()) {
                (false, false) => "both loci",
                (false, true) => "the left locus",
                _ => "the right locus",
            };
            return Err(Incomparability::UnstatedBuild {
                side: side.to_string(),
            });
        };
        if mine != theirs {
            return Err(Incomparability::BuildMismatch {
                left: mine.label(),
                right: theirs.label(),
            });
        }
        if self.convention != other.convention {
            return Err(Incomparability::ConventionMismatch {
                left: self.convention.to_string(),
                right: other.convention.to_string(),
            });
        }
        if self.contig != other.contig {
            return Err(Incomparability::ContigMismatch {
                left: self.contig.clone(),
                right: other.contig.clone(),
            });
        }
        Ok(())
    }
}

/// A genomic interval, whose length depends on its convention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicInterval {
    pub build: BuildBinding,
    pub contig: String,
    pub start: u64,
    pub end: u64,
    pub convention: CoordinateConvention,
}

impl GenomicInterval {
    pub fn new(
        build: BuildBinding,
        contig: impl Into<String>,
        start: u64,
        end: u64,
        convention: CoordinateConvention,
    ) -> Self {
        GenomicInterval {
            build,
            contig: contig.into(),
            start,
            end,
            convention,
        }
    }

    /// Number of bases covered.
    ///
    /// The same `(start, end)` pair spans a different number of bases under each convention,
    /// which is why the convention travels with the interval rather than being applied by
    /// whichever reader happens to consume it.
    pub fn length(&self) -> u64 {
        match self.convention {
            CoordinateConvention::ZeroBasedHalfOpen => self.end.saturating_sub(self.start),
            CoordinateConvention::OneBasedInclusive => {
                self.end.saturating_sub(self.start).saturating_add(1)
            }
        }
    }
}

/// A declared move of coordinates from one build to another.
///
/// The shape mirrors [`bioprism_scope::ScopeMapping`]: a cross-build move is a transport, and a
/// transport with an empty [`LossLedger`] is a defect rather than a clean result. Lifting drops
/// regions that have no counterpart, splits others, and changes the coordinates of everything
/// else; a ledger claiming none of that happened is a claim no chain file supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildTransport {
    pub from: GenomeBuild,
    pub to: GenomeBuild,
    /// The tool and chain that performed the lift, recorded verbatim.
    pub method: String,
    pub loss: LossLedger,
}

impl BuildTransport {
    pub fn new(from: GenomeBuild, to: GenomeBuild, method: impl Into<String>) -> Self {
        BuildTransport {
            from,
            to,
            method: method.into(),
            loss: LossLedger::default(),
        }
    }

    pub fn declaring(mut self, loss: LossLedger) -> Self {
        self.loss = loss;
        self
    }

    /// Rejects a transport that claims to have lost nothing.
    pub fn check(&self) -> Result<(), ReferenceError> {
        if self.loss.is_empty() {
            return Err(ReferenceError::UndeclaredLoss {
                from: self.from.label(),
                to: self.to.label(),
            });
        }
        Ok(())
    }

    /// Applies the transport to a locus, given the coordinate the lifting tool produced.
    ///
    /// `lifted` is `None` when the tool could not map the position; the caller must say why,
    /// because "unmapped" without a reason is indistinguishable from "not attempted". The
    /// convention is carried through unchanged: a lift changes the assembly, not whether
    /// coordinates are 0- or 1-based.
    pub fn apply(
        &self,
        source: &GenomicPosition,
        lifted: Option<(String, u64)>,
        unmapped_reason: Option<&str>,
    ) -> Result<LiftOutcome, ReferenceError> {
        self.check()?;
        let declared = source.build.require("source locus")?;
        if declared != &self.from {
            return Err(ReferenceError::TransportSourceMismatch {
                expected: self.from.label(),
                found: declared.label(),
            });
        }
        match lifted {
            Some((contig, position)) => Ok(LiftOutcome::Mapped {
                position: GenomicPosition::new(
                    BuildBinding::Declared(self.to.clone()),
                    contig,
                    position,
                    source.convention,
                ),
                record: LiftRecord {
                    from: self.from.clone(),
                    to: self.to.clone(),
                    method: self.method.clone(),
                    loss: self.loss.clone(),
                },
            }),
            None => {
                let reason = unmapped_reason.ok_or_else(|| ReferenceError::UnmappedWithoutReason {
                    context: format!("{}:{}", source.contig, source.position),
                })?;
                Ok(LiftOutcome::Unmapped {
                    reason: reason.to_string(),
                })
            }
        }
    }

    /// Renders the transport as a `bioprism-scope` mapping over the `genome_build` dimension.
    ///
    /// `genome_build` is already classified [`bioprism_scope::ScopeClass::Coordinate`] in the
    /// default dimension registry, so a build lift is expressible in the existing scope algebra
    /// and does not need a parallel one. The resulting mapping passes
    /// [`bioprism_scope::ScopeMapping::check`] exactly when the loss ledger is non-empty.
    pub fn to_scope_mapping(&self) -> ScopeMapping {
        ScopeMapping {
            from: ScopeKey::new().exact("genome_build", self.from.label()),
            to: ScopeKey::new().exact("genome_build", self.to.label()),
            kind: MappingKind::Transport {
                justification: self.method.clone(),
            },
            loss: self.loss.clone(),
        }
    }

    /// Convenience for callers that want the scope verdict directly.
    pub fn scope_check(&self) -> MappingCheck {
        self.to_scope_mapping().check()
    }
}

/// What a lift produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LiftOutcome {
    Mapped {
        position: GenomicPosition,
        record: LiftRecord,
    },
    /// No counterpart in the target build. Kept as an outcome rather than an error because an
    /// unmappable region is data about the region, not a failure of the pipeline.
    Unmapped {
        reason: String,
    },
}

/// The receipt attached to a successful lift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiftRecord {
    pub from: GenomeBuild,
    pub to: GenomeBuild,
    pub method: String,
    pub loss: LossLedger,
}
