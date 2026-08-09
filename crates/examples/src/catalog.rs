//! The registered slices.
//!
//! One function per scenario, each citing the blueprint module it makes runnable. Blueprint 43.41
//! is the canonical one — the radiogenomic cohort-integrity compiler — and the rest exist because
//! a single passing example proves almost nothing: it cannot tell a working oracle from one that
//! always answers `invalid`, a mandatory closure from a lucky dependency path, or a refusal from
//! a silent truncation.
//!
//! The decisive skeleton is identical across every slice: the same eleven protected facts, the
//! same five check factors, the same deterministic oracle. Only the *structure around it* and the
//! *query asked of it* change. That is what makes the differences between these reports
//! attributable to one variable at a time rather than to a different benchmark.

use crate::expectation::{
    BundleExpectation, BundleProbe, Compiled, Expectation, GraphWalkProbe, Refusal,
};
use crate::property::Property;
use crate::report::RefusalCode;
use crate::scenario::{QueryOverlay, SliceWorld};
use crate::slice::VerticalSlice;
use bioprism_section::OracleStatus;
use bioprism_worldgen::{LeakageMechanism, PolicySpec, WorldSpec, LOCAL_LAB_VALUE};

/// The eleven facts the compiler selects for the split-integrity query, in certificate order.
///
/// Written out rather than counted. Two strategies can both select eleven facts and disagree
/// about which eleven, and only one of those disagreements changes the verdict; a count-based
/// assertion cannot tell them apart.
pub const DECISIVE_FACTS: [&str; 11] = [
    "fact.cohort",
    "fact.decision_cut",
    "fact.label_source",
    "fact.negative_duplicates",
    "fact.policy",
    "fact.preprocess_fit",
    "fact.scanner",
    "fact.site",
    "fact.specimen_dates",
    "fact.split",
    "fact.subject_aliases",
];

/// The four witness kinds the oracle emits, in the order it emits them.
pub const ALL_WITNESS_KINDS: [&str; 4] = [
    "identity_leakage",
    "site_leakage",
    "temporal_leakage",
    "preprocessing_leakage",
];

/// The four passes `bioprism-fiber` declares it cannot run against the v0.1 wire schemas.
pub const DEFERRED_PASSES: [&str; 4] = [
    "obstruction_tests",
    "abstract_interpretation",
    "decision_quotient",
    "rate_distortion",
];

/// The protected tags the generated audit query carries.
const PROTECTED_TAGS: [&str; 10] = [
    "identity",
    "split",
    "site",
    "scanner",
    "time",
    "specimen",
    "preprocessing",
    "policy",
    "negative_evidence",
    "protected",
];

/// The canonical 43.41 world, shared by the compiler slice and the baseline slice.
///
/// Shared deliberately, for the same reason as [`NARROW_TARGET_WORLD`]: "the graph walk ties the
/// compiler" is a claim about one world, and it is only checkable when both slices demonstrably
/// ran against the same bytes.
const CANONICAL_WORLD: &str = "radiogenomic-integrity-v1";

/// The world both narrow-target slices run against.
///
/// Shared deliberately. The pair's argument is that the *only* difference between a wrong answer
/// and a right one is the protected-tag list, and that is only checkable if the two worlds are
/// byte-identical — which a shared `world_id` and a deterministic generator guarantee.
const NARROW_TARGET_WORLD: &str = "narrow-target-pair-v1";

/// The world both policy slices run against.
///
/// Shared for the third time and for the third instance of the same reason. The pair's argument is
/// that the withheld fact is withheld *by a clause the caller did not accept* and by nothing else,
/// which is only checkable when the two runs face the same corpus: a requirement lives on the
/// world, a grant lives on the query, so accepting the clause moves the query and leaves the world
/// byte-identical.
const POLICY_CONSENT_WORLD: &str = "policy-consent-pair-v1";

/// The twelve facts the external-confirmation skeleton compiles when the lab value is readable.
///
/// [`DECISIVE_FACTS`] plus `fact.local_lab`, in certificate order. Written out for the reason the
/// eleven are: the interesting difference between the policy pair's two runs is *which* fact left
/// the selection, and a count cannot express that.
pub const DECISIVE_FACTS_WITH_LOCAL_LAB: [&str; 12] = [
    "fact.cohort",
    "fact.decision_cut",
    "fact.label_source",
    "fact.local_lab",
    "fact.negative_duplicates",
    "fact.policy",
    "fact.preprocess_fit",
    "fact.scanner",
    "fact.site",
    "fact.specimen_dates",
    "fact.split",
    "fact.subject_aliases",
];

