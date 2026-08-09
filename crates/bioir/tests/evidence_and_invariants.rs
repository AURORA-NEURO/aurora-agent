//! Invariants of blueprint 25.11 (evidence objects) and the 39.05 closure constraint.

use bioprism_bioir::invariants;
use bioprism_bioir::{
    AccessPolicy, ContextProjection, Derivation, EvidenceError, EvidenceId, EvidenceIssue,
    EvidenceLedger, EvidenceObject, LensId, Locator, MeasurementContext, Modality, ProtectedClass,
    Provenance, QualityAssertion, Quantity, Relation, SpecimenId, Stance, SubjectId,
};
use bioprism_ids::ContentHash;
use bioprism_scope::{Interval, ScopeKey, Timestamp};
use std::collections::{BTreeMap, BTreeSet};

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("well-formed timestamp")
}

fn eid(text: &str) -> EvidenceId {
    EvidenceId::parse(text).expect("well-formed evidence id")
}

fn provenance() -> Provenance {
    Provenance {
        adapter: "registry-csv".to_string(),
        adapter_version: "0.4.1".to_string(),
        parser_version: "2.0.0".to_string(),
        extracted_at: ts("2026-05-01T00:00:00Z"),
        source: "site-registry-v3/table2.csv".to_string(),
    }
}

fn quality() -> QualityAssertion {
    QualityAssertion {
        grade: "curated".to_string(),
        asserted_by: "data steward".to_string(),
        caveats: BTreeSet::new(),
    }
}

fn cell(id: &str, row: &str) -> EvidenceObject {
    EvidenceObject {
        id: eid(id),
        artifact_hash: ContentHash::of_bytes(b"table2.csv"),
        locator: Locator::TableCell {
            table: "table2".to_string(),
            row: row.to_string(),
            column: "ki67_index".to_string(),
        },
        modality: Modality::Tabular,
        content_type: "text/csv".to_string(),
        bindings: BTreeMap::new(),
        context: MeasurementContext::default(),
        quality: quality(),
        provenance: provenance(),
        validity: Interval {
            start: Some(ts("2026-01-01T00:00:00Z")),
            end: Some(ts("2027-01-01T00:00:00Z")),
        },
        access: AccessPolicy::default(),
        derivation: None,
    }
}

