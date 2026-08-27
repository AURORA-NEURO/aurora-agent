//! World authoring and the mutation families §27 leaves to nobody else.
//!
//! Implements blueprint section 27, *Benchmark Factory and Evaluation Hub* — specifically the seven
//! modules that no other crate in the workspace discharges: 27.01 (parent bioworld authoring),
//! 27.02 (observed worlds from real data), 27.03 (semi-synthetic worlds), 27.04 (mechanistic and
//! simulated worlds), 27.10 (assay-fault and pre-analytic mutations), 27.11 (specimen lineage and
//! identity mutations) and 27.14 (multimodal contradiction programs). [`coverage`] is the
//! machine-checked record of who owns the other fifteen.
//!
//! # The module this crate is built around
//!
//! **27.14, multimodal contradictions.** Imaging says one thing and pathology says another. That is
//! not noise to be averaged away; it is the situation a research platform exists to surface, and
//! the failure mode is a contradiction *silently reconciled* — a pipeline that picked a winner and
//! moved on.
//!
//! So [`contradiction`] makes a disagreement a first-class object: which modalities disagree, on
//! what quantity, over what scope, and what could account for it. There is no field naming the
//! correct modality and no function that returns one. Its resolution has **three** states, not two
//! — resolvable by evidence that exists, unresolvable with the missing evidence named, and *not yet
//! examined* — and the third is the one that gets folded into the second everywhere else. "We
//! cannot tell" and "we have not looked" are indistinguishable on a scoreboard and opposite in a
//! laboratory.
//!
//! # The authoring ladder and its honesty requirement
//!
//! An observed world, a semi-synthetic world and a mechanistic simulation are three different
//! epistemic objects, and a benchmark built from a simulation cannot make a claim about biology
//! that the simulation itself assumed. [`provenance`] makes that a type: a world declares the rungs
//! it stands on, transitively, and [`provenance::support`] refuses a claim that exceeds them. A
//! claim naming a quantity the construction *fixed* is refused first and separately, because a
//! circular claim is void rather than merely out of scope.
//!
//! # The two mutation families, and why they are not the neighbouring crates'
//!
//! [`preanalytic`] is 27.10. `crates/stress` degrades *measurement*; a pre-analytic fault changes
//! the specimen *before measurement exists*, and its postcondition is the opposite one — the
//! biological state must be byte-identical afterwards. That is the line between 27.10 and 27.09's
//! controlled semantic mutations, and it is executable here.
//!
//! [`lineage`] is 27.11. `crates/mutation` learned that deduplication must not be defeatable by
//! renaming, so its content digest excludes labels. The identity case is that lesson from the other
//! side: the same exclusion is exactly why a relabelled specimen is invisible to a content digest,
//! and must be caught by identity evidence or not at all.
//!
//! # What this crate is not
//!
//! No cohort data, no simulator, no imaging, no assay, no pathology, no genotyping, no execution
//! and no clock. Every type here is a *declaration about* a world and every function checks
//! declarations against each other. Nothing verifies that a declaration is true — that is expert
//! review, which is human work, and [`authoring::Reviewed`] records whether it happened rather than
//! pretending to do it.
//!
//! The clinical vocabulary is likewise minimal and deliberately so. Modality, quantity, lens and
//! assay-scope names are opaque strings; §27 fixes none of them, and the one place a marker
//! vocabulary exists in the workspace is `bioprism_onco::MolecularMarker`, which that crate
//! documents as a worked instantiation. [`preanalytic::FaultKind`]'s variants name ordinary
//! laboratory handling variables and are labelled illustrative at the point of use: each is
//! assigned to one of 27.10's own seven stages, and none asserts a magnitude, a threshold, or an
//! effect on any measurement.
//!
//! # Boilerplate in §27, and how it was measured
//!
//! §27's twenty-two module documents contain **1,190 non-blank lines**, of which **638 (53.6%)** are
//! byte-identical across every module. The template is total: the "Interfaces", "Observability",
//! "Implementation ladder" and "Release gates" sections in full, all ten section headings, and four
//! lines of front matter.
//!
//! One number with no method is not a measurement, so here are three definitions and both
//! sensitivities.
//!
//! | Definition | Boilerplate |
//! |---|---|
//! | non-blank lines shared by all 22 modules | 638 / 1,190 = **53.6%** |
//! | those lines restricted to runs of ≥3 consecutive shared lines | 484 / 1,190 = **40.7%** |
//! | characters on lines shared by all 22 modules | 33,572 / 54,406 = **61.7%** |
//!
//! **Threshold sensitivity: none.** The line-level figure is 53.6% whether the threshold is "shared
//! by all 22", "by at least 20", or "by at least 12". Sharing in §27 is bimodal — 539 distinct
//! lines appear in exactly one module, 28 appear in all twenty-two, and only six lines fall
//! anywhere in between — so no threshold in that range moves the number at all. This is why
//! `crates/hubapi`'s three-way spread and `crates/infra`'s nine-point spread do not reproduce here:
//! those corpora have partial sharing and §27 has essentially none.
//!
//! **Definition sensitivity: 21 points.** The three definitions above span 40.7% to 61.7% on the
//! same corpus. Character-weighting raises the figure because the shared lines are long prose
//! paragraphs and the unique lines are short list items; the contiguous-run restriction lowers it
//! because it excludes the ten shared *headings*, which are structure rather than filler. Quoting
//! any one of these without its definition would be quoting a choice.
//!
//! The differentiated remainder per module is 23–29 lines: a purpose sentence, six workflow steps,
//! five required artifacts, five validation items, one critical-design-decision paragraph and five
//! characteristic failures. **The failure lists carry nearly all of this crate's testable content**
//! — almost every refusal in [`error`] cites one — which is the same finding `crates/oncoworlds`
//! reported for section 30.
//!
//! # Where §27 names a construction it never specifies
//!
//! Recorded here rather than invented, and each is a caller-supplied value at the point of use:
//!
//! * **Gold and Silver parent tiers** (27.01 workflow step 7) — named, never defined.
//!   [`authoring::Tier::Silver`] carries the author's declared relaxations instead.
//! * **The QC panel and every fault signature** (27.10) — 27.10 requires a "QC signal" and a
//!   "detectability range" and names no signal and no threshold. Both are the caller's.
//! * **Cost and information gain** (27.14 validation "information-gain ordering") — no probability
//!   model exists anywhere in §27, so [`contradiction::next_actions`] ranks by the count of live
//!   hypotheses an action refutes and says in its own documentation that this is a set-cover
//!   surrogate rather than information gain.
//! * **The reference discordance distribution** (27.14 required artifact) — required, never
//!   populated. [`contradiction::expectedness`] refuses without one rather than defaulting.
//! * **The modality, quantity and lens vocabularies** (27.14) — 27.14 requires "modality-specific
//!   lenses" and lists no modalities.
//! * **"Expert plausibility" and "expert review"** (27.01, 27.03, 27.11 validation) — a human step
//!   with no stated criteria. [`authoring::Reviewed`] records the outcome in three states.
//! * **Detectability and realism bounds** (27.03 "validate detectability and realism", 27.10 "fault
//!   too obvious") — no bound is given anywhere. This crate reports the measurable part (a
//!   detectability floor against a caller's threshold) and does not judge realism.
//! * **The trajectory-to-decision-cell compilation** (27.06) — a whole module of construction with
//!   no specification of the unit boundary. [`coverage`] records it as unclaimed.
//!
//! # Example
//!
//! ```
//! use bioprism_worldfactory::provenance::{Claim, ClaimKind, Provenance, support};
//!
//! let simulated = Provenance::mechanistic(["tumour growth rate"]);
//!
//! // The simulator wrote the growth rate down, so the world cannot be evidence for it.
//! let circular = support(&simulated, Claim::new(ClaimKind::Biology, "tumour growth rate"));
//! assert!(circular.is_err());
//!
//! // Detecting an injected effect is exactly what a construction rung is good for.
//! let detection = support(
//!     &simulated,
//!     Claim::new(ClaimKind::DetectingInjectedStructure, "tumour growth rate"),
//! )?;
//! assert!(detection.caveat().contains("no empirical biological validity"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod authoring;
pub mod contradiction;
pub mod coverage;
pub mod error;
pub mod lineage;
pub mod mechanistic;
pub mod observed;
pub mod preanalytic;
pub mod provenance;
pub mod semisynthetic;
pub mod protocol_simulation_federated_control_plane;

pub use contradiction::{ContradictionProgram, ResolutionState, ValidatedProgram};
pub use error::WorldFactoryError;
pub use provenance::{Claim, ClaimKind, GroundedClaim, Provenance, Rung};
pub use protocol_simulation_federated_control_plane::{
    protocol_simulation_manifest, simulate_protocol, PeerProtocolSummary4,
    ProtocolDraft4, ProtocolScenario4, ProtocolSimulationArtifact8,
    ProtocolSimulationError, ProtocolSimulationReport8, ProtocolStage4,
    CONTRACT_VERSION as PROTOCOL_SIMULATION_FEDERATED_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as PROTOCOL_SIMULATION_FEDERATED_CONTROL_FEATURE_ID,
};
