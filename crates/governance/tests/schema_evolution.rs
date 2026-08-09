//! End-to-end governance of the wire formats this workspace actually ships.
//!
//! The unit tests inside the crate check the rules. These check that the rules, applied to the
//! real `fiber-context-certificate` documents, reproduce the decisions the workspace already made
//! for other reasons — and, where they can, prove the digest claims by hashing rather than by
//! asserting them.

use bioprism_governance::known::{certificate_extended, certificate_reference};
use bioprism_governance::{
    apply_mode, classify, diff, negotiate, ClaimAudit, CompatibilityClass, CompatibilityError,
    CompatibilityMode, DeprecationError, DeprecationLedger, FieldSpec, FieldType, GovernanceError,
    LossItem, Migration, MigrationError, MigrationStep, SchemaDescriptor, SchemaId, VersionBump,
    SCHEMA_VERSION_KEY,
};
use bioprism_ids::ContentHash;
use bioprism_section::{
    Backend, CertificateProfile, CertificateVerification, ContextCertificate, InfluenceClass,
    OmissionGroup, OmissionManifest, OracleStatus, OracleVerdict, PlanDescriptor,
    ReferenceOmissions, SourceHashes, CERTIFICATE_SCHEMA_VERSION,
    CERTIFICATE_SCHEMA_VERSION_EXTENDED,
};
use serde_json::{json, Value};

fn certificate() -> ContextCertificate {
    ContextCertificate {
        world_id: "world-1".into(),
        query_id: "query-1".into(),
        selected_facts: vec!["f1".into(), "f2".into()],
        selected_factors: vec!["k1".into()],
        protected_closure: vec!["f1".into()],
        omissions: ReferenceOmissions {
            total_facts: 5,
            exploratory_facts: 2,
            classification: "bounded".into(),
            inaccessible_selected_before_cut: Vec::new(),
        },
        plan: PlanDescriptor {
            backend: Backend::BackwardFactorSliceReference,
            compiled_factor_count: 1,
            compiled_fact_count: 2,
            total_factor_count: 3,
            total_fact_count: 5,
            max_selected_factor_arity: 2,
            fallback: None,
        },
        oracle: OracleVerdict {
            status: OracleStatus::Valid,
            witnesses: Vec::new(),
            oracle_kind: "split-integrity".into(),
        },
        source_hashes: SourceHashes {
            world_sha256: "0".repeat(64),
            query_sha256: "1".repeat(64),
            decision_section_sha256: "2".repeat(64),
        },
        limitations: vec!["research use only".into()],
        manifest: OmissionManifest {
            groups: vec![OmissionGroup {
                reason: "outside the protected closure and the factor scope".into(),
                influence: InfluenceClass::Zero,
                count: 3,
                bound: None,
                examples: vec!["f3".into()],
            }],
        },
    }
}

fn document(profile: CertificateProfile) -> Value {
    certificate()
        .to_json(profile)
        .expect("a certificate serialises")
}

#[test]
fn reviewing_the_shipped_extended_profile_reproduces_the_decision_the_workspace_already_made() {
    let change = diff(&certificate_reference(), &certificate_extended()).expect("one lineage");
    let review = classify(&change);

    assert_eq!(review.class, CompatibilityClass::Breaking);
    assert_eq!(review.required_bump, VersionBump::Major);
    review
        .version_gate()
        .expect("0.1 -> 0.2 under a zero major carries a breaking change");

    let audit = review.audit_claim(CompatibilityClass::ForwardCompatible);
    assert!(
        !audit.is_upheld(),
        "the extended profile adds two required hashed fields; it is not merely forward compatible"
    );
    match audit {
        ClaimAudit::Contradicted { evidence, .. } => assert_eq!(evidence.len(), 2),
        ClaimAudit::Upheld { .. } => unreachable!(),
    }
}

#[test]
fn a_reader_that_ignores_what_it_does_not_understand_destroys_a_certificate_it_merely_read() {
    let extended = document(CertificateProfile::Extended);
    assert!(
        ContextCertificate::verify(&extended)
            .expect("verification runs")
            .is_valid(),
        "the fixture must verify before anything is done to it"
    );

    let unknown = certificate_reference().check_document(&extended).unknown;
    assert_eq!(unknown, ["omission_manifest", "supports_sufficiency_claim"]);

    let (ignored, ignore_report) =
        apply_mode(CompatibilityMode::Ignore, &extended, &unknown, |_| true)
            .expect("ignore never errors");
    assert!(ignore_report.would_alter_digest());
    assert!(matches!(
        ContextCertificate::verify(&ignored).expect("verification runs"),
        CertificateVerification::DigestMismatch { .. }
    ));

    let (preserved, preserve_report) = apply_mode(
        CompatibilityMode::PreserveAndForward,
        &extended,
        &unknown,
        |_| true,
    )
    .expect("preserve never errors");
    assert!(!preserve_report.would_alter_digest());
    assert!(ContextCertificate::verify(&preserved)
        .expect("verification runs")
        .is_valid());

    assert_eq!(
        certificate_reference().mode,
        CompatibilityMode::PreserveAndForward,
        "this is why the format declares the mode it declares"
    );
}

