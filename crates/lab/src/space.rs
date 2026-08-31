//! The architecture search space, and what is not in it.
//!
//! Blueprint 09.04 names fourteen component kinds and a parameter list, then constrains candidates
//! to "typed interfaces, acyclic or bounded-control graph, effect permissions, maximum cost,
//! provider availability, data policy, and protected components". Blueprint 09.10 names the
//! complement — "permission core, audit, secrets, benchmark splits, and release rules are
//! protected" — and it is the complement that carries the weight. A search space that can reach
//! the benchmark splits is a search space that will eventually optimize them.
//!
//! A [`CandidateArchitecture`] here doubles as the immutable configuration bundle of 09.11:
//! the same identifier that names a point in the search space names the thing a holdout records
//! exposure against, and the thing a rollback restores. They are one object because the moment
//! they are two, an exposure ledger and a deployment can disagree about which configuration was
//! measured.
//!
//! # Lineage is the load-bearing field
//!
//! [`CandidateArchitecture::derived_from`] exists so [`ArchitectureSpace::lineage`] can answer
//! "what was this built out of". Holdout contamination travels down that chain: if a holdout was
//! used to select a parent, a child tuned from the parent inherits the burn even though the child
//! itself was never scored. Without lineage, the cheapest way to launder a burned holdout is to
//! rename the configuration.
//!
//! # Not implemented, deliberately
//!
//! No component *implementations* — a [`ComponentSpec`] is a declaration, and nothing in this
//! crate can run one. No provider catalogue, no availability check, no cost model: `cost_units`
//! is a number the caller declares and this crate only compares against a ceiling. No candidate
//! *generation*: 09.04 asks for a generator over the space and there is none here, because a
//! generator without an executor produces candidates nobody can evaluate.

use crate::error::SpaceError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Identifier of one immutable configuration bundle.
///
/// Immutable in the 09.11 sense: the same id always denotes the same components and parameters.
/// [`ArchitectureSpace::register`] refuses to rebind one, so "we rolled back to `v3`" names a
/// determinate thing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigurationId(String);

impl ConfigurationId {
    pub fn new(id: impl Into<String>) -> Self {
        ConfigurationId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConfigurationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The fourteen component kinds of 09.04, plus the router 09.08 owns.
///
/// Closed on purpose. An open set would let a candidate declare a component kind whose
/// compatibility rules nobody wrote, and compatibility is the only thing that makes two candidates
/// comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    ObservationParser,
    ContextSelector,
    Compressor,
    HypothesisGenerator,
    Planner,
    ModelPolicy,
    ToolRouter,
    Executor,
    Verifier,
    MemoryReader,
    MemoryWriter,
    BranchController,
    RetryController,
    Terminator,
    /// The evaluation-conditioned router. Implemented in `bioprism-routing`, declared here.
    Router,
}

impl ComponentKind {
    pub const ALL: [ComponentKind; 15] = [
        ComponentKind::ObservationParser,
        ComponentKind::ContextSelector,
        ComponentKind::Compressor,
        ComponentKind::HypothesisGenerator,
        ComponentKind::Planner,
        ComponentKind::ModelPolicy,
        ComponentKind::ToolRouter,
        ComponentKind::Executor,
        ComponentKind::Verifier,
        ComponentKind::MemoryReader,
        ComponentKind::MemoryWriter,
        ComponentKind::BranchController,
        ComponentKind::RetryController,
        ComponentKind::Terminator,
        ComponentKind::Router,
    ];

    /// Kinds without which a candidate cannot be executed at all, so comparing it against one that
    /// has them is comparing two different things.
    pub const REQUIRED: [ComponentKind; 3] = [
        ComponentKind::ContextSelector,
        ComponentKind::Executor,
        ComponentKind::Terminator,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ComponentKind::ObservationParser => "observation_parser",
            ComponentKind::ContextSelector => "context_selector",
            ComponentKind::Compressor => "compressor",
            ComponentKind::HypothesisGenerator => "hypothesis_generator",
            ComponentKind::Planner => "planner",
            ComponentKind::ModelPolicy => "model_policy",
            ComponentKind::ToolRouter => "tool_router",
            ComponentKind::Executor => "executor",
            ComponentKind::Verifier => "verifier",
            ComponentKind::MemoryReader => "memory_reader",
            ComponentKind::MemoryWriter => "memory_writer",
            ComponentKind::BranchController => "branch_controller",
            ComponentKind::RetryController => "retry_controller",
            ComponentKind::Terminator => "terminator",
            ComponentKind::Router => "router",
        }
    }
}

