//! Migrations that can be checked before they are trusted.
//!
//! Blueprint 40.37 names four non-negotiable invariants; two of them are enforced here. *Migration
//! is explicit and versioned*: a [`Migration`] carries the [`SchemaId`] it moves between and
//! refuses a document that declares a different source version. *Lossy migration is never silent*:
//! [`Migration::audit_loss`] compares what a migration actually drops against what it declared it
//! drops, and an undeclared drop is [`MigrationError::UndeclaredLoss`], not a warning.
//!
//! # What "total" means and why an empty corpus fails
//!
//! 25.22 asks for "forward/backward fixtures" and 40.37 for "all historical fixtures", without
//! saying what passing looks like. Here a migration is total over a source version when every
//! document that conforms to the source descriptor migrates without error *and* the result
//! conforms to the target descriptor. [`TotalityReport::is_total`] returns false for an empty
//! corpus: a migration checked against no documents is unchecked, and reporting that as totality
//! would be the same category error as scoring an unmeasured capability zero.
//!
//! # Deliberately not implemented
//!
//! - **No database.** 40.37's "partial DB migration" failure mode cannot occur here because there
//!   is no store, no transaction and no rollback. [`Migration::apply`] is a pure function from one
//!   JSON value to another.
//! - **No migration runner.** Nothing walks a directory, streams a table or writes anything. The
//!   projection-rebuild CLI that 40.37 lists under "Interfaces" would be a caller of this.
//! - **No code-carrying migrations.** A migration is a list of declarative steps, not a closure,
//!   precisely so that it can be serialised, hashed, reviewed and inverted. That rules out
//!   genuinely computational migrations; those would need a step variant and a review of what
//!   determinism means for it.

use crate::descriptor::SchemaDescriptor;
use crate::error::MigrationError;
use crate::pointer;
use crate::version::SchemaId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// The conventional key a document uses to declare its own schema. All three formats in this
/// workspace carry it, inside the hashed bytes.
pub const SCHEMA_VERSION_KEY: &str = "schema_version";

/// One declarative edit.
///
/// Every variant is either invertible or explicitly not. [`MigrationStep::Drop`] is the only lossy
/// one, and it is lossy by definition rather than by accident — which is what makes
/// [`Migration::audit_loss`] able to give a complete answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum MigrationStep {
    /// Adds a key that must not already exist. Inverse: [`MigrationStep::Drop`].
    Introduce { path: String, value: Value },
    /// Removes a key. Not invertible: the value is gone.
    Drop { path: String },
    /// Moves a key to a name that must not already exist. Inverse: the reverse rename.
    Rename { from: String, to: String },
    /// Replaces an exact value with another. Inverse: the swap.
    ///
    /// The exactness is what makes this usable for the `schema_version` field: a document whose
    /// value does not match `from` is not a document of the source version, and saying so is more
    /// useful than silently rewriting it.
    Replace {
        path: String,
        from: Value,
        to: Value,
    },
}

impl MigrationStep {
    fn apply(&self, document: &mut Value, index: usize) -> Result<(), MigrationError> {
        let failed = |detail: String| MigrationError::StepFailed {
            step: index,
            detail,
        };
        match self {
            MigrationStep::Introduce { path, value } => {
                if pointer::contains(document, path) {
                    return Err(failed(format!(
                        "cannot introduce {path:?}: the document already carries it"
                    )));
                }
                pointer::insert(document, path, value.clone()).map_err(failed)?;
                Ok(())
            }
            MigrationStep::Drop { path } => {
                pointer::remove(document, path);
                Ok(())
            }
            MigrationStep::Rename { from, to } => {
                if pointer::contains(document, to) {
                    return Err(failed(format!(
                        "cannot rename {from:?} to {to:?}: {to:?} is already present and would be \
                         overwritten"
                    )));
                }
                match pointer::remove(document, from) {
                    None => Ok(()),
                    Some(value) => {
                        pointer::insert(document, to, value).map_err(failed)?;
                        Ok(())
                    }
                }
            }
            MigrationStep::Replace { path, from, to } => match pointer::get(document, path) {
                None => Err(failed(format!(
                    "{path:?} is absent, so it cannot be replaced"
                ))),
                Some(found) if found == from => {
                    pointer::insert(document, path, to.clone()).map_err(failed)?;
                    Ok(())
                }
                Some(found) => Err(failed(format!(
                    "{path:?} is {found}, not the expected {from}"
                ))),
            },
        }
    }

