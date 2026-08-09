//! Pack health (blueprint 15.19, benchmark meta-evaluation).
//!
//! 15.19 exists because a benchmark can fail on its own terms while every run against it succeeds
//! mechanically. Its defect list is concrete: instruction–verifier mismatch, hidden-state leak,
//! environment brittleness, default-pass errors, contamination, duplicate or correlated
//! descendants. 15.00 adds the portfolio-level consequence — "retire saturated or redundant
//! packs".
//!
//! The design decision here is that health is not a report you can decline to read. A
//! [`PackAssessment`] is the only thing that produces a number, and it refuses when the findings
//! are blocking. A saturated pack published as "94% pass" is worse than publishing nothing,
//! because the 94% moves with sampling noise and reads as capability; so
//! [`PackAssessment::reportable_score`] returns [`PackError::UnreportablePack`] rather than a
//! score with a footnote. There is no accessor that returns the number anyway.
//!
//! Every assessment carries the digest of the pack it assessed. The blueprint invariant that
//! "every reported result is linked to an immutable run, task, benchmark-pack, architecture,
//! model, evaluator, and environment version" is otherwise trivially violated: a pack gets a
//! parent added, its saturation finding is a revision out of date, and the stale finding is what
//! gates publication.
//!
//! What is not detected here. Instruction–verifier mismatch, hidden-state leakage and environment
//! brittleness are properties of a pack's cells and oracles, and finding them requires executing
//! the pack — that is the runtime's and the oracle crate's work, not a declaration's. This module
//! detects the defects that are visible in observed *outcomes*: saturation, floors, a trivial
//! baseline that passes, contamination signatures, an ungrounded oracle set, and counts that were
//! never materialized.

use crate::calibration::{
    CalibrationPolicy, DifficultyCalibration, Discrimination, SystemObservation,
};
use crate::error::PackError;
use crate::ir::{PackId, PackIr};
use crate::taxonomy::OracleTier;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// A deliberately stupid solver run against the pack.
///
/// The point of 15.19's degeneracy check is not that trivial baselines exist but that they are
/// *run*. "Always answer the first option", "return the input unchanged", "always abstain" — if
/// one of these clears the pack, the pack measures the heuristic and not the construct in its
/// manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrivialBaseline {
    /// What the heuristic does, in enough detail to reproduce it.
    pub name: String,
    pub observation: SystemObservation,
}

/// Evidence that the pack's instances, or their answers, were visible before evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum ContaminationSignal {
    /// The answers are published somewhere reachable.
    PublicAnswerKey { location: String },
    /// Instances were found in a training corpus.
    CorpusMembership {
        corpus: String,
        matched_instances: u64,
    },
    /// The pack was public before the evaluated model's data cutoff. Not proof of memorization,
    /// but it removes the basis for claiming there was none.
    ReleasedBeforeCutoff {
        pack_release: String,
        model_cutoff: String,
    },
    /// The sharpest available signal: the same system does markedly better on instances that were
    /// public than on held-out instances drawn from the same generator.
    MemorizationGap {
        public: SystemObservation,
        held_out: SystemObservation,
    },
}

/// Everything measured *about* a pack, as opposed to declared by it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Observations {
    pub calibration: DifficultyCalibration,
    pub trivial_baselines: Vec<TrivialBaseline>,
    pub contamination: Vec<ContaminationSignal>,
}

/// Thresholds for the health checks that are not already covered by [`CalibrationPolicy`].
///
/// Conventions again, and marked as such. `degenerate_absolute` says a heuristic clearing half the
/// pack is disqualifying on its own; `degenerate_margin` catches the subtler and more common case
/// where the heuristic is merely indistinguishable from the best real system.
/// `memorization_margin` is the pass-rate gap between public and held-out instances that is
/// treated as contamination rather than noise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HealthPolicy {
    pub calibration: CalibrationPolicy,
    pub degenerate_absolute: f64,
    pub degenerate_margin: f64,
    pub memorization_margin: f64,
    /// Materialized fraction below which the declared instance count is an advisory finding.
    pub materialization_floor: f64,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        HealthPolicy {
            calibration: CalibrationPolicy::default(),
            degenerate_absolute: 0.5,
            degenerate_margin: 0.05,
            memorization_margin: 0.10,
            materialization_floor: 0.01,
        }
    }
}

/// How badly a finding compromises the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The pack may still be reported, with the finding attached.
    Advisory,
    /// No score may be published from this pack.
    Blocking,
}

