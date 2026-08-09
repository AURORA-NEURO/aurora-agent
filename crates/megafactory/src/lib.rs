//! The six modules of section 35 that `bioprism-scale` and `bioprism-factory` did not claim.
//!
//! Section 35 is the million-scale benchmark factory. `bioprism-scale` owns its content half —
//! portfolio design, escrow, generation scheduling, deduplication and effective size, contamination
//! splits, content-addressed storage, adaptive selection, cost, the worked accounting example,
//! release versioning and audit. `bioprism-factory` owns its scheduling half: jobs, workers, leases
//! and idempotency-aware recovery. What was left is six modules, and this crate is those:
//!
//! | Module | Here | Refuses |
//! |---|---|---|
//! | 35.02 observed-data world authoring | [`authoring`] | a latent nobody can know, recorded as if known |
//! | 35.03 semi-synthetic world construction | [`semisynthetic`] | a recovery figure from a panel the batch label solves |
//! | 35.04 mechanistic simulation and assay twins | [`twin`] | a counterfactual published without the model it is true under |
//! | 35.06 trajectory capture and workflow mining | [`trajectory`] | a digest that confirms the value it redacted |
//! | 35.07 the BioDecision compiler | [`boundary`] | an agreement score that averages away a missed irreversible decision |
//! | 35.13 distributed execution and fault tolerance | [`placement`], [`executed`] | a worker judged by an oracle in its own trust domain |
//!
//! # The classification, which is the headline
//!
//! The test this workspace applies is *is the detailed design a set of predicates over an artifact,
//! or a description of what people do?* — with `bioprism-bioevalx`'s refinement that the split can
//! run inside a module rather than between modules.
//!
//! **None of the six is a pure process module, and none is purely predicates either. All six split
//! inside, and the predicate share ranges from nearly the whole module to two components out of
//! six.** That is a different answer from `bioprism-stewardship`'s §14, where twelve of eighteen
//! modules were wholly process, and from `bioprism-bioevalx`'s §26, where the split ran per *block*
//! because every module ended with the same four process blocks. Section 35 splits per *component*:
//! each module's required-components list mixes things a library can check with things only an
//! organisation can do, and the mixture is different every time.
//!
//! | Module | Components | Predicates over an artifact | Named and not built |
//! |---|---:|---|---|
//! | 35.02 | 6 | temporal lineage, native artifact preservation, decision reconstruction *(the artifact, not the act)* | data-use and provenance review, oracle mesh selection |
//! | 35.03 | 6 | assay and batch interaction, identity and temporal consistency, detectability calibration, transfer validation | background artifact selection, latent insertion model |
//! | 35.04 | 6 | intervention model, mechanistic state engine *(as arithmetic)*, assay forward model *(as a named transformation)* | specimen and sampling process, posterior calibration, workflow and cost model |
//! | 35.06 | 6 | privacy-aware redaction, artifact and state snapshots *(completeness)* | event capture, tool and error logs, human annotations, causal candidate extraction |
//! | 35.07 | 7 | all seven, as the boundary taxonomy and the agreement measured over it | — |
//! | 35.13 | 7 | data-local scheduling, worker attestation, retry and idempotency *(fencing)*, quarantine *(via the ledger)* | resource-aware queue, resource classes, checkpoint and resume |
//!
//! 35.07 is the outlier and it is the one worth building: its seven required components *are* a
//! taxonomy, and a taxonomy is a type. 35.06 is the opposite — four of its six components describe
//! instrumentation that a library with no process to attach to cannot have.
//!
//! # How much of section 35 is template
//!
//! Method: take the eighteen module files, excluding the section index; normalise each line by
//! trimming trailing whitespace; count a line as shared when the identical line occurs in at least
//! *k* of the eighteen. Blank lines are **excluded**, which is where this differs from the figure
//! already in `docs/COVERAGE.md`.
//!
//! | Measure | Value |
//! |---|---:|
//! | Module files | 18 |
//! | Non-blank lines | 1,195 |
//! | Shared non-blank lines | 936 |
//! | **Boilerplate, non-blank** | **78.3%** |
//! | Boilerplate, counting blank lines as shared | 82.3% |
//! | Mean non-blank lines per module | 66.4 |
//! | Distinguishing lines per module | 14 to 16 |
//!
//! ## Reproducing the recorded figure, and where the difference is
//!
//! `docs/COVERAGE.md` records 82.3% for this section with 14 to 16 distinguishing lines per module.
//! The distinguishing-line range reproduces exactly. The percentage does **not** reproduce under the
//! method above, and the discrepancy has a single cause: every module file has the same blank-line
//! skeleton, so all 270 blank lines are "shared" under any duplication rule. Counting them gives
//! 1,206 of 1,465 = 82.3%, which matches the recorded figure to the digit; excluding them gives 936
//! of 1,195 = 78.3%. Both are correct measurements of different questions, and the four-point gap
//! is entirely blank lines. `COVERAGE.md` already warns that its column was produced by different
//! methods and that small differences between rows should be distrusted; this is a worked instance
//! of why.
//!
//! ## Sensitivity
//!
//! **The threshold does nothing.** *k* = 50%, 80% and 100% of the eighteen files give 78.3% in all
//! three cases — not approximately, identically. Every shared line in this section is shared by
//! every module: there is no middle stratum of lines common to some files and not others. Section
//! 35 is the pure-skeleton regime, the same one `bioprism-bioevalx` reported for §26 and §07 and
//! the opposite of the mixed corpus where the threshold dominates. Nothing in the table above moves
//! if a reader prefers a different *k*.
//!
//! The six modules this crate covers contribute 87 distinguishing lines between them: 14 each for
//! 35.02, 35.03, 35.04 and 35.06, 15 for 35.07, and 16 for 35.13. Every one of those lines is a
//! title, a one-sentence purpose, a required component, or an operational metric. There is no
//! detailed design text in this section beyond those four kinds of line, which is why this crate's
//! prose spends more words on what it refuses than on what it implements.
//!
//! # The claim this crate must not route around
//!
//! `AGENTS.md`: **instance count is not benchmark count.** `bioprism-scale` made that
//! unrepresentable by giving `NominalCount` neither `Serialize` nor `Deserialize`, so a count
//! reaches a report only inside an `EffectiveSize` carrying the inflation ratio beside it. An
//! execution ledger is the obvious place for that guard to spring a leak, so [`executed`] extends
//! it instead: [`executed::ExecutedReport`] contains no bare instance count, and a hygiene test
//! asserts that no source file in this crate names the guarded type at all.
//!
//! # Bearing on the two arithmetic failures in the blueprint's worked example
//!
//! `bioprism-scale` found that the section's own accounting does not reconcile twice over — that
//! decisions times families is 420,000 rather than the stated 1,200,000, which needs a non-integer
//! 2.857 parameterizations per family that appears in no component list; and that reaching a
//! million valid instances from 1.2 million enumerable descendants implies a yield above 83% that
//! is never stated while yield is listed elsewhere as a quantity to be measured.
//!
//! Two of these six modules bear on both, and neither rescues the arithmetic.
//!
//! **The missing parameterization axis exists, and it does not buy classes.** 35.03 supplies a
//! concrete parameterization: one background panel, one latent, several stated effect sizes is
//! several instances of one mutation family over one parent decision. 35.04 supplies another: the
//! same intervention re-run under each alternative model. Both multiply the enumerable count
//! exactly as the missing axis would — and both produce instances sharing a parent, a family and an
//! oracle signature, which is precisely the triple `bioprism-scale`'s equivalence relation refuses
//! to split. So the axis that would reconcile 1.2 million is real and available, and using it
//! leaves the class ceiling exactly where `bioprism-scale` put it. `tests/reconciliation.rs`
//! asserts this on a corpus rather than in prose.
//!
//! **The unstated yield has named causes here, and this crate measures none of them.** Two of the
//! validity gates that make yield less than one are in this crate:
//! `semisynthetic::ConfoundReport::is_usable_as_an_oracle` rejects a panel whose batch label solves
//! it, and `twin::DiscrepancyProbe::is_usable_as_an_oracle` rejects a counterfactual whose sign
//! flips across the plausible model set. Neither has a rate, because a rate depends on the panel
//! and the model set a caller brings. That is the point: the blueprint assumes a yield above 83%
//! while its own components include gates whose pass rate nobody has ever measured.
//!
//! # What is not implemented
//!
//! In-memory and single-process throughout. **No distributed execution** — [`placement`] decides
//! whether one assignment is admissible and never runs anything, and its fence registry is a
//! `BTreeMap` where a deployment would have a durable compare-and-set. **No real storage**, no
//! network, no process spawning, no object store. **No instrumentation**: [`trajectory`] cannot
//! capture a notebook, a shell or an agent, and receives sessions a caller built. **No compositing
//! or simulation of anything biological**: [`semisynthetic`] inserts nothing and [`twin`]'s state
//! engine is caller-parameterised arithmetic that knows no rate.
//!
//! Nothing reads a clock, an environment variable, or a system random source. Timestamps are data
//! supplied by the producer; the one randomised procedure, `semisynthetic::assign_insertions`, uses
//! `bioprism_worldgen::rng::SplitMix64` from an explicit seed. Two hygiene tests enforce both
//! properties over the crate's own text, and each has a companion test proving the scanner fires on
//! a planted violation — a scanner that detects nothing is worse than no scanner.
//!
//! Of the eight release gates `bioprism_scale::QualityGate` enumerates, this crate can evaluate
//! three: rights permit the selected release mode, boundaries are meaningful to a named reviewer,
//! and a non-LLM oracle covers the defect. The other five are left unrecorded so that
//! `ReleaseAudit::finish` refuses.

