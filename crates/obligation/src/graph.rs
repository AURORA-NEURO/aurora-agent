//! The decision obligation graph.
//!
//! Blueprint 39.06: *a goal is compiled into explicit obligations rather than a vague
//! instruction*, and those obligations form a dependency graph. The worked example is a
//! radiogenomic validation audit whose "modeling allowed" node sits downstream of identity,
//! temporal firewall, split independence, measurement provenance and estimand obligations.
//!
//! The load-bearing operation is [`ObligationGraph::effective_states`], which is why the
//! dependency edges exist at all. An obligation recorded as `satisfied` while a precondition it
//! rests on is still `open` is *not* satisfied for any purpose a gate cares about, and reporting
//! it as satisfied is how a dependency graph degenerates into a checklist. Effective state is
//! computed over the dependencies and is what [`crate::gate`] reads.
//!
//! History is append-only. A state is never overwritten, because the audit question is usually
//! "when did this stop being open, and who said so", not "what is it now".
//!
//! Not implemented here: obligation *compilation*. Turning a natural-language goal into this
//! graph is 39.02's evidence-graph compiler; this crate takes the graph as given and enforces its
//! semantics.

use crate::error::ObligationError;
use crate::invariants::ProtectedClass;
use crate::state::{ObligationState, StateRecord};
use bioprism_ids::{CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};

/// One explicit obligation compiled from a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Obligation {
    pub id: String,
    /// What must be true, in the auditor's words. Never a state name.
    pub statement: String,
    /// Obligations that must hold before this one can mean anything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Protected classes of 39.05 this obligation is responsible for keeping in the projection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_classes: Vec<ProtectedClass>,
    /// Decision value used by the budget controller when allocating tokens.
    pub value: f64,
    /// Mandatory obligations belong to the closure that 39.16 forbids trading away for tokens.
    pub mandatory: bool,
    history: Vec<StateRecord>,
}

impl Obligation {
    pub fn new(id: impl Into<String>, statement: impl Into<String>) -> Self {
        Obligation {
            id: id.into(),
            statement: statement.into(),
            depends_on: Vec::new(),
            protected_classes: Vec::new(),
            value: 1.0,
            mandatory: false,
            history: Vec::new(),
        }
    }

    pub fn depending_on<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.depends_on = dependencies.into_iter().map(Into::into).collect();
        self
    }

    pub fn protecting<I>(mut self, classes: I) -> Self
    where
        I: IntoIterator<Item = ProtectedClass>,
    {
        self.protected_classes = classes.into_iter().collect();
        self
    }

    pub fn with_value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    pub fn mandatory(mut self) -> Self {
        self.mandatory = true;
        self
    }

    /// The most recent record, or `None` while the obligation is unseen.
    pub fn current(&self) -> Option<&StateRecord> {
        self.history.last()
    }

    /// The recorded state, before dependencies are taken into account.
    pub fn recorded_state(&self) -> ObligationState {
        self.current()
            .map_or(ObligationState::Unseen, |record| record.state)
    }

    /// The recording actor's confidence, `0.0` while unseen.
    pub fn confidence(&self) -> f64 {
        self.current().map_or(0.0, |record| record.confidence)
    }

    pub fn history(&self) -> &[StateRecord] {
        &self.history
    }

    /// Every evidence locator ever cited for this obligation, newest last.
    pub fn evidence(&self) -> Vec<&str> {
        self.history
            .iter()
            .flat_map(|record| record.evidence.iter().map(String::as_str))
            .collect()
    }

    /// Appends a validated transition.
    pub fn record(&mut self, record: StateRecord) -> Result<(), ObligationError> {
        record.validate(&self.id)?;
        self.history.push(record);
        Ok(())
    }
}

/// A goal compiled into obligations with dependencies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObligationGraph {
    pub goal: String,
    obligations: BTreeMap<String, Obligation>,
}