/// One thing wrong with a pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum HealthFinding {
    /// Everyone passes.
    Saturated {
        pooled_pass_rate: f64,
        systems: usize,
    },
    /// Everyone fails. Blocking for the same reason saturation is — nothing is ordered — and
    /// because a floor is usually an environment fault masquerading as difficulty.
    Floored {
        pooled_pass_rate: f64,
        systems: usize,
    },
    /// Not enough systems or trials to characterise the pack yet.
    NotYetCharacterised { reason: String },
    /// A trivial heuristic clears the pack, so the pack measures the heuristic.
    Degenerate {
        baseline: String,
        baseline_pass_rate: f64,
        best_system_pass_rate: f64,
    },
    /// The instances or their answers were visible before evaluation.
    Contaminated { signal: ContaminationSignal },
    /// Nothing can settle a disagreement by re-running.
    NoGroundedOracle { tiers: Vec<OracleTier> },
    /// The advertised instance count is mostly unvalidated.
    CountsNotMaterialized {
        declared: u64,
        validated: u64,
        materialized_fraction: f64,
    },
}

impl HealthFinding {
    pub fn severity(&self) -> Severity {
        match self {
            HealthFinding::Saturated { .. }
            | HealthFinding::Floored { .. }
            | HealthFinding::Degenerate { .. }
            | HealthFinding::Contaminated { .. }
            | HealthFinding::NoGroundedOracle { .. } => Severity::Blocking,
            HealthFinding::NotYetCharacterised { .. }
            | HealthFinding::CountsNotMaterialized { .. } => Severity::Advisory,
        }
    }

    /// The sentence that goes next to the pack in a portfolio table.
    pub fn explain(&self) -> String {
        match self {
            HealthFinding::Saturated {
                pooled_pass_rate,
                systems,
            } => format!(
                "saturated: {pooled_pass_rate:.3} pooled pass rate across {systems} system(s); \
                 remaining headroom is smaller than run-to-run noise"
            ),
            HealthFinding::Floored {
                pooled_pass_rate,
                systems,
            } => format!(
                "floored: {pooled_pass_rate:.3} pooled pass rate across {systems} system(s); \
                 check the environment before concluding the tasks are hard"
            ),
            HealthFinding::NotYetCharacterised { reason } => {
                format!("not yet characterised: {reason}")
            }
            HealthFinding::Degenerate {
                baseline,
                baseline_pass_rate,
                best_system_pass_rate,
            } => format!(
                "degenerate: the trivial baseline `{baseline}` passes at {baseline_pass_rate:.3} \
                 against a best system rate of {best_system_pass_rate:.3}; the pack measures the \
                 heuristic"
            ),
            HealthFinding::Contaminated { signal } => match signal {
                ContaminationSignal::PublicAnswerKey { location } => {
                    format!("contaminated: answers are published at {location}")
                }
                ContaminationSignal::CorpusMembership {
                    corpus,
                    matched_instances,
                } => format!(
                    "contaminated: {matched_instances} instance(s) found in corpus {corpus}"
                ),
                ContaminationSignal::ReleasedBeforeCutoff {
                    pack_release,
                    model_cutoff,
                } => format!(
                    "contaminated: pack released {pack_release}, before the model cutoff \
                     {model_cutoff}; memorization cannot be ruled out"
                ),
                ContaminationSignal::MemorizationGap { public, held_out } => format!(
                    "contaminated: {:.3} on public instances against {:.3} held out",
                    public.pass_rate().unwrap_or(0.0),
                    held_out.pass_rate().unwrap_or(0.0)
                ),
            },
            HealthFinding::NoGroundedOracle { tiers } => format!(
                "no execution-grounded oracle; declared tiers are {tiers:?}, so no disagreement \
                 can be settled by re-running"
            ),
            HealthFinding::CountsNotMaterialized {
                declared,
                validated,
                materialized_fraction,
            } => format!(
                "counts not materialized: {validated} of {declared} declared instances validated \
                 ({materialized_fraction:.4} of the headline)"
            ),
        }
    }
}

/// Overall state of a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdict {
    Healthy,
    /// Usable, with findings attached to any published score.
    Degraded,
    /// No score may be published.
    Unreportable,
}

/// The health of one revision of one pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackHealth {
    pub pack: PackId,
    pub pack_digest: ContentHash,
    pub findings: Vec<HealthFinding>,
}

impl PackHealth {
    pub fn verdict(&self) -> HealthVerdict {
        if self
            .findings
            .iter()
            .any(|f| f.severity() == Severity::Blocking)
        {
            HealthVerdict::Unreportable
        } else if self.findings.is_empty() {
            HealthVerdict::Healthy
        } else {
            HealthVerdict::Degraded
        }
    }

    pub fn is_reportable(&self) -> bool {
        self.verdict() != HealthVerdict::Unreportable
    }

    pub fn blocking(&self) -> Vec<&HealthFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity() == Severity::Blocking)
            .collect()
    }

    pub fn advisories(&self) -> Vec<&HealthFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity() == Severity::Advisory)
            .collect()
    }
}

/// A pack, its measured health, and its calibration, bound to one revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackAssessment {
    pub pack: PackId,
    pub pack_digest: ContentHash,
    pub calibration: DifficultyCalibration,
    pub health: PackHealth,
    pub policy: HealthPolicy,
}

