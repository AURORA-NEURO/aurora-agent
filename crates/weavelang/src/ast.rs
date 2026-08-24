//! The WeaveLang abstract syntax tree.
//!
//! Blueprint 23.03 phase 1 asks the parser to "attach stable source locations", so every node here
//! carries a [`Span`]. Nothing is desugared at this stage: `race first valid` and `fork` keep their
//! surface shape, and normalization into explicit acts and policies happens in [`crate::lower`],
//! which is where 23.03 puts it.
//!
//! The tree covers the declarations 23.37 gives a grammar for — package, import, type, interface,
//! role, policy, choreography and weave — and the statements its `weave` and control-flow examples
//! use. 23.02 adds three forms 23.37 does not repeat (`molecule`, `thread`, `shared`) and they are
//! **not** parsed here; the divergence is recorded in the crate docs rather than guessed at, since
//! 23.02 gives `molecule` a body grammar 23.37 never confirms.
//!
//! The AST is `Serialize` but not `Deserialize`: it is an intermediate the compiler owns, and the
//! artifact meant to be exchanged, signed and replayed is WeaveIR (23.03: "the semantic IR remains
//! the signed source of truth"). Offering a deserializer would invite a caller to hand-build a tree
//! that never went through the parser.

use crate::diagnostic::Span;
use serde::Serialize;

/// A dotted name such as `repo.read`, `lead.plan` or `tests.pass`.
///
/// One type for effect names, member accesses and predicates, because WeaveLang writes all three
/// identically and the distinction is a matter of position, resolved during lowering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Path {
    pub segments: Vec<String>,
    pub span: Span,
}

impl Path {
    pub fn text(&self) -> String {
        self.segments.join(".")
    }

    pub fn head(&self) -> &str {
        self.segments.first().map(String::as_str).unwrap_or("")
    }
}

/// A package-qualified name: `namespace:name/item@version` (23.37).
///
/// `item` and `version` are optional because `import aurora:core@0.1` omits the item and
/// `interface aurora:repair/fix` may omit the version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualifiedName {
    pub namespace: String,
    pub name: String,
    pub item: Option<String>,
    pub version: Option<String>,
    pub span: Span,
}

