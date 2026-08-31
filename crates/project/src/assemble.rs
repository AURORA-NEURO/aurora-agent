//! From a scan to a `fiber-world/0.1` document the FIBER pipeline can judge.
//!
//! The world deliberately carries the *decision* layer, not the file layer: per-component
//! inventory facts, the aggregate variables the release factor consumes, and one fact per
//! caller-supplied issue. The per-file evidence stays in the sealed
//! [`bioprism_adapter::Ingestion`] the scan produced — a world with ten thousand per-file facts
//! would make every compile budget a fight about colour, and the component digests keep the
//! file layer addressable from the world without copying it in.
//!
//! Honesty decisions, stated:
//!
//! * The scan's loss report ships **into** the world as the `scan_loss_summary` fact, tagged
//!   protected, and the release oracle requires it. A verdict computed without seeing what the
//!   scan skipped would be a verdict about a tree nobody scanned.
//! * Protected facts are the ones a project audit must never lose: the dependency
//!   declarations, the unpinned subset, the test inventory, the CI inventory, and the loss
//!   summary. Component inventories, marker totals and doc counts are colour — reachable
//!   through factors, droppable when irrelevant.
//! * No clocks. The scan event's `event_time` and `availability_time` and every query's
//!   `decision_time` are one caller-supplied timestamp, defaulting to
//!   [`DEFAULT_DECISION_TIME`]; the model is "the scan is evidence at the instant the caller
//!   decides", and a caller who wants a real timeline supplies one.

use crate::packs;
use crate::scan::{
    component_display, component_slug, dependency_value, listing_value, FileContent, Issue,
    ProjectScan,
};
use crate::ProjectError;
use bioprism_ids::ContentHash;
use bioprism_world::World;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The fixed epoch default for the scan event and every generated query. A fixed string rather
/// than a wall clock, so two assemblies of the same scan are byte-identical.
pub const DEFAULT_DECISION_TIME: &str = "1970-01-01T00:00:00Z";

/// The value bound to the `scan` scope dimension of every aggregate fact: which scanner
/// ontology produced the numbers.
pub const SCAN_DIMENSION_VALUE: &str = "bioprism.project/0.1.0";

/// The six variables the release factor consumes, and the aggregate half of every issue
/// factor's inputs.
pub const DECISION_INPUTS: [&str; 6] = [
    "dependency_declarations",
    "unpinned_dependencies",
    "test_function_total",
    "todo_marker_total",
    "ci_workflow_inventory",
    "scan_loss_summary",
];

/// Every variable the scan event produces — the decision inputs plus the exploratory
/// aggregates.
pub const AGGREGATE_VARIABLES: [&str; 10] = [
    "dependency_declarations",
    "unpinned_dependencies",
    "test_function_total",
    "todo_marker_total",
    "ci_workflow_inventory",
    "ci_workflow_count",
    "source_file_total",
    "uninterpreted_file_total",
    "doc_inventory",
    "scan_loss_summary",
];

/// Declared thresholds for the emitted pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thresholds {
    /// `todo_burden` fires when `todo_marker_total` is at least this. The default is 50 and it
    /// is a declared editorial choice, not a measurement.
    pub todo_burden_at_least: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            todo_burden_at_least: 50,
        }
    }
}

/// Caller-supplied context for one assembly.
#[derive(Debug, Clone, Default)]
pub struct AssemblyOptions {
    /// RFC 3339. Used as the scan event's `event_time` and `availability_time` and as the
    /// generated queries' `decision_time`. Empty string means [`DEFAULT_DECISION_TIME`].
    pub decision_time: String,
    pub issues: Vec<Issue>,
    pub thresholds: Thresholds,
}

impl AssemblyOptions {
    fn decision_time(&self) -> &str {
        if self.decision_time.is_empty() {
            DEFAULT_DECISION_TIME
        } else {
            &self.decision_time
        }
    }
}

