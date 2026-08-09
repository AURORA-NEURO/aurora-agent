//! Quotations pinned to their source, so a reword elsewhere fails here.
//!
//! This crate paraphrases three things it does not own: `AGENTS.md`'s house rules, the blocker
//! strings in `bioprism-examples`, and the measurements in `README.md`. Paraphrase rots. A
//! sentence in `AGENTS.md` gets sharpened, a blocker gets fixed and its text deleted, a number in
//! the README gets remeasured — and a cookbook that copied the old wording goes on asserting it,
//! with no mechanism anywhere that notices.
//!
//! A [`PinnedQuote`] is the mechanism. It carries the workspace-relative path it came from and the
//! exact substring it is quoting, and [`PinnedQuote::still_present_in`] fails when that substring
//! is no longer there. The needle is a *fragment*, not the whole sentence, because a fragment
//! survives reflowing a paragraph and dies when the claim changes — which is the discrimination
//! wanted. A whole-line needle would break on every line-wrap edit and be deleted within a month.
//!
//! # Why this crate quotes `crates/examples` as text rather than depending on it
//!
//! `bioprism-examples` owns the vertical slices and the blocked-claim registry, and this crate
//! must not take a dependency on it: the two would then be one deployment unit, and a cookbook
//! that cannot be built while `bioprism-examples` is mid-edit is a cookbook unavailable exactly
//! when a reader is trying to find out what the platform does. So the relationship is textual and
//! the pin is what keeps it honest. `bioprism-bioworlds` reached the same arrangement with the
//! same crate for the same reason.
//!
//! # Not implemented
//!
//! No fuzzy matching, no normalisation of whitespace or quote characters. A pin that tolerated
//! near-matches would tolerate exactly the small edit that changes a claim's meaning.

use serde::{Deserialize, Serialize};

/// A quotation together with the file it must still appear in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinnedQuote {
    /// Workspace-relative, forward-slashed.
    pub source: String,
    /// The exact substring. Verified byte-for-byte.
    pub needle: String,
}

impl PinnedQuote {
    pub fn new(source: impl Into<String>, needle: impl Into<String>) -> Self {
        PinnedQuote {
            source: source.into(),
            needle: needle.into(),
        }
    }

    /// Whether the quoted text is still in the given file contents.
    pub fn still_present_in(&self, contents: &str) -> bool {
        contents.contains(&self.needle)
    }
}

/// The house rules in `AGENTS.md` that this cookbook's anti-recipes sharpen.
///
/// Named individually rather than collected in a list constant, because each one is cited from a
/// specific anti-recipe and a reader following the citation should land on the sentence, not on a
/// bag of sentences.
pub mod agents {
    use super::PinnedQuote;

    const AGENTS: &str = "AGENTS.md";

    pub fn admissibility() -> PinnedQuote {
        PinnedQuote::new(
            AGENTS,
            "**Admissible** — right verdict *and* full protected closure. Cheapness alone is not a win.",
        )
    }

    pub fn incomplete_basis() -> PinnedQuote {
        PinnedQuote::new(
            AGENTS,
            "**A right answer from an incomplete basis is not a pass.**",
        )
    }

    pub fn instance_count() -> PinnedQuote {
        PinnedQuote::new(AGENTS, "**Instance count is not benchmark count.**")
    }

    pub fn zero_is_not_unknown() -> PinnedQuote {
        PinnedQuote::new(AGENTS, "**Zero influence is not unknown influence.**")
    }

    pub fn unmeasured_is_not_zero() -> PinnedQuote {
        PinnedQuote::new(AGENTS, "**Unmeasured is not zero.**")
    }

    pub fn no_score_or_zero() -> PinnedQuote {
        PinnedQuote::new(AGENTS, "There is no `score_or_zero`")
    }
}

/// Measurements this cookbook repeats from `README.md`.
///
/// Repeated at all only because two anti-recipes are about *reading* them wrongly, and an
/// anti-recipe that did not state the number would be asking a reader to take the misreading on
/// trust. Every one of these is a number this crate did not measure.
pub mod readme {
    use super::PinnedQuote;

    const README: &str = "README.md";

    pub fn compiled_fact_count() -> PinnedQuote {
        PinnedQuote::new(README, "**11 facts (1.45% of the world)**")
    }

    pub fn baselines_tie_on_the_reference_world() -> PinnedQuote {
        PinnedQuote::new(README, "select *exactly the same eleven facts*")
    }

    pub fn lexical_closure_shortfall() -> PinnedQuote {
        PinnedQuote::new(README, "from a **91% protected closure**")
    }
}

/// Obstacles `bioprism-examples` records against the properties it cannot exercise.
///
/// Quoted, not restated: the wording is that crate's finding and it has the right to change it.
/// The pin is what turns "changed it" into a failing test here rather than into a cookbook that
/// quietly disagrees with the registry it points at.
pub mod examples_blockers {
    use super::PinnedQuote;

    const PROPERTY: &str = "crates/examples/src/property.rs";

    pub fn mutation_families() -> PinnedQuote {
        PinnedQuote::new(PROPERTY, "38.01 names six mutation families")
    }

    pub fn backend_portfolio() -> PinnedQuote {
        PinnedQuote::new(
            PROPERTY,
            "bioprism-fiber hard-codes Backend::BackwardFactorSliceReference",
        )
    }

    pub fn abstention() -> PinnedQuote {
        PinnedQuote::new(
            PROPERTY,
            "OracleVerdict::abstain exists in bioprism-section but no path in bioprism-fiber constructs it",
        )
    }

    pub fn decision_loss() -> PinnedQuote {
        PinnedQuote::new(
            PROPERTY,
            "fiber-query/0.1 carries neither permitted_actions nor decision_loss",
        )
    }
}

/// Every pin this crate ships, for the test that checks them all at once.
pub fn all() -> Vec<PinnedQuote> {
    vec![
        agents::admissibility(),
        agents::incomplete_basis(),
        agents::instance_count(),
        agents::zero_is_not_unknown(),
        agents::unmeasured_is_not_zero(),
        agents::no_score_or_zero(),
        readme::compiled_fact_count(),
        readme::baselines_tie_on_the_reference_world(),
        readme::lexical_closure_shortfall(),
        examples_blockers::mutation_families(),
        examples_blockers::backend_portfolio(),
        examples_blockers::abstention(),
        examples_blockers::decision_loss(),
    ]
}
