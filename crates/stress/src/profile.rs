//! The stress verdict: a robustness profile, not a score.
//!
//! Blueprint 32.07's scoring section asks for *"failure onset as mutation intensity increases"* and
//! for the *"effective—not nominal—test size"*. Neither is a pass mark. A family that reports "83%
//! of conclusions survived" has thrown away the only actionable number in the run — the magnitude
//! at which each conclusion stopped being true — and replaced it with an average over quantities
//! that are not commensurable.
//!
//! So a run produces, per conclusion: the largest intensity at which its declared relation still
//! held, the intensity at which it first failed, and what the failure looked like. Three things
//! are kept strictly apart in the report:
//!
//! - A **probed** relation that fails is the finding. Report the breaking point.
//! - A **required** relation that fails indicts the procedure, not its robustness. A discriminative
//!   ranking cannot legitimately move under a pure reweighting; if it does, the label
//!   "discriminative" is wrong.
//! - A **cohort postcondition** that fails indicts the generator. Everything measured at that
//!   intensity is uninterpretable, so nothing is measured there: the rung is abandoned rather than
//!   scored, because a number produced by a broken generator is worse than a missing one.
//!
//! And one thing that is refused outright. If the batch a shift targets contains only one class,
//! the shift is collinear with the biology and no conclusion's response can be attributed to
//! either. 32.06 names this — *"batch and condition are perfectly confounded without
//! acknowledgement"*. The profile reports [`Identifiability::Confounded`] and marks its findings
//! uninformative rather than emitting a clean pass, because a stress that cannot discriminate
//! passes everything.

use crate::cohort::Cohort;
use crate::conclusion::{Character, Procedure};
use crate::error::StressError;
use crate::family::{Knob, Magnitude, Stress, StressFamily};
use crate::invariant::PostconditionResult;
use crate::perturb::perturb;
use crate::relation::{declare, Obligation};
use serde::{Deserialize, Serialize};

/// Whether a stress can be told apart from the biology it sits next to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "identifiability", rename_all = "snake_case")]
pub enum Identifiability {
    /// The stress does not target a batch, so batch confounding is not in play.
    NotApplicable,
    /// The targeted batch contains both classes; a response can be attributed to the shift.
    Separable { batch: String, overlap: f64 },
    /// The targeted batch contains one class only. Nothing here is attributable.
    Confounded { batch: String, only: String },
}

impl Identifiability {
    pub fn informative(&self) -> bool {
        !matches!(self, Identifiability::Confounded { .. })
    }

    /// Assesses the parent cohort against the stress about to be applied.
    pub fn of(cohort: &Cohort, stress: &Stress) -> Identifiability {
        let Knob::BatchEffect { batch, .. } = &stress.knob else {
            return Identifiability::NotApplicable;
        };
        let positives = cohort
            .subjects
            .iter()
            .filter(|subject| &subject.batch == batch && subject.condition)
            .count();
        let negatives = cohort
            .subjects
            .iter()
            .filter(|subject| &subject.batch == batch && !subject.condition)
            .count();
        let total = positives + negatives;
        if total == 0 || positives == 0 || negatives == 0 {
            return Identifiability::Confounded {
                batch: batch.clone(),
                only: if positives == 0 { "negative" } else { "positive" }.into(),
            };
        }
        Identifiability::Separable {
            batch: batch.clone(),
            overlap: positives.min(negatives) as f64 / total as f64,
        }
    }
}

/// A postcondition of the generator that failed, and where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorDefect {
    pub magnitude: Magnitude,
    pub invariant: String,
    pub expected: String,
    pub observed: String,
}

/// One conclusion's response across the sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConclusionRobustness {
    pub conclusion_id: String,
    pub character: Character,
    pub obligation: Obligation,
    pub relation: String,
    pub rationale: String,
    /// The largest intensity at which the relation still held.
    pub held_through: Option<Magnitude>,
    /// The smallest intensity at which it failed. The number worth reporting.
    pub broke_at: Option<Magnitude>,
    pub expected_at_break: Option<String>,
    pub observed_at_break: Option<String>,
}

impl ConclusionRobustness {
    pub fn survived(&self) -> bool {
        self.broke_at.is_none()
    }

    /// A required relation that failed. Not a robustness finding — a mislabelled procedure.
    pub fn indicts_procedure(&self) -> bool {
        self.obligation == Obligation::Required && self.broke_at.is_some()
    }

