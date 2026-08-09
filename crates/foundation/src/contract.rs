//! Falsifiable Biological Contracts.
//!
//! Blueprint 24.07 calls the FBC "the smallest semantically complete unit of biological
//! evaluation", and gives the reason plainly: contracts "prevent benchmark authors from grading
//! plausible prose without specifying what would count as wrong". That sentence is the whole
//! module. A contract with an empty falsifier list is not a strict contract with no falsifiers
//! yet — it is a benchmark that cannot be failed, and [`FalsifiableContract::admit`] refuses it.
//!
//! The second enforceable idea in 24.07 is inheritance. A specialized contract may refine a
//! broader one "only by narrowing scope, strengthening evidence obligations, restricting
//! actions, or improving the oracle. It cannot silently weaken safety or uncertainty
//! requirements." That is a partial order with five checkable conditions, and
//! [`FalsifiableContract::refines`] is that order. Scope narrowing delegates to
//! `bioprism_scope::ScopeKey::refines` rather than reimplementing a second, subtly different
//! refinement rule inside this crate.
//!
//! Not implemented: the oracle mesh itself. 24.07 names "how evidence is combined for scoring"
//! as a contract component, but combination logic belongs to the oracle crates. What is checked
//! here is that a reference standard is named and that a refinement never lowers the reviewer
//! floor — the only part of oracle strength that section 24 makes concrete.

use crate::boundary::UseTier;
use crate::error::{ContractError, RefinementViolation};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The five terminal states blueprint 24.07 admits.
///
/// `Underdetermined` and `Invalid` are the two that benchmarks usually lack, and their absence
/// is why systems get graded as wrong for correctly declining to answer an unanswerable
/// question, or for noticing that the task itself was malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    Success,
    Contradicted,
    Underdetermined,
    Invalid,
    OutOfBudget,
}

impl Termination {
    pub const ALL: [Termination; 5] = [
        Termination::Success,
        Termination::Contradicted,
        Termination::Underdetermined,
        Termination::Invalid,
        Termination::OutOfBudget,
    ];
}

/// The unchecked form of a contract, as authored or parsed.
///
/// Kept separate from [`FalsifiableContract`] so that the admissibility checks cannot be
/// skipped by constructing the checked type directly.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContractDraft {
    pub id: String,
    /// What question or decision is being evaluated.
    pub intent: String,
    /// Biological population, scale, time and modality, as a scope key.
    #[serde(default)]
    pub scope: ScopeKey,
    /// Allowed use. 24.07's example carries `evidence_use: research-evaluation-only`.
    pub evidence_use: Option<UseTier>,
    /// What must be true before the contract is meaningful.
    #[serde(default)]
    pub state_preconditions: BTreeSet<String>,
    /// Information required for a defensible conclusion.
    #[serde(default)]
    pub evidence_obligations: BTreeSet<String>,
    /// Allowed observations, assays, analyses or abstentions.
    #[serde(default)]
    pub actions: BTreeSet<String>,
    /// Structured output schema for the claim.
    pub claim_schema: String,
    /// Observations or outcomes that would challenge the claim.
    #[serde(default)]
    pub falsifiers: BTreeSet<String>,
    /// Named reference standard; how it is combined belongs to the oracle mesh.
    pub reference_standard: String,
    /// Reviewer floor for the reference standard, mirroring the blueprint's
    /// `minimum_reviewers`. Zero means "not a reviewed standard", which is legal for
    /// code-derived truth and refuses to be silently lowered by a refinement.
    #[serde(default)]
    pub minimum_reviewers: usize,
    /// Whether the claim must carry uncertainty.
    #[serde(default)]
    pub uncertainty_required: bool,
    /// Resource dimensions this contract charges against (24.10). Recorded, not enforced here.
    #[serde(default)]
    pub resource_ledger: BTreeSet<String>,
    #[serde(default)]
    pub terminations: BTreeSet<Termination>,
}

/// A contract that has passed admissibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContractDraft")]
pub struct FalsifiableContract {
    draft: ContractDraft,
}

impl TryFrom<ContractDraft> for FalsifiableContract {
    type Error = ContractError;

    fn try_from(draft: ContractDraft) -> Result<Self, Self::Error> {
        FalsifiableContract::admit(draft)
    }
}

