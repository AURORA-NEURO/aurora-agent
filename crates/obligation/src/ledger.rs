//! The omission ledger and the sufficiency certificate.
//!
//! Blueprint 39.17 exists to *distinguish "small" context from "sufficient" context*, and its
//! four invariants are the shape of this module:
//!
//! 1. every omitted mandatory candidate carries an explicit reason — [`OmissionLedger::validate`]
//!    rejects [`OmissionReason::Unclassified`] on a mandatory row;
//! 2. policy-redacted evidence is distinguished from irrelevant evidence — those are two
//!    different [`OmissionReason`] variants mapping to two different influence classes, and
//!    nothing collapses them into one token-saving phrase;
//! 3. sufficiency is scoped to a decision and a role — both are fields of
//!    [`SufficiencyCertificate`], and a capsule checks that they match its own;
//! 4. a certificate may be unknown or failed, and is never fabricated —
//!    [`SufficiencyStatus`] has all four outcomes and [`SufficiencyCertificate::recheck`] lets a
//!    consumer recompute a certificate it did not produce.
//!
//! ## Where the influence class comes from
//!
//! This module does not define its own relevance vocabulary. It reuses
//! [`bioprism_section::InfluenceClass`], whose central distinction is exactly the one a
//! sufficiency claim rests on: `Zero` means *no dependency path reaches the decision*, while
//! `Unknown` means *nobody checked*. An unmet obligation with no relevance record defaults to
//! `Unknown`, so silence can never produce a sufficient certificate. That default is the whole
//! safety property: the failure mode being defended against is a compiler that drops evidence for
//! size and reports a clean bill because nothing objected.
//!
//! Not implemented here: signatures. 39.17 wants the certificate signed with graph and compiler
//! hashes; the graph hash is bound, and attestation belongs to the release-gate machinery of
//! 39.22.

use crate::budget::TokenEstimate;
use crate::error::LedgerError;
use crate::graph::ObligationGraph;
use crate::invariants::ClosureReport;
use crate::state::ObligationState;
use bioprism_scope::Timestamp;
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Why a candidate was left out of the projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum OmissionReason {
    /// The compiler dropped it without classifying it. Representable so the failure is
    /// detectable, never legal for a mandatory candidate.
    Unclassified,
    /// No dependency path from this candidate reaches the decision.
    Irrelevant,
    /// Another selected node already carries the same information.
    Redundant { equivalent_to: String },
    /// It did not fit the token budget. A size decision, not a relevance finding.
    BudgetExceeded,
    /// Its value per token fell below the acquisition threshold.
    BelowAcquisitionThreshold,
    /// Policy, consent or residency forbade including it. Never the same thing as irrelevant.
    PolicyRedacted { policy: String },
    /// Not released before the decision's visibility cutoff.
    NotAvailableAtCut { visibility_cutoff: Timestamp },
}

impl OmissionReason {
    pub fn label(&self) -> &'static str {
        match self {
            OmissionReason::Unclassified => "unclassified",
            OmissionReason::Irrelevant => "irrelevant",
            OmissionReason::Redundant { .. } => "redundant",
            OmissionReason::BudgetExceeded => "budget_exceeded",
            OmissionReason::BelowAcquisitionThreshold => "below_acquisition_threshold",
            OmissionReason::PolicyRedacted { .. } => "policy_redacted",
            OmissionReason::NotAvailableAtCut { .. } => "not_available_at_cut",
        }
    }

    /// The influence class a reason implies before any explicit argument is supplied.
    ///
    /// `budget_exceeded` maps to `Unknown` rather than `Zero` on purpose. Dropping something
    /// because it was large says nothing about whether it mattered, and the honest default is
    /// that nobody checked.
    pub fn default_influence(&self) -> InfluenceClass {
        match self {
            OmissionReason::Irrelevant | OmissionReason::Redundant { .. } => InfluenceClass::Zero,
            OmissionReason::PolicyRedacted { .. } => InfluenceClass::InaccessibleByPolicy,
            OmissionReason::NotAvailableAtCut { .. } => InfluenceClass::DeferredAcquisition,
            OmissionReason::Unclassified
            | OmissionReason::BudgetExceeded
            | OmissionReason::BelowAcquisitionThreshold => InfluenceClass::Unknown,
        }
    }

    pub fn is_classified(&self) -> bool {
        !matches!(self, OmissionReason::Unclassified)
    }
}