    pub fn line(&self) -> String {
        match (&self.broke_at, self.obligation) {
            (None, _) => format!(
                "{}: held through {} ({})",
                self.conclusion_id,
                self.held_through
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "no rung".into()),
                self.relation
            ),
            (Some(magnitude), Obligation::Probed) => format!(
                "{}: broke at {magnitude} — expected {}, observed {}",
                self.conclusion_id,
                self.expected_at_break.as_deref().unwrap_or("-"),
                self.observed_at_break.as_deref().unwrap_or("-")
            ),
            (Some(magnitude), Obligation::Required) => format!(
                "{}: REQUIRED relation failed at {magnitude} — expected {}, observed {}. The \
                 procedure is mislabelled, not fragile.",
                self.conclusion_id,
                self.expected_at_break.as_deref().unwrap_or("-"),
                self.observed_at_break.as_deref().unwrap_or("-")
            ),
        }
    }
}

/// What the cohort itself looked like at one rung of the sweep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepPoint {
    pub magnitude: Magnitude,
    /// Kish effective sample size. Falls as reweighting concentrates mass.
    pub effective_n: f64,
    pub nominal_n: usize,
    /// Subjects the assay could no longer measure.
    pub unresolved: usize,
    /// Base rate over analysable subjects. Drifts when censoring is class-dependent, which is
    /// 32.03's *"depth changes class prevalence through filtering"*.
    pub analysable_prevalence: f64,
    /// Set when the generator's own postconditions failed here, so nothing was scored.
    pub abandoned: bool,
}

/// The verdict of running one stress family against one cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RobustnessProfile {
    pub family: StressFamily,
    pub blueprint_module: String,
    pub stress_id: String,
    pub cohort_id: String,
    pub parent_digest: String,
    pub identifiability: Identifiability,
    pub sweep: Vec<SweepPoint>,
    pub findings: Vec<ConclusionRobustness>,
    pub generator_defects: Vec<GeneratorDefect>,
    pub caveat: String,
}

impl RobustnessProfile {
    /// Conclusions that held at every rung.
    pub fn survivors(&self) -> Vec<&ConclusionRobustness> {
        self.findings
            .iter()
            .filter(|finding| finding.survived())
            .collect()
    }

    /// The weakest probed conclusion: the one that broke at the lowest intensity.
    pub fn first_to_break(&self) -> Option<&ConclusionRobustness> {
        self.findings
            .iter()
            .filter(|finding| finding.obligation == Obligation::Probed)
            .filter(|finding| finding.broke_at.is_some())
            .min_by_key(|finding| finding.broke_at)
    }

    /// Required relations that failed. Never robustness findings.
    pub fn mislabelled_procedures(&self) -> Vec<&ConclusionRobustness> {
        self.findings
            .iter()
            .filter(|finding| finding.indicts_procedure())
            .collect()
    }

    pub fn generator_is_sound(&self) -> bool {
        self.generator_defects.is_empty()
    }

    /// The sentence a report should lead with. Never a score.
    pub fn headline(&self) -> String {
        if let Identifiability::Confounded { batch, only } = &self.identifiability {
            return format!(
                "{} against {}: NOT IDENTIFIABLE. Batch {batch} contains only {only} subjects, so \
                 the shift is collinear with the condition and no conclusion's response can be \
                 attributed to either. No robustness claim is made.",
                self.family.as_str(),
                self.cohort_id
            );
        }
        if !self.generator_is_sound() {
            return format!(
                "{} against {}: GENERATOR DEFECTIVE. {} postcondition(s) failed, the first being \
                 {} at magnitude {}. Nothing was scored at the affected intensities.",
                self.family.as_str(),
                self.cohort_id,
                self.generator_defects.len(),
                self.generator_defects[0].invariant,
                self.generator_defects[0].magnitude
            );
        }
        let survivors = self.survivors().len();
        let broke = self.findings.len() - survivors;
        let onset = self
            .first_to_break()
            .and_then(|finding| finding.broke_at)
            .map(|magnitude| format!("first failure at magnitude {magnitude}"))
            .unwrap_or_else(|| "no failure up to full magnitude".into());
        let detail = if broke == 0 {
            String::new()
        } else {
            format!(" Breaking points: {}.", self.breaking_points().join("; "))
        };
        format!(
            "{} against {}: {survivors} of {} conclusion(s) held to full magnitude, {onset}. {}{detail}",
            self.family.as_str(),
            self.cohort_id,
            self.findings.len(),
            self.family.claim()
        )
    }

    /// Every conclusion that broke, with the intensity that broke it.
    pub fn breaking_points(&self) -> Vec<String> {
        self.findings
            .iter()
            .filter(|finding| !finding.survived())
            .map(|finding| {
                format!(
                    "{} at {}",
                    finding.conclusion_id,
                    finding
                        .broke_at
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "-".into())
                )
            })
            .collect()
    }
}

