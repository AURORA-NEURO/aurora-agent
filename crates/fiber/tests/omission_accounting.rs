//! The zero-influence group is what is left after a partition, and the partition is checked.
//!
//! `AGENTS.md` forbids "provably cannot matter" and "nobody checked" from sharing a
//! representation. The omitted population is counted rather than enumerated, so the proven group
//! is arrived at by subtraction, and the subtraction is sound exactly to the extent that every
//! other omission was named first. These tests exercise the ways the naming can come up short:
//! more than one displaced provider of a needed variable, a displaced provider whose winner was
//! withheld for an unrelated reason, the two shapes of a corpus that gives two facts the same
//! identifier so no identifier-keyed accounting can tell them apart, and a sibling output of a
//! multi-output factor, which the backward slice never needs and the compiled region carries
//! anyway.
//!
//! The sibling-output tests are the ones that check the compiler against the artefact it ships
//! beside the certificate. `bioprism_fiber::plan::compile_region` is rebuilt here from the same
//! world and asked which factors carry the variable, because the claim under test is not that the
//! group came out a particular size but that the certificate and the region agree about what the
//! query reached.

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

/// The two ways one identifier can stand for two facts that provide a variable the compile reaches.
///
/// Both are refusals rather than smaller numbers, and they are refused by different checks:
/// [`Collision::WinnerAmongDisplaced`] by the compiler's ambiguity guard, which both classifying
/// passes run so that one collision produces one verdict, and
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
                if variables.iter().any(|subject| subject == variable)
                    && !ids.iter().any(|existing| existing == id) =>
            {
                ids.push(id.clone());
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

/// A carried sibling output with two providers under one identifier refuses the same way.
///
/// The collision is the corpus's, not the pass's, so the two passes that can meet it must say the
/// same thing about it. Before the guard reached this pass they did not: the region-carried loop
/// pushed both colliding copies, and `ProvenUnreachable::from_classified` refused as
/// `NamedTwice` — a verdict about the classifier disagreeing with itself, for a corpus that cannot
/// tell two facts apart. It also cost the balance the other verdict keeps: the group counted one
/// fact twice, named it twice in `examples`, and `total_omitted` came out at 2 against a corpus of
/// 3. Two passes, one defect, two answers is the inconsistency this accounting exists to prevent.
#[test]
fn a_carried_variable_with_two_providers_under_one_identifier_yields_no_proven_group() {
    let mut world = reference_example("multi_output_world.json");
    facts_of(&mut world).push(json!({
        "id": "fact.drift",
        "provenance": ["assay/drift.json"],
        "provides": "calibration_drift",
        "scope": {"cohort": "MULTI-001"},
        "tags": ["measurement"],
        "value": ["stable", "drifting"],
    }));
    let source = CollidingIdentifiers {
        inner: World::from_json(world).expect("world loads"),
        collision: Collision::WinnerAmongDisplaced {
            variable: "calibration_drift".to_string(),
        },
    };
    let query =
        Query::from_json(reference_example("multi_output_query.json")).expect("query loads");
    let out = compile(&source, &query).expect("compiles");

    assert_eq!(
        out.trace.unproven_remainder,
        Some(UnprovenRemainder::AmbiguousIdentifier {
            variables: vec!["calibration_drift".to_string()]
        }),
        "a variable only the region carries refuses through the same check as a needed one"
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::Zero),
        0,
        "no fact may be published as provably irrelevant over a corpus that cannot name it"
    );
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts,
        "and the refusal keeps the books balanced, which absorbing the collision did not"
    );

    let carried = out
        .certificate
        .manifest
        .groups
        .iter()
        .find(|group| {
            group
                .reason
                .contains("a selected factor carries in its scope")
        })
        .expect("the carried group is still published");
    assert_eq!(
        carried.count, 1,
        "one fact, counted once: the colliding copy is refused, not counted a second time"
    );
    assert_eq!(carried.examples, vec!["fact.drift".to_string()]);
    assert!(!out.certificate.manifest.supports_sufficiency_claim());
}

