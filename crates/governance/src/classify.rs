//! Compatibility class, derived from the diff and not from the author.
//!
//! Blueprint 25.22 requires a "compatibility class" field and 14.16 promises that "breaking
//! changes use new namespace/version and migration path". Neither document defines the classes it
//! names. The definitions below are this crate's, stated plainly because the blueprint's silence
//! is the single largest gap in this area:
//!
//! | Class | Meaning |
//! |---|---|
//! | [`CompatibilityClass::Compatible`] | Every existing document is readable under the new version, every new document is readable by a reader of the old version, and no artifact's digest moves. |
//! | [`CompatibilityClass::ForwardCompatible`] | Existing documents stay readable under the new version. Documents written under the new version may not be readable by an old reader. |
//! | [`CompatibilityClass::Breaking`] | Existing documents stop being readable, or an existing artifact's digest moves. |
//!
//! Note the direction. Avro and the schema-registry literature use "forward compatible" for the
//! opposite arrow — old code reading new data. The word is used here for the direction that
//! matters to an archive: whether the format can move forward over its own history. Anyone
//! importing this vocabulary should read the table, not the name.
//!
//! # The rule that outranks everything else
//!
//! [`affects_digest`] decides, per change, whether the canonical bytes of an artifact that already
//! exists would come out different. Any change for which it is true is [`CompatibilityClass::Breaking`],
//! regardless of how benign the change looks field by field, and [`Classification::assert_class`]
//! refuses an author's claim of compatibility with a [`DigestBreach`] rather than a warning. Three
//! implementations — CPython, the eager Rust path, the indexed store — currently agree on the
//! reference certificate digest. A schema change is the cheapest way to end that agreement without
//! anybody noticing, so it is the one thing here that is not a judgement call.
//!
//! # Why the declared mode is an input
//!
//! The same field addition is compatible under [`crate::mode::CompatibilityMode::PreserveAndForward`]
//! and merely forward-compatible under [`crate::mode::CompatibilityMode::Reject`], because under
//! `Reject` every deployed reader refuses the new document on sight. Classification therefore
//! reads the modes off the two descriptors. This is 43.35's declared-mode clause having a
//! consequence rather than being a comment.

use crate::diff::{FieldChange, SchemaDiff};
use crate::error::{CompatibilityError, DigestBreach, GovernanceError};
use crate::mode::CompatibilityMode;
use crate::version::{observed_bump, SchemaId, VersionBump};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The three classes, ordered by severity so that a diff's class is the maximum over its changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityClass {
    Compatible,
    ForwardCompatible,
    Breaking,
}

impl CompatibilityClass {
    pub fn as_str(self) -> &'static str {
        match self {
            CompatibilityClass::Compatible => "compatible",
            CompatibilityClass::ForwardCompatible => "forward-compatible",
            CompatibilityClass::Breaking => "breaking",
        }
    }

    /// The smallest version move a change of this class may ship under.
    ///
    /// A compatible change still requires a patch bump: 14.16's "every reported result is linked
    /// to an immutable ... version" only holds if the version moves whenever the bytes can.
    pub fn required_bump(self) -> VersionBump {
        match self {
            CompatibilityClass::Compatible => VersionBump::Patch,
            CompatibilityClass::ForwardCompatible => VersionBump::Minor,
            CompatibilityClass::Breaking => VersionBump::Major,
        }
    }
}

impl fmt::Display for CompatibilityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a change moves the canonical bytes of a document that already exists.
///
/// The asymmetries here are the substance:
///
/// - Adding an **optional hashed field with no default** is digest-safe. Old documents simply do
///   not carry the key, their bytes are untouched, and their digests still verify. This is the
///   only escape hatch a hashed format has, and it is why [`crate::descriptor::Presence`] records
///   the default separately from the optionality.
/// - Adding an optional hashed field **with a materialised default** is not safe: a reader writes
///   the key into every old document it touches.
/// - **Removing** a hashed field is never safe, because existing documents carry it.
/// - **Tightening** optional to required does not move any digest. It invalidates old documents,
///   which is breaking for a different reason — that is [`classify`]'s job, not this function's.
pub fn affects_digest(change: &FieldChange) -> bool {
    match change {
        FieldChange::Added(spec) => {
            spec.digest.is_hashed()
                && (spec.presence.is_required() || spec.presence.materialised_default().is_some())
        }
        FieldChange::Removed(spec) => spec.digest.is_hashed(),
        FieldChange::TypeChanged { digest, .. } => digest.is_hashed(),
        FieldChange::PresenceChanged {
            from, to, digest, ..
        } => digest.is_hashed() && from.materialised_default() != to.materialised_default(),
        FieldChange::DigestRoleChanged { .. } => true,
    }
}

