//! The field-level difference between two versions of a wire format.
//!
//! Blueprint 40.37's execution path opens with "classify compatibility" and its inputs are
//! "old/new schema versions". It does not say who classifies. This crate's answer is that the
//! *diff* does, and the author does not get a vote: [`diff`] is a pure function of two
//! [`SchemaDescriptor`]s, and everything downstream — the compatibility class, the required
//! version bump, whether an artifact's digest moves — is derived from the change list rather than
//! read off a label in a changelog.
//!
//! That is the whole reason this module exists separately from [`crate::classify`]. A tool that
//! accepted "this release is minor" as an input could only ever check spelling. A tool that
//! computes the change list can contradict the author.
//!
//! Not implemented: structural diffing of an opaque object's interior. If a descriptor declares
//! `plan` as [`crate::descriptor::FieldType::Object`], a change inside `plan` is invisible here —
//! by construction, since the descriptor declined to promise anything about it. Promote the field
//! to a [`crate::descriptor::FieldType::Group`] to make its members diffable.

use crate::descriptor::{DigestRole, FieldSpec, FieldType, Presence, SchemaDescriptor};
use crate::error::CompatibilityError;
use crate::mode::CompatibilityMode;
use crate::version::SchemaId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// One difference between two descriptors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum FieldChange {
    Added(FieldSpec),
    Removed(FieldSpec),
    TypeChanged {
        path: String,
        from: FieldType,
        to: FieldType,
        digest: DigestRole,
    },
    /// Covers optional/required moves and default changes, which are the same question — what a
    /// reader does when the key is absent — asked two ways.
    PresenceChanged {
        path: String,
        from: Presence,
        to: Presence,
        digest: DigestRole,
    },
    /// A field moved into or out of the hashed bytes. Never harmless.
    DigestRoleChanged {
        path: String,
        from: DigestRole,
        to: DigestRole,
    },
}

impl FieldChange {
    pub fn path(&self) -> &str {
        match self {
            FieldChange::Added(spec) | FieldChange::Removed(spec) => &spec.path,
            FieldChange::TypeChanged { path, .. }
            | FieldChange::PresenceChanged { path, .. }
            | FieldChange::DigestRoleChanged { path, .. } => path,
        }
    }

    /// The digest role that decides whether this change touches hashed bytes. For
    /// [`FieldChange::DigestRoleChanged`] the answer is whichever side is hashed, because moving
    /// in and moving out both change the byte string.
    pub fn digest_role(&self) -> DigestRole {
        match self {
            FieldChange::Added(spec) | FieldChange::Removed(spec) => spec.digest,
            FieldChange::TypeChanged { digest, .. }
            | FieldChange::PresenceChanged { digest, .. } => *digest,
            FieldChange::DigestRoleChanged { from, to, .. } => {
                if from.is_hashed() || to.is_hashed() {
                    DigestRole::Hashed
                } else {
                    DigestRole::Excluded
                }
            }
        }
    }
}

impl fmt::Display for FieldChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldChange::Added(spec) => write!(
                f,
                "added {} ({}, {}, {})",
                spec.path, spec.ty, spec.presence, spec.digest
            ),
            FieldChange::Removed(spec) => write!(
                f,
                "removed {} ({}, {}, {})",
                spec.path, spec.ty, spec.presence, spec.digest
            ),
            FieldChange::TypeChanged { path, from, to, .. } => {
                write!(f, "{path} retyped {from} -> {to}")
            }
            FieldChange::PresenceChanged { path, from, to, .. } => {
                write!(f, "{path} presence {from} -> {to}")
            }
            FieldChange::DigestRoleChanged { path, from, to } => {
                write!(f, "{path} digest role {from} -> {to}")
            }
        }
    }
}

/// Every difference between two descriptors, plus the declared-mode move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDiff {
    pub from: SchemaId,
    pub to: SchemaId,
    pub from_mode: CompatibilityMode,
    pub to_mode: CompatibilityMode,
    pub changes: Vec<FieldChange>,
}

impl SchemaDiff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && !self.mode_changed()
    }

    pub fn mode_changed(&self) -> bool {
        self.from_mode != self.to_mode
    }

    pub fn change_at(&self, path: &str) -> Option<&FieldChange> {
        self.changes.iter().find(|change| change.path() == path)
    }
}

/// The path at which both descriptors carry their own version string, if it is the same path.
///
/// A format whose version marker is inside its hashed bytes — which is every format here — retypes
/// that one field at every single release, by definition. Reporting it would make every diff
/// contain a breaking retype for a reason that carries no information about the change, and would
/// bury the retypes that do. The marker is recognised structurally (a
/// [`FieldType::Const`] of the descriptor's own id), so a field that stops being a version marker,
/// or moves, is still reported.
fn version_marker_path(from: &SchemaDescriptor, to: &SchemaDescriptor) -> Option<String> {
    let old = from.version_marker()?;
    let new = to.version_marker()?;
    (old.path == new.path).then(|| old.path.clone())
}

