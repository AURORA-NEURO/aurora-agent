//! The claim-evidence graph: which claim rests on what, and what contradicts it (26.03).
//!
//! 26.03's design detail refuses the obvious metric first: "The score is not based on citation
//! count. A single correctly localized artifact can be sufficient; many irrelevant citations can
//! reduce precision and context efficiency." So the object here is a graph with typed edges, and
//! the reported quantity is a partition of claims into states — not a ratio, and never a count of
//! citations.
//!
//! # Support and contradiction are both edges
//!
//! 26.03 lists "claim-support and contradiction edges" as an evaluation target and
//! "contradiction acknowledgment" as a metric. A claim with three supporting citations and one
//! contradicting one is not 75% grounded; it is a *contested* claim, and
//! [`ClaimState::Contested`] is a separate state from [`ClaimState::Supported`]. Averaging the
//! edges is exactly the collapse this workspace keeps refusing in other currencies — a contested
//! claim that reads as mostly-supported has lost the finding.
//!
//! # Staleness is measured against a freeze, not against now
//!
//! 26.03's last failure mode is "source updated after benchmark freeze". That is a comparison
//! between two recorded times, so [`Grounding::stale_against`] takes the freeze as an argument.
//! Nothing in this crate reads a clock: a staleness verdict that depends on when the test ran is
//! not reproducible, and reproducibility is the neighbouring module's whole subject.
//!
//! # Not implemented
//!
//! No locator resolution. 26.03 asks to "verify locators and hashes"; `bioprism-bioir` already
//! says that no artifact-shape contract exists to resolve a locator against and checks internal
//! well-formedness only. This module records whether a caller resolved a locator
//! ([`Evidence::locator_status`]) and refuses to treat `NotChecked` as resolved — the same
//! `NotChecked`-as-a-real-state move `bioprism-safety` makes for signatures.
//!
//! No claim extraction. 26.03's step 1 is "extract atomic claims" from prose, which is a language
//! task, not a predicate over an artifact; claims arrive here already atomised.
//!
//! No assay-compatibility check (step 4). Deciding that an assay measured a compatible analyte
//! needs the assay ontology `bioprism-bioir` deliberately does not have.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};

use crate::error::GroundingError;

const MAX_GROUNDING_TEXT_BYTES: usize = 256;
const MAX_CLAIMS: usize = 8192;
const MAX_EVIDENCE: usize = 8192;
const MAX_EDGES: usize = 16384;
const MAX_LINEAGE: usize = 256;

/// Whether anyone actually checked that an evidence locator resolves.
///
/// Three states, and the middle one is the point: "nobody checked" is not "resolves".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "locator")]
pub enum LocatorStatus {
    /// The locator was dereferenced and the artifact was there with the expected digest.
    Resolved { digest: String },
    /// Nobody dereferenced it. Not evidence of absence, and not evidence of presence.
    NotChecked,
    /// It was dereferenced and did not resolve.
    Unresolvable { detail: String },
}

impl LocatorStatus {
    /// Whether this locator has been shown to point at something.
    pub fn is_resolved(&self) -> bool {
        matches!(self, LocatorStatus::Resolved { .. })
    }
}

/// One evidence object a claim can rest on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    /// The chain from this artifact back to a specimen, if it has one. 26.03's "lineage
    /// completeness"; an empty chain on a derived file is its "derived file loses specimen
    /// ancestor" failure mode.
    #[serde(default)]
    pub lineage: Vec<String>,
    /// When the source last changed. Compared against a freeze, never against a wall clock.
    pub last_modified: Timestamp,
    pub locator_status: LocatorStatus,
}

impl Evidence {
    /// Declare an evidence object whose locator nobody has checked yet.
    pub fn declared(id: impl Into<String>, last_modified: Timestamp) -> Self {
        Evidence {
            id: id.into(),
            lineage: Vec::new(),
            last_modified,
            locator_status: LocatorStatus::NotChecked,
        }
    }

    /// Record that the locator resolved to an artifact with this digest.
    pub fn resolving_to(mut self, digest: impl Into<String>) -> Self {
        self.locator_status = LocatorStatus::Resolved {
            digest: digest.into(),
        };
        self
    }

    /// Record the specimen ancestry chain.
    pub fn with_lineage(mut self, lineage: Vec<String>) -> Self {
        self.lineage = lineage;
        self
    }
}

/// What an edge says about the claim it points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// This evidence supports the claim.
    Supports,
    /// This evidence contradicts the claim.
    Contradicts,
    /// This evidence was cited for the claim and does not bear on it — 26.03's "citation supports
    /// adjacent but not actual claim". A distinct state, because it is the failure most likely to
    /// be mistaken for support by a counting metric.
    Adjacent,
}

