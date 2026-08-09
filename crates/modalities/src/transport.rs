//! Moving a measurement between modalities, with the loss written down.
//!
//! Bulk transcriptomics measures a population average, single-cell measures a distribution. A bulk
//! value is recoverable from single-cell data and the reverse is not, and this module is where
//! that asymmetry lives. Aggregating a distribution to its summary is a function; recovering the
//! distribution from the summary is not, and [`ModalityTransport::invert`] refuses to pretend
//! otherwise.
//!
//! # Three operations, not one
//!
//! [`TransportKind`] follows `bioprism-scope`'s insistence in 43.05 that restriction, transport,
//! aggregation and extension are distinct operations that must not collapse into a single "join".
//! Here the distinction is about resolution:
//!
//! * [`TransportKind::Aggregation`] **removes** an axis. It is exact — the population mean of a set
//!   of per-cell values is that mean, with no assumption — and it is not invertible.
//! * [`TransportKind::Deconvolution`] **creates** an axis from a declared reference. It is an
//!   estimate, and it *is* invertible, because recomposing the estimated components returns the
//!   input it was constrained by.
//! * [`TransportKind::Imputation`] fills gaps at an axis that already exists. It is an estimate and
//!   is not invertible, because the output does not distinguish the filled entries from the
//!   observed ones.
//!
//! The counter-intuitive line is the middle one: the exact operation is the non-invertible one.
//! That is the whole asymmetry, and it is why "we deconvolved and then re-aggregated, and got the
//! bulk values back" is not evidence that the deconvolution was right.
//!
//! # The ledger is derived, not supplied
//!
//! [`ModalityTransport::aggregating`] builds its own [`LossLedger`] from the axes it collapses, so
//! a caller cannot forget to declare that the distribution is gone. Resolution-creating transports
//! must additionally name their basis — the reference panel, the model — because 28.03 lists
//! deconvolution among the decisions whose answer depends on that input, and an unnamed input is
//! an unauditable result.
//!
//! # Not implemented
//!
//! Nothing here computes anything. There is no mean, no matrix, no reference panel and no
//! estimator: [`ModalityTransport::apply`] rewrites a *descriptor's* resolution declarations, so
//! that a downstream [`crate::support::supports`] refuses claims the transport is not entitled to.
//! The numbers come from whichever tool actually did the work.

use crate::descriptor::{Modality, ModalityDescriptor, Resolution, ResolutionStatus};
use crate::error::TransportRefusal;
use bioprism_scope::{AggregationOperator, LossLedger};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What kind of resolution change a transport performs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportKind {
    /// Collapses an axis by summarising over it.
    Aggregation { operator: AggregationOperator },
    /// Creates an axis by apportioning a summary across components of a declared reference.
    Deconvolution {
        /// The reference panel or signature matrix the apportionment was made against.
        reference: String,
        /// The operator that recomposes the components back into the input.
        recomposition: AggregationOperator,
    },
    /// Fills missing entries at an axis that already exists.
    Imputation { model: String },
}

impl TransportKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportKind::Aggregation { .. } => "aggregation",
            TransportKind::Deconvolution { .. } => "deconvolution",
            TransportKind::Imputation { .. } => "imputation",
        }
    }

    /// Whether the operation introduces assumptions beyond the input values.
    ///
    /// Aggregation does not: the mean of some numbers is those numbers' mean. Deconvolution and
    /// imputation do, and [`Fidelity::Estimated`] carries what the assumption was.
    pub fn fidelity(&self) -> Fidelity {
        match self {
            TransportKind::Aggregation { .. } => Fidelity::Exact,
            TransportKind::Deconvolution { reference, .. } => Fidelity::Estimated {
                conditioned_on: reference.clone(),
            },
            TransportKind::Imputation { model } => Fidelity::Estimated {
                conditioned_on: model.clone(),
            },
        }
    }
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a transported value follows from its input alone.
///
/// Named separately from [`bioprism_standards::Exactness`], which answers a different question:
/// that type says whether a unit conversion factor is defined or conventional, and its
/// `Conventional` variant carries a convention rather than a scientific assumption. A conversion
/// from millimetres to centimetres is exact whoever performs it; a deconvolution is conditioned on
/// a reference panel that someone chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fidelity", rename_all = "snake_case")]
pub enum Fidelity {
    Exact,
    Estimated { conditioned_on: String },
}

