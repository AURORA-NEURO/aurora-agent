//! The FIBER epistemic calculus, minus the parts other crates already own.
//!
//! Blueprint §43 is the thesis section of the distribution, and most of it is already implemented:
//! `bioprism-fiber` owns the compiler pipeline, `bioprism-section` owns what it emits,
//! `bioprism-ids` owns canonical bytes, `bioprism-influence` owns the numeric bounds. This crate
//! takes the modules nothing else had claimed, and it takes them in the order of how much they
//! were blocking.
//!
//! ## The gap this crate was built to close
//!
//! `bioprism-examples` maintains a catalogue of the platform's own claims and marks the ones no
//! slice can exercise. Two of them named the same obstacle:
//!
//! > `decision_equivalence_quotient` — *fiber-query/0.1 carries neither permitted_actions nor
//! > decision_loss*
//! >
//! > `rate_distortion_optimisation` — *the same missing decision_loss field: there is no loss to
//! > trade distortion against*
//!
//! Every decision-theoretic module of §43 is downstream of that one absence. [`ratedistortion`]
//! and [`voi`] therefore take the loss from the caller and build the calculus on it, and [`gap`]
//! states in field-level detail what `fiber-query/0.1` would have to carry for the compiler to
//! reach the same result on its own. Along the way [`gap`] turned up something worse than the
//! missing field: the v0.1 parser accepts a `decision_loss` key, drops it, and goes on reporting
//! it as missing, so a caller who supplies it is told nothing.
//!
//! ## What is checked rather than asserted
//!
//! Greedy selection under a submodular objective has a `1 − 1/e` guarantee. The guarantee is worth
//! nothing if the objective is not actually submodular, and "information gain is submodular" is
//! stated far more often than it is true. So [`submodularity`] checks the condition by exhaustion
//! over every subset of small instances, [`optimal`] computes the true optimum by brute force, and
//! [`mod@greedy`] refuses to report an approximation factor whose precondition [`theorem`] has not
//! seen a passing check for. There is no path from a caller to a `1 − 1/e` without a
//! [`submodularity::SubmodularityReport`] having been run and passed.
//!
//! The measurement, on the seeds the suite runs:
//!
//! | Family | Instances | Passed the check | Worst greedy/optimal ratio | Below `1 − 1/e` |
//! |---|---|---|---|---|
//! | weighted coverage | 40 | 40 | 0.909 | 0 |
//! | decision-regret reduction | 61 | 17 | 0.000 | 4 |
//!
//! Two things there are not what a summary would predict. The worst coverage ratio is 0.909, not
//! 0.632 — the bound is tight only on adversarial constructions, so it is a floor nobody came near
//! rather than a prediction. And **17 of 60 random decision instances genuinely are monotone
//! submodular**, which is why the condition is checked per instance and not decided per objective.
//! [`optimal`] has the full table and the caveats, including that 43 of the 61 regret instances
//! are degenerate and make the mean meaningless.
//!
//! ## Module map
//!
//! | Module | Blueprint | What it is |
//! |---|---|---|
//! | [`decision`] [`evidence`] | 43.50, 43.02 | actions, loss, beliefs, observed evidence, evidence actions |
//! | [`ratedistortion`] | 43.50, 43.12 | decision distortion, the frontier, identification, abstention |
//! | [`voi`] | 43.50 | value of information, bundles, complementarity |
//! | [`gap`] | 43.50 | the wire fields `fiber-query/0.1` lacks, and one it silently drops |
//! | [`objective`] [`submodularity`] [`mod@greedy`] [`optimal`] | 43.14 | set functions, the exhaustive check, greedy, the measured ratio |
//! | [`separator`] | 43.29 | separators, typed messages, exactness on trees, leak refusal |
//! | [`library`] | 43.31 | biological scope dimensions and factor templates, and where they disagree with `bioprism-scope` |
//! | [`lens`] | 43.49 | query optics, the laws, and which ones each optic actually satisfies |
//! | [`continuation`] | 43.30 | rebase classified in decision terms, over `bioprism-weave`'s capsule |
//! | [`patterns`] | 43.32 | OncoWorld research query patterns and their non-clinical boundary |
//! | [`theorem`] | 43.47 | the runtime preconditions every guarantee in this crate is gated on |
//!
//! ## 43.44–43.47: prose, and the one exception
//!
//! Four §43 modules describe process rather than behaviour. `crates/stewardship` did this
//! classification for §14 and the same discipline applies here: `tools/coverage.sh` matches any
//! `NN.MM` token anywhere under `crates/`, so citing a module is enough to move the number, and a
//! citation with nothing behind it is the one thing that must not happen.
//!
//! - **43.44, research paper and theorem agenda** — prose. A publication plan: which claims go in
//!   which paper, in what order, with what negative results. Its only code-shaped artifact is an
//!   "open theorem and negative-result registry", which is 43.47's registry under another name.
//!   Not implemented, not cited elsewhere in this crate.
//! - **43.45, glossary and notation** — prose. A term table and a symbol table. A glossary is not
//!   a type. `AGENTS.md` already carries the workspace's operative glossary, which is where a
//!   contributor will actually look. Not implemented.
//! - **43.46, primary-source and implementation ledger** — prose. Its entry tuple
//!   `⟨source, type, claim, module, assumptions, implementation, validation, license, status⟩` is
//!   schema-shaped, but the module's substance is citation hygiene: adding sources before claims
//!   are finalised, labelling preprints, checking licences. An empty ledger struct would be a
//!   type with no content, and the content is the work. Not implemented.
//! - **43.47, formal semantics and theorem sketches** — **mixed, and the code half is implemented
//!   in [`theorem`]**. Its definitions and its five theorem candidates are prose, and stay prose;
//!   this crate proves nothing. But its runtime contract asks for something else: a
//!   "machine-checkable precondition indicating whether a theorem applies to a compiled plan", and
//!   a "counterexample corpus" of "worlds violating one assumption at a time". Both are code, both
//!   are things this crate needs anyway — the `1 − 1/e` factor is exactly a guarantee whose
//!   precondition has to be checked before it may be quoted — and [`theorem`] is that gate. What
//!   is implemented is the applicability check and the counterexample corpus. What is not is any
//!   proof, any mechanisation, and Theorem candidates A, B and E, which are about compiler passes
//!   this crate does not contain.
//!
//! So: three prose, one mixed with a narrow implemented core. `43.45` and `43.46` appear in this
//! crate only in the bullets above, saying they are not implemented. `43.44` appears once more, in
//! [`theorem`], where its list of candidate formal results is quoted as the *source* of the
//! adaptive-selection clause — that is a citation of where a claim came from, not a claim to have
//! implemented the research agenda.
//!
//! ## How much of §43 is skeleton
//!
//! Worth knowing before treating a module's length as a measure of its content. Over the 50 §43
//! modules — 4,558 lines, 3,328 non-blank, 28,508 words — counting any non-blank line that recurs
//! verbatim in at least 40 of the 50 modules:
//!
//! | Definition | Non-blank lines | Words |
//! |---|---|---|
//! | recurring body lines only | 24.3% | 17.4% |
//! | recurring body lines + YAML front matter | 39.4% | 24.4% |
//!
//! Sensitivity, in the order it turned out to matter:
//!
//! - **Threshold barely matters.** At a 50% recurrence threshold the figure is 24.3% of non-blank
//!   lines; at 80% it is *identically* 24.3%; at 95% it is 21.0%. Nothing recurs in a middling
//!   number of modules — the skeleton is in all 50 or in one or two.
//! - **Unit matters more.** The same definition reads 24.3% by non-blank line and 17.4% by word,
//!   a 7-point spread, because the repeated lines are short (`|---|---|`, `## Release gate`) while
//!   the prose lines are long.
//! - **Definition matters most.** Counting the YAML front matter moves the line figure by 15.1
//!   points, from 24.3% to 39.4%. It is ten lines per module of `title`, `status`, `owner`,
//!   `product` and `token_profile`, and it is either pure skeleton or the module's identity
//!   depending on what the number is for. Front matter alone moved §23 by 4.3 points in an earlier
//!   measurement; here it moves §43 by more than three times that, because §43's modules are
//!   shorter. **This figure counts it in the second row and not the first, and both are reported.**
//!
//! One line dominates the word count: the Release gate paragraph is 57 words, identical in all 50
//! modules, so that single sentence is 10% of §43's words on its own.
//!
//! ## What is not implemented
//!
//! See [`NOT_IMPLEMENTED`]. The headline omissions: no causal identification in the do-calculus
//! sense, no adaptive or sequential acquisition policy, no lazy-greedy speedup claim beyond
//! agreement with plain greedy, no matroid or partition constraints, no loopy message damping, and
//! no second context capsule.

