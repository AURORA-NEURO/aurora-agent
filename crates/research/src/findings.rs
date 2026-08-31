//! Findings derived by fixed rules from measurements — never free-generated.
//!
//! Each public function here is one derivation rule: it reads a typed measurement the runner
//! produced, and emits [`Finding`]s whose claims are format strings over that measurement's
//! fields. No rule consults the request's question, no rule ranks or editorialises beyond the
//! stated comparisons, and every finding cites the content digests of the artifacts it was
//! derived from. The rules are code, so the tests exercise them on synthetic fixtures directly.
//!
//! Negative findings are first-class: a tie between the compiler and a baseline, a panel with no
//! admissible strategy, an inflation ratio above one, a reduction whose re-check diverged — each
//! is emitted in exactly the same shape as a positive finding, flagged `negative: true` and
//! nothing else. The repository's own headline finding is a tie; this module is built to keep
//! reporting it.

use bioprism_baseline::{Comparison, StrategyResult, SweepTable};
use bioprism_mutation::{Diversity, Family};
use bioprism_prism::{Minimization, Preservation};
use serde::{Deserialize, Serialize};

/// The only level this runner may emit.
///
/// A single-variant enum, so "conclusion", "hypothesis", or any other level is unrepresentable in
/// the type system, not merely unadvised: no constructor exists, serde refuses every other
/// string, and a `match` over the type proves at compile time that `Observation` is all there is.
/// The runner measures; a human decides what the measurements mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationLevel {
    Observation,
}

impl ObservationLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            ObservationLevel::Observation => "observation",
        }
    }
}

/// One derived finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// The derivation rule that produced this finding, so a reader can locate the code.
    pub rule: String,
    /// The claim, rendered from measurement fields by the rule.
    pub claim: String,
    pub level: ObservationLevel,
    /// Content digests of the artifacts this finding was derived from.
    pub supported_by: Vec<String>,
    /// True for null results and results against separation. Same shape, same register.
    pub negative: bool,
}

fn finding(rule: &str, claim: String, supported_by: &[&str], negative: bool) -> Finding {
    Finding {
        rule: rule.to_string(),
        claim,
        level: ObservationLevel::Observation,
        supported_by: supported_by.iter().map(|d| (*d).to_string()).collect(),
        negative,
    }
}

/// Rule `reference_anchor`: the embedded fixture pair reproduced the pinned parity certificate.
///
/// Emitted only after the runner has already checked the digest — a mismatch aborts the run
/// before any finding exists — so this records the anchor rather than asserting it.
pub fn reference_anchor_finding(pinned_digest: &str, certificate_artifact_digest: &str) -> Finding {
    finding(
        "reference_anchor",
        format!(
            "the committed reference fixture compiles to the pinned cross-language parity \
             certificate digest {pinned_digest}"
        ),
        &[certificate_artifact_digest],
        false,
    )
}

fn is_baseline(result: &StrategyResult) -> bool {
    result.name != "fiber" && result.name != "full-context"
}

