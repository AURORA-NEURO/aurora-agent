//! Distributed compute and placement (12.14): matching work to workers that described
//! themselves.
//!
//! 12.14 has five subsections and all five are predicates, which makes it the only one of the
//! seven implemented end to end. Worker declaration and placement are [`Fleet::place`]; sharding
//! and the deterministic seed map are [`plan_shards`]; stragglers are [`AttemptRecord`]; worker
//! trust is [`TaskLease`] and [`ResultEvidence`].
//!
//! # The word the module is built on is "declaration"
//!
//! 12.14's first subsection is titled *Worker declaration* and lists nine things a worker states
//! about itself, ending with "provider credentials by reference". Every one of those is the
//! worker's own account. The section's fifth subsection then asks for "remote attestation where
//! justified", which concedes that the account may be wrong, and never says what a placement
//! decision made from an unattested declaration is worth.
//!
//! Here a fleet holds [`Attested<WorkerDeclaration>`], and a placement inherits
//! [`Basis::weakest_of`] the declarations it relied on. A job placed on a worker that the
//! platform probed carries [`Basis::FirstHand`]; the identical placement onto a worker that
//! merely claimed the same GPU carries [`Basis::Declared`]. They are different values, they do
//! not compare equal, and [`PlacementPolicy::require_verified_declaration`] can refuse the second
//! without having to re-derive which facts were trusted.
//!
//! # An unattributable timeout is not an agent failure
//!
//! 12.14 says "timeouts remain evaluation outcomes when caused by the agent, not infrastructure",
//! and gives no rule for a timeout whose cause is unknown. The comfortable default is to charge
//! the agent, because that keeps the platform's own numbers clean — which is precisely why
//! [`attribute_timeout`] refuses it. Unless the evidence positively establishes both that the
//! infrastructure was healthy and that the agent was running, the result is
//! [`Attribution::Unclassified`], which 12.12's budget policy then charges to the platform.
//!
//! # There is no `Verified`
//!
//! 12.14 wants an "output digest/signature" checked against an "immutable input manifest".
//! Checking one means reading the bytes, and this crate has no bytes. [`ResultEvidence`]
//! therefore has two variants — the worker supplied a digest, or it did not — and no third
//! variant asserting the digest was confirmed. A type that could express a verification this
//! crate cannot perform is a type somebody will eventually construct.
//!
//! # Not implemented
//!
//! No scheduler, no queue, no execution, no network, no signatures, no attestation, no
//! preemption handling, no autoscaling, no fairness accounting across tenants. [`Fleet::place`]
//! returns a decision; nothing acts on it. Speculative execution is modelled as a record of
//! attempts, not as concurrency — there are no threads here. Resource quantities (memory, disk,
//! accelerator counts) are not modelled at all: 12.14 lists them and a bin-packing implementation
//! would be the interesting part, so its absence is a real gap rather than a simplification.

use crate::basis::{Attested, Basis, Coverage};
use crate::error::{check_name, PlacementError};
use crate::provider::{Capability, IsolationStrength, ProviderId, Region, ThreatLevel, TrustDomain};
use crate::slo::{Attribution, Confidence, FailureDomain};
use bioprism_ids::ContentHash;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

macro_rules! placement_id {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, PlacementError> {
                let value = value.into();
                if !check_name(&value) {
                    return Err(PlacementError::MalformedField {
                        field: $field,
                        value,
                    });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = PlacementError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

placement_id!(WorkerId, "worker id");
placement_id!(TaskId, "task id");
placement_id!(UnitId, "unit id");

/// What a worker says about itself.
///
/// Held inside an [`Attested`] everywhere it is used, never bare, so there is no path where a
/// declaration is read without the record of who said it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDeclaration {
    pub id: WorkerId,
    pub provider: ProviderId,
    pub trust_domain: TrustDomain,
    pub isolation: IsolationStrength,
    pub capabilities: BTreeSet<Capability>,
    pub data_regions: BTreeSet<Region>,
    /// Cache or checkpoint keys this worker already holds, for 12.14's affinity rule.
    pub warm_keys: BTreeSet<String>,
}

/// What a job needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRequirements {
    pub task: TaskId,
    pub needs: BTreeSet<Capability>,
    /// Regions the job's data may be processed in. Empty means unconstrained.
    pub permitted_regions: BTreeSet<Region>,
    pub trust_domain: TrustDomain,
    pub threat: ThreatLevel,
    /// 12.14: "sensitive jobs use approved worker pools."
    pub sensitive: bool,
    pub affinity_key: Option<String>,
}

/// The rules the scheduler applies on top of the requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementPolicy {
    /// Refuse a placement whose worker facts are merely declared.
    pub require_verified_declaration: bool,
    /// Trust domains a sensitive job may run in.
    pub approved_pools: BTreeSet<TrustDomain>,
}

