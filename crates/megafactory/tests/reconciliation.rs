//! What actually ran, and what this crate's parameterization axes do to the section's arithmetic.
//!
//! Blueprint 35.13's last scale constraint — "generated instances are enumerable; results name the
//! subset actually executed" — plus the two places `bioprism-scale` found the section's own worked
//! example failing to reconcile. The tests below assert this crate's bearing on both rather than
//! restating it in prose.

use bioprism_factory::Idempotency;
use bioprism_megafactory::{ExecutedReport, ExecutionLedger, FenceRegistry, PlacementError};
use bioprism_scale::corpus::{Corpus, GeneratedItem};
use bioprism_scale::{EffectiveSize, EffectiveSizeReport, SimilarityRelation};

/// One parent decision, one mutation family, and `parameterizations` instances of it.
///
/// This is exactly the axis the section's stated descendant count needs and never names: a
/// semi-synthetic panel run at several stated effect sizes, or one intervention re-run under
/// several alternative models, is several instances of one family over one decision.
fn one_family_at(parameterizations: usize) -> Corpus {
    let mut corpus = Corpus::new();
    corpus
        .insert(GeneratedItem::parent("p1", "d1", "digest-parent"))
        .expect("fresh id");
    for index in 0..parameterizations {
        corpus
            .insert(GeneratedItem::descendant(
                format!("p1-mask-{index}"),
                "p1",
                "mask",
                "sig-mask",
                format!("digest-mask-{index}"),
                "d1",
            ))
            .expect("fresh id");
    }
    corpus
}

fn effective_under(report: &EffectiveSizeReport, relation: SimilarityRelation) -> &EffectiveSize {
    report.under(relation).expect("every relation is measured")
}

#[test]
fn parameterizations_multiply_instances_and_do_not_multiply_classes() {
    let three = EffectiveSizeReport::measure(&one_family_at(3)).expect("measurable");
    let ten = EffectiveSizeReport::measure(&one_family_at(10)).expect("measurable");

    let three_classes = effective_under(&three, SimilarityRelation::EquivalenceClass);
    let ten_classes = effective_under(&ten, SimilarityRelation::EquivalenceClass);

    assert_eq!(three_classes.nominal, 4);
    assert_eq!(ten_classes.nominal, 11);
    assert_eq!(
        three_classes.effective, ten_classes.effective,
        "raising the parameterization count from 3 to 10 adds seven instances and not one class; \
         this is the axis that would reconcile the section's stated descendant count, and using it \
         leaves the class ceiling exactly where it was"
    );
    assert_eq!(three_classes.effective, 2);
    assert!(ten_classes.inflation_ratio > three_classes.inflation_ratio);
}

#[test]
fn the_content_digest_relation_does_see_the_parameterizations_and_the_class_relation_does_not() {
    let report = EffectiveSizeReport::measure(&one_family_at(5)).expect("measurable");
    assert_eq!(
        effective_under(&report, SimilarityRelation::ContentDigest).effective,
        6,
        "distinct effect sizes are distinct content"
    );
    assert_eq!(
        effective_under(&report, SimilarityRelation::EquivalenceClass).effective,
        2,
        "and they still probe one failure mode over one decision"
    );
    assert_eq!(
        effective_under(&report, SimilarityRelation::ParentWorld).effective,
        1
    );
}

#[test]
fn the_most_conservative_relation_is_the_one_a_claim_should_rest_on() {
    let report = EffectiveSizeReport::measure(&one_family_at(10)).expect("measurable");
    let conservative = report.most_conservative().expect("non-empty");
    assert_eq!(conservative.relation, SimilarityRelation::ParentWorld);
    assert!(conservative
        .headline()
        .contains("Instance count is not benchmark count"));
}

fn ledger_over(items: &[&str]) -> ExecutionLedger {
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    for (index, item) in items.iter().enumerate() {
        let job = format!("job-{index}");
        let fence = registry.issue(&job);
        ledger
            .commit(&registry, &job, item, fence, Idempotency::Idempotent)
            .expect("committed under a current fence");
    }
    ledger
}

