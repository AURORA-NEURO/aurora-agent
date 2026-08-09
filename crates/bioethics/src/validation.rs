//! Blueprint 36.21 — quality management, validation and release gates.
//!
//! 36.21's purpose sentence is a promise: "define evidence required before modules progress from
//! experimental to verified". The module then does not define it. Its Scope lists seven evidence
//! kinds and its "Required controls" block lists six *measurements* — gate pass rate, open risk
//! debt, validation coverage, post-release defect, reproducibility, audit findings — none with a
//! denominator, and none of them a control. 36.21 is the only module in §36 whose required-control
//! list is a metrics list, which is worth knowing before reading anything else here.
//!
//! What is left is one checkable predicate, and this module is that predicate and nothing else:
//! **a module may be recorded verified only when every evidence kind 36.21 names is present.**
//!
//! # The ladder has two rungs because the module names two
//!
//! [`Maturity`] is `Experimental` and `Verified`. There is no `Candidate`, no `Beta`, no
//! `Provisional`. 36.21 names exactly two states and inventing intermediate rungs would produce a
//! ladder whose middle nobody specified, which is worse than a short ladder because a middle rung
//! is where a reviewer parks something rather than deciding about it.
//!
//! # Absent evidence is absent, and a blank reference is absent too
//!
//! [`VerifiedModule`] has private fields, one constructor and no `Deserialize`.
//! [`ValidationDossier::verify`] refuses while [`ValidationDossier::missing`] is non-empty, and
//! `missing` counts a record whose reference is blank, because a checklist row with a tick and no
//! pointer is the characteristic way a validation file passes without anyone having looked. This
//! is registered as [`crate::safeguard::Impossibility::NoVerifiedModuleExistsWithUnmetEvidence`].
//!
//! # Independence, and the only criterion available
//!
//! 36.21 requires "independent reproduction" and states no independence criterion. The only one
//! this crate can check is structural non-identity — the reproducer's name differs from the
//! author's. That is deliberately weak, it is the same weak criterion `bioprism-stewardship`
//! applies to reviewer separation, and stating it as weak is the point: an implementation that
//! called it "independence verified" would be asserting something no string comparison can know.
//!
//! # What this module is not
//!
//! * Not a metric gate. `bioprism-metrics::gate` owns release gates over measurements, including
//!   the rule that an aggregate over a grid with an unmeasured cell is not an aggregate.
//! * Not a claim ladder. `bioprism-atlas` owns which tier the evidence licenses and
//!   `bioprism-stewardship::claim` owns which sentence that tier permits. Nothing here reads or
//!   writes either.
//! * Not a schema-compatibility lifecycle. `bioprism-governance` owns versioning, compatibility
//!   classification and deprecation, including the rule that a change moving an artifact's digest
//!   cannot be classified compatible.
//! * Not a trust tier. `bioprism-registry` computes those from pack bytes.
//!
//! The subject here is an *implementation module* and its validation file, which is a different
//! object from a pack, a claim, a score and a schema.
//!
//! # Not implemented
//!
//! No design-review workflow, no change-control board, no risk register, no defect tracker, no
//! audit schedule, no reviewer assignment, no clock and no identity system. Where 36.21 describes
//! people meeting, this module holds the row they would fill in and reports which rows are empty —
//! the shape `bioprism-safety` uses for 13.26's governance record.

use crate::error::BioethicsError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The two states 36.21 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Maturity {
    Experimental,
    Verified,
}

impl Maturity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Maturity::Experimental => "experimental",
            Maturity::Verified => "verified",
        }
    }
}

impl fmt::Display for Maturity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The seven evidence kinds of 36.21's Scope, transcribed.
///
/// Closed. A dossier cannot name a kind 36.21 never asked for, and cannot omit one by not
/// mentioning it — [`ValidationDossier::missing`] iterates this list rather than the dossier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    RequirementsAndRiskFile,
    DesignReview,
    UnitAndConformanceTests,
    ScientificValidation,
    SecurityAndPrivacyReview,
    IndependentReproduction,
    ChangeControl,
}

