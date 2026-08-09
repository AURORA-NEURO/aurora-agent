//! Semi-synthetic world construction.
//!
//! Blueprint 35.03: "embed controlled biological or measurement states into realistic observed
//! backgrounds to obtain stronger oracles." The oracle is stronger because the latent is known by
//! construction rather than inferred — which is exactly the property that makes semi-synthetic
//! panels fail silently when they fail.
//!
//! ## The failure this module exists to catch
//!
//! If every background that received an insertion came from one batch and every background that
//! did not came from another, a system can score perfectly by reading the batch label and never
//! looking at the biology. The panel then measures batch bookkeeping, and it does so while
//! reporting a high latent-recovery number — the metric 35.03 itself lists first.
//!
//! [`ConfoundReport`] cross-tabulates batch against insertion status and answers one parameter-free
//! question: **does the batch determine the label?** If every batch is pure and both labels occur,
//! the answer is yes and [`ConfoundReport::is_usable_as_an_oracle`] is false regardless of how good
//! the recovery figure looks. No threshold is involved, so there is no constant to invent. The
//! imbalance figure beside it is *reported* rather than compared against a cut-off, because this
//! crate has no basis for choosing one.
//!
//! Assignment is done by [`assign_insertions`], seeded, using `bioprism_worldgen::rng::SplitMix64`.
//! Randomising the assignment does **not** make the confound impossible — at panel sizes where
//! semi-synthetic work actually happens, some seeds produce a batch-determined panel outright. The
//! crate's test suite searches the seed space and names such a seed, so the claim is a
//! reproducible finding rather than a warning.
//!
//! ## Transfer is never assumed
//!
//! 35.03 lists "transfer to observed data" as a metric and "transfer validation" as a required
//! component. [`TransferClaim`] has two variants and no `Default`: a claim is either validated
//! against a named observed cohort or it is `Unvalidated` with a reason. A semi-synthetic result
//! carries the unvalidated state by construction until someone does the observed-data work.
//!
//! ## What this module does not do
//!
//! It inserts nothing. There is no image compositor, no read simulator, no spike-in model, and no
//! assay forward model — the latter is [`crate::twin`]'s, and even there it is caller-parameterised.
//! What is here is the *bookkeeping* of an insertion campaign and the predicates over it. The
//! realism audit that 35.03 also asks for is a human judgement and is absent, not stubbed.
//!
//! No effect size, detection rate, or batch-effect magnitude is known to this crate. Every such
//! quantity is a field a caller fills in.

use crate::error::InsertionError;
use bioprism_scale::audit::ReleaseAudit;
use bioprism_scale::QualityGate;
use bioprism_scope::{Interval, Timestamp};
use bioprism_worldgen::rng::SplitMix64;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A real artifact used as the background a latent state is embedded into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Background {
    pub id: String,
    /// The processing batch, site, or run this background came from. The confounder that matters.
    pub batch: String,
    /// When the background was observed. An insertion asserting a state outside this window claims
    /// something the background cannot support.
    pub observed: Interval,
}

impl Background {
    pub fn new(id: impl Into<String>, batch: impl Into<String>, observed: Interval) -> Self {
        Background {
            id: id.into(),
            batch: batch.into(),
            observed,
        }
    }
}

/// One embedded latent state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Insertion {
    pub background: String,
    pub latent: String,
    /// When the inserted state is asserted to begin.
    pub onset: Timestamp,
    /// The magnitude the caller says was inserted, in the caller's own units.
    ///
    /// This crate never supplies, defaults, or interprets it. It travels so that a recovery figure
    /// can be read next to the size of the thing that was supposed to be recovered — a recall of
    /// 1.0 against an enormous insertion says nothing about a realistic one.
    pub stated_effect_size: f64,
}

impl Insertion {
    pub fn new(
        background: impl Into<String>,
        latent: impl Into<String>,
        onset: Timestamp,
        stated_effect_size: f64,
    ) -> Self {
        Insertion {
            background: background.into(),
            latent: latent.into(),
            onset,
            stated_effect_size,
        }
    }
}

/// A set of backgrounds and the insertions made into them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Panel {
    pub backgrounds: Vec<Background>,
    pub insertions: Vec<Insertion>,
}

impl Panel {
    pub fn new(backgrounds: Vec<Background>, insertions: Vec<Insertion>) -> Self {
        Panel {
            backgrounds,
            insertions,
        }
    }