impl QualifiedName {
    /// The canonical rendering, which is also the identity used in package-lock hashing.
    pub fn text(&self) -> String {
        let mut out = format!("{}:{}", self.namespace, self.name);
        if let Some(item) = &self.item {
            out.push('/');
            out.push_str(item);
        }
        if let Some(version) = &self.version {
            out.push('@');
            out.push_str(version);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Duration {
        millis: u64,
        text: String,
    },
    Text(String),
    /// `usd(5)` and friends: a currency tag with an amount in minor units.
    Money {
        currency: String,
        minor_units: i64,
    },
}

/// A type expression: `probability`, `list<assumption-ref>`, `Claim<P>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeRef {
    pub name: String,
    pub arguments: Vec<TypeRef>,
    pub span: Span,
}

impl TypeRef {
    /// The canonical rendering used as a payload type in WeaveIR.
    pub fn text(&self) -> String {
        if self.arguments.is_empty() {
            self.name.clone()
        } else {
            let inner: Vec<String> = self.arguments.iter().map(TypeRef::text).collect();
            format!("{}<{}>", self.name, inner.join(","))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Program {
    pub package: Option<PackageDecl>,
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageDecl {
    pub name: QualifiedName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportDecl {
    pub name: QualifiedName,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Item {
    Type(TypeDecl),
    Interface(InterfaceDecl),
    Role(RoleDecl),
    Policy(PolicyDecl),
    Choreography(ChoreographyDecl),
    Weave(WeaveDecl),
}

impl Item {
    pub fn name(&self) -> &str {
        match self {
            Item::Type(decl) => &decl.name,
            Item::Interface(decl) => &decl.name,
            Item::Role(decl) => &decl.name,
            Item::Policy(decl) => &decl.name,
            Item::Choreography(decl) => &decl.name,
            Item::Weave(decl) => &decl.name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Item::Type(decl) => decl.span,
            Item::Interface(decl) => decl.span,
            Item::Role(decl) => decl.span,
            Item::Policy(decl) => decl.span,
            Item::Choreography(decl) => decl.span,
            Item::Weave(decl) => decl.span,
        }
    }

    /// The declaration keyword, for "duplicate declaration" diagnostics.
    pub fn keyword(&self) -> &'static str {
        match self {
            Item::Type(_) => "type",
            Item::Interface(_) => "interface",
            Item::Role(_) => "role",
            Item::Policy(_) => "policy",
            Item::Choreography(_) => "choreography",
            Item::Weave(_) => "weave",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeDecl {
    pub name: String,
    pub body: TypeBody,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TypeBody {
    Record(Vec<Param>),
    /// A variant case carries at most one payload type, which is what 23.37's `outcome` shows.
    Variant(Vec<VariantCase>),
    Alias(TypeRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantCase {
    pub name: String,
    pub payload: Option<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InterfaceDecl {
    pub name: String,
    pub methods: Vec<MethodDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MethodDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: Option<TypeRef>,
    /// The effect set of 23.04: part of the type, not documentation.
    pub effects: Vec<Path>,
    pub throws: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoleDecl {
    pub name: String,
    /// Capability names, optionally versioned: `challenge@1`.
    pub provides: Vec<VersionedName>,
    /// Effects this role needs in order to do its job.
    pub requires: Vec<Path>,
    pub clearance: Option<Clearance>,
    pub minimum_profile: Option<MinimumProfile>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionedName {
    pub name: String,
    pub version: Option<String>,
    pub span: Span,
}

impl VersionedName {
    pub fn text(&self) -> String {
        match &self.version {
            Some(version) => format!("{}@{}", self.name, version),
            None => self.name.clone(),
        }
    }
}

/// `clearance confidential/research` — a level and an optional compartment (23.04's label lattice).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Clearance {
    pub level: String,
    pub compartments: Vec<String>,
    pub span: Span,
}

/// `minimum-profile prism://capability/challenge >= 0.80`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MinimumProfile {
    pub reference: String,
    pub threshold: f64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PolicyDecl {
    pub name: String,
    pub allow_effects: Vec<Path>,
    pub deny_effects: Vec<Path>,
    /// `require human for [main.merge]` — the approval gate 23.03 phase 4 demands for
    /// irreversible effects.
    pub require_human_for: Vec<Path>,
    pub budgets: Vec<BudgetLimit>,
    pub max_participants: Option<u64>,
    pub span: Span,
}

/// `budget tokens <= 120000` or `budget money <= usd(5)`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BudgetLimit {
    pub resource: String,
    pub limit: Literal,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChoreographyDecl {
    pub name: String,
    pub steps: Vec<ChoreoStep>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ChoreoStep {
    /// `Lead -> Reviewer: propose<Plan>`
    Message {
        from: String,
        to: String,
        act: String,
        payload: Option<TypeRef>,
        span: Span,
    },
    /// `choice by Reviewer { accept: ... challenge: ... }`
    Choice {
        by: String,
        branches: Vec<ChoiceBranch>,
        span: Span,
    },
}

impl ChoreoStep {
    pub fn span(&self) -> Span {
        match self {
            ChoreoStep::Message { span, .. } | ChoreoStep::Choice { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChoiceBranch {
    pub label: String,
    pub steps: Vec<ChoreoStep>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeaveDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: Option<TypeRef>,
    /// The policy named by `using safe-repair`. Optional in the grammar; a program without one
    /// declares no ceiling, which lowering reports rather than treating as unlimited.
    pub using_policy: Option<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// `@decision-cell(capability="context.information-value")` (23.37, "Evaluation hook").
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<(String, Literal)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Stmt {
    /// `bind lead to role Lead`
    Bind {
        name: String,
        role: String,
        span: Span,
    },
    /// `let plan = ask lead.plan(issue)`, optionally attributed as an evaluation hook.
    Let {
        attributes: Vec<Attribute>,
        name: String,
        value: Expr,
        span: Span,
    },
    /// `send propose(plan) from lead to reviewer`
    Send {
        act: String,
        arguments: Vec<Expr>,
        from: String,
        to: String,
        span: Span,
    },
    /// `match await reviewer.decision { accept(p) => execute p }`
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `par { ... }` — concurrent, not parallel: 23.34 gives no scheduler, and neither does this
    /// crate. Lowering records the branches as unordered, and the evaluator interleaves them in a
    /// fixed order so that a trace is reproducible.
    Par {
        body: Vec<Stmt>,
        span: Span,
    },
    /// `race first valid { branch fast-model { ... } }`
    Race {
        policy: String,
        branches: Vec<Branch>,
        span: Span,
    },
    /// `checkpoint c = current`
    Checkpoint {
        name: String,
        source: String,
        span: Span,
    },
    /// `fork from c { branch h1 with budget tokens(10000) { ... } }`
    Fork {
        from: String,
        branches: Vec<Branch>,
        span: Span,
    },
    /// `join using verified-best`
    Join {
        using: String,
        span: Span,
    },
    /// `commit worker to lead when task.accepted { ... }`
    Commit(CommitStmt),
    /// `watch evidence where contradiction.blocking { ... }`
    Watch {
        subject: String,
        condition: Path,
        actions: Vec<WatchAction>,
        span: Span,
    },
    /// `context for skeptic { include ... }`
    Context(ContextStmt),
    /// `stop success when commitments.all-closed and verifier.pass`
    Stop {
        outcome: String,
        condition: Expr,
        span: Span,
    },
    Return {
        value: Expr,
        span: Span,
    },
    /// `execute p` and `resolve c` in 23.37's match arms.
    Execute {
        value: Expr,
        span: Span,
    },
    Resolve {
        value: Expr,
        span: Span,
    },
    /// `publish finding into evidence` (23.02).
    Publish {
        value: Expr,
        into: String,
        span: Span,
    },
    /// `repeat until evidence.separates(h) or budget.low { ... }` (23.02).
    Repeat {
        until: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `spawn role skeptic`
    Spawn {
        role: String,
        span: Span,
    },
    /// `delegate patcher.produce-patch(x)` (23.02).
    Delegate {
        value: Expr,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Bind { span, .. }
            | Stmt::Let { span, .. }
            | Stmt::Send { span, .. }
            | Stmt::Match { span, .. }
            | Stmt::Par { span, .. }
            | Stmt::Race { span, .. }
            | Stmt::Checkpoint { span, .. }
            | Stmt::Fork { span, .. }
            | Stmt::Join { span, .. }
            | Stmt::Watch { span, .. }
            | Stmt::Stop { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Execute { span, .. }
            | Stmt::Resolve { span, .. }
            | Stmt::Publish { span, .. }
            | Stmt::Repeat { span, .. }
            | Stmt::Spawn { span, .. }
            | Stmt::Delegate { span, .. } => *span,
            Stmt::Commit(stmt) => stmt.span,
            Stmt::Context(stmt) => stmt.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommitStmt {
    pub debtor: String,
    pub creditor: String,
    pub trigger: Path,
    pub deliver: TypeRef,
    pub before: Option<Literal>,
    pub satisfy: Vec<Path>,
    pub compensate: Option<Compensation>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Compensation {
    pub action: String,
    pub on: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContextStmt {
    pub recipient: String,
    pub includes: Vec<Include>,
    pub resolution: Option<String>,
    pub max_tokens: Option<u64>,
    pub span: Span,
}

/// `include evidence strongest both-sides`, `include assumptions unresolved top 5`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Include {
    pub subject: String,
    pub selectors: Vec<String>,
    pub limit: Option<u64>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum WatchAction {
    /// `pause effects [publish, main.merge]`
    PauseEffects {
        effects: Vec<Path>,
        span: Span,
    },
    SpawnRole {
        role: String,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Branch {
    pub name: String,
    /// `with budget tokens(10000)` — a *lease*, in 23.04's sense: moved out of the parent, not
    /// copied into the branch.
    pub budget: Vec<BudgetGrant>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetGrant {
    pub resource: String,
    pub amount: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pattern {
    pub case: String,
    pub binding: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Expr {
    Path(Path),
    Call {
        callee: Path,
        arguments: Vec<Argument>,
        span: Span,
    },
    /// `ask lead.plan(issue)` — a request act, not a function call.
    Ask {
        call: Box<Expr>,
        span: Span,
    },
    /// `await reviewer.decision`
    Await {
        target: Path,
        span: Span,
    },
    /// `choose evidence by information-value`
    Choose {
        subject: String,
        by: Path,
        span: Span,
    },
    Literal {
        value: Literal,
        span: Span,
    },
    /// The `current` keyword in `checkpoint c = current`.
    Current {
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Path(path) => path.span,
            Expr::Call { span, .. }
            | Expr::Ask { span, .. }
            | Expr::Await { span, .. }
            | Expr::Choose { span, .. }
            | Expr::Literal { span, .. }
            | Expr::Current { span }
            | Expr::Binary { span, .. } => *span,
        }
    }
}

/// A call argument, optionally named: `ask lead.generate-hypotheses(issue, limit: 3)` (23.02).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Argument {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    And,
    Or,
    LessEq,
    GreaterEq,
    Less,
    Greater,
}

impl BinaryOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::LessEq => "<=",
            BinaryOp::GreaterEq => ">=",
            BinaryOp::Less => "<",
            BinaryOp::Greater => ">",
        }
    }
}