#[test]
fn a_writer_and_a_reader_that_declare_different_modes_never_get_as_far_as_exchanging_bytes() {
    let error = negotiate(certificate_reference().mode, CompatibilityMode::Ignore)
        .expect_err("43.35 requires the mode to be declared, and these declarations disagree");
    assert!(matches!(
        error,
        CompatibilityError::ModeDisagreement { .. }
    ));
}

#[test]
fn an_optional_field_with_no_default_evolves_the_certificate_without_moving_any_digest() {
    let mut fields = certificate_reference().fields().to_vec();
    fields.push(FieldSpec::optional("advisory", FieldType::String));
    let next = SchemaDescriptor::new(
        SchemaId::parse("fiber-context-certificate/0.1.1").expect("parses"),
        CompatibilityMode::PreserveAndForward,
        fields,
    )
    .expect("well formed");

    let review = classify(&diff(&certificate_reference(), &next).expect("one lineage"));
    assert_eq!(review.class, CompatibilityClass::Compatible);
    assert!(review.digest_affecting().is_empty());
    review
        .assert_class(CompatibilityClass::Compatible)
        .expect("the one addition a hashed format can absorb");

    let existing = document(CertificateProfile::Reference);
    let before = ContentHash::of_value(&existing).expect("hashes");
    assert!(
        next.check_document(&existing).conforms(),
        "an existing artifact is still a valid document under the new version"
    );
    assert_eq!(
        before,
        ContentHash::of_value(&existing).expect("hashes"),
        "and nothing about reading it under the new version changed its bytes"
    );
    assert!(ContextCertificate::verify(&existing)
        .expect("verification runs")
        .is_valid());
}

fn downgrade() -> Migration {
    Migration::new(
        SchemaId::parse(CERTIFICATE_SCHEMA_VERSION_EXTENDED).expect("parses"),
        SchemaId::parse(CERTIFICATE_SCHEMA_VERSION).expect("parses"),
        vec![
            MigrationStep::Replace {
                path: SCHEMA_VERSION_KEY.into(),
                from: json!(CERTIFICATE_SCHEMA_VERSION_EXTENDED),
                to: json!(CERTIFICATE_SCHEMA_VERSION),
            },
            MigrationStep::Drop {
                path: "omission_manifest".into(),
            },
            MigrationStep::Drop {
                path: "supports_sufficiency_claim".into(),
            },
        ],
    )
    .expect("distinct versions")
}

#[test]
fn projecting_an_extended_certificate_back_to_the_reference_profile_must_declare_what_it_drops() {
    let corpus = vec![document(CertificateProfile::Extended)];

    let error = downgrade()
        .audit_loss(&corpus)
        .expect_err("dropping the influence-classified manifest is not a silent operation");
    assert!(matches!(
        error,
        MigrationError::UndeclaredLoss { ref path, .. } if path == "omission_manifest"
    ));

    let declared = downgrade().declaring_loss(vec![
        LossItem::new(
            "omission_manifest",
            "the v0.1 wire format has no room for influence-classified omission groups; a v0.1 \
             projection cannot support a sufficiency claim",
        ),
        LossItem::new(
            "supports_sufficiency_claim",
            "derived from the manifest, so it cannot outlive it",
        ),
    ]);
    let audit = declared.audit_loss(&corpus).expect("declared loss is allowed");
    assert!(!audit.lossless());
    assert!(!audit.invertible);
    assert_eq!(audit.lost, ["omission_manifest", "supports_sufficiency_claim"]);

    let report = declared.totality_over(&corpus, &certificate_extended(), &certificate_reference());
    assert!(
        report.is_total(),
        "every extended certificate must produce a valid reference certificate: {report:?}"
    );
}

#[test]
fn a_migrated_certificate_no_longer_matches_its_own_embedded_digest() {
    let extended = document(CertificateProfile::Extended);
    let projected = downgrade().apply(&extended).expect("the projection applies");

    assert!(matches!(
        ContextCertificate::verify(&projected).expect("verification runs"),
        CertificateVerification::DigestMismatch { .. }
    ));
    assert!(
        certificate_reference().check_document(&projected).is_clean(),
        "the projection is a structurally valid v0.1 document — it is only its identity that did \
         not survive"
    );
}

#[test]
fn a_field_cannot_leave_a_shipped_format_without_a_finished_lifecycle() {
    let kept: Vec<FieldSpec> = certificate_reference()
        .fields()
        .iter()
        .filter(|field| field.path != "limitations")
        .cloned()
        .collect();
    let next = SchemaDescriptor::new(
        SchemaId::parse("fiber-context-certificate/0.3").expect("parses"),
        CompatibilityMode::PreserveAndForward,
        kept,
    )
    .expect("well formed");
    let change = diff(&certificate_reference(), &next).expect("one lineage");

    let findings = DeprecationLedger::new().audit_removals(&change);
    assert!(matches!(
        findings.as_slice(),
        [DeprecationError::RemovedWithoutRecord { path }] if path == "limitations"
    ));

    let review = classify(&change);
    assert_eq!(review.class, CompatibilityClass::Breaking);
    let error = review
        .assert_class(CompatibilityClass::Compatible)
        .expect_err("removing a hashed field moves the digest of every existing certificate");
    assert!(matches!(error, GovernanceError::Digest(_)));
}
