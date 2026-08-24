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
//!
//! # A refusal is not an abstention
//!
//! An oracle error was previously turned into `OracleVerdict::abstain`, the same swallow
//! `bioprism_prism::fork` and `bioprism_prism::minimize` removed, and it did the most damage here
//! because this module's output *is* the repository's headline result. An abstention is a verdict
//! like any other, so a row whose oracle refused compared equal to a reference whose oracle also
//! refused: `underdetermined` against `underdetermined`, empty witness set against empty witness
//! set. Every derived field then agreed. `verdict_preserving` said the strategy had preserved a
//! verdict nobody obtained, `missing_witnesses` was empty and read as "nothing decisive was
//! dropped", `admissible()` was satisfiable, and `cheapest_admissible()` would name a winner.
//!
//! 43.40's rule about missing evidence applies unchanged: an answer nobody could obtain and an
//! answer of "underdetermined" are different states and must not share a representation. Two
//! mechanisms keep them apart and they differ in shape — see [`compare()`] for why.
//!
//! # Which of the two is reachable today
//!
//! Neither, on the shipped panel over the shipped worlds. `bioprism_fiber::oracle::evaluate`
//! refuses on two conditions, and both require a *pair* of provided variables to be present
//! together: an alias spanning subjects whose `split_assignment` arms cannot be ordered, and a
//! non-string entry in `label_source_time` beside a `training_decision_time`. Dropping either
//! member of either pair removes the refusal, so `evaluate` is monotone under key removal. Every
//! strategy selects a subset of the world full-context selects, and no shipped world has two facts
//! providing the same variable, so each row's value map is a key-restriction of the reference's —
//! and a row cannot refuse unless the reference already did. The reference does not refuse on
//! either shipped world.
//!
//! It is represented rather than omitted because none of that is a property of this crate.
//! Monotonicity belongs to one oracle and is asserted nowhere; the key-restriction step assumes a
//! world whose provided variables are unique, which `bioprism_world::validate` reports as an error
//! but `World::from_json` accepts; and [`compare()`] is public API reached by
//! `bioprism context compare --world <any world that loads>` and by `bioprism_routing`'s lab over
//! caller-supplied tasks. `crates/mutation/tests/metamorphic.rs` already builds a refusing world
//! out of the shipped fixture by deleting one subject's split arm. Leaving the state out would make
//! the old swallow reappear silently the day any of those moves.

use crate::strategy::{ContextStrategy, Selection};
use bioprism_fiber::{oracle, FiberError, Query};
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Why no comparison exists to report.
///
/// One variant, because a comparison has exactly one precondition and inventing more would be the
/// kind of implied capability this workspace treats as a lie. [`ContextStrategy::select`] is
/// infallible by trait signature, so every other way this harness can fail belongs to an individual
/// row and is [`RowRefusal`] instead.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompareError {
    /// The oracle refused the full-context reference, so there is no verdict for a row to preserve.
    ///
    /// The refusal was previously turned into an abstention and adopted as the reference, after
    /// which every row that also refused matched it exactly and the table reported a panel of
    /// verdict-preserving, admissible strategies over a question nobody answered.
    #[error(
        "the oracle refused the {facts} fact(s) of the full-context reference, so no strategy has \
         a verdict to preserve: {source}"
    )]
    OracleRefusedReference {
        facts: usize,
        #[source]
        source: FiberError,
    },
}

/// Why a row carries no verdict.
///
/// Carries the compiler's own [`FiberError`] rather than a rendered string so that a caller
/// classifying this refusal — `bioprism-cli` routes it to an exit code — decides against the same
/// type every other oracle refusal in the workspace is decided against, and cannot drift from them.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RowRefusal {
    /// The oracle refused the evidence this strategy selected.
    #[error("the oracle refused the {facts_exposed} fact(s) this strategy selected: {source}")]
    OracleRefused {
        facts_exposed: usize,
        #[source]
        source: FiberError,
    },
}

/// One judged row's oracle answer, measured against the full-context reference.
///
/// Every field here is a statement about a verdict that was actually reached. `missing_witnesses`
/// being empty means the oracle judged this selection and found nothing decisive dropped; it can
/// never mean that nobody looked, because a row nobody could judge holds no `Judgement` at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Judgement {
    pub status: OracleStatus,
    pub witnesses: Vec<String>,
    /// True when this selection reaches the same verdict, with the same witnesses, as
    /// full-context.
    pub verdict_preserving: bool,
    pub missing_witnesses: Vec<String>,
    pub spurious_witnesses: Vec<String>,
}

