//! Bounded protocol model checking: deadlock, liveness and orphan analysis.
//!
//! Blueprint 23.45. Typed messages do not stop a legal conversation from stalling forever or
//! terminating with a message nobody ever read, so the choreography is translated into a finite
//! transition system and searched.
//!
//! # The rule this module exists to enforce
//!
//! *"No deadlock was found within the bound"* and *"no deadlock exists"* are different answers,
//! and [`Verdict`] gives them different shapes: [`Verdict::Holds`] is a proof over an exhaustively
//! explored state space, [`Verdict::Inconclusive`] is an admission that the search stopped early
//! and names which bound stopped it. There is no accessor that collapses the two, and
//! [`Verdict::is_proof`] is true only for the first. The same distinction appears in
//! [`bioprism_section::InfluenceClass`] — `Zero` means checked and provably irrelevant, `Unknown`
//! means nobody looked — so [`ModelCheckReport::omissions`] renders an inconclusive verdict as an
//! omission group that cannot support a sufficiency claim.
//!
//! # The model
//!
//! Roles are communicating finite state machines whose states are residual [`LocalType`]s;
//! recursion is unfolded on demand and folds back to a syntactically identical term, so a regular
//! protocol has a finite reachable set with no widening. Communication is asynchronous over
//! per-direction FIFO channels of capacity [`ExplorationBound::channel_capacity`]. Asynchrony is
//! what makes orphan analysis meaningful at all: under rendezvous a message cannot be left
//! unread.
//!
//! When a send is blocked *only* because its channel is full, the exploration has been truncated
//! relative to the unbounded-channel semantics. That is recorded as [`BoundHit::ChannelCapacity`]
//! and the state is never classified as a deadlock, because a queue-full stall is an artifact of
//! the analysis rather than a defect in the protocol.
//!
//! # Soundness of each answer
//!
//! A deadlock, orphan or unexpected-message witness is a reachable state, so finding one inside a
//! truncated search is still sound: the trace replays. A *liveness* violation is the absence of a
//! path to a terminal state, which a truncated search cannot establish — the missing path might
//! be among the states never expanded. Liveness therefore returns [`Verdict::Inconclusive`]
//! whenever the exploration was truncated, even when a candidate violation was seen.
//!
//! # Deliberately not implemented
//!
//! No scheduler, no clock, no network, no fairness assumptions, and no timeouts: 23.45 requires
//! fairness assumptions to be explicit, and the honest way to be explicit about having none is to
//! model none. Authority deadlock, budget starvation, commitment cycles, livelock against a
//! declared progress measure and unreachable compensation are named as failure patterns by 23.45
//! but are not given transition semantics there; grants and budgets belong to the weave kernel
//! (23.13, 23.16) and compensation reachability is checked structurally in [`crate::saga`]
//! instead. Symbolic policy checking, trace-guided exploration and runtime monitor synthesis are
//! out of scope. Automatic repair suggestions are not emitted: 23.45 says the compiler may
//! suggest but not silently apply, and a suggestion no one implemented is worse than none.

use crate::session::{Label, LocalType, Role, WellFormedGlobal};
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// The properties this checker knows how to falsify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    /// Every role waits for a message no role is enabled to send.
    Deadlock,
    /// Some reachable state can no longer reach a terminal state in which every role is done and
    /// every channel is empty.
    Liveness,
    /// The protocol terminates with a message still queued: it was sent and never read.
    OrphanMessage,
    /// The head of a channel carries a label the receiver does not offer.
    UnexpectedMessage,
}

impl Property {
    pub fn as_str(self) -> &'static str {
        match self {
            Property::Deadlock => "deadlock",
            Property::Liveness => "liveness",
            Property::OrphanMessage => "orphan_message",
            Property::UnexpectedMessage => "unexpected_message",
        }
    }
}

/// The explicit exploration bound. Every number here is a reason an answer might be "unknown".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorationBound {
    /// Maximum distinct states to expand.
    pub max_states: usize,
    /// Maximum number of events from the initial state.
    pub max_depth: usize,
    /// Messages that may sit in one direction of one channel at once.
    pub channel_capacity: usize,
}

