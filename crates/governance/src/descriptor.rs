//! What a wire format actually promises, field by field.
//!
//! Blueprint 25.22 requires a schema id, a version and a compatibility class per contract, and
//! 40.37 takes "old/new schema versions" as an input without saying what a schema version *is* as
//! a value. A version string alone cannot be diffed, so the diffable object is here: a
//! [`SchemaDescriptor`] is an ordered list of [`FieldSpec`], each carrying the three properties
//! that decide what a change means — its type, whether it must be present, and whether it is part
//! of the bytes that get hashed.
//!
//! # This is not a JSON Schema validator
//!
//! [`SchemaDescriptor::check_document`] answers "is this field present, is it roughly the right
//! shape, is anything here undeclared". It does not check patterns, ranges, enums or nested array
//! item types. A real validator belongs in the SDK; what schema *evolution* needs is exactly the
//! part that changes meaning between versions, and a regex tightening does not change which bytes
//! are hashed. Where the published JSON Schema in `schemas/fiber-v0.1/` is stricter, it wins at
//! ingest; this descriptor is the evolution contract, not the ingest gate.
//!
//! # Digest role is the load-bearing property
//!
//! [`DigestRole::Hashed`] marks a field whose presence and value are inside the canonical bytes
//! that a content hash covers. For the Context Certificate that is every body field: `verify`
//! clones the document, removes only `certificate_sha256`, and hashes the rest. That single
//! implementation detail is what makes almost any certificate field change a digest change, and it
//! is why [`DigestRole`] is a declared property rather than an inferred one.

use crate::error::DescriptorError;
use crate::mode::CompatibilityMode;
use crate::version::SchemaId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

/// The coarse shape of a field's value.
///
/// [`FieldType::Object`] is opaque: the descriptor declines to describe its interior, the way the
/// shipped certificate schema declares `plan` and `oracle` as bare objects.
/// [`FieldType::Group`] is the opposite — an object whose members are themselves declared paths.
/// The distinction exists so that "unknown field" has a definite meaning: undeclared keys inside
/// an opaque object are not unknown, because nothing was claimed about them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
    /// An object this descriptor deliberately does not describe.
    Object,
    /// An object whose members appear as dotted paths in this descriptor.
    Group,
    /// Any JSON value, including null. Used where the format genuinely accepts anything.
    Any,
    /// A fixed string, as `schema_version` is in every format here.
    Const(String),
}

impl FieldType {
    /// Whether a value is admissible under this type. Deliberately shallow; see the module docs.
    pub fn admits(&self, value: &Value) -> bool {
        match self {
            FieldType::String => value.is_string(),
            FieldType::Integer => value.is_i64() || value.is_u64(),
            FieldType::Number => value.is_number(),
            FieldType::Boolean => value.is_boolean(),
            FieldType::Array => value.is_array(),
            FieldType::Object | FieldType::Group => value.is_object(),
            FieldType::Any => true,
            FieldType::Const(expected) => value.as_str() == Some(expected.as_str()),
        }
    }

    pub fn is_opaque(&self) -> bool {
        matches!(self, FieldType::Object | FieldType::Any)
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldType::String => f.write_str("string"),
            FieldType::Integer => f.write_str("integer"),
            FieldType::Number => f.write_str("number"),
            FieldType::Boolean => f.write_str("boolean"),
            FieldType::Array => f.write_str("array"),
            FieldType::Object => f.write_str("object"),
            FieldType::Group => f.write_str("group"),
            FieldType::Any => f.write_str("any"),
            FieldType::Const(value) => write!(f, "const {value:?}"),
        }
    }
}

/// Whether a field must be present, and what a reader does when it is not.
///
/// The `default` is the difference between an addition that is free and one that is not. An
/// optional field with no default is simply absent from old documents, so their bytes are
/// unchanged. An optional field whose default is *materialised* into the document adds a key to
/// every old document the moment a new reader touches it, and if the field is hashed that moves
/// the digest. 25.22 does not draw this line; the digest guarantee forces it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "presence", rename_all = "snake_case")]
pub enum Presence {
    Required,
    Optional {
        /// A value a reader writes into the document when the key is absent. `None` means the key
        /// simply stays absent, which is the digest-safe choice.
        default: Option<Value>,
    },
}

impl Presence {
    pub fn optional() -> Self {
        Presence::Optional { default: None }
    }

    pub fn optional_with_default(value: Value) -> Self {
        Presence::Optional {
            default: Some(value),
        }
    }

    pub fn is_required(&self) -> bool {
        matches!(self, Presence::Required)
    }

    /// The default a reader would materialise, if any.
    pub fn materialised_default(&self) -> Option<&Value> {
        match self {
            Presence::Required => None,
            Presence::Optional { default } => default.as_ref(),
        }
    }
}

impl fmt::Display for Presence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Presence::Required => f.write_str("required"),
            Presence::Optional { default: None } => f.write_str("optional"),
            Presence::Optional {
                default: Some(value),
            } => write!(f, "optional (default {value})"),
        }
    }
}

