//! Difficulty calibration and effective diversity.
//!
//! Blueprint 06.13. Two numbers a benchmark report must not conflate with anything else.
//!
//! **Effective diversity is the number of independent equivalence classes, not the instance
//! count.** This is a workspace non-negotiable, and the definition is not reinvented here:
//! `bioprism_mutation::diversity` already counts distinct `(parent, mutation family, oracle
//! signature)` triples, and [`EffectiveDiversity`] counts exactly the same triples. It is restated
//! rather than imported because this crate does not depend on the mutation engine — the compiler
//! must be usable on a hand-authored family that no mutation program produced. **If the two ever
//! disagree, `bioprism_mutation` is authoritative**; this is the derived copy.
//!
//! **Unmeasured is not zero.** An instance no panel ran has [`DifficultyEstimate::Unmeasured`],
//! which is a different variant, not a success rate of 0.0. There is no accessor that turns one
//! into the other.
//!
//! ## Trivial cues and safety vetoes
//!
//! 06.13 asks for "deliberately weak and rule-based policies to detect trivial cues": an instance a
//! rule-based policy solves is measuring the cue, not the capability. It also notes that safety
//! veto cases "may intentionally be easy but remain important", so easiness alone is not a defect —
//! the caller declares which instances are vetoes and those are labelled rather than pruned.
//!
//! ## What is deliberately not implemented
//!
//! 06.13's hierarchical difficulty model — success probability as a function of architecture
//! capability with random effects for parent and mutation family — is not here. Fitting one needs
//! an inference library this offline workspace does not carry, and a two-point difference of
//! proportions presented as a fitted model would be a much worse lie than an honest difference of
//! proportions. [`InstanceCalibration::discrimination`] is therefore exactly that: observed strong
//! pass rate minus observed weak pass rate, with the sample sizes attached so a reader can see how
//! little it rests on.
//!
//! Drift recalibration is also absent. It requires results from more than one point in time, and
//! this crate has no clock and no store; the caller re-runs calibration and compares.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How capable the policy that produced a run was.
///
/// The panel must span this range or discrimination is not measurable: a panel of equals tells you
/// an instance is hard, never whether it is *informative*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTier {
    /// A deterministic policy with no model in it. Anything it solves has a surface cue.
    RuleBased,
    Weak,
    Baseline,
    Strong,
}

impl CapabilityTier {
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityTier::RuleBased => "rule_based",
            CapabilityTier::Weak => "weak",
            CapabilityTier::Baseline => "baseline",
            CapabilityTier::Strong => "strong",
        }
    }
}

/// One pilot-panel execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelRun {
    pub instance_id: String,
    pub architecture: String,
    pub tier: CapabilityTier,
    pub passed: bool,
}

impl PanelRun {
    pub fn new(
        instance_id: impl Into<String>,
        architecture: impl Into<String>,
        tier: CapabilityTier,
        passed: bool,
    ) -> Self {
        PanelRun {
            instance_id: instance_id.into(),
            architecture: architecture.into(),
            tier,
            passed,
        }
    }
}

/// What the panel measured, or the fact that it measured nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DifficultyEstimate {
    /// No panel run touched this instance. Categorically distinct from "everyone failed it".
    Unmeasured { reason: String },
    Measured {
        attempts: usize,
        passes: usize,
        success_rate: f64,
        strong_attempts: usize,
        weak_attempts: usize,
        /// Strong-tier pass rate minus weak-tier pass rate. `None` when one side was never run,
        /// because a difference against nothing is not zero.
        discrimination: Option<f64>,
    },
}

impl DifficultyEstimate {
    /// The measured success rate, or `None`. Deliberately not `success_rate_or_zero`.
    pub fn success_rate(&self) -> Option<f64> {
        match self {
            DifficultyEstimate::Measured { success_rate, .. } => Some(*success_rate),
            DifficultyEstimate::Unmeasured { .. } => None,
        }
    }
}

