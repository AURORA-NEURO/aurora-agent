//! The token, latency, privacy and tool budget controller.
//!
//! Blueprint 39.16 states four non-negotiable invariants, and each one is a method or a missing
//! method here:
//!
//! 1. *Budgets attenuate and cannot be duplicated by delegation.* A [`Lease`] can only sublease
//!    from its own remainder, so delegation strictly shrinks the resource. There is no way to mint
//!    a lease that is not carved out of a parent.
//! 2. *Safety and mandatory invariants are not traded away for tokens.* If the mandatory closure
//!    does not fit in the run budget, [`TokenBudgetController::plan`] returns
//!    [`crate::error::BudgetError::MandatoryClosureUnaffordable`] rather than dropping mandatory
//!    obligations to make the numbers work.
//! 3. *Spent privacy exposure is not refunded by deleting context.* [`BudgetLedger`] can record
//!    exposure and report it. It has no method that lowers it, deliberately.
//! 4. *Rejected branches retain their consumed resource ledger.* Rejecting a branch stops further
//!    spending and keeps every token it already burned.
//!
//! ## Counts are estimates, and say so
//!
//! Every token number in this module travels with the [`EstimationMethod`] that produced it, the
//! same discipline `bioprism_section::layers` applies to its own four-characters-per-token
//! heuristic. The default estimator here *is* that heuristic: it is not a tokenizer, it does not
//! know the provider's vocabulary, and it will be wrong. Summing counts from different estimators
//! yields [`EstimationMethod::Mixed`], because a total assembled from two different rulers is not
//! a measurement by either.
//!
//! Not implemented here: real provider tokenizers, latency budgets and tool-call leases. 39.16
//! covers all four resources; this module implements the token axis and the affine lease
//! machinery, and [`EstimationMethod::ProviderTokenizer`] is the seam where a real adapter
//! attaches.

use crate::error::BudgetError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a token count was arrived at. Reported next to every count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum EstimationMethod {
    /// Four characters per token over the serialised form. A heuristic, not a tokenizer.
    CharsPerToken4,
    /// The caller supplied the number and owns its accuracy.
    DeclaredByCaller,
    /// A named provider tokenizer produced the count.
    ProviderTokenizer { name: String },
    /// A sum over counts from more than one estimator.
    Mixed { methods: Vec<String> },
}

impl EstimationMethod {
    /// A short label for reports and events.
    pub fn label(&self) -> String {
        match self {
            EstimationMethod::CharsPerToken4 => "estimated:chars-per-token-4".to_string(),
            EstimationMethod::DeclaredByCaller => "declared-by-caller".to_string(),
            EstimationMethod::ProviderTokenizer { name } => format!("tokenizer:{name}"),
            EstimationMethod::Mixed { methods } => format!("mixed:{}", methods.join("+")),
        }
    }

    /// Whether the count is a measurement rather than a guess.
    pub fn is_measured(&self) -> bool {
        matches!(self, EstimationMethod::ProviderTokenizer { .. })
    }
}

/// A token count and the method that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEstimate {
    pub tokens: usize,
    pub method: EstimationMethod,
}

impl TokenEstimate {
    pub fn declared(tokens: usize) -> Self {
        TokenEstimate {
            tokens,
            method: EstimationMethod::DeclaredByCaller,
        }
    }

    /// The four-characters-per-token heuristic over a string.
    pub fn of_text(text: &str) -> Self {
        TokenEstimate {
            tokens: text.len() / 4,
            method: EstimationMethod::CharsPerToken4,
        }
    }

    /// The same heuristic over a JSON rendering.
    pub fn of_value(value: &Value) -> Self {
        let rendered = serde_json::to_string(value).unwrap_or_default();
        TokenEstimate::of_text(&rendered)
    }

    pub fn from_provider(tokens: usize, name: impl Into<String>) -> Self {
        TokenEstimate {
            tokens,
            method: EstimationMethod::ProviderTokenizer { name: name.into() },
        }
    }

