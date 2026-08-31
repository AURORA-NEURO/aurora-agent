//! The Agent Interweave Fabric: the layer that sits above the microkernel.
//!
//! Implements the parts of blueprint §23 with checkable semantics that no other crate in this
//! workspace owns: the composition algebra and its substitution laws (23.41), effect types and
//! agent-to-agent information flow (23.14), capability molecules and virtual agents (23.11), the
//! goal-to-molecule compiler (23.42), the distributed epistemic CRDT and scoped common ground
//! (23.44), attested capability and contextual reputation (23.46), semantic lifecycle and
//! compaction (23.48), the blackboard and replicated state (23.17), negotiation and service
//! contracts (23.15), dynamic topology (23.20), and the layered protocol stack (23.01).
//!
//! # What this crate is not allowed to do
//!
//! Three crates already own pieces of §23, and this one builds *above* them rather than beside
//! them. Stated concretely, because "we reuse the kernel" is easy to write and easy not to mean:
//!
//! - **`bioprism-weave`** is the microkernel and a trusted computing base whose size is a design
//!   constraint. It owns communicative acts (23.05), the commitment and epistemic ledgers (23.07,
//!   23.08), Context Capsules (23.09), continuations (23.10), authority with transitive revocation
//!   (23.13), affine budgets (23.16) and the cognitive ABI (23.49). This crate *calls* it —
//!   [`algebra::check_affine_non_duplication`] and [`negotiate::ContractNet`] run a real
//!   `weave::Budget` rather than reimplementing non-duplication, [`contract::AgentContract`]'s
//!   authority component is `weave::Capability`, and [`flow::Principal::capsule_clearance`] renders
//!   into the kernel's clearance vocabulary and reports what that rendering loses.
//! - **`bioprism-choreography`** owns session types (23.06), model checking (23.45), adjudication
//!   (23.18), quorum (23.19), sagas (23.21) and fault tolerance (23.22). [`algebra::fuse`] takes a
//!   `choreography::WellFormedGlobal` — not a `GlobalType` — so well-formedness is decided once, in
//!   the crate that owns it. [`synth::reject`] takes a deadlock finding as an argument rather than
//!   deriving one.
//! - **`bioprism-weavelang`** owns the language (23.02), the IR and pipeline (23.03), the
//!   operational semantics (23.34) and the syntax and schema references (23.37, 23.38). It already
//!   enforces `effects(lower(P)) ⊆ declared_effects(P)` over a *program*;
//!   [`algebra::substitutable`] states the same direction over a *participant*, which is a
//!   different quantifier and not a second implementation.
//!
//! Two more crates set rules this one inherits rather than relitigates. `bioprism-policy`
//! implements 43.33's information-flow fibers over data products, so [`flow`] deliberately takes
//! only the agent-to-agent case. `bioprism-atlas` established that unmeasured and measured-poor are
//! categorically distinct, and [`reputation::ContextLookup::Unmeasured`] and
//! [`contract::AssuranceProfile::success_lower_bound_bp`] carry no number at all rather than a
//! zero. `bioprism-ledger` established that compaction must declare a retained window and that
//! verification afterwards is weaker; [`lifecycle::VerificationStrength`] has no variant meaning
//! "as strong as before".
//!
//! # Five claims the types enforce
//!
//! **Substitution is effects-subset and guarantees-superset, and nine clauses wide.**
//! [`algebra::substitutable`] returns [`algebra::SubstitutionVerdict`] with every failing clause
//! named. 23.41's own counterexample — a cheaper model with identical JSON output that is less
//! calibrated — fails on a clause that has nothing to do with effects.
//!
//! **An unlabelled value is not a public one.** [`flow::Labelling::Unlabelled`] refuses every flow,
//! there is no constructor turning it into [`flow::Sensitivity::Public`], and [`flow::taint_join`]
//! over an empty input list yields unlabelled rather than public. The same shape appears in
//! [`effect::Scope::Undeclared`], where containment is [`effect::Containment::Undecided`] rather
//! than true or false.
//!
//! **A reputation that spans contexts is unrepresentable.** [`reputation::Reputation`] has no
//! `overall()`, no `Ord`, and no iterator yielding a score without its
//! [`reputation::ReputationContext`]. Lookup is exact-match with no widening, so an agent measured
//! on literature retrieval under read-only access returns *unmeasured* for code modification with
//! production credentials.
//!
//! **Common ground is not the intersection of two belief sets.** [`ground::common_ground`] computes
//! three levels and the intersection is the weakest of them. Two agents each knowing `X` without
//! knowing that the other knows is [`ground::GroundLevel::Distributed`], and it does not support
//! coordination.
//!
//! **A compaction that would orphan a live conclusion is refused, naming the conclusion.**
//! [`lifecycle::CompactionPlan::check`] returns
//! [`lifecycle::CompactionRefusal::WouldOrphanConclusion`] with the premise and every conclusion
//! that rested on it.
//!
//! # What is deliberately absent
//!
//! No runtime. No network, no transport, no scheduler, no concurrency, no real participants, no
//! tool invocation, no model call. No clock: every temporal question takes an explicit
//! [`reputation::LogicalTime`] and a caller supplies it, which is what makes an expiry test
//! reproducible. No randomness. No cryptography: content hashes come from `bioprism_ids` and there
//! are no signatures anywhere, so 23.46's attestations are structured records rather than verified
//! ones and 23.44's "resistance to forged events" is explicitly out of reach — validation here is
//! causal, not cryptographic.
//!
//! Nothing here performs an effect. [`effect::Effect`] is a declaration that something *may*
//! happen; no filesystem is touched, no process is executed and no message is sent. The whole crate
//! is a calculus over declarations, and its value is that a declaration can be checked before
//! anything runs.
//!
//! # Modules of §23 skipped as prose
//!
//! Four modules of §23 are argument rather than specification and are not implemented, here or
//! anywhere: **23.00** (the overview and thesis) motivates the section, **23.35** (implementation
//! roadmap and vertical slices) schedules work, **23.36** (open research program) poses questions,
//! and **23.40** (novelty boundary and ecosystem positioning) compares against other systems. None
//! defines a type, an invariant or a decision procedure. Turning any of them into code would mean
//! inventing the semantics they deliberately do not fix.
//!
//! # Places §23 names a law and does not state it
//!
//! Recorded here because a reader checking this crate against the blueprint will find these gaps
//! and should know they were found rather than filled silently. Each is resolved in the module that
//! needed it, and each resolution is this crate's reading:
//!
//! 1. **23.14 defines irreversibility classes E0–E4 and the eighteen-effect taxonomy and never
//!    relates them.** A policy saying "class ≥ E3 requires quorum" has nothing to evaluate.
//!    [`effect::EffectKind::irreversibility_floor`] is the mapping.
//! 2. **23.14 never says what an effect with no `<resource>` parameter means.** Its own examples use
//!    both forms. Resolved as [`effect::Scope::Undeclared`], which is neither "all" nor "none".
//! 3. **23.44 gives `common-ground(thread, role-set, purpose, clearance, time)` and never says what
//!    it computes.** Resolved as [`ground::GroundLevel`]'s three levels, with the caveat about
//!    unbounded common knowledge stated in that module rather than assumed away.
//! 4. **23.44 lists eleven derived epistemic states and says they are "derived from event structure
//!    and policy" without giving the policy.** Resolved as [`ground::interpret`]'s ordered check
//!    list, written as an ordering so a disagreement is about one ordering.
//! 5. **23.46 lists seven revocation grounds and three thread responses and never relates them.**
//!    Resolved in [`reputation::RevocationEvent::recommended_response`].
//! 6. **23.48 lists eleven object states, says "transitions are typed and auditable", and gives no
//!    transition table.** Resolved in [`lifecycle::ObjectState::may_transition_to`], with the three
//!    constraints that shape it stated.
//! 7. **23.41's identity law names four things an identity must not change — provenance, budgets,
//!    time, security labels — and time is not checkable without a clock.**
//!    [`algebra::identity_law_unchecked_conditions`] says so rather than passing silently.
//! 8. **23.41's substitution relation has seven clauses and its own worked counterexample fails on
//!    a clause not in the list.** Two clauses are added and marked as additions in
//!    [`algebra::substitutable`].
//! 9. **23.11's version rule says a patch is permitted "only when PRISM regression gates pass"**,
//!    which is not a property of two molecule values. [`molecule::required_bump`] answers the
//!    contract half and says which half it answered.
//!
//! # §23's boilerplate, measured
//!
//! §23 is the least duplicated section of this blueprint, and the measurement is worth stating
//! precisely because the same metric described the same way has been reported as both 16.2% and
//! 10.4% by different readers. Both are right; they differ on one preprocessing choice.
//!
//! Method: over all 50 modules of `23_AGENT_INTERWEAVE_FABRIC/`, take non-blank lines, right-trim
//! them, and count a line as duplicated when the identical string occurs in at least one other
//! module of the section.
//!
//! | preprocessing | non-blank lines | verbatim duplicated | distinct strings shared |
//! |---|---|---|---|
//! | as written | 6001 | 967 (**16.1%**) | 107 / 5086 (2.1%) |
//! | YAML front matter stripped | 5651 | 667 (**11.8%**) | 102 / 5031 (2.0%) |
//! | front matter stripped, lines ≥ 4 chars | 5400 | 431 (8.0%) | 98 / 5021 (2.0%) |
//! | front matter stripped, lines ≥ 12 chars | 4847 | 176 (3.6%) | 75 / 4732 (1.6%) |
//!
//! The gap is the seven-line YAML header every module carries, of which five lines are
//! byte-identical across all fifty — 250 duplicated lines, 4.3 percentage points, entirely
//! mechanical. Counting it gives ~16%; excluding it gives ~12%, which is the same neighbourhood as
//! the 10.7% `bioprism-weavelang` reports over its own slightly different line filter. The
//! sensitivity is monotone and steep: raising the minimum line length from 0 to 12 characters takes
//! the figure from 16.1% to 3.6%, because most of what genuinely repeats is short — `- cost;`,
//! `## Purpose`, ```` ```text ````, closing fences.
//!
//! The number that does not move is the last column. Only **2.1%** of *distinct* line-strings
//! appear in more than one module, and it stays at 1.6–2.1% under every preprocessing above. That
//! is the honest headline: §23's modules share formatting, not content. Compare §42 at 93.6%. This
//! crate therefore follows its source closely, and the nine gaps listed above are the complete list
//! of places where following it required a decision it did not make.

