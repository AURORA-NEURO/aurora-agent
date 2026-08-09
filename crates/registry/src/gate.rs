//! The release gate: the check CI runs before a pack becomes a claim.
//!
//! Blueprint 00.09 (release gates), 43.40 (metrics, release gates and the claim ladder), 27.16
//! (release policy) and 40.32 (conformance). Three outcomes, because two are not enough:
//!
//! - **Pass** — the evidence supports the required tier. Carries the diversity headline, because
//!   43.40 requires generated scale to be reported separately from independent parents and the
//!   only reliable way to make that happen is to put the sentence in the pass message.
//! - **Fail** — the pack is a coherent artifact that does not clear the bar. Reasons are named and
//!   remediable; 43.40 is explicit that "a failed gate blocks the corresponding claim, not
//!   necessarily all experimental release".
//! - **Block** — the pack must not enter CI at all. This is the fail-closed path of 10.02
//!   ("fail closed where integrity or safety is affected"): a broken attestation, a quarantined
//!   digest, an instance presented as validated that was never checked, or a diversity collapse.
//!
//! # The two honesty gates
//!
//! Both are blocking, not failing, and both come from the executive summary's refusal to let
//! "one million automatically paraphrased questions equal one million meaningful benchmarks".
//!
//! 1. **A pack claiming N instances must report effective diversity for those N.** If the
//!    diversity report describes a different instance set than the pack carries, the count is
//!    unaccompanied and the pack is blocked — not failed, because a mismatched report is not a
//!    threshold that can be argued about.
//! 2. **A pack collapsing to fewer than three equivalence classes may not be published as a
//!    benchmark.** The floor is `bioprism_mutation::Diversity::is_publishable`. Such a family is a
//!    robustness check and should be labelled as one; the gate refuses to let it be labelled
//!    otherwise.
//!
//! What is *not* gated here: cost, latency, memory, privacy, burden and robustness from 43.40's
//! metric family; subgroup and site behaviour; regression budgets against a previous release. All
//! of those need a run, and this crate only sees the benchmark.

use crate::pack::{BenchmarkPack, PackError};
use crate::tier::{assess, TierPolicy, TrustTier, UnmetRequirement};
use bioprism_prism::Attestation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// The CI policy a pack is gated against.
///
/// `quarantined` is the whole of the policy layer 10.02 describes: a set of digests this CI
/// refuses regardless of tier. Revocation, propagation and federation of that set are not
/// implemented — somebody has to hand this crate the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub required_tier: TrustTier,
    /// Below this many equivalence classes a pack may not be published as a benchmark.
    pub min_equivalence_classes: usize,
    /// Instances per equivalence class. Above this the pack is inflated, not large.
    pub max_inflation_ratio: f64,
    #[serde(default)]
    pub quarantined: BTreeSet<String>,
    pub tiers: TierPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            required_tier: TrustTier::Reviewed,
            min_equivalence_classes: 3,
            max_inflation_ratio: 25.0,
            quarantined: BTreeSet::new(),
            tiers: TierPolicy::default(),
        }
    }
}

impl Policy {
    /// The loosest policy that still enforces the honesty gates: any tier is acceptable, but a
    /// collapsed or unaccounted pack is still blocked. For research use, per 43.40's rule that a
    /// failed gate need not block experimental release.
    pub fn experimental() -> Self {
        Policy {
            required_tier: TrustTier::Exploratory,
            ..Policy::default()
        }
    }
}

/// A named reason the gate did not pass cleanly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum GateFinding {
    AttestationInvalid {
        detail: String,
    },
    Quarantined {
        pack_sha256: String,
    },
    /// The instance count and the diversity report describe different things.
    EffectiveDiversityNotReported {
        instances_carried: usize,
        instances_reported: usize,
    },
    DiversityCollapsed {
        equivalence_classes: usize,
        floor: usize,
    },
    /// An instance whose declared relation was violated, or never checked at all.
    UnvalidatedInstance {
        instance_id: String,
        reason: String,
    },
    OrphanInstance {
        instance_id: String,
        parent_sha256: String,
    },
    TierBelowRequired {
        required: TrustTier,
        earned: TrustTier,
        unmet: Vec<UnmetRequirement>,
    },
    InflationAboveCeiling {
        ratio: f64,
        ceiling: f64,
    },
    Undigestible {
        detail: String,
    },
}