    /// Renders the count with its method attached, for anywhere a human reads the number.
    pub fn label(&self) -> String {
        format!("{} tokens ({})", self.tokens, self.method.label())
    }

    /// Sums estimates, degrading the method to [`EstimationMethod::Mixed`] when they disagree.
    ///
    /// One estimator throughout means the total is still that estimator's number. Two or more
    /// means it is nobody's: the parts were measured with different rulers, and the sum inherits
    /// the accuracy of neither.
    pub fn sum<'a, I>(estimates: I) -> TokenEstimate
    where
        I: IntoIterator<Item = &'a TokenEstimate>,
    {
        let mut tokens = 0usize;
        let mut methods: Vec<EstimationMethod> = Vec::new();
        for estimate in estimates {
            tokens += estimate.tokens;
            if !methods.contains(&estimate.method) {
                methods.push(estimate.method.clone());
            }
        }
        let method = if methods.is_empty() {
            EstimationMethod::DeclaredByCaller
        } else if methods.len() == 1 {
            methods.swap_remove(0)
        } else {
            EstimationMethod::Mixed {
                methods: methods.iter().map(EstimationMethod::label).collect(),
            }
        };
        TokenEstimate { tokens, method }
    }
}

/// One obligation's claim on the run budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetRequest {
    pub obligation: String,
    /// Decision value, in whatever units the caller scores obligations in.
    pub value: f64,
    pub estimate: TokenEstimate,
    /// Mandatory requests belong to the closure and are funded before anything else.
    pub mandatory: bool,
}

impl BudgetRequest {
    pub fn new(obligation: impl Into<String>, value: f64, estimate: TokenEstimate) -> Self {
        BudgetRequest {
            obligation: obligation.into(),
            value,
            estimate,
            mandatory: false,
        }
    }

    pub fn mandatory(mut self) -> Self {
        self.mandatory = true;
        self
    }

    /// Value per estimated token. The greedy ordering key of 39.07's selection algorithm.
    pub fn value_density(&self) -> f64 {
        self.value / self.estimate.tokens.max(1) as f64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Allocation {
    pub obligation: String,
    pub tokens: usize,
    pub method: EstimationMethod,
    pub mandatory: bool,
    pub value_density: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    /// The remaining budget could not hold it.
    BudgetExhausted,
    /// Its value per token fell below the acquisition threshold, so it was never worth buying.
    BelowAcquisitionThreshold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    pub obligation: String,
    pub tokens: usize,
    pub method: EstimationMethod,
    pub reason: RejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopDecision {
    /// Everything worth buying fit.
    WithinBudget,
    /// Something was dropped for size; the caller must decide whether to escalate the budget.
    BudgetExhausted,
    /// Estimated and actual spend diverged beyond tolerance; the estimator is not trustworthy for
    /// this workload and further planning on it is not safe.
    EstimatorDriftEscalation,
}

/// The output of one planning pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetPlan {
    pub total_tokens: usize,
    /// Tokens reserved for the mandatory closure before any exploratory allocation.
    pub mandatory_reserved: usize,
    pub allocated: Vec<Allocation>,
    pub rejected: Vec<Rejection>,
    pub stop: StopDecision,
    /// Method of the summed allocation, so the headline number is never unlabelled.
    pub allocated_estimate: TokenEstimate,
}

impl BudgetPlan {
    pub fn allocated_tokens(&self) -> usize {
        self.allocated.iter().map(|entry| entry.tokens).sum()
    }

    pub fn remaining(&self) -> usize {
        self.total_tokens.saturating_sub(self.allocated_tokens())
    }

    pub fn allocated_obligations(&self) -> Vec<&str> {
        self.allocated
            .iter()
            .map(|entry| entry.obligation.as_str())
            .collect()
    }
}

/// Handle to a lease held by the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LeaseId(pub usize);

/// A delegated slice of the run budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub id: LeaseId,
    pub holder: String,
    pub parent: Option<LeaseId>,
    pub granted: usize,
    pub spent: usize,
    /// Tokens handed to children. Held against this lease so delegation cannot duplicate budget.
    pub subleased: usize,
    /// A rejected branch may not spend further, and keeps everything it spent.
    pub rejected: bool,
}

impl Lease {
    pub fn remaining(&self) -> usize {
        self.granted.saturating_sub(self.spent + self.subleased)
    }
}

/// Append-only budget events, per 39.16's persistence contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BudgetEvent {
    MandatoryReserved {
        tokens: usize,
    },
    Allocated {
        obligation: String,
        tokens: usize,
        method: String,
    },
    Rejected {
        obligation: String,
        tokens: usize,
        reason: RejectionReason,
    },
    Leased {
        holder: String,
        tokens: usize,
        parent: Option<String>,
    },
    Spent {
        holder: String,
        tokens: usize,
        method: String,
    },
    BranchRejected {
        holder: String,
        reason: String,
        consumed: usize,
    },
    Reconciled {
        obligation: String,
        estimated: usize,
        actual: usize,
        method: String,
        drift: f64,
    },
    PrivacyExposed {
        units: u64,
        detail: String,
    },
}

