//! Invariants of observed-data world authoring, blueprint 35.02.

use bioprism_ids::ContentHash;
use bioprism_megafactory::authoring::AuthoringProperty;
use bioprism_megafactory::{
    check_release_lineage, ArtifactRecord, AuthoredWorld, AuthoringError, LatentClaim, LatentTruth,
    LimitationsCard, ReconstructedDecision, ReleaseMode, RightsReview,
};
use bioprism_scale::audit::{Auditor, ReleaseAudit};
use bioprism_scope::Timestamp;

fn at(nanos: i128) -> Timestamp {
    Timestamp::from_nanos_utc(nanos)
}

fn digest(label: &str) -> ContentHash {
    ContentHash::of_value(&serde_json::json!({ "artifact": label })).expect("finite json")
}

fn world(id: &str, selected: ReleaseMode) -> AuthoredWorld {
    AuthoredWorld {
        id: id.into(),
        derived_from: None,
        rights: RightsReview::new("rights-office", selected, selected, "DUA-A"),
        decision: ReconstructedDecision {
            id: format!("{id}-decision"),
            question: "escalate or observe".into(),
            decided_at: at(1_000),
        },
        artifacts: vec![ArtifactRecord::native("scan", at(500), digest("scan"))],
        latents: Vec::new(),
        limitations: LimitationsCard::new("authoring-expert"),
    }
}

fn finding(report: &bioprism_megafactory::AuthoringReport, property: AuthoringProperty) -> bool {
    report
        .findings
        .iter()
        .find(|finding| finding.property == property)
        .expect("every property is reported")
        .held
}

#[test]
fn an_unavailable_latent_yields_no_value_and_has_no_placeholder() {
    let unavailable = LatentTruth::unavailable("no confirmatory assay was run on this specimen");
    assert_eq!(unavailable.value(), None);
    assert!(unavailable.is_unavailable());

    let established = LatentTruth::established("positive", "orthogonal assay");
    assert_eq!(established.value(), Some("positive"));
    assert!(!established.is_unavailable());
}

#[test]
fn an_unavailable_latent_serialises_as_unavailable_and_carries_no_value_field() {
    let json = serde_json::to_string(&LatentTruth::unavailable("specimen exhausted"))
        .expect("serialisable");
    assert!(json.contains(r#""latent_truth":"unavailable""#), "{json}");
    assert!(
        !json.contains("\"value\""),
        "an unavailable latent must not carry a value field at all: {json}"
    );
}

#[test]
fn an_unavailable_latent_absent_from_the_card_fails_the_card_check() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.latents.push(LatentClaim::new(
        "true tumour fraction",
        LatentTruth::unavailable("no orthogonal quantification exists for this cohort"),
    ));
    let report = authored.check().expect("well formed");
    assert!(!finding(
        &report,
        AuthoringProperty::LimitationsCardComplete
    ));
    assert_eq!(report.unavailable_latents, vec!["true tumour fraction"]);
}

#[test]
fn a_card_that_states_the_gap_passes_the_card_check() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.latents.push(LatentClaim::new(
        "true tumour fraction",
        LatentTruth::unavailable("no orthogonal quantification exists for this cohort"),
    ));
    authored.limitations = LimitationsCard::new("authoring-expert").stating(
        "true tumour fraction",
        "no orthogonal quantification exists",
    );
    let report = authored.check().expect("well formed");
    assert!(finding(&report, AuthoringProperty::LimitationsCardComplete));
    assert!(report.all_checked_properties_held());
}

#[test]
fn an_artifact_observed_after_the_decision_breaks_time_validity() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.artifacts.push(ArtifactRecord::native(
        "late-report",
        at(9_000),
        digest("late"),
    ));
    let report = authored.check().expect("well formed");
    assert!(!finding(&report, AuthoringProperty::TimeValid));
    assert!(report
        .failing()
        .iter()
        .any(|failing| failing.detail.contains("late-report")));
}

#[test]
fn an_artifact_observed_exactly_at_the_decision_instant_is_time_valid() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.artifacts.push(ArtifactRecord::native(
        "same-instant",
        at(1_000),
        digest("same"),
    ));
    let report = authored.check().expect("well formed");
    assert!(finding(&report, AuthoringProperty::TimeValid));
}

#[test]
fn a_derived_form_naming_an_unpreserved_native_fails_preservation() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.artifacts.push(ArtifactRecord::derived(
        "segmentation",
        "raw-volume",
        at(700),
        digest("seg"),
    ));
    let report = authored.check().expect("well formed");
    assert!(!finding(
        &report,
        AuthoringProperty::NativeArtifactPreserved
    ));
}

#[test]
fn a_derived_form_over_a_preserved_native_passes_preservation() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored
        .artifacts
        .push(ArtifactRecord::native("raw-volume", at(400), digest("raw")));
    authored.artifacts.push(ArtifactRecord::derived(
        "segmentation",
        "raw-volume",
        at(700),
        digest("seg"),
    ));
    let report = authored.check().expect("well formed");
    assert!(finding(&report, AuthoringProperty::NativeArtifactPreserved));
}

#[test]
fn a_derived_form_naming_another_derived_form_is_not_preservation() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.artifacts.push(ArtifactRecord::derived(
        "first-pass",
        "scan",
        at(600),
        digest("first"),
    ));
    authored.artifacts.push(ArtifactRecord::derived(
        "second-pass",
        "first-pass",
        at(700),
        digest("second"),
    ));
    let report = authored.check().expect("well formed");
    assert!(
        !finding(&report, AuthoringProperty::NativeArtifactPreserved),
        "a chain of derivations does not preserve a native artifact"
    );
}

