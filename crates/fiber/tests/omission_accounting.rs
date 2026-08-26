//! The zero-influence group is what is left after a partition, and the partition is checked.
//!
//! `AGENTS.md` forbids "provably cannot matter" and "nobody checked" from sharing a
//! representation. The omitted population is counted rather than enumerated, so the proven group
//! is arrived at by subtraction, and the subtraction is sound exactly to the extent that every
//! other omission was named first. These tests exercise the ways the naming can come up short:
//! more than one displaced provider of a needed variable, a displaced provider whose winner was
//! withheld for an unrelated reason, and the two shapes of a corpus that gives two facts the same
//! identifier so no identifier-keyed accounting can tell them apart.
//!
//! One test here asserts a defect rather than a guarantee.
//! `a_sibling_output_of_a_selected_factor_is_published_as_proven_though_the_region_carries_it`
//! pins the population the remainder still absorbs silently, so that the crate documentation's
//! statement of the limit and the compiler's behaviour cannot drift apart without a failure.

use bioprism_fiber::{compile, CompileOutput, Query, UnprovenRemainder};
use bioprism_section::{InfluenceClass, OmissionAccountingError};
use bioprism_world::{World, WorldSource};
use serde_json::{json, Value};
use std::path::PathBuf;

fn reference_example(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "reference",
        "fiber_runtime",
        "examples",
        name,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("reference example is readable"))
        .expect("reference example is valid JSON")
}

/// A measurement fact in the shadowed-evidence world's cohort, carrying no protected tag.
///
/// Unprotected on purpose: the protected closure is unioned into the selection regardless of what
/// the slice found, so a protected fact is never omitted and could not exercise any of this.
fn measurement(id: &str, provides: &str, value: Value) -> Value {
    json!({
        "id": id,
        "provenance": [format!("assay/{id}.json")],
        "provides": provides,
        "scope": {"cohort": "SHADOW-001"},
        "tags": ["measurement"],
        "value": value,
    })
}

fn facts_of(world: &mut Value) -> &mut Vec<Value> {
    world["facts"].as_array_mut().expect("facts is an array")
}

fn compiled(world: Value, query: Value) -> CompileOutput {
    let world = World::from_json(world).expect("world loads");
    let query = Query::from_json(query).expect("query loads");
    compile(&world, &query).expect("compiles")
}

fn group_in(
    out: &CompileOutput,
    class: InfluenceClass,
) -> Option<&bioprism_section::OmissionGroup> {
    out.certificate
        .manifest
        .groups
        .iter()
        .find(|group| group.influence == class)
}

/// The manifest must account for the whole omitted population, in every fixture.
///
/// The proven group is the corpus count minus everything else on the manifest, so a manifest whose
/// groups do not sum to `omissions.total_facts` has either lost an omission or counted one twice —
/// and the group that absorbs the difference is the one asserting a proof. Checking the sum is the
/// cheapest available check that the subtraction was over a partition.
#[test]
fn every_reference_world_publishes_a_manifest_that_sums_to_its_own_omitted_count() {
    for (world, query) in [
        (
            "shadowed_evidence_world.json",
            "shadowed_evidence_query.json",
        ),
        (
            "deferred_evidence_world.json",
            "deferred_evidence_query.json",
        ),
        (
            "policy_restricted_world.json",
            "policy_restricted_query.json",
        ),
        ("multi_output_world.json", "multi_output_query.json"),
        ("radiogenomic_world.json", "leakage_query.json"),
    ] {
        let out = compiled(reference_example(world), reference_example(query));
        assert_eq!(
            out.certificate.manifest.total_omitted(),
            out.certificate.omissions.total_facts,
            "{world}: the manifest's groups must partition the omitted corpus"
        );
        assert_eq!(
            out.trace.unproven_remainder, None,
            "{world}: the accounting balances, so no proof should have been refused"
        );
    }
}

