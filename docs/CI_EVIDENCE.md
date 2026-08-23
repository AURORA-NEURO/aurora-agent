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
GitHub Actions-shaped payload (`run` plus `jobs`), a GitLab CI payload (`pipeline` plus `jobs`), or
a generic payload (`run_id`, `conclusion`, and `checks`) and returns the exact `CiRunEvidence` shape
above, already bound to a regenerated plan digest. Provider statuses are mapped to the canonical
pass/fail/skipped/cancelled/unknown states; GitLab duration seconds are normalized to milliseconds.

Provider payloads commonly lack per-check result digests. In that case the normalizer derives a
content digest from the supplied check object and emits both `derived_result_digest_count` and
per-check warning labels. A supplied but malformed digest is refused rather than silently replaced.
The derived digest identifies the caller-supplied payload object; it is not a log, signature, or
provider-authentication proof. The route remains non-networked and non-executing.
`developer_delivery_audit` accepts the same request as `ci_provider` and composes normalization
and audit in one explicit delivery call. The result keeps the normalized provider projection and
the downstream `ci_evidence` audit separately visible; `ci_provider` and canonical `ci_evidence`
are mutually exclusive inputs.

## Reusable GitHub Actions exporter

The repository also ships a dependency-free composite action at
`.github/actions/github-actions-evidence`. It has two explicit input modes. Manual mode packages a
caller-selected checks JSON file and the current workflow-run metadata. Discovery mode uses the
provided `GITHUB_TOKEN` to retrieve one run and its bounded jobs from the GitHub API, then packages
the same exact GitHub Actions-shaped payload accepted by `ci_provider_normalize`.

Manual mode remains useful when a consumer already has a reviewed, transformed check set:

```yaml
jobs:
  evidence:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Produce bounded check rows
        run: |
          mkdir -p .aurora
          python - <<'PY'
          import json
          json.dump({"jobs": [
              {"name": "unit", "conclusion": "success"},
              {"name": "lint", "conclusion": "success"},
          ]}, open(".aurora/checks.json", "w", encoding="utf-8"))
          PY
      - id: aurora-evidence
        uses: AURORA-NEURO/aurora-agent/.github/actions/github-actions-evidence@<reviewed-commit>
        with:
          checks: .aurora/checks.json
          output: .aurora/github-actions-provider-payload.json
      - name: Retain the handoff
        uses: actions/upload-artifact@v4
        with:
          name: aurora-provider-payload
          path: .aurora/github-actions-provider-payload.json
```

The `checks` input may be an array or an object containing `jobs`/`checks`. Discovery mode is
selected with `discover: true` and requires `github-token`, `api-url`, `repository`, and a run id
from the input, event, or runner environment; it refuses a simultaneous `checks` input. The API
response is bounded to 64 jobs and 2 MiB per response, and a larger job list is refused rather than
truncated. Job names, statuses, conclusions, optional RFC 3339 durations, and job URLs are
normalized; malformed timestamps, duplicate names, mismatched run ids, invalid endpoints, and
network/API failures remain errors. The token is used only in the request header and is never
serialized into the payload or action outputs.

Set `collect-evidence: true` with `collection-output` to make the discovery pass retrieve the
bounded `/artifacts` metadata list (at most 128 rows) and derive one log-locator row per discovered
job that exposes `logs_url`. The action does not follow either locator by default. Artifact rows use
a digest of selected provider metadata unless the caller supplied a digest; log-row digests cover
the locator metadata, not downloaded log bytes. More than 128 artifacts or locators is refused
rather than silently truncated. Manual `artifacts`, `logs`, and `attestations` files use the same
bounded row shapes, while attestations are always caller-supplied declarations.

