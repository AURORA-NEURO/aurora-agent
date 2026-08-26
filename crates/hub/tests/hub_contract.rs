//! End-to-end invariants of the public hub, exercised through the published API only.
//!
//! The unit tests inside each module check one refusal at a time. These check that the refusals
//! still hold when the modules are composed, which is where a contract layer usually fails: each
//! piece is careful and the seam between two of them is not.

use bioprism_hub::{
    accept, lint_claim, AccessTier, Ancestor, Attribution, BioAtlasCard, Board, BoardId,
    BudgetEnvelope, BuildProvenance, ComparabilityConditions, ContaminationKind,
    ContaminationWitness, Decision, DeclaredScope, DisclosureLedger, Entry, Epoch, EvidenceScale,
    HubError, Licence, ModerationLedger, ModerationState, NonClaim, Provenance, PublicationState,
    Redistribution, Score, Submission, SubmissionDraft, SubmissionId, Submitter, SubmitterId,
    UnrankableReason, VerificationStatus,
};
use bioprism_ids::{ContentHash, IdError};

fn pack() -> ContentHash {
    ContentHash::of_bytes(b"glioma-holdout-v3")
}

fn submitter(id: &str) -> Submitter {
    Submitter::unverified(SubmitterId::parse(id).unwrap()).declaring_no_conflicts()
}

fn draft(id: &str, submitter_id: &str) -> SubmissionDraft {
    SubmissionDraft {
        id: Some(SubmissionId::parse(id).unwrap()),
        submitter: Some(SubmitterId::parse(submitter_id).unwrap()),
        content: Some(ContentHash::of_bytes(id.as_bytes())),
        scope: Some(DeclaredScope {
            disease: vec!["glioma".into()],
            modality: vec!["mri".into()],
            decision_family: vec!["evidence-acquisition".into()],
            intended_use: "compare two context compilers on one worldline".into(),
            out_of_scope: vec!["paediatric cohorts".into()],
        }),
        licence: Some(Licence::permissive("CC0-1.0")),
        provenance: Some(Provenance {
            ancestors: Vec::new(),
            build: BuildProvenance {
                toolchain: "rustc 1.85".into(),
                source_digest: ContentHash::of_bytes(b"src"),
                reproducible: true,
            },
            attestations: vec!["local-signature".into()],
        }),
        does_not_establish: vec![NonClaim::clinical_validity()],
        attributions: Vec::new(),
        evidence_scale: Some(EvidenceScale::new(400, 40)),
        claimed_verification: None,
        submitted_at: Epoch(1),
    }
}

fn conditions() -> ComparabilityConditions {
    ComparabilityConditions {
        pack: pack(),
        pack_version: "3.0.1".into(),
        split: "hidden-holdout".into(),
        metric: "first-divergence-rate".into(),
        higher_is_better: false,
        oracle_tier: "deterministic".into(),
        access_mode: AccessTier::Public,
        budget: BudgetEnvelope::unbounded(),
        protocol: ContentHash::of_bytes(b"scoring-protocol-v1"),
    }
}

fn entry(id: &str, value: f64, at: Epoch) -> Entry {
    Entry {
        submission: SubmissionId::parse(id).unwrap(),
        conditions: conditions(),
        score: Score::point(value),
        computed_at: at,
        acknowledges_disclosure: false,
        scale: EvidenceScale::new(400, 40),
    }
}

fn publish(ledger: &mut ModerationLedger, submission: Submission, from: u64) -> SubmissionId {
    let id = submission.id.clone();
    ledger.open(submission, "hub", Epoch(from)).unwrap();
    ledger
        .transition(
            &id,
            ModerationState::UnderReview,
            Decision::by("reviewer-1", Epoch(from + 1)),
        )
        .unwrap();
    ledger
        .transition(
            &id,
            ModerationState::Accepted,
            Decision::by("reviewer-1", Epoch(from + 2)),
        )
        .unwrap();
    id
}

fn board(min_verification: VerificationStatus) -> Board {
    Board {
        id: BoardId::parse("glioma-first-divergence").unwrap(),
        conditions: conditions(),
        min_verification,
    }
}

