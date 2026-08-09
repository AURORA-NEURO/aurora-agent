//! The one rule these six section tails share, asserted once per currency.
//!
//! Each of the modules in this crate came from a different blueprint section and none of them
//! knows about the others. They nonetheless all had to solve the same problem, because it is the
//! problem this workspace exists for: **"nobody checked" and "checked and it cannot matter" must
//! not share a representation.**
//!
//! Each test below is that rule in one currency. Together they are the argument that the rule is
//! not a stylistic preference in `bioprism-fiber` but a structural requirement that reappears
//! wherever a system records what it did not do.
//!
//! A unit test inside each module asserts the local behaviour. These assert the *pattern*, which is
//! what would be lost if someone later "simplified" one of the three-state enums into a boolean.

use bioprism_ids::ContentHash;
use bioprism_sweep::{
    cache::{Attempt, AttemptLedger},
    conform::{differential, CapabilityCard, Check, ClaimState, Drift, RunRef},
    effects::{OutcomeTally, SafetyOutcome},
    explorer::{project, summarize, Clearance, Elision, NodeKind, TraceNode},
    fidelity::{Declaration, Level},
    interop::{CaseResult, CompatibilityCase, SuiteResult, SuiteStatus},
    provenance::{Alias, Catalog, ResolveOutcome, Revision, Tombstone, TombstoneReason},
    redact::{ScoredOutcome, Tally},
    regression::Fragility,
    state::{
        compare, CaptureMethod, Component, EqualityKind, Partition, StateVerdict, Visibility,
        WorldStateManifest,
    },
};

fn hash(seed: &str) -> ContentHash {
    ContentHash::of_bytes(seed.as_bytes())
}

