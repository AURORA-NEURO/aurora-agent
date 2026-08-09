//! Reproducibility certification, and the claim it is not (26.11).
//!
//! 26.11's design detail is a warning about its own module: "Reproducibility and validity are
//! separate. A perfectly reproducible pipeline can consistently compute the wrong estimand or use
//! the wrong specimen mapping." A certificate that says a rerun matched is a fact about the
//! *computation*, and it is routinely quoted as if it were a fact about the *biology*.
//!
//! So [`Certificate`] carries an explicit [`Certificate::supports`] method that always returns
//! [`ReproError::NotAValidityClaim`], and there is no method returning a bare boolean called
//! anything like `is_valid`. The certificate can answer "did the outputs match" ([`Certificate::
//! reproduced`]) and nothing else. This is the same shape as `bioprism-safety`'s
//! `ContainmentRequest` with no `ContainmentPerformed`: the type that would carry the stronger
//! claim is absent on purpose.
//!
//! # First divergence, not a match rate
//!
//! 26.11 step 4 is "trace discrepancies to first divergence". [`Certificate::first_divergence`]
//! returns the earliest output in declared order that fell outside tolerance, which is the
//! actionable object; a "numerical agreement" percentage over all outputs would average a
//! catastrophic mismatch in step one with fifty matching downstream artifacts that were all
//! derived from it.
//!
//! # Tolerance is per output and must be stated
//!
//! There is no default tolerance. A comparison of floating-point outputs "within tolerance" where
//! the tolerance was chosen after seeing the difference is 26.11's implicit version of the
//! post-hoc rubric problem [`crate::reveal`] guards against, and the only structural defence is to
//! make the tolerance part of the declaration.
//!
//! # Not implemented
//!
//! No execution. This crate spawns nothing; a caller reruns the workflow and brings the outputs.
//! No environment reconstruction — 26.11's step 1 ("pin inputs and environment") is a property of
//! a container image and a lockfile, and the platform-level machinery for it is
//! `bioprism-infra`'s. No figure regeneration and no statistical recomputation: both are named in
//! 26.11's evaluation target and both need to run code. No "discrepancy localization time": that
//! is a stopwatch reading about a human, not a predicate over an artifact.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::ReproError;

/// What kind of thing was compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// A byte-identical artifact: a digest, a serialized record.
    Exact,
    /// A number compared within a declared tolerance.
    Numeric,
    /// A figure or table compared against the data it claims to show.
    Derived,
}

/// One declared output and how closely a rerun must match it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSpec {
    pub id: String,
    pub kind: OutputKind,
    /// Absolute tolerance for [`OutputKind::Numeric`]. Ignored for the other kinds, where the only
    /// admissible tolerance is zero.
    pub tolerance: f64,
}

impl OutputSpec {
    /// An output that must match byte for byte.
    pub fn exact(id: impl Into<String>) -> Self {
        OutputSpec {
            id: id.into(),
            kind: OutputKind::Exact,
            tolerance: 0.0,
        }
    }

    /// A numeric output with a stated absolute tolerance.
    pub fn numeric(id: impl Into<String>, tolerance: f64) -> Result<Self, ReproError> {
        let id = id.into();
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(ReproError::BadTolerance {
                output: id,
                tolerance,
            });
        }
        Ok(OutputSpec {
            id,
            kind: OutputKind::Numeric,
            tolerance,
        })
    }

    /// A figure or table checked against its own source data.
    pub fn derived(id: impl Into<String>) -> Self {
        OutputSpec {
            id: id.into(),
            kind: OutputKind::Derived,
            tolerance: 0.0,
        }
    }
}

/// What a rerun produced for one output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "observation")]
pub enum Observed {
    /// Two digests, compared by equality.
    Digests { original: String, rerun: String },
    /// Two numbers, compared against the spec's tolerance.
    Numbers { original: f64, rerun: f64 },
    /// The rerun did not produce this output at all. Distinct from a mismatch: a missing output is
    /// a broken workflow, and calling it a numerical disagreement would misdirect the reader.
    Absent,
}

/// How one output came out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum OutputVerdict {
    Matched,
    Diverged { detail: String },
    Missing,
}

impl OutputVerdict {
    /// Whether this output reproduced.
    pub fn matched(&self) -> bool {
        matches!(self, OutputVerdict::Matched)
    }
}

/// A declared set of outputs and the rerun to check against them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reexecution {
    /// The workflow that was rerun, as an opaque identifier.
    pub workflow: String,
    /// Whether the environment was pinned. Recorded, never inferred: an unpinned rerun that
    /// happened to match has not demonstrated portability.
    pub environment_pinned: bool,
    specs: Vec<OutputSpec>,
    observations: Vec<(String, Observed)>,
}

