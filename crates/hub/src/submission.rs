//! The submission contract: who may submit, and what a submission must carry.
//!
//! Blueprint 34.16 (Challenges, Submissions and Hidden Holdouts) names the submission artifact as
//! a primary capability but does not enumerate its fields. 34.15 supplies the missing half by
//! listing what the *published* object must resolve to — provenance, access policy, health, and a
//! limitations card — and 43.43 supplies the discipline: every empirical claim is linked to a
//! comparison, and nonclaims are explicit rather than implied by omission.
//!
//! # The contract
//!
//! Five things, and a submission missing any one of them is refused with the field named:
//!
//! | Field | Why it is mandatory |
//! |---|---|
//! | `content` | A score with no content digest cannot be reproduced or corrected. |
//! | `scope` | 43.43 forbids universal claims; a claim needs a boundary to be narrower than. |
//! | `licence` | 36.14 makes redistribution terms an inherited property, not an afterthought. |
//! | `provenance` | 34.15: "every rendered score resolves to immutable result objects". |
//! | `does_not_establish` | 36.12: research evidence must be distinguished from clinical direction. |
//!
//! The last one is the unusual one, and it is the one most worth having. A submitter who cannot
//! write down what their result does not show has not finished thinking about what it does show.
//! The hub requires at minimum a clinical-validity nonclaim, because that is the inference a
//! public biological leaderboard most invites a reader to make unaided.
//!
//! # Earned, not asserted
//!
//! [`VerificationStatus`] is the field a dishonest submission would most like to set. It cannot:
//! [`accept`] refuses any draft claiming better than [`VerificationStatus::SelfReported`], and the
//! only route upward is [`crate::moderation::ModerationLedger::attest`], which requires an actor
//! who is not the submitter. Same reasoning as `bioprism_registry`'s trust tiers — a field that
//! says what tier something is would be an assertion.
//!
//! # What is not implemented
//!
//! The hub never sees the submitted bytes. It cannot check that `content` is the digest of
//! anything, that the build is reproducible, or that the submitter exists. It guarantees only
//! that a digest was declared and that every later record cites the same one, which is what makes
//! a contradiction findable later.

use crate::attribution::{Ancestor, Attribution, Licence, LicenceStack};
use crate::error::HubError;
use crate::id::{Epoch, SubmissionId, SubmitterId};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// How much independent checking a result has survived.
///
/// 34.15: "public pages distinguish self-reported, reproduced, verified, and prospectively
/// validated results". Ordered, and the order is load-bearing: a board with a verification floor
/// compares against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The submitter ran it and says so. The default, and the only status a submitter may claim.
    SelfReported,
    /// Someone else re-ran the submitted artifact and got the same numbers.
    Reproduced,
    /// A reviewer executed it against the hub's own copy of the pack and protocol.
    Verified,
    /// Checked against outcomes that were not available when the result was produced.
    ProspectivelyValidated,
}

impl VerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationStatus::SelfReported => "self-reported",
            VerificationStatus::Reproduced => "reproduced",
            VerificationStatus::Verified => "verified",
            VerificationStatus::ProspectivelyValidated => "prospectively-validated",
        }
    }
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether the hub will take anything from this account at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "standing")]
pub enum Standing {
    /// Not identity-checked. May submit; results stay self-reported until reviewed.
    Unverified,
    /// Identity and affiliation checked by the operator out of band.
    Verified,
    /// Additionally trusted to review other submissions.
    Steward,
    /// May not submit. The reason is recorded so a suspension can be appealed.
    Suspended { reason: String },
}

/// A submitting party, as far as the hub is concerned.
///
/// Not a principal: there is no authentication here and none is planned in this crate. The
/// operator's identity provider decides that a request really came from this `id`; the hub decides
/// what an account in this standing is allowed to do with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submitter {
    pub id: SubmitterId,
    pub standing: Standing,
    /// 36.16: conflicts and sponsorship must be declared before a result is publishable, and an
    /// empty list is a different statement from an undeclared one — hence the flag.
    pub conflicts_declared: bool,
    pub conflicts: Vec<String>,
}

impl Submitter {
    /// An unverified account with no declared conflicts of interest. Cannot submit until
    /// `conflicts_declared` is set, even if the list stays empty.
    pub fn unverified(id: SubmitterId) -> Submitter {
        Submitter {
            id,
            standing: Standing::Unverified,
            conflicts_declared: false,
            conflicts: Vec::new(),
        }
    }

