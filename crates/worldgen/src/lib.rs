//! Synthetic structural benchmark families.
//!
//! Implements blueprint 43.39. The purpose is falsifiability: `docs/FINDINGS.md` records that the
//! shipped reference world cannot separate FIBER from a tuned graph walk or a lexical retriever,
//! because all three select the identical eleven facts. A claim measured only on that world is
//! not a claim about the method.
//!
//! This crate makes the structure a parameter, so the question becomes empirical — *under which
//! structures does each strategy succeed?* — rather than rhetorical.
//!
//! # What a spec can now vary
//!
//! Three knobs answer "is the benchmark separable": [`DistractorAttachment`], [`TagStyle`] and
//! `relay_depth`. Seven more answer "what can the compiler be made to do": the release schedule
//! ([`EventSpec`]), the protected set ([`WorldSpec::protected_variables`]), the decision cut
//! ([`WorldSpec::decision_time`]), the decisive structure ([`Skeleton`]), competing terminals
//! ([`WorldSpec::hypotheses`]), declared absences ([`DeclaredAbsence`]) and policy
//! ([`PolicySpec`]). The second group exists because `crates/bioworlds` had to build four worlds
//! by hand for want of it and wrote down, field by field, what was missing.
//!
//! All seven default to the behaviour that preceded them. `WorldSpec::reference_like` and
//! `WorldSpec::discriminating` still generate the documents committed under `fixtures/generated/`,
//! byte for byte.
//!
//! # Not implemented
//!
//! * **Fact values are not parameterised.** A spec chooses which defects are injected
//!   ([`LeakageMechanism`]) and the generator computes the values; there is no field that sets a
//!   fact's value directly, so a world needing particular data still has to be built by hand.
//! * **[`Skeleton`] has no `Custom` variant.** `bioprism-fiber`'s v0.1 oracle only understands
//!   split integrity, so a world with a different decision would compile to `Valid` with an empty
//!   witness list and read as clean rather than as unjudged.
//! * **The exclusion between hypotheses is declared, not interpreted.** It is a factor of kind
//!   `exclusion_rule`; no pass reads it, and `bioprism-fiber` still has no path that constructs an
//!   abstaining verdict.
//! * **A declared absence carries a status string, not a typed observation status.**
//!   `fiber-world/0.1` has no such type.
//! * **Policy covers read access only.** Role, purpose, consent and residency are registered scope
//!   dimensions that no generated world binds and no pass reads.
//! * **Two of the six mutation families 38.01 names have no knob**: prevalence shift and assay
//!   uncertainty are not generable here.

pub mod generate;
pub mod rng;
pub mod spec;

pub use generate::{generate, Generated};
pub use spec::{
    CheckSpec, DeclaredAbsence, DistractorAttachment, EventSpec, LeakageMechanism, PolicySpec,
    Skeleton, TagStyle, WorldSpec, ASSAY_TAG, CENTRAL_LAB_CONFIRMATION, HYPOTHESIS_EXCLUSION,
    LOCAL_LAB_VALUE, PROTECTED_TAG, PROTECTED_VOCABULARY, REFERENCE_DECISION_TIME,
};
