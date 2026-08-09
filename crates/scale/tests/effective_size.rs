//! Blueprint 35.10, and the claim `AGENTS.md` calls non-negotiable: instance count is not
//! benchmark count.

use bioprism_scale::corpus::{content_digest, Corpus, GeneratedItem};
use bioprism_scale::effective::{
    cluster_stability, hierarchical_effective_size, ClusterStability, EffectiveSize,
    EffectiveSizeReport, RelationQuality, SimilarityRelation,
};
use bioprism_scale::error::ScaleError;
use serde_json::json;
use std::collections::BTreeMap;

fn world(subject: &str, value: i64) -> serde_json::Value {
    json!({
        "world_id": format!("world-{subject}"),
        "description": format!("a world about {subject}"),
        "facts": [{ "id": "f1", "provides": "dose", "value": value }],
        "factors": [],
        "events": [],
    })
}

/// One parent, many descendants that all test the same thing.
fn paraphrase_corpus(count: usize) -> Corpus {
    let mut corpus = Corpus::new();
    corpus
        .insert(GeneratedItem::parent("p1", "d1", "digest-parent"))
        .unwrap();
    for index in 0..count {
        corpus
            .insert(GeneratedItem::descendant(
                format!("p1#m{index}"),
                "p1",
                "rename-subjects",
                "Sufficient|closure",
                format!("digest-{index}"),
                "d1",
            ))
            .unwrap();
    }
    corpus
}

#[test]
fn renaming_a_world_does_not_create_a_new_item() {
    let original = world("alpha", 10);
    let mut renamed = original.clone();
    renamed["world_id"] = json!("world-completely-different");
    renamed["description"] = json!("a world about something else entirely");

    assert_eq!(
        content_digest(&original).unwrap(),
        content_digest(&renamed).unwrap(),
        "a generator that can defeat deduplication by renaming can inflate instance count for free"
    );
}

#[test]
fn changing_a_single_fact_value_does_create_a_new_item() {
    assert_ne!(
        content_digest(&world("alpha", 10)).unwrap(),
        content_digest(&world("alpha", 11)).unwrap()
    );
}

#[test]
fn a_thousand_paraphrases_of_one_parent_are_one_equivalence_class() {
    let corpus = paraphrase_corpus(1_000);
    let size = EffectiveSize::measure(&corpus, SimilarityRelation::EquivalenceClass).unwrap();

    assert_eq!(size.nominal, 1_001);
    assert_eq!(
        size.effective, 2,
        "the parent and one class of descendants; a thousand paraphrases add nothing"
    );
    assert!(size.inflation_ratio.expect("two classes is a denominator") > 500.0);
    assert!(!EffectiveSize::measure(&corpus, SimilarityRelation::ParentWorld)
        .unwrap()
        .is_publishable_as_a_benchmark());
}

#[test]
fn a_nominal_count_is_never_serialized_without_the_effective_count() {
    let corpus = paraphrase_corpus(10);
    let size = EffectiveSize::measure(&corpus, SimilarityRelation::EquivalenceClass).unwrap();
    let encoded = serde_json::to_value(&size).unwrap();

    assert!(encoded.get("nominal").is_some());
    assert!(
        encoded.get("effective").is_some() && encoded["inflation_ratio"].is_number(),
        "NominalCount has no Serialize impl, so the only route to a published count is this struct"
    );
    assert!(encoded.get("relation").is_some(), "an effective size without its relation is unreadable");
}

/// An inflation ratio is at least 1 by construction, so the `0.0` an empty corpus used to emit was
/// not a low inflation but a value the quantity cannot take. The key stays, saying `null`, because
/// a vanished key and a ratio of zero are both things a reader would have to guess at.
#[test]
fn an_empty_corpus_has_no_inflation_ratio_rather_than_a_ratio_of_zero() {
    let size = EffectiveSize::measure(&Corpus::new(), SimilarityRelation::EquivalenceClass).unwrap();

    assert_eq!(size.nominal, 0);
    assert_eq!(size.effective, 0);
    assert_eq!(size.inflation_ratio, None);

    let encoded = serde_json::to_value(&size).unwrap();
    assert!(
        encoded["inflation_ratio"].is_null(),
        "an absent ratio must be visibly absent, not a number outside its own range"
    );
    assert!(size.headline().contains("inflation undefined"));
}

#[test]
fn every_relation_states_what_it_refuses_to_merge() {
    for relation in SimilarityRelation::ALL {
        assert!(!relation.treats_as_same().is_empty());
        assert!(
            !relation.does_not_merge().is_empty(),
            "{} must say what it leaves separate, because that is where inflation hides",
            relation.as_str()
        );
    }
}

