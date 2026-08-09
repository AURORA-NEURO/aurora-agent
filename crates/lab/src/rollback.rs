//! Deployment state, checkpoints, and rollback.
//!
//! Blueprint 09.11: *"Architecture, router, model resolution, policy, and component artifacts are
//! immutable versions. Rollback restores a complete known-good bundle, not selected files."* The
//! second clause is why [`Deployment::rollback`] takes a [`Checkpoint`] and not a list of changes:
//! there is no API here for restoring part of a configuration, because a partially restored
//! configuration is a configuration nobody has ever measured.
//!
//! # What a rollback cannot restore
//!
//! A checkpoint captures two things: which configuration was deployed, and how much exposure each
//! holdout had accumulated at the time. Rolling back restores the first exactly. It cannot restore
//! the second, and this crate refuses to pretend otherwise.
//!
//! The reason is the whole argument of [`crate::holdout`]. Between the checkpoint and the rollback,
//! configurations were scored, compared and chosen using those holdouts. That happened. If a
//! rollback rewound the exposure ledger, the sequence "measure on the holdout, see a bad number,
//! roll back, measure again" would yield two clean measurements from a surface that had already
//! told you its answer — and the second one would be the one that got published. So exposure is
//! append-only across a rollback, and [`RollbackReceipt`] states exactly what was burned in the
//! interval and can never be recovered.
//!
//! That is the honest reading of "restore the configuration and the exposure state": the
//! configuration is restored, the exposure state is *reconciled and reported*, and the difference
//! between the two is the receipt.
//!
//! # Not implemented, deliberately
//!
//! No automatic rollback trigger. 09.11 asks for "automatic rollback on safety, reliability, cost,
//! or latency thresholds"; that needs live telemetry and a clock, and this crate has neither.
//! [`Deployment::rollback`] is a function a monitor would call, not the monitor. No canary, no
//! shadow routing, no traffic split, no manual stop control — all four are runtime concerns and
//! none of them is here.

use crate::error::RollbackError;
use crate::holdout::{ExposureWatermark, HoldoutId, HoldoutLedger};
use crate::space::{ArchitectureSpace, ConfigurationId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A known-good point: one configuration, and every holdout's exposure position at the time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub label: String,
    pub configuration: ConfigurationId,
    pub exposure: BTreeMap<HoldoutId, ExposureWatermark>,
}

/// What one holdout accrued between a checkpoint and the rollback that followed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureSinceCheckpoint {
    pub holdout: HoldoutId,
    /// Exposure events appended since the checkpoint.
    pub events_since: usize,
    /// Configurations first burned in the interval. None of these can be cleanly measured again,
    /// whatever the deployment was rolled back to.
    pub configurations_burned_since: Vec<ConfigurationId>,
    /// Whether the interval's queries retired the holdout outright.
    pub retired_in_interval: bool,
}

/// The result of a rollback, stated so a reader can see the asymmetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackReceipt {
    pub from: ConfigurationId,
    pub restored: ConfigurationId,
    /// The exposure that could not be rolled back, per holdout. Empty means the interval spent
    /// nothing, which is the only case in which a rollback is genuinely a return to the past.
    pub exposure_retained: Vec<ExposureSinceCheckpoint>,
    /// Holdouts registered after the checkpoint was taken, and therefore outside its coverage.
    pub outside_checkpoint: Vec<HoldoutId>,
}

impl RollbackReceipt {
    /// Whether the rollback restored the world, rather than only the configuration.
    ///
    /// False whenever any holdout moved in the interval. A caller that wants to report "we rolled
    /// back cleanly" has to check this, and it will usually say no.
    pub fn is_complete_restoration(&self) -> bool {
        self.exposure_retained
            .iter()
            .all(|entry| entry.events_since == 0)
            && self.outside_checkpoint.is_empty()
    }

    /// Every configuration burned in the interval, across all holdouts.
    pub fn permanently_burned(&self) -> Vec<(HoldoutId, ConfigurationId)> {
        self.exposure_retained
            .iter()
            .flat_map(|entry| {
                entry
                    .configurations_burned_since
                    .iter()
                    .map(|configuration| (entry.holdout.clone(), configuration.clone()))
            })
            .collect()
    }
}

/// One entry in a deployment's own history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum DeploymentEvent {
    Promoted {
        from: ConfigurationId,
        to: ConfigurationId,
        rationale: String,
    },
    RolledBack {
        from: ConfigurationId,
        to: ConfigurationId,
        checkpoint: String,
    },
}

/// The deployed configuration, the bundles it can be rolled back to, and the access ledger.
///
/// These three live in one object because 09.11's rollback restores a *bundle*, and a bundle whose
/// exposure history is stored somewhere else can be restored while its history is not consulted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deployment {
    pub space: ArchitectureSpace,
    pub holdouts: HoldoutLedger,
    current: ConfigurationId,
    history: Vec<DeploymentEvent>,
}