/// One change, classified, with the reason in words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeVerdict {
    pub change: FieldChange,
    pub class: CompatibilityClass,
    pub affects_digest: bool,
    pub rationale: String,
}

impl ChangeVerdict {
    /// The typed breach for a digest-moving change, for a caller that wants to fail rather than
    /// report.
    pub fn breach(&self, schema: &SchemaId) -> Option<DigestBreach> {
        if !self.affects_digest {
            return None;
        }
        Some(DigestBreach {
            schema: schema.name.clone(),
            path: self.change.path().to_string(),
            role: self.change.digest_role(),
            detail: self.rationale.clone(),
        })
    }
}

/// The classification of a whole diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    pub from: SchemaId,
    pub to: SchemaId,
    pub class: CompatibilityClass,
    pub required_bump: VersionBump,
    pub observed_bump: VersionBump,
    pub verdicts: Vec<ChangeVerdict>,
    /// A declared-mode move, which is breaking on its own terms. See 43.35.
    pub mode_change: Option<(CompatibilityMode, CompatibilityMode)>,
}

impl Classification {
    pub fn is_compatible(&self) -> bool {
        self.class == CompatibilityClass::Compatible
    }

    pub fn digest_affecting(&self) -> Vec<&ChangeVerdict> {
        self.verdicts
            .iter()
            .filter(|verdict| verdict.affects_digest)
            .collect()
    }

    pub fn breaches(&self) -> Vec<DigestBreach> {
        self.verdicts
            .iter()
            .filter_map(|verdict| verdict.breach(&self.to))
            .collect()
    }

    /// Whether the version moved far enough for what actually changed.
    ///
    /// This is the mechanical half of 14.16's "task/oracle/scoring change never masquerades as a
    /// patch": the class is derived from the diff, the required bump falls out of the class, and
    /// the observed bump is read off the two version strings. Nobody is asked what they intended.
    pub fn version_gate(&self) -> Result<(), CompatibilityError> {
        if self.observed_bump >= self.required_bump {
            Ok(())
        } else {
            Err(CompatibilityError::VersionBumpTooSmall {
                name: self.to.name.clone(),
                from: self.from.to_string(),
                to: self.to.to_string(),
                class: self.class.to_string(),
                required: self.required_bump,
                observed: self.observed_bump,
            })
        }
    }

    /// Checks an author's stated compatibility class against the derived one.
    ///
    /// A claim of [`CompatibilityClass::Compatible`] over a digest-moving change fails as a
    /// [`DigestBreach`] rather than as a mere contradiction, because that specific mistake ends
    /// cross-language parity and a reviewer skimming a list of contradictions would not see it.
    pub fn assert_class(&self, claimed: CompatibilityClass) -> Result<(), GovernanceError> {
        if claimed == CompatibilityClass::Compatible {
            if let Some(breach) = self.breaches().into_iter().next() {
                return Err(GovernanceError::Digest(breach));
            }
        }
        if claimed >= self.class {
            Ok(())
        } else {
            Err(GovernanceError::Compatibility(
                CompatibilityError::VersionBumpTooSmall {
                    name: self.to.name.clone(),
                    from: self.from.to_string(),
                    to: self.to.to_string(),
                    class: format!("{} (author claimed {claimed})", self.class),
                    required: self.class.required_bump(),
                    observed: claimed.required_bump(),
                },
            ))
        }
    }

