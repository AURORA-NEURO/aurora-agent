//! Conformance suites, golden fixtures, the test pyramid and release gates.
//!
//! Implements blueprint 40.31 (test pyramid and quality gates), 40.32 (conformance suites and
//! certification), 40.33 (golden fixtures and synthetic data), the evidence half of 40.44
//! (release checklist) and the release gate of 43.40, under the governance model of section 14 —
//! 14.06 promotion evidence, 14.12 claim tiers, 14.20 nonconformity handling and 14.21
//! reproduction levels.
//!
//! # The one idea
//!
//! A test suite exists to make a claim falsifiable. Everything here follows from asking what
//! could quietly stop that being true:
//!
//! - The suite tests the *implementation that wrote it*, so a second implementation cannot be
//!   checked against the same bar. Answer: cases are [`ConformanceCase`] documents, not `#[test]`
//!   functions, evaluated over published JSON through the [`Implementation`] boundary.
//! - The fixtures drift and the assertions silently follow them. Answer: every fixture carries a
//!   frozen digest and a shape, and a mismatch is a hard [`ConformanceError`] naming what moved —
//!   never a failing case, because once the inputs are unknown every verdict in the run is void.
//! - The suite is all end-to-end, or all unit, and green either way. Answer: [`pyramid`] reports
//!   the distribution and names the thin layer.
//! - Release is gated on a boolean that someone eventually flips. Answer: [`assess`] returns a
//!   [`ReleaseDecision`] that names the unmet gate, its blueprint clause, and the evidence.
//!
//! ```no_run
//! use bioprism_conformance::fiber_suite::{
//!     fiber_suite, shipped_baseline, workspace_fixture_root, FiberReference,
//! };
//! use bioprism_conformance::{assess, FixtureStore};
//!
//! let suite = fiber_suite();
//! let fixtures = FixtureStore::load(workspace_fixture_root(), &suite.manifest)?;
//! fixtures.verify()?;
//!
//! let report = suite
//!     .run(&FiberReference, &fixtures)?
//!     .declaring_baseline(shipped_baseline());
//! println!("{}", report.summary());
//! println!("{}", assess(&report).explain());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # What this crate does not do
//!
//! Named here rather than discovered later, because a quality-gate crate that overstates its own
//! coverage is the worst possible thing for it to be.
//!
//! - **No coverage instrumentation.** There is no line, branch or contract-coverage measurement.
//!   40.31 asks for coverage that is "contract-based, not only line-based"; what this crate
//!   offers is a per-case record of which blueprint modules a run *claims* to enforce, which is a
//!   traceability index, not a measurement. Nothing verifies that a case enforcing `43.13`
//!   exercises anything of 43.13.
//! - **No mutation-score tooling.** 40.31's verification plan calls for mutation testing to show
//!   the gates are sensitive to real defects. There is none here, and no other crate in this
//!   workspace supplies it, so the sensitivity of these gates is untested.
//! - **No flakiness history.** [`SuiteReport::retried_to_green`] is a caller declaration. Nothing
//!   observes reruns, so the gate enforcing 40.31's fourth invariant is only as good as the
//!   honesty of the harness that fills the field.
//! - **No clean-environment attestation.** [`EnvironmentManifest`] records the platform and
//!   explicitly lists what it does not attest: toolchain, lockfile, container, isolation.
//! - **No signing, expiry evaluation or revocation.** A [`ConformanceCertificate`] carries
//!   caller-supplied stamps and a content digest. 40.32 wants certificates bound to an
//!   implementation digest with an expiry and a revocation path; only the digest of the *suite*
//!   is bound here.
//! - **No network isolation check.** 40.31 requires external-network tests to be separated. The
//!   [`Implementation`] trait cannot observe whether an implementation reached the network.
//!
//! # Where the blueprint is vague, and what was chosen
//!
//! - 40.31 names the pyramid layers but states no ratios. [`pyramid::THIN_LAYER_SHARE`] and
//!   [`pyramid::DOMINANT_LAYER_SHARE`] are this project's numbers, exposed as constants so the
//!   choice is arguable rather than buried.
//! - 40.32 requires "certificate reference with expiry" without naming a clock or a validity
//!   period. Stamps are caller-supplied and unevaluated.
//! - 40.32 distinguishes "pass/fail/partial by requirement" but does not define partial. Here it
//!   is [`CaseOutcome::Unsupported`]: the implementation does not publish the artifact the
//!   requirement observes.
//! - 43.40 requires "an equal-engineering baseline demonstrates whether the claimed optimization
//!   is real" without saying who judges equal engineering. The gate checks that a comparator is
//!   declared and preserved the decision; it cannot check that it was competently built.

pub mod case;
pub mod context_compilation_federated_control_plane;
pub mod error;
pub mod fiber_suite;
pub mod fixture;
pub mod gate;
pub mod implementation;
pub mod knowledge_world_assurance;
pub mod pyramid;
pub mod suite;

pub use case::{
    CaseBuilder, CaseInput, ConformanceCase, Expectation, Layer, Override, OverrideOp, Requirement,
};
pub use error::{ConformanceError, InsertObstacle};
pub use fiber_suite::{
    fiber_suite, shipped_baseline, structural_profile, FiberReference, FIBER_SUITE_ID,
};
pub use fixture::{
    FixtureCard, FixtureDrift, FixtureManifest, FixtureRole, FixtureShape, FixtureStore,
    ShapeChange, FIXTURE_MANIFEST_SCHEMA_VERSION,
};
pub use gate::{assess, BaselineReference, ReleaseDecision, ReleaseGate, UnmetGate};
pub use implementation::{
    artifact, CompileArtifacts, CompileFailure, EnvironmentManifest, Implementation,
    ImplementationIdentity,
};
pub use knowledge_world_assurance::{
    assure_knowledge_world, knowledge_world_assurance_manifest, KnowledgeWorldAssuranceError,
    KnowledgeWorldDisposition, ScopedResearchClaim, ScopedResearchClaimsRequest,
    TypedKnowledgeWorldReceipt, CONTRACT_VERSION as KNOWLEDGE_WORLD_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_WORLD_ASSURANCE_FEATURE_ID,
};
pub use pyramid::{ImbalanceFinding, PyramidBalance, PyramidShape};
pub use suite::{
    CaseOutcome, CaseResult, ConformanceCertificate, RequirementStatus, Suite, SuiteReport,
    CONFORMANCE_CERTIFICATE_SCHEMA_VERSION, REPORT_SCHEMA_VERSION, SUITE_SCHEMA_VERSION,
};