impl GateFinding {
    /// Whether this finding forbids the pack from being used at all, as opposed to forbidding the
    /// claim it wanted to make.
    pub fn blocking(&self) -> bool {
        match self {
            GateFinding::AttestationInvalid { .. }
            | GateFinding::Quarantined { .. }
            | GateFinding::EffectiveDiversityNotReported { .. }
            | GateFinding::DiversityCollapsed { .. }
            | GateFinding::UnvalidatedInstance { .. }
            | GateFinding::OrphanInstance { .. }
            | GateFinding::Undigestible { .. } => true,
            GateFinding::TierBelowRequired { .. } | GateFinding::InflationAboveCeiling { .. } => {
                false
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            GateFinding::AttestationInvalid { detail } => {
                format!("attestation does not verify: {detail}")
            }
            GateFinding::Quarantined { pack_sha256 } => {
                format!("digest {pack_sha256} is quarantined by policy")
            }
            GateFinding::EffectiveDiversityNotReported {
                instances_carried,
                instances_reported,
            } => format!(
                "pack carries {instances_carried} instance(s) but reports effective diversity for \
                 {instances_reported}; an instance count without its effective diversity is not a \
                 publishable number"
            ),
            GateFinding::DiversityCollapsed {
                equivalence_classes,
                floor,
            } => format!(
                "{equivalence_classes} equivalence class(es), floor {floor}: this family is a \
                 robustness check and may not be published as a benchmark"
            ),
            GateFinding::UnvalidatedInstance {
                instance_id,
                reason,
            } => format!("instance {instance_id} is not validated: {reason}"),
            GateFinding::OrphanInstance {
                instance_id,
                parent_sha256,
            } => format!("instance {instance_id} descends from absent parent {parent_sha256}"),
            GateFinding::TierBelowRequired {
                required,
                earned,
                unmet,
            } => format!(
                "policy requires {required}, evidence earns {earned}; {} requirement(s) unmet",
                unmet.len()
            ),
            GateFinding::InflationAboveCeiling { ratio, ceiling } => format!(
                "{ratio:.2} instances per equivalence class exceeds the ceiling of {ceiling:.2}"
            ),
            GateFinding::Undigestible { detail } => {
                format!("pack cannot be content-addressed: {detail}")
            }
        }
    }
}

/// The gate's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GateOutcome {
    Pass {
        tier: TrustTier,
        /// The sentence a report must lead with, from `Diversity::headline`.
        headline: String,
        notes: Vec<String>,
    },
    Fail {
        tier: TrustTier,
        findings: Vec<GateFinding>,
    },
    Block {
        findings: Vec<GateFinding>,
    },
}

impl GateOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, GateOutcome::Pass { .. })
    }

    pub fn is_block(&self) -> bool {
        matches!(self, GateOutcome::Block { .. })
    }

    pub fn findings(&self) -> &[GateFinding] {
        match self {
            GateOutcome::Pass { .. } => &[],
            GateOutcome::Fail { findings, .. } | GateOutcome::Block { findings } => findings,
        }
    }

    /// 0 pass, 1 fail, 2 block. Distinct so a CI job can treat a blocked artifact differently
    /// from a claim that missed its bar.
    pub fn exit_code(&self) -> i32 {
        match self {
            GateOutcome::Pass { .. } => 0,
            GateOutcome::Fail { .. } => 1,
            GateOutcome::Block { .. } => 2,
        }
    }

    /// A plain-text report, one finding per line.
    pub fn report(&self) -> String {
        match self {
            GateOutcome::Pass {
                tier,
                headline,
                notes,
            } => {
                let mut lines = vec![format!("PASS at {tier}"), headline.clone()];
                lines.extend(notes.iter().cloned());
                lines.join("\n")
            }
            GateOutcome::Fail { tier, findings } => {
                let mut lines = vec![format!("FAIL (evidence earns {tier})")];
                lines.extend(findings.iter().map(GateFinding::describe));
                lines.join("\n")
            }
            GateOutcome::Block { findings } => {
                let mut lines = vec!["BLOCK".to_string()];
                lines.extend(findings.iter().map(GateFinding::describe));
                lines.join("\n")
            }
        }
    }
}