impl Default for ExplorationBound {
    /// Capacity two, because capacity one truncates even simple two-message-in-flight protocols
    /// and would make [`Verdict::Holds`] almost unreachable; the state and depth limits are large
    /// enough for the protocols a single choreography realistically declares.
    fn default() -> Self {
        ExplorationBound {
            max_states: 20_000,
            max_depth: 400,
            channel_capacity: 2,
        }
    }
}

impl ExplorationBound {
    pub fn new(max_states: usize, max_depth: usize, channel_capacity: usize) -> Self {
        ExplorationBound {
            max_states,
            max_depth,
            channel_capacity,
        }
    }

    /// A deliberately tiny bound, for demonstrating that truncation is reported rather than
    /// rounded down to "no defect found".
    pub fn tiny() -> Self {
        ExplorationBound {
            max_states: 4,
            max_depth: 3,
            channel_capacity: 1,
        }
    }
}

/// Which bound stopped the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundHit {
    MaxStates,
    MaxDepth,
    /// A send was blocked by a full channel, so states reachable with deeper queues were not
    /// visited. This is the usual outcome for a recursive protocol, whose unbounded-channel state
    /// space is infinite.
    ChannelCapacity,
}

impl BoundHit {
    pub fn as_str(self) -> &'static str {
        match self {
            BoundHit::MaxStates => "max_states",
            BoundHit::MaxDepth => "max_depth",
            BoundHit::ChannelCapacity => "channel_capacity",
        }
    }
}

/// One event in a counterexample trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Send { from: Role, to: Role, label: Label },
    Receive { from: Role, to: Role, label: Label },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Send { from, to, label } => write!(f, "{from} sends {label} to {to}"),
            Event::Receive { from, to, label } => write!(f, "{to} receives {label} from {from}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceStep {
    pub index: usize,
    pub event: Event,
}

/// What a role is doing in the state a counterexample reaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RoleStatus {
    Done,
    WaitingFor {
        peer: Role,
        expecting: Vec<Label>,
    },
    ReadyToSend {
        peer: Role,
        offering: Vec<Label>,
    },
    /// A free recursion variable or another term with no transition. Reachable only for
    /// hand-written systems; projection of a well-formed global type never produces one.
    Stuck {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleState {
    pub role: Role,
    pub status: RoleStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelState {
    pub from: Role,
    pub to: Role,
    pub queued: Vec<Label>,
}

/// The role states, grants-free, that 23.45 asks a counterexample to carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSummary {
    pub roles: Vec<RoleState>,
    pub channels: Vec<ChannelState>,
}

/// A replayable witness that a property is false.
///
/// Produced by breadth-first search, so `steps` is a shortest path to the offending state: a
/// counterexample a human is expected to read must not include events that were not needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counterexample {
    pub property: Property,
    pub steps: Vec<TraceStep>,
    pub state: StateSummary,
    pub explanation: String,
}

impl fmt::Display for Counterexample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for step in &self.steps {
            writeln!(f, "state {}: {}", step.index, step.event)?;
        }
        writeln!(f, "{}", self.property.as_str().to_uppercase())?;
        write!(f, "{}", self.explanation)
    }
}

/// The answer to one property question.
///
/// Three shapes, not two, and the third is the whole point. A checker that returned `bool` would
/// have to encode "I stopped looking" as one of "true" or "false", and both are lies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// The property holds over the entire reachable state space, which was explored exhaustively.
    Holds { states_explored: usize },
    /// The property is false, and here is the trace.
    Fails(Box<Counterexample>),
    /// The search stopped at a bound. Nothing is claimed about the states beyond it.
    Inconclusive {
        bound: ExplorationBound,
        hit: BoundHit,
        states_explored: usize,
    },
}

impl Verdict {
    /// True only for [`Verdict::Holds`]. An inconclusive verdict is not a weak yes.
    pub fn is_proof(&self) -> bool {
        matches!(self, Verdict::Holds { .. })
    }

