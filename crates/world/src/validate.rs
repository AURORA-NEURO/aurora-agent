//! Structural diagnostics beyond reference compatibility.
//!
//! Everything here is *strictly* advisory: none of it may reject a world the CPython reference
//! accepts. Blueprint 40.19 asks for leakage canaries and graph-integrity rules, and 43.37
//! insists that structural limits be first-class outputs rather than silent behaviour, so these
//! checks surface conditions that would otherwise degrade a compile quietly.

use crate::world::World;
use bioprism_scope::DimensionRegistry;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn with_code<'a>(&'a self, code: &'a str) -> impl Iterator<Item = &'a Diagnostic> + 'a {
        self.diagnostics.iter().filter(move |d| d.code == code)
    }

    fn push(&mut self, severity: Severity, code: &'static str, subject: String, message: String) {
        self.diagnostics.push(Diagnostic { severity, code, subject, message });
    }
}

pub fn validate(world: &World, registry: &DimensionRegistry) -> ValidationReport {
    let mut report = ValidationReport::default();

    for variable in &world.index().shadowed_variables {
        report.push(
            Severity::Error,
            "shadowed_variable",
            variable.clone(),
            format!(
                "more than one fact provides {variable:?}; the last one silently wins, so the \
                 compiled decision section depends on document order"
            ),
        );
    }

    let provided: BTreeSet<&str> = world.facts.iter().map(|f| f.provides.as_str()).collect();
    let produced: BTreeSet<&str> = world
        .factors
        .iter()
        .flat_map(|f| f.outputs.iter().map(|o| o.as_str()))
        .collect();

    let known_events: BTreeSet<&str> = world.events.iter().map(|e| e.id.as_str()).collect();
    for event in &world.events {
        for parent in &event.causal_parents {
            if !known_events.contains(parent.as_str()) {
                report.push(
                    Severity::Error,
                    "dangling_causal_parent",
                    event.id.as_str().to_string(),
                    format!("causal parent {} does not exist", parent.as_str()),
                );
            }
        }
        if event.is_backdated() {
            report.push(
                Severity::Warning,
                "backdated_event",
                event.id.as_str().to_string(),
                format!(
                    "availability_time {} precedes event_time {}",
                    event.availability_time, event.event_time
                ),
            );
        }
        for variable in &event.produces {
            if !provided.contains(variable.as_str()) && !produced.contains(variable.as_str()) {
                report.push(
                    Severity::Warning,
                    "event_produces_unknown_variable",
                    event.id.as_str().to_string(),
                    format!(
                        "event produces {:?}, which no fact provides and no factor outputs; the \
                         temporal cut can never admit it",
                        variable.as_str()
                    ),
                );
            }
        }
    }

    for factor in &world.factors {
        if factor.outputs.is_empty() {
            report.push(
                Severity::Warning,
                "factor_without_outputs",
                factor.id.as_str().to_string(),
                "factor produces nothing and can never be reached by backward slicing".into(),
            );
        }
        if let Some(scope) = &factor.scope {
            for dimension in scope.unclassified_dimensions(registry) {
                report.push(
                    Severity::Warning,
                    "unclassified_scope_dimension",
                    factor.id.as_str().to_string(),
                    format!(
                        "scope dimension {dimension:?} has no class; protected closure is \
                         defined per class and cannot be proven over it"
                    ),
                );
            }
        }
    }

    for fact in &world.facts {
        for dimension in fact.scope.unclassified_dimensions(registry) {
            report.push(
                Severity::Warning,
                "unclassified_scope_dimension",
                fact.id.as_str().to_string(),
                format!(
                    "scope dimension {dimension:?} has no class; protected closure is defined \
                     per class and cannot be proven over it"
                ),
            );
        }
    }

    for cycle in find_factor_cycles(world) {
        report.push(
            Severity::Warning,
            "factor_cycle",
            cycle.join(" -> "),
            "factors form a dependency cycle; backward slicing terminates but the compiled \
             region has no acyclic elimination order"
                .into(),
        );
    }

    report
}

/// Finds cycles in the variable dependency graph induced by factors.
///
/// Backward slicing is protected by a visited set and terminates regardless, but a cycle means
/// no elimination order exists, which the physical planner must know about (43.18).
fn find_factor_cycles(world: &World) -> Vec<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unvisited,
        InProgress,
        Done,
    }

    let variables: Vec<&str> = {
        let mut set = BTreeSet::new();
        for factor in &world.factors {
            for v in factor.inputs.iter().chain(factor.outputs.iter()) {
                set.insert(v.as_str());
            }
        }
        set.into_iter().collect()
    };
    let position = |name: &str| variables.iter().position(|v| *v == name);

    let mut marks = vec![Mark::Unvisited; variables.len()];
    let mut cycles = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    fn visit(
        node: usize,
        world: &World,
        variables: &[&str],
        marks: &mut [Mark],
        stack: &mut Vec<usize>,
        cycles: &mut Vec<Vec<String>>,
        position: &impl Fn(&str) -> Option<usize>,
    ) {
        marks[node] = Mark::InProgress;
        stack.push(node);
        for factor in world.producers_of(variables[node]) {
            for input in &factor.inputs {
                if let Some(next) = position(input.as_str()) {
                    match marks[next] {
                        Mark::InProgress => {
                            let start = stack.iter().position(|n| *n == next).unwrap_or(0);
                            let mut cycle: Vec<String> = stack[start..]
                                .iter()
                                .map(|n| variables[*n].to_string())
                                .collect();
                            cycle.push(variables[next].to_string());
                            cycles.push(cycle);
                        }
                        Mark::Unvisited => {
                            visit(next, world, variables, marks, stack, cycles, position)
                        }
                        Mark::Done => {}
                    }
                }
            }
        }
        stack.pop();
        marks[node] = Mark::Done;
    }

    for node in 0..variables.len() {
        if marks[node] == Mark::Unvisited {
            visit(
                node, world, &variables, &mut marks, &mut stack, &mut cycles, &position,
            );
        }
    }

    cycles
}
