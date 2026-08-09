//! Distributed execution: placement, fencing, and what actually ran.
//!
//! Blueprint 35.13. `bioprism-factory` owns the queue — jobs, workers, leases, heartbeats, and the
//! recovery branch on idempotency that makes an ambiguous lease expiry safe. Its own documentation
//! names what it leaves out: "no backpressure model, no fair-share scheduling across tenants, and
//! no distributed lease fencing". This module is that remainder, and it is deliberately not a
//! second queue. There is no `enqueue`, no `claim`, no worker loop and no ordering policy.
//! [`place`] answers one question about one proposed assignment, and answers it as a predicate.
//!
//! The mandatory-safety-stratum rule that section 35 also states — a scheduler may not skip safety
//! strata for information gain — is `bioprism_scale::adaptive`'s, enforced there by refusing a
//! budget below the mandatory floor. It is not restated here.
//!
//! ## Three refusals and one thing that is merely recorded
//!
//! [`place`] refuses an undeclared resource class, delegating to `factory`'s own
//! `WorkerCapability::can_run` rather than reimplementing capability matching. It refuses an
//! unattested worker for restricted work: 35.13 lists worker attestation as a required component,
//! and 35.02's constraint that descendants inherit access restrictions is worthless if the
//! restricted artifact is handed to a machine nobody vouched for. And it refuses a worker sitting
//! in the same trust domain as the oracle that will judge the job — section 35's own scale
//! constraint is "oracle independence is preserved in distributed and federated execution", which
//! read as a predicate over a placement is exactly this, and it exists nowhere else in the
//! workspace.
//!
//! Non-local placement is *not* refused. Data-local scheduling is a preference, not a safety rule,
//! so a placement away from the data succeeds and carries [`Placement::transfer_bytes`]. What must
//! never happen is that it happens silently.
//!
//! ## Fencing is the write-side check a lease cannot make
//!
//! A lease says a worker holds an attempt. It cannot say that a worker whose lease *expired* is
//! actually dead — that ambiguity is the whole reason `factory` branches on idempotency. A
//! [`FenceRegistry`] issues a monotone token per job, and [`FenceRegistry::admit`] rejects a commit
//! bearing a superseded one. That is what makes a resurrected worker's late write rejectable rather
//! than merely unlikely. It is not a distributed lock: nothing here coordinates anything, and in a
//! real deployment the registry is whatever the durable store's compare-and-set is.
//!
//! ## Duplicate execution is two different findings, never one number
//!
//! 35.13 lists duplicate execution as an operational metric. A duplicate commit on an idempotent
//! job is wasted compute; a duplicate commit on a non-idempotent one is a double-applied external
//! effect. [`DuplicateReport`] keeps them in separate fields and offers no total, because a summed
//! duplicate count lets a hundred wasted re-indexes hide one double-charged run.
//!
//! ## Not implemented
//!
//! In-memory, single-process, no network, no real workers, no clock. Everything here is a decision
//! *about* distribution rather than an instance of it. Throughput, tail latency and budget
//! adherence — three of 35.13's six metrics — are properties of a running system and are absent
//! rather than approximated.

use crate::error::PlacementError;
use bioprism_factory::{Idempotency, Job, WorkerCapability};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A site, tenant, or operator boundary. Two things in one domain can collude by accident.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrustDomain(pub String);