#[test]
fn a_run_report_names_the_subset_that_executed_and_not_the_subset_that_could_have() {
    let corpus = one_family_at(10);
    let ledger = ledger_over(&["p1-mask-0", "p1-mask-1"]);
    let report = ExecutedReport::measure(&corpus, &ledger).expect("measurable");

    let executed = effective_under(&report.executed, SimilarityRelation::ContentDigest);
    let enumerated = effective_under(&report.enumerated, SimilarityRelation::ContentDigest);
    assert_eq!(
        executed.nominal, 3,
        "two descendants plus the parent they need"
    );
    assert_eq!(enumerated.nominal, 11);
    assert_eq!(report.items_never_executed, 8);
}

#[test]
fn executing_a_descendant_keeps_its_ancestors_so_lineage_still_resolves() {
    let corpus = one_family_at(4);
    let ledger = ledger_over(&["p1-mask-2"]);
    let report = ExecutedReport::measure(&corpus, &ledger).expect("measurable");

    assert_eq!(
        effective_under(&report.executed, SimilarityRelation::ParentWorld).effective,
        1,
        "dropping the unexecuted parent would promote the descendant to a parent world and inflate \
         the count of independent worlds"
    );
    assert_eq!(
        effective_under(&report.executed, SimilarityRelation::ContentDigest).nominal,
        2
    );
}

#[test]
fn an_executed_item_that_was_never_enumerated_is_refused() {
    let corpus = one_family_at(2);
    let ledger = ledger_over(&["p1-mask-99"]);
    assert_eq!(
        ExecutedReport::measure(&corpus, &ledger),
        Err(PlacementError::ExecutedItemNotEnumerated(
            "p1-mask-99".into()
        ))
    );
}

#[test]
fn a_run_over_nothing_reports_zero_executed_and_the_whole_corpus_unexecuted() {
    let corpus = one_family_at(3);
    let report = ExecutedReport::measure(&corpus, &ExecutionLedger::new()).expect("measurable");
    assert_eq!(report.items_never_executed, 4);
    assert_eq!(
        effective_under(&report.executed, SimilarityRelation::ContentDigest).nominal,
        0
    );
    assert!(report.headline().contains("never ran and are not evidence"));
}

#[test]
fn the_executed_report_carries_no_instance_count_outside_an_effective_size() {
    let corpus = one_family_at(6);
    let ledger = ledger_over(&["p1-mask-0"]);
    let report = ExecutedReport::measure(&corpus, &ledger).expect("measurable");

    let value = serde_json::to_value(&report).expect("serialisable");
    let object = value.as_object().expect("a json object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["enumerated", "executed", "items_never_executed"],
        "any new top-level field is a new way for a bare instance count to reach a report"
    );

    for relation_report in ["executed", "enumerated"] {
        let sizes = object[relation_report]["by_relation"]
            .as_array()
            .expect("an array of effective sizes");
        for size in sizes {
            assert!(size.get("nominal").is_some());
            assert!(
                size.get("effective").is_some() && size.get("inflation_ratio").is_some(),
                "a nominal count only ever appears beside its effective count and inflation ratio"
            );
            assert!(size.get("relation").is_some());
        }
    }
}

#[test]
fn the_headline_leads_with_the_executed_subset_and_the_sentence_that_qualifies_it() {
    let corpus = one_family_at(6);
    let ledger = ledger_over(&["p1-mask-0", "p1-mask-3"]);
    let report = ExecutedReport::measure(&corpus, &ledger).expect("measurable");
    let headline = report.headline();
    assert!(headline.starts_with("executed subset:"));
    assert!(headline.contains("Instance count is not benchmark count"));
    assert!(headline.contains("never ran and are not evidence"));
}

#[test]
fn a_second_commit_of_the_same_item_does_not_grow_the_executed_subset() {
    let corpus = one_family_at(3);
    let mut registry = FenceRegistry::new();
    let mut ledger = ExecutionLedger::new();
    let fence = registry.issue("job-0");
    for _ in 0..5 {
        ledger
            .commit(
                &registry,
                "job-0",
                "p1-mask-0",
                fence,
                Idempotency::Idempotent,
            )
            .expect("committed");
    }
    let report = ExecutedReport::measure(&corpus, &ledger).expect("measurable");
    assert_eq!(
        effective_under(&report.executed, SimilarityRelation::ContentDigest).nominal,
        2,
        "re-running one item five times executes one item"
    );
    assert_eq!(ledger.duplicates().wasted_idempotent_commits, 4);
}