    /// The step that undoes this one, or why there is none.
    pub fn inverse(&self, index: usize) -> Result<MigrationStep, MigrationError> {
        match self {
            MigrationStep::Introduce { path, .. } => Ok(MigrationStep::Drop { path: path.clone() }),
            MigrationStep::Drop { path } => Err(MigrationError::NotInvertible {
                step: index,
                detail: format!("dropping {path:?} discards its value, which cannot be recovered"),
            }),
            MigrationStep::Rename { from, to } => Ok(MigrationStep::Rename {
                from: to.clone(),
                to: from.clone(),
            }),
            MigrationStep::Replace { path, from, to } => Ok(MigrationStep::Replace {
                path: path.clone(),
                from: to.clone(),
                to: from.clone(),
            }),
        }
    }

    /// The path this step removes information from, if any.
    pub fn dropped_path(&self) -> Option<&str> {
        match self {
            MigrationStep::Drop { path } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for MigrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationStep::Introduce { path, value } => write!(f, "introduce {path} = {value}"),
            MigrationStep::Drop { path } => write!(f, "drop {path}"),
            MigrationStep::Rename { from, to } => write!(f, "rename {from} -> {to}"),
            MigrationStep::Replace { path, from, to } => {
                write!(f, "replace {path}: {from} -> {to}")
            }
        }
    }
}

/// Something a migration knowingly discards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossItem {
    pub path: String,
    /// Why the information is not carried forward. 40.37 requires a loss *report*, and a path with
    /// no reason is a list, not a report.
    pub reason: String,
}

impl LossItem {
    pub fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        LossItem {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

/// A versioned, declarative, checkable migration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Migration {
    pub from: SchemaId,
    pub to: SchemaId,
    pub steps: Vec<MigrationStep>,
    pub declared_loss: Vec<LossItem>,
}

impl Migration {
    pub fn new(
        from: SchemaId,
        to: SchemaId,
        steps: Vec<MigrationStep>,
    ) -> Result<Self, MigrationError> {
        if from == to {
            return Err(MigrationError::IdentityMigration {
                from: from.to_string(),
                to: to.to_string(),
            });
        }
        Ok(Migration {
            from,
            to,
            steps,
            declared_loss: Vec::new(),
        })
    }

    pub fn declaring_loss(mut self, loss: Vec<LossItem>) -> Self {
        self.declared_loss = loss;
        self
    }

    /// Migrates one document.
    ///
    /// A document that declares a `schema_version` other than this migration's source is refused.
    /// A document that declares none is accepted: not every format self-describes, and inventing a
    /// version for it would be worse than migrating what was asked for.
    pub fn apply(&self, document: &Value) -> Result<Value, MigrationError> {
        if !document.is_object() {
            return Err(MigrationError::NotAnObject);
        }
        if let Some(declared) = pointer::get(document, SCHEMA_VERSION_KEY).and_then(Value::as_str) {
            if declared != self.from.to_string() {
                return Err(MigrationError::SourceVersionMismatch {
                    from: self.from.to_string(),
                    to: self.to.to_string(),
                    found: declared.to_string(),
                });
            }
        }

        let mut migrated = document.clone();
        for (index, step) in self.steps.iter().enumerate() {
            step.apply(&mut migrated, index)?;
        }
        Ok(migrated)
    }

    /// The migration that undoes this one, or the first reason there is none.
    pub fn inverse(&self) -> Result<Migration, MigrationError> {
        let mut steps = Vec::with_capacity(self.steps.len());
        for (index, step) in self.steps.iter().enumerate().rev() {
            steps.push(step.inverse(index)?);
        }
        Migration::new(self.to.clone(), self.from.clone(), steps)
    }

    /// Whether every path this migration statically discards was declared.
    pub fn undeclared_static_losses(&self) -> Vec<String> {
        let declared: BTreeSet<&str> = self
            .declared_loss
            .iter()
            .map(|item| item.path.as_str())
            .collect();
        self.steps
            .iter()
            .filter_map(MigrationStep::dropped_path)
            .filter(|path| !declared.contains(path))
            .map(str::to_string)
            .collect()
    }