impl Fidelity {
    pub fn is_exact(&self) -> bool {
        matches!(self, Fidelity::Exact)
    }
}

/// A declared move of a measurement from one modality's resolution to another's.
///
/// Constructed only through [`ModalityTransport::aggregating`],
/// [`ModalityTransport::deconvolving`] and [`ModalityTransport::imputing`], each of which builds
/// the loss ledger itself. There is no literal constructor, so there is no transport in existence
/// whose ledger says it lost nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityTransport {
    pub from: Modality,
    pub to: Modality,
    pub kind: TransportKind,
    /// The axis the transport removes or creates.
    pub axis: Resolution,
    loss: LossLedger,
}

impl ModalityTransport {
    /// Collapses `axis`, summarising with `operator`.
    ///
    /// Refused when the source does not resolve the axis being collapsed: aggregating over cells
    /// that were never resolved is not an aggregation, it is a relabelling.
    pub fn aggregating(
        source: &ModalityDescriptor,
        to: Modality,
        axis: Resolution,
        operator: AggregationOperator,
    ) -> Result<Self, TransportRefusal> {
        if !source.resolution(axis).is_resolved() {
            return Err(TransportRefusal::SourceLacksAxis {
                from: source.modality,
                kind: "aggregation".to_string(),
                axis,
            });
        }
        let loss = LossLedger::default()
            .discarding(format!(
                "the distribution over {axis}; only its {operator:?} survives"
            ))
            .discarding(format!(
                "the ability to attribute any part of the result to a single {axis}"
            ));
        Ok(ModalityTransport {
            from: source.modality,
            to,
            kind: TransportKind::Aggregation { operator },
            axis,
            loss,
        })
    }

    /// Creates `axis` by apportioning against a named reference.
    ///
    /// Refused when the reference is unnamed, and refused when the source already resolves the
    /// axis — deconvolving an axis you measured is not a transport, and declaring it as one would
    /// mark measured values as estimates.
    pub fn deconvolving(
        source: &ModalityDescriptor,
        to: Modality,
        axis: Resolution,
        reference: impl Into<String>,
        recomposition: AggregationOperator,
    ) -> Result<Self, TransportRefusal> {
        let reference = reference.into();
        if reference.trim().is_empty() {
            return Err(TransportRefusal::UnstatedBasis {
                from: source.modality,
                to,
                kind: "deconvolution".to_string(),
                axis,
            });
        }
        if source.resolution(axis).is_resolved() {
            return Err(TransportRefusal::AggregationWouldAddResolution {
                from: source.modality,
                to,
                axis,
            });
        }
        let loss = LossLedger::default()
            .adding_uncertainty(format!(
                "{axis} was apportioned against {reference}, not observed"
            ))
            .conditioned_on(format!(
                "the components present in the specimen are those in {reference}"
            ));
        Ok(ModalityTransport {
            from: source.modality,
            to,
            kind: TransportKind::Deconvolution {
                reference,
                recomposition,
            },
            axis,
            loss,
        })
    }

    /// Fills gaps at an axis the source already resolves.
    pub fn imputing(
        source: &ModalityDescriptor,
        to: Modality,
        axis: Resolution,
        model: impl Into<String>,
    ) -> Result<Self, TransportRefusal> {
        let model = model.into();
        if model.trim().is_empty() {
            return Err(TransportRefusal::UnstatedBasis {
                from: source.modality,
                to,
                kind: "imputation".to_string(),
                axis,
            });
        }
        let loss = LossLedger::default()
            .adding_uncertainty(format!("entries at {axis} were filled by {model}"))
            .discarding(
                "which entries were observed and which were filled, unless a mask is kept \
                 alongside"
                    .to_string(),
            );
        Ok(ModalityTransport {
            from: source.modality,
            to,
            kind: TransportKind::Imputation { model },
            axis,
            loss,
        })
    }