/// Two displaced providers of one needed variable are two unproven omissions, not one.
///
/// The world ships two facts providing `risk_score`; a third makes the middle one shadowed as well.
/// Both displaced facts have a backward dependency path to the target through
/// `factor.claim_support`, and neither was bounded. A pass that only noticed the fact immediately
/// behind the winner would leave the other in the proven group with a bound of `0.0`.
#[test]
fn a_second_displaced_provider_of_the_same_variable_is_also_unproven() {
    let mut world = reference_example("shadowed_evidence_world.json");
    facts_of(&mut world).insert(
        3,
        measurement(
            "fact.risk_score_interim",
            "risk_score",
            json!(["low", "high"]),
        ),
    );
    let out = compiled(world, reference_example("shadowed_evidence_query.json"));

    let unproven = group_in(&out, InfluenceClass::Unknown).expect("both displaced facts land here");
    assert_eq!(unproven.count, 2);
    assert_eq!(
        unproven.examples,
        vec![
            "fact.risk_score_interim".to_string(),
            "fact.risk_score_provisional".to_string()
        ]
    );
    assert_eq!(
        group_in(&out, InfluenceClass::Zero)
            .expect("fact.aside is still proved")
            .count,
        1,
        "only the fact providing a variable no factor consumes is proved"
    );
    assert_eq!(out.certificate.manifest.total_omitted(), 3);
    assert!(!out.certificate.manifest.supports_sufficiency_claim());
}

/// A displaced provider survives its winner being withheld, and is counted exactly once.
///
/// `fact.risk_score_final` wins the document-order tiebreak and is then withheld by the data
/// policy, which puts it in the policy group. `fact.risk_score_provisional` was displaced by it and
/// is unproven. Both are omitted for different reasons and the manifest must say so once each: were
/// the policy-withheld winner also counted as a displaced provider, the manifest would report more
/// omissions than the corpus has, and were the displaced fact dropped along with its winner it
/// would land in the proven group.
#[test]
fn a_displaced_provider_whose_winner_was_withheld_by_policy_is_neither_dropped_nor_double_counted()
{
    let mut world = reference_example("shadowed_evidence_world.json");
    facts_of(&mut world).insert(
        0,
        json!({
            "id": "fact.policy",
            "provenance": ["governance/policy.json"],
            "provides": "data_policy",
            "scope": {"cohort": "SHADOW-001"},
            "tags": ["policy", "protected"],
            "value": ["research-only", "no-identifiable-export"],
        }),
    );
    let last = facts_of(&mut world).len() - 1;
    assert_eq!(
        facts_of(&mut world)[last]["id"],
        json!("fact.risk_score_final")
    );
    facts_of(&mut world)[last]["scope"] = json!({
        "cohort": "SHADOW-001",
        "policy": "no-identifiable-export",
    });

    let mut query = reference_example("shadowed_evidence_query.json");
    query["policy"] = json!(["research-only"]);
    let out = compiled(world, query);

    assert_eq!(
        group_in(&out, InfluenceClass::InaccessibleByPolicy)
            .expect("the winner is withheld by policy")
            .examples,
        vec!["fact.risk_score_final".to_string()]
    );
    let unproven = group_in(&out, InfluenceClass::Unknown).expect("the displaced fact survives");
    assert_eq!(
        unproven.examples,
        vec!["fact.risk_score_provisional".to_string()]
    );
    assert_eq!(unproven.count, 1);
    assert_eq!(
        group_in(&out, InfluenceClass::Zero)
            .expect("fact.aside is still proved")
            .count,
        1
    );
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts,
        "three omissions, three reasons, none counted twice"
    );
}

/// The two ways one identifier can stand for two facts that provide a needed variable.
///
/// Both are refusals rather than smaller numbers, and they are refused by different checks:
/// [`Collision::WinnerAmongDisplaced`] by the compiler's ambiguity guard, because the displaced
/// fact is filtered out against the selection before any accounting sees it, and
/// [`Collision::OneIdentifierForTwoVariables`] by
/// [`bioprism_section::ProvenUnreachable::from_classified`], because both copies reach it.
enum Collision {
    /// The winner's own identifier appears among `variable`'s displaced providers.
    WinnerAmongDisplaced { variable: String },
    /// One identifier is reported as a displaced provider of each of two needed variables.
    OneIdentifierForTwoVariables { variables: [String; 2], id: String },
}

/// A source that reports two facts under one identifier.
///
/// [`World::from_json`] refuses a duplicate fact id, so the eager path cannot produce either shape.
/// The compiler is written against [`WorldSource`] and not against `World`, and the other shipped
/// implementation is `bioprism_store::LazyWorld`, which answers `shadowed_provider_ids` by reading
/// an on-disk index it never re-derives from the records. A store written by an older builder,
/// truncated mid-write or edited by hand can therefore report exactly this, and the guarantee has
/// to hold at the boundary the compiler actually consumes rather than at the one constructor that
/// happens to check.
struct CollidingIdentifiers {
    inner: World,
    collision: Collision,
}

