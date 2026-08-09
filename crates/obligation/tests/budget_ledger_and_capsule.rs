use bioprism_obligation::{
    BioContextCapsule, BudgetError, BudgetPlan, BudgetRequest, CapsuleError, CertificateCheck,
    ClosureReport, ClosureSelection, ClosureViolation, EstimationMethod, LedgerError, Obligation,
    ObligationGraph, ObligationState, OmissionLedger, OmissionReason, OmittedCandidate,
    ProtectedClass, RejectionReason, RelevanceIndex, RelevanceRecord, Representation, StateRecord,
    StopDecision, SufficiencyCertificate, SufficiencyInputs, SufficiencyStatus,
    TokenBudgetController, TokenEstimate,
};
use bioprism_scope::Timestamp;
use bioprism_section::{InfluenceClass, Layer};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("fixture timestamp parses")
}

fn satisfied() -> StateRecord {
    StateRecord::new(
        ObligationState::Satisfied,
        "compiler",
        at("2026-08-07T00:00:00Z"),
        0.9,
    )
    .with_evidence(["fact.a"])
}

/// Two independent obligations, so sufficiency tests are about the certificate rather than about
/// dependency propagation.
fn two_obligation_graph() -> ObligationGraph {
    let mut graph = ObligationGraph::new("audit a cohort split");
    graph
        .insert(Obligation::new("identity", "specimen identity resolved").mandatory())
        .expect("unique");
    graph
        .insert(Obligation::new("units", "units and reference build known"))
        .expect("unique");
    graph
}

fn satisfied_closure() -> ClosureReport {
    ClosureSelection::all_verbose().check()
}

fn empty_plan() -> BudgetPlan {
    TokenBudgetController::new(1_000)
        .plan(&[])
        .expect("an empty plan always fits")
}

#[test]
fn the_ten_protected_classes_are_the_ten_the_blueprint_lists() {
    assert_eq!(ProtectedClass::ALL.len(), 10);
    let indices: Vec<u8> = ProtectedClass::ALL
        .iter()
        .map(|class| class.blueprint_index())
        .collect();
    assert_eq!(indices, (1..=10).collect::<Vec<u8>>());
    for class in ProtectedClass::ALL {
        assert_eq!(ProtectedClass::parse(class.as_str()), Some(class));
    }
}

#[test]
fn a_stable_code_preserves_the_semantics_but_a_code_nobody_can_resolve_does_not() {
    let mut selection = ClosureSelection::all_verbose();
    selection.declare(
        ProtectedClass::UnitsAndReference,
        Representation::code("ref:grch38@1-based-fwd"),
    );
    assert!(selection.check().is_satisfied());

    selection.declare(
        ProtectedClass::UnitsAndReference,
        Representation::dangling_code("ref:opaque"),
    );
    let report = selection.check();
    assert!(!report.is_satisfied());
    assert!(matches!(
        report.violations[0],
        ClosureViolation::UnresolvableCode { .. }
    ));
}

#[test]
fn dropping_a_protected_class_outright_violates_the_closure() {
    let mut selection = ClosureSelection::all_verbose();
    selection.declare(
        ProtectedClass::ContradictionAndNegativeEvidence,
        Representation::Omitted,
    );
    let report = selection.check();
    assert_eq!(
        report.violated_classes(),
        vec![ProtectedClass::ContradictionAndNegativeEvidence]
    );
}

#[test]
fn a_protected_class_the_projection_never_mentions_is_a_violation_not_a_pass() {
    let mut selection = ClosureSelection::new();
    for class in ProtectedClass::ALL {
        if class != ProtectedClass::CounterfactualBoundary {
            selection.declare(class, Representation::Verbose);
        }
    }
    let report = selection.check();
    assert_eq!(
        report.violations,
        vec![ClosureViolation::Undeclared {
            class: ProtectedClass::CounterfactualBoundary
        }]
    );
}

#[test]
fn declaring_a_class_inapplicable_needs_a_reason() {
    let mut selection = ClosureSelection::all_verbose();
    selection.declare(
        ProtectedClass::CounterfactualBoundary,
        Representation::not_applicable("   "),
    );
    assert!(matches!(
        selection.check().violations[0],
        ClosureViolation::UnreasonedInapplicability { .. }
    ));

    selection.declare(
        ProtectedClass::CounterfactualBoundary,
        Representation::not_applicable("this projection carries no causal claim"),
    );
    assert!(selection.check().is_satisfied());
}