#[test]
fn a_report_leads_with_its_most_conservative_relation() {
    let corpus = paraphrase_corpus(50);
    let report = EffectiveSizeReport::measure(&corpus).unwrap();
    let conservative = report.most_conservative().unwrap();

    assert_eq!(conservative.relation, SimilarityRelation::ParentWorld);
    assert_eq!(conservative.effective, 1, "everything descends from one parent world");
    assert!(report.headline().contains("Instance count is not benchmark count"));
    for size in &report.by_relation {
        assert!(conservative.effective <= size.effective);
    }
}

#[test]
fn parent_concentration_names_a_benchmark_that_is_one_world_in_a_costume() {
    let corpus = paraphrase_corpus(99);
    let size = EffectiveSize::measure(&corpus, SimilarityRelation::EquivalenceClass).unwrap();
    assert_eq!(size.parent_concentration, 1.0);
}

#[test]
fn independent_parents_have_an_inflation_ratio_of_one() {
    let mut corpus = Corpus::new();
    for index in 0..25 {
        corpus
            .insert(GeneratedItem::parent(
                format!("p{index}"),
                format!("d{index}"),
                format!("digest-{index}"),
            ))
            .unwrap();
    }
    let size = EffectiveSize::measure(&corpus, SimilarityRelation::ParentWorld).unwrap();
    assert_eq!(size.effective, 25);
    assert_eq!(size.inflation_ratio, Some(1.0));
    assert_eq!(size.duplicates_collapsed, 0);
}

#[test]
fn the_content_digest_relation_collapses_byte_identical_descendants() {
    let mut corpus = Corpus::new();
    corpus.insert(GeneratedItem::parent("p1", "d1", "digest-parent")).unwrap();
    for index in 0..8 {
        corpus
            .insert(GeneratedItem::descendant(
                format!("p1#m{index}"),
                "p1",
                format!("family-{index}"),
                format!("signature-{index}"),
                "identical-content",
                "d1",
            ))
            .unwrap();
    }

    let by_content = EffectiveSize::measure(&corpus, SimilarityRelation::ContentDigest).unwrap();
    let by_class = EffectiveSize::measure(&corpus, SimilarityRelation::EquivalenceClass).unwrap();

    assert_eq!(by_content.effective, 2, "eight identical worlds are one item, plus the parent");
    assert_eq!(
        by_class.effective, 9,
        "the class relation trusts the declared family and signature, so identical content \
         labelled eight ways looks like eight classes — which is why both are reported"
    );
}

#[test]
fn relation_quality_is_measured_against_labelled_truth() {
    let mut corpus = Corpus::new();
    corpus.insert(GeneratedItem::parent("p1", "d1", "digest-parent")).unwrap();
    let mut truth = BTreeMap::new();
    truth.insert("p1".to_string(), "class-parent".to_string());
    for index in 0..6 {
        let id = format!("p1#m{index}");
        corpus
            .insert(GeneratedItem::descendant(
                &id,
                "p1",
                "rename",
                "Sufficient",
                if index < 3 { "same-a" } else { "same-b" },
                "d1",
            ))
            .unwrap();
        truth.insert(id, if index < 3 { "class-a".into() } else { "class-b".into() });
    }

    let content =
        RelationQuality::evaluate(&corpus, SimilarityRelation::ContentDigest, &truth).unwrap();
    assert!(content.is_measured(), "the labelling holds duplicates and the relation merged pairs");
    assert_eq!(content.duplicate_recall, Some(1.0));
    assert_eq!(content.false_merge_rate, Some(0.0));

    let parent =
        RelationQuality::evaluate(&corpus, SimilarityRelation::ParentWorld, &truth).unwrap();
    assert_eq!(parent.duplicate_recall, Some(1.0));
    assert!(
        parent.false_merge_rate.expect("merging everything merges pairs") > 0.0,
        "merging everything recovers every duplicate and invents many more"
    );
}

/// A perfect recall and a zero false-merge rate over an empty labelling are the two most
/// flattering numbers this struct can hold, and neither was a measurement.
#[test]
fn a_relation_nobody_could_test_does_not_score_perfectly() {
    let mut corpus = Corpus::new();
    for index in 0..4 {
        corpus
            .insert(GeneratedItem::parent(
                format!("p{index}"),
                format!("d{index}"),
                format!("digest-{index}"),
            ))
            .unwrap();
    }

    // Every item is its own true class, so there is not one true duplicate pair to recall, and the
    // content-digest relation merges nothing, so there is not one merged pair to be wrong about.
    let truth: BTreeMap<String, String> = (0..4)
        .map(|index| (format!("p{index}"), format!("class-{index}")))
        .collect();

    let quality =
        RelationQuality::evaluate(&corpus, SimilarityRelation::ContentDigest, &truth).unwrap();

    assert_eq!(quality.true_duplicate_pairs, 0);
    assert_eq!(quality.merged_pairs, 0);
    assert_eq!(
        quality.duplicate_recall, None,
        "1.0 said the relation recovered every duplicate it was shown; it was shown none"
    );
    assert_eq!(
        quality.false_merge_rate, None,
        "0.0 said the relation invented no false merges; it made no merges at all"
    );
    assert!(!quality.is_measured());
}