/// The append-only record. Nothing here subtracts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetLedger {
    events: Vec<BudgetEvent>,
    privacy_exposure: u64,
}

impl BudgetLedger {
    pub fn events(&self) -> &[BudgetEvent] {
        &self.events
    }

    fn append(&mut self, event: BudgetEvent) {
        self.events.push(event);
    }

    pub fn spent_by(&self, holder: &str) -> usize {
        self.events
            .iter()
            .filter_map(|event| match event {
                BudgetEvent::Spent {
                    holder: who,
                    tokens,
                    ..
                } if who == holder => Some(*tokens),
                _ => None,
            })
            .sum()
    }

    /// Privacy exposure spent so far. There is no counterpart that reduces it: deleting context
    /// after the fact does not un-disclose it (39.16 invariant 3).
    pub fn privacy_exposure(&self) -> u64 {
        self.privacy_exposure
    }
}

/// Allocates the run budget across obligations, roles and branches.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenBudgetController {
    total: usize,
    acquisition_threshold: f64,
    drift_tolerance: f64,
    leased: usize,
    leases: Vec<Lease>,
    ledger: BudgetLedger,
}

impl TokenBudgetController {
    pub fn new(total: usize) -> Self {
        TokenBudgetController {
            total,
            acquisition_threshold: 0.0,
            drift_tolerance: 0.25,
            leased: 0,
            leases: Vec::new(),
            ledger: BudgetLedger::default(),
        }
    }

    /// Minimum value per token for an exploratory request to be worth acquiring (39.07, step 7).
    pub fn with_acquisition_threshold(mut self, threshold: f64) -> Self {
        self.acquisition_threshold = threshold;
        self
    }

    /// Relative estimate-versus-actual divergence above which planning is no longer trustworthy.
    pub fn with_drift_tolerance(mut self, tolerance: f64) -> Self {
        self.drift_tolerance = tolerance;
        self
    }

    pub fn ledger(&self) -> &BudgetLedger {
        &self.ledger
    }

    pub fn total(&self) -> usize {
        self.total
    }

    /// Budget not yet handed to any lease.
    pub fn unleased(&self) -> usize {
        self.total.saturating_sub(self.leased)
    }

