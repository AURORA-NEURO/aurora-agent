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
