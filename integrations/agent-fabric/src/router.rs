//! Agent registry and fair capability routing.
//!
//! Routing answers one question: *which* registered agent, of those advertising every required
//! capability and currently able to run work, should get the next dispatch? The answer is
//! deterministic round-robin within the task's shard preference order — a per-(shard,
//! capability) cursor walks candidates in registration order, so eligible agents in a lane are
//! chosen strictly fairly (counts differ by at most one while eligibility is stable). No
//! candidate is a real answer (`None`), distinct from "all busy" or "quota exhausted", which the
//! scheduler reports separately; collapsing those would make an unroutable task look throttled.

use crate::capability::{Capability, CapabilitySet};
use crate::ids::{AgentId, ShardId};
use crate::shard;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterError {
    UnknownAgent,
}

impl fmt::Display for RouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAgent => write!(f, "agent is not registered"),
        }
    }
}

impl std::error::Error for RouterError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentState {
    /// Eligible for dispatch.
    Active,
    /// Draining: finishes current work, takes no new work.
    Draining,
    /// Down: assumed dead until explicitly reactivated. Leases it held expire normally.
    Down,
}

#[derive(Clone, Debug)]
pub struct AgentRecord {
    pub name: String,
    pub caps: CapabilitySet,
    pub state: AgentState,
}

#[derive(Debug, Default)]
pub struct Router {
    shard_count: u64,
    agents: BTreeMap<AgentId, AgentRecord>,
    agent_shard: BTreeMap<AgentId, ShardId>,
    by_primary_cap: BTreeMap<Capability, Vec<AgentId>>,
    next_agent: u64,
    cursors: BTreeMap<(ShardId, Capability), usize>,
}

impl Router {
    pub fn new(shard_count: u64) -> Self {
        assert!(
            shard_count > 0 && shard_count <= 4096,
            "shard count must be in 1..=4096"
        );
        Self {
            shard_count,
            ..Default::default()
        }
    }

    pub fn shard_count(&self) -> u64 {
        self.shard_count
    }

    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Registers an agent and places it on a shard derived from its name via rendezvous hashing,
    /// so placement is stable across identical registrations and spreads without coordination.
    /// Registration order determines id assignment and therefore candidate order — both are
    /// deterministic given the same call sequence.
    pub fn register(&mut self, name: &str, caps: CapabilitySet) -> AgentId {
        assert!(
            !caps.is_empty(),
            "an agent advertising nothing can never be routed to"
        );
        self.next_agent += 1;
        let id = AgentId::new(self.next_agent);
        let key = crate::ids::mix64(u64::from_be_bytes(
            crate::digest::sha256(name.as_bytes()).as_bytes()[..8]
                .try_into()
                .expect("8 bytes"),
        ));
        let placed = shard::home_shard(self.shard_count, key);
        self.agents.insert(
            id,
            AgentRecord {
                name: name.to_string(),
                caps,
                state: AgentState::Active,
            },
        );
        self.agent_shard.insert(id, placed);
        let primary = self.agents[&id].caps.primary().expect("nonempty").clone();
        self.by_primary_cap.entry(primary).or_default().push(id);
        id
    }

    pub fn record(&self, id: AgentId) -> Option<&AgentRecord> {
        self.agents.get(&id)
    }

    pub fn shard_of(&self, id: AgentId) -> Option<ShardId> {
        self.agent_shard.get(&id).copied()
    }

    pub fn set_state(&mut self, id: AgentId, state: AgentState) -> Result<(), RouterError> {
        match self.agents.get_mut(&id) {
            Some(rec) => {
                rec.state = state;
                Ok(())
            }
            None => Err(RouterError::UnknownAgent),
        }
    }

    pub fn state_of(&self, id: AgentId) -> Option<AgentState> {
        self.agents.get(&id).map(|r| r.state)
    }