pub mod algebra;
pub mod blackboard;
pub mod contract;
pub mod effect;
pub mod experiment_design_interoperability_gateway;
pub mod experiment_design_contract_model;
pub mod flow;
pub mod ground;
pub mod lifecycle;
pub mod molecule;
pub mod negotiate;
pub mod reputation;
pub mod stack;
pub mod synth;
pub mod topology;

pub use experiment_design_interoperability_gateway::{
    negotiate_experiment_design, negotiate_experiment_design_json,
    validate_experiment_design_json, experiment_design_interoperability_manifest,
    DesignAssignment8, DesignCapability4, ExecutableExperimentDesign8,
    ExperimentDesignGatewayError, ExperimentDesignRequest4, ExperimentObjective4,
    CONTENT_TYPE as EXPERIMENT_DESIGN_GATEWAY_CONTENT_TYPE,
    CONTRACT_VERSION as EXPERIMENT_DESIGN_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as EXPERIMENT_DESIGN_GATEWAY_FEATURE_ID,
    INPUT_SCHEMA as EXPERIMENT_DESIGN_GATEWAY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EXPERIMENT_DESIGN_GATEWAY_OUTPUT_SCHEMA,
};
pub use experiment_design_contract_model::{
    negotiate_experiment_design_contract, experiment_design_contract_manifest,
    DesignContractCandidate4, DesignContractError, ExecutableExperimentDesign2,
    ExecutableExperimentDesignArtifact2, FabricExperimentDesignContractRequest4,
    CONTRACT_VERSION as EXPERIMENT_DESIGN_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as EXPERIMENT_DESIGN_CONTRACT_MODEL_FEATURE_ID,
    INPUT_SCHEMA as EXPERIMENT_DESIGN_CONTRACT_MODEL_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EXPERIMENT_DESIGN_CONTRACT_MODEL_OUTPUT_SCHEMA,
};