/// Surfaces 09.10 places outside the evolvable set.
///
/// `BenchmarkSplits` is the one this crate is built around. If a proposal can move which tasks are
/// in the holdout, every guarantee in [`crate::holdout`] is decorative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedSurface {
    PermissionCore,
    AuditLog,
    Secrets,
    BenchmarkSplits,
    ReleaseRules,
}

impl ProtectedSurface {
    pub const ALL: [ProtectedSurface; 5] = [
        ProtectedSurface::PermissionCore,
        ProtectedSurface::AuditLog,
        ProtectedSurface::Secrets,
        ProtectedSurface::BenchmarkSplits,
        ProtectedSurface::ReleaseRules,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProtectedSurface::PermissionCore => "permission_core",
            ProtectedSurface::AuditLog => "audit_log",
            ProtectedSurface::Secrets => "secrets",
            ProtectedSurface::BenchmarkSplits => "benchmark_splits",
            ProtectedSurface::ReleaseRules => "release_rules",
        }
    }

    /// Why this surface is protected, in the sentence a reviewer needs when a proposal is refused.
    pub fn rationale(self) -> &'static str {
        match self {
            ProtectedSurface::PermissionCore => {
                "a system that can widen its own permissions has no permission model"
            }
            ProtectedSurface::AuditLog => {
                "a system that can rewrite the audit log cannot be audited"
            }
            ProtectedSurface::Secrets => {
                "credential handling is not a performance axis and must not be traded against one"
            }
            ProtectedSurface::BenchmarkSplits => {
                "a system that can move the holdout will move it until it is not a holdout"
            }
            ProtectedSurface::ReleaseRules => {
                "the gate that decides what ships cannot be an output of what is being gated"
            }
        }
    }
}

/// A declared parameter value. Deliberately not `serde_json::Value`: a parameter set that can hold
/// arbitrary JSON is a parameter set two candidates cannot be diffed against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum ParameterValue {
    Text(String),
    Integer(i64),
    Real(f64),
    Flag(bool),
}

impl ParameterValue {
    pub fn render(&self) -> String {
        match self {
            ParameterValue::Text(text) => text.clone(),
            ParameterValue::Integer(value) => value.to_string(),
            ParameterValue::Real(value) => format!("{value}"),
            ParameterValue::Flag(value) => value.to_string(),
        }
    }
}

/// One declared component of a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub id: String,
    pub kind: ComponentKind,
    /// The 09.04 parameter list — prompts, tool visibility, branch count, context budget,
    /// retrieval objective, memory policy, verification depth, retry limits, thresholds.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
    /// Components this one feeds. Edges are validated against the candidate, not assumed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feeds: Vec<String>,
}

impl ComponentSpec {
    pub fn new(id: impl Into<String>, kind: ComponentKind) -> Self {
        ComponentSpec {
            id: id.into(),
            kind,
            parameters: BTreeMap::new(),
            feeds: Vec::new(),
        }
    }

    pub fn with_parameter(mut self, key: impl Into<String>, value: ParameterValue) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    pub fn feeding<I, S>(mut self, targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.feeds = targets.into_iter().map(Into::into).collect();
        self
    }
}

/// One point in the search space and one immutable configuration bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateArchitecture {
    pub id: ConfigurationId,
    /// The configuration this one was derived from, if any. Contamination travels this edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<ConfigurationId>,
    pub components: Vec<ComponentSpec>,
    /// Declared cost in whatever unit the caller's ceiling is stated in. Not estimated here.
    pub cost_units: u64,
    /// Protected surfaces the candidate admits to touching. Populated by the proposer; a candidate
    /// that lies about this is outside what a type can catch, and 09.10's reviewer gate is the
    /// backstop the blueprint names for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touches_protected: Vec<ProtectedSurface>,
}

impl CandidateArchitecture {
    pub fn new(id: impl Into<String>) -> Self {
        CandidateArchitecture {
            id: ConfigurationId::new(id),
            derived_from: None,
            components: Vec::new(),
            cost_units: 0,
            touches_protected: Vec::new(),
        }
    }