/// Why a worker was chosen.
///
/// Recorded rather than recomputed, because 12.14 asks the scheduler to record the assignment and
/// an assignment without its reasons cannot be audited after the fleet has changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchReason {
    CapabilitiesSatisfied,
    RegionPermitted,
    TrustDomainMatched,
    IsolationAdequate,
    PoolApproved,
    WarmForAffinityKey,
}

/// Why no worker was chosen.
///
/// A value, not an error: the scheduler did its job and the answer is no. Every variant names the
/// specific constraint, so "no capacity" — the answer that tells an operator nothing — is not
/// expressible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    /// The fleet is empty.
    NoWorkers,
    /// No worker declared this capability.
    CapabilityUnavailable { capability: String },
    /// Workers exist with the capability, but none in a permitted region.
    NoPermittedRegion { permitted: Vec<String> },
    /// No worker in the job's trust domain.
    TrustDomainUnavailable { trust_domain: String },
    /// Candidates exist but none isolates strongly enough for this threat level.
    IsolationInadequate { required: String, threat: String },
    /// A sensitive job whose only candidates are outside the approved pools.
    PoolNotApproved { trust_domain: String },
    /// Candidates exist but the policy requires verified declarations and none has one.
    DeclarationsUnverified { candidates: Vec<String> },
}

/// A placement, or a refusal.
///
/// There is no fallback variant and no `Option<WorkerId>`. A scheduler that could return "none,
/// but here is one anyway" is the shape 12.14's placement rules exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PlacementDecision {
    Placed {
        worker: WorkerId,
        basis: Basis,
        reasons: BTreeSet<MatchReason>,
    },
    Refused {
        refusal: Refusal,
    },
}

impl PlacementDecision {
    pub fn worker(&self) -> Option<&WorkerId> {
        match self {
            PlacementDecision::Placed { worker, .. } => Some(worker),
            PlacementDecision::Refused { .. } => None,
        }
    }

    pub fn basis(&self) -> Option<&Basis> {
        match self {
            PlacementDecision::Placed { basis, .. } => Some(basis),
            PlacementDecision::Refused { .. } => None,
        }
    }
}

/// The declared workers available to a scheduler.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fleet {
    workers: BTreeMap<WorkerId, Attested<WorkerDeclaration>>,
}

impl Fleet {
    pub fn new() -> Self {
        Fleet::default()
    }

    /// Records a worker's own account of itself.
    pub fn declare(
        &mut self,
        declaration: WorkerDeclaration,
        by: crate::basis::PartyId,
        at: Epoch,
    ) -> Result<(), PlacementError> {
        self.insert(Attested::declared(declaration, by, at))
    }

    /// Records facts the platform probed for itself.
    pub fn probe(
        &mut self,
        declaration: WorkerDeclaration,
        at: Epoch,
    ) -> Result<(), PlacementError> {
        self.insert(Attested::first_hand(declaration, at))
    }

    fn insert(&mut self, record: Attested<WorkerDeclaration>) -> Result<(), PlacementError> {
        let id = record.value().id.clone();
        if self.workers.contains_key(&id) {
            return Err(PlacementError::DuplicateWorker {
                worker: id.to_string(),
            });
        }
        self.workers.insert(id, record);
        Ok(())
    }

