//! Typed failures for pack declaration, assessment and publication.
//!
//! Blueprint 03.06 and 15.00 share one failure-handling rule: detect the condition explicitly,
//! fail closed where integrity is affected, and emit an actionable diagnostic rather than
//! silently repairing state. Every variant below marks a place where returning a
//! plausible-looking number would be worse than returning nothing — an unpinned dependency
//! resolved by name, a count that was never materialized, a score taken from a saturated pack.

use bioprism_ids::CanonicalError;
use thiserror::Error;

/// Every way a pack can fail to be declared, assessed or published.
#[derive(Debug, Error)]
pub enum PackError {
    /// Pack identifiers appear in published results and in dependency edges, so they are
    /// constrained rather than free-form.
    #[error(
        "pack id `{0}` is not of the form `namespace.name` over [a-z0-9.-] starting with a letter"
    )]
    MalformedPackId(String),

    /// 03.06 requires dependencies pinned by digest. Name resolution would let a pack's
    /// meaning change under a published result without the result's identifiers changing.
    #[error("pack `{pack}` depends on `{dependency}` without a digest; dependencies are pinned by digest, never resolved by name")]
    UnpinnedDependency { pack: String, dependency: String },

    /// 03.06: "Published counts must be materializable and validated, not merely theoretical."
    #[error("pack `{pack}` declares {declared} instances but its seed range can materialize only {available}")]
    CountsExceedGenerator {
        pack: String,
        declared: u64,
        available: u64,
    },

    /// Validation is a filter, so it can never admit more than was generated.
    #[error("pack `{pack}` reports {validated} validated instances out of {declared} declared; validation cannot admit more instances than exist")]
    ValidatedExceedsDeclared {
        pack: String,
        declared: u64,
        validated: u64,
    },

    /// A pack with no capability claim cannot be placed on the coverage matrix, so its score
    /// cannot be interpreted. 15.00 makes the capability mapping part of pack eligibility.
    #[error("pack `{0}` claims no capability family; a pack that names nothing it measures produces an uninterpretable score")]
    NoCapabilityClaim(String),

    /// 15.00 pack eligibility requires nontrivial oracles.
    #[error("pack `{0}` declares no oracle; there is nothing that could decide an instance")]
    NoOracle(String),

    /// Schema ranges bound which runtimes may execute the pack.
    #[error("pack `{pack}` declares schema range {min}..={max}, which is empty")]
    EmptySchemaRange { pack: String, min: u32, max: u32 },

    /// An observation with more passes than trials is a data-entry fault, not a pass rate
    /// above one.
    #[error("system `{system}` records {passes} passes in {trials} trials")]
    ImpossibleObservation {
        system: String,
        passes: u32,
        trials: u32,
    },

    /// Blueprint invariant: every reported result is linked to an immutable benchmark-pack
    /// version. An assessment of one revision says nothing about another.
    #[error("assessment is bound to pack digest {assessed}, but the pack presented has digest {presented}")]
    AssessmentDigestMismatch { assessed: String, presented: String },

    /// The central refusal of 15.19: an unhealthy pack yields no number at all.
    #[error("pack `{pack}` is not reportable as a score: {findings}")]
    UnreportablePack { pack: String, findings: String },

    /// A pack with no trials has no pass rate; an absent measurement is not a zero.
    #[error("pack `{0}` has no recorded trials, so it has no pass rate")]
    NoObservations(String),

    /// 03.06 trust boundary: metadata is parsed before code, so a manifest is readable even
    /// when the rest of the document is not.
    #[error("pack document has no `manifest` object")]
    MissingManifest,

    #[error("pack document is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("pack document could not be canonicalized for hashing: {0}")]
    Canonical(#[from] CanonicalError),

    #[error("no pack with id `{0}` is in the portfolio")]
    UnknownPack(String),
}