    /// Explicitly declare that there are no conflicts. The distinction from the default matters:
    /// silence is not a declaration.
    pub fn declaring_no_conflicts(mut self) -> Submitter {
        self.conflicts_declared = true;
        self.conflicts.clear();
        self
    }

    pub fn declaring(mut self, conflicts: Vec<String>) -> Submitter {
        self.conflicts_declared = true;
        self.conflicts = conflicts;
        self
    }

    /// Whether this account may present a submission at all.
    pub fn check_eligibility(&self) -> Result<(), HubError> {
        if let Standing::Suspended { reason } = &self.standing {
            return Err(HubError::SubmitterSuspended {
                submitter: self.id.to_string(),
                reason: reason.clone(),
            });
        }
        if !self.conflicts_declared {
            return Err(HubError::ConflictsUndeclared {
                submitter: self.id.to_string(),
            });
        }
        Ok(())
    }
}

/// The boundary a claim is allowed to be made inside.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredScope {
    pub disease: Vec<String>,
    pub modality: Vec<String>,
    /// At least one is required: a result about no decision family is a result about nothing.
    pub decision_family: Vec<String>,
    pub intended_use: String,
    /// Populations, modalities or settings the submitter believes this does not transfer to.
    pub out_of_scope: Vec<String>,
}

/// The kind of inference a submission explicitly disowns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonClaimKind {
    /// "This does not establish clinical validity or fitness for patient care." Mandatory.
    ClinicalValidity,
    /// "This does not establish that the result transfers outside the declared scope."
    Generalization,
    /// "This does not establish superiority over systems not evaluated here." (43.43)
    UniversalSuperiority,
    /// "This does not establish a causal mechanism."
    Causality,
    /// "Absence from our search does not establish worldwide novelty." (43.43)
    Novelty,
    Other,
}

impl NonClaimKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NonClaimKind::ClinicalValidity => "clinical-validity",
            NonClaimKind::Generalization => "generalization",
            NonClaimKind::UniversalSuperiority => "universal-superiority",
            NonClaimKind::Causality => "causality",
            NonClaimKind::Novelty => "novelty",
            NonClaimKind::Other => "other",
        }
    }
}

/// One sentence stating what the submission does not establish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonClaim {
    pub kind: NonClaimKind,
    pub statement: String,
}

impl NonClaim {
    pub fn new(kind: NonClaimKind, statement: impl Into<String>) -> NonClaim {
        NonClaim {
            kind,
            statement: statement.into(),
        }
    }

    /// The nonclaim the hub will not accept a biological submission without.
    pub fn clinical_validity() -> NonClaim {
        NonClaim::new(
            NonClaimKind::ClinicalValidity,
            "This result is research evidence. It establishes no clinical validity and supports no \
             patient-specific decision.",
        )
    }
}

/// How the artifact was built, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildProvenance {
    pub toolchain: String,
    pub source_digest: ContentHash,
    /// The submitter's claim that a rebuild reproduces `source_digest`. Unchecked here; it becomes
    /// checkable only when a reviewer attests [`VerificationStatus::Reproduced`].
    pub reproducible: bool,
}

/// Where the submission came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Artifacts this one derives from. Drives licence and attribution propagation.
    pub ancestors: Vec<Ancestor>,
    pub build: BuildProvenance,
    /// Free-form references to signatures or attestations held elsewhere. The hub does not verify
    /// them; it records that they were cited, so a missing one is visible.
    pub attestations: Vec<String>,
}

/// The honest denominator of a benchmark result.
///
/// The executive summary's rule — "PRISM must never imply that one million automatically
/// paraphrased questions equal one million meaningful benchmarks" — has a direct consequence for a
/// public hub: an entry may report how many instances it ran, but the number a reader compares
/// with is the count of independent equivalence classes. Reporting instances with no class count
/// is refused rather than silently rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceScale {
    pub instances: usize,
    /// Distinct (parent, mutation family, oracle signature) classes, as counted upstream.
    pub equivalence_classes: usize,
}

impl EvidenceScale {
    pub fn new(instances: usize, equivalence_classes: usize) -> EvidenceScale {
        EvidenceScale {
            instances,
            equivalence_classes,
        }
    }

    /// instances / classes. 1.0 means every instance was independent.
    pub fn inflation_ratio(&self) -> f64 {
        if self.equivalence_classes == 0 {
            0.0
        } else {
            self.instances as f64 / self.equivalence_classes as f64
        }
    }