    pub fn get(&self, id: &WorkerId) -> Option<&Attested<WorkerDeclaration>> {
        self.workers.get(id)
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    /// Chooses a worker, or says exactly which constraint eliminated the last candidate.
    ///
    /// Filters run in the order capability, region, trust domain, pool approval, isolation,
    /// declaration basis. The order determines which refusal is reported when several apply, and
    /// it runs from the most mechanical to the most consequential so the reported reason is the
    /// one an operator can least easily fix by adding hardware.
    ///
    /// Among survivors, a worker warm for the affinity key wins; otherwise the lowest worker id.
    /// Both tie-breaks are total and depend on nothing outside the arguments, so two runs of the
    /// same pipeline place identically.
    pub fn place(&self, job: &JobRequirements, policy: &PlacementPolicy) -> PlacementDecision {
        if self.workers.is_empty() {
            return PlacementDecision::Refused {
                refusal: Refusal::NoWorkers,
            };
        }

        let mut candidates: Vec<&Attested<WorkerDeclaration>> = self.workers.values().collect();

        for capability in &job.needs {
            let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
                .iter()
                .copied()
                .filter(|record| record.value().capabilities.contains(capability))
                .collect();
            if remaining.is_empty() {
                return PlacementDecision::Refused {
                    refusal: Refusal::CapabilityUnavailable {
                        capability: capability.to_string(),
                    },
                };
            }
            candidates = remaining;
        }

        if !job.permitted_regions.is_empty() {
            let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
                .iter()
                .copied()
                .filter(|record| {
                    record
                        .value()
                        .data_regions
                        .iter()
                        .any(|region| job.permitted_regions.contains(region))
                })
                .collect();
            if remaining.is_empty() {
                return PlacementDecision::Refused {
                    refusal: Refusal::NoPermittedRegion {
                        permitted: job
                            .permitted_regions
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                    },
                };
            }
            candidates = remaining;
        }

        let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
            .iter()
            .copied()
            .filter(|record| record.value().trust_domain == job.trust_domain)
            .collect();
        if remaining.is_empty() {
            return PlacementDecision::Refused {
                refusal: Refusal::TrustDomainUnavailable {
                    trust_domain: job.trust_domain.to_string(),
                },
            };
        }
        candidates = remaining;

        if job.sensitive {
            let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
                .iter()
                .copied()
                .filter(|record| policy.approved_pools.contains(&record.value().trust_domain))
                .collect();
            if remaining.is_empty() {
                return PlacementDecision::Refused {
                    refusal: Refusal::PoolNotApproved {
                        trust_domain: job.trust_domain.to_string(),
                    },
                };
            }
            candidates = remaining;
        }

        let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
            .iter()
            .copied()
            .filter(|record| record.value().isolation.adequate_for(job.threat))
            .collect();
        if remaining.is_empty() {
            return PlacementDecision::Refused {
                refusal: Refusal::IsolationInadequate {
                    required: job.threat.minimum_isolation().name().to_string(),
                    threat: job.threat.name().to_string(),
                },
            };
        }
        candidates = remaining;

        if policy.require_verified_declaration {
            let remaining: Vec<&Attested<WorkerDeclaration>> = candidates
                .iter()
                .copied()
                .filter(|record| record.basis().is_first_hand())
                .collect();
            if remaining.is_empty() {
                return PlacementDecision::Refused {
                    refusal: Refusal::DeclarationsUnverified {
                        candidates: candidates
                            .iter()
                            .map(|record| record.value().id.to_string())
                            .collect(),
                    },
                };
            }
            candidates = remaining;
        }

        let chosen = job
            .affinity_key
            .as_ref()
            .and_then(|key| {
                candidates
                    .iter()
                    .copied()
                    .find(|record| record.value().warm_keys.contains(key))
            })
            .unwrap_or(candidates[0]);

        let mut reasons: BTreeSet<MatchReason> = [
            MatchReason::CapabilitiesSatisfied,
            MatchReason::TrustDomainMatched,
            MatchReason::IsolationAdequate,
        ]
        .into_iter()
        .collect();
        if !job.permitted_regions.is_empty() {
            reasons.insert(MatchReason::RegionPermitted);
        }
        if job.sensitive {
            reasons.insert(MatchReason::PoolApproved);
        }
        if job
            .affinity_key
            .as_ref()
            .is_some_and(|key| chosen.value().warm_keys.contains(key))
        {
            reasons.insert(MatchReason::WarmForAffinityKey);
        }

        PlacementDecision::Placed {
            worker: chosen.value().id.clone(),
            basis: Basis::weakest_of([chosen.basis()]),
            reasons,
        }
    }
}

/// A deterministic seed for one unit of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seed(u64);

impl Seed {
    pub fn value(self) -> u64 {
        self.0
    }
}

/// The scheduler's record of what went where and with what seed.
///
/// 12.14 asks for both in one sentence — "the scheduler records assignment and deterministic seed
/// map" — and they are one struct here for the same reason: a seed map without its assignment
/// cannot be replayed onto the same shards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardPlan {
    pub shards: BTreeMap<u64, BTreeSet<UnitId>>,
    pub seeds: BTreeMap<UnitId, Seed>,
}