/// A sibling output the compiled region carries is unproven, not proven with a bound of `0.0`.
///
/// `factor.joint_readout` emits `risk_score` and `calibration_drift` together; the backward slice
/// enters that factor through `risk_score`, so `calibration_drift` is produced by the compiled
/// program rather than required by it and never becomes a needed variable.
/// `QueryRegion::from_world_slice` nonetheless puts it in that same factor's scope, and scope
/// membership is exactly the relation `bioprism_fiber::influence` perturbs along — the relation
/// whose *absence* it reports as `NotPosable::OutsideCompiledRegion`, documented there as not zero
/// influence. A fact providing that variable is therefore omitted and has an image in the compiled
/// region, so the one thing the certificate may not say about it is that no dependency path reaches
/// the target.
///
/// The region is rebuilt here from the same world and interrogated directly, because a count is not
/// the claim. The claim is that the certificate and the region shipped beside it agree, and the
/// only way to test that is to ask them both.
///
/// `fact.scanner` is the control. It provides `scanner_id`, which only `factor.drift_model`
/// consumes, and that factor produces `calibration_drift` alone — a variable no selected factor
/// needs — so it is not in the region at all and stays proven. Were the pass classing everything
/// omitted as unknown, this assertion would be the one that failed.
#[test]
fn a_sibling_output_of_a_selected_factor_is_unproven_because_the_compiled_region_carries_it() {
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

    let unproven = group_in(&out, InfluenceClass::Unknown)
        .expect("the fact the region carries is classified, not left to the remainder");
    assert_eq!(unproven.count, 1);
    assert_eq!(unproven.examples, vec!["fact.drift".to_string()]);
    assert_eq!(
        unproven.bound, None,
        "no bound was computed for it, and a group that names none may not carry one"
    );
    assert!(
        unproven
            .reason
            .contains("calibration_drift in factor.joint_readout"),
        "the reason must name the variable and the factor that carries it, so the contradiction \
         can be checked against the selected factors on this same certificate: {}",
        unproven.reason
    );
    assert!(!out.certificate.manifest.supports_sufficiency_claim());

    let proven = group_in(&out, InfluenceClass::Zero).expect("fact.scanner is still proved");
    assert_eq!(
        proven.count, 1,
        "fact.scanner alone: no selected factor has scanner_id in scope"
    );
    assert_eq!(proven.bound, Some(0.0));
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts,
        "moving a fact between groups must not drop it or count it twice"
    );
    assert_eq!(
        out.trace.unproven_remainder, None,
        "the accounting balances; naming this population is a classification, not a refusal"
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
        "the region carries the variable, which is why the certificate may not call it unreachable"
    );
    assert!(
        !region
            .factors()
            .iter()
            .any(|factor| factor.scope().iter().any(|name| name == "scanner_id")),
        "and does not carry the control's variable, which is why that one stays proven"
    );
}

/// Both providers of a carried sibling output are unproven, the tiebreak winner included.
///
/// The distinction from the displaced-provider pass, and the half of this defect a narrower fix
/// would miss. For a *needed* variable the winner is selected and only the losers are omitted, so
/// naming the losers is enough. For a variable that only a factor's scope carries, nothing selected
/// any provider of it — selection is keyed on the needed set apart from the protected closure, and
/// neither fact below is protected — so the winner of the document-order tiebreak is omitted
/// exactly like its shadowed sibling and is the fact that was being published with a bound of
/// `0.0`. A pass that asked only `shadowed_provider_ids` would leave it there.
#[test]
fn every_provider_of_a_carried_sibling_output_is_unproven_including_the_tiebreak_winner() {
    let mut world = reference_example("multi_output_world.json");
    for id in ["fact.drift_provisional", "fact.drift_final"] {
        facts_of(&mut world).push(json!({
            "id": id,
            "provenance": [format!("assay/{id}.json")],
            "provides": "calibration_drift",
            "scope": {"cohort": "MULTI-001"},
            "tags": ["measurement"],
            "value": ["stable", "drifting"],
        }));
    }
    let out = compiled(world, reference_example("multi_output_query.json"));

    let unproven = group_in(&out, InfluenceClass::Unknown).expect("both providers land here");
    assert_eq!(unproven.count, 2);
    assert_eq!(
        unproven.examples,
        vec![
            "fact.drift_final".to_string(),
            "fact.drift_provisional".to_string()
        ],
        "the winner of the tiebreak is named alongside the fact it displaced"
    );
    assert_eq!(
        group_in(&out, InfluenceClass::Zero)
            .expect("fact.scanner is still proved")
            .count,
        1
    );
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts
    );
    assert_eq!(out.trace.unproven_remainder, None);
}

