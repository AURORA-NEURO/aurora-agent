# Reviewed OpenAPI workflow integrations

`python/prism_sdk/workflow_openapi.py` imports an explicit list of reviewed OpenAPI GET operations
into `WorkflowOperationCatalog`. Each registration includes an executable HTTP adapter and checked
input and output schemas. A document's operation count is not an independently tested service count.
This is a repository-defined SDK extension; the checkout does not contain a detailed source
blueprint for an OpenAPI importer, and this module makes no blueprint-coverage claim.

The importer follows a deliberately restricted subset of the
[OpenAPI specification](https://spec.openapis.org/oas/v3.0.3.html): path parameter substitution,
scalar `simple` path serialization, scalar `form` query serialization, and explicitly declared
JSON success responses. Unsupported selected contracts fail import rather than silently losing
their constraints. Unselected operations are never registered.

```python
import json
from pathlib import Path

from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.workflow_openapi import import_openapi_operations

catalog = WorkflowOperationCatalog()
document = json.loads(Path("operator-reviewed-openapi.json").read_text(encoding="utf-8"))
operations = import_openapi_operations(
    catalog,
    document,
    integration_id="research_service",
    server_url="https://research.example.org/api",
    allowed_hosts=["research.example.org"],
    reviewed_operation_ids=["lookup_record"],
    approve_spec_import=True,
)

# Inspect catalog.to_dict() before separately authorizing source dispatch.
# The arguments must exactly match the selected operation's declared parameters.
result = catalog.execute(
    "research_service.lookup_record",
    {"record_id": "operator-selected-record"},
    approve_source_dispatch=True,
)
```

The URL and operation in this example are illustrative. No example endpoint is claimed as a live
integration. Applications can register independently reviewed specifications under distinct
`integration_id` values; the shared catalogue currently bounds registrations to 4,096 operations
and 1,024 integration identifiers. The tests exercise 225 distinct operation mappings and 210
independent registration identifiers using a local injected response fixture. These demonstrate
extensibility and HTTP request construction, not verified connectivity to 210 services.

Import does not fetch a document or contact a server. `approve_spec_import=True` is an operator's
statement that the specification and chosen operations have been reviewed. It grants no source
dispatch permission. Each execution independently requires literal `approve_source_dispatch=True`.
GET classification does not prove that an arbitrary provider implements a read-only operation;
review must establish the provider's behavior. These adapters carry research data and metadata;
HTTP/schema success does not establish scientific validity or authorize clinical use.

An application may supply a [typed connection session](WORKFLOW_CONNECTIONS.md) for lazy,
revocable, account-bound credential use. An older `auth_headers={...}` interface accepts a copied
static header mapping. Callable header resolvers are refused without invocation: a callable
could replace its account or credential while retaining an already-approved catalogue identity.
Mutating the original mapping cannot change an installed executor. Each mapping registration has
an independent random binding, so registering a replacement mapping changes `catalog_digest` even
when the operation specification is identical. That binding is independent of secret bytes and
does not identify or authenticate an account. Static mapped headers remain in process memory for
the executor lifetime; they have no session revocation or account observation support.

Raw key fields cannot be declared as operation parameters. Header authentication
may be declared using OpenAPI `apiKey` in `header`, or HTTP `basic` / `bearer`; a declared secured
operation requires application-owned authentication. A typed connection must match the declared
header name and authentication policy; its supported policies are bearer and API-key headers.
The importer does not obtain keys, refresh OAuth tokens, persist credentials, or infer that a
supplied credential satisfies a provider's policy.
Header values, raw request arguments, response headers, and URLs with argument values are excluded
from receipts. Credential-resolution and transport failures suppress application exception text.
Typed connection imports additionally require literal `approve_public_metadata_response=True`:
the application has reviewed the selected operation and scoped resources for public metadata
retention. This is an application attestation, not an SDK privacy certification. General private
account payload retention is not implemented by the workflow checkpoint store.

Receipts contain operation/integration identity, specification, request and response digests,
HTTP status, response byte count, optional caller-supplied retrieval time, and explicit response
schema verification. `transport_mode` distinguishes `application_injected` from `pinned_https`;
`live_verification` remains `unassessed` in both cases. The former `live_verified` boolean has been
removed because selecting a transport does not prove a live-provider observation: tests can mock
the default connection too. Independent live connectivity and evidence-quality measurements must
be recorded separately.
The catalogue descriptor also includes the reviewed document digest and fixed server so replacing
either changes `catalog_digest` and invalidates a plan bound to the previous catalogue.
Typed connection descriptors additionally bind the opaque session, application subject, fixed
service/server/authentication policy, expiry and exact operation/resource permissions. A new
session therefore requires a new plan. Revocation, expiry, credential-handle substitution and
closed sessions refuse catalogue serialization and execution, including reuse of a completed
checkpoint with the same catalogue object.

The default transport permits fixed HTTPS destinations on exact lowercase allowlisted hosts. It
rejects userinfo, queries in the base URL, variable servers, redirect statuses, changed final URLs,
path traversal, nonstandard ports, private addresses and loopback addresses. DNS results must all
be public; the chosen checked address is used directly while TLS verifies the original hostname.
The connection uses an absolute request deadline (default 15 seconds, maximum 120) and reads at most
the configured response limit plus one byte (default 1,048,576, maximum 4,000,000). HTTP redirects
are never followed. It performs no retries. The deadline includes bounded DNS admission and each
blocking I/O operation; synchronous application callbacks are checked for elapsed time but cannot
be forcibly interrupted by this SDK.
For typed connections, argument, scope and dispatch-approval checks run first; public-DNS
admission and verified pinned TLS precede resolving secret bytes. The connection closes
on credential-resolution failure before a request is sent. An admitted synchronous request cannot
be recalled by a later revocation. If the session is invalid by receipt construction, the adapter
refuses the result instead of issuing a successful connection-bound receipt.

`allow_private_hosts=True` is an explicit application policy for trusted private HTTPS services;
it also admits their nonstandard ports. It does not disable TLS verification. The default rejects
these destinations. An injected `transport(url, *, headers, timeout_seconds, max_response_bytes)`
is trusted application code and owns its actual network admission, timeout and bounded-read
implementation. The importer still checks its returned response size, status, content type, final
URL, JSON shape and declared schema. Test injection never opens arbitrary network connections.
The connection tests include an actual loopback HTTP peer using fixture-only credentials through
an explicitly injected transport. This checks HTTP mapping and header delivery; it does not
exercise a live provider or weaken the default HTTPS destination policy.

Supported schema and contract surface:

- OpenAPI 3.0.x supplied as a finite JSON mapping or JSON string, at most 4,000,000 bytes.
- A fixed explicitly selected `servers` entry; no path or operation server overrides.
- Explicit unique `operationId` values; selected operations must use `get`.
- Scalar string, integer, number and boolean path/query arguments. Path parameters must be
  required. Inheritance and operation-level overrides are keyed by parameter location and name;
  colliding argument names across locations are refused.
- Schema types `object`, `array`, `string`, `integer`, `number`, `boolean`; bounded nested
  `properties`, `required`, boolean `additionalProperties`, array `items`, scalar `enum`, numeric
  `minimum` / `maximum`, string `minLength` / `maxLength`, array `minItems` / `maxItems`, and
  `title` / `description` annotations. Response roots must be objects. Optional parameters remain
  absent when omitted; defaults are not synthesized.
- Explicit success status codes whose `application/json` bodies all declare the same schema.
  Every returned success status must be declared; error statuses remain failures.

Not implemented and refused when needed by a selected contract:

- OpenAPI 2, 3.1 or 3.2, YAML parsing, automatic specification download or provider discovery.
- Local or remote `$ref` resolution, polymorphic/composed schemas, `nullable`, `format`, regex
  `pattern`, default interpretation, schema-valued `additionalProperties`, binary/file values,
  callbacks, webhooks, external response links and request bodies.
- Mutation methods, header/cookie arguments, object/array parameter serialization, nonstandard
  styles, wildcard success status codes, heterogeneous success schemas, top-level array
  responses, streaming, pagination, automatic retries and rate-limit scheduling.
- Query/cookie API keys, OAuth flow handling, token refresh, interactive login, credential
  persistence, proxy support, independent service availability or evidence-quality measurement.

Focused verification from the repository root:

```powershell
$env:PYTHONPATH = 'python'
python -m pytest python/tests/test_workflow_openapi.py -q
```

The default transport now uses `workflow_openapi_http.py`, with one absolute deadline across
bounded DNS admission, TCP, verified TLS, header admission, every send/read and strict HTTP
framing. At most four outstanding platform DNS jobs can remain live; a timed-out resolver keeps
its slot until it actually exits. It pins a checked numeric address while retaining certificate
and original-hostname verification. Routing, proxy, compression and framing headers cannot be
supplied as authentication headers. The response parser is shared with
[reviewed public HTTP](REVIEWED_PUBLIC_HTTP.md), whose credentialless policy is unchanged.

Typed connection credentials are resolved **after verified TLS** and checked before the first
send. Application credential resolvers and injected transports are synchronous trusted code;
the SDK cannot forcibly interrupt a blocked callback. It checks elapsed time before dispatch
and after an injected response, so an over-budget result is refused. Injected transports receive
the original configured timeout and own their actual I/O bounds. Revocation after header
admission cannot retract bytes already sent. No automatic retry or rate-limit sleep is performed.

`OpenApiTransportError` retains sanitized failure class, optional HTTP status and optional
bounded numeric Retry-After seconds. Missing/unsupported Retry-After is `None`, distinct from an
observed zero. No source error body, credential or foreign exception text is published.
Transport configuration and policy digests enter the descriptor/catalogue identity and receipt;
changing timeout, response bound or private-network policy requires a newly compiled plan.
Injected transports have independent random registration bindings, never secret fingerprints.

The focused wire tests use fixture TLS streams and establish framing, deadline and admission
behavior. They do not demonstrate connectivity to a protected provider. Python's
[HTTP client](https://docs.python.org/3.12/library/http.client.html) and
[socket](https://docs.python.org/3.12/library/socket.html) documentation describe the underlying
platform interfaces; the total-deadline and stricter framing policies are repository extensions.