`download-evidence: true` is a separate, explicit opt-in for bounded byte collection. It implies
evidence collection during discovery and follows every artifact/log `uri` in the selected rows;
manual mode can use the same switch with explicit `uri` fields. Locators must be absolute HTTPS
URLs without embedded credentials or fragments. A response is capped at 16 MiB and one collection
at 256 MiB; declared `Content-Length` values and actual reads are both checked. Authorization is
attached only to the initial request and removed from redirected requests, while insecure redirect
targets are refused. The resulting row digest is SHA-256 over the response bytes retrieved locally,
and outputs include the downloaded artifact/log counts and total byte count. Archives are not
extracted or validated, logs are not interpreted or executed, and attestation statements are not
signature-verified. This is reported as `verification: local_byte_hash_only`, not provider
authentication or remote-content proof.

Every exporter artifact/log row now carries an optional `digest_scope`: `provider_metadata` means
the digest covers selected provider metadata, `caller_declared` means the caller supplied the
digest, and `local_response_bytes` means the exporter hashed a bounded response locally. The Rust
audit validates the scope, counts local-byte rows, and requires a retained URI for the local scope;
the scope is provenance metadata, not an independent re-fetch or signature check. Attestation rows
may additionally carry `subject_digest`. When present, the audit compares it with the digest of the
named run, artifact, or log and emits a blocking mismatch finding. This is content binding only;
attestation signatures, provider identity, key validity, and revocation remain unverified.

For a `workflow_run` trigger, discovery prefers the upstream run id in the event over the
downstream workflow's own `GITHUB_RUN_ID`; callers can still override it explicitly with `run-id`.

Both modes expose `payload-path`, `payload-digest`, `run-id`, `check-count`, and `discovery-mode`
outputs for a subsequent API/MCP handoff or artifact manifest. Collection mode additionally exposes
`collection-path`, `collection-digest`, and row counts. The action always emits
`download-mode`, `downloaded-artifact-count`, `downloaded-log-count`, and `downloaded-byte-count`,
with zero counts when byte collection is disabled. Output is canonical JSON with deterministic
SHA-256 digests. The default collection envelope reports `execution: not_started` and
`verification: metadata_only`; an explicitly downloaded envelope reports
`verification: local_byte_hash_only`. Neither mode executes a check, verifies a signature or
attestation, approves a release, or turns an API response into authenticated provider truth.
Missing per-check digests are intentionally left for the Rust normalizer to derive and label.
Pin the action to a reviewed commit or release tag in consumer repositories rather than floating on
`main`.

To emit the exact handoff accepted by `ci_provider_evidence_import`, provide both `ci` (the explicit
caller-owned `CiRequest`) and `evidence-output`. The action emits `evidence-path` and
`evidence-digest`, with `source: provider_observed` for discovery and `source: caller_attested` for
manual rows. It never infers the CI plan from observed jobs. The emitted request can be posted to the
MCP/REST/SDK registry surfaces, which re-run the canonical Rust audit before retention.

Discovery example:

```yaml
- id: aurora-evidence
  uses: AURORA-NEURO/aurora-agent/.github/actions/github-actions-evidence@<reviewed-commit>
  with:
    discover: 'true'
    github-token: ${{ github.token }}
    run-id: ${{ github.run_id }}
    collect-evidence: 'true'
    output: .aurora/github-actions-provider-payload.json
    collection-output: .aurora/github-actions-provider-evidence-collection.json
```

To request local byte hashes as well, add `download-evidence: 'true'`. The request is deliberately
separate from metadata collection because it can retrieve up to the documented byte bounds and
may refuse an expired artifact, a missing locator, an oversized response, or a non-HTTPS redirect.

The repository's public Python CI job invokes this local action in both modes. It exercises manual
mode against repository fixtures, then uses the runner token to discover the current workflow, jobs,
artifact metadata, and log locators. Each emitted file is re-read and checked for canonical fields,
mode, run identity, bounded row counts, explicit metadata-only posture, and token absence. This
catches broken composite action metadata, output-name expressions, runner path handling, API
permissions, discovery serialization, and manual serialization drift in addition to unit-level
refusal tests.