#[test]
fn a_submission_reaches_a_ranking_only_after_review_by_someone_other_than_its_author() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();

    let submission = accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap();
    let id = publish(&mut moderation, submission, 1);

    let strict = board(VerificationStatus::Verified);
    let before = strict.rank(&[entry("sub-1", 0.14, Epoch(2))], &moderation, &disclosure);
    assert!(before.ranked.is_empty());
    assert!(matches!(
        before.unranked[0].reason,
        UnrankableReason::BelowVerificationFloor { .. }
    ));

    assert!(moderation
        .attest(&id, VerificationStatus::Verified, "lab-a", Epoch(4))
        .is_err());
    moderation
        .attest(&id, VerificationStatus::Verified, "reviewer-2", Epoch(4))
        .unwrap();

    let after = strict.rank(&[entry("sub-1", 0.14, Epoch(2))], &moderation, &disclosure);
    assert_eq!(after.ranked.len(), 1);
    assert_eq!(after.ranked[0].verification, VerificationStatus::Verified);
    lint_claim(&after.headline()).unwrap();
}

#[test]
fn entries_from_two_splits_form_two_boards_and_are_never_ordered_against_each_other() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();

    publish(
        &mut moderation,
        accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap(),
        1,
    );
    publish(
        &mut moderation,
        accept(draft("sub-2", "lab-b"), &submitter("lab-b")).unwrap(),
        4,
    );

    let hidden = entry("sub-1", 0.30, Epoch(2));
    let mut public_split = entry("sub-2", 0.05, Epoch(2));
    public_split.conditions.split = "public".into();

    let classes = bioprism_hub::partition(&[hidden.clone(), public_split.clone()]);
    assert_eq!(
        classes.len(),
        2,
        "two splits must not collapse into one board"
    );

    let err = bioprism_hub::rank_order(&public_split, &hidden).expect_err("different splits");
    assert!(matches!(err, HubError::NotComparable { .. }));

    let rendered = board(VerificationStatus::SelfReported).rank(
        &[hidden, public_split],
        &moderation,
        &disclosure,
    );
    assert_eq!(rendered.ranked.len(), 1);
    assert_eq!(rendered.unranked.len(), 1);
    assert!(rendered.headline().contains("without a rank"));
}

#[test]
fn publishing_against_a_held_out_pack_discloses_it_and_later_scores_must_say_so() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();

    publish(
        &mut moderation,
        accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap(),
        1,
    );
    publish(
        &mut moderation,
        accept(draft("sub-2", "lab-b"), &submitter("lab-b")).unwrap(),
        4,
    );

    disclosure.disclose(&pack(), Epoch(5)).unwrap();

    let early = entry("sub-1", 0.30, Epoch(2));
    let late = entry("sub-2", 0.05, Epoch(9));
    let rendered = board(VerificationStatus::SelfReported).rank(
        &[early, late.clone()],
        &moderation,
        &disclosure,
    );

    assert_eq!(
        rendered.ranked.len(),
        1,
        "the post-disclosure score is not ranked"
    );
    assert_eq!(rendered.ranked[0].entry.submission.as_str(), "sub-1");
    assert!(matches!(
        rendered.unranked[0].reason,
        UnrankableReason::Ineligible { .. }
    ));

    let mut acknowledged = late;
    acknowledged.acknowledges_disclosure = true;
    let rendered = board(VerificationStatus::SelfReported).rank(
        &[entry("sub-1", 0.30, Epoch(2)), acknowledged],
        &moderation,
        &disclosure,
    );
    assert_eq!(rendered.ranked.len(), 2);
    let leader = &rendered.leaders()[0];
    assert_eq!(leader.entry.submission.as_str(), "sub-2");
    assert!(leader
        .label
        .caveat()
        .contains("not evidence of generalisation"));
}