/// Why an instance's difficulty is or is not useful.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CalibrationVerdict {
    /// Separates the tiers. The only shape that measures capability.
    Discriminating,
    /// Every architecture passed. Carries no information about any of them.
    UniversallyPassed,
    /// Every architecture failed. May be a hard instance or a broken one; 06.13 is explicit that
    /// hardness alone is not quality, so this is a flag for repair, not a trophy.
    UniversallyFailed,
    /// A rule-based or weak policy solved it. Whatever it measures, it is not the capability.
    TrivialCue { solved_by: String },
    /// Easy by design and kept anyway: a safety veto the caller declared.
    SafetyVeto,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstanceCalibration {
    pub instance_id: String,
    pub estimate: DifficultyEstimate,
    pub verdict: CalibrationVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub instances: Vec<InstanceCalibration>,
    pub discriminating: usize,
    pub trivial_cue: usize,
    pub universally_passed: usize,
    pub universally_failed: usize,
    pub unmeasured: usize,
    pub safety_vetoes: usize,
}

impl Calibration {
    /// Instances worth keeping in a primary pack: discriminating, or a declared safety veto.
    pub fn informative(&self) -> Vec<&InstanceCalibration> {
        self.instances
            .iter()
            .filter(|instance| {
                matches!(
                    instance.verdict,
                    CalibrationVerdict::Discriminating | CalibrationVerdict::SafetyVeto
                )
            })
            .collect()
    }
}

/// Calibrates every instance the panel touched, plus every instance it did not.
///
/// `known_instances` exists so that an instance nobody ran still appears in the report as
/// [`CalibrationVerdict::Unmeasured`]. A calibration built only from the runs it happens to have is
/// how an unmeasured instance silently becomes an absent one.
pub fn calibrate(
    runs: &[PanelRun],
    known_instances: &BTreeSet<String>,
    safety_vetoes: &BTreeSet<String>,
) -> Calibration {
    let mut grouped: BTreeMap<&str, Vec<&PanelRun>> = BTreeMap::new();
    for run in runs {
        grouped.entry(run.instance_id.as_str()).or_default().push(run);
    }

    let mut all: BTreeSet<&str> = known_instances.iter().map(String::as_str).collect();
    for id in grouped.keys() {
        all.insert(id);
    }

    let mut instances = Vec::new();
    for id in all {
        let runs = grouped.get(id).cloned().unwrap_or_default();
        if runs.is_empty() {
            instances.push(InstanceCalibration {
                instance_id: id.to_string(),
                estimate: DifficultyEstimate::Unmeasured {
                    reason: "no pilot-panel run executed this instance".to_string(),
                },
                verdict: CalibrationVerdict::Unmeasured,
            });
            continue;
        }

        let attempts = runs.len();
        let passes = runs.iter().filter(|run| run.passed).count();
        let strong: Vec<&&PanelRun> = runs
            .iter()
            .filter(|run| run.tier == CapabilityTier::Strong)
            .collect();
        let weak: Vec<&&PanelRun> = runs
            .iter()
            .filter(|run| {
                matches!(run.tier, CapabilityTier::Weak | CapabilityTier::RuleBased)
            })
            .collect();
        let rate = |group: &[&&PanelRun]| {
            group.iter().filter(|run| run.passed).count() as f64 / group.len() as f64
        };
        let discrimination = if strong.is_empty() || weak.is_empty() {
            None
        } else {
            Some(rate(&strong) - rate(&weak))
        };

        let estimate = DifficultyEstimate::Measured {
            attempts,
            passes,
            success_rate: passes as f64 / attempts as f64,
            strong_attempts: strong.len(),
            weak_attempts: weak.len(),
            discrimination,
        };

        let cue = runs
            .iter()
            .find(|run| {
                run.passed
                    && matches!(run.tier, CapabilityTier::RuleBased | CapabilityTier::Weak)
            })
            .map(|run| run.architecture.clone());

        let verdict = if let Some(solved_by) = cue {
            CalibrationVerdict::TrivialCue { solved_by }
        } else if safety_vetoes.contains(id) {
            CalibrationVerdict::SafetyVeto
        } else if passes == attempts {
            CalibrationVerdict::UniversallyPassed
        } else if passes == 0 {
            CalibrationVerdict::UniversallyFailed
        } else {
            CalibrationVerdict::Discriminating
        };

        instances.push(InstanceCalibration {
            instance_id: id.to_string(),
            estimate,
            verdict,
        });
    }

    let count = |predicate: fn(&CalibrationVerdict) -> bool| {
        instances
            .iter()
            .filter(|instance| predicate(&instance.verdict))
            .count()
    };

    Calibration {
        discriminating: count(|v| matches!(v, CalibrationVerdict::Discriminating)),
        trivial_cue: count(|v| matches!(v, CalibrationVerdict::TrivialCue { .. })),
        universally_passed: count(|v| matches!(v, CalibrationVerdict::UniversallyPassed)),
        universally_failed: count(|v| matches!(v, CalibrationVerdict::UniversallyFailed)),
        unmeasured: count(|v| matches!(v, CalibrationVerdict::Unmeasured)),
        safety_vetoes: count(|v| matches!(v, CalibrationVerdict::SafetyVeto)),
        instances,
    }
}