    pub fn loss(&self) -> &LossLedger {
        &self.loss
    }

    pub fn fidelity(&self) -> Fidelity {
        self.kind.fidelity()
    }

    /// Audits the transport the way [`bioprism_scope::ScopeMapping::check`] audits a mapping.
    ///
    /// Always `Ok` for transports built here, since the constructors write the ledger. It exists
    /// for transports that arrived by deserialisation, where an empty ledger is representable and
    /// is exactly the defect 43.05 asks to be surfaced.
    pub fn check(&self) -> Result<(), TransportRefusal> {
        if self.loss.is_empty() {
            return Err(TransportRefusal::UndeclaredLoss {
                from: self.from,
                to: self.to,
            });
        }
        Ok(())
    }

    /// The transport that undoes this one, when one exists.
    ///
    /// Aggregation has no inverse: the ledger says the distribution was discarded, and nothing
    /// recovers it. Imputation has no inverse either, for a smaller reason — the output does not
    /// say which entries were filled, so there is nothing to remove. Deconvolution does: its
    /// components were constrained to recompose into the input, so recomposing them is a genuine
    /// left inverse.
    ///
    /// The direction of the asymmetry is worth restating because it inverts the intuition. The
    /// *exact* operation is the one that cannot be undone, and the *estimated* one is the one that
    /// can — which is why a successful round trip through deconvolution and back says nothing
    /// whatever about whether the deconvolution was correct.
    pub fn invert(&self) -> Result<ModalityTransport, TransportRefusal> {
        match &self.kind {
            TransportKind::Aggregation { operator } => Err(TransportRefusal::NotInvertible {
                from: self.from,
                to: self.to,
                kind: "aggregation".to_string(),
                because: format!(
                    "the {operator:?} over {} is all that survives; the distribution it \
                     summarised is not recoverable from it",
                    self.axis
                ),
            }),
            TransportKind::Imputation { model } => Err(TransportRefusal::NotInvertible {
                from: self.from,
                to: self.to,
                kind: "imputation".to_string(),
                because: format!(
                    "{model} left no record of which entries at {} it filled, so there is nothing \
                     to remove",
                    self.axis
                ),
            }),
            TransportKind::Deconvolution { recomposition, .. } => {
                let loss = LossLedger::default()
                    .discarding(format!(
                        "the estimated distribution over {}; recomposition returns the summary it \
                         was constrained by, which is not evidence that the estimate was right",
                        self.axis
                    ))
                    .discarding(format!(
                        "the ability to attribute any part of the result to a single {}",
                        self.axis
                    ));
                Ok(ModalityTransport {
                    from: self.to,
                    to: self.from,
                    kind: TransportKind::Aggregation {
                        operator: *recomposition,
                    },
                    axis: self.axis,
                    loss,
                })
            }
        }
    }

    pub fn is_invertible(&self) -> bool {
        self.invert().is_ok()
    }

    /// The descriptor a value has after the transport.
    ///
    /// This is the payoff. An aggregated descriptor declares the collapsed axis
    /// [`ResolutionStatus::Unresolved`], so a claim about it is refused for want of resolution. A
    /// deconvolved descriptor declares it [`ResolutionStatus::Imputed`], which
    /// [`crate::support::supports`] admits for a composition claim and refuses for a cell-intrinsic
    /// one — the exact distinction 28.03 draws between deconvolution as a benchmark decision and
    /// composition as a characteristic failure.
    pub fn apply(&self, source: &ModalityDescriptor) -> Result<ModalityDescriptor, TransportRefusal> {
        self.check()?;
        let status = match &self.kind {
            TransportKind::Aggregation { .. } => ResolutionStatus::Unresolved,
            TransportKind::Deconvolution { reference, .. } => ResolutionStatus::Imputed {
                source: self.from,
                by: format!("deconvolution against {reference}"),
            },
            TransportKind::Imputation { model } => ResolutionStatus::Imputed {
                source: self.from,
                by: format!("imputation by {model}"),
            },
        };
        Ok(source.clone().with_status(self.axis, status))
    }