impl Deployment {
    /// Starts a deployment at an already-registered configuration.
    pub fn new(
        space: ArchitectureSpace,
        holdouts: HoldoutLedger,
        current: ConfigurationId,
    ) -> Result<Self, RollbackError> {
        if !space.contains(&current) {
            return Err(RollbackError::BundleMissing(current.to_string()));
        }
        Ok(Deployment {
            space,
            holdouts,
            current,
            history: Vec::new(),
        })
    }

    pub fn current(&self) -> &ConfigurationId {
        &self.current
    }

    pub fn history(&self) -> &[DeploymentEvent] {
        &self.history
    }

    /// Takes a checkpoint of the current configuration and every holdout's exposure position.
    pub fn checkpoint(&self, label: impl Into<String>) -> Checkpoint {
        Checkpoint {
            label: label.into(),
            configuration: self.current.clone(),
            exposure: self.holdouts.watermarks(),
        }
    }

    /// Promotes a configuration, recording the selection against the holdout that justified it.
    ///
    /// `selected_using` is `Option` because a promotion can be justified by something other than a
    /// holdout — a safety review, a dependency bump, a revert. Passing `None` says so explicitly
    /// and burns nothing. Passing a holdout burns it for this configuration forever, which is the
    /// price of having used it to decide.
    pub fn promote(
        &mut self,
        configuration: &ConfigurationId,
        selected_using: Option<&HoldoutId>,
        rationale: impl Into<String>,
    ) -> Result<(), RollbackError> {
        if !self.space.contains(configuration) {
            return Err(RollbackError::BundleMissing(configuration.to_string()));
        }
        let rationale = rationale.into();
        if let Some(holdout) = selected_using {
            self.holdouts
                .record_selection(holdout, configuration, &rationale)
                .map_err(|_| RollbackError::BundleMissing(holdout.to_string()))?;
        }
        self.history.push(DeploymentEvent::Promoted {
            from: self.current.clone(),
            to: configuration.clone(),
            rationale,
        });
        self.current = configuration.clone();
        Ok(())
    }

    /// Restores the checkpoint's whole bundle and reports the exposure that survived it.
    pub fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<RollbackReceipt, RollbackError> {
        if !self.space.contains(&checkpoint.configuration) {
            return Err(RollbackError::BundleMissing(
                checkpoint.configuration.to_string(),
            ));
        }

        let present = self.holdouts.ids();
        let missing: Vec<HoldoutId> = checkpoint
            .exposure
            .keys()
            .filter(|id| !present.contains(id))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(RollbackError::HoldoutSetChanged {
                covered: checkpoint
                    .exposure
                    .keys()
                    .map(HoldoutId::to_string)
                    .collect(),
                present: present.iter().map(HoldoutId::to_string).collect(),
            });
        }

        let mut exposure_retained = Vec::new();
        for (id, watermark) in &checkpoint.exposure {
            let holdout = self
                .holdouts
                .get(id)
                .ok_or_else(|| RollbackError::BundleMissing(id.to_string()))?;
            let current = holdout.watermark();
            if watermark.0 > current.0 {
                return Err(RollbackError::WatermarkAhead {
                    holdout: id.to_string(),
                    watermark: watermark.0,
                    current: current.0,
                });
            }
            let since = &holdout.exposure()[watermark.0..];
            let mut burned: Vec<ConfigurationId> = Vec::new();
            for event in since {
                if event.kind.consumes_query_budget() && !burned.contains(&event.configuration) {
                    burned.push(event.configuration.clone());
                }
            }
            let retired_in_interval = holdout.is_retired() && !since.is_empty();
            exposure_retained.push(ExposureSinceCheckpoint {
                holdout: id.clone(),
                events_since: since.len(),
                configurations_burned_since: burned,
                retired_in_interval,
            });
        }

        let outside_checkpoint: Vec<HoldoutId> = present
            .into_iter()
            .filter(|id| !checkpoint.exposure.contains_key(id))
            .collect();

        for id in checkpoint.exposure.keys() {
            if let Some(holdout) = self.holdouts.get_mut(id) {
                holdout.record_rollback(&checkpoint.configuration);
            }
        }

        let from = self.current.clone();
        self.history.push(DeploymentEvent::RolledBack {
            from: from.clone(),
            to: checkpoint.configuration.clone(),
            checkpoint: checkpoint.label.clone(),
        });
        self.current = checkpoint.configuration.clone();

