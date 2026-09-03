//! What `fiber-query/0.2` does with a key it does not know, and what 0.1 did.
//!
//! # The finding, kept because the fix is only legible against it
//!
//! `crates/epistemic` found that a query carrying a `decision_loss` field — the field 43.50 needs
//! and `crates/examples` records as missing — was accepted with no error, no warning and no
//! effect. The parser read the keys it knew and dropped the rest, so
//! `Query::missing_contract_fields` went on reporting the field missing while the document
//! appeared to supply it.
//!
//! Checking that produced a second half the original finding did not name, and it is the sharper
//! one. An unknown key was ignored *semantically* but not *cryptographically*: the query document
//! is hashed whole into `source_hashes.query_sha256`, which is inside the certificate's own
//! digest. So two compiles that selected identical facts, made identical omissions and produced a
//! byte-identical Decision Section carried **different certificate digests**.
//!
//! That is the wrong way round. A certificate digest is the platform's identity for a compiled
//! answer, and it moved for a change that provably could not affect the answer — while a caller
//! who believed they supplied a decision loss got no signal that nothing read it.
//!
//! # What shipped
//!
//! `fiber-query/0.2` refuses the document. The tests below are now regression guards rather than
//! a record of a defect: the rejection is asserted, the change is *classified* by
//! `bioprism-governance` rather than asserted to be breaking, the accepted key set is held against
//! the published schema, and the reference pair still compiles to the digest three implementations
//! agree on.
//!
//! Two things did not change and are pinned here so that nobody has to rediscover them.
//!
//! **A declared field that no pass reads still moves the digest.** `role` is parsed and consulted
//! by nothing, and editing it still produces a different certificate for an identical answer. That
//! is not the same defect wearing a different hat: the certificate binds the document it was
//! given, and `role` is part of that document by the format's own declaration. What 0.2 removed is
//! the ability to move an answer's identity through bytes the parser *refuses to interpret*.
//!
//! **`fiber-world/0.1` has the identical hole.** It is a second format, a second lineage and a far
//! larger dependent set, so it is a second change; the guard below pins the hole open rather than
//! letting the query fix imply a world fix that did not happen.

use bioprism_fiber::{
    compile, FiberError, Query, ACCEPTED_QUERY_SCHEMA_VERSIONS, QUERY_FIELD_PATHS,
};
use bioprism_governance::classify::{classify, CompatibilityClass};
use bioprism_governance::descriptor::{FieldSpec, FieldType, SchemaDescriptor};
use bioprism_governance::diff::diff;
use bioprism_governance::mode::CompatibilityMode;
use bioprism_governance::version::{SchemaId, VersionBump};
use bioprism_section::CertificateProfile;
use bioprism_world::World;
use serde_json::{json, Value};

/// The digest three implementations agree on: CPython, the eager Rust path, the indexed store.
const REFERENCE_CERTIFICATE_DIGEST: &str =
    "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4";

fn reference_world() -> World {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fiber-v0.1/radiogenomic_world.json"
    );
    let raw: Value = serde_json::from_str(
        &std::fs::read_to_string(path).expect("the reference world is readable"),
    )
    .expect("the reference world is JSON");
    World::from_json(raw).expect("the reference world parses")
}

fn reference_query_document() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fiber-v0.1/leakage_query.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the reference query is readable"))
        .expect("the reference query parses")
}

fn with_extra_key(key: &str, value: Value) -> Value {
    let mut document = reference_query_document();
    document
        .as_object_mut()
        .expect("a query document is an object")
        .insert(key.to_string(), value);
    document
}

