#![allow(ambiguous_glob_reexports)]

//! The small remainders: the tails of six sections that are otherwise nearly complete.
//!
//! This crate exists because fourteen blueprint modules were each too small for a crate of their
//! own — two from §03 (Core Specifications), five from §04 (Ingestion and Interop), three from §05
//! (Execution Runtime), one from §08 (Adaptive Evaluation), two from §10 (Registry and Hub) and one
//! from §13 (Security, Privacy and Safety). They are the leftovers of sections whose main body
//! already has an owner, and leftovers of that kind are usually leftovers for a reason: they are
//! the awkward module nobody wanted.
//!
//! That turned out to be the finding. Twelve of the fourteen are code-bearing, a far higher share
//! than any other remainder examined in this workspace — `crates/stewardship` found twelve of
//! eighteen §14 modules were process, `crates/atlashub` found twelve of seventeen §34 modules were
//! interface or process. What is left at the end of a nearly-finished section is not the process
//! documentation; the process documentation gets absorbed early. What is left is the module that
//! was genuinely hard to place.
//!
//! # Classification of all fourteen
//!
//! | Module | Verdict | Where it went |
//! |---|---|---|
//! | 03.03 World State IR | code-bearing | [`state`] |
//! | 03.11 Provenance, Identifiers and Versioning | code-bearing | [`provenance`] |
//! | 04.01 Ingestion Pipeline | code-bearing | [`ingest`] |
//! | *OpenTelemetry Adapter* (§04) | integration surface | [`bioprism_trace::otel`] |
//! | 04.03 Runner and Benchmark Adapters | code-bearing | [`interop`] |
//! | 04.04 Environment and Artifact Capture | code-bearing | [`capture`] |
//! | 04.05 Redaction, Privacy and Data Minimization | code-bearing | [`redact`] |
//! | 05.10 Distributed Execution, Caching and Recovery | code-bearing | [`cache`] |
//! | 05.11 Shepherd and External Fork-Runtime Integration | code-bearing | [`bridge`] |
//! | 05.12 Runtime Conformance and Microbenchmarks | code-bearing | [`conform`] |
//! | 08.06 Regression-Focused Scheduling | code-bearing | [`regression`] |
//! | *Registry Overview* (§10) | overview, already discharged | **not implemented** — see below |
//! | 10.14 Trace and Fork Explorer | part rendering, part predicate | [`explorer`], in part |
//! | 13.11 Effects, Permissions and Human Approval | code-bearing | [`effects`] |
//!
//! The remaining registry overview is named in prose rather than by id, deliberately.
//! `tools/coverage.sh` counts any `NN.MM` token under `crates/` or `docs/`, so writing its id here
//! would move the coverage number without moving capability — which is the one thing this
//! workspace must not do.
//!
//! **The OpenTelemetry adapter** is now owned by `bioprism-trace::otel`: it accepts recorded OTLP
//! JSON, maps spans into Event IR, preserves source payloads, and exposes a semantic-loss report.
//! It intentionally does not become a network exporter or claim ownership of every provider's
//! evolving semantic conventions. Unknown fields, inferred kinds, missing timestamps, unresolved
//! parents, and unsupported links remain visible and block a lossless compilation claim.
//!
//! **The registry overview** is an architecture statement whose every predicate is discharged
//! elsewhere. Immutable resolution, signed manifests, digest and revocation verification before
//! materialisation, and the local-first contract are `bioprism-registry` and `bioprism-hubapi`;
//! leaderboard rows pointing at result bundles rather than at a database are `bioprism-hub`'s
//! `leaderboard` and `disclosure`. Implementing it here would mean building a fourth registry.
//! That is a finding, not a gap.
//!
//! # The rule all six sections turned out to need
//!
//! None of these modules knows about the others. They nonetheless all had to solve the same
//! problem, in six vocabularies: **"nobody checked" and "checked and it cannot matter" must not
//! share a representation.** That is `bioprism-fiber`'s rule about influence classes, and it
//! reappears here as:
//!
//! | Section | Currency | The two states that must not merge |
//! |---|---|---|
//! | 03.03 | state comparison | `Unchecked` vs `Equal` — one uncomparable component makes a comparison [`state::StateVerdict::Indeterminate`], never equal |
//! | 03.11 | resolution | `Tombstoned` vs `Unknown` — a withdrawal carries policy metadata, an absence carries nothing |
//! | 04.04 | capture | declared omission vs undeclared dimension — [`capture::Capture::validate`] names the second |
//! | 04.05 | scoring | `Unevaluable` vs `Fail` — and [`redact::Tally::rate`] returns `None`, not `0.0` |
//! | 04.03 | conformance | `NotRun` vs `Fail` — a suite with a hole is [`interop::SuiteStatus::Incomplete`] |
//! | 05.10 | execution | `NotAttempted` vs `Incomplete` — never dispatched is not lost in flight |
//! | 05.12 | providers | `Untested` vs `Failed` — and two untested providers get [`conform::Drift::Indeterminate`], not agreement |
//! | 08.06 | risk | unknown coverage contributes the *maximum* term, not zero |
//! | 10.14 | display | `Collapsed` vs `NotLoaded` vs `Withheld` — three reasons a node is off the screen |
//! | 13.11 | safety | `AttemptedAndIntercepted` vs `Refused` — containment does not erase intent |
//!
//! [`fidelity`] is the shared vocabulary underneath: a four-level lattice whose two middle levels
//! are exactly `Equivalent` (checked, cannot matter) and `Degraded` (differs, nobody established
//! whether it matters), with a meet, a monotone-downward update and no way to raise. §03's
//! restoration confidence, §04's reproducibility level and §05's fallback declaration are all that
//! lattice. `tests/unknown_is_not_zero.rs` asserts the pattern once per currency, so a later
//! simplification of any one of them into a boolean fails a test rather than a review.
//!
//! # Boilerplate in these six sections
//!
//! Measured as: a line is boilerplate if it appears in at least *t* of a section's modules, after
//! dropping YAML front matter, counting non-blank lines. Sensitivity run over threshold
//! (0.3/0.5/0.7/0.9), unit (line / heading-block / character) and definition (front matter in or
//! out).
//!
//! | Section | n | line, t=0.5 | t=0.3→0.9 | block | char | +front matter |
//! |---|---|---|---|---|---|---|
//! | §03 | 12 | 64.8% | flat | 48.8% | 69.8% | +2.2 |
//! | §04 | 6 | 68.2% | flat | 49.2% | 71.5% | +1.9 |
//! | §05 | 12 | 68.9% | flat | 50.0% | 72.7% | +1.9 |
//! | §08 | 8 | 69.3% | flat | 50.0% | 74.3% | +1.8 |
//! | §10 | 22 | 61.1% | **55.1% at 0.9** | 44.7% | 67.6% | +2.5 |
//! | §13 | 26 | 65.4% | flat | 48.7% | 69.7% | +2.1 |
//!
//! Five of the six are squarely in the **unit-dominated** regime: threshold does not move the
//! number at all — not by a tenth of a point between t=0.3 and t=0.9 — while switching the unit
//! from line to heading-block moves it by about twenty points. The shared blocks ("Invariants",
//! "Failure modes and mitigations", "Testing strategy", "Metrics", "Implementation sequence") are
//! byte-identical across a section, so they count once each as a block and many times each as
//! lines.
//!
//! Definition is a small constant here — front matter adds 1.8 to 2.5 points — which does **not**
//! reproduce the settled §23 and §43 findings of 4.3 and 15.1 points. The reason is arithmetic
//! rather than editorial: these modules run to roughly seventy non-blank lines and front matter is
//! seven of them, so it can only ever move the fraction by a couple of points. Where front matter
//! dominated, the modules were short. That refines "definition matters most" into something
//! testable: definition matters most *when the definitional slice is a large share of the
//! document*, and here it is not.
//!
//! §10 is the exception, and it is the §39 regime in miniature — the one section here whose corpus
//! is split. Twenty-two modules, of which the web UX, moderation, federation and internationalisation
//! ones depart from the template the other eighteen follow rigidly. That split is precisely the
//! condition under which the threshold starts to bite, and §10 is the only one of the six where it
//! does: six points between t=0.7 and t=0.9.
//!
//! So the taxonomy generalises with one correction. Threshold matters only in a mixed corpus. Unit
//! matters everywhere. Definition matters in proportion to how much of the document the definition
//! covers, which is why it dominated in the short-module sections and barely registers in these.
//!
//! # What is not implemented anywhere in this crate
//!
//! Stated once here and again at each module.
//!
//! * **No I/O of any kind.** No filesystem, no network, no clock, no RNG. Digests come from
//!   `bioprism-ids`; every other value arrives from the caller. Several modules — [`capture`],
//!   [`cache`], [`conform`] — describe systems that must eventually do I/O, and what is here is the
//!   contract those systems would have to satisfy, not the systems.
//! * **No signatures.** [`provenance`] verifies digests and records publisher names. Deciding a
//!   name corresponds to a party is a deployment's job, exactly as in `bioprism-registry`.
//! * **No parsers.** [`ingest`] enforces the shape of a two-phase import; `bioprism-adapter` and
//!   `bioprism-trace` do the reading.
//! * **No measurement.** [`conform`] enumerates §05's performance suite and stores a caller's
//!   numbers without ever comparing one to a threshold, because the blueprint supplies no
//!   thresholds. [`bridge`] likewise records no fork-latency or cache-reuse figure: 05.11's
//!   evaluation section asks for measurements and none has been taken here.
//! * **No invented constants.** The only numeric literals in the crate's logic are 0 and 1, the
//!   ends of a normalised scale. [`regression::Weights::uniform`] is labelled illustrative in its
//!   own documentation and is uniform precisely so that it expresses no preference; 08.06's
//!   sentinel-budget floor is a required argument rather than a default, and a plan validated
//!   without one is refused.
//! * **No enforcement.** [`effects`] models approval, revalidation and scoring. Nothing here
//!   intercepts anything; `bioprism-runtime`'s effect policy is the layer that refuses.
//!
//! # Where the content was already discharged
//!
//! Findings rather than gaps, and the reason several of these modules are thinner than their
//! blueprint pages:
//!
//! | Blueprint content | Already owned by |
//! |---|---|
//! | ingestion loss accounting (lossless vs unaudited) | `bioprism-adapter::loss` |
//! | redaction receipts, semantic replacement, evaluated-versus-matched | `bioprism-policy::redaction` |
//! | first divergence between two branches | `bioprism-trace::divergence` |
//! | effect classification by reversibility, undeclared-effect refusal | `bioprism-runtime::effect` |
//! | opaque run and event ids, content digests, canonical bytes | `bioprism-ids` |
//! | registry resolution, trust tiers, mirrors, non-transiting trust | `bioprism-registry`, `bioprism-hubapi` |
//! | adaptive panels, stopping, reproducible selection | `bioprism-adaptive` |
//! | twenty-five of §13's twenty-six threat modules | `bioprism-safety`, `bioprism-policy` |
//!
//! # Example
//!
//! The shape the whole crate keeps: a gap that is declared, and a gap that is not.
//!
//! ```
//! use bioprism_sweep::capture::{Capture, CaptureProfile, Dimension};
//! use bioprism_sweep::fidelity::Level;
//!
//! // A capture that mentions only one of the nine inventory dimensions is invalid, and the eight
//! // it is silent about are named rather than counted.
//! let silent = Capture::new(CaptureProfile::MetadataOnly).including(Dimension::Files);
//! assert_eq!(silent.undeclared().len(), 8);
//! assert!(silent.validate().is_err());
//!
//! // Declaring the omission — with a reason — makes the capture valid, and lowers what it may
//! // claim about reproducibility. It does not make the gap disappear.
//! let declared = Dimension::ALL
//!     .into_iter()
//!     .filter(|d| *d != Dimension::Hardware)
//!     .fold(Capture::new(CaptureProfile::MetadataOnly), |c, d| c.including(d))
//!     .omitting(Dimension::Hardware, "single-node CI; hardware is fixed by the image")?;
//!
//! assert!(declared.validate().is_ok());
//! assert_eq!(declared.reproducibility()?.level(), Level::Absent);
//! # Ok::<(), bioprism_sweep::SweepError>(())
//! ```