/// Rules over one 43.38 comparison. Emits, in this order:
///
/// * `cheapest_admissible` — the cheapest strategy satisfying the whole contract, named with its
///   cost; `negative` when that strategy is not `fiber`. Or `no_admissible_strategy`
///   (`negative`) when nothing satisfied the contract.
/// * `fiber_tied_by_baseline` (`negative`) — fiber admissible, and at least one baseline
///   (excluding `full-context`, admissible by construction) admissible at fiber's fact cost or
///   below. This is the tie rule: a tie is a required negative finding, never a footnote.
/// * `fiber_separated` — fiber admissible and no baseline admissible at its cost or below.
/// * `fiber_inadmissible` (`negative`) — fiber did not satisfy the contract on this world.
/// * `oracle_refused_row` — per refused row: recorded as unjudged, neither sound nor unsound.
pub fn comparison_findings(comparison: &Comparison, artifact_digest: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let supported = [artifact_digest];

    match comparison.cheapest_admissible() {
        Some(best) => findings.push(finding(
            "cheapest_admissible",
            format!(
                "cheapest admissible strategy on world {} ({} facts total) is {} at {} facts \
                 ({:.2}% of world)",
                comparison.world_id,
                comparison.total_facts,
                best.name,
                best.facts_exposed,
                best.fraction_of_world * 100.0
            ),
            &supported,
            best.name != "fiber",
        )),
        None => findings.push(finding(
            "no_admissible_strategy",
            format!(
                "no strategy satisfied the decision contract on world {} ({} facts total)",
                comparison.world_id, comparison.total_facts
            ),
            &supported,
            true,
        )),
    }

    let fiber = comparison.results.iter().find(|r| r.name == "fiber");
    if let Some(fiber) = fiber {
        if fiber.admissible() {
            let tying: Vec<&StrategyResult> = comparison
                .results
                .iter()
                .filter(|r| {
                    is_baseline(r) && r.admissible() && r.facts_exposed <= fiber.facts_exposed
                })
                .collect();
            if tying.is_empty() {
                findings.push(finding(
                    "fiber_separated",
                    format!(
                        "on world {} no baseline was admissible at fiber's cost of {} facts or \
                         below",
                        comparison.world_id, fiber.facts_exposed
                    ),
                    &supported,
                    false,
                ));
            } else {
                let names: Vec<String> = tying
                    .iter()
                    .map(|r| format!("{} at {} facts", r.name, r.facts_exposed))
                    .collect();
                findings.push(finding(
                    "fiber_tied_by_baseline",
                    format!(
                        "tie on world {}: fiber is admissible at {} facts and so {} {} — fiber \
                         is not separated from the baseline panel on this world",
                        comparison.world_id,
                        fiber.facts_exposed,
                        if names.len() == 1 { "is" } else { "are" },
                        names.join(", ")
                    ),
                    &supported,
                    true,
                ));
            }
        } else {
            findings.push(finding(
                "fiber_inadmissible",
                format!(
                    "fiber did not satisfy the decision contract on world {}: verdict-preserving \
                     {}, protected closure {:.0}%",
                    comparison.world_id,
                    match fiber.verdict_preserving() {
                        Some(true) => "yes",
                        Some(false) => "no",
                        None => "not judged (oracle refused)",
                    },
                    fiber.protected_recall * 100.0
                ),
                &supported,
                true,
            ));
        }
    }

    for refused in comparison.refused() {
        findings.push(finding(
            "oracle_refused_row",
            format!(
                "strategy {} was not judged on world {}: the oracle refused its selection of {} \
                 facts, so it is neither sound nor unsound",
                refused.name, comparison.world_id, refused.facts_exposed
            ),
            &supported,
            false,
        ));
    }

    findings
}

/// Rules over the 43.39 sweep table, using the same cell categories the sweep figure draws:
/// `full-context` is excluded from the counts because it is admissible by construction.
///
/// * `sweep_ties` (`negative`) — cells where fiber and at least one baseline are both
///   admissible. Required whenever any tie cell exists.
/// * `sweep_fiber_only` — cells where fiber alone is admissible.
/// * `sweep_fiber_inadmissible` (`negative`) — cells where fiber is not admissible.
/// * `sweep_none_admissible` (`negative`) — cells where nothing is admissible.
pub fn sweep_findings(table: &SweepTable, artifact_digest: &str) -> Vec<Finding> {
    let supported = [artifact_digest];
    let total = table.cells.len();
    let mut ties = 0usize;
    let mut fiber_only = 0usize;
    let mut fiber_inadmissible = 0usize;
    let mut none_admissible = 0usize;
    for cell in &table.cells {
        let fiber = cell.row("fiber").is_some_and(|row| row.admissible);
        let baselines = cell
            .rows
            .iter()
            .filter(|row| row.strategy != "fiber" && row.strategy != "full-context")
            .filter(|row| row.admissible)
            .count();
        match (fiber, baselines) {
            (true, 0) => fiber_only += 1,
            (true, _) => ties += 1,
            (false, 0) => {
                fiber_inadmissible += 1;
                none_admissible += 1;
            }
            (false, _) => fiber_inadmissible += 1,
        }
    }

    let mut findings = Vec::new();
    if ties > 0 {
        findings.push(finding(
            "sweep_ties",
            format!(
                "fiber is not separated in {ties} of {total} sweep cells: at least one baseline \
                 is admissible alongside it (full-context excluded, admissible by construction)"
            ),
            &supported,
            true,
        ));
    }
    if fiber_only > 0 {
        findings.push(finding(
            "sweep_fiber_only",
            format!("fiber is the only admissible strategy in {fiber_only} of {total} sweep cells"),
            &supported,
            false,
        ));
    }
    if fiber_inadmissible > 0 {
        findings.push(finding(
            "sweep_fiber_inadmissible",
            format!("fiber is not admissible in {fiber_inadmissible} of {total} sweep cells"),
            &supported,
            true,
        ));
    }
    if none_admissible > 0 {
        findings.push(finding(
            "sweep_none_admissible",
            format!("no strategy is admissible in {none_admissible} of {total} sweep cells"),
            &supported,
            true,
        ));
    }
    findings
}

