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
redundant reads. The compiler now rejects unused steps, but arbitrary multi-step task fidelity
remains unmeasured. Use the reviewed template for the composed glioma workflow.

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

Eight concrete adapters are registered: PubMed, Europe PMC, ClinicalTrials.gov, GDC projects,
cBioPortal studies, NCBI Gene, Open Targets, and GWAS Catalog. Their source selectors and result
scope remain bounded by each reviewed adapter. Two local glioma operations do metadata
reconciliation and handoff; they are not additional external services.

The [OpenAPI importer](WORKFLOW_OPENAPI.md) registers explicitly reviewed GET operations with
fixed HTTPS hosts and application-owned transient auth headers. CLI imports use a JSON config
with exactly `document`, `integration_id`, `server_url`, `allowed_hosts`, and
`reviewed_operation_ids`; `document` is relative to the config file. Pass repeatable
`--openapi-config FILE --approve-spec-import` to each command that must reproduce that catalogue.
The CLI does not collect auth secrets. SDK applications own protected auth resolution.

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