impl ShardPlan {
    /// Every unit that appears anywhere in the plan.
    pub fn assigned(&self) -> BTreeSet<&UnitId> {
        self.shards.values().flatten().collect()
    }

    /// The shard a unit landed on.
    pub fn shard_of(&self, unit: &UnitId) -> Option<u64> {
        self.shards
            .iter()
            .find(|(_, units)| units.contains(unit))
            .map(|(index, _)| *index)
    }
}

/// Shards units across `shards` buckets and derives a seed for each.
///
/// A unit is a parent or cell boundary, never smaller. 12.14 requires sharding at those
/// boundaries "to preserve clustered statistics", so this function takes whole units and has no
/// way to split one — the invariant is in the signature rather than in a check.
///
/// Bucket assignment is by the unit's own derived seed rather than by position, so adding a unit
/// does not reshuffle the others, and the seed is a digest of the unit id and the caller's salt
/// so two runs with the same salt agree exactly.
pub fn plan_shards(
    units: impl IntoIterator<Item = UnitId>,
    shards: u64,
    salt: &str,
) -> Result<ShardPlan, PlacementError> {
    let units: BTreeSet<UnitId> = units.into_iter().collect();
    if shards == 0 {
        return Err(PlacementError::ImpossibleShardCount {
            shards,
            units: units.len() as u64,
        });
    }
    let mut plan = ShardPlan {
        shards: (0..shards).map(|index| (index, BTreeSet::new())).collect(),
        seeds: BTreeMap::new(),
    };
    for unit in units {
        let seed = derive_seed(&unit, salt)?;
        let index = seed.value() % shards;
        plan.shards.entry(index).or_default().insert(unit.clone());
        plan.seeds.insert(unit, seed);
    }
    Ok(plan)
}

fn derive_seed(unit: &UnitId, salt: &str) -> Result<Seed, PlacementError> {
    let digest = ContentHash::of_value(&json!({ "unit": unit.as_str(), "salt": salt }))?;
    let hex = digest.as_str();
    let prefix = hex.get(hex.len().saturating_sub(16)..).unwrap_or(hex);
    let value = u64::from_str_radix(prefix, 16).map_err(|error| PlacementError::Seed {
        detail: error.to_string(),
    })?;
    Ok(Seed(value))
}

/// Whether a task may be run twice.
///
/// A plain boolean would be enough for the check and is not enough for the reader: 12.14 permits
/// speculation "only for safe/idempotent tasks", and a field called `idempotent: bool` on a task
/// invites a caller to set it because the task usually works twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeatability {
    /// Running it twice produces the same effect as running it once.
    Idempotent,
    /// Running it twice may produce a second effect.
    SideEffecting,
}

/// The record of a speculative race.
///
/// The losing attempts are a field, not a log line. 12.14 says to "record winning/losing
/// attempts", and a straggler mitigation that discards the losers cannot afterwards answer
/// whether the two workers agreed — which is the only question that makes speculation safe to
/// have run at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    task: TaskId,
    winner: WorkerId,
    losers: Vec<WorkerId>,
}

impl AttemptRecord {
    /// Opens a race, refusing a task that is not safe to run twice.
    pub fn speculate(
        task: TaskId,
        repeatability: Repeatability,
        winner: WorkerId,
        losers: impl IntoIterator<Item = WorkerId>,
    ) -> Result<Self, PlacementError> {
        if repeatability == Repeatability::SideEffecting {
            return Err(PlacementError::SpeculationUnsafe {
                task: task.to_string(),
            });
        }
        Ok(AttemptRecord {
            task,
            winner,
            losers: losers.into_iter().collect(),
        })
    }

    pub fn task(&self) -> &TaskId {
        &self.task
    }

    pub fn winner(&self) -> &WorkerId {
        &self.winner
    }

    pub fn losers(&self) -> &[WorkerId] {
        &self.losers
    }