/// Whether a field's bytes are covered by the artifact's content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DigestRole {
    /// Inside the canonical bytes. Changing this field changes the artifact's identity.
    Hashed,
    /// Outside them — the digest field itself, or a transport envelope.
    Excluded,
}

impl DigestRole {
    pub fn is_hashed(self) -> bool {
        matches!(self, DigestRole::Hashed)
    }
}

impl fmt::Display for DigestRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DigestRole::Hashed => f.write_str("hashed"),
            DigestRole::Excluded => f.write_str("not hashed"),
        }
    }
}

/// One declared field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSpec {
    /// A dotted path. `plan.backend` names `backend` inside a [`FieldType::Group`] called `plan`.
    pub path: String,
    pub ty: FieldType,
    pub presence: Presence,
    pub digest: DigestRole,
}

impl FieldSpec {
    /// A required, hashed field — the default posture, because that is what every field of the
    /// three shipped formats actually is.
    pub fn required(path: impl Into<String>, ty: FieldType) -> Self {
        FieldSpec {
            path: path.into(),
            ty,
            presence: Presence::Required,
            digest: DigestRole::Hashed,
        }
    }

    pub fn optional(path: impl Into<String>, ty: FieldType) -> Self {
        FieldSpec {
            path: path.into(),
            ty,
            presence: Presence::optional(),
            digest: DigestRole::Hashed,
        }
    }

    pub fn with_presence(mut self, presence: Presence) -> Self {
        self.presence = presence;
        self
    }

    pub fn excluded_from_digest(mut self) -> Self {
        self.digest = DigestRole::Excluded;
        self
    }

    /// The path of the object this field lives in, if it is nested.
    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('.').map(|(parent, _)| parent)
    }
}

/// A version of a wire format, as a diffable object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDescriptor {
    pub id: SchemaId,
    /// What a reader of *this* version does with a field it does not know. 43.35 requires that
    /// this be declared rather than assumed.
    pub mode: CompatibilityMode,
    fields: Vec<FieldSpec>,
}

impl SchemaDescriptor {
    pub fn new(
        id: SchemaId,
        mode: CompatibilityMode,
        fields: Vec<FieldSpec>,
    ) -> Result<Self, DescriptorError> {
        for (index, field) in fields.iter().enumerate() {
            if field.path.is_empty() {
                return Err(DescriptorError::EmptyPath);
            }
            if field.path.split('.').any(str::is_empty) {
                return Err(DescriptorError::EmptySegment(field.path.clone()));
            }
            if fields[..index].iter().any(|prior| prior.path == field.path) {
                return Err(DescriptorError::DuplicateField(field.path.clone()));
            }
            if let Some(parent) = field.parent_path() {
                let declared = fields.iter().find(|other| other.path == parent);
                match declared {
                    Some(spec) if spec.ty == FieldType::Group => {}
                    _ => {
                        return Err(DescriptorError::DescendsIntoOpaqueField {
                            path: field.path.clone(),
                            parent: parent.to_string(),
                        })
                    }
                }
            }
        }
        Ok(SchemaDescriptor { id, mode, fields })
    }

    pub fn fields(&self) -> &[FieldSpec] {
        &self.fields
    }

    pub fn field(&self, path: &str) -> Option<&FieldSpec> {
        self.fields.iter().find(|field| field.path == path)
    }

    pub fn paths(&self) -> Vec<&str> {
        self.fields.iter().map(|field| field.path.as_str()).collect()
    }

    pub fn hashed_paths(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|field| field.digest.is_hashed())
            .map(|field| field.path.as_str())
            .collect()
    }

    /// The field that carries this format's own version string, if it has one.
    ///
    /// Recognised structurally rather than by name: it is the field declared as
    /// [`FieldType::Const`] of this descriptor's own id. All three formats in this workspace put it
    /// at `schema_version`, inside the hashed bytes, which is why [`crate::diff::diff`] has to
    /// treat it specially — see the note there.
    pub fn version_marker(&self) -> Option<&FieldSpec> {
        let expected = FieldType::Const(self.id.to_string());
        self.fields.iter().find(|field| field.ty == expected)
    }

    pub fn top_level_paths(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|field| !field.path.contains('.'))
            .map(|field| field.path.as_str())
            .collect()
    }

    /// Every path a document actually carries, using this descriptor's opacity decisions.
    ///
    /// Descent stops at any declared field that is not a [`FieldType::Group`]: the interior of an
    /// opaque object is the format's business, not this descriptor's.
    pub fn observed_paths(&self, document: &Value) -> Vec<String> {
        let mut found = Vec::new();
        if let Some(map) = document.as_object() {
            self.walk(map, "", &mut found);
        }
        found
    }

    fn walk(&self, map: &Map<String, Value>, prefix: &str, found: &mut Vec<String>) {
        for (key, value) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            let descend = matches!(
                self.field(&path).map(|field| &field.ty),
                Some(FieldType::Group)
            );
            found.push(path.clone());
            if descend {
                if let Some(nested) = value.as_object() {
                    self.walk(nested, &path, found);
                }
            }
        }
    }

    /// Compares a document against this descriptor.
    ///
    /// Unknown fields are *reported*, never judged here: what happens to them is
    /// [`CompatibilityMode`]'s decision, and separating the two is the whole point of 43.35's
    /// declared-mode rule.
    pub fn check_document(&self, document: &Value) -> DocumentCheck {
        let mut check = DocumentCheck::default();
        if !document.is_object() {
            check.not_an_object = true;
            return check;
        }

        for field in &self.fields {
            match crate::pointer::get(document, &field.path) {
                Some(value) => {
                    if !field.ty.admits(value) {
                        check.mistyped.push(Mistyped {
                            path: field.path.clone(),
                            expected: field.ty.clone(),
                            found: describe(value),
                        });
                    }
                }
                None => {
                    if field.presence.is_required() {
                        check.missing.push(field.path.clone());
                    }
                }
            }
        }

        let declared: Vec<&str> = self.paths();
        for path in self.observed_paths(document) {
            if !declared.contains(&path.as_str()) {
                check.unknown.push(path);
            }
        }

        check
    }
}

