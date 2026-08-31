//! Partial credit that names its rule (26.02, step 6).
//!
//! Biology rarely admits binary grading. A pipeline that identifies the right gene, the right
//! direction of effect and the wrong magnitude has done most of the work, and a benchmark that
//! scores it zero is measuring its own crudeness. But the fix — awarding 0.6 — is worse than the
//! problem unless the 0.6 is reconstructible, because an unreconstructible partial score is where
//! every 26.14 reward-hacking failure enters: the grader's discretion becomes the surface the
//! system optimises against.
//!
//! The rule enforced here is that **credit is a function application, not a number**. A [`Credit`]
//! exists only as the output of [`CreditRule::award`], carries the rule id and version that
//! produced it, and carries a content hash over the exact inputs. Two runs of the same rule on the
//! same evidence produce the same digest; a run whose digest does not match its stated inputs is
//! not credit, it is an assertion.
//!
//! # The forfeit
//!
//! 26.02 says partial credit is retained "only when the remaining conclusion is meaningful". A
//! critical error class leaves no meaningful remainder — a result on the wrong specimen is not
//! 70% about the right specimen — so [`CreditRule::award`] refuses rather than scaling down. The
//! difference matters: a small credit still contributes to an average, and averaging a specimen
//! swap into a leaderboard is how it stops being visible.
//!
//! # Not implemented
//!
//! Terms are independent and additive. Real rubrics have conditional structure — "magnitude credit
//! only if the direction term was earned" — which this module cannot express; such a rubric must
//! be encoded as a separate rule per branch, with the branch chosen before the evidence is seen so
//! that the choice is not itself a degree of freedom.

use std::collections::BTreeSet;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::CreditError;
use crate::validation::valid_text;
use crate::wrongness::BiologicalErrorClass;

const MAX_CREDIT_TERMS: usize = 1024;
const MAX_EVIDENCE_TERMS: usize = 4096;

/// One named thing a prediction can earn credit for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditTerm {
    pub term_id: String,
    /// Relative weight. Normalised across the rule's terms at award time, so weights are readable
    /// as "this is twice as important as that" rather than as a share of a hidden total.
    pub weight: f64,
    /// What a grader must observe for this term to be satisfied. Prose, and load-bearing: it is
    /// the difference between a rule and a lookup table.
    pub criterion: String,
}

/// What the grader observed, as inputs to a rule.
///
/// Deliberately just a set of satisfied term ids plus the classified errors. A rule that needs
/// more than this to fire is a rule whose inputs were not written down.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreditEvidence {
    pub satisfied: BTreeSet<String>,
    pub errors: Vec<BiologicalErrorClass>,
}

impl CreditEvidence {
    pub fn satisfying(terms: impl IntoIterator<Item = String>) -> Self {
        CreditEvidence {
            satisfied: terms.into_iter().collect(),
            errors: Vec::new(),
        }
    }

    pub fn with_error(mut self, class: BiologicalErrorClass) -> Self {
        self.errors.push(class);
        self
    }

    fn validate(&self, rule_id: &str) -> Result<(), CreditError> {
        if self.satisfied.len() > MAX_EVIDENCE_TERMS {
            return Err(CreditError::InvalidEvidence {
                rule_id: rule_id.to_string(),
                detail: format!("at most {MAX_EVIDENCE_TERMS} satisfied terms are supported"),
            });
        }
        if self.satisfied.iter().any(|term_id| !valid_text(term_id)) {
            return Err(CreditError::InvalidEvidence {
                rule_id: rule_id.to_string(),
                detail: "satisfied term ids must be bounded, trimmed, control-free strings".into(),
            });
        }
        let mut classes = BTreeSet::new();
        if self.errors.iter().any(|class| !classes.insert(*class)) {
            return Err(CreditError::InvalidEvidence {
                rule_id: rule_id.to_string(),
                detail: "error classes must be unique".into(),
            });
        }
        Ok(())
    }

    fn canonical_for_digest(&self) -> Self {
        let mut canonical = self.clone();
        canonical.errors.sort();
        canonical
    }
}

/// A named, versioned partial-credit rubric.
///
/// The version is not decoration. 26.20 forbids retroactive weight changes and 26.23 asks that
/// benchmark instruments be maintained as instruments; a rubric edited in place makes every
/// previously published score unverifiable, so an edit is a new version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditRule {
    rule_id: String,
    version: u32,
    terms: Vec<CreditTerm>,
}