    /// Reserves the mandatory closure, then allocates what remains by value density.
    ///
    /// Mandatory requests are never traded against exploratory ones: if they do not fit, planning
    /// fails. A plan that quietly dropped an identity or temporal-firewall obligation to make room
    /// for a larger table would be exactly the outcome 39.16 forbids.
    pub fn plan(&mut self, requests: &[BudgetRequest]) -> Result<BudgetPlan, BudgetError> {
        let mandatory_tokens: usize = requests
            .iter()
            .filter(|request| request.mandatory)
            .map(|request| request.estimate.tokens)
            .sum();
        if mandatory_tokens > self.total {
            return Err(BudgetError::MandatoryClosureUnaffordable {
                required: mandatory_tokens,
                total: self.total,
            });
        }
        self.ledger.append(BudgetEvent::MandatoryReserved {
            tokens: mandatory_tokens,
        });

        let mut allocated: Vec<Allocation> = requests
            .iter()
            .filter(|request| request.mandatory)
            .map(|request| Allocation {
                obligation: request.obligation.clone(),
                tokens: request.estimate.tokens,
                method: request.estimate.method.clone(),
                mandatory: true,
                value_density: request.value_density(),
            })
            .collect();
        allocated.sort_by(|a, b| a.obligation.cmp(&b.obligation));

        let mut exploratory: Vec<&BudgetRequest> = requests
            .iter()
            .filter(|request| !request.mandatory)
            .collect();
        exploratory.sort_by(|a, b| {
            b.value_density()
                .partial_cmp(&a.value_density())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.obligation.cmp(&b.obligation))
        });

        let mut remaining = self.total - mandatory_tokens;
        let mut rejected = Vec::new();
        for request in exploratory {
            if request.value_density() < self.acquisition_threshold {
                rejected.push(Rejection {
                    obligation: request.obligation.clone(),
                    tokens: request.estimate.tokens,
                    method: request.estimate.method.clone(),
                    reason: RejectionReason::BelowAcquisitionThreshold,
                });
                continue;
            }
            if request.estimate.tokens <= remaining {
                remaining -= request.estimate.tokens;
                allocated.push(Allocation {
                    obligation: request.obligation.clone(),
                    tokens: request.estimate.tokens,
                    method: request.estimate.method.clone(),
                    mandatory: false,
                    value_density: request.value_density(),
                });
            } else {
                rejected.push(Rejection {
                    obligation: request.obligation.clone(),
                    tokens: request.estimate.tokens,
                    method: request.estimate.method.clone(),
                    reason: RejectionReason::BudgetExhausted,
                });
            }
        }

        for entry in &allocated {
            self.ledger.append(BudgetEvent::Allocated {
                obligation: entry.obligation.clone(),
                tokens: entry.tokens,
                method: entry.method.label(),
            });
        }
        for entry in &rejected {
            self.ledger.append(BudgetEvent::Rejected {
                obligation: entry.obligation.clone(),
                tokens: entry.tokens,
                reason: entry.reason,
            });
        }

        let stop = if rejected
            .iter()
            .any(|entry| entry.reason == RejectionReason::BudgetExhausted)
        {
            StopDecision::BudgetExhausted
        } else {
            StopDecision::WithinBudget
        };

        let estimates: Vec<TokenEstimate> = allocated
            .iter()
            .map(|entry| TokenEstimate {
                tokens: entry.tokens,
                method: entry.method.clone(),
            })
            .collect();