/// The check that reads the two lab variables. Its presence is what makes them decisive.
const CONFIRMATION_CHECK: &str = "factor.confirmation_check";

/// Every slice this crate registers, in the order a reader should meet them.
///
/// [`Property::DeterministicReplay`] is appended to every slice rather than assigned to one.
/// Byte-identical replay is not a property of a particular scenario; it is a property each slice
/// either has or has not, and the suite replays all of them.
pub fn all() -> Vec<VerticalSlice> {
    let slices = vec![
        radiogenomic_integrity(),
        reference_world_tie(),
        structural_discrimination(),
        clean_cohort(),
        temporal_firewall(),
        budget_refusal(),
        relevance_only_narrow_target(),
        protected_closure_narrow_target(),
        mutation_identity(),
        mutation_site(),
        mutation_temporal(),
        mutation_preprocessing(),
        cohort_scale(),
        unprotected_temporal_withholding(),
        policy_blocked_omission(),
        policy_released_control(),
        attested_bundle_replay(),
    ];

    slices
        .into_iter()
        .map(|mut slice| {
            if !slice.exercised().contains(&Property::DeterministicReplay) {
                slice.also_exercises.push(Property::DeterministicReplay);
            }
            slice
        })
        .collect()
}

/// 43.41, the canonical slice: the radiogenomic cohort-integrity compiler.
///
/// A 762-fact world in which four independent leakage mechanisms have been injected at once and
/// 750 exploratory summaries hang off the cohort hub. The claim is not that the compiler is small
/// — it is that the eleven facts it selects are *the* eleven that carry the decision, and that the
/// oracle returns four witnesses a human can check by hand against the source manifests.
pub fn radiogenomic_integrity() -> VerticalSlice {
    VerticalSlice::new(
        "radiogenomic-integrity-v1",
        "Radiogenomic cohort integrity: four leakage mechanisms, four exact witnesses",
        Property::ExactLeakageWitnesses,
        SliceWorld::new(WorldSpec::reference_like(750).with_world_id(CANONICAL_WORLD)),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .unmatched_protected_tags(Vec::<String>::new())
                .dropped_protected(Vec::<String>::new())
                .unresolved_obligation_count(0)
                .refinement_frontier_actions(Vec::<String>::new())
                .omitted_fact_count(751)
                .omission_influence_classes(["zero"])
                .supports_sufficiency_claim(true)
                .certificate_verifies(true)
                .backend("backward_factor_slice_reference".to_string())
                .deferred_passes_include(DEFERRED_PASSES),
        ),
    )
    .citing(["43.41", "38.01", "43.13", "43.26"])
    .also_exercising([
        Property::OmissionInfluenceClassified,
        Property::CertificateSelfVerifies,
        Property::DeferredPassesDeclared,
    ])
    .narrating(
        "The world injects identity, site, temporal and preprocessing leakage simultaneously and \
         surrounds the decisive evidence with 750 exploratory summaries that all consume the same \
         protected cohort hub. Compilation selects eleven facts and the oracle returns four \
         witnesses — a shared alias landing in two splits, one site per split, a label derived \
         after the training cut, and a preprocessing fit drawn across everything. Each witness \
         names the subjects involved, so a reader can refute it from the manifests without \
         rerunning anything. What this slice does not establish is that compiling was necessary: \
         see reference-world-tie-v1, which runs the competitor on this same world.",
    )
}

