//! The exact serialised form of every typed identifier this crate publishes.
//!
//! Each identifier carries `#[serde(try_from = "String", into = "String")]`, which collapses the
//! newtype to a bare JSON string. A round-trip test cannot protect that shape: rewriting both
//! halves to an object form still round-trips cleanly. These types are published surface, so the
//! bytes on the wire are the contract and are asserted literally rather than symmetrically.

use bioprism_ids::{EventId, FactId, FactorId, IdError, QueryId, RunId, VariableName, WorldId};

macro_rules! assert_bare_string_wire_form {
    ($ty:ty) => {{
        let id = <$ty>::parse("sample-1").expect("well-formed identifier");
        assert_eq!(id.as_str(), "sample-1");
        assert_eq!(id.to_string(), "sample-1");
        assert_eq!(
            serde_json::to_string(&id).expect("identifier serialises"),
            "\"sample-1\""
        );
        let decoded: $ty = serde_json::from_str("\"sample-1\"").expect("identifier deserialises");
        assert_eq!(decoded, id);
        assert_eq!(String::from(id), "sample-1");
    }};
}

macro_rules! assert_kind_and_validation {
    ($ty:ty, $kind:literal) => {{
        assert_eq!(<$ty>::KIND, $kind);
        assert_eq!(<$ty>::parse(""), Err(IdError::Empty { kind: $kind }));
        assert_eq!(
            <$ty>::parse("a\u{7}b"),
            Err(IdError::ControlCharacter {
                kind: $kind,
                value: "a\u{7}b".to_string(),
            })
        );
    }};
}

#[test]
fn every_compiler_identifier_serialises_as_a_bare_json_string() {
    assert_bare_string_wire_form!(WorldId);
    assert_bare_string_wire_form!(QueryId);
    assert_bare_string_wire_form!(FactId);
    assert_bare_string_wire_form!(FactorId);
    assert_bare_string_wire_form!(EventId);
    assert_bare_string_wire_form!(VariableName);
    assert_bare_string_wire_form!(RunId);
}

#[test]
fn every_compiler_identifier_reports_its_own_kind_when_it_refuses_a_value() {
    assert_kind_and_validation!(WorldId, "world");
    assert_kind_and_validation!(QueryId, "query");
    assert_kind_and_validation!(FactId, "fact");
    assert_kind_and_validation!(FactorId, "factor");
    assert_kind_and_validation!(EventId, "event");
    assert_kind_and_validation!(VariableName, "variable");
    assert_kind_and_validation!(RunId, "run");
}

/// A module that shadows every prelude name the macro expansion reaches for.
///
/// `macro_rules!` is hygienic for local bindings and not for paths: whatever `String`, `Result`,
/// `Ok`, `Err`, `str`, `Into`, `From` and `TryFrom` mean *here* is what they would mean inside an
/// expansion that named them bare. Each is bound to an unrelated unit struct, so a bare path in
/// the expansion cannot compile — `Result<Self, IdError>` would be a unit struct given two type
/// arguments, `Ok(..)` a call to a struct that takes none.
///
/// The identifier is declared here and asserted from outside, because assertions written in this
/// scope could not name `String` either.
mod a_scope_that_shadows_every_prelude_name_the_expansion_uses {
    #![allow(non_camel_case_types, dead_code)]

    pub struct String;
    pub struct str;
    pub struct Result;
    pub struct Ok;
    pub struct Err;
    pub struct Into;
    pub struct From;
    pub struct TryFrom;

    bioprism_ids::validated_string_id!(
        /// Exists only to be generated in hostile scope.
        ShadowedId,
        "shadowed"
    );
}

/// The macro's path-hygiene claim, made falsifiable.
///
/// Not a compile test alone: an expansion could resolve and still misbehave, so the generated type
/// is put through the same wire-format and validation assertions every shipped identifier answers.
/// Remove one `::std::` or `::core::` prefix from the expansion and this file stops compiling,
/// which is what makes the doc comment on `validated_string_id!` a claim rather than a hope.
#[test]
fn an_identifier_generated_where_the_prelude_is_shadowed_behaves_like_every_other_one() {
    use a_scope_that_shadows_every_prelude_name_the_expansion_uses::ShadowedId;

    assert_bare_string_wire_form!(ShadowedId);
    assert_kind_and_validation!(ShadowedId, "shadowed");
}
