//! The five modules are one chain, and these tests exercise the joins between them.
//!
//! Each unit test in `src/` holds one rule inside one module. What those cannot show is the
//! property the crate is actually for: that a claim at the end of the chain cannot be stronger than
//! the weakest link behind it. A causal claim rests on a predeclared analysis, which rests on a
//! plan sealed before the results, which was scored by an evaluator whose review named the
//! dimensions it covered — and a break anywhere is visible at the claim.

use bioprism_atlas::ClaimTier;
use bioprism_governance::SchemaVersion;
use bioprism_ids::ContentHash;
use bioprism_registry::TrustTier;
use bioprism_stewardship::{
    full_corpus, AccessLedger, Actor, AnalysisPlan, BoundaryError, ClaimClass, ClaimError,
    ClaimRegister, ClaimSentence, ClaimStatus, ClaimSubject, ClusteringUnit, ComparisonAssertion,
    ConfirmatoryFinding, CrossingTrigger, DataContract, DerivativeRights, DimensionScopedApproval,
    DomainPackCard, DomainReview, Epoch, EscalationDocket, EvaluatorRevision, ExploratoryFinding,
    Finding, ObservedResults, PermittedResearchScope, ProcessingPath, PromotionBasis,
    PublicationRight, PurposeTag, ReviewDimension, ReviewError, ReviewRecord, SealedPlan,
    TrialDisposition,
};

fn approved_evaluator() -> (EvaluatorRevision, DimensionScopedApproval) {
    let revision = EvaluatorRevision::new(
        "divergence-oracle",
        SchemaVersion::parse("1.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v1"),
        false,
    );
    let mut record = ReviewRecord::new(
        revision.clone(),
        Actor::author("pack-author"),
        Actor::independent_reviewer("reviewer-1"),
    )
    .against(full_corpus(4));
    for dimension in ReviewDimension::mandatory_for(false) {
        record = record
            .finding(dimension, Finding::passed("examined against the corpus"))
            .unwrap();
    }
    let approval = record.conclude().unwrap();
    (revision, approval)
}

fn confirmed(metric: &str) -> ConfirmatoryFinding {
    let sealed = AnalysisPlan::new(metric, ClusteringUnit::ParentTask, 40, "after 400 trials")
        .seal(Epoch(2))
        .unwrap();
    let results = ObservedResults::new(Epoch(6))
        .measuring(metric, 0.19)
        .counting(TrialDisposition::Scored, 400);
    sealed.confirm(&results, metric).unwrap()
}

fn causal_sentence() -> ClaimSentence {
    ClaimSentence::new(
        ClaimClass::ResearchCausal,
        ClaimSubject::new("pack-a", TrustTier::Gold, "arch-a", "run-9"),
        Actor::author("lab-a"),
        "structural decision cells, 40 independent parents",
        "first-divergence-rate",
    )
    .with_uncertainty("95% interval clustered at parent task")
    .with_resource_policy("8k tokens, 3 retries")
    .limited_by("single site")
}

#[test]
fn the_whole_chain_holds_from_evaluator_review_to_a_promoted_claim() {
    let (revision, approval) = approved_evaluator();
    assert!(approval.carries_to(&revision).is_ok());
    assert!(approval.covers(ReviewDimension::RewardWithoutIntent));

    let finding = confirmed("first-divergence-rate");
    let claim = causal_sentence()
        .issue(
            ClaimTier::ControlledHiddenMultiSite,
            Some(&finding),
            None,
            Epoch(7),
        )
        .unwrap();

    let mut register = ClaimRegister::new();
    let id = register.publish(claim).unwrap();
    register
        .promote(
            &id,
            &Actor::independent_reviewer("hub"),
            PromotionBasis::Reproducibility,
            Epoch(8),
        )
        .unwrap();
    assert_eq!(register.status(&id), Some(&ClaimStatus::Published));
    assert!(register.sentence(&id).unwrap().limitations.len() == 1);
}

#[test]
fn removing_the_predeclaration_removes_the_causal_claim() {
    assert!(matches!(
        causal_sentence().issue(ClaimTier::ControlledHiddenMultiSite, None, None, Epoch(7)),
        Err(ClaimError::CausalClaimWithoutPredeclaration)
    ));
    let exploratory = ExploratoryFinding::new(
        "first-divergence-rate",
        0.19,
        Epoch(6),
        "metric chosen late",
    );
    assert!(!exploratory.is_confirmatory());
}

#[test]
fn a_scoring_change_invalidates_the_review_the_published_claim_rested_on() {
    let (_, approval) = approved_evaluator();
    let rescored = EvaluatorRevision::new(
        "divergence-oracle",
        SchemaVersion::parse("2.0.0").unwrap(),
        ContentHash::of_bytes(b"scoring-v2"),
        false,
    );
    assert!(matches!(
        approval.carries_to(&rescored),
        Err(ReviewError::ApprovalDoesNotCarry { .. })
    ));
}

#[test]
fn an_admitted_pack_still_cannot_release_through_an_open_crossing() {
    let card = DomainPackCard::new(
        "neuro-pack",
        Actor::author("author-1"),
        "public derivatives",
        "structural MRI",
        "adult volunteers",
        "format workflow reproducibility",
    )
    .limited_by("no clinical population")
    .scoped_to(PermittedResearchScope::ImagingFormatWorkflow)
    .reviewed_by(DomainReview::new(
        Actor::domain_expert("neuroradiologist-1"),
        Epoch(1),
    ));
    let standing = card.admit().unwrap();
    assert!(standing.permits(PermittedResearchScope::ImagingFormatWorkflow));

    let docket = EscalationDocket::raise(
        "d-1",
        CrossingTrigger::OutputEntersCarePathway,
        "a care team asked for the cohort summary",
        Epoch(3),
    );
    assert!(matches!(
        docket.release_permitted(),
        Err(BoundaryError::CrossingOpen { .. })
    ));
}

