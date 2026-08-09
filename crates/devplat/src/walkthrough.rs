//! A quickstart, as a value whose standing is computed rather than asserted.
//!
//! The developer-platform section asks repeatedly for onboarding material — scaffolding that
//! generates "tests and documentation stubs, not empty code alone", an authoring loop written as
//! an arrow diagram, a workflow snippet a consumer pastes into CI. None of those modules says what
//! a quickstart *is*, so this crate says it: an ordered list of steps, each of which either names
//! an API (and is therefore a claim about some artifact) or declares itself narration and says
//! why.
//!
//! [`Walkthrough::standing`] is the payoff. It is derived from the claims, so a document cannot
//! advertise itself as verified. Three values:
//!
//! - [`Standing::CheckableHere`] — every claim is about a crate of this workspace. A test can
//!   refute the document.
//! - [`Standing::PartlyOutside`] — some claims are about a foreign artifact. The document is
//!   partially guarded, and the report has to say which part.
//! - [`Standing::EntirelyOutside`] — no claim can be checked from this repository. The document is
//!   still worth writing; it is simply not evidence about this workspace, and
//!   [`Walkthrough::documents_absent_artifact`] returns true so a reader is told.
//!
//! # Why this is not a second cookbook
//!
//! `bioprism-cookbook` holds recipes: goal, steps, the claim demonstrated, at least one checkable
//! property, the pitfall. Every recipe it can hold is in-tree, because [`Check::EnforcedByTest`]
//! names a workspace test. A walkthrough here is the *complement*: the documents whose subject is
//! mostly not in this repository, which the recipe type correctly refuses to represent. The two
//! catalogues are asserted disjoint in [`crate::audit`], and this crate defines no recipe, no
//! anti-recipe and no second `Check`.
//!
//! [`Check::EnforcedByTest`]: bioprism_cookbook::Check::EnforcedByTest

use serde::{Deserialize, Serialize};

use crate::claim::{ApiClaim, ApiName, Evidence};
use crate::error::WalkthroughError;
use crate::surface::{Locale, Surface, SurfaceKind};

/// A walkthrough identifier: lower-case, hyphenated, no whitespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WalkthroughId(String);

impl WalkthroughId {
    pub fn parse(value: impl Into<String>) -> Result<Self, WalkthroughError> {
        let value: String = value.into();
        let shaped = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !value.starts_with('-')
            && !value.ends_with('-');
        if !shaped {
            return Err(WalkthroughError::MalformedId { id: value });
        }
        Ok(WalkthroughId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a step is, once you insist that it be one thing or the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "body", rename_all = "snake_case")]
pub enum StepBody {
    /// The step tells the reader to call something. That is a claim.
    Names(ApiClaim),
    /// The step explains, orients or warns, and names nothing.
    ///
    /// Allowed, and required to justify itself. Narration is how a quickstart quietly stops being
    /// checkable: prose accretes, claims do not, and the document keeps its green badge.
    Narration { because: String },
}

/// One step of a walkthrough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// What the reader does, in the imperative.
    pub instruction: String,
    pub body: StepBody,
}

impl Step {
    /// A step that names an API.
    pub fn naming(instruction: impl Into<String>, claim: ApiClaim) -> Self {
        Step {
            instruction: instruction.into(),
            body: StepBody::Names(claim),
        }
    }

    /// A step that names nothing, and says why that is right.
    pub fn narrating(instruction: impl Into<String>, because: impl Into<String>) -> Self {
        Step {
            instruction: instruction.into(),
            body: StepBody::Narration {
                because: because.into(),
            },
        }
    }

    pub fn claim(&self) -> Option<&ApiClaim> {
        match &self.body {
            StepBody::Names(claim) => Some(claim),
            StepBody::Narration { .. } => None,
        }
    }
}

/// How much of a document this repository can be asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "standing", rename_all = "snake_case")]
pub enum Standing {
    /// Every claim names a crate of this workspace.
    CheckableHere { claims: usize },
    /// Mixed. `here` claims are guarded; `outside` claims are not and never will be.
    PartlyOutside { here: usize, outside: usize },
    /// No claim is about this repository.
    EntirelyOutside { claims: usize },
}

impl Standing {
    pub fn as_str(self) -> &'static str {
        match self {
            Standing::CheckableHere { .. } => "checkable here",
            Standing::PartlyOutside { .. } => "partly outside",
            Standing::EntirelyOutside { .. } => "entirely outside",
        }
    }

    /// Claims a workspace test could refute.
    pub fn guarded_claims(self) -> usize {
        match self {
            Standing::CheckableHere { claims } => claims,
            Standing::PartlyOutside { here, .. } => here,
            Standing::EntirelyOutside { .. } => 0,
        }
    }

    /// Claims no workspace test will ever refute.
    pub fn unguarded_claims(self) -> usize {
        match self {
            Standing::CheckableHere { .. } => 0,
            Standing::PartlyOutside { outside, .. } => outside,
            Standing::EntirelyOutside { claims } => claims,
        }
    }
}