#[test]
fn derived_evidence_cannot_drop_an_access_label_its_ancestor_carried() {
    let mut ledger = EvidenceLedger::new();
    let mut controlled = cell("ev-raw", "3");
    controlled.access = AccessPolicy::labelled(["dbgap-controlled"]);
    ledger.insert(controlled).expect("ancestor inserts");

    let mut laundered = cell("ev-summary", "3");
    laundered.derivation = Some(Derivation {
        ancestors: vec![eid("ev-raw")],
        transform: "cohort mean".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    assert_eq!(
        ledger.insert(laundered),
        Err(EvidenceError::AccessLabelDropped {
            evidence: "ev-summary".to_string(),
            ancestor: "ev-raw".to_string(),
            label: "dbgap-controlled".to_string(),
        }),
        "a summary of controlled data is still controlled data"
    );
}

#[test]
fn derived_evidence_carrying_its_ancestors_labels_is_admitted() {
    let mut ledger = EvidenceLedger::new();
    let mut controlled = cell("ev-raw", "3");
    controlled.access = AccessPolicy::labelled(["dbgap-controlled"]);
    ledger.insert(controlled).expect("ancestor inserts");

    let mut summary = cell("ev-summary", "3");
    summary.access = AccessPolicy::labelled(["dbgap-controlled", "aggregate-only"]);
    summary.derivation = Some(Derivation {
        ancestors: vec![eid("ev-raw")],
        transform: "cohort mean".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    ledger.insert(summary).expect("labels may be added");

    let effective = ledger
        .effective_access_labels(&eid("ev-summary"))
        .expect("ancestry resolves");
    assert!(effective.contains("dbgap-controlled"));
    assert!(effective.contains("aggregate-only"));
}

#[test]
fn effective_access_labels_accumulate_over_a_whole_derivation_chain() {
    let mut ledger = EvidenceLedger::new();
    let mut root = cell("ev-1", "1");
    root.access = AccessPolicy::labelled(["consent-tier-2"]);
    ledger.insert(root).expect("root inserts");

    let mut middle = cell("ev-2", "1");
    middle.access = AccessPolicy::labelled(["consent-tier-2", "site-internal"]);
    middle.derivation = Some(Derivation {
        ancestors: vec![eid("ev-1")],
        transform: "normalise".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    ledger.insert(middle).expect("middle inserts");

    let mut leaf = cell("ev-3", "1");
    leaf.access = AccessPolicy::labelled(["consent-tier-2", "site-internal"]);
    leaf.derivation = Some(Derivation {
        ancestors: vec![eid("ev-2")],
        transform: "threshold".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    ledger.insert(leaf).expect("leaf inserts");

    assert_eq!(
        ledger.ancestry(&eid("ev-3")).expect("walks"),
        vec![eid("ev-2"), eid("ev-1")]
    );
    assert_eq!(
        ledger.effective_access_labels(&eid("ev-3")).expect("walks"),
        ["consent-tier-2", "site-internal"]
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<String>>()
    );
}

#[test]
fn derived_evidence_naming_an_unknown_ancestor_is_refused() {
    let mut ledger = EvidenceLedger::new();
    let mut orphan = cell("ev-summary", "3");
    orphan.derivation = Some(Derivation {
        ancestors: vec![eid("ev-missing")],
        transform: "cohort mean".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    assert_eq!(
        ledger.insert(orphan),
        Err(EvidenceError::UnknownAncestor {
            evidence: "ev-summary".to_string(),
            ancestor: "ev-missing".to_string(),
        })
    );
}

#[test]
fn evidence_that_derives_from_itself_is_refused() {
    let mut ledger = EvidenceLedger::new();
    let mut looped = cell("ev-1", "1");
    looped.derivation = Some(Derivation {
        ancestors: vec![eid("ev-1")],
        transform: "identity".to_string(),
        transform_version: "1.0.0".to_string(),
    });
    assert_eq!(
        ledger.insert(looped),
        Err(EvidenceError::SelfDerivation {
            evidence: "ev-1".to_string()
        })
    );
}

#[test]
fn a_relation_with_no_asserter_is_refused() {
    let mut ledger = EvidenceLedger::new();
    ledger.insert(cell("ev-1", "1")).expect("inserts");
    ledger.insert(cell("ev-2", "2")).expect("inserts");

    assert_eq!(
        ledger.assert_relation(Relation {
            subject: eid("ev-1"),
            object: eid("ev-2"),
            stance: Stance::Contradicts,
            asserted_by: "   ".to_string(),
            asserted_at: ts("2026-05-02T00:00:00Z"),
            rationale: String::new(),
        }),
        Err(EvidenceError::UnattributedRelation {
            subject: "ev-1".to_string(),
            object: "ev-2".to_string(),
        }),
        "a contradiction with no asserter pretends to be a fact about the world"
    );
}

#[test]
fn contradictions_are_visible_from_both_sides_of_the_relation() {
    let mut ledger = EvidenceLedger::new();
    ledger.insert(cell("ev-1", "1")).expect("inserts");
    ledger.insert(cell("ev-2", "2")).expect("inserts");
    ledger
        .assert_relation(Relation {
            subject: eid("ev-1"),
            object: eid("ev-2"),
            stance: Stance::Contradicts,
            asserted_by: "reviewer-4".to_string(),
            asserted_at: ts("2026-05-02T00:00:00Z"),
            rationale: "the two tables report different denominators".to_string(),
        })
        .expect("relation is attributed");

    assert_eq!(ledger.relations_for(&eid("ev-1")).len(), 1);
    assert_eq!(
        ledger.relations_for(&eid("ev-2")).len(),
        1,
        "the contradicted object must see the contradiction too"
    );
    assert_eq!(ledger.contradictions().len(), 1);
    assert!(ledger
        .audit(ts("2026-06-01T00:00:00Z"))
        .iter()
        .any(|issue| matches!(issue, EvidenceIssue::UnadjudicatedContradiction { .. })));
}

#[test]
fn a_relation_naming_evidence_outside_the_ledger_is_refused() {
    let mut ledger = EvidenceLedger::new();
    ledger.insert(cell("ev-1", "1")).expect("inserts");
    assert_eq!(
        ledger.assert_relation(Relation {
            subject: eid("ev-1"),
            object: eid("ev-9"),
            stance: Stance::Supports,
            asserted_by: "reviewer-4".to_string(),
            asserted_at: ts("2026-05-02T00:00:00Z"),
            rationale: String::new(),
        }),
        Err(EvidenceError::UnknownEvidence {
            evidence: "ev-9".to_string()
        })
    );
}

#[test]
fn a_sequence_locator_without_a_reference_build_is_not_resolvable() {
    let mut ledger = EvidenceLedger::new();
    let mut variant = cell("ev-variant", "1");
    variant.modality = Modality::Sequence;
    variant.locator = Locator::SequenceRange {
        sequence: "chr7".to_string(),
        reference_build: String::new(),
        start: 55_019_017,
        end: 55_019_365,
    };
    let error = ledger.insert(variant).expect_err("a coordinate needs a build");
    assert!(error.to_string().contains("reference build"));
}

#[test]
fn an_empty_image_region_and_an_inverted_span_are_not_resolvable() {
    let mut ledger = EvidenceLedger::new();
    let mut region = cell("ev-roi", "1");
    region.modality = Modality::Image;
    region.locator = Locator::ImageRegion {
        series: "flair".to_string(),
        frame: "18".to_string(),
        x0: 40,
        y0: 40,
        x1: 40,
        y1: 90,
    };
    assert!(matches!(
        ledger.insert(region),
        Err(EvidenceError::UnresolvableLocator { .. })
    ));

    let mut span = cell("ev-span", "1");
    span.modality = Modality::Text;
    span.locator = Locator::DocumentSpan {
        document: "path-report-1".to_string(),
        start: 900,
        end: 120,
    };
    assert!(matches!(
        ledger.insert(span),
        Err(EvidenceError::UnresolvableLocator { .. })
    ));
}

#[test]
fn an_artifact_hash_mismatch_names_both_hashes() {
    let object = cell("ev-1", "1");
    assert_eq!(object.verify_artifact(b"table2.csv"), Ok(()));

    let error = object
        .verify_artifact(b"table2-revised.csv")
        .expect_err("the bytes changed");
    let rendered = error.to_string();
    assert!(rendered.contains(object.artifact_hash.as_str()));
    assert!(rendered.contains(ContentHash::of_bytes(b"table2-revised.csv").as_str()));
}

#[test]
fn evidence_outside_its_validity_interval_is_stale_at_the_decision_time() {
    let mut ledger = EvidenceLedger::new();
    ledger.insert(cell("ev-1", "1")).expect("inserts");

    assert!(ledger.stale_at(ts("2026-06-01T00:00:00Z")).is_empty());
    assert_eq!(ledger.stale_at(ts("2028-06-01T00:00:00Z")).len(), 1);
    assert!(ledger
        .audit(ts("2028-06-01T00:00:00Z"))
        .contains(&EvidenceIssue::Stale {
            evidence: eid("ev-1"),
            at: ts("2028-06-01T00:00:00Z"),
        }));
}

#[test]
fn evidence_valid_at_no_instant_is_refused() {
    let mut ledger = EvidenceLedger::new();
    let mut impossible = cell("ev-1", "1");
    impossible.validity = Interval {
        start: Some(ts("2027-01-01T00:00:00Z")),
        end: Some(ts("2026-01-01T00:00:00Z")),
    };
    assert_eq!(
        ledger.insert(impossible),
        Err(EvidenceError::EmptyValidityInterval {
            evidence: "ev-1".to_string()
        })
    );
}

#[test]
fn a_duplicate_evidence_identifier_is_refused() {
    let mut ledger = EvidenceLedger::new();
    ledger.insert(cell("ev-1", "1")).expect("inserts");
    assert_eq!(
        ledger.insert(cell("ev-1", "2")),
        Err(EvidenceError::DuplicateEvidence {
            evidence: "ev-1".to_string()
        })
    );
    assert_eq!(ledger.len(), 1);
}

#[test]
fn the_content_hash_of_an_evidence_object_changes_with_its_locator() {
    let object = cell("ev-1", "1");
    let mut moved = cell("ev-1", "2");
    moved.modality = Modality::Tabular;
    assert_ne!(
        object.content_hash().expect("hashes"),
        moved.content_hash().expect("hashes")
    );
}

fn contextual_ledger() -> EvidenceLedger {
    let mut ledger = EvidenceLedger::new();
    let mut object = cell("ev-1", "1");
    object.context = MeasurementContext {
        subject: Some(SubjectId::parse("pt-1").expect("subject id")),
        specimen: Some(SpecimenId::parse("blk-1").expect("specimen id")),
        lens: Some(LensId::parse("bulk-rnaseq-tpm").expect("lens id")),
        observed_at: Some(ts("2026-02-01T00:00:00Z")),
        scope: ScopeKey::new().exact("site", "site-a"),
    };
    object.access = AccessPolicy::labelled(["consent-tier-2"]);
    ledger.insert(object).expect("inserts");
    ledger
}

#[test]
fn a_projection_without_a_cohort_omits_the_cohort_structure_class() {
    let ledger = contextual_ledger();
    let report = invariants::audit(&ContextProjection {
        evidence: Some(&ledger),
        ..ContextProjection::default()
    });

    assert!(report.omitted.contains(&ProtectedClass::CohortStructure));
    assert!(!report.is_closed());
    assert!(report.retained.contains(&ProtectedClass::Identity));
    assert!(report.retained.contains(&ProtectedClass::AccessAndConsent));
}

#[test]
fn the_counterfactual_support_boundary_is_unrepresentable_in_this_crate() {
    let report = invariants::audit(&ContextProjection::default());
    assert!(report
        .unrepresentable
        .contains(&ProtectedClass::CounterfactualSupport));
    assert!(
        !report.omitted.contains(&ProtectedClass::CounterfactualSupport),
        "a class this crate cannot carry is not the caller's omission"
    );
    assert_eq!(
        report.retained.len() + report.omitted.len() + report.unrepresentable.len(),
        ProtectedClass::ALL.len()
    );
}

#[test]
fn an_empty_projection_omits_every_representable_protected_class() {
    let report = invariants::audit(&ContextProjection::default());
    assert_eq!(report.retained.len(), 0);
    assert_eq!(report.omitted.len(), ProtectedClass::ALL.len() - 1);
}

#[test]
fn a_full_projection_retains_every_representable_protected_class() {
    use bioprism_bioir::{
        AssayLens, Calibration, CohortDefinition, CohortId, ComparabilityRule,
        EligibilityRule, ErrorModel, Estimand, GroupingKey, Identifiability, LensCatalog,
        LineageGraph, MaterialRequirement, MeasurementScale, MeasurementTarget, MissingnessClass,
        Predicate, ProtocolChain, QcContract, Representation, Specimen, TimeAnchor,
        UncertaintyBudget, UncertaintyComponent, UncertaintyKind, UnitOfAnalysis,
    };

    let ledger = contextual_ledger();

    let mut lineage = LineageGraph::new();
    lineage
        .insert(
            Specimen::collected(
                SpecimenId::parse("blk-1").expect("specimen id"),
                SubjectId::parse("pt-1").expect("subject id"),
                ts("2026-01-01T00:00:00Z"),
                "left temporal lobe",
                "FFPE block",
                Quantity::new(10.0, "mL"),
            )
            .with_consent(["research-use"]),
        )
        .expect("inserts");

    let mut catalog = LensCatalog::new();
    catalog
        .register(AssayLens {
            id: LensId::parse("bulk-rnaseq-tpm").expect("lens id"),
            version: "1.0.0".to_string(),
            target: MeasurementTarget {
                quantity: "gene expression".to_string(),
                entity: "transcript".to_string(),
                unit: "TPM".to_string(),
                scale: MeasurementScale::Ratio,
                identifiability: Identifiability::Relative,
            },
            material: MaterialRequirement {
                material: "FFPE block".to_string(),
                minimum: Quantity::new(1.0, "mL"),
                destructive: true,
            },
            protocol: ProtocolChain {
                instrument: "NovaSeq X".to_string(),
                protocol: "poly-A".to_string(),
                protocol_version: "3".to_string(),
                steps: vec![],
            },
            calibration: Calibration::uncalibrated(),
            error_model: ErrorModel {
                form: "negative binomial".to_string(),
                noise_sd: None,
                missingness: MissingnessClass::BelowDetection,
                known_artifacts: vec![],
            },
            comparability: ComparabilityRule::default(),
            qc: QcContract::default(),
            known_failure_modes: BTreeSet::new(),
        })
        .expect("registers");

    let cohort = CohortDefinition {
        id: CohortId::parse("gbm-2026").expect("cohort id"),
        population: "adults".to_string(),
        source_datasets: vec!["site-registry-v3".to_string()],
        rules: vec![EligibilityRule::include(
            "adult",
            Predicate::AttributeAtLeast {
                key: "age".to_string(),
                threshold: 18.0,
            },
        )],
        time_anchor: TimeAnchor {
            event: "resection".to_string(),
            horizon_days: Some(365),
            censoring_rule: "administrative".to_string(),
        },
        unit: UnitOfAnalysis::Subject,
        grouping: GroupingKey::default(),
        estimand: Estimand {
            target: "overall survival".to_string(),
            unit: UnitOfAnalysis::Subject,
            population: "adults".to_string(),
            contrast: None,
            summary: "risk difference".to_string(),
        },
    };

    let mut budget = UncertaintyBudget::new();
    budget
        .declare(
            "claim-1",
            UncertaintyComponent::new(
                UncertaintyKind::Epistemic,
                Representation::StandardError { value: 0.04 },
                "12-month overall survival",
            ),
        )
        .expect("declares");

    let report = invariants::audit(&ContextProjection {
        evidence: Some(&ledger),
        lineage: Some(&lineage),
        cohort: Some(&cohort),
        lenses: Some(&catalog),
        uncertainty: Some(&budget),
    });

    assert!(
        report.is_closed(),
        "unexpectedly omitted: {:?}",
        report.omitted
    );
    assert_eq!(report.retained.len(), ProtectedClass::ALL.len() - 1);
}
