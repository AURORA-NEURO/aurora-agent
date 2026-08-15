//! The explained residue: every uncovered blueprint module, and why no crate implements it.
//!
//! `docs/COVERAGE.md` states the end state this workspace is aiming at, and it is not a full bar:
//!
//! > **100% is not the target and never was** — some remaining modules are prose that no crate
//! > should implement, and the honest end state is a backlog whose residue is explained rather than
//! > empty.
//!
//! That sentence has been true and unenforceable. The explanations existed — ten crates had read
//! their section's remainder and written down what they found — but they lived in ten `lib.rs` doc
//! comments, in prose, in whichever crate happened to be built next to them. Nothing could ask *how
//! many of the fifty-five are process*, nothing noticed when two crates disagreed, and nothing
//! failed when a module left the backlog and its explanation did not. That last one is no longer
//! hypothetical: twenty-seven modules left in a single pass, and the register said so before anybody
//! read it.
//!
//! This crate is that accounting, typed.
//!
//! ```
//! use bioprism_residue::{residue, Report};
//!
//! let register = residue()?;
//! let report = Report::of(&register);
//!
//! // Four of the five verdicts mean a module will never move. One means it still can, and it is
//! // counted separately rather than folded in.
//! assert_eq!(report.distribution.modules.total(), register.len());
//! assert!(report.distribution.work_remaining > 0);
//! # Ok::<(), bioprism_residue::RegisterError>(())
//! ```
//!
//! # The one idea
//!
//! **A verdict cannot be asserted without a source.** [`Verdict`] has private fields and one
//! constructor, and that constructor requires a [`Source`]: the crate whose judgement it is, the
//! file it was recorded in, a fragment of that file which must still be present, and the argument
//! in prose. Deserialisation goes through the same gate, because the wire form is the form a
//! register travels in.
//!
//! `bioprism-atlas` gates a measurement this way and `bioprism-metrics` gates a score. The reason
//! is identical: an explanation that reaches a reader without its basis is indistinguishable from
//! one that has none, and this register would otherwise be a second place to write "process" and
//! move on.
//!
//! The gate is not decorative. `discharged elsewhere` naming no crate does not compile into a
//! value; `nobody has read it` with no survey does not either; and a survey that found no recorded
//! judgement cannot be marked as transcribed *from* one, which is the single self-contradictory
//! pair the vocabulary admits.
//!
//! # The trap this crate is shaped around
//!
//! `tools/coverage.sh` counts a blueprint module as covered when its `NN.MM` token appears anywhere
//! under `crates/` or `docs/`. A register listing fifty-five uncovered ids would mark all
//! fifty-five covered and take the headline to 100% — a number produced entirely by the document
//! complaining that the number is produced that way.
//!
//! Two crates have already been bitten. `docs/BACKLOG.md` emptied itself on its second run, because
//! the run that generated it counted the file it had just written. `crates/sweep`'s citation test
//! once cited an id it existed to assert was uncited.
//!
//! So [`ModuleKey`] holds a section and an index as integers, has no constructor taking a string,
//! and renders an id only at run time; and [`citations::audit`] reproduces the coverage script's
//! token rule and scans this crate's own directory, failing on any token that names a module the
//! register classifies. `crates/devplat`'s audit caught the subtler version of the same mistake — a
//! measured percentage written to two decimal places whose integer part fell in the section range
//! scans as a citation — so every figure in this crate is written to one decimal place.
//!
//! # The five verdicts, and where they came from
//!
//! Reused, not re-cut. A sixth vocabulary for the same distinctions is how two crates end up
//! disagreeing about a module for reasons that are entirely about wording.
//!
//! | Verdict | Established by | The test |
//! |---|---|---|
//! | [`Classification::Process`] | `crates/stewardship` | is the detailed design a set of predicates over an artifact, or a description of what people do? |
//! | [`Classification::ForeignArtifact`] | `crates/ops`, `crates/devplat` | code-bearing, precise and testable, and not Rust and not in this repository |
//! | [`Classification::DischargedElsewhere`] | `crates/worldfactory`, `crates/sweep` | the content is implemented under a different section's id, by a crate that never names this one |
//! | [`Classification::BlockLevelSplit`] | `crates/bioevalx` | the prose/code division runs *inside* the module, so it is simultaneously implemented and not |
//! | [`Classification::GenuinelyUncovered`] | the honest default | nobody has read it, or it is real work not yet done |
//!
//! # What the register says
//!
//! Fifty-six modules, eighty recorded verdicts. By primary verdict:
//!
//! ```text
//! process                37   councils, cadence, appeals, a dashboard somebody reads
//! foreign artifact       10   Python, TypeScript, two GitHub Actions, a workflow file, OTLP
//! discharged elsewhere    9   the content exists, under another section's id
//! genuinely uncovered     0   nobody has read it, or it is real work not yet done
//! ```
//!
//! **Exactly one module carries work on any reading**, and it is a second reading rather than a
//! headline — see the regeneration note below before reading that zero as good news. **Eighty of
//! the eighty verdicts are transcriptions** of a sentence a classifying crate wrote about the
//! module named; the one exception is marked [`Standing::InferredHere`] and sits beside a
//! transcription on the same module, so the disagreement has somewhere to land.
//!
//! Two findings a token scan cannot produce:
//!
//! **Ten modules are contested** — two crates, two verdicts, both recorded, neither adjudicated.
//! All ten are one argument: whether a section whose modules define no estimator is prose, or is
//! already discharged by the crate implementing the discipline around every estimator it names.
//! Picking a winner would delete the evidence that the workspace has two answers.
//!
//! **Ten verdicts name their own author as the discharger** — a crate that built the content
//! under a different section's id, so coverage reports the module untouched while the capability
//! exists. Every one of those is a module a contributor could otherwise pick up and build twice.
//!
//! # The number that moved, and why it is not a victory lap
//!
//! This register was first written against a backlog of eighty-four modules, of which **twenty-six
//! carried work and nineteen had no recorded judgement at all**. Four crates then landed —
//! `dataops`, `interweave`, `megafactory`, `bioethics` — and the backlog fell to fifty-seven.
//!
//! The honest accounting of where twenty-five of the twenty-six went: **all of them left the
//! backlog**, because the crate that owned their section was written and cited them. Not one was
//! reclassified into a bucket that never moves, and
//! `the_sections_that_had_nobody_have_left_the_register_rather_than_been_reclassified` asserts it by
//! checking those sections are *absent* rather than relabelled.
//!
//! The twenty-sixth is the one to keep looking at. `crates/bioethics` read the sandboxing module and
//! declined it as perimeter infrastructure a sibling already positioned, which is a discharge — and
//! in the same paragraph recorded that all thirteen of its required controls need a process
//! boundary, a network stack or a scanner, none of which exists here. The discharge is transcribed;
//! the reading that the control therefore exists nowhere is this register's, marked as such, and
//! kept as a second verdict. A register showing zero work remaining while the workspace has no
//! sandbox would be the flattering answer, and `AGENTS.md` already names that failure: a missing
//! capability that is stated is a limitation, one that is implied to exist is a lie.
//!
//! # This crate does not classify anything
//!
//! Stated plainly because the temptation runs the other way. Where a crate reached a verdict, this
//! register transcribes it — including where two crates transcribe to different verdicts. Where no
//! crate reached one, the entry says so and names the search. There is no rule here that decides a
//! module's verdict from its text, no heuristic over titles, and no default that fills an empty
//! entry: [`Entry::new`] refuses a module with no verdicts, because that value is exactly a
//! `docs/BACKLOG.md` line and this crate exists to be the thing that is not that.
//!
//! Also absent: no blueprint reading. Nothing here opens the distribution, so every classification
//! is somebody else's reading of the source module rather than this crate's. No clock, no
//! randomness, no network. The only files read are `docs/BACKLOG.md`, the workspace manifests and
//! the loci named in the register, all as text.
//!
//! # Keeping it true
//!
//! [`reconcile::reconcile`] compares the register against the live backlog in both directions and
//! resolves every source against the working tree — the crate must be a workspace member, the file
//! must live inside that crate, and the anchored fragment must still be there. Four crates are
//! being written against these sections right now, so a module leaving the backlog is the expected
//! case and is a deletion, not a rewrite. [`register`] carries the procedure.

pub mod citations;
pub mod entry;
pub mod error;
pub mod module;
pub mod reconcile;
pub mod register;
pub mod report;
pub mod verdict;

pub use citations::{audit_self, forbidden_ids, scan, CitationAudit};
pub use entry::{Entry, EntryFields, Register};
pub use error::{CitationError, ReconciliationError, RegisterError, VerdictError};
pub use module::{ModuleKey, ModuleKeyFields};
pub use reconcile::{reconcile, BacklogLine, Reconciliation, SourceDefect, SourceProblem};
pub use register::residue;
pub use report::{Counts, Distribution, Highlight, Report};
pub use verdict::{
    Classification, Dischargers, ForeignSurface, Source, SourceFields, Standing, Survey,
    UncoveredStanding, Verdict, VerdictFields,
};

/// The schema version of the register objects this crate emits.
///
/// Bumped when a serialized shape changes, for the reason `bioprism-metrics` versions its own: a
/// stored [`Entry`] read under different construction rules is an explanation claiming a source it
/// may no longer have.
pub const RESIDUE_SCHEMA_VERSION: &str = "bioprism-residue/0.1";