/// An onboarding document, with its claims made explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WalkthroughWire")]
pub struct Walkthrough {
    id: WalkthroughId,
    goal: String,
    subject: Surface,
    steps: Vec<Step>,
}

impl Walkthrough {
    pub fn draft(id: WalkthroughId, goal: impl Into<String>, subject: Surface) -> WalkthroughDraft {
        WalkthroughDraft {
            id,
            goal: goal.into(),
            subject,
            steps: Vec::new(),
        }
    }

    pub fn id(&self) -> &WalkthroughId {
        &self.id
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// The artifact the document is *about*, which is not always where its claims live: an MCP
    /// quickstart is about a wire protocol and still names a crate that serves it.
    pub fn subject(&self) -> &Surface {
        &self.subject
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn claims(&self) -> Vec<&ApiClaim> {
        self.steps.iter().filter_map(Step::claim).collect()
    }

    /// Derived from the claims. There is no setter and no override.
    pub fn standing(&self) -> Standing {
        let claims = self.claims();
        let here = claims
            .iter()
            .filter(|claim| claim.surface().locale() == Locale::InRepository)
            .count();
        let outside = claims.len() - here;
        match (here, outside) {
            (0, n) => Standing::EntirelyOutside { claims: n },
            (n, 0) => Standing::CheckableHere { claims: n },
            (here, outside) => Standing::PartlyOutside { here, outside },
        }
    }

    /// Whether this document is entirely about something this repository does not contain.
    ///
    /// Not a criticism of the document. A Python quickstart is the right document to write for a
    /// Python SDK. It is a statement about what a green test run here does and does not mean.
    pub fn documents_absent_artifact(&self) -> bool {
        matches!(self.standing(), Standing::EntirelyOutside { .. })
    }

    /// Claims that currently contradict the working tree.
    pub fn refuted_claims(&self) -> Vec<&ApiClaim> {
        self.claims()
            .into_iter()
            .filter(|claim| claim.is_refuted())
            .collect()
    }

    /// The fraction of steps that name nothing, in tenths of a percent, so the figure is exact.
    ///
    /// Integer arithmetic rather than a float because this number appears in a digest.
    pub fn narration_permille(&self) -> u32 {
        if self.steps.is_empty() {
            return 0;
        }
        let narration = self.steps.len() - self.claims().len();
        u32::try_from(narration * 1000 / self.steps.len()).unwrap_or(u32::MAX)
    }
}

/// An unsealed walkthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkthroughDraft {
    id: WalkthroughId,
    goal: String,
    subject: Surface,
    steps: Vec<Step>,
}

impl WalkthroughDraft {
    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Refuse rather than return a weaker document.
    pub fn seal(self) -> Result<Walkthrough, WalkthroughError> {
        let id = self.id.as_str().to_string();
        if self.goal.trim().is_empty() {
            return Err(WalkthroughError::NoGoal { id });
        }
        if self.steps.is_empty() {
            return Err(WalkthroughError::NoSteps { id });
        }
        for (index, step) in self.steps.iter().enumerate() {
            let ordinal = index + 1;
            if step.instruction.trim().is_empty() {
                return Err(WalkthroughError::EmptyInstruction { id, ordinal });
            }
            if let StepBody::Narration { because } = &step.body {
                if because.trim().is_empty() {
                    return Err(WalkthroughError::UndeclaredNarration { id, ordinal });
                }
            }
        }
        if self.steps.iter().all(|step| step.claim().is_none()) {
            return Err(WalkthroughError::EntirelyNarration { id });
        }
        Ok(Walkthrough {
            id: self.id,
            goal: self.goal,
            subject: self.subject,
            steps: self.steps,
        })
    }
}

#[derive(Deserialize)]
struct WalkthroughWire {
    id: WalkthroughId,
    goal: String,
    subject: Surface,
    steps: Vec<Step>,
}

impl TryFrom<WalkthroughWire> for Walkthrough {
    type Error = WalkthroughError;

