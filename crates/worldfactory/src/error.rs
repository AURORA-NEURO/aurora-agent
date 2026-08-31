//! Typed refusals.
//!
//! Every construction in this crate that can fail fails with a named reason rather than an
//! `Option` or a boolean, because the reason is the product. A benchmark factory that reports
//! "rejected: 412" has told an author nothing; a factory that reports "rejected because the
//! seeded discordance had no admissible explanation" has told them their program is wrong.
//!
//! The refusals are grouped by the blueprint module that motivates them, and each variant's doc
//! comment names the failure from that module's *Failure and abuse risks* list that it blocks.
//! Those lists are the only part of a §27 module that is module-specific enough to implement
//! against — see the boilerplate measurement in the crate root.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 27.01. Why a candidate parent world could not be frozen.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum FreezeRefusal {
    /// 27.01 required artifacts. A parent missing one of the seven named artifacts is not a
    /// parent, it is a draft, and freezing it makes the gap permanent and invisible.
    #[error("candidate is missing the required artifact `{artifact}`")]
    MissingArtifact { artifact: String },
    /// 27.01 failure "author encodes preferred workflow as only acceptable path", and 27.02
    /// workflow step 4, "identify decision points and alternative valid paths". A decision map in
    /// which every decision point admits exactly one action encodes the author's habits as the
    /// oracle.
    #[error(
        "every one of the {points} decision points admits exactly one action, so the world scores \
         agreement with the author rather than competence"
    )]
    SinglePathAuthoring { points: usize },
    /// 27.01 failure "future information leaks". An artifact that only became available after the
    /// decision point is evidence the decider could not have had.
    #[error(
        "artifact `{artifact}` became available at {available_at}, after decision point \
         `{decision}` at {decided_at}"
    )]
    FutureInformation {
        artifact: String,
        decision: String,
        available_at: String,
        decided_at: String,
    },
    /// 27.01 validation "clean rebuild". A parent that cannot be rebuilt from its manifest is a
    /// one-off, and every result computed on it is unreproducible by construction.
    #[error("the candidate has no recorded clean rebuild, so it cannot be reproduced")]
    NoCleanRebuild,
    /// 27.01 validation "license review" and 27.02 failure "controlled data accidentally
    /// embedded".
    #[error("the candidate embeds controlled asset `{asset}`, which may not be redistributed")]
    ControlledAssetEmbedded { asset: String },
    /// A review ran and found something. Distinct from a review that never ran: one is a known
    /// defect and the other is an unknown, and 27.01's whole point is that quality begins with
    /// audited parents.
    #[error("the `{review}` review failed: {finding}")]
    ReviewFailed { review: String, finding: String },
    /// A review never ran, and the tier being requested requires it. Freezing at a lower tier is
    /// available and records the gap; silently freezing at the higher one is not.
    #[error("the `{review}` review was not performed, and the {tier} tier requires it")]
    ReviewNotPerformed { review: String, tier: String },
}

/// 27.02. Why an observed world could not be declared.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ObservedRefusal {
    /// 27.02 validation "cohort count reconciliation". The declared cohort size and the sum of the
    /// declared strata must agree; when they do not, one of them is wrong and neither can be used.
    #[error("declared cohort size {declared} does not reconcile with the strata summing to {strata_total}")]
    CohortCountUnreconciled { declared: u64, strata_total: u64 },
    /// 27.02 failure "selection bias presented as world truth". A cohort assembled by an
    /// undeclared procedure supports no statement about a population.
    #[error("the cohort's selection procedure is undeclared, so the world cannot stand for a population")]
    UndeclaredSelection,
    /// 27.02 validation "source-version pinning". An unpinned source silently changes underneath
    /// every result computed against it.
    #[error("source `{reference}` is not pinned to a version")]
    UnpinnedSource { reference: String },
}

