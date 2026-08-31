//! The pack document is strict at the boundary: everything validates on load, nothing lazily.

use bioprism_domain::{DomainPack, DOMAIN_SCHEMA_VERSION};
use bioprism_scope::ScopeClass;
use serde_json::{json, Value};

fn minimal_oracle() -> Value {
    json!({
        "kind": "rule/minimal-v1",
        "checks": [
            { "name": "check", "description": "d", "when": { "kind": "exists", "variable": "v" } }
        ]
    })
}

#[test]
fn a_minimal_pack_parses_and_defaults_are_explicit() {
    let pack = DomainPack::from_json(&json!({
        "schema_version": DOMAIN_SCHEMA_VERSION,
        "name": "minimal",
        "description": "smallest declarable pack",
        "oracle": minimal_oracle()
    }))
    .expect("parses");
    assert_eq!(pack.name(), "minimal");
    assert_eq!(pack.goal(), None);
    assert!(pack.protected_tags().is_empty());
    assert_eq!(pack.dimension_registry().classify("cohort"), ScopeClass::Identity);
}

#[test]
fn an_undeclared_key_is_refused_before_a_missing_field_is_reported() {
    let error = DomainPack::from_json(&json!({
        "schema_version": DOMAIN_SCHEMA_VERSION,
        "name": "typo",
        "description": "d",
        "oracel": minimal_oracle()
    }))
    .expect_err("misspelled key is refused");
    assert!(error.to_string().contains("oracel"), "{error}");
}

#[test]
fn a_pack_without_the_declared_schema_version_is_refused() {
    for document in [
        json!({ "name": "n", "description": "d", "oracle": minimal_oracle() }),
        json!({ "schema_version": "bioprism-domain/0.2", "name": "n", "description": "d",
                "oracle": minimal_oracle() }),
    ] {
        DomainPack::from_json(&document).expect_err("schema version is not optional");
    }
}

#[test]
fn a_pack_validates_its_dimension_document_on_load_not_on_use() {
    let error = DomainPack::from_json(&json!({
        "schema_version": DOMAIN_SCHEMA_VERSION,
        "name": "bad-dims",
        "description": "d",
        "scope_dimensions": {
            "schema_version": "bioprism-scope-dimensions/0.1",
            "dimensions": { "cohort": "policy" }
        },
        "oracle": minimal_oracle()
    }))
    .expect_err("a reclassifying dimension table is refused at the boundary");
    assert!(error.to_string().contains("reclassify"), "{error}");
}

#[test]
fn a_pack_name_is_a_slug_because_it_names_files_and_verdicts() {
    for name in ["", "Has Spaces", "UPPER", "under_score"] {
        DomainPack::from_json(&json!({
            "schema_version": DOMAIN_SCHEMA_VERSION,
            "name": name,
            "description": "d",
            "oracle": minimal_oracle()
        }))
        .expect_err("non-slug name is refused");
    }
}