    /// Agents whose capability set covers `caps` AND whose state is Active, in registration
    /// order. The set routing actually picks from.
    pub fn capable(&self, caps: &CapabilitySet) -> Vec<AgentId> {
        let primary = match caps.primary() {
            Some(p) => p,
            None => return Vec::new(),
        };
        self.by_primary_cap
            .get(primary)
            .map(|candidates| {
                candidates
                    .iter()
                    .copied()
                    .filter(|id| {
                        self.agents
                            .get(id)
                            .is_some_and(|r| r.state == AgentState::Active && r.caps.covers(caps))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Picks the next agent for a task requiring `caps`, walking the task's shard preference and
    /// round-robining within each shard's eligible set, skipping busy agents. Advances only the
    /// cursors of lanes it actually consumed from, so fairness holds per lane.
    pub fn pick(
        &mut self,
        caps: &CapabilitySet,
        shard_preference: &[ShardId],
        busy: &BTreeSet<AgentId>,
    ) -> Option<AgentId> {
        let eligible = self.capable(caps);
        if eligible.is_empty() {
            return None;
        }
        let primary = caps.primary().cloned().expect("checked nonempty");
        for pref in shard_preference {
            let mut in_shard: Vec<AgentId> = eligible
                .iter()
                .copied()
                .filter(|id| self.agent_shard.get(id) == Some(pref))
                .collect();
            if in_shard.is_empty() {
                continue;
            }
            // eligible() returns registration order; keep that order explicit for the walk.
            in_shard.sort_unstable();
            let cursor = self.cursors.entry((*pref, primary.clone())).or_insert(0);
            let n = in_shard.len();
            for step in 0..n {
                let idx = (*cursor + step) % n;
                let cand = in_shard[idx];
                if !busy.contains(&cand) {
                    *cursor = (idx + 1) % n;
                    return Some(cand);
                }
            }
        }
        None
    }

    pub fn counts_by_state(&self) -> (usize, usize, usize) {
        let mut active = 0;
        let mut draining = 0;
        let mut down = 0;
        for r in self.agents.values() {
            match r.state {
                AgentState::Active => active += 1,
                AgentState::Draining => draining += 1,
                AgentState::Down => down += 1,
            }
        }
        (active, draining, down)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute() -> CapabilitySet {
        CapabilitySet::one(Capability::parse("compute").expect("cap"))
    }

    #[test]
    fn strict_round_robin_spreads_a_lane_within_one_dispatch_of_even() {
        let mut r = Router::new(1);
        let mut ids = Vec::new();
        for i in 0..8 {
            ids.push(r.register(&format!("w{i}"), compute()));
        }
        let busy = BTreeSet::new();
        let mut picks = Vec::new();
        for _ in 0..16 {
            picks.push(
                r.pick(&compute(), &[ShardId::new(0)], &busy)
                    .expect("route"),
            );
        }
        let mut counts: BTreeMap<AgentId, usize> = BTreeMap::new();
        for p in picks {
            *counts.entry(p).or_default() += 1;
        }
        assert_eq!(counts.len(), 8, "every agent participates");
        assert!(
            counts.values().all(|&c| c == 2),
            "exactly even over a multiple: {counts:?}"
        );
    }

    #[test]
    fn busy_agents_are_skipped_without_losing_their_turn_in_the_rotation() {
        let mut r = Router::new(1);
        let a = r.register("a", compute());
        let b = r.register("b", compute());
        let mut busy = BTreeSet::new();
        busy.insert(a);
        assert_eq!(r.pick(&compute(), &[ShardId::new(0)], &busy), Some(b));
        busy.remove(&a);
        // Next pick continues after b's position, wrapping to a.
        assert_eq!(
            r.pick(&compute(), &[ShardId::new(0)], &BTreeSet::new()),
            Some(a)
        );
    }

    #[test]
    fn no_active_candidate_is_none_and_distinct_from_all_busy() {
        let mut r = Router::new(1);
        assert_eq!(
            r.pick(&compute(), &[ShardId::new(0)], &BTreeSet::new()),
            None
        );
        let a = r.register("a", compute());
        r.set_state(a, AgentState::Down).expect("exists");
        assert_eq!(
            r.pick(&compute(), &[ShardId::new(0)], &BTreeSet::new()),
            None
        );
    }

    #[test]
    fn draining_agents_finish_out_of_the_pool_immediately() {
        let mut r = Router::new(1);
        let a = r.register("a", compute());
        r.set_state(a, AgentState::Draining).expect("exists");
        assert!(r.capable(&compute()).is_empty());
    }

    #[test]
    fn multi_capability_tasks_require_every_declared_capability() {
        let mut r = Router::new(1);
        let _gpu_only = r.register(
            "g",
            CapabilitySet::one(Capability::parse("gpu").expect("cap")),
        );
        let mut need_both = vec![
            Capability::parse("gpu").expect("cap"),
            Capability::parse("net").expect("cap"),
        ];
        need_both.sort();
        let both = CapabilitySet::from_caps(need_both);
        assert!(r.capable(&both).is_empty());
    }

    #[test]
    fn placement_lands_on_the_rendezvous_home_shard_for_the_name() {
        let mut r = Router::new(4);
        let a = r.register("stable-name", compute());
        let expected = crate::shard::home_shard(
            4,
            crate::ids::mix64(u64::from_be_bytes(
                crate::digest::sha256(b"stable-name").as_bytes()[..8]
                    .try_into()
                    .expect("8 bytes"),
            )),
        );
        assert_eq!(r.shard_of(a), Some(expected));
    }
}
