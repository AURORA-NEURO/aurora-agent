//! Typed failures of the lens grammar.
//!
//! Every variant here is a *construction* failure: something that would have produced a lens
//! declaration, a coverage claim or a report that lies. None of them is an "answer is bad"
//! signal. A lens that cannot answer says so through [`crate::LensOutcome`], which distinguishes
//! refusal from absent evidence; a lens that cannot be *built correctly* fails here.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LensError {
    /// A lens declared no question. 42.01 compiles views from questions; a lens with no question
    /// is a rendering with a name attached.
    #[error("lens `{lens}` declares an empty question")]
    EmptyQuestion { lens: String },

    /// A lens declared scope preconditions but did not declare the refusal it must issue when
    /// they are unmet. Declaring a precondition you will silently ignore is worse than declaring
    /// none.
    #[error(
        "lens `{lens}` declares {count} scope precondition(s) but does not declare \
         `scope_precondition_unmet` among its refusals"
    )]
    PreconditionWithoutRefusal { lens: String, count: usize },

    /// A lens refused for a reason it never declared. The catalogue is a contract: a reader must
    /// be able to learn what a lens will not answer *before* asking.
    #[error("lens `{lens}` refused with undeclared reason `{reason}`")]
    UndeclaredRefusal { lens: String, reason: &'static str },

    /// A witness produced a different number of cells than it declared columns. 42.27 requires
    /// the non-visual rendition to be the answer, not a caption; a ragged row is not readable as
    /// a table.
    #[error(
        "witness `{kind}` from lens `{lens}` declares {columns} column(s) but produced {cells} cell(s)"
    )]
    MalformedWitness {
        lens: String,
        kind: &'static str,
        columns: usize,
        cells: usize,
    },

    /// A coverage claimed to be partial named nothing outstanding. 42.30's failure mode is a
    /// partially loaded view that looks finished; an unnamed remainder is exactly that.
    #[error("partial coverage of lens `{lens}` names no pending region")]
    PartialCoverageWithoutPendingRegion { lens: String },

    /// A coverage examined at least as many units as were eligible yet claimed to be partial, or
    /// examined more units than exist.
    #[error("coverage of lens `{lens}` examined {examined} of {eligible} eligible unit(s)")]
    IncoherentCoverage {
        lens: String,
        examined: usize,
        eligible: usize,
    },

    /// An evidence gap named no absent requirement. "Evidence absent" without saying *which*
    /// evidence is a refusal wearing a different label.
    #[error("lens `{lens}` reported absent evidence without naming an absent requirement")]
    EmptyEvidenceGap { lens: String },

    /// A measured value was not a real number. There is no representation for a quantity that
    /// is simultaneously present and meaningless, because renderers turn one into a blank cell.
    #[error("assay `{assay}` produced a non-finite value for `{analyte}`")]
    NonFiniteMeasurement { assay: String, analyte: String },

    /// An anytime curve carried no planned stratum, so its coverage is undefined and every point
    /// on it would read as final. 42.22's whole content is that a curve knows what it has not
    /// reached.
    #[error("anytime evaluation `{evaluation}` declares no planned stratum")]
    NoPlannedStrata { evaluation: String },

    /// An anytime curve reported a point in a stratum that was never planned, so the denominator
    /// the curve reports against does not contain the numerator.
    #[error("anytime evaluation `{evaluation}` observed unplanned stratum `{stratum}`")]
    UnplannedStratum { evaluation: String, stratum: String },

    /// The report body could not be canonicalised, so no receipt digest exists for it.
    #[error("lens `{lens}` report could not be canonicalised: {detail}")]
    Uncanonicalisable { lens: String, detail: String },
}