    /// True only for [`Verdict::Fails`].
    pub fn is_refuted(&self) -> bool {
        matches!(self, Verdict::Fails(_))
    }

    pub fn counterexample(&self) -> Option<&Counterexample> {
        match self {
            Verdict::Fails(counterexample) => Some(counterexample),
            _ => None,
        }
    }

    /// How much of the decision this verdict leaves unaccounted for, in the vocabulary the
    /// context certificate already uses: a proof omits nothing, a refutation is a fact, and an
    /// inconclusive answer is [`InfluenceClass::Unknown`] — nobody checked.
    pub fn influence(&self) -> InfluenceClass {
        match self {
            Verdict::Holds { .. } | Verdict::Fails(_) => InfluenceClass::Zero,
            Verdict::Inconclusive { .. } => InfluenceClass::Unknown,
        }
    }

    pub fn states_explored(&self) -> usize {
        match self {
            Verdict::Holds { states_explored } => *states_explored,
            Verdict::Inconclusive {
                states_explored, ..
            } => *states_explored,
            Verdict::Fails(_) => 0,
        }
    }
}

/// The four verdicts, plus the bound they were computed under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCheckReport {
    pub bound: ExplorationBound,
    pub states_explored: usize,
    pub truncated: Option<BoundHit>,
    pub deadlock: Verdict,
    pub liveness: Verdict,
    pub orphan: Verdict,
    pub unexpected_message: Verdict,
}

impl ModelCheckReport {
    pub fn verdicts(&self) -> [(Property, &Verdict); 4] {
        [
            (Property::Deadlock, &self.deadlock),
            (Property::Liveness, &self.liveness),
            (Property::OrphanMessage, &self.orphan),
            (Property::UnexpectedMessage, &self.unexpected_message),
        ]
    }

    /// True when every property was proved, not merely left unrefuted.
    pub fn all_proved(&self) -> bool {
        self.verdicts().iter().all(|(_, v)| v.is_proof())
    }

    pub fn refutations(&self) -> Vec<&Counterexample> {
        self.verdicts()
            .iter()
            .filter_map(|(_, v)| v.counterexample())
            .collect()
    }

    /// The report as an omission manifest: one group per property that was not settled.
    ///
    /// A manifest that [`OmissionManifest::supports_sufficiency_claim`] accepts is exactly a
    /// report in which no property is inconclusive. This is the bridge that stops a downstream
    /// consumer from reading a truncated search as a clean bill of health, because the sufficiency
    /// rule it already applies to evidence now applies to protocol analysis too.
    pub fn omissions(&self) -> OmissionManifest {
        let mut manifest = OmissionManifest::default();
        for (property, verdict) in self.verdicts() {
            if let Verdict::Inconclusive {
                hit,
                states_explored,
                ..
            } = verdict
            {
                manifest.push(OmissionGroup {
                    reason: format!(
                        "{} unchecked beyond the {} bound",
                        property.as_str(),
                        hit.as_str()
                    ),
                    influence: InfluenceClass::Unknown,
                    count: *states_explored,
                    bound: None,
                    examples: Vec::new(),
                });
            }
        }
        manifest
    }
}

/// A system of communicating roles: what the checker actually explores.
///
/// Constructible from a projection *or* from hand-written local types, because the interesting
/// deadlocks are the ones a human wrote by hand — the whole content of 23.06's theorem is that
/// projection cannot produce them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    locals: BTreeMap<Role, LocalType>,
}

impl System {
    pub fn new(locals: BTreeMap<Role, LocalType>) -> Self {
        System { locals }
    }

    pub fn from_locals(locals: impl IntoIterator<Item = (Role, LocalType)>) -> Self {
        System {
            locals: locals.into_iter().collect(),
        }
    }

    /// The projected system of a well-formed global type.
    pub fn from_projection(global: &WellFormedGlobal) -> Self {
        System {
            locals: global.projections().clone(),
        }
    }