#[test]
fn cluster_stability_is_one_when_two_relations_partition_identically() {
    let mut corpus = Corpus::new();
    for index in 0..6 {
        corpus
            .insert(GeneratedItem::parent(
                format!("p{index}"),
                format!("d{index}"),
                format!("digest-{index}"),
            ))
            .unwrap();
    }
    let stability = cluster_stability(
        &corpus,
        SimilarityRelation::ContentDigest,
        SimilarityRelation::ParentWorld,
    )
    .unwrap();
    assert_eq!(stability.rand_index(), Some(1.0));
    assert_eq!(stability, ClusterStability::Measured { agreements: 15, pairs: 15 });
}

/// The Rand index over zero pairs was 1.0 — two relations reported as agreeing perfectly about a
/// comparison neither of them was ever asked to make.
#[test]
fn nothing_evaluable_is_not_perfect_stability() {
    let mut corpus = Corpus::new();
    corpus
        .insert(GeneratedItem::parent("p0", "d0", "digest-0"))
        .unwrap();

    let stability = cluster_stability(
        &corpus,
        SimilarityRelation::ContentDigest,
        SimilarityRelation::ParentWorld,
    )
    .unwrap();

    assert_eq!(stability, ClusterStability::NoPairs { items: 1 });
    assert_eq!(stability.rand_index(), None);
    assert!(!stability.is_measured());

    let report = EffectiveSizeReport::measure(&corpus).unwrap();
    assert!(
        !report.cluster_stability.is_measured(),
        "a one-item corpus cannot demonstrate that two relations mean the same thing"
    );

    let encoded = serde_json::to_value(stability).unwrap();
    assert_eq!(encoded["cluster_stability"], "no_pairs");
    assert!(
        encoded.get("agreements").is_none() && encoded.get("pairs").is_none(),
        "a stability with no pairs must carry no counts for a renderer to divide"
    );
}

/// Too few pairs to compare and too many to enumerate are different findings, and the older
/// `Option<f64>` answered both with `None`.
#[test]
fn a_corpus_too_small_to_have_pairs_is_not_a_corpus_too_large_to_enumerate() {
    let mut empty = Corpus::new();
    empty
        .insert(GeneratedItem::parent("p0", "d0", "digest-0"))
        .unwrap();
    let small = EffectiveSizeReport::measure(&empty).unwrap();

    assert!(matches!(
        small.cluster_stability,
        ClusterStability::NoPairs { items: 1 }
    ));
    assert_ne!(
        small.cluster_stability,
        ClusterStability::NotEnumerated {
            items: 1,
            limit: bioprism_scale::STABILITY_PAIR_LIMIT
        }
    );
}

#[test]
fn cluster_stability_falls_when_relations_disagree() {
    let corpus = paraphrase_corpus(20);
    let report = EffectiveSizeReport::measure(&corpus).unwrap();
    let stability = report
        .cluster_stability
        .rand_index()
        .expect("small corpus is measurable");
    assert!(
        stability < 1.0,
        "content digest separates the paraphrases that the class relation merges"
    );
}

#[test]
fn hierarchical_effective_size_falls_as_intra_parent_correlation_rises() {
    let near_independent = hierarchical_effective_size(100_000, 400, 0.01).unwrap();
    let moderate = hierarchical_effective_size(100_000, 400, 0.05).unwrap();
    let clustered = hierarchical_effective_size(100_000, 400, 0.20).unwrap();

    assert!(near_independent > moderate && moderate > clustered);
    assert!((moderate - 7_434.9).abs() < 1.0, "got {moderate}");
    assert!(
        clustered < 2_000.0,
        "100,000 instances over 400 parents at rho=0.2 are worth under two thousand observations"
    );
}

#[test]
fn hierarchical_effective_size_rejects_a_correlation_outside_the_unit_interval() {
    assert!(matches!(
        hierarchical_effective_size(100, 10, 1.0),
        Err(ScaleError::CorrelationOutOfRange(_))
    ));
    assert!(matches!(
        hierarchical_effective_size(100, 10, -0.1),
        Err(ScaleError::CorrelationOutOfRange(_))
    ));
}

#[test]
fn a_duplicate_item_id_is_refused() {
    let mut corpus = Corpus::new();
    corpus.insert(GeneratedItem::parent("p1", "d1", "digest")).unwrap();
    assert!(matches!(
        corpus.insert(GeneratedItem::parent("p1", "d1", "digest")),
        Err(ScaleError::DuplicateItem(id)) if id == "p1"
    ));
}
