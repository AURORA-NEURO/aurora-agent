//! Measuring whether the router's confidence means anything.
//!
//! Blueprint 09.08 asks the router to report uncertainty and lets that uncertainty drive
//! fallback. It does not say how to check that the number is honest, and a confidence score that
//! is never checked is decoration. So this module measures it against the only ground truth
//! available: the oracle retrospective selector.
//!
//! For each routed task the outcome is a single bit — did the router pick the architecture the
//! oracle would have picked? Bin the decisions by confidence, and in each bin compare mean stated
//! confidence to the observed hit rate. The weighted mean absolute gap is the expected calibration
//! error. Zero means the score is a usable probability; a large gap means it is an ordering at
//! best.
//!
//! Two deliberate limitations, stated rather than hidden. First, abstentions carry confidence
//! zero by construction, so including them would make any router look well calibrated at the
//! bottom of the range; [`CalibrationCurve::build`] is therefore fed routed decisions only.
//! Second, with a few dozen tasks the per-bin hit rates are extremely noisy — this is a
//! diagnostic, not a guarantee, and the bin counts are reported so a reader can see that.

use crate::error::RoutingError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub lower: f64,
    pub upper: f64,
    pub decisions: usize,
    pub mean_confidence: f64,
    /// Fraction of decisions in this bin that matched the oracle's choice.
    pub oracle_agreement: f64,
}

impl CalibrationBin {
    pub fn gap(&self) -> f64 {
        (self.mean_confidence - self.oracle_agreement).abs()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationCurve {
    pub bins: Vec<CalibrationBin>,
    /// Total routed decisions the curve was built from. Small numbers make every bin unreliable.
    pub decisions: usize,
}

impl CalibrationCurve {
    /// `points` are `(stated confidence, matched the oracle's choice)` for **routed** decisions.
    pub fn build(points: &[(f64, bool)], bin_count: usize) -> Result<Self, RoutingError> {
        if bin_count == 0 {
            return Err(RoutingError::NoCalibrationBins);
        }

        let width = 1.0 / bin_count as f64;
        let mut bins: Vec<CalibrationBin> = (0..bin_count)
            .map(|index| CalibrationBin {
                lower: index as f64 * width,
                upper: (index + 1) as f64 * width,
                decisions: 0,
                mean_confidence: 0.0,
                oracle_agreement: 0.0,
            })
            .collect();

        let mut totals: Vec<(f64, usize)> = vec![(0.0, 0); bin_count];
        for (confidence, matched) in points {
            let clamped = confidence.clamp(0.0, 1.0);
            let index = ((clamped / width) as usize).min(bin_count - 1);
            bins[index].decisions += 1;
            totals[index].0 += clamped;
            if *matched {
                totals[index].1 += 1;
            }
        }

        for (index, bin) in bins.iter_mut().enumerate() {
            if bin.decisions == 0 {
                continue;
            }
            let count = bin.decisions as f64;
            bin.mean_confidence = totals[index].0 / count;
            bin.oracle_agreement = totals[index].1 as f64 / count;
        }

        Ok(CalibrationCurve {
            bins,
            decisions: points.len(),
        })
    }

    /// Decision-weighted mean absolute gap between stated confidence and observed agreement.
    ///
    /// Empty bins are skipped rather than counted as perfect, which would let a router with three
    /// decisions and ten bins report a near-zero error.
    pub fn expected_calibration_error(&self) -> Option<f64> {
        if self.decisions == 0 {
            return None;
        }
        let weighted: f64 = self
            .bins
            .iter()
            .filter(|bin| bin.decisions > 0)
            .map(|bin| bin.decisions as f64 * bin.gap())
            .sum();
        Some(weighted / self.decisions as f64)
    }

    pub fn populated_bins(&self) -> impl Iterator<Item = &CalibrationBin> {
        self.bins.iter().filter(|bin| bin.decisions > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_needs_at_least_one_bin() {
        assert_eq!(
            CalibrationCurve::build(&[(0.5, true)], 0).unwrap_err(),
            RoutingError::NoCalibrationBins
        );
    }

    #[test]
    fn a_perfectly_calibrated_router_has_zero_expected_calibration_error() {
        let points = vec![
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, true),
            (0.9, false),
        ];
        let curve = CalibrationCurve::build(&points, 10).unwrap();
        assert!(curve.expected_calibration_error().unwrap() < 1e-12);
    }

    #[test]
    fn an_overconfident_router_shows_a_gap_the_size_of_its_overconfidence() {
        let points = vec![(1.0, true), (1.0, false), (1.0, false), (1.0, false)];
        let curve = CalibrationCurve::build(&points, 4).unwrap();
        assert!((curve.expected_calibration_error().unwrap() - 0.75).abs() < 1e-12);
    }

    #[test]
    fn empty_bins_do_not_flatter_the_calibration_error() {
        let sparse = CalibrationCurve::build(&[(1.0, false)], 20).unwrap();
        assert_eq!(sparse.populated_bins().count(), 1);
        assert!((sparse.expected_calibration_error().unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_curve_with_no_decisions_reports_no_calibration_error_rather_than_zero() {
        let curve = CalibrationCurve::build(&[], 5).unwrap();
        assert_eq!(curve.expected_calibration_error(), None);
    }

    #[test]
    fn a_curve_round_trips_through_json() {
        let curve = CalibrationCurve::build(&[(0.3, true), (0.7, false)], 4).unwrap();
        let text = serde_json::to_string(&curve).unwrap();
        assert_eq!(
            serde_json::from_str::<CalibrationCurve>(&text).unwrap(),
            curve
        );
    }
}