    pub fn locals(&self) -> &BTreeMap<Role, LocalType> {
        &self.locals
    }

    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.locals.keys()
    }

    /// Checks all four properties under `bound`.
    pub fn check(&self, bound: ExplorationBound) -> ModelCheckReport {
        let exploration = explore(self, bound);
        exploration.report(bound)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    locals: BTreeMap<Role, LocalType>,
    channels: BTreeMap<(Role, Role), Vec<Label>>,
}

impl State {
    fn initial(system: &System) -> Self {
        State {
            locals: system.locals.clone(),
            channels: BTreeMap::new(),
        }
    }

    fn all_done(&self) -> bool {
        self.locals
            .values()
            .all(|local| matches!(local.unfold(), LocalType::End))
    }

    fn channels_empty(&self) -> bool {
        self.channels.values().all(|queue| queue.is_empty())
    }

    fn is_success(&self) -> bool {
        self.all_done() && self.channels_empty()
    }

    fn summary(&self) -> StateSummary {
        let roles = self
            .locals
            .iter()
            .map(|(role, local)| RoleState {
                role: role.clone(),
                status: match local.unfold() {
                    LocalType::End => RoleStatus::Done,
                    LocalType::Offer { from, branches } => RoleStatus::WaitingFor {
                        peer: from,
                        expecting: branches.into_iter().map(|b| b.label).collect(),
                    },
                    LocalType::Select { to, branches } => RoleStatus::ReadyToSend {
                        peer: to,
                        offering: branches.into_iter().map(|b| b.label).collect(),
                    },
                    LocalType::Var { var } => RoleStatus::Stuck {
                        reason: format!("free recursion variable {var}"),
                    },
                    LocalType::Rec { var, .. } => RoleStatus::Stuck {
                        reason: format!("unproductive recursion on {var}"),
                    },
                },
            })
            .collect();

        let channels = self
            .channels
            .iter()
            .filter(|(_, queue)| !queue.is_empty())
            .map(|((from, to), queue)| ChannelState {
                from: from.clone(),
                to: to.clone(),
                queued: queue.clone(),
            })
            .collect();

        StateSummary { roles, channels }
    }
}

struct Expansion {
    transitions: Vec<(Event, State)>,
    /// A send was suppressed by a full channel, so this state's successors are incomplete.
    capacity_blocked: bool,
    /// The head of some channel matched no offered label.
    unexpected: Option<(Role, Role, Label)>,
}

fn expand(state: &State, capacity: usize) -> Expansion {
    let mut transitions = Vec::new();
    let mut capacity_blocked = false;
    let mut unexpected = None;

    for (role, local) in &state.locals {
        match local.unfold() {
            LocalType::Select { to, branches } => {
                let queue_len = state
                    .channels
                    .get(&(role.clone(), to.clone()))
                    .map(Vec::len)
                    .unwrap_or(0);
                if queue_len >= capacity {
                    capacity_blocked = true;
                    continue;
                }
                for branch in branches {
                    let mut next = state.clone();
                    next.locals
                        .insert(role.clone(), branch.continuation.clone());
                    next.channels
                        .entry((role.clone(), to.clone()))
                        .or_default()
                        .push(branch.label.clone());
                    transitions.push((
                        Event::Send {
                            from: role.clone(),
                            to: to.clone(),
                            label: branch.label.clone(),
                        },
                        next,
                    ));
                }
            }
            LocalType::Offer { from, branches } => {
                let queue = match state.channels.get(&(from.clone(), role.clone())) {
                    Some(queue) if !queue.is_empty() => queue,
                    _ => continue,
                };
                let head = queue[0].clone();
                match branches.iter().find(|branch| branch.label == head) {
                    Some(branch) => {
                        let mut next = state.clone();
                        next.locals
                            .insert(role.clone(), branch.continuation.clone());
                        let Some(queue) = next.channels.get_mut(&(from.clone(), role.clone()))
                        else {
                            continue;
                        };
                        queue.remove(0);
                        transitions.push((
                            Event::Receive {
                                from: from.clone(),
                                to: role.clone(),
                                label: head.clone(),
                            },
                            next,
                        ));
                    }
                    None => {
                        if unexpected.is_none() {
                            unexpected = Some((from.clone(), role.clone(), head));
                        }
                    }
                }
            }
            LocalType::End | LocalType::Var { .. } | LocalType::Rec { .. } => {}
        }
    }

    Expansion {
        transitions,
        capacity_blocked,
        unexpected,
    }
}

