//! Protected closure.
//!
//! Blueprint 43.13: compute a mandatory closure of evidence *before* any relevance, compression,
//! abstraction or model-driven selection runs. 39.05 enumerates the non-compressible classes —
//! identity, time, units and reference frames, provenance and versions, cohort and split
//! structure, uncertainty and detection limits, contradictory and negative evidence, privacy and
//! consent, and the observation/interpretation/claim distinction.
//!
//! The ordering is the point. A relevance heuristic that runs first can drop the duplicate-alias
//! record that makes the whole split invalid, and no downstream step can recover it. Closure is
//! computed by tag membership and unioned into the selection regardless of what the slice found.

use bioprism_world::WorldSource;
use std::collections::BTreeSet;

/// Facts carrying at least one protected tag.
pub fn protected_closure<S: WorldSource + ?Sized>(
    source: &S,
    protected_tags: &BTreeSet<String>,
) -> BTreeSet<String> {
    source.fact_ids_with_any_tag(protected_tags)
}

/// Protected tags that no fact in the world carries.
///
/// A query asking to protect `consent` in a world where nothing is tagged `consent` has not
/// protected anything, and a certificate that silently reports an empty closure for that tag
/// would overstate its guarantees.
pub fn unmatched_tags<S: WorldSource + ?Sized>(
    source: &S,
    protected_tags: &BTreeSet<String>,
) -> Vec<String> {
    protected_tags
        .iter()
        .filter(|tag| source.count_with_tag(tag) == 0)
        .cloned()
        .collect()
}

/// Protected facts that the temporal cut or a policy filter removed from the selection.
///
/// 43.13 treats this as a blocking condition rather than a warning: the closure was declared
/// mandatory, so a compile that could not deliver it has not satisfied the contract and the
/// consumer must refine or abstain.
pub fn dropped_protected(
    protected: &BTreeSet<String>,
    selected: &BTreeSet<String>,
) -> Vec<String> {
    protected.difference(selected).cloned().collect()
}