impl FalsifiableContract {
    /// The admissibility gate of 24.07.
    ///
    /// Each refusal names the missing component rather than reporting "invalid contract",
    /// because the author is the person who has to fix it.
    pub fn admit(draft: ContractDraft) -> Result<Self, ContractError> {
        let id = draft.id.clone();
        if draft.intent.trim().is_empty() {
            return Err(ContractError::NoIntent { contract: id });
        }
        if draft.falsifiers.iter().all(|f| f.trim().is_empty()) {
            return Err(ContractError::NoFalsifier { contract: id });
        }
        if draft.actions.iter().all(|a| a.trim().is_empty()) {
            return Err(ContractError::NoAction { contract: id });
        }
        if draft.claim_schema.trim().is_empty() {
            return Err(ContractError::NoClaimSchema { contract: id });
        }
        if draft.reference_standard.trim().is_empty() {
            return Err(ContractError::NoReferenceStandard { contract: id });
        }
        if draft.terminations.is_empty() {
            return Err(ContractError::NoTermination { contract: id });
        }
        Ok(FalsifiableContract { draft })
    }

    pub fn id(&self) -> &str {
        &self.draft.id
    }

    pub fn intent(&self) -> &str {
        &self.draft.intent
    }

    pub fn scope(&self) -> &ScopeKey {
        &self.draft.scope
    }

    pub fn falsifiers(&self) -> impl Iterator<Item = &str> {
        self.draft.falsifiers.iter().map(String::as_str)
    }

    pub fn actions(&self) -> impl Iterator<Item = &str> {
        self.draft.actions.iter().map(String::as_str)
    }

    pub fn evidence_obligations(&self) -> impl Iterator<Item = &str> {
        self.draft.evidence_obligations.iter().map(String::as_str)
    }

    pub fn uncertainty_required(&self) -> bool {
        self.draft.uncertainty_required
    }

    pub fn minimum_reviewers(&self) -> usize {
        self.draft.minimum_reviewers
    }

    pub fn admits_termination(&self, termination: Termination) -> bool {
        self.draft.terminations.contains(&termination)
    }

    /// The contract-inheritance order of 24.07.
    ///
    /// Returns the first violation found rather than a list, because a refinement that breaks
    /// one of these rules is not partially valid.
    pub fn refines(&self, parent: &FalsifiableContract) -> Result<(), ContractError> {
        let violation = if !self.draft.scope.refines(&parent.draft.scope) {
            Some(RefinementViolation::WidensScope)
        } else if !parent
            .draft
            .evidence_obligations
            .is_subset(&self.draft.evidence_obligations)
        {
            Some(RefinementViolation::DropsEvidenceObligation)
        } else if !self.draft.actions.is_subset(&parent.draft.actions) {
            Some(RefinementViolation::AdmitsForbiddenAction)
        } else if !parent.draft.falsifiers.is_subset(&self.draft.falsifiers) {
            Some(RefinementViolation::DropsFalsifier)
        } else if parent.draft.uncertainty_required && !self.draft.uncertainty_required {
            Some(RefinementViolation::RelaxesUncertainty)
        } else if self.draft.minimum_reviewers < parent.draft.minimum_reviewers {
            Some(RefinementViolation::DropsEvidenceObligation)
        } else if weakens_use(parent.draft.evidence_use, self.draft.evidence_use) {
            Some(RefinementViolation::WeakensUseRestriction)
        } else {
            None
        };
        match violation {
            None => Ok(()),
            Some(reason) => Err(ContractError::IllegalRefinement {
                child: self.draft.id.clone(),
                parent: parent.draft.id.clone(),
                reason,
            }),
        }
    }
}