/// 27.03. Why a graft could not be applied, or its result could not be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum GraftRefusal {
    /// 27.03 workflow step 5, "leave unrelated structure unchanged". A graft that edited a fact
    /// outside its declared target set has an undocumented blast radius, and the changed-state
    /// manifest it emits is a lie.
    #[error("graft `{graft}` changed fact `{fact}`, which is outside its declared target set")]
    OutsideTargetSet { graft: String, fact: String },
    /// 27.03 failure "only one file changes and reveals answer". A graft whose entire footprint is
    /// the one fact the oracle asks about is a lookup, not a benchmark.
    #[error(
        "graft `{graft}` changed exactly one fact, `{fact}`, and that fact is the one the oracle \
         asks about"
    )]
    SingleFactTell { graft: String, fact: String },
    /// 27.03 workflow step 1, "select a validated observed world". A graft onto a fact that was
    /// never observed is grafting onto a graft, and the resulting world has no observed structure
    /// under the point it claims to be testing.
    #[error("graft `{graft}` targets fact `{fact}`, which is itself injected, not observed")]
    TargetIsItselfInjected { graft: String, fact: String },
    /// 27.03 failure "synthetic label presented as observed fact". Deserialising a fact with no
    /// declared origin would produce a world that has forgotten what it invented.
    #[error("fact `{fact}` was deserialised without a declared origin")]
    OriginNotDeclared { fact: String },
}

/// 27.04. Why a mechanistic world could not be declared.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum SimulatorRefusal {
    /// 27.04 required artifact "model-limit card". A simulator that declares no assumptions
    /// declares no limits, and then every claim drawn from it looks unconditional.
    #[error("simulator `{simulator}` declares no assumptions, so no claim from it can be bounded")]
    NoDeclaredAssumptions { simulator: String },
    /// 27.04 failure "parameters chosen after model results". Calibration that postdates the
    /// result it supports is fitting, and the recovery test it passed proves nothing.
    #[error(
        "simulator `{simulator}` was calibrated at {calibrated_at}, after the result it is cited \
         for at {result_at}"
    )]
    CalibratedAfterResult {
        simulator: String,
        calibrated_at: String,
        result_at: String,
    },
    /// 27.04 validation "out-of-calibration tests". A parameter regime outside the calibrated
    /// envelope is extrapolation, and the simulator's competence there is unmeasured.
    #[error(
        "parameter `{parameter}` at {value} lies outside the calibrated interval \
         [{low}, {high}]"
    )]
    OutOfCalibration {
        parameter: String,
        value: String,
        low: String,
        high: String,
    },
}

/// 27.10. Why a pre-analytic mutation was not admitted.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum PreanalyticRefusal {
    /// 27.10 failure "biological state accidentally changes". This is the postcondition that makes
    /// the family a *pre-analytic* family: a handling fault degrades what can be measured, it does
    /// not edit the biology. A mutation that changed the biological state is a controlled semantic
    /// mutation (27.09) wearing a pre-analytic label.
    #[error(
        "mutation `{mutation}` altered the biological state (`{field}`), which a handling fault \
         cannot do"
    )]
    BiologicalStateChanged { mutation: String, field: String },
    /// 27.10 required artifact "QC signal". A fault with no observable signature asks the agent to
    /// detect something the world does not contain.
    #[error("mutation `{mutation}` left no QC signature, so its detection task is unanswerable")]
    NoQcSignature { mutation: String },
    /// 27.10 failure "QC label leaks answer". A QC field whose value names the fault is a label,
    /// not a signal.
    #[error("mutation `{mutation}` wrote its own name into QC field `{field}`")]
    QcLabelLeaksAnswer { mutation: String, field: String },
    /// 27.10 validation "cross-stage consistency". A fault injected at one stage whose downstream
    /// stage records still describe the pre-fault specimen is internally contradictory in a way no
    /// laboratory produces.
    #[error(
        "mutation `{mutation}` acted at stage `{stage}` but stage `{downstream}` still records the \
         pre-fault state"
    )]
    StagesInconsistent {
        mutation: String,
        stage: String,
        downstream: String,
    },
    /// 27.10 validation "false-positive control". The zero-intensity member of a family must be a
    /// no-op; if it is not, every detection reported by the family is uninterpretable.
    #[error("the null member of family `{family}` changed the specimen, so the family has no false-positive control")]
    NullMemberIsNotNull { family: String },
    /// 27.10's critical design decision makes the asked-for response depend on "available actions".
    /// Demanding a correction in a world with no correction action is an unanswerable task dressed
    /// as a hard one.
    #[error("mutation `{mutation}` expects `{response}`, but the world offers no `{missing}`")]
    ResponseNotAvailable {
        mutation: String,
        response: String,
        missing: String,
    },
}

