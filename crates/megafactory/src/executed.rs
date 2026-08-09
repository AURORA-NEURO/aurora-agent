//! The subset actually executed, in the only form this workspace lets a count be published in.
//!
//! Blueprint 35.13, the last of its scale constraints and the one every module in section 35
//! repeats verbatim: "generated instances are enumerable; results name the subset actually
//! executed." [`crate::placement::ExecutionLedger`] records what committed; this turns that record
//! into a figure a report may carry.
//!
//! ## Extending the guard rather than paralleling it
//!
//! `AGENTS.md` calls it non-negotiable that instance count is not benchmark count, and
//! `bioprism-scale` made that unrepresentable: `NominalCount` implements neither `Serialize` nor
//! `Deserialize`, so an instance count reaches a report only inside an `EffectiveSize`, which
//! carries the effective count and the inflation ratio in the same object.
//!
//! This module could easily have become the route around that — an execution ledger is a natural
//! place for a `usize` called `executed` to escape into a summary. It does not. [`ExecutedReport`]
//! has no bare instance count anywhere in it: `executed` and `enumerated` are both
//! `EffectiveSizeReport`s built from `bioprism-scale`'s own measurement over a real corpus, and
//! the one plain number, [`ExecutedReport::items_never_executed`], is a count of items *not*
//! evidence — the only direction in which a large number is not a boast. The crate's hygiene test
//! asserts the string `NominalCount` appears in no source file here, so no future edit can quietly
//! reintroduce the escape hatch.
//!
//! ## The unexecuted remainder is reported, not dropped
//!
//! An enumerated item that never ran is not evidence, but it is also not nothing: it is the gap
//! between what a release *could* have tested and what it did. [`ExecutedReport`] carries both
//! measured over the same corpus in the same pass, so the ratio between them cannot be assembled
//! from two different populations.
//!
//! ## Not implemented
//!
//! No execution. The ledger is a record a caller supplies. Nothing here reruns anything, and the
//! hierarchical deflation that turns executed classes into independent observations is
//! `bioprism_scale::effective::hierarchical_effective_size` — it takes an intra-parent correlation
//! the caller must state, and this module does not state one on anyone's behalf.

use crate::error::PlacementError;
use crate::placement::ExecutionLedger;
use bioprism_scale::corpus::{Corpus, GeneratedItem};
use bioprism_scale::EffectiveSizeReport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What a run may say about its own size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutedReport {
    /// Every relation measured over the executed subset.
    pub executed: EffectiveSizeReport,
    /// Every relation measured over the full enumerated corpus, for the same items' lineage.
    pub enumerated: EffectiveSizeReport,
    /// Enumerated items that never committed. Not evidence, and named rather than dropped.
    pub items_never_executed: usize,
}

impl ExecutedReport {
    /// Builds both reports from one corpus and one ledger.
    ///
    /// The executed sub-corpus keeps every ancestor of an executed item, so lineage still resolves
    /// and the equivalence-class and parent-world relations mean the same thing in both reports.
    /// Dropping unexecuted ancestors would silently promote an executed descendant to a parent
    /// world and inflate the parent-world class count — which is the exact shape of error this
    /// whole apparatus exists to prevent.
    pub fn measure(corpus: &Corpus, ledger: &ExecutionLedger) -> Result<Self, PlacementError> {
        let executed_ids = ledger.executed_items();
        let mut keep: BTreeSet<String> = BTreeSet::new();
        for id in &executed_ids {
            if corpus.get(id).is_none() {
                return Err(PlacementError::ExecutedItemNotEnumerated(id.clone()));
            }
            let mut cursor = id.clone();
            loop {
                if !keep.insert(cursor.clone()) {
                    break;
                }
                let item = corpus
                    .get(&cursor)
                    .ok_or_else(|| PlacementError::ExecutedItemNotEnumerated(cursor.clone()))?;
                match item.derived_from.clone() {
                    Some(parent) => cursor = parent,
                    None => break,
                }
            }
        }

        let mut executed_corpus = Corpus::new();
        let kept: Vec<GeneratedItem> = corpus
            .iter()
            .filter(|item| keep.contains(&item.id))
            .cloned()
            .collect();
        executed_corpus.extend(kept);

        let executed = EffectiveSizeReport::measure(&executed_corpus)
            .map_err(|error| PlacementError::ExecutedSubset(error.to_string()))?;
        let enumerated = EffectiveSizeReport::measure(corpus)
            .map_err(|error| PlacementError::ExecutedSubset(error.to_string()))?;

        Ok(ExecutedReport {
            executed,
            enumerated,
            items_never_executed: corpus.len().saturating_sub(keep.len()),
        })
    }

    /// The sentence a run report leads with.
    ///
    /// Delegates to `bioprism-scale`'s own headline, which already ends with "Instance count is not
    /// benchmark count", and prefixes the subset qualifier section 35 requires. There is no
    /// formatting path here that emits a size without the relation that produced it.
    pub fn headline(&self) -> String {
        format!(
            "executed subset: {} ({} enumerated item(s) never ran and are not evidence)",
            self.executed.headline(),
            self.items_never_executed
        )
    }
}
