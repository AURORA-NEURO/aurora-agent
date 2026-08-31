//! Lowering WeaveLang to WeaveIR, with a property the compiler proves about its own output.
//!
//! Blueprint 23.03 phases 2 through 4: name resolution, then effect and authority checking. The
//! phases this crate does **not** run are named in the crate docs; what it does run, it runs to a
//! stated conclusion.
//!
//! # The preserved property
//!
//! A compiled program must declare no more authority than its source did. Concretely, for a source
//! program `P` and its lowering `lower(P)`:
//!
//! ```text
//! effects(lower(P)) ⊆ declared_effects(P) ⊆ allow(policy)    and    declared_effects(P) ∩ deny(policy) = ∅
//! ```
//!
//! `declared_effects(P)` is read from the source alone — the `requires` list of every role the
//! program binds, plus the `effects` clause of every interface method it calls. `effects(lower(P))`
//! is recomputed from the emitted transitions by a separate walk. The two are compared at the end
//! of every lowering, and a mismatch is [`LowerError::EffectIntroducedByLowering`].
//!
//! This is not a tautology, and the case that makes it bite is a commitment. `commit patcher to
//! lead when ready { deliver Patch satisfy with tests.pass }` lowers to a discharge transition
//! whose world effect is `tests.pass`, because discharging that commitment means running those
//! tests. If no role in the program declared the effect of running tests, the source understated
//! what the program does, and the compiler says so at compile time rather than letting the kernel
//! discover it when the act is refused. This is the same discipline `bioprism-benchcompiler`
//! applies to minimisation: the output must be provably no stronger than the input.
//!
//! # The budget ceiling
//!
//! Branch leases are checked by *running* `bioprism_weave::Budget`, the kernel's own affine
//! allowance, over the policy ceiling at compile time. `Budget::split` moves an allowance out of
//! the parent and does not implement `Clone`, so a program whose branch leases exceed its ceiling
//! fails the same way the kernel would fail it at run time — there is one implementation of the
//! rule, not a compile-time copy that could drift from the run-time one.
//!
//! Money is the exception, and it is recorded rather than enforced. 23.37's policy example writes
//! `budget money <= usd(5)`, and 23.16's resource vocabulary — the one the kernel accounts — has
//! tokens, tool calls and wall-clock milliseconds and no money. The ceiling is carried into the IR
//! as an [`crate::ir::UnenforceableBudget`] with its reason, because a declared-and-unenforced
//! ceiling and an enforced one must not share a representation.

use crate::ast::*;
use crate::diagnostic::{Diagnostic, Span};
use crate::ir::*;
use bioprism_ids::ContentHash;
use bioprism_weave::{ActKind, Budget, BudgetError, Resource};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LowerError {
    #[error("this source declares no `weave` program, so there is nothing to compile")]
    NoWeaveProgram,

    #[error("this source declares {} weave programs ({}); a compilation unit has exactly one entry point", names.len(), names.join(", "))]
    MultipleWeavePrograms { names: Vec<String> },

    #[error("weave `{weave}` at {span} declares no policy, so no budget ceiling or effect allowance exists to preserve; add `using <policy>`")]
    NoPolicy { weave: String, span: Span },

    #[error("`{name}` at {span} is not a declared policy")]
    UnknownPolicy { name: String, span: Span },

    #[error("`{name}` at {span} is not a declared role; declared roles are {known:?}")]
    UnknownRole {
        name: String,
        span: Span,
        known: Vec<String>,
    },

    #[error("`{name}` at {span} is not bound to a role; add `bind {name} to role <Role>`")]
    UnboundParticipant { name: String, span: Span },

    #[error("`{name}` at {span} is not a declared checkpoint")]
    UnknownCheckpoint { name: String, span: Span },

    #[error("effect `{effect}` required by role `{role}` is not allowed by policy `{policy}`")]
    UndeclaredEffect {
        effect: String,
        role: String,
        policy: String,
        span: Span,
    },

    #[error("effect `{effect}` required by role `{role}` is denied by policy `{policy}`")]
    DeniedEffect {
        effect: String,
        role: String,
        policy: String,
        span: Span,
    },

    #[error("lowering transition `{transition}` introduced effect `{effect}`, which the source never declared; declared effects are {declared:?}")]
    EffectIntroducedByLowering {
        transition: String,
        effect: String,
        declared: Vec<String>,
        span: Span,
    },

    #[error("branch `{branch}` leases {requested} {resource:?} at {span} but the policy ceiling leaves {available}")]
    BudgetCeilingExceeded {
        branch: String,
        resource: Resource,
        requested: u64,
        available: u64,
        span: Span,
    },

    #[error(
        "branch `{branch}` at {span} leases `{resource}`, which policy `{policy}` never allocates"
    )]
    BudgetResourceNotAllocated {
        branch: String,
        resource: String,
        policy: String,
        span: Span,
    },

    #[error("`{block}` at {span} has an empty body; an empty branch reaches no state and cannot be given a meaning")]
    EmptyBlock { block: &'static str, span: Span },

    #[error("ask expression at {span} does not have a valid participant and method target")]
    InvalidAskTarget { span: Span },

    #[error(transparent)]
    Ir(#[from] IrError),
}

impl Diagnostic for LowerError {
    fn code(&self) -> &'static str {
        match self {
            LowerError::NoWeaveProgram => "WEAVE-E3101",
            LowerError::MultipleWeavePrograms { .. } => "WEAVE-E3102",
            LowerError::NoPolicy { .. } => "WEAVE-E3103",
            LowerError::UnknownPolicy { .. } => "WEAVE-E3104",
            LowerError::UnknownRole { .. } => "WEAVE-E3105",
            LowerError::UnboundParticipant { .. } => "WEAVE-E3106",
            LowerError::UnknownCheckpoint { .. } => "WEAVE-E3107",
            LowerError::UndeclaredEffect { .. } => "WEAVE-E3201",
            LowerError::DeniedEffect { .. } => "WEAVE-E3202",
            LowerError::EffectIntroducedByLowering { .. } => "WEAVE-E3203",
            LowerError::BudgetCeilingExceeded { .. } => "WEAVE-E3301",
            LowerError::BudgetResourceNotAllocated { .. } => "WEAVE-E3302",
            LowerError::EmptyBlock { .. } => "WEAVE-E3108",
            LowerError::InvalidAskTarget { .. } => "WEAVE-E3109",
            LowerError::Ir(error) => error.code(),
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            LowerError::NoWeaveProgram
            | LowerError::MultipleWeavePrograms { .. }
            | LowerError::Ir(_) => None,
            LowerError::NoPolicy { span, .. }
            | LowerError::UnknownPolicy { span, .. }
            | LowerError::UnknownRole { span, .. }
            | LowerError::UnboundParticipant { span, .. }
            | LowerError::UnknownCheckpoint { span, .. }
            | LowerError::UndeclaredEffect { span, .. }
            | LowerError::DeniedEffect { span, .. }
            | LowerError::EffectIntroducedByLowering { span, .. }
            | LowerError::BudgetCeilingExceeded { span, .. }
            | LowerError::BudgetResourceNotAllocated { span, .. }
            | LowerError::EmptyBlock { span, .. }
            | LowerError::InvalidAskTarget { span } => Some(*span),
        }
    }
}