    pub fn derived_from(mut self, parent: impl Into<String>) -> Self {
        self.derived_from = Some(ConfigurationId::new(parent));
        self
    }

    pub fn with_component(mut self, component: ComponentSpec) -> Self {
        self.components.push(component);
        self
    }

    pub fn costing(mut self, cost_units: u64) -> Self {
        self.cost_units = cost_units;
        self
    }

    pub fn touching(mut self, surface: ProtectedSurface) -> Self {
        self.touches_protected.push(surface);
        self
    }

    pub fn component(&self, id: &str) -> Option<&ComponentSpec> {
        self.components.iter().find(|spec| spec.id == id)
    }

    pub fn kinds(&self) -> BTreeSet<ComponentKind> {
        self.components.iter().map(|spec| spec.kind).collect()
    }

    /// Rejects duplicates, dangling edges, cycles, missing required kinds, a cost over the ceiling,
    /// and any declared protected surface.
    ///
    /// All five are fatal rather than repairable. A candidate with a cycle has no execution order;
    /// a candidate missing a terminator does not halt; a candidate that touches a protected surface
    /// is not a candidate but a policy change wearing a candidate's clothes.
    pub fn validate(&self, cost_ceiling: u64) -> Result<(), SpaceError> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for spec in &self.components {
            if !seen.insert(spec.id.as_str()) {
                return Err(SpaceError::DuplicateComponent {
                    component: spec.id.clone(),
                });
            }
        }
        for spec in &self.components {
            for target in &spec.feeds {
                if !seen.contains(target.as_str()) {
                    return Err(SpaceError::DanglingEdge {
                        component: spec.id.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
        self.check_acyclic()?;

        let kinds = self.kinds();
        for required in ComponentKind::REQUIRED {
            if !kinds.contains(&required) {
                return Err(SpaceError::MissingRequiredComponent {
                    candidate: self.id.to_string(),
                    kind: required.as_str().to_string(),
                });
            }
        }
        if let Some(surface) = self.touches_protected.first() {
            return Err(SpaceError::ProtectedSurfaceTouched {
                candidate: self.id.to_string(),
                surface: surface.as_str().to_string(),
            });
        }
        if self.cost_units > cost_ceiling {
            return Err(SpaceError::CostCeilingExceeded {
                candidate: self.id.to_string(),
                cost_units: self.cost_units,
                ceiling: cost_ceiling,
            });
        }
        Ok(())
    }

    fn check_acyclic(&self) -> Result<(), SpaceError> {
        let mut indegree: BTreeMap<&str, usize> = self
            .components
            .iter()
            .map(|spec| (spec.id.as_str(), 0usize))
            .collect();
        for spec in &self.components {
            for target in &spec.feeds {
                if let Some(degree) = indegree.get_mut(target.as_str()) {
                    *degree += 1;
                }
            }
        }
        let mut ready: VecDeque<&str> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut visited = 0usize;
        while let Some(id) = ready.pop_front() {
            visited += 1;
            let Some(spec) = self.components.iter().find(|spec| spec.id == id) else {
                return Err(SpaceError::InvariantViolation(format!(
                    "topological queue referenced missing component `{id}`"
                )));
            };
            for target in &spec.feeds {
                if let Some(degree) = indegree.get_mut(target.as_str()) {
                    let Some(next) = degree.checked_sub(1) else {
                        return Err(SpaceError::InvariantViolation(format!(
                            "indegree underflow while visiting `{id}` -> `{target}`"
                        )));
                    };
                    *degree = next;
                    if *degree == 0 {
                        ready.push_back(target.as_str());
                    }
                }
            }
        }
        if visited != self.components.len() {
            let stuck = indegree
                .iter()
                .filter(|(_, degree)| **degree > 0)
                .map(|(id, _)| *id)
                .next()
                .unwrap_or("<unknown>");
            return Err(SpaceError::ComponentCycle(stuck.to_string()));
        }
        Ok(())
    }

    /// Component ids and parameters whose values differ from `other`.
    ///
    /// This is what an evolution card's "what changed" field is filled from, and what
    /// [`crate::evolution`] needs in order to refuse a card that claims a change it cannot name.
    pub fn diff(&self, other: &CandidateArchitecture) -> Vec<String> {
        let mut changes = Vec::new();
        let mine: BTreeMap<&str, &ComponentSpec> = self
            .components
            .iter()
            .map(|spec| (spec.id.as_str(), spec))
            .collect();
        let theirs: BTreeMap<&str, &ComponentSpec> = other
            .components
            .iter()
            .map(|spec| (spec.id.as_str(), spec))
            .collect();
        for (id, spec) in &theirs {
            match mine.get(id) {
                None => changes.push(format!("added component `{id}` ({})", spec.kind.as_str())),
                Some(existing) => {
                    if existing.kind != spec.kind {
                        changes.push(format!(
                            "component `{id}` kind {} -> {}",
                            existing.kind.as_str(),
                            spec.kind.as_str()
                        ));
                    }
                    for (key, value) in &spec.parameters {
                        match existing.parameters.get(key) {
                            None => changes.push(format!(
                                "component `{id}` gained parameter `{key}` = {}",
                                value.render()
                            )),
                            Some(before) if before != value => changes.push(format!(
                                "component `{id}` parameter `{key}` {} -> {}",
                                before.render(),
                                value.render()
                            )),
                            Some(_) => {}
                        }
                    }
                    for key in existing.parameters.keys() {
                        if !spec.parameters.contains_key(key) {
                            changes.push(format!("component `{id}` lost parameter `{key}`"));
                        }
                    }
                }
            }
        }
        for id in mine.keys() {
            if !theirs.contains_key(id) {
                changes.push(format!("removed component `{id}`"));
            }
        }
        if self.cost_units != other.cost_units {
            changes.push(format!(
                "cost_units {} -> {}",
                self.cost_units, other.cost_units
            ));
        }
        changes.sort();
        changes
    }
}

/// The registry of immutable configuration bundles.
///
/// 09.11: "Architecture, router, model resolution, policy, and component artifacts are immutable
/// versions. Rollback restores a complete known-good bundle, not selected files." There is
/// therefore no method here that mutates a registered bundle — only [`ArchitectureSpace::register`]
/// and reads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureSpace {
    bundles: BTreeMap<ConfigurationId, CandidateArchitecture>,
}

impl ArchitectureSpace {
    pub fn new() -> Self {
        ArchitectureSpace::default()
    }

    /// Registers a bundle, refusing a rebind and an unregistered parent.
    ///
    /// The parent check is what makes [`ArchitectureSpace::lineage`] total: a chain that reaches an
    /// id nobody registered would silently stop, and a silently truncated lineage is a
    /// contamination check that returns clean.
    pub fn register(&mut self, candidate: CandidateArchitecture) -> Result<(), SpaceError> {
        if self.bundles.contains_key(&candidate.id) {
            return Err(SpaceError::DuplicateConfiguration(candidate.id.to_string()));
        }
        if let Some(parent) = &candidate.derived_from {
            if !self.bundles.contains_key(parent) {
                return Err(SpaceError::UnregisteredParent {
                    candidate: candidate.id.to_string(),
                    parent: parent.to_string(),
                });
            }
        }
        self.bundles.insert(candidate.id.clone(), candidate);
        Ok(())
    }

    pub fn get(&self, id: &ConfigurationId) -> Option<&CandidateArchitecture> {
        self.bundles.get(id)
    }

    pub fn contains(&self, id: &ConfigurationId) -> bool {
        self.bundles.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.bundles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bundles.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CandidateArchitecture> {
        self.bundles.values()
    }

    /// `id` first, then its ancestors oldest-last.
    ///
    /// Walks stored parent edges rather than trusting a cached root, and treats a cycle as a typed
    /// error rather than looping. Registration forbids both a cycle and a dangling parent, so a
    /// failure here means the registry was deserialized from something that skipped those checks.
    pub fn lineage(&self, id: &ConfigurationId) -> Result<Vec<ConfigurationId>, SpaceError> {
        let mut chain = Vec::new();
        let mut seen: BTreeSet<ConfigurationId> = BTreeSet::new();
        let mut current = Some(id.clone());
        while let Some(step) = current {
            if !seen.insert(step.clone()) {
                return Err(SpaceError::LineageCycle(step.to_string()));
            }
            let bundle = self
                .bundles
                .get(&step)
                .ok_or_else(|| SpaceError::UnknownConfiguration(step.to_string()))?;
            chain.push(step);
            current = bundle.derived_from.clone();
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal(id: &str) -> CandidateArchitecture {
        CandidateArchitecture::new(id)
            .with_component(
                ComponentSpec::new("select", ComponentKind::ContextSelector).feeding(["run"]),
            )
            .with_component(ComponentSpec::new("run", ComponentKind::Executor).feeding(["stop"]))
            .with_component(ComponentSpec::new("stop", ComponentKind::Terminator))
    }

    #[test]
    fn a_candidate_without_a_terminator_is_refused_rather_than_defaulted() {
        let candidate = CandidateArchitecture::new("c")
            .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
            .with_component(ComponentSpec::new("run", ComponentKind::Executor));
        assert_eq!(
            candidate.validate(100),
            Err(SpaceError::MissingRequiredComponent {
                candidate: "c".to_string(),
                kind: "terminator".to_string(),
            })
        );
    }

    #[test]
    fn a_candidate_that_touches_the_benchmark_splits_is_not_a_candidate() {
        let candidate = minimal("c").touching(ProtectedSurface::BenchmarkSplits);
        let error = candidate.validate(100).unwrap_err();
        assert!(matches!(
            error,
            SpaceError::ProtectedSurfaceTouched { ref surface, .. } if surface == "benchmark_splits"
        ));
    }

    #[test]
    fn a_component_cycle_is_an_error_rather_than_an_arbitrary_order() {
        let candidate = CandidateArchitecture::new("c")
            .with_component(
                ComponentSpec::new("select", ComponentKind::ContextSelector).feeding(["run"]),
            )
            .with_component(ComponentSpec::new("run", ComponentKind::Executor).feeding(["select"]))
            .with_component(ComponentSpec::new("stop", ComponentKind::Terminator));
        assert!(matches!(
            candidate.validate(100),
            Err(SpaceError::ComponentCycle(_))
        ));
    }

    #[test]
    fn an_edge_to_a_component_that_is_not_in_the_candidate_is_dangling() {
        let candidate = minimal("c").with_component(
            ComponentSpec::new("verify", ComponentKind::Verifier).feeding(["nowhere"]),
        );
        assert_eq!(
            candidate.validate(100),
            Err(SpaceError::DanglingEdge {
                component: "verify".to_string(),
                target: "nowhere".to_string(),
            })
        );
    }

    #[test]
    fn a_registered_configuration_id_cannot_be_rebound_to_different_components() {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        assert_eq!(
            space.register(minimal("v1").costing(9)),
            Err(SpaceError::DuplicateConfiguration("v1".to_string()))
        );
    }

    #[test]
    fn lineage_reaches_every_ancestor_so_contamination_cannot_be_renamed_away() {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v2").derived_from("v1")).unwrap();
        space.register(minimal("v3").derived_from("v2")).unwrap();
        let chain = space.lineage(&ConfigurationId::new("v3")).unwrap();
        assert_eq!(
            chain,
            vec![
                ConfigurationId::new("v3"),
                ConfigurationId::new("v2"),
                ConfigurationId::new("v1")
            ]
        );
    }

    #[test]
    fn a_candidate_whose_parent_is_unregistered_is_refused_so_lineage_stays_total() {
        let mut space = ArchitectureSpace::new();
        assert_eq!(
            space.register(minimal("v2").derived_from("ghost")),
            Err(SpaceError::UnregisteredParent {
                candidate: "v2".to_string(),
                parent: "ghost".to_string(),
            })
        );
    }

    #[test]
    fn a_diff_names_the_parameter_that_changed_rather_than_reporting_that_something_did() {
        let before = minimal("v1").with_component(
            ComponentSpec::new("branch", ComponentKind::BranchController)
                .with_parameter("max_branches", ParameterValue::Integer(2)),
        );
        let after = minimal("v2").with_component(
            ComponentSpec::new("branch", ComponentKind::BranchController)
                .with_parameter("max_branches", ParameterValue::Integer(5)),
        );
        assert_eq!(
            before.diff(&after),
            vec!["component `branch` parameter `max_branches` 2 -> 5".to_string()]
        );
    }
}
