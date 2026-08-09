//! The comparison harness.
//!
//! Blueprint 43.38 asks for matched comparison that separates logical completeness from physical
//! efficiency. The ordering of those two matters and is enforced here.
//!
//! Facts exposed is a **cost**. It is only meaningful among strategies that still reach the right
//! answer. So every strategy's selection is fed to the same deterministic oracle and its verdict
//! compared to the full-context verdict; a strategy that drops a decisive witness is marked
//! `verdict_preserving: false` and is disqualified from the cost ranking rather than celebrated
//! for being small. This is 43.41's stop rule — "If FIBER omits any decisive witness, block
//! advancement" — applied symmetrically to every competitor.

use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::{oracle, Query};
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct StrategyResult {
    pub name: String,
    pub method: String,
    pub facts_exposed: usize,
    pub fraction_of_world: f64,
    pub status: OracleStatus,
    pub witnesses: Vec<String>,
    /// True when this selection reaches the same verdict, with the same witnesses, as
    /// full-context.
    pub verdict_preserving: bool,
    pub missing_witnesses: Vec<String>,
    pub spurious_witnesses: Vec<String>,
    /// Fraction of the world's protected-tagged facts retained.
    pub protected_recall: f64,
    pub notes: Vec<String>,
}

impl StrategyResult {
    /// Whether the mandatory protected closure was delivered in full.
    pub fn closure_complete(&self) -> bool {
        self.protected_recall >= 1.0
    }

    /// Whether this strategy satisfied the decision contract at all.
    ///
    /// Both conditions are required. Reaching the right verdict while dropping protected evidence
    /// is not a pass: 43.13 declares the closure mandatory *before* any relevance or compression
    /// step, precisely so that a strategy cannot be credited for guessing correctly from an
    /// incomplete basis. On the discriminating world a lexical retriever does exactly that — right
    /// answer, 91% closure — and must not be ranked above a compiler that delivered both.
    pub fn admissible(&self) -> bool {
        self.verdict_preserving && self.closure_complete()
    }
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub world_id: String,
    pub query_id: String,
    pub total_facts: usize,
    pub reference_status: OracleStatus,
    pub reference_witnesses: Vec<String>,
    pub results: Vec<StrategyResult>,
}

impl Comparison {
    /// The cheapest strategy that satisfied the whole contract: right verdict *and* full closure.
    pub fn cheapest_admissible(&self) -> Option<&StrategyResult> {
        self.results
            .iter()
            .filter(|r| r.admissible())
            .min_by_key(|r| r.facts_exposed)
    }

    /// Strategies that reached the right verdict from an incomplete protected closure.
    ///
    /// The most dangerous category in the panel: they look correct and are not trustworthy.
    pub fn lucky(&self) -> impl Iterator<Item = &StrategyResult> {
        self.results
            .iter()
            .filter(|r| r.verdict_preserving && !r.closure_complete())
    }