impl CreditRule {
    /// Builds a rule, rejecting weights that cannot be normalised.
    pub fn new(
        rule_id: impl Into<String>,
        version: u32,
        terms: Vec<CreditTerm>,
    ) -> Result<Self, CreditError> {
        let rule_id = rule_id.into();
        let rule = CreditRule {
            rule_id,
            version,
            terms,
        };
        rule.validate()?;
        Ok(rule)
    }

    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn terms(&self) -> &[CreditTerm] {
        &self.terms
    }

    /// Applies the rule.
    ///
    /// Refuses outright when any observed error class is critical, and refuses when any error was
    /// left unclassified — an unexamined failure cannot be shown to leave a meaningful remainder,
    /// and 26.02's condition for retaining credit is exactly that it does.
    pub fn award(&self, evidence: &CreditEvidence) -> Result<Credit, CreditError> {
        self.validate()?;
        evidence.validate(&self.rule_id)?;
        if evidence
            .errors
            .contains(&BiologicalErrorClass::Unclassified)
        {
            return Err(CreditError::UnclassifiedError {
                rule_id: self.rule_id.clone(),
            });
        }
        if let Some(&class) = evidence
            .errors
            .iter()
            .find(|c| !c.severity().admits_partial_credit())
        {
            return Err(CreditError::CriticalErrorForfeitsCredit { class });
        }

        let total: f64 = self.terms.iter().map(|t| t.weight).sum();
        let mut basis = Vec::with_capacity(self.terms.len());
        let mut earned = 0.0;
        for term in &self.terms {
            let satisfied = evidence.satisfied.contains(&term.term_id);
            if satisfied {
                earned += term.weight;
            }
            basis.push(AwardedTerm {
                term_id: term.term_id.clone(),
                weight: term.weight,
                satisfied,
            });
        }

        let fraction = earned / total;
        if !(0.0..=1.0).contains(&fraction) {
            return Err(CreditError::FractionOutOfRange {
                rule_id: self.rule_id.clone(),
                fraction,
            });
        }

        let digest = digest_of(self, &evidence.canonical_for_digest())?;

        Ok(Credit {
            rule_id: self.rule_id.clone(),
            rule_version: self.version,
            fraction,
            basis,
            digest,
        })
    }

    fn validate(&self) -> Result<(), CreditError> {
        if !valid_text(&self.rule_id) {
            return Err(CreditError::InvalidRule {
                rule_id: self.rule_id.clone(),
                detail: "rule_id must be a bounded, trimmed, control-free string".into(),
            });
        }
        if self.version == 0 {
            return Err(CreditError::InvalidRule {
                rule_id: self.rule_id.clone(),
                detail: "version must be positive".into(),
            });
        }
        if self.terms.is_empty() {
            return Err(CreditError::NoBasis {
                rule_id: self.rule_id.clone(),
            });
        }
        if self.terms.len() > MAX_CREDIT_TERMS {
            return Err(CreditError::InvalidRule {
                rule_id: self.rule_id.clone(),
                detail: format!("at most {MAX_CREDIT_TERMS} terms are supported"),
            });
        }
        let mut term_ids = BTreeSet::new();
        for term in &self.terms {
            if !valid_text(&term.term_id) || !valid_text(&term.criterion) {
                return Err(CreditError::InvalidRule {
                    rule_id: self.rule_id.clone(),
                    detail: "term ids and criteria must be bounded, trimmed, control-free strings"
                        .into(),
                });
            }
            if !term_ids.insert(term.term_id.clone()) {
                return Err(CreditError::InvalidRule {
                    rule_id: self.rule_id.clone(),
                    detail: "term ids must be unique".into(),
                });
            }
            if !term.weight.is_finite() || term.weight <= 0.0 {
                return Err(CreditError::FractionOutOfRange {
                    rule_id: self.rule_id.clone(),
                    fraction: term.weight,
                });
            }
        }
        let total: f64 = self.terms.iter().map(|term| term.weight).sum();
        if !total.is_finite() || total <= 0.0 {
            return Err(CreditError::InvalidRule {
                rule_id: self.rule_id.clone(),
                detail: "term weights must have a finite positive total".into(),
            });
        }
        Ok(())
    }
}

