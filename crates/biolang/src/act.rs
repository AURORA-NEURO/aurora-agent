//! BioWeave Role and Scientific Act IR — blueprint 25.15.
//!
//! 25.15's purpose is one sentence: "Extend Weave with typed scientific roles and state
//! transitions." So this module is a projection of `bioprism-weave`, not a rival kernel. Weave owns
//! the communicative acts, the commitment and epistemic ledgers, the authority table and the affine
//! budgets; what is added here is the scientific vocabulary 25.15 names and the two transitions it
//! requires — evidence and specimen.
//!
//! # Where the IR and the implementing crate disagree
//!
//! 25.15 requires "acts such as hypothesize, measure, challenge, reproduce, reserve, attest,
//! retract". `bioprism-weave`'s `ActKind` is `ask, claim, propose, accept, reject, challenge,
//! discharge, delegate, revoke, attest`. Two of the seven line up exactly; the rest do not, and the
//! mismatch is not cosmetic:
//!
//! - **`measure`, `reproduce` and `reserve` have no communicative act.** They are not speech; they
//!   are things done to material and instruments, which is precisely why 25.15 asks for them and
//!   why a communication kernel does not have them. [`ScientificActKind::communicative_act`]
//!   returns `None` for all three, rather than mapping them onto `claim` and losing the distinction
//!   between doing an experiment and saying you did one.
//! - **`retract` has no counterpart at all.** Weave's `challenge` documentation is explicit that a
//!   challenge "does not retract" a claim and that both survive in the ledger, and `revoke` applies
//!   to authority grants, not to claims. A scientific record needs retraction; the microkernel
//!   deliberately has no act that removes a claim. This IR therefore carries `Retract` as an act
//!   with no communicative projection, and a runtime that needs it must add a *superseding* entry
//!   rather than deleting one.
//!
//! # What is deliberately not implemented
//!
//! No kernel, no session types, no ledger enforcement. `bioprism-weave` has all of that and it is a
//! trusted computing base whose size is a design constraint; a second copy here would be a second
//! thing to audit.

use crate::error::ActError;
use crate::ids::ActId;
use bioprism_bioir::{EvidenceId, SpecimenId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The acts 25.15 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScientificActKind {
    Hypothesize,
    Measure,
    Challenge,
    Reproduce,
    Reserve,
    Attest,
    Retract,
}

impl ScientificActKind {
    pub const ALL: [ScientificActKind; 7] = [
        ScientificActKind::Hypothesize,
        ScientificActKind::Measure,
        ScientificActKind::Challenge,
        ScientificActKind::Reproduce,
        ScientificActKind::Reserve,
        ScientificActKind::Attest,
        ScientificActKind::Retract,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ScientificActKind::Hypothesize => "hypothesize",
            ScientificActKind::Measure => "measure",
            ScientificActKind::Challenge => "challenge",
            ScientificActKind::Reproduce => "reproduce",
            ScientificActKind::Reserve => "reserve",
            ScientificActKind::Attest => "attest",
            ScientificActKind::Retract => "retract",
        }
    }

    /// The `bioprism-weave` `ActKind` this projects onto, by name, when one exists.
    ///
    /// `None` is a finding, not a placeholder. See the module documentation.
    pub fn communicative_act(self) -> Option<&'static str> {
        match self {
            ScientificActKind::Hypothesize => Some("claim"),
            ScientificActKind::Challenge => Some("challenge"),
            ScientificActKind::Attest => Some("attest"),
            ScientificActKind::Measure
            | ScientificActKind::Reproduce
            | ScientificActKind::Reserve
            | ScientificActKind::Retract => None,
        }
    }

    /// True when the act must post to a ledger. 25.15: "Acts update explicit ledgers rather than
    /// only transcript text."
    pub fn requires_ledger_entry(self) -> bool {
        !matches!(self, ScientificActKind::Hypothesize)
    }
}

impl fmt::Display for ScientificActKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a role may do and how far its word goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioRole {
    pub role: String,
    /// What the role is competent to do, in prose. 25.15 requires "role capability".
    pub capability: String,
    /// The domain the role's authority covers, e.g. `"neuro-oncology imaging"`.
    pub domain_scope: String,
    pub allowed_acts: BTreeSet<ScientificActKind>,
    /// The `bioprism-weave` capabilities this role holds.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub authority: BTreeSet<String>,
    /// Evidence the role must supply when it claims.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub evidence_obligations: BTreeSet<String>,
}

impl BioRole {
    pub fn new(
        role: impl Into<String>,
        capability: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        BioRole {
            role: role.into(),
            capability: capability.into(),
            domain_scope: domain.into(),
            allowed_acts: BTreeSet::new(),
            authority: BTreeSet::new(),
            evidence_obligations: BTreeSet::new(),
        }
    }

    pub fn permitting(mut self, kind: ScientificActKind) -> Self {
        self.allowed_acts.insert(kind);
        self
    }