#[test]
fn a_mandatory_closure_larger_than_the_run_budget_fails_instead_of_dropping_obligations() {
    let mut controller = TokenBudgetController::new(100);
    let error = controller
        .plan(&[
            BudgetRequest::new("identity", 10.0, TokenEstimate::declared(80)).mandatory(),
            BudgetRequest::new("temporal_firewall", 10.0, TokenEstimate::declared(60)).mandatory(),
        ])
        .expect_err("mandatory invariants are not tradeable for tokens");

    assert_eq!(
        error,
        BudgetError::MandatoryClosureUnaffordable {
            required: 140,
            total: 100
        }
    );
}

#[test]
fn exploratory_allocation_follows_value_per_token_and_reports_what_did_not_fit() {
    let mut controller = TokenBudgetController::new(100);
    let plan = controller
        .plan(&[
            BudgetRequest::new("dense", 10.0, TokenEstimate::declared(10)),
            BudgetRequest::new("sparse", 1.0, TokenEstimate::declared(50)),
            BudgetRequest::new("middling", 9.0, TokenEstimate::declared(50)),
        ])
        .expect("nothing is mandatory");

    assert_eq!(plan.allocated_obligations(), vec!["dense", "middling"]);
    assert_eq!(plan.rejected[0].obligation, "sparse");
    assert_eq!(plan.stop, StopDecision::BudgetExhausted);
    assert_eq!(plan.allocated_tokens(), 60);
}

#[test]
fn the_mandatory_closure_is_reserved_before_anything_exploratory_is_bought() {
    let mut controller = TokenBudgetController::new(100);
    let plan = controller
        .plan(&[
            BudgetRequest::new("huge_table", 100.0, TokenEstimate::declared(95)),
            BudgetRequest::new("identity", 0.1, TokenEstimate::declared(40)).mandatory(),
        ])
        .expect("the closure fits");

    assert_eq!(plan.mandatory_reserved, 40);
    assert_eq!(plan.allocated_obligations(), vec!["identity"]);
    assert_eq!(plan.rejected[0].obligation, "huge_table");
}

#[test]
fn a_candidate_below_the_acquisition_threshold_is_never_bought() {
    let mut controller = TokenBudgetController::new(1_000).with_acquisition_threshold(0.5);
    let plan = controller
        .plan(&[BudgetRequest::new(
            "cheap_but_worthless",
            1.0,
            TokenEstimate::declared(100),
        )])
        .expect("nothing is mandatory");

    assert!(plan.allocated.is_empty());
    assert_eq!(
        plan.rejected[0].reason,
        RejectionReason::BelowAcquisitionThreshold
    );
    assert_eq!(plan.stop, StopDecision::WithinBudget);
}

#[test]
fn a_sublease_cannot_exceed_the_budget_its_parent_holds() {
    let mut controller = TokenBudgetController::new(1_000);
    let planner = controller
        .lease("planner", 400)
        .expect("budget is available");
    controller
        .sublease(planner, "imaging", 300)
        .expect("the parent holds enough");

    let error = controller
        .sublease(planner, "pathology", 200)
        .expect_err("delegation must not duplicate budget");

    assert_eq!(
        error,
        BudgetError::LeaseExceedsHolding {
            holder: "pathology".into(),
            requested: 200,
            available: 100
        }
    );
    assert_eq!(
        controller
            .lease_state(planner)
            .expect("lease exists")
            .remaining(),
        100
    );
}

#[test]
fn a_rejected_branch_keeps_its_consumed_ledger_and_cannot_spend_again() {
    let mut controller = TokenBudgetController::new(1_000);
    let branch = controller
        .lease("branch", 100)
        .expect("budget is available");
    controller
        .spend(branch, &TokenEstimate::declared(40))
        .expect("the spend is within the lease");
    controller
        .reject_branch(branch, "the branch lost the comparison")
        .expect("lease exists");

    let error = controller
        .spend(branch, &TokenEstimate::declared(10))
        .expect_err("a closed branch may not spend");

    assert!(matches!(error, BudgetError::LeaseRejected(_)));
    assert_eq!(
        controller.ledger().spent_by("branch"),
        40,
        "rejection is not a refund"
    );
    assert_eq!(
        controller.lease_state(branch).expect("lease exists").spent,
        40
    );
}