impl WorldSource for CollidingIdentifiers {
    fn world_id(&self) -> &str {
        self.inner.world_id()
    }
    fn world_digest(&self) -> bioprism_ids::ContentHash {
        self.inner.world_digest()
    }
    fn total_facts(&self) -> usize {
        self.inner.total_facts() + 1
    }
    fn total_factors(&self) -> usize {
        self.inner.total_factors()
    }
    fn count_with_tag(&self, tag: &str) -> usize {
        self.inner.count_with_tag(tag)
    }
    fn fact_ids_with_any_tag(
        &self,
        tags: &std::collections::BTreeSet<String>,
    ) -> std::collections::BTreeSet<String> {
        self.inner.fact_ids_with_any_tag(tags)
    }
    fn fact(&self, id: &str) -> Option<bioprism_world::Fact> {
        self.inner.fact(id).cloned()
    }
    fn fact_providing(&self, variable: &str) -> Option<bioprism_world::Fact> {
        self.inner.fact_providing(variable).cloned()
    }
    fn shadowed_provider_ids(&self, variable: &str) -> Vec<String> {
        let mut ids = self.inner.shadowed_provider_ids(variable);
        match &self.collision {
            Collision::WinnerAmongDisplaced { variable: subject } if subject == variable => {
                if let Some(winner) = self.inner.fact_providing(variable) {
                    ids.push(winner.id.as_str().to_string());
                }
            }
            Collision::OneIdentifierForTwoVariables { variables, id }
                if variables.iter().any(|subject| subject == variable) =>
            {
                if !ids.iter().any(|existing| existing == id) {
                    ids.push(id.clone());
                }
            }
            _ => {}
        }
        ids
    }
    fn factor(&self, id: &str) -> Option<bioprism_world::Factor> {
        self.inner.factor(id).cloned()
    }
    fn producer_ids(&self, variable: &str) -> Vec<String> {
        self.inner.producer_ids(variable)
    }
    fn events(&self) -> Vec<bioprism_world::CausalEvent> {
        self.inner.events()
    }
}

/// Two facts under one identifier make the proven group unmintable, and the compile says so.
///
/// Everything after the slice is keyed by fact identifier, so a source that gives a displaced fact
/// the winner's own identifier hides it: it is filtered out of the displaced group as though it had
/// been delivered, and it then falls into the remainder and is published as provably unable to
/// matter. It provides a variable the slice needs, so that is the exact claim the manifest may not
/// make.
#[test]
fn a_needed_variable_with_two_providers_under_one_identifier_yields_no_proven_group() {
    let world =
        World::from_json(reference_example("shadowed_evidence_world.json")).expect("world loads");
    let source = CollidingIdentifiers {
        inner: world,
        collision: Collision::WinnerAmongDisplaced {
            variable: "risk_score".to_string(),
        },
    };
    let query =
        Query::from_json(reference_example("shadowed_evidence_query.json")).expect("query loads");
    let out = compile(&source, &query).expect("compiles");

    assert_eq!(
        out.trace.unproven_remainder,
        Some(UnprovenRemainder::AmbiguousIdentifier {
            variables: vec!["risk_score".to_string()]
        }),
        "the compile must report which check declined the proof, not merely omit the group"
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::Zero),
        0,
        "no fact may be published as provably irrelevant over a corpus that cannot name it"
    );
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts,
        "declining the proof must not drop the omissions it would have covered"
    );
    assert!(!out.certificate.manifest.supports_sufficiency_claim());

    let remainder = out
        .certificate
        .manifest
        .groups
        .iter()
        .find(|group| group.reason.contains("two providers under one identifier"))
        .expect("the refused remainder is on the manifest");
    assert_eq!(remainder.influence, InfluenceClass::Unknown);
    assert_eq!(remainder.bound, None);
    assert!(
        remainder.reason.contains("the collision is on risk_score"),
        "the variable that refused the proof belongs in the reason: {}",
        remainder.reason
    );
    assert!(
        remainder.examples.is_empty(),
        "examples names facts, and this group's refusal is that its facts cannot be named"
    );
}

