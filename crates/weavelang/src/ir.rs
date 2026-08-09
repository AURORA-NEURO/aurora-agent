//! WeaveIR: the canonical machine representation.
//!
//! Blueprint 23.03 gives the top-level document shape and 23.38 gives the canonical event envelope
//! and the payload documents that travel with it. Both are followed here. Where they disagree, the
//! disagreement is real and is called out below rather than resolved by preference.
//!
//! **Field naming.** 23.03's document uses `snake_case` (`weave_ir_version`, `program_id`,
//! `state_graph`, `actor_role`, `payload_type`); 23.38's envelope uses `camelCase`
//! (`weaveVersion`, `eventId`, `programId`, `payloadRef`). This crate keeps each in the case its
//! own module specifies, because both are hashed and a canonicalisation that renamed one of them
//! would stop matching whatever else implements the other. So [`WeaveIr`] serialises `snake_case`
//! and [`WeaveEvent`] serialises `camelCase`, in one crate, deliberately.
//!
//! **Canonical bytes.** 23.38's canonicalisation requirements — UTF-8, deterministic field
//! ordering, no non-finite numbers, explicit schema version — are exactly the guarantees
//! `bioprism_ids::to_canonical_bytes` already provides, so this module calls it rather than
//! writing a second serialiser. The workspace's cross-language parity rests on there being one
//! canonical encoder; a second one that agreed today would drift.
//!
//! **What 23.38 asks for that cannot be produced deterministically.** Two fields of its envelope
//! are omitted, and their absence is the honest option rather than a gap:
//!
//! - `time` is a wall-clock timestamp. This crate has no clock. The field exists on
//!   [`WeaveEvent`] as an `Option` a caller may supply, and is never generated here, so a trace
//!   produced by this compiler is reproducible byte for byte.
//! - `signature` is absent from the struct entirely. 23.38's own canonicalisation rule says
//!   "signatures excluded from the signed payload they wrap", so an envelope that carried its own
//!   signature could not be the thing signed. [`WeaveEvent::signing_bytes`] returns what a signer
//!   should sign; wrapping that in a signature envelope is the caller's job, and no key material
//!   ever reaches this crate.
//! - `eventId` is written `urn:uuid:...` in 23.38. Minting a UUID requires randomness, which would
//!   destroy replay determinism, so event identifiers are derived from the thread, the logical
//!   clock and the payload digest. The scheme is stated in [`derive_event_id`].
//!
//! Not implemented: no schema registry, no JSON Schema or Protobuf or WIT loading (23.03 phase 2
//! lists them; this crate treats a schema reference as an opaque string it records and never
//! resolves), no signing, no target generation (23.03 phase 11), and no optimiser. WeaveIR here is
//! a single semantic level, not the four-level lowering pipeline 23.03 sketches; role projection
//! and binding are not performed.

use bioprism_ids::{to_canonical_bytes, ContentHash};
use bioprism_weave::Fidelity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The IR schema version, `weave_ir_version` in 23.03's document.
pub const WEAVE_IR_VERSION: &str = "0.1.0";

/// The event envelope version, `weaveVersion` in 23.38.
pub const WEAVE_EVENT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IrError {
    #[error("WeaveIR could not be encoded as JSON: {0}")]
    Encoding(String),

    #[error("WeaveIR could not be canonicalised: {0}")]
    Canonical(String),
}

impl crate::diagnostic::Diagnostic for IrError {
    fn code(&self) -> &'static str {
        match self {
            IrError::Encoding(_) => "WEAVE-E3001",
            IrError::Canonical(_) => "WEAVE-E3002",
        }
    }

    /// Always `None`: a canonicalisation failure is a property of a whole document, and pinning it
    /// to a token in the source would be a guess.
    fn span(&self) -> Option<crate::diagnostic::Span> {
        None
    }
}

