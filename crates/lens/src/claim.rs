//! Claim, evidence and contradiction — blueprint 42.11.
//!
//! 42.11 asks to "show claims, scope, supporting evidence, counterevidence, methods, populations,
//! uncertainty, and open obligations". The word doing the work is **scope**. A claim in BioPRISM
//! is a local section — "a value valid inside one scope, not a globally true statement"
//! (`AGENTS.md`) — so a piece of evidence supports a claim only if the evidence's scope sits
//! inside the claim's. Evidence gathered in a population the claim does not cover is not weak
//! support; it is support for a different claim.
//!
//! This lens therefore checks refinement using the partial order of 43.03 rather than counting
//! citations. [`ScopeKey::refines`](bioprism_scope::ScopeKey::refines) is a *partial* order, and
//! that matters twice over: two scopes constraining disjoint dimensions are incomparable, and
//! pooling across them invents a population nobody sampled. Asked to do that, the lens refuses
//! with [`RefusalReason::WouldAggregateIncomparableScopes`] rather than returning a pooled
//! number.
//!
//! # Contradiction is kept, not resolved
//!
//! When supporting and contradicting evidence both exist, this lens emits a
//! [`ClaimFinding::Contradiction`] naming both items. It does not weigh them, adjudicate them,
//! or compute a net direction. 03.10's label distributions keep reviewer disagreement rather
//! than resolving it, and the same reasoning applies here: a lens that silently picks a winner
//! has deleted the finding a reader most needed.
//!
//! # Not implemented
//!
//! No uncertainty quantification. 42.11 lists "uncertainty" among the things to show, and this
//! lens carries whatever uncertainty an evidence item declares as text, but it does not compute,
//! combine or propagate intervals — that needs an estimator and a dependency structure neither
//! 42.11 nor 33.01 specifies. No method taxonomy either; "methods" in the blueprint's list has no
//! enumerated vocabulary anywhere in section 42, so a made-up one would be a guess with a type.

use crate::grammar::{
    Coverage, EvidenceRequirement, Lens, LensDeclaration, LensId, LensOutcome, PendingRegion,
    Refusal, RefusalReason,
};
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};

/// One piece of evidence, with the scope it was gathered in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    pub source: String,
    pub scope: ScopeKey,
    /// Whatever the source says about its own uncertainty, carried verbatim and uninterpreted.
    #[serde(default)]
    pub stated_uncertainty: Option<String>,
}

impl EvidenceItem {
    pub fn new(id: impl Into<String>, source: impl Into<String>, scope: ScopeKey) -> Self {
        EvidenceItem {
            id: id.into(),
            source: source.into(),
            scope,
            stated_uncertainty: None,
        }
    }
}

/// A claim, its scope, what supports it, what contradicts it, and what remains open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    pub claim: String,
    pub scope: ScopeKey,
    pub supporting: Vec<EvidenceItem>,
    pub contradicting: Vec<EvidenceItem>,
    /// Obligations the compiler could not discharge for this claim, as 43.25 records them.
    #[serde(default)]
    pub open_obligations: Vec<String>,
}

impl ClaimRecord {
    pub fn new(claim: impl Into<String>, scope: ScopeKey) -> Self {
        ClaimRecord {
            claim: claim.into(),
            scope,
            supporting: Vec::new(),
            contradicting: Vec::new(),
            open_obligations: Vec::new(),
        }
    }

    pub fn supported_by(mut self, item: EvidenceItem) -> Self {
        self.supporting.push(item);
        self
    }

    pub fn contradicted_by(mut self, item: EvidenceItem) -> Self {
        self.contradicting.push(item);
        self
    }

    pub fn with_obligation(mut self, obligation: impl Into<String>) -> Self {
        self.open_obligations.push(obligation.into());
        self
    }
}

/// The claim set under inspection.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ClaimDossier {
    pub claims: Vec<ClaimRecord>,
    /// Whether the caller wants one answer pooled across every claim's scope. Set this and the
    /// lens will refuse unless the scopes are comparable.
    #[serde(default)]
    pub pool_across_scopes: bool,
    /// Claims the caller knows about but has not loaded yet. Named, so a partial answer says what
    /// it has not seen.
    #[serde(default)]
    pub unloaded_claims: Vec<String>,
}

