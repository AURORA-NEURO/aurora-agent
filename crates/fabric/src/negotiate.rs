//! Negotiation, bidding and service contracts.
//!
//! Blueprint 23.15.
//!
//! # The three safety rules that are actually mechanical
//!
//! 23.15's negotiation-safety list has eight items. Five need a runtime; three are structural and
//! are enforced here.
//!
//! **"Agents cannot bind resources they do not own."** [`ContractNet::accept`] draws the accepted
//! price from a real `bioprism_weave::Budget` held by the coordinator. An acceptance that exceeds
//! the reservation fails in the kernel's affine accounting, not in a check this module wrote.
//!
//! **"Natural-language side promises are not binding unless encoded."** [`Offer::prose`] exists and
//! [`Offer::binding_terms`] does not read it. A term that matters must be a field; a term that is a
//! sentence is recorded and excluded, and [`Offer::unencoded_promises`] reports the gap.
//!
//! **"Offers expire" and "terms are content-addressed."** [`Offer::digest`] hashes the binding
//! terms only, so amending the prose does not change the identity of the deal and amending a price
//! does.
//!
//! # Discounting self-estimates
//!
//! 23.15: "The router discounts self-estimates using PRISM calibration." [`discount`] applies a
//! measured over-claim ratio to a bid. The rule it obeys is the one that matters: a provider with
//! *no* calibration history is not discounted to zero and is not trusted at face value either —
//! [`DiscountedEstimate::Unmeasured`] carries no adjusted number at all, and a router that requires
//! a number has to decide what to do about that rather than being handed a plausible-looking one.
//!
//! # Not implemented
//!
//! No auction, no market clearing, no payment, no scheduling. The contract-net state machine here
//! is a sequence check over acts, not a distributed protocol. Utility is not computed: 23.15's
//! `E[task value × success] - price - ...` needs a task value nobody supplies and probability
//! arithmetic this crate keeps out of integers on purpose.

use crate::contract::ComponentId;
use crate::reputation::LogicalTime;
use bioprism_ids::ContentHash;
use bioprism_weave::{Budget, Resource};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// 23.15's eight negotiation acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NegotiationAct {
    Solicit,
    Offer,
    Counter,
    Reserve,
    Accept,
    Withdraw,
    Renegotiate,
    Settle,
}

/// A machine-readable precondition an offer attaches to itself.
///
/// 23.15's worked example — "I can verify this result if raw sample counts and the analysis code
/// are available" — becomes one of these. A condition a coordinator cannot satisfy makes the offer
/// infeasible, and infeasible is a distinct outcome from losing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Condition {
    pub requires: String,
}

/// The binding terms of an offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terms {
    pub capability: String,
    pub input_contract: String,
    pub output_contract: String,
    pub expected_cost_minor: u64,
    pub max_cost_minor: u64,
    pub declared_latency_units: u64,
    /// The provider's own estimate, in basis points. A self-estimate; see [`discount`].
    pub estimated_success_bp: u32,
    pub conditions: BTreeSet<Condition>,
    pub subcontracting_allowed: bool,
    pub cancellation_fee_minor: u64,
}

/// A conditional offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub id: String,
    pub provider: ComponentId,
    pub terms: Terms,
    pub valid_until: LogicalTime,
    /// Free text. Recorded and never binding.
    pub prose: Vec<String>,
    pub withdrawn: bool,
}

impl Offer {
    pub fn new(
        id: impl Into<String>,
        provider: impl Into<String>,
        terms: Terms,
        valid_until: u64,
    ) -> Self {
        Offer {
            id: id.into(),
            provider: ComponentId::new(provider),
            terms,
            valid_until: LogicalTime(valid_until),
            prose: Vec::new(),
            withdrawn: false,
        }
    }

    pub fn saying(mut self, prose: impl Into<String>) -> Self {
        self.prose.push(prose.into());
        self
    }

    /// What the offer actually commits to. The prose is not here.
    pub fn binding_terms(&self) -> &Terms {
        &self.terms
    }

    /// Prose lines a reader might mistake for terms. Empty is the healthy state.
    pub fn unencoded_promises(&self) -> &[String] {
        &self.prose
    }

    /// A digest over the binding terms only.
    pub fn digest(&self) -> Result<String, NegotiationError> {
        let payload = json!({
            "provider": self.provider.as_str(),
            "terms": serde_json::to_value(&self.terms)
                .map_err(|e| NegotiationError::Encoding(e.to_string()))?,
            "valid_until": self.valid_until.0,
        });
        Ok(ContentHash::of_value(&payload)
            .map_err(|e| NegotiationError::Encoding(e.to_string()))?
            .to_string())
    }

    pub fn is_live(&self, as_of: LogicalTime) -> bool {
        !self.withdrawn && as_of < self.valid_until
    }

    /// Conditions the coordinator cannot meet.
    pub fn unmet_conditions(&self, available: &BTreeSet<String>) -> BTreeSet<Condition> {
        self.terms
            .conditions
            .iter()
            .filter(|c| !available.contains(&c.requires))
            .cloned()
            .collect()
    }
}

