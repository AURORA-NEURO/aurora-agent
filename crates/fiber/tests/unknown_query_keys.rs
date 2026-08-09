//! What `fiber-query/0.1` does with a key it does not know.
//!
//! `crates/epistemic` found that a query carrying a `decision_loss` field — the field 43.50 needs
//! and `crates/examples` records as missing — is accepted with no error, no warning and no effect.
//! The parser reads the keys it knows and drops the rest, so `Query::missing_contract_fields` goes
//! on reporting the field missing while the document appears to supply it.
//!
//! Checking that produced a second half the original finding did not name, and it is the sharper
//! one. An unknown key is ignored *semantically* but not *cryptographically*: the query document is
//! hashed whole into `source_hashes.query_sha256`, which is inside the certificate's own digest. So
//! two compiles that selected identical facts, made identical omissions and produced a
//! byte-identical Decision Section carry **different certificate digests**.
//!
//! That is the wrong way round. A certificate digest is the platform's identity for a compiled
//! answer, and it now moves for a change that provably could not affect the answer — while a
//! caller who believes they supplied a decision loss gets no signal that nothing read it.
//!
//! Rejecting unknown keys would close both halves at once. That is a wire-format change: by
//! `crates/governance`'s own rule it is breaking, because a document that parses today would stop
//! parsing, so it needs a `fiber-query/0.1` to `0.2` bump and a matching change to the CPython
//! reference. These tests pin the present behaviour so the decision is made deliberately rather
//! than discovered.

use bioprism_fiber::{compile, Query};
use bioprism_section::CertificateProfile;
use bioprism_world::World;
use serde_json::{json, Value};

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
fn a_decision_loss_field_is_accepted_and_read_by_nothing() {
    let document = with_extra_key(
        "decision_loss",
        json!({
            "actions": ["treat", "observe"],
            "models": ["m1"],
            "loss": [[0.0, 1.0]]
        }),
    );

    let query = Query::from_json(document.clone()).expect("an unknown key does not fail the parse");

    assert!(
        query
            .missing_contract_fields()
            .iter()
            .any(|field| field.contains("decision_loss")),
        "the field is reported missing by the very query document that supplies it; that is the \
         defect, not a quirk of this test"
    );
}

#[test]
fn an_entirely_meaningless_key_is_accepted_too() {
    let document = with_extra_key("utter_nonsense_key", json!(12345));
    assert!(
        Query::from_json(document.clone()).is_ok(),
        "fiber-query/0.1 has no unknown-key rejection at all, so the decision_loss case is not \
         special — it is the general behaviour seen through a field somebody cared about"
    );
}

#[test]
fn an_ignored_key_still_moves_the_certificate_digest_it_could_not_have_affected() {
    let world = reference_world();

    let baseline_document = reference_query_document();
    let baseline = compile(
        &world,
        &Query::from_json(baseline_document.clone()).expect("the reference query parses"),
    )
    .expect("the reference world compiles");

    let noisy_document = with_extra_key("utter_nonsense_key", json!(12345));
    let noisy = compile(
        &world,
        &Query::from_json(noisy_document.clone()).expect("an unknown key does not fail the parse"),
    )
    .expect("the reference world still compiles");

    assert_eq!(
        baseline.certificate.selected_facts, noisy.certificate.selected_facts,
        "an unread key cannot change which facts were selected"
    );
    assert_eq!(
        baseline.certificate.source_hashes.decision_section_sha256,
        noisy.certificate.source_hashes.decision_section_sha256,
        "nor the Decision Section that was delivered"
    );

    assert_ne!(
        baseline.certificate.source_hashes.query_sha256,
        noisy.certificate.source_hashes.query_sha256,
        "but the whole query document is hashed, so the query digest moves"
    );
    let baseline_digest = baseline
        .certificate
        .digest(CertificateProfile::Reference)
        .expect("the reference certificate hashes");
    let noisy_digest = noisy
        .certificate
        .digest(CertificateProfile::Reference)
        .expect("the noisy certificate hashes");
    assert_ne!(
        baseline_digest, noisy_digest,
        "and the query digest is inside the certificate, so a compile that provably could not \
         differ carries a different identity"
    );
}
