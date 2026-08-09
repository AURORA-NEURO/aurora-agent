//! A reference compile, held against a golden fixture and then degraded.
//!
//! Blueprint 39.21's definition of done is that "another engineer can invoke it from a clean
//! checkout, reproduce its golden output" and "observe a typed failure for each declared error
//! class". The unit tests inside each module check one invariant at a time; this file checks the
//! thing the module actually exists for — that a realistic compile can be pinned, and that each of
//! the compression defects section 39 is afraid of fails the pin with a diagnosis naming what moved.
//!
//! The reference compile is a molecular tumour board decision. It carries the four node classes
//! 39.01 forbids compressing away — an invariant, a contradiction, a failed assay, an uncertainty —
//! so every degradation below removes something the section says must survive.

use bioprism_obligation::{EstimationMethod, SufficiencyStatus, TokenEstimate};
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};
use bioprism_tokens::{
    pin_single, plan_recomputation, project, CacheValidity, CompiledContext, ContextDrift,
    ContextEpoch, ContextFixture, ContextNode, Currency, DriftSeverity, FixtureVerdict,
    InvalidationGraph, NodeKind, ProjectionPolicy, ReusePolicy, ValidityDeclaration,
    Visibility, WorldDigest, WorldObservation,
};
use std::collections::BTreeMap;

const COMPILER: &str = "compiler/1.4.0";
const POLICY: &str = "policy/board-minimal";

fn tokens(count: usize) -> TokenEstimate {
    TokenEstimate::declared(count)
}

/// The reference compile every test in this file starts from.
fn reference_compile() -> CompiledContext {
    let mut omissions = OmissionManifest::default();
    omissions.push(OmissionGroup {
        reason: "repeated_background_definitions".to_string(),
        influence: InfluenceClass::Zero,
        count: 31,
        bound: None,
        examples: vec!["doc/glioma-primer#1".to_string()],
    });
    omissions.push(OmissionGroup {
        reason: "raw_expression_matrix_cells".to_string(),
        influence: InfluenceClass::Bounded,
        count: 1_200_000,
        bound: Some(0.01),
        examples: vec!["matrix/rnaseq".to_string()],
    });

    CompiledContext::new(
        COMPILER,
        "decision/mtb-2026-04",
        "molecular",
        POLICY,
        SufficiencyStatus::Sufficient,
    )
    .with_node(
        ContextNode::new("n/reference-build", NodeKind::Invariant, tokens(8))
            .filling_slot("reference_build")
            .at("world://assembly/GRCh38"),
    )
    .with_node(
        ContextNode::new("n/specimen-lineage", NodeKind::Invariant, tokens(24))
            .filling_slot("specimen_lineage")
            .at("world://specimen/A-3#lineage"),
    )
    .with_node(
        ContextNode::new("n/idh-call", NodeKind::Evidence, tokens(90))
            .serving("o/molecular-subtype")
            .at("world://assay/idh#call")
            .rendered_as("IDH1 R132H detected by immunohistochemistry"),
    )
    .with_node(
        ContextNode::new("n/idh-sequencing-discordance", NodeKind::Contradiction, tokens(70))
            .serving("o/molecular-subtype")
            .at("world://assay/idh-ngs#call"),
    )
    .with_node(
        ContextNode::new("n/mgmt-assay-failed", NodeKind::NegativeEvidence, tokens(35))
            .serving("o/mgmt-status")
            .at("world://assay/mgmt#qc"),
    )
    .with_node(
        ContextNode::new("n/subtype-posterior", NodeKind::Uncertainty, tokens(45))
            .serving("o/molecular-subtype")
            .at("world://model/subtype#posterior"),
    )
    .with_node(
        ContextNode::new("n/consent-restriction", NodeKind::PolicyRestriction, tokens(18))
            .at("world://consent/A-3"),
    )
    .with_node(
        ContextNode::new("n/expression-view", NodeKind::Summary, tokens(160))
            .serving("o/expression-programme")
            .at("world://matrix/rnaseq#quantiles"),
    )
    .with_omissions(omissions)
}

fn golden() -> ContextFixture {
    pin_single(
        "fx/mtb-2026-04",
        "decision/mtb-2026-04",
        &reference_compile(),
        10,
    )
    .expect("the reference compile carries no holdout state and can be pinned")
}

#[test]
fn the_reference_compile_reproduces_its_own_golden_exactly() {
    let report = golden().check(&reference_compile()).expect("checks");
    assert_eq!(report.verdict, FixtureVerdict::Accepted);
    assert!(report.drifts.is_empty());
    assert!(report.diagnosis().is_empty());
}