#[test]
fn privacy_exposure_is_never_refunded() {
    let mut controller = TokenBudgetController::new(1_000);
    controller.record_privacy_exposure(3, "released a redacted subject identifier");
    let branch = controller.lease("branch", 10).expect("budget is available");
    controller
        .reject_branch(branch, "the branch was abandoned")
        .expect("lease exists");

    assert_eq!(
        controller.ledger().privacy_exposure(),
        3,
        "discarding the context does not un-disclose what it exposed"
    );
}

#[test]
fn every_reported_token_count_names_the_method_that_produced_it() {
    let heuristic = TokenEstimate::of_text("a moderately sized rendering of some evidence");
    assert!(heuristic.label().contains("estimated:chars-per-token-4"));
    assert!(!heuristic.method.is_measured());

    let measured = TokenEstimate::from_provider(12, "claude-tokenizer");
    assert!(measured.label().contains("tokenizer:claude-tokenizer"));
    assert!(measured.method.is_measured());
}

#[test]
fn summing_counts_from_different_estimators_reports_a_mixed_method() {
    let mixed = TokenEstimate::sum([
        &TokenEstimate::of_text("abcdefgh"),
        &TokenEstimate::declared(5),
    ]);
    assert_eq!(mixed.tokens, 7);
    assert!(matches!(mixed.method, EstimationMethod::Mixed { .. }));
    assert!(mixed.label().contains("mixed:"));

    let one_tokenizer = TokenEstimate::sum([
        &TokenEstimate::from_provider(4, "claude-tokenizer"),
        &TokenEstimate::from_provider(6, "claude-tokenizer"),
    ]);
    assert_eq!(
        one_tokenizer.method,
        EstimationMethod::ProviderTokenizer {
            name: "claude-tokenizer".into()
        },
        "a total measured throughout by one tokenizer is still that tokenizer's number"
    );

    let uniform = TokenEstimate::sum([&TokenEstimate::declared(2), &TokenEstimate::declared(3)]);
    assert_eq!(uniform.method, EstimationMethod::DeclaredByCaller);
}

#[test]
fn estimator_drift_beyond_tolerance_escalates_rather_than_being_absorbed() {
    let mut controller = TokenBudgetController::new(1_000);
    assert_eq!(
        controller.reconcile("identity", &TokenEstimate::declared(100), 110),
        StopDecision::WithinBudget
    );
    assert_eq!(
        controller.reconcile("identity", &TokenEstimate::declared(100), 220),
        StopDecision::EstimatorDriftEscalation
    );
}

#[test]
fn a_mandatory_candidate_omitted_without_a_classified_reason_fails_validation() {
    let mut ledger = OmissionLedger::default();
    ledger.push(
        OmittedCandidate::new(
            "fact.consent_record",
            OmissionReason::Unclassified,
            TokenEstimate::declared(40),
        )
        .mandatory(),
    );

    assert_eq!(
        ledger.validate(),
        Err(LedgerError::UnclassifiedMandatoryOmission(
            "fact.consent_record".into()
        ))
    );
}

#[test]
fn claiming_irrelevance_without_an_argument_fails_validation() {
    let mut ledger = OmissionLedger::default();
    ledger.push(OmittedCandidate::new(
        "fact.unused",
        OmissionReason::Irrelevant,
        TokenEstimate::declared(10),
    ));
    assert_eq!(
        ledger.validate(),
        Err(LedgerError::UnsupportedInfluenceClaim("fact.unused".into()))
    );

    let mut argued = OmissionLedger::default();
    argued.push(
        OmittedCandidate::new(
            "fact.unused",
            OmissionReason::Irrelevant,
            TokenEstimate::declared(10),
        )
        .proven_irrelevant("no dependency path reaches the estimand"),
    );
    assert_eq!(argued.validate(), Ok(()));
}

#[test]
fn a_policy_redaction_is_never_grouped_with_irrelevant_evidence() {
    let mut ledger = OmissionLedger::default();
    ledger.push(
        OmittedCandidate::new(
            "fact.germline_variants",
            OmissionReason::PolicyRedacted {
                policy: "consent:no-secondary-use".into(),
            },
            TokenEstimate::declared(200),
        )
        .retrievable_by("request controlled-access review"),
    );
    ledger.push(
        OmittedCandidate::new(
            "fact.scanner_serial",
            OmissionReason::Irrelevant,
            TokenEstimate::declared(5),
        )
        .proven_irrelevant("no dependency path reaches the estimand"),
    );
    ledger.validate().expect("both rows are classified");

    let manifest = ledger.to_manifest();
    assert_eq!(manifest.groups.len(), 2);
    assert!(!manifest.supports_sufficiency_claim());
    let policy = manifest
        .groups
        .iter()
        .find(|group| group.reason == "policy_redacted")
        .expect("the policy group survives grouping");
    assert_eq!(policy.influence, InfluenceClass::InaccessibleByPolicy);
    assert_eq!(ledger.retrieval_actions().len(), 1);
}

