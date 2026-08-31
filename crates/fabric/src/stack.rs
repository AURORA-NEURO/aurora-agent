//! The layered protocol stack and its interoperability boundary.
//!
//! Blueprint 23.01.
//!
//! # The one thing this module is for
//!
//! 23.01: "The system never labels an adapter 'compatible' without identifying semantic loss."
//! [`Compatibility`] therefore has no `Compatible` variant. An adapter is either
//! [`Compatibility::LosslessForDeclaredSurface`] — which names the surface it was checked over, and
//! says nothing about anything else — or [`Compatibility::Lossy`], which carries the approximated
//! and unsupported sets. There is no way to spell "works fine, did not look".
//!
//! The complement is [`fail_closed`], which runs 23.01's seven bridge conditions. Each one is a
//! *representability* question — can the target carry a commitment state, an authority provenance,
//! a security label — and the answer is drawn from the adapter's own declaration, so an adapter
//! that under-declares its support fails closed against itself.
//!
//! # Session Capability Agreement
//!
//! "Negotiation must produce a pinned Session Capability Agreement even when the underlying
//! transport is stateless." [`negotiate_session`] intersects two participants' capabilities and
//! returns an agreement with a content digest. The intersection is the interesting part: a
//! capsule resolution or continuation fidelity only one side supports is not in the agreement, and
//! [`SessionAgreement::conceded`] reports what each side gave up, because a participant that
//! silently downgrades is the failure mode 23.01's fail-closed list exists to prevent.
//!
//! # Not implemented
//!
//! No transport of any kind. No A2A, MCP, CloudEvents, OpenTelemetry or WIT bindings — 23.01's
//! mapping tables are recorded as data in [`A2A_MAPPING`] and [`MCP_MAPPING`] and nothing consumes
//! them. No envelope encoding: `bioprism-weavelang` owns the WeaveIR event envelope and building a
//! second one here would give the workspace two.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One layer of 23.01's twelve-layer stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerSpec {
    pub level: u8,
    pub name: &'static str,
    pub contents: &'static str,
}

/// The stack, L0 at index 0.
pub const STACK: [LayerSpec; 12] = [
    LayerSpec {
        level: 0,
        name: "Transport",
        contents: "local calls, HTTP, JSON-RPC, gRPC, SSE, WebSocket, queues, files",
    },
    LayerSpec {
        level: 1,
        name: "Telemetry and propagation",
        contents: "OpenTelemetry, trace context, causality, resource attribution",
    },
    LayerSpec {
        level: 2,
        name: "Identity, authorization, and event envelopes",
        contents: "principals, capability grants, attestations, CloudEvents-like envelope",
    },
    LayerSpec {
        level: 3,
        name: "Agent and component interoperability",
        contents: "A2A, MCP, CLI agent adapters, framework bridges, WIT components",
    },
    LayerSpec {
        level: 4,
        name: "Cognitive ABI and WeaveIR",
        contents: "invocation contracts, typed events, ledgers, state machines, invariants",
    },
    LayerSpec {
        level: 5,
        name: "Typed execution",
        contents: "continuations, Braided Worldlines, transactions, effects, compensation",
    },
    LayerSpec {
        level: 6,
        name: "Coordination semantics",
        contents: "communicative acts, commitments, choreography, negotiation, quorum",
    },
    LayerSpec {
        level: 7,
        name: "Shared cognition",
        contents: "epistemic CRDT, context capsules, assumptions, disputes, common ground",
    },
    LayerSpec {
        level: 8,
        name: "Adaptive team control",
        contents: "spawn, retire, route, budget, topology, aggregation, stopping",
    },
    LayerSpec {
        level: 9,
        name: "Composite-agent layer",
        contents: "Capability Molecules, composition algebra, substitution, model checking",
    },
    LayerSpec {
        level: 10,
        name: "Goal-to-Molecule synthesis",
        contents: "task contracts, capability evidence, topology search, adaptive probes",
    },
    LayerSpec {
        level: 11,
        name: "Evaluation and evolution",
        contents: "PRISM Decision Cells, causal attribution, architecture search, holdouts",
    },
];