#[test]
fn a_duplicate_artifact_id_is_a_structural_error_not_a_finding() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored
        .artifacts
        .push(ArtifactRecord::native("scan", at(400), digest("other")));
    assert_eq!(
        authored.check(),
        Err(AuthoringError::DuplicateArtifact {
            world: "w1".into(),
            artifact: "scan".into()
        })
    );
}

#[test]
fn a_duplicate_latent_question_is_a_structural_error() {
    let mut authored = world("w1", ReleaseMode::Controlled);
    authored.latents.push(LatentClaim::new(
        "stage",
        LatentTruth::established("II", "pathology report"),
    ));
    authored.latents.push(LatentClaim::new(
        "stage",
        LatentTruth::unavailable("report was ambiguous"),
    ));
    assert_eq!(
        authored.check(),
        Err(AuthoringError::DuplicateLatentQuestion {
            world: "w1".into(),
            question: "stage".into()
        })
    );
}

#[test]
fn release_modes_order_from_open_to_enclave() {
    assert!(ReleaseMode::Open.is_broader_than(ReleaseMode::Enclave));
    assert!(ReleaseMode::Registered.is_broader_than(ReleaseMode::Controlled));
    assert!(!ReleaseMode::Enclave.is_broader_than(ReleaseMode::Open));
    assert!(!ReleaseMode::Controlled.is_broader_than(ReleaseMode::Controlled));
}

#[test]
fn a_rights_review_refuses_a_selection_broader_than_it_permits() {
    let review = RightsReview::new("office", ReleaseMode::Controlled, ReleaseMode::Open, "DUA");
    assert!(!review.permits_selection());
    let narrower = RightsReview::new(
        "office",
        ReleaseMode::Controlled,
        ReleaseMode::Enclave,
        "DUA",
    );
    assert!(narrower.permits_selection());
}

#[test]
fn a_descendant_may_not_widen_its_parents_release_mode() {
    let parent = world("parent", ReleaseMode::Controlled);
    let mut child = world("child", ReleaseMode::Open);
    child.derived_from = Some("parent".into());
    let error = check_release_lineage(&[parent, child]).expect_err("widening must be refused");
    assert_eq!(
        error,
        AuthoringError::ReleaseModeWidened {
            world: "child".into(),
            parent: "parent".into(),
            parent_mode: "controlled",
            requested: "open",
        }
    );
}

#[test]
fn a_descendant_may_narrow_its_parents_release_mode() {
    let parent = world("parent", ReleaseMode::Registered);
    let mut child = world("child", ReleaseMode::Enclave);
    child.derived_from = Some("parent".into());
    assert!(check_release_lineage(&[parent, child]).is_ok());
}

#[test]
fn a_grandchild_is_compared_against_every_ancestor_not_only_its_parent() {
    let grandparent = world("gp", ReleaseMode::Enclave);
    let mut parent = world("p", ReleaseMode::Controlled);
    parent.derived_from = Some("gp".into());
    let mut child = world("c", ReleaseMode::Controlled);
    child.derived_from = Some("p".into());

    let error = check_release_lineage(&[grandparent, parent, child])
        .expect_err("the chain must be walked to the root");
    match error {
        AuthoringError::ReleaseModeWidened { parent, .. } => {
            assert_eq!(parent, "gp", "the violated edge is against the grandparent")
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_lineage_naming_an_absent_parent_is_refused_rather_than_exempt() {
    let mut orphan = world("child", ReleaseMode::Open);
    orphan.derived_from = Some("nowhere".into());
    assert_eq!(
        check_release_lineage(&[orphan]),
        Err(AuthoringError::UnknownParent {
            world: "child".into(),
            parent: "nowhere".into()
        })
    );
}

#[test]
fn a_lineage_cycle_is_a_typed_error_rather_than_a_hang() {
    let mut first = world("a", ReleaseMode::Controlled);
    first.derived_from = Some("b".into());
    let mut second = world("b", ReleaseMode::Controlled);
    second.derived_from = Some("a".into());
    assert!(matches!(
        check_release_lineage(&[first, second]),
        Err(AuthoringError::LineageCycle(_))
    ));
}

#[test]
fn the_authoring_check_records_one_release_gate_and_leaves_the_other_seven_unevaluated() {
    let authored = world("w1", ReleaseMode::Controlled);
    let report = authored.check().expect("well formed");
    assert_eq!(report.gates_left_to_others.len(), 7);

    let mut audit = ReleaseAudit::open("w1", "factory", Auditor::new("independent-site"))
        .expect("an independent auditor");
    report.contribute_to(&mut audit);
    assert!(
        audit.finish().is_err(),
        "one crate's opinion on one gate must not finish a release audit"
    );
}

#[test]
fn a_report_where_every_checked_property_held_is_still_not_a_release_decision() {
    let authored = world("w1", ReleaseMode::Controlled);
    let report = authored.check().expect("well formed");
    assert!(report.all_checked_properties_held());
    assert!(report.rights_permit_selection);
    assert!(
        !report.gates_left_to_others.is_empty(),
        "the report must name what it did not check"
    );
    assert_eq!(report.findings.len(), AuthoringProperty::ALL.len());
}