#[test]
fn a_policy_redaction_that_names_no_policy_is_not_reproducible() {
    let mut ledger = OmissionLedger::default();
    ledger.push(OmittedCandidate::new(
        "fact.x",
        OmissionReason::PolicyRedacted {
            policy: "  ".into(),
        },
        TokenEstimate::declared(1),
    ));
    assert_eq!(
        ledger.validate(),
        Err(LedgerError::PolicyReasonNotReproducible("fact.x".into()))
    );
}

#[test]
fn dropping_a_candidate_for_size_is_recorded_as_unknown_influence_not_as_irrelevance() {
    let row = OmittedCandidate::new(
        "fact.big_matrix",
        OmissionReason::BudgetExceeded,
        TokenEstimate::declared(9_000),
    );
    assert_eq!(row.influence, InfluenceClass::Unknown);
    assert!(!row.influence.supports_sufficiency());
}

#[test]
fn an_unmet_obligation_whose_influence_was_never_analysed_voids_the_sufficiency_claim() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let ledger = OmissionLedger::default();
    let closure = satisfied_closure();
    let relevance = RelevanceIndex::default();

    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    assert_eq!(certificate.status, SufficiencyStatus::Unknown);
    assert_eq!(certificate.unmet.len(), 1);
    assert_eq!(certificate.unmet[0].obligation, "units");
    assert_eq!(certificate.unmet[0].influence, InfluenceClass::Unknown);
}

#[test]
fn an_unmet_obligation_proven_irrelevant_still_permits_a_sufficient_certificate() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let ledger = OmissionLedger::default();
    let closure = satisfied_closure();
    let mut relevance = RelevanceIndex::default();
    relevance.insert(RelevanceRecord::irrelevant(
        "units",
        "the estimand is unitless; no selected factor reads a unit",
    ));

    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    assert!(certificate.is_sufficient());
    assert_eq!(certificate.unmet[0].influence, InfluenceClass::Zero);
    assert!(certificate.unmet[0].justification.is_some());
}

#[test]
fn a_definite_gap_outranks_an_unexamined_one_in_the_verdict() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let mut ledger = OmissionLedger::default();
    ledger.push(OmittedCandidate::new(
        "fact.consent",
        OmissionReason::PolicyRedacted {
            policy: "consent:no-secondary-use".into(),
        },
        TokenEstimate::declared(10),
    ));
    let closure = satisfied_closure();
    let relevance = RelevanceIndex::default();

    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    assert_eq!(certificate.status, SufficiencyStatus::Insufficient);
    assert!(certificate
        .reasons
        .iter()
        .any(|reason| reason.contains("inaccessible_by_policy")));
}

#[test]
fn a_protected_closure_violation_fails_the_certificate_rather_than_merely_weakening_it() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    graph
        .record(
            "units",
            StateRecord::new(
                ObligationState::NotApplicable,
                "compiler",
                at("2026-08-07T00:00:00Z"),
                1.0,
            )
            .with_reason("unitless estimand"),
        )
        .expect("exists");
    let ledger = OmissionLedger::default();
    let mut selection = ClosureSelection::all_verbose();
    selection.declare(ProtectedClass::Identity, Representation::Omitted);
    let closure = selection.check();
    let relevance = RelevanceIndex::default();

    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    assert_eq!(certificate.status, SufficiencyStatus::Failed);
}

#[test]
fn a_hand_written_sufficient_certificate_is_contradicted_by_a_recheck() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let ledger = OmissionLedger::default();
    let closure = satisfied_closure();
    let relevance = RelevanceIndex::default();
    let inputs = SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    };

    let mut forged = SufficiencyCertificate::decide(inputs);
    assert_eq!(forged.status, SufficiencyStatus::Unknown);
    forged.status = SufficiencyStatus::Sufficient;

    assert_eq!(
        forged.recheck(inputs),
        CertificateCheck::Contradicted {
            claimed: SufficiencyStatus::Sufficient,
            recomputed: SufficiencyStatus::Unknown,
        }
    );
}

