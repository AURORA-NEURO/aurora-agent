//! The `signed_result_bundle_replay` claim, walked end to end — including the half that is still
//! out of reach.
//!
//! `bioprism-examples` states the claim as: *"a signed result bundle verifies independently of the
//! runtime that produced it"*, and records it blocked because *"no signing or bundle crate is in
//! this crate's dependency set; certificates are content-addressed but unsigned"*.
//!
//! The claim has two words doing work. **"Bundle"** is now available, and so is authentication over
//! it. **"Independently"** splits in two:
//!
//! - Independent of the *runtime*: reachable, and exercised below. Nothing in the verify path links
//!   `bioprism-fiber`; this crate depends on `bioprism-ids` and `bioprism-section` only, and
//!   `bioprism-section` deliberately depends on neither the world model nor the compiler.
//! - Independent of the *producer*: **not** reachable, and the negative tests below pin down why. A
//!   verifier must hold the producer's secret, and a verifier holding it can mint an identical
//!   bundle. That gap needs a public-key signature, which cannot be built from `sha2`.

use bioprism_bundle::{
    Attestation, AttestationCheck, AttestationPurpose, AttestedBundle, ClaimedProducer,
    EmbeddedCertificate, EntryRole, EnvironmentFacts, KeyIdentity, ProvenanceState,
    RecordedProvenance, RejectedProvenance, RejectionReason, ReproductionAttempt,
    ReproductionVerdict, ResultBundle, SecretKey, ToolchainFacts, ToolchainPolicy,
};
use bioprism_ids::ContentHash;
use bioprism_section::{
    Backend, CertificateProfile, ContextCertificate, InfluenceClass, OmissionGroup,
    OmissionManifest, OracleStatus, OracleVerdict, PlanDescriptor, ReferenceOmissions, SourceHashes,
};
use serde_json::{json, Value};

fn producer_key() -> SecretKey {
    SecretKey::new(KeyIdentity::new("aurora-hub-2026"), vec![0x7a; 32])
}

fn reviewer_key_with_no_shared_secret() -> SecretKey {
    SecretKey::new(KeyIdentity::new("reviewer-14"), vec![0x0f; 32])
}

fn certificate() -> ContextCertificate {
    ContextCertificate {
        world_id: "onco-slice-1".into(),
        query_id: "q-split-integrity".into(),
        selected_facts: vec!["f-alt77".into(), "f-site".into()],
        selected_factors: vec!["x-split".into()],
        protected_closure: vec!["f-alt77".into()],
        omissions: ReferenceOmissions {
            total_facts: 9,
            exploratory_facts: 7,
            classification: "bounded".into(),
            inaccessible_selected_before_cut: vec![],
        },
        plan: PlanDescriptor {
            backend: Backend::BackwardFactorSliceReference,
            compiled_factor_count: 1,
            compiled_fact_count: 2,
            total_factor_count: 4,
            total_fact_count: 9,
            max_selected_factor_arity: 2,
            fallback: None,
        },
        oracle: OracleVerdict {
            status: OracleStatus::Valid,
            witnesses: vec![],
            oracle_kind: "split_integrity".into(),
        },
        source_hashes: SourceHashes {
            world_sha256: "a".repeat(64),
            query_sha256: "b".repeat(64),
            decision_section_sha256: "c".repeat(64),
        },
        limitations: vec!["synthetic reference world".into()],
        manifest: OmissionManifest {
            groups: vec![OmissionGroup {
                reason: "no dependency path to the target".into(),
                influence: InfluenceClass::Zero,
                count: 7,
                bound: None,
                examples: vec!["f-noise-1".into()],
            }],
        },
    }
}

fn section() -> Value {
    json!({
        "schema_version": "fiber-decision-section/0.1",
        "layers": {"l0": ["f-alt77"], "l1": ["f-site"]}
    })
}

fn query() -> Value {
    json!({"target": "split_integrity", "cut": "2026-01-01"})
}