/// Gates a pack for CI.
///
/// Blocking findings are collected first and short-circuit the rest: there is no point reporting
/// that a pack missed the reviewed tier when its attestation does not verify, because nothing it
/// says about itself is evidence.
pub fn gate(pack: &BenchmarkPack, policy: &Policy) -> GateOutcome {
    let mut blocking = Vec::new();

    match pack.self_attestation() {
        Attestation::Valid => {}
        Attestation::Mismatch {
            claimed,
            recomputed,
        } => blocking.push(GateFinding::AttestationInvalid {
            detail: format!("claims {claimed}, hashes to {recomputed}"),
        }),
        Attestation::Malformed(detail) => blocking.push(GateFinding::AttestationInvalid { detail }),
    }

    match pack.digest() {
        Ok(digest) if policy.quarantined.contains(digest.as_str()) => {
            blocking.push(GateFinding::Quarantined {
                pack_sha256: digest.as_str().to_string(),
            });
        }
        Ok(_) => {}
        Err(error) => blocking.push(GateFinding::Undigestible {
            detail: error.to_string(),
        }),
    }

    if pack.diversity.instances != pack.instances.len() {
        blocking.push(GateFinding::EffectiveDiversityNotReported {
            instances_carried: pack.instances.len(),
            instances_reported: pack.diversity.instances,
        });
    }

    if pack.diversity.equivalence_classes < policy.min_equivalence_classes {
        blocking.push(GateFinding::DiversityCollapsed {
            equivalence_classes: pack.diversity.equivalence_classes,
            floor: policy.min_equivalence_classes,
        });
    }

    for entry in pack.unvalidated() {
        blocking.push(GateFinding::UnvalidatedInstance {
            instance_id: entry.id().to_string(),
            reason: entry.postcondition.reason(),
        });
    }

    for entry in pack.orphans() {
        blocking.push(GateFinding::OrphanInstance {
            instance_id: entry.id().to_string(),
            parent_sha256: entry.parent_sha256.clone(),
        });
    }

    if !blocking.is_empty() {
        return GateOutcome::Block { findings: blocking };
    }

    let assessment = assess(pack, &policy.tiers);
    let mut failing = Vec::new();

    if assessment.earned < policy.required_tier {
        failing.push(GateFinding::TierBelowRequired {
            required: policy.required_tier,
            earned: assessment.earned,
            unmet: assessment.unmet_for(policy.required_tier),
        });
    }

    let ratio = pack.diversity.inflation_ratio;
    if ratio.is_finite() && ratio > policy.max_inflation_ratio {
        failing.push(GateFinding::InflationAboveCeiling {
            ratio,
            ceiling: policy.max_inflation_ratio,
        });
    }

    if !failing.is_empty() {
        return GateOutcome::Fail {
            tier: assessment.earned,
            findings: failing,
        };
    }

    GateOutcome::Pass {
        tier: assessment.earned,
        headline: pack.diversity.headline(),
        notes: vec![
            format!(
                "{} independent parent world(s); {} of {} attempted mutation(s) were accepted \
                 (yield {:.0}%).",
                pack.parents.len(),
                pack.yield_ledger.accepted,
                pack.yield_ledger.attempted,
                pack.yield_ledger.yield_rate() * 100.0
            ),
            "Passing this gate establishes that the pack's evidence is internally consistent and \
             sufficient for the tier claimed. It is not a judgment that the benchmark is \
             scientifically sound (40.32: conformance is not trust or accuracy)."
                .to_string(),
        ],
    }
}

/// Gates an attested document read from disk, verifying it before anything else.
///
/// This is the third-party entry point: the caller has bytes they did not produce, and the first
/// question is whether the bytes are what they claim to be.
pub fn gate_document(document: &Value, policy: &Policy) -> GateOutcome {
    match BenchmarkPack::from_attested(document) {
        Ok(pack) => gate(&pack, policy),
        Err(PackError::AttestationFailed(detail)) => GateOutcome::Block {
            findings: vec![GateFinding::AttestationInvalid { detail }],
        },
        Err(error) => GateOutcome::Block {
            findings: vec![GateFinding::Undigestible {
                detail: error.to_string(),
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_and_failing_findings_are_distinguished() {
        assert!(GateFinding::DiversityCollapsed {
            equivalence_classes: 1,
            floor: 3
        }
        .blocking());
        assert!(!GateFinding::InflationAboveCeiling {
            ratio: 30.0,
            ceiling: 25.0
        }
        .blocking());
        assert_eq!(GateOutcome::Block { findings: vec![] }.exit_code(), 2);
    }
}
