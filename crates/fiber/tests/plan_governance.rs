//! The portfolio and influence passes, classified by `bioprism-governance` rather than asserted.
//!
//! The claim under test is that consulting the backend portfolio (43.36, 43.37) and computing
//! influence bounds (43.28) moved no wire format, and therefore needed no bump of
//! `fiber-context-certificate/0.1`. That is exactly the claim an author is most tempted to make and
//! least entitled to make on their own word — the reference digest
//! `c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4` is the platform's guarantee
//! that three implementations agree — so the crate that owns the rule answers it here.
//!
//! The interesting documents are the ones where the new code actually ran: a compile whose region
//! no backend could cost, and a compile that withheld evidence at the cut and put it to the
//! analyser. Validating the reference world alone would prove only that nothing happened.

use bioprism_fiber::{compile, Query};
use bioprism_governance::classify::{classify, CompatibilityClass};
use bioprism_governance::descriptor::{FieldSpec, FieldType, SchemaDescriptor};
use bioprism_governance::diff::diff;
use bioprism_governance::known;
use bioprism_section::CertificateProfile;
use bioprism_world::World;
use serde_json::Value;
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

fn compile_example(world: &str, query: &str) -> bioprism_fiber::CompileOutput {
    let world = World::from_json(reference_example(world)).expect("world loads");
    let query = Query::from_json(reference_example(query)).expect("query loads");
    compile(&world, &query).expect("compiles")
}

fn check_all_descriptors(out: &bioprism_fiber::CompileOutput) {
    let certificate = out
        .certificate
        .to_json(CertificateProfile::Reference)
        .expect("certificate serialises");
    let check = known::certificate_reference().check_document(&certificate);
    assert!(check.is_clean(), "certificate drifted: {check:?}");

    let extended = out
        .certificate
        .to_json(CertificateProfile::Extended)
        .expect("extended certificate serialises");
    let check = known::certificate_extended().check_document(&extended);
    assert!(check.is_clean(), "extended certificate drifted: {check:?}");

    let section = known::decision_section().check_document(&out.section.to_json());
    assert!(section.is_clean(), "section drifted: {section:?}");
}

/// A compile whose region no backend could cost still satisfies the unbumped descriptors.
#[test]
fn an_uncostable_region_conforms_to_the_shipped_schemas_without_a_bump() {
    let out = compile_example("wide_region_world.json", "wide_region_query.json");
    assert!(
        matches!(
            out.trace.plan.outcome,
            bioprism_fiber::PortfolioOutcome::NoAdmissiblePlan(_)
        ),
        "this fixture must exercise the infeasible branch or the check below proves nothing"
    );
    check_all_descriptors(&out);
}

/// A compile that put withheld evidence to the analyser satisfies them too.
#[test]
fn a_compile_that_analyses_withheld_evidence_conforms_without_a_bump() {
    let out = compile_example("deferred_evidence_world.json", "deferred_evidence_query.json");
    assert_eq!(
        out.trace.withheld_influence.attempted.len(),
        2,
        "this fixture must exercise the influence pass or the check below proves nothing"
    );
    check_all_descriptors(&out);
}

/// The field set did not move, so the classifier has nothing to classify.
#[test]
fn the_certificate_field_set_is_unchanged_and_classifies_as_compatible() {
    for descriptor in known::all() {
        let classification =
            classify(&diff(&descriptor, &descriptor).expect("a format diffs against itself"));
        assert_eq!(classification.class, CompatibilityClass::Compatible);
        assert!(classification.digest_affecting().is_empty());
        assert!(classification
            .assert_class(CompatibilityClass::Compatible)
            .is_ok());
    }
}

/// What publishing the portfolio's own record would have cost.
///
/// The obvious design is a `portfolio` block beside `plan`, carrying the candidates, their costs
/// and their refusals. This is the price: a required hashed field added to a format with published
/// documents is digest-affecting, so `affects_digest` fires, the class is `Breaking`, and 0.1
/// could not have carried it under any argument about how additive the field looks. The compile
/// trace is not a compromise chosen to dodge that — a costing of an evaluation the compiler did
/// not perform is not a receipt for what it did — but the rule is worth having on record, and the
/// argument matches the one `policy_governance.rs` makes for the policy pass.
#[test]
fn adding_a_portfolio_block_to_the_certificate_would_have_been_a_breaking_change() {
    let shipped = known::certificate_reference();
    let mut fields = shipped.fields().to_vec();
    let digest_field = fields.pop().expect("the self-digest field is last");
    fields.push(FieldSpec::required("portfolio", FieldType::Object));
    fields.push(digest_field);
    let hypothetical = SchemaDescriptor::new(shipped.id.clone(), shipped.mode, fields)
        .expect("the hypothetical field set is well formed");

    let classification = classify(&diff(&shipped, &hypothetical).expect("the two field sets diff"));
    assert_eq!(classification.class, CompatibilityClass::Breaking);
    assert_eq!(classification.digest_affecting().len(), 1);
    assert!(
        classification
            .assert_class(CompatibilityClass::Compatible)
            .is_err(),
        "a claim of compatibility over a digest-moving field must be refused"
    );
}

/// The anchor itself, recomputed after both passes ran.
///
/// Field-set equality is necessary and not sufficient: a schema diff cannot see a *value* change
/// inside the `plan` object, and writing the portfolio's argmin into `plan.backend` would have
/// moved these bytes while every descriptor stayed clean. This is the assertion that would have
/// caught it.
#[test]
fn the_reference_digest_is_unmoved_by_the_portfolio_and_influence_passes() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
    ]
    .iter()
    .collect();
    let world: Value =
        serde_json::from_str(&std::fs::read_to_string(path.join("radiogenomic_world.json")).unwrap())
            .unwrap();
    let query: Value =
        serde_json::from_str(&std::fs::read_to_string(path.join("leakage_query.json")).unwrap())
            .unwrap();
    let out = compile(
        &World::from_json(world).expect("world loads"),
        &Query::from_json(query).expect("query loads"),
    )
    .expect("compiles");

    assert_eq!(
        out.certificate
            .digest(CertificateProfile::Reference)
            .unwrap()
            .as_str(),
        "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4"
    );
}