fn describe(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(_) => "boolean".into(),
        Value::Number(_) => "number".into(),
        Value::String(_) => "string".into(),
        Value::Array(_) => "array".into(),
        Value::Object(_) => "object".into(),
    }
}

/// A field that is present but the wrong shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mistyped {
    pub path: String,
    pub expected: FieldType,
    pub found: String,
}

impl fmt::Display for Mistyped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: expected {}, found {}",
            self.path, self.expected, self.found
        )
    }
}

/// What a descriptor found in a document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentCheck {
    pub not_an_object: bool,
    pub missing: Vec<String>,
    pub mistyped: Vec<Mistyped>,
    pub unknown: Vec<String>,
}

impl DocumentCheck {
    /// Whether the document satisfies the declared contract, setting unknown fields aside.
    ///
    /// Unknown fields are excluded on purpose: under preserve-and-forward they are expected, and
    /// folding them in here would make every additive change look like a validation failure.
    pub fn conforms(&self) -> bool {
        !self.not_an_object && self.missing.is_empty() && self.mistyped.is_empty()
    }

    pub fn is_clean(&self) -> bool {
        self.conforms() && self.unknown.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn id() -> SchemaId {
        SchemaId::parse("test-format/1.0").expect("parses")
    }

    fn descriptor(fields: Vec<FieldSpec>) -> SchemaDescriptor {
        SchemaDescriptor::new(id(), CompatibilityMode::PreserveAndForward, fields)
            .expect("descriptor is well formed")
    }

    #[test]
    fn a_nested_path_under_an_opaque_object_is_rejected_at_construction() {
        let error = SchemaDescriptor::new(
            id(),
            CompatibilityMode::PreserveAndForward,
            vec![
                FieldSpec::required("plan", FieldType::Object),
                FieldSpec::required("plan.backend", FieldType::String),
            ],
        )
        .expect_err("declaring the interior of an object declared opaque is a contradiction");
        assert!(matches!(
            error,
            DescriptorError::DescendsIntoOpaqueField { .. }
        ));
    }

    #[test]
    fn a_duplicate_field_declaration_is_rejected_at_construction() {
        let error = SchemaDescriptor::new(
            id(),
            CompatibilityMode::Reject,
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::optional("world_id", FieldType::String),
            ],
        )
        .expect_err("two declarations of one path have no defined meaning");
        assert!(matches!(error, DescriptorError::DuplicateField(path) if path == "world_id"));
    }

    #[test]
    fn the_interior_of_an_opaque_object_is_not_an_unknown_field() {
        let schema = descriptor(vec![FieldSpec::required("plan", FieldType::Object)]);
        let check = schema.check_document(&json!({"plan": {"backend": "eager", "extra": 1}}));
        assert!(
            check.is_clean(),
            "nothing was claimed about the interior of plan, so nothing there can be a surprise: \
             {check:?}"
        );
    }

    #[test]
    fn the_interior_of_a_group_is_walked_and_undeclared_members_are_unknown() {
        let schema = descriptor(vec![
            FieldSpec::required("plan", FieldType::Group),
            FieldSpec::required("plan.backend", FieldType::String),
        ]);
        let check = schema.check_document(&json!({"plan": {"backend": "eager", "extra": 1}}));
        assert_eq!(check.unknown, ["plan.extra"]);
        assert!(check.conforms());
    }

    #[test]
    fn a_missing_required_field_is_reported_and_a_missing_optional_one_is_not() {
        let schema = descriptor(vec![
            FieldSpec::required("world_id", FieldType::String),
            FieldSpec::optional("note", FieldType::String),
        ]);
        let check = schema.check_document(&json!({}));
        assert_eq!(check.missing, ["world_id"]);
        assert!(check.mistyped.is_empty());
    }

    #[test]
    fn a_const_field_only_admits_its_own_value() {
        let ty = FieldType::Const("fiber-context-certificate/0.1".into());
        assert!(ty.admits(&json!("fiber-context-certificate/0.1")));
        assert!(!ty.admits(&json!("fiber-context-certificate/0.2-extended")));
        assert!(!ty.admits(&json!(1)));
    }
}