pub mod authoring;
pub mod boundary;
pub mod error;
pub mod executed;
pub mod placement;
pub mod semisynthetic;
pub mod trajectory;
pub mod twin;

pub use authoring::{
    check_release_lineage, ArtifactForm, ArtifactRecord, AuthoredWorld, AuthoringFinding,
    AuthoringProperty, AuthoringReport, LatentClaim, LatentTruth, Limitation, LimitationsCard,
    ReconstructedDecision, ReleaseMode, RightsReview,
};
pub use boundary::{
    compile_cell_spans, Boundary, BoundaryAgreement, BoundaryKind, CellSpan, CompressionReport,
    KindAgreement, ReferenceBoundaries, Replayable,
};
pub use error::{
    AuthoringError, BoundaryError, CaptureError, InsertionError, PlacementError, TwinError,
};
pub use executed::ExecutedReport;
pub use placement::{
    place, AccessTier, Attestation, Commit, DuplicateReport, ExecutionLedger, Fence, FenceRegistry,
    Locale, Placement, TrustDomain, WorkRequest, WorkerProfile,
};
pub use semisynthetic::{
    assign_insertions, Background, BatchCell, ConfoundReport, Insertion, LatentRecovery, Panel,
    TransferClaim,
};
pub use trajectory::{
    CaptureSession, Completeness, Field, LeakageScan, RedactedSession, RedactionPolicy, Span,
    SpanKind,
};
pub use twin::{
    counterfactual, AssayReadout, DiscrepancyProbe, Intervention, MechanisticModel, Trajectory,
    TwinTruth,
};

/// This crate's own source text, for the hygiene tests that check rules no type can carry.
///
/// The tests that read this scan for hardcoded constants, for clock and random-source access, and
/// for blueprint ids outside this crate's scope. Each scanner has a companion test that plants a
/// violation and asserts the scanner sees it.
pub const SOURCES: [(&str, &str); 9] = [
    ("authoring.rs", include_str!("authoring.rs")),
    ("boundary.rs", include_str!("boundary.rs")),
    ("error.rs", include_str!("error.rs")),
    ("executed.rs", include_str!("executed.rs")),
    ("lib.rs", include_str!("lib.rs")),
    ("placement.rs", include_str!("placement.rs")),
    ("semisynthetic.rs", include_str!("semisynthetic.rs")),
    ("trajectory.rs", include_str!("trajectory.rs")),
    ("twin.rs", include_str!("twin.rs")),
];

/// The six blueprint modules this crate implements. Nothing else may be cited anywhere in it.
pub const COVERED_MODULES: [&str; 6] = ["35.02", "35.03", "35.04", "35.06", "35.07", "35.13"];
