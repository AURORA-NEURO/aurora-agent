//! WeaveLang, WeaveIR, and the compiler between them.
//!
//! Implements blueprint 23.02 (the language), 23.03 (the IR and compiler pipeline), 23.34 (the
//! operational semantics and its invariants), 23.37 (the syntax reference) and 23.38 (the canonical
//! IR schema and its worked event examples).
//!
//! A WeaveLang program describes "the observable contract of collaboration" (23.02) and compiles to
//! WeaveIR, whose events are the typed communicative acts `bioprism-weave` already understands.
//! That direction is the whole architecture: this crate compiles *to* the kernel and never extends
//! it. Acts (23.05), the commitment and epistemic ledgers (23.07, 23.08), capsules (23.09),
//! continuations (23.10), authority (23.13), budgets (23.16) and the ABI (23.49) all belong to
//! `bioprism-weave`, which is a trusted computing base whose size is a design constraint. Where
//! this crate needs one of them it uses the kernel's type — `ActKind` names the acts, `Budget`
//! enforces the ceiling at compile time, `Fidelity` is what a resume grade maps onto — rather than
//! defining a second one that would eventually disagree.
//!
//! # The pipeline
//!
//! ```text
//! source ──lexer──▶ tokens ──parser──▶ AST ──lower──▶ WeaveIR ──semantics──▶ event trace + digest
//!                                       │                │
//!                                    contract        canonical bytes
//!                                  (type system)     (bioprism-ids)
//! ```
//!
//! [`compile::compile`] runs the first three arrows. [`contract::check`] is the cognitive type
//! system of 23.04, run against a roster of participant contracts a caller supplies, because 23.02
//! binds participants at run time and the same program is well-typed against one roster and not
//! another. [`semantics::Machine`] is 23.34's `Σ --event--> Σ'` with its eight safety properties as
//! executable checks. [`printer::print`] goes back the other way, AST to canonical source, which is
//! what makes `parse(print(parse(s))) == parse(s)` testable: a construct the parser silently drops
//! cannot be printed, so the round trip catches it.
//!
//! # Three properties this crate is built around
//!
//! **Effects are preserved by compilation.** `effects(lower(P)) ⊆ declared_effects(P) ⊆
//! allow(policy)`, checked by recomputing the left side from the emitted IR after every lowering. A
//! lowering that introduced an effect the source never declared is a compiler bug and is reported
//! as [`lower::LowerError::EffectIntroducedByLowering`] rather than discovered at run time.
//!
//! **Budgets are affine.** Branch leases are drawn from the policy ceiling by *running*
//! `bioprism_weave::Budget`, so there is one implementation of the non-duplication rule rather than
//! a compile-time copy that could drift. [`contract::Lease`] is likewise not `Clone`, and
//! [`semantics::Machine`] is not `Clone` because it owns a `Budget`.
//!
//! **Runs are deterministic.** The same program over the same inputs produces the same event
//! sequence and the same digest. There is no clock, no randomness, and no scheduler anywhere in
//! this crate; canonical bytes come from `bioprism_ids::to_canonical_bytes` so that the workspace
//! has exactly one canonical encoder.
//!
//! # What is not implemented
//!
//! Stated rather than implied, because a missing capability that is named is a limitation and one
//! that is implied to exist is a lie.
//!
//! Of 23.03's eleven compiler phases, this crate runs 1 to 4 and part of 9. It does **not**:
//!
//! - resolve schemas (phase 2). JSON Schema, Protobuf, WIT and Weave type packages are recorded as
//!   opaque strings and never loaded, so payload compatibility is unchecked.
//! - check refinements (phase 3, and 23.04's `where provenance.complete and
//!   source.independent_count >= 2`). Refinement syntax is not even parsed.
//! - propagate information-flow labels through values (phase 5). Clearance is checked participant
//!   to participant and thread-wide, not per value, and there are no declassification gates.
//! - project a global choreography into local role automata, or detect deadlocks (phase 6). The
//!   declared choreography is preserved verbatim in the IR and analysed no further; the one piece
//!   of phase 6 present is unreachable-state detection in [`semantics::LivenessReport`].
//! - estimate loop bounds or validate TTLs (phase 7). `repeat until` bounds are carried as guard
//!   strings and never evaluated, so termination is not proven and is not claimed.
//! - match roles to participants, plan adapters, or record semantic losses (phase 8).
//! - generate any target (phase 11): no bytecode, no Python or TypeScript stubs, no A2A cards, no
//!   MCP bindings, no WIT components, no OpenTelemetry.
//!
//! There is also no runtime scheduler, no concurrency, no network, no distributed execution, no
//! tool invocation, no model call, no signing, and no optimiser. WeaveIR here is one semantic level,
//! not 23.03's four-level lowering chain. `par` interleaves in source order and `race` explores its
//! branches in declaration order; both are recorded as what they are.
//!
//! Three of 23.02's constructs — `molecule`, `thread` and `shared` — are refused by the parser
//! rather than guessed at, because 23.37 gives them no grammar and inventing one would produce a
//! compiler that accepts programs no other implementation would.
//!
//! # Where 23.37 and 23.38 disagree with each other, and with themselves
//!
//! Recorded here because a reader comparing this crate against the blueprint will hit all of them.
//!
//! 1. **Field case.** 23.03's IR document is `snake_case` (`weave_ir_version`, `state_graph`,
//!    `actor_role`); 23.38's event envelope is `camelCase` (`weaveVersion`, `payloadRef`). Both are
//!    hashed, so both are kept exactly as written, in one crate, deliberately.
//! 2. **`//` is ambiguous inside 23.37.** It specifies `//` line comments and also writes
//!    `prism://capability/challenge`. Taken literally, the role declaration in its own example
//!    never closes. Resolved in [`lexer`]: `//` after a `:` is a scheme separator.
//! 3. **23.38 asks for what determinism forbids.** `eventId` is `urn:uuid:...` (needs randomness),
//!    `time` is a wall clock (needs a clock), and `signature` sits inside the envelope its own
//!    canonicalisation rules say must exclude signatures. Event identifiers are content-derived,
//!    `time` is an `Option` a caller may supply and this crate never fills, and there is no
//!    signature field at all.
//! 4. **`R2` is unexplained.** 23.38's continuation has `"fidelity": "R2"` and no grade semantics
//!    anywhere in §23. The mapping onto the kernel's three-state `Fidelity` is
//!    [`ir::ResumeGrade::to_fidelity`] and is this crate's reading, written down so a disagreement
//!    is visible as a disagreement about that table.
//! 5. **`budget money <= usd(5)` cannot be enforced.** 23.37's policy example declares it; 23.16's
//!    resource vocabulary, which the kernel accounts, has no money. It is carried into the IR as an
//!    [`ir::UnenforceableBudget`] with its reason rather than dropped or rejected.
//! 6. **23.37's reserved-keyword list is incomplete against its own examples.** `using`, `await`,
//!    `match`, `choose`, `record`, `variant`, `deliver`, `satisfy`, `compensate`, `resolution`,
//!    `current` and a dozen more appear in keyword position and are absent from the table. They are
//!    reserved here; [`lexer::is_reserved`] lists which came from where.
//! 7. **23.37's `weave run` binds roles nothing declares.** `role Lead` and `role Reviewer` are
//!    used and never defined. [`reference::SYNTAX_REFERENCE`] keeps that as it stands, so it parses
//!    and does not resolve; [`reference::COMPLETE_PROGRAM`] is the version that compiles.
//! 8. **23.02 and 23.37 are two dialects.** 23.02 writes `effects allow [...]` where 23.37 writes
//!    `allow effects [...]`, puts a `policy { }` block inside the `weave` body, and adds
//!    `fork N from checkpoint c`, `join c using r { require ... }` and `assurance`. Transposed
//!    keywords are accepted both ways; the rest is not implemented.
//!
//! # A note on §23 as a source
//!
//! Twelve sections of this blueprint measured 52-94% identical scaffolding. §23 does not: 10.7% of
//! its non-blank lines appear in any other module of the section. 23.38 is the least duplicated
//! module in it at 4.7% and 23.37 is close behind at 7.7%. They carry real content and this crate
//! follows them closely; the divergences above are the complete list.