#[test]
fn contaminating_a_pack_empties_its_board_without_deleting_any_entry() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();
    publish(
        &mut moderation,
        accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap(),
        1,
    );

    disclosure
        .record_contamination(
            &pack(),
            ContaminationWitness {
                kind: ContaminationKind::InstancesPublished,
                detail: "the full holdout appeared in a public repository".into(),
                observed_at: Epoch(8),
                reported_by: "audit-1".into(),
            },
        )
        .unwrap();

    let rendered = board(VerificationStatus::SelfReported).rank(
        &[entry("sub-1", 0.14, Epoch(2))],
        &moderation,
        &disclosure,
    );
    assert!(rendered.ranked.is_empty());
    assert_eq!(
        rendered.unranked.len(),
        1,
        "the entry is shown, not dropped"
    );
    let headline = rendered.headline();
    assert!(headline.contains("No entry on this board is currently rankable"));
    lint_claim(&headline).unwrap();
}

#[test]
fn a_derived_submission_carries_its_ancestors_terms_all_the_way_onto_the_card() {
    let ancestor_digest = ContentHash::of_bytes(b"upstream-pack");
    let attribution = Attribution {
        holder: "Neuro Consortium".into(),
        citation: "Neuro Consortium, BioAtlas pack 2026".into(),
        source: ancestor_digest.clone(),
    };
    let ancestor = Ancestor::new(
        ancestor_digest,
        Licence {
            name: "research-only-v1".into(),
            redistribution: Redistribution::ResearchOnly,
            attribution_required: true,
            commercial_use: false,
            access: AccessTier::Public,
        },
    )
    .with_attribution(attribution.clone());

    let mut derived = draft("sub-derived", "lab-a");
    let mut provenance = derived.provenance.take().unwrap();
    provenance.ancestors = vec![ancestor];
    derived.provenance = Some(provenance);

    let dropped = accept(derived.clone(), &submitter("lab-a"))
        .expect_err("permissive declaration and no attribution");
    assert!(matches!(dropped, HubError::MissingAttribution { .. }));

    derived.attributions = vec![attribution];
    let escalated = accept(derived.clone(), &submitter("lab-a"))
        .expect_err("permissive declaration over a research-only ancestor");
    assert!(matches!(escalated, HubError::LicenceEscalation { .. }));

    derived.licence = Some(Licence {
        name: "derived-research-only".into(),
        redistribution: Redistribution::ResearchOnly,
        attribution_required: true,
        commercial_use: false,
        access: AccessTier::Public,
    });
    let submission = accept(derived, &submitter("lab-a")).expect("terms now match the ancestry");
    assert_eq!(
        submission.licence_stack.effective().redistribution,
        Redistribution::ResearchOnly
    );

    let mut moderation = ModerationLedger::new();
    let id = publish(&mut moderation, submission, 1);
    let card = BioAtlasCard::render(moderation.record(&id).unwrap(), "1.0.0");
    assert_eq!(card.state, PublicationState::Available);
    assert_eq!(card.attributions.len(), 1);
    assert_eq!(card.attributions[0].holder, "Neuro Consortium");
    assert_eq!(card.provenance.len(), 1);
}

#[test]
fn withdrawing_a_published_entry_removes_its_rank_and_its_score_but_not_its_record() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();

    let id = publish(
        &mut moderation,
        accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap(),
        1,
    );
    publish(
        &mut moderation,
        accept(draft("sub-2", "lab-b"), &submitter("lab-b")).unwrap(),
        4,
    );

    let entries = [
        entry("sub-1", 0.05, Epoch(2)),
        entry("sub-2", 0.40, Epoch(2)),
    ];
    let before = board(VerificationStatus::SelfReported).rank(&entries, &moderation, &disclosure);
    assert_eq!(before.leaders()[0].entry.submission.as_str(), "sub-1");

    moderation
        .transition(
            &id,
            ModerationState::Withdrawn,
            Decision::by("lab-a", Epoch(10)).because("upstream consent revoked"),
        )
        .unwrap();

    let after = board(VerificationStatus::SelfReported).rank(&entries, &moderation, &disclosure);
    assert_eq!(after.ranked.len(), 1);
    assert_eq!(after.leaders()[0].entry.submission.as_str(), "sub-2");

    let record = moderation.record(&id).expect("record survives withdrawal");
    let card = BioAtlasCard::render(record, "1.0.1");
    assert_eq!(card.state, PublicationState::Withdrawn);
    assert_eq!(card.score.value(), None);
    assert!(card
        .with_score(Score::point(0.05), before.ranked[0].label.clone())
        .is_err());

    let tombstone = moderation.tombstone(&id).expect("tombstone retained");
    assert_eq!(tombstone.reason, "upstream consent revoked");
    assert_eq!(moderation.history(&id).len(), 4);
}