pub mod audit_integrity_support;
pub mod bridge;
pub mod cache;
pub mod capture;
pub mod conform;
pub mod effects;
pub mod error;
pub mod explorer;
pub mod federated_continual_audit_integrity_contract_model;
pub mod federated_continual_audit_integrity_inference;
pub mod federated_continual_audit_integrity_research_copilot;
pub mod federated_continual_audit_integrity_workflow_fabric;
pub mod fidelity;
pub mod ingest;
pub mod interop;
pub mod local_audit_integrity_contract_model;
pub mod local_audit_integrity_inference;
pub mod local_audit_integrity_research_copilot;
pub mod local_audit_integrity_workflow_fabric;
pub mod multimodal_audit_integrity_contract_model;
pub mod multimodal_audit_integrity_inference;
pub mod multimodal_audit_integrity_research_copilot;
pub mod multimodal_audit_integrity_workflow_fabric;
pub mod provenance;
pub mod redact;
pub mod regression;
pub mod state;
pub mod throughput_audit_integrity_contract_model;
pub mod throughput_audit_integrity_inference;
pub mod throughput_audit_integrity_research_copilot;
pub mod throughput_audit_integrity_workflow_fabric;

pub use audit_integrity_support::*;
pub use error::SweepError;
pub use federated_continual_audit_integrity_contract_model::*;
pub use federated_continual_audit_integrity_inference::*;
pub use federated_continual_audit_integrity_research_copilot::*;
pub use federated_continual_audit_integrity_workflow_fabric::*;
pub use fidelity::{meet, meet_all, Declaration, Level};
pub use local_audit_integrity_contract_model::*;
pub use local_audit_integrity_inference::*;
pub use local_audit_integrity_research_copilot::*;
pub use local_audit_integrity_workflow_fabric::*;
pub use multimodal_audit_integrity_contract_model::*;
pub use multimodal_audit_integrity_inference::*;
pub use multimodal_audit_integrity_research_copilot::*;
pub use multimodal_audit_integrity_workflow_fabric::*;
pub use throughput_audit_integrity_contract_model::*;
pub use throughput_audit_integrity_inference::*;
pub use throughput_audit_integrity_research_copilot::*;
pub use throughput_audit_integrity_workflow_fabric::*;