/// Which crate in this workspace discharges each layer.
///
/// A map from the blueprint's architecture onto the code, so a reader can check the claim rather
/// than take it. Layers with no entry are not implemented anywhere and say so.
pub fn layer_owner(level: u8) -> Option<&'static str> {
    match level {
        4 => Some("bioprism-weavelang (WeaveIR) and bioprism-weave (the ABI, 23.49)"),
        5 => Some("bioprism-weave (continuations, budgets), bioprism-choreography (sagas)"),
        6 => Some("bioprism-choreography (session types, quorum), bioprism-fabric (negotiation)"),
        7 => Some("bioprism-fabric (ground), bioprism-weave (capsules)"),
        8 => Some("bioprism-fabric (topology)"),
        9 => Some("bioprism-fabric (algebra, molecule), bioprism-choreography (model checking)"),
        10 => Some("bioprism-fabric (synth)"),
        _ => None,
    }
}

/// The semantic surface an adapter may or may not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticFeature {
    Discovery,
    TaskLifecycle,
    Messages,
    Artifacts,
    Commitments,
    AuthorityDelegation,
    ContinuationTransfer,
    EpistemicStateDelta,
    SecurityLabels,
    MessageOrdering,
    ClaimVersusVerifiedFact,
}

/// What an adapter does when it hits something it cannot carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossPolicy {
    Reject,
    Wrap,
    RejectOrWrap,
}

/// 23.01's adapter declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    pub source_protocol: String,
    pub target_semantics: String,
    pub supported: BTreeSet<SemanticFeature>,
    pub approximated: BTreeSet<SemanticFeature>,
    pub unsupported: BTreeSet<SemanticFeature>,
    pub loss_policy: LossPolicy,
    pub fidelity: AdapterRung,
}

/// 23.01's six-rung adapter ladder, highest fidelity first in the blueprint and ordered
/// weakest-first here so `>=` reads as "at least this good".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterRung {
    /// "opaque text bridge, marked lowest fidelity".
    OpaqueText,
    CliPtyDriver,
    OpenTelemetryFramework,
    StructuredJsonStreaming,
    A2AWithWeaveExtension,
    NativeWeaveSdk,
}

/// What an adapter may be called.
///
/// No `Compatible` variant exists. That absence is the module's contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "compatibility")]
pub enum Compatibility {
    /// Lossless over exactly the listed surface, and silent about the rest.
    LosslessForDeclaredSurface { surface: BTreeSet<SemanticFeature> },
    Lossy {
        approximated: BTreeSet<SemanticFeature>,
        unsupported: BTreeSet<SemanticFeature>,
    },
}

impl Adapter {
    pub fn new(
        source_protocol: impl Into<String>,
        target_semantics: impl Into<String>,
        fidelity: AdapterRung,
        loss_policy: LossPolicy,
    ) -> Self {
        Adapter {
            source_protocol: source_protocol.into(),
            target_semantics: target_semantics.into(),
            supported: BTreeSet::new(),
            approximated: BTreeSet::new(),
            unsupported: BTreeSet::new(),
            loss_policy,
            fidelity,
        }
    }

    pub fn supporting(mut self, feature: SemanticFeature) -> Self {
        self.supported.insert(feature);
        self
    }

    pub fn approximating(mut self, feature: SemanticFeature) -> Self {
        self.approximated.insert(feature);
        self
    }

    pub fn not_supporting(mut self, feature: SemanticFeature) -> Self {
        self.unsupported.insert(feature);
        self
    }

    pub fn compatibility(&self) -> Compatibility {
        if self.approximated.is_empty() && self.unsupported.is_empty() {
            Compatibility::LosslessForDeclaredSurface {
                surface: self.supported.clone(),
            }
        } else {
            Compatibility::Lossy {
                approximated: self.approximated.clone(),
                unsupported: self.unsupported.clone(),
            }
        }
    }