/// 03.03 — state: a component nobody could compare is not a component that matched.
#[test]
fn in_state_comparison_unchecked_does_not_collapse_into_equal() {
    let make = |digest: &str| {
        WorldStateManifest::new(
            "s",
            vec![Component::new(
                "svc",
                Partition::ExternalService,
                Visibility::AgentVisible,
                CaptureMethod::LiveDependency,
                hash(digest),
                EqualityKind::DeclaredEquivalence,
                Declaration::degraded("live service").unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
    };
    let comparison = compare(&make("a"), &make("b"), &[]).unwrap();
    assert_eq!(comparison.verdict(), StateVerdict::Indeterminate);
    assert_ne!(comparison.verdict(), StateVerdict::Equal);
}

/// 03.11 — provenance: a withdrawal is not a silence.
#[test]
fn in_a_catalog_tombstoned_does_not_collapse_into_unknown() {
    let alias = Alias::new("aurora", "pack", "1.0.0").unwrap();
    let revision = Revision::new(hash("r"));
    let mut catalog = Catalog::new();
    catalog.publish_alias(&alias, revision.clone());
    catalog.tombstone(
        Tombstone::new(revision, TombstoneReason::Dangerous, "contains holdout answers").unwrap(),
    );
    let outcome = catalog.resolve(&alias);
    assert!(matches!(outcome, ResolveOutcome::Tombstoned { .. }));
    assert_ne!(outcome, ResolveOutcome::Unknown);
}

/// 04.05 — redaction: evidence that was removed is not evidence of a pass.
#[test]
fn in_scoring_unevaluable_does_not_collapse_into_pass_or_fail() {
    let outcome = ScoredOutcome::Unevaluable { missing: vec!["messages.content".into()] };
    assert!(!outcome.is_pass());
    assert!(!outcome.is_fail());

    let mut tally = Tally::default();
    tally.add(&outcome);
    assert_eq!(tally.rate(), None);
    assert_eq!(tally.unevaluable(), 1);
}

/// 04.03 — interop: a compatibility case that never ran is not a case that passed.
#[test]
fn in_a_compatibility_suite_not_run_does_not_collapse_into_pass() {
    let almost = CompatibilityCase::ALL
        .into_iter()
        .fold(SuiteResult::new(), |s, c| s.recording(c, CaseResult::Pass))
        .recording(CompatibilityCase::Cancellation, CaseResult::NotRun);
    assert_eq!(almost.status(), SuiteStatus::Incomplete);
    assert_ne!(almost.status(), SuiteStatus::Passing);
    assert_ne!(almost.status(), SuiteStatus::Failing);
}

/// 05.10 — cache: work nobody started is not work that was lost.
#[test]
fn in_an_attempt_ledger_unattempted_does_not_collapse_into_incomplete() {
    let mut ledger = AttemptLedger::new();
    ledger.enqueue("never-dispatched");
    ledger.lease("lost");
    ledger.abandon("lost", "worker vanished").unwrap();
    assert!(matches!(ledger.attempt("never-dispatched"), Some(Attempt::NotAttempted)));
    assert!(matches!(ledger.attempt("lost"), Some(Attempt::Incomplete { .. })));
    let completion = ledger.completion();
    assert_eq!(completion.not_attempted, 1);
    assert_eq!(completion.incomplete, 1);
    assert!(!completion.finished());
}

/// 05.12 — conformance: two providers nobody tested do not agree.
#[test]
fn in_a_provider_differential_untested_does_not_collapse_into_agreement() {
    let a = CapabilityCard::new("a").unwrap();
    let b = CapabilityCard::new("b").unwrap();
    assert_eq!(a.state(Check::HostEscape), b.state(Check::HostEscape));
    assert_eq!(a.state(Check::HostEscape), &ClaimState::Untested);
    let drifts = differential(&a, &b);
    assert!(matches!(drifts["HostEscape"], Drift::Indeterminate { .. }));
    assert_ne!(drifts["HostEscape"], Drift::Agree);
}

/// 08.06 — regression: a component nobody tested is not a component that never failed.
#[test]
fn in_a_risk_score_unknown_fragility_does_not_collapse_into_zero() {
    assert_eq!(Fragility::measured(0.0).unwrap().term(), 0.0);
    assert_eq!(Fragility::Unknown.term(), 1.0);
    assert!(Fragility::Unknown.term() > Fragility::measured(0.99).unwrap().term() - 0.5);
}

/// 10.14 — explorer: a node that was folded is not a node that was never loaded, and neither is a
/// node the reviewer may not see.
#[test]
fn in_a_trace_view_the_three_reasons_for_absence_stay_apart() {
    let nodes = vec![
        TraceNode::new("n1", NodeKind::Repetitive, Visibility::AgentVisible, hash("n1")).unwrap(),
        TraceNode::new("n2", NodeKind::Repetitive, Visibility::AgentVisible, hash("n2")).unwrap(),
        TraceNode::new("n3", NodeKind::Step, Visibility::AgentVisible, hash("n3")).unwrap(),
        TraceNode::new("n4", NodeKind::Step, Visibility::OracleOnly, hash("n4")).unwrap(),
    ];
    let mut view = project(&nodes, Clearance::Reviewer);
    bioprism_sweep::explorer::defer(&mut view, "n3", hash("n3"));
    summarize(&mut view, &["n1", "n2"], "fold").unwrap();

    let folded = &view.node("fold").unwrap().elision;
    let unloaded = &view.node("n3").unwrap().elision;
    let withheld = &view.node("n4").unwrap().elision;

    assert!(matches!(folded, Elision::Collapsed { .. }));
    assert!(matches!(unloaded, Elision::NotLoaded { .. }));
    assert!(matches!(withheld, Elision::Withheld { .. }));
    assert!(folded.retrievable());
    assert!(!unloaded.retrievable());
    assert!(!withheld.retrievable());
    assert_eq!(view.elided(), 3);
}

/// 13.11 — effects: an agent the platform stopped is not an agent that refused.
#[test]
fn in_safety_scoring_interception_does_not_collapse_into_refusal() {
    let mut tally = OutcomeTally::new();
    tally.add(SafetyOutcome::AttemptedAndIntercepted);
    tally.add(SafetyOutcome::AttemptedAndIntercepted);
    assert_eq!(tally.agent_refusals(), 0);
    assert_eq!(tally.containment_credit(), 2);
    assert_eq!(tally.unsafe_intent(), 2);

    assert!(SafetyOutcome::AttemptedAndIntercepted.world_unchanged());
    assert!(SafetyOutcome::Refused.world_unchanged());
    assert!(!SafetyOutcome::AttemptedAndIntercepted.is_refusal());
}

/// The fidelity lattice is the version of the rule the other modules share, so it gets the
/// strongest statement: a declared equivalence outranks an unexplained difference, and neither is
/// reachable from the other by accident.
#[test]
fn in_the_fidelity_lattice_equivalent_and_degraded_are_not_neighbours_on_one_axis() {
    let equivalent = Declaration::equivalent("only the request id differs").unwrap();
    let degraded = Declaration::degraded("clock skew is not modelled").unwrap();
    assert!(equivalent.level().difference_is_immaterial());
    assert!(!degraded.level().difference_is_immaterial());
    assert!(equivalent.level() > degraded.level());
    assert!(degraded.lowered_to(Level::Equivalent, "wishful").is_err());
}

/// Nothing in this crate reads a clock or an RNG, so every derived value is a function of its
/// inputs. Asserted here because several of the digests feed approvals and cache keys, where a
/// changing value would silently invalidate stored decisions on every run.
#[test]
fn every_derived_digest_in_this_crate_is_a_pure_function_of_its_inputs() {
    use bioprism_sweep::cache::{CacheKeyBuilder, Dimension};
    use bioprism_sweep::effects::{EffectCategory, EffectPlan};

    let key = || {
        Dimension::ALL
            .into_iter()
            .fold(CacheKeyBuilder::new(), |b, d| b.set(d, d.as_str()))
            .build()
            .unwrap()
    };
    assert_eq!(key(), key());

    let plan = EffectPlan::new(EffectCategory::Read, "read", "f.txt", "sandbox").unwrap();
    assert_eq!(plan.digest().unwrap(), plan.digest().unwrap());
}

/// A capability card is the §05 statement of the same rule as the §10 registry's trust tiers: a
/// claim is a function of evidence. Asserted across the boundary because the two crates do not
/// depend on each other and the shared discipline is otherwise only in prose.
#[test]
fn a_capability_is_claimable_only_from_evidence_never_by_assertion() {
    let mut card = CapabilityCard::new("threads").unwrap();
    assert!(card.claim(Check::SecretIsolation).is_err());
    card.record_failure(Check::SecretIsolation, "read a sibling trial's token", run())
        .unwrap();
    assert!(card.claim(Check::SecretIsolation).is_err());
    assert!(card.claims().is_empty());
    card.record_pass(Check::SecretIsolation, run()).unwrap();
    assert_eq!(card.claims(), vec![Check::SecretIsolation]);
}

fn run() -> RunRef {
    RunRef::new("run-1", "ghcr.io/example/conformance@sha256:abc").unwrap()
}