/// What became of one row's trip through the oracle.
///
/// Two states that must stay two. The comparison against the reference lives *inside*
/// [`RowVerdict::Judged`], so a refused row has no `verdict_preserving` field to set — the equality
/// that produced the defect is not merely tested against, it cannot be written down. This mirrors
/// `bioprism_prism::fork::Arm`, where an arm that produced no verdict carries no measurement.
#[derive(Debug, Clone, PartialEq)]
pub enum RowVerdict {
    /// The oracle judged the selection and the row was compared to the reference.
    Judged(Judgement),
    /// The oracle refused the selection, so this row was never compared to anything.
    Refused(RowRefusal),
}

/// One strategy's row.
///
/// The cost columns are outside [`StrategyResult::verdict`] on purpose, and this is where the shape
/// departs from `fork::Arm`, which keeps `facts_exposed` inside its failure variant. A refused arm
/// there produced nothing the cell could judge and so has no measurement of any kind. A refused row
/// here still selected facts, still paid for them, and still did or did not deliver the protected
/// closure — those are measurements the oracle plays no part in, and suppressing them would be its
/// own dishonesty. What is suppressed is exactly what the oracle was needed for.
#[derive(Debug, Clone)]
pub struct StrategyResult {
    pub name: String,
    pub method: String,
    pub facts_exposed: usize,
    pub fraction_of_world: f64,
    /// The oracle's answer, or its refusal.
    pub verdict: RowVerdict,
    /// Fraction of the world's protected-tagged facts retained.
    pub protected_recall: f64,
    pub notes: Vec<String>,
}

impl StrategyResult {
    /// The oracle's answer, or `None` where the oracle declined to give one.
    ///
    /// There is deliberately no accessor that reports a refused row as an abstaining or an unsound
    /// one, which is the coercion the two states exist to prevent.
    pub fn judgement(&self) -> Option<&Judgement> {
        match &self.verdict {
            RowVerdict::Judged(judgement) => Some(judgement),
            RowVerdict::Refused(_) => None,
        }
    }

    pub fn refusal(&self) -> Option<&RowRefusal> {
        match &self.verdict {
            RowVerdict::Judged(_) => None,
            RowVerdict::Refused(refusal) => Some(refusal),
        }
    }

    /// The verdict this selection reached, or `None` where the oracle refused it.
    pub fn status(&self) -> Option<OracleStatus> {
        self.judgement().map(|judgement| judgement.status)
    }

    /// Whether this selection reached the reference verdict, or `None` where nothing was compared.
    ///
    /// Three-valued rather than a `bool`, for the reason 43.40 gives: `false` would report that the
    /// oracle had examined this selection and found it wanting, and a refusal is not a refutation.
    pub fn verdict_preserving(&self) -> Option<bool> {
        self.judgement().map(|judgement| judgement.verdict_preserving)
    }

    /// Whether the mandatory protected closure was delivered in full.
    ///
    /// Unqualified by the oracle, because the closure is a property of the selection: 43.13 puts it
    /// before any relevance step, and it is measurable whether or not anything was judged.
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
    ///
    /// A refused row is never admissible, and the negation is deliberately *not* a claim that it
    /// violated the contract: this is a positive predicate about demonstrated compliance, and
    /// [`Comparison::refused`] is where a reader finds the rows that demonstrated nothing.
    pub fn admissible(&self) -> bool {
        self.verdict_preserving() == Some(true) && self.closure_complete()
    }
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub world_id: String,
    pub query_id: String,
    pub total_facts: usize,
    /// The reference verdict every row is measured against.
    ///
    /// Always a verdict the oracle actually returned: a reference it refused never reaches this
    /// struct, so `underdetermined` here means the oracle answered "underdetermined" and never
    /// means it declined to answer.
    pub reference_status: OracleStatus,
    pub reference_witnesses: Vec<String>,
    pub results: Vec<StrategyResult>,
}