/// Maps a WeaveLang budget resource name onto the kernel's vocabulary (23.16).
///
/// Returns `None` for a name the kernel cannot account. The caller must record it as unenforceable
/// rather than dropping it.
pub fn kernel_resource(name: &str) -> Option<Resource> {
    match name {
        "tokens" => Some(Resource::Tokens),
        "tool-calls" | "toolCalls" | "tool_calls" => Some(Resource::ToolCalls),
        "wall-clock" | "latency" | "wall-clock-millis" => Some(Resource::WallClockMillis),
        _ => None,
    }
}

/// Whether an effect name denotes a production side effect rather than a read.
///
/// 23.04 lists `read(resource)` and `write(resource)` as different effects, and 23.34's
/// replay-safety property is about the second kind only. WeaveLang effect names are dotted verbs
/// (`repo.read`, `branch.write`, `test.run`, `main.merge`), so the last segment decides.
///
/// The default is *mutating*. An effect whose verb this table does not recognise is treated as a
/// side effect, so a name nobody anticipated cannot slip through a replay. Being wrong in that
/// direction costs a refused replay; being wrong the other way costs a production write during one.
pub fn mutates_world(effect: &str) -> bool {
    let verb = effect.rsplit('.').next().unwrap_or(effect);
    !matches!(
        verb,
        "read" | "list" | "get" | "search" | "inspect" | "view" | "query" | "observe"
    )
}

/// Maps a WeaveLang act name onto the kernel's act vocabulary (23.05).
///
/// Returns `None` for a name that is not one of the kernel's ten acts; the caller decides what a
/// non-act message lowers to. The kernel owns this vocabulary and this function only reads it.
pub fn kernel_act(name: &str) -> Option<ActKind> {
    Some(match name {
        "ask" => ActKind::Ask,
        "claim" => ActKind::Claim,
        "propose" => ActKind::Propose,
        "accept" => ActKind::Accept,
        "reject" => ActKind::Reject,
        "challenge" => ActKind::Challenge,
        "discharge" => ActKind::Discharge,
        "delegate" => ActKind::Delegate,
        "revoke" => ActKind::Revoke,
        "attest" => ActKind::Attest,
        _ => return None,
    })
}

