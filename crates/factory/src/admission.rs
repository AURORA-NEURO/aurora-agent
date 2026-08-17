//! Explicit queue admission and fair-share bounds.
//!
//! A lifecycle store can be correct and still be unsafe to expose if it accepts unbounded work.
//! [`QueueAdmissionPolicy`] keeps total jobs, active leases, and per-resource-class occupancy
//! bounded. The policy is supplied at enqueue time so a deployment can choose limits without
//! making the persisted job state pretend that a local process-wide policy is distributed truth.

use crate::error::FactoryError;
use crate::job::{Job, ResourceClass};
use crate::store::JobStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Backpressure and class fair-share limits for one queue controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueAdmissionPolicy {
    pub max_jobs: usize,
    pub max_active_leases: usize,
    #[serde(default)]
    pub max_jobs_by_class: BTreeMap<ResourceClass, usize>,
    #[serde(default)]
    pub max_active_leases_by_class: BTreeMap<ResourceClass, usize>,
}

impl QueueAdmissionPolicy {
    pub fn new(max_jobs: usize, max_active_leases: usize) -> Self {
        Self {
            max_jobs,
            max_active_leases,
            max_jobs_by_class: BTreeMap::new(),
            max_active_leases_by_class: BTreeMap::new(),
        }
    }

    pub fn with_resource_class_limit(
        mut self,
        class: ResourceClass,
        max_jobs: usize,
        max_active_leases: usize,
    ) -> Self {
        self.max_jobs_by_class.insert(class, max_jobs);
        self.max_active_leases_by_class
            .insert(class, max_active_leases);
        self
    }

    pub fn validate(&self) -> Result<(), FactoryError> {
        if self.max_jobs == 0 || self.max_active_leases == 0 {
            return Err(FactoryError::InvalidAdmissionPolicy {
                reason: "max_jobs and max_active_leases must both be positive".into(),
            });
        }
        if self.max_active_leases > self.max_jobs {
            return Err(FactoryError::InvalidAdmissionPolicy {
                reason: "max_active_leases cannot exceed max_jobs".into(),
            });
        }
        for (class, limit) in &self.max_jobs_by_class {
            if *limit == 0 {
                return Err(FactoryError::InvalidAdmissionPolicy {
                    reason: format!("max_jobs_by_class[{class:?}] must be positive"),
                });
            }
        }
        for (class, limit) in &self.max_active_leases_by_class {
            if *limit == 0 {
                return Err(FactoryError::InvalidAdmissionPolicy {
                    reason: format!("max_active_leases_by_class[{class:?}] must be positive"),
                });
            }
            if *limit > self.max_active_leases {
                return Err(FactoryError::InvalidAdmissionPolicy {
                    reason: format!(
                        "max_active_leases_by_class[{class:?}] cannot exceed max_active_leases"
                    ),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_enqueue(&self, store: &JobStore, job: &Job) -> Result<(), FactoryError> {
        self.validate()?;
        if store.len() >= self.max_jobs {
            return Err(FactoryError::AdmissionLimit {
                dimension: "total_jobs".into(),
                limit: self.max_jobs,
                observed: store.len(),
            });
        }
        let class_jobs = store
            .counts_by_class()
            .get(&job.resource_class)
            .copied()
            .unwrap_or(0);
        if let Some(limit) = self.max_jobs_by_class.get(&job.resource_class) {
            if class_jobs >= *limit {
                return Err(FactoryError::AdmissionLimit {
                    dimension: format!("jobs_by_class:{:?}", job.resource_class),
                    limit: *limit,
                    observed: class_jobs,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn check_lease(
        &self,
        store: &JobStore,
        class: ResourceClass,
    ) -> Result<(), FactoryError> {
        self.validate()?;
        let active_leases = store.active_lease_count();
        if active_leases >= self.max_active_leases {
            return Err(FactoryError::AdmissionLimit {
                dimension: "active_leases".into(),
                limit: self.max_active_leases,
                observed: active_leases,
            });
        }
        let active_class_leases = store.active_lease_counts_by_class();
        let active_class = active_class_leases.get(&class).copied().unwrap_or(0);
        if let Some(limit) = self.max_active_leases_by_class.get(&class) {
            if active_class >= *limit {
                return Err(FactoryError::AdmissionLimit {
                    dimension: format!("active_leases_by_class:{class:?}"),
                    limit: *limit,
                    observed: active_class,
                });
            }
        }
        Ok(())
    }
}

impl Default for QueueAdmissionPolicy {
    fn default() -> Self {
        Self::new(100_000, 1_024)
    }
}
