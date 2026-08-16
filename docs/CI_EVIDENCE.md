# CI execution evidence

`ci_execution_evidence_audit` is the handoff between the Rust workbench’s review-only CI plan and
run evidence supplied by a caller or provider adapter. It does not execute checks or contact
GitHub. It regenerates the plan from the supplied `CiRequest`, then compares the run report with
the resulting digest and exact check names.

## Request shape

```json
{
  "ci": {
    "workflow": "consumer contracts",
    "triggers": ["push", "pull_request"],
    "rust_toolchain": "stable",
    "offline": true,
    "checks": [
      {"name": "tests", "run": "cargo test --workspace --offline", "required": true},
      {"name": "lint", "run": "cargo clippy --workspace --offline", "required": false}
    ]
  },
  "evidence": {
    "run_id": "run-42",
    "provider": "github_actions",
    "source": "provider_observed",
    "plan_digest": "<digest of the regenerated plan>",
    "conclusion": "success",
    "checks": [
      {"name": "tests", "status": "passed", "result_digest": "<digest>"},
      {"name": "lint", "status": "passed", "result_digest": "<digest>"}
    ]
  }
}
```

Every check evidence row carries a valid result digest. The audit rejects duplicate or unknown
check names, reports missing checks, and does not collapse `failed`, `skipped`, `cancelled`, or
`unknown` into a pass. `caller_attested` and `provider_observed` identify provenance only; they are
not cryptographic verification claims.

## Result semantics

`valid`/`structurally_valid` means the plan digest and exact check/evidence structure reconcile.
`complete` means every planned check has one evidence row. `ci_evidence_ready`/`release_candidate`
requires structural validity, complete evidence, a successful run conclusion, and a passing status
for every planned check. The result also carries a deterministic `evidence_digest` over the plan
digest and evidence object.

The execution field remains `evidence_supplied_not_executed_here` and verification remains
`structural_only`. A ready result is therefore a bounded handoff for an external release workflow,
not proof that a runner executed the commands, that logs are authentic, or that the artifact is
safe, clinically valid, scientifically valid, or deployed.

## SDK surfaces

- Python: `CiExecutionEvidenceRequest`, `CiExecutionEvidenceReport`, and sync/async Workspace and
  HTTP methods.
- TypeScript: `CiExecutionEvidenceArgs`, `CiExecutionEvidenceResult`, and
  `ApiClient.ciExecutionEvidenceAudit(...)`.
- MCP: the `ci_execution_evidence_audit` tool with the
  `bioprism-devplat-ci-execution-evidence/0.1` schema.
