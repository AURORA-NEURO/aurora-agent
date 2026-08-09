//! Blueprint 35.11. The leakage control that matters once a factory generates faster than anyone
//! can read: a lineage family must never straddle a train/test boundary.

use bioprism_scale::corpus::{Corpus, GeneratedItem};
use bioprism_scale::error::{ScaleError, SplitError};
use bioprism_scale::split::{verify_item_assignment, Contamination, FamilySplit, Tier};
use std::collections::{BTreeMap, BTreeSet};

/// A parent, a child, and a grandchild that shares nothing recognisable with its cousin.
fn three_generation_corpus() -> Corpus {
    let mut corpus = Corpus::new();
    corpus.insert(GeneratedItem::parent("p1", "d1", "digest-p1")).unwrap();
    corpus
        .insert(GeneratedItem::descendant(
            "child",
            "p1",
            "rename-subjects",
            "Sufficient",
            "digest-child",
            "d1",
        ))
        .unwrap();
    corpus
        .insert(GeneratedItem::descendant(
            "grandchild",
            "child",
            "add-distractors",
            "Insufficient",
            "digest-grandchild",
            "d1",
        ))
        .unwrap();
    corpus
        .insert(GeneratedItem::parent("p2", "d2", "digest-p2"))
        .unwrap();
    corpus
}

#[test]
fn two_items_from_one_parent_world_cannot_straddle_a_split() {
    let corpus = three_generation_corpus();
    let mut assignment = BTreeMap::new();
    assignment.insert("p1".to_string(), Tier::Public);
    assignment.insert("child".to_string(), Tier::Public);
    assignment.insert("grandchild".to_string(), Tier::Hidden);
    assignment.insert("p2".to_string(), Tier::Hidden);

    match verify_item_assignment(&corpus, &assignment) {
        Err(SplitError::FamilyStraddlesSplit {
            family,
            left_item,
            left,
            right_item,
            right,
        }) => {
            assert_eq!(family, "p1");
            assert_eq!(left, "public");
            assert_eq!(right, "hidden");
            assert_ne!(left_item, right_item);
        }
        other => panic!("a straddling split must be refused by name, got {other:?}"),
    }
}

#[test]
fn a_grandchild_belongs_to_its_grandparents_family_not_its_parents() {
    let corpus = three_generation_corpus();
    assert_eq!(corpus.family_of("grandchild").unwrap(), "p1");
    assert_eq!(corpus.root_of("grandchild").unwrap().id, "p1");

    let families = corpus.families().unwrap();
    assert_eq!(families.get("p1").map(Vec::len), Some(3));
    assert_eq!(families.get("p2").map(Vec::len), Some(1));
}

#[test]
fn a_family_split_cannot_represent_a_straddle() {
    let corpus = three_generation_corpus();
    let mut split = FamilySplit::new();
    split.assign_family("p1", Tier::Public).unwrap();
    split.assign_family("p2", Tier::Hidden).unwrap();

    let resolved = split.resolve(&corpus).unwrap();
    assert_eq!(resolved["p1"], Tier::Public);
    assert_eq!(resolved["child"], Tier::Public);
    assert_eq!(
        resolved["grandchild"],
        Tier::Public,
        "there is no method that could have put it anywhere else"
    );
    assert_eq!(resolved["p2"], Tier::Hidden);
}

#[test]
fn surface_dissimilarity_does_not_save_a_straddling_split() {
    let mut corpus = Corpus::new();
    corpus.insert(GeneratedItem::parent("p1", "d1", "digest-p1")).unwrap();
    corpus
        .insert(GeneratedItem::descendant(
            "cousin-a",
            "p1",
            "remove-leakage",
            "Insufficient",
            "utterly-different-digest-a",
            "d1",
        ))
        .unwrap();
    corpus
        .insert(GeneratedItem::descendant(
            "cousin-b",
            "p1",
            "camouflage-tags",
            "Sufficient",
            "utterly-different-digest-b",
            "d9",
        ))
        .unwrap();

    let mut assignment = BTreeMap::new();
    assignment.insert("p1".to_string(), Tier::Public);
    assignment.insert("cousin-a".to_string(), Tier::Public);
    assignment.insert("cousin-b".to_string(), Tier::Hidden);

    assert!(
        matches!(
            verify_item_assignment(&corpus, &assignment),
            Err(SplitError::FamilyStraddlesSplit { .. })
        ),
        "different digests, different families, different decisions — and still one parent"
    );
}

