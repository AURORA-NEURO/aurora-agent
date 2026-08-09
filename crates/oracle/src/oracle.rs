//! The provider contract.
//!
//! 40.21 names `OracleProvider` as an interface and requires that "each oracle states what it
//! establishes and cannot establish". That statement lives on the [`OracleManifest`], so the
//! trait is deliberately thin: one required accessor for the declaration, one required method
//! that does the work, and defaults for the rest.
//!
//! # Why `evaluate` returns a `Result`
//!
//! The section-31 oracle contract lists `not-evaluable` among an oracle's *return values*, which
//! reads as an infallible signature. Keeping a `Result` is a deliberate departure. There are two
//! genuinely different outcomes and they must not share a representation:
//!
//! * [`Position::NotEvaluable`](crate::Position) — the oracle is healthy and this evidence is
//!   outside what it can speak to. Combination counts it as participation.
//! * [`OracleError`] — the oracle, or the harness around it, is broken. 31.15 lists "grader bug"
//!   as a distinct class of disagreement source precisely because it must not be absorbed into
//!   the epistemics. A failed oracle contributes nothing and is reported as a failure
//!   (see [`crate::OracleFailure`]); it does not become an abstention.
//!
//! Collapsing the second into the first would make a mesh of broken oracles look like a mesh of
//! appropriately cautious ones.

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::Judgement;
use crate::ladder::EvidenceTier;
use crate::manifest::OracleManifest;

/// A versioned, scoped evaluator that can be registered in an [`crate::OracleMesh`].
///
/// Object-safe: the mesh holds `Box<dyn Oracle>` and must be able to mix a schema checker, a
/// re-execution comparator, and a judge in one collection.
pub trait Oracle {
    /// The declaration this oracle stands behind.
    fn manifest(&self) -> &OracleManifest;

    /// Judges one piece of evidence.
    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError>;

    /// The `namespace:name` identity, without the version.
    fn kind(&self) -> &str {
        self.manifest().kind()
    }

    /// The rung this oracle's judgements actually occupy.
    ///
    /// This is the *effective* tier, so an oracle that declared itself deterministic while sharing
    /// training data with the evaluated system reports the demoted rung here (31.01). Reading the
    /// undemoted claim requires going through [`OracleManifest::declared_tier`], which makes the
    /// optimistic number harder to reach for than the honest one.
    fn tier(&self) -> EvidenceTier {
        self.manifest().effective_tier()
    }
}
