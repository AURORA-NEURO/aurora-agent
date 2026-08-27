//! The remainder of §23: adapters, components, evaluation, credit, security, governance, humans.
//!
//! Four crates already own most of the Agent Interweave Fabric. `bioprism-weave` is the microkernel
//! (23.05, 23.07–23.10, 23.13, 23.16, 23.49) and a trusted computing base whose size is a design
//! constraint, so this crate builds *above* it and adds nothing to it. `bioprism-choreography` owns
//! session types and their analysis (23.06, 23.18, 23.19, 23.21, 23.22, 23.45).
//! `bioprism-weavelang` owns the language, its IR and its operational semantics (23.02, 23.03,
//! 23.34, 23.37, 23.38). `bioprism-fabric` owns the composition algebra and everything built on it
//! (23.01, 23.11, 23.14, 23.15, 23.17, 23.20, 23.41, 23.42, 23.44, 23.46, 23.48).
//!
//! This crate implements the nine modules none of them claimed:
//!
//! | module | here |
//! |---|---|
//! | 23.24 protocol adapters | [`adapter`] |
//! | 23.25 component runtime and sandboxed composition | [`component`] |
//! | 23.27 evaluation and microbenchmark generation | [`microbench`] |
//! | 23.28 orchestration learning and credit assignment | [`credit`] |
//! | 23.29 security threat model and trust boundaries | [`threat`] |
//! | 23.31 governance, versioning and conformance | [`conformance`] |
//! | 23.33 reference interweave workflows | [`workflow`] |
//! | 23.39 WeaveBench packs and microbenchmark taxonomy | [`packs`] |
//! | 23.47 human and organizational participants | [`human`] |
//!
//! # The classification, and where the line actually fell
//!
//! The test the other crates have used is whether a module's detailed design is *a set of
//! predicates over an artefact* or *a description of what people do*. All nine of these are
//! specification by that test, but `bioprism-bioevalx`'s refinement applies to six of them: the
//! split runs **inside** the module rather than between modules, so each is part implemented and
//! part named. Every module doc states its own line; in summary:
//!
//! - **23.24, 23.29, 23.39 and 23.47** are specification nearly throughout. What is left out of
//!   each is execution — no transport, no red team, no benchmark instances, no scheduler — rather
//!   than anything the blueprint states and this crate declines to decide.
//! - **23.25** splits at the runtime boundary. Component contracts, the capability-import rule and
//!   the six composition checks are predicates; the WebAssembly component model, the WIT parser and
//!   the seven execution providers are a runtime, and nothing here executes anything.
//! - **23.27 and 23.28** split at the data boundary. Mutation classes, oracle ordering, scale
//!   accounting, difference rewards and exact Shapley values are computable; procedural generation
//!   needs parents that do not exist, and every one of 23.28's six search methods is a learning
//!   algorithm this crate does not implement.
//! - **23.31** splits cleanly down the middle. The stability ladder, conformance dimensions,
//!   version pinning, namespacing and the compatibility promise are predicates. The Weave
//!   Enhancement Proposal process, the four governance bodies and the licensing policy are what
//!   people do, and they are named in prose in [`conformance`] and not modelled.
//! - **23.33** is the one where the split is uncomfortable. The six reference workflows are
//!   *content* — WeaveLang programs, fixtures, tutorials, diagrams — and none of it exists in this
//!   workspace. What is specification is the nine-item completeness requirement over each workflow
//!   and the one effect envelope 23.33 states, so [`workflow`] implements those and reports the
//!   honest consequence: all six workflows owe all nine deliverables, fifty-four in total, and the
//!   arithmetic is a test so the number moves only when an artefact lands.
//!
//! # Four modules skipped as prose, and this crate agrees
//!
//! `bioprism-fabric` did not implement 23.00 (overview and thesis), 23.35 (implementation roadmap),
//! 23.36 (open research program) or 23.40 (novelty boundary and ecosystem positioning), on the
//! grounds that none defines a type, an invariant or a decision procedure. **None of those four is
//! among this crate's nine**, so nothing here re-classifies them; the position is recorded because
//! agreeing explicitly is cheaper than leaving a reader to infer agreement from silence. Having
//! read all four while working through the section, this crate concurs: 23.35 schedules work,
//! 23.36 poses questions, 23.40 compares against other systems, and 23.00 motivates. Turning any of
//! them into code would mean inventing the semantics they deliberately do not fix.
//!
//! # What is deliberately absent
//!
//! No runtime, no transport, no network, no scheduler, no concurrency, no process, no sandbox, no
//! model call. Nothing here performs an effect of any kind.
//!
//! No clock. Every temporal question takes a tick or a deadline a caller supplies —
//! [`human::Approval::authorizes`] takes both the grant time and the current time as arguments,
//! which is what makes an expiry test reproducible. No randomness: [`microbench::select`] is a
//! deterministic ordering with an explicit tie-break, not a sample.
//!
//! No cryptography. 23.29's first control is "signed identities and artifact hashes" and
//! [`threat::Control::SignedIdentitiesAndArtifactHashes`] is a name this crate can require and
//! cannot verify. [`human::OrgIdentity`] carries an attestation identifier and its issuer and
//! checks neither.
//!
//! No floating point in any result. [`credit::Rational`] is an exact reduced fraction and
//! [`packs::Measurement`] is basis points, so a Shapley efficiency check and a scorecard comparison
//! give the same answer on every platform.
//!
//! # Places §23 names a mechanism and does not specify it
//!
//! Nine, across the nine modules. Each is resolved at the definition that needed it, each
//! resolution is marked as this crate's reading rather than the blueprint's, and each is listed
//! here so a reader checking this crate against §23 finds them named rather than discovers them.
//!
//! 1. **23.24's G0–G5 grade ladder and 23.01's six-rung adapter ladder range over the same
//!    adapters and are never related.** They are also not inter-derivable: [`adapter::surface_of`]
//!    bridges six pairs, five of 23.01's features have no grade counterpart and seven of 23.24's
//!    surfaces have no feature counterpart, so an adapter declared only in 23.01's vocabulary
//!    cannot be graded above [`adapter::Grade::G1`].
//! 2. **23.24 never says whether grades are cumulative.** [`adapter::Grade::requires`] reads them
//!    as cumulative, because six unrelated labels would make the table's ordering meaningless.
//! 3. **23.25 lists eight host services and 23.14 lists eighteen effect kinds, and nothing relates
//!    them** — which leaves 23.25's central sentence, that a component without a filesystem import
//!    cannot touch the filesystem, with nothing to evaluate.
//!    [`component::HostService::required_for`] is the mapping.
//! 4. **23.25 says every provider "declares isolation fidelity" and never says what fidelities
//!    exist or how they compare.** [`component::IsolationFidelity`] is a five-level ordering.
//! 5. **23.27 gives seven microbenchmark families and 23.39 gives twelve packs, sharing four names
//!    outright and never relating the two lists.** [`microbench::Family::packs`] is the mapping and
//!    [`microbench::PACKS_WITH_NO_FAMILY`] is its consequence: the families are not a cover, so
//!    three packs — negotiation and budgets, semantic interoperability, security and privacy — are
//!    unreachable from 23.27's generator.
//! 6. **23.29 lists eleven threats and fifteen controls in adjacent sections and never relates
//!    them.** [`threat::Threat::controls`] is the relation, and it is falsifiable: the tests assert
//!    that every threat has a control and every control answers a threat.
//! 7. **23.29's trust zones never say which zone may discharge which control.**
//!    [`threat::Control::minimum_zone`] says, which is what makes "trusting self-reported
//!    capability" a describable error rather than a warning.
//! 8. **23.31 gives six stability levels and no transition table.**
//!    [`conformance::Stability::may_transition_to`] is one, with the three constraints that shape
//!    it written down. Separately, 23.31 gives twelve conformance dimensions with no dependency
//!    relation; [`conformance::Dimension::prerequisites`] is deliberately weaker, reporting unmet
//!    prerequisites as findings rather than errors, because 23.31 never says a gapped claim is
//!    invalid.
//! 9. **23.47 gives a Decision Capsule with `reversibility: low` and never relates reversibility to
//!    how much evidence a human must be shown**, while `bioprism-section` supplies exactly a
//!    depth-of-evidence ladder. [`human::required_layer`] is the mapping.
//!
//! Two further gaps are worth recording as gaps rather than as resolutions, because this crate did
//! **not** fill them. 23.24 asks for trace correlation and 23.01's `SemanticFeature` cannot express
//! it; rather than add a feature to a sibling crate's vocabulary, [`adapter`] keeps the two
//! vocabularies apart and reports the ceiling. And 23.16's resource vocabulary has no
//! human-attention resource, which 23.47 needs; rather than add a variant to the microkernel,
//! [`human::Reviewer`] accounts attention as `WallClockMillis` and its module doc says plainly that
//! a thread's elapsed milliseconds and a person's review minutes are different quantities sharing a
//! unit here.
//!
//! # §23's boilerplate, measured independently
//!
//! `bioprism-fabric` reports 16.1% verbatim duplication as written and 11.8% with YAML front matter
//! stripped, over the 50 modules of `23_AGENT_INTERWEAVE_FABRIC/`. Re-running the same method
//! independently reproduces that table exactly: 6001 non-blank lines, 967 duplicated (16.1%), 107
//! of 5086 distinct strings shared (2.1%); front matter stripped, 5651 lines, 667 duplicated
//! (11.8%), 102 of 5031 distinct (2.0%).
//!
//! The **16.2%** figure that has also been reported is reproducible and is not a rounding
//! disagreement. It is what the same method returns when the distribution's own root `README.md`
//! and `AGENTS.md` are swept in alongside the fifty §23 modules: 6155 lines, 995 duplicated,
//! 16.2%. Those two files carry the same seven-line YAML header shape as every module, so adding
//! them adds duplicates faster than they add lines. **The stripped figure is 11.8% either way**,
//! which is exactly why the stripped number is the one that survived four independent measurements
//! and the as-written number did not.
//!
//! The figure that matters is neither: only **2.1%** of *distinct* line-strings appear in more than
//! one module of §23, stable at 1.6–2.1% under every preprocessing tried. §23's modules share
//! formatting, not content. Across this crate's own nine, the sharing is lower still — between 1.5%
//! and 7.9% of each module's non-blank lines appear in any other of the nine, and what repeats is
//! `## Purpose`, ```` ```text ```` and closing fences. Nothing in these nine modules could have
//! been inferred from a sibling, and this crate follows each of them line by line.