/// Lowers a parsed program to WeaveIR.
pub fn lower_program(program: &Program, source: &str) -> Result<WeaveIr, LowerError> {
    let weaves: Vec<&WeaveDecl> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Weave(decl) => Some(decl),
            _ => None,
        })
        .collect();
    let weave = match weaves.as_slice() {
        [] => return Err(LowerError::NoWeaveProgram),
        [single] => *single,
        many => {
            return Err(LowerError::MultipleWeavePrograms {
                names: many.iter().map(|decl| decl.name.clone()).collect(),
            })
        }
    };

    let roles: BTreeMap<&str, &RoleDecl> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Role(decl) => Some((decl.name.as_str(), decl)),
            _ => None,
        })
        .collect();
    let interfaces: Vec<&InterfaceDecl> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Interface(decl) => Some(decl),
            _ => None,
        })
        .collect();
    let policies: BTreeMap<&str, &PolicyDecl> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Policy(decl) => Some((decl.name.as_str(), decl)),
            _ => None,
        })
        .collect();

    let policy_name = weave
        .using_policy
        .as_deref()
        .ok_or_else(|| LowerError::NoPolicy {
            weave: weave.name.clone(),
            span: weave.span,
        })?;
    let policy = *policies
        .get(policy_name)
        .ok_or_else(|| LowerError::UnknownPolicy {
            name: policy_name.to_string(),
            span: weave.span,
        })?;

    let mut lowering = Lowering {
        roles: &roles,
        interfaces: &interfaces,
        policy,
        bindings: BTreeMap::new(),
        checkpoints: BTreeSet::new(),
        declared_effects: BTreeSet::new(),
        transitions: Vec::new(),
        states: BTreeSet::new(),
        terminal_states: BTreeSet::new(),
        monitors: Vec::new(),
        hooks: Vec::new(),
        ceiling: budget_ceiling(policy),
        next_state: 0,
    };

    lowering.collect_bindings(&weave.body)?;
    lowering.collect_declared_effects(&weave.body)?;
    lowering.check_effects_against_policy()?;

    let entry = "start".to_string();
    let exit = "complete".to_string();
    lowering.states.insert(entry.clone());
    lowering.states.insert(exit.clone());
    lowering.terminal_states.insert(exit.clone());
    lowering.lower_block(&weave.body, &entry, &exit, "weave", weave.span)?;

    lowering.verify_effect_preservation()?;

    let choreography = lower_choreography(program, &entry, &lowering.terminal_states);
    let policies_ir = policies
        .values()
        .map(|decl| (decl.name.clone(), lower_policy(decl)))
        .collect();

    let mut ir = WeaveIr {
        weave_ir_version: WEAVE_IR_VERSION.to_string(),
        program_id: String::new(),
        package_lock: package_lock(program)?,
        roles: roles.values().map(|decl| lower_role(decl)).collect(),
        interfaces: interfaces
            .iter()
            .map(|decl| lower_interface(decl))
            .collect(),
        participants: lowering.participants(&weave.body),
        choreography,
        policies: policies_ir,
        ledgers: vec!["commitment".to_string(), "epistemic".to_string()],
        state_graph: lowering.state_graph(),
        monitors: lowering.monitors.clone(),
        evaluation_hooks: lowering.hooks.clone(),
        provenance: ProvenanceIr {
            program_name: weave.name.clone(),
            package: program.package.as_ref().map(|decl| decl.name.text()),
            source_sha256: ContentHash::of_bytes(source.as_bytes())
                .as_str()
                .to_string(),
            compiler: "bioprism-weavelang".to_string(),
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    ir.assign_identity()?;
    Ok(ir)
}

fn budget_ceiling(policy: &PolicyDecl) -> Budget {
    let mut budget = Budget::new();
    for limit in &policy.budgets {
        if let (Some(resource), Literal::Integer(value)) =
            (kernel_resource(&limit.resource), &limit.limit)
        {
            budget = budget.with(resource, (*value).max(0) as u64);
        }
    }
    budget
}

fn lower_policy(decl: &PolicyDecl) -> PolicyIr {
    let mut budgets = Vec::new();
    let mut unenforceable = Vec::new();
    for limit in &decl.budgets {
        match (kernel_resource(&limit.resource), &limit.limit) {
            (Some(resource), Literal::Integer(value)) => budgets.push(EnforcedBudget {
                resource,
                limit: (*value).max(0) as u64,
            }),
            (Some(resource), Literal::Duration { millis, .. }) => budgets.push(EnforcedBudget {
                resource,
                limit: *millis,
            }),
            (_, other) => unenforceable.push(UnenforceableBudget {
                declared_resource: limit.resource.clone(),
                declared_limit: limit_text(other),
                reason: format!(
                    "`{}` is not a resource the Weave kernel accounts; 23.16 allocates tokens, tool calls and wall-clock milliseconds only",
                    limit.resource
                ),
            }),
        }
    }
    PolicyIr {
        id: decl.name.clone(),
        allow_effects: decl.allow_effects.iter().map(Path::text).collect(),
        deny_effects: decl.deny_effects.iter().map(Path::text).collect(),
        require_human_for: decl.require_human_for.iter().map(Path::text).collect(),
        budgets,
        unenforceable_budgets: unenforceable,
        max_participants: decl.max_participants,
    }
}

fn limit_text(limit: &Literal) -> String {
    match limit {
        Literal::Integer(value) => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::Duration { text, .. } => text.clone(),
        Literal::Text(text) => text.clone(),
        Literal::Money {
            currency,
            minor_units,
        } => format!(
            "{}({}.{:02})",
            currency,
            minor_units / 100,
            minor_units % 100
        ),
    }
}

fn lower_role(decl: &RoleDecl) -> RoleIr {
    RoleIr {
        id: decl.name.clone(),
        provides: decl.provides.iter().map(VersionedName::text).collect(),
        requires_effects: decl.requires.iter().map(Path::text).collect(),
        clearance: decl
            .clearance
            .as_ref()
            .map(|clearance| SecurityLabel {
                level: clearance.level.clone(),
                compartments: clearance.compartments.clone(),
            })
            .unwrap_or_else(|| SecurityLabel::new("public")),
        minimum_profile: decl
            .minimum_profile
            .as_ref()
            .map(|profile| MinimumProfileIr {
                reference: profile.reference.clone(),
                threshold: profile.threshold,
            }),
        substitution: SubstitutionPolicy::EffectSubset,
    }
}

fn lower_interface(decl: &InterfaceDecl) -> InterfaceIr {
    InterfaceIr {
        id: decl.name.clone(),
        methods: decl
            .methods
            .iter()
            .map(|method| MethodIr {
                name: method.name.clone(),
                params: method
                    .params
                    .iter()
                    .map(|param| ParamIr {
                        name: param.name.clone(),
                        type_ref: param.ty.text(),
                    })
                    .collect(),
                returns: method.returns.as_ref().map(TypeRef::text),
                effects: method.effects.iter().map(Path::text).collect(),
                throws: method.throws.clone(),
            })
            .collect(),
    }
}

fn lower_choreography(
    program: &Program,
    initial: &str,
    terminals: &BTreeSet<String>,
) -> ChoreographyIr {
    let declared = program.items.iter().find_map(|item| match item {
        Item::Choreography(decl) => Some(decl),
        _ => None,
    });
    let mut steps = Vec::new();
    let mut roles = BTreeSet::new();
    if let Some(decl) = declared {
        flatten_choreography(&decl.steps, None, None, &mut steps, &mut roles);
    }
    ChoreographyIr {
        id: declared
            .map(|decl| decl.name.clone())
            .unwrap_or_else(|| "implicit".to_string()),
        roles: roles.into_iter().collect(),
        initial_state: initial.to_string(),
        terminal_states: terminals.iter().cloned().collect(),
        declared_steps: steps,
    }
}

fn flatten_choreography(
    steps: &[ChoreoStep],
    label: Option<&str>,
    decided_by: Option<&str>,
    out: &mut Vec<ChoreoStepIr>,
    roles: &mut BTreeSet<String>,
) {
    for step in steps {
        match step {
            ChoreoStep::Message {
                from,
                to,
                act,
                payload,
                ..
            } => {
                roles.insert(from.clone());
                roles.insert(to.clone());
                out.push(ChoreoStepIr {
                    from: from.clone(),
                    to: to.clone(),
                    act: act.clone(),
                    payload_type: payload.as_ref().map(TypeRef::text),
                    choice_label: label.map(str::to_string),
                    decided_by: decided_by.map(str::to_string),
                });
            }
            ChoreoStep::Choice { by, branches, .. } => {
                roles.insert(by.clone());
                for branch in branches {
                    flatten_choreography(&branch.steps, Some(&branch.label), Some(by), out, roles);
                }
            }
        }
    }
}

/// The package lock: a digest over the imports, in canonical order.
///
/// 23.03 makes program identity depend on "semantic IR and package lock", so the lock has to be
/// deterministic and independent of import order in the source.
fn package_lock(program: &Program) -> Result<String, IrError> {
    let mut names: Vec<String> = program
        .imports
        .iter()
        .map(|import| import.name.text())
        .collect();
    names.sort();
    names.dedup();
    let value = serde_json::json!({ "imports": names });
    content_ref(&value)
}

struct Lowering<'a> {
    roles: &'a BTreeMap<&'a str, &'a RoleDecl>,
    interfaces: &'a [&'a InterfaceDecl],
    policy: &'a PolicyDecl,
    bindings: BTreeMap<String, String>,
    checkpoints: BTreeSet<String>,
    declared_effects: BTreeSet<String>,
    transitions: Vec<TransitionIr>,
    states: BTreeSet<String>,
    terminal_states: BTreeSet<String>,
    monitors: Vec<MonitorIr>,
    hooks: Vec<EvaluationHookIr>,
    ceiling: Budget,
    next_state: usize,
}