    /// The author's claim, contradicted where the evidence disagrees.
    pub fn audit_claim(&self, claimed: CompatibilityClass) -> ClaimAudit {
        if claimed >= self.class {
            ClaimAudit::Upheld { derived: self.class }
        } else {
            ClaimAudit::Contradicted {
                claimed,
                derived: self.class,
                evidence: self
                    .verdicts
                    .iter()
                    .filter(|verdict| verdict.class > claimed)
                    .map(|verdict| format!("{}: {}", verdict.change, verdict.rationale))
                    .collect(),
            }
        }
    }
}

/// The outcome of checking a stated class against the derived one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "audit", rename_all = "snake_case")]
pub enum ClaimAudit {
    Upheld {
        derived: CompatibilityClass,
    },
    Contradicted {
        claimed: CompatibilityClass,
        derived: CompatibilityClass,
        /// The changes that outrank the claim, each with why.
        evidence: Vec<String>,
    },
}

impl ClaimAudit {
    pub fn is_upheld(&self) -> bool {
        matches!(self, ClaimAudit::Upheld { .. })
    }
}

/// Classifies one change under the two declared modes.
fn classify_change(
    change: &FieldChange,
    from_mode: CompatibilityMode,
    to_mode: CompatibilityMode,
) -> ChangeVerdict {
    let digest = affects_digest(change);
    let (class, rationale) = if digest {
        (
            CompatibilityClass::Breaking,
            digest_rationale(change).to_string(),
        )
    } else {
        match change {
            FieldChange::Added(spec) if spec.presence.is_required() => (
                CompatibilityClass::Breaking,
                "a required field is absent from every document written before it existed".into(),
            ),
            FieldChange::Added(_) => {
                if from_mode.tolerates_unknown() {
                    (
                        CompatibilityClass::Compatible,
                        format!(
                            "an optional field with no default, and readers of the old version \
                             declare {from_mode}"
                        ),
                    )
                } else {
                    (
                        CompatibilityClass::ForwardCompatible,
                        format!(
                            "readers of the old version declare {from_mode}, so they refuse any \
                             document carrying this field"
                        ),
                    )
                }
            }
            FieldChange::Removed(spec) => {
                if !to_mode.tolerates_unknown() {
                    (
                        CompatibilityClass::Breaking,
                        format!(
                            "readers of the new version declare {to_mode}, so every existing \
                             document carrying this field is refused"
                        ),
                    )
                } else if spec.presence.is_required() {
                    (
                        CompatibilityClass::ForwardCompatible,
                        "old readers require this field and new documents will not carry it".into(),
                    )
                } else {
                    (
                        CompatibilityClass::Compatible,
                        format!(
                            "an optional field outside the hashed bytes, and readers of the new \
                             version declare {to_mode}"
                        ),
                    )
                }
            }
            FieldChange::TypeChanged { .. } => (
                CompatibilityClass::Breaking,
                "a value valid under the old type is not valid under the new one".into(),
            ),
            FieldChange::PresenceChanged { from, to, .. } => {
                if !from.is_required() && to.is_required() {
                    (
                        CompatibilityClass::Breaking,
                        "documents that legitimately omitted this field become invalid".into(),
                    )
                } else if from.is_required() && !to.is_required() {
                    (
                        CompatibilityClass::ForwardCompatible,
                        "old readers require a field new documents may omit".into(),
                    )
                } else {
                    (
                        CompatibilityClass::ForwardCompatible,
                        "the meaning of an absent key changed, which changes results without \
                         changing bytes"
                            .into(),
                    )
                }
            }
            FieldChange::DigestRoleChanged { .. } => (
                CompatibilityClass::Breaking,
                "a field moved across the boundary of the hashed bytes".into(),
            ),
        }
    };

    ChangeVerdict {
        change: change.clone(),
        class,
        affects_digest: digest,
        rationale,
    }
}

