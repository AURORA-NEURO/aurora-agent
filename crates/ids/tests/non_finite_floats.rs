//! What happens to a non-finite float on the way to a digest.
//!
//! [`bioprism_ids::CanonicalError::NonFiniteNumber`] exists and is unreachable through the path
//! every producer in this workspace actually uses. `serde_json::to_value` maps `NaN`, `+inf` and
//! `-inf` to `Value::Null` before the canonical encoder ever sees a number, so the encoder has
//! nothing to refuse.
//!
//! The consequence is narrower than "a NaN hashes like a missing field" — it does not, because an
//! explicit null and an absent key are different documents. The consequence is that **a corrupt
//! number is silently laundered into a stated null**, which in a platform whose thesis is that
//! "unknown" and "zero" must never share a representation is the wrong direction to fail in.
//!
//! It also carries a latent cross-language hazard. CPython's `json.dumps` emits the bare token
//! `NaN` by default, which is not JSON and which serde would refuse to parse. Rust and CPython
//! therefore disagree about what a non-finite float *is*, and they only agree today because no
//! producer has emitted one.
//!
//! The typed `ContentHash::of_serializable` path now rejects non-finite floats before conversion.
//! This file keeps the separate `of_value` limit explicit: once callers have converted data to a
//! `Value`, a stated null cannot be distinguished from a float that serde_json already mapped to
//! null. Producers should hash their typed value before that lossy conversion, or validate the
//! numeric fields before constructing a `Value`.

use bioprism_ids::ContentHash;
use serde_json::json;
use std::collections::BTreeMap;

fn hash_of(value: &serde_json::Value) -> String {
    ContentHash::of_value(value)
        .expect("a value serde produced is canonicalisable")
        .as_str()
        .to_string()
}

fn single_float(value: f64) -> serde_json::Value {
    serde_json::to_value(BTreeMap::from([("a", value)])).expect("a float map serialises")
}

#[test]
fn a_nan_an_infinity_and_a_stated_null_are_one_document_by_the_time_they_are_hashed() {
    let nan = single_float(f64::NAN);
    let positive_infinity = single_float(f64::INFINITY);
    let negative_infinity = single_float(f64::NEG_INFINITY);
    let stated_null = json!({ "a": null });

    assert_eq!(nan, stated_null, "serde_json::to_value maps NaN to null");
    assert_eq!(positive_infinity, stated_null);
    assert_eq!(negative_infinity, stated_null);

    let digest = hash_of(&stated_null);
    assert_eq!(hash_of(&nan), digest);
    assert_eq!(hash_of(&positive_infinity), digest);
    assert_eq!(hash_of(&negative_infinity), digest);
}

#[test]
fn a_stated_null_and_an_absent_key_remain_different_documents() {
    let stated = json!({ "id": "x", "amount": null });
    let absent = json!({ "id": "x" });

    assert_ne!(
        hash_of(&stated),
        hash_of(&absent),
        "the canonical encoder distinguishes a key present with a null value from a missing key; \
         only the non-finite float collapses, and it collapses onto the former"
    );
}

#[test]
fn serde_json_values_cannot_represent_non_finite_numbers() {
    let finite = single_float(1.5);
    assert!(ContentHash::of_value(&finite).is_ok());

    assert!(
        serde_json::Number::from_f64(f64::NAN).is_none(),
        "serde_json refuses to build a non-finite Number, which is why no Value carrying one \
         reaches the canonical encoder and why NonFiniteNumber cannot fire on this path"
    );
}