/// A compiled program: 23.03's top-level structure, field for field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeaveIr {
    pub weave_ir_version: String,
    /// `urn:weave:program:<name>@sha256:<digest>`, assigned by [`WeaveIr::assign_identity`].
    pub program_id: String,
    pub package_lock: String,
    pub roles: Vec<RoleIr>,
    pub interfaces: Vec<InterfaceIr>,
    pub participants: Vec<ParticipantIr>,
    pub choreography: ChoreographyIr,
    pub policies: BTreeMap<String, PolicyIr>,
    pub ledgers: Vec<String>,
    pub state_graph: StateGraph,
    pub monitors: Vec<MonitorIr>,
    pub evaluation_hooks: Vec<EvaluationHookIr>,
    pub provenance: ProvenanceIr,
}

impl WeaveIr {
    /// The bytes a signature or a certificate is taken over.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, IrError> {
        let value = serde_json::to_value(self).map_err(|e| IrError::Encoding(e.to_string()))?;
        to_canonical_bytes(&value).map_err(|e| IrError::Canonical(e.to_string()))
    }

    /// The digest of the whole document, including its assigned identity.
    pub fn digest(&self) -> Result<String, IrError> {
        Ok(ContentHash::of_bytes(&self.canonical_bytes()?)
            .as_str()
            .to_string())
    }

    /// The digest of the *semantic* content: everything except provenance and the identity itself.
    ///
    /// 23.03: "A program identity is derived from semantic IR and package lock, not from source
    /// formatting", and 23.38 requires "excluded non-semantic metadata". Provenance records which
    /// compiler ran and over what source text, so including it would make two byte-identical
    /// programs compiled by two versions of this crate into two different programs.
    pub fn semantic_digest(&self) -> Result<String, IrError> {
        let mut value = serde_json::to_value(self).map_err(|e| IrError::Encoding(e.to_string()))?;
        if let Some(map) = value.as_object_mut() {
            map.remove("provenance");
            map.remove("program_id");
        }
        let bytes = to_canonical_bytes(&value).map_err(|e| IrError::Canonical(e.to_string()))?;
        Ok(ContentHash::of_bytes(&bytes).as_str().to_string())
    }

    /// Assigns `program_id` from the semantic digest. Idempotent.
    pub fn assign_identity(&mut self) -> Result<(), IrError> {
        self.program_id = String::new();
        let digest = self.semantic_digest()?;
        self.program_id = format!(
            "urn:weave:program:{}@sha256:{}",
            self.provenance.program_name, digest
        );
        Ok(())
    }
}

/// Where a program came from. Excluded from program identity by design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceIr {
    pub program_name: String,
    pub package: Option<String>,
    /// Digest of the source text, so a compiled artifact can be traced back to its input even
    /// though the input does not participate in identity.
    pub source_sha256: String,
    pub compiler: String,
    pub compiler_version: String,
}