/// A child weakens the use restriction when the parent declared one and the child either drops
/// it or declares a broader tier. Only research-evaluation is admissible anywhere in the
/// system, so "broader" here means "anything else", including absent.
fn weakens_use(parent: Option<UseTier>, child: Option<UseTier>) -> bool {
    match (parent, child) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(parent), Some(child)) => parent.is_admissible() && !child.is_admissible(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ContractDraft {
        ContractDraft {
            id: "fbc:glioma:response:0042".to_string(),
            intent: "distinguish treatment effect from progressive disease".to_string(),
            scope: ScopeKey::new()
                .exact("population", "adult-diffuse-glioma")
                .exact("modality", "mri"),
            evidence_use: Some(UseTier::ResearchEvaluationOnly),
            state_preconditions: ["prior-scan-exists".to_string()].into(),
            evidence_obligations: ["treatment-timeline".to_string()].into(),
            actions: [
                "inspect-prior-scan".to_string(),
                "compute-volume-change".to_string(),
                "abstain".to_string(),
            ]
            .into(),
            claim_schema: "response-hypothesis@1".to_string(),
            falsifiers: ["later-confirmed-regression-without-new-therapy".to_string()].into(),
            reference_standard: "longitudinal-consensus-distribution".to_string(),
            minimum_reviewers: 2,
            uncertainty_required: true,
            resource_ledger: ["radiologist-time".to_string()].into(),
            terminations: Termination::ALL.into_iter().collect(),
        }
    }

    fn admitted() -> FalsifiableContract {
        FalsifiableContract::admit(draft()).unwrap()
    }

    #[test]
    fn a_contract_with_no_falsifier_is_not_a_claim_and_is_refused() {
        let mut draft = draft();
        draft.falsifiers.clear();
        let err = FalsifiableContract::admit(draft).unwrap_err();
        assert_eq!(
            err,
            ContractError::NoFalsifier {
                contract: "fbc:glioma:response:0042".to_string()
            }
        );
    }

    #[test]
    fn a_contract_with_no_reference_standard_leaves_wrong_undefined_and_is_refused() {
        let mut draft = draft();
        draft.reference_standard = "  ".to_string();
        assert!(matches!(
            FalsifiableContract::admit(draft).unwrap_err(),
            ContractError::NoReferenceStandard { .. }
        ));
    }

    #[test]
    fn a_contract_admitting_no_terminal_state_can_never_finish_and_is_refused() {
        let mut draft = draft();
        draft.terminations.clear();
        assert!(matches!(
            FalsifiableContract::admit(draft).unwrap_err(),
            ContractError::NoTermination { .. }
        ));
    }

    #[test]
    fn underdetermined_is_a_terminal_state_a_contract_may_offer() {
        assert!(admitted().admits_termination(Termination::Underdetermined));
    }

    #[test]
    fn a_child_that_widens_scope_does_not_refine_its_parent() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "fbc:glioma:response:0042:child".to_string();
        child_draft.scope = ScopeKey::new().exact("population", "adult-diffuse-glioma");
        let child = FalsifiableContract::admit(child_draft).unwrap();
        let err = child.refines(&parent).unwrap_err();
        assert!(matches!(
            err,
            ContractError::IllegalRefinement {
                reason: RefinementViolation::WidensScope,
                ..
            }
        ));
    }

    #[test]
    fn a_child_that_narrows_scope_and_keeps_everything_else_refines_its_parent() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft.scope = child_draft.scope.exact("site", "site-a");
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(child.refines(&parent).is_ok());
    }

    #[test]
    fn a_child_admitting_an_action_the_parent_forbade_does_not_refine_it() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft
            .actions
            .insert("order-second-biopsy".to_string());
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(matches!(
            child.refines(&parent).unwrap_err(),
            ContractError::IllegalRefinement {
                reason: RefinementViolation::AdmitsForbiddenAction,
                ..
            }
        ));
    }

    #[test]
    fn a_child_dropping_a_parent_falsifier_does_not_refine_it() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft.falsifiers =
            ["pathology-showing-predominant-treatment-effect".to_string()].into();
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(matches!(
            child.refines(&parent).unwrap_err(),
            ContractError::IllegalRefinement {
                reason: RefinementViolation::DropsFalsifier,
                ..
            }
        ));
    }

    #[test]
    fn a_child_cannot_silently_relax_the_parents_uncertainty_requirement() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft.uncertainty_required = false;
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(matches!(
            child.refines(&parent).unwrap_err(),
            ContractError::IllegalRefinement {
                reason: RefinementViolation::RelaxesUncertainty,
                ..
            }
        ));
    }

    #[test]
    fn a_child_cannot_lower_the_reviewer_floor_of_the_reference_standard() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft.minimum_reviewers = 1;
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(child.refines(&parent).is_err());
    }

    #[test]
    fn a_child_cannot_drop_the_parents_research_use_restriction() {
        let parent = admitted();
        let mut child_draft = draft();
        child_draft.id = "child".to_string();
        child_draft.evidence_use = None;
        let child = FalsifiableContract::admit(child_draft).unwrap();
        assert!(matches!(
            child.refines(&parent).unwrap_err(),
            ContractError::IllegalRefinement {
                reason: RefinementViolation::WeakensUseRestriction,
                ..
            }
        ));
    }

    #[test]
    fn a_child_may_strengthen_evidence_obligations_but_never_drop_one() {
        let parent = admitted();
        let mut stronger = draft();
        stronger.id = "stronger".to_string();
        stronger
            .evidence_obligations
            .insert("steroid-exposure".to_string());
        assert!(FalsifiableContract::admit(stronger)
            .unwrap()
            .refines(&parent)
            .is_ok());

        let mut weaker = draft();
        weaker.id = "weaker".to_string();
        weaker.evidence_obligations.clear();
        assert!(matches!(
            FalsifiableContract::admit(weaker)
                .unwrap()
                .refines(&parent)
                .unwrap_err(),
            ContractError::IllegalRefinement {
                reason: RefinementViolation::DropsEvidenceObligation,
                ..
            }
        ));
    }

    #[test]
    fn an_inadmissible_contract_arriving_as_json_is_refused_at_deserialization() {
        let json = serde_json::to_string(&{
            let mut d = draft();
            d.falsifiers.clear();
            d
        })
        .unwrap();
        assert!(serde_json::from_str::<FalsifiableContract>(&json).is_err());
    }
}