    /// Identity and temporal consistency, which are structural rather than statistical.
    ///
    /// A background appearing twice, an insertion into a background that is not in the panel, two
    /// insertions into one background, and an onset outside the background's own observation window
    /// are all errors rather than findings: every statistic below would be computed over a panel
    /// whose meaning nobody can state.
    pub fn check_consistency(&self) -> Result<(), InsertionError> {
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for background in &self.backgrounds {
            if !ids.insert(background.id.as_str()) {
                return Err(InsertionError::DuplicateBackground(background.id.clone()));
            }
        }
        let by_id: BTreeMap<&str, &Background> = self
            .backgrounds
            .iter()
            .map(|background| (background.id.as_str(), background))
            .collect();

        let mut inserted: BTreeSet<&str> = BTreeSet::new();
        for insertion in &self.insertions {
            let background = by_id
                .get(insertion.background.as_str())
                .copied()
                .ok_or_else(|| InsertionError::UnknownBackground(insertion.background.clone()))?;
            if !inserted.insert(insertion.background.as_str()) {
                return Err(InsertionError::DoubleInsertion(
                    insertion.background.clone(),
                ));
            }
            if !background.observed.contains(insertion.onset) {
                return Err(InsertionError::OnsetOutsideBackground {
                    background: background.id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Background ids carrying an insertion, in id order.
    pub fn inserted_ids(&self) -> BTreeSet<&str> {
        self.insertions
            .iter()
            .map(|insertion| insertion.background.as_str())
            .collect()
    }
}

/// One batch's contribution to the cross-tabulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchCell {
    pub backgrounds: usize,
    pub inserted: usize,
}

impl BatchCell {
    /// Whether every background in this batch shares one insertion status.
    pub fn is_pure(&self) -> bool {
        self.inserted == 0 || self.inserted == self.backgrounds
    }
}

/// Whether the batch label alone can reproduce the insertion label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfoundReport {
    pub by_batch: BTreeMap<String, BatchCell>,
    pub backgrounds: usize,
    pub inserted: usize,
    /// Batches in which every background shares one insertion status.
    pub pure_batches: Vec<String>,
    /// Every batch is pure and both labels occur: the batch label is a perfect classifier.
    pub batch_determines_label: bool,
    /// Largest absolute gap between a batch's inserted share and the panel's. Reported without a
    /// threshold — this crate has no basis for choosing one, and an invented cut-off would be the
    /// number everyone quotes.
    pub worst_batch_imbalance: f64,
}

impl ConfoundReport {
    pub fn measure(panel: &Panel) -> Result<Self, InsertionError> {
        panel.check_consistency()?;
        let inserted_ids = panel.inserted_ids();
        let mut by_batch: BTreeMap<String, BatchCell> = BTreeMap::new();
        for background in &panel.backgrounds {
            let cell = by_batch
                .entry(background.batch.clone())
                .or_insert(BatchCell {
                    backgrounds: 0,
                    inserted: 0,
                });
            cell.backgrounds += 1;
            if inserted_ids.contains(background.id.as_str()) {
                cell.inserted += 1;
            }
        }

        let backgrounds = panel.backgrounds.len();
        let inserted = inserted_ids.len();
        let overall_share = if backgrounds == 0 {
            0.0
        } else {
            inserted as f64 / backgrounds as f64
        };
        let pure_batches: Vec<String> = by_batch
            .iter()
            .filter(|(_, cell)| cell.is_pure())
            .map(|(batch, _)| batch.clone())
            .collect();
        let both_labels_occur = inserted > 0 && inserted < backgrounds;
        let worst_batch_imbalance = by_batch
            .values()
            .map(|cell| {
                let share = cell.inserted as f64 / cell.backgrounds as f64;
                (share - overall_share).abs()
            })
            .fold(0.0f64, f64::max);

        Ok(ConfoundReport {
            batch_determines_label: both_labels_occur
                && pure_batches.len() == by_batch.len()
                && !by_batch.is_empty(),
            by_batch,
            backgrounds,
            inserted,
            pure_batches,
            worst_batch_imbalance,
        })
    }

    /// Whether both insertion labels occur at all. A panel with no insertions, or with nothing but,
    /// has no discrimination to measure.
    pub fn is_mixed(&self) -> bool {
        self.inserted > 0 && self.inserted < self.backgrounds
    }

    /// The one question this report answers.
    ///
    /// A confounded panel is not a weaker oracle, it is a different one: it measures whether the
    /// system can read a batch label.
    pub fn is_usable_as_an_oracle(&self) -> bool {
        self.is_mixed() && !self.batch_determines_label
    }

    pub fn headline(&self) -> String {
        if !self.is_mixed() {
            return format!(
                "{} backgrounds, {} inserted: one label only, so there is no discrimination to \
                 measure",
                self.backgrounds, self.inserted
            );
        }
        if self.batch_determines_label {
            format!(
                "every one of {} batches is pure across {} backgrounds: the batch label alone \
                 reproduces the insertion label, so latent recovery on this panel measures batch \
                 bookkeeping",
                self.by_batch.len(),
                self.backgrounds
            )
        } else {
            format!(
                "{} backgrounds over {} batches, {} inserted; {} batch(es) pure, worst batch \
                 imbalance {:.3}",
                self.backgrounds,
                self.by_batch.len(),
                self.inserted,
                self.pure_batches.len(),
                self.worst_batch_imbalance
            )
        }
    }

    /// Records the non-LLM-oracle release gate, which a valid insertion campaign does supply.
    ///
    /// A semi-synthetic insertion is a non-LLM oracle by construction: the truth is what was put
    /// in. The gate passes only when the panel is not batch-determined, because a confounded panel
    /// is an oracle for the wrong thing.
    pub fn contribute_to(&self, audit: &mut ReleaseAudit) {
        audit.record(
            QualityGate::NonLlmOracle,
            self.is_usable_as_an_oracle(),
            self.headline(),
        );
    }
}

/// How well a detector recovered what was inserted.
///
/// Truth here is not estimated: it is the insertion list. That is the entire reason semi-synthetic
/// panels exist, and the reason a confounded panel is so dangerous — the recovery figure stays
/// exactly computable while ceasing to mean anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatentRecovery {
    pub called: usize,
    pub inserted: usize,
    pub true_positives: usize,
    pub false_positives: usize,
    pub recall: f64,
    pub precision: f64,
    /// Carried so a recovery figure is never read apart from the confound check that licenses it.
    pub panel_usable_as_an_oracle: bool,
}

impl LatentRecovery {
    /// `calls` are background ids the detector flagged as carrying the latent.
    pub fn measure(panel: &Panel, calls: &[String]) -> Result<Self, InsertionError> {
        let confound = ConfoundReport::measure(panel)?;
        let known: BTreeSet<&str> = panel
            .backgrounds
            .iter()
            .map(|background| background.id.as_str())
            .collect();
        let mut called: BTreeSet<&str> = BTreeSet::new();
        for call in calls {
            if !known.contains(call.as_str()) {
                return Err(InsertionError::UnknownDetectorCall(call.clone()));
            }
            called.insert(call.as_str());
        }
        let inserted = panel.inserted_ids();
        let true_positives = called.intersection(&inserted).count();
        let false_positives = called.len() - true_positives;

        Ok(LatentRecovery {
            called: called.len(),
            inserted: inserted.len(),
            true_positives,
            false_positives,
            recall: if inserted.is_empty() {
                0.0
            } else {
                true_positives as f64 / inserted.len() as f64
            },
            precision: if called.is_empty() {
                0.0
            } else {
                true_positives as f64 / called.len() as f64
            },
            panel_usable_as_an_oracle: confound.is_usable_as_an_oracle(),
        })
    }
}

/// Whether a semi-synthetic finding has been checked against observed data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "transfer", rename_all = "snake_case")]
pub enum TransferClaim {
    /// Checked on a named observed cohort, with the agreement that was found.
    Validated {
        observed_cohort: String,
        agreement: f64,
    },
    /// Not checked, and why. There is no `Default`, so this state has to be written down.
    Unvalidated { reason: String },
}

impl TransferClaim {
    pub fn validated(observed_cohort: impl Into<String>, agreement: f64) -> Self {
        TransferClaim::Validated {
            observed_cohort: observed_cohort.into(),
            agreement,
        }
    }

    pub fn unvalidated(reason: impl Into<String>) -> Self {
        TransferClaim::Unvalidated {
            reason: reason.into(),
        }
    }

    pub fn is_validated(&self) -> bool {
        matches!(self, TransferClaim::Validated { .. })
    }
}

/// Assigns `target` insertions across `backgrounds` deterministically from `seed`.
///
/// A partial Fisher-Yates over the background ids in sorted order, so the same seed and the same
/// panel always produce the same assignment on any machine. `onset` is taken from each background's
/// own observation window, so the result passes [`Panel::check_consistency`] by construction.
///
/// Randomisation is not a defence against the batch confound. It makes a batch-determined panel
/// unlikely, not impossible, and "unlikely" at panel sizes of a few dozen means "happens". Run
/// [`ConfoundReport::measure`] on the result; the tests demonstrate a seed where this is not
/// hypothetical.
pub fn assign_insertions(
    backgrounds: &[Background],
    seed: u64,
    target: usize,
    latent: &str,
    stated_effect_size: f64,
) -> Vec<Insertion> {
    let mut ordered: Vec<&Background> = backgrounds.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));

    let mut rng = SplitMix64::new(seed);
    let take = target.min(ordered.len());
    for index in 0..take {
        let swap = index + rng.below(ordered.len() - index);
        ordered.swap(index, swap);
    }

    let mut chosen: Vec<Insertion> = ordered[..take]
        .iter()
        .map(|background| {
            Insertion::new(
                &background.id,
                latent,
                onset_inside(&background.observed),
                stated_effect_size,
            )
        })
        .collect();
    chosen.sort_by(|left, right| left.background.cmp(&right.background));
    chosen
}

/// An instant the half-open interval contains, for a non-empty interval.
///
/// The bounded case takes the start. An unbounded start with a bounded end takes one nanosecond
/// before the end, because the interval is half-open and the end itself is outside it.
fn onset_inside(observed: &Interval) -> Timestamp {
    match (observed.start, observed.end) {
        (Some(start), _) => start,
        (None, Some(end)) => Timestamp::from_nanos_utc(end.as_nanos_utc() - 1),
        (None, None) => Timestamp::from_nanos_utc(0),
    }
}