pub use algebra::{
    check_affine_non_duplication, check_authority_attenuation, check_commitment_conservation,
    check_epistemic_monotonicity, check_identity, check_parallel_commutativity,
    check_sequential_associativity, compare, substitutable, substitute, Composition,
    CompositionError, EquivalenceDimension, EquivalenceReport, Law, LawOutcome, LawReport,
    LeasePlan, Operator, ParallelJustification, SubstitutionContext, SubstitutionObjection,
    LabelChange, SubstitutionVerdict, TerminalPath, Violation,
};
pub use contract::{
    identity, AgentContract, AssuranceProfile, ComponentId, DeclaredCommitment, EpistemicContract,
    FailureContract, InterfaceType, ResourceEnvelope, UncertaintySemantics,
};
pub use effect::{
    ComposedPolicy, Containment, Effect, EffectKind, EffectPolicy, EffectSet, Gate, Inclusion,
    Irreversibility, ResourcePattern, Scope,
};
pub use flow::{
    declassify, project_for, taint_join, Declassifier, DisclosureMatrix, FlowDecision, FlowLabel,
    FlowRefusal, Labelling, Principal, Projection, PurposeRestriction, Residual, Sensitivity,
};
pub use ground::{
    common_ground, conflicts, interpret, CommonGround, Conflict, DerivedState, EpistemicOp,
    GroundLevel, OpKind, Proposition, Replica,
};
pub use lifecycle::{
    claim_global_deletion, close_thread, resume_permitted, ClosureOutcome, CompactionCertificate,
    CompactionPlan, CompactionRefusal, DerivationGraph, LifecycleGraph, LifecycleObject,
    ObjectState, VerificationStrength,
};
pub use molecule::{Molecule, MoleculeCard, MoleculeError, Version};
pub use negotiate::{discount, ContractNet, DiscountedEstimate, Offer, Terms};
pub use reputation::{
    bind, independent_subjects, Attestation, BindingDecision, CapabilityAssertion, CapabilityCard,
    ContextLookup, ContextualScore, EvidenceLayer, IdentityLayers, LogicalTime, Reputation,
    ReputationContext,
};
pub use stack::{fail_closed, negotiate_session, Adapter, Compatibility, SemanticFeature, STACK};
pub use synth::{evaluate, reject, synthesize, Candidacy, Candidate, Goal, RejectionReason, Role};
pub use topology::{Controller, DwellPolicy, Member, Pattern, Topology, TopologyAction};

/// The blueprint section this crate implements.
pub const BLUEPRINT_SECTION: &str = "23_AGENT_INTERWEAVE_FABRIC";
