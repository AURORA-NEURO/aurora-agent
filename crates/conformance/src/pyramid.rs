//! The test pyramid of blueprint 40.31, measured rather than assumed.
//!
//! 40.31 requires "unit/property/integration/conformance/replay results" and coverage that is
//! "contract-based, not only line-based". It does not say in what proportion, and no honest
//! reading of it produces a number. So the thresholds below are a **project choice**, stated as
//! constants so that a reviewer can disagree with the specific value without having to reverse
//! it out of the logic.
//!
//! What the shape is for: a suite that is entirely end-to-end passes or fails as one bit and
//! localises nothing, while a suite that is entirely unit tests certifies field names and never
//! establishes that the vertical slice works. Both are green. Reporting the distribution, and
//! naming which layer is thin, is the only way that distinction reaches anyone.
//!
//! Not implemented: line coverage, branch coverage and mutation score. 40.31's verification plan
//! asks for mutation testing and this crate has no instrumentation and no mutation tooling — a
//! balanced pyramid here says the *cases are distributed*, not that the code is covered.

use crate::case::Layer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A layer holding less than this share of the suite is reported thin.
pub const THIN_LAYER_SHARE: f64 = 0.05;

/// A layer holding more than this share of the suite is reported dominant.
pub const DOMINANT_LAYER_SHARE: f64 = 0.60;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PyramidShape {
    pub counts: BTreeMap<Layer, usize>,
}

impl PyramidShape {
    pub fn from_layers(layers: impl IntoIterator<Item = Layer>) -> Self {
        let mut counts: BTreeMap<Layer, usize> = BTreeMap::new();
        for layer in Layer::ALL {
            counts.insert(layer, 0);
        }
        for layer in layers {
            *counts.entry(layer).or_insert(0) += 1;
        }
        PyramidShape { counts }
    }

    pub fn count(&self, layer: Layer) -> usize {
        self.counts.get(&layer).copied().unwrap_or(0)
    }

    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }

    pub fn share(&self, layer: Layer) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.count(layer) as f64 / total as f64
        }
    }

    /// Whether the distribution is a pyramid, and if not, what is wrong with it.
    pub fn balance(&self) -> PyramidBalance {
        let total = self.total();
        if total == 0 {
            return PyramidBalance::Unbalanced {
                findings: vec![ImbalanceFinding::NoCases],
            };
        }

        let mut findings = Vec::new();
        for layer in Layer::ALL {
            let count = self.count(layer);
            let share = self.share(layer);
            if count == 0 {
                findings.push(ImbalanceFinding::LayerAbsent { layer });
            } else if share < THIN_LAYER_SHARE {
                findings.push(ImbalanceFinding::LayerThin {
                    layer,
                    count,
                    share,
                });
            }
            if share > DOMINANT_LAYER_SHARE {
                findings.push(ImbalanceFinding::LayerDominant { layer, share });
            }
        }

        // The pyramid is inverted when the slowest layer outnumbers the fastest. Checked on the
        // extremes only: the middle layers legitimately vary with what a contract needs.
        let unit = self.count(Layer::Unit);
        let end_to_end = self.count(Layer::EndToEnd);
        if end_to_end > unit {
            findings.push(ImbalanceFinding::Inverted {
                unit,
                end_to_end,
            });
        }

        if findings.is_empty() {
            PyramidBalance::Balanced
        } else {
            PyramidBalance::Unbalanced { findings }
        }
    }

    /// One line per layer, for a report a human reads.
    pub fn describe(&self) -> String {
        let total = self.total();
        let mut lines = vec![format!("{total} cases")];
        for layer in Layer::ALL {
            lines.push(format!(
                "  {:<12} {:>3}  {:>5.1}%",
                layer.as_str(),
                self.count(layer),
                self.share(layer) * 100.0
            ));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "balance", rename_all = "snake_case")]
pub enum PyramidBalance {
    Balanced,
    Unbalanced { findings: Vec<ImbalanceFinding> },
}

impl PyramidBalance {
    pub fn is_balanced(&self) -> bool {
        matches!(self, PyramidBalance::Balanced)
    }

    pub fn findings(&self) -> &[ImbalanceFinding] {
        match self {
            PyramidBalance::Balanced => &[],
            PyramidBalance::Unbalanced { findings } => findings,
        }
    }

    /// The layers a reviewer should add cases to, in the order they were found.
    pub fn thin_layers(&self) -> Vec<Layer> {
        self.findings()
            .iter()
            .filter_map(|finding| match finding {
                ImbalanceFinding::LayerAbsent { layer } => Some(*layer),
                ImbalanceFinding::LayerThin { layer, .. } => Some(*layer),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum ImbalanceFinding {
    NoCases,
    LayerAbsent {
        layer: Layer,
    },
    LayerThin {
        layer: Layer,
        count: usize,
        share: f64,
    },
    LayerDominant {
        layer: Layer,
        share: f64,
    },
    /// More end-to-end cases than unit cases: defects that a fast case could localise are being
    /// caught only by the slowest, least diagnostic layer.
    Inverted {
        unit: usize,
        end_to_end: usize,
    },
}

impl fmt::Display for ImbalanceFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImbalanceFinding::NoCases => f.write_str("the suite contains no cases"),
            ImbalanceFinding::LayerAbsent { layer } => {
                write!(f, "the {layer} layer is empty")
            }
            ImbalanceFinding::LayerThin {
                layer,
                count,
                share,
            } => write!(
                f,
                "the {layer} layer is thin: {count} cases, {:.1}% of the suite",
                share * 100.0
            ),
            ImbalanceFinding::LayerDominant { layer, share } => write!(
                f,
                "the {layer} layer dominates: {:.1}% of the suite",
                share * 100.0
            ),
            ImbalanceFinding::Inverted { unit, end_to_end } => write!(
                f,
                "the pyramid is inverted: {end_to_end} end-to-end cases against {unit} unit cases"
            ),
        }
    }
}