/// 27.11. Why a specimen-identity program was not admitted.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum IdentityProgramRefusal {
    /// 27.11 workflow steps 4 and 5. A relabelling changes no content, so the only thing that can
    /// catch it is identity evidence. Withholding that evidence is legitimate — but then the
    /// expected response is abstention, and demanding detection asks for the impossible.
    #[error(
        "program `{program}` expects detection of `{operation}`, but the world exposes no identity \
         evidence that could distinguish it"
    )]
    UndetectableByConstruction { program: String, operation: String },
    /// 27.11 validation "acyclic lineage".
    #[error("specimen `{specimen}` is its own ancestor")]
    LineageCycle { specimen: String },
    /// 27.11 failure "all modalities changed so no contradiction remains". A swap propagated to
    /// every artifact is not a swap, it is a rename of the whole subject, and nothing in the world
    /// disagrees with anything else.
    #[error(
        "operation `{operation}` was propagated to all {artifacts} artifacts, leaving no \
         disagreement for anyone to find"
    )]
    PropagatedEverywhere { operation: String, artifacts: usize },
    /// 27.11 failure "swap crosses inaccessible datasets".
    #[error("operation `{operation}` moves material across the access boundary between `{left}` and `{right}`")]
    CrossesAccessBoundary {
        operation: String,
        left: String,
        right: String,
    },
    /// 27.11 required artifact "mass-balance effect", validation "quantity conservation".
    #[error(
        "specimen `{specimen}` yields aliquots totalling {child_total} from a parent of {parent_mass}"
    )]
    MassNotConserved {
        specimen: String,
        child_total: String,
        parent_mass: String,
    },
}

/// 27.14. Why two readings could not be posed as a contradiction, or a contradiction could not be
/// narrowed.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ContradictionRefusal {
    /// The reading's public representation could not be reconciled with the value accessor. This
    /// is a malformed reading, not a scientific disagreement, and must not be turned into one.
    #[error("modality `{modality}` carries an inconsistent reading: {detail}")]
    InvalidReading { modality: String, detail: String },

    /// The readings are about different quantities, so there is no shared subject for them to
    /// disagree about. This is 27.14's "assay scope" axis showing up before a contradiction is even
    /// posed, and it is a different situation from two readings of one quantity.
    #[error("the readings report different quantities, `{left}` and `{right}`")]
    DifferentQuantities { left: String, right: String },
    /// One modality was never applied to the quantity. An absence of evidence is not a conflict,
    /// and the rule `bioprism_oncoworlds` fixed for markers holds here: a modality that did not
    /// look has not made a negative finding.
    #[error("modality `{modality}` was not examined for this quantity, so it has not disagreed with anything")]
    ModalityNotExamined { modality: String },
    /// The two values are compatible — the same category, or two intervals that intersect. 27.14's
    /// failure "uncertainty not represented" is the mistake of comparing point estimates and
    /// manufacturing a disagreement out of precision the measurements never claimed.
    #[error("the readings agree on `{value}`")]
    ReadingsAgree { value: String },
    /// 27.14 validation "scope and time alignment". Readings whose scopes do not overlap are not
    /// in disagreement; they are statements about different things, and calling them a
    /// contradiction manufactures a puzzle out of a category error. The dimension that separates
    /// them is named, because that dimension *is* the finding.
    #[error("the two readings do not overlap on dimension `{dimension}`: {reason}")]
    ScopesDoNotOverlap { dimension: String, reason: String },
    /// `bioprism_scope::Meet::Conflict`: the same dimension bound to values of different kinds.
    /// This is a modelling error in the world, and reporting it as "no overlap" would hide a bug
    /// behind a scientific-sounding sentence.
    #[error("dimension `{dimension}` is bound to values of different kinds in the two readings")]
    IncomparableScopes { dimension: String },
    /// 27.14 failure "discordance impossible biologically". If no explanation in the vocabulary can
    /// account for the disagreement, the program has constructed something that does not happen,
    /// and an agent that cannot solve it has not failed.
    #[error(
        "no admissible explanation accounts for the disagreement between `{left}` and `{right}`"
    )]
    NoAdmissibleExplanation { left: String, right: String },
    /// 27.14 failure "one modality arbitrarily labeled correct". Narrowing requires a
    /// discriminator that names what it refutes. Preferring a modality is not evidence.
    #[error("modality `{modality}` was asserted correct without a discriminator that refutes the alternatives")]
    ModalityPreferredWithoutEvidence { modality: String },
    /// A discriminator that refutes every live hypothesis leaves the empty set, which is not an
    /// answer: it says the world contains no account of its own contents. That is a defect in the
    /// program, and it must not be reported as a confident resolution.
    #[error("discriminator `{discriminator}` refutes every remaining hypothesis, leaving no account of the world")]
    AllHypothesesRefuted { discriminator: String },
    /// 27.14 required artifact "reference distribution". Whether a discordance rate is surprising
    /// is a question about a reference, and without one "unexpected" is an aesthetic judgement.
    #[error("no reference discordance distribution is declared for the modality pair `{left}`/`{right}`")]
    NoReferenceDistribution { left: String, right: String },
    /// 27.14 validation "answer-cue scan". If the seeded explanation is the unique reading with
    /// some surface property, the program is solvable without reasoning about biology at all.
    #[error(
        "the seeded explanation is recoverable from surface cue `{cue}` without examining evidence"
    )]
    TrivialCue { cue: String },
}