impl ClaimDossier {
    pub fn new(claims: Vec<ClaimRecord>) -> Self {
        ClaimDossier {
            claims,
            pool_across_scopes: false,
            unloaded_claims: Vec::new(),
        }
    }
}

/// What the claim–evidence lens found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimFinding {
    /// No supporting evidence at all. Distinct from contradicted: nobody has looked.
    Unsupported { claim: String },
    /// Evidence exists but its scope does not sit inside the claim's, so it does not establish
    /// the claim as stated.
    ScopeOverreach {
        claim: String,
        evidence: String,
        claim_scope: String,
        evidence_scope: String,
    },
    /// Both supporting and contradicting evidence exist, in comparable scopes. Kept, not resolved.
    Contradiction {
        claim: String,
        supporting: String,
        contradicting: String,
    },
    /// An obligation the compiler could not discharge, still attached to the claim.
    OpenObligation { claim: String, obligation: String },
}

impl Witness for ClaimFinding {
    fn kind(&self) -> &'static str {
        match self {
            ClaimFinding::Unsupported { .. } => "unsupported_claim",
            ClaimFinding::ScopeOverreach { .. } => "scope_overreach",
            ClaimFinding::Contradiction { .. } => "contradiction",
            ClaimFinding::OpenObligation { .. } => "open_obligation",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            ClaimFinding::Unsupported { .. } => &["claim", "supporting_items"],
            ClaimFinding::ScopeOverreach { .. } => {
                &["claim", "evidence", "claim_scope", "evidence_scope"]
            }
            ClaimFinding::Contradiction { .. } => &["claim", "supporting", "contradicting"],
            ClaimFinding::OpenObligation { .. } => &["claim", "obligation"],
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            ClaimFinding::Unsupported { claim } => vec![Cell::text(claim.clone()), Cell::count(0)],
            ClaimFinding::ScopeOverreach {
                claim,
                evidence,
                claim_scope,
                evidence_scope,
            } => vec![
                Cell::text(claim.clone()),
                Cell::id(evidence.clone()),
                Cell::text(claim_scope.clone()),
                Cell::text(evidence_scope.clone()),
            ],
            ClaimFinding::Contradiction {
                claim,
                supporting,
                contradicting,
            } => vec![
                Cell::text(claim.clone()),
                Cell::id(supporting.clone()),
                Cell::id(contradicting.clone()),
            ],
            ClaimFinding::OpenObligation { claim, obligation } => {
                vec![Cell::text(claim.clone()), Cell::text(obligation.clone())]
            }
        }
    }

    fn sentence(&self) -> String {
        match self {
            ClaimFinding::Unsupported { claim } => {
                format!("`{claim}` has no supporting evidence in this dossier")
            }
            ClaimFinding::ScopeOverreach {
                claim,
                evidence,
                claim_scope,
                evidence_scope,
            } => format!(
                "evidence {evidence} was gathered in scope {evidence_scope}, which does not \
                 refine the scope {claim_scope} of `{claim}`"
            ),
            ClaimFinding::Contradiction {
                claim,
                supporting,
                contradicting,
            } => format!(
                "`{claim}` is supported by {supporting} and contradicted by {contradicting}; \
                 both are retained"
            ),
            ClaimFinding::OpenObligation { claim, obligation } => {
                format!("`{claim}` carries the undischarged obligation `{obligation}`")
            }
        }
    }
}

fn describe(scope: &ScopeKey) -> String {
    if scope.is_empty() {
        return "unscoped".to_string();
    }
    let parts: Vec<String> = scope
        .iter()
        .map(|(dimension, value)| format!("{dimension}={}", value.describe()))
        .collect();
    parts.join(",")
}

/// Blueprint 42.11.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaimEvidenceLens;

impl ClaimEvidenceLens {
    pub const ID: &'static str = "claim_evidence";

    fn incomparable_pair(dossier: &ClaimDossier) -> Option<(String, String)> {
        for (i, left) in dossier.claims.iter().enumerate() {
            for right in dossier.claims.iter().skip(i + 1) {
                if !left.scope.refines(&right.scope) && !right.scope.refines(&left.scope) {
                    return Some((describe(&left.scope), describe(&right.scope)));
                }
            }
        }
        None
    }
}