pub mod continuation;
pub mod decision;
pub mod error;
pub mod evidence;
pub mod gap;
pub mod greedy;
pub mod lens;
pub mod library;
pub mod objective;
pub mod optimal;
pub mod patterns;
pub mod ratedistortion;
pub mod rng;
pub mod separator;
pub mod submodularity;
pub mod theorem;
pub mod voi;

pub use decision::{Belief, DecisionProblem};
pub use error::EpistemicError;
pub use evidence::{Acquisition, EvidenceItem, EvidencePool, Outcome};
pub use gap::{RequiredField, PROPOSED_SCHEMA_VERSION, REQUIRED_FOR_RATE_DISTORTION};
pub use greedy::{greedy, lazy_greedy, Constraint, Selection};
pub use objective::{Coverage, HypothesisElimination, RegretReduction, SetFunction, Tabulated};
pub use optimal::{brute_force_optimum, measure_ratio, RatioMeasurement};
pub use ratedistortion::{
    evaluate_context, frontier, identification, minimal_sufficient_context, DistortionCriterion,
    Frontier, Identification, Sufficiency,
};

pub use submodularity::{check, SubmodularityReport, Violation};
pub use theorem::{Applicability, Guarantee};
pub use voi::{complementarity, joint_value, value_of_information, ValueOfInformation};