/// 43.38 and FINDINGS §1: the honest negative result on the shipped world.
///
/// The same world as the canonical slice, with an equal-engineering neighbourhood walk swept
/// across every depth. Depths 5 and 6 select exactly the compiled eleven. The compiler wins
/// nothing here, and a reference example that only ever ran the compiler would never have found
/// that out.
pub fn reference_world_tie() -> VerticalSlice {
    VerticalSlice::new(
        "reference-world-tie-v1",
        "The shipped world does not discriminate: a tuned graph walk selects the same eleven facts",
        Property::ReferenceWorldDoesNotDiscriminate,
        SliceWorld::new(WorldSpec::reference_like(750).with_world_id(CANONICAL_WORLD)),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure_satisfied(true),
        ),
    )
    .citing(["43.38", "43.39", "43.41"])
    .with_graph_walk(
        GraphWalkProbe::new(16, vec![5, 6])
            .at(4, 0, false)
            .at(5, 11, true)
            .at(6, 11, true),
    )
    .narrating(
        "Blueprint 43.41 requires that if graph baselines remain compact under equal \
         optimization, that result is reported. It does. On the world the distribution ships, a \
         depth-5 incidence walk returns exactly the eleven facts the compiler returns — the same \
         set, not merely the same count — with full protected closure and the same verdict. The \
         distribution's own comparison script measures only depths 7 and unbounded, the two \
         settings where the walk collapses to the whole world. This slice exists so that the \
         inconvenient measurement is the one that runs first.",
    )
}

/// 43.39: the corner of the structural space where the strategies separate.
///
/// Distractors attached near the target and decisive facts pushed behind a three-step relay chain
/// leave no depth that is simultaneously sound, closed and compact. Depths 5 to 10 take 98% of the
/// world and still miss every witness, which is the worst quadrant available.
pub fn structural_discrimination() -> VerticalSlice {
    VerticalSlice::new(
        "structural-discrimination-v1",
        "A world built to discriminate leaves the neighbourhood walk no usable depth",
        Property::StructuralDiscriminationNoUsableDepth,
        SliceWorld::new(
            WorldSpec::discriminating(750).with_world_id("structural-discrimination-v1"),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .supports_sufficiency_claim(true),
        ),
    )
    .citing(["43.39", "43.38", "43.01"])
    .with_graph_walk(
        GraphWalkProbe::new(16, vec![])
            .at(7, 750, false)
            .at(11, 761, true),
    )
    .narrating(
        "The decisive skeleton and the oracle are unchanged from the canonical slice; only two \
         structural knobs move. Distractors now attach near the target rather than at the cohort \
         hub, and the decisive facts sit behind a three-step relay chain. The consequence is that \
         every depth admitting the decisive set also admits every distractor. Depth 7 pulls 750 \
         facts and still reproduces none of the witnesses; depth 11 is the first sound setting and \
         by then it has taken the entire world. The usable window is empty, where on the shipped \
         world it was {5, 6}. The compiler's selection is unchanged at eleven facts, which is the \
         point: its cost tracks the decision, not the corpus.",
    )
}