impl EvidenceKind {
    pub const ALL: [EvidenceKind; 7] = [
        EvidenceKind::RequirementsAndRiskFile,
        EvidenceKind::DesignReview,
        EvidenceKind::UnitAndConformanceTests,
        EvidenceKind::ScientificValidation,
        EvidenceKind::SecurityAndPrivacyReview,
        EvidenceKind::IndependentReproduction,
        EvidenceKind::ChangeControl,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::RequirementsAndRiskFile => "requirements_and_risk_file",
            EvidenceKind::DesignReview => "design_review",
            EvidenceKind::UnitAndConformanceTests => "unit_and_conformance_tests",
            EvidenceKind::ScientificValidation => "scientific_validation",
            EvidenceKind::SecurityAndPrivacyReview => "security_and_privacy_review",
            EvidenceKind::IndependentReproduction => "independent_reproduction",
            EvidenceKind::ChangeControl => "change_control",
        }
    }

    /// The blueprint's own words. Not elaborated.
    pub const fn describe(self) -> &'static str {
        match self {
            EvidenceKind::RequirementsAndRiskFile => "requirements and risk file",
            EvidenceKind::DesignReview => "design review",
            EvidenceKind::UnitAndConformanceTests => "unit and conformance tests",
            EvidenceKind::ScientificValidation => "scientific validation",
            EvidenceKind::SecurityAndPrivacyReview => "security and privacy review",
            EvidenceKind::IndependentReproduction => "independent reproduction",
            EvidenceKind::ChangeControl => "change control",
        }
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One filed piece of evidence.
///
/// `reference` is where a reader goes to check it. It is never fetched, parsed or validated; the
/// only thing checked is that it is not blank, because a blank pointer is how a checklist passes
/// without anyone having looked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub kind: EvidenceKind,
    pub reference: String,
    /// Who filed it. Compared with the module's author for the independent-reproduction row and
    /// otherwise recorded only.
    pub attested_by: String,
}

impl EvidenceRecord {
    pub fn new(
        kind: EvidenceKind,
        reference: impl Into<String>,
        attested_by: impl Into<String>,
    ) -> Self {
        EvidenceRecord {
            kind,
            reference: reference.into(),
            attested_by: attested_by.into(),
        }
    }

    fn is_blank(&self) -> bool {
        self.reference.trim().is_empty()
    }
}

/// A module's validation file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDossier {
    pub subject: String,
    pub author: String,
    evidence: BTreeMap<EvidenceKind, EvidenceRecord>,
}

impl ValidationDossier {
    pub fn new(subject: impl Into<String>, author: impl Into<String>) -> Self {
        ValidationDossier {
            subject: subject.into(),
            author: author.into(),
            evidence: BTreeMap::new(),
        }
    }

    pub fn with(mut self, record: EvidenceRecord) -> Self {
        self.evidence.insert(record.kind, record);
        self
    }

    pub fn get(&self, kind: EvidenceKind) -> Option<&EvidenceRecord> {
        self.evidence.get(&kind)
    }

    /// The evidence kinds 36.21 requires and this dossier does not have, in blueprint order.
    ///
    /// A record with a blank reference counts as missing.
    pub fn missing(&self) -> Vec<EvidenceKind> {
        EvidenceKind::ALL
            .into_iter()
            .filter(|kind| match self.evidence.get(kind) {
                None => true,
                Some(record) => record.is_blank(),
            })
            .collect()
    }

    /// `Verified` only if a [`VerifiedModule`] could be minted. Reading this is not the same as
    /// holding one.
    pub fn maturity(&self) -> Maturity {
        if self.missing().is_empty() {
            Maturity::Verified
        } else {
            Maturity::Experimental
        }
    }

    /// The only constructor for a [`VerifiedModule`].
    ///
    /// Checks the evidence first and independence second, so a dossier that is both incomplete and
    /// self-reproduced reports the incompleteness — the problem that has to be fixed either way.
    pub fn verify(&self) -> Result<VerifiedModule, BioethicsError> {
        let missing = self.missing();
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|kind| kind.as_str()).collect();
            return Err(BioethicsError::UnmetValidationEvidence {
                subject: self.subject.clone(),
                missing: names.join(", "),
            });
        }

        let reproduction = self
            .evidence
            .get(&EvidenceKind::IndependentReproduction)
            .expect("missing() is empty, so every kind is present");
        if reproduction.attested_by == self.author {
            return Err(BioethicsError::ReproducerIsAuthor {
                subject: self.subject.clone(),
                actor: self.author.clone(),
            });
        }

        Ok(VerifiedModule {
            subject: self.subject.clone(),
            author: self.author.clone(),
            reproduced_by: reproduction.attested_by.clone(),
            evidence: self.evidence.clone(),
        })
    }
}

/// A module that carried every evidence kind 36.21 names.
///
/// # Why there is no `Deserialize`
///
/// A decoded value would assert that the check ran, which is the only thing the type means. The
/// dossier is the transportable object; verification is a decision that has to be taken by
/// whoever is relying on it, in their own process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedModule {
    subject: String,
    author: String,
    reproduced_by: String,
    evidence: BTreeMap<EvidenceKind, EvidenceRecord>,
}

impl VerifiedModule {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn reproduced_by(&self) -> &str {
        &self.reproduced_by
    }

    pub fn evidence(&self, kind: EvidenceKind) -> &EvidenceRecord {
        self.evidence
            .get(&kind)
            .expect("a verified module carries every evidence kind")
    }

    /// Always [`Maturity::Verified`]. There is no field to hold anything else.
    pub const fn maturity(&self) -> Maturity {
        Maturity::Verified
    }
}
