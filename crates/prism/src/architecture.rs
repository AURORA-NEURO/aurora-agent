//! Architectures under test.
//!
//! Blueprint 03.05 describes all evaluated systems through a common observable contract while
//! preserving their differences. In this platform the component being varied is the **context
//! policy**: which evidence reaches the decision. Everything else — the world, the query, the
//! oracle — is held identical by the cell, which is what makes the comparison matched.
//!
//! That is the executive summary's wedge, stated concretely: *show that context policy, not model
//! identity, explains the regression.*

use bioprism_baseline::{
    ConnectedComponent, ContextStrategy, FiberCompiled, FullContext, KHopIncidence, LexicalTopK,
    QueryGraph,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StrategySpec {
    Fiber,
    FullContext,
    GraphKHop { depth: usize },
    ConnectedComponent,
    QueryGraph,
    LexicalTopK { k: usize },
}

impl StrategySpec {
    pub fn build(&self) -> Box<dyn ContextStrategy> {
        match self {
            StrategySpec::Fiber => Box::new(FiberCompiled),
            StrategySpec::FullContext => Box::new(FullContext),
            StrategySpec::GraphKHop { depth } => Box::new(KHopIncidence { depth: *depth }),
            StrategySpec::ConnectedComponent => Box::new(ConnectedComponent),
            StrategySpec::QueryGraph => Box::new(QueryGraph),
            StrategySpec::LexicalTopK { k } => Box::new(LexicalTopK { k: *k }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Architecture {
    pub name: String,
    pub strategy: StrategySpec,
}

impl Architecture {
    pub fn new(name: impl Into<String>, strategy: StrategySpec) -> Self {
        Architecture {
            name: name.into(),
            strategy,
        }
    }

    /// The panel used by the default regression pack: the compiler against the strongest
    /// alternatives, including a graph walk at its best depth rather than only where it fails.
    pub fn default_panel() -> Vec<Architecture> {
        vec![
            Architecture::new("fiber", StrategySpec::Fiber),
            Architecture::new("full-context", StrategySpec::FullContext),
            Architecture::new("graph-5-hop", StrategySpec::GraphKHop { depth: 5 }),
            Architecture::new("graph-7-hop", StrategySpec::GraphKHop { depth: 7 }),
            Architecture::new("lexical-top-11", StrategySpec::LexicalTopK { k: 11 }),
        ]
    }
}
