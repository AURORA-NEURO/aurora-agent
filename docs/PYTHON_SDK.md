# Python SDK

This document covers blueprint modules **11.04** (Python SDK), **11.05** (Python benchmark
authoring SDK), **11.15** (evaluator/oracle/mutation SDK), and **11.16** (environment and pack
authoring SDK). It also documents the typed client for the metrics analytics bridge. The package
remains dependency-free and keeps Rust authoritative for canonical bytes, scientific invariants,
metric arithmetic, oracle decisions, and release gates.

The repository ships `python/prism_sdk`, a standard-library client for the Rust MCP server. It is
the integration layer above the deterministic kernel described by [ADR-001](ADR-001-language-strategy.md):
Python can orchestrate and author requests, while Rust remains the owner of canonical bytes,
domain invariants, release gates, and evidence semantics.

## Lifecycle

Both clients enforce the MCP sequence:

```text
construct argv -> start child without a shell -> initialize -> notifications/initialized
       -> tools/list / tools/call / resources/read -> close stdin -> terminate or kill child
```

`Client` is synchronous and `AsyncClient` uses `asyncio.create_subprocess_exec`. Neither accepts a
shell command string. The caller supplies an argv sequence, an optional working directory, and an
optional environment overlay.

Every frame is UTF-8 JSON followed by one newline. Outbound and inbound frames have a default
20 MB bound, and every response has a finite timeout. A malformed frame, mismatched request id,
missing result, unexpected EOF, or process exit is a protocol/transport failure—not an empty tool
result.

## Error classes

The package distinguishes:

| Class | Meaning | Retry implication |
|---|---|---|
| `ArgumentError` | local argument or frame bound failure | fix the call; no request was sent |
| `LifecycleError` | start/initialize ordering error | fix client usage |
| `TransportError` / `ProcessExited` | process or stdio failure | retry only after inspecting process state |
| `ResponseTimeout` | peer exceeded the configured bound | retry only if the operation is safe to repeat |
| `ProtocolError` | peer violated JSON-RPC/MCP shape | do not interpret the result as evidence |
| `RemoteError` | JSON-RPC method-level error | use `code`, `message`, and `data` |
| `ToolRefusal` | valid tool payload with `ok: false` or `isError` | preserve the refusal; do not treat it as success |
| `ApiError` | HTTP gateway returned a bounded structured error | inspect status/payload; do not retry a domain refusal blindly |

`ToolResult` retains the raw MCP envelope, exposes all text blocks, decodes the server's JSON
projection, and provides `require_ok()` for callers that explicitly want an exception on a refusal.
Callers that need to render partial evidence can use `value()` without discarding the raw envelope.

## Domain helpers

`Workspace` and `AsyncWorkspace` are thin, typed facades. They do not duplicate domain models or
invent defaults:

- `developer_delivery_audit(...)` forwards exact nested evidence and creates a release request only
  when the caller supplies an id and target list.
- `bioatlas_publication_audit(atlas, ...)` preserves optional evidence, card, leaderboard, and
  weighting contracts and never implies publication without explicit targets.
- `compile_context(world, query, ...)` invokes `fiber_compile` while leaving policy/profile choices
  caller-controlled.
- `trace_otel_ingest(trace_id, otlp_json=... | document=..., ...)` invokes the bounded OTLP JSON
  importer and preserves its semantic-loss/readiness report.
- `developer_workbench(session, dashboard=..., ci=...)` composes the Rust authoring-session and
  notebook audit with optional hole-preserving capability queries and review-only GitHub Actions
  planning. The facade validates only the outer mappings; Rust validates digests, dependencies,
  evidence posture, release readiness, safe paths, and deterministic YAML.
- `agent_mission(mission_id, goal, steps, policy=...)` previews or executes a cross-domain tool DAG.
  `MissionStep` and `MissionPolicy` preserve domain labels, dependencies, explicit execution
  allow-lists, side-effect posture, refusal propagation, and output budgets; the server remains the
  authority for ordering and execution. `MissionBinding` can route a JSON-pointer field from a
  successful direct prerequisite into an existing argument slot.
- `capability_discover(...)` searches the complete domain catalogue by intent, domain, group, or
  tool and can attach authoritative MCP schemas for the returned routing matches.
- `capability_audit(include_groups=...)` verifies catalogue/schema parity, input-schema quality,
  coverage gaps, and intentional multi-group membership.
- `capability_route(goal, needs, ...)` batches named needs into a digest-bound, non-executing route
  proposal, preserving explicit tool matches separately from ranked candidates.
- `tool(name, arguments)` remains available for every current and future MCP domain.

`ApiClient` and `AsyncApiClient` provide the same standard-library SDK posture for the HTTP
gateway: health/capability discovery, typed `capability_discover`, `capability_audit`, and
`capability_route` helpers, REST tool calls, cursor-based event pages, and signed
webhook subscription/delivery acknowledgement. They preserve status and JSON error payloads in
`ApiError` and do not recreate Rust domain semantics.

## Metrics analytics across domains

`MetricObservation`, `PairedObservation`, and `CalibrationObservation` are small typed request
models for the Rust `metrics_analytics_audit` kernel. They deliberately use caller-owned
`dimension`, `domain`, `unit`, and `condition` strings, so one request can describe verification,
oncology, multimodal agreement, translation, runtime cost, or multi-agent coordination without a
new SDK release for every domain:

```python
from prism_sdk import (
    AnalyticsDirection,
    CalibrationObservation,
    MetricObservation,
)

report = workspace.metrics_analytics_audit(
    [MetricObservation(
        id="world-1",
        dimension="verification",
        domain="oncology",
        system="agent-a",
        value=0.82,
        direction=AnalyticsDirection.HIGHER_IS_BETTER,
        unit="fraction",
        condition="pack/4",
        replicate_group="parent-world-1",
        cost=4.2,
        latency_ms=180.0,
        evidence="reproduced",
    )],
    calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
)
```

The response preserves measured versus declared/missing/blocked populations, descriptive
performance/cost/latency summaries, replicate spread, paired deltas and retention for robustness
or cross-modal/translation/design review, and equal-width calibration bins with Brier and expected
calibration error. `Workspace` and `AsyncWorkspace` send exact wire models; they do not impute
missing rows, pool dependent trials, run an assay, infer a causal effect, or convert a contrast
into clinical evidence. An empty measured population stays `null` rather than becoming zero.

## Authoring packs, cells, and mutations

The authoring layer constructs the exact JSON shapes consumed by the Rust pack, decision-cell, and
mutation crates. It validates local invariants before a request is sent and computes the same
canonical SHA-256 content address as `bioprism-ids`:

```python
from prism_sdk import (
    DecisionCellBuilder,
    InputRef,
    PackBuilder,
    Workspace,
)

pack = (
    PackBuilder(
        pack_id="demo.pack",
        version=(1, 0, 0),
        schema_range=(1, 2),
        title="Decision evidence",
        measures="sufficiency of evidence selection",
        blueprint_module="15.01",
        axis="mechanism",
        capabilities=[{"agent": "evidence_acquisition"}],
        domains=["science"],
        owners=["aurora"],
        license="Apache-2.0",
    )
    .parent("world:demo", 3)
    .decision_family("smallest-sufficient-context")
    .mutation_relation("preserves_verdict")
    .oracle("deterministic")
    .authored_instances(8)
    .build()
)

cell = (
    DecisionCellBuilder(
        "cell-1",
        "select evidence",
        InputRef.from_document("world.json", {"world": 1}),
        InputRef.from_document("query.json", {"query": 1}),
    )
    .accepting("valid", "equivalent")
    .requiring_witness("closure")
    .build()
)
```

`PackArtifact.to_mcp_arguments()` prepares a digest-bound `pack_health_assess` request;
`Workspace.pack_health_assess()` then delegates health, calibration, contamination, and
reportability to Rust. `MutationPlan.standard()` mirrors the deterministic Rust suite and
`Workspace.mutation_family()` runs it against a root-confined world. The authoring layer never
marks a benchmark healthy, a mutation postcondition held, or a cell scientifically correct on its
own. `DecisionCell.accepts()` only evaluates the declared set-valued contract: wrong verdicts,
missing witnesses, and incomplete protected closure remain distinct failures.

## Oracle mesh and evaluation workflows

`OracleManifest`, `JudgementBuilder`, and `PositionDistribution` author the complete evidence
record used by `bioprism-oracle`: versioned `namespace:name` identities, declared versus effective
tiers, independence demotion, explicit evidential planes, fixed UTC validity windows, admissibility
states, checkable findings, abstentions, and tied distributions. A circular oracle is demoted by
the local builder before its judgement is serialized, so a caller cannot accidentally claim a
stronger rung than its declaration permits:

```python
from prism_sdk import (
    EvidenceTier,
    Independence,
    JudgementBuilder,
    OracleManifest,
    OracleRef,
    OracleVersion,
    Position,
    ValidityWindow,
)

manifest = OracleManifest(
    OracleRef("demo:checksum", OracleVersion(1, 0, 0)),
    EvidenceTier.DETERMINISTIC,
    frozenset({"artifact"}),
    frozenset({"biological", "causal"}),
    ValidityWindow("2025-01-01T00:00:00Z", "2030-01-01T00:00:00Z"),
    independence=Independence(),
)
judgement = JudgementBuilder(
    manifest, "2026-01-01T00:00:00Z", Position.SUPPORTED
).rationale("canonical artifact check passed").build()
```

`Workspace.oracle_combine()` forwards these judgements to the Rust mesh, which preserves
same-tier disagreement, suppressed weaker overrides, inadmissible evidence, and unknown states.
`oracle_reference_panel()` and `oracle_missingness()` cover independent-reader consensus and
small-cell/missingness boundaries. `bioeval_reference_audit()`, `evaluation_worldline_audit()`,
`evaluation_reproduction_check()`, and `evaluation_trajectory_check()` expose reference
uncertainty, future-evidence firewalls, reproducibility divergence, and bounded trajectory
properties. These helpers never locally choose a biological truth or convert an abstention into a
negative result.

The package deliberately does not claim to implement DICOM/NIfTI/AnnData/VCF readers, inferential
statistics, OTLP export, a notebook UI, or CI deployment. The repository now ships a bounded REST
gateway, pack/cell/mutation authoring contracts, and oracle/evaluation request contracts, but gRPC,
durable event storage, external webhook delivery, biological format adapters, statistical
estimators, and those domain artifacts remain separate contracts.

## Verification

From the repository root:

```bash
cd python
python -W error::ResourceWarning -m unittest discover -s tests -v
python -m compileall -q prism_sdk tests
```

The tests use a subprocess fake MCP peer so lifecycle, framing, protocol, remote errors, structured
refusals, sync/async parity, and cleanup are exercised through actual pipes rather than direct
function calls.