/// Rule `mutation_yield`: the suite's yield and effective diversity, inflation stated.
///
/// `negative` when the independent equivalence classes number fewer than the instances — the
/// exact inflation the diversity accounting exists to expose.
pub fn mutation_findings(
    family: &Family,
    diversity: &Diversity,
    family_artifact_digest: &str,
    diversity_artifact_digest: &str,
) -> Vec<Finding> {
    vec![finding(
        "mutation_yield",
        format!(
            "metamorphic suite on parent {}: {} accepted, {} rejected, {} duplicate(s), yield \
             {:.0}%; {} independent equivalence classes from {} instances (inflation x{:.2}) — \
             instance count is not benchmark count",
            family.parent_id,
            family.accepted.len(),
            family.rejected.len(),
            family.duplicates.len(),
            family.yield_rate() * 100.0,
            diversity.equivalence_classes,
            diversity.instances,
            diversity.inflation_ratio
        ),
        &[family_artifact_digest, diversity_artifact_digest],
        diversity.equivalence_classes < diversity.instances,
    )]
}

/// Rule `minimize_reduction`: the 1-minimal reduction and its independent re-check.
///
/// `negative` unless the re-check is `Preserved`: a diverged reduction lost what it claimed to
/// keep, and an unverifiable one is unchecked rather than checked — neither may read as a pass.
///
/// The claim's *wording* is conditioned on the same outcome, not only its flag. Leading with
/// "a 1-minimal subset preserves the oracle signature" and appending the re-check after a
/// semicolon asserts as established the very thing a diverged re-check contradicts, and a reader
/// who stops at the first clause — or a table that shows only its beginning — carries away the
/// opposite of the measurement. A negative outcome therefore leads.
pub fn minimization_findings(
    minimization: &Minimization,
    preservation: &Preservation,
    artifact_digest: &str,
) -> Vec<Finding> {
    let searched = format!(
        "the search reduced {} facts to {} in {} evaluation(s), with the signature it minimised \
         against recorded as {} (witnesses [{}]); {} fact(s) held unjudged",
        minimization.started_from,
        minimization.minimal.len(),
        minimization.evaluations,
        minimization.preserved_status,
        minimization.preserved_witnesses.join(", "),
        minimization.unjudged.len(),
    );
    let claim = match preservation {
        Preservation::Preserved => format!(
            "a 1-minimal subset of {} of {} facts preserves the oracle signature ({}, witnesses \
             [{}]) after {} evaluations; {} fact(s) held unjudged; the independent re-check \
             preserved the signature",
            minimization.minimal.len(),
            minimization.started_from,
            minimization.preserved_status,
            minimization.preserved_witnesses.join(", "),
            minimization.evaluations,
            minimization.unjudged.len(),
        ),
        Preservation::Diverged { status, witnesses } => format!(
            "the independent re-check DIVERGED to status {status} (witnesses [{}]): the reduced \
             set did NOT preserve the oracle signature, so no preservation is claimed — {}",
            witnesses.join(", "),
            searched,
        ),
        Preservation::Unverifiable { detail } => format!(
            "the independent re-check was unverifiable — the oracle refused the minimal set \
             ({detail}) — so preservation is unchecked rather than checked and no preservation is \
             claimed; {}",
            searched,
        ),
    };
    vec![finding(
        "minimize_reduction",
        claim,
        &[artifact_digest],
        !preservation.is_preserved(),
    )]
}