fn digest_rationale(change: &FieldChange) -> &'static str {
    match change {
        FieldChange::Added(_) => {
            "a reader materialises this key into documents that did not carry it, so their \
             canonical bytes change"
        }
        FieldChange::Removed(_) => {
            "existing documents carry this key inside the hashed bytes, so dropping it changes \
             their digest"
        }
        FieldChange::TypeChanged { .. } => {
            "the same logical value serialises to different bytes under the new type"
        }
        FieldChange::PresenceChanged { .. } => {
            "the value a reader materialises for an absent key changed, and that value is hashed"
        }
        FieldChange::DigestRoleChanged { .. } => {
            "the set of hashed keys changed, so every existing digest was computed over a \
             different byte string"
        }
    }
}

/// Classifies a diff.
///
/// The class is the maximum severity over the individual changes, plus [`CompatibilityClass::Breaking`]
/// if the declared mode moved. An empty diff with no mode move classifies as
/// [`CompatibilityClass::Compatible`] and requires no bump.
pub fn classify(diff: &SchemaDiff) -> Classification {
    let verdicts: Vec<ChangeVerdict> = diff
        .changes
        .iter()
        .map(|change| classify_change(change, diff.from_mode, diff.to_mode))
        .collect();

    let mode_change = if diff.mode_changed() {
        Some((diff.from_mode, diff.to_mode))
    } else {
        None
    };

    let mut class = verdicts
        .iter()
        .map(|verdict| verdict.class)
        .max()
        .unwrap_or(CompatibilityClass::Compatible);
    if mode_change.is_some() {
        class = CompatibilityClass::Breaking;
    }

    let required_bump = if diff.is_empty() {
        VersionBump::None
    } else {
        class.required_bump()
    };

    Classification {
        from: diff.from.clone(),
        to: diff.to.clone(),
        class,
        required_bump,
        observed_bump: observed_bump(&diff.from.version, &diff.to.version),
        verdicts,
        mode_change,
    }
}