    /// Applies the migration to every document a source descriptor accepts.
    ///
    /// Documents that do not conform to `source` are counted as outside the domain rather than as
    /// failures: a migration is asked to be total over its source *version*, not over arbitrary
    /// JSON.
    pub fn totality_over(
        &self,
        corpus: &[Value],
        source: &SchemaDescriptor,
        target: &SchemaDescriptor,
    ) -> TotalityReport {
        let mut report = TotalityReport {
            from: self.from.to_string(),
            to: self.to.to_string(),
            ..TotalityReport::default()
        };

        for (index, document) in corpus.iter().enumerate() {
            if !source.check_document(document).conforms() {
                report.outside_domain.push(index);
                continue;
            }
            report.checked += 1;
            match self.apply(document) {
                Err(error) => report.failures.push(DocumentFailure {
                    index,
                    detail: error.to_string(),
                }),
                Ok(migrated) => {
                    let check = target.check_document(&migrated);
                    if !check.conforms() {
                        report.output_violations.push(DocumentFailure {
                            index,
                            detail: format!(
                                "missing {:?}, mistyped {:?}",
                                check.missing,
                                check
                                    .mistyped
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                            ),
                        });
                    }
                }
            }
        }

        report
    }

    /// Migrates each document forward and back, and holds the result against the declared loss.
    ///
    /// Three outcomes are possible and all three are honest:
    ///
    /// - The migration inverts and every document is recovered byte for byte — [`LossAudit::lossless`].
    /// - The migration does not invert, or a document is not recovered, and every affected path
    ///   appears in `declared_loss` — a declared-lossy migration.
    /// - Something was lost that nobody declared — [`MigrationError::UndeclaredLoss`].
    pub fn audit_loss(&self, corpus: &[Value]) -> Result<LossAudit, MigrationError> {
        let declared: BTreeSet<&str> = self
            .declared_loss
            .iter()
            .map(|item| item.path.as_str())
            .collect();
        let mut lost: BTreeSet<String> = BTreeSet::new();

        for path in self.steps.iter().filter_map(MigrationStep::dropped_path) {
            lost.insert(path.to_string());
        }

        let inverse = self.inverse();
        let mut round_tripped = 0usize;
        let mut invertible = false;

        if let Ok(back) = &inverse {
            invertible = true;
            for document in corpus {
                let forward = self.apply(document)?;
                let recovered = back.apply(&forward)?;
                if &recovered == document {
                    round_tripped += 1;
                } else {
                    for path in differing_paths(document, &recovered) {
                        lost.insert(path);
                    }
                }
            }
        }

        if let Some(path) = lost.iter().find(|path| !declared.contains(path.as_str())) {
            return Err(MigrationError::UndeclaredLoss {
                from: self.from.to_string(),
                to: self.to.to_string(),
                path: path.clone(),
            });
        }

        Ok(LossAudit {
            invertible,
            round_tripped,
            corpus_size: corpus.len(),
            lost: lost.into_iter().collect(),
            declared: self.declared_loss.clone(),
        })
    }
}

/// Every top-level path where two documents disagree, in either direction.
fn differing_paths(original: &Value, recovered: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
    let empty = serde_json::Map::new();
    let left = original.as_object().unwrap_or(&empty);
    let right = recovered.as_object().unwrap_or(&empty);
    for (key, value) in left {
        if right.get(key) != Some(value) {
            paths.insert(key.clone());
        }
    }
    for key in right.keys() {
        if !left.contains_key(key) {
            paths.insert(key.clone());
        }
    }
    paths.into_iter().collect()
}

/// One document that failed, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentFailure {
    pub index: usize,
    pub detail: String,
}

/// The result of running a migration over a fixture corpus.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotalityReport {
    pub from: String,
    pub to: String,
    /// Documents that conformed to the source descriptor and were therefore in the domain.
    pub checked: usize,
    /// Documents that did not conform to the source descriptor.
    pub outside_domain: Vec<usize>,
    pub failures: Vec<DocumentFailure>,
    /// Documents that migrated without error but produced something the target rejects.
    pub output_violations: Vec<DocumentFailure>,
}

