//! Adaptive evaluation: small panels that keep their confidence honest.
//!
//! Implements blueprint section 08 (08.01 scheduler, 08.02 capability posterior and item model,
//! 08.03 information-gain and decision-value selection, 08.04 coverage/fairness/safety
//! constraints, 08.05 sequential stopping, 08.07 batching, 08.08 reproducible selection) together
//! with 43.15's information-directed acquisition ratio, and targets **Gate 4** of the critical
//! path: a 500–2,000-instance adaptive panel that "estimates the same meaningful capability
//! differences as a much larger exhaustive run while preserving coverage constraints and
//! confidence guarantees".
//!
//! # The problem this crate exists for
//!
//! Gate 3 produces ten thousand validated instances from at most a hundred audited parents, and
//! `crates/mutation` already refuses to call that ten thousand benchmarks. Gate 4 then samples
//! from them — and the sampling is the easy half. The hard half is that **the instances are not
//! independent**. They descend from a small number of parents, they inherit those parents'
//! structure and difficulty, and an architecture that handles one parent well handles most of its
//! descendants well.
//!
//! Count them as independent and the arithmetic is brutal. A thousand instances from ten parents,
//! strongly parent-driven, carry roughly the information of ten trials. The naive 95% interval is
//! about three points wide; the honest one is about fifty. Every downstream claim — this
//! architecture is better, this release is safe, this regression is real — inherits that factor.
//! It is the failure mode blueprint 08 lists in every module ("Generated instances inflate scale
//! while adding little independent diagnostic information") and it is silent: nothing crashes,
//! no number looks odd, the panel simply reports a precision it never had.
//!
//! So this crate reports **both** intervals, always, and the ratio between them. See
//! [`estimate::CapabilityEstimate::inflation`].
//!
//! # The five pieces
//!
//! | Module | Blueprint | What it does |
//! |---|---|---|
//! | [`beta`] | 08.02 | Beta-Bernoulli posterior; log-gamma, incomplete beta and quantiles written out longhand |
//! | [`cluster`] | 08.02 | Intraclass correlation, design effect, effective sample size, cluster bootstrap |
//! | [`select`] | 08.03, 43.15 | Expected posterior-variance reduction per unit cost, discounted by parent over-sampling |
//! | [`coverage`] | 08.04 | Hard floors per capability, per parent and per sentinel; refusal to report below them |
//! | [`stopping`] | 08.05 | Width and threshold targets, futility, and the effective sample size actually achieved |
//!
//! [`panel::AdaptivePanel`] wires them together; [`ledger::TrialLedger`] is where evidence enters
//! and where dispatch is kept from being mistaken for it.
//!
//! # Honest limits
//!
//! Stated once here and again at each site.
//!
//! * **One level of clustering.** Parent only. Repeated trials of the same instance and
//!   mutation-family sub-clusters within a parent are not modelled — so the ledger *refuses* a
//!   second scored trial on an instance rather than applying a one-level model to two-level data.
//! * **No item model.** Difficulty and discrimination (08.02's item-response layer) are absent;
//!   the point estimate is a pass rate on the instances actually run, not a difficulty-adjusted
//!   ability. Two panels with different difficulty mixes are not directly comparable.
//! * **Not anytime-valid.** Stopping checks a fixed credible interval repeatedly. A confidence
//!   sequence is the right instrument and is not implemented; see [`stopping`].
//! * **No exploration reserve.** Selection is greedy inside the coverage gate. 08.03 asks for
//!   reserved probability mass against posterior entrenchment; the coverage floors are the only
//!   defence here.
//! * **Unpaired comparisons.** [`panel::Comparison`] assumes independent posteriors and therefore
//!   overstates separation when two architectures ran the same instances.
//! * **No scheduler evaluation.** 08.08's deployment gate — adaptive scheduling may not control a
//!   release until it beats a fixed stratified panel on decision accuracy and safety — is a
//!   program to run, not a function; nothing here asserts it has been passed.

pub mod beta;
pub mod cluster;
pub mod coverage;
pub mod error;
pub mod estimate;
pub mod id;
pub mod ledger;
pub mod panel;
pub mod rng;
pub mod select;
pub mod stopping;

pub use beta::{
    ln_beta, ln_gamma, probability_first_exceeds_second, regularized_incomplete_beta, BetaPosterior,
    BetaPrior, CredibleInterval,
};
pub use cluster::{
    cluster_bootstrap_interval, marginal_independent_weight, BootstrapConfig, Cluster,
    ClusterSummary, Icc,
};
pub use coverage::{CoveragePolicy, CoverageStatus, Shortfall};
pub use error::AdaptiveError;
pub use estimate::{
    naive_probability_of_superiority, probability_of_superiority, CapabilityEstimate,
};
pub use id::{CapabilityId, InstanceId, ParentId};
pub use ledger::{Outcome, Trial, TrialLedger};
pub use panel::{AdaptivePanel, CapabilityAudit, Comparison, PanelAudit, PanelConfig};
pub use select::{
    select_batch, select_next, Candidate, CoverageGate, IccSource, ScoredCandidate,
    SelectionConfig, SelectionRecord,
};
pub use stopping::{
    best_case_effective_trials, Question, StopReason, StoppingRule, StoppingVerdict,
};
