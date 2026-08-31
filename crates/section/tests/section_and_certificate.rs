use bioprism_section::{
    Backend, CertificateProfile, CertificateVerification, ContextCertificate, DecisionSection,
    EvidenceCapsule, InfluenceClass, InformativeBound, LeakageWitness, OmissionAccountingError,
    OmissionGroup, OmissionManifest, OracleStatus, OracleVerdict, PlanDescriptor,
    ProvenUnreachable, ReferenceOmissions, RefinementOption, SourceHashes, UnresolvedObligation,
};
use serde_json::{json, Value};

fn verdict() -> OracleVerdict {
    OracleVerdict::new(
        "deterministic_split_integrity_v1",
        vec![LeakageWitness::PreprocessingLeakage {
            detail: "preprocessing fit used all subjects before split".into(),
        }],
    )
}

fn certificate() -> ContextCertificate {
    ContextCertificate {
        world_id: "w".into(),
        query_id: "q".into(),
        selected_facts: vec!["fact.a".into()],
        selected_factors: vec!["factor.a".into()],
        protected_closure: vec!["fact.a".into()],
        omissions: ReferenceOmissions {
            total_facts: 3,
            exploratory_facts: 2,
            classification: "no_backward_dependency_path_or_temporally_inaccessible".into(),
            inaccessible_selected_before_cut: vec![],
        },
        plan: PlanDescriptor {
            backend: Backend::BackwardFactorSliceReference,
            compiled_factor_count: 1,
            compiled_fact_count: 1,
            total_factor_count: 4,
            total_fact_count: 4,
            max_selected_factor_arity: 2,
            fallback: None,
        },
        oracle: verdict(),
        source_hashes: SourceHashes {
            world_sha256: "00".repeat(32),
            query_sha256: "11".repeat(32),
            decision_section_sha256: "22".repeat(32),
        },
        limitations: vec!["reference slicer".into()],
        manifest: OmissionManifest::default(),
    }
}

#[test]
fn evidence_capsule_fills_absent_tags_and_provenance() {
    let capsule = EvidenceCapsule::from_raw_fact(&json!({
        "id": "fact.a",
        "provides": "a",
        "value": 1,
        "scope": { "cohort": "C" }
    }));
    assert_eq!(capsule.tags, Vec::<String>::new());
    assert_eq!(capsule.provenance, Vec::<String>::new());
    assert_eq!(capsule.scope, json!({ "cohort": "C" }));
}

#[test]
fn section_emits_the_v0_1_field_set() {
    let section = DecisionSection {
        world_id: "w".into(),
        query_id: "q".into(),
        decision_time: "2025-01-01T00:00:00Z".into(),
        goal: "g".into(),
        selected_evidence: vec![EvidenceCapsule::from_raw_fact(&json!({
            "id": "fact.a", "provides": "a", "value": 1, "scope": {}
        }))],
        selected_factors: vec![json!({ "id": "factor.a" })],
        oracle: verdict(),
        unresolved_obligations: vec![UnresolvedObligation::InaccessibleAtCut {
            fact_id: "fact.future".into(),
        }],
        refinement_frontier: vec![RefinementOption {
            action: "advance_time_cut_or_use_retrospective_mode".into(),
            facts: vec!["fact.future".into()],
        }],
    };

    let document = section.to_json();
    let map = document.as_object().unwrap();
    let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "decision_time",
            "goal",
            "oracle",
            "query_id",
            "refinement_frontier",
            "schema_version",
            "selected_evidence",
            "selected_factors",
            "unresolved_obligations",
            "world_id",
        ]
    );

    assert_eq!(
        map["unresolved_obligations"][0],
        json!({ "type": "inaccessible_at_cut", "fact_id": "fact.future" })
    );
    assert!(section.requires_refinement());
    assert_eq!(map["oracle"]["status"], json!("invalid"));
    assert_eq!(
        map["oracle"]["witnesses"][0]["type"],
        json!("preprocessing_leakage")
    );
}