#[test]
fn required_arrays_and_budget_bounds_are_enforced_at_the_wire_boundary() {
    let mut missing_tags = reference_query_document();
    missing_tags
        .as_object_mut()
        .expect("a query document is an object")
        .remove("protected_tags");
    assert!(matches!(
        Query::from_json(missing_tags),
        Err(FiberError::MissingQueryField("protected_tags"))
    ));

    let mut wrong_tags = reference_query_document();
    wrong_tags["protected_tags"] = json!("not-an-array");
    assert!(matches!(
        Query::from_json(wrong_tags),
        Err(FiberError::WrongQueryFieldType {
            field: "protected_tags",
            ..
        })
    ));

    let mut no_targets = reference_query_document();
    no_targets["targets"] = json!([]);
    assert!(matches!(
        Query::from_json(no_targets),
        Err(FiberError::InvalidIdentifier(_))
    ));

    let mut zero_budget = reference_query_document();
    zero_budget["budgets"]["max_facts"] = json!(0);
    assert!(matches!(
        Query::from_json(zero_budget),
        Err(FiberError::InvalidBudget(_))
    ));
}

#[test]
fn policy_must_be_an_array_of_non_empty_strings() {
    let mut wrong_type = reference_query_document();
    wrong_type["policy"] = json!({"allow": true});
    assert!(matches!(
        Query::from_json(wrong_type),
        Err(FiberError::WrongQueryFieldType {
            field: "policy",
            ..
        })
    ));

    let mut empty_clause = reference_query_document();
    empty_clause["policy"] = json!([""]);
    assert!(matches!(
        Query::from_json(empty_clause),
        Err(FiberError::InvalidDecisionContract(_))
    ));
}

fn certificate_digest(document: &Value) -> String {
    compile(
        &reference_world(),
        &Query::from_json(document.clone()).expect("the document is an accepted query"),
    )
    .expect("the reference world compiles")
    .certificate
    .digest(CertificateProfile::Reference)
    .expect("the certificate hashes")
    .as_str()
    .to_string()
}

#[test]
fn a_decision_loss_field_is_refused_by_name_rather_than_discarded() {
    let document = with_extra_key(
        "decision_loss",
        json!({
            "actions": ["treat", "observe"],
            "models": ["m1"],
            "loss": [[0.0, 1.0]]
        }),
    );

    match Query::from_json(document) {
        Err(FiberError::UnknownQueryFields { fields, accepted }) => {
            assert_eq!(
                fields,
                ["decision_loss"],
                "the refusal must name the offending key; 'invalid query' would send the caller \
                 back through the whole document"
            );
            assert_eq!(
                accepted, QUERY_FIELD_PATHS,
                "and list what the format does declare, so the caller can tell a field the schema \
                 lacks from one they misspelled"
            );
        }
        other => panic!("a decision_loss field must be refused, got {other:?}"),
    }
}

#[test]
fn the_refusal_names_every_undeclared_key_sorted_rather_than_the_first_one_found() {
    let mut document = with_extra_key("utter_nonsense_key", json!(12345));
    document["decision_loss"] = json!({});
    document["budgets"]["max_items"] = json!(3);

    match Query::from_json(document) {
        Err(FiberError::UnknownQueryFields { fields, .. }) => assert_eq!(
            fields,
            ["budgets.max_items", "decision_loss", "utter_nonsense_key"],
            "serde_json runs with preserve_order, so document order is not stable enough to report \
             against; and a caller fixing a generator wants the whole list, not one round trip per \
             key"
        ),
        other => panic!("every undeclared key must be refused, got {other:?}"),
    }
}

#[test]
fn an_undeclared_key_is_refused_before_a_missing_required_field_is_reported() {
    let mut document = with_extra_key("target", json!(["split_supports_external_validity"]));
    document
        .as_object_mut()
        .expect("a query document is an object")
        .remove("targets");

    match Query::from_json(document) {
        Err(FiberError::UnknownQueryFields { fields, .. }) => assert_eq!(fields, ["target"]),
        other => panic!(
            "a misspelled key must be reported as the misspelling it is, not as the field it \
             failed to be; got {other:?}"
        ),
    }
}

