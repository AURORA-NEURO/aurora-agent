//! Dimension classifications can come from a document, not only from the built-in table.
//!
//! Blueprint 43.03 makes protected closure a per-class rule, so classification is semantic:
//! the default table covers the neuro-oncology vocabulary, and any other domain declares its
//! own dimensions. The rule that a canonical dimension cannot be silently reclassified holds
//! for documents exactly as it does for [`DimensionRegistry::register`].

use bioprism_scope::{DimensionRegistry, ScopeClass, DIMENSIONS_SCHEMA_VERSION};
use serde_json::json;

#[test]
fn a_domain_document_extends_the_default_table() {
    let registry = DimensionRegistry::from_json(&json!({
        "schema_version": DIMENSIONS_SCHEMA_VERSION,
        "dimensions": {
            "account": "identity",
            "venue": "region",
            "matter": "specimen",
            "filing_date": "time"
        }
    }))
    .expect("document loads");

    assert_eq!(registry.classify("account"), ScopeClass::Identity);
    assert_eq!(registry.classify("venue"), ScopeClass::Region);
    assert_eq!(registry.classify("cohort"), ScopeClass::Identity);
    assert_eq!(registry.classify("never_declared"), ScopeClass::Unclassified);
}

#[test]
fn a_document_cannot_reclassify_a_canonical_dimension() {
    let error = DimensionRegistry::from_json(&json!({
        "schema_version": DIMENSIONS_SCHEMA_VERSION,
        "dimensions": { "cohort": "policy" }
    }))
    .expect_err("reclassification is refused");
    assert!(error.contains("cannot reclassify"), "unexpected error: {error}");
}

#[test]
fn an_unknown_class_name_is_refused_with_the_canonical_set_named() {
    let error = DimensionRegistry::from_json(&json!({
        "schema_version": DIMENSIONS_SCHEMA_VERSION,
        "dimensions": { "account": "identify" }
    }))
    .expect_err("unknown class is refused");
    assert!(error.contains("identity, region"), "unexpected error: {error}");
}

#[test]
fn unclassified_is_not_a_declarable_class() {
    assert_eq!(ScopeClass::parse("unclassified"), None);
    let error = DimensionRegistry::from_json(&json!({
        "schema_version": DIMENSIONS_SCHEMA_VERSION,
        "dimensions": { "account": "unclassified" }
    }))
    .expect_err("declaring the absence of a classification is refused");
    assert!(error.contains("unknown class"), "unexpected error: {error}");
}

#[test]
fn a_document_without_the_declared_schema_version_is_refused() {
    for document in [
        json!({ "dimensions": { "account": "identity" } }),
        json!({ "schema_version": "bioprism-scope-dimensions/0.2", "dimensions": {} }),
        json!({ "schema_version": DIMENSIONS_SCHEMA_VERSION, "dims": {} }),
    ] {
        DimensionRegistry::from_json(&document).expect_err("malformed document is refused");
    }
}