pub mod ast;
pub mod compile;
pub mod contract;
pub mod diagnostic;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
pub mod printer;
pub mod reference;
pub mod release_assurance;
pub mod limitation_closure_control_plane;
pub mod semantics;

pub use compile::{compile, CompileError};
pub use contract::{check, CognitiveType, ParticipantContract, TypeError};
pub use diagnostic::{render, Diagnostic, Span};
pub use ir::{WeaveEvent, WeaveIr, WEAVE_EVENT_VERSION, WEAVE_IR_VERSION};
pub use lower::{lower_program, LowerError};
pub use parser::{parse, ParseError};
pub use printer::print;
pub use release_assurance::{
    assure_weavelang_release, ReleaseAssuranceDisposition, ReleaseAssuranceError,
    WeaveLangReleaseAssuranceReceipt, WeaveLangReleaseAssuranceRequest,
    CONTRACT_VERSION as WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID,
};
pub use limitation_closure_control_plane::{
    assure_weavelang_limitation_closure, weavelang_limitation_closure_manifest,
    ClosureDisposition, LimitationClosureError, PeerClosureSummary, WeavelangClosureReceipt,
    WeavelangClosureRequest, WeavelangLimitationCase,
    CONTRACT_VERSION as WEAVELANG_LIMITATION_CLOSURE_CONTRACT_VERSION,
    FEATURE_ID as WEAVELANG_LIMITATION_CLOSURE_FEATURE_ID,
};
pub use semantics::{ExecutionMode, Invariant, LivenessReport, Machine, Trace};