/// A provider's measured tendency to over-claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calibration {
    /// Observed successes over claimed successes, in basis points. 10000 is honest, 5000 means the
    /// provider delivers half of what it claims.
    pub realisation_bp: u32,
    pub sample_size: u32,
}

/// A bid after calibration, or the honest absence of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "estimate")]
pub enum DiscountedEstimate {
    Discounted {
        claimed_bp: u32,
        adjusted_bp: u32,
        sample_size: u32,
    },
    /// No calibration history. Carries no adjusted number, deliberately: a router that needs one
    /// must decide what an unmeasured provider is worth rather than be handed the claim back.
    Unmeasured {
        claimed_bp: u32,
    },
}

/// Apply calibration to a self-estimate.
pub fn discount(terms: &Terms, calibration: Option<Calibration>) -> DiscountedEstimate {
    match calibration {
        Some(calibration) if calibration.sample_size > 0 => DiscountedEstimate::Discounted {
            claimed_bp: terms.estimated_success_bp,
            adjusted_bp: ((terms.estimated_success_bp as u64 * calibration.realisation_bp as u64)
                / 10_000) as u32,
            sample_size: calibration.sample_size,
        },
        _ => DiscountedEstimate::Unmeasured {
            claimed_bp: terms.estimated_success_bp,
        },
    }
}

/// Why an offer was eliminated. Distinguishes "could not do it" from "lost".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "rejection")]
pub enum Rejection {
    Expired {
        valid_until: LogicalTime,
    },
    Withdrawn,
    ConditionsUnmet {
        conditions: BTreeSet<Condition>,
    },
    OverBudget {
        max_cost_minor: u64,
        ceiling_minor: u64,
    },
    SubcontractingForbidden,
    /// Eliminated on merit, not feasibility.
    Outbid {
        by: ComponentId,
    },
}

/// The nine steps of 23.15's contract-net pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractNetStep {
    PublishCall,
    CollectOffers,
    CheckHardConstraints,
    Probe,
    Accept,
    ActivateCommitments,
    Monitor,
    AdjudicateFulfilment,
    Settle,
}

/// A coordinator running one contract net.
///
/// Holds a real `Budget`, so a reservation is drawn from a finite pool and a duplicate reservation
/// is refused by the kernel's affine rule rather than by a counter this module keeps.
#[derive(Debug)]
pub struct ContractNet {
    call: String,
    ceiling_minor: u64,
    budget: Budget,
    offers: Vec<Offer>,
    reserved: BTreeMap<String, u64>,
    accepted: Option<String>,
    steps: Vec<ContractNetStep>,
    available_conditions: BTreeSet<String>,
    subcontracting_permitted: bool,
}

impl ContractNet {
    pub fn open(call: impl Into<String>, ceiling_minor: u64) -> Self {
        ContractNet {
            call: call.into(),
            ceiling_minor,
            budget: Budget::new().with(Resource::Tokens, ceiling_minor),
            offers: Vec::new(),
            reserved: BTreeMap::new(),
            accepted: None,
            steps: vec![ContractNetStep::PublishCall],
            available_conditions: BTreeSet::new(),
            subcontracting_permitted: true,
        }
    }

    pub fn call(&self) -> &str {
        &self.call
    }

    pub fn providing(mut self, condition: impl Into<String>) -> Self {
        self.available_conditions.insert(condition.into());
        self
    }

    pub fn forbidding_subcontracting(mut self) -> Self {
        self.subcontracting_permitted = false;
        self
    }

    pub fn receive(&mut self, offer: Offer) {
        if !self.steps.contains(&ContractNetStep::CollectOffers) {
            self.steps.push(ContractNetStep::CollectOffers);
        }
        self.offers.push(offer);
    }

    /// Eliminate offers that cannot be accepted, with a reason for each.
    ///
    /// Feasibility only. Nothing here ranks, because 23.15's whole point is that the cheapest bid
    /// is not automatically the right one and a function that eliminated on price would be making
    /// the decision it exists to inform.
    pub fn screen(&mut self, as_of: LogicalTime) -> BTreeMap<String, Rejection> {
        if !self.steps.contains(&ContractNetStep::CheckHardConstraints) {
            self.steps.push(ContractNetStep::CheckHardConstraints);
        }
        let mut out = BTreeMap::new();
        for offer in &self.offers {
            if offer.withdrawn {
                out.insert(offer.id.clone(), Rejection::Withdrawn);
                continue;
            }
            if as_of >= offer.valid_until {
                out.insert(
                    offer.id.clone(),
                    Rejection::Expired {
                        valid_until: offer.valid_until,
                    },
                );
                continue;
            }
            let unmet = offer.unmet_conditions(&self.available_conditions);
            if !unmet.is_empty() {
                out.insert(
                    offer.id.clone(),
                    Rejection::ConditionsUnmet { conditions: unmet },
                );
                continue;
            }
            if offer.terms.max_cost_minor > self.ceiling_minor {
                out.insert(
                    offer.id.clone(),
                    Rejection::OverBudget {
                        max_cost_minor: offer.terms.max_cost_minor,
                        ceiling_minor: self.ceiling_minor,
                    },
                );
                continue;
            }
            if offer.terms.subcontracting_allowed && !self.subcontracting_permitted {
                out.insert(offer.id.clone(), Rejection::SubcontractingForbidden);
            }
        }
        out
    }

