# Reviewed public HTTPS transport

`python/prism_sdk/reviewed_public_http.py` is a dependency-free transport for new public research
adapters. It is a repository-defined extension with no source blueprint implementation claim.
Existing source modules and their transport digests remain unchanged.

The application installs exact credentialless HTTPS routes. Dispatch accepts an installed route
identifier and a separate literal approval; it has no URL, request body, header, credential,
proxy, redirect, or retry parameter. An adapter still owns its source selectors, JSON parsing,
tree and record budgets, normalization, provenance replay, and scientific limitations.

## Installed configuration and request

```python
from prism_sdk.reviewed_public_http import (
    ReviewedPublicHttpClient,
    ReviewedPublicHttpConfig,
    ReviewedPublicHttpError,
)

# Illustrative routes, supplied by application code after reviewing the source contract.
configuration = ReviewedPublicHttpConfig(
    allowed_hosts=("research.example.org",),
    reviewed_routes={
        "tp53_annotation": "https://research.example.org/annotations/TP53?format=json",
    },
    timeout_ms=30_000,
    max_response_bytes=1_000_000,
    accepted_content_types=("application/json",),
)
client = ReviewedPublicHttpClient(configuration)
reviewed_configuration_digest = configuration.config_digest
reviewed_policy_digest = client.transport_policy_digest

try:
    response = client.get("tp53_annotation", approve_source_dispatch=True)
except ReviewedPublicHttpError as failure:
    failure_metadata = failure.to_dict()
else:
    public_bytes = response.body
    acquisition_metadata = response.to_metadata()
```

The application must install configuration in its trusted code. This is not an endpoint registry
that a model or browser may populate. GET selection alone does not establish an endpoint's source
semantics; the adapter author reviews those semantics separately. Dispatching an unknown ID or
anything other than literal `True` refuses before DNS or socket creation.

Configuration snapshots are immutable and deterministic. Mutating an original host list, route
mapping, or media-type list cannot change an installed client. `config_digest` binds all routes,
hosts, deadlines, response/header/framing limits, accepted media types and the versioned policy.
`REVIEWED_PUBLIC_HTTP_POLICY_ID`, `REVIEWED_PUBLIC_HTTP_POLICY_VERSION` and
`REVIEWED_PUBLIC_HTTP_POLICY_DIGEST` identify that policy before a request occurs. Both the config
and client expose `transport_policy_digest`; `configuration.to_metadata()` returns a defensive
copy of the reviewed configuration and its digest. These are consistency identifiers, not code
attestation, signed provider provenance or authorization inferred from a hash.
Each dispatch revalidates all current configuration values against its registration snapshot before
DNS. It then uses a detached validated copy throughout the request, so even a trusted callback
mutating the original object cannot alter its wire request or receipt binding. Deliberate Python
reflection that substitutes configuration values or the encoded snapshot is refused on admission;
these consistency checks do not sandbox arbitrary same-process Python code.

## Admission and transport limits

At most 32 exact lowercase DNS hostnames and 128 named routes can be installed in one config.
Routes use HTTPS port 443, optionally spelled `:443`, with a fixed absolute path. Userinfo,
fragments, control bytes, backslashes, relative path segments, malformed escapes and credential-
shaped query keys are refused. Query keys are decoded before checking; ambiguous nested escaping
is refused. URL values are at most 8,192 ASCII characters, paths and queries at most 4,096 each,
and queries at most 64 fields. The complete configuration is at most 262,144 UTF-8 bytes.
Application code remains responsible for choosing public, non-sensitive route values.

The configured per-request deadline is 100 through 30,000 milliseconds. One absolute monotonic
deadline covers DNS waiting, connect, TLS handshake, every send, status/header parsing, every body
receive, and chunk framing. A late result is refused. It does not reset the budget for a trickling
header, body, chunk stream or partial request send. Each `get()` has its own budget; a multi-request
adapter must separately enforce its total operation budget.

