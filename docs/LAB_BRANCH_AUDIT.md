# Inference-lab branch audit

`lab_branch_audit` exposes the deterministic risk-triggered branching ledger in
`bioprism-lab::risk`. It answers a narrower question than “did branching improve the system?”:
given a stated trigger policy and a set of declared decision states, when did the controller spend
extra computation, why did it spend it, what did the caller say it caught, and what escaped anyway?

The endpoint plans and audits. It does not fork a runtime suffix, invoke a verifier, execute a
tool, or infer a learned risk threshold.

## Request

```json
{
  "policy": {
    "ceiling": { "max_branches": 4, "max_verifier_calls": 2 },
    "on_undetermined": "escalate",
    "rules": [
      {
        "id": "irreversible",
        "trigger": {
          "trigger": "reversibility_at_least",
          "level": "irreversible"
        },
        "action": "fork_suffixes",
        "cost": { "branches": 2, "verifier_calls": 1 }
      }
    ]
  },
  "decisions": [
    {
      "decision": "candidate-write",
      "features": {
        "reversibility": "irreversible",
        "permission": "external_effect",
        "value_at_stake": "severe",
        "unseparated_hypotheses": 2,
        "unmet_mandatory_obligations": 1,
        "historical_failure_rate": null,
        "verifier_available": false
      },
      "caught": {
        "what": "unsafe suffix",
        "would_have_been": "write would proceed"
      },
      "escaped": "a secondary harm remained"
    }
  ],
  "max_rows": 100
}
```

`decisions` is bounded to 1–512 objects. `max_rows` bounds returned decision rows to 1–1,000;
`rows_omitted` always preserves the discarded denominator. `historical_failure_rate: null` means
unmeasured, not zero. The server rejects non-finite or out-of-range supplied rates.

## Policy integrity

The server deserializes the policy and then reconstructs it through `BranchPolicy::new`. This is
important: serialized policy data cannot bypass the kernel's checks for vacuous triggers or a
per-rule branch/verifier cost above the hard ceiling. Ordered rules retain first-match semantics.

`on_undetermined` is an explicit policy choice:

- `escalate` treats an unmeasured predicate as fired and marks the plan
  `on_undetermined: true`;
- `proceed` treats it as not fired.

The report keeps measured firing separate from escalation caused by ignorance. A branch that is
justified because nobody measured the failure rate is not the same finding as a branch justified
by an observed high failure rate.

## Successful projection

The schema is `bioprism-mcp/lab-branch-audit/0.1`.

- `yield` includes the full denominator, escalations, undetermined escalations, branch/verifier
  spend, catches, wasted escalations, escaped harms after escalation, and escaped harms without
  escalation.
- `verdict` is `nothing_triggered`, `paid_and_caught_nothing`, `mixed`, or
  `every_escalation_caught_something`. The kernel checks the unflattering paid-without-catch case
  before emitting a positive verdict.
- Each row retains the decision, risk features, selected plan, trigger prose, cost, and optional
  catch/escape declarations.
- A catch must carry `would_have_been`, the single-path counterfactual. “The verifier ran” alone
  is not evidence that the extra branch was useful.

## Fail-closed behavior

```json
{
  "ok": false,
  "schema": "bioprism-mcp/lab-branch-audit/0.1",
  "stage": "policy_validation",
  "refusal": "branch policy would spend 2 branches against a hard ceiling of 1",
  "fail_closed": true
}
```

Policy validation refusals emit no partial ledger. Malformed decision objects are rejected before
planning. The SDK parser rejects non-reconciled yield/decision counts, unknown verdict tags,
non-finite catch ratios, and row omission mismatches.

## SDK surfaces

- Python exposes `LabBranchAuditArgs`, `LabBranchAuditReport`, and
  `lab_branch_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`.
- TypeScript exposes `labBranchAudit(...)`, `LabBranchAuditArgs`, and `LabBranchAuditResult`.

Use this audit with `runtime_execution_simulate` when a declared branch needs actual local replay
evidence. The branch ledger itself is not a verifier result, a safety clearance, or proof that an
escaped harm was prevented in a counterfactual world.
