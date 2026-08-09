//! Grounding (26.03), acquisition accounting (26.05), estimand declaration (26.09) and
//! reproducibility certification (26.11).

use bioprism_bioevalx::acquisition::{
    Action, AcquisitionKind, Obligation, ReferencePolicy, Trace,
};
use bioprism_bioevalx::error::{AcquisitionError, EstimandError, GroundingError, ReproError};
use bioprism_bioevalx::estimand::{
    ClaimKind, Corroboration, Estimand, Evidentiary, Finding, Identification,
};
use bioprism_bioevalx::grounding::{ClaimState, EdgeKind, Evidence, Grounding, SupportEdge};
use bioprism_bioevalx::repro::{Observed, OutputSpec, Reexecution};
use bioprism_scope::Timestamp;

fn at(rfc3339: &str) -> Timestamp {
    Timestamp::parse(rfc3339).expect("fixture timestamp parses")
}

fn edge(claim: &str, evidence: &str, kind: EdgeKind) -> SupportEdge {
    SupportEdge {
        claim: claim.into(),
        evidence: evidence.into(),
        kind,
    }
}

#[test]
fn a_claim_with_both_supporting_and_contradicting_evidence_is_contested_not_mostly_supported() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("first claim");
    for id in ["fish", "ngs", "ihc"] {
        graph
            .evidence(Evidence::declared(id, at("2026-01-01T00:00:00Z")).resolving_to("d"))
            .expect("distinct");
    }
    graph.link(edge("amplified", "fish", EdgeKind::Supports)).expect("declared");
    graph.link(edge("amplified", "ngs", EdgeKind::Supports)).expect("declared");
    graph
        .link(edge("amplified", "ihc", EdgeKind::Contradicts))
        .expect("declared");

    assert_eq!(graph.state("amplified"), Some(ClaimState::Contested));
    let census = graph.census();
    assert_eq!(census.contested, 1);
    assert_eq!(census.supported, 0);
    assert!(!census.fully_grounded());
}

#[test]
fn support_from_an_unresolved_locator_is_asserted_rather_than_shown() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("first claim");
    graph
        .evidence(Evidence::declared("fish", at("2026-01-01T00:00:00Z")))
        .expect("distinct");
    graph.link(edge("amplified", "fish", EdgeKind::Supports)).expect("declared");

    assert_eq!(graph.state("amplified"), Some(ClaimState::SupportUnverified));
}

#[test]
fn a_pile_of_adjacent_citations_does_not_ground_a_claim() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("first claim");
    for id in ["review-a", "review-b", "review-c"] {
        graph
            .evidence(Evidence::declared(id, at("2026-01-01T00:00:00Z")).resolving_to("d"))
            .expect("distinct");
        graph.link(edge("amplified", id, EdgeKind::Adjacent)).expect("declared");
    }

    assert_eq!(graph.state("amplified"), Some(ClaimState::Unsupported));
    assert_eq!(graph.census().adjacent_citations, 3);
}

#[test]
fn staleness_is_measured_against_the_declared_freeze_and_not_against_a_clock() {
    let mut graph = Grounding::new();
    graph.claim("c").expect("first claim");
    graph
        .evidence(Evidence::declared("fresh", at("2026-01-01T00:00:00Z")))
        .expect("distinct");
    graph
        .evidence(Evidence::declared("edited", at("2026-06-01T00:00:00Z")))
        .expect("distinct");

    assert_eq!(graph.stale_against(at("2026-03-01T00:00:00Z")), vec!["edited"]);
    assert!(graph.stale_against(at("2026-12-01T00:00:00Z")).is_empty());
}

#[test]
fn an_edge_to_an_undeclared_claim_is_refused() {
    let mut graph = Grounding::new();
    graph
        .evidence(Evidence::declared("fish", at("2026-01-01T00:00:00Z")))
        .expect("distinct");

    assert!(matches!(
        graph.link(edge("ghost", "fish", EdgeKind::Supports)),
        Err(GroundingError::UnknownClaim(_))
    ));
}

#[test]
fn a_derived_artifact_with_no_specimen_ancestor_is_reported() {
    let mut graph = Grounding::new();
    graph
        .evidence(
            Evidence::declared("matrix", at("2026-01-01T00:00:00Z"))
                .with_lineage(vec!["specimen-1".into()]),
        )
        .expect("distinct");
    graph
        .evidence(Evidence::declared("orphan", at("2026-01-01T00:00:00Z")))
        .expect("distinct");

    assert_eq!(graph.lineage_gaps(), vec!["orphan"]);
}

#[test]
fn an_acquisition_closing_an_obligation_nobody_opened_is_refused() {
    let mut trace = Trace::against(vec![Obligation::required("subtype")]);

    let outcome = trace.perform(
        Action::new("order-panel", AcquisitionKind::Assay, 10).closing("unrelated"),
    );

    assert!(matches!(
        outcome,
        Err(AcquisitionError::UnopenedObligation { .. })
    ));
}