/// One ledger row, keyed by the candidate node it describes.
///
/// A row that claims `Zero` or `Bounded` influence must carry the argument behind the claim.
/// `Redundant` supplies its own — the node it duplicates *is* the argument — but plain
/// irrelevance does not: asserting that nothing depends on a candidate is a statement about the
/// dependency graph, and it has to be made rather than assumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmittedCandidate {
    pub candidate_id: String,
    /// The obligation this candidate would have served, when it serves one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obligation: Option<String>,
    pub reason: OmissionReason,
    pub influence: InfluenceClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
    /// The argument behind a `Zero` or `Bounded` claim. Required for both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
    pub mandatory: bool,
    /// How to get it back, per 39.17's retrieval expansion actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_action: Option<String>,
    pub estimate: TokenEstimate,
}

impl OmittedCandidate {
    pub fn new(
        candidate_id: impl Into<String>,
        reason: OmissionReason,
        estimate: TokenEstimate,
    ) -> Self {
        let influence = reason.default_influence();
        let argument = match &reason {
            OmissionReason::Redundant { equivalent_to } => {
                Some(format!("carried by `{equivalent_to}`"))
            }
            _ => None,
        };
        OmittedCandidate {
            candidate_id: candidate_id.into(),
            obligation: None,
            reason,
            influence,
            bound: None,
            argument,
            mandatory: false,
            retrieval_action: None,
            estimate,
        }
    }

    pub fn for_obligation(mut self, obligation: impl Into<String>) -> Self {
        self.obligation = Some(obligation.into());
        self
    }

    pub fn mandatory(mut self) -> Self {
        self.mandatory = true;
        self
    }

    pub fn retrievable_by(mut self, action: impl Into<String>) -> Self {
        self.retrieval_action = Some(action.into());
        self
    }

    /// Asserts zero influence with the argument that justifies it.
    pub fn proven_irrelevant(mut self, argument: impl Into<String>) -> Self {
        self.influence = InfluenceClass::Zero;
        self.argument = Some(argument.into());
        self
    }

    /// Asserts bounded influence with the bound and the argument behind it.
    pub fn bounded(mut self, bound: f64, argument: impl Into<String>) -> Self {
        self.influence = InfluenceClass::Bounded;
        self.bound = Some(bound);
        self.argument = Some(argument.into());
        self
    }
}

/// Every compression decision, made auditable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OmissionLedger {
    rows: Vec<OmittedCandidate>,
}

impl OmissionLedger {
    pub fn push(&mut self, row: OmittedCandidate) {
        self.rows.push(row);
    }

    pub fn rows(&self) -> &[OmittedCandidate] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Rejects the failure classes 39.17 names: unclassified omissions, unreproducible policy
    /// reasons, and influence claims that assert more than their argument supports.
    pub fn validate(&self) -> Result<(), LedgerError> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for row in &self.rows {
            if !seen.insert(row.candidate_id.as_str()) {
                return Err(LedgerError::DuplicateCandidate(row.candidate_id.clone()));
            }

            if row.mandatory && !row.reason.is_classified() {
                return Err(LedgerError::UnclassifiedMandatoryOmission(
                    row.candidate_id.clone(),
                ));
            }
            if let OmissionReason::PolicyRedacted { policy } = &row.reason {
                if policy.trim().is_empty() {
                    return Err(LedgerError::PolicyReasonNotReproducible(
                        row.candidate_id.clone(),
                    ));
                }
            }
            if row.influence.supports_sufficiency()
                && row
                    .argument
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            {
                return Err(LedgerError::UnsupportedInfluenceClaim(
                    row.candidate_id.clone(),
                ));
            }
            if row.influence == InfluenceClass::Bounded && row.bound.is_none() {
                return Err(LedgerError::MissingInfluenceBound(row.candidate_id.clone()));
            }
        }
        Ok(())
    }

    /// Groups the rows into the manifest shape `bioprism_section` already defines.
    ///
    /// Grouping is by reason *and* influence, never by reason alone: two candidates dropped under
    /// the same policy, one of them with a bound and one without, are not the same omission.
    pub fn to_manifest(&self) -> OmissionManifest {
        let mut grouped: BTreeMap<(&str, &str), OmissionGroup> = BTreeMap::new();
        for row in &self.rows {
            let key = (row.reason.label(), row.influence.as_str());
            let group = grouped.entry(key).or_insert_with(|| OmissionGroup {
                reason: row.reason.label().to_string(),
                influence: row.influence,
                count: 0,
                bound: None,
                examples: Vec::new(),
            });
            group.count += 1;
            if let Some(bound) = row.bound {
                group.bound = Some(group.bound.map_or(bound, |current: f64| current.max(bound)));
            }
            if group.examples.len() < 3 {
                group.examples.push(row.candidate_id.clone());
            }
        }
        OmissionManifest {
            groups: grouped.into_values().collect(),
        }
    }

    /// Retrieval expansion actions for everything that can still be fetched.
    pub fn retrieval_actions(&self) -> Vec<&str> {
        self.rows
            .iter()
            .filter_map(|row| row.retrieval_action.as_deref())
            .collect()
    }

    pub fn policy_redacted(&self) -> impl Iterator<Item = &OmittedCandidate> {
        self.rows
            .iter()
            .filter(|row| matches!(row.reason, OmissionReason::PolicyRedacted { .. }))
    }

    /// Tokens not spent because of these omissions, labelled with the estimator behind the sum.
    pub fn estimated_tokens_withheld(&self) -> TokenEstimate {
        TokenEstimate::sum(self.rows.iter().map(|row| &row.estimate))
    }
}