impl Lowering<'_> {
    fn new_state(&mut self) -> String {
        self.next_state += 1;
        let id = format!("s{:03}", self.next_state);
        self.states.insert(id.clone());
        id
    }

    /// Records every `bind x to role R` in the program, at any depth.
    ///
    /// Bindings are gathered before any statement is lowered, because a `send` may name a
    /// participant bound inside a branch it does not lexically follow.
    fn collect_bindings(&mut self, body: &[Stmt]) -> Result<(), LowerError> {
        for statement in body {
            if let Stmt::Bind { name, role, span } = statement {
                if !self.roles.contains_key(role.as_str()) {
                    return Err(LowerError::UnknownRole {
                        name: role.clone(),
                        span: *span,
                        known: self.roles.keys().map(|key| key.to_string()).collect(),
                    });
                }
                self.bindings.insert(name.clone(), role.clone());
            }
            if let Stmt::Checkpoint { name, .. } = statement {
                self.checkpoints.insert(name.clone());
            }
            for nested in nested_blocks(statement) {
                self.collect_bindings(nested)?;
            }
        }
        Ok(())
    }

    /// The effect set the source declares: bound roles' `requires`, plus called methods' `effects`.
    fn collect_declared_effects(&mut self, body: &[Stmt]) -> Result<(), LowerError> {
        for role in self.bindings.values() {
            if let Some(decl) = self.roles.get(role.as_str()) {
                for effect in &decl.requires {
                    self.declared_effects.insert(effect.text());
                }
            }
        }
        self.collect_called_method_effects(body);
        Ok(())
    }

    fn collect_called_method_effects(&mut self, body: &[Stmt]) {
        for statement in body {
            if let Stmt::Let { value, .. } = statement {
                if let Some(method) = asked_method(value) {
                    for effect in self.method_effects(&method) {
                        self.declared_effects.insert(effect);
                    }
                }
            }
            for nested in nested_blocks(statement) {
                self.collect_called_method_effects(nested);
            }
        }
    }

    fn method_effects(&self, method: &str) -> Vec<String> {
        self.interfaces
            .iter()
            .flat_map(|interface| interface.methods.iter())
            .filter(|declaration| declaration.name == method)
            .flat_map(|declaration| declaration.effects.iter().map(Path::text))
            .collect()
    }

    fn check_effects_against_policy(&self) -> Result<(), LowerError> {
        let allow: BTreeSet<String> = self.policy.allow_effects.iter().map(Path::text).collect();
        let deny: BTreeSet<String> = self.policy.deny_effects.iter().map(Path::text).collect();

        for effect in &self.declared_effects {
            let (role, span) = self.effect_origin(effect);
            if deny.contains(effect) {
                return Err(LowerError::DeniedEffect {
                    effect: effect.clone(),
                    role,
                    policy: self.policy.name.clone(),
                    span,
                });
            }
            if !allow.contains(effect) {
                return Err(LowerError::UndeclaredEffect {
                    effect: effect.clone(),
                    role,
                    policy: self.policy.name.clone(),
                    span,
                });
            }
        }
        Ok(())
    }

    /// Which role asked for an effect, so the diagnostic can name it rather than the policy alone.
    fn effect_origin(&self, effect: &str) -> (String, Span) {
        for role in self.bindings.values() {
            if let Some(decl) = self.roles.get(role.as_str()) {
                if let Some(path) = decl.requires.iter().find(|path| path.text() == effect) {
                    return (role.clone(), path.span);
                }
            }
        }
        for interface in self.interfaces {
            for method in &interface.methods {
                if let Some(path) = method.effects.iter().find(|path| path.text() == effect) {
                    return (format!("{}.{}", interface.name, method.name), path.span);
                }
            }
        }
        (String::new(), self.policy.span)
    }

    /// The compiler's check on its own output.
    fn verify_effect_preservation(&self) -> Result<(), LowerError> {
        for transition in &self.transitions {
            for effect in &transition.effects.world {
                if !self.declared_effects.contains(effect) {
                    return Err(LowerError::EffectIntroducedByLowering {
                        transition: transition.id.clone(),
                        effect: effect.clone(),
                        declared: self.declared_effects.iter().cloned().collect(),
                        span: self.policy.span,
                    });
                }
            }
        }
        Ok(())
    }

    fn role_of(&self, participant: &str, span: Span) -> Result<String, LowerError> {
        self.bindings
            .get(participant)
            .cloned()
            .ok_or_else(|| LowerError::UnboundParticipant {
                name: participant.to_string(),
                span,
            })
    }

    fn emit(&mut self, transition: TransitionIr) {
        self.states.insert(transition.from.clone());
        self.states.insert(transition.to.clone());
        self.transitions.push(transition);
    }

    fn human_required(&self, effects: &[String]) -> bool {
        let gated: BTreeSet<String> = self
            .policy
            .require_human_for
            .iter()
            .map(Path::text)
            .collect();
        effects.iter().any(|effect| gated.contains(effect))
    }

    fn lower_block(
        &mut self,
        body: &[Stmt],
        entry: &str,
        exit: &str,
        block: &'static str,
        span: Span,
    ) -> Result<(), LowerError> {
        let advancing = body.iter().filter(|statement| advances(statement)).count();
        if advancing == 0 {
            return Err(LowerError::EmptyBlock { block, span });
        }

        let mut current = entry.to_string();
        let mut seen = 0;
        for statement in body {
            if advances(statement) {
                seen += 1;
                let target = if seen == advancing {
                    exit.to_string()
                } else {
                    self.new_state()
                };
                self.lower_advancing(statement, &current, &target)?;
                current = target;
            } else {
                self.lower_static(statement, &current)?;
            }
        }
        Ok(())
    }

    fn lower_static(&mut self, statement: &Stmt, current: &str) -> Result<(), LowerError> {
        match statement {
            Stmt::Bind { .. } => {}
            Stmt::Checkpoint { name, .. } => {
                self.checkpoints.insert(name.clone());
                self.monitors.push(MonitorIr {
                    id: format!("monitor-state-{name}"),
                    kind: MonitorKind::StateVersionConflict,
                    subject: name.clone(),
                });
            }
            Stmt::Join { using, .. } => {
                self.monitors.push(MonitorIr {
                    id: format!("monitor-join-{using}"),
                    kind: MonitorKind::StateVersionConflict,
                    subject: using.clone(),
                });
            }
            Stmt::Context(context) => {
                self.monitors.push(MonitorIr {
                    id: format!("monitor-flow-{}", context.recipient),
                    kind: MonitorKind::InformationFlow,
                    subject: context.recipient.clone(),
                });
            }
            Stmt::Watch {
                subject, actions, ..
            } => {
                self.monitors.push(MonitorIr {
                    id: format!("monitor-watch-{subject}"),
                    kind: MonitorKind::MessageOrder,
                    subject: subject.clone(),
                });
                for action in actions {
                    if let WatchAction::PauseEffects { effects, .. } = action {
                        for effect in effects {
                            self.monitors.push(MonitorIr {
                                id: format!("monitor-pause-{}", effect.text()),
                                kind: MonitorKind::AuthorityValidity,
                                subject: effect.text(),
                            });
                        }
                    }
                }
            }
            Stmt::Stop { outcome, span, .. } => {
                let terminal = format!("stopped-{outcome}");
                self.terminal_states.insert(terminal.clone());
                self.emit(TransitionIr {
                    id: format!("stop-{outcome}"),
                    from: current.to_string(),
                    to: terminal,
                    actor_role: String::new(),
                    act: ActKind::Attest.as_str().to_string(),
                    payload_type: "aurora:weave/termination@1".to_string(),
                    guard: vec![outcome.clone()],
                    effects: TransitionEffects {
                        ledger: vec!["close_thread".to_string()],
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: false,
                });
                let _ = span;
            }
            Stmt::Return { .. } => {
                let terminal = "returned".to_string();
                self.terminal_states.insert(terminal.clone());
                self.emit(TransitionIr {
                    id: "return".to_string(),
                    from: current.to_string(),
                    to: terminal,
                    actor_role: String::new(),
                    act: ActKind::Attest.as_str().to_string(),
                    payload_type: "aurora:weave/result@1".to_string(),
                    guard: Vec::new(),
                    effects: TransitionEffects {
                        ledger: vec!["close_thread".to_string()],
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: false,
                });
            }
            Stmt::Let {
                attributes, name, ..
            } => {
                self.record_hooks(attributes, name);
            }
            other => {
                debug_assert!(
                    !advances(other),
                    "an advancing statement reached lower_static"
                );
            }
        }
        Ok(())
    }

    fn record_hooks(&mut self, attributes: &[Attribute], name: &str) {
        for attribute in attributes {
            if attribute.name != "decision-cell" {
                continue;
            }
            let capability = attribute
                .arguments
                .iter()
                .find(|(key, _)| key == "capability")
                .and_then(|(_, value)| match value {
                    Literal::Text(text) => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            self.hooks.push(EvaluationHookIr {
                id: format!("hook-{name}"),
                capability,
                snapshot_include: vec![
                    "world".to_string(),
                    "local-view".to_string(),
                    "evidence-ledger".to_string(),
                    "candidates".to_string(),
                    "budgets".to_string(),
                ],
                counterfactual_dimensions: vec![
                    "context_policy".to_string(),
                    "model".to_string(),
                    "communication_resolution".to_string(),
                ],
            });
            self.monitors.push(MonitorIr {
                id: format!("monitor-hook-{name}"),
                kind: MonitorKind::EvaluationHook,
                subject: name.to_string(),
            });
        }
    }

    fn lower_advancing(
        &mut self,
        statement: &Stmt,
        current: &str,
        target: &str,
    ) -> Result<(), LowerError> {
        match statement {
            Stmt::Let {
                attributes,
                name,
                value,
                span,
            } => {
                self.record_hooks(attributes, name);
                let Some((participant, method)) = ask_target(value) else {
                    return Err(LowerError::InvalidAskTarget { span: *span });
                };
                let role = self.role_of(&participant, *span)?;
                let mut effects = self.method_effects(&method);
                if effects.is_empty() {
                    effects = self
                        .roles
                        .get(role.as_str())
                        .map(|decl| decl.requires.iter().map(Path::text).collect())
                        .unwrap_or_default();
                }
                let human = self.human_required(&effects);
                self.emit(TransitionIr {
                    id: format!("ask-{name}"),
                    from: current.to_string(),
                    to: target.to_string(),
                    actor_role: role,
                    act: ActKind::Ask.as_str().to_string(),
                    payload_type: format!("aurora:weave/request@1#{method}"),
                    guard: Vec::new(),
                    effects: TransitionEffects {
                        ledger: vec!["append_event".to_string()],
                        mutating: effects
                            .iter()
                            .filter(|effect| mutates_world(effect))
                            .cloned()
                            .collect(),
                        world: effects,
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: human,
                });
            }
            Stmt::Send {
                act,
                from,
                to,
                span,
                ..
            } => {
                let role = self.role_of(from, *span)?;
                self.role_of(to, *span)?;
                let kind = kernel_act(act).unwrap_or(ActKind::Ask);
                self.emit(TransitionIr {
                    id: format!("send-{act}-{from}-{to}"),
                    from: current.to_string(),
                    to: target.to_string(),
                    actor_role: role,
                    act: kind.as_str().to_string(),
                    payload_type: format!("aurora:weave/{act}@1"),
                    guard: Vec::new(),
                    effects: TransitionEffects {
                        ledger: vec!["append_event".to_string()],
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: false,
                });
                self.monitors.push(MonitorIr {
                    id: format!("monitor-order-{act}"),
                    kind: MonitorKind::MessageOrder,
                    subject: act.clone(),
                });
            }
            Stmt::Match {
                scrutinee,
                arms,
                span,
            } => {
                let decider = await_target(scrutinee)
                    .and_then(|path| self.bindings.get(path.head()).cloned());
                for arm in arms {
                    let arm_entry = self.new_state();
                    let kind = kernel_act(&arm.pattern.case).unwrap_or(ActKind::Ask);
                    self.emit(TransitionIr {
                        id: format!("choice-{}", arm.pattern.case),
                        from: current.to_string(),
                        to: arm_entry.clone(),
                        actor_role: decider.clone().unwrap_or_default(),
                        act: kind.as_str().to_string(),
                        payload_type: format!("aurora:weave/{}@1", arm.pattern.case),
                        guard: vec![format!("decision == {}", arm.pattern.case)],
                        effects: TransitionEffects {
                            ledger: vec!["append_event".to_string()],
                            ..TransitionEffects::default()
                        },
                        requires_human_approval: false,
                    });
                    self.lower_block(&arm.body, &arm_entry, target, "match arm", arm.span)?;
                }
                let _ = span;
            }
            // No scheduler: `par` lowers to a fixed interleaving in source order, which is what
            // makes a trace reproducible. Concurrency is a runtime concern this crate does not own.
            Stmt::Par { body, span } => {
                self.lower_block(body, current, target, "par", *span)?;
            }
            Stmt::Race { branches, span, .. } | Stmt::Fork { branches, span, .. } => {
                if let Stmt::Fork { from, .. } = statement {
                    if !self.checkpoints.contains(from) {
                        return Err(LowerError::UnknownCheckpoint {
                            name: from.clone(),
                            span: *span,
                        });
                    }
                }
                for branch in branches {
                    self.lease(branch)?;
                    let branch_entry = self.new_state();
                    self.emit(TransitionIr {
                        id: format!("branch-{}", branch.name),
                        from: current.to_string(),
                        to: branch_entry.clone(),
                        actor_role: String::new(),
                        act: ActKind::Delegate.as_str().to_string(),
                        payload_type: "aurora:weave/branch@1".to_string(),
                        guard: Vec::new(),
                        effects: TransitionEffects {
                            budget: branch
                                .budget
                                .iter()
                                .map(|grant| format!("reserve:{}", grant.resource))
                                .collect(),
                            authority: vec!["attenuate".to_string()],
                            ..TransitionEffects::default()
                        },
                        requires_human_approval: false,
                    });
                    self.lower_block(&branch.body, &branch_entry, target, "branch", branch.span)?;
                }
            }
            Stmt::Commit(commit) => {
                let debtor = self.role_of(&commit.debtor, commit.span)?;
                let creditor = self.role_of(&commit.creditor, commit.span)?;
                let proposed = self.new_state();
                let accepted = self.new_state();
                self.emit(TransitionIr {
                    id: format!("commit-propose-{}", commit.debtor),
                    from: current.to_string(),
                    to: proposed.clone(),
                    actor_role: debtor.clone(),
                    act: ActKind::Propose.as_str().to_string(),
                    payload_type: commit.deliver.text(),
                    guard: vec![commit.trigger.text()],
                    effects: TransitionEffects {
                        ledger: vec!["append_event".to_string()],
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: false,
                });
                self.emit(TransitionIr {
                    id: format!("commit-accept-{}", commit.creditor),
                    from: proposed,
                    to: accepted.clone(),
                    actor_role: creditor,
                    act: ActKind::Accept.as_str().to_string(),
                    payload_type: commit.deliver.text(),
                    guard: Vec::new(),
                    effects: TransitionEffects {
                        ledger: vec!["create_commitment".to_string()],
                        authority: vec!["issue_grant".to_string()],
                        budget: vec!["reserve".to_string()],
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: false,
                });
                // Discharging is where the commitment's quality predicates become real work, and
                // therefore where an effect the source never declared would show up.
                let satisfied: Vec<String> = commit.satisfy.iter().map(Path::text).collect();
                let human = self.human_required(&satisfied);
                self.emit(TransitionIr {
                    id: format!("commit-discharge-{}", commit.debtor),
                    from: accepted,
                    to: target.to_string(),
                    actor_role: debtor,
                    act: ActKind::Discharge.as_str().to_string(),
                    payload_type: commit.deliver.text(),
                    guard: satisfied.clone(),
                    effects: TransitionEffects {
                        ledger: vec!["discharge_commitment".to_string()],
                        mutating: satisfied
                            .iter()
                            .filter(|effect| mutates_world(effect))
                            .cloned()
                            .collect(),
                        world: satisfied,
                        ..TransitionEffects::default()
                    },
                    requires_human_approval: human,
                });
                self.monitors.push(MonitorIr {
                    id: format!("monitor-commitment-{}", commit.debtor),
                    kind: MonitorKind::CommitmentTransition,
                    subject: commit.debtor.clone(),
                });
                if commit.before.is_some() {
                    self.monitors.push(MonitorIr {
                        id: format!("monitor-deadline-{}", commit.debtor),
                        kind: MonitorKind::Timeout,
                        subject: commit.debtor.clone(),
                    });
                }
            }
            Stmt::Repeat { body, span, until } => {
                self.lower_block(body, current, target, "repeat", *span)?;
                if let Some(transition) = self
                    .transitions
                    .iter_mut()
                    .rev()
                    .find(|transition| transition.to == target)
                {
                    transition.guard.push(format!("until {}", render(until)));
                }
            }
            Stmt::Execute { span, .. } => {
                self.simple_act(current, target, ActKind::Discharge, "execute", *span)
            }
            Stmt::Resolve { span, .. } => {
                self.simple_act(current, target, ActKind::Attest, "resolve", *span)
            }
            Stmt::Delegate { span, .. } => {
                self.simple_act(current, target, ActKind::Delegate, "delegate", *span)
            }
            Stmt::Publish { span, .. } => {
                self.simple_act(current, target, ActKind::Claim, "publish", *span)
            }
            Stmt::Spawn { role, span } => {
                if !self.roles.contains_key(role.as_str()) {
                    return Err(LowerError::UnknownRole {
                        name: role.clone(),
                        span: *span,
                        known: self.roles.keys().map(|key| key.to_string()).collect(),
                    });
                }
                self.simple_act(current, target, ActKind::Delegate, "spawn", *span)
            }
            other => {
                debug_assert!(!advances(other), "unhandled advancing statement");
            }
        }
        Ok(())
    }

    fn simple_act(&mut self, current: &str, target: &str, kind: ActKind, label: &str, _span: Span) {
        self.emit(TransitionIr {
            id: format!("{label}-{}", self.transitions.len()),
            from: current.to_string(),
            to: target.to_string(),
            actor_role: String::new(),
            act: kind.as_str().to_string(),
            payload_type: format!("aurora:weave/{label}@1"),
            guard: Vec::new(),
            effects: TransitionEffects {
                ledger: vec!["append_event".to_string()],
                ..TransitionEffects::default()
            },
            requires_human_approval: false,
        });
    }

    /// Draws a branch's lease out of the policy ceiling using the kernel's own affine budget.
    fn lease(&mut self, branch: &Branch) -> Result<(), LowerError> {
        for grant in &branch.budget {
            let Some(resource) = kernel_resource(&grant.resource) else {
                return Err(LowerError::BudgetResourceNotAllocated {
                    branch: branch.name.clone(),
                    resource: grant.resource.clone(),
                    policy: self.policy.name.clone(),
                    span: grant.span,
                });
            };
            match self.ceiling.split(resource, grant.amount) {
                Ok(lease) => {
                    // The lease is dropped: an allowance handed to a branch is gone from the
                    // parent whether or not the branch spends it, which is what makes the sum
                    // across a fork conserved rather than merely bounded.
                    drop(lease);
                }
                Err(BudgetError::Unallocated(resource)) => {
                    return Err(LowerError::BudgetResourceNotAllocated {
                        branch: branch.name.clone(),
                        resource: format!("{resource:?}"),
                        policy: self.policy.name.clone(),
                        span: grant.span,
                    })
                }
                Err(BudgetError::Exhausted {
                    resource,
                    requested,
                    available,
                }) => {
                    return Err(LowerError::BudgetCeilingExceeded {
                        branch: branch.name.clone(),
                        resource,
                        requested,
                        available,
                        span: grant.span,
                    })
                }
            }
        }
        Ok(())
    }

    fn participants(&self, body: &[Stmt]) -> Vec<ParticipantIr> {
        let grade = required_grade(body);
        self.bindings
            .iter()
            .map(|(name, role)| ParticipantIr {
                id: name.clone(),
                role: role.clone(),
                required_abi_grade: grade,
                bound: false,
            })
            .collect()
    }

    fn state_graph(&self) -> StateGraph {
        let nodes = self
            .states
            .iter()
            .map(|state| {
                let outgoing: Vec<&TransitionIr> = self
                    .transitions
                    .iter()
                    .filter(|transition| &transition.from == state)
                    .collect();
                StateNode {
                    id: state.clone(),
                    enabled_acts: {
                        let mut acts: Vec<String> = outgoing
                            .iter()
                            .map(|transition| transition.act.clone())
                            .collect();
                        acts.sort();
                        acts.dedup();
                        acts
                    },
                    outstanding_commitments: outgoing
                        .iter()
                        .filter(|transition| {
                            transition
                                .effects
                                .ledger
                                .iter()
                                .any(|entry| entry == "create_commitment")
                        })
                        .map(|transition| transition.id.clone())
                        .collect(),
                    effect_bounds: {
                        let mut bounds: Vec<String> = outgoing
                            .iter()
                            .flat_map(|transition| transition.effects.world.iter().cloned())
                            .collect();
                        bounds.sort();
                        bounds.dedup();
                        bounds
                    },
                    guards: {
                        let mut guards: Vec<String> = outgoing
                            .iter()
                            .flat_map(|transition| transition.guard.iter().cloned())
                            .collect();
                        guards.sort();
                        guards.dedup();
                        guards
                    },
                }
            })
            .collect();
        StateGraph {
            nodes,
            transitions: self.transitions.clone(),
        }
    }
}

/// The cognitive ABI grade a program demands of whoever fills a role (23.49).
///
/// 23.49 names the grade but gives no table mapping language constructs onto it, so this is the
/// crate's rule, stated rather than hidden: a program that forks needs participants that can hold
/// an exact continuation, one that only checkpoints needs a lossy one, and one that does neither
/// needs no resumption at all.
fn required_grade(body: &[Stmt]) -> u8 {
    let mut grade = 1;
    for statement in body {
        match statement {
            Stmt::Fork { .. } => return 3,
            Stmt::Checkpoint { .. } => grade = grade.max(2),
            _ => {}
        }
        for nested in nested_blocks(statement) {
            grade = grade.max(required_grade(nested));
        }
    }
    grade
}

/// Whether a statement moves the choreography from one state to the next.
fn advances(statement: &Stmt) -> bool {
    match statement {
        Stmt::Let { value, .. } => ask_target(value).is_some(),
        Stmt::Send { .. }
        | Stmt::Match { .. }
        | Stmt::Par { .. }
        | Stmt::Race { .. }
        | Stmt::Fork { .. }
        | Stmt::Commit(_)
        | Stmt::Repeat { .. }
        | Stmt::Execute { .. }
        | Stmt::Resolve { .. }
        | Stmt::Delegate { .. }
        | Stmt::Publish { .. }
        | Stmt::Spawn { .. } => true,
        Stmt::Bind { .. }
        | Stmt::Checkpoint { .. }
        | Stmt::Join { .. }
        | Stmt::Context(_)
        | Stmt::Watch { .. }
        | Stmt::Stop { .. }
        | Stmt::Return { .. } => false,
    }
}

fn nested_blocks(statement: &Stmt) -> Vec<&[Stmt]> {
    match statement {
        Stmt::Par { body, .. } | Stmt::Repeat { body, .. } => vec![body.as_slice()],
        Stmt::Race { branches, .. } | Stmt::Fork { branches, .. } => branches
            .iter()
            .map(|branch| branch.body.as_slice())
            .collect(),
        Stmt::Match { arms, .. } => arms.iter().map(|arm| arm.body.as_slice()).collect(),
        _ => Vec::new(),
    }
}

/// The participant and method an `ask` expression targets, if it is one.
fn ask_target(expr: &Expr) -> Option<(String, String)> {
    let Expr::Ask { call, .. } = expr else {
        return None;
    };
    let path = match call.as_ref() {
        Expr::Call { callee, .. } => callee,
        Expr::Path(path) => path,
        _ => return None,
    };
    let participant = path.segments.first()?.clone();
    let method = path.segments.get(1).cloned().unwrap_or_default();
    Some((participant, method))
}

fn asked_method(expr: &Expr) -> Option<String> {
    ask_target(expr).map(|(_, method)| method)
}

fn await_target(expr: &Expr) -> Option<&Path> {
    match expr {
        Expr::Await { target, .. } => Some(target),
        _ => None,
    }
}

/// Renders an expression back to source-like text for a transition guard.
fn render(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.text(),
        Expr::Call { callee, .. } => format!("{}(..)", callee.text()),
        Expr::Ask { call, .. } => format!("ask {}", render(call)),
        Expr::Await { target, .. } => format!("await {}", target.text()),
        Expr::Choose { subject, by, .. } => format!("choose {subject} by {}", by.text()),
        Expr::Current { .. } => "current".to_string(),
        Expr::Literal { value, .. } => match value {
            Literal::Integer(value) => value.to_string(),
            Literal::Float(value) => value.to_string(),
            Literal::Duration { text, .. } => text.clone(),
            Literal::Text(text) => format!("{text:?}"),
            Literal::Money {
                currency,
                minor_units,
            } => format!("{currency}({minor_units})"),
        },
        Expr::Binary {
            op, left, right, ..
        } => format!("{} {} {}", render(left), op.as_str(), render(right)),
    }
}
