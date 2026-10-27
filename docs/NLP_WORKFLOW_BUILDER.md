# Executable natural-language workflow builder

The Python SDK now compiles local-model proposals into executable integration DAGs. This is a
repository-defined application layer over `LLMRuntime` and the reviewed retrieval adapters. It is
separate from the Rust workflow simulation/binding contract and does not claim blueprint coverage
or FIBER sufficiency for a retrieved source bundle.

## Run locally

Install the SDK from the repository and use an installed Ollama model:

```powershell
python -m pip install ./python
aurora-agent workflow-catalogue
aurora-agent workflow-run --task "Search PubMed for two glioma literature records" --model YOUR_LOCAL_MODEL --operation pubmed.search --max-steps 1 --approve-provider-call --approve-source-dispatch --out plan.json --checkpoint-store run.json --outputs-file public-results.json
```

`workflow-run` proposes and validates the plan, saves it to `--out`, then executes. The two flags
independently authorize the local provider and selected source reads for that bounded request.
`--base-url` defaults to `http://127.0.0.1:11434/v1` and accepts only loopback model endpoints.
No OpenAI key, hosted provider, or running Rust MCP server is required.

For a separately reviewed plan, use `workflow-build` with the same task/model arguments and
`--approve-provider-call --out plan.json`, followed by:

```powershell
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json --max-step-calls 1
aurora-agent workflow-execute --plan plan.json --approve-source-dispatch --checkpoint-store run.json --resume
```

The glioma [evidence template](GLIOMA_WORKFLOW_BUILDER.md) provides a deterministic composition of
PubMed, ClinicalTrials.gov, optional TCGA GDC summaries, reconciliation, and a review handoff.
The question labels the research task; source lanes remain the reviewed fixed queries.

## Plan and execution contract

An unsealed proposal has exactly `name`, `steps`, and `outputs`. Each step has `id`, `operation`,
`arguments`, and `depends_on`. Arguments may contain `{"$ref":"source_step#/json/pointer"}`;
`source_step#` selects the source's entire value. References must stay within the declared
dependency ancestry. Every step must contribute to a declared output.

The compiler rejects unknown operations, duplicate IDs, cycles, missing dependencies, malformed
references, invalid literal arguments, unused steps, and effectful operations. It normalizes
declared defaults and seals the task digest, exact catalogue digest, graph, and outputs. The
executor rechecks the plan and every resolved argument before dispatch. It runs at most 32 steps
per invocation and permits at most three explicit attempts per step. A changed catalogue requires
recompilation, even when the changed operation was not selected.

Model visibility is bounded separately from catalogue capacity. Up to 32 operations are shown by
default, using deterministic lexical overlap when the catalogue is larger. `--operation` provides
an exact allowlist; the SDK can configure a different visibility bound up to 128. Omitted operations
are unavailable in that planning pass. This is not a measured relevance or task-quality guarantee.

The local model receives a structural output schema, then the compiler validates its proposal.
Schema validity does not prove that a plan fulfills the natural-language task. The small-model
live check produced a usable single-source plan; an attempted larger plan contained disconnected,
redundant reads. The compiler rejects unused steps. Small prospective multi-step probes have
retained failures and do not establish broad task fidelity; see the
[local planning evaluation](LOCAL_WORKFLOW_PLANNING_EVALUATION.md). The reviewed templates
provide fixed compositions for their documented research scopes.

