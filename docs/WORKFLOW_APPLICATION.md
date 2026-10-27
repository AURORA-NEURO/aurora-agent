# Local workflow application

The opt-in application at `/workflows/` edits and executes the **Python executable workflow**
format through the existing bounded Rust HTTP server. The SDK remains the only DAG validator and
executor. The separate Rust `/v1/missions` grammar is not used for these plans. This is a
repository-defined application extension; it does not claim a new blueprint module implementation.

The page provides a public-operation catalogue, task entry, bounded glioma interpretation, fixed
public-source templates or local model planning, editable step cards and proposal JSON, retained
task-contract inspection, saved revision validation, exact-plan approval, asynchronous run monitoring, cancellation, explicit
resume/retry controls, and explicit output/checkpoint downloads. Execution completion and
scientific review remain separate states.

## Start the application

Build with the pinned offline dependencies:

```powershell
cargo build -p bioprism-api --offline
```

Create an ordinary private state directory and configure absolute executable and SDK paths. The
SDK directory contains `prism_sdk/`; it can be the checkout's `python/` directory or an installed
SDK directory. Install Python 3.12 or newer. Supply a bearer token of at least 32 visible bytes in
the operator environment. Avoid putting the token in command-line arguments or shell history.

```powershell
# AURORA_API_TOKEN is already supplied in this process's environment.
# Substitute the operator's actual absolute paths and a directory they own.
& 'C:\aurora\target\debug\bioprism-api.exe' `
  --bind 127.0.0.1:8787 `
  --root 'C:\aurora' `
  --workflow-state 'C:\aurora-state' `
  --workflow-python 'C:\Python312\python.exe' `
  --workflow-python-path 'C:\aurora\python' `
  --workflow-base-url http://127.0.0.1:11434/v1 `
  --workflow-model qwen2.5:1.5b `
  --workflow-research-sources `
  --workflow-molecular-glioma `
  --workflow-glioma-mutations `
  --workflow-gene-references
```

`--token-env VARIABLE_NAME` selects a different existing environment variable. The default is
`AURORA_API_TOKEN`. Application mode refuses startup without the long token, even on loopback.
It also requires an exact numeric loopback bind with a fixed nonzero port. The ordinary server
retains its previous behavior when `--workflow-state` is absent.

Open `http://127.0.0.1:8787/workflows/`, enter the token, and connect. The token stays in the page's
JavaScript closure and is sent in bearer headers. It is never placed in a URL, browser storage,
worker environment, or API configuration debug output. Reloading the page clears it. A browser
or password manager may have its own behavior outside the application's control.

The bounded glioma interpreter needs no model call. An example it accepts is:

> Review glioma evidence using PubMed and trials; PubMed limit 2; trials page size 3; trials max pages 1.

Unsupported clauses produce exact unresolved spans. Local model planning requires the separately
selected provider-call approval and an operator-configured, already available Ollama model.
Configuring a model does not establish its planning quality or automatically download weights.

The last two flags independently enable fixed, model-free TP53 templates. Both default to false;
omitting them leaves the default catalogue unchanged. The page enables a template only after the
connected catalogue contains its source operation and both local analysis/review operations.
Browser requests cannot turn on an omitted capability. In these templates, task text is a retained
research label, not a query interpreted into gene or cohort selectors:

- `--workflow-glioma-mutations` enables **TP53 mutation aggregate review** (`glioma-mutations`):
  a three-step plan reads fixed public GDC TP53 mutation-affected case aggregates for TCGA-GBM
  and TCGA-LGG, performs local analysis, and produces a scientific review hold. Missing counts
  remain unknown. It supplies no gene-specific denominator, prevalence, rates, pooled project
  count, or clinical conclusion.
- `--workflow-gene-references` enables **TP53 gene reference review** (`gene-references`):
  a four-step plan reads reviewed Ensembl TP53 references and NCBI TP53 identity data, compares
  exact candidate identifier strings locally, and produces a scientific review hold. Ambiguity
  remains visible; string agreement does not validate identifier equivalence. It joins no
  measurements, cohorts, mutation aggregates, or outcomes.

These choices need no provider-call approval or model configuration. Execution still requires
the separate exact-plan source approval. Registration does not establish live source coverage
or scientific validity.

## Plan and run authority

The server launches only the fixed module `prism_sdk.workflow_api_worker` with operator-owned
configuration, using `Command` directly without a shell. Browser requests cannot select a Python
executable, module, argv, environment, file path, MCP peer, OpenAPI document, credential, or private
connection. See [the worker protocol](WORKFLOW_API_WORKER.md) for the authoritative stored format.

Building retains the exact task, catalogue binding, and fixed requirements. Edits submit only the
proposal's `name`, `steps`, and `outputs` alongside the current opaque plan ID and digest. The SDK
validates the edit against the retained task and contract. A changed saved plan has a changed
digest; an earlier run or stale approval cannot execute that revision. The browser also clears
its approval checkbox on edits. Validation diagnostics project bounded reference paths and fixed
contract issues from the SDK audits, without source values. A source-schema path that is deferred
until execution remains unknown at edit time; a schema-reachable path does not establish that a
future response contains a value or scientifically supports the question.

The run button grants public source dispatch for the exact saved digest. Provider planning
approval does not authorize source execution. Resume requires a new source grant. Retrying a
failed or interrupted read and releasing durable cancellation each require their own literal
boolean control. A queued run whose execution thread never started may be explicitly admitted
through the resume endpoint; it cannot claim retry or checkpoint-cancellation release.

