//! Standards-versioned archive migration adapter for preclinical glioma research objects.
//!
//! Archive migrations must preserve identifiers and provenance while making every semantic loss
//! and rollback boundary explicit.  This feature applies only caller-declared typed field rules;
//! it never guesses a missing value, rewrites artifact bytes, or silently drops an unknown
//! mandatory field.  A migration report is a product artifact that an archive steward can review,
//! replay, and roll back locally before a new research-object edition is admitted.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectMigration1@2";
pub const MAX_FIELDS: usize = 2_048;
pub const MAX_ARTIFACTS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveSchemaProfile {
    pub schema_id: String,
    pub version: String,
    pub required_field_order: Vec<String>,
    pub allowed_field_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveArtifactRef {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub required: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveObject {
    pub object_id: String,
    pub schema_id: String,
    pub schema_version: String,
    pub fields: BTreeMap<String, String>,
    pub artifacts: Vec<ArchiveArtifactRef>,
    pub provenance_digest: ContentHash,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveMigrationTransform {
    Identity,
    Rename,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMigrationRule {
    pub source_field: String,
    pub target_field: String,
    pub transform: ArchiveMigrationTransform,
    pub reversible: bool,
    pub semantic_loss_milli: u16,
    pub rule_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMigrationRequest {
    pub source: ArchiveObject,
    pub target: ArchiveSchemaProfile,
    pub rules: Vec<ArchiveMigrationRule>,
    pub max_semantic_loss_milli: u16,
    pub allow_optional_drop: bool,
    pub require_rollback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveMigrationFieldStatus {
    Preserved,
    Renamed,
    OmittedOptional,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMigrationFieldDecision {
    pub source_field: String,
    pub target_field: Option<String>,
    pub status: ArchiveMigrationFieldStatus,
    pub transform: Option<ArchiveMigrationTransform>,
    pub rule_version: Option<String>,
    pub semantic_loss_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigratedArchiveObject {
    pub object_id: String,
    pub schema_id: String,
    pub schema_version: String,
    pub fields: BTreeMap<String, String>,
    pub artifacts: Vec<ArchiveArtifactRef>,
    pub provenance_digest: ContentHash,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveMigrationDisposition {
    Ready,
    Lossy,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMigrationReport {
    pub feature_id: String,
    pub output_schema: String,
    pub source_schema_id: String,
    pub source_schema_version: String,
    pub target_schema_id: String,
    pub target_schema_version: String,
    pub decisions: Vec<ArchiveMigrationFieldDecision>,
    pub migrated: Option<MigratedArchiveObject>,
    pub rollback_field_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub limitations: Vec<String>,
    pub semantic_loss_milli: u16,
    pub disposition: ArchiveMigrationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArchiveMigrationError {
    #[error("archive migration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("archive migration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("archive migration digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn valid_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|major| !major.is_empty() && major.bytes().all(|byte| byte.is_ascii_digit()))
        && parts.all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn digest_input(report: &ArchiveMigrationReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "source_schema_id": report.source_schema_id,
        "source_schema_version": report.source_schema_version,
        "target_schema_id": report.target_schema_id,
        "target_schema_version": report.target_schema_version,
        "decisions": report.decisions,
        "migrated": report.migrated,
        "rollback_field_order": report.rollback_field_order,
        "omission_order": report.omission_order,
        "blocked_order": report.blocked_order,
        "limitations": report.limitations,
        "semantic_loss_milli": report.semantic_loss_milli,
        "disposition": report.disposition,
    })
}

fn validate_profile(profile: &ArchiveSchemaProfile) -> bool {
    valid_text(&profile.schema_id)
        && valid_version(&profile.version)
        && profile.required_field_order.len() <= MAX_FIELDS
        && profile.allowed_field_order.len() <= MAX_FIELDS
        && canonical(&profile.required_field_order)
        && canonical(&profile.allowed_field_order)
        && profile
            .required_field_order
            .iter()
            .all(|field| profile.allowed_field_order.binary_search(field).is_ok())
        && profile
            .required_field_order
            .iter()
            .chain(profile.allowed_field_order.iter())
            .all(|field| valid_text(field))
}

fn validate_object(object: &ArchiveObject) -> bool {
    valid_text(&object.object_id)
        && valid_text(&object.schema_id)
        && valid_version(&object.schema_version)
        && object.fields.len() <= MAX_FIELDS
        && object.fields.keys().all(|field| valid_text(field))
        && object.fields.values().all(|value| valid_text(value))
        && object.artifacts.len() <= MAX_ARTIFACTS
        && object
            .artifacts
            .windows(2)
            .all(|pair| pair[0].artifact_id < pair[1].artifact_id)
        && object.artifacts.iter().all(|artifact| {
            valid_text(&artifact.artifact_id) && artifact.content_hash.as_str().len() == 64
        })
        && object.provenance_digest.as_str().len() == 64
        && canonical(&object.uncertainty_order)
        && canonical(&object.negative_evidence_order)
}

fn validate_migrated_object(object: &MigratedArchiveObject) -> bool {
    valid_text(&object.object_id)
        && valid_text(&object.schema_id)
        && valid_version(&object.schema_version)
        && object.fields.len() <= MAX_FIELDS
        && object.fields.keys().all(|field| valid_text(field))
        && object.fields.values().all(|value| valid_text(value))
        && object.artifacts.len() <= MAX_ARTIFACTS
        && object
            .artifacts
            .windows(2)
            .all(|pair| pair[0].artifact_id < pair[1].artifact_id)
        && object.artifacts.iter().all(|artifact| {
            valid_text(&artifact.artifact_id) && artifact.content_hash.as_str().len() == 64
        })
        && object.provenance_digest.as_str().len() == 64
        && canonical(&object.uncertainty_order)
        && canonical(&object.negative_evidence_order)
}

fn validate_request(request: &ArchiveMigrationRequest) -> Result<(), ArchiveMigrationError> {
    if !validate_object(&request.source)
        || !validate_profile(&request.target)
        || request.rules.len() > MAX_FIELDS
        || request.max_semantic_loss_milli > 1_000
        || request.rules.iter().any(|rule| {
            !valid_text(&rule.source_field)
                || !valid_text(&rule.target_field)
                || !valid_version(&rule.rule_version)
                || rule.semantic_loss_milli > 1_000
                || (matches!(rule.transform, ArchiveMigrationTransform::Identity)
                    && rule.source_field != rule.target_field)
        })
    {
        return Err(ArchiveMigrationError::InvalidRequest(
            "source object, target profile, bounded rules, versions, fields, and transform identities are required".into(),
        ));
    }
    if request.source.schema_id.trim().is_empty() {
        return Err(ArchiveMigrationError::InvalidRequest(
            "source schema identity is required".into(),
        ));
    }
    let mut target_fields = BTreeSet::new();
    let mut source_fields = BTreeSet::new();
    for rule in &request.rules {
        if !target_fields.insert(rule.target_field.clone())
            || !source_fields.insert(rule.source_field.clone())
        {
            return Err(ArchiveMigrationError::InvalidRequest(
                "migration rules must map each source and target field at most once".into(),
            ));
        }
        if request
            .target
            .allowed_field_order
            .binary_search(&rule.target_field)
            .is_err()
        {
            return Err(ArchiveMigrationError::InvalidRequest(
                "migration rule targets a field outside the target schema profile".into(),
            ));
        }
    }
    Ok(())
}

impl ArchiveMigrationReport {
    pub fn validate(&self) -> Result<(), ArchiveMigrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_text(&self.source_schema_id)
            || !valid_version(&self.source_schema_version)
            || !valid_text(&self.target_schema_id)
            || !valid_version(&self.target_schema_version)
            || self.semantic_loss_milli > 1_000
            || !canonical(&self.rollback_field_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.limitations)
            || self
                .decisions
                .windows(2)
                .any(|pair| pair[0].source_field >= pair[1].source_field)
            || self.decisions.iter().any(|decision| {
                !valid_text(&decision.source_field)
                    || decision
                        .target_field
                        .as_deref()
                        .is_some_and(|field| !valid_text(field))
                    || decision.semantic_loss_milli > 1_000
                    || !valid_text(&decision.rationale)
            })
        {
            return Err(ArchiveMigrationError::InvalidOutput(
                "migration identity, versions, ordering, loss, or decision invariants are invalid"
                    .into(),
            ));
        }
        let decision_fields = self
            .decisions
            .iter()
            .map(|decision| decision.source_field.clone())
            .collect::<BTreeSet<_>>();
        if self
            .rollback_field_order
            .iter()
            .any(|field| !decision_fields.contains(field))
            || self
                .omission_order
                .iter()
                .any(|field| !decision_fields.contains(field))
            || self.blocked_order.iter().any(|entry| !valid_text(entry))
            || self
                .migrated
                .as_ref()
                .is_some_and(|object| !validate_migrated_object(object))
        {
            return Err(ArchiveMigrationError::InvalidOutput(
                "migration partitions or migrated object reference unknown or invalid fields"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ArchiveMigrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ArchiveMigrationError::InvalidOutput(
                "archive migration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Apply an explicit, reversible metadata migration without rewriting research artifacts.
pub fn migrate_glioma_archive_object(
    request: &ArchiveMigrationRequest,
) -> Result<ArchiveMigrationReport, ArchiveMigrationError> {
    validate_request(request)?;
    let source_fields = request
        .source
        .fields
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let rules = request
        .rules
        .iter()
        .map(|rule| (rule.source_field.clone(), rule))
        .collect::<BTreeMap<_, _>>();
    let mut decisions = Vec::new();
    let mut migrated_fields = BTreeMap::new();
    let mut rollback = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut limitations = BTreeSet::new();
    let mut total_loss = 0_u16;
    for field in &source_fields {
        let Some(value) = request.source.fields.get(field) else {
            continue;
        };
        let Some(rule) = rules.get(field) else {
            if request
                .target
                .required_field_order
                .binary_search(field)
                .is_ok()
                || !request.allow_optional_drop
            {
                blocked.insert(format!("{field}:missing-migration-rule"));
                decisions.push(ArchiveMigrationFieldDecision {
                    source_field: field.clone(),
                    target_field: None,
                    status: ArchiveMigrationFieldStatus::Blocked,
                    transform: None,
                    rule_version: None,
                    semantic_loss_milli: 1_000,
                    rationale: "source field has no declared target mapping".into(),
                });
            } else {
                omissions.insert(field.clone());
                limitations.insert(format!("{field}:optional-field-omitted"));
                decisions.push(ArchiveMigrationFieldDecision {
                    source_field: field.clone(),
                    target_field: None,
                    status: ArchiveMigrationFieldStatus::OmittedOptional,
                    transform: None,
                    rule_version: None,
                    semantic_loss_milli: 0,
                    rationale: "optional source field omitted only under explicit policy".into(),
                });
            }
            continue;
        };
        total_loss = total_loss.saturating_add(rule.semantic_loss_milli);
        if rule.semantic_loss_milli > request.max_semantic_loss_milli {
            blocked.insert(format!("{}:semantic-loss-budget", rule.source_field));
        }
        if request.require_rollback && !rule.reversible {
            blocked.insert(format!("{}:rollback-not-reversible", rule.source_field));
        }
        let status = match rule.transform {
            ArchiveMigrationTransform::Identity => ArchiveMigrationFieldStatus::Preserved,
            ArchiveMigrationTransform::Rename => ArchiveMigrationFieldStatus::Renamed,
        };
        migrated_fields.insert(rule.target_field.clone(), value.clone());
        if rule.reversible {
            rollback.insert(rule.source_field.clone());
        }
        decisions.push(ArchiveMigrationFieldDecision {
            source_field: rule.source_field.clone(),
            target_field: Some(rule.target_field.clone()),
            status,
            transform: Some(rule.transform),
            rule_version: Some(rule.rule_version.clone()),
            semantic_loss_milli: rule.semantic_loss_milli,
            rationale: "declared version-pinned transform applied without changing artifact bytes"
                .into(),
        });
    }
    for required in &request.target.required_field_order {
        if !migrated_fields.contains_key(required) {
            blocked.insert(format!("required-field-missing:{required}"));
        }
    }
    decisions.sort_by(|left, right| left.source_field.cmp(&right.source_field));
    let disposition = if !blocked.is_empty() {
        ArchiveMigrationDisposition::Blocked
    } else if !omissions.is_empty() || total_loss > 0 {
        ArchiveMigrationDisposition::Lossy
    } else {
        ArchiveMigrationDisposition::Ready
    };
    let migrated = if disposition == ArchiveMigrationDisposition::Blocked {
        None
    } else {
        Some(MigratedArchiveObject {
            object_id: request.source.object_id.clone(),
            schema_id: request.target.schema_id.clone(),
            schema_version: request.target.version.clone(),
            fields: migrated_fields,
            artifacts: request.source.artifacts.clone(),
            provenance_digest: request.source.provenance_digest.clone(),
            uncertainty_order: request.source.uncertainty_order.clone(),
            negative_evidence_order: request.source.negative_evidence_order.clone(),
        })
    };
    let mut output = ArchiveMigrationReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        source_schema_id: request.source.schema_id.clone(),
        source_schema_version: request.source.schema_version.clone(),
        target_schema_id: request.target.schema_id.clone(),
        target_schema_version: request.target.version.clone(),
        decisions,
        migrated,
        rollback_field_order: rollback.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        limitations: limitations.into_iter().collect(),
        semantic_loss_milli: total_loss.min(1_000),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-archive-migration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ArchiveMigrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> ArchiveMigrationRequest {
        ArchiveMigrationRequest {
            source: ArchiveObject {
                object_id: "glioma-object".into(),
                schema_id: "aurora.glioma.result".into(),
                schema_version: "1.0".into(),
                fields: BTreeMap::from([
                    ("effect".into(), "0.42".into()),
                    ("study".into(), "study-1".into()),
                ]),
                artifacts: vec![ArchiveArtifactRef {
                    artifact_id: "result".into(),
                    content_hash: hash("result"),
                    required: true,
                    local_only: true,
                }],
                provenance_digest: hash("provenance"),
                uncertainty_order: vec!["tail".into()],
                negative_evidence_order: vec!["null".into()],
            },
            target: ArchiveSchemaProfile {
                schema_id: "aurora.glioma.result".into(),
                version: "2.0".into(),
                required_field_order: vec!["effect".into(), "study_id".into()],
                allowed_field_order: vec!["effect".into(), "study_id".into()],
            },
            rules: vec![
                ArchiveMigrationRule {
                    source_field: "effect".into(),
                    target_field: "effect".into(),
                    transform: ArchiveMigrationTransform::Identity,
                    reversible: true,
                    semantic_loss_milli: 0,
                    rule_version: "1.0".into(),
                },
                ArchiveMigrationRule {
                    source_field: "study".into(),
                    target_field: "study_id".into(),
                    transform: ArchiveMigrationTransform::Rename,
                    reversible: true,
                    semantic_loss_milli: 0,
                    rule_version: "1.0".into(),
                },
            ],
            max_semantic_loss_milli: 0,
            allow_optional_drop: false,
            require_rollback: true,
        }
    }

    #[test]
    fn migration_preserves_provenance_and_applies_reversible_rename() {
        let report = migrate_glioma_archive_object(&request()).unwrap();
        assert_eq!(report.disposition, ArchiveMigrationDisposition::Ready);
        let migrated = report.migrated.as_ref().unwrap();
        assert_eq!(migrated.fields.get("study_id"), Some(&"study-1".into()));
        assert_eq!(migrated.provenance_digest, hash("provenance"));
        assert_eq!(report.rollback_field_order, vec!["effect", "study"]);
        report.validate().unwrap();
    }

    #[test]
    fn unknown_mandatory_field_blocks_without_synthesizing_a_value() {
        let mut request = request();
        request.rules.pop();
        let report = migrate_glioma_archive_object(&request).unwrap();
        assert_eq!(report.disposition, ArchiveMigrationDisposition::Blocked);
        assert!(report
            .blocked_order
            .iter()
            .any(|item| item.contains("study")));
        assert!(report.migrated.is_none());
    }

    #[test]
    fn lossy_optional_drop_is_explicit_and_digest_bound() {
        let mut request = request();
        request.target.required_field_order = vec!["effect".into(), "study_id".into()];
        request.rules.pop();
        request.allow_optional_drop = true;
        request.target.required_field_order = vec!["effect".into()];
        let report = migrate_glioma_archive_object(&request).unwrap();
        assert_eq!(report.disposition, ArchiveMigrationDisposition::Lossy);
        assert_eq!(report.omission_order, vec!["study"]);
        let mut mutated = report.clone();
        mutated.limitations.push("tampered".into());
        assert!(mutated.validate().is_err());
    }

    #[test]
    fn non_reversible_rule_blocks_when_rollback_is_required() {
        let mut request = request();
        request.rules[0].reversible = false;
        let report = migrate_glioma_archive_object(&request).unwrap();
        assert_eq!(report.disposition, ArchiveMigrationDisposition::Blocked);
        assert!(report
            .blocked_order
            .iter()
            .any(|item| item.contains("rollback-not-reversible")));
    }
}