fn bundle() -> ResultBundle {
    ResultBundle::builder("run-onco-slice-1")
        .carrying_certificate("certificate", &certificate(), CertificateProfile::Extended)
        .expect("carries the certificate")
        .carrying("section", EntryRole::DecisionSection, section())
        .expect("carries the section")
        .carrying("query", EntryRole::Query, query())
        .expect("carries the query")
        .referencing(
            "world",
            EntryRole::World,
            ContentHash::of_bytes(b"onco-slice-1 world bytes"),
            Some("registry://bioworlds/onco-slice-1".into()),
            ProvenanceState::Recorded(
                RecordedProvenance::new(
                    "registry://bioworlds",
                    ContentHash::of_bytes(b"onco-slice-1 world bytes"),
                )
                .pinned_at("rev-2026-08-01"),
            ),
        )
        .referencing(
            "oracle",
            EntryRole::Oracle,
            ContentHash::of_bytes(b"split integrity oracle"),
            None,
            ProvenanceState::Unrecorded,
        )
        .in_environment(
            EnvironmentFacts::undeclared()
                .with_os("linux")
                .with_arch("x86_64")
                .with_container_image_digest("sha256:".to_string() + &"d".repeat(64)),
        )
        .with_toolchain(
            ToolchainFacts::declared()
                .with_rustc_version("1.85.0")
                .with_crate_version("bioprism-section", "0.1.0")
                .with_crate_version("bioprism-ids", "0.1.0"),
        )
        .build()
        .expect("builds")
}

fn attested() -> AttestedBundle {
    AttestedBundle::produce(
        bundle(),
        &producer_key(),
        AttestationPurpose::PublisherManifest,
        ClaimedProducer::new("AURORA BioPRISM Working Group"),
    )
    .expect("attests")
}

#[test]
fn a_bundle_verifies_after_transport_without_linking_the_runtime_that_produced_it() {
    let wire = serde_json::to_string(&attested()).expect("serialises");
    let received: AttestedBundle = serde_json::from_str(&wire).expect("deserialises");

    let (verified, authenticated) = received
        .verify(&producer_key())
        .expect("verifies from bytes alone");

    assert_eq!(verified.certificate(), EmbeddedCertificate::SelfVerified);
    assert_eq!(authenticated.key_identity().as_str(), "aurora-hub-2026");
    assert_eq!(
        verified.entry_checks().len(),
        5,
        "certificate, section, query, world, oracle"
    );
    assert_eq!(
        verified.not_recomputed(),
        vec!["oracle", "world"],
        "referenced entries are reported, never counted as passing checks"
    );
}

#[test]
fn replaying_the_bundle_reproduces_it_and_names_what_was_not_compared() {
    let attested = attested();
    let replay = ReproductionAttempt::on_toolchain(
        ToolchainFacts::declared()
            .with_rustc_version("1.85.0")
            .with_crate_version("bioprism-section", "0.1.0")
            .with_crate_version("bioprism-ids", "0.1.0"),
    )
    .under_policy(ToolchainPolicy::RequireDeclaredMatch)
    .producing(
        "certificate",
        certificate()
            .to_json(CertificateProfile::Extended)
            .expect("renders"),
    )
    .producing("section", section())
    .producing("query", query())
    .replay(&attested.bundle);

    let ReproductionVerdict::Reproduced(reproduced) = &replay else {
        panic!("expected reproduction, got {replay:?}");
    };
    assert_eq!(reproduced.compared(), ["certificate", "query", "section"]);
    assert_eq!(
        reproduced.not_compared(),
        ["oracle", "world"],
        "a reproduction claims exactly the entries it compared"
    );
    assert!(!reproduced.is_complete());
    assert_eq!(replay.status_word(), "reproduced");
}

#[test]
fn a_replay_that_recompiles_a_different_section_diverges_and_names_the_entry() {
    let replay = ReproductionAttempt::on_toolchain(ToolchainFacts::declared())
        .producing(
            "certificate",
            certificate()
                .to_json(CertificateProfile::Extended)
                .expect("renders"),
        )
        .producing("query", query())
        .producing(
            "section",
            json!({
                "schema_version": "fiber-decision-section/0.1",
                "layers": {"l0": ["f-alt77"], "l1": []}
            }),
        )
        .replay(&bundle());

    let ReproductionVerdict::Diverged { first_divergence } = &replay else {
        panic!("expected divergence, got {replay:?}");
    };
    assert_eq!(first_divergence.entry, "section");
    assert_eq!(
        first_divergence.matched_before,
        vec!["certificate".to_string(), "query".to_string()]
    );
    assert_eq!(replay.status_word(), "non-reproducible");
    assert!(replay.honest_label().contains("diverged at entry `section`"));
}

