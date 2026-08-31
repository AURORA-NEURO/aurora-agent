//! Grounding (26.03), acquisition accounting (26.05), estimand declaration (26.09) and
//! reproducibility certification (26.11).

use bioprism_bioevalx::acquisition::{AcquisitionKind, Action, Obligation, ReferencePolicy, Trace};
use bioprism_bioevalx::error::{AcquisitionError, EstimandError, GroundingError, ReproError};
use bioprism_bioevalx::estimand::{
    ClaimKind, Corroboration, Estimand, Evidentiary, Finding, Identification,
};
use bioprism_bioevalx::grounding::{ClaimState, EdgeKind, Evidence, Grounding, SupportEdge};
use bioprism_bioevalx::repro::{Observed, OutputKind, OutputSpec, Reexecution};
use bioprism_scope::Timestamp;

fn at(rfc3339: &str) -> Timestamp {
    Timestamp::parse(rfc3339).expect("fixture timestamp parses")
}

fn trace(obligations: Vec<Obligation>) -> Trace {
    Trace::against(obligations).expect("valid obligation set")
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
    graph
        .link(edge("amplified", "fish", EdgeKind::Supports))
        .expect("declared");
    graph
        .link(edge("amplified", "ngs", EdgeKind::Supports))
        .expect("declared");
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
fn malformed_claim_and_locator_identifiers_are_refused_at_admission() {
    let mut graph = Grounding::new();
    let claim_refusal = graph
        .claim(" ")
        .expect_err("a blank claim id cannot anchor graph edges");
    assert!(matches!(claim_refusal, GroundingError::InvalidClaim { .. }));

    let evidence_refusal = graph
        .evidence(
            Evidence::declared("fish", at("2026-01-01T00:00:00Z"))
                .resolving_to(" "),
        )
        .expect_err("a resolved locator must carry a digest");
    assert!(matches!(
        evidence_refusal,
        GroundingError::InvalidEvidence { .. }
    ));
}

#[test]
fn duplicate_lineage_entries_are_refused_instead_of_claiming_a_clean_ancestry_chain() {
    let mut graph = Grounding::new();
    let refusal = graph
        .evidence(
            Evidence::declared("matrix", at("2026-01-01T00:00:00Z"))
                .with_lineage(vec!["specimen-1".into(), "specimen-1".into()]),
        )
        .expect_err("one ancestor must not be counted twice");

    assert!(matches!(
        refusal,
        GroundingError::InvalidEvidence { .. }
    ));
}

#[test]
fn duplicate_edges_are_refused_instead_of_inflating_adjacent_citation_counts() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("valid claim");
    graph
        .evidence(Evidence::declared("review", at("2026-01-01T00:00:00Z")))
        .expect("valid evidence");
    let citation = edge("amplified", "review", EdgeKind::Adjacent);
    graph.link(citation.clone()).expect("first edge");

    let refusal = graph
        .link(citation)
        .expect_err("the same typed edge is one citation, not two");

    assert!(matches!(refusal, GroundingError::DuplicateEdge { .. }));
    assert_eq!(graph.census().adjacent_citations, 1);
}

#[test]
fn support_from_an_unresolved_locator_is_asserted_rather_than_shown() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("first claim");
    graph
        .evidence(Evidence::declared("fish", at("2026-01-01T00:00:00Z")))
        .expect("distinct");
    graph
        .link(edge("amplified", "fish", EdgeKind::Supports))
        .expect("declared");

    assert_eq!(
        graph.state("amplified"),
        Some(ClaimState::SupportUnverified)
    );
}