/// One typed edge from evidence to claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportEdge {
    pub claim: String,
    pub evidence: String,
    pub kind: EdgeKind,
}

/// What the graph says about one claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ClaimState {
    /// At least one resolved supporting edge and no contradicting edge.
    Supported,
    /// Supporting and contradicting edges both present. Not a fraction — a finding.
    Contested,
    /// Only contradicting edges.
    Contradicted,
    /// No supporting or contradicting edges at all, or only adjacent ones.
    Unsupported,
    /// Supporting edges exist but none of them has a resolved locator, so the support is asserted
    /// rather than shown.
    SupportUnverified,
}

/// A claim, its evidence, and the edges between them.
/// A derived claim-evidence graph. It serializes for reporting but is intentionally rebuilt
/// through the checked admission methods so persisted edges cannot bypass endpoint validation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Grounding {
    claims: BTreeSet<String>,
    evidence: BTreeMap<String, Evidence>,
    edges: Vec<SupportEdge>,
}

impl Grounding {
    /// An empty graph.
    pub fn new() -> Self {
        Grounding::default()
    }

    /// Add an atomic claim.
    pub fn claim(&mut self, id: impl Into<String>) -> Result<(), GroundingError> {
        let id = id.into();
        validate_grounding_text(&id).map_err(|detail| GroundingError::InvalidClaim {
            claim: id.clone(),
            detail,
        })?;
        if self.claims.len() >= MAX_CLAIMS {
            return Err(GroundingError::TooManyClaims { limit: MAX_CLAIMS });
        }
        if !self.claims.insert(id.clone()) {
            return Err(GroundingError::DuplicateClaim(id));
        }
        Ok(())
    }

    /// Add an evidence object.
    pub fn evidence(&mut self, evidence: Evidence) -> Result<(), GroundingError> {
        validate_evidence(&evidence)?;
        if self.evidence.len() >= MAX_EVIDENCE {
            return Err(GroundingError::TooManyEvidence {
                limit: MAX_EVIDENCE,
            });
        }
        if self.evidence.contains_key(&evidence.id) {
            return Err(GroundingError::DuplicateEvidence(evidence.id));
        }
        self.evidence.insert(evidence.id.clone(), evidence);
        Ok(())
    }

    /// Add an edge, refusing one whose endpoints are not both declared.
    pub fn link(&mut self, edge: SupportEdge) -> Result<(), GroundingError> {
        validate_edge(&edge)?;
        if !self.claims.contains(&edge.claim) {
            return Err(GroundingError::UnknownClaim(edge.claim));
        }
        if !self.evidence.contains_key(&edge.evidence) {
            return Err(GroundingError::UnknownEvidence(edge.evidence));
        }
        if self.edges.len() >= MAX_EDGES {
            return Err(GroundingError::TooManyEdges { limit: MAX_EDGES });
        }
        if self.edges.iter().any(|existing| existing == &edge) {
            return Err(GroundingError::DuplicateEdge {
                claim: edge.claim,
                evidence: edge.evidence,
                kind: format!("{:?}", edge.kind),
            });
        }
        self.edges.push(edge);
        Ok(())
    }

    /// Classify one claim.
    pub fn state(&self, claim: &str) -> Option<ClaimState> {
        if !self.claims.contains(claim) {
            return None;
        }
        let mut supported = false;
        let mut supported_resolved = false;
        let mut contradicted = false;
        for edge in self.edges.iter().filter(|e| e.claim == claim) {
            let resolved = self
                .evidence
                .get(&edge.evidence)
                .map(|e| e.locator_status.is_resolved())
                .unwrap_or(false);
            match edge.kind {
                EdgeKind::Supports => {
                    supported = true;
                    supported_resolved |= resolved;
                }
                EdgeKind::Contradicts => contradicted = true,
                EdgeKind::Adjacent => {}
            }
        }
        Some(match (supported, supported_resolved, contradicted) {
            (true, _, true) => ClaimState::Contested,
            (true, true, false) => ClaimState::Supported,
            (true, false, false) => ClaimState::SupportUnverified,
            (false, _, true) => ClaimState::Contradicted,
            (false, _, false) => ClaimState::Unsupported,
        })
    }

    /// Every claim's state, in claim order.
    pub fn states(&self) -> BTreeMap<&str, ClaimState> {
        self.claims
            .iter()
            .filter_map(|claim| self.state(claim).map(|state| (claim.as_str(), state)))
            .collect()
    }