/// 43.41: the oracle is not a constant `invalid`.
///
/// The cheapest way to score perfectly on a leakage benchmark is to always answer "leakage". A
/// suite without this slice cannot distinguish that from a working oracle.
pub fn clean_cohort() -> VerticalSlice {
    VerticalSlice::new(
        "clean-cohort-v1",
        "A world with no injected defect returns a valid verdict and no witnesses",
        Property::CleanWorldValidVerdict,
        SliceWorld::new(
            WorldSpec::discriminating(20)
                .with_world_id("clean-cohort-v1")
                .with_leakage(vec![]),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Valid)
                .witness_kinds(Vec::<String>::new())
                .selected_facts(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .unresolved_obligation_count(0)
                .refinement_frontier_actions(Vec::<String>::new())
                .supports_sufficiency_claim(true)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.41"])
    .narrating(
        "Identical generator, identical query, identical oracle — with the leakage list empty. \
         Every subject sits at one site, no alias is shared, no label postdates the cut, and \
         preprocessing is fit after the split. The verdict is valid with an empty witness list. \
         This is the negative control that makes every other slice in the suite mean something: \
         without it, an oracle hard-wired to return invalid would pass all of them.",
    )
}

/// 43.09: evidence the temporal cut withholds, and the verdict that must not be trusted.
///
/// The most important slice in the suite. Moving the decision time one month earlier makes the
/// training event unreleased, which withholds the split assignment, the training cut and the
/// preprocessing scope. The oracle, given what remains, finds nothing wrong and returns `valid` —
/// and it is *wrong*. What stops that answer being published is not the verdict but the three
/// unresolved obligations, the non-empty refinement frontier and the unsatisfied protected
/// closure that accompany it.
pub fn temporal_firewall() -> VerticalSlice {
    VerticalSlice::new(
        "temporal-firewall-v1",
        "An early decision cut withholds protected evidence and the valid verdict is not trustworthy",
        Property::TemporalAccessibilityWithholdsEvidence,
        SliceWorld::new(WorldSpec::reference_like(20).with_world_id("temporal-firewall-v1"))
            .with_query(QueryOverlay::new().decision_time("2024-12-01T00:00:00Z")),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Valid)
                .witness_kinds(Vec::<String>::new())
                .selected_fact_count(8)
                .protected_closure_size(11)
                .protected_closure_satisfied(false)
                .dropped_protected(["fact.decision_cut", "fact.preprocess_fit", "fact.split"])
                .unresolved_obligation_count(3)
                .refinement_frontier_actions(["advance_time_cut_or_use_retrospective_mode"])
                .omission_influence_classes(["zero", "deferred_acquisition"])
                .supports_sufficiency_claim(false)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.09", "43.25", "43.26", "43.28", "38.08"])
    .also_exercising([
        Property::RefinementFrontierNonEmpty,
        Property::OmissionInfluenceClassified,
    ])
    .narrating(
        "The training event becomes available on 2025-01-01; the decision is taken on 2024-12-01. \
         Three protected facts — the split assignment, the training cut and the preprocessing fit \
         scope — are governed by that event and are therefore unreadable. The oracle sees eight \
         facts, finds no contradiction among them, and returns valid with no witnesses. That \
         verdict is a false negative, and the slice asserts it, because pretending the compiler \
         detects leakage it cannot see would be the more comfortable lie. What the certificate \
         does carry is the correct accounting: three deferred-acquisition omissions, three \
         unresolved obligations, a refinement frontier naming the move that would discharge them, \
         a protected closure marked unsatisfied, and a sufficiency claim of false. A consumer that \
         reads only `status` gets the wrong answer; a consumer that reads the certificate cannot.",
    )
}

/// 43.13 and 43.25: refusal rather than a smaller context.
///
/// A budget below the protected closure has exactly one correct outcome. Returning the facts that
/// fit would be a context that looks compiled, carries a certificate, and has had the
/// decision-changing evidence removed from it.
pub fn budget_refusal() -> VerticalSlice {
    VerticalSlice::new(
        "budget-refusal-v1",
        "A budget below the protected closure is refused, not trimmed to fit",
        Property::RefusalOverSilentTruncation,
        SliceWorld::new(WorldSpec::reference_like(20).with_world_id("budget-refusal-v1"))
            .with_query(QueryOverlay::new().max_facts(5)),
        Expectation::refuses(
            Refusal::new(RefusalCode::BudgetExceeded)
                .selected(11)
                .max_facts(5),
        ),
    )
    .citing(["43.13", "43.25"])
    .narrating(
        "The query asks for at most five facts. The mandatory closure is eleven and 43.13 forbids \
         trimming it, so there is no admissible five-fact answer and the compiler says so with a \
         typed error naming both numbers. The failure mode this rules out is the attractive one: \
         return the five highest-scoring facts, attach a certificate, and let the consumer assume \
         the closure held. The refusal is asserted down to the selected count, so a future change \
         that refuses for some unrelated reason does not quietly pass this slice.",
    )
}

/// The negative control for protected closure: what relevance alone would have returned.
///
/// Retarget the query at `policy_validity` and drop every protected tag. The dependency slice is
/// then a single fact, the oracle sees one value, finds nothing wrong, and returns `valid`. This
/// is a confidently wrong answer produced by a strategy that did nothing incorrect — it followed
/// relevance exactly.
pub fn relevance_only_narrow_target() -> VerticalSlice {
    VerticalSlice::new(
        "relevance-only-narrow-target-v1",
        "Relevance alone selects one fact and returns a confidently wrong valid verdict",
        Property::RelevanceOnlySelectionIsUnsound,
        SliceWorld::new(WorldSpec::reference_like(20).with_world_id(NARROW_TARGET_WORLD))
            .with_query(
                QueryOverlay::new()
                    .query_id("relevance-only-narrow-target-v1-policy")
                    .targets(["policy_validity"])
                    .protected_tags(Vec::<String>::new()),
            ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Valid)
                .witness_kinds(Vec::<String>::new())
                .selected_facts(["fact.policy"])
                .protected_closure(Vec::<String>::new())
                .protected_closure_satisfied(true)
                .supports_sufficiency_claim(true),
        ),
    )
    .citing(["43.13", "43.41", "39.05"])
    .narrating(
        "The world is the leaky one from the canonical slice; only the query changes. Asking \
         whether the data policy is satisfied, with no protected tags declared, gives a backward \
         slice of exactly one fact. The oracle receives that one value, has nothing to contradict \
         it, and returns valid. Note what the certificate says: closure satisfied, sufficiency \
         claim true, every omission classed zero-influence — all correct, all conditional on the \
         declared factor graph, and all together adding up to a wrong answer about a world with \
         four leakage defects in it. The pairing with \
         protected-closure-narrow-target-v1 is the argument: the only difference between the two \
         is the protected-tag list.",
    )
}

/// 43.13: the mandatory closure enters the selection whatever relevance found.
///
/// The same one-fact query as the negative control, with the protected tags restored. Ten facts
/// no dependency path reaches are pulled in by closure alone, and the verdict flips from `valid`
/// to `invalid` with all four witnesses.
pub fn protected_closure_narrow_target() -> VerticalSlice {
    VerticalSlice::new(
        "protected-closure-narrow-target-v1",
        "Protected closure adds ten facts the dependency slice never reached, and flips the verdict",
        Property::ProtectedClosureOverridesRelevance,
        SliceWorld::new(
            WorldSpec::reference_like(20).with_world_id(NARROW_TARGET_WORLD),
        )
        .with_query(
            QueryOverlay::new()
                .query_id("protected-closure-narrow-target-v1-policy")
                .targets(["policy_validity"])
                .protected_tags(
                    PROTECTED_TAGS
                        .iter()
                        .map(|t| (*t).to_string())
                        .chain(["consent".to_string()])
                        .collect::<Vec<_>>(),
                ),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .unmatched_protected_tags(["consent"])
                .supports_sufficiency_claim(true),
        ),
    )
    .citing(["43.13", "39.05"])
    .also_exercising([Property::UnmatchedProtectedTagsReported])
    .narrating(
        "One field differs from relevance-only-narrow-target-v1: the protected-tag list. The \
         dependency slice still reaches exactly one fact, and closure adds the other ten \
         regardless — the alias table, the split assignment, the site map, the label lineage, the \
         negative duplicate screen. The verdict flips to invalid with all four witnesses. This is \
         the ordering argument of 43.13 as an executable difference: a relevance heuristic that \
         ran first could not have recovered any of it. The query also protects `consent`, which no \
         fact in this world carries, and the compiler reports that tag as unmatched rather than \
         counting it as a satisfied empty closure — a protected tag that matched nothing has \
         protected nothing.",
    )
}

fn mutation(id: &str, mechanism: LeakageMechanism, witness: &str, detail: &str) -> VerticalSlice {
    VerticalSlice::new(
        id.to_string(),
        format!("Isolated mutation: {witness}"),
        Property::MechanismIsolation,
        SliceWorld::new(
            WorldSpec::reference_like(20)
                .with_world_id(id.to_string())
                .with_leakage(vec![mechanism]),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds([witness])
                .selected_facts(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.41", "38.01", "19.05"])
    .narrating(format!(
        "{detail} Exactly one witness kind is produced, and the compiled selection is unchanged \
         at eleven facts. Isolation is what makes the oracle diagnostic rather than a pass/fail \
         bit: a suite that only ever ran the all-defects world could not tell a detector of four \
         mechanisms from a detector of one that happens to fire whenever any is present."
    ))
}

/// One alias resolving to subjects on both sides of the split.
pub fn mutation_identity() -> VerticalSlice {
    mutation(
        "mutation-identity-v1",
        LeakageMechanism::Identity,
        "identity_leakage",
        "Alias ALT-77 is attached to the first and last subject, which the generator places in \
         different splits. Nothing else changes: one site, no future labels, preprocessing fit \
         after the split.",
    )
}

/// Each split drawn from exactly one, differing, site.
pub fn mutation_site() -> VerticalSlice {
    mutation(
        "mutation-site-v1",
        LeakageMechanism::Site,
        "site_leakage",
        "Every training subject is scanned at site A and every test subject at site B, so site is \
         perfectly confounded with the split and no external-generalisation claim survives.",
    )
}

/// A label derived from evidence that postdates the training cut.
pub fn mutation_temporal() -> VerticalSlice {
    mutation(
        "mutation-temporal-v1",
        LeakageMechanism::Temporal,
        "temporal_leakage",
        "One subject's label is derived from a source recorded on 2025-06-01, five months after \
         the 2025-01-01 training cut. This is the mutation 43.41's worked micro-example describes: \
         file order is unchanged, so similarity retrieval surfaces the result without noticing \
         that it did not exist yet.",
    )
}

/// A transform fit across every subject before the split was drawn.
pub fn mutation_preprocessing() -> VerticalSlice {
    mutation(
        "mutation-preprocessing-v1",
        LeakageMechanism::Preprocessing,
        "preprocessing_leakage",
        "The preprocessing fit scope is recorded as all_subjects_before_split, so test statistics \
         entered the transform the training data was normalised with.",
    )
}

/// 38.01's runtime contract: 80 to 200 subjects.
///
/// The generator's subject count is a parameter, so the contract is checkable rather than
/// aspirational. Nothing about the compiled region changes: the selection stays at eleven facts
/// while the cohort grows thirtyfold, which is the compile-cost claim of 43.34 in its cheapest
/// observable form.
pub fn cohort_scale() -> VerticalSlice {
    VerticalSlice::new(
        "cohort-scale-v1",
        "The reference world generates at 38.01's 80-200 subject scale without changing the compiled region",
        Property::CohortScaleContract,
        SliceWorld::new(WorldSpec {
            subjects: 120,
            ..WorldSpec::reference_like(20).with_world_id("cohort-scale-v1")
        }),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .certificate_verifies(true),
        ),
    )
    .citing(["38.01", "43.34", "43.41"])
    .narrating(
        "One hundred and twenty subjects instead of four. Each protected fact's *value* grows — \
         the split map, the site map and the alias table now carry 120 entries each — but the \
         number of facts the compiler selects does not move, because compile cost tracks the \
         compiled region and not the corpus. The four witnesses still name the exact subjects \
         involved, which at this cohort size is the difference between a usable finding and a \
         flag.",
    )
}

/// 43.09: withholding that leaves the mandatory closure intact.
///
/// The partner to [`temporal_firewall`], and the reason the two must both exist. There, an early
/// cut removed *protected* evidence and the certificate said so through `dropped_protected` and an
/// unsatisfied closure. Here the cut removes `fact.central_lab`, which the query does not protect,
/// so `dropped_protected` is empty and the closure is complete — and evidence the decision depends
/// on is gone anyway. A consumer checking only the closure sees nothing wrong.
pub fn unprotected_temporal_withholding() -> VerticalSlice {
    VerticalSlice::new(
        "unprotected-temporal-withholding-v1",
        "A decisive unprotected fact is withheld at the cut while the protected closure stays complete",
        Property::NonProtectedTemporalWithholding,
        SliceWorld::new(
            WorldSpec::external_confirmation(20)
                .with_world_id("unprotected-temporal-withholding-v1"),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS_WITH_LOCAL_LAB)
                .selected_factors_include([CONFIRMATION_CHECK])
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .dropped_protected(Vec::<String>::new())
                .inaccessible_selected_before_cut(["fact.central_lab"])
                .unresolved_obligation_count(1)
                .refinement_frontier_actions(["advance_time_cut_or_use_retrospective_mode"])
                .omission_influence_classes(["zero", "deferred_acquisition"])
                .omitted_fact_count(22)
                .supports_sufficiency_claim(false)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.09", "43.13", "43.26", "38.08"])
    .also_exercising([
        Property::OmissionInfluenceClassified,
        Property::RefinementFrontierNonEmpty,
    ])
    .narrating(
        "The world adds a sixth check that reads two lab variables, both tagged `assay_result`, \
         which no query in this crate protects. `local_lab_value` is released before the decision \
         cut and `central_lab_confirmation` months after it, so the two are identical in tagging \
         and in event management and differ only in when a reader may see them. The compile \
         therefore selects twelve facts rather than eleven, loses `fact.central_lab` to the cut, \
         and reports `dropped_protected` empty with the closure satisfied — the state \
         temporal-firewall-v1 cannot produce, because there every withheld fact was also \
         protected. What stands in for the closure here is the rest of the accounting: the \
         withheld fact is named, its omission group is classed deferred_acquisition rather than \
         zero, the sufficiency claim is false, and the frontier names the move that would recover \
         it. This is the separation 43.09 asks for made checkable: a temporal withholding is a \
         different failure from a closure violation, and a certificate that reported only the \
         second would call this world clean.",
    )
}

/// 43.33 and 39.05: evidence a policy screen withheld, classed as such.
///
/// `local_lab_value` requires the clause `consent-tier-2`. The corpus grants it and the query does
/// not accept it, so the fact is withheld — and the omission is recorded as `inaccessible_by_policy`
/// rather than swept into the structural-irrelevance group with the 21 exploratory summaries.
pub fn policy_blocked_omission() -> VerticalSlice {
    VerticalSlice::new(
        "policy-blocked-omission-v1",
        "A fact the caller is not entitled to read is withheld and classed as policy-blocked, not as irrelevant",
        Property::PolicyBlockedOmission,
        SliceWorld::new(WorldSpec::policy_restricted(20).with_world_id(POLICY_CONSENT_WORLD)),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .selected_factors_include([CONFIRMATION_CHECK])
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .dropped_protected(Vec::<String>::new())
                .inaccessible_selected_before_cut(["fact.central_lab"])
                .unresolved_obligation_count(2)
                .refinement_frontier_actions([
                    "declare_the_required_policy_clauses_or_obtain_a_grant",
                    "advance_time_cut_or_use_retrospective_mode",
                ])
                .omission_influence_classes(["zero", "inaccessible_by_policy", "deferred_acquisition"])
                .omitted_fact_count(23)
                .supports_sufficiency_claim(false)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.26", "43.33", "39.05"])
    .also_exercising([
        Property::OmissionInfluenceClassified,
        Property::RefinementFrontierNonEmpty,
    ])
    .narrating(
        "The corpus declares that `local_lab_value` may be released only to a reader holding \
         `consent-tier-2`; the query accepts `research-only` alone. The screen withholds the fact \
         and the certificate carries three omission groups where the reference world carries one: \
         21 facts no dependency path reaches, classed zero; one withheld by the data policy, \
         classed inaccessible_by_policy; and one governed by an event the cut has not reached, \
         classed deferred_acquisition. The three are not interchangeable and this slice asserts \
         all three by name, because the failure mode worth ruling out is the tidy one — a screen \
         that withholds a fact and then files it under 'not reachable from the target', which is \
         true of nothing here and would make the certificate say the decision rested on \
         everything relevant. The mandatory closure is untouched: 43.13 forbids trimming it, so a \
         policy requirement over a protected variable is a refusal rather than an omission, and \
         this world deliberately puts the requirement somewhere the compile can survive it. The \
         partner slice, policy-released-control-v1, accepts the clause on the same corpus.",
    )
}

/// The control for [`policy_blocked_omission`]: the same corpus, with the clause accepted.
///
/// A withheld fact and a fact the world never had are indistinguishable from one compile. Granting
/// the clause on byte-identical world bytes is what makes the exclusion attributable to the policy
/// decision rather than to the world's contents.
pub fn policy_released_control() -> VerticalSlice {
    VerticalSlice::new(
        "policy-released-control-v1",
        "Accepting the clause on the same corpus releases the fact and removes the policy omission group",
        Property::PolicyBlockedOmission,
        SliceWorld::new(WorldSpec {
            policy: PolicySpec::requiring(LOCAL_LAB_VALUE, "consent-tier-2")
                .accepting("consent-tier-2"),
            ..WorldSpec::policy_restricted(20).with_world_id(POLICY_CONSENT_WORLD)
        })
        .with_query(QueryOverlay::new().query_id("policy-consent-pair-v1-consented")),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS_WITH_LOCAL_LAB)
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .dropped_protected(Vec::<String>::new())
                .inaccessible_selected_before_cut(["fact.central_lab"])
                .unresolved_obligation_count(1)
                .refinement_frontier_actions(["advance_time_cut_or_use_retrospective_mode"])
                .omission_influence_classes(["zero", "deferred_acquisition"])
                .omitted_fact_count(22)
                .supports_sufficiency_claim(false)
                .certificate_verifies(true),
        ),
    )
    .citing(["43.33", "39.05"])
    .narrating(
        "One field differs from policy-blocked-omission-v1: the clause list the query accepts. A \
         requirement is declared on the corpus and a grant is declared on the query, so accepting \
         `consent-tier-2` leaves the world byte-identical — the two slices share a world digest, \
         which is what makes this a control rather than a second experiment. `fact.local_lab` \
         returns to the selection, the inaccessible_by_policy group disappears, and the policy \
         obligation with it. What does not change is the deferred_acquisition group: \
         `fact.central_lab` is still behind a release the cut has not reached, and no clause a \
         caller accepts moves a date. Reading the two reports side by side is the whole argument \
         that the first slice's omission was a policy decision and not a gap in the corpus.",
    )
}

/// 34.14 and 19.06: this slice's own compile, packaged and handed back.
///
/// The bundle carries the certificate and section that were just compiled, the query as asked, and
/// the world by digest. A verifier recomputes what travelled, finds the world unchecked and says
/// so, and authenticates the manifest — under a shared secret published in this crate's source,
/// which is why the report also records that the verifier can mint the identical bytes.
pub fn attested_bundle_replay() -> VerticalSlice {
    VerticalSlice::new(
        "attested-bundle-replay-v1",
        "A compiled certificate survives a bundle round trip, and the tag authenticates a key rather than a party",
        Property::AttestedResultBundleReplay,
        SliceWorld::new(
            WorldSpec::reference_like(20).with_world_id("attested-bundle-replay-v1"),
        ),
        Expectation::compiles(
            Compiled::new()
                .status(OracleStatus::Invalid)
                .witness_kinds(ALL_WITNESS_KINDS)
                .selected_facts(DECISIVE_FACTS)
                .protected_closure(DECISIVE_FACTS)
                .protected_closure_satisfied(true)
                .omission_influence_classes(["zero"])
                .omitted_fact_count(21)
                .supports_sufficiency_claim(true)
                .certificate_verifies(true),
        ),
    )
    .citing(["34.14", "19.06", "43.26", "38.01"])
    .with_bundle(
        BundleProbe::new(
            "attested-bundle-replay-v1",
            "aurora-examples-2026",
            vec![0x2a; 32],
            "AURORA BioPRISM reference examples",
        )
        .expecting(
            BundleExpectation::new()
                .recomputed_entries(["certificate", "query", "section"])
                .not_recomputed(["world"])
                .embedded_certificate("self_verified".to_string())
                .survives_json_round_trip(true)
                .authenticated_key("aurora-examples-2026".to_string())
                .scheme("symmetric-shared-secret".to_string())
                .repudiability("forgeable-by-any-verifier".to_string())
                .without_the_key("wrong_key_offered".to_string())
                .verifier_forgery_is_identical(true),
        ),
    )
    .narrating(
        "The certificate this bundle carries was not written for the bundle; it came out of the \
         compile three lines above it, which is the difference between testing the bundle format \
         and testing that a compiled result survives transport. Three entries travel and are \
         rehashed on arrival — the certificate, the section, the query — and the world travels as \
         a digest, which verification reports as one entry it could not check rather than as a \
         fourth pass. The carried certificate still satisfies 43.26's own self-verification after \
         the round trip. Then the part that would be easy to leave out: the scheme is HMAC-SHA256 \
         under a shared secret, so a reviewer holding a different key learns nothing at all rather \
         than learning that the tag is bad, and a reviewer holding *this* key mints a \
         byte-identical bundle. Both are recorded as observations. The claim this slice is \
         registered against was narrowed for that reason — 'signed' and 'independently' were both \
         doing work the workspace cannot pay for, and the honest statement is that the bundle \
         verifies without the compiler, not without the producer.",
    )
}