All clients share four DNS-job slots. A resolver that outlives its wait retains its slot until
it finishes. A fifth blocked request cannot spawn another DNS job, and a late resolver result
cannot initiate HTTP. Each DNS result is bounded to 64 answers; every answer must be a public,
non-multicast IPv4/IPv6 address at port 443. The selected numeric address is passed directly to
the socket, and TLS authenticates the original reviewed hostname. Proxy environment variables
are not read. The fixed request headers select the reviewed Accept types, identity encoding,
the versioned User-Agent, and connection close. No authorization, cookies or payload is sent.

Successful status is exactly 200. Accepted media types are exact `application/json` or explicit
`application/...+json` types; wildcards and parameters in configured media types are refused.
Responses may omit a charset or declare UTF-8/UTF8, including quoted forms. Other charset or
media-type parameters and compression are refused. Successful body bytes must be strict UTF-8.
The transport does **not** parse or validate JSON syntax, duplicate JSON keys, nonfinite JSON
values or tree shape. A successful transport observation cannot replace those adapter checks.

Body bytes default to 1,000,000 and can be configured from 1 through 4,000,000. Header bytes default
to 65,536, with a configurable range of 128 through 65,536. Framing bytes default to 65,536, with a
range of 16 through 65,536. These are independent per-response limits. Total received wire bytes
are bounded by their sum. The HTTP parser requires HTTP/1.0 or HTTP/1.1 status lines, CRLF headers,
unique valid headers, a real CRLF blank header terminator (including interim 100 responses), and
unambiguous framing. Content-Length must be bounded and match the body.
Chunked responses require HTTP/1.1, bounded hexadecimal sizes, exact CRLF delimiters and a final
empty trailer line. Chunk extensions, trailers, redirects, unsupported encodings and malformed
headers refuse the request. Close-delimited bodies retain the same deadline and byte limits.
Policy version `2` explicitly includes bounded EOF framing in its deterministic policy identity.

## Values, failures and verification

`ReviewedPublicHttpResponse` is frozen and retains immutable `body` bytes alongside `status`,
`request_url`, `response_bytes`, `body_sha256`, `content_type`, `transport_policy_digest` and
`config_digest`. The constructor verifies the byte count, digest and UTF-8 representation.
`to_metadata()` omits body bytes and raw headers. Its `live_verification` remains `unassessed`;
fixtures, successful TLS and schema checks do not count as independently measured integrations.
An actual empty HTTP body remains the actual empty bytes; a source adapter must decide whether
its source contract permits that observation. No failure is replaced with an empty success.

`ReviewedPublicHttpError` retains immutable `failure_class`, optional `status_code` and optional
`retry_after_seconds`. Its string and `to_dict()` contain no payload, raw headers, DNS answers,
URL arguments or foreign exception details. For HTTP 429, one bounded numeric Retry-After from
zero through 86,400 seconds remains metadata. A missing, date-shaped, oversized, negative or
nonfinite value remains `None`, not zero. There is no sleep or automatic retry. Other non-200
statuses remain failures. Cleanup attempts all owned resources; a close failure cannot turn a
request into a successful observation or expose foreign exception text.

There is no caller-supplied fetch callback in this transport. An adapter may provide its own
fixture seam, but it must bind that separately and label it as caller-supplied. Reusing these
response fields or policy digests does not establish that a fixture used the built-in transport.
The module does not provide OAuth, private-account access, credentials, integration discovery,
generic model-controlled network execution, scientific validity or clinical decisions.

Focused offline verification uses real HTTP parsing over fake TLS wire fragments:

```powershell
$env:PYTHONPATH = 'python'
python -W error::ResourceWarning -m pytest python/tests/test_reviewed_public_http.py -q
```

Those tests exercise fixed route/approval admission, DNS-slot retention, TLS hostname pinning,
partial sends and header/body/framing deadlines, strict valid and malformed framing, UTF-8/media
checks, response bounds, retry metadata, immutable snapshots and sanitized failures. They make
no live request and do not measure service availability or biological evidence quality.