impl TrustDomain {
    pub fn new(name: impl Into<String>) -> Self {
        TrustDomain(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where data physically is. An opaque label; this crate has no topology.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Locale(pub String);

impl Locale {
    pub fn new(name: impl Into<String>) -> Self {
        Locale(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether anything vouches for what this worker is running.
///
/// Two states, no `Default`. An unattested worker is not a worker with a bad measurement; it is one
/// nobody checked, and the workspace's position is that those must not share a representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "attestation", rename_all = "snake_case")]
pub enum Attestation {
    /// A measurement of the worker's image, and who vouched for it.
    Attested {
        measurement: ContentHash,
        vouched_by: String,
    },
    /// Nobody checked.
    Unattested,
}

impl Attestation {
    pub fn is_attested(&self) -> bool {
        matches!(self, Attestation::Attested { .. })
    }
}

/// How restricted the work's inputs are.
///
/// Ordered, with `Open` least restricted. The blueprint names access tiers without enumerating
/// them, so the three levels are **illustrative**; what carries the rule is that
/// [`AccessTier::requires_attestation`] is true above the open tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessTier {
    Open,
    Restricted,
    /// Data may not leave its locale at all.
    Enclave,
}

impl AccessTier {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessTier::Open => "open",
            AccessTier::Restricted => "restricted",
            AccessTier::Enclave => "enclave",
        }
    }

    pub fn requires_attestation(self) -> bool {
        self > AccessTier::Open
    }

    /// Whether work at this tier may run anywhere but where its data lives.
    pub fn permits_transfer(self) -> bool {
        self < AccessTier::Enclave
    }
}

/// A worker, as a placement decision sees it.
///
/// Wraps `bioprism_factory::WorkerCapability` rather than restating resource classes or lease
/// durations. The three fields added here are the ones a single-process queue has no reason to
/// carry and a distributed one cannot work without.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerProfile {
    pub capability: WorkerCapability,
    pub domain: TrustDomain,
    pub locale: Locale,
    pub attestation: Attestation,
}

impl WorkerProfile {
    pub fn new(
        capability: WorkerCapability,
        domain: TrustDomain,
        locale: Locale,
        attestation: Attestation,
    ) -> Self {
        WorkerProfile {
            capability,
            domain,
            locale,
            attestation,
        }
    }
}

/// What a job needs from wherever it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRequest {
    /// Where the job's inputs live.
    pub data_locale: Locale,
    pub access_tier: AccessTier,
    /// The trust domain hosting the oracle that will judge this job's output.
    pub oracle_domain: TrustDomain,
    /// Input size, used only to record the cost of a non-local placement.
    pub input_bytes: u64,
}

/// An accepted assignment, and what accepting it cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub job: String,
    pub worker: String,
    pub data_local: bool,
    /// Zero for a local placement. Non-zero means the data moved, and the report says so.
    pub transfer_bytes: u64,
    pub access_tier: AccessTier,
    pub worker_domain: TrustDomain,
}

/// Decides whether `worker` may run `job` under `request`.
///
/// The three refusals are documented at the module level. Everything else — priority, ordering,
/// fairness, backpressure — is not this function's business and is not silently decided here.
pub fn place(
    job: &Job,
    request: &WorkRequest,
    worker: &WorkerProfile,
) -> Result<Placement, PlacementError> {
    if !worker.capability.can_run(job.resource_class) {
        return Err(PlacementError::ClassNotDeclared {
            worker: worker.capability.worker_id.clone(),
            class: format!("{:?}", job.resource_class).to_lowercase(),
        });
    }
    if request.access_tier.requires_attestation() && !worker.attestation.is_attested() {
        return Err(PlacementError::UnattestedWorker {
            worker: worker.capability.worker_id.clone(),
            tier: request.access_tier.as_str(),
        });
    }
    if worker.domain == request.oracle_domain {
        return Err(PlacementError::OracleDomainCollision {
            worker: worker.capability.worker_id.clone(),
            domain: worker.domain.0.clone(),
            job: job.id.clone(),
        });
    }

    let data_local = worker.locale == request.data_locale;
    if !data_local && !request.access_tier.permits_transfer() {
        return Err(PlacementError::EnclaveTransfer {
            job: job.id.clone(),
            worker: worker.capability.worker_id.clone(),
            data_locale: request.data_locale.0.clone(),
            worker_locale: worker.locale.0.clone(),
        });
    }

    Ok(Placement {
        job: job.id.clone(),
        worker: worker.capability.worker_id.clone(),
        data_local,
        transfer_bytes: if data_local { 0 } else { request.input_bytes },
        access_tier: request.access_tier,
        worker_domain: worker.domain.clone(),
    })
}

/// A monotone write token for one job.
///
/// Opaque and not constructible outside this module: the only source is
/// [`FenceRegistry::issue`], so a worker cannot mint a fresher token by guessing a larger number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Fence(u64);

impl Fence {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Issues fences and rejects commits bearing superseded ones.
///
/// In-memory and single-process. In a real deployment this is a durable compare-and-set; the logic
/// is the same and the storage is not this crate's.
#[derive(Debug, Clone, Default)]
pub struct FenceRegistry {
    current: BTreeMap<String, u64>,
}

impl FenceRegistry {
    pub fn new() -> Self {
        FenceRegistry::default()
    }

