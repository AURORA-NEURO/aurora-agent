# Application-bound workflow connections

`python/prism_sdk/workflow_connections.py` provides a typed application-owned credential session
for reviewed OpenAPI reads. It binds a fixed service, HTTPS server, expected account, application
tenant/actor/session, credential handle, authentication policy, expiry and explicit operation/resource
permissions. It is a repository-defined SDK extension; no source blueprint module id is claimed.

This module uses the existing in-memory `CredentialStore` directly. A service does not become an
LLM provider to obtain a credential handle. Importing or constructing a connection makes no network
request, discovers no account and obtains no secret. Account observation and connection authorization
are separate application decisions; each workflow source dispatch still requires its own approval.

The application supplies non-secret context identifiers and an already-configured handle:

```python
from prism_sdk.workflow_connections import (
    WorkflowConnectionContext,
    WorkflowConnectionResource,
    WorkflowConnectionSession,
)
from prism_sdk.workflow_openapi import import_openapi_operations

# credential_store and credential_handle are owned by the calling application.
# The handle's service identifier must be "research_service".
context = WorkflowConnectionContext("tenant-opaque-id", "actor-opaque-id", "session-opaque-id")
connection = WorkflowConnectionSession(
    service_id="research_service",
    context=context,
    expected_account_id="application-selected-account-id",
    credential_store=credential_store,
    credential=credential_handle,
    server_url="https://research.example.org/api",
    resources=[WorkflowConnectionResource(
        "research_service.lookup_record",
        {"record_id": ["reviewed-public-record-id"]},
    )],
    ttl_seconds=900,
)

# Obtain this observation through the application's separately reviewed identity process.
# Calling observe_account does not authenticate a provider or inspect a token.
connection.observe_account(observed_account_id, approve_identity_observation=True)
connection.authorize(approve_connection_use=True)
import_openapi_operations(
    catalog,
    reviewed_document,
    integration_id="research_service",
    server_url="https://research.example.org/api",
    allowed_hosts=["research.example.org"],
    reviewed_operation_ids=["lookup_record"],
    approve_spec_import=True,
    connection=connection,
    connection_context=context,
    approve_public_metadata_response=True,
)
```

The endpoint and operation above are illustrative. No provider connection is claimed. The
application must establish its current caller's tenant/actor/session before passing the context,
inspect `connection.binding_metadata()` before approval, and isolate runs and storage by that
subject. This SDK does not authenticate a browser request or make a shared catalogue safe to hand
to an unrelated tenant. Context identifiers should be opaque application identifiers, not secrets
or personal details.

`observe_account` records **application attestation**, never `provider_verified`. An observation
that differs from the immutable expected account latches `account_mismatch`; later observations
cannot repair that session. Create a new session after correcting the account selection. Account
authentication, signed identity proofs and provider scope verification remain outside this module.

Metadata keeps independent dimensions:

| Dimension | States and meaning |
| --- | --- |
| Credential | `configured` means the exact store handle is available; `unavailable` means expired, revoked, replaced or otherwise unresolved. Neither means an account was authenticated. |
| Account | `unknown`, `application_attested`, or `failed`; a configured credential does not promote unknown identity. |
| Authorization | `not_authorized` or `authorized`; account observation alone grants no connection use. |
| Lifecycle | `active`, `expired`, `revoked`, `closed`, or `unknown` when the clock cannot be evaluated. |
| Live verification | Always `unassessed`; fixture responses and successful schema checks do not establish a remote service observation. |

Account ids and credential values are absent from session metadata, catalogue descriptors and
receipts. The connection id is random and independent of both. No secret-derived fingerprint is
published. Application subject ids, public resource constraints and policy fields are non-secret
configuration; `binding_metadata()` exposes those fields for review. It is not a credential dump.

Resource grants name one registered operation and exact permitted scalar values per constrained
argument. Every path parameter must be constrained; a static route may have an explicitly empty
constraint mapping. Constrained optional arguments must be supplied. String, integer, float and
boolean values retain their strict JSON types, so `false` does not authorize integer `0`. All
arguments must independently satisfy the imported operation schema. There are at most 128 scoped
operations, 64 constrained arguments per operation and 128 values per argument. Resource objects
snapshot their input mappings, so mutating the original mapping cannot widen a grant.

Permissions, expected account, subject, handle identity, server, policy, expiry and clock are fixed
for a session. Exact private configuration snapshots refuse accidental mutation of those fields.
The SDK rechecks local state after foreign clock and credential callbacks before returning headers.
These guards catch application mistakes; they do not sandbox a process owner who can rewrite Python
code. Changing the account, permission set, credential handle or subject requires a new opaque
session binding and a newly compiled plan. Credential rotation under an existing handle is not
supported.

Session lifetimes are finite, greater than zero and at most one day; the default is 900 seconds.
`revoke()` and `close()` invalidate this connection without revoking other sessions sharing its
credential handle. The application may separately call `credential_store.revoke(handle)` to
invalidate all users of that handle. All these invalidations are checked before dispatch and when
the catalogue is serialized or hashed. Consequently a completed checkpoint cannot be replayed
through the same now-invalid session, even when replay would make no new source call. These
checks do not erase copies of public results already delivered to the application.

The default HTTPS transport validates arguments, source approval, resource scope and destination
policy before resolving the credential at header construction. It checks public DNS and prepares
the pinned, certificate-verified TLS connection first. Secret resolution does not run during catalogue inspection, planning,
checkpoint validation or refused private-DNS requests. An injected application transport owns its
network admission; the SDK still checks scope before resolving headers. Authentication is limited
to fixed bearer or API-key headers. Headers and application exception details never enter receipts.

Connection registration additionally requires literal `approve_public_metadata_response=True`.
The application must review that selected operations and resources return public research metadata
appropriate for the existing explicit-value checkpoint/export path. The flag records responsibility;
it does not inspect data classification or certify privacy. General private account payloads,
patient data and private-response persistence are not enabled by this interface.

The legacy OpenAPI `auth_headers` option accepts only a copied static mapping. Callable resolvers
are refused without invocation because they can change credentials under an old plan. Each static
mapping registration has an independent random binding, account identity remains `unknown`, and
there is no session revocation lifecycle. Use the typed session for deferred and revocable access.

Not implemented: OAuth authorization-code or PKCE flows, callback/state handling, refresh or
provider revocation, browser login, operating-system or external secret vault persistence, provider
identity probes, remote authorization-scope verification, general private payload retention,
cross-process session restoration, distributed revocation or independently measured service coverage.
CredentialStore external-resolver registration is an existing application integration point; this
module does not call a resolver or fetch credentials automatically.

Verification uses fixture credentials only, including a real loopback HTTP peer behind an injected
transport. It establishes protocol delivery and local lifecycle behavior, not connectivity to a
protected provider, OAuth support, or additional live integrations:

```powershell
$env:PYTHONPATH = 'python'
python -W error::ResourceWarning -m pytest python/tests/test_workflow_connections.py python/tests/test_workflow_openapi.py -q
```
