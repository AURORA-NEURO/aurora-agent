//! Autonomous research protocol runner: a validated request in, a digested dossier and a
//! rendered report out — with every finding derived by a fixed rule from a cited measurement.
//!
//! This is autonomous **measurement science over synthetic decision worlds** — committed fixtures
//! and seeded generators — and nothing else. The blueprint does not specify a research protocol
//! runner; the request document, protocol shape, dossier schema, finding rules, and report layout
//! here are this crate's design, stated as such. What the steps *measure* is specified, and each
//! step calls the crate that owns it: 43.26 context certificates via `bioprism-fiber`, the 43.38
//! equal-engineering comparison and the 43.39 structural families and sweep via
//! `bioprism-baseline`/`bioprism-worldgen`, the 03.08/32 metamorphic suite via
//! `bioprism-mutation`, and the 1-minimal reduction (in 43.40/43.41's refusal vocabulary) via
//! `bioprism-prism`. This crate adds orchestration and receipts, never measurement logic.
//!
//! # The honesty rules this crate is built around
//!
//! - **The question is never interpreted.** A [`ResearchRequest`] records its free-text question
//!   verbatim; the protocol is planned from the *other* fields alone, and no code path anywhere
//!   in this crate branches on the question's content. The runner executes the protocol; it does
//!   not understand the question.
//! - **Findings are derived, never free-generated.** Every [`Finding`] comes from one of the
//!   fixed rules in [`findings`], is levelled [`ObservationLevel::Observation`] — a
//!   single-variant enum, so no other level is representable — and cites the content digests of
//!   the artifacts it was derived from.
//! - **Negative findings are first-class.** A tie between the compiler and a baseline is a
//!   *required* finding, flagged `negative: true` and rendered in the same register as any
//!   positive result. The repository's own headline finding is a tie.
//! - **Every dossier anchors to the pinned parity digest.** Step 0 compiles the committed
//!   `fixtures/fiber-v0.1` pair (embedded at build time) and aborts unless the certificate digest
//!   is [`PINNED_REFERENCE_CERTIFICATE_SHA256`], the value CPython, the eager Rust path, and the
//!   indexed store agree on.
//! - **Partial protocols are unrepresentable.** A step that cannot complete is a typed
//!   [`ResearchError`]; [`dossier::StepOutcome`] has exactly one variant, so an emitted dossier
//!   cannot claim a step it did not finish.
//! - **The dossier is tamper-evident.** `dossier_sha256` is computed over the canonical document
//!   with the digest field removed; [`verify_dossier`] recomputes it, checks the 64-hex shape
//!   separately from mismatch, and checks that every finding's supporting digests actually name
//!   artifacts the dossier carries.
//!
//! # Boundary
//!
//! Research and developer infrastructure: it does not diagnose an individual, recommend
//! treatment, triage care, enroll participants, or claim medical-device functionality. It can
//! never claim biology or medicine, literature or prior-work coverage, external-world
//! observation, or release-level claims from fixture evidence. Oracle review is a human gate.
//! These sentences also ship inside every dossier as [`REQUIRED_LIMITATIONS`].
//!
//! # Not implemented
//!
//! - **No question understanding.** The question is recorded and rendered verbatim, digested,
//!   and never parsed, matched, or routed on. There is no NLP anywhere in this crate.
//! - **No literature.** Nothing searches, cites, or claims coverage of prior work; the report's
//!   citations are content digests of this run's own artifacts.
//! - **No scheduling or recurrence.** One request runs one protocol to one dossier in one call.
//! - **No wall-clock.** No timestamps, durations, or dates appear in any artifact; determinism
//!   is byte-for-byte and the only order is protocol order.
//! - **No oracle acceptance.** The runner emits observations for human review and accepts,
//!   approves, and releases nothing.
//! - **Figures are static SVG only**, rendered by `bioprism-figures`: no raster, no scripts, no
//!   interactivity.
//! - **No I/O.** Fixtures are compiled in; the caller writes the dossier, report, and figures
//!   wherever they belong.
//! - **No generator knobs beyond the presets.** A request chooses one committed 43.39 family,
//!   one seed, and up to six distractor counts; skeleton, events, protected set, decision time,
//!   and policy stay at each preset's committed values, and the sweep runs the committed default
//!   grid at the grid's own seed.

pub mod dossier;
pub mod error;
pub mod findings;
pub mod protocol;
pub mod report;
pub mod request;
pub mod runner;

pub use dossier::{
    artifact_record, build_dossier, verify_dossier, RecordedArtifact, StepOutcome, DOSSIER_SCHEMA,
    INLINE_ARTIFACT_CAP_BYTES, REQUIRED_LIMITATIONS,
};
pub use error::ResearchError;
pub use findings::{
    comparison_findings, minimization_findings, mutation_findings, reference_anchor_finding,
    sweep_findings, Finding, ObservationLevel,
};
pub use protocol::{plan_protocol, ProtocolStep, ResearchProtocol};
pub use report::{render_report, RenderedReport};
pub use request::{
    ResearchRequest, ResearchRequestDocument, WorldFamily, MAX_DISTRACTORS_PER_POINT,
    MAX_DISTRACTOR_POINTS, MAX_QUESTION_BYTES, MAX_RESEARCH_ID_CHARS,
};
pub use runner::{run_research, PINNED_REFERENCE_CERTIFICATE_SHA256, UNSWEPT_KNOBS_CAVEAT};