/// An assembled project world with its dimension document, pack and generated queries.
#[derive(Debug, Clone)]
pub struct ProjectWorld {
    /// `project-` plus a content-derived digest prefix, so the same tree always assembles to
    /// the same world id and a changed tree never reuses one.
    pub world_id: String,
    pub world: Value,
    /// The `bioprism-scope-dimensions/0.1` document classifying every dimension this world's
    /// scopes bind.
    pub dimensions: Value,
    /// The `bioprism-domain/0.1` release-readiness pack, thresholds applied.
    pub pack: Value,
    /// A `fiber-query/0.2` document targeting `release_integrity_status`.
    pub release_query: Value,
    /// One `fiber-query/0.2` document per issue, keyed by issue id, targeting
    /// `issue_<id>_context_status`.
    pub issue_queries: BTreeMap<String, Value>,
}

struct ComponentSummary {
    files: u64,
    lines: u64,
    todo_markers: u64,
    fixme_markers: u64,
    unimplemented_markers: u64,
    test_functions: u64,
    uninterpreted_files: u64,
    listing: Vec<usize>,
}

impl ProjectWorld {
    /// Assembles the world. Fails rather than guessing on component-slug collisions and on
    /// worlds the reference validator would reject; the returned document has already passed
    /// [`bioprism_world::World::from_json`].
    pub fn assemble(
        scan: &ProjectScan,
        options: &AssemblyOptions,
    ) -> Result<ProjectWorld, ProjectError> {
        let decision_time = options.decision_time();

        let mut components: BTreeMap<String, ComponentSummary> = BTreeMap::new();
        for (position, file) in scan.files.iter().enumerate() {
            let summary = components
                .entry(file.component.clone())
                .or_insert(ComponentSummary {
                    files: 0,
                    lines: 0,
                    todo_markers: 0,
                    fixme_markers: 0,
                    unimplemented_markers: 0,
                    test_functions: 0,
                    uninterpreted_files: 0,
                    listing: Vec::new(),
                });
            summary.files += 1;
            summary.listing.push(position);
            match &file.content {
                FileContent::Text {
                    lines,
                    todo_markers,
                    fixme_markers,
                    unimplemented_markers,
                    test_functions,
                } => {
                    summary.lines += lines;
                    summary.todo_markers += todo_markers;
                    summary.fixme_markers += fixme_markers;
                    summary.unimplemented_markers += unimplemented_markers;
                    summary.test_functions += test_functions.unwrap_or(0);
                }
                _ => summary.uninterpreted_files += 1,
            }
        }

        let mut slugs: BTreeMap<String, String> = BTreeMap::new();
        for component in components.keys() {
            let slug = component_slug(component);
            if let Some(existing) = slugs.insert(slug.clone(), component.clone()) {
                return Err(ProjectError::Assembly(format!(
                    "component directories {existing:?} and {component:?} share the slug \
                     {slug:?}; their inventory variables would collide"
                )));
            }
        }

        let world_id = derive_world_id(scan)?;
        let mut facts: Vec<Value> = Vec::new();
        let mut factors: Vec<Value> = Vec::new();

        for (component, summary) in &components {
            let listing = listing_value(
                &summary
                    .listing
                    .iter()
                    .map(|&position| scan.files[position].clone())
                    .collect::<Vec<_>>(),
            );
            let digest = ContentHash::of_value(&listing)
                .map_err(|error| ProjectError::Assembly(error.to_string()))?;
            let slug = component_slug(component);
            let mut value = Map::new();
            value.insert("files".to_string(), Value::from(summary.files));
            value.insert("lines".to_string(), Value::from(summary.lines));
            value.insert("sha256".to_string(), Value::String(digest.to_string()));
            value.insert(
                "todo_markers".to_string(),
                Value::from(summary.todo_markers),
            );
            value.insert(
                "fixme_markers".to_string(),
                Value::from(summary.fixme_markers),
            );
            value.insert(
                "unimplemented_markers".to_string(),
                Value::from(summary.unimplemented_markers),
            );
            value.insert(
                "test_functions".to_string(),
                Value::from(summary.test_functions),
            );
            value.insert(
                "uninterpreted_files".to_string(),
                Value::from(summary.uninterpreted_files),
            );
            facts.push(fact(
                format!("fact.component.{slug}"),
                format!("component_{slug}_inventory"),
                Value::Object(value),
                scope_pairs(&[
                    ("project", &scan.project),
                    ("component", &component_display(component)),
                ]),
                &["component"],
                format!("{}/{}", scan.project, component_display(component)),
            ));
        }

        let dependency_declarations: Vec<Value> =
            scan.dependencies.iter().map(dependency_value).collect();
        let unpinned: Vec<Value> = scan
            .unpinned_dependencies()
            .into_iter()
            .map(dependency_value)
            .collect();
        let workflows: Vec<Value> = scan
            .workflows
            .iter()
            .cloned()
            .map(Value::String)
            .collect();
        let docs: Vec<Value> = scan.docs.iter().cloned().map(Value::String).collect();
        let loss_counts = scan.loss_kind_counts();
        let mut loss_summary = Map::new();
        loss_summary.insert(
            "total".to_string(),
            Value::from(scan.loss.entries().len() as u64),
        );
        loss_summary.insert(
            "counts".to_string(),
            Value::Object(
                loss_counts
                    .iter()
                    .map(|(kind, count)| (kind.clone(), Value::from(*count)))
                    .collect(),
            ),
        );

        let mut doc_inventory = Map::new();
        doc_inventory.insert("count".to_string(), Value::from(docs.len() as u64));
        doc_inventory.insert("paths".to_string(), Value::Array(docs));

        let aggregates: [(&str, Value, &[&str]); 10] = [
            (
                "dependency_declarations",
                Value::Array(dependency_declarations),
                &["dependency", "protected"],
            ),
            (
                "unpinned_dependencies",
                Value::Array(unpinned),
                &["dependency", "protected"],
            ),
            (
                "test_function_total",
                Value::from(scan.test_function_total()),
                &["tests", "protected"],
            ),
            (
                "todo_marker_total",
                Value::from(scan.todo_marker_total()),
                &["markers"],
            ),
            (
                "ci_workflow_inventory",
                Value::Array(workflows),
                &["ci", "protected"],
            ),
            (
                "ci_workflow_count",
                Value::from(scan.workflows.len() as u64),
                &["exploratory"],
            ),
            (
                "source_file_total",
                Value::from(scan.files.len() as u64),
                &["exploratory"],
            ),
            (
                "uninterpreted_file_total",
                Value::from(scan.uninterpreted_file_total()),
                &["exploratory"],
            ),
            ("doc_inventory", Value::Object(doc_inventory), &["exploratory"]),
            (
                "scan_loss_summary",
                Value::Object(loss_summary),
                &["loss", "protected"],
            ),
        ];
        for (variable, value, tags) in aggregates {
            facts.push(fact(
                format!("fact.aggregate.{variable}"),
                variable.to_string(),
                value,
                scope_pairs(&[("project", &scan.project), ("scan", SCAN_DIMENSION_VALUE)]),
                tags,
                format!("{}#aggregate={variable}", scan.project),
            ));
        }

        factors.push(factor(
            "factor.project_release_review".to_string(),
            DECISION_INPUTS.iter().map(|s| s.to_string()).collect(),
            vec!["release_integrity_status".to_string()],
            scope_pairs(&[("project", &scan.project)]),
            &["protected"],
        ));

        let mut issue_queries = BTreeMap::new();
        for issue in &options.issues {
            let (resolved, unresolved) = resolve_components(&issue.components, &components);
            let mut value = Map::new();
            value.insert("title".to_string(), Value::String(issue.title.clone()));
            if let Some(body) = &issue.body {
                value.insert("body".to_string(), Value::String(body.clone()));
            }
            value.insert(
                "components".to_string(),
                Value::Array(
                    resolved
                        .iter()
                        .map(|dir| Value::String(component_display(dir)))
                        .collect(),
                ),
            );
            value.insert(
                "unresolved_components".to_string(),
                Value::Array(unresolved.iter().cloned().map(Value::String).collect()),
            );
            facts.push(fact(
                format!("fact.issue.{}", issue.id),
                format!("issue_{}_record", issue.id),
                Value::Object(value),
                scope_pairs(&[("project", &scan.project), ("issue", &issue.id)]),
                &["issue"],
                format!("issues:{}", issue.id),
            ));

            let mut inputs: Vec<String> = resolved
                .iter()
                .map(|dir| format!("component_{}_inventory", component_slug(dir)))
                .collect();
            inputs.push(format!("issue_{}_record", issue.id));
            inputs.extend(DECISION_INPUTS.iter().map(|s| s.to_string()));
            inputs.sort();
            inputs.dedup();
            factors.push(factor(
                format!("factor.issue_{}_review", issue.id),
                inputs,
                vec![format!("issue_{}_context_status", issue.id)],
                scope_pairs(&[("project", &scan.project), ("issue", &issue.id)]),
                &[],
            ));

        }

        facts.sort_by_key(id_of);
        factors.sort_by_key(id_of);

        let event = {
            let mut map = Map::new();
            map.insert("id".to_string(), Value::String("event.scan".to_string()));
            map.insert(
                "event_time".to_string(),
                Value::String(decision_time.to_string()),
            );
            map.insert(
                "availability_time".to_string(),
                Value::String(decision_time.to_string()),
            );
            map.insert("causal_parents".to_string(), Value::Array(Vec::new()));
            map.insert(
                "produces".to_string(),
                Value::Array(
                    AGGREGATE_VARIABLES
                        .iter()
                        .map(|v| Value::String(v.to_string()))
                        .collect(),
                ),
            );
            Value::Object(map)
        };

        let fact_count = facts.len();
        let mut world = Map::new();
        world.insert(
            "schema_version".to_string(),
            Value::String("fiber-world/0.1".to_string()),
        );
        world.insert("world_id".to_string(), Value::String(world_id.clone()));
        world.insert(
            "description".to_string(),
            Value::String(format!(
                "Static-scan model of project {:?}: component inventories, dependency \
                 declarations, test and CI inventories, and the scan's own loss report as \
                 evidence.",
                scan.project
            )),
        );
        world.insert("facts".to_string(), Value::Array(facts));
        world.insert("factors".to_string(), Value::Array(factors));
        world.insert("events".to_string(), Value::Array(vec![event]));
        let world = Value::Object(world);

        World::from_json(world.clone())?;

        let release_query = packs::release_query(&world_id, decision_time, fact_count);
        for issue in &options.issues {
            issue_queries.insert(
                issue.id.clone(),
                packs::issue_query(&issue.id, &world_id, decision_time, fact_count),
            );
        }

        Ok(ProjectWorld {
            world_id,
            world,
            dimensions: packs::dimension_document(),
            pack: packs::release_readiness_pack(options.thresholds.todo_burden_at_least),
            release_query,
            issue_queries,
        })
    }
}