/// Convenience for the common gate: classify, then refuse an under-sized version bump.
pub fn classify_and_gate(diff: &SchemaDiff) -> Result<Classification, CompatibilityError> {
    let classification = classify(diff);
    classification.version_gate()?;
    Ok(classification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{FieldSpec, FieldType, Presence, SchemaDescriptor};
    use crate::diff::diff;
    use serde_json::json;

    fn schema(
        version: &str,
        mode: CompatibilityMode,
        fields: Vec<FieldSpec>,
    ) -> SchemaDescriptor {
        SchemaDescriptor::new(
            SchemaId::parse(&format!("test-format/{version}")).expect("parses"),
            mode,
            fields,
        )
        .expect("descriptor is well formed")
    }

    fn base(mode: CompatibilityMode) -> SchemaDescriptor {
        schema(
            "1.0",
            mode,
            vec![FieldSpec::required("world_id", FieldType::String)],
        )
    }

    fn classify_between(from: &SchemaDescriptor, to: &SchemaDescriptor) -> Classification {
        classify(&diff(from, to).expect("lineage"))
    }

    #[test]
    fn a_change_that_would_alter_an_existing_digest_cannot_be_classified_compatible() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "2.0",
            CompatibilityMode::PreserveAndForward,
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::required("query_id", FieldType::String),
            ],
        );
        let classification = classify_between(&from, &to);

        assert_eq!(classification.class, CompatibilityClass::Breaking);
        assert_eq!(classification.digest_affecting().len(), 1);
        let error = classification
            .assert_class(CompatibilityClass::Compatible)
            .expect_err("a digest-moving change is not compatible at any version");
        assert!(matches!(error, GovernanceError::Digest(_)));
        assert!(error.to_string().contains("query_id"));
    }

    #[test]
    fn an_optional_hashed_field_with_no_default_is_the_one_digest_safe_addition() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "1.0.1",
            CompatibilityMode::PreserveAndForward,
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::optional("note", FieldType::String),
            ],
        );
        let classification = classify_between(&from, &to);
        assert_eq!(classification.class, CompatibilityClass::Compatible);
        assert!(classification.digest_affecting().is_empty());
        assert!(classification
            .assert_class(CompatibilityClass::Compatible)
            .is_ok());
    }

    #[test]
    fn the_same_addition_becomes_breaking_the_moment_it_carries_a_materialised_default() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "2.0",
            CompatibilityMode::PreserveAndForward,
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::optional("note", FieldType::String)
                    .with_presence(Presence::optional_with_default(json!(""))),
            ],
        );
        let classification = classify_between(&from, &to);
        assert_eq!(classification.class, CompatibilityClass::Breaking);
        assert!(affects_digest(&classification.verdicts[0].change));
    }

    #[test]
    fn the_same_addition_is_only_forward_compatible_when_old_readers_declare_reject() {
        let from = base(CompatibilityMode::Reject);
        let to = schema(
            "1.1",
            CompatibilityMode::Reject,
            vec![
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::optional("note", FieldType::String),
            ],
        );
        let classification = classify_between(&from, &to);
        assert_eq!(classification.class, CompatibilityClass::ForwardCompatible);
        assert_eq!(classification.required_bump, VersionBump::Minor);
    }

    #[test]
    fn an_author_who_labels_a_breaking_change_minor_is_contradicted_with_evidence() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "2.0",
            CompatibilityMode::PreserveAndForward,
            vec![FieldSpec::required("world_id", FieldType::Integer)],
        );
        let classification = classify_between(&from, &to);
        let audit = classification.audit_claim(CompatibilityClass::ForwardCompatible);
        match audit {
            ClaimAudit::Contradicted {
                claimed,
                derived,
                evidence,
            } => {
                assert_eq!(claimed, CompatibilityClass::ForwardCompatible);
                assert_eq!(derived, CompatibilityClass::Breaking);
                assert!(evidence.iter().any(|line| line.contains("world_id")));
            }
            ClaimAudit::Upheld { .. } => panic!("a retype is not forward compatible"),
        }
    }

    #[test]
    fn a_breaking_change_shipped_under_a_patch_bump_fails_the_version_gate() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "1.0.1",
            CompatibilityMode::PreserveAndForward,
            vec![FieldSpec::required("world_id", FieldType::Integer)],
        );
        let error = classify_and_gate(&diff(&from, &to).expect("lineage"))
            .expect_err("a retype cannot ship as a patch");
        assert!(matches!(
            error,
            CompatibilityError::VersionBumpTooSmall {
                required: VersionBump::Major,
                ..
            }
        ));
    }

    #[test]
    fn changing_the_declared_unknown_field_mode_is_breaking_on_its_own() {
        let from = base(CompatibilityMode::PreserveAndForward);
        let to = schema(
            "2.0",
            CompatibilityMode::Ignore,
            vec![FieldSpec::required("world_id", FieldType::String)],
        );
        let classification = classify_between(&from, &to);
        assert!(classification.verdicts.is_empty());
        assert_eq!(classification.class, CompatibilityClass::Breaking);
        assert_eq!(
            classification.mode_change,
            Some((
                CompatibilityMode::PreserveAndForward,
                CompatibilityMode::Ignore
            ))
        );
    }

    #[test]
    fn tightening_optional_to_required_is_breaking_without_moving_any_digest() {
        let from = schema(
            "1.0",
            CompatibilityMode::PreserveAndForward,
            vec![FieldSpec::optional("note", FieldType::String)],
        );
        let to = schema(
            "2.0",
            CompatibilityMode::PreserveAndForward,
            vec![FieldSpec::required("note", FieldType::String)],
        );
        let classification = classify_between(&from, &to);
        assert_eq!(classification.class, CompatibilityClass::Breaking);
        assert!(
            classification.digest_affecting().is_empty(),
            "documents that carried the field are byte-identical; the break is that documents \
             which omitted it are now invalid"
        );
    }

    #[test]
    fn an_empty_diff_requires_no_version_bump_at_all() {
        let classification = classify_between(&base(CompatibilityMode::Reject), &base(CompatibilityMode::Reject));
        assert_eq!(classification.class, CompatibilityClass::Compatible);
        assert_eq!(classification.required_bump, VersionBump::None);
        assert!(classification.version_gate().is_ok());
    }
}