/// Capabilities named by §43 that this crate does not implement, and what stands in their way.
///
/// Kept as a list rather than a paragraph so a later contributor can delete a line when they
/// close one. A missing capability that is stated is a limitation; one that is implied to exist is
/// a lie.
pub const NOT_IMPLEMENTED: &[&str] = &[
    "43.50: causal identification in the do-calculus sense. There is no graph, no back-door or \
     front-door criterion, and no instrument. Identification here is decision-relative — whether \
     the surviving models disagree about what to do — and the type is named to force the \
     distinction.",
    "43.50: sensitivity analysis to hidden confounding. The compatible-model set is supplied by \
     the caller; nothing widens it to account for an unobserved common cause.",
    "43.15/43.50: adaptive and sequential acquisition. Bundles are priced non-adaptively, which \
     is a lower bound on what an adaptive planner would achieve.",
    "43.14: the cost vector. Cost is a scalar here; the token, compute, latency, privacy, \
     specimen and expert-burden components 43.14 specifies are scalarised by the caller before \
     they arrive.",
    "43.14: matroid and partition constraints, and the partial-enumeration knapsack variant that \
     earns the (1-1/e)/2 factor. Cardinality and a simple cost-benefit knapsack are implemented; \
     the knapsack case reports no guarantee.",
    "43.14: beam, branch-and-bound and MCTS fallback planners. When submodularity fails, this \
     crate reports the failure and the measured ratio; it does not switch planner.",
    "43.29: iterative message passing with damping, and any convergence criterion for loopy \
     structures. One collection pass on a tree is exact and is checked; a loopy partition is \
     labelled approximate and left there.",
    "43.29: the separator-message ABI as a wire format. The type is in-process only; \
     bioprism-weave owns the transport and the authority that travels with it.",
    "43.30: the continuation capsule, the checkpoint store, and the Weave ABI adapters. \
     bioprism-weave owns all three. This module classifies a rebase and nothing else.",
    "43.31: unit conversion and coordinate transforms. bioprism-standards owns the dimension \
     system and the frames; this crate does not depend on it and does not reimplement it.",
    "43.31: ontology bindings with source versions. bioprism-standards owns term catalogues.",
    "43.49: dependent types as such. Indices are runtime-validated tags on values, not type \
     parameters, which is what 43.49 itself recommends for the public SDK.",
    "43.49: profunctor optics and lens composition beyond sequential focus chains.",
    "43.47: every proof. Theorem candidates A, B and E concern compiler passes this crate does \
     not contain, and no statement here is mechanised in Lean, TLA+ or Coq.",
];

/// Rejects duplicate identifiers in a collection.
///
/// Shared by every constructor that takes a `Vec<String>` of names. Identity has to be a key for a
/// result to be replayable, and a silently-shadowed duplicate produces a selection that cannot be
/// mapped back to its inputs.
pub(crate) fn unique(ids: &[String], collection: &str) -> Result<(), EpistemicError> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id.as_str()) {
            return Err(EpistemicError::DuplicateIdentifier {
                collection: collection.to_string(),
                id: id.clone(),
            });
        }
    }
    Ok(())
}