/// `project-` plus the first 12 hex digits of the digest of the canonical file listing.
fn derive_world_id(scan: &ProjectScan) -> Result<String, ProjectError> {
    let digest = ContentHash::of_value(&listing_value(&scan.files))
        .map_err(|error| ProjectError::Assembly(error.to_string()))?;
    Ok(format!("project-{}", &digest.as_str()[..12]))
}

/// Maps declared issue components to component directory keys.
///
/// An entry resolves when it names a component directory, its display name, its slug, or a
/// path inside a component directory (longest directory prefix wins, then the top-level
/// segment). Anything else lands in the unresolved list — declared on the issue fact, never
/// silently dropped, and never guessed at: relevance here comes only from declaration.
fn resolve_components(
    declared: &[String],
    components: &BTreeMap<String, ComponentSummary>,
) -> (Vec<String>, Vec<String>) {
    let mut resolved = BTreeSet::new();
    let mut unresolved = Vec::new();

    for raw in declared {
        let entry = raw.replace('\\', "/");
        let entry = entry
            .strip_prefix("./")
            .unwrap_or(&entry)
            .trim_end_matches('/');

        let direct = components.keys().find(|dir| {
            dir.as_str() == entry
                || component_display(dir) == entry
                || component_slug(dir) == entry
        });
        if let Some(dir) = direct {
            resolved.insert(dir.clone());
            continue;
        }

        let prefix = components
            .keys()
            .filter(|dir| !dir.is_empty() && entry.starts_with(&format!("{dir}/")))
            .max_by_key(|dir| dir.len());
        if let Some(dir) = prefix {
            resolved.insert(dir.clone());
            continue;
        }

        let top_level = entry.split('/').next().unwrap_or_default();
        if !top_level.is_empty() && entry.contains('/') && components.contains_key(top_level) {
            resolved.insert(top_level.to_string());
            continue;
        }

        unresolved.push(raw.clone());
    }

    (resolved.into_iter().collect(), unresolved)
}