#[test]
fn the_reference_compile_is_reproducible_across_repeated_compiles() {
    let first = reference_compile().semantic_digest().expect("digests");
    for _ in 0..16 {
        assert_eq!(
            reference_compile().semantic_digest().expect("digests"),
            first
        );
    }
}

#[test]
fn dropping_the_contradiction_to_save_tokens_fails_the_golden_with_the_node_named() {
    let mut compressed = reference_compile();
    compressed
        .nodes
        .retain(|node| node.node_id != "n/idh-sequencing-discordance");

    let report = golden().check(&compressed).expect("checks");
    assert_eq!(report.verdict, FixtureVerdict::Rejected);
    let diagnosis = report.diagnosis();
    assert!(
        diagnosis
            .iter()
            .any(|line| line.contains("n/idh-sequencing-discordance")),
        "the diagnosis must name the node, not only report a difference: {diagnosis:?}"
    );
    assert!(diagnosis
        .iter()
        .any(|line| line.contains("contradiction nodes fell") || line.contains("contradiction")));
}

#[test]
fn turning_a_failed_assay_into_ordinary_evidence_is_a_critical_drift() {
    let mut relabelled = reference_compile();
    for node in &mut relabelled.nodes {
        if node.node_id == "n/mgmt-assay-failed" {
            node.kind = NodeKind::Evidence;
        }
    }
    let report = golden().check(&relabelled).expect("checks");
    assert_eq!(report.critical().count(), 2);
    assert!(report.drifts.iter().any(|drift| matches!(
        drift,
        ContextDrift::NodeKindChanged {
            expected: NodeKind::NegativeEvidence,
            actual: NodeKind::Evidence,
            ..
        }
    )));
}

#[test]
fn a_bounded_omission_degrading_to_unknown_fails_even_though_the_context_got_smaller() {
    let mut degraded = reference_compile();
    let smaller_total: usize = degraded.nodes.iter().map(|node| node.estimate.tokens).sum();
    for group in &mut degraded.omissions.groups {
        if group.reason == "raw_expression_matrix_cells" {
            group.influence = InfluenceClass::Unknown;
            group.bound = None;
        }
    }
    degraded.nodes.retain(|node| node.node_id != "n/expression-view");
    let now_total: usize = degraded.nodes.iter().map(|node| node.estimate.tokens).sum();
    assert!(now_total < smaller_total, "the change did reduce token cost");

    let report = golden().check(&degraded).expect("checks");
    assert_eq!(report.verdict, FixtureVerdict::Rejected);
    assert!(report.drifts.iter().any(|drift| matches!(
        drift,
        ContextDrift::OmissionInfluenceWeakened {
            expected: InfluenceClass::Bounded,
            actual: InfluenceClass::Unknown,
            ..
        }
    )));
}

#[test]
fn rewording_every_node_leaves_the_golden_accepted() {
    let mut reworded = reference_compile();
    for node in &mut reworded.nodes {
        if node.rendering.is_some() {
            *node = node.clone().rendered_as("a completely different sentence");
        }
    }
    let report = golden().check(&reworded).expect("checks");
    assert!(report.verdict.is_accepted());
    assert_eq!(report.advisories().count(), 1);
    assert_eq!(report.critical().count(), 0);
    assert_eq!(report.regressions().count(), 0);
}

#[test]
fn swapping_the_estimator_invalidates_the_token_band_rather_than_violating_it() {
    let mut retokenized = reference_compile();
    for node in &mut retokenized.nodes {
        node.estimate = TokenEstimate::from_provider(node.estimate.tokens * 2 + 3, "cl100k_base");
    }
    let report = golden().check(&retokenized).expect("checks");
    let estimator_drift = report
        .drifts
        .iter()
        .find(|drift| matches!(drift, ContextDrift::EstimatorChanged { .. }))
        .expect("the estimator change is reported");
    assert_eq!(estimator_drift.severity(), DriftSeverity::Critical);
    assert!(estimator_drift.describe().contains("different rulers"));
    assert!(
        !report
            .drifts
            .iter()
            .any(|drift| matches!(drift, ContextDrift::TokenTotalOutsideBand { .. })),
        "a band violation would be a meaningless finding once the ruler changed"
    );
}

#[test]
fn a_golden_taken_from_a_compile_containing_hidden_truth_cannot_be_created() {
    let leaky = reference_compile().with_node(
        ContextNode::new("n/oracle-subtype", NodeKind::Evidence, tokens(4))
            .with_visibility(Visibility::Holdout),
    );
    assert!(pin_single("fx/leaky", "decision/mtb-2026-04", &leaky, 10).is_err());
}

