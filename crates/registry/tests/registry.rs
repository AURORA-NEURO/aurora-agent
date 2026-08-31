//! Invariant tests for the registry, built on real generated worlds and real mutation lineage.
//!
//! Fixtures come from `bioprism_worldgen` (43.39 structural worlds) through
//! `bioprism_mutation::generate` (32, Gate 3), so the packs under test carry lineage that was
//! actually validated by an oracle rather than lineage that was typed into a literal. The
//! adversarial packs are then produced by editing those honest packs, which is exactly the threat
//! model: a publisher who ran the pipeline and then improved the numbers.

use bioprism_mutation::{
    generate as generate_family, standard_suite, Family, Mutation, MutationKind,
};
use bioprism_prism::{Attestation, DecisionCell, InputRef};
use bioprism_registry::{
    assess, evaluate_tier, gate, gate_document, promote, BenchmarkPack, GateFinding, GateOutcome,
    OracleDisagreement, PackError, PackInstance, ParentRef, Policy, PostconditionEvidence,
    PromotionError, PublicationEvent, RebuildAttestation, RegistryError, RegistryIndex, Resolution,
    ReviewFinding, ReviewRecord, TierPolicy, TierVerdict, TrustTier,
};
use bioprism_section::OracleStatus;
use bioprism_worldgen::{DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PUBLISHER: &str = "aurora-bioprism";

fn spec(n: usize) -> WorldSpec {
    WorldSpec {
        world_id: format!("parent-world-{n}"),
        subjects: 4,
        distractors: 3 + n,
        relay_depth: n % 2,
        attachment: DistractorAttachment::Hub,
        tag_style: TagStyle::Distinct,
        leakage: LeakageMechanism::ALL.to_vec(),
        seed: 20_260_808 + n as u64,
        ..WorldSpec::reference_like(3 + n)
    }
}

fn family(n: usize) -> Family {
    let world = bioprism_worldgen::generate(&spec(n)).world;
    generate_family(&world, &standard_suite()).expect("worldgen worlds are mutable")
}

fn decision_cell() -> DecisionCell {
    let generated = bioprism_worldgen::generate(&spec(0));
    DecisionCell::new(
        "cell.split-integrity",
        "Is this train/test split contaminated, and by which mechanism?",
        InputRef::new("worldgen://parent-world-0", &generated.world),
        InputRef::new("worldgen://parent-world-0/query", &generated.query),
    )
    .accepting(OracleStatus::Invalid)
    .requiring_witness("identity_leakage")
}

/// An honest pack over `parents` independent parent worlds, without independent evidence.
fn honest_pack(parents: usize) -> BenchmarkPack {
    let mut builder = BenchmarkPack::builder("oncoworld/split-integrity", "1.0.0")
        .intended_use(
            "Regression family for split-integrity decisions on synthetic structural worlds.",
        )
        .publisher(PUBLISHER)
        .license("Apache-2.0")
        .limited_by("Synthetic worlds only; establishes nothing about observed biological cohorts.")
        .cell(decision_cell());
    for n in 0..parents {
        builder = builder.family(&family(n), "synthetic, bioprism-worldgen 43.39");
    }
    builder.build().expect("families carry their own parents")
}

/// Attaches a rebuild and an approving review, both naming the pack's current core digest.
fn with_independent_evidence(mut pack: BenchmarkPack) -> BenchmarkPack {
    let core = pack.core_digest().expect("digestible").as_str().to_string();
    pack.provenance.rebuilds.push(RebuildAttestation {
        rebuilt_by: "independent-mirror".into(),
        rebuilt_core_sha256: core.clone(),
        command: "bioprism pack rebuild oncoworld/split-integrity@1.0.0".into(),
    });
    pack.provenance.reviews.push(ReviewRecord {
        reviewer: "reviewer-b".into(),
        reviewed_core_sha256: core,
        finding: ReviewFinding::Approved,
        notes: "Parent worlds and mutation relations inspected.".into(),
    });
    pack
}

fn gold_pack() -> BenchmarkPack {
    with_independent_evidence(honest_pack(5))
}

#[test]
fn a_pack_built_from_validated_families_attests_to_itself_and_a_third_party_can_verify_it() {
    let pack = honest_pack(3);
    let document = pack.attest().expect("digestible");

    assert!(BenchmarkPack::verify(&document).is_valid());
    let reloaded = BenchmarkPack::from_attested(&document).expect("verified document reloads");
    assert_eq!(reloaded, pack);

    assert!(!pack.instances.is_empty());
    assert_eq!(pack.diversity.instances, pack.instances.len());
    assert!(pack.orphans().is_empty());
    assert!(pack.unvalidated().is_empty());
    assert!(pack.yield_ledger.is_consistent());
}

#[test]
fn tampering_with_an_attested_pack_document_is_detected() {
    let pack = honest_pack(3);
    let mut document = pack.attest().expect("digestible");
    document["diversity"]["equivalence_classes"] = json!(9_999);

    match BenchmarkPack::verify(&document) {
        Attestation::Mismatch {
            claimed,
            recomputed,
        } => assert_ne!(claimed, recomputed),
        other => panic!("edited document should not verify: {other:?}"),
    }
    assert!(matches!(
        BenchmarkPack::from_attested(&document),
        Err(PackError::AttestationFailed(_))
    ));
}

#[test]
fn a_document_without_a_digest_is_malformed_rather_than_trusted() {
    let pack = honest_pack(2);
    let body = serde_json::to_value(&pack).expect("serialisable");
    assert!(matches!(
        BenchmarkPack::verify(&body),
        Attestation::Malformed(_)
    ));
}

#[test]
fn a_pack_cannot_be_promoted_past_what_its_evidence_supports() {
    let pack = honest_pack(5);

    let granted = promote(&pack, TrustTier::GeneratedVerified).expect("lineage is validated");
    assert_eq!(granted.granted, TrustTier::GeneratedVerified);

    let refused = promote(&pack, TrustTier::Gold).expect_err("no independent evidence");
    let PromotionError::EvidenceInsufficient {
        earned, ref unmet, ..
    } = refused
    else {
        panic!("expected insufficient evidence, got {refused:?}");
    };
    assert!(earned < TrustTier::Gold);
    assert!(unmet.iter().any(
        |entry| entry.requirement == bioprism_registry::Requirement::IndependentRebuildVerified
    ));
    assert!(unmet.iter().any(
        |entry| entry.requirement == bioprism_registry::Requirement::IndependentReviewApproved
    ));
}

#[test]
fn a_pack_with_an_independent_rebuild_and_review_earns_gold() {
    let pack = gold_pack();
    let (tier, blockers) = evaluate_tier(&pack);
    assert_eq!(tier, TrustTier::Gold, "blocked by {blockers:?}");
    assert!(blockers.is_empty());

    let promotion = promote(&pack, TrustTier::Gold).expect("evidence supports gold");
    assert_eq!(promotion.granted, TrustTier::Gold);
    assert_eq!(promotion.earned, TrustTier::Gold);
    assert!(promotion
        .satisfied
        .contains(&bioprism_registry::Requirement::IndependentReviewApproved));
}

#[test]
fn editing_the_content_of_a_pack_detaches_every_review_of_it() {
    let mut pack = gold_pack();
    assert_eq!(evaluate_tier(&pack).0, TrustTier::Gold);

    pack.intended_use = "Quietly repurposed after review.".into();

    let verdict = bioprism_registry::reassess(&pack, TrustTier::Gold, &TierPolicy::default());
    let TierVerdict::Demoted {
        claimed,
        earned,
        ref reasons,
    } = verdict
    else {
        panic!("a review of earlier content must not carry forward: {verdict:?}");
    };
    assert_eq!(claimed, TrustTier::Gold);
    assert_eq!(
        earned,
        TrustTier::GeneratedVerified,
        "an edit detaches the rebuild attestation too, not only the review"
    );
    assert!(reasons
        .iter()
        .any(|entry| entry.detail.contains("does not carry forward")));
    assert!(reasons.iter().any(
        |entry| entry.requirement == bioprism_registry::Requirement::IndependentRebuildVerified
    ));
}

#[test]
fn a_publisher_reviewing_their_own_pack_does_not_earn_gold() {
    let mut pack = honest_pack(5);
    let core = pack.core_digest().expect("digestible").as_str().to_string();
    pack.provenance.rebuilds.push(RebuildAttestation {
        rebuilt_by: PUBLISHER.into(),
        rebuilt_core_sha256: core.clone(),
        command: "self".into(),
    });
    pack.provenance.reviews.push(ReviewRecord {
        reviewer: PUBLISHER.into(),
        reviewed_core_sha256: core,
        finding: ReviewFinding::Approved,
        notes: "Looks good to me.".into(),
    });

    let (tier, blockers) = evaluate_tier(&pack);
    assert_eq!(tier, TrustTier::GeneratedVerified);
    assert!(blockers
        .iter()
        .any(|entry| entry.detail.contains("none both independent")));
}

#[test]
fn an_instance_whose_postcondition_was_never_checked_blocks_the_gate() {
    let mut pack = honest_pack(5);
    let borrowed = pack.instances[0].clone();
    let mut unchecked = borrowed.clone();
    unchecked.instance.id = format!("{}-unchecked", borrowed.id());
    unchecked.postcondition = PostconditionEvidence::NotChecked {
        relation: "reorder".into(),
        reason: "oracle run timed out; counted as passing".into(),
    };
    pack.instances.push(unchecked);
    pack.diversity.instances = pack.instances.len();
    pack.yield_ledger.accepted = pack.instances.len();
    pack.yield_ledger.attempted =
        pack.yield_ledger.accepted + pack.yield_ledger.rejected + pack.yield_ledger.duplicates;

    let outcome = gate(&pack, &Policy::default());
    assert!(outcome.is_block(), "{}", outcome.report());
    assert!(outcome
        .findings()
        .iter()
        .any(|finding| matches!(finding, GateFinding::UnvalidatedInstance { .. })));
    assert_eq!(outcome.exit_code(), 2);
}

#[test]
fn a_pack_that_inflates_its_instance_count_without_reporting_effective_diversity_is_blocked() {
    let mut pack = honest_pack(5);
    let clone_of = pack.instances[0].clone();
    let mut duplicate = clone_of.clone();
    duplicate.instance.id = format!("{}-copy", clone_of.id());
    pack.instances.push(duplicate);

    let outcome = gate(&pack, &Policy::default());
    assert!(outcome.is_block(), "{}", outcome.report());
    assert!(outcome
        .findings()
        .iter()
        .any(|finding| matches!(finding, GateFinding::EffectiveDiversityNotReported { .. })));

    let (tier, blockers) = evaluate_tier(&pack);
    assert_eq!(tier, TrustTier::Exploratory);
    assert!(blockers.iter().any(|entry| entry.requirement
        == bioprism_registry::Requirement::DiversityAccountingMatchesInstances));
}

#[test]
fn a_family_collapsing_below_three_equivalence_classes_may_not_be_published_as_a_benchmark() {
    let world = bioprism_worldgen::generate(&spec(0)).world;
    let narrow = vec![Mutation::new(
        "reorder-facts",
        MutationKind::ReorderFacts { seed: 7 },
    )];
    let thin = generate_family(&world, &narrow).expect("mutable");

    let pack = BenchmarkPack::builder("oncoworld/thin", "0.1.0")
        .intended_use("One invariance check.")
        .publisher(PUBLISHER)
        .license("Apache-2.0")
        .limited_by("A single relation over a single parent.")
        .family(&thin, "synthetic")
        .build()
        .expect("builds");

    assert!(!pack.diversity.is_publishable());
    let outcome = gate(&pack, &Policy::experimental());
    assert!(outcome.is_block(), "{}", outcome.report());
    assert!(outcome
        .findings()
        .iter()
        .any(|finding| matches!(finding, GateFinding::DiversityCollapsed { .. })));
    assert!(outcome.report().contains("robustness check"));
}

#[test]
fn a_pack_without_a_license_is_demoted_to_generated_verified_with_the_reason_named() {
    let mut pack = gold_pack();
    pack.provenance.license = None;

    let assessment = assess(&pack, &TierPolicy::default());
    assert_eq!(assessment.earned, TrustTier::GeneratedVerified);
    let unmet = assessment.unmet_for(TrustTier::Reviewed);
    assert_eq!(unmet.len(), 1);
    assert_eq!(
        unmet[0].requirement,
        bioprism_registry::Requirement::LicenseDeclared
    );
    assert!(unmet[0].to_string().contains("no license declared"));
}

#[test]
fn an_unresolved_oracle_disagreement_holds_a_pack_below_reviewed() {
    fn pack_with(resolution: Resolution) -> (BenchmarkPack, String) {
        let mut pack = honest_pack(5);
        let instance_id = pack.instances[0].id().to_string();
        let mut statuses = BTreeMap::new();
        statuses.insert(
            "fiber-split-integrity/0.1".to_string(),
            "invalid".to_string(),
        );
        statuses.insert("model-judge/0.1".to_string(), "valid".to_string());
        pack.oracle_disagreements.push(OracleDisagreement {
            instance_id: instance_id.clone(),
            statuses,
            resolution,
        });
        (with_independent_evidence(pack), instance_id)
    }

    let (open, instance_id) = pack_with(Resolution::Unresolved);
    let (tier, blockers) = evaluate_tier(&open);
    assert_eq!(tier, TrustTier::GeneratedVerified);
    assert!(blockers.iter().any(|entry| entry.requirement
        == bioprism_registry::Requirement::NoUnresolvedOracleDisagreement
        && entry.detail.contains(&instance_id)));

    let (settled, _) = pack_with(Resolution::ResolvedInFavourOf {
        oracle: "fiber-split-integrity/0.1".into(),
        rationale: "Deterministic oracle outranks a model judgment (10.02).".into(),
    });
    assert_eq!(evaluate_tier(&settled).0, TrustTier::Gold);
}

#[test]
fn an_orphan_instance_is_refused_at_build_and_blocks_the_gate() {
    let honest = honest_pack(3);
    let adopted = PackInstance {
        parent_sha256: "0".repeat(64),
        ..honest.instances[0].clone()
    };

    let error = BenchmarkPack::builder("oncoworld/orphan", "0.1.0")
        .intended_use("Orphan check.")
        .parent(ParentRef::new("known", "1".repeat(64), "synthetic"))
        .instance(adopted.clone())
        .build()
        .expect_err("27.16 forbids orphan descendants");
    assert!(matches!(error, PackError::OrphanInstance { .. }));

    let mut smuggled = honest;
    smuggled.instances.push(adopted);
    smuggled.diversity.instances = smuggled.instances.len();
    smuggled.yield_ledger.accepted = smuggled.instances.len();
    smuggled.yield_ledger.attempted = smuggled.yield_ledger.accepted
        + smuggled.yield_ledger.rejected
        + smuggled.yield_ledger.duplicates;

    let outcome = gate(&smuggled, &Policy::experimental());
    assert!(outcome.is_block(), "{}", outcome.report());
    assert!(outcome
        .findings()
        .iter()
        .any(|finding| matches!(finding, GateFinding::OrphanInstance { .. })));
}

#[test]
fn a_pack_carrying_a_non_finite_number_cannot_be_attested_and_supports_no_claim() {
    let mut pack = gold_pack();
    pack.diversity.inflation_ratio = f64::NAN;

    assert!(!pack.self_attestation().is_valid());
    let (tier, blockers) = evaluate_tier(&pack);
    assert_eq!(tier, TrustTier::Unranked);
    assert!(blockers
        .iter()
        .any(|entry| entry.requirement == bioprism_registry::Requirement::AttestationVerifies));
    assert!(gate(&pack, &Policy::experimental()).is_block());
}

#[test]
fn a_pack_that_passes_the_gate_reports_effective_diversity_and_not_only_a_count() {
    let pack = gold_pack();
    let outcome = gate(&pack, &Policy::default());
    let GateOutcome::Pass {
        tier, ref headline, ..
    } = outcome
    else {
        panic!("gold pack should pass: {}", outcome.report());
    };
    assert_eq!(tier, TrustTier::Gold);
    assert!(headline.contains("independent equivalence classes"));
    assert!(headline.contains("Instance count is not benchmark count"));
    assert_eq!(outcome.exit_code(), 0);
    assert!(outcome
        .report()
        .contains("not a judgment that the benchmark is"));
}

#[test]
fn gating_a_document_verifies_it_before_reading_anything_it_says() {
    let pack = gold_pack();
    let mut document = pack.attest().expect("digestible");
    assert!(gate_document(&document, &Policy::default()).is_pass());

    document["intended_use"] = json!("Now claims to be a clinical benchmark.");
    let outcome = gate_document(&document, &Policy::default());
    assert!(outcome.is_block());
    assert!(outcome
        .findings()
        .iter()
        .any(|finding| matches!(finding, GateFinding::AttestationInvalid { .. })));
}

#[test]
fn the_registry_stores_by_digest_and_resolves_the_human_name_to_it() {
    let pack = gold_pack();
    let mut registry = RegistryIndex::new();
    let digest = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect("gold is earned");

    assert_eq!(
        registry.resolve("oncoworld/split-integrity@1.0.0"),
        Some(digest.as_str())
    );
    assert_eq!(registry.tier_of(&digest), Some(TrustTier::Gold));
    assert_eq!(registry.load(&digest).expect("stored"), pack);
    assert!(registry.verify_all().is_empty());

    let again = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect("republishing identical content is idempotent");
    assert_eq!(again, digest);
    assert_eq!(registry.log().len(), 1);
}

#[test]
fn the_registry_refuses_a_tier_the_evidence_does_not_support() {
    let pack = honest_pack(5);
    let mut registry = RegistryIndex::new();
    let error = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect_err("no independent evidence");

    let RegistryError::TierNotEarned {
        earned, ref unmet, ..
    } = error
    else {
        panic!("expected a tier refusal, got {error:?}");
    };
    assert_eq!(earned, TrustTier::GeneratedVerified);
    assert!(!unmet.is_empty());
    assert!(registry.is_empty());
    assert!(registry.log().is_empty());
}

#[test]
fn a_correction_is_a_new_version_that_supersedes_the_old_one_which_stays_readable() {
    let original = gold_pack();
    let mut registry = RegistryIndex::new();
    let old = registry
        .publish(&original, TrustTier::Gold, &TierPolicy::default())
        .expect("publishes");

    let mut corrected = honest_pack(5);
    corrected.version = "1.0.1".into();
    corrected.limitations.push(
        "Corrects the 1.0.0 limitation statement, which omitted the leakage mechanisms covered."
            .into(),
    );
    let corrected = with_independent_evidence(corrected);

    let new = registry
        .supersede(
            &old,
            &corrected,
            TrustTier::Gold,
            "1.0.0 understated its limitations",
            &TierPolicy::default(),
        )
        .expect("supersedes");

    assert_ne!(new, old);
    assert!(registry.get(&old).is_some(), "history must stay readable");
    assert_eq!(registry.load(&old).expect("still stored"), original);
    assert!(matches!(
        registry.status(&old),
        Some(bioprism_registry::PackStatus::Superseded { .. })
    ));
    assert!(registry.status(&new).expect("stored").is_active());
    assert_eq!(
        registry.resolve("oncoworld/split-integrity@1.0.1"),
        Some(new.as_str())
    );
    assert_eq!(
        registry.resolve("oncoworld/split-integrity@1.0.0"),
        Some(old.as_str())
    );

    assert!(matches!(
        registry.supersede(
            &old,
            &corrected,
            TrustTier::Gold,
            "again",
            &TierPolicy::default()
        ),
        Err(RegistryError::AlreadySuperseded { .. })
    ));
}

#[test]
fn rebinding_a_published_version_to_different_content_is_refused() {
    let mut registry = RegistryIndex::new();
    let original = gold_pack();
    registry
        .publish(&original, TrustTier::Gold, &TierPolicy::default())
        .expect("publishes");

    let mut same_name_new_content = original.clone();
    same_name_new_content
        .limitations
        .push("Silently added after publication.".into());
    let republished = with_independent_evidence(BenchmarkPack {
        provenance: bioprism_registry::Provenance {
            publisher: PUBLISHER.into(),
            license: Some("Apache-2.0".into()),
            reviews: Vec::new(),
            rebuilds: Vec::new(),
        },
        ..same_name_new_content
    });

    let error = registry
        .publish(&republished, TrustTier::Gold, &TierPolicy::default())
        .expect_err("a version binds to exactly one digest");
    assert!(matches!(error, RegistryError::VersionAlreadyBound { .. }));
}

#[test]
fn the_publication_log_is_append_only_and_earlier_entries_are_never_rewritten() {
    let mut registry = RegistryIndex::new();
    let pack = with_independent_evidence(honest_pack(5));
    let digest = registry
        .publish(&pack, TrustTier::Reviewed, &TierPolicy::default())
        .expect("reviewed is earned");
    let first = registry.log()[0].clone();

    registry
        .promote(&digest, TrustTier::Gold, &TierPolicy::default())
        .expect("evidence supports gold");
    registry
        .withdraw(&digest, "superseded by an observed-data pack")
        .expect("withdrawable");

    assert_eq!(registry.log()[0], first);
    assert_eq!(registry.log().len(), 3);
    let sequences: Vec<u64> = registry
        .log()
        .iter()
        .map(PublicationEvent::sequence)
        .collect();
    assert_eq!(sequences, vec![0, 1, 2]);
    assert_eq!(registry.history(&digest).len(), 3);
    assert_eq!(registry.tier_of(&digest), Some(TrustTier::Gold));
    assert!(
        registry.get(&digest).is_some(),
        "withdrawal preserves bytes"
    );

    assert!(matches!(
        registry.promote(&digest, TrustTier::Reviewed, &TierPolicy::default()),
        Err(RegistryError::NotAPromotion { .. })
    ));
}

#[test]
fn a_tightened_policy_demotes_a_published_pack_and_records_why() {
    let mut registry = RegistryIndex::new();
    let pack = gold_pack();
    let digest = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect("publishes");

    let stricter = TierPolicy {
        gold_parent_floor: 50,
        ..TierPolicy::default()
    };
    let verdict = registry.reassess(&digest, &stricter).expect("stored");
    let TierVerdict::Demoted {
        claimed,
        earned,
        ref reasons,
    } = verdict
    else {
        panic!("a raised floor must demote: {verdict:?}");
    };
    assert_eq!(claimed, TrustTier::Gold);
    assert_eq!(earned, TrustTier::Reviewed);
    assert!(reasons
        .iter()
        .any(|entry| entry.detail.contains("floor is 50")));
    assert_eq!(registry.tier_of(&digest), Some(TrustTier::Reviewed));
    assert!(matches!(
        registry.log().last(),
        Some(PublicationEvent::Demoted { .. })
    ));
}

#[test]
fn a_hand_edited_registry_file_is_caught_by_reverifying_every_artifact() {
    let mut registry = RegistryIndex::new();
    let digest = registry
        .publish(&gold_pack(), TrustTier::Gold, &TierPolicy::default())
        .expect("publishes");

    let mut persisted: Value = serde_json::to_value(&registry).expect("registry serialises");
    persisted["artifacts"][&digest]["intended_use"] =
        json!("Suitable for clinical decision support.");
    let reloaded: RegistryIndex = serde_json::from_value(persisted).expect("still parses");

    let broken = reloaded.verify_all();
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].0, digest);
    assert!(matches!(broken[0].1, Attestation::Mismatch { .. }));
    assert!(matches!(
        reloaded.load(&digest),
        Err(RegistryError::AttestationFailed(_))
    ));
}

