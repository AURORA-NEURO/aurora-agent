//! Reference BioWorlds and runnable vertical slices.
//!
//! Implements blueprint 19 (Reference Examples) and 38 (Reference BioWorlds and Vertical Slices),
//! with 43.41 — the radiogenomic cohort-integrity compiler — as the canonical slice.
//!
//! This crate adds no theory. Its job is to make the platform's claims *runnable and
//! self-checking*. A reference example that no test executes is documentation pretending to be
//! evidence: it cannot tell you that the claim still holds, only that somebody once wrote it down.
//! So a [`VerticalSlice`] carries the outcome it expects, [`VerticalSlice::run`] returns a
//! digested [`SliceReport`], and a slice whose claim stops holding fails a test rather than
//! quietly becoming stale prose.
//!
//! ```
//! use bioprism_examples::SliceRegistry;
//!
//! let registry = SliceRegistry::standard();
//! let report = registry.run_all()?;
//! assert!(report.holds(), "{}", report.render());
//! # Ok::<(), bioprism_examples::ExampleError>(())
//! ```
//!
//! # Reading a report back
//!
//! [`SliceReport`] and [`RegistryReport`], and every struct they are made of, refuse a field they
//! do not declare. Both digests are recomputed by re-serialising the *parsed* report, so a key the
//! reader dropped would be outside the seal by construction: the recomputation never sees it, the
//! claimed digest still agrees, and a report carrying content nobody hashed reads as intact.
//!
//! # The two halves of the catalogue
//!
//! [`SliceRegistry::run_all`] reports which of [`Property::ALL`] the registered slices exercise
//! *and* which they do not. The second half is the valuable one. A catalogue listing only what
//! passes reads as completeness, and the properties nobody wrote a slice for are precisely the
//! ones a newcomer will assume are covered. Every unexercised property carries a
//! [`Property::blocker`] naming the concrete obstacle — a field the wire schema does not have, a
//! knob the generator does not expose, a code path nothing constructs.
//!
//! # What this crate does not do
//!
//! * **No baseline harness.** The neighbourhood walk in [`walk`] is a small local
//!   re-implementation so that an example does not depend on the comparison crate it is meant to
//!   make checkable. There is no vector or embedding retriever here at all: no embedding model is
//!   available offline, so the dense-retrieval column of 43.41's comparison table is missing
//!   rather than approximated.
//! * **No fixture replay.** Slices build their worlds from `bioprism-worldgen` specs, so every
//!   world in this crate is reproducible from a serialisable spec. The shipped
//!   `fixtures/fiber-v0.1` golden certificate is checked by `bioprism-conformance`, not here.
//! * **No decision cells and no PRISM forks.** 38.01's acceptance list includes forking two
//!   architectures from one cell and localising the first divergence between them. A slice is one
//!   compile and emits pass receipts rather than decisions, so there is no trajectory here for a
//!   fork to diverge along; the claim stays registered as unexercised, with the obstacle rewritten
//!   to say which two crates would have to meet.
//! * **No signatures, and therefore no third-party verifiability.** 38.01 also asks for a *signed*
//!   result bundle. `bioprism-bundle` offers HMAC-SHA256 and nothing asymmetric, so a verifier
//!   needs the producing secret and could have written the tag. The claim is registered under the
//!   narrower name [`Property::AttestedResultBundleReplay`], and the slice that exercises it
//!   records the forgery as an observation.
//! * **No timing or memory measurement.** 43.41's evaluation program asks for tokens, bytes,
//!   compile time and peak memory. Slices assert *what* was compiled, never how fast, because a
//!   wall-clock assertion in a test suite is a flake generator rather than a measurement.

pub mod bundle;
pub mod catalog;
pub mod error;
pub mod expectation;
pub mod property;
pub mod registry;
pub mod report;
pub mod scenario;
pub mod slice;
pub mod walk;

pub use bundle::BundleInputs;
pub use error::ExampleError;
pub use expectation::{
    BundleExpectation, BundleProbe, Compiled, DepthExpectation, Expectation, GraphWalkProbe,
    Refusal,
};
pub use property::{Property, PropertyClaim};
pub use registry::{CoverageReport, RegistryReport, SliceRegistry};
pub use report::{
    BundleObservation, CompiledObservation, DeferredPass, DepthObservation, GraphWalkObservation,
    Observations, PassObservation, RefusalCode, RefusalObservation, SliceReport,
};
pub use scenario::{QueryOverlay, SliceWorld};
pub use slice::VerticalSlice;