pub mod adapter;
pub mod component;
pub mod conformance;
pub mod credit;
pub mod human;
pub mod interweave_contract_frontier_federated_control_plane;
pub mod microbench;
pub mod packs;
pub mod threat;
pub mod workflow;
pub mod workflow_execution;

/// The blueprint section this crate completes.
pub const BLUEPRINT_SECTION: &str = "23_AGENT_INTERWEAVE_FABRIC";

/// The nine module ids this crate implements, for a reader checking coverage against capability.
///
/// Every id here is backed by a module above. No id appears anywhere in this crate that is not in
/// this list or already owned by a sibling, which is the discipline that keeps a coverage
/// percentage from moving without capability moving with it.
///
/// That sentence used to be an assurance. `tests/citations.rs` is the mechanism behind it: it
/// reproduces `tools/coverage.sh`'s token rule over this crate's own tree, re-derives ownership by
/// reading every other file the script reads, and fails on any id this crate would be the only
/// place for. It matters here more than most — this crate names forty-one distinct §23 ids for nine
/// implemented modules, the widest gap between citation and capability in the workspace.
pub const IMPLEMENTED_MODULES: [&str; 9] = [
    "23.24", "23.25", "23.27", "23.28", "23.29", "23.31", "23.33", "23.39", "23.47",
];