#[test]
fn review_evidence_can_be_published_without_changing_what_the_benchmark_tests() {
    let bare = honest_pack(5);
    let evidenced = with_independent_evidence(bare.clone());

    assert_eq!(
        bare.core_digest().expect("digestible"),
        evidenced.core_digest().expect("digestible"),
        "provenance is outside the core digest"
    );
    assert_ne!(
        bare.digest().expect("digestible"),
        evidenced.digest().expect("digestible"),
        "but it is a different artifact"
    );

    let name = bare.name();
    let mut registry = RegistryIndex::new();
    let first = registry
        .publish(&bare, TrustTier::GeneratedVerified, &TierPolicy::default())
        .expect("publishes");
    let second = registry
        .publish(&evidenced, TrustTier::Gold, &TierPolicy::default())
        .expect("same content, richer provenance, same version");

    assert_ne!(first, second);
    assert_eq!(registry.resolve(&name), Some(second.as_str()));
    assert_eq!(
        registry.content_of(&name),
        registry.core_digest_of(&first),
        "the version stays bound to one benchmark content"
    );
    assert_eq!(registry.tier_of(&first), Some(TrustTier::GeneratedVerified));
    assert_eq!(registry.tier_of(&second), Some(TrustTier::Gold));

    let core = registry.core_digest_of(&first).expect("stored").to_string();
    let mut revisions = registry.revisions_of_content(&core);
    revisions.sort_unstable();
    let mut expected = vec![first.as_str(), second.as_str()];
    expected.sort_unstable();
    assert_eq!(revisions, expected);
}