    pub fn unsound(&self) -> impl Iterator<Item = &StrategyResult> {
        self.results.iter().filter(|r| !r.verdict_preserving)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "world_id": self.world_id,
            "query_id": self.query_id,
            "total_facts": self.total_facts,
            "reference": {
                "source": "full-context",
                "status": self.reference_status.as_str(),
                "witnesses": self.reference_witnesses,
            },
            "cheapest_admissible_strategy": self.cheapest_admissible().map(|r| r.name.clone()),
            "results": self.results.iter().map(|r| json!({
                "name": r.name,
                "method": r.method,
                "facts_exposed": r.facts_exposed,
                "fraction_of_world": r.fraction_of_world,
                "status": r.status.as_str(),
                "witnesses": r.witnesses,
                "verdict_preserving": r.verdict_preserving,
                "missing_witnesses": r.missing_witnesses,
                "spurious_witnesses": r.spurious_witnesses,
                "protected_recall": r.protected_recall,
                "closure_complete": r.closure_complete(),
                "admissible": r.admissible(),
                "notes": r.notes,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut text = String::new();
        let _ = writeln!(
            text,
            "# Equal-engineering context comparison: {}\n\nworld `{}`, query `{}`, {} facts total",
            self.world_id, self.world_id, self.query_id, self.total_facts
        );
        let _ = writeln!(
            text,
            "\nReference verdict (full-context): **{}** with witnesses {}",
            self.reference_status.as_str(),
            if self.reference_witnesses.is_empty() {
                "none".to_string()
            } else {
                self.reference_witnesses.join(", ")
            }
        );
        let _ = writeln!(
            text,
            "\n| Strategy | Facts | % of world | Verdict | Sound? | Closure | Admissible |"
        );
        let _ = writeln!(text, "|---|---:|---:|---|:-:|---:|:-:|");
        for result in &self.results {
            let _ = writeln!(
                text,
                "| {} | {} | {:.2}% | {} | {} | {:.0}% | {} |",
                result.name,
                result.facts_exposed,
                result.fraction_of_world * 100.0,
                result.status.as_str(),
                if result.verdict_preserving { "yes" } else { "**no**" },
                result.protected_recall * 100.0,
                if result.admissible() { "yes" } else { "**no**" }
            );
        }

        match self.cheapest_admissible() {
            Some(best) => {
                let _ = writeln!(
                    text,
                    "\nCheapest admissible strategy (right verdict **and** full protected \
                     closure): **{}** at {} facts ({:.2}% of world).",
                    best.name,
                    best.facts_exposed,
                    best.fraction_of_world * 100.0
                );
            }
            None => {
                let _ = writeln!(text, "\nNo strategy satisfied the decision contract.");
            }
        }

        for result in self.lucky() {
            let _ = writeln!(
                text,
                "\n- `{}` reached the correct verdict from an **incomplete protected closure** \
                 ({:.0}%). Under 43.13 the closure is mandatory before any relevance step, so \
                 this is a contract violation that guessed right, not a pass.",
                result.name,
                result.protected_recall * 100.0
            );
        }

        for result in self.unsound() {
            let _ = writeln!(
                text,
                "\n- `{}` is **not sound**: missing {}{}",
                result.name,
                if result.missing_witnesses.is_empty() {
                    "no witnesses".to_string()
                } else {
                    result.missing_witnesses.join(", ")
                },
                if result.spurious_witnesses.is_empty() {
                    String::new()
                } else {
                    format!("; spurious {}", result.spurious_witnesses.join(", "))
                }
            );
        }

        let _ = writeln!(
            text,
            "\n## Methods\n"
        );
        for result in &self.results {
            let _ = writeln!(text, "- **{}** — {}", result.name, result.method);
            for note in &result.notes {
                let _ = writeln!(text, "  - {note}");
            }
        }

        let _ = writeln!(
            text,
            "\nFacts exposed is a cost, not a score. It ranks only among verdict-preserving \
             strategies. This world is constructed to expose hub expansion; it demonstrates \
             compiler mechanics, not universal superiority."
        );

        text
    }
}

fn verdict_for(world: &World, selection: &Selection) -> OracleVerdict {
    let values: BTreeMap<String, Value> = selection
        .facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values).unwrap_or_else(|_| OracleVerdict::abstain(oracle::ORACLE_KIND, vec![]))
}

pub fn compare(
    world: &World,
    query: &Query,
    strategies: &[&dyn ContextStrategy],
) -> Comparison {
    let protected_total = world
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .count();

    let full = crate::strategy::FullContext;
    let reference_selection = full.select(world, query);
    let reference = verdict_for(world, &reference_selection);
    let reference_kinds: BTreeSet<String> = reference
        .witness_kinds()
        .into_iter()
        .map(str::to_string)
        .collect();

    let mut results = Vec::new();
    for strategy in strategies {
        let selection = strategy.select(world, query);
        let verdict = verdict_for(world, &selection);
        let kinds: BTreeSet<String> = verdict
            .witness_kinds()
            .into_iter()
            .map(str::to_string)
            .collect();

        let protected_kept = selection
            .facts
            .iter()
            .filter_map(|id| world.fact(id))
            .filter(|fact| fact.has_any_tag(&query.protected_tags))
            .count();

        results.push(StrategyResult {
            name: strategy.name(),
            method: strategy.method(),
            facts_exposed: selection.facts.len(),
            fraction_of_world: if world.facts.is_empty() {
                0.0
            } else {
                selection.facts.len() as f64 / world.facts.len() as f64
            },
            status: verdict.status,
            witnesses: kinds.iter().cloned().collect(),
            verdict_preserving: verdict.status == reference.status && kinds == reference_kinds,
            missing_witnesses: reference_kinds.difference(&kinds).cloned().collect(),
            spurious_witnesses: kinds.difference(&reference_kinds).cloned().collect(),
            protected_recall: if protected_total == 0 {
                1.0
            } else {
                protected_kept as f64 / protected_total as f64
            },
            notes: selection.notes,
        });
    }

    Comparison {
        world_id: world.world_id.as_str().to_string(),
        query_id: query.query_id.as_str().to_string(),
        total_facts: world.facts.len(),
        reference_status: reference.status,
        reference_witnesses: reference_kinds.into_iter().collect(),
        results,
    }
}

/// The default panel.
///
/// Includes the graph walk at its *best* depth, not only at depths where it degenerates.
/// Reporting a baseline solely at settings that make it look bad is the unequal-engineering
/// failure 43.38 exists to prevent, and on the reference world depth 5 is where the graph walk
/// is strongest — strong enough to tie the compiler exactly.
pub fn default_panel() -> Vec<Box<dyn ContextStrategy>> {
    vec![
        Box::new(crate::strategy::FullContext),
        Box::new(crate::incidence::KHopIncidence { depth: 4 }),
        Box::new(crate::incidence::KHopIncidence { depth: 5 }),
        Box::new(crate::incidence::KHopIncidence { depth: 6 }),
        Box::new(crate::incidence::KHopIncidence { depth: 7 }),
        Box::new(crate::incidence::ConnectedComponent),
        Box::new(crate::incidence::QueryGraph),
        Box::new(crate::lexical::LexicalTopK { k: 11 }),
        Box::new(crate::lexical::LexicalTopK { k: 50 }),
        Box::new(crate::strategy::FiberCompiled),
    ]
}