Each background process runs the SDK's serial executor. Up to four run processes and eight
control processes may be active, with planning limited to two of those control slots; this is run concurrency, not parallel execution of DAG branches.
Per-plan and per-run OS locks in the worker prevent competing execution/edit admission. No run
is automatically resumed after a restart. Cancellation is cooperative: an admitted synchronous
call can finish before subsequent dispatch stops. The durable cancellation hold survives worker
and server restarts until explicitly released.

Status responses contain metadata, checkpoint progress, and recognized review states. They omit
source values and acquisition receipts. Explicit exports download the actual output object or checkpoint object at the JSON root,
without a worker-response wrapper. The HTTP server and browser retain the authoritative export
text so large integers and checkpoint consistency digests survive downloading. They include
retained public values and full checkpoints; the operator must protect the state directory and exported files. Task text and
plans are retained, so use public, non-sensitive research tasks. This application provides no
patient-data intake, clinical decision support, diagnosis, treatment selection, or enrollment.

## HTTP routes

All API routes require bearer authentication. GET routes accept no query parameters. POST routes
require `Content-Type: application/json`, unique JSON fields at every nesting level, and exact
action-specific fields. IDs are 32 lowercase hexadecimal characters; they are never file paths.

| Method | Route | Behavior |
|---|---|---|
| GET | `/v1/executable-workflows/catalogue` | Installed operation descriptors |
| POST | `/v1/executable-workflows/interpret` | Pure bounded glioma interpretation |
| POST | `/v1/executable-workflows/plans` | Build and retain a validated plan |
| GET | `/v1/executable-workflows/plans/{id}` | Retained task, contract, and current plan |
| POST | `/v1/executable-workflows/plans/validate` | Validate and save an edited revision |
| POST | `/v1/executable-workflows/runs` | Create a run, then start a background worker |
| GET | `/v1/executable-workflows/runs/{id}` | Metadata-only inspection |
| POST | `/v1/executable-workflows/runs/{id}/cancel` | Persist a cancellation request |
| POST | `/v1/executable-workflows/runs/{id}/resume` | Explicit reviewed admission or resume |
| GET | `/v1/executable-workflows/runs/{id}/outputs` | Explicit JSON output download |
| GET | `/v1/executable-workflows/runs/{id}/checkpoint` | Explicit full checkpoint download |

Creation and resume return HTTP 202 after the process thread is admitted. Poll the returned run
ID to observe the result; 202 does not mean that source calls completed. `worker_process` reports
the observed background-process state and any sanitized protocol refusal. Durable SDK run status
is independent of that transport observation. A lost worker, timeout, invalid response, or pipe
overflow requires inspection and explicit subsequent action; the server never retries it.

Worker request bytes are bounded to 1,000,000, response bytes to 16,000,000, and captured stderr to
64 KiB. Control requests have a 30-second process deadline, model planning 300 seconds, and an
execution process 900 seconds. Exceeding a bound kills that worker and records an interrupted
transport observation. An already admitted source operation can then have uncertain outcome;
the SDK's checkpoint and retry boundary remains authoritative.

## Browser boundary and limits

Application mode checks exact `Host` and, when present, exact `Origin` on **every** request before
the legacy OPTIONS handler or any worker starts. Cross-site Fetch Metadata is also refused.
There is no wildcard CORS response in this mode. Static assets can be fetched without the bearer
after the same Host/Origin checks. JSON routes require the bearer, including on loopback.

Responses use `no-store`, request IDs, `nosniff`, frame denial, a restrictive same-origin content
security policy, and no-referrer policy. The page inserts task, model, source, plan, and error text
using DOM `textContent` or form `value`; it does not interpret evidence as HTML. A JSON editor codec limits input/output to 16,000,000 UTF-8 bytes, 250,000 values, and
128 nesting levels. It checks pretty-output byte use before large joins and preserves original number lexemes, including integers beyond JavaScript
`Number.MAX_SAFE_INTEGER`, `0.0`, and `-0.0`, and rejects duplicate object fields. Native
number coercion is used only for operator-entered bounded invocation controls. Downloads fetch
with bearer headers and create a temporary Blob URL.

This is a single-operator local application, without TLS termination, OAuth/account sessions,
multi-tenant storage, distributed scheduling, private-account adapters, MCP session continuity,
effectful sinks, general NLP completeness, or scientific approval. Multiple public adapters can
share provider infrastructure; operation count is not a count of independent live services.
Model refusal and unsupported interpretation remain visible failures. The state consistency
digests do not authenticate files against another actor who can write the operator's state root.

## Verification

Run the actual bridge/HTTP tests, followed by the existing crate suite and warning checks:

```powershell
cargo test -p bioprism-api --offline
cargo clippy -p bioprism-api --all-targets --offline -- -D warnings -A clippy::too_many_arguments
node --check crates/api/assets/workflows/app.js
node --test crates/api/assets/workflows/json.test.mjs
```

The bridge tests use a real Python subprocess and ordinary temporary files. They verify retained
contract enforcement, stale-digest refusal, cancellation across process restarts, background resume
without implicit release, metadata/export separation, strict duplicate fields, and actual TCP
Host/Origin checks. A deliberately separate process fixture checks pipe overflow, timeout, and
credential-environment exclusion, and exact large-integer transport; it is not counted as a connected external integration.