#[test]
fn an_unknown_digest_is_a_typed_error_rather_than_a_silent_miss() {
    let mut registry = RegistryIndex::new();
    let absent = "f".repeat(64);
    assert!(matches!(
        registry.load(&absent),
        Err(RegistryError::UnknownDigest(_))
    ));
    assert!(matches!(
        registry.withdraw(&absent, "never existed"),
        Err(RegistryError::UnknownDigest(_))
    ));
    assert!(matches!(
        registry.promote(&absent, TrustTier::Gold, &TierPolicy::default()),
        Err(RegistryError::UnknownDigest(_))
    ));
    assert!(registry.status(&absent).is_none());
}

#[test]
fn a_withdrawn_pack_cannot_be_superseded_and_a_pack_cannot_supersede_itself() {
    let pack = gold_pack();
    let mut registry = RegistryIndex::new();
    let digest = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect("publishes");

    assert!(matches!(
        registry.supersede(
            &digest,
            &pack,
            TrustTier::Gold,
            "by itself",
            &TierPolicy::default()
        ),
        Err(RegistryError::SelfSupersession(_))
    ));

    registry
        .withdraw(&digest, "parent worlds were mislicensed")
        .expect("withdrawable");
    let mut replacement = honest_pack(5);
    replacement.version = "2.0.0".into();
    let replacement = with_independent_evidence(replacement);
    assert!(matches!(
        registry.supersede(
            &digest,
            &replacement,
            TrustTier::Gold,
            "relicensed",
            &TierPolicy::default()
        ),
        Err(RegistryError::Withdrawn { .. })
    ));

    assert!(
        registry.get(&digest).is_some(),
        "10.05: metadata and reason remain visible after withdrawal"
    );
}