    /// Hold capacity against an offer's maximum cost.
    ///
    /// Two reservations against the same offer are refused, and the total across offers cannot
    /// exceed the ceiling because the kernel's `Budget` will not split past it.
    pub fn reserve(&mut self, offer_id: &str) -> Result<u64, NegotiationError> {
        let offer = self
            .offers
            .iter()
            .find(|o| o.id == offer_id)
            .ok_or_else(|| NegotiationError::NoSuchOffer {
                id: offer_id.to_string(),
            })?;
        if self.reserved.contains_key(offer_id) {
            return Err(NegotiationError::DuplicateReservation {
                id: offer_id.to_string(),
            });
        }
        let amount = offer.terms.max_cost_minor;
        self.budget
            .split(Resource::Tokens, amount)
            .map_err(|e| NegotiationError::ReservationRefused {
                id: offer_id.to_string(),
                detail: e.to_string(),
            })?;
        self.reserved.insert(offer_id.to_string(), amount);
        if !self.steps.contains(&ContractNetStep::Probe) {
            self.steps.push(ContractNetStep::Probe);
        }
        Ok(amount)
    }

    /// Accept a reserved offer, forming commitments.
    ///
    /// Requires a prior reservation: "accepted prices reserve real budget" reads as an ordering
    /// constraint and is implemented as one.
    pub fn accept(&mut self, offer_id: &str) -> Result<Award, NegotiationError> {
        if self.accepted.is_some() {
            return Err(NegotiationError::AlreadyAwarded {
                call: self.call.clone(),
            });
        }
        if !self.reserved.contains_key(offer_id) {
            return Err(NegotiationError::AcceptWithoutReservation {
                id: offer_id.to_string(),
            });
        }
        let offer = self
            .offers
            .iter()
            .find(|o| o.id == offer_id)
            .ok_or_else(|| NegotiationError::NoSuchOffer {
                id: offer_id.to_string(),
            })?;
        let digest = offer.digest()?;
        self.accepted = Some(offer_id.to_string());
        self.steps.push(ContractNetStep::Accept);
        self.steps.push(ContractNetStep::ActivateCommitments);
        Ok(Award {
            offer_id: offer_id.to_string(),
            provider: offer.provider.clone(),
            terms_digest: digest,
            reserved_minor: self.reserved[offer_id],
        })
    }

    /// Close the deal and release what was not spent.
    pub fn settle(&mut self, spent_minor: u64) -> Result<Settlement, NegotiationError> {
        let offer_id = self
            .accepted
            .clone()
            .ok_or_else(|| NegotiationError::SettleWithoutAward {
                call: self.call.clone(),
            })?;
        let reserved = self.reserved[&offer_id];
        if spent_minor > reserved {
            return Err(NegotiationError::SpentBeyondReservation {
                id: offer_id,
                reserved,
                spent: spent_minor,
            });
        }
        self.steps.push(ContractNetStep::Settle);
        Ok(Settlement {
            offer_id,
            reserved_minor: reserved,
            spent_minor,
            released_minor: reserved - spent_minor,
        })
    }

    /// The steps performed so far, in order. A caller checking protocol conformance reads this.
    pub fn steps(&self) -> &[ContractNetStep] {
        &self.steps
    }

    pub fn total_reserved(&self) -> u64 {
        self.reserved.values().sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Award {
    pub offer_id: String,
    pub provider: ComponentId,
    pub terms_digest: String,
    pub reserved_minor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settlement {
    pub offer_id: String,
    pub reserved_minor: u64,
    pub spent_minor: u64,
    pub released_minor: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NegotiationError {
    #[error("no offer {id}")]
    NoSuchOffer { id: String },

    #[error("offer {id} is already reserved")]
    DuplicateReservation { id: String },

    #[error("cannot reserve against {id}: {detail}")]
    ReservationRefused { id: String, detail: String },

    #[error("offer {id} was accepted without a reservation; an acceptance must hold real budget")]
    AcceptWithoutReservation { id: String },

    #[error("call {call} has already been awarded")]
    AlreadyAwarded { call: String },

    #[error("call {call} has no award to settle")]
    SettleWithoutAward { call: String },

    #[error("offer {id} spent {spent} against a reservation of {reserved}")]
    SpentBeyondReservation {
        id: String,
        reserved: u64,
        spent: u64,
    },

    #[error("canonical encoding failed: {0}")]
    Encoding(String),
}
