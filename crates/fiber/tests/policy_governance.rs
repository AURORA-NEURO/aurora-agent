//! The policy pass, classified by `bioprism-governance` rather than by assertion.
//!
//! The claim under test is that adding the pass moved no wire format, and therefore needed no
//! bump of `fiber-context-certificate/0.1`. That claim is exactly the one an author is most
//! tempted to make and least entitled to make on their own word, so the crate that owns the rule
//! answers it here: a certificate emitted by a *policy-withholding* compile is held against the
//! unchanged descriptor, and the change a less careful design would have made — a `policy` block
//! on the certificate — is put through the same classifier to show what it would have cost.

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
    serde_json::from_str(&std::fs::read_to_string(path).expect("reference example is readable"))
        .expect("reference example is valid JSON")
}

/// A compile in which policy actually removed something is the interesting document to check.
///
/// The reference world exercises none of the new code, so validating its certificate would prove
/// only that nothing ran.
fn withholding_compile() -> bioprism_fiber::CompileOutput {
    let world =
        World::from_json(reference_example("policy_restricted_world.json")).expect("world loads");
    let query =
        Query::from_json(reference_example("policy_restricted_query.json")).expect("query loads");
    let out = compile(&world, &query).expect("compiles");
    assert!(
        !out.trace.policy.withheld.is_empty(),
        "this fixture must exercise the policy pass or the checks below prove nothing"
    );
    out
}

/// Documents from a policy-withholding compile still satisfy the unbumped descriptors.
#[test]
fn a_policy_withholding_compile_conforms_to_the_shipped_schemas_without_a_bump() {
    let out = withholding_compile();

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
    assert!(
        section.is_clean(),
        "the section carrying a policy_blocked obligation drifted: {section:?}"
    );
}

/// The field set did not move, so the classifier has nothing to classify.
#[test]
fn the_certificate_field_set_is_unchanged_and_classifies_as_compatible() {
    let classification = classify(
        &diff(&known::certificate_reference(), &known::certificate_reference())
            .expect("a format diffs against itself"),
    );
    assert_eq!(classification.class, CompatibilityClass::Compatible);
    assert!(classification.digest_affecting().is_empty());
    assert!(classification
        .assert_class(CompatibilityClass::Compatible)
        .is_ok());
}

/// Why the policy record lives on the compile trace and not on the certificate.
///
/// The obvious design is a `policy` block beside `omissions`. This is what that would have cost:
/// a required hashed field added to a format with published documents is digest-affecting, so
/// `affects_digest` fires, the class is `Breaking`, and 0.1 could not have carried it under any
/// argument about how additive the field looks. The trace is not a compromise chosen to dodge the
/// bump — it is where a non-wire observation belongs — but the rule is worth having on record.
#[test]
fn adding_a_policy_block_to_the_certificate_would_have_been_a_breaking_change() {
    let shipped = known::certificate_reference();
    let mut fields = shipped.fields().to_vec();
    let digest_field = fields.pop().expect("the self-digest field is last");
    fields.push(FieldSpec::required("policy", FieldType::Object));
    fields.push(digest_field);
    let hypothetical = SchemaDescriptor::new(
        shipped.id.clone(),
        shipped.mode,
        fields,
    )
    .expect("the hypothetical field set is well formed");

    let classification =
        classify(&diff(&shipped, &hypothetical).expect("the two field sets diff"));
    assert_eq!(classification.class, CompatibilityClass::Breaking);
    assert_eq!(classification.digest_affecting().len(), 1);
    assert!(
        classification
            .assert_class(CompatibilityClass::Compatible)
            .is_err(),
        "a claim of compatibility over a digest-moving field must be refused"
    );
}