#[test]
fn certificate_digest_covers_the_body_and_detects_tampering() {
    let document = certificate()
        .to_json(CertificateProfile::Reference)
        .unwrap();
    assert!(ContextCertificate::verify(&document).unwrap().is_valid());

    let mut tampered = document.clone();
    tampered["selected_facts"] = json!(["fact.a", "fact.smuggled"]);
    match ContextCertificate::verify(&tampered).unwrap() {
        CertificateVerification::DigestMismatch { .. } => {}
        other => panic!("tampering must be detected, got {other:?}"),
    }

    let mut stripped = document;
    stripped
        .as_object_mut()
        .unwrap()
        .remove("certificate_sha256");
    assert!(matches!(
        ContextCertificate::verify(&stripped).unwrap(),
        CertificateVerification::Malformed(_)
    ));
}

#[test]
fn extended_profile_changes_schema_version_and_therefore_the_digest() {
    let cert = certificate();
    let reference = cert.to_json(CertificateProfile::Reference).unwrap();
    let extended = cert.to_json(CertificateProfile::Extended).unwrap();

    assert_eq!(
        reference["schema_version"],
        json!("fiber-context-certificate/0.1")
    );
    assert_eq!(
        extended["schema_version"],
        json!("fiber-context-certificate/0.2-extended")
    );
    assert_ne!(
        reference["certificate_sha256"],
        extended["certificate_sha256"]
    );
    assert!(reference.get("omission_manifest").is_none());
    assert!(extended.get("omission_manifest").is_some());
    assert!(ContextCertificate::verify(&extended).unwrap().is_valid());
}

#[test]
fn zero_influence_and_unknown_influence_are_not_interchangeable() {
    let mut provable = OmissionManifest::default();
    provable.push(OmissionGroup {
        reason: "no backward dependency path to target".into(),
        influence: InfluenceClass::Zero,
        count: 750,
        bound: None,
        examples: vec!["fact.explore.0".into()],
    });
    assert!(provable.supports_sufficiency_claim());

    let mut unchecked = provable.clone();
    unchecked.push(OmissionGroup {
        reason: "not analysed".into(),
        influence: InfluenceClass::Unknown,
        count: 1,
        bound: None,
        examples: vec![],
    });
    assert!(
        !unchecked.supports_sufficiency_claim(),
        "a single unanalysed group must void the sufficiency claim"
    );
    assert_eq!(unchecked.total_omitted(), 751);
    assert_eq!(unchecked.count_in(InfluenceClass::Zero), 750);
    assert_eq!(unchecked.blocking_groups().count(), 1);
}

#[test]
fn policy_blocked_and_deferred_omissions_never_support_sufficiency() {
    for class in [
        InfluenceClass::InaccessibleByPolicy,
        InfluenceClass::DeferredAcquisition,
        InfluenceClass::Unknown,
    ] {
        assert!(
            !class.supports_sufficiency(),
            "{class:?} must not count as sufficient"
        );
    }
    for class in [InfluenceClass::Zero, InfluenceClass::Bounded] {
        assert!(class.supports_sufficiency());
    }
}

#[test]
fn a_clean_oracle_run_is_valid_and_abstention_is_representable() {
    let clean = OracleVerdict::new("deterministic_split_integrity_v1", vec![]);
    assert_eq!(clean.status, OracleStatus::Valid);

    let abstained = OracleVerdict::abstain("deterministic_split_integrity_v1", vec![]);
    assert_eq!(abstained.status, OracleStatus::Underdetermined);
    let encoded: Value = serde_json::to_value(&abstained).unwrap();
    assert_eq!(encoded["status"], json!("underdetermined"));
}

#[test]
fn plan_reports_selection_ratios_without_asserting_they_are_small() {
    let plan = PlanDescriptor {
        backend: Backend::DirectMaterialization,
        compiled_factor_count: 756,
        compiled_fact_count: 761,
        total_factor_count: 756,
        total_fact_count: 761,
        max_selected_factor_arity: 4,
        fallback: None,
    };
    assert_eq!(plan.fact_selection_ratio(), 1.0);
    assert_eq!(plan.factor_selection_ratio(), 1.0);
}