#[test]
fn no_number_the_reference_compile_reports_claims_to_be_a_measurement() {
    let total = reference_compile().total_estimate();
    assert!(!total.method.is_measured());
    assert_eq!(total.method, EstimationMethod::DeclaredByCaller);
}

#[test]
fn the_boards_projection_to_the_statistician_drops_the_private_view_and_records_it() {
    let compiled = reference_compile().with_node(
        ContextNode::new("n/imaging-volumes", NodeKind::Evidence, tokens(600)).with_visibility(
            Visibility::PeerPrivate {
                owner_role: "imaging".to_string(),
            },
        ),
    );
    let policy = ProjectionPolicy::new("policy/stats", "statistics", "board", "effect_estimate")
        .showing([NodeKind::Invariant, NodeKind::Evidence, NodeKind::Uncertainty])
        .with_mandatory_contradictions()
        .allowing_expansion("expand:evidence");

    let projection = project(&compiled, &policy).expect("projects");
    assert!(!projection.node_ids().contains("n/imaging-volumes"));
    assert!(projection.accounts_for_every_drop());
    assert!(projection.node_ids().contains("n/idh-sequencing-discordance"));
    assert_eq!(projection.sufficiency, SufficiencyStatus::Unknown);
    assert!(projection.cost.dropped.tokens >= 600);
    assert!(!projection.cost.is_measured());
}

#[test]
fn a_compile_pinned_against_a_world_digest_goes_stale_when_the_world_moves() {
    let declaration = ValidityDeclaration::new(
        "ctx/mtb-2026-04",
        ContextEpoch(41),
        CacheValidity::UntilWorldChanges {
            compiled_against: WorldDigest::new("world/rev-41"),
        },
        bioprism_tokens::BiologicalValidity::AsOfDecisionEpoch {
            decision_epoch: ContextEpoch(41),
        },
    );

    let unchecked = declaration.assess(&WorldObservation::nothing_observed());
    assert!(unchecked.is_undetermined());
    assert!(!unchecked.is_within_declared_validity());
    assert!(ReusePolicy::STRICT
        .admit("ctx/mtb-2026-04", &unchecked)
        .is_err());

    let same_world = declaration
        .assess(&WorldObservation::nothing_observed().with_world(WorldDigest::new("world/rev-41")));
    let moved_world = declaration
        .assess(&WorldObservation::nothing_observed().with_world(WorldDigest::new("world/rev-52")));
    assert!(same_world.is_within_declared_validity());
    assert!(moved_world.is_expired());
    assert_ne!(same_world, moved_world);
    assert_ne!(unchecked, same_world);
}

#[test]
fn a_corrected_assay_recomputes_its_dependents_and_names_what_it_leaves_alone() {
    let graph = InvalidationGraph::new()
        .derived("view/idh", "assay/idh")
        .derived("capsule/molecular", "view/idh")
        .derived("capsule/board", "capsule/molecular")
        .derived("view/mgmt", "assay/mgmt")
        .derived("capsule/imaging", "series/mri");

    let plan = plan_recomputation(
        &graph,
        &["assay/idh".to_string()],
        &BTreeMap::new(),
        ReusePolicy::STRICT,
        None,
    )
    .expect("plans");

    let would = plan.would_recompute();
    assert!(would.contains("capsule/board"));
    assert!(!would.contains("view/mgmt"));
    assert!(plan.retained.contains("capsule/imaging"));

    let explanation = plan.explain();
    assert!(explanation
        .iter()
        .any(|line| line.contains("capsule/board") && line.contains("assay/idh")));
}

#[test]
fn a_cached_capsule_whose_currency_is_undetermined_is_recomputed_under_a_strict_policy() {
    let graph = InvalidationGraph::new()
        .with_unit("capsule/board")
        .with_unit("capsule/imaging");
    let declaration = ValidityDeclaration::new(
        "capsule/board",
        ContextEpoch(3),
        CacheValidity::Ttl { ttl_epochs: 2 },
        bioprism_tokens::BiologicalValidity::AsOfDecisionEpoch {
            decision_epoch: ContextEpoch(3),
        },
    );
    let mut checked: BTreeMap<String, Currency> = BTreeMap::new();
    checked.insert(
        "capsule/board".to_string(),
        declaration.assess(&WorldObservation::nothing_observed()),
    );

    let strict =
        plan_recomputation(&graph, &[], &checked, ReusePolicy::STRICT, None).expect("plans");
    assert_eq!(strict.fan_out(), 1);
    assert!(strict.explain()[0].contains("currency could not be established"));

    let offline =
        plan_recomputation(&graph, &[], &checked, ReusePolicy::OFFLINE, None).expect("plans");
    assert!(offline.is_empty());
}