#[test]
fn a_reviewer_without_the_producers_key_cannot_check_the_attestation_at_all() {
    let attested = attested();
    let check = attested.attestation.verify(&reviewer_key_with_no_shared_secret());
    assert_eq!(
        check,
        AttestationCheck::WrongKeyOffered {
            attested: KeyIdentity::new("aurora-hub-2026"),
            offered: KeyIdentity::new("reviewer-14"),
        },
        "this is the shape of the remaining blockage: without the producing secret there is no \
         verification path at all, not even a failing one"
    );

    let verified = attested.bundle.verify().expect("integrity still checks");
    assert_eq!(
        verified.certificate(),
        EmbeddedCertificate::SelfVerified,
        "content integrity is verifiable by anyone; origin is not"
    );
}

#[test]
fn a_reviewer_who_can_verify_can_also_forge_an_identical_bundle() {
    let genuine = attested();
    let shared_secret_in_reviewers_hands =
        SecretKey::new(KeyIdentity::new("aurora-hub-2026"), vec![0x7a; 32]);

    genuine
        .verify(&shared_secret_in_reviewers_hands)
        .expect("the reviewer can verify");

    let forgery = AttestedBundle::produce(
        bundle(),
        &shared_secret_in_reviewers_hands,
        AttestationPurpose::PublisherManifest,
        ClaimedProducer::new("AURORA BioPRISM Working Group"),
    )
    .expect("and can therefore also produce");

    assert_eq!(
        forgery, genuine,
        "byte-identical, which is precisely what 'no third-party verifiability' means"
    );
}

#[test]
fn a_verified_result_names_the_key_and_never_the_self_reported_producer() {
    let attested = attested();
    assert_eq!(
        attested.attestation.claimed_producer.claimed_name(),
        "AURORA BioPRISM Working Group"
    );

    let (_, authenticated) = attested.verify(&producer_key()).expect("verifies");
    let rendered = format!(
        "{} {}",
        authenticated.honest_label(),
        serde_json::to_string(&authenticated).expect("serialises")
    );
    assert!(rendered.contains("aurora-hub-2026"));
    assert!(
        !rendered.contains("AURORA BioPRISM Working Group"),
        "a verification result carries no route to a principal: {rendered}"
    );
}

#[test]
fn the_bundle_reports_recorded_unrecorded_and_rejected_provenance_separately() {
    let mixed = ResultBundle::builder("run-mixed")
        .carrying("section", EntryRole::DecisionSection, section())
        .expect("carries")
        .referencing(
            "world",
            EntryRole::World,
            ContentHash::of_bytes(b"w"),
            None,
            ProvenanceState::Recorded(
                RecordedProvenance::new("registry://bioworlds", ContentHash::of_bytes(b"w"))
                    .pinned_at("rev-1"),
            ),
        )
        .referencing(
            "pack",
            EntryRole::BenchmarkPack,
            ContentHash::of_bytes(b"p"),
            None,
            ProvenanceState::Rejected(RejectedProvenance::new(
                "registry://packs/onco:latest",
                RejectionReason::FloatingRevision {
                    reference: "latest".into(),
                },
            )),
        )
        .build()
        .expect("builds");

    let posture = mixed
        .verify()
        .expect("verifies")
        .supply_chain_posture()
        .clone();
    assert_eq!(posture.recorded, vec!["world".to_string()]);
    assert_eq!(posture.unrecorded, vec!["section".to_string()]);
    assert_eq!(posture.rejected, vec!["pack".to_string()]);
    assert!(!posture.is_fully_recorded());
    assert!(
        posture.honest_label().contains("nobody checked"),
        "an unrecorded input is never rendered as untrusted"
    );
}

#[test]
fn a_hub_receipt_tag_cannot_be_passed_off_as_a_publisher_attestation() {
    let receipt = Attestation::produce(
        AttestationPurpose::HubPublicationReceipt,
        bundle().verify().expect("verifies").manifest_digest().clone(),
        &producer_key(),
        ClaimedProducer::new("AURORA BioAtlas"),
    )
    .expect("attests");

    assert!(!receipt
        .verify_for(&producer_key(), AttestationPurpose::PublisherManifest)
        .is_authenticated());
    assert!(receipt
        .verify_for(&producer_key(), AttestationPurpose::HubPublicationReceipt)
        .is_authenticated());
}

#[test]
fn the_whole_pipeline_is_deterministic_across_two_independent_constructions() {
    assert_eq!(attested(), attested());
    assert_eq!(
        attested().attestation.tag,
        attested().attestation.tag,
        "no clock and no RNG: the same inputs always give the same tag"
    );
}