    /// Refuse a scale that headlines instances without classes, or that claims more classes than
    /// instances (which no counting procedure can produce).
    pub fn validate(&self) -> Result<(), HubError> {
        if self.instances > 0 && self.equivalence_classes == 0 {
            return Err(HubError::InstanceCountWithoutClasses {
                instances: self.instances,
                classes: self.equivalence_classes,
            });
        }
        if self.equivalence_classes > self.instances {
            return Err(HubError::InstanceCountWithoutClasses {
                instances: self.instances,
                classes: self.equivalence_classes,
            });
        }
        Ok(())
    }

    /// The sentence a hub page must lead with. Classes first, instances second.
    pub fn headline(&self) -> String {
        format!(
            "{} independent equivalence class(es) across {} instance(s) (inflation x{:.2}). \
             Instance count is not benchmark count.",
            self.equivalence_classes,
            self.instances,
            self.inflation_ratio()
        )
    }
}

/// A submission as presented, before the contract has been checked.
///
/// Every mandatory field is `Option` so that [`accept`] can name the one that is missing. The
/// alternative — a constructor taking nine arguments — makes the same mistakes silently, by
/// position.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SubmissionDraft {
    pub id: Option<SubmissionId>,
    pub submitter: Option<SubmitterId>,
    pub content: Option<ContentHash>,
    pub scope: Option<DeclaredScope>,
    pub licence: Option<Licence>,
    pub provenance: Option<Provenance>,
    pub does_not_establish: Vec<NonClaim>,
    /// Attributions the submitter carries forward. Checked against ancestry, not trusted.
    pub attributions: Vec<Attribution>,
    pub evidence_scale: Option<EvidenceScale>,
    /// Refused above [`VerificationStatus::SelfReported`]. Present at all only so that claiming
    /// more produces a named refusal instead of being quietly overwritten.
    pub claimed_verification: Option<VerificationStatus>,
    pub submitted_at: Epoch,
}

/// A submission that satisfies the contract.
///
/// Only [`accept`] constructs one. The fields are public for reading and serialization; the type
/// cannot be assembled field-by-field from outside because `licence_stack` is opaque and only
/// [`LicenceStack::derive`] produces it. That is the invariant: if you are holding a `Submission`,
/// its licence position was computed from its ancestry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Submission {
    pub id: SubmissionId,
    pub submitter: SubmitterId,
    pub content: ContentHash,
    pub scope: DeclaredScope,
    pub licence_stack: LicenceStack,
    pub provenance: Provenance,
    pub does_not_establish: Vec<NonClaim>,
    pub evidence_scale: Option<EvidenceScale>,
    pub verification: VerificationStatus,
    pub submitted_at: Epoch,
}

impl Submission {
    /// Whether the submission disowns a given inference.
    pub fn disclaims(&self, kind: NonClaimKind) -> bool {
        self.does_not_establish.iter().any(|c| c.kind == kind)
    }

    /// The limitations card of 34.15, rendered from the declared scope and nonclaims. Deliberately
    /// plain text: a card that needs a renderer to be readable is a card that gets dropped from
    /// exports.
    pub fn limitations_card(&self) -> String {
        let mut out = String::new();
        out.push_str("Intended use: ");
        out.push_str(&self.scope.intended_use);
        out.push_str("\nDecision families: ");
        out.push_str(&self.scope.decision_family.join(", "));
        if !self.scope.out_of_scope.is_empty() {
            out.push_str("\nOut of scope: ");
            out.push_str(&self.scope.out_of_scope.join(", "));
        }
        out.push_str("\nThis submission does not establish:");
        for claim in &self.does_not_establish {
            out.push_str("\n  - ");
            out.push_str(&claim.statement);
        }
        if let Some(scale) = &self.evidence_scale {
            out.push_str("\nEvidence scale: ");
            out.push_str(&scale.headline());
        }
        let credit = self.licence_stack.credit_line();
        if !credit.is_empty() {
            out.push('\n');
            out.push_str(&credit);
        }
        out
    }
}

/// Nonclaims the hub refuses a submission without.
const REQUIRED_NONCLAIMS: [NonClaimKind; 1] = [NonClaimKind::ClinicalValidity];