#[test]
fn a_certificate_computed_against_an_older_graph_is_detected_as_stale() {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let ledger = OmissionLedger::default();
    let closure = satisfied_closure();
    let relevance = RelevanceIndex::default();
    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    graph.record("units", satisfied()).expect("exists");
    let check = certificate.recheck(SufficiencyInputs {
        decision: "cohort-audit@1",
        role: "statistician",
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    assert!(matches!(check, CertificateCheck::StaleGraph { .. }));
    assert!(!check.is_consistent());
}

fn capsule_with(role: &str, certificate_role: &str) -> BioContextCapsule {
    let mut graph = two_obligation_graph();
    graph.record("identity", satisfied()).expect("exists");
    let ledger = OmissionLedger::default();
    let closure = satisfied_closure();
    let mut relevance = RelevanceIndex::default();
    relevance.insert(RelevanceRecord::irrelevant("units", "unitless estimand"));
    let certificate = SufficiencyCertificate::decide(SufficiencyInputs {
        decision: "bdc:cohort-audit@1",
        role: certificate_role,
        graph: &graph,
        ledger: &ledger,
        closure: &closure,
        relevance: &relevance,
        compiler_version: "context-compiler@0.1.0",
    });

    BioContextCapsule {
        world_ref: "bw:oncoworld@0.1".into(),
        decision_ref: "bdc:cohort-audit@1".into(),
        role_ref: role.into(),
        base_capsule_ref: None,
        valid_at: at("2026-08-07T00:00:00Z"),
        visibility_cutoff: at("2026-08-07T00:00:00Z"),
        objective: "audit whether the split estimates external generalization".into(),
        output_contract: "cohort-audit@1".into(),
        obligation_frontier: graph.frontier().expect("graph is acyclic"),
        closure,
        budget: empty_plan(),
        omission_ledger: ledger,
        sufficiency: certificate,
        compiler_version: "context-compiler@0.1.0".into(),
    }
}

#[test]
fn a_capsule_whose_certificate_belongs_to_another_role_is_invalid() {
    let capsule = capsule_with("role:statistician@1", "role:privacy-reviewer@1");
    let error = capsule
        .validate()
        .expect_err("sufficiency is scoped to a decision and a role");
    assert!(matches!(
        error,
        CapsuleError::CertificateScopeMismatch { .. }
    ));

    let matching = capsule_with("role:statistician@1", "role:statistician@1");
    matching.validate().expect("scopes agree");
}

#[test]
fn a_capsule_may_not_see_past_its_own_visibility_cutoff() {
    let mut capsule = capsule_with("role:statistician@1", "role:statistician@1");
    capsule.visibility_cutoff = at("2026-09-01T00:00:00Z");
    let error = capsule
        .validate()
        .expect_err("evidence released after the decision must stay invisible");
    assert!(matches!(error, CapsuleError::CutAfterValidTime { .. }));
}

#[test]
fn a_capsule_missing_a_required_identity_field_is_invalid() {
    let mut capsule = capsule_with("role:statistician@1", "role:statistician@1");
    capsule.world_ref = String::new();
    assert_eq!(
        capsule.validate(),
        Err(CapsuleError::MissingField("world_ref"))
    );
}

#[test]
fn every_capsule_layer_reports_the_sufficiency_status_and_the_omission_count() {
    let capsule = capsule_with("role:statistician@1", "role:statistician@1");
    for layer in Layer::ALL {
        let rendered = capsule.render(layer);
        assert_eq!(
            rendered["sufficiency"]["status"],
            "sufficient",
            "layer {} must report the verdict",
            layer.as_str()
        );
        assert!(rendered["sufficiency"]["omitted_candidates"].is_number());
    }

    let l0 = capsule.render(Layer::L0);
    assert!(
        l0.get("omission_ledger").is_none(),
        "L0 carries the fact of omission, not the rows"
    );
    assert!(capsule.render(Layer::L2).get("omission_ledger").is_some());
}

#[test]
fn a_capsule_is_content_addressed() {
    let capsule = capsule_with("role:statistician@1", "role:statistician@1");
    let id = capsule
        .capsule_id()
        .expect("capsule is canonically hashable");
    assert!(id.starts_with("bctx:"));
    assert_eq!(id.len(), "bctx:".len() + 64);
    assert_eq!(id, capsule.capsule_id().expect("hashing is deterministic"));
}