        Ok(BudgetPlan {
            total_tokens: self.total,
            mandatory_reserved: mandatory_tokens,
            allocated_estimate: TokenEstimate::sum(estimates.iter()),
            allocated,
            rejected,
            stop,
        })
    }

    /// Leases budget to a role or branch out of the unleased remainder.
    pub fn lease(
        &mut self,
        holder: impl Into<String>,
        tokens: usize,
    ) -> Result<LeaseId, BudgetError> {
        let holder = holder.into();
        let available = self.unleased();
        if tokens > available {
            return Err(BudgetError::LeaseExceedsHolding {
                holder,
                requested: tokens,
                available,
            });
        }
        self.leased += tokens;
        let id = LeaseId(self.leases.len());
        self.leases.push(Lease {
            id,
            holder: holder.clone(),
            parent: None,
            granted: tokens,
            spent: 0,
            subleased: 0,
            rejected: false,
        });
        self.ledger.append(BudgetEvent::Leased {
            holder,
            tokens,
            parent: None,
        });
        Ok(id)
    }

    /// Carves a child lease out of a parent.
    ///
    /// The parent's remainder shrinks by exactly what the child receives, which is the mechanism
    /// behind 39.16 invariant 1: a delegated branch cannot end up holding more than its delegator.
    pub fn sublease(
        &mut self,
        parent: LeaseId,
        holder: impl Into<String>,
        tokens: usize,
    ) -> Result<LeaseId, BudgetError> {
        let holder = holder.into();
        let parent_lease = self.lease_mut(parent)?;
        if parent_lease.rejected {
            return Err(BudgetError::LeaseRejected(parent_lease.holder.clone()));
        }
        let available = parent_lease.remaining();
        if tokens > available {
            return Err(BudgetError::LeaseExceedsHolding {
                holder,
                requested: tokens,
                available,
            });
        }
        parent_lease.subleased += tokens;
        let parent_holder = parent_lease.holder.clone();

        let id = LeaseId(self.leases.len());
        self.leases.push(Lease {
            id,
            holder: holder.clone(),
            parent: Some(parent),
            granted: tokens,
            spent: 0,
            subleased: 0,
            rejected: false,
        });
        self.ledger.append(BudgetEvent::Leased {
            holder,
            tokens,
            parent: Some(parent_holder),
        });
        Ok(id)
    }

    /// Records actual spend against a lease.
    pub fn spend(&mut self, lease: LeaseId, estimate: &TokenEstimate) -> Result<(), BudgetError> {
        let method = estimate.method.label();
        let tokens = estimate.tokens;
        let held = self.lease_mut(lease)?;
        if held.rejected {
            return Err(BudgetError::LeaseRejected(held.holder.clone()));
        }
        if tokens > held.remaining() {
            return Err(BudgetError::LeaseExceedsHolding {
                holder: held.holder.clone(),
                requested: tokens,
                available: held.remaining(),
            });
        }
        held.spent += tokens;
        let holder = held.holder.clone();
        self.ledger.append(BudgetEvent::Spent {
            holder,
            tokens,
            method,
        });
        Ok(())
    }

    /// Closes a branch without refunding it.
    ///
    /// 39.16 invariant 4: the tokens a rejected branch consumed were still consumed. The lease is
    /// marked rejected so it cannot spend again, and its spend stays in the ledger.
    pub fn reject_branch(
        &mut self,
        lease: LeaseId,
        reason: impl Into<String>,
    ) -> Result<(), BudgetError> {
        let reason = reason.into();
        let held = self.lease_mut(lease)?;
        held.rejected = true;
        let holder = held.holder.clone();
        let consumed = held.spent;
        self.ledger.append(BudgetEvent::BranchRejected {
            holder,
            reason,
            consumed,
        });
        Ok(())
    }

    /// Compares an estimate against measured spend and decides whether to keep planning on it.
    pub fn reconcile(
        &mut self,
        obligation: impl Into<String>,
        estimate: &TokenEstimate,
        actual: usize,
    ) -> StopDecision {
        let estimated = estimate.tokens;
        let drift = (actual as f64 - estimated as f64) / estimated.max(1) as f64;
        self.ledger.append(BudgetEvent::Reconciled {
            obligation: obligation.into(),
            estimated,
            actual,
            method: estimate.method.label(),
            drift,
        });
        if drift.abs() > self.drift_tolerance {
            StopDecision::EstimatorDriftEscalation
        } else {
            StopDecision::WithinBudget
        }
    }

    /// Records privacy exposure. There is no inverse.
    pub fn record_privacy_exposure(&mut self, units: u64, detail: impl Into<String>) {
        self.ledger.privacy_exposure += units;
        self.ledger.append(BudgetEvent::PrivacyExposed {
            units,
            detail: detail.into(),
        });
    }

    pub fn lease_state(&self, lease: LeaseId) -> Option<&Lease> {
        self.leases.get(lease.0)
    }

    pub fn leases(&self) -> &[Lease] {
        &self.leases
    }

    fn lease_mut(&mut self, lease: LeaseId) -> Result<&mut Lease, BudgetError> {
        self.leases
            .get_mut(lease.0)
            .ok_or(BudgetError::UnknownLease(lease.0))
    }
}