impl Comparison {
    /// The cheapest strategy that satisfied the whole contract: right verdict *and* full closure.
    ///
    /// A row the oracle refused cannot win this, however few facts it exposed, because it has no
    /// verdict to have preserved.
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
            .filter(|r| r.verdict_preserving() == Some(true) && !r.closure_complete())
    }

    /// Strategies the oracle judged and found not to preserve the reference verdict.
    ///
    /// Refused rows are absent by construction. Folding them in would restate the defect this
    /// module was fixed for with the sign flipped: an unexamined strategy reported as a refuted one.
    pub fn unsound(&self) -> impl Iterator<Item = &StrategyResult> {
        self.results
            .iter()
            .filter(|r| r.verdict_preserving() == Some(false))
    }

    /// Strategies whose selection the oracle would not judge. Counted as neither sound nor unsound.
    pub fn refused(&self) -> impl Iterator<Item = &StrategyResult> {
        self.results.iter().filter(|r| r.refusal().is_some())
    }

    /// Whether every row in the panel reached a verdict.
    ///
    /// Stronger than an empty [`Comparison::unsound`] list, and deliberately so: a panel in which
    /// nothing was judged has an empty unsound list too.
    pub fn is_fully_judged(&self) -> bool {
        self.results.iter().all(|r| r.judgement().is_some())
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
            "results": self.results.iter().map(|r| {
                // A refused row serialises with no `status`, `verdict_preserving`,
                // `missing_witnesses`, `spurious_witnesses` or `admissible` key at all. There is
                // nothing for a downstream reader to coerce, default or count as a zero, and an
                // absent key cannot be mistaken for a measured one the way `false` and `[]` were.
                // `judged` tags the block that follows, so nobody has to infer the state from
                // which keys happen to be present.
                let mut row = Map::new();
                row.insert("name".into(), json!(r.name));
                row.insert("method".into(), json!(r.method));
                row.insert("facts_exposed".into(), json!(r.facts_exposed));
                row.insert("fraction_of_world".into(), json!(r.fraction_of_world));
                row.insert("judged".into(), json!(r.judgement().is_some()));
                match &r.verdict {
                    RowVerdict::Judged(judgement) => {
                        row.insert("status".into(), json!(judgement.status.as_str()));
                        row.insert("witnesses".into(), json!(judgement.witnesses));
                        row.insert(
                            "verdict_preserving".into(),
                            json!(judgement.verdict_preserving),
                        );
                        row.insert("missing_witnesses".into(), json!(judgement.missing_witnesses));
                        row.insert(
                            "spurious_witnesses".into(),
                            json!(judgement.spurious_witnesses),
                        );
                    }
                    RowVerdict::Refused(refusal) => {
                        row.insert("refusal".into(), json!(refusal.to_string()));
                    }
                }
                row.insert("protected_recall".into(), json!(r.protected_recall));
                row.insert("closure_complete".into(), json!(r.closure_complete()));
                match &r.verdict {
                    RowVerdict::Judged(_) => {
                        row.insert("admissible".into(), json!(r.admissible()));
                    }
                    RowVerdict::Refused(_) => {}
                }
                row.insert("notes".into(), json!(r.notes));
                Value::Object(row)
            }).collect::<Vec<_>>(),
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
            // The oracle-derived cells of a refused row are dashes rather than `no` or `0`, and the
            // verdict cell names the state outright so the dashes cannot be read as a blank.
            let (verdict, sound, admissible) = match &result.verdict {
                RowVerdict::Judged(judgement) => (
                    judgement.status.as_str().to_string(),
                    if judgement.verdict_preserving { "yes" } else { "**no**" },
                    if result.admissible() { "yes" } else { "**no**" },
                ),
                RowVerdict::Refused(_) => ("**refused**".to_string(), "—", "—"),
            };
            let _ = writeln!(
                text,
                "| {} | {} | {:.2}% | {} | {} | {:.0}% | {} |",
                result.name,
                result.facts_exposed,
                result.fraction_of_world * 100.0,
                verdict,
                sound,
                result.protected_recall * 100.0,
                admissible
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

        for result in self.refused() {
            let _ = writeln!(
                text,
                "\n- `{}` was **not judged**: {}. It is ranked as neither sound nor unsound and \
                 cannot be admissible, because nothing about its verdict was established.",
                result.name,
                result
                    .refusal()
                    .expect("a refused row carries its refusal")
            );
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
            let judgement = result
                .judgement()
                .expect("an unsound row was judged by definition");
            let _ = writeln!(
                text,
                "\n- `{}` is **not sound**: missing {}{}",
                result.name,
                if judgement.missing_witnesses.is_empty() {
                    "no witnesses".to_string()
                } else {
                    judgement.missing_witnesses.join(", ")
                },
                if judgement.spurious_witnesses.is_empty() {
                    String::new()
                } else {
                    format!("; spurious {}", judgement.spurious_witnesses.join(", "))
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

fn verdict_for(world: &World, selection: &Selection) -> Result<OracleVerdict, FiberError> {
    let values: BTreeMap<String, Value> = selection
        .facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values)
}

fn witness_kinds(verdict: &OracleVerdict) -> BTreeSet<String> {
    verdict
        .witness_kinds()
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Runs every strategy against the same world, query and oracle.
///
/// # Why this returns a `Result` and still keeps a per-row state
///
/// `bioprism_prism::fork` kept its refusal as a *state* because an `Err` there would answer an
/// arm-level question by discarding the arms that did run. `bioprism_prism::minimize` used an `Err`
/// for the candidate it was reducing, because that verdict is the target rather than a step. A
/// comparison row has both halves, so this function borrows both remedies for the same reasons —
/// it mirrors `minimize`, not `fork`, and the split falls in the same place.
///
/// The full-context reference is not a row of the table; it is the target every row is measured
/// against, and it is computed before any strategy runs. An `Err` for it discards nothing, because
/// nothing has happened yet — and the alternative is worse than the fork's was. A panel with one
/// unjudged arm still compares the others; a *comparison* against a reference the oracle never
/// produced is not a weakened comparison but a meaningless one. 43.38 orders logical completeness
/// before physical efficiency, and with no reference verdict there is no completeness axis at all:
/// what remains is a bare cost ranking, which is precisely the "smallest wins" reading the ordering
/// exists to forbid. That is [`CompareError::OracleRefusedReference`].
///
/// A *strategy's* selection is a step, and here the fork's reasoning transfers verbatim: an `Err` on
/// the seventh of ten strategies would throw away nine measured rows to report one nobody could
/// judge, and a panel exists to be compared. The row is kept, its refusal is recorded in
/// [`RowVerdict::Refused`], and every field that would have needed a verdict is absent rather than
/// defaulted.
pub fn compare(
    world: &World,
    query: &Query,
    strategies: &[&dyn ContextStrategy],
) -> Result<Comparison, CompareError> {
    let protected_total = world
        .facts
        .iter()
        .filter(|fact| fact.has_any_tag(&query.protected_tags))
        .count();

    let full = crate::strategy::FullContext;
    let reference_selection = full.select(world, query);
    let reference = verdict_for(world, &reference_selection).map_err(|source| {
        CompareError::OracleRefusedReference {
            facts: reference_selection.facts.len(),
            source,
        }
    })?;
    let reference_kinds = witness_kinds(&reference);

    let mut results = Vec::new();
    for strategy in strategies {
        let selection = strategy.select(world, query);
        let facts_exposed = selection.facts.len();
        let verdict = match verdict_for(world, &selection) {
            Ok(verdict) => {
                let kinds = witness_kinds(&verdict);
                RowVerdict::Judged(Judgement {
                    verdict_preserving: verdict.status == reference.status
                        && kinds == reference_kinds,
                    missing_witnesses: reference_kinds.difference(&kinds).cloned().collect(),
                    spurious_witnesses: kinds.difference(&reference_kinds).cloned().collect(),
                    witnesses: kinds.into_iter().collect(),
                    status: verdict.status,
                })
            }
            Err(source) => RowVerdict::Refused(RowRefusal::OracleRefused {
                facts_exposed,
                source,
            }),
        };

        let protected_kept = selection
            .facts
            .iter()
            .filter_map(|id| world.fact(id))
            .filter(|fact| fact.has_any_tag(&query.protected_tags))
            .count();

        results.push(StrategyResult {
            name: strategy.name(),
            method: strategy.method(),
            facts_exposed,
            fraction_of_world: if world.facts.is_empty() {
                0.0
            } else {
                facts_exposed as f64 / world.facts.len() as f64
            },
            verdict,
            protected_recall: if protected_total == 0 {
                1.0
            } else {
                protected_kept as f64 / protected_total as f64
            },
            notes: selection.notes,
        });
    }

    Ok(Comparison {
        world_id: world.world_id.as_str().to_string(),
        query_id: query.query_id.as_str().to_string(),
        total_facts: world.facts.len(),
        reference_status: reference.status,
        reference_witnesses: reference_kinds.into_iter().collect(),
        results,
    })
}

/// The default panel.
///
/// Includes the graph walk at its *best* depth, not only at depths where it degenerates.
/// Reporting a baseline solely at settings that make it look bad is the unequal-engineering
/// failure 43.38 exists to prevent, and on the reference world depth 5 is where the graph walk
/// is strongest — strong enough to tie the compiler exactly. The directed dependency walk enters
/// unbounded, *its* strongest setting, where it ties the compiler on every shipped world.
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
        Box::new(crate::embedding::EmbeddingTopK { k: 11 }),
        Box::new(crate::embedding::EmbeddingTopK { k: 50 }),
        Box::new(crate::directed::DirectedDependencyWalk::unbounded()),
        Box::new(crate::strategy::FiberCompiled),
    ]
}