/// How freely a participant may be substituted for the one a role was bound to (23.03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubstitutionPolicy {
    /// Any participant whose effect set is a subset of the role's declared requirement.
    EffectSubset,
    /// Only the identical capability set. 23.04: broader undeclared effects are not substitutable.
    Exact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleIr {
    pub id: String,
    pub provides: Vec<String>,
    pub requires_effects: Vec<String>,
    pub clearance: SecurityLabel,
    pub minimum_profile: Option<MinimumProfileIr>,
    pub substitution: SubstitutionPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MinimumProfileIr {
    pub reference: String,
    pub threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceIr {
    pub id: String,
    pub methods: Vec<MethodIr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodIr {
    pub name: String,
    pub params: Vec<ParamIr>,
    pub returns: Option<String>,
    pub effects: Vec<String>,
    pub throws: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamIr {
    pub name: String,
    pub type_ref: String,
}

/// A role occupant. Unbound at compile time: 23.02 makes participant binding a runtime step, so
/// the compiler emits the slot and the requirement, never an agent identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantIr {
    pub id: String,
    pub role: String,
    /// Cognitive ABI portability grade the role demands of whoever fills it (23.49).
    pub required_abi_grade: u8,
    pub bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoreographyIr {
    pub id: String,
    pub roles: Vec<String>,
    pub initial_state: String,
    pub terminal_states: Vec<String>,
    /// The declared global protocol, flattened. Preserved rather than projected: compiling a
    /// global protocol into local role automata is 23.03 phase 6, and this crate does not do it.
    pub declared_steps: Vec<ChoreoStepIr>,
}

/// One step of a declared choreography, with its enclosing choice label if it had one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoreoStepIr {
    pub from: String,
    pub to: String,
    pub act: String,
    pub payload_type: Option<String>,
    /// `Some(label)` when the step sits inside `choice by R { label: ... }`.
    pub choice_label: Option<String>,
    pub decided_by: Option<String>,
}

/// 23.03's state graph: stable choreography states and the transitions between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StateGraph {
    pub nodes: Vec<StateNode>,
    pub transitions: Vec<TransitionIr>,
}

impl StateGraph {
    pub fn node(&self, id: &str) -> Option<&StateNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Transitions leaving a state, in declaration order. Order is part of the semantics: it is
    /// what makes an interleaving reproducible without a scheduler.
    pub fn outgoing(&self, state: &str) -> Vec<&TransitionIr> {
        self.transitions
            .iter()
            .filter(|transition| transition.from == state)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateNode {
    pub id: String,
    pub enabled_acts: Vec<String>,
    pub outstanding_commitments: Vec<String>,
    pub effect_bounds: Vec<String>,
    pub guards: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionIr {
    pub id: String,
    pub from: String,
    pub to: String,
    pub actor_role: String,
    /// The communicative act, named as `bioprism_weave::ActKind` serialises it. The kernel owns
    /// the act vocabulary (23.05); this is a reference into it, not a second definition.
    pub act: String,
    pub payload_type: String,
    pub guard: Vec<String>,
    pub effects: TransitionEffects,
    /// Whether an irreversible effect on this transition needs a human grant first (23.03 phase 4).
    pub requires_human_approval: bool,
}

/// The effect record of 23.03's transition example, plus two fields it does not have.
///
/// `world` and `mutating` are additions. 23.34's replay-safety property ("historical replay cannot
/// invoke production side effects") needs to know which transitions touch the world, and 23.03's
/// `ledger`/`authority`/`budget` triple cannot express that. They are two fields rather than one
/// because 23.04 separates `read(resource)` from `write(resource)`: `repo.read` is an effect a
/// transition needs authority for, but it is not a production side effect, and a replay that
/// refused every declared effect would refuse to replay anything. `mutating` is always a subset of
/// `world`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TransitionEffects {
    pub ledger: Vec<String>,
    pub authority: Vec<String>,
    pub budget: Vec<String>,
    pub world: Vec<String>,
    pub mutating: Vec<String>,
}

impl TransitionEffects {
    pub fn mutates_world(&self) -> bool {
        !self.mutating.is_empty()
    }
}

/// The runtime monitors 23.03 phase 9 enumerates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorKind {
    MessageOrder,
    CommitmentTransition,
    AuthorityValidity,
    BudgetConsumption,
    InformationFlow,
    StateVersionConflict,
    Timeout,
    EvaluationHook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorIr {
    pub id: String,
    pub kind: MonitorKind,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationHookIr {
    pub id: String,
    pub capability: String,
    pub snapshot_include: Vec<String>,
    pub counterfactual_dimensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyIr {
    pub id: String,
    pub allow_effects: Vec<String>,
    pub deny_effects: Vec<String>,
    pub require_human_for: Vec<String>,
    /// Ceilings the kernel can actually account, keyed by `bioprism_weave::Resource`.
    pub budgets: Vec<EnforcedBudget>,
    /// Ceilings the source declared that no kernel resource corresponds to.
    ///
    /// 23.37's own policy example writes `budget money <= usd(5)`, and 23.16's resource vocabulary
    /// has no money. Dropping it silently would leave a program looking as though it had a spend
    /// ceiling; rejecting the program would refuse the reference example. So it is recorded, with
    /// its reason, inside the hashed IR — an unenforced ceiling and an enforced one must never
    /// share a representation.
    pub unenforceable_budgets: Vec<UnenforceableBudget>,
    pub max_participants: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnforcedBudget {
    pub resource: bioprism_weave::Resource,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnenforceableBudget {
    pub declared_resource: String,
    pub declared_limit: String,
    pub reason: String,
}

/// 23.04's security label: a sensitivity level and a compartment set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SecurityLabel {
    pub level: String,
    pub compartments: Vec<String>,
}

impl SecurityLabel {
    pub fn new(level: impl Into<String>) -> Self {
        SecurityLabel {
            level: level.into(),
            compartments: Vec::new(),
        }
    }

    /// The rank of a level in 23.04's lattice: `public < internal < confidential < restricted`.
    ///
    /// An unrecognised level ranks above every named one. That is deliberate: an unknown
    /// sensitivity is treated as the most restrictive, so a typo in a clearance cannot widen it.
    pub fn rank(level: &str) -> u8 {
        match level {
            "public" => 0,
            "internal" => 1,
            "confidential" => 2,
            "restricted" => 3,
            _ => u8::MAX,
        }
    }

    /// Whether a holder of `self` may receive a value labelled `other`.
    ///
    /// Dominance is level-at-least-as-high **and** compartment-superset. Both halves are needed:
    /// `restricted/patient-data` does not clear `restricted/site-7`.
    pub fn dominates(&self, other: &SecurityLabel) -> bool {
        if SecurityLabel::rank(&self.level) < SecurityLabel::rank(other.level.as_str()) {
            return false;
        }
        other
            .compartments
            .iter()
            .all(|compartment| self.compartments.contains(compartment))
    }
}

/// 23.38's canonical event envelope.
///
/// See the module docs for `time`, `signature` and `eventId`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaveEvent {
    pub weave_version: String,
    pub event_id: String,
    pub event_type: String,
    pub source: String,
    pub thread_id: String,
    pub program_id: String,
    pub choreography_state: String,
    pub logical_clock: u64,
    pub causal_parents: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub schema: String,
    pub security_label: SecurityLabel,
    pub payload_ref: String,
    pub idempotency_key: String,
}

impl WeaveEvent {
    /// The bytes to sign, which by 23.38's rule exclude any signature.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, IrError> {
        let value = serde_json::to_value(self).map_err(|e| IrError::Encoding(e.to_string()))?;
        to_canonical_bytes(&value).map_err(|e| IrError::Canonical(e.to_string()))
    }

    pub fn digest(&self) -> Result<String, IrError> {
        Ok(ContentHash::of_bytes(&self.signing_bytes()?)
            .as_str()
            .to_string())
    }
}

/// The event type string 23.38 uses: `aurora.weave.act.<kind>.v1`.
pub fn event_type_for(act: bioprism_weave::ActKind) -> String {
    format!("aurora.weave.act.{}.v1", act.as_str())
}

/// Derives a deterministic event identifier.
///
/// 23.38 writes `urn:uuid:...`, which cannot be produced without randomness. The substitute is a
/// URN over the digest of the three things that already distinguish an event inside a thread: the
/// thread, the logical clock, and the payload digest. Two runs of the same program on the same
/// inputs therefore produce the same identifiers, which is what makes a replayed trace comparable
/// at all.
pub fn derive_event_id(thread_id: &str, logical_clock: u64, payload_ref: &str) -> String {
    let seed = serde_json::json!({
        "thread": thread_id,
        "clock": logical_clock,
        "payload": payload_ref,
    });
    let digest = ContentHash::of_value(&seed)
        .expect("seed contains only strings and an integer")
        .as_str()
        .to_string();
    format!("urn:weave:event:sha256:{digest}")
}

/// A content reference in 23.37's `sha256:...` form.
pub fn content_ref(value: &Value) -> Result<String, IrError> {
    let hash = ContentHash::of_value(value).map_err(|e| IrError::Canonical(e.to_string()))?;
    Ok(format!("sha256:{}", hash.as_str()))
}

/// 23.38's claim payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimPayload {
    pub proposition_id: String,
    pub rendering: String,
    pub endorser: String,
    pub evidence: Vec<EvidenceRef>,
    pub assumptions: Vec<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub artifact: String,
    pub locator: Locator,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    pub kind: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Confidence {
    pub kind: String,
    pub value: f64,
    pub profile: String,
}

/// 23.38's Context Capsule document.
///
/// Distinct from `bioprism_weave::ContextCapsule`, which is the *projection machinery* the kernel
/// runs against a compiled Decision Section. This is the serialised document 23.38 specifies, which
/// the compiler emits and the kernel's projection fills; the two are not the same object and
/// merging them would put projection logic in a language crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextCapsuleIr {
    pub capsule_id: String,
    pub base_hash: String,
    pub recipient: String,
    pub objective: String,
    pub success_contract: String,
    pub world_snapshot: String,
    pub common_ground: CommonGround,
    pub focus: Focus,
    pub open_obligations: Vec<String>,
    pub grants: Vec<String>,
    pub budgets: BTreeMap<String, u64>,
    pub omissions: Vec<Omission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonGround {
    pub verified: Vec<String>,
    pub working: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Focus {
    pub claim: String,
    pub support: Vec<String>,
    pub counterevidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Omission {
    pub scope: String,
    pub reason: String,
}

/// 23.38's commitment event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitmentIr {
    pub commitment_id: String,
    pub creditor: String,
    pub debtor: String,
    pub antecedent: Antecedent,
    pub deliverable_type: String,
    pub deadline: String,
    pub quality_predicates: Vec<String>,
    pub authority: Vec<String>,
    pub budget_lease: String,
    pub remedy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Antecedent {
    pub event: String,
}

/// The resume grade 23.38's continuation writes as `"R2"`.
///
/// 23.38 gives the grades no semantics at all — `"R2"` appears once, unexplained. The kernel's
/// `Fidelity` (23.10) has three states with stated meanings, so the mapping below is *this crate's
/// reading*, not the blueprint's: R0 is inspect-only, R1 and R2 resume with something dropped, and
/// only R3 is byte-identical. It is written down here so that a disagreement with another
/// implementation shows up as a disagreement about this table rather than as a silent divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResumeGrade {
    R0,
    R1,
    R2,
    R3,
}

impl ResumeGrade {
    pub fn to_fidelity(self) -> Fidelity {
        match self {
            ResumeGrade::R0 => Fidelity::Frozen,
            ResumeGrade::R1 | ResumeGrade::R2 => Fidelity::Lossy,
            ResumeGrade::R3 => Fidelity::Exact,
        }
    }

    /// The grade written as the cognitive ABI portability number in `Participant::abi_grade`.
    pub fn as_grade(self) -> u8 {
        match self {
            ResumeGrade::R0 => 0,
            ResumeGrade::R1 => 1,
            ResumeGrade::R2 => 2,
            ResumeGrade::R3 => 3,
        }
    }
}

/// 23.38's continuation document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationIr {
    pub continuation_id: String,
    pub fidelity: ResumeGrade,
    pub world_snapshot: String,
    pub local_role_state: String,
    pub epistemic_checkpoint: String,
    pub commitment_checkpoint: String,
    pub open_obligations: Vec<String>,
    pub grants: Vec<String>,
    pub budget_lease: String,
    pub resume_input_schema: String,
    pub expected_output_schema: String,
    pub invariants: Vec<String>,
}

/// 23.38's Molecule Card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoleculeCard {
    pub name: String,
    pub version: String,
    pub interfaces: Vec<String>,
    pub program_hash: String,
    pub policy_hash: String,
    pub choreography_hash: String,
    pub effects: MoleculeEffects,
    pub verified_profile: String,
    pub adapters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoleculeEffects {
    pub required: Vec<String>,
    pub forbidden: Vec<String>,
}