/// Check the submission contract, or refuse naming the field.
///
/// Order of checks is chosen so the submitter sees the refusal they can act on first: eligibility
/// (nothing else matters if the account cannot submit), then identity agreement, then field
/// completeness, then the nonclaim floor, then licence propagation.
pub fn accept(draft: SubmissionDraft, submitter: &Submitter) -> Result<Submission, HubError> {
    submitter.check_eligibility()?;

    let id = draft.id.ok_or(HubError::MissingRequiredField {
        field: "id",
        why: "a submission that cannot be cited cannot be corrected or superseded",
    })?;
    let declared_submitter = draft.submitter.ok_or(HubError::MissingRequiredField {
        field: "submitter",
        why: "every published record names who asserted it",
    })?;
    if declared_submitter != submitter.id {
        return Err(HubError::SubmitterMismatch {
            declared: declared_submitter.to_string(),
            presenting: submitter.id.to_string(),
        });
    }

    let content = draft.content.ok_or(HubError::MissingRequiredField {
        field: "content",
        why: "a score with no content digest resolves to no immutable result object (34.15)",
    })?;

    let scope = draft.scope.ok_or(HubError::MissingRequiredField {
        field: "scope",
        why: "43.43 forbids universal claims, so a claim must declare its boundary",
    })?;
    if scope.decision_family.is_empty() {
        return Err(HubError::MissingRequiredField {
            field: "scope.decision_family",
            why: "a result about no decision family is a result about nothing",
        });
    }
    if scope.intended_use.trim().is_empty() {
        return Err(HubError::MissingRequiredField {
            field: "scope.intended_use",
            why: "a reader cannot judge misuse without a declared use",
        });
    }

    let licence = draft.licence.ok_or(HubError::MissingRequiredField {
        field: "licence",
        why: "36.14 makes redistribution terms inheritable, so they must exist to inherit",
    })?;

    let provenance = draft.provenance.ok_or(HubError::MissingRequiredField {
        field: "provenance",
        why: "34.15 requires every rendered score to resolve to immutable result objects",
    })?;
    if provenance.build.toolchain.trim().is_empty() {
        return Err(HubError::MissingRequiredField {
            field: "provenance.build.toolchain",
            why: "a build nobody can name is a build nobody can repeat",
        });
    }

    if draft.does_not_establish.is_empty() {
        return Err(HubError::MissingRequiredField {
            field: "does_not_establish",
            why: "36.12 requires research evidence to be distinguished from clinical direction",
        });
    }
    for required in REQUIRED_NONCLAIMS {
        if !draft.does_not_establish.iter().any(|c| c.kind == required) {
            return Err(HubError::MissingRequiredNonClaim {
                kind: required.as_str(),
            });
        }
    }
    for claim in &draft.does_not_establish {
        if claim.statement.trim().is_empty() {
            return Err(HubError::MissingRequiredField {
                field: "does_not_establish[].statement",
                why: "a nonclaim with no sentence disclaims nothing",
            });
        }
    }

    if let Some(scale) = &draft.evidence_scale {
        scale.validate()?;
    }

    match draft.claimed_verification {
        None | Some(VerificationStatus::SelfReported) => {}
        Some(claimed) => {
            return Err(HubError::SelfAssertedVerification {
                claimed: claimed.as_str(),
            })
        }
    }

    let licence_stack =
        LicenceStack::derive(&licence, &provenance.ancestors, &draft.attributions)?;

    Ok(Submission {
        id,
        submitter: submitter.id.clone(),
        content,
        scope,
        licence_stack,
        provenance,
        does_not_establish: draft.does_not_establish,
        evidence_scale: draft.evidence_scale,
        verification: VerificationStatus::SelfReported,
        submitted_at: draft.submitted_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribution::Redistribution;
    use crate::fixtures::{complete_draft, submitter};

    #[test]
    fn a_complete_draft_is_accepted_as_self_reported() {
        let submission = accept(complete_draft(), &submitter()).expect("complete draft");
        assert_eq!(submission.verification, VerificationStatus::SelfReported);
        assert!(submission.disclaims(NonClaimKind::ClinicalValidity));
        assert_eq!(
            submission.licence_stack.effective().redistribution,
            Redistribution::Permissive
        );
    }

    #[test]
    fn a_submission_missing_a_content_digest_is_refused_naming_the_field() {
        let mut draft = complete_draft();
        draft.content = None;
        let err = accept(draft, &submitter()).expect_err("no digest");
        assert!(matches!(
            err,
            HubError::MissingRequiredField { field: "content", .. }
        ));
    }

    #[test]
    fn every_mandatory_field_is_refused_by_name_when_absent() {
        type Clear = fn(&mut SubmissionDraft);
        let cases: [(&str, Clear); 5] = [
            ("content", |d| d.content = None),
            ("scope", |d| d.scope = None),
            ("licence", |d| d.licence = None),
            ("provenance", |d| d.provenance = None),
            ("does_not_establish", |d| d.does_not_establish.clear()),
        ];
        for (name, clear) in cases {
            let mut draft = complete_draft();
            clear(&mut draft);
            match accept(draft, &submitter()) {
                Err(HubError::MissingRequiredField { field, .. }) => assert_eq!(field, name),
                other => panic!("expected `{name}` to be named, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_submission_that_does_not_disclaim_clinical_validity_is_refused() {
        let mut draft = complete_draft();
        draft.does_not_establish = vec![NonClaim::new(
            NonClaimKind::Generalization,
            "does not transfer outside glioma",
        )];
        let err = accept(draft, &submitter()).expect_err("no clinical-validity nonclaim");
        assert_eq!(
            err,
            HubError::MissingRequiredNonClaim {
                kind: "clinical-validity"
            }
        );
    }

    #[test]
    fn a_submitter_cannot_declare_its_own_result_verified() {
        let mut draft = complete_draft();
        draft.claimed_verification = Some(VerificationStatus::Verified);
        let err = accept(draft, &submitter()).expect_err("self-asserted verification");
        assert_eq!(
            err,
            HubError::SelfAssertedVerification {
                claimed: "verified"
            }
        );
    }

    #[test]
    fn a_suspended_submitter_is_refused_before_any_field_is_read() {
        let suspended = Submitter {
            id: SubmitterId::parse("lab-a").unwrap(),
            standing: Standing::Suspended {
                reason: "repeated contamination".into(),
            },
            conflicts_declared: true,
            conflicts: Vec::new(),
        };
        let empty = SubmissionDraft::default();
        let err = accept(empty, &suspended).expect_err("suspended");
        assert!(matches!(err, HubError::SubmitterSuspended { .. }));
    }

    #[test]
    fn silence_about_conflicts_is_not_a_declaration_of_none() {
        let silent = Submitter::unverified(SubmitterId::parse("lab-a").unwrap());
        let err = accept(complete_draft(), &silent).expect_err("conflicts undeclared");
        assert_eq!(
            err,
            HubError::ConflictsUndeclared {
                submitter: "lab-a".into()
            }
        );
        let declared = silent.declaring_no_conflicts();
        assert!(accept(complete_draft(), &declared).is_ok());
    }

    #[test]
    fn a_draft_presented_under_another_partys_name_is_refused() {
        let mut draft = complete_draft();
        draft.submitter = Some(SubmitterId::parse("lab-b").unwrap());
        let err = accept(draft, &submitter()).expect_err("mismatched submitter");
        assert_eq!(
            err,
            HubError::SubmitterMismatch {
                declared: "lab-b".into(),
                presenting: "lab-a".into(),
            }
        );
    }

    #[test]
    fn instance_count_without_equivalence_classes_is_refused() {
        let mut draft = complete_draft();
        draft.evidence_scale = Some(EvidenceScale::new(1_000_000, 0));
        let err = accept(draft, &submitter()).expect_err("inflated instance count");
        assert_eq!(
            err,
            HubError::InstanceCountWithoutClasses {
                instances: 1_000_000,
                classes: 0
            }
        );
    }

    #[test]
    fn the_evidence_headline_leads_with_classes_not_instances() {
        let scale = EvidenceScale::new(1000, 4);
        let headline = scale.headline();
        let classes_at = headline.find("4 independent").expect("classes present");
        let instances_at = headline.find("1000 instance").expect("instances present");
        assert!(classes_at < instances_at, "headline was: {headline}");
        assert!((scale.inflation_ratio() - 250.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_limitations_card_always_carries_the_nonclaims() {
        let submission = accept(complete_draft(), &submitter()).unwrap();
        let card = submission.limitations_card();
        assert!(card.contains("does not establish"));
        assert!(card.contains("no clinical validity"));
        assert!(card.contains("paediatric cohorts"));
    }

    #[test]
    fn an_accepted_submission_round_trips_through_json() {
        let submission = accept(complete_draft(), &submitter()).unwrap();
        let encoded = serde_json::to_string(&submission).expect("serialize");
        let decoded: Submission = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, submission);
    }
}