/// One identifier reported for two displaced providers is refused, not silently collapsed.
///
/// This is the second shape of the same defect and it is caught by a different check. The
/// compiler's ambiguity guard compares a displaced identifier against the winner of the *same*
/// variable, so it never sees this one; what sees it is
/// [`bioprism_section::ProvenUnreachable::from_classified`], and only because the displaced
/// population is handed over with its duplicates intact. Collecting it into a set first would
/// remove the second copy before the constructor could judge it, and the fact it stood for would
/// reappear in the remainder as provably unable to matter — the reassuring rendering of an
/// accounting that is provably wrong.
#[test]
fn one_identifier_for_two_displaced_providers_is_refused_rather_than_deduplicated() {
    let world =
        World::from_json(reference_example("shadowed_evidence_world.json")).expect("world loads");
    let source = CollidingIdentifiers {
        inner: world,
        collision: Collision::OneIdentifierForTwoVariables {
            variables: ["cohort_id".to_string(), "risk_score".to_string()],
            id: "fact.risk_score_provisional".to_string(),
        },
    };
    let query =
        Query::from_json(reference_example("shadowed_evidence_query.json")).expect("query loads");
    let out = compile(&source, &query).expect("compiles");

    assert_eq!(
        out.trace.unproven_remainder,
        Some(UnprovenRemainder::Accounting(
            OmissionAccountingError::NamedTwice {
                fact: "fact.risk_score_provisional".to_string()
            }
        )),
        "the duplicate must reach the constructor that refuses it"
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::Zero),
        0,
        "no remainder follows from an accounting that names one fact twice"
    );
    assert!(!out.certificate.manifest.supports_sufficiency_claim());
    assert!(out
        .certificate
        .manifest
        .groups
        .iter()
        .any(|group| group.reason.contains("do not partition the omitted corpus")));
}

/// A sibling output of a selected factor is published as proven, and the region can still perturb it.
///
/// The known limit of the zero group, pinned so it stays known. `factor.joint_readout` emits
/// `risk_score` and `calibration_drift` together; the backward slice enters that factor through
/// `risk_score`, so `calibration_drift` never becomes a needed variable and no lookup in the
/// displaced-provider pass ever asks about it. `QueryRegion::from_world_slice` meanwhile puts
/// `calibration_drift` in that same factor's scope, and scope membership is exactly the relation
/// `bioprism_fiber::influence` perturbs along — the relation whose *absence* it reports as
/// `NotPosable::OutsideCompiledRegion`, documented there as not zero influence. So a fact providing
/// it is omitted, has an image in the compiled region, and is nonetheless published with a bound of
/// `0.0` under a class that says no dependency path reaches the target. The crate documentation
/// states this rather than claiming a per-fact proof, and this test is what keeps the two agreeing.
#[test]
fn a_sibling_output_of_a_selected_factor_is_published_as_proven_though_the_region_carries_it() {
    let mut world = reference_example("multi_output_world.json");
    facts_of(&mut world).push(json!({
        "id": "fact.drift",
        "provenance": ["assay/drift.json"],
        "provides": "calibration_drift",
        "scope": {"cohort": "MULTI-001"},
        "tags": ["measurement"],
        "value": ["stable", "drifting"],
    }));
    let loaded = World::from_json(world.clone()).expect("world loads");
    let out = compiled(world, reference_example("multi_output_query.json"));

    assert!(
        !out.certificate
            .selected_facts
            .contains(&"fact.drift".to_string()),
        "the fact is omitted"
    );
    assert!(
        out.certificate
            .selected_factors
            .contains(&"factor.joint_readout".to_string()),
        "the factor carrying its variable is on the certificate as selected"
    );
    let proven =
        group_in(&out, InfluenceClass::Zero).expect("the remainder is published as proven");
    assert_eq!(
        proven.count, 2,
        "fact.scanner and fact.drift, and only fact.scanner is out of the region"
    );
    assert_eq!(proven.bound, Some(0.0));
    assert_eq!(
        out.trace.unproven_remainder, None,
        "nothing in this compile declines the proof, which is the defect"
    );

    let region = bioprism_fiber::plan::compile_region(
        &loaded,
        "multi-output-sibling",
        ["split_integrity_status"],
    )
    .expect("region builds");
    let carrying: Vec<&str> = region
        .factors()
        .iter()
        .filter(|factor| {
            factor
                .scope()
                .iter()
                .any(|name| name == "calibration_drift")
        })
        .map(|factor| factor.id())
        .collect();
    assert_eq!(
        carrying,
        vec!["factor.joint_readout"],
        "the compiled region has an image of the variable the certificate calls unreachable"
    );
}

/// The same displaced provider under a distinct identifier still yields a proven group, so the
/// check is refusing the collision and not the shadowing.
#[test]
fn a_distinctly_named_displaced_provider_still_yields_a_proven_group() {
    let mut world = reference_example("shadowed_evidence_world.json");
    facts_of(&mut world).insert(
        3,
        measurement(
            "fact.risk_score_second_opinion",
            "risk_score",
            json!(["low", "high"]),
        ),
    );
    let out = compiled(world, reference_example("shadowed_evidence_query.json"));

    assert_eq!(out.trace.unproven_remainder, None);
    assert_eq!(out.certificate.manifest.count_in(InfluenceClass::Zero), 1);
}
