//! Token profiles and the cost estimator (41.06).
//!
//! 41.06 defines five loading depths — handle, card, brief, contract, deep reference — and three
//! invariants: "actual counts may be measured with target tokenizers", "estimated cost is
//! disclosed", and "routes remain valid under their declared budget or fail explicitly".
//!
//! # There is no tokenizer here, and the type says so
//!
//! This workspace is offline and has no tokenizer dependency, so every number this module
//! produces is an estimate. That fact is carried in the type rather than in a comment:
//! [`TokenCost`] has no bare constructor, [`TokenCost::estimate`] stamps [`CostBasis::Estimated`]
//! with the estimator's id, and [`TokenCost::measured`] cannot be called without naming the
//! tokenizer that did the measuring. Nothing in this crate calls `measured`; the constructor
//! exists so a caller that *does* have a tokenizer can upgrade a profile without the two kinds of
//! number ever becoming interchangeable.
//!
//! Addition takes the weaker basis ([`TokenCost::sum`]): a total containing one estimate is an
//! estimate. This is the same rule `bioprism-section` applies to sufficiency — one unknown group
//! voids the claim — applied to arithmetic.
//!
//! # The estimator
//!
//! [`ESTIMATOR_ID`] names the rule so a stored profile can be invalidated when the rule changes.
//! The rule is the four-characters-per-token heuristic, floored at one token per whitespace-run,
//! because the heuristic alone badly under-counts the short-token text that documentation is full
//! of (`|`, `-`, `##`, ids like `41.03`). It is deterministic, allocation-free, and wrong in a
//! stated direction: it is a *ceiling-ish* estimate for prose and a floor for dense tables. It is
//! not calibrated against any real tokenizer, because calibrating against one we cannot run would
//! be a fabricated measurement.

use serde::{Deserialize, Serialize};

/// Identifier of the estimation rule, versioned so stored costs can be invalidated.
pub const ESTIMATOR_ID: &str = "chars4-words-floor/v1";

/// Estimate the token cost of a string.
///
/// `max(ceil(chars / 4), whitespace_runs)`. Counted in `char`s rather than bytes so the number
/// does not change when a document gains an em dash.
pub fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    let words = text.split_whitespace().count() as u32;
    chars.div_ceil(4).max(words)
}

/// Where a token count came from. Serialised, so a persisted profile cannot lose the distinction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "basis")]
pub enum CostBasis {
    /// Produced by [`estimate_tokens`] under the named rule. Never present it as a count.
    Estimated { estimator: String },
    /// Produced by running a named tokenizer over the exact bytes.
    Measured { tokenizer: String },
}

impl CostBasis {
    pub fn is_measurement(&self) -> bool {
        matches!(self, CostBasis::Measured { .. })
    }
}

/// A token count together with the reason it is believable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCost {
    pub tokens: u32,
    pub basis: CostBasis,
}

impl TokenCost {
    pub fn estimate(text: &str) -> Self {
        TokenCost {
            tokens: estimate_tokens(text),
            basis: CostBasis::Estimated {
                estimator: ESTIMATOR_ID.to_string(),
            },
        }
    }

    /// A count from a real tokenizer. Requires naming it; there is no way to record a measurement
    /// anonymously.
    pub fn measured(tokens: u32, tokenizer: impl Into<String>) -> Self {
        TokenCost {
            tokens,
            basis: CostBasis::Measured {
                tokenizer: tokenizer.into(),
            },
        }
    }

    pub fn zero_estimate() -> Self {
        TokenCost {
            tokens: 0,
            basis: CostBasis::Estimated {
                estimator: ESTIMATOR_ID.to_string(),
            },
        }
    }

    pub fn is_measurement(&self) -> bool {
        self.basis.is_measurement()
    }

    /// Total of many costs, carrying the weaker basis.
    ///
    /// A sum of one measurement and one estimate is an estimate. Reporting it as measured would
    /// let an unmeasured module hide inside a measured route total.
    pub fn sum<'a, I: IntoIterator<Item = &'a TokenCost>>(costs: I) -> Self {
        let mut total = 0u32;
        let mut tokenizer: Option<String> = None;
        let mut all_measured = true;
        for cost in costs {
            total = total.saturating_add(cost.tokens);
            match &cost.basis {
                CostBasis::Measured { tokenizer: name } => match &tokenizer {
                    Some(existing) if existing != name => all_measured = false,
                    Some(_) => {}
                    None => tokenizer = Some(name.clone()),
                },
                CostBasis::Estimated { .. } => all_measured = false,
            }
        }
        match (all_measured, tokenizer) {
            (true, Some(name)) => TokenCost::measured(total, name),
            _ => TokenCost {
                tokens: total,
                basis: CostBasis::Estimated {
                    estimator: ESTIMATOR_ID.to_string(),
                },
            },
        }
    }
}

/// The five loading depths of 41.06.
///
/// Ordered: a higher level is strictly more text. [`ProfileLevel::Card`] is the level 41.04 says
/// an agent should see "before they decide to load the full file", and it is the default for
/// context nodes that a bundle needs to *mention* rather than to quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileLevel {
    /// L0 — identity only: module id, title, path. Enough to cite, not enough to reason from.
    Handle,
    /// L1 — the 41.04 context card.
    Card,
    /// L2 — the module's own summary of its decision.
    Brief,
    /// L3 — the normative text. The only level from which an obligation may be read.
    Contract,
    /// L4 — contract plus appendices, derivations, tables.
    DeepReference,
}

impl ProfileLevel {
    pub const ALL: [ProfileLevel; 5] = [
        ProfileLevel::Handle,
        ProfileLevel::Card,
        ProfileLevel::Brief,
        ProfileLevel::Contract,
        ProfileLevel::DeepReference,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProfileLevel::Handle => "L0",
            ProfileLevel::Card => "L1",
            ProfileLevel::Brief => "L2",
            ProfileLevel::Contract => "L3",
            ProfileLevel::DeepReference => "L4",
        }
    }

    /// Parse the `token_profile:` front-matter field the blueprint modules carry.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().trim_matches('"') {
            "L0" => Some(ProfileLevel::Handle),
            "L1" => Some(ProfileLevel::Card),
            "L2" => Some(ProfileLevel::Brief),
            "L3" => Some(ProfileLevel::Contract),
            "L4" => Some(ProfileLevel::DeepReference),
            _ => None,
        }
    }

    /// Whether text at this level may be cited as normative.
    ///
    /// 41.04: "cards never replace normative contracts". A card is a lossy rendering written by
    /// the registry, so an obligation read out of one is an obligation read out of a summary.
    pub fn is_normative(self) -> bool {
        matches!(self, ProfileLevel::Contract | ProfileLevel::DeepReference)
    }
}