fn id_of(document: &Value) -> String {
    document
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn scope_pairs(pairs: &[(&str, &str)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(dimension, value)| (dimension.to_string(), Value::String(value.to_string())))
            .collect(),
    )
}

fn fact(
    id: String,
    provides: String,
    value: Value,
    scope: Value,
    tags: &[&str],
    provenance: String,
) -> Value {
    let mut map = Map::new();
    map.insert("id".to_string(), Value::String(id));
    map.insert("provides".to_string(), Value::String(provides));
    map.insert("value".to_string(), value);
    map.insert("scope".to_string(), scope);
    map.insert(
        "tags".to_string(),
        Value::Array(tags.iter().map(|t| Value::String(t.to_string())).collect()),
    );
    map.insert(
        "provenance".to_string(),
        Value::Array(vec![Value::String(provenance)]),
    );
    Value::Object(map)
}

fn factor(
    id: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    scope: Value,
    tags: &[&str],
) -> Value {
    let mut map = Map::new();
    map.insert("id".to_string(), Value::String(id));
    map.insert(
        "inputs".to_string(),
        Value::Array(inputs.into_iter().map(Value::String).collect()),
    );
    map.insert(
        "outputs".to_string(),
        Value::Array(outputs.into_iter().map(Value::String).collect()),
    );
    map.insert(
        "kind".to_string(),
        Value::String("deterministic_rule".to_string()),
    );
    map.insert("cost".to_string(), Value::from(1.0));
    map.insert("scope".to_string(), scope);
    map.insert(
        "tags".to_string(),
        Value::Array(tags.iter().map(|t| Value::String(t.to_string())).collect()),
    );
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_components(dirs: &[&str]) -> BTreeMap<String, ComponentSummary> {
        dirs.iter()
            .map(|dir| {
                (
                    dir.to_string(),
                    ComponentSummary {
                        files: 0,
                        lines: 0,
                        todo_markers: 0,
                        fixme_markers: 0,
                        unimplemented_markers: 0,
                        test_functions: 0,
                        uninterpreted_files: 0,
                        listing: Vec::new(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn a_file_path_resolves_to_its_component_and_an_unknown_name_stays_unresolved() {
        let components = empty_components(&["", "src", ".github"]);
        let (resolved, unresolved) = resolve_components(
            &["src/lib.rs".to_string(), "nonexistent-module".to_string()],
            &components,
        );
        assert_eq!(resolved, vec!["src".to_string()]);
        assert_eq!(unresolved, vec!["nonexistent-module".to_string()]);
    }

    #[test]
    fn a_nested_component_wins_over_its_top_level_directory_by_longest_prefix() {
        let components = empty_components(&["crates", "crates/adapter"]);
        let (resolved, unresolved) =
            resolve_components(&["crates/adapter/src/lib.rs".to_string()], &components);
        assert_eq!(resolved, vec!["crates/adapter".to_string()]);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn the_root_component_is_addressable_by_its_display_name() {
        let components = empty_components(&["", "src"]);
        let (resolved, unresolved) = resolve_components(&["root".to_string()], &components);
        assert_eq!(resolved, vec![String::new()]);
        assert!(unresolved.is_empty());
    }
}
