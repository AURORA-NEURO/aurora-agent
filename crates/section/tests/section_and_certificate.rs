use bioprism_section::{
    Backend, CertificateProfile, CertificateVerification, ContextCertificate, DecisionSection,
    EvidenceCapsule, InfluenceClass, InformativeBound, LeakageWitness, OmissionGroup,
    OmissionManifest, OracleStatus, OracleVerdict, PlanDescriptor, ReferenceOmissions,
    RefinementOption, SourceHashes, UnresolvedObligation,
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
    assert_eq!(map["oracle"]["witnesses"][0]["type"], json!("preprocessing_leakage"));
}

#[test]
fn certificate_digest_covers_the_body_and_detects_tampering() {
    let document = certificate().to_json(CertificateProfile::Reference).unwrap();
    assert!(ContextCertificate::verify(&document).unwrap().is_valid());

    let mut tampered = document.clone();
    tampered["selected_facts"] = json!(["fact.a", "fact.smuggled"]);
    match ContextCertificate::verify(&tampered).unwrap() {
        CertificateVerification::DigestMismatch { .. } => {}
        other => panic!("tampering must be detected, got {other:?}"),
    }

    let mut stripped = document;
    stripped.as_object_mut().unwrap().remove("certificate_sha256");
    assert!(matches!(
        ContextCertificate::verify(&stripped).unwrap(),
        CertificateVerification::Malformed(_)
    ));
}

#[test]
fn a_shape_broken_certificate_digest_is_malformed_rather_than_a_mismatch() {
    let document = certificate().to_json(CertificateProfile::Reference).unwrap();
    for broken in [
        "NOT-64-LOWERCASE-HEX-CHARACTERS",
        &"AB".repeat(32),
        "abc",
        "",
    ] {
        let mut claimed = document.clone();
        claimed["certificate_sha256"] = json!(broken);
        match ContextCertificate::verify(&claimed).unwrap() {
            CertificateVerification::Malformed(reason) => assert!(
                reason.contains("certificate_sha256"),
                "the reason must name the field: {reason}"
            ),
            other => panic!(
                "{broken:?} is a defect in the claimed digest, not evidence that the body moved, \
                 but verification answered {other:?}"
            ),
        }
    }

    let mut wrong = document;
    wrong["certificate_sha256"] = json!("0".repeat(64));
    assert!(matches!(
        ContextCertificate::verify(&wrong).unwrap(),
        CertificateVerification::DigestMismatch { .. }
    ));
}