        Ok(RollbackReceipt {
            from,
            restored: checkpoint.configuration.clone(),
            exposure_retained,
            outside_checkpoint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holdout::{Holdout, Partition};
    use crate::space::{CandidateArchitecture, ComponentKind, ComponentSpec};

    fn minimal(id: &str) -> CandidateArchitecture {
        CandidateArchitecture::new(id)
            .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
            .with_component(ComponentSpec::new("run", ComponentKind::Executor))
            .with_component(ComponentSpec::new("stop", ComponentKind::Terminator))
    }

    fn deployment() -> Deployment {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v2").derived_from("v1")).unwrap();
        let mut holdouts = HoldoutLedger::new();
        holdouts
            .register(Holdout::new(
                "private-a",
                Partition::RotatingPrivateCertification,
                8,
            ))
            .unwrap();
        Deployment::new(space, holdouts, ConfigurationId::new("v1")).unwrap()
    }

    #[test]
    fn a_rollback_restores_the_configuration_but_never_un_burns_a_holdout() {
        let mut deployment = deployment();
        let checkpoint = deployment.checkpoint("before-v2");
        let holdout = HoldoutId::new("private-a");
        deployment
            .promote(&ConfigurationId::new("v2"), Some(&holdout), "beat v1")
            .unwrap();

        let receipt = deployment.rollback(&checkpoint).unwrap();
        assert_eq!(deployment.current(), &ConfigurationId::new("v1"));
        assert!(!receipt.is_complete_restoration());
        assert_eq!(
            receipt.permanently_burned(),
            vec![(holdout.clone(), ConfigurationId::new("v2"))]
        );
        assert!(deployment
            .holdouts
            .get(&holdout)
            .unwrap()
            .is_burned_for(&ConfigurationId::new("v2")));
    }

    #[test]
    fn measuring_after_a_rollback_does_not_get_a_second_clean_shot_at_the_holdout() {
        let mut deployment = deployment();
        let checkpoint = deployment.checkpoint("before-v2");
        let holdout = HoldoutId::new("private-a");
        deployment
            .promote(&ConfigurationId::new("v2"), Some(&holdout), "beat v1")
            .unwrap();
        deployment.rollback(&checkpoint).unwrap();

        let space = deployment.space.clone();
        assert!(deployment
            .holdouts
            .measure(&holdout, &space, &ConfigurationId::new("v2"), "rate", 0.9)
            .is_err());
    }

    #[test]
    fn a_rollback_over_an_interval_that_spent_nothing_is_a_complete_restoration() {
        let mut deployment = deployment();
        let checkpoint = deployment.checkpoint("clean");
        deployment
            .promote(&ConfigurationId::new("v2"), None, "dependency bump")
            .unwrap();
        let receipt = deployment.rollback(&checkpoint).unwrap();
        assert!(receipt.is_complete_restoration());
        assert!(receipt.permanently_burned().is_empty());
    }

    #[test]
    fn a_promotion_justified_by_something_other_than_a_holdout_burns_nothing() {
        let mut deployment = deployment();
        deployment
            .promote(&ConfigurationId::new("v2"), None, "reverting a bad revert")
            .unwrap();
        assert_eq!(
            deployment
                .holdouts
                .get(&HoldoutId::new("private-a"))
                .unwrap()
                .queries_used(),
            0
        );
    }

    #[test]
    fn a_checkpoint_naming_an_unregistered_bundle_cannot_be_rolled_back_to() {
        let mut deployment = deployment();
        let checkpoint = Checkpoint {
            label: "ghost".to_string(),
            configuration: ConfigurationId::new("v9"),
            exposure: BTreeMap::new(),
        };
        assert_eq!(
            deployment.rollback(&checkpoint),
            Err(RollbackError::BundleMissing("v9".to_string()))
        );
    }

    #[test]
    fn a_holdout_registered_after_the_checkpoint_is_reported_as_outside_its_coverage() {
        let mut deployment = deployment();
        let checkpoint = deployment.checkpoint("before");
        deployment
            .holdouts
            .register(Holdout::new("public", Partition::PublicEvaluation, 4))
            .unwrap();
        let receipt = deployment.rollback(&checkpoint).unwrap();
        assert_eq!(receipt.outside_checkpoint, vec![HoldoutId::new("public")]);
        assert!(!receipt.is_complete_restoration());
    }

    #[test]
    fn deployment_history_records_the_rollback_rather_than_erasing_the_promotion() {
        let mut deployment = deployment();
        let checkpoint = deployment.checkpoint("before-v2");
        deployment
            .promote(&ConfigurationId::new("v2"), None, "beat v1")
            .unwrap();
        deployment.rollback(&checkpoint).unwrap();
        assert_eq!(deployment.history().len(), 2);
        assert!(matches!(
            deployment.history()[0],
            DeploymentEvent::Promoted { .. }
        ));
    }
}