#[test]
fn a_family_cannot_be_reassigned_to_another_tier() {
    let mut split = FamilySplit::new();
    split.assign_family("p1", Tier::Hidden).unwrap();
    split.assign_family("p1", Tier::Hidden).unwrap();

    match split.assign_family("p1", Tier::Public) {
        Err(SplitError::FamilyAlreadyAssigned {
            family,
            existing,
            requested,
        }) => {
            assert_eq!(family, "p1");
            assert_eq!(existing, "hidden");
            assert_eq!(requested, "public");
        }
        other => panic!("silently moving a family between tiers is how a hidden set leaks: {other:?}"),
    }
}

#[test]
fn an_unassigned_item_blocks_the_release_rather_than_defaulting_to_public() {
    let corpus = three_generation_corpus();
    let mut split = FamilySplit::new();
    split.assign_family("p1", Tier::Public).unwrap();

    match split.resolve(&corpus) {
        Err(SplitError::UnassignedItem { item, family }) => {
            assert_eq!(family, "p2");
            assert_eq!(item, "p2");
        }
        other => panic!("an unassigned item must not be silently released: {other:?}"),
    }
}

#[test]
fn a_split_report_counts_families_not_only_items() {
    let corpus = three_generation_corpus();
    let mut split = FamilySplit::new();
    split.assign_family("p1", Tier::Public).unwrap();
    split.assign_family("p2", Tier::Hidden).unwrap();

    let report = split.report(&corpus).unwrap();
    assert_eq!(report.items_by_tier["public"], 3);
    assert_eq!(report.items_by_tier["hidden"], 1);
    assert_eq!(report.families_by_tier["public"], 1);
    assert_eq!(report.families_by_tier["hidden"], 1);
    assert_eq!(report.intact_families, report.total_families);
    assert_eq!(report.total_families, 2);
}

#[test]
fn a_lineage_cycle_is_a_typed_error_not_an_infinite_loop() {
    let mut corpus = Corpus::new();
    corpus
        .insert(GeneratedItem::descendant("a", "b", "f", "s", "da", "d1"))
        .unwrap();
    corpus
        .insert(GeneratedItem::descendant("b", "a", "f", "s", "db", "d1"))
        .unwrap();

    assert!(matches!(corpus.root_of("a"), Err(ScaleError::LineageCycle(_))));
}

#[test]
fn a_dangling_parent_names_both_the_item_and_the_missing_parent() {
    let mut corpus = Corpus::new();
    corpus
        .insert(GeneratedItem::descendant("orphan", "ghost", "f", "s", "d", "d1"))
        .unwrap();

    match corpus.root_of("orphan") {
        Err(ScaleError::DanglingParent { item, parent }) => {
            assert_eq!(item, "orphan");
            assert_eq!(parent, "ghost");
        }
        other => panic!("a broken lineage must name both sides: {other:?}"),
    }
}

#[test]
fn training_exposure_is_found_by_digest_and_never_needs_the_text() {
    let corpus = three_generation_corpus();
    let mut contamination = Contamination::new();
    contamination.declare_corpus("web-crawl-2025", ["digest-grandchild".to_string()]);

    let findings = contamination.scan(&corpus);
    assert_eq!(findings.len(), 1);
    match &findings[0] {
        SplitError::TrainingExposure {
            item,
            digest,
            corpus: name,
        } => {
            assert_eq!(item, "grandchild");
            assert_eq!(digest, "digest-grandchild");
            assert_eq!(name, "web-crawl-2025");
        }
        other => panic!("expected a training-exposure finding, got {other:?}"),
    }
}

#[test]
fn an_uncontaminated_corpus_produces_no_findings() {
    let corpus = three_generation_corpus();
    let mut contamination = Contamination::new();
    contamination.declare_corpus("web-crawl-2025", ["some-other-digest".to_string()]);
    assert!(contamination.scan(&corpus).is_empty());
}

#[test]
fn a_reproduced_canary_names_the_canary() {
    let mut contamination = Contamination::new();
    contamination.plant_canary("canary-7", "canary-digest");
    contamination.plant_canary("canary-8", "unseen-digest");

    let observed: BTreeSet<String> = ["canary-digest".to_string()].into_iter().collect();
    let detected = contamination.detect_canaries(&observed);
    assert_eq!(detected.len(), 1);
    assert!(matches!(
        &detected[0],
        SplitError::CanaryDetected { canary } if canary == "canary-7"
    ));
}