/// One generated benchmark instance, described only by what determines its diagnostic content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchInstance {
    pub instance_id: String,
    /// Content digest of the parent it descends from. Never the parent's *name*: renaming a parent
    /// must not create a new lineage.
    pub parent_digest: String,
    pub mutation_family: String,
    /// What the instance actually tests: the oracle's verdict and witness contract.
    pub oracle_signature: String,
}

/// The honest denominator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveDiversity {
    /// What a naive report would headline.
    pub instances: usize,
    pub parents: usize,
    pub families: usize,
    pub signatures: usize,
    /// Distinct `(parent, family, signature)` triples.
    pub equivalence_classes: usize,
    /// instances / equivalence_classes. 1.0 means every instance is independent.
    pub inflation_ratio: f64,
    pub caveat: String,
}

impl EffectiveDiversity {
    /// The sentence a report leads with.
    pub fn headline(&self) -> String {
        format!(
            "{} instance(s) from {} parent(s) across {} mutation famil(ies), providing {} \
             independent equivalence class(es) (inflation x{:.2}). Instance count is not benchmark \
             count.",
            self.instances,
            self.parents,
            self.families,
            self.equivalence_classes,
            self.inflation_ratio
        )
    }

    /// The effective sample size 06.13 asks for.
    ///
    /// Identical to [`Self::equivalence_classes`], and named separately only because "effective
    /// sample size" is what a statistics-shaped reader will look for. Reporting the instance count
    /// under this name is precisely the inflation the metric exists to prevent.
    pub fn effective_sample_size(&self) -> usize {
        self.equivalence_classes
    }

    /// Whether a family carries enough independent information to publish as a benchmark.
    ///
    /// The same conservative gate `bioprism_mutation` applies: a family collapsing into two or
    /// fewer classes is a robustness check and should be labelled as one.
    pub fn is_publishable(&self) -> bool {
        self.equivalence_classes >= 3
    }
}

/// Counts independent equivalence classes over a set of instances.
pub fn effective_diversity(instances: &[BenchInstance]) -> EffectiveDiversity {
    let mut parents = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut signatures = BTreeSet::new();
    let mut classes = BTreeSet::new();

    for instance in instances {
        parents.insert(instance.parent_digest.as_str());
        families.insert(instance.mutation_family.as_str());
        signatures.insert(instance.oracle_signature.as_str());
        classes.insert(format!(
            "{}|{}|{}",
            instance.parent_digest, instance.mutation_family, instance.oracle_signature
        ));
    }

    let equivalence_classes = classes.len();
    EffectiveDiversity {
        instances: instances.len(),
        parents: parents.len(),
        families: families.len(),
        signatures: signatures.len(),
        equivalence_classes,
        inflation_ratio: if equivalence_classes == 0 {
            0.0
        } else {
            instances.len() as f64 / equivalence_classes as f64
        },
        caveat: "Equivalence classes are distinct (parent digest, mutation family, oracle \
                 signature) triples, the same definition bioprism_mutation::diversity uses. This \
                 measures independent diagnostic information, not difficulty or realism, and makes \
                 no claim about correlation beyond those three axes."
            .to_string(),
    }
}