impl ObligationGraph {
    pub fn new(goal: impl Into<String>) -> Self {
        ObligationGraph {
            goal: goal.into(),
            obligations: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, obligation: Obligation) -> Result<(), ObligationError> {
        if !obligation.value.is_finite() {
            return Err(ObligationError::NonFiniteValue {
                obligation: obligation.id.clone(),
                value: obligation.value,
            });
        }
        if self.obligations.contains_key(&obligation.id) {
            return Err(ObligationError::DuplicateObligation(obligation.id));
        }
        self.obligations.insert(obligation.id.clone(), obligation);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Obligation> {
        self.obligations.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Obligation> {
        self.obligations.values()
    }

    pub fn len(&self) -> usize {
        self.obligations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.obligations.is_empty()
    }

    pub fn mandatory_ids(&self) -> Vec<&str> {
        self.obligations
            .values()
            .filter(|obligation| obligation.mandatory)
            .map(|obligation| obligation.id.as_str())
            .collect()
    }

    /// Appends a state record to one obligation.
    pub fn record(&mut self, id: &str, record: StateRecord) -> Result<(), ObligationError> {
        let obligation = self
            .obligations
            .get_mut(id)
            .ok_or_else(|| ObligationError::UnknownObligation(id.to_string()))?;
        obligation.record(record)
    }

    /// Rejects dangling dependencies and cycles.
    ///
    /// Both are fatal rather than repairable: a dangling dependency means an obligation the goal
    /// declared is missing from the graph, and a cycle means no obligation in it can ever be
    /// established without assuming itself.
    pub fn validate(&self) -> Result<(), ObligationError> {
        self.check_references()?;
        self.topological_order().map(|_| ())
    }

    fn check_references(&self) -> Result<(), ObligationError> {
        for obligation in self.obligations.values() {
            for dependency in &obligation.depends_on {
                if !self.obligations.contains_key(dependency) {
                    return Err(ObligationError::DanglingDependency {
                        obligation: obligation.id.clone(),
                        missing: dependency.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Dependencies before dependents, deterministic under identifier order.
    pub fn topological_order(&self) -> Result<Vec<&str>, ObligationError> {
        self.check_references()?;

        let mut indegree: BTreeMap<&str, usize> = self
            .obligations
            .keys()
            .map(|id| (id.as_str(), 0usize))
            .collect();
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for obligation in self.obligations.values() {
            for dependency in &obligation.depends_on {
                dependents
                    .entry(dependency.as_str())
                    .or_default()
                    .push(obligation.id.as_str());
                *indegree.entry(obligation.id.as_str()).or_default() += 1;
            }
        }

        let mut ready: VecDeque<&str> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut ordered = Vec::with_capacity(self.obligations.len());
        while let Some(id) = ready.pop_front() {
            ordered.push(id);
            for dependent in dependents.get(id).into_iter().flatten() {
                let degree = indegree.get_mut(dependent).expect("dependent is indexed");
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(dependent);
                }
            }
        }

        if ordered.len() != self.obligations.len() {
            let stuck = indegree
                .iter()
                .filter(|(_, degree)| **degree > 0)
                .map(|(id, _)| *id)
                .next()
                .unwrap_or("<unknown>");
            return Err(ObligationError::DependencyCycle(stuck.to_string()));
        }
        Ok(ordered)
    }

    /// The state of every obligation once its dependencies are taken into account.
    ///
    /// The rules, in the order they apply:
    ///
    /// - `not_applicable` is stable. An obligation that does not apply cannot be weakened by
    ///   anything upstream.
    /// - a `contradicted` dependency contradicts its dependents. If the precondition is refuted,
    ///   so is everything resting on it.
    /// - an `unresolvable` dependency makes its dependents no stronger than unresolvable.
    /// - any other undischarged dependency caps its dependents at `partially_supported`. This is
    ///   the rule that stops a checklist from reporting green.
    pub fn effective_states(&self) -> Result<BTreeMap<String, ObligationState>, ObligationError> {
        let order = self.topological_order()?;
        let mut effective: BTreeMap<String, ObligationState> = BTreeMap::new();
        for id in order {
            let obligation = self.obligations.get(id).expect("ordered id is present");
            let own = obligation.recorded_state();
            let state = if own == ObligationState::NotApplicable {
                own
            } else {
                let worst = obligation
                    .depends_on
                    .iter()
                    .filter_map(|dependency| effective.get(dependency).copied())
                    .reduce(ObligationState::weaker_of);
                match worst {
                    None => own,
                    Some(ObligationState::Contradicted) => ObligationState::Contradicted,
                    Some(ObligationState::Unresolvable) => {
                        own.weaker_of(ObligationState::Unresolvable)
                    }
                    Some(ObligationState::WaivedWithReason) => {
                        own.weaker_of(ObligationState::WaivedWithReason)
                    }
                    Some(ObligationState::Satisfied | ObligationState::NotApplicable) => own,
                    Some(_) => own.weaker_of(ObligationState::PartiallySupported),
                }
            };
            effective.insert(id.to_string(), state);
        }
        Ok(effective)
    }

    pub fn effective_state(&self, id: &str) -> Result<ObligationState, ObligationError> {
        self.effective_states()?
            .get(id)
            .copied()
            .ok_or_else(|| ObligationError::UnknownObligation(id.to_string()))
    }

    /// Obligations that are not evidentially discharged but whose dependencies are.
    ///
    /// This is the `obligation_frontier` a BioContext capsule carries (39.03): the work that can
    /// actually be done next, highest decision value first.
    pub fn frontier(&self) -> Result<Vec<FrontierEntry>, ObligationError> {
        let effective = self.effective_states()?;
        let mut frontier: Vec<FrontierEntry> = self
            .obligations
            .values()
            .filter(|obligation| {
                let state = effective
                    .get(&obligation.id)
                    .copied()
                    .unwrap_or(ObligationState::Unseen);
                !state.is_evidentially_discharged()
                    && obligation.depends_on.iter().all(|dependency| {
                        effective
                            .get(dependency)
                            .copied()
                            .is_some_and(ObligationState::is_evidentially_discharged)
                    })
            })
            .map(|obligation| FrontierEntry {
                obligation: obligation.id.clone(),
                statement: obligation.statement.clone(),
                recorded: obligation.recorded_state(),
                effective: effective
                    .get(&obligation.id)
                    .copied()
                    .unwrap_or(ObligationState::Unseen),
                confidence: obligation.confidence(),
                value: obligation.value,
                mandatory: obligation.mandatory,
            })
            .collect();
        frontier.sort_by(|a, b| {
            b.value
                .partial_cmp(&a.value)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.obligation.cmp(&b.obligation))
        });
        Ok(frontier)
    }

    /// Obligations that are not evidentially discharged, by effective state.
    pub fn undischarged(&self) -> Result<Vec<(String, ObligationState)>, ObligationError> {
        Ok(self
            .effective_states()?
            .into_iter()
            .filter(|(_, state)| !state.is_evidentially_discharged())
            .collect())
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("obligation graph is serialisable")
    }

    /// Content hash over the whole graph, used to bind a sufficiency certificate to the graph it
    /// was computed from so a stale certificate is detectable (39.17 failure semantics).
    pub fn content_hash(&self) -> Result<ContentHash, CanonicalError> {
        ContentHash::of_value(&self.to_json())
    }
}

/// One actionable obligation on the frontier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontierEntry {
    pub obligation: String,
    pub statement: String,
    /// What the last actor recorded.
    pub recorded: ObligationState,
    /// What the dependency graph allows that record to mean.
    pub effective: ObligationState,
    pub confidence: f64,
    pub value: f64,
    pub mandatory: bool,
}