#[test]
fn the_whole_hub_state_survives_a_json_round_trip() {
    let mut moderation = ModerationLedger::new();
    let mut disclosure = DisclosureLedger::new();
    disclosure.declare_held_out(&pack()).unwrap();
    let id = publish(
        &mut moderation,
        accept(draft("sub-1", "lab-a"), &submitter("lab-a")).unwrap(),
        1,
    );
    moderation
        .attest(&id, VerificationStatus::Reproduced, "reviewer-2", Epoch(4))
        .unwrap();

    let rendered = board(VerificationStatus::SelfReported).rank(
        &[entry("sub-1", 0.14, Epoch(2))],
        &moderation,
        &disclosure,
    );
    let card = BioAtlasCard::render(moderation.record(&id).unwrap(), "1.0.0")
        .with_score(
            rendered.ranked[0].entry.score,
            rendered.ranked[0].label.clone(),
        )
        .unwrap();

    for encoded in [
        serde_json::to_string(&moderation).unwrap(),
        serde_json::to_string(&disclosure).unwrap(),
        serde_json::to_string(&rendered).unwrap(),
        serde_json::to_string(&card).unwrap(),
    ] {
        assert!(!encoded.is_empty());
    }

    let decoded: ModerationLedger =
        serde_json::from_str(&serde_json::to_string(&moderation).unwrap()).unwrap();
    assert_eq!(decoded, moderation);
    let decoded: DisclosureLedger =
        serde_json::from_str(&serde_json::to_string(&disclosure).unwrap()).unwrap();
    assert_eq!(decoded, disclosure);
    let decoded: BioAtlasCard =
        serde_json::from_str(&serde_json::to_string(&card).unwrap()).unwrap();
    assert_eq!(decoded, card);
}

/// Asserts the six observable properties of one `bioprism_ids::validated_string_id!` expansion.
///
/// The hub's identifiers are generated by the same macro as `bioprism-ids`' and
/// `bioprism-bioir`', so one edit to that expansion changes the wire form of all three crates at
/// once. Every identifier below appears verbatim in a published submission record, which makes
/// its serialised bytes part of the hub's contract rather than an implementation detail: a
/// submission archived as `"sub-1"` must not come back as `{"value": "sub-1"}` after a refactor
/// that still round-trips.
macro_rules! assert_shared_identifier_contract {
    ($ty:ty, $kind:literal) => {{
        let id = <$ty>::parse("sample-1").expect("well-formed identifier");
        assert_eq!(id.as_str(), "sample-1");
        assert_eq!(id.to_string(), "sample-1");
        assert_eq!(
            serde_json::to_string(&id).expect("identifier serialises"),
            "\"sample-1\""
        );
        let decoded: $ty = serde_json::from_str("\"sample-1\"").expect("identifier deserialises");
        assert_eq!(decoded, id);
        assert_eq!(String::from(id), "sample-1");

        assert_eq!(<$ty>::KIND, $kind);
        assert_eq!(<$ty>::parse(""), Err(IdError::Empty { kind: $kind }));
        assert_eq!(
            <$ty>::parse("a\u{7}b"),
            Err(IdError::ControlCharacter {
                kind: $kind,
                value: "a\u{7}b".to_string(),
            })
        );
    }};
}

#[test]
fn every_hub_identifier_serialises_as_a_bare_json_string_and_names_its_own_kind() {
    assert_shared_identifier_contract!(SubmissionId, "submission");
    assert_shared_identifier_contract!(SubmitterId, "submitter");
    assert_shared_identifier_contract!(BoardId, "board");
}
