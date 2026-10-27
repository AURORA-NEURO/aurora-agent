# Executable workflow application worker

`python -m prism_sdk.workflow_api_worker` is a dependency-free stdio boundary for the existing
Rust HTTP application. It opens no Python listener. It is a repository-defined application
interface, with no claimed blueprint module identifier.

The worker invokes the existing workflow compiler, task-contract validator, glioma interpreter
and durable executor. Its application records add task retention, editable plan revisions,
exact revision approval, owned run storage and process activity tracking. They do not define a
second workflow language.

## Operator configuration

Launch the fixed module using an explicitly configured Python executable and argument vector:

```powershell
python -m prism_sdk.workflow_api_worker --root C:\private\aurora-workflows `
  --base-url http://127.0.0.1:11435/v1 --model qwen2.5:1.5b
```

The root's parent must already exist. The worker creates the root, `plans/` and `runs/` when
absent. The operator must restrict this directory to the application account. Root ancestors,
record directories and existing files must be ordinary filesystem entries without symbolic
links, Windows reparse points or hard-link aliases. Records contain consistency digests;
those digests do not authenticate an attacker with write access to application storage.

`--research-sources` adds the three separately reviewed public metadata readers.
`--molecular-glioma` registers the two local molecular coverage/review operations. These
registration switches do not establish live service coverage or authorize any dispatch.
`--glioma-mutations` registers the fixed GDC TP53 aggregate reader and its two local review
operations. `--gene-references` registers the fixed Ensembl TP53 reader and its two local
comparison/review operations; NCBI Gene is already in the default catalogue. All switches
remain opt-in. `--protein-pathways` adds the fixed UniProt P04637 and Reactome human annotation
readers plus two local protein/pathway review operations. Its `protein-pathways` planner selects
the exact four-step template without a model. General human annotations do not establish glioma
pathway activity, enrichment, scientific identifier equivalence or clinical support. All switches
default to false and preserve the legacy catalogue when omitted. The `glioma-mutations` and
`gene-references` planners select exact three/four-stage templates without a model call.
Task text labels those fixed queries and does not change their source selection. Operator
registration, exact retained contracts and explicit source approval remain separate checks.
`--timeout-seconds` bounds the configured local-model transport timeout to 1–300 seconds.
The default is 120 seconds. The model endpoint must be loopback and cannot contain userinfo,
query parameters or fragments. The worker never obtains an OpenAI key.

No request field can choose a filesystem path, model endpoint, executable, environment,
MCP process, OpenAPI specification, protected account, OAuth token or effectful operation.
Only the operator chooses startup configuration. The HTTP application must preserve that
boundary when launching this module, using an argument vector without a shell.

## Wire protocol

Write exactly one UTF-8 JSON object to stdin, then close stdin. The request bound is
1,000,000 bytes. The worker emits exactly one JSON envelope and a newline on stdout.
Duplicate fields, non-finite numbers, multiple JSON values, unsupported fields and invalid
literal boolean/integer controls are refused. Successful actions exit 0; error envelopes exit 2.

```json
{"schema":"aurora-workflow-api-response/0.1","ok":true,"data":{}}
```

```json
{"schema":"aurora-workflow-api-response/0.1","ok":false,"error":{"code":"revision_conflict","message":"The retained plan changed; review its current revision."}}
```

Error text is selected by the application. Remote exception text, source payloads, credentials,
paths and stack traces are not included. Execution failures remain separate observable run
states. An error response is never converted to an empty successful source result.

Every request has an `action` field:

| Action | Additional request fields | Result |
| --- | --- | --- |
| `catalogue` | none | Actual registered SDK operation catalogue and digest |
| `interpret` | `task` | Pure bounded glioma interpretation, including unresolved spans |
| `plan` | `task`, `planner`; optional `requirements`, `approve_provider_call`, `max_steps`, `planning_attempts`; incremental only: `max_model_calls`, `decision_attempts`, `operation_ids` | A retained compiler-accepted plan, or a tagged nonplanned result without plan retention |
| `get_plan` | `plan_id` | Current retained plan and task |
| `validate` | `plan_id`, `expected_plan_digest`, `proposal` | Checked and persisted replacement revision |
| `create_run` | `plan_id`, `expected_plan_digest` | New opaque run ID; no dispatch |
| `execute` | `run_id`, `expected_plan_digest`, `approve_source_dispatch`; optional `resume`, `retry_failed`, `resume_cancelled`, `max_step_calls` | Durable execution and metadata-only observation |
| `inspect` | `run_id` | Metadata-only run state and checkpoint projection |
| `cancel` | `run_id` | Acknowledged cancellation request |
| `export` | `run_id`, `kind` = `outputs` or `checkpoint`; optional `format` = `native` or `json_text` | Explicit retained public values or full checkpoint |

IDs are random 32-character lowercase hexadecimal values. Digests are 64-character lowercase
hexadecimal values. The server passes these opaque IDs; it cannot pass file names.
Unknown fields are rejected separately for every action.

The plan response has `status`, `plan_id`, `task`, `planner`, `plan_digest`, `plan`,
`source_dispatch: false` and, for typed intent, `interpretation`. The complete SDK plan includes
its task digest, catalogue digest, steps, outputs and fixed requirements. `planner` accepts
`glioma-intent`, `local-model`, `local-incremental`, `glioma-mutations`, `gene-references` or
`protein-pathways`. Both local-model choices require an operator-configured model and separate
literal `approve_provider_call: true`. Model generation is never source approval.
`max_steps` defaults to 16, accepts 1–32 and remains fixed in the retained task contract.
`planning_attempts` defaults to 1, accepts 1–4 and remains 1 for the typed planner.

`local-incremental` is an explicit opt-in to model-selected, bounded stepwise planning. Its legacy
`planning_attempts` must remain 1. `max_model_calls` is an integer from 1–128 (default 64), and
`decision_attempts` is an integer from 1–4 (default 2). Each accepted step needs at least two model
calls plus a later finish decision; rejected decisions consume the same total call budget. An
optional `operation_ids` is a nonempty exact allowlist of registered IDs, without duplicates.
The planner offers at most 32 visible read-only operations by default and refuses unsupported
schemas before provider dispatch. Omit the allowlist to offer the installed admissible catalogue.
Incremental controls on another planner are refused rather than ignored. Provider transport
retries remain one; the HTTP application's separate 300-second planning deadline still applies.

```json
{"action":"plan","task":"Inspect public PubMed metadata","planner":"local-incremental","approve_provider_call":true,"max_steps":2,"max_model_calls":5,"decision_attempts":1,"operation_ids":["pubmed.search"]}
```

The real tagged planning receipt is returned as `interpretation`. `planned` alone contains an
executable plan, which the worker validates and retains with its original task contract and
receipt. `clarification_required`, `refused` and `budget_exhausted` contain no `plan` or `plan_id`
and create no plan/run record, even if the receipt contains accepted partial step syntax. They
trigger no fallback graph, implicit retry, source dispatch or execution. A provider/configuration
failure remains an error envelope. Structural acceptance does not measure natural-language task
fidelity or scientific validity; callers must supply fixed requirements for constrained claims.

The browser exposes the incremental planner, call and decision bounds and optional operation
allowlist explicitly. It displays nonplanned receipts as inert text and clears executable-plan
approval state. Protein/pathway template availability requires all four installed operations;
browser choices cannot enable operator catalogue switches. The Rust server's corresponding
startup switch is `--workflow-protein-pathways`.

For example, typed planning makes no model or source call:

```json
{"action":"plan","task":"Review glioma evidence using PubMed only; exclude trials and TCGA.","planner":"glioma-intent"}
```

An unresolved typed request returns `status: "needs_clarification"`, the actual interpretation
and `source_dispatch: false`. It does not persist a broader plan.

## Editing and approval

`validate.proposal` has exactly `name`, `steps` and `outputs`, using the existing SDK format.
The worker always compiles that proposal against the retained original task and original
derived/caller contract. Browser-supplied task digests, catalogue digests, plan digests and
replacement requirements are not accepted as editing authority. Invalid edits do not replace
the current revision. A concurrent or stale edit returns `revision_conflict`.

The validation action projects the authoritative SDK reference-path and requirement audits into
`data.validation` on success or `error.diagnostics` on refusal. The browser may render these
diagnostics as inert text. They include issue codes, step/operation identities, paths, rule/link
indices and audit digests, with no actual argument values or source receipts. Each row collection
retains at most 64 rows, reports its actual omitted count and limits displayed text fields to
256 UTF-8 bytes without controls. A diagnostic audit that cannot complete is `not_assessed`,
without a fabricated zero-issue count. Structural validation still uses the existing compiler.
An admitted output pointer remains `runtime_resolution_required` until the actual source value
is available; schema admission is not evidence that a field exists or a claim is supported.

`create_run` freezes the exact current plan after the caller supplies its digest. Creating a
run grants no source access. `execute` requires that same digest and a separate literal source
approval. An edit that changes a plan's digest invalidates execution/resume authority for runs
created against the earlier revision. Existing observations remain available for inspection
and explicit export. A name-only edit changes the digest and therefore requires fresh review.

Each execute admission holds an OS file lock for its run and plan. Run admission alone may
wait up to one second in bounded 20 ms intervals for a brief idle observation lock to clear;
polling and cancellation keep their nonblocking lock behavior. Admission counters, grants,
checkpoints and source calls remain untouched until the run lock is acquired and the retained
plan and run are rechecked. A competing execution or edit receives `resource_busy`.
The HTTP server can run `execute` in a background child after
`create_run` returns its run ID, then poll `inspect`. It must not turn a timeout into a restart.

## Durable execution, cancellation and recovery

`max_step_calls` defaults to 32 and accepts literal integers 1–32. The authoritative SDK
executor persists a checkpoint before crossing each dispatch boundary and after each real
observation. Persistence failure before dispatch prevents that dispatch. Completed observations
are reused after validation; failed or unknown-completion calls never silently become success.

A run admitted once requires `resume: true` on later admissions. `retry_failed: true` and
`resume_cancelled: true` additionally require `resume: true`; neither flag supplies source
approval. The SDK retains its existing independent retry, call-budget and cancellation checks.

Cancellation increments an application-owned sequence under a short separate lock. The
executing process observes that sequence at checkpoint boundaries, stops admitting new calls
and drains an already admitted synchronous source call. The actual source result or failure
is retained before cancellation. A cancellation request is not evidence that the process has
stopped. An explicit cancellation release captures the observed sequence after run identity
validation and before waiting for admission. Once admitted, it can advance the release watermark
only through that captured generation, while preserving an already higher watermark. A later
cancellation, including one received during the admission wait, increments the sequence and is
not consumed by that earlier release.

`inspect.active` comes from an OS-held execution lock, rather than a timestamp or a stale lock
file. A retained `running` manifest with no lock owner is `interrupted`. Its checkpoint may
still describe an in-flight call whose completion is unknown. There is no automatic replay.
A later explicit resume still requires the SDK's distinct retry grant when a failure or
in-flight boundary remains. An admitted run with no durable checkpoint returns
`recovery_required`; it cannot guess that no call happened and restart.

Polling and execution responses include only run identities, state, checkpoint metadata and
recognized scientific review dispositions. They omit result values and source receipts.
`export` is a separate explicit action that returns retained public evidence. Application
storage itself contains those public values and requires the operator's retention policy.
Execution completion does not approve scientific claims or authorize clinical action.

The default `native` export preserves the original protocol's envelope with its `outputs` or
`checkpoint` field. `format: "json_text"` instead returns `run_id`, `plan_digest`, `status`,
`kind` and `json_text`, without duplicating the native artifact. `json_text` is the canonical
JSON string of the actual artifact root: the SDK checkpoint object for `checkpoint`, or the
outputs object for `outputs`. Write that string directly to the downloaded file. A checkpoint
download is therefore accepted by `load_workflow_checkpoint` without unwrapping an application
response. Passing the payload through a JSON string also preserves integers beyond JavaScript's
safe integer range and float tokens such as `0.0` and `-0.0`; an intermediary must not parse and
reserialize the artifact's numbers. The Rust HTTP application requests this format, unwraps the
string into the actual HTTP attachment and lets the browser download its response text unchanged.

## Limits

The inert `providers` action inspects the [reviewed provider registry](WORKFLOW_PROVIDER_REGISTRY.md)
bound to this exact configured catalogue. It accepts no source grants, URLs or credentials and
performs no provider/model calls. Reader registration, service grouping and live verification stay
separate. Operator startup `--denominator-coverage` enables the additional GDC SSM-data coverage
reader plus TP53 count and denominator-assessment operations. Planner `denominator-coverage`
builds the fixed four-step native-input review contract without a model or source dispatch.

This is a serial, bounded public research workflow application. It does not provide scheduling,
distributed leases, account authentication, OAuth, protected patient data, arbitrary writes,
MCP session recovery, model-grounded synthesis, automatic retries, a general natural-language
grammar or clinical decision support. Existing catalogue capacities are not counts of working
integrations. The HTTP application supplies browser authentication, origin/host policy,
body/output bounds and safe rendering; this worker alone does not secure an HTTP endpoint.

Its filesystem checks protect the application boundary against request-selected paths and
existing links/aliases. The private-root OS permission requirement is material: the stdlib
path operations do not claim an adversarial same-account process cannot race directory entries.

Dedicated tests exercise the actual stdio subprocess protocol, retained typed contracts,
model/source approval separation, real fixture dispatch, cancellation, checkpoint failures,
revision invalidation, restart uncertainty, metadata projection and explicit exports. Fixture
calls are not reported as live provider coverage.

Repeated operator startup `--capability gene-identity --capability glioma-ontology` enables fixed
[reviewed capability profiles](WORKFLOW_CAPABILITIES.md). The catalogue inventory distinguishes
selection, exact descriptor availability and unmeasured source availability. Plan requests select
`capability:gene-identity` or `capability:glioma-ontology`; they cannot supply configuration or
enable another profile. Fixed source contracts conjoin caller requirements and the requested step
bound. Neither planning nor inspection consumes model/source authority. Run metadata also projects
native denominator and three-source identifier review holds without claiming scientific approval.