## Provider artifacts, logs, and attestations

`ci_provider_evidence_audit` is the deeper conformance handoff for consumers that have more than a
run summary. It accepts the same provider payload plus bounded `artifacts`, `logs`, and
`attestations` arrays. Artifact and log rows must carry a unique id, a valid content digest, the
normalized provider and run id, and—when present—a check name from the regenerated plan. URI text
is retained as a locator by the Rust audit but is never fetched there. Rows may declare their digest
scope, and an attestation may bind `subject_digest` to its named run, artifact, or log. Attestation
issuer, method, and statement digest remain declarations unless an external verifier is supplied.

The reusable action's collection envelope is intentionally one step earlier than this audit. It
binds GitHub's run id and provider identity to each discovered row, makes locator/metadata digest
scope visible, and can be converted without format translation when the caller supplies `ci`. The
Rust audit remains the authority for plan/check reconciliation, row subject binding, and retained
registry identity; the action does not claim that its metadata collection is an audit result.

The audit returns separate deterministic record digests for each row family, linked-row counts,
subject counts, explicit local-byte hash counts, subject-digest binding counts, canonical nested CI
evidence, and sorted findings. Duplicate ids, invalid digest
syntax, unknown checks, provider/run mismatches, missing bindings, malformed locators, and unknown
attestation subjects are blocking findings. Rows are preserved even when invalid so an operator can
inspect and repair the original handoff rather than receiving a rewritten green projection.

`conformance_ready` means the canonical CI evidence and attached rows satisfy these structural
predicates. It does not mean the remote artifact exists, a log was downloaded, a provider was
authenticated, a signature was checked, or a check was executed. Attestation statements remain
available for a later verifier; this route explicitly reports `verification: structural_only`.

The same request can be supplied as `developer_delivery_audit.ci_provider_evidence`. Delivery then
keeps the provider-evidence audit, canonical `ci_evidence`, and the existing provider/run target
separate, and adds an explicit `ci_provider_evidence` release target. When a
`developer_delivery_receipt` is built from that audit, the receipt carries a dedicated evidence row
whose digest is computed over the complete provider-evidence projection. Receipt verification
recomputes that row and reports `evidence_mismatch` if its digest or retained rows are tampered with.
The provider-evidence input remains mutually exclusive with `ci_evidence` and `ci_provider` so a
receipt cannot silently blend provenance paths.

The retained registry exposes the three binding counts in import summaries and compact query rows.
Queries can require minimum `min_local_byte_hash_artifacts`, `min_local_byte_hash_logs`, or
`min_attestation_subject_digest_bindings` thresholds before returning a row. These are explicit
retention predicates over the audited report, not inferred provider trust or release approval.

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
  corresponding provider normalizer; `CiProviderEvidenceRequest` and `CiProviderEvidenceReport`
  expose the artifact/log/attestation conformance handoff across all four facades.
  `developer_delivery_audit(..., ci_provider_evidence=...)` and the typed delivery report expose
  the independent provider-evidence target; delivery receipts preserve its digest row.
- TypeScript: `CiExecutionEvidenceArgs`, `CiExecutionEvidenceResult`, and
  `ApiClient.ciExecutionEvidenceAudit(...)`, plus `CiProviderNormalizationArgs`,
  `CiProviderNormalizationResult`, and `ApiClient.ciProviderNormalize(...)`, plus
  `CiProviderEvidenceArgs`, `CiProviderEvidenceResult`, and
  `ApiClient.ciProviderEvidenceAudit(...)`.
- MCP: the `ci_execution_evidence_audit`, `ci_provider_normalize`, and
  `ci_provider_evidence_audit` tools with the
  `bioprism-devplat-ci-execution-evidence/0.1` and
  `bioprism-devplat-ci-provider-normalization/0.1` and
  `bioprism-devplat-ci-provider-evidence/0.1` schemas.