#[test]
fn a_retrieval_that_closes_nothing_still_open_is_redundant() {
    let mut trace = Trace::against(vec![Obligation::required("subtype")]);
    trace
        .perform(Action::new("first", AcquisitionKind::Retrieval, 3).closing("subtype"))
        .expect("obligation is open");
    trace
        .perform(Action::new("second", AcquisitionKind::Retrieval, 3).closing("subtype"))
        .expect("obligation was declared");

    let redundant = trace.redundant();
    assert_eq!(redundant.len(), 1);
    assert_eq!(redundant[0].id, "second");
}

#[test]
fn assays_ordered_after_the_decision_was_already_admissible_are_unnecessary() {
    let mut trace = Trace::against(vec![Obligation::required("subtype")]);
    trace
        .perform(Action::new("panel", AcquisitionKind::Assay, 40).closing("subtype"))
        .expect("obligation is open");
    trace
        .perform(Action::new("methylation", AcquisitionKind::Assay, 60))
        .expect("declared");

    assert!(trace.admissible());
    assert_eq!(
        trace.unnecessary().iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        vec!["methylation"]
    );
}

#[test]
fn cheap_evidence_ahead_of_the_decisive_source_shows_up_as_deferred_cost() {
    let mut trace = Trace::against(vec![
        Obligation::required("subtype"),
        Obligation::optional("context"),
    ]);
    trace
        .perform(Action::new("read-notes", AcquisitionKind::Metadata, 2).closing("context"))
        .expect("obligation is open");
    trace
        .perform(Action::new("search", AcquisitionKind::Retrieval, 5))
        .expect("declared");
    trace
        .perform(Action::new("panel", AcquisitionKind::Assay, 40).closing("subtype"))
        .expect("obligation is open");

    assert_eq!(trace.deferred_decisive(), Some(7));
}

#[test]
fn closing_every_optional_obligation_does_not_make_a_decision_admissible() {
    let mut trace = Trace::against(vec![
        Obligation::required("subtype"),
        Obligation::optional("context"),
    ]);
    trace
        .perform(Action::new("read-notes", AcquisitionKind::Metadata, 2).closing("context"))
        .expect("obligation is open");

    assert!(!trace.admissible());
    assert_eq!(trace.open().len(), 1);
    assert!(trace.open()[0].required);
}

#[test]
fn regret_without_a_named_reference_policy_refuses() {
    let trace = Trace::against(vec![Obligation::required("subtype")]);

    assert!(matches!(
        trace.regret_against(None),
        Err(AcquisitionError::NoReferencePolicy)
    ));
}

#[test]
fn regret_against_a_policy_that_never_became_admissible_is_not_like_for_like() {
    let mut trace = Trace::against(vec![Obligation::required("subtype")]);
    trace
        .perform(Action::new("panel", AcquisitionKind::Assay, 40).closing("subtype"))
        .expect("obligation is open");

    let regret = trace
        .regret_against(Some(&ReferencePolicy::new("random-acquisition", 10, false)))
        .expect("policy named");

    assert_eq!(regret.cost_difference, 30);
    assert!(!regret.like_for_like());
}

#[test]
fn an_estimand_missing_any_of_the_five_elements_cannot_be_constructed() {
    for (index, name) in ["intervention", "comparator", "unit", "outcome", "horizon"]
        .iter()
        .enumerate()
    {
        let mut parts = ["drug", "placebo", "patient", "survival", "24 months"];
        parts[index] = "  ";
        let outcome = Estimand::declare(parts[0], parts[1], parts[2], parts[3], parts[4], "cohort");
        match outcome {
            Err(EstimandError::MissingElement(missing)) => assert_eq!(missing, *name),
            other => panic!("expected {name} to be required, got {other:?}"),
        }
    }
}

#[test]
fn a_simulator_cannot_corroborate_itself_into_real_world_truth() {
    let estimand = Estimand::declare("knockdown", "control", "cell line", "viability", "72h", "twin")
        .expect("all five declared");
    let mut finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::ModelConditional {
            model: "pdac-twin-v2".into(),
        },
    );

    let outcome = finding.promote(Corroboration {
        source: "pdac-twin-v2".into(),
        kind: ClaimKind::Intervention,
        detail: "ran it again".into(),
    });

    assert!(matches!(
        outcome,
        Err(EstimandError::NoAutomaticPromotion { .. })
    ));
    assert!(finding.still_model_conditional());
}