/// A stated argument about how much an unmet obligation can move the decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelevanceRecord {
    pub obligation: String,
    pub influence: InfluenceClass,
    /// Why the class holds. A class without an argument is an assertion, not a proof.
    pub argument: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
}

impl RelevanceRecord {
    /// No dependency path from this obligation reaches the decision.
    pub fn irrelevant(obligation: impl Into<String>, argument: impl Into<String>) -> Self {
        RelevanceRecord {
            obligation: obligation.into(),
            influence: InfluenceClass::Zero,
            argument: argument.into(),
            bound: None,
        }
    }

    /// The obligation can move the decision, by at most `bound`.
    pub fn bounded(obligation: impl Into<String>, bound: f64, argument: impl Into<String>) -> Self {
        RelevanceRecord {
            obligation: obligation.into(),
            influence: InfluenceClass::Bounded,
            argument: argument.into(),
            bound: Some(bound),
        }
    }
}

/// Relevance arguments, keyed by obligation. Absence means `Unknown`, never `Zero`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RelevanceIndex {
    records: BTreeMap<String, RelevanceRecord>,
}

impl RelevanceIndex {
    pub fn insert(&mut self, record: RelevanceRecord) -> Option<RelevanceRecord> {
        self.records.insert(record.obligation.clone(), record)
    }

    pub fn get(&self, obligation: &str) -> Option<&RelevanceRecord> {
        self.records.get(obligation)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// The four outcomes 39.17 allows. `Sufficient` is one of them, not the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SufficiencyStatus {
    /// Every obligation is discharged or provably cannot move the decision.
    Sufficient,
    /// A gap exists that is known to matter, or is inaccessible.
    Insufficient,
    /// The checks ran and at least one gap has unknown influence. Nobody looked.
    Unknown,
    /// The certification itself could not be performed soundly.
    Failed,
}

impl SufficiencyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SufficiencyStatus::Sufficient => "sufficient",
            SufficiencyStatus::Insufficient => "insufficient",
            SufficiencyStatus::Unknown => "unknown",
            SufficiencyStatus::Failed => "failed",
        }
    }
}

/// An obligation that the selection did not discharge, with what is known about the gap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnmetObligation {
    pub obligation: String,
    pub effective: ObligationState,
    pub influence: InfluenceClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// Everything the certificate is computed from.
#[derive(Debug, Clone, Copy)]
pub struct SufficiencyInputs<'a> {
    pub decision: &'a str,
    pub role: &'a str,
    pub graph: &'a ObligationGraph,
    pub ledger: &'a OmissionLedger,
    pub closure: &'a ClosureReport,
    pub relevance: &'a RelevanceIndex,
    pub compiler_version: &'a str,
}

/// The receipt that says whether a small context is also an adequate one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SufficiencyCertificate {
    pub decision: String,
    pub role: String,
    pub status: SufficiencyStatus,
    pub unmet: Vec<UnmetObligation>,
    /// Human-readable reasons for anything other than `sufficient`.
    pub reasons: Vec<String>,
    pub manifest: OmissionManifest,
    /// Binds the certificate to the graph it was computed from, so a stale certificate is
    /// detectable rather than merely wrong.
    pub graph_sha256: String,
    pub compiler_version: String,
}