/// The half of the defect that was cryptographic, closed at the only place it could be closed.
#[test]
fn no_document_the_parser_accepts_can_move_the_digest_through_bytes_it_refuses_to_interpret() {
    let baseline = certificate_digest(&reference_query_document());
    assert_eq!(
        baseline, REFERENCE_CERTIFICATE_DIGEST,
        "the reference pair still compiles to the digest CPython and the indexed store agree on"
    );

    assert!(
        Query::from_json(with_extra_key("utter_nonsense_key", json!(12345))).is_err(),
        "the only route from an uninterpreted byte to the certificate digest was an accepted \
         parse, and there is no longer one"
    );
}

/// A query that legitimately differs gets a different identity, and that is the correct behaviour.
///
/// `role` is declared by the format and read by no pass in this crate. Editing it cannot change
/// which facts are selected or what the Decision Section says, and it still moves the certificate
/// digest. That is the certificate binding its inputs — `role` is part of the document the caller
/// handed over — and it is categorically different from the defect 0.2 fixed, where the moving
/// bytes were ones no version of the format had ever agreed were input.
#[test]
fn a_declared_but_unread_field_still_moves_the_digest_because_the_document_genuinely_differs() {
    let world = reference_world();
    let baseline_document = reference_query_document();
    let mut edited_document = baseline_document.clone();
    edited_document["role"] = json!("a-different-declared-role");

    let baseline = compile(
        &world,
        &Query::from_json(baseline_document).expect("the reference query parses"),
    )
    .expect("the reference world compiles");
    let edited = compile(
        &world,
        &Query::from_json(edited_document.clone()).expect("role is a declared field"),
    )
    .expect("the reference world still compiles");

    assert_eq!(
        baseline.certificate.selected_facts, edited.certificate.selected_facts,
        "no pass reads role, so it cannot change the selection"
    );
    assert_eq!(
        baseline.certificate.source_hashes.decision_section_sha256,
        edited.certificate.source_hashes.decision_section_sha256,
        "nor the Decision Section that was delivered"
    );
    assert_ne!(
        certificate_digest(&edited_document),
        REFERENCE_CERTIFICATE_DIGEST,
        "and the certificate still binds the document it was given, undeclared keys aside"
    );
}

/// The frozen reference pair keeps its 0.1 label, and a 0.2 reader still reads it.
///
/// Re-labelling it would move `query_sha256` and therefore the certificate digest of the one
/// artifact whose stability is the entire cross-language guarantee — to record a change in the
/// reader rather than in the document. The published schema has declared
/// `additionalProperties: false` since 0.1, so no document that was ever valid is invalidated by
/// 0.2; only the readers' leniency is withdrawn.
#[test]
fn a_query_still_labelled_0_1_is_read_and_compiles_to_the_unmoved_reference_digest() {
    let document = reference_query_document();
    assert_eq!(
        document["schema_version"], "fiber-query/0.1",
        "the frozen fixture must keep the label it was frozen under"
    );
    assert_eq!(certificate_digest(&document), REFERENCE_CERTIFICATE_DIGEST);

    let mut relabelled = document.clone();
    relabelled["schema_version"] = json!("fiber-query/0.2");
    assert!(
        Query::from_json(relabelled.clone()).is_ok(),
        "both labels are read: {ACCEPTED_QUERY_SCHEMA_VERSIONS:?}"
    );
    assert_ne!(
        certificate_digest(&relabelled),
        REFERENCE_CERTIFICATE_DIGEST,
        "which is exactly why the fixture was not relabelled: the version string is inside the \
         hashed bytes, so a purely editorial relabel would have moved the parity anchor"
    );

    assert!(matches!(
        Query::from_json({
            let mut unknown_version = document.clone();
            unknown_version["schema_version"] = json!("fiber-query/0.9");
            unknown_version
        }),
        Err(FiberError::UnsupportedQuerySchema { .. })
    ));
}