struct Exploration {
    states: Vec<State>,
    parents: Vec<Option<(usize, Event)>>,
    successors: Vec<Vec<usize>>,
    /// Index-aligned: whether this state's expansion was cut short by the channel bound.
    incomplete: Vec<bool>,
    unexpected: Vec<Option<(Role, Role, Label)>>,
    truncated: Option<BoundHit>,
}

fn explore(system: &System, bound: ExplorationBound) -> Exploration {
    let initial = State::initial(system);
    let mut index: BTreeMap<State, usize> = BTreeMap::new();
    let mut exploration = Exploration {
        states: vec![initial.clone()],
        parents: vec![None],
        successors: vec![Vec::new()],
        incomplete: vec![false],
        unexpected: vec![None],
        truncated: None,
    };
    index.insert(initial, 0);

    let mut depths = vec![0usize];
    let mut frontier = VecDeque::from([0usize]);

    while let Some(current) = frontier.pop_front() {
        let depth = depths[current];
        if depth >= bound.max_depth {
            exploration.truncated.get_or_insert(BoundHit::MaxDepth);
            exploration.incomplete[current] = true;
            continue;
        }

        let expansion = expand(&exploration.states[current], bound.channel_capacity);
        if expansion.capacity_blocked {
            exploration
                .truncated
                .get_or_insert(BoundHit::ChannelCapacity);
            exploration.incomplete[current] = true;
        }
        exploration.unexpected[current] = expansion.unexpected;

        for (event, next) in expansion.transitions {
            let target = match index.get(&next) {
                Some(existing) => *existing,
                None => {
                    if exploration.states.len() >= bound.max_states {
                        exploration.truncated.get_or_insert(BoundHit::MaxStates);
                        exploration.incomplete[current] = true;
                        continue;
                    }
                    let fresh = exploration.states.len();
                    index.insert(next.clone(), fresh);
                    exploration.states.push(next);
                    exploration.parents.push(Some((current, event.clone())));
                    exploration.successors.push(Vec::new());
                    exploration.incomplete.push(false);
                    exploration.unexpected.push(None);
                    depths.push(depth + 1);
                    frontier.push_back(fresh);
                    fresh
                }
            };
            exploration.successors[current].push(target);
        }
    }

    exploration
}

impl Exploration {
    fn trace_to(&self, target: usize) -> Vec<TraceStep> {
        let mut reversed = Vec::new();
        let mut cursor = target;
        while let Some((parent, event)) = &self.parents[cursor] {
            reversed.push(event.clone());
            cursor = *parent;
        }
        reversed.reverse();
        reversed
            .into_iter()
            .enumerate()
            .map(|(index, event)| TraceStep { index, event })
            .collect()
    }

    fn counterexample(&self, property: Property, at: usize, explanation: String) -> Verdict {
        Verdict::Fails(Box::new(Counterexample {
            property,
            steps: self.trace_to(at),
            state: self.states[at].summary(),
            explanation,
        }))
    }

    /// The answer when no counterexample was found: a proof if the search was exhaustive, and an
    /// explicit admission of ignorance if it was not.
    fn settle(&self, bound: ExplorationBound) -> Verdict {
        match self.truncated {
            Some(hit) => Verdict::Inconclusive {
                bound,
                hit,
                states_explored: self.states.len(),
            },
            None => Verdict::Holds {
                states_explored: self.states.len(),
            },
        }
    }