    pub fn attempts(&self) -> usize {
        1 + self.losers.len()
    }
}

/// What is known about why something ran out of time.
///
/// Both fields are `Option` because "we did not check" is a real state and the whole attribution
/// rule turns on it. A `bool` defaulting to `true` for infrastructure health would make every
/// unmonitored timeout an agent failure, silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutEvidence {
    /// Whether the platform confirmed its own components were healthy through the window.
    pub infrastructure_healthy: Option<bool>,
    /// Whether the agent was observed doing work.
    pub agent_made_progress: Option<bool>,
}

/// Attributes a timeout, refusing to guess.
///
/// Returns [`Attribution::Unclassified`] whenever the evidence does not positively establish the
/// cause. That is the expensive direction — 12.12's default budget policy charges unclassified
/// failures to the platform — and it is the only one that does not let an unmonitored platform
/// improve its own availability figure by declining to look.
pub fn attribute_timeout(evidence: &TimeoutEvidence) -> Attribution {
    match (evidence.infrastructure_healthy, evidence.agent_made_progress) {
        (Some(false), _) => Attribution::classified(
            FailureDomain::PlatformInfrastructure,
            ["infrastructure reported unhealthy during the window".to_string()],
            Confidence::Certain,
        )
        .unwrap_or_else(|_| Attribution::unclassified("evidence could not be recorded")),
        (Some(true), Some(true)) => Attribution::classified(
            FailureDomain::Agent,
            [
                "infrastructure healthy through the window".to_string(),
                "agent observed making progress".to_string(),
            ],
            Confidence::Probable,
        )
        .unwrap_or_else(|_| Attribution::unclassified("evidence could not be recorded")),
        (Some(true), Some(false)) => Attribution::unclassified(
            "infrastructure healthy but the agent was never observed running",
        ),
        _ => Attribution::unclassified("infrastructure health was not established"),
    }
}

/// A short-lived grant of the right to run one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLease {
    pub task: TaskId,
    pub worker: WorkerId,
    pub issued_at: Epoch,
    pub expires_at: Epoch,
    /// The digest of the inputs the worker was given.
    pub input_manifest: ContentHash,
}

/// What a returned result came with.
///
/// Two variants, and no third. Confirming a digest means hashing the bytes it commits to, this
/// crate holds no bytes, and so there is no `Verified`. A caller that needs one has to go
/// somewhere that can read the artifact — which is the correct outcome and is the reason the gap
/// is expressed as a missing variant rather than a `TODO`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evidence", rename_all = "snake_case")]
pub enum ResultEvidence {
    /// The worker returned a digest of its output. Nothing here has checked it.
    DigestClaimed { by: WorkerId, digest: ContentHash },
    /// The worker returned no digest at all.
    Unattested { by: WorkerId, reason: String },
}

impl ResultEvidence {
    /// True when a digest was supplied. Named for what it tests: not `is_verified`.
    pub fn has_claimed_digest(&self) -> bool {
        matches!(self, ResultEvidence::DigestClaimed { .. })
    }
}

/// Accepts a result against its lease.
///
/// The expiry check comes first and is an error rather than a value: a result arriving after its
/// lease expired may have been produced by a worker the scheduler has already replaced, and
/// treating that as merely unattested would let it into the record.
pub fn accept_result(
    lease: &TaskLease,
    evidence: ResultEvidence,
    arrived_at: Epoch,
) -> Result<Attested<ResultEvidence>, PlacementError> {
    if arrived_at > lease.expires_at {
        return Err(PlacementError::LeaseExpired {
            task: lease.task.to_string(),
            expires_at: lease.expires_at.tick(),
            arrived_at: arrived_at.tick(),
        });
    }
    let basis = match &evidence {
        ResultEvidence::DigestClaimed { by, .. } => Basis::Declared {
            by: crate::basis::PartyId::parse(by.as_str()).map_err(|_| {
                PlacementError::MalformedField {
                    field: "worker id",
                    value: by.to_string(),
                }
            })?,
            declared_at: arrived_at,
        },
        ResultEvidence::Unattested { reason, .. } => Basis::Unobserved {
            reason: reason.clone(),
        },
    };
    Ok(Attested::new(
        evidence,
        basis,
        Coverage::Complete { observed: 1 },
    ))
}
