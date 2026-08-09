//! The agent reading protocol (41.12).
//!
//! 41.12 defines what a coding agent, research agent or reviewer must do with this corpus:
//! "resolve route, load cards, expand contracts, record loaded IDs, cite paths/headings, report
//! unresolved obligations", under the invariants that "agents never imply implementation from
//! specification", "private reasoning is not required", and "clinical boundaries remain active".
//!
//! # A protocol nobody can check is a suggestion
//!
//! The blueprint states the protocol as prose an agent is expected to follow. Prose is not
//! enforceable, so this module makes the agent's side of it an artifact: a [`ReadingReceipt`]
//! records which modules were loaded, at which depth, which headings were cited, and which
//! obligations were left open. [`check_receipt`] then compares it against the bundle that was
//! actually delivered and returns [`ProtocolViolation`]s.
//!
//! Three of the checks are worth naming.
//!
//! **Implication of implementation from specification.** A [`Claim`] tagged
//! [`ClaimKind::WhatIsBuilt`] sourced from a node whose status is
//! [`NodeStatus::Specification`](crate::registry::NodeStatus::Specification) is a violation, full
//! stop. This is the single most common failure mode of an agent reading a build-ready
//! specification corpus: the document says what the system must do, in the present tense, and the
//! agent reports it as what the system does.
//!
//! **Citing a heading that does not exist.** A citation naming a heading absent from the module's
//! own text is caught by re-reading the delivered text, so a fabricated section reference is a
//! typed finding rather than something a human notices later.
//!
//! **Citing a non-normative rendering as normative.** An obligation cited from a
//! [`ProfileLevel::Card`] is an obligation read out of a summary this crate wrote. 41.04: "cards
//! never replace normative contracts".
//!
//! # Not implemented
//!
//! Nothing here inspects reasoning. 41.12 says "private reasoning is not required" and this
//! module takes that literally: the receipt records what was *loaded* and what was *claimed*, and
//! there is no field for why. A protocol that demanded a rationale would be unverifiable in the
//! same way the prose version is.

use crate::bundle::ContextBundle;
use crate::markdown::headings;
use crate::registry::{DocGraph, ModuleId, NodeStatus};
use crate::route::RouteId;
use crate::tokens::ProfileLevel;
use serde::{Deserialize, Serialize};

/// What a claim is about. The distinction 41.12's first invariant turns on, and 39.05's ninth
/// protected class ("observation, interpretation, hypothesis, causal claim, recommendation").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// "The specification requires X." Safe from any node.
    WhatIsSpecified,
    /// "The system does X." Only sourced honestly from an implementation or tool node.
    WhatIsBuilt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub statement: String,
    pub sourced_from: ModuleId,
    pub kind: ClaimKind,
}

/// A path-and-heading citation, as 41.12 requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub module: ModuleId,
    pub heading: String,
    /// The depth the text was read at. An obligation cited from a non-normative level is flagged.
    pub level: ProfileLevel,
}

/// What an agent did with a bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingReceipt {
    pub route: RouteId,
    pub loaded: Vec<(ModuleId, ProfileLevel)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Citation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<Claim>,
    /// 41.12: "report unresolved obligations". Empty means the agent asserts there are none, which
    /// is itself a claim [`check_receipt`] can contradict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_obligations: Vec<String>,
}

impl ReadingReceipt {
    pub fn new(route: RouteId) -> Self {
        ReadingReceipt {
            route,
            loaded: Vec::new(),
            citations: Vec::new(),
            claims: Vec::new(),
            unresolved_obligations: Vec::new(),
        }
    }

    pub fn loading(mut self, module: ModuleId, level: ProfileLevel) -> Self {
        self.loaded.push((module, level));
        self
    }

    pub fn citing(mut self, citation: Citation) -> Self {
        self.citations.push(citation);
        self
    }

    pub fn claiming(mut self, claim: Claim) -> Self {
        self.claims.push(claim);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "violation")]
pub enum ProtocolViolation {
    /// A mandatory module of the bundle was never loaded.
    MandatoryModuleNotLoaded { module: ModuleId },
    /// A module was loaded that the bundle did not deliver. Not pedantry: the agent read
    /// something outside the compiled context, so the bundle's omission record no longer
    /// describes what informed the answer.
    LoadedOutsideBundle { module: ModuleId },
    /// Loaded at a shallower level than delivered, then cited as though it were the contract.
    CitedBelowDeliveredLevel {
        module: ModuleId,
        cited_at: ProfileLevel,
        delivered_at: ProfileLevel,
    },
    /// 41.12's first invariant.
    ImpliedImplementationFromSpecification { module: ModuleId, statement: String },
    /// A heading that is not in the module's text.
    CitedMissingHeading { module: ModuleId, heading: String },
    /// An obligation cited from a card or handle. 41.04: cards never replace contracts.
    CitedNonNormativeLevel { module: ModuleId, level: ProfileLevel },
    /// The bundle was not sufficient and the receipt reported no unresolved obligations.
    SufficiencyGapNotReported { blocking: Vec<String> },
}

/// Compare a receipt against the bundle that was delivered.
///
/// Returns findings rather than a bool: a reviewer wants every violation at once, and a receipt
/// with three problems is a different situation from one with one.
pub fn check_receipt(
    graph: &DocGraph,
    bundle: &ContextBundle,
    receipt: &ReadingReceipt,
) -> Vec<ProtocolViolation> {
    let mut violations = Vec::new();

    for module in bundle.mandatory_ids() {
        if !receipt.loaded.iter().any(|(id, _)| id == module) {
            violations.push(ProtocolViolation::MandatoryModuleNotLoaded {
                module: module.clone(),
            });
        }
    }

    for (module, level) in &receipt.loaded {
        let Some(entry) = bundle.entries.iter().find(|entry| &entry.module == module) else {
            violations.push(ProtocolViolation::LoadedOutsideBundle {
                module: module.clone(),
            });
            continue;
        };
        if *level > entry.level {
            violations.push(ProtocolViolation::CitedBelowDeliveredLevel {
                module: module.clone(),
                cited_at: *level,
                delivered_at: entry.level,
            });
        }
    }

    for citation in &receipt.citations {
        if !citation.level.is_normative() {
            violations.push(ProtocolViolation::CitedNonNormativeLevel {
                module: citation.module.clone(),
                level: citation.level,
            });
        }
        let found = graph
            .node(&citation.module)
            .and_then(|node| node.text_at(citation.level))
            .map(|text| {
                headings(&text)
                    .iter()
                    .any(|heading| heading.text == citation.heading)
            })
            .unwrap_or(false);
        if !found {
            violations.push(ProtocolViolation::CitedMissingHeading {
                module: citation.module.clone(),
                heading: citation.heading.clone(),
            });
        }
    }

    for claim in &receipt.claims {
        let status = graph.node(&claim.sourced_from).map(|node| node.status);
        if claim.kind == ClaimKind::WhatIsBuilt && status == Some(NodeStatus::Specification) {
            violations.push(ProtocolViolation::ImpliedImplementationFromSpecification {
                module: claim.sourced_from.clone(),
                statement: claim.statement.clone(),
            });
        }
    }

    if let crate::bundle::Sufficiency::NotSufficient { blocking } = &bundle.sufficiency {
        if receipt.unresolved_obligations.is_empty() {
            violations.push(ProtocolViolation::SufficiencyGapNotReported {
                blocking: blocking.clone(),
            });
        }
    }

    violations
}