Caller-authored [requirements](WORKFLOW_REQUIREMENTS.md) can bind exact operations, outputs,
arguments, value flow and attempt caps into a plan. `--requirements FILE --planning-attempts 2`
enables bounded repair with the same fixed contract. SDK providers must set `max_attempts=1`
so transport retries cannot exceed the planning budget. See the
[recorded small-model limitations](WORKFLOW_DEVELOPMENT_PLAN.md#observed-local-model-limits).

## Durable public-value checkpoints

An explicit checkpoint contains completed public values, original receipts, their digests,
normalized argument digests, per-step attempts, and in-flight/failure state. It is atomically
replaced before each dispatch and after results. Persistence failure before dispatch prevents the
call. Resume verifies every retained row against the plan and output schema; completed steps are
not invoked again. Digests check consistency, not source authenticity or operator identity.

Failure remains `failed` or `blocked`, never a successful empty source. A failed/interrupted step
requires `--resume --retry-failed`; it cannot silently run again. Use `workflow-execute` to resume
a one-shot run. There is no automatic synthetic fallback. Checkpoint/output/plan/import paths
must be distinct, and an existing checkpoint requires explicit resume. Source clocks are canonical
whole-second UTC timestamps.

Stdout retains metadata only. `--outputs-file` and `--checkpoint-store` explicitly retain public
source content; they are unsuitable for credentials or patient records. The executor is sequential
and does not implement distributed locking or effectful exactly-once guarantees.

## Integration coverage

The opt-in [local workflow application](WORKFLOW_APPLICATION.md) adds an editor and authenticated
HTTP routes to the existing Rust server. A fixed [Python worker](WORKFLOW_API_WORKER.md) retains
the task and its contract, validates edited proposals through this SDK, binds execution approval
to the current plan digest, and exposes run/cancel/resume controls. Operator startup configuration
owns executable paths, storage and the local model endpoint. The browser cannot select those
resources. Default polling retains metadata; outputs and checkpoints require explicit export.

Reference checks distinguish known impossible output paths from paths that require runtime
resolution. Native typed glioma contracts additionally bind the complete source bundle at its
specific consumer argument path; an invented child pointer cannot satisfy that value-flow rule.
The operation context supplied to the model includes actual input and output schemas. None of
these structural checks establishes that arbitrary natural-language intent was understood.

Eight concrete adapters are registered: PubMed, Europe PMC, ClinicalTrials.gov, GDC projects,
cBioPortal studies, NCBI Gene, Open Targets, and GWAS Catalog. Their source selectors and result
scope remain bounded by each reviewed adapter. Two local glioma operations do metadata
reconciliation and handoff; they are not additional external services.
Three further reviewed readers are available with `--research-sources`: OpenAlex, Crossref
and Zenodo. Their [bounded public metadata contract](RESEARCH_SOURCE_INTEGRATIONS.md) retains
source-reported counts and unknown coverage. Existing default catalogue identities stay stable.
The [MCP bridge](WORKFLOW_MCP.md) supports explicitly reviewed tools in an operator-configured
connected session. `--grounded-glioma` adds the
[local drafting and citation audit](GLIOMA_WORKFLOW_SYNTHESIS.md) operations; their outputs remain
blocked or held for review according to the exact audit result.

The [OpenAPI importer](WORKFLOW_OPENAPI.md) registers explicitly reviewed GET operations with
fixed HTTPS hosts and application-owned transient auth headers. CLI imports use a JSON config
with exactly `document`, `integration_id`, `server_url`, `allowed_hosts`, and
`reviewed_operation_ids`; `document` is relative to the config file. Pass repeatable
`--openapi-config FILE --approve-spec-import` to each command that must reproduce that catalogue.
The CLI does not collect auth secrets. SDK applications can use
[typed connection sessions](WORKFLOW_CONNECTIONS.md) to bind an application-attested account,
subject, fixed resource scope and revocable credential handle into catalogue identity. Account
attestation is supplied by the application; OAuth and provider identity verification remain
unimplemented. Legacy header mappings are copied with a per-registration opaque binding;
mutable header callables are refused.

The independent [local artifact sink](WORKFLOW_ARTIFACT.md) can publish one exact approved JSON
artifact through persistent scoped grants and the existing SQLite effect journal. This SDK API
is separate from the read-only DAG executor. Adding an effectful operation to a plan still fails;
end-to-end write operations, compensation and retry policies remain unfinished.

Capacity is 4,096 executable operation registrations across 1,024 integration identities. Fixture
tests exercise hundreds of registrations; this is not a claim that hundreds of services are
connected or independently verified. OAuth, arbitrary OpenAPI references, write methods,
webhooks, and generic pagination are not implemented by this importer.

## Validation

The added tests exercise source adapter parsing, schema/reference constraints, catalogue drift,
checkpoint corruption, crash/retry state, approval boundaries, CLI artifact collisions, and
OpenAPI request construction without external calls. The local live check separately exercised an
Ollama-generated PubMed plan and the complete five-stage glioma template against PubMed,
ClinicalTrials.gov, and NCI GDC, including a pause/resume without refetching completed literature.
These runs establish those paths on the checked environment; they do not establish all-service
availability, scientific completeness, clinical applicability, or arbitrary NLP task success.