/// A sibling output nothing selected is unproven-because-carried even when the cut governs it.
///
/// The precondition is in the name and it is load-bearing: `fact.drift` below carries no protected
/// tag, so nothing puts it in the selection and the cut has nothing to remove. The cut therefore
/// never classifies it, and the region-carried pass does — a label the compiler can check, in place
/// of one it never evaluated. Drop the precondition and the claim is false;
/// [`a_protected_sibling_output_behind_the_temporal_cut_is_deferred_because_the_cut_removed_it`]
/// is that case.
///
/// Both labels void the sufficiency claim, so the certificate is not weaker than the evidence
/// either way. What may never happen is the fact landing in the proven group, and that holds
/// under both preconditions for a reason that needs neither: whatever the cut removes it names.
#[test]
fn an_unprotected_sibling_output_behind_the_temporal_cut_is_unproven_rather_than_deferred() {
    let mut world = reference_example("multi_output_world.json");
    facts_of(&mut world).push(json!({
        "id": "fact.drift",
        "provenance": ["assay/drift.json"],
        "provides": "calibration_drift",
        "scope": {"cohort": "MULTI-001"},
        "tags": ["measurement"],
        "value": ["stable", "drifting"],
    }));
    world["events"] = json!([{
        "id": "event.drift_release",
        "event_time": "2024-06-01T00:00:00Z",
        "availability_time": "2030-01-01T00:00:00Z",
        "causal_parents": [],
        "produces": ["calibration_drift"],
    }]);
    let out = compiled(world, reference_example("multi_output_query.json"));

    assert!(
        out.trace
            .temporal_cut
            .event_managed()
            .contains("calibration_drift"),
        "the cut knows the variable is governed by an event it has not released"
    );
    assert_eq!(
        out.certificate.omissions.inaccessible_selected_before_cut,
        Vec::<String>::new(),
        "the cut never saw the fact, because nothing selected it for the cut to remove"
    );
    assert!(
        group_in(&out, InfluenceClass::DeferredAcquisition).is_none(),
        "so no deferred group exists to hold it"
    );

    let unproven = group_in(&out, InfluenceClass::Unknown).expect("it is classified anyway");
    assert_eq!(unproven.examples, vec!["fact.drift".to_string()]);
    assert_eq!(
        group_in(&out, InfluenceClass::Zero)
            .expect("fact.scanner is still proved")
            .count,
        1,
        "the fact behind the cut is not in the proven group, which is the property that matters"
    );
    assert!(!out.certificate.manifest.supports_sufficiency_claim());
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts
    );
}

/// A protected sibling output *is* in the selection, and the cut removes it and defers it.
///
/// The counterexample to the wider claim, in three facts. The selection is not keyed on the needed
/// set alone: it is the needed set's providers unioned with the protected closure, so a fact
/// carrying one of the query's protected tags is selected whatever variable it provides, and a
/// sibling output's provider is reachable by the temporal cut after all. The same world with
/// `protected` dropped from the tags is
/// [`an_unprotected_sibling_output_behind_the_temporal_cut_is_unproven_rather_than_deferred`], and
/// the two land in different classes.
///
/// What survives the counterexample is the property the accounting needs: the cut names what it
/// removes, so `fact.drift` is in the deferred group rather than in a proven one. That half of the
/// argument holds however the fact entered the selection, which is why it is the half the
/// compiler's guarantee rests on.
#[test]
fn a_protected_sibling_output_behind_the_temporal_cut_is_deferred_because_the_cut_removed_it() {
    let mut world = reference_example("multi_output_world.json");
    facts_of(&mut world).retain(|fact| fact["id"] != json!("fact.scanner"));
    world["factors"]
        .as_array_mut()
        .expect("factors is an array")
        .retain(|factor| factor["id"] != json!("factor.drift_model"));
    facts_of(&mut world).push(json!({
        "id": "fact.drift",
        "provenance": ["assay/drift.json"],
        "provides": "calibration_drift",
        "scope": {"cohort": "MULTI-001"},
        "tags": ["measurement", "protected"],
        "value": ["stable", "drifting"],
    }));
    world["events"] = json!([{
        "id": "event.drift_release",
        "event_time": "2024-06-01T00:00:00Z",
        "availability_time": "2030-01-01T00:00:00Z",
        "causal_parents": [],
        "produces": ["calibration_drift"],
    }]);
    let out = compiled(world, reference_example("multi_output_query.json"));

    assert!(
        out.certificate
            .protected_closure
            .contains(&"fact.drift".to_string()),
        "the protected closure put a sibling output's provider in the selection, which is the step \
         the wider claim says cannot happen"
    );
    assert_eq!(
        out.certificate.omissions.inaccessible_selected_before_cut,
        vec!["fact.drift".to_string()],
        "and the cut then removed it, so the cut does reach this population"
    );

    let deferred =
        group_in(&out, InfluenceClass::DeferredAcquisition).expect("the cut names what it removes");
    assert_eq!(deferred.examples, vec!["fact.drift".to_string()]);
    assert!(
        group_in(&out, InfluenceClass::Unknown).is_none(),
        "the region-carried pass does not also claim it: one omission, one class"
    );
    assert_eq!(
        out.certificate.manifest.count_in(InfluenceClass::Zero),
        0,
        "and it is not in the proven group, which is the guarantee the counterexample leaves intact"
    );
    assert_eq!(
        out.certificate.manifest.total_omitted(),
        out.certificate.omissions.total_facts
    );
    assert_eq!(out.trace.unproven_remainder, None);
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