#[test]
fn a_published_artifact_cannot_be_quietly_republished_at_a_different_tier() {
    let pack = with_independent_evidence(honest_pack(5));
    let mut registry = RegistryIndex::new();
    registry
        .publish(&pack, TrustTier::Reviewed, &TierPolicy::default())
        .expect("reviewed is earned");

    let error = registry
        .publish(&pack, TrustTier::Gold, &TierPolicy::default())
        .expect_err("re-publication must not rewrite a recorded tier");
    assert!(matches!(error, RegistryError::AlreadyPublished { .. }));
    assert_eq!(registry.log().len(), 1);
}

/// A publisher whose digest field holds a typo has not been shown to have edited the pack.
///
/// The registry's whole job is telling a consumer whether a pack it did not build is the pack it
/// claims to be. `Attestation::Mismatch` is the answer that says somebody changed the benchmark
/// after attesting it; answering it for a `pack_sha256` that is not a digest at all names the
/// wrong party, and a tier decision made on that reading would hold an honest pack below Gold for
/// a reason its publisher cannot find.
#[test]
fn a_malformed_pack_digest_is_reported_as_malformed_and_never_as_a_mismatch() {
    let pack = honest_pack(3);
    let attested = pack.attest().expect("digestible");
    assert_eq!(BenchmarkPack::verify(&attested), Attestation::Valid);

    let claimed = attested["pack_sha256"]
        .as_str()
        .expect("a digest")
        .to_string();
    for broken in [
        String::new(),
        "not-a-digest".to_string(),
        claimed.to_ascii_uppercase(),
        claimed[..63].to_string(),
        format!("{claimed}0"),
    ] {
        let mut document = attested.clone();
        document["pack_sha256"] = Value::String(broken.clone());
        assert!(
            matches!(BenchmarkPack::verify(&document), Attestation::Malformed(_)),
            "pack_sha256 = {broken:?} is a defect in the claimed digest, not evidence that the \
             pack content changed"
        );
    }

    let mut edited = attested;
    edited["intended_use"] = json!("rewritten after publication");
    assert!(
        matches!(BenchmarkPack::verify(&edited), Attestation::Mismatch { .. }),
        "an edit to the pack body is the case Mismatch exists for, and it must still reach it"
    );
}
