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

`ApiClient` and `AsyncApiClient` cover health, capabilities, tools, REST calls, event cursors, and
the signed webhook outbox. HTTP failures raise `ApiError` with the status and structured payload;
the client does not retry domain refusals or treat a transport `2xx` as scientific acceptance.
See [`docs/HTTP_API.md`](../docs/HTTP_API.md) for the route and delivery contract.

The package also includes dependency-free authoring builders for digest-bound benchmark packs,
set-valued decision cells, and the deterministic metamorphic mutation suite. They validate local
JSON and cross-field invariants, then let `Workspace.pack_health_assess()` and
`Workspace.mutation_family()` delegate final decisions to the Rust kernel. See
[`docs/PYTHON_SDK.md`](../docs/PYTHON_SDK.md) for the full authoring contract.