#[test]
fn a_refused_crossing_stays_refused_across_the_whole_ledger() {
    let mut docket = EscalationDocket::raise(
        "d-2",
        CrossingTrigger::ScopeExtensionRequested,
        "extend the pack to prognosis",
        Epoch(3),
    );
    let expert = Actor::domain_expert("clinical-lead");
    docket
        .refuse(&expert, "prognosis is excluded", Epoch(4))
        .unwrap();
    let text = serde_json::to_string(&docket).unwrap();
    let restored: EscalationDocket = serde_json::from_str(&text).unwrap();
    assert!(restored.disposition().is_refused());
    assert!(matches!(
        restored.release_permitted(),
        Err(BoundaryError::HumanRefusalIsFinal { .. })
    ));
}

#[test]
fn withdrawing_the_data_a_claim_rests_on_produces_a_correction_not_an_erasure() {
    let mut contract = DataContract::new(
        "contract-1",
        Actor::steward("steward-1"),
        "consented volunteers",
        "site-a export",
        "restricted",
        [PurposeTag::new("benchmark-evaluation")],
        [PurposeTag::new("model-training")],
    )
    .unwrap()
    .releasing("aggregate_score")
    .with_rights(DerivativeRights::permitting_derivatives())
    .publishing(PublicationRight::AggregateOnly);

    let grant = contract
        .grant(
            "grant-1",
            Actor::independent_reviewer("lab-a"),
            [PurposeTag::new("benchmark-evaluation")],
            ["aggregate_score".to_string()],
            Epoch(1),
            Epoch(20),
        )
        .unwrap();
    let grant_id = grant.id.clone();
    let mut ledger = AccessLedger::new();
    ledger.hold(grant);
    ledger
        .record(
            &contract,
            &grant_id,
            &Actor::independent_reviewer("lab-a"),
            &PurposeTag::new("benchmark-evaluation"),
            ProcessingPath::new("arch-a", "model-a"),
            Epoch(5),
            vec![ContentHash::of_bytes(b"result-bundle")],
            Some("leaderboard-claim-1".into()),
        )
        .unwrap();

    let assessment = ledger.withdraw(&mut contract, Epoch(9)).unwrap();
    assert_eq!(assessment.corrections().len(), 1);
    assert_eq!(ledger.audit(&contract.id).len(), 1);
    assert!(contract.withdrawn_at().is_some());
}

#[test]
fn a_disputed_claim_survives_the_dispute_and_the_history_records_both() {
    let finding = confirmed("first-divergence-rate");
    let claim = causal_sentence()
        .issue(
            ClaimTier::ControlledHiddenMultiSite,
            Some(&finding),
            None,
            Epoch(7),
        )
        .unwrap();
    let mut register = ClaimRegister::new();
    let id = register.publish(claim).unwrap();
    let challenger = Actor::independent_reviewer("lab-b");
    register
        .dispute(&id, &challenger, "the split leaks parents", Epoch(8))
        .unwrap();
    assert!(matches!(
        register.promote(&id, &challenger, PromotionBasis::Efficiency, Epoch(9)),
        Err(ClaimError::ClaimDisputed { .. })
    ));
    register
        .resolve_dispute(
            &id,
            &Actor::independent_reviewer("council"),
            "no leak found",
            Epoch(10),
        )
        .unwrap();
    assert_eq!(register.history(&id).unwrap().len(), 3);
    assert!(register
        .promote(&id, &challenger, PromotionBasis::Efficiency, Epoch(11))
        .is_ok());
}

#[test]
fn a_generality_claim_needs_more_domains_than_a_comparative_one() {
    let base = || {
        ClaimSentence::new(
            ClaimClass::PublicBenchmarkResult,
            ClaimSubject::new("pack-a", TrustTier::Reviewed, "arch-a", "run-1"),
            Actor::author("lab-a"),
            "public worlds",
            "recall",
        )
        .with_uncertainty("95% interval")
        .with_resource_policy("8k tokens")
        .limited_by("one modality")
    };
    assert!(base()
        .asserting(ComparisonAssertion::BetterThan {
            baseline: "bm25".into()
        })
        .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(3))
        .is_ok());
    assert!(matches!(
        base()
            .asserting(ComparisonAssertion::Generally {
                validated_domains: vec!["structural".into()]
            })
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(3)),
        Err(ClaimError::GeneralityWithoutBreadth { .. })
    ));
}

#[test]
fn every_artifact_the_crate_publishes_round_trips_or_deliberately_does_not() {
    let (_, approval) = approved_evaluator();
    let approval_json = serde_json::to_string(&approval).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&approval_json).is_ok());

    let sealed = AnalysisPlan::new("m", ClusteringUnit::World, 10, "after 100")
        .seal(Epoch(1))
        .unwrap();
    let plan_json = serde_json::to_string(&sealed).unwrap();
    let restored: SealedPlan = serde_json::from_str(&plan_json).unwrap();
    assert!(restored.verify().is_ok());
    assert_eq!(restored.digest(), sealed.digest());
}

#[test]
fn the_chain_is_deterministic_across_two_identical_constructions() {
    let a = confirmed("first-divergence-rate");
    let b = confirmed("first-divergence-rate");
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    let (_, first) = approved_evaluator();
    let (_, second) = approved_evaluator();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}