    /// Issues the next fence for `job`, superseding any earlier one.
    pub fn issue(&mut self, job: &str) -> Fence {
        let entry = self.current.entry(job.to_string()).or_insert(0);
        *entry += 1;
        Fence(*entry)
    }

    pub fn current(&self, job: &str) -> Option<Fence> {
        self.current.get(job).copied().map(Fence)
    }

    /// Accepts a commit only under the current fence.
    ///
    /// A worker resurrected after its lease was reassigned still holds its old fence, and this is
    /// where that write is rejected. `factory`'s recovery decides whether the *job* may be retried;
    /// this decides whether a specific late write may land.
    pub fn admit(&self, job: &str, presented: Fence) -> Result<(), PlacementError> {
        let current = self
            .current
            .get(job)
            .copied()
            .ok_or_else(|| PlacementError::NoFenceIssued(job.to_string()))?;
        if presented.0 != current {
            return Err(PlacementError::StaleFence {
                job: job.to_string(),
                presented: presented.0,
                current,
            });
        }
        Ok(())
    }
}

/// One admitted commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub job: String,
    pub item: String,
    pub fence: u64,
    pub idempotency: Idempotency,
}

/// Duplicate execution, kept in two separate columns on purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DuplicateReport {
    /// Repeat commits on jobs safe to re-run. Wasted compute.
    pub wasted_idempotent_commits: usize,
    /// Repeat commits on jobs whose effects cannot be repeated safely. Incidents.
    pub repeated_effect_incidents: usize,
    /// Repeat commits on compensable jobs, which are incidents unless compensation ran.
    pub compensable_repeat_commits: usize,
    /// Job ids with more than one admitted commit.
    pub jobs_committed_more_than_once: Vec<String>,
}

impl DuplicateReport {
    /// Deliberately absent: a total.
    ///
    /// Summing the three columns produces a number in which a hundred wasted re-indexes outweigh
    /// one double-charged external effect. If a caller wants that number they must write the
    /// addition themselves and own it.
    pub fn has_incidents(&self) -> bool {
        self.repeated_effect_incidents > 0 || self.compensable_repeat_commits > 0
    }
}

/// What actually ran, recorded at commit time.
///
/// Section 35 repeats the constraint in every module: "generated instances are enumerable; results
/// name the subset actually executed." This is the record of that subset, and
/// [`crate::executed`] is what turns it into a figure a report may carry.
#[derive(Debug, Clone, Default)]
pub struct ExecutionLedger {
    commits: Vec<Commit>,
}

impl ExecutionLedger {
    pub fn new() -> Self {
        ExecutionLedger::default()
    }

    /// Records a commit, admitting it only under the current fence.
    pub fn commit(
        &mut self,
        registry: &FenceRegistry,
        job: &str,
        item: &str,
        fence: Fence,
        idempotency: Idempotency,
    ) -> Result<(), PlacementError> {
        registry.admit(job, fence)?;
        self.commits.push(Commit {
            job: job.to_string(),
            item: item.to_string(),
            fence: fence.get(),
            idempotency,
        });
        Ok(())
    }

    pub fn commits(&self) -> &[Commit] {
        &self.commits
    }

    /// Item ids that committed at least once, in id order and without repeats.
    pub fn executed_items(&self) -> Vec<String> {
        let mut items: Vec<String> = self
            .commits
            .iter()
            .map(|commit| commit.item.clone())
            .collect();
        items.sort();
        items.dedup();
        items
    }

    pub fn duplicates(&self) -> DuplicateReport {
        let mut counts: BTreeMap<&str, (usize, Idempotency)> = BTreeMap::new();
        for commit in &self.commits {
            let entry = counts
                .entry(commit.job.as_str())
                .or_insert((0, commit.idempotency));
            entry.0 += 1;
        }
        let mut report = DuplicateReport::default();
        for (job, (count, idempotency)) in counts {
            if count <= 1 {
                continue;
            }
            report.jobs_committed_more_than_once.push(job.to_string());
            let repeats = count - 1;
            match idempotency {
                Idempotency::Idempotent => report.wasted_idempotent_commits += repeats,
                Idempotency::NonIdempotent => report.repeated_effect_incidents += repeats,
                Idempotency::Compensable => report.compensable_repeat_commits += repeats,
            }
        }
        report
    }
}