/// Computes the field-level difference between two versions of one wire format.
///
/// Refuses two unrelated cases rather than producing a meaningless change list: descriptors with
/// different names are different namespaces (14.16 requires a breaking change to *take* a new
/// namespace, so a cross-namespace diff would be classifying two unrelated formats), and two
/// variant labels at the same release are siblings with no direction between them.
pub fn diff(
    from: &SchemaDescriptor,
    to: &SchemaDescriptor,
) -> Result<SchemaDiff, CompatibilityError> {
    if from.id.name != to.id.name {
        return Err(CompatibilityError::DifferentSchemas {
            from: from.id.to_string(),
            to: to.id.to_string(),
        });
    }
    if to.id.sibling_variant_of(&from.id) {
        return Err(CompatibilityError::NotASuccessor {
            from: from.id.to_string(),
            to: to.id.to_string(),
        });
    }

    let mut changes = Vec::new();
    let version_marker = version_marker_path(from, to);

    for old in from.fields() {
        match to.field(&old.path) {
            None => changes.push(FieldChange::Removed(old.clone())),
            Some(new) => {
                if old.digest != new.digest {
                    changes.push(FieldChange::DigestRoleChanged {
                        path: old.path.clone(),
                        from: old.digest,
                        to: new.digest,
                    });
                }
                if old.ty != new.ty && version_marker.as_deref() != Some(old.path.as_str()) {
                    changes.push(FieldChange::TypeChanged {
                        path: old.path.clone(),
                        from: old.ty.clone(),
                        to: new.ty.clone(),
                        digest: new.digest,
                    });
                }
                if old.presence != new.presence {
                    changes.push(FieldChange::PresenceChanged {
                        path: old.path.clone(),
                        from: old.presence.clone(),
                        to: new.presence.clone(),
                        digest: new.digest,
                    });
                }
            }
        }
    }

    for new in to.fields() {
        if from.field(&new.path).is_none() {
            changes.push(FieldChange::Added(new.clone()));
        }
    }

    Ok(SchemaDiff {
        from: from.id.clone(),
        to: to.id.clone(),
        from_mode: from.mode,
        to_mode: to.mode,
        changes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(version: &str, fields: Vec<FieldSpec>) -> SchemaDescriptor {
        SchemaDescriptor::new(
            SchemaId::parse(&format!("test-format/{version}")).expect("parses"),
            CompatibilityMode::PreserveAndForward,
            fields,
        )
        .expect("descriptor is well formed")
    }

    fn base() -> SchemaDescriptor {
        schema(
            "1.0",
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::required("count", FieldType::Integer),
            ],
        )
    }

    #[test]
    fn diffing_a_descriptor_against_itself_finds_nothing() {
        let computed = diff(&base(), &base()).expect("same name, same release");
        assert!(computed.is_empty());
    }

    #[test]
    fn two_different_schema_names_are_not_a_version_lineage() {
        let other = SchemaDescriptor::new(
            SchemaId::parse("other-format/1.0").expect("parses"),
            CompatibilityMode::PreserveAndForward,
            vec![],
        )
        .expect("well formed");
        assert!(matches!(
            diff(&base(), &other),
            Err(CompatibilityError::DifferentSchemas { .. })
        ));
    }

    #[test]
    fn a_sibling_variant_at_the_same_release_has_no_diff_direction() {
        let variant = schema(
            "1.0-extended",
            vec![FieldSpec::required("world_id", FieldType::String)],
        );
        assert!(matches!(
            diff(&base(), &variant),
            Err(CompatibilityError::NotASuccessor { .. })
        ));
    }

    #[test]
    fn a_field_that_changes_type_and_presence_at_once_yields_both_changes() {
        let next = schema(
            "1.1",
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::optional("count", FieldType::String),
            ],
        );
        let computed = diff(&base(), &next).expect("lineage");
        assert_eq!(computed.changes.len(), 2);
        assert!(computed
            .changes
            .iter()
            .any(|change| matches!(change, FieldChange::TypeChanged { .. })));
        assert!(computed
            .changes
            .iter()
            .any(|change| matches!(change, FieldChange::PresenceChanged { .. })));
    }

    #[test]
    fn a_changed_default_is_a_presence_change_rather_than_no_change_at_all() {
        let from = schema(
            "1.0",
            vec![FieldSpec::optional("note", FieldType::String)
                .with_presence(Presence::optional_with_default(json!("")))],
        );
        let to = schema(
            "1.1",
            vec![FieldSpec::optional("note", FieldType::String)
                .with_presence(Presence::optional_with_default(json!("unknown")))],
        );
        let computed = diff(&from, &to).expect("lineage");
        assert!(matches!(
            computed.change_at("note"),
            Some(FieldChange::PresenceChanged { .. })
        ));
    }

    #[test]
    fn a_declared_mode_change_is_visible_on_the_diff_even_with_no_field_changes() {
        let mut strict = base();
        strict.mode = CompatibilityMode::Reject;
        strict.id = SchemaId::parse("test-format/1.1").expect("parses");
        let computed = diff(&base(), &strict).expect("lineage");
        assert!(computed.changes.is_empty());
        assert!(computed.mode_changed());
        assert!(!computed.is_empty());
    }
}
