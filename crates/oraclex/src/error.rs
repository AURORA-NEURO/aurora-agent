//! Structured failures for the reference-standard and mutation-validation layers.
//!
//! Every variant names the artifact that was wrong and what was expected of it. None of them is a
//! verdict: a malformed declaration is a caller error, whereas "the evidence does not settle this"
//! is [`crate::Determination::Unresolved`] and travels as a value, not as an `Err`.

use thiserror::Error;

/// Everything this crate refuses to construct or evaluate.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum OracleXError {
    /// [`crate::Unresolved`] with nothing named. The whole point of the state is the list.
    #[error("an unresolved determination must name at least one missing item of evidence")]
    UnresolvedWithoutMissingEvidence,

    /// [`crate::NotEvaluable`] with an empty reason.
    #[error("a not-evaluable determination must state why the check does not apply")]
    NotEvaluableWithoutReason,

    /// A contradiction with no finding is an assertion, not a witness.
    #[error("a contradicted determination must carry at least one finding as its witness")]
    ContradictionWithoutFinding,

    /// 31.05, 31.11: a reference standard names the process that produced it.
    #[error("reference standard '{standard}' declares no measurement process")]
    StandardWithoutProcess { standard: String },

    /// 31.06: a consensus rule applied to no reads.
    #[error("consensus rule '{rule}' was applied to an empty reader panel")]
    EmptyPanel { rule: String },

    /// 31.06: the same reader contributed twice, so the panel size overstates independence.
    #[error("reader '{reader}' appears more than once in the panel")]
    DuplicateReader { reader: String },

    /// 31.12: two date sources with no declared precedence between them.
    #[error("source hierarchy does not rank '{left}' against '{right}'")]
    UnrankedSources { left: String, right: String },

    /// 31.12, 32.16: a value that cannot be a measurement.
    #[error("'{field}' received the non-finite value {value}")]
    NonFinite { field: &'static str, value: f64 },

    /// 32.16: two quantities in different dimensions were compared.
    #[error("cannot compare {left} with {right}: different dimensions")]
    DimensionMismatch { left: String, right: String },

    /// 32.16: a conversion the caller did not supply the constant for. The crate never guesses a
    /// molecular weight.
    #[error("no caller-supplied factor converts {from} to {to}")]
    NoConversionFactor { from: String, to: String },

    /// 32.05, 32.21: a declaration whose parent is not content-addressed.
    #[error("mutation declaration '{declaration}' has no content-addressed parent")]
    UnrootedDeclaration { declaration: String },

    /// 31.09: an escrowed outcome was requested without a token from a predeclared reveal rule.
    #[error("escrow '{escrow}' cannot be opened: no reveal rule has fired")]
    EscrowSealed { escrow: String },

    /// 31.09: a reveal rule that was written after the snapshot was frozen.
    #[error("reveal rule '{rule}' was declared at {declared_at}, after the snapshot froze at {frozen_at}")]
    RuleDeclaredAfterFreeze {
        rule: String,
        declared_at: String,
        frozen_at: String,
    },
}