    fn try_from(wire: WalkthroughWire) -> Result<Self, Self::Error> {
        let mut draft = Walkthrough::draft(wire.id, wire.goal, wire.subject);
        for step in wire.steps {
            draft = draft.step(step);
        }
        draft.seal()
    }
}

fn foreign_claim(
    api: &str,
    kind: SurfaceKind,
    artifact: &str,
    reason: &str,
) -> Result<ApiClaim, WalkthroughError> {
    let surface = Surface::foreign(kind, artifact).expect("static surfaces are well formed");
    ApiClaim::about(
        ApiName::parse(api).expect("static api names are non-empty"),
        surface,
    )
    .outside(reason)
    .seal()
    .map_err(|_| WalkthroughError::MalformedId {
        id: api.to_string(),
    })
}

fn rust_claim(api: &str, krate: &str, file: &str) -> Result<ApiClaim, WalkthroughError> {
    let name = bioprism_cookbook::CrateName::parse(krate).expect("static crate names are valid");
    let surface = Surface::rust(&name).expect("static crate names are workspace crates");
    ApiClaim::about(
        ApiName::parse(api).expect("static api names are non-empty"),
        surface,
    )
    .resolved_in(file)
    .seal()
    .map_err(|_| WalkthroughError::MalformedId {
        id: api.to_string(),
    })
}

/// The quickstarts the developer-platform section's remaining modules assume.
///
/// Six documents. Their standings are the finding: the two the section actually writes out — the
/// Python SDK's example block and the CI workflow snippet — are entirely outside this repository,
/// and the three this crate can guarantee are the three subjects it implemented. The MCP
/// quickstart is the interesting one, because it is genuinely mixed: the tool names are a wire
/// contract and the server that answers them is a crate.
pub fn standard_walkthroughs() -> Result<Vec<Walkthrough>, WalkthroughError> {
    let python_reason = "the artifact is an importable Python distribution; no file in this \
                         checkout defines it, and no test here can import it";
    let ci_reason = "the artifact is a workflow file evaluated by a CI provider in a consumer's \
                     repository";
    let mcp_reason = "a tool name is a wire-protocol string agreed between client and server; \
                      resolving it here would check the server's spelling, not the protocol";

    let python = Walkthrough::draft(
        WalkthroughId::parse("python-sdk-quickstart")?,
        "Compile decision cells from an imported trace and compare two architectures, following \
         the developer-platform section's own Python example.",
        Surface::foreign(SurfaceKind::PythonPackage, "prism_sdk")
            .expect("static surface is well formed"),
    )
    .step(Step::naming(
        "Open a local platform handle.",
        foreign_claim(
            "Prism.local",
            SurfaceKind::PythonPackage,
            "prism_sdk",
            python_reason,
        )?,
    ))
    .step(Step::naming(
        "Import a trace from a JSONL file through the OpenTelemetry adapter.",
        foreign_claim(
            "prism.traces.import_path",
            SurfaceKind::PythonPackage,
            "prism_trace",
            python_reason,
        )?,
    ))
    .step(Step::naming(
        "Mine decision cells at the first divergence.",
        foreign_claim(
            "prism.compiler.mine",
            SurfaceKind::PythonPackage,
            "prism_compiler",
            python_reason,
        )?,
    ))
    .step(Step::naming(
        "Compare a baseline architecture against a candidate and read the paired effects.",
        foreign_claim(
            "prism.lab.compare",
            SurfaceKind::PythonPackage,
            "prism_eval",
            python_reason,
        )?,
    ))
    .seal()?;

    let ci = Walkthrough::draft(
        WalkthroughId::parse("ci-regression-gate")?,
        "Gate a pull request on evaluation regressions, following the section's workflow example.",
        Surface::foreign(SurfaceKind::GitHubAction, ".github/workflows/prism.yml")
            .expect("static surface is well formed"),
    )
    .step(Step::narrating(
        "Decide which sentinels run on every pull request and which run nightly.",
        "the choice is a policy about the consumer's repository, and naming an API for it would \
         invent a configuration key the section does not define",
    ))
    .step(Step::naming(
        "Reference the published action at a pinned major version.",
        foreign_claim(
            "aurora-neuro/prism-action@v1",
            SurfaceKind::GitHubAction,
            "prism-action",
            ci_reason,
        )?,
    ))
    .step(Step::naming(
        "Point the action at the architecture file and the branch to compare against.",
        foreign_claim(
            "with.compare-to",
            SurfaceKind::GitHubAction,
            "prism-action",
            ci_reason,
        )?,
    ))
    .seal()?;

    let mcp = Walkthrough::draft(
        WalkthroughId::parse("mcp-agent-quickstart")?,
        "Let an agent compile a decision context over the Model Context Protocol without linking \
         the engine.",
        Surface::foreign(SurfaceKind::McpTool, "prism-mcp").expect("static surface is well formed"),
    )
    .step(Step::naming(
        "Start the stdio server.",
        rust_claim("bioprism_mcp::serve", "bioprism-mcp", "crates/mcp/src/lib.rs")?,
    ))
    .step(Step::naming(
        "Read the tool definitions the server advertises.",
        rust_claim(
            "bioprism_mcp::tool_definitions",
            "bioprism-mcp",
            "crates/mcp/src/lib.rs",
        )?,
    ))
    .step(Step::naming(
        "Call the compile tool by its protocol name.",
        foreign_claim("tools/call", SurfaceKind::McpTool, "prism-mcp", mcp_reason)?,
    ))
    .seal()?;

    let reporting = Walkthrough::draft(
        WalkthroughId::parse("one-evidence-state-report")?,
        "Render the same evidence for four audiences without any of them disagreeing about a \
         number.",
        Surface::rust(&bioprism_cookbook::CrateName::parse("bioprism-devplat").expect("valid"))
            .expect("this crate is a workspace crate"),
    )
    .step(Step::naming(
        "Assemble the figures, each with a source pointer.",
        rust_claim(
            "bioprism_devplat::report::EvidenceState",
            "bioprism-devplat",
            "crates/devplat/src/report.rs",
        )?,
    ))
    .step(Step::naming(
        "Render for each audience.",
        rust_claim(
            "bioprism_devplat::report::render",
            "bioprism-devplat",
            "crates/devplat/src/report.rs",
        )?,
    ))
    .step(Step::naming(
        "Check that the comparability banner precedes the headline.",
        rust_claim(
            "bioprism_devplat::report::Rendering",
            "bioprism-devplat",
            "crates/devplat/src/report.rs",
        )?,
    ))
    .seal()?;

    let reproduction = Walkthrough::draft(
        WalkthroughId::parse("partial-reproduction-report")?,
        "Report a figure as partially reproduced, without collapsing the outcome to pass or fail.",
        Surface::rust(&bioprism_cookbook::CrateName::parse("bioprism-devplat").expect("valid"))
            .expect("this crate is a workspace crate"),
    )
    .step(Step::naming(
        "Record the ten evidence obligations, including the ones in conflict.",
        rust_claim(
            "bioprism_devplat::repro::ObligationLedger",
            "bioprism-devplat",
            "crates/devplat/src/repro.rs",
        )?,
    ))
    .step(Step::naming(
        "Seal the report, which refuses a verification status while an obligation is open.",
        rust_claim(
            "bioprism_devplat::repro::ReproductionReport",
            "bioprism-devplat",
            "crates/devplat/src/repro.rs",
        )?,
    ))
    .step(Step::narrating(
        "Read the eight statuses and pick the one that is true.",
        "choosing between `directionally reproduced` and `not reproduced` is a judgement about \
         evidence, and an API that made it automatically would be the collapse this module exists \
         to prevent",
    ))
    .seal()?;

    let security = Walkthrough::draft(
        WalkthroughId::parse("evaluator-exploit-gate")?,
        "Decide whether a benchmark release is stable when an agent can write the grade file.",
        Surface::rust(&bioprism_cookbook::CrateName::parse("bioprism-devplat").expect("valid"))
            .expect("this crate is a workspace crate"),
    )
    .step(Step::naming(
        "Score the four axes separately.",
        rust_claim(
            "bioprism_devplat::exploit::CellScore",
            "bioprism-devplat",
            "crates/devplat/src/exploit.rs",
        )?,
    ))
    .step(Step::naming(
        "Evaluate the release gate.",
        rust_claim(
            "bioprism_devplat::exploit::release_gate",
            "bioprism-devplat",
            "crates/devplat/src/exploit.rs",
        )?,
    ))
    .seal()?;

    Ok(vec![python, ci, mcp, reporting, reproduction, security])
}

/// Re-check a walkthrough's in-tree claims against a working tree.
///
/// Reuses `bioprism-cookbook`'s [`Workspace`](bioprism_cookbook::Workspace) rather than walking the
/// tree again. In-tree claims are re-derived from the file text: if the named symbol's last
/// segment is no longer present, the claim comes back [`Evidence::AbsentFromTree`] and the
/// document is refuted. Out-of-tree claims are returned unchanged, because there is nothing to
/// look at and inventing a check for them would be the exact confusion this crate names.
pub fn recheck(
    walkthrough: &Walkthrough,
    workspace: &bioprism_cookbook::Workspace,
) -> Vec<(String, Evidence)> {
    walkthrough
        .claims()
        .into_iter()
        .map(|claim| {
            let api = claim.api().as_str().to_string();
            match claim.evidence() {
                Evidence::ResolvedInTree { file } => {
                    let needle = api.rsplit("::").next().unwrap_or(&api).to_string();
                    match workspace.read(file) {
                        Ok(text) if text.contains(&needle) => (
                            api,
                            Evidence::ResolvedInTree {
                                file: file.to_string(),
                            },
                        ),
                        _ => (api, Evidence::AbsentFromTree),
                    }
                }
                other => (api, other.clone()),
            }
        })
        .collect()
}