impl Lens for ClaimEvidenceLens {
    type Evidence = ClaimDossier;
    type Witness = ClaimFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.11",
            "for each claim, does evidence in the claim's own scope support it, does anything \
             contradict it, and what remains undischarged?",
            vec![
                EvidenceRequirement::new("dossier.claims", "the claims and their scopes"),
                EvidenceRequirement::new(
                    "dossier.evidence",
                    "supporting and contradicting items, each with the scope it was gathered in",
                ),
                EvidenceRequirement::new(
                    "dossier.obligations",
                    "obligations the compiler left undischarged",
                ),
            ],
            Vec::new(),
            vec![
                RefusalReason::WouldAggregateIncomparableScopes,
                RefusalReason::NoAnswerableFormulation,
            ],
        )
        .expect("42.11 declaration is well formed")
    }

    fn answer(&self, _scope: &ScopeKey, dossier: &ClaimDossier) -> LensOutcome<ClaimFinding> {
        if dossier.claims.is_empty() {
            return LensOutcome::Refused(Refusal::new(
                RefusalReason::NoAnswerableFormulation,
                "the dossier contains no claim to evaluate",
            ));
        }

        if dossier.pool_across_scopes {
            if let Some((left, right)) = Self::incomparable_pair(dossier) {
                return LensOutcome::Refused(Refusal::new(
                    RefusalReason::WouldAggregateIncomparableScopes,
                    format!(
                        "scopes {left} and {right} do not refine one another; pooling them would \
                         describe a population that was never sampled"
                    ),
                ));
            }
        }

        let mut findings = Vec::new();
        for record in &dossier.claims {
            if record.supporting.is_empty() {
                findings.push(ClaimFinding::Unsupported {
                    claim: record.claim.clone(),
                });
            }
            for item in record.supporting.iter().chain(&record.contradicting) {
                if !item.scope.refines(&record.scope) {
                    findings.push(ClaimFinding::ScopeOverreach {
                        claim: record.claim.clone(),
                        evidence: item.id.clone(),
                        claim_scope: describe(&record.scope),
                        evidence_scope: describe(&item.scope),
                    });
                }
            }
            for support in &record.supporting {
                for against in &record.contradicting {
                    findings.push(ClaimFinding::Contradiction {
                        claim: record.claim.clone(),
                        supporting: support.id.clone(),
                        contradicting: against.id.clone(),
                    });
                }
            }
            for obligation in &record.open_obligations {
                findings.push(ClaimFinding::OpenObligation {
                    claim: record.claim.clone(),
                    obligation: obligation.clone(),
                });
            }
        }

        let examined = dossier.claims.len();
        let eligible = examined + dossier.unloaded_claims.len();
        let coverage = if dossier.unloaded_claims.is_empty() {
            Coverage::complete(Self::ID, examined, eligible)
        } else {
            Coverage::partial(
                Self::ID,
                examined,
                eligible,
                dossier
                    .unloaded_claims
                    .iter()
                    .map(|claim| PendingRegion::new(claim.clone(), "claim not loaded"))
                    .collect(),
            )
        };
        match coverage {
            Ok(coverage) => LensOutcome::Answered {
                witnesses: findings,
                coverage,
            },
            Err(_) => LensOutcome::Refused(Refusal::new(
                RefusalReason::NoAnswerableFormulation,
                "the dossier's loaded and unloaded claim counts are inconsistent",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{run, Completeness, ReportOutcome};

    fn population(value: &str) -> ScopeKey {
        ScopeKey::new().exact("population", value)
    }

    #[test]
    fn evidence_gathered_outside_the_claim_scope_is_flagged_as_overreach_not_counted_as_support() {
        let claim = ClaimRecord::new("PDL1 predicts response", population("nsclc"))
            .supported_by(EvidenceItem::new("E1", "trial-A", population("melanoma")));
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::new(vec![claim]),
        )
        .unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "scope_overreach")
            .expect("overreach detected");
        assert!(row.sentence.contains("melanoma"));
        assert!(row.sentence.contains("nsclc"));
    }

    #[test]
    fn evidence_inside_the_claim_scope_raises_no_overreach() {
        let narrow = ScopeKey::new()
            .exact("population", "nsclc")
            .exact("stage", "IV");
        let claim = ClaimRecord::new("PDL1 predicts response", population("nsclc"))
            .supported_by(EvidenceItem::new("E1", "trial-A", narrow));
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::new(vec![claim]),
        )
        .unwrap();
        assert!(report.witnesses().is_empty());
    }

    #[test]
    fn a_claim_with_no_supporting_evidence_is_unsupported_not_contradicted() {
        let claim = ClaimRecord::new("X causes Y", population("nsclc"));
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::new(vec![claim]),
        )
        .unwrap();
        assert_eq!(report.witnesses().len(), 1);
        assert_eq!(report.witnesses()[0].kind, "unsupported_claim");
    }

    #[test]
    fn a_contradiction_names_both_items_and_picks_no_winner() {
        let claim = ClaimRecord::new("X causes Y", population("nsclc"))
            .supported_by(EvidenceItem::new("E1", "trial-A", population("nsclc")))
            .contradicted_by(EvidenceItem::new("E2", "trial-B", population("nsclc")));
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::new(vec![claim]),
        )
        .unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "contradiction")
            .expect("contradiction retained");
        assert!(row.sentence.contains("E1"));
        assert!(row.sentence.contains("E2"));
        assert!(row.sentence.contains("both are retained"));
    }

    #[test]
    fn pooling_incomparable_scopes_is_refused_rather_than_answered() {
        let dossier = ClaimDossier {
            claims: vec![
                ClaimRecord::new("A", ScopeKey::new().exact("population", "nsclc")),
                ClaimRecord::new("B", ScopeKey::new().exact("assay", "IHC")),
            ],
            pool_across_scopes: true,
            unloaded_claims: Vec::new(),
        };
        let report = run(&ClaimEvidenceLens, &ScopeKey::new(), &dossier).unwrap();
        match report.outcome() {
            ReportOutcome::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    RefusalReason::WouldAggregateIncomparableScopes
                );
                assert!(refusal.detail.contains("never sampled"));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn pooling_comparable_scopes_is_answered() {
        let dossier = ClaimDossier {
            claims: vec![
                ClaimRecord::new("A", ScopeKey::new().exact("population", "nsclc"))
                    .supported_by(EvidenceItem::new("E1", "t", population("nsclc"))),
                ClaimRecord::new(
                    "B",
                    ScopeKey::new()
                        .exact("population", "nsclc")
                        .exact("stage", "IV"),
                )
                .supported_by(EvidenceItem::new(
                    "E2",
                    "t",
                    ScopeKey::new()
                        .exact("population", "nsclc")
                        .exact("stage", "IV"),
                )),
            ],
            pool_across_scopes: true,
            unloaded_claims: Vec::new(),
        };
        let report = run(&ClaimEvidenceLens, &ScopeKey::new(), &dossier).unwrap();
        assert!(report.is_answered());
    }

    #[test]
    fn an_unloaded_claim_makes_the_answer_partial_and_names_the_claim() {
        let dossier = ClaimDossier {
            claims: vec![ClaimRecord::new("A", population("nsclc"))
                .supported_by(EvidenceItem::new("E1", "t", population("nsclc")))],
            pool_across_scopes: false,
            unloaded_claims: vec!["B".into(), "C".into()],
        };
        let report = run(&ClaimEvidenceLens, &ScopeKey::new(), &dossier).unwrap();
        assert_eq!(
            report.completeness(),
            Completeness::Partial {
                examined: 1,
                eligible: 3
            }
        );
        assert!(report.spoken().iter().any(|l| l.contains("pending B")));
    }

    #[test]
    fn an_open_obligation_survives_into_the_answer() {
        let claim = ClaimRecord::new("A", population("nsclc"))
            .supported_by(EvidenceItem::new("E1", "t", population("nsclc")))
            .with_obligation("germline_status_unresolved");
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::new(vec![claim]),
        )
        .unwrap();
        assert!(report
            .witnesses()
            .iter()
            .any(|r| r.kind == "open_obligation"
                && r.sentence.contains("germline_status_unresolved")));
    }

    #[test]
    fn an_empty_dossier_is_refused_with_a_declared_reason_not_answered_as_clean() {
        let report = run(
            &ClaimEvidenceLens,
            &ScopeKey::new(),
            &ClaimDossier::default(),
        )
        .unwrap();
        assert_eq!(report.outcome().as_str(), "refused");
    }
}