    fn deadlock(&self, bound: ExplorationBound) -> Verdict {
        for (candidate, state) in self.states.iter().enumerate() {
            let stalled = self.successors[candidate].is_empty() && !self.incomplete[candidate];
            if stalled && !state.all_done() {
                let waiting: Vec<String> = state
                    .summary()
                    .roles
                    .iter()
                    .filter(|role| !matches!(role.status, RoleStatus::Done))
                    .map(|role| match &role.status {
                        RoleStatus::WaitingFor { peer, expecting } => {
                            let labels: Vec<&str> = expecting.iter().map(Label::as_str).collect();
                            format!("{} waits for {} to send {:?}", role.role, peer, labels)
                        }
                        other => format!("{} is {:?}", role.role, other),
                    })
                    .collect();
                return self.counterexample(
                    Property::Deadlock,
                    candidate,
                    format!(
                        "no role can move and not every role is finished: {}",
                        waiting.join("; ")
                    ),
                );
            }
        }
        self.settle(bound)
    }

    fn orphan(&self, bound: ExplorationBound) -> Verdict {
        for (candidate, state) in self.states.iter().enumerate() {
            if state.all_done() && !state.channels_empty() {
                let stranded: Vec<String> = state
                    .channels
                    .iter()
                    .filter(|(_, queue)| !queue.is_empty())
                    .map(|((from, to), queue)| {
                        let labels: Vec<&str> = queue.iter().map(Label::as_str).collect();
                        format!("{from} -> {to} still holds {labels:?}")
                    })
                    .collect();
                return self.counterexample(
                    Property::OrphanMessage,
                    candidate,
                    format!(
                        "every role reached its terminal state with messages never read: {}",
                        stranded.join("; ")
                    ),
                );
            }
        }
        self.settle(bound)
    }

    fn unexpected_message(&self, bound: ExplorationBound) -> Verdict {
        for (candidate, unexpected) in self.unexpected.iter().enumerate() {
            if let Some((from, to, label)) = unexpected {
                return self.counterexample(
                    Property::UnexpectedMessage,
                    candidate,
                    format!("{to} offers no branch for {label} sent by {from}"),
                );
            }
        }
        self.settle(bound)
    }

    /// Liveness: from every reachable state, a successful terminal state is still reachable.
    ///
    /// Computed by reverse reachability from the successful terminals, which is only meaningful
    /// over a complete graph. Under truncation the answer is [`Verdict::Inconclusive`] with no
    /// attempt at a trace, because a state that looks stranded here may reach a terminal through
    /// a successor that was never expanded.
    fn liveness(&self, bound: ExplorationBound) -> Verdict {
        if let Some(hit) = self.truncated {
            return Verdict::Inconclusive {
                bound,
                hit,
                states_explored: self.states.len(),
            };
        }

        let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); self.states.len()];
        for (source, targets) in self.successors.iter().enumerate() {
            for target in targets {
                predecessors[*target].push(source);
            }
        }

        let mut can_finish = vec![false; self.states.len()];
        let mut frontier: VecDeque<usize> = VecDeque::new();
        for (candidate, state) in self.states.iter().enumerate() {
            if state.is_success() {
                can_finish[candidate] = true;
                frontier.push_back(candidate);
            }
        }
        while let Some(current) = frontier.pop_front() {
            for predecessor in &predecessors[current] {
                if !can_finish[*predecessor] {
                    can_finish[*predecessor] = true;
                    frontier.push_back(*predecessor);
                }
            }
        }

        for (candidate, reaches) in can_finish.iter().enumerate() {
            if !reaches {
                return self.counterexample(
                    Property::Liveness,
                    candidate,
                    "no continuation from this state reaches a terminal state with every role \
                     finished and every channel empty"
                        .to_string(),
                );
            }
        }

        Verdict::Holds {
            states_explored: self.states.len(),
        }
    }

    fn report(&self, bound: ExplorationBound) -> ModelCheckReport {
        ModelCheckReport {
            bound,
            states_explored: self.states.len(),
            truncated: self.truncated,
            deadlock: self.deadlock(bound),
            liveness: self.liveness(bound),
            orphan: self.orphan(bound),
            unexpected_message: self.unexpected_message(bound),
        }
    }
}