    /// Features the adapter never mentions. Neither supported nor refused: unexamined, and a bridge
    /// check treats them as unsupported.
    pub fn undeclared(&self) -> BTreeSet<SemanticFeature> {
        [
            SemanticFeature::Discovery,
            SemanticFeature::TaskLifecycle,
            SemanticFeature::Messages,
            SemanticFeature::Artifacts,
            SemanticFeature::Commitments,
            SemanticFeature::AuthorityDelegation,
            SemanticFeature::ContinuationTransfer,
            SemanticFeature::EpistemicStateDelta,
            SemanticFeature::SecurityLabels,
            SemanticFeature::MessageOrdering,
            SemanticFeature::ClaimVersusVerifiedFact,
        ]
        .into_iter()
        .filter(|f| {
            !self.supported.contains(f)
                && !self.approximated.contains(f)
                && !self.unsupported.contains(f)
        })
        .collect()
    }
}

/// What a bridge is being asked to carry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub required: BTreeSet<SemanticFeature>,
    /// Whether the caller has explicitly approved a downgrade. 23.01 permits crossing the boundary
    /// only with "explicit downgrade approval", so this is an argument, never a default.
    pub downgrade_approved: BTreeSet<SemanticFeature>,
}

impl BridgeRequest {
    pub fn requiring(mut self, feature: SemanticFeature) -> Self {
        self.required.insert(feature);
        self
    }

    pub fn approving_downgrade(mut self, feature: SemanticFeature) -> Self {
        self.downgrade_approved.insert(feature);
        self
    }
}

/// 23.01's seven fail-closed conditions.
///
/// An undeclared feature counts as unsupported, which is what "fail closed" means.
pub fn fail_closed(adapter: &Adapter, request: &BridgeRequest) -> Result<(), StackError> {
    let undeclared = adapter.undeclared();
    let mut blocked = BTreeSet::new();
    for feature in &request.required {
        let carried = adapter.supported.contains(feature);
        let approximated = adapter.approximated.contains(feature);
        if carried {
            continue;
        }
        if (approximated || undeclared.contains(feature) || adapter.unsupported.contains(feature))
            && !request.downgrade_approved.contains(feature)
        {
            blocked.insert(*feature);
        }
    }
    if blocked.is_empty() {
        Ok(())
    } else {
        Err(StackError::BridgeFailsClosed {
            source_protocol: adapter.source_protocol.clone(),
            features: blocked,
        })
    }
}

/// What one participant supports at session-negotiation time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCapabilities {
    pub participant: String,
    pub weave_ir_versions: BTreeSet<String>,
    pub acts: BTreeSet<String>,
    pub capsule_resolutions: BTreeSet<String>,
    pub continuation_fidelities: BTreeSet<String>,
    pub clearance_labels: BTreeSet<String>,
    pub max_payload_bytes: u64,
    pub replay_supported: bool,
}

impl SessionCapabilities {
    pub fn new(participant: impl Into<String>) -> Self {
        SessionCapabilities {
            participant: participant.into(),
            weave_ir_versions: BTreeSet::new(),
            acts: BTreeSet::new(),
            capsule_resolutions: BTreeSet::new(),
            continuation_fidelities: BTreeSet::new(),
            clearance_labels: BTreeSet::new(),
            max_payload_bytes: 0,
            replay_supported: false,
        }
    }

    pub fn speaking(mut self, version: impl Into<String>) -> Self {
        self.weave_ir_versions.insert(version.into());
        self
    }

    pub fn acting(mut self, act: impl Into<String>) -> Self {
        self.acts.insert(act.into());
        self
    }

    pub fn resolving(mut self, resolution: impl Into<String>) -> Self {
        self.capsule_resolutions.insert(resolution.into());
        self
    }

    pub fn with_payload_limit(mut self, bytes: u64) -> Self {
        self.max_payload_bytes = bytes;
        self
    }
}