/// A claim that the world it was drawn from cannot support.
///
/// The ladder of 27.01–27.04 in one type. See [`crate::provenance`] for what each variant means
/// and why the refusal is the *product* rather than an obstacle to it.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ClaimRefusal {
    /// The claim is of a kind that the rungs this world stands on cannot reach.
    ///
    /// Note that this is not a one-directional ladder. An observed world cannot support a claim
    /// about detecting injected structure, because it has none; a mechanistic world cannot support
    /// a claim about biology, because everything in it was written down by a modeller. Neither is
    /// "stronger"; they answer different questions.
    #[error("a {claim} claim requires {required}; this world stands on {stands_on}")]
    ExceedsRung {
        claim: String,
        required: String,
        stands_on: String,
    },
    /// The world was built by assuming the very thing the claim asserts. 27.04's "simulator encodes
    /// benchmark answer trivially" and 27.03's "synthetic label presented as observed fact" are the
    /// same error seen from two rungs, and this is the variant that catches both.
    #[error(
        "`{quantity}` is an assumption of the construction ({assumed_by}), so the world cannot be \
         evidence for it"
    )]
    AssumedByConstruction {
        quantity: String,
        assumed_by: String,
    },
    /// 27.02 workflow step 5, "declare what counterfactuals are unsupported". An observed world is
    /// weakest exactly where it is most often used: the question of what would have happened
    /// instead.
    #[error("the counterfactual `{counterfactual}` is declared unsupported by this world's study design")]
    CounterfactualNotIdentified { counterfactual: String },
    /// 27.02 failure "selection bias presented as world truth", raised at claim time rather than
    /// construction time because a selected cohort is perfectly good evidence about itself.
    #[error("the cohort was assembled by {selection}, which does not support a claim about `{population}`")]
    SelectedCohort {
        selection: String,
        population: String,
    },
}

/// The crate's umbrella error.
///
/// Present so callers can propagate with `?` across modules. Each variant keeps its module's
/// refusal intact rather than flattening to a string, because the caller that wants to *handle* a
/// refusal needs its fields, and the caller that only wants to print it gets `Display` either way.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldFactoryError {
    #[error(transparent)]
    Freeze(#[from] FreezeRefusal),
    #[error(transparent)]
    Observed(#[from] ObservedRefusal),
    #[error(transparent)]
    Graft(#[from] GraftRefusal),
    #[error(transparent)]
    Simulator(#[from] SimulatorRefusal),
    #[error(transparent)]
    Preanalytic(#[from] PreanalyticRefusal),
    #[error(transparent)]
    Identity(#[from] IdentityProgramRefusal),
    #[error(transparent)]
    Contradiction(#[from] ContradictionRefusal),
    #[error(transparent)]
    Claim(#[from] ClaimRefusal),
}
