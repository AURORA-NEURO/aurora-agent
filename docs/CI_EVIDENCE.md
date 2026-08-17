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

## Provider normalization

`ci_provider_normalize` is the ingestion boundary before the audit. It accepts either a bounded
GitHub Actions-shaped payload (`run` plus `jobs`) or a generic payload (`run_id`, `conclusion`, and
`checks`) and returns the exact `CiRunEvidence` shape above, already bound to a regenerated plan
digest. Provider statuses are mapped to the canonical pass/fail/skipped/cancelled/unknown states.

Provider payloads commonly lack per-check result digests. In that case the normalizer derives a
content digest from the supplied check object and emits both `derived_result_digest_count` and
per-check warning labels. A supplied but malformed digest is refused rather than silently replaced.
The derived digest identifies the caller-supplied payload object; it is not a log, signature, or
provider-authentication proof. The route remains non-networked and non-executing.
`developer_delivery_audit` accepts the same request as `ci_provider` and composes normalization
and audit in one explicit delivery call. The result keeps the normalized provider projection and
the downstream `ci_evidence` audit separately visible; `ci_provider` and canonical `ci_evidence`
are mutually exclusive inputs.

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
  HTTP methods. `CiProviderNormalizationRequest` and `CiProviderNormalizationReport` expose the
  corresponding provider normalizer.
- TypeScript: `CiExecutionEvidenceArgs`, `CiExecutionEvidenceResult`, and
  `ApiClient.ciExecutionEvidenceAudit(...)`, plus `CiProviderNormalizationArgs`,
  `CiProviderNormalizationResult`, and `ApiClient.ciProviderNormalize(...)`.
- MCP: the `ci_execution_evidence_audit` and `ci_provider_normalize` tools with the
  `bioprism-devplat-ci-execution-evidence/0.1` and
  `bioprism-devplat-ci-provider-normalization/0.1` schemas.