/// A score that survived the health gate, with everything a reader needs to discount it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportedScore {
    pub pack: PackId,
    pub pack_digest: ContentHash,
    pub pooled_pass_rate: f64,
    pub discrimination: Discrimination,
    /// Non-blocking findings. Present in the type so they cannot be dropped in rendering.
    pub advisories: Vec<HealthFinding>,
}

impl PackAssessment {
    /// The only way to obtain a number from a pack.
    ///
    /// Fails when the assessment was taken against a different revision, when any finding is
    /// blocking, or when no trials were recorded. The third case matters as much as the first
    /// two: an unevaluated pack has no pass rate, and returning 0.0 for it would put "hard" and
    /// "unmeasured" in the same column.
    pub fn reportable_score(&self, pack: &PackIr) -> Result<ReportedScore, PackError> {
        let presented = pack.digest()?;
        if presented != self.pack_digest {
            return Err(PackError::AssessmentDigestMismatch {
                assessed: self.pack_digest.to_string(),
                presented: presented.to_string(),
            });
        }
        let blocking = self.health.blocking();
        if !blocking.is_empty() {
            let findings = blocking
                .iter()
                .map(|f| f.explain())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(PackError::UnreportablePack {
                pack: self.pack.to_string(),
                findings,
            });
        }
        let pooled = self
            .calibration
            .pooled_pass_rate()
            .ok_or_else(|| PackError::NoObservations(self.pack.to_string()))?;
        Ok(ReportedScore {
            pack: self.pack.clone(),
            pack_digest: self.pack_digest.clone(),
            pooled_pass_rate: pooled,
            discrimination: self.calibration.discrimination(&self.policy.calibration),
            advisories: self.health.advisories().into_iter().cloned().collect(),
        })
    }
}

/// Run every implemented health check against a pack and its observations.
///
/// The checks are ordered so that the most disqualifying appear first in the finding list, which
/// is what a truncated table will show.
///
/// The degeneracy check needs a measured best system and is skipped when there is not one. Reading
/// an absent best rate as zero would make every trivial baseline degenerate by arithmetic and would
/// publish `best_system_pass_rate: 0.0` as though a system had been run and had failed everything;
/// a pack nobody has run yet is [`HealthFinding::NotYetCharacterised`], which the discrimination
/// check above has already recorded.
pub fn assess(
    pack: &PackIr,
    observations: &Observations,
    policy: &HealthPolicy,
) -> Result<PackAssessment, PackError> {
    let digest = pack.digest()?;
    let mut findings = Vec::new();

    match observations.calibration.discrimination(&policy.calibration) {
        Discrimination::Saturated {
            pooled_pass_rate,
            systems,
        } => findings.push(HealthFinding::Saturated {
            pooled_pass_rate,
            systems,
        }),
        Discrimination::Floored {
            pooled_pass_rate,
            systems,
        } => findings.push(HealthFinding::Floored {
            pooled_pass_rate,
            systems,
        }),
        Discrimination::Undetermined { reason } => {
            findings.push(HealthFinding::NotYetCharacterised { reason })
        }
        Discrimination::Discriminating { .. } => {}
    }

    if let Some(best) = observations.calibration.best().and_then(|o| o.pass_rate()) {
        for baseline in &observations.trivial_baselines {
            let Some(rate) = baseline.observation.pass_rate() else {
                continue;
            };
            if rate >= policy.degenerate_absolute || rate + policy.degenerate_margin >= best {
                findings.push(HealthFinding::Degenerate {
                    baseline: baseline.name.clone(),
                    baseline_pass_rate: rate,
                    best_system_pass_rate: best,
                });
            }
        }
    }

    for signal in &observations.contamination {
        if let ContaminationSignal::MemorizationGap { public, held_out } = signal {
            let gap = match (public.pass_rate(), held_out.pass_rate()) {
                (Some(p), Some(h)) => p - h,
                _ => continue,
            };
            if gap < policy.memorization_margin {
                continue;
            }
        }
        findings.push(HealthFinding::Contaminated {
            signal: signal.clone(),
        });
    }

    if !pack.content.has_grounded_oracle() {
        findings.push(HealthFinding::NoGroundedOracle {
            tiers: pack.content.oracles.clone(),
        });
    }

    let counts = pack.content.counts();
    if let Some(fraction) = counts.materialized_fraction() {
        if fraction < policy.materialization_floor {
            findings.push(HealthFinding::CountsNotMaterialized {
                declared: counts.declared_instances,
                validated: counts.validated_instances,
                materialized_fraction: fraction,
            });
        }
    }

    Ok(PackAssessment {
        pack: pack.manifest.id.clone(),
        pack_digest: digest.clone(),
        calibration: observations.calibration.clone(),
        health: PackHealth {
            pack: pack.manifest.id.clone(),
            pack_digest: digest,
            findings,
        },
        policy: *policy,
    })
}
