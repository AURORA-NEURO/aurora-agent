# Prism Python SDK

This package is the Python integration layer above the Rust AURORA/Prism kernel. It speaks the
repository's newline-delimited JSON-RPC MCP transport using only the Python standard library.

```python
from prism_sdk import Client, Workspace

with Client(["../target/release/bioprism-mcp", "--root", ".."], cwd="python") as client:
    workspace = Workspace(client)
    report = workspace.developer_delivery_audit(
        request_id="notebook-1",
        targets=["developer_platform", "repository_scope"],
    )
    print(report["release_request"]["targets"])
```

The async client has the same lifecycle and result semantics:

```python
from prism_sdk import AsyncClient, AsyncWorkspace

async with AsyncClient(["../target/release/bioprism-mcp", "--root", ".."], cwd="python") as client:
    report = await AsyncWorkspace(client).developer_delivery_audit()
```

The SDK keeps JSON-RPC transport failures, protocol violations, server errors, and structured tool
refusals distinct. It never invokes a shell, accepts unbounded frames, turns a refusal into a
successful value, or recreates the Rust domain model. `Workspace` helpers are deliberately thin
facades over exact MCP tools; `tool()` remains available for every current and future domain.

For a running `bioprism-api` gateway, the same standard-library package provides bounded HTTP
access:

```python
from prism_sdk import ApiClient

api = ApiClient("http://127.0.0.1:8787", bearer_token="0123456789abcdef")
print(api.capabilities()["tool_count"])
result = api.call_tool("modality_catalog", {})
page = api.events(after=0, limit=100)
```

`ApiClient` and `AsyncApiClient` cover health, capabilities, tools, typed capability discovery,
parity audits, route proposals, REST calls, event cursors, and the signed webhook outbox. HTTP
failures raise `ApiError` with the status and structured payload;
the client does not retry domain refusals or treat a transport `2xx` as scientific acceptance.
See [`docs/HTTP_API.md`](../docs/HTTP_API.md) for the route and delivery contract.

The package also includes dependency-free authoring builders for digest-bound benchmark packs,
set-valued decision cells, deterministic metamorphic mutations, versioned oracle manifests,
evidence judgements, reference panels, evaluation requests, and typed metrics observations,
paired contrasts, and calibration forecasts. They validate local JSON and cross-field invariants,
then let `Workspace` delegate final decisions and arithmetic to the Rust kernel through
`pack_health_assess()`, `mutation_family()`, `oracle_combine()`,
`metrics_analytics_audit()`, `developer_workbench()`, `agent_mission()`, `capability_discover()`, and the evaluation helpers.
The mission layer lets an agent preview or explicitly execute a bounded, allow-listed graph across
the existing domain tools while retaining refusals and blocking dependent work. The workbench
keeps authoring/notebook sessions, stale digests, capability holes, release posture, and review-only
CI planning in one evidence-bearing response; it does not pretend to execute a hosted UI or GitHub
runner. `MissionBinding` supports validated field-level dataflow between direct prerequisite steps,
and `CapabilityQuery` routes across the complete domain catalogue with optional tool schemas;
`capability_audit()` verifies the catalogue against the authoritative MCP schema set, and
`capability_route()` batches named needs without executing the returned candidates.
`AdapterRegistry` and `adapter_plan()` add a dependency-free format boundary for tabular and
biological sources: explicit DICOM, NIfTI/BIDS, AnnData/Zarr, VCF, BAM/CRAM, and OME-Zarr routes
are delegated to the mature Python ecosystem, while dependency missingness, scope dimensions, and
semantic-loss declarations remain visible before parsing. The planners never sniff or fetch bytes.
See
[`docs/PYTHON_SDK.md`](../docs/PYTHON_SDK.md) for the full authoring contract.