/// The accepted key set and the published schema are one thing, checked as an equality.
///
/// `bioprism-governance`'s `known.rs` holds the certificate's descriptor against the bytes the
/// code emits for the same reason, and its module docs record what the drift cost when it was
/// allowed: the published schema described ten fields while the implementations emitted twelve.
/// The query had the same drift in the other direction — the compiler has always read `goal` and
/// this schema never listed it — and a subset check would not have caught it.
#[test]
fn the_published_json_schema_declares_exactly_the_keys_the_parser_accepts() {
    const PUBLISHED: &str = include_str!("../../../schemas/fiber-v0.1/query.schema.json");
    let published: Value = serde_json::from_str(PUBLISHED).expect("the published schema parses");
    let properties = published["properties"]
        .as_object()
        .expect("the published schema lists properties");

    let mut declared: Vec<String> = properties.keys().cloned().collect();
    for key in published["properties"]["budgets"]["properties"]
        .as_object()
        .expect("budgets is a closed object with declared members")
        .keys()
    {
        declared.push(format!("budgets.{key}"));
    }
    declared.sort();

    assert_eq!(
        declared, QUERY_FIELD_PATHS,
        "a key the parser accepts and the schema omits is a field no conforming producer knows it \
         may send; a key the schema declares and the parser refuses is a document the schema says \
         is valid and the compiler rejects"
    );
    assert_eq!(
        published["additionalProperties"],
        json!(false),
        "the schema said this before the parser did; 0.2 is the parser catching up"
    );
}

/// The version bump is classified by the crate that owns the rule, not claimed by this one.
///
/// The field set did not move: nothing was added, removed or retyped. What moved is the reader's
/// declared handling of a key nobody declared, and 43.35 makes that a first-class property rather
/// than an implementation detail. `bioprism-governance` therefore returns exactly two facts, both
/// load-bearing and neither one this test's opinion: the change is `Breaking` and needs a major
/// bump, which under a zero major is `0.1 -> 0.2`; and it is `Breaking` *without* moving any
/// digest, which is what let the reference fixtures stay byte-identical.
#[test]
fn refusing_unknown_keys_classifies_as_breaking_without_affecting_a_single_digest() {
    let from = query_descriptor("fiber-query/0.1", CompatibilityMode::PreserveAndForward);
    let to = query_descriptor("fiber-query/0.2", CompatibilityMode::Reject);

    let classification = classify(&diff(&from, &to).expect("0.1 and 0.2 are one lineage"));

    assert!(
        classification.verdicts.is_empty(),
        "no field changed, so there is nothing to classify field by field: {:?}",
        classification.verdicts
    );
    assert_eq!(
        classification.mode_change,
        Some((
            CompatibilityMode::PreserveAndForward,
            CompatibilityMode::Reject
        ))
    );
    assert_eq!(classification.class, CompatibilityClass::Breaking);
    assert_eq!(classification.required_bump, VersionBump::Major);
    assert_eq!(
        classification.observed_bump,
        VersionBump::Major,
        "0.1 -> 0.2 under a zero major is the breaking bump"
    );
    classification
        .version_gate()
        .expect("the shipped bump is large enough for what changed");

    assert!(
        classification.digest_affecting().is_empty(),
        "and it moves no digest, which is why every shipped fixture and the reference certificate \
         are byte-identical across this change"
    );
    assert!(
        classification
            .assert_class(CompatibilityClass::Compatible)
            .is_err(),
        "a document that parsed yesterday and does not today is not a compatible change, however \
         few fields moved"
    );
}

/// Preserve-and-forward is right for an artifact being relayed and wrong for an input being run.
///
/// The 0.1 reader kept the whole document verbatim — `Query::raw` retains it and the certificate
/// hashes it — so by the letter of `CompatibilityMode` it was `PreserveAndForward` and a round
/// trip left every digest intact. That is the correct posture for a Context Certificate passing
/// through an old reader. For a query it is the bug: preserving bytes you have refused to
/// interpret is precisely how an unread key reached the certificate digest. An input document's
/// only safe posture is `Reject`.
#[test]
fn the_mode_that_is_correct_for_a_relayed_certificate_is_the_wrong_one_for_an_executed_query() {
    assert!(CompatibilityMode::PreserveAndForward.preserves_bytes());
    assert!(
        CompatibilityMode::PreserveAndForward.tolerates_unknown(),
        "which for an input means: hashed into the answer's identity, and read by nothing"
    );
    assert!(!CompatibilityMode::Reject.tolerates_unknown());
}