/// One term's contribution, retained on the award.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AwardedTerm {
    pub term_id: String,
    pub weight: f64,
    pub satisfied: bool,
}

/// Credit that can be recomputed.
///
/// There is no public constructor. A `Credit` is obtained from [`CreditRule::award`] or by
/// deserialising one that was — and a deserialised award proves nothing until
/// [`Credit::verify`] re-derives its digest from the rule and evidence it claims. That is the
/// whole guarantee: the number travels with the means to reproduce it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credit {
    rule_id: String,
    rule_version: u32,
    fraction: f64,
    basis: Vec<AwardedTerm>,
    digest: ContentHash,
}

impl Credit {
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    pub fn rule_version(&self) -> u32 {
        self.rule_version
    }

    /// The share of available credit earned, in `[0, 1]`.
    ///
    /// Safe to expose as a bare float precisely because it cannot exist without the rule id,
    /// version, per-term basis and digest sitting beside it. Contrast [`crate::score::BioScore`],
    /// where a bare float would discard the reference's uncertainty and is therefore withheld.
    pub fn fraction(&self) -> f64 {
        self.fraction
    }

    pub fn basis(&self) -> &[AwardedTerm] {
        &self.basis
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Re-runs the rule on the evidence and compares the whole award.
    ///
    /// Not just the digest. The digest covers the *inputs*, which is what makes it a stable name
    /// for them, but a serialised award can have had its `fraction` edited without disturbing it.
    /// Recomputing catches that, and it is cheap: the rule is a pure function of its inputs, which
    /// is the property the whole module exists to guarantee.
    ///
    /// A `false` here means the credit was not produced by that rule on that evidence, whatever
    /// its `rule_id` says.
    pub fn verify(&self, rule: &CreditRule, evidence: &CreditEvidence) -> bool {
        if self.validate().is_err() {
            return false;
        }
        match rule.award(evidence) {
            Ok(recomputed) => recomputed == *self,
            Err(_) => false,
        }
    }

    fn validate(&self) -> Result<(), CreditError> {
        if !valid_text(&self.rule_id) {
            return Err(CreditError::InvalidAward {
                rule_id: self.rule_id.clone(),
                detail: "rule_id must be a bounded, trimmed, control-free string".into(),
            });
        }
        if self.rule_version == 0 {
            return Err(CreditError::InvalidAward {
                rule_id: self.rule_id.clone(),
                detail: "rule version must be positive".into(),
            });
        }
        if !self.fraction.is_finite() || !(0.0..=1.0).contains(&self.fraction) {
            return Err(CreditError::InvalidAward {
                rule_id: self.rule_id.clone(),
                detail: "fraction must be finite and between 0 and 1".into(),
            });
        }
        if self.basis.is_empty() || self.basis.len() > MAX_CREDIT_TERMS {
            return Err(CreditError::InvalidAward {
                rule_id: self.rule_id.clone(),
                detail: "award basis must contain between 1 and the supported term limit entries"
                    .into(),
            });
        }
        let mut term_ids = BTreeSet::new();
        for term in &self.basis {
            if !valid_text(&term.term_id)
                || !term.weight.is_finite()
                || term.weight <= 0.0
                || !term_ids.insert(term.term_id.clone())
            {
                return Err(CreditError::InvalidAward {
                    rule_id: self.rule_id.clone(),
                    detail: "award basis must contain unique named positive finite terms".into(),
                });
            }
        }
        Ok(())
    }
}

/// Content hash over the canonical bytes of the rule *and* the evidence together.
///
/// Both halves, because a digest over the evidence alone would let a rubric be swapped under a
/// published score, and a digest over the rule alone would let the inputs be.
fn digest_of(rule: &CreditRule, evidence: &CreditEvidence) -> Result<ContentHash, CreditError> {
    let fail = |detail: String| CreditError::NotDigestible {
        rule_id: rule.rule_id.clone(),
        detail,
    };
    let mut value = Map::new();
    value.insert(
        "rule".to_string(),
        serde_json::to_value(rule).map_err(|e| fail(e.to_string()))?,
    );
    value.insert(
        "evidence".to_string(),
        serde_json::to_value(evidence).map_err(|e| fail(e.to_string()))?,
    );
    ContentHash::of_value(&Value::Object(value)).map_err(|e| fail(e.to_string()))
}