#[test]
fn a_model_conditional_finding_carries_its_qualifier_into_the_sentence_it_licenses() {
    let estimand = Estimand::declare("knockdown", "control", "cell line", "viability", "72h", "twin")
        .expect("all five declared");
    let mut finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::ModelConditional {
            model: "pdac-twin-v2".into(),
        },
    );

    assert!(finding.claim_language().contains("model-conditional on pdac-twin-v2"));
    assert!(finding.claim_language().contains("identification not assessed"));

    finding
        .promote(Corroboration {
            source: "GSE-14520".into(),
            kind: ClaimKind::Intervention,
            detail: "replicated in an external cohort".into(),
        })
        .expect("an external source may corroborate");

    assert!(!finding.still_model_conditional());
    assert!(!finding.claim_language().contains("model-conditional"));
}

#[test]
fn an_association_finding_cannot_be_rendered_in_interventional_language() {
    let estimand = Estimand::declare("high expression", "low expression", "patient", "survival", "5y", "TCGA")
        .expect("all five declared");
    let finding = Finding::new(
        estimand,
        ClaimKind::Association,
        Evidentiary::Observational {
            dataset: "TCGA".into(),
        },
    );

    let sentence = finding.claim_language();
    assert!(sentence.contains("is associated with"));
    assert!(!sentence.contains("changes"));
}

#[test]
fn naming_an_identification_strategy_is_not_the_same_as_probing_it() {
    let declared = Identification::Declared {
        strategy: "backdoor adjustment".into(),
        assumptions: vec!["no unmeasured confounding".into()],
    };

    assert_eq!(declared.assumptions().len(), 1);
    assert!(!declared.was_probed());
    assert!(!Identification::NotAssessed.was_probed());
}

#[test]
fn a_reproducibility_certificate_refuses_to_support_a_biological_claim() {
    let mut rerun = Reexecution::declaring("fig2", true, vec![OutputSpec::exact("fig2.png")])
        .expect("distinct outputs");
    rerun
        .observe(
            "fig2.png",
            Observed::Digests {
                original: "aa".into(),
                rerun: "aa".into(),
            },
        )
        .expect("first observation");
    let certificate = rerun.certify().expect("outputs were declared");

    assert!(certificate.reproduced());
    assert!(matches!(
        certificate.supports("MGMT methylation predicts response"),
        Err(ReproError::NotAValidityClaim(_))
    ));
}

#[test]
fn the_first_divergence_is_reported_in_declaration_order_not_as_a_match_rate() {
    let mut rerun = Reexecution::declaring(
        "pipeline",
        true,
        vec![
            OutputSpec::exact("cohort.json"),
            OutputSpec::numeric("hazard-ratio", 0.01).expect("finite tolerance"),
            OutputSpec::exact("table1.csv"),
        ],
    )
    .expect("distinct outputs");
    rerun
        .observe(
            "cohort.json",
            Observed::Digests {
                original: "aa".into(),
                rerun: "bb".into(),
            },
        )
        .expect("first observation");
    rerun
        .observe(
            "hazard-ratio",
            Observed::Numbers {
                original: 1.4,
                rerun: 1.9,
            },
        )
        .expect("first observation");
    rerun
        .observe(
            "table1.csv",
            Observed::Digests {
                original: "cc".into(),
                rerun: "cc".into(),
            },
        )
        .expect("first observation");

    let certificate = rerun.certify().expect("outputs were declared");
    let (id, _) = certificate.first_divergence().expect("something diverged");

    assert_eq!(id, "cohort.json");
}

#[test]
fn a_rerun_cannot_improve_its_result_by_producing_fewer_outputs() {
    let mut rerun = Reexecution::declaring(
        "pipeline",
        true,
        vec![OutputSpec::exact("a"), OutputSpec::exact("b")],
    )
    .expect("distinct outputs");
    rerun
        .observe(
            "a",
            Observed::Digests {
                original: "aa".into(),
                rerun: "aa".into(),
            },
        )
        .expect("first observation");

    let certificate = rerun.certify().expect("outputs were declared");

    assert!(!certificate.reproduced());
    assert_eq!(certificate.missing(), vec!["b"]);
}

#[test]
fn reproducing_only_under_a_pinned_environment_is_not_portability() {
    let mut rerun = Reexecution::declaring("p", true, vec![OutputSpec::exact("a")])
        .expect("distinct outputs");
    rerun
        .observe(
            "a",
            Observed::Digests {
                original: "aa".into(),
                rerun: "aa".into(),
            },
        )
        .expect("first observation");
    let certificate = rerun.certify().expect("outputs were declared");

    assert!(certificate.reproduced());
    assert!(!certificate.portability_demonstrated());
}

#[test]
fn a_numeric_output_needs_a_finite_non_negative_tolerance() {
    assert!(matches!(
        OutputSpec::numeric("x", -1.0),
        Err(ReproError::BadTolerance { .. })
    ));
    assert!(matches!(
        OutputSpec::numeric("x", f64::NAN),
        Err(ReproError::BadTolerance { .. })
    ));
}