/// `fiber-world/0.1` still has the hole. Pinned open so the query fix does not imply a world fix.
///
/// The world parser reads named keys and retains the raw document deliberately, because
/// certificate hashes are taken over the original bytes — so a world carrying a key nothing reads
/// compiles to identical facts and a different certificate digest, exactly as the query did. It is
/// not fixed here because it is a different named format (a second lineage `SchemaId::successor_of`
/// will not compare against this one), because `World::validate_reference_compat` is documented as
/// deliberately no stricter than the CPython `validate_world`, and because a world is not flat:
/// `scope` is an open extension point that `bioprism-scope` registers dimensions into, so
/// "undeclared key" inside a fact needs a design decision this change has no basis to make.
///
/// When that change happens, this test is the one that has to be rewritten, and its failure is the
/// notification.
#[test]
fn the_world_parser_still_accepts_an_undeclared_key_and_still_moves_the_digest_for_it() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/fiber-v0.1/radiogenomic_world.json"
    );
    let baseline_document: Value = serde_json::from_str(
        &std::fs::read_to_string(path).expect("the reference world is readable"),
    )
    .expect("the reference world is JSON");

    let mut noisy_document = baseline_document.clone();
    noisy_document
        .as_object_mut()
        .expect("a world document is an object")
        .insert("utter_nonsense_key".to_string(), json!(12345));

    let noisy = World::from_json(noisy_document)
        .expect("fiber-world/0.1 has no unknown-key rejection; that is the open half");
    let query = Query::from_json(reference_query_document()).expect("the reference query parses");

    let baseline = compile(
        &World::from_json(baseline_document).expect("the reference world parses"),
        &query,
    )
    .expect("compiles");
    let compiled = compile(&noisy, &query).expect("still compiles");

    assert_eq!(
        baseline.certificate.selected_facts, compiled.certificate.selected_facts,
        "an unread key cannot change which facts were selected"
    );
    assert_ne!(
        baseline.certificate.source_hashes.world_sha256,
        compiled.certificate.source_hashes.world_sha256,
        "but the whole world document is hashed, so the world digest moves — and with it the \
         certificate's, for a change that provably could not affect the answer"
    );
}

/// `fiber-query/0.2` as `bioprism-governance` describes it, for the classifier above.
///
/// Every field is hashed because the query document is hashed whole into
/// `source_hashes.query_sha256`; there is no excluded key, because unlike a certificate a query
/// carries no digest of its own.
fn query_descriptor(id: &str, mode: CompatibilityMode) -> SchemaDescriptor {
    SchemaDescriptor::new(
        SchemaId::parse(id).expect("a query schema id is well formed"),
        mode,
        vec![
            FieldSpec::required("schema_version", FieldType::Const(id.to_string())),
            FieldSpec::required("query_id", FieldType::String),
            FieldSpec::required("targets", FieldType::Array),
            FieldSpec::required("protected_tags", FieldType::Array),
            FieldSpec::required("decision_time", FieldType::String),
            FieldSpec::required("budgets", FieldType::Group),
            FieldSpec::required("budgets.max_facts", FieldType::Integer),
            FieldSpec::optional("budgets.max_tokens", FieldType::Integer),
            FieldSpec::optional("role", FieldType::String),
            FieldSpec::optional("policy", FieldType::Array),
            FieldSpec::optional("distortion_tolerance", FieldType::Number),
            FieldSpec::optional("goal", FieldType::String),
        ],
    )
    .expect("the query field set is well formed")
}