impl TotalityReport {
    /// Whether the migration is total over the source version, *as evidenced by this corpus*.
    ///
    /// False for an empty domain. A migration nobody exercised is not a migration anybody checked,
    /// and 40.37's verification plan opens with "all historical fixtures" for exactly this reason.
    pub fn is_total(&self) -> bool {
        self.checked > 0 && self.failures.is_empty() && self.output_violations.is_empty()
    }
}

/// What a round trip recovered, and what it did not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LossAudit {
    /// Whether every step had an inverse.
    pub invertible: bool,
    /// How many documents came back byte for byte.
    pub round_tripped: usize,
    pub corpus_size: usize,
    /// Paths that did not survive, whether statically or observed.
    pub lost: Vec<String>,
    pub declared: Vec<LossItem>,
}

impl LossAudit {
    /// Whether the migration lost nothing at all — proven by round trip, not asserted.
    pub fn lossless(&self) -> bool {
        self.invertible && self.lost.is_empty() && self.round_tripped == self.corpus_size
    }
}

/// The registry 40.37 lists under "Interfaces", as a graph over schema versions.
///
/// Migrations compose: a document at `0.1` reaches `0.3` by way of whatever chain is registered.
/// There is no automatic transitive closure and no shortest-path heuristic beyond breadth-first
/// order, because a migration is reviewed content and synthesising one would defeat the point.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MigrationRegistry {
    migrations: BTreeMap<(String, String), Migration>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        MigrationRegistry::default()
    }

    pub fn register(&mut self, migration: Migration) -> Result<(), MigrationError> {
        let key = (migration.from.to_string(), migration.to.to_string());
        if self.migrations.contains_key(&key) {
            return Err(MigrationError::DuplicateMigration {
                from: key.0,
                to: key.1,
            });
        }
        self.migrations.insert(key, migration);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.migrations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.migrations.is_empty()
    }

    pub fn get(&self, from: &SchemaId, to: &SchemaId) -> Option<&Migration> {
        self.migrations.get(&(from.to_string(), to.to_string()))
    }

    /// The chain of migrations that carries a document from one version to another.
    ///
    /// Breadth-first over registered edges, with ties broken by the version's storage order so the
    /// answer is deterministic across runs and machines.
    pub fn path(&self, from: &SchemaId, to: &SchemaId) -> Result<Vec<&Migration>, MigrationError> {
        let start = from.to_string();
        let goal = to.to_string();
        if start == goal {
            return Ok(Vec::new());
        }

        let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (source, target) in self.migrations.keys() {
            adjacency
                .entry(source.as_str())
                .or_default()
                .push(target.as_str());
        }

        let mut came_from: BTreeMap<&str, &str> = BTreeMap::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        seen.insert(start.as_str());
        queue.push_back(start.as_str());

        while let Some(node) = queue.pop_front() {
            if node == goal {
                break;
            }
            for next in adjacency.get(node).into_iter().flatten().copied() {
                if seen.insert(next) {
                    came_from.insert(next, node);
                    queue.push_back(next);
                }
            }
        }

        if !seen.contains(goal.as_str()) {
            return Err(MigrationError::NoPath {
                from: start,
                to: goal,
            });
        }

        let mut reversed: Vec<(&str, &str)> = Vec::new();
        let mut cursor = goal.as_str();
        while cursor != start {
            let Some(previous) = came_from.get(cursor).copied() else {
                return Err(MigrationError::NoPath {
                    from: start.clone(),
                    to: goal.clone(),
                });
            };
            reversed.push((previous, cursor));
            cursor = previous;
        }
        reversed.reverse();

        let mut path = Vec::with_capacity(reversed.len());
        for (source, target) in reversed {
            let Some(migration) = self
                .migrations
                .get(&(source.to_string(), target.to_string()))
            else {
                return Err(MigrationError::NoPath {
                    from: start.clone(),
                    to: goal.clone(),
                });
            };
            path.push(migration);
        }
        Ok(path)
    }

    /// Migrates a document along the registered chain.
    pub fn migrate(
        &self,
        from: &SchemaId,
        to: &SchemaId,
        document: &Value,
    ) -> Result<Value, MigrationError> {
        let mut current = document.clone();
        for migration in self.path(from, to)? {
            current = migration.apply(&current)?;
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{FieldSpec, FieldType, SchemaDescriptor};
    use crate::mode::CompatibilityMode;
    use serde_json::json;

    fn id(version: &str) -> SchemaId {
        SchemaId::parse(&format!("test-format/{version}")).expect("parses")
    }

    fn descriptor(version: &str, fields: Vec<FieldSpec>) -> SchemaDescriptor {
        SchemaDescriptor::new(id(version), CompatibilityMode::PreserveAndForward, fields)
            .expect("well formed")
    }

    fn version_step(from: &str, to: &str) -> MigrationStep {
        MigrationStep::Replace {
            path: SCHEMA_VERSION_KEY.into(),
            from: json!(format!("test-format/{from}")),
            to: json!(format!("test-format/{to}")),
        }
    }

    fn rename_migration() -> Migration {
        Migration::new(
            id("1.0"),
            id("2.0"),
            vec![
                version_step("1.0", "2.0"),
                MigrationStep::Rename {
                    from: "world".into(),
                    to: "world_id".into(),
                },
            ],
        )
        .expect("distinct versions")
    }

    fn corpus() -> Vec<Value> {
        vec![
            json!({"schema_version": "test-format/1.0", "world": "w1"}),
            json!({"schema_version": "test-format/1.0", "world": "w2"}),
        ]
    }

    #[test]
    fn a_migration_refuses_a_document_that_declares_a_different_source_version() {
        let error = rename_migration()
            .apply(&json!({"schema_version": "test-format/1.5", "world": "w1"}))
            .expect_err("a 1.5 document is not in this migration's domain");
        assert!(matches!(
            error,
            MigrationError::SourceVersionMismatch { ref found, .. } if found == "test-format/1.5"
        ));
    }

    #[test]
    fn a_lossless_migration_recovers_every_document_byte_for_byte() {
        let audit = rename_migration().audit_loss(&corpus()).expect("no loss");
        assert!(audit.lossless());
        assert_eq!(audit.round_tripped, 2);
        assert!(audit.lost.is_empty());
    }

    #[test]
    fn a_migration_that_drops_a_field_without_declaring_it_is_an_error_not_a_warning() {
        let lossy = Migration::new(
            id("1.0"),
            id("2.0"),
            vec![
                version_step("1.0", "2.0"),
                MigrationStep::Drop {
                    path: "world".into(),
                },
            ],
        )
        .expect("distinct versions");

        let error = lossy
            .audit_loss(&corpus())
            .expect_err("40.37 invariant 3: a lossy migration is never silent");
        assert!(matches!(
            error,
            MigrationError::UndeclaredLoss { ref path, .. } if path == "world"
        ));
        assert_eq!(lossy.undeclared_static_losses(), ["world"]);
    }

    #[test]
    fn the_same_migration_passes_once_it_states_what_it_drops_and_why() {
        let lossy = Migration::new(
            id("1.0"),
            id("2.0"),
            vec![
                version_step("1.0", "2.0"),
                MigrationStep::Drop {
                    path: "world".into(),
                },
            ],
        )
        .expect("distinct versions")
        .declaring_loss(vec![LossItem::new(
            "world",
            "superseded by world_id, which carries the scope the bare name never did",
        )]);

        let audit = lossy
            .audit_loss(&corpus())
            .expect("declared loss is allowed");
        assert!(!audit.lossless());
        assert!(!audit.invertible);
        assert_eq!(audit.lost, ["world"]);
        assert!(lossy.undeclared_static_losses().is_empty());
    }

    #[test]
    fn a_migration_verified_against_an_empty_corpus_is_not_total() {
        let source = descriptor("1.0", vec![FieldSpec::required("world", FieldType::String)]);
        let target = descriptor(
            "2.0",
            vec![FieldSpec::required("world_id", FieldType::String)],
        );
        let report = rename_migration().totality_over(&[], &source, &target);
        assert_eq!(report.checked, 0);
        assert!(
            !report.is_total(),
            "a migration nobody exercised has not been shown to be total"
        );
    }

    #[test]
    fn a_migration_is_total_when_every_source_document_produces_a_valid_target_document() {
        let source = descriptor(
            "1.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world", FieldType::String),
            ],
        );
        let target = descriptor(
            "2.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world_id", FieldType::String),
            ],
        );
        let report = rename_migration().totality_over(&corpus(), &source, &target);
        assert_eq!(report.checked, 2);
        assert!(report.is_total());
    }

    #[test]
    fn a_document_outside_the_source_version_is_not_counted_against_totality() {
        let source = descriptor(
            "1.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world", FieldType::String),
            ],
        );
        let target = descriptor(
            "2.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world_id", FieldType::String),
            ],
        );
        let mut documents = corpus();
        documents.push(json!({"schema_version": "test-format/1.0"}));
        let report = rename_migration().totality_over(&documents, &source, &target);
        assert_eq!(report.outside_domain, [2]);
        assert_eq!(report.checked, 2);
        assert!(report.is_total());
    }

    #[test]
    fn a_migration_that_produces_an_invalid_target_document_is_not_total_even_though_it_ran() {
        let source = descriptor(
            "1.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world", FieldType::String),
            ],
        );
        let target = descriptor(
            "2.0",
            vec![
                FieldSpec::required(SCHEMA_VERSION_KEY, FieldType::String),
                FieldSpec::required("world_id", FieldType::String),
                FieldSpec::required("query_id", FieldType::String),
            ],
        );
        let report = rename_migration().totality_over(&corpus(), &source, &target);
        assert!(report.failures.is_empty());
        assert_eq!(report.output_violations.len(), 2);
        assert!(!report.is_total());
    }

    #[test]
    fn renaming_onto_an_occupied_key_fails_rather_than_overwriting_it() {
        let error = rename_migration()
            .apply(&json!({
                "schema_version": "test-format/1.0",
                "world": "w1",
                "world_id": "already here"
            }))
            .expect_err("an overwrite would lose the occupant silently");
        assert!(matches!(error, MigrationError::StepFailed { .. }));
    }

    #[test]
    fn a_migration_between_identical_versions_is_refused_at_construction() {
        assert!(matches!(
            Migration::new(id("1.0"), id("1.0"), vec![]),
            Err(MigrationError::IdentityMigration { .. })
        ));
    }

    #[test]
    fn the_registry_chains_migrations_across_intermediate_versions() {
        let mut registry = MigrationRegistry::new();
        registry.register(rename_migration()).expect("first edge");
        registry
            .register(
                Migration::new(
                    id("2.0"),
                    id("3.0"),
                    vec![
                        version_step("2.0", "3.0"),
                        MigrationStep::Introduce {
                            path: "query_id".into(),
                            value: json!("unknown"),
                        },
                    ],
                )
                .expect("distinct versions"),
            )
            .expect("second edge");

        let chain = registry
            .path(&id("1.0"), &id("3.0"))
            .expect("a path exists");
        assert_eq!(chain.len(), 2);

        let migrated = registry
            .migrate(&id("1.0"), &id("3.0"), &corpus()[0])
            .expect("chain applies");
        assert_eq!(
            migrated,
            json!({
                "schema_version": "test-format/3.0",
                "world_id": "w1",
                "query_id": "unknown"
            })
        );
    }

    #[test]
    fn a_gap_in_the_registry_is_a_typed_absence_rather_than_a_silent_identity() {
        let mut registry = MigrationRegistry::new();
        registry.register(rename_migration()).expect("edge");
        assert!(matches!(
            registry.path(&id("1.0"), &id("9.0")),
            Err(MigrationError::NoPath { .. })
        ));
        assert!(registry
            .path(&id("1.0"), &id("1.0"))
            .expect("a version reaches itself")
            .is_empty());
    }

    #[test]
    fn registering_the_same_edge_twice_is_refused() {
        let mut registry = MigrationRegistry::new();
        registry.register(rename_migration()).expect("first");
        assert!(matches!(
            registry.register(rename_migration()),
            Err(MigrationError::DuplicateMigration { .. })
        ));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn a_non_object_document_is_refused_before_any_step_runs() {
        assert!(matches!(
            rename_migration().apply(&json!(["not", "an", "object"])),
            Err(MigrationError::NotAnObject)
        ));
    }
}
