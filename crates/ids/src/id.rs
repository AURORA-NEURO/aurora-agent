//! Typed identifiers.
//!
//! The blueprint's invariant (40.05, and the MCP invariant list in 11.11) is that a benchmark
//! family, a parent world, a generated instance, an execution trial and a scored result are
//! never conflated. Distinct newtypes make conflation a compile error rather than a convention.

/// Declares a validated string newtype that serialises as a bare JSON string.
///
/// Exported rather than kept private because `bioprism-bioir` and `bioprism-hub` publish
/// identifiers of exactly this shape over the same [`crate::IdError`]. Separate copies of the
/// expansion drift, and drift here is not a style defect: these types are published surface,
/// so a divergence is a silent wire-format change in one crate and not the others.
///
/// # Why the name is not `typed_id`
///
/// `bioprism-adaptive` and `bioprism-biolang` each define a private `typed_id!` that expands over
/// that crate's own error type, so neither can call this one and neither should be folded into it.
/// Textual scope resolves the clash in their favour today, which is exactly the problem: an
/// exported macro that shares a name with two divergent local ones is a trap waiting for whichever
/// call site loses the local definition. The exported name describes what it builds instead.
///
/// # Path hygiene
///
/// `macro_rules!` is hygienic for local bindings and not for paths, so a caller that shadows a
/// prelude name would otherwise get that shadow spliced into the expansion — a broken build at
/// best and a differently-behaved identifier at worst. Every path here is therefore absolute,
/// `String`, `str`, `Result`, `Ok`, `Err`, the conversion traits and the derive macros included.
/// A caller needs only `serde` in its manifest.
///
/// The claim is checked rather than asserted: `tests/identifier_wire_format.rs` invokes this macro
/// inside a module that shadows every one of those names with an unrelated type, and asserts the
/// generated identifier still parses, displays, validates and serialises as it does anywhere else.
#[macro_export]
macro_rules! validated_string_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::cmp::PartialOrd,
            ::core::cmp::Ord,
            ::core::hash::Hash,
            ::serde::Serialize,
            ::serde::Deserialize
        )]
        #[serde(try_from = "::std::string::String", into = "::std::string::String")]
        pub struct $name(::std::string::String);

        impl $name {
            pub const KIND: &'static ::core::primitive::str = $kind;

            pub fn parse(
                value: impl ::core::convert::Into<::std::string::String>,
            ) -> ::core::result::Result<Self, $crate::IdError> {
                let value: ::std::string::String = ::core::convert::Into::into(value);
                if value.is_empty() {
                    return ::core::result::Result::Err($crate::IdError::Empty { kind: $kind });
                }
                if value.chars().any(|c| c.is_control()) {
                    return ::core::result::Result::Err($crate::IdError::ControlCharacter {
                        kind: $kind,
                        value,
                    });
                }
                ::core::result::Result::Ok($name(value))
            }

            pub fn as_str(&self) -> &::core::primitive::str {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl ::core::convert::From<$name> for ::std::string::String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl ::core::convert::TryFrom<::std::string::String> for $name {
            type Error = $crate::IdError;

            fn try_from(
                value: ::std::string::String,
            ) -> ::core::result::Result<Self, Self::Error> {
                $name::parse(value)
            }
        }
    };
}

validated_string_id!(
    /// Identifies an immutable world release.
    WorldId,
    "world"
);
validated_string_id!(
    /// Identifies a typed decision query contract.
    QueryId,
    "query"
);
validated_string_id!(
    /// Identifies a local evidence section within a world.
    FactId,
    "fact"
);
validated_string_id!(
    /// Identifies a typed factor within a world.
    FactorId,
    "factor"
);
validated_string_id!(
    /// Identifies a causal event in the world's event structure.
    EventId,
    "event"
);
validated_string_id!(
    /// Names a variable produced by a fact or a factor.
    VariableName,
    "variable"
);
validated_string_id!(
    /// Identifies one execution of the compiler or runtime.
    RunId,
    "run"
);