    /// Renders the transport as a `bioprism-scope` mapping over the modality dimension.
    ///
    /// Keeps this crate's vocabulary inside the 43.05 taxonomy rather than beside it: a
    /// cross-modality move is a [`bioprism_scope::MappingKind`] over a scope key that binds
    /// `assay` and `resolution`, and `bioprism-scope`'s own audit applies to it unchanged.
    pub fn to_scope_mapping(&self) -> bioprism_scope::ScopeMapping {
        let from = bioprism_scope::ScopeKey::new()
            .exact("assay", self.from.as_str())
            .exact("resolution", self.axis.as_str());
        let to = bioprism_scope::ScopeKey::new()
            .exact("assay", self.to.as_str())
            .exact("resolution", self.axis.as_str());
        let kind = match &self.kind {
            TransportKind::Aggregation { operator } => bioprism_scope::MappingKind::Aggregation {
                operator: *operator,
            },
            TransportKind::Deconvolution { reference, .. } => {
                bioprism_scope::MappingKind::Transport {
                    justification: format!("deconvolution against {reference}"),
                }
            }
            TransportKind::Imputation { model } => bioprism_scope::MappingKind::Transport {
                justification: format!("imputation by {model}"),
            },
        };
        bioprism_scope::ScopeMapping {
            from,
            to,
            kind,
            loss: self.loss.clone(),
        }
    }
}

/// A chain of transports and the descriptor it produces.
///
/// Chains are where a loss ledger earns its keep: aggregate to bulk, deconvolve back, and the
/// result's cell axis is [`ResolutionStatus::Imputed`] rather than
/// [`ResolutionStatus::Resolved`]. Nothing in the arithmetic notices that the round trip happened;
/// the descriptor does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportChain {
    steps: Vec<ModalityTransport>,
}

impl TransportChain {
    pub fn new() -> Self {
        TransportChain { steps: Vec::new() }
    }

    pub fn then(mut self, step: ModalityTransport) -> Self {
        self.steps.push(step);
        self
    }

    pub fn steps(&self) -> &[ModalityTransport] {
        &self.steps
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The combined ledger, in application order.
    pub fn loss(&self) -> LossLedger {
        let mut combined = LossLedger::default();
        for step in &self.steps {
            for entry in &step.loss.discarded {
                combined = combined.discarding(entry.clone());
            }
            for entry in &step.loss.uncertainty_added {
                combined = combined.adding_uncertainty(entry.clone());
            }
            for entry in &step.loss.policy_conditions {
                combined = combined.conditioned_on(entry.clone());
            }
        }
        combined
    }

    /// True only when every step is exact.
    ///
    /// One estimated step makes the chain estimated, however many exact ones surround it. The
    /// stricter-of-the-two rule `bioprism_standards::Exactness` uses for composed units.
    pub fn fidelity(&self) -> Fidelity {
        for step in &self.steps {
            if let Fidelity::Estimated { conditioned_on } = step.fidelity() {
                return Fidelity::Estimated { conditioned_on };
            }
        }
        Fidelity::Exact
    }

    pub fn apply(&self, source: &ModalityDescriptor) -> Result<ModalityDescriptor, TransportRefusal> {
        let mut current = source.clone();
        for step in &self.steps {
            current = step.apply(&current)?;
        }
        Ok(current)
    }

    /// True when the chain returns a descriptor to the resolutions it started with.
    ///
    /// Deliberately *not* a claim that the values are recovered. A round trip through
    /// deconvolution restores nothing: the axis comes back as
    /// [`ResolutionStatus::Imputed`], never as [`ResolutionStatus::Resolved`], so this returns
    /// false for exactly the chain a caller most wants it to return true for.
    pub fn restores(&self, source: &ModalityDescriptor) -> bool {
        match self.apply(source) {
            Ok(result) => Resolution::ALL
                .into_iter()
                .all(|axis| result.resolution(axis) == source.resolution(axis)),
            Err(_) => false,
        }
    }
}

impl Default for TransportChain {
    fn default() -> Self {
        TransportChain::new()
    }
}