/// Runs one stress family across the intensity ladder and reports the breaking points.
///
/// The stress supplies the knob at full setting; its magnitude field is ignored, because the
/// sweep sets it. Everything is derived from the parent cohort and the seed, so two runs of the
/// same arguments produce the same profile.
pub fn profile(
    parent: &Cohort,
    stress: &Stress,
    procedures: &[Procedure],
) -> Result<RobustnessProfile, StressError> {
    parent.validate()?;
    let identifiability = Identifiability::of(parent, stress);
    let parent_digest = parent.digest()?.as_str().to_string();

    let mut findings: Vec<ConclusionRobustness> = Vec::new();
    let mut baselines = Vec::new();
    for procedure in procedures {
        let baseline = procedure.conclude(parent)?;
        let declared = declare(&stress.at(Magnitude::FULL), procedure, parent)?;
        findings.push(ConclusionRobustness {
            conclusion_id: baseline.id.clone(),
            character: baseline.character,
            obligation: declared.obligation,
            relation: declared.relation.describe(),
            rationale: declared.rationale,
            held_through: None,
            broke_at: None,
            expected_at_break: None,
            observed_at_break: None,
        });
        baselines.push(baseline);
    }

    let mut sweep = Vec::new();
    let mut defects = Vec::new();

    for magnitude in Magnitude::ladder() {
        let at = stress.at(magnitude);
        let perturbed = perturb(parent, &at)?;
        let stressed = perturbed.cohort();

        let abandoned = !perturbed.is_valid();
        for check in perturbed.defects() {
            if let PostconditionResult::Violated { expected, observed } = &check.result {
                defects.push(GeneratorDefect {
                    magnitude,
                    invariant: check.invariant.label().to_string(),
                    expected: expected.clone(),
                    observed: observed.clone(),
                });
            }
        }
        sweep.push(SweepPoint {
            magnitude,
            effective_n: stressed.effective_n(),
            nominal_n: stressed.len(),
            unresolved: stressed.unresolved_count(),
            analysable_prevalence: stressed.prevalence(),
            abandoned,
        });
        if abandoned {
            continue;
        }

        for ((procedure, baseline), finding) in procedures
            .iter()
            .zip(baselines.iter())
            .zip(findings.iter_mut())
        {
            if finding.broke_at.is_some() {
                continue;
            }
            let declared = declare(&at, procedure, parent)?;
            let outcome = match procedure.conclude(stressed) {
                Ok(after) => declared.relation.check(baseline, &after),
                Err(error) => PostconditionResult::violated(
                    declared.relation.describe(),
                    format!("the conclusion no longer exists: {error}"),
                ),
            };
            match outcome {
                PostconditionResult::Held => finding.held_through = Some(magnitude),
                PostconditionResult::Violated { expected, observed } => {
                    finding.broke_at = Some(magnitude);
                    finding.expected_at_break = Some(expected);
                    finding.observed_at_break = Some(observed);
                }
            }
        }
    }

    Ok(RobustnessProfile {
        family: stress.family(),
        blueprint_module: stress.family().blueprint_module().to_string(),
        stress_id: stress.id.clone(),
        cohort_id: parent.id.clone(),
        parent_digest,
        identifiability,
        sweep,
        findings,
        generator_defects: defects,
        caveat: format!(
            "Breaking points are resolved to the {}-rung intensity ladder and are upper bounds: a \
             conclusion reported as breaking at magnitude m failed somewhere in the interval below \
             m. Survival to full magnitude is evidence about this cohort under this stress family \
             at this seed, and is not a general robustness claim.",
            Magnitude::ladder().len()
        ),
    })
}

/// Several families run against one cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StressReport {
    pub cohort_id: String,
    pub profiles: Vec<RobustnessProfile>,
}

impl StressReport {
    /// Runs every stress against the cohort.
    pub fn run(
        parent: &Cohort,
        stresses: &[Stress],
        procedures: &[Procedure],
    ) -> Result<StressReport, StressError> {
        let profiles = stresses
            .iter()
            .map(|stress| profile(parent, stress, procedures))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StressReport {
            cohort_id: parent.id.clone(),
            profiles,
        })
    }

    /// The family that broke a conclusion at the lowest intensity.
    ///
    /// Blueprint 32.07 asks the hub to report the *"worst mutation family"*. The comparison is
    /// only meaningful because every family's magnitude one means "the whole of the stress its
    /// author declared" — it is a comparison of declared budgets, not of physical units.
    pub fn worst_family(&self) -> Option<&RobustnessProfile> {
        self.profiles
            .iter()
            .filter(|profile| profile.identifiability.informative())
            .filter(|profile| profile.generator_is_sound())
            .filter_map(|profile| {
                profile
                    .first_to_break()
                    .and_then(|finding| finding.broke_at)
                    .map(|magnitude| (magnitude, profile))
            })
            .min_by_key(|(magnitude, _)| *magnitude)
            .map(|(_, profile)| profile)
    }

    pub fn headline(&self) -> String {
        let lines: Vec<String> = self
            .profiles
            .iter()
            .map(RobustnessProfile::headline)
            .collect();
        lines.join("\n")
    }
}
