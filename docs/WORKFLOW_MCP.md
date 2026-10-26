# Reviewed MCP workflow integrations

`python/prism_sdk/workflow_mcp.py` binds an operator-reviewed list of tools from an already
connected `Client` to `WorkflowOperationCatalog`. Every registered entry has a real executor
using the repository's existing bounded stdio MCP protocol client. Importing the module performs
no discovery, launches no process, and opens no connection. This is a repository-defined SDK
extension; no unavailable source-blueprint capability is claimed.

The bridge follows the [MCP tools contract](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
for tool discovery, structured object results, optional output schemas, and execution error
flags. Server-provided annotations are untrusted descriptions. A server's `readOnlyHint` cannot
register a tool or authorize a call.

```python
from prism_sdk.client import Client
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.workflow_mcp import (
    mcp_workflow_server_identity,
    register_mcp_workflow_operations,
)

# Application code owns client creation, the reviewed argv, credentials and lifecycle.
# The bridge receives an existing connected client; it never creates or connects one.
def register_reviewed_search(client: Client):
    catalog = WorkflowOperationCatalog()
    identity = mcp_workflow_server_identity(client)  # Metadata only; no protocol request.
    operations = register_mcp_workflow_operations(
        catalog,
        client,
        integration_id="research_mcp",
        reviewed_read_only_tools=["search_metadata"],
        approve_tool_discovery=True,
    )
    return catalog, identity, operations

# Later, after separately authorizing source dispatch:
# result = catalog.execute(
#     "research_mcp.search_metadata",
#     {"query": "operator-selected public research query"},
#     approve_source_dispatch=True,
# )
```

The example tool is illustrative. It is not a claim that a particular service is installed,
connected, or independently verified. `reviewed_read_only_tools` is both the explicit tool
allowlist and the operator's read-only classification. Review must establish actual provider
behavior; a protocol annotation or a checked JSON schema cannot prove that a tool has no effects.
The current workflow executor accepts read-only tools. Authentication, consent, data-use
boundaries and any independent evidence-quality review remain application responsibilities.

Registration requires literal `approve_tool_discovery=True` and a live initialized `Client`.
It sends bounded `tools/list` requests, supports at most sixteen pages and 512 total tools, and
never sends `tools/call`. Missing, duplicate or malformed tool definitions fail registration.
Only explicitly reviewed names are registered; unrelated tools remain unavailable to a workflow.
All selected schemas are checked before catalogue mutation. Unsupported schema keywords, refs,
formats and credential-shaped schema fields cause refusal rather than a permissive replacement.

Each execution independently requires literal `approve_source_dispatch=True`. Without it, the
bridge sends neither discovery nor execution requests. With approval, it validates arguments,
checks the pinned connected-session identity, refreshes the tool list, checks every selected
definition against the review snapshot, and only then calls the exact reviewed tool name. A
changed input/output schema, description, annotation, selected tool removal, session,
configuration or caller-owned environment override prevents dispatch. Returned values are
checked against the reviewed output schema before becoming workflow results.

The local server identity binds the exact connected client, process, initialized session,
ClientConfig and explicit environment overrides. Exact argv and environment values are held only
in private in-memory equality snapshots. They are not serialized or hashed into published
credential fingerprints. An opaque session binding changes when those settings or the live
session change. Public metadata contains transport bounds, protocol version, handshake metadata
digests and the local server identity digest. Catalogue descriptors bind both server identity and
reviewed-tool digests, so a plan cannot silently use another connected session or tool contract.
This is a local binding, not remote server authentication or a proof of provider behavior.

Bindings deliberately last for one connected client session. Closing/reconnecting a client,
replacing its configuration, or starting another process requires fresh registration and a new
plan. Existing plans and checkpoints cannot automatically resume against that new binding.
The application owns the client lifetime, including keeping connections alive across workflow
stages. Ambient OS environment inherited when the caller originally launched a process is not
recoverable through `Client`; the bridge pins explicit caller-owned overrides and the actual
connected process/session. Arbitrary mutation of trusted Python application internals is outside
the bridge's protection boundary.

The result policy is explicit:

- `isError=True`, malformed error flags and JSON-RPC/transport failures are refusals. Remote
  exception messages, stderr tails and error payloads are withheld from bridge exceptions and
  receipts. A refusal never becomes an empty successful result.
- `structuredContent` must be a JSON object. When present it is the workflow value. Other content
  is omitted from the value, and the receipt records the omitted block count. A declared
  `outputSchema` requires structured content and is checked against it.
- Without an output schema, legacy compatibility accepts exactly one text content block whose
  text parses as an unambiguous finite JSON object. Multiple text blocks, duplicate JSON keys,
  plain text, arrays, scalar values, image-only results and embedded-resource-only results are
  refused. There is no invented `{"result": ...}` wrapper.
- If no output schema is declared, an object-only shape boundary applies and the receipt marks
  `output_schema_declared=False`. This does not claim semantic output validation.
- Tool data remains untrusted. Protocol success does not establish a scientific claim, clinical
  applicability, or patient-data authorization. No generic `ok` data property is reinterpreted as
  a protocol flag; `isError` and explicit schema checks determine this transport boundary.

Receipts retain operation/integration/tool identity; server, selected-definition, argument and
value digests; result encoding; omitted unstructured block count; declared-schema status; and an
optional caller-supplied retrieval clock. They contain no argv, environment values, raw arguments,
source values, raw content blocks, credentials or remote error details. Values are transient in
`WorkflowOperationResult`; exporting a workflow checkpoint is a separate explicit retention choice.

Bounds and limitations:

- Uses the existing synchronous stdio `Client`. HTTP/SSE MCP transports, server installation,
  shell command generation, OAuth setup, credential discovery, server registries, subscriptions,
  resource fetching, prompts, sampling and effectful-tool admission are not implemented here.
- Requires a finite client timeout from 0.1 through 120 seconds and a client frame bound no
  larger than 20,000,000 bytes. Discovery totals are bounded to 20,000,000 JSON bytes; each selected
  schema must also fit the existing operation-schema bound. The result envelope is bounded to
  2,000,000 bytes by default, configurable from one through 4,000,000 bytes. The client framing
  limit applies before the bridge's narrower result projection bound.
- Refreshes selected tool contracts immediately before each call. MCP does not provide an atomic
  schema-version precondition for `tools/call`; the bridge cannot prevent a server from changing
  behavior after it answers `tools/list`. Caller review and server-side controls still matter.
- Standard workflow catalogue limits remain 4,096 operations and 1,024 integration identifiers.
  Tests register 225 tool definitions through one real local paginated stdio peer and execute
  selected names; separate tests execute tools from two independent local peers. These are
  protocol/capacity tests, not claims of 225 independently connected services.

Focused verification from the repository root:

```powershell
$env:PYTHONPATH = 'python'
python -m pytest python/tests/test_workflow_mcp.py -q
```

## CLI connection lifetime

`workflow-catalogue` and `workflow-run` accept repeatable `--mcp-config FILE` arguments and a
separate `--approve-mcp-discovery` grant. Each JSON file has exactly these required fields, with
optional `cwd`:

```json
{
  "integration_id": "research_mcp",
  "command": ["python", "operator-reviewed-server.py"],
  "cwd": ".",
  "reviewed_read_only_tools": ["search_metadata"]
}
```

This is an illustrative operator-managed server, not an installed integration. `command` is an
argv array passed directly to `Client` without a shell. Relative `cwd` resolves from the config
file's directory; an omitted `cwd` inherits the caller's working directory. The configuration
does not accept environment, credential or authentication fields. Existing client inheritance
of the application's environment remains unchanged.

```powershell
aurora-agent workflow-catalogue --mcp-config reviewed-mcp.json --approve-mcp-discovery
aurora-agent workflow-run --task "Review public research metadata" --model installed-local-model `
  --out reviewed-plan.json --mcp-config reviewed-mcp.json --approve-mcp-discovery `
  --approve-provider-call --approve-source-dispatch --outputs-file public-results.json
```

The CLI validates all configurations and input/output path roles before starting any process.
It admits at most eight MCP configs, 64 argv parts per config, 32,768 total argv UTF-8 bytes,
8,192 bytes per part, and 512 explicitly reviewed tool names per config. An optional working
directory must already exist. Every started client closes on command completion or failure,
including partial startup, discovery, model and source failures. Discovery approval does not
authorize a model call or a tool call; provider and source approvals remain independent.

MCP configurations are refused by `workflow-build`, `workflow-template` and `workflow-execute`.
Their separate processes cannot preserve a connected-session binding. A one-shot MCP run still
writes its reviewable plan before dispatch and may export a checkpoint, but that checkpoint
cannot be resumed by another CLI invocation after its clients close. Use a caller-owned
long-lived SDK client to preserve one session across checkpoint pauses. `workflow-catalogue`
output describes the session it inspected; a later `workflow-run` creates a fresh binding.

The optional `--research-sources` flag adds the reviewed OpenAlex, Crossref and Zenodo readers.
Without it the existing default catalogue stays unchanged, including its saved-plan binding.
For `workflow-build` and `workflow-run`, `--requirements FILE` accepts an operator-authored
digest-bound workflow contract, and `--planning-attempts 1..4` explicitly bounds local model
planning calls. The default is one call. Requirements express checked operation, output,
argument and data-flow constraints; they do not prove that arbitrary natural-language intent
has been fully captured.

`--grounded-glioma` optionally registers local glioma drafting and exact citation audit operations
on every workflow command. It requires `--model`; optional `--base-url` and `--timeout` select the
credentialless loopback provider. Registration and template generation do not invoke that model.
A grounded template has six steps with TCGA retrieval, or five with `--no-tcga`:

```powershell
aurora-agent workflow-template --task "Review public glioma research evidence gaps" `
  --grounded-glioma --model installed-local-model --out grounded-plan.json
aurora-agent workflow-execute --plan grounded-plan.json --grounded-glioma `
  --model installed-local-model --approve-source-dispatch --approve-provider-call `
  --outputs-file grounded-review.json
```

The model, endpoint and synthesis bounds remain part of the catalogue binding and must match
between template/build and execution. Provider approval is independently supplied at execution;
source approval cannot enable the synthesis model. A completed workflow can still contain an
audit held for qualified scientific review. Its model text is unverified, scientific support
remains unestablished, and no clinical action is authorized.