impl SufficiencyCertificate {
    /// Derives a certificate from the graph, the ledger, the closure check and the relevance
    /// arguments.
    ///
    /// Precedence between negative outcomes is deliberate: `Failed` beats `Insufficient` beats
    /// `Unknown`. A definite gap is more informative than an unexamined one, so a selection with
    /// both reports `Insufficient`; but an unexamined gap alone can never report `Sufficient`.
    pub fn decide(inputs: SufficiencyInputs<'_>) -> SufficiencyCertificate {
        let mut reasons: Vec<String> = Vec::new();
        let mut failed = false;
        let mut insufficient = false;
        let mut unknown = false;

        let graph_sha256 = match inputs.graph.content_hash() {
            Ok(hash) => hash.as_str().to_string(),
            Err(error) => {
                failed = true;
                reasons.push(format!("graph is not canonically hashable: {error}"));
                String::new()
            }
        };

        if !inputs.closure.is_satisfied() {
            failed = true;
            for violation in &inputs.closure.violations {
                reasons.push(format!(
                    "protected class `{}` (39.05 item {}) failed closure: {}",
                    violation.class().as_str(),
                    violation.class().blueprint_index(),
                    serde_json::to_string(violation).unwrap_or_else(|_| "unserialisable".into())
                ));
            }
        }

        if let Err(error) = inputs.ledger.validate() {
            failed = true;
            reasons.push(format!("omission ledger is not auditable: {error}"));
        }

        let mut unmet = Vec::new();
        match inputs.graph.undischarged() {
            Err(error) => {
                failed = true;
                reasons.push(format!("obligation graph is unverifiable: {error}"));
            }
            Ok(open) => {
                for (obligation, effective) in open {
                    let record = inputs.relevance.get(&obligation);
                    let influence = record.map_or(InfluenceClass::Unknown, |r| r.influence);
                    if !influence.supports_sufficiency() {
                        if influence == InfluenceClass::Unknown {
                            unknown = true;
                            reasons.push(format!(
                                "obligation `{obligation}` is `{}` and its influence on the \
                                 decision was never analysed",
                                effective.as_str()
                            ));
                        } else {
                            insufficient = true;
                            reasons.push(format!(
                                "obligation `{obligation}` is `{}` and its gap is classified `{}`",
                                effective.as_str(),
                                influence.as_str()
                            ));
                        }
                    }
                    unmet.push(UnmetObligation {
                        obligation,
                        effective,
                        influence,
                        justification: record.map(|r| r.argument.clone()),
                    });
                }
            }
        }

        let manifest = inputs.ledger.to_manifest();
        for group in manifest.blocking_groups() {
            if group.influence == InfluenceClass::Unknown {
                unknown = true;
                reasons.push(format!(
                    "{} omitted candidate(s) grouped as `{}` have unanalysed influence",
                    group.count, group.reason
                ));
            } else {
                insufficient = true;
                reasons.push(format!(
                    "{} omitted candidate(s) grouped as `{}` are classified `{}`",
                    group.count,
                    group.reason,
                    group.influence.as_str()
                ));
            }
        }

        let status = if failed {
            SufficiencyStatus::Failed
        } else if insufficient {
            SufficiencyStatus::Insufficient
        } else if unknown {
            SufficiencyStatus::Unknown
        } else {
            SufficiencyStatus::Sufficient
        };

        SufficiencyCertificate {
            decision: inputs.decision.to_string(),
            role: inputs.role.to_string(),
            status,
            unmet,
            reasons,
            manifest,
            graph_sha256,
            compiler_version: inputs.compiler_version.to_string(),
        }
    }

    pub fn is_sufficient(&self) -> bool {
        self.status == SufficiencyStatus::Sufficient
    }

    /// Recomputes the verdict from the same inputs and compares it to the stored one.
    ///
    /// A certificate that arrives over the wire is a claim. This is how a consumer refuses to take
    /// the claim on trust — including the case where the certificate was written by hand, or was
    /// computed against a graph that has since moved on.
    pub fn recheck(&self, inputs: SufficiencyInputs<'_>) -> CertificateCheck {
        let recomputed = SufficiencyCertificate::decide(inputs);
        if recomputed.graph_sha256 != self.graph_sha256 {
            return CertificateCheck::StaleGraph {
                certified: self.graph_sha256.clone(),
                current: recomputed.graph_sha256,
            };
        }
        if recomputed.decision != self.decision || recomputed.role != self.role {
            return CertificateCheck::ScopeMismatch {
                certified: format!("{}/{}", self.decision, self.role),
                current: format!("{}/{}", recomputed.decision, recomputed.role),
            };
        }
        if recomputed.status == self.status {
            CertificateCheck::Consistent
        } else {
            CertificateCheck::Contradicted {
                claimed: self.status,
                recomputed: recomputed.status,
            }
        }
    }
}

/// The outcome of rechecking a certificate against live inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum CertificateCheck {
    Consistent,
    /// The certificate claims a status the inputs do not support.
    Contradicted {
        claimed: SufficiencyStatus,
        recomputed: SufficiencyStatus,
    },
    /// The certificate was computed against a different obligation graph.
    StaleGraph {
        certified: String,
        current: String,
    },
    /// The certificate belongs to a different decision or role.
    ScopeMismatch {
        certified: String,
        current: String,
    },
}

impl CertificateCheck {
    pub fn is_consistent(&self) -> bool {
        matches!(self, CertificateCheck::Consistent)
    }
}
