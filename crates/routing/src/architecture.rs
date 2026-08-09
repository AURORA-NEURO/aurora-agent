//! The routable choices, and the fence around them.
//!
//! Blueprint 09.08 constrains the candidate set to "only approved architecture versions
//! compatible with providers, data policy, and task tools" and states that "the router cannot
//! synthesize unreviewed code in production". [`ApprovedSet`] is that fence: routing is a
//! selection *within* a reviewed list, never a synthesis step.
//!
//! In this workspace an "architecture" is a **context architecture** — one of the equal-engineering
//! context strategies from `bioprism-baseline`. It is not a model, not a prompt, and not an agent
//! topology. That restriction is deliberate and is repeated in the crate root: routing between
//! context strategies is a claim this repository can actually measure with the deterministic
//! oracle, whereas routing between models is not.
//!
//! Every variant carries the configuration knob that makes it a distinct *version* rather than a
//! distinct family. `graph-5-hop` and `graph-7-hop` are separate approved architectures because
//! `crates/baseline/tests/equal_engineering.rs` shows the usable depth window is two settings
//! wide and cannot be derived from the query — which is precisely the kind of choice a router is
//! supposed to make from evidence.

use crate::error::RoutingError;
use bioprism_baseline::{
    ConnectedComponent, ContextStrategy, FiberCompiled, FullContext, KHopIncidence, LexicalTopK,
    QueryGraph,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Architecture {
    /// Every fact in the world. Admissible by construction and maximally expensive.
    FullContext,
    /// Depth-limited walk of the undirected incidence graph.
    GraphKHop { depth: usize },
    /// The unbounded walk: the whole connected component.
    HypergraphComponent,
    /// Facts feeding any factor incident to a query target.
    QueryGraph,
    /// BM25 over fact text, truncated at `k`.
    LexicalTopK { k: usize },
    /// The FIBER compiler entered as one competitor among the others.
    FiberCompiled,
}

impl Architecture {
    /// The name `bioprism-baseline` gives this strategy.
    ///
    /// Evidence rows are keyed by architecture, and comparison rows are keyed by strategy name.
    /// If the two ever disagree an outcome would be attributed to the wrong architecture, so
    /// [`Architecture::verify_label`] checks the correspondence rather than trusting it.
    pub fn label(&self) -> String {
        match self {
            Architecture::FullContext => "full-context".to_string(),
            Architecture::GraphKHop { depth } => format!("graph-{depth}-hop"),
            Architecture::HypergraphComponent => "hypergraph-component".to_string(),
            Architecture::QueryGraph => "query-graph".to_string(),
            Architecture::LexicalTopK { k } => format!("lexical-top-{k}"),
            Architecture::FiberCompiled => "fiber".to_string(),
        }
    }

    pub fn strategy(&self) -> Box<dyn ContextStrategy> {
        match self {
            Architecture::FullContext => Box::new(FullContext),
            Architecture::GraphKHop { depth } => Box::new(KHopIncidence { depth: *depth }),
            Architecture::HypergraphComponent => Box::new(ConnectedComponent),
            Architecture::QueryGraph => Box::new(QueryGraph),
            Architecture::LexicalTopK { k } => Box::new(LexicalTopK { k: *k }),
            Architecture::FiberCompiled => Box::new(FiberCompiled),
        }
    }

    /// Whether this architecture is admissible on *every* world without consulting evidence.
    ///
    /// Only full context qualifies: it exposes the whole world, so it cannot drop a decisive
    /// witness and cannot break the protected closure. This is the property that makes it the
    /// only sound choice for a hard safety floor, and the reason it is also the most expensive
    /// comparator in the Gate 6 panel.
    pub fn admissible_by_construction(&self) -> bool {
        matches!(self, Architecture::FullContext)
    }

    pub fn verify_label(&self) -> Result<(), RoutingError> {
        let actual = self.strategy().name();
        let expected = self.label();
        if actual == expected {
            return Ok(());
        }
        Err(RoutingError::ArchitectureLabelMismatch {
            architecture: format!("{self:?}"),
            expected,
            actual,
        })
    }
}

/// A reviewed, non-empty, deduplicated candidate list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Architecture>", into = "Vec<Architecture>")]
pub struct ApprovedSet(Vec<Architecture>);

impl ApprovedSet {
    /// Deduplicates and sorts, so two callers listing the same architectures in different orders
    /// produce byte-identical evidence and byte-identical reports.
    pub fn new(
        architectures: impl IntoIterator<Item = Architecture>,
    ) -> Result<Self, RoutingError> {
        let mut list: Vec<Architecture> = architectures.into_iter().collect();
        list.sort();
        list.dedup();
        if list.is_empty() {
            return Err(RoutingError::NoApprovedArchitectures);
        }
        for architecture in &list {
            architecture.verify_label()?;
        }
        Ok(ApprovedSet(list))
    }

    /// The panel this workspace already measures in `crates/baseline`.
    ///
    /// The graph walk appears at depths 4, 5 and 7 rather than only at the depth where it
    /// explodes. Entering a baseline solely at settings that make it look bad is the
    /// unequal-engineering failure blueprint 43.38 exists to prevent, and it would also hand the
    /// router an artificially easy win.
    pub fn reference_panel() -> Self {
        ApprovedSet::new([
            Architecture::FullContext,
            Architecture::GraphKHop { depth: 4 },
            Architecture::GraphKHop { depth: 5 },
            Architecture::GraphKHop { depth: 7 },
            Architecture::HypergraphComponent,
            Architecture::QueryGraph,
            Architecture::LexicalTopK { k: 11 },
            Architecture::LexicalTopK { k: 50 },
            Architecture::FiberCompiled,
        ])
        .expect("the reference panel is non-empty and its labels match the baseline crate")
    }

    pub fn contains(&self, architecture: &Architecture) -> bool {
        self.0.contains(architecture)
    }

    pub fn require(&self, architecture: &Architecture) -> Result<(), RoutingError> {
        if self.contains(architecture) {
            return Ok(());
        }
        Err(RoutingError::UnapprovedArchitecture {
            architecture: architecture.label(),
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Architecture> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false. Kept so callers do not have to special-case the invariant, which is enforced
    /// at construction.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<Architecture>> for ApprovedSet {
    type Error = RoutingError;

    fn try_from(value: Vec<Architecture>) -> Result<Self, Self::Error> {
        ApprovedSet::new(value)
    }
}

impl From<ApprovedSet> for Vec<Architecture> {
    fn from(value: ApprovedSet) -> Self {
        value.0
    }
}

impl<'a> IntoIterator for &'a ApprovedSet {
    type Item = &'a Architecture;
    type IntoIter = std::slice::Iter<'a, Architecture>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_approved_architecture_names_the_baseline_strategy_it_actually_runs() {
        for architecture in ApprovedSet::reference_panel().iter() {
            architecture
                .verify_label()
                .expect("label and strategy name must agree or evidence is misattributed");
        }
    }

    #[test]
    fn an_empty_approved_set_is_rejected_because_there_is_nothing_to_route_between() {
        assert_eq!(
            ApprovedSet::new([]).unwrap_err(),
            RoutingError::NoApprovedArchitectures
        );
    }

    #[test]
    fn the_approved_set_deduplicates_and_orders_so_reports_are_reproducible() {
        let first = ApprovedSet::new([
            Architecture::FiberCompiled,
            Architecture::FullContext,
            Architecture::FiberCompiled,
        ])
        .unwrap();
        let second =
            ApprovedSet::new([Architecture::FullContext, Architecture::FiberCompiled]).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn an_unapproved_architecture_is_refused_rather_than_quietly_routed_to() {
        let approved = ApprovedSet::new([Architecture::FullContext]).unwrap();
        assert_eq!(
            approved.require(&Architecture::FiberCompiled).unwrap_err(),
            RoutingError::UnapprovedArchitecture {
                architecture: "fiber".to_string()
            }
        );
    }

    #[test]
    fn full_context_is_the_only_architecture_admissible_without_evidence() {
        let admissible: Vec<String> = ApprovedSet::reference_panel()
            .iter()
            .filter(|a| a.admissible_by_construction())
            .map(Architecture::label)
            .collect();
        assert_eq!(admissible, vec!["full-context".to_string()]);
    }

    #[test]
    fn an_architecture_round_trips_through_json() {
        let architecture = Architecture::GraphKHop { depth: 5 };
        let text = serde_json::to_string(&architecture).unwrap();
        assert_eq!(
            serde_json::from_str::<Architecture>(&text).unwrap(),
            architecture
        );
    }
}