#[test]
fn extended_profile_changes_schema_version_and_therefore_the_digest() {
    let cert = certificate();
    let reference = cert.to_json(CertificateProfile::Reference).unwrap();
    let extended = cert.to_json(CertificateProfile::Extended).unwrap();

    assert_eq!(reference["schema_version"], json!("fiber-context-certificate/0.1"));
    assert_eq!(
        extended["schema_version"],
        json!("fiber-context-certificate/0.2-extended")
    );
    assert_ne!(reference["certificate_sha256"], extended["certificate_sha256"]);
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

/// The only value that can be handed to the sanctioned constructor is one that excludes something.
#[test]
fn a_vacuous_bound_cannot_be_named_as_an_informative_bound() {
    for refused in [1.0, 1.5, f64::INFINITY, f64::NAN, -0.1] {
        assert!(
            InformativeBound::new(refused).is_none(),
            "{refused} permits every answer or is not a distance, and must not be nameable"
        );
    }
    let admitted = InformativeBound::new(0.25).expect("a bound below one excludes something");
    assert_eq!(admitted.value(), 0.25);

    let group = OmissionGroup::bounded("influence bounded by 0.25", 3, admitted, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Bounded);
    assert!(group.has_informative_bound());
}

/// A group whose bound permits every answer is admitted as unknown, and unknown voids sufficiency.
#[test]
fn a_vacuous_bounded_group_is_refused_admission_and_cannot_support_sufficiency() {
    let mut manifest = OmissionManifest::default();
    manifest.push(OmissionGroup {
        reason: "influence bounded by 1".into(),
        influence: InfluenceClass::Bounded,
        count: 4,
        bound: Some(1.0),
        examples: vec!["fact.withheld".into()],
    });

    let admitted = &manifest.groups[0];
    assert_eq!(admitted.influence, InfluenceClass::Unknown);
    assert_eq!(admitted.bound, None);
    assert!(
        admitted.reason.contains("a bound of 1 permits every answer"),
        "the refused value belongs on the certificate, not in a silent downgrade: {}",
        admitted.reason
    );
    assert!(!manifest.supports_sufficiency_claim());
    assert_eq!(manifest.count_in(InfluenceClass::Bounded), 0);
    assert_eq!(
        manifest.total_omitted(),
        4,
        "refusing the bound must not lose the omitted members"
    );
}

/// Claiming the class with no number at all is the same refusal.
#[test]
fn a_bounded_class_with_no_bound_at_all_is_refused_admission() {
    let manifest = OmissionManifest::from_groups([OmissionGroup {
        reason: "influence bounded".into(),
        influence: InfluenceClass::Bounded,
        count: 1,
        bound: None,
        examples: vec![],
    }]);
    assert_eq!(manifest.groups[0].influence, InfluenceClass::Unknown);
    assert!(manifest.groups[0]
        .reason
        .contains("a bounded class was claimed with no bound at all"));
    assert!(!manifest.supports_sufficiency_claim());
}

/// The verifier's own entry point is gated, because it is the one facing untrusted bytes.
///
/// `bioprism-section` depends on neither `world` nor `fiber` so that a consumer can check a
/// certificate without linking the engine that produced it. Every group such a consumer holds
/// arrived through serde, so a check that ran only in the compiler's constructors would have
/// protected the party that already knows the bound is vacuous and left the party that does not
/// reading `supports_sufficiency_claim` as true.
#[test]
fn a_vacuous_bounded_group_parsed_from_a_document_cannot_reach_a_sufficiency_claim() {
    let document = json!({
        "groups": [
            {
                "reason": "no backward dependency path to target",
                "influence": "zero",
                "count": 750
            },
            {
                "reason": "influence bounded by 1 in total variation",
                "influence": "bounded",
                "count": 2,
                "bound": 1.0,
                "examples": ["fact.withheld"]
            }
        ]
    });
    let manifest: OmissionManifest = serde_json::from_value(document).expect("manifest parses");

    assert_eq!(manifest.groups[0].influence, InfluenceClass::Zero);
    assert_eq!(
        manifest.groups[1].influence,
        InfluenceClass::Unknown,
        "a bound of one on the wire is not a bound and must not be read as one"
    );
    assert_eq!(manifest.groups[1].bound, None);
    assert!(
        !manifest.supports_sufficiency_claim(),
        "the vacuous group must block the claim it was dressed up to support"
    );
    assert_eq!(manifest.blocking_groups().count(), 1);
}

/// The gate refuses vacuous claims and nothing else.
#[test]
fn an_informative_bounded_group_survives_a_parse_unchanged() {
    let document = json!({
        "groups": [{
            "reason": "influence bounded by 0.125 in total variation",
            "influence": "bounded",
            "count": 2,
            "bound": 0.125,
            "examples": ["fact.withheld"]
        }]
    });
    let manifest: OmissionManifest = serde_json::from_value(document.clone()).unwrap();

    assert_eq!(manifest.groups[0].influence, InfluenceClass::Bounded);
    assert_eq!(manifest.groups[0].bound, Some(0.125));
    assert!(manifest.supports_sufficiency_claim());
    assert_eq!(
        serde_json::to_value(&manifest).unwrap(),
        document,
        "an informative group round-trips byte for byte"
    );
}

#[test]
fn policy_blocked_and_deferred_omissions_never_support_sufficiency() {
    for class in [
        InfluenceClass::InaccessibleByPolicy,
        InfluenceClass::DeferredAcquisition,
        InfluenceClass::Unknown,
    ] {
        assert!(!class.supports_sufficiency(), "{class:?} must not count as sufficient");
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

#[test]
fn the_four_reference_witnesses_serialise_exactly_as_before_the_open_variant_existed() {
    let mut site_by_split = std::collections::BTreeMap::new();
    site_by_split.insert("test".to_string(), vec!["B".to_string()]);
    site_by_split.insert("train".to_string(), vec!["A".to_string()]);
    let mut future = std::collections::BTreeMap::new();
    future.insert("S003".to_string(), "2025-06-01".to_string());

    let witnesses = [
        LeakageWitness::IdentityLeakage {
            alias: "ALT-77".into(),
            subjects: vec!["S001".into(), "S003".into()],
            splits: vec!["test".into(), "train".into()],
        },
        LeakageWitness::SiteLeakage { site_by_split },
        LeakageWitness::TemporalLeakage {
            decision_time: "2025-01-01".into(),
            future_label_sources: future,
        },
        LeakageWitness::PreprocessingLeakage {
            detail: "preprocessing fit used all subjects before split".into(),
        },
    ];

    let encoded: Vec<Value> = witnesses
        .iter()
        .map(|w| serde_json::to_value(w).unwrap())
        .collect();
    assert_eq!(
        encoded,
        vec![
            json!({
                "type": "identity_leakage",
                "alias": "ALT-77",
                "subjects": ["S001", "S003"],
                "splits": ["test", "train"]
            }),
            json!({
                "type": "site_leakage",
                "site_by_split": { "test": ["B"], "train": ["A"] }
            }),
            json!({
                "type": "temporal_leakage",
                "decision_time": "2025-01-01",
                "future_label_sources": { "S003": "2025-06-01" }
            }),
            json!({
                "type": "preprocessing_leakage",
                "detail": "preprocessing fit used all subjects before split"
            }),
        ]
    );
}

#[test]
fn a_domain_check_witness_round_trips_and_is_a_violation_not_a_score() {
    let mut observed = std::collections::BTreeMap::new();
    observed.insert("buyer_account".to_string(), "\"ACC-9\"".to_string());
    observed.insert("seller_account".to_string(), "\"ACC-9\"".to_string());
    let witness = LeakageWitness::DomainCheck {
        check: "self_cross".into(),
        observed,
        detail: "buyer and seller resolve to the same account".into(),
    };
    assert_eq!(witness.kind(), "domain_check");

    let encoded = serde_json::to_value(&witness).unwrap();
    assert_eq!(encoded["type"], json!("domain_check"));
    let decoded: LeakageWitness = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, witness);

    let verdict = OracleVerdict::new("rule/trade-surveillance-v1", vec![witness]);
    assert_eq!(verdict.status, OracleStatus::Invalid);
    assert_eq!(verdict.witness_kinds(), vec!["domain_check"]);
}