impl Reexecution {
    /// Declare the outputs a rerun must reproduce.
    pub fn declaring(
        workflow: impl Into<String>,
        environment_pinned: bool,
        specs: Vec<OutputSpec>,
    ) -> Result<Self, ReproError> {
        let mut seen = BTreeSet::new();
        for spec in &specs {
            if !seen.insert(spec.id.clone()) {
                return Err(ReproError::DuplicateOutput(spec.id.clone()));
            }
        }
        Ok(Reexecution {
            workflow: workflow.into(),
            environment_pinned,
            specs,
            observations: Vec::new(),
        })
    }

    /// Record what the rerun produced for one declared output.
    pub fn observe(
        &mut self,
        output: impl Into<String>,
        observed: Observed,
    ) -> Result<(), ReproError> {
        let output = output.into();
        if self.observations.iter().any(|(id, _)| *id == output) {
            return Err(ReproError::DuplicateOutput(output));
        }
        self.observations.push((output, observed));
        Ok(())
    }

    /// Compare and certify.
    ///
    /// An output that was declared and never observed becomes [`OutputVerdict::Missing`] rather
    /// than being skipped, so a rerun cannot improve its match rate by producing fewer outputs.
    pub fn certify(&self) -> Result<Certificate, ReproError> {
        if self.specs.is_empty() {
            return Err(ReproError::NothingCompared);
        }
        let mut verdicts = Vec::new();
        for spec in &self.specs {
            let observed = self
                .observations
                .iter()
                .find(|(id, _)| *id == spec.id)
                .map(|(_, o)| o.clone())
                .unwrap_or(Observed::Absent);
            let verdict = match (&spec.kind, &observed) {
                (_, Observed::Absent) => OutputVerdict::Missing,
                (OutputKind::Numeric, Observed::Numbers { original, rerun }) => {
                    let delta = (original - rerun).abs();
                    if delta <= spec.tolerance {
                        OutputVerdict::Matched
                    } else {
                        OutputVerdict::Diverged {
                            detail: format!(
                                "|{original} - {rerun}| = {delta} exceeds tolerance {}",
                                spec.tolerance
                            ),
                        }
                    }
                }
                (_, Observed::Digests { original, rerun }) => {
                    if original == rerun {
                        OutputVerdict::Matched
                    } else {
                        OutputVerdict::Diverged {
                            detail: format!("{original} != {rerun}"),
                        }
                    }
                }
                (kind, observation) => OutputVerdict::Diverged {
                    detail: format!("{observation:?} is not a comparison for a {kind:?} output"),
                },
            };
            verdicts.push((spec.id.clone(), verdict));
        }
        Ok(Certificate {
            workflow: self.workflow.clone(),
            environment_pinned: self.environment_pinned,
            verdicts,
        })
    }

    /// The declared specs, in order.
    pub fn specs(&self) -> &[OutputSpec] {
        &self.specs
    }
}

/// The receipt: what matched, what did not, and where it first went wrong.
///
/// There is deliberately no field or method on this type that says anything about whether the
/// workflow computed the right thing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    pub workflow: String,
    pub environment_pinned: bool,
    verdicts: Vec<(String, OutputVerdict)>,
}

impl Certificate {
    /// Whether every declared output matched.
    pub fn reproduced(&self) -> bool {
        self.verdicts.iter().all(|(_, v)| v.matched())
    }

    /// The first declared output that did not match, in declaration order.
    pub fn first_divergence(&self) -> Option<(&str, &OutputVerdict)> {
        self.verdicts
            .iter()
            .find(|(_, v)| !v.matched())
            .map(|(id, v)| (id.as_str(), v))
    }

    /// Outputs the rerun never produced.
    pub fn missing(&self) -> Vec<&str> {
        self.verdicts
            .iter()
            .filter(|(_, v)| matches!(v, OutputVerdict::Missing))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Every verdict, in declaration order.
    pub fn verdicts(&self) -> &[(String, OutputVerdict)] {
        &self.verdicts
    }

    /// Always refuses.
    ///
    /// This method exists so that the refusal is discoverable at the place a caller would look for
    /// the affirmative. A reader searching for "does this certificate support my claim about the
    /// tumour" finds a function whose entire body is 26.11's design detail, rather than finding
    /// nothing and reaching for [`Certificate::reproduced`] instead.
    pub fn supports(&self, biological_claim: &str) -> Result<(), ReproError> {
        Err(ReproError::NotAValidityClaim(biological_claim.to_string()))
    }

    /// Whether this certificate demonstrates portability as well as repeatability.
    ///
    /// Reproducing in an unpinned environment is a stronger result; reproducing in a pinned one is
    /// the weaker claim and is reported as such rather than as "reproducible".
    pub fn portability_demonstrated(&self) -> bool {
        self.reproduced() && !self.environment_pinned
    }
}