#[test]
fn a_pile_of_adjacent_citations_does_not_ground_a_claim() {
    let mut graph = Grounding::new();
    graph.claim("amplified").expect("first claim");
    for id in ["review-a", "review-b", "review-c"] {
        graph
            .evidence(Evidence::declared(id, at("2026-01-01T00:00:00Z")).resolving_to("d"))
            .expect("distinct");
        graph
            .link(edge("amplified", id, EdgeKind::Adjacent))
            .expect("declared");
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

    assert_eq!(
        graph.stale_against(at("2026-03-01T00:00:00Z")),
        vec!["edited"]
    );
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
    let mut trace = trace(vec![Obligation::required("subtype")]);

    let outcome =
        trace.perform(Action::new("order-panel", AcquisitionKind::Assay, 10).closing("unrelated"));

    assert!(matches!(
        outcome,
        Err(AcquisitionError::UnopenedObligation { .. })
    ));
}

#[test]
fn duplicate_or_malformed_obligations_are_refused_before_a_trace_exists() {
    let duplicate = Trace::against(vec![
        Obligation::required("subtype"),
        Obligation::optional("subtype"),
    ])
    .expect_err("one obligation id cannot carry two admissibility meanings");
    assert!(matches!(
        duplicate,
        AcquisitionError::DuplicateObligation(id) if id == "subtype"
    ));

    let malformed = Trace::against(vec![Obligation::required(" ")])
        .expect_err("an obligation id is an identity, not whitespace");
    assert!(matches!(
        malformed,
        AcquisitionError::InvalidObligation { .. }
    ));
}

#[test]
fn malformed_actions_are_refused_and_cost_totals_saturate_instead_of_wrapping() {
    let mut trace = trace(vec![Obligation::required("subtype")]);
    let refusal = trace
        .perform(Action::new(" ", AcquisitionKind::Assay, u64::MAX))
        .expect_err("an action id is required even when it closes nothing");
    assert!(matches!(refusal, AcquisitionError::InvalidAction { .. }));

    trace
        .perform(Action::new("first", AcquisitionKind::Assay, u64::MAX))
        .expect("valid action");
    trace
        .perform(Action::new("second", AcquisitionKind::Assay, u64::MAX))
        .expect("valid action");
    assert_eq!(trace.cost(), u64::MAX);
    assert_eq!(
        trace.cost_by_kind().get(&AcquisitionKind::Assay),
        Some(&u64::MAX)
    );
}

#[test]
fn a_retrieval_that_closes_nothing_still_open_is_redundant() {
    let mut trace = trace(vec![Obligation::required("subtype")]);
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
    let mut trace = trace(vec![Obligation::required("subtype")]);
    trace
        .perform(Action::new("panel", AcquisitionKind::Assay, 40).closing("subtype"))
        .expect("obligation is open");
    trace
        .perform(Action::new("methylation", AcquisitionKind::Assay, 60))
        .expect("declared");

    assert!(trace.admissible());
    assert_eq!(
        trace
            .unnecessary()
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec!["methylation"]
    );
}

#[test]
fn cheap_evidence_ahead_of_the_decisive_source_shows_up_as_deferred_cost() {
    let mut trace = trace(vec![
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
    let mut trace = trace(vec![
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
    let trace = trace(vec![Obligation::required("subtype")]);

    assert!(matches!(
        trace.regret_against(None),
        Err(AcquisitionError::NoReferencePolicy)
    ));
}

#[test]
fn regret_against_a_policy_that_never_became_admissible_is_not_like_for_like() {
    let mut trace = trace(vec![Obligation::required("subtype")]);
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
fn a_reference_policy_needs_a_bounded_identity_before_regret_is_reported() {
    let trace = trace(vec![Obligation::required("subtype")]);

    let refusal = trace
        .regret_against(Some(&ReferencePolicy::new(" ", 0, false)))
        .expect_err("regret without a named baseline is not interpretable");

    assert!(matches!(
        refusal,
        AcquisitionError::InvalidReferencePolicy { .. }
    ));
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
fn an_estimand_rejects_unbounded_or_control_text() {
    let too_long = "x".repeat(257);
    let cases = [
        ("drug\n", "control", "patient", "survival", "24 months", "cohort"),
        ("drug", "control", "patient", &too_long, "24 months", "cohort"),
    ];

    for (intervention, comparator, unit, outcome, horizon, scope) in cases {
        assert!(matches!(
            Estimand::declare(intervention, comparator, unit, outcome, horizon, scope),
            Err(EstimandError::InvalidField { .. })
        ));
    }
}

#[test]
fn transport_rejects_malformed_targets_before_scope_lookup() {
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
    .expect("all five declared");

    assert!(matches!(
        estimand.transport_to(" external\n", &std::collections::BTreeSet::new()),
        Err(EstimandError::InvalidField { field, .. }) if field == "target"
    ));
}

#[test]
fn a_simulator_cannot_corroborate_itself_into_real_world_truth() {
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
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
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
    .expect("all five declared");
    let mut finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::ModelConditional {
            model: "pdac-twin-v2".into(),
        },
    );

    assert!(finding
        .claim_language()
        .contains("model-conditional on pdac-twin-v2"));
    assert!(finding
        .claim_language()
        .contains("identification not assessed"));

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
fn corroboration_kind_and_source_identity_are_checked() {
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
    .expect("all five declared");
    let mut finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::ModelConditional {
            model: "pdac-twin-v2".into(),
        },
    );
    let corroboration = Corroboration {
        source: "GSE-14520".into(),
        kind: ClaimKind::Association,
        detail: "replicated in an external cohort".into(),
    };

    assert!(matches!(
        finding.promote(corroboration),
        Err(EstimandError::InvalidCorroboration { .. })
    ));

    let valid = Corroboration {
        source: "GSE-14520".into(),
        kind: ClaimKind::Intervention,
        detail: "replicated in an external cohort".into(),
    };
    finding.promote(valid.clone()).expect("first source is accepted");
    assert!(matches!(
        finding.promote(valid),
        Err(EstimandError::DuplicateCorroboration { source_id })
            if source_id == "GSE-14520"
    ));
}

#[test]
fn invalid_identification_cannot_enter_a_finding() {
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
    .expect("all five declared");
    let finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::Experimental {
            study: "study-1".into(),
        },
    );

    let result = finding.identified_by(Identification::Declared {
        strategy: "backdoor adjustment".into(),
        assumptions: vec!["no unmeasured confounding".into(), "no unmeasured confounding".into()],
    });
    assert!(matches!(
        result,
        Err(EstimandError::InvalidIdentification { .. })
    ));
}

#[test]
fn malformed_evidence_source_is_refused_on_promotion() {
    let estimand = Estimand::declare(
        "knockdown",
        "control",
        "cell line",
        "viability",
        "72h",
        "twin",
    )
    .expect("all five declared");
    let mut finding = Finding::new(
        estimand,
        ClaimKind::Intervention,
        Evidentiary::ModelConditional {
            model: "pdac-twin-v2\n".into(),
        },
    );

    assert!(matches!(
        finding.promote(Corroboration {
            source: "GSE-14520".into(),
            kind: ClaimKind::Intervention,
            detail: "replicated in an external cohort".into(),
        }),
        Err(EstimandError::InvalidField { field, .. }) if field == "model"
    ));
}

#[test]
fn an_association_finding_cannot_be_rendered_in_interventional_language() {
    let estimand = Estimand::declare(
        "high expression",
        "low expression",
        "patient",
        "survival",
        "5y",
        "TCGA",
    )
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
    let mut rerun =
        Reexecution::declaring("p", true, vec![OutputSpec::exact("a")]).expect("distinct outputs");
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

#[test]
fn a_rerun_only_accepts_declared_outputs_with_valid_observations() {
    let mut rerun =
        Reexecution::declaring("pipeline", true, vec![OutputSpec::exact("result")])
            .expect("valid declaration");

    assert!(matches!(
        rerun.observe(
            "other",
            Observed::Digests {
                original: "aa".into(),
                rerun: "aa".into(),
            }
        ),
        Err(ReproError::UnknownOutput(output)) if output == "other"
    ));
    assert!(matches!(
        rerun.observe(
            "result",
            Observed::Digests {
                original: " ".into(),
                rerun: "aa".into(),
            }
        ),
        Err(ReproError::InvalidObservation { output_id, .. }) if output_id == "result"
    ));
    assert!(matches!(
        rerun.observe(
            "result",
            Observed::Numbers {
                original: f64::INFINITY,
                rerun: 1.0,
            }
        ),
        Err(ReproError::InvalidObservation { output_id, .. }) if output_id == "result"
    ));
}

#[test]
fn a_reproducibility_declaration_rejects_invalid_persistable_specs() {
    let malformed = OutputSpec {
        id: "result".into(),
        kind: OutputKind::Exact,
        tolerance: 0.1,
    };
    assert!(matches!(
        Reexecution::declaring("pipeline", true, vec![malformed]),
        Err(ReproError::InvalidOutput { output_id, .. }) if output_id == "result"
    ));
    assert!(matches!(
        Reexecution::declaring(" pipeline", true, vec![OutputSpec::exact("result")]),
        Err(ReproError::InvalidOutput { output_id, .. }) if output_id == " pipeline"
    ));
}

#[test]
fn serialized_reproducibility_receipts_are_outputs_not_input_state() {
    let mut rerun =
        Reexecution::declaring("pipeline", true, vec![OutputSpec::exact("result")])
            .expect("valid declaration");
    rerun
        .observe(
            "result",
            Observed::Digests {
                original: "aa".into(),
                rerun: "aa".into(),
            },
        )
        .expect("valid observation");
    let certificate = rerun.certify().expect("valid receipt");
    let encoded = serde_json::to_value(&certificate).expect("certificate serializes");

    assert_eq!(encoded["workflow"], "pipeline");
    assert!(encoded["verdicts"].is_array());
}

#[test]
fn persisted_reexecution_requests_are_revalidated_on_input() {
    let rerun =
        Reexecution::declaring("pipeline", true, vec![OutputSpec::exact("result")])
            .expect("valid declaration");
    let mut encoded = serde_json::to_value(&rerun).expect("reexecution serializes");
    encoded["observations"] = serde_json::json!([[
        "undeclared",
        {
            "observation": "digests",
            "original": "aa",
            "rerun": "aa"
        }
    ]]);

    let parsed: Result<Reexecution, _> = serde_json::from_value(encoded);
    assert!(parsed.is_err());
}