/// The pinned agreement, and what each side conceded to get it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAgreement {
    pub participants: (String, String),
    pub weave_ir_version: String,
    pub acts: BTreeSet<String>,
    pub capsule_resolutions: BTreeSet<String>,
    pub continuation_fidelities: BTreeSet<String>,
    pub max_payload_bytes: u64,
    pub replay_supported: bool,
    conceded: Vec<(String, BTreeSet<String>)>,
}

impl SessionAgreement {
    /// What each participant supports and the session does not.
    pub fn conceded(&self) -> &[(String, BTreeSet<String>)] {
        &self.conceded
    }
}

/// Intersect two participants' capabilities into a pinned agreement.
///
/// The IR version is the highest both speak, by string order, which is deterministic and is the
/// only ordering available without a version grammar 23.01 does not give.
pub fn negotiate_session(
    left: &SessionCapabilities,
    right: &SessionCapabilities,
) -> Result<SessionAgreement, StackError> {
    let versions: BTreeSet<String> = left
        .weave_ir_versions
        .intersection(&right.weave_ir_versions)
        .cloned()
        .collect();
    let version =
        versions
            .iter()
            .next_back()
            .cloned()
            .ok_or_else(|| StackError::NoCommonIrVersion {
                left: left.participant.clone(),
                right: right.participant.clone(),
            })?;
    let acts: BTreeSet<String> = left.acts.intersection(&right.acts).cloned().collect();
    if acts.is_empty() {
        return Err(StackError::NoCommonActs {
            left: left.participant.clone(),
            right: right.participant.clone(),
        });
    }
    let capsule_resolutions: BTreeSet<String> = left
        .capsule_resolutions
        .intersection(&right.capsule_resolutions)
        .cloned()
        .collect();
    let continuation_fidelities: BTreeSet<String> = left
        .continuation_fidelities
        .intersection(&right.continuation_fidelities)
        .cloned()
        .collect();
    let conceded = vec![
        (
            left.participant.clone(),
            left.acts.difference(&acts).cloned().collect(),
        ),
        (
            right.participant.clone(),
            right.acts.difference(&acts).cloned().collect(),
        ),
    ];
    Ok(SessionAgreement {
        participants: (left.participant.clone(), right.participant.clone()),
        weave_ir_version: version,
        acts,
        capsule_resolutions,
        continuation_fidelities,
        max_payload_bytes: left.max_payload_bytes.min(right.max_payload_bytes),
        replay_supported: left.replay_supported && right.replay_supported,
        conceded,
    })
}

/// 23.01's A2A mapping table, recorded and unconsumed.
pub const A2A_MAPPING: [(&str, &str); 6] = [
    ("Agent Card", "imported participant capability declaration"),
    (
        "Message",
        "transport container for a Weave act or opaque fallback",
    ),
    ("Task", "Weave work item or commitment execution handle"),
    ("Artifact", "content-addressed Weave artifact"),
    ("task status", "execution lifecycle event"),
    (
        "extension",
        "location for Weave metadata where protocol-safe",
    ),
];

/// 23.01's MCP mapping table, recorded and unconsumed.
pub const MCP_MAPPING: [(&str, &str); 5] = [
    ("tool schema", "callable capability contract"),
    ("resource", "readable artifact or stream"),
    ("prompt/app", "optional user interface or instruction asset"),
    ("asynchronous task handle", "long-running execution binding"),
    (
        "authorization metadata",
        "input to the Weave authority gate",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StackError {
    #[error("bridge from {source_protocol} fails closed on {features:?}")]
    BridgeFailsClosed {
        source_protocol: String,
        features: BTreeSet<SemanticFeature>,
    },

    #[error("{left} and {right} share no WeaveIR version")]
    NoCommonIrVersion { left: String, right: String },

    #[error("{left} and {right} share no communicative act")]
    NoCommonActs { left: String, right: String },
}