    /// A census of claim states.
    ///
    /// A partition, not a ratio: 26.03's metric list names "grounded-claim precision" and
    /// "evidence recall" and defines neither denominator, and there is no honest way to pick one
    /// for a caller. What is reported is the five counts, which is strictly more information than
    /// any ratio derived from them.
    pub fn census(&self) -> Census {
        let mut census = Census::default();
        for state in self.states().into_values() {
            match state {
                ClaimState::Supported => census.supported += 1,
                ClaimState::Contested => census.contested += 1,
                ClaimState::Contradicted => census.contradicted += 1,
                ClaimState::Unsupported => census.unsupported += 1,
                ClaimState::SupportUnverified => census.support_unverified += 1,
            }
        }
        census.adjacent_citations = self
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Adjacent)
            .count();
        census
    }

    /// Evidence whose source changed after the benchmark was frozen.
    ///
    /// Returns the ids, so a caller can name them. 26.03 calls this "stale-evidence rate"; the
    /// rate is a division a caller can do once it has decided what the denominator is.
    pub fn stale_against(&self, freeze: Timestamp) -> Vec<&str> {
        self.evidence
            .values()
            .filter(|e| e.last_modified > freeze)
            .map(|e| e.id.as_str())
            .collect()
    }

    /// Evidence objects with no recorded ancestry.
    pub fn lineage_gaps(&self) -> Vec<&str> {
        self.evidence
            .values()
            .filter(|e| e.lineage.is_empty())
            .map(|e| e.id.as_str())
            .collect()
    }

    /// The edges, in insertion order.
    pub fn edges(&self) -> &[SupportEdge] {
        &self.edges
    }
}

fn validate_grounding_text(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_GROUNDING_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err("value must be a bounded, trimmed, control-free string".into());
    }
    Ok(())
}

fn validate_evidence(evidence: &Evidence) -> Result<(), GroundingError> {
    validate_grounding_text(&evidence.id).map_err(|detail| GroundingError::InvalidEvidence {
        evidence: evidence.id.clone(),
        detail,
    })?;
    if evidence.lineage.len() > MAX_LINEAGE {
        return Err(GroundingError::InvalidEvidence {
            evidence: evidence.id.clone(),
            detail: format!("lineage may contain at most {MAX_LINEAGE} ancestors"),
        });
    }
    let mut ancestors = BTreeSet::new();
    for ancestor in &evidence.lineage {
        validate_grounding_text(ancestor).map_err(|detail| GroundingError::InvalidEvidence {
            evidence: evidence.id.clone(),
            detail: format!("lineage entry is invalid: {detail}"),
        })?;
        if !ancestors.insert(ancestor.clone()) {
            return Err(GroundingError::InvalidEvidence {
                evidence: evidence.id.clone(),
                detail: "lineage entries must be unique".into(),
            });
        }
    }
    match &evidence.locator_status {
        LocatorStatus::Resolved { digest } => {
            validate_grounding_text(digest).map_err(|detail| GroundingError::InvalidEvidence {
                evidence: evidence.id.clone(),
                detail: format!("resolved digest is invalid: {detail}"),
            })?;
        }
        LocatorStatus::Unresolvable { detail } => {
            validate_grounding_text(detail).map_err(|detail| GroundingError::InvalidEvidence {
                evidence: evidence.id.clone(),
                detail: format!("unresolvable detail is invalid: {detail}"),
            })?;
        }
        LocatorStatus::NotChecked => {}
    }
    Ok(())
}

fn validate_edge(edge: &SupportEdge) -> Result<(), GroundingError> {
    validate_grounding_text(&edge.claim).map_err(|detail| GroundingError::InvalidEdge {
        claim: edge.claim.clone(),
        evidence: edge.evidence.clone(),
        detail: format!("claim endpoint is invalid: {detail}"),
    })?;
    validate_grounding_text(&edge.evidence).map_err(|detail| GroundingError::InvalidEdge {
        claim: edge.claim.clone(),
        evidence: edge.evidence.clone(),
        detail: format!("evidence endpoint is invalid: {detail}"),
    })?;
    Ok(())
}

/// The partition of claims by state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Census {
    pub supported: usize,
    pub contested: usize,
    pub contradicted: usize,
    pub unsupported: usize,
    pub support_unverified: usize,
    /// Citations that were offered and bear on nothing. 26.03: "many irrelevant citations can
    /// reduce precision", so this is reported beside the claim states rather than folded into them.
    pub adjacent_citations: usize,
}

impl Census {
    /// How many claims were classified.
    pub fn claims(&self) -> usize {
        self.supported
            + self.contested
            + self.contradicted
            + self.unsupported
            + self.support_unverified
    }

    /// Whether every claim reached [`ClaimState::Supported`].
    ///
    /// The only predicate offered over a census, and it is deliberately all-or-nothing: a release
    /// gate that reads "87% grounded" has to decide what the other 13% were, and this crate will
    /// not decide that on a caller's behalf.
    pub fn fully_grounded(&self) -> bool {
        self.claims() > 0 && self.supported == self.claims()
    }
}