    pub fn holding(mut self, capability: impl Into<String>) -> Self {
        self.authority.insert(capability.into());
        self
    }
}

/// How strongly the actor is bound by the act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentType {
    /// Offered for consideration; binds nobody.
    Tentative,
    /// Asserted, and the actor stands behind it.
    Asserted,
    /// Undertaken as an obligation to be discharged.
    Undertaken,
}

/// What the act did to the evidence ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence_transition", rename_all = "snake_case")]
pub enum EvidenceTransition {
    Introduced {
        evidence: EvidenceId,
    },
    Corroborated {
        evidence: EvidenceId,
    },
    Contradicted {
        evidence: EvidenceId,
    },
    /// Superseded rather than removed. Weave keeps both positions; so does this.
    Superseded {
        evidence: EvidenceId,
        by: EvidenceId,
    },
}

/// What the act did to material. 25.15: "A specimen act obeys material conservation."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "specimen_transition", rename_all = "snake_case")]
pub enum SpecimenTransition {
    Reserved { specimen: SpecimenId, amount: f64 },
    Consumed { specimen: SpecimenId, amount: f64 },
    Released { specimen: SpecimenId, amount: f64 },
}

impl SpecimenTransition {
    pub fn specimen(&self) -> &SpecimenId {
        match self {
            SpecimenTransition::Reserved { specimen, .. }
            | SpecimenTransition::Consumed { specimen, .. }
            | SpecimenTransition::Released { specimen, .. } => specimen,
        }
    }

    /// How much material this removes from the available pool. Releases return it.
    pub fn draw(&self) -> f64 {
        match self {
            SpecimenTransition::Reserved { amount, .. }
            | SpecimenTransition::Consumed { amount, .. } => *amount,
            SpecimenTransition::Released { amount, .. } => -*amount,
        }
    }
}

/// One scientific act.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificAct {
    pub act_id: ActId,
    pub kind: ScientificActKind,
    pub actor_role: String,
    pub commitment: CommitmentType,
    /// Evidence the act identifies. 25.15: "A claim act identifies evidence and scope."
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub cites: BTreeSet<EvidenceId>,
    /// The scope the claim is made in, as a rendered scope key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_transitions: Vec<EvidenceTransition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub specimen_transitions: Vec<SpecimenTransition>,
}

impl ScientificAct {
    pub fn new(
        act_id: ActId,
        kind: ScientificActKind,
        actor_role: impl Into<String>,
        commitment: CommitmentType,
    ) -> Self {
        ScientificAct {
            act_id,
            kind,
            actor_role: actor_role.into(),
            commitment,
            cites: BTreeSet::new(),
            claim_scope: None,
            evidence_transitions: Vec::new(),
            specimen_transitions: Vec::new(),
        }
    }

    pub fn citing(mut self, evidence: EvidenceId) -> Self {
        self.cites.insert(evidence);
        self
    }

    pub fn scoped(mut self, scope: impl Into<String>) -> Self {
        self.claim_scope = Some(scope.into());
        self
    }

    pub fn moving_evidence(mut self, transition: EvidenceTransition) -> Self {
        self.evidence_transitions.push(transition);
        self
    }

    pub fn moving_material(mut self, transition: SpecimenTransition) -> Self {
        self.specimen_transitions.push(transition);
        self
    }

    /// The invariants 25.15 states, given the role that performed the act and what material is left.
    pub fn validate(
        &self,
        role: &BioRole,
        available: &dyn Fn(&SpecimenId) -> f64,
    ) -> Result<(), ActError> {
        let act = self.act_id.to_string();

        if !role.allowed_acts.contains(&self.kind) {
            return Err(ActError::RoleNotAuthorised {
                role: role.role.clone(),
                act_kind: self.kind.to_string(),
            });
        }

        if matches!(
            self.kind,
            ScientificActKind::Hypothesize
                | ScientificActKind::Challenge
                | ScientificActKind::Attest
        ) {
            if self.cites.is_empty() {
                return Err(ActError::ClaimWithout {
                    act,
                    missing: "evidence".to_string(),
                });
            }
            if self.claim_scope.is_none() {
                return Err(ActError::ClaimWithout {
                    act,
                    missing: "scope".to_string(),
                });
            }
        }

        if self.kind.requires_ledger_entry()
            && self.evidence_transitions.is_empty()
            && self.specimen_transitions.is_empty()
        {
            return Err(ActError::ActWithoutLedgerEntry {
                act,
                act_kind: self.kind.to_string(),
            });
        }

        for transition in &self.specimen_transitions {
            let draw = transition.draw();
            if draw <= 0.0 {
                continue;
            }
            let remaining = available(transition.specimen());
            if draw > remaining {
                return Err(ActError::MaterialOverdrawn {
                    act: act.clone(),
                    specimen: transition.specimen().to_string(),
                    requested: draw,
                    available: remaining,
                });
            }
        }

        Ok(())
    }
}
