//! The blueprint's own example programs, as compilable source.
//!
//! Blueprint 23.37 is a syntax reference written as a dozen separate code blocks, not as one file.
//! [`SYNTAX_REFERENCE`] concatenates its declaration blocks verbatim and [`CONTROL_FLOW_REFERENCE`]
//! collects its statement blocks into a `weave` body, which is the only structural change made: the
//! statement blocks are not valid at file scope in any reading of the grammar.
//!
//! Two edits were unavoidable and are marked in place:
//!
//! - 23.37 writes `{ ... }` as a placeholder for a branch body in the race, fork and parallel
//!   examples. An ellipsis is not syntax, so those bodies carry a real statement instead.
//! - 23.37's `weave run` binds `role Lead` and `role Reviewer`, which no block in 23.37 declares.
//!   [`SYNTAX_REFERENCE`] leaves that as it stands, so it parses but does not resolve;
//!   [`COMPLETE_PROGRAM`] is the version with the missing roles declared, and is what the lowering
//!   and semantics tests compile.
//!
//! Keeping the reference text in the crate rather than in a test file is deliberate: it is the
//! executable statement of which dialect this compiler implements.

/// 23.37's declaration blocks, concatenated in the order the module presents them.
pub const SYNTAX_REFERENCE: &str = r#"
package aurora:reliable-repair@0.1.0
import aurora:core@0.1
import aurora:git@1.0 as git

type hypothesis = record {
  proposition: claim-ref,
  assumptions: list<assumption-ref>,
  confidence: probability
}

type outcome = variant {
  success(report),
  partial(partial-report),
  unknown(reason),
  failed(error)
}

interface investigator {
  inspect(target: artifact-ref) -> evidence-set
    effects [artifact.read]
    throws [unavailable, schema-error]
}

role skeptic {
  provides [challenge@1, verify@1]
  requires [artifact.read]
  clearance confidential/research
  minimum-profile prism://capability/challenge >= 0.80
}

policy safe-repair {
  allow effects [repo.read, branch.write, test.run]
  deny effects [main.write, deploy.production]
  require human for [main.merge]
  budget tokens <= 120000
  budget money <= usd(5)
  max-participants 6
}

choreography review {
  Lead -> Reviewer: propose<Plan>
  choice by Reviewer {
    accept: Reviewer -> Lead: accept<Plan>
    challenge: Reviewer -> Lead: challenge<Plan>
  }
}

weave run(issue: Issue, repo: Repository) -> Report
  using safe-repair {

  bind lead to role Lead
  bind reviewer to role Reviewer

  let plan = ask lead.plan(issue)
  send propose(plan) from lead to reviewer

  match await reviewer.decision {
    accept(p) => execute p
    challenge(c) => resolve c
  }
}
"#;

/// 23.37's parallelism, race, fork/join, commitment, watch, context, termination and evaluation-hook
/// blocks, gathered into one `weave` body.
pub const CONTROL_FLOW_REFERENCE: &str = r#"
package aurora:control-flow@0.1.0

policy open {
  allow effects [search.read, publish, main.merge, evidence.read]
  require human for [main.merge]
  budget tokens <= 120000
}

weave explore(q: Query) -> Report using open {
  par {
    let a = ask agent-a.search(q)
    let b = ask agent-b.search(q)
  }

  race first valid {
    branch fast-model { let quick = ask fast.answer(q) }
    branch deep-model { let slow = ask deep.answer(q) }
  }

  checkpoint c = current
  fork from c {
    branch h1 with budget tokens(10000) { let one = ask worker.try(q) }
    branch h2 with budget tokens(10000) { let two = ask worker.try(q) }
  }
  join using verified-best

  commit worker to lead when task.accepted {
    deliver Patch before 15m
    satisfy with tests.pass
    compensate revert-branch on violation
  }

  watch evidence where contradiction.blocking {
    pause effects [publish, main.merge]
    spawn role skeptic
  }

  context for skeptic {
    include current-claim
    include evidence strongest both-sides
    include assumptions unresolved top 5
    resolution audit
    max-tokens 16000
  }

  @decision-cell(capability="context.information-value")
  let next = choose evidence by information-value

  stop success when commitments.all-closed and verifier.pass
  stop partial when budget.exhausted and useful-artifacts.exist
  stop blocked when human-input.required
}
"#;

/// The reference program with the roles its `weave` body binds actually declared.
///
/// This is 23.37's `weave run` made name-resolvable, and is what the lowering, type-checking and
/// operational-semantics tests compile end to end.
pub const COMPLETE_PROGRAM: &str = r#"
package aurora:reliable-repair@0.1.0

policy safe-repair {
  allow effects [repo.read, branch.write, test.run, artifact.read, plan.read, decision.read]
  deny effects [main.write, deploy.production]
  require human for [main.merge]
  budget tokens <= 120000
  budget tool-calls <= 64
  max-participants 6
}

interface planner {
  plan(issue: Issue) -> Plan
    effects [plan.read]
    throws [unavailable]
}

role Lead {
  provides [plan@1]
  requires [repo.read, plan.read]
  clearance confidential/research
}

role Reviewer {
  provides [challenge@1, verify@1]
  requires [artifact.read, decision.read]
  clearance confidential/research
}

choreography review {
  Lead -> Reviewer: propose<Plan>
  choice by Reviewer {
    accept: Reviewer -> Lead: accept<Plan>
    challenge: Reviewer -> Lead: challenge<Plan>
  }
}

weave run(issue: Issue, repo: Repository) -> Report using safe-repair {
  bind lead to role Lead
  bind reviewer to role Reviewer

  let plan = ask lead.plan(issue)
  send propose(plan) from lead to reviewer

  match await reviewer.decision {
    accept(p) => execute p
    challenge(c) => resolve c
  }
}
"#;
