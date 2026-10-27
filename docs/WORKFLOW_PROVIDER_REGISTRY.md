# Reviewed provider and evidence registry

`python/prism_sdk/workflow_provider_registry.py` adds a versioned inventory bound to actual
executable catalogue entries. It supports onboarding and request inspection as the number of real
integrations grows. It does not manufacture providers from operation IDs, endpoints, imported
schemas or catalogue capacity. This is an application contract without a blueprint module ID.

```powershell
aurora-agent workflow-catalogue --provider-registry
aurora-agent workflow-catalogue --provider-registry --research-sources --glioma-mutations --gene-references --protein-pathways --denominator-coverage --capability gene-identity --capability glioma-ontology
```

Inspection performs no source or model request and enables no optional reader. The ordinary
catalogue output and saved-plan identity remain separate. The local application exposes the same
inventory through its provider inspection route; startup catalogue switches control which
readers are actually registered.

## Units and states

The manifest contains **18 concrete reviewed reader paths in 15 declared API service groups**.
The unchanged default catalogue registers eight readers in seven groups; ten optional readers
remain `not_registered`. PubMed and NCBI Gene share the NCBI E-utilities group. GDC project
summaries, fixed TP53 mutation counts and project SSM-data coverage share one GDC group.
HGNC and NCI EVS add native identifier and terminology readers through fixed
[capability profiles](WORKFLOW_CAPABILITIES.md). NCI EVS and GDC have separate reviewed API
contracts but a shared NCI affiliation; their organizational independence remains unassessed.

These are application-reviewed API groupings, not independently operated organizations or
independently verified live providers. Europe PMC and GWAS Catalog remain separate reviewed API
contracts on a common EBI authority; sharing or differing hostnames cannot establish organizational
independence. The independent live-provider count has `value: null` and an explicit
`not_established_by_registry` state.

| Reader registration state | Meaning |
| --- | --- |
| `registered_reviewed_default` | Exact normalized descriptor and default executor code, literal captures and identity match the reviewed registration |
| `not_registered` | Implemented optional reader is absent from this catalogue; this is not an outage |
| `registered_contract_differs` | The operation ID exists with a different normalized contract or transport binding |
| `registered_executor_not_reviewed_default` | The descriptor matches, but a caller executor or fetcher replaces the reviewed default |

Unknown local, OpenAPI and MCP operations remain visible in `unreviewed_operation_ids`. Their
executor identity is anchored, while provider/configuration semantics are explicitly unchecked by
this registry. Importing an API or registering a hundred tools therefore cannot inflate reviewed
service coverage. The existing OpenAPI, MCP and connection layers retain their own configuration
and authorization checks.

Every reviewed record keeps registration, credentialless account status, dispatch authorization,
live observation, continuous availability and scientific support separate. No observation is
imputed from a missing value. A successful reader can return a source-specific empty result; the
registry does not infer empty-result semantics or availability from a successful method return.

## Contract and runtime binding

Provider identities, reader contracts and evidence references have individual schema versions.
Their public mappings are immutable, and exported documents are detached copies. The manifest
version and digest bind:

- Explicit service identity and review basis, with provider verification still unestablished.
- Exact operation descriptor, argument defaults/bounds, output schema, auth and read-only status.
- Source adapter, registration, normalization/validation component SHA-256 values. Readers using
  reviewed public HTTPS also bind that transport component.
- Repository review-document references and separate external evidence references, if supplied.

The registry binding adds the actual catalogue digest. Every inspection, preparation and dispatch
checks the catalogue, executor identities, captured default controls and component source bytes.
Changed registration, executor or source bytes require a new registry. Contradictory hashes for a
shared component are refused, rather than retaining whichever reader was processed last.

Component hashes identify retained source bytes. They do not attest the interpreter, arbitrary
foreign module globals, all transitive runtime dependencies, upstream official API versions or
remote service behavior. The upstream contract version is explicitly `not_established`; the
repository's normalized adapter contract is what is actually versioned here. This is an application
consistency boundary, not a sandbox against arbitrary code in the same Python process.

## SDK request inspection and execution

```python
from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
from prism_sdk.workflow_provider_registry import reviewed_workflow_provider_registry

catalog = builtin_workflow_operation_catalog()
registry = reviewed_workflow_provider_registry(catalog)
inventory = registry.to_dict()
inspection = registry.inspect_request("pubmed.search", {"per_specialty_limit": 2})
request = registry.prepare("pubmed.search", {"per_specialty_limit": 2})
# Preparation normalizes defaults and checks schemas; it grants no dispatch authority.
execution = registry.execute(request, approve_source_dispatch=True)
observation = execution.observation()
native_bundle = execution.result.value
```

The prepared request binds the exact normalized arguments, reader contract, registry and catalogue.
Dispatch accepts only the same current request and literal source approval. It delegates to the
existing concrete adapter; there is no second HTTP implementation or retry mechanism. No secret,
model-selected endpoint, provider discovery or account setup is accepted through this API.

The observation retains one catalogue-execute invocation, actual result/receipt digests or a
sanitized failure, and its clock/binding. It does **not** call that invocation one HTTP request:
readers have different request counts, and actual HTTP traffic is not independently observed by
this wrapper. `actual_http_request_count.value` remains null. Foreign error messages, native
payloads, titles and credentials do not appear in the metadata-only observation.

`WorkflowProviderDispatchError.observation()` retains a failed attempt. If the binding changes or
becomes uninspectable after entry, `binding_invalidated_after_dispatch` retains the original
adapter success/failure and any actual result/receipt digests while refusing result admission.
It never recasts that entered attempt as zero calls. The application can separately retain native
source-specific failure evidence; this registry does not infer that every schema/adapter failure
means the remote provider was unavailable.

`registry.verify_observation(observation, request, result=execution.result)` checks retained
binding/schema/digest consistency without dispatch. It is an integrity check, not a fresh
acquisition, scientific validator or authenticated audit. A fixture can replace a foreign module
global while the default executor and `builtin_https` receipt remain unchanged; focused tests
demonstrate why those facts cannot establish network authenticity or live-provider coverage.

## Evidence and onboarding

`WorkflowProviderEvidenceReference` records an operation ID, separate acquisition/replay/fixture/
failed/unjudged kind, opaque reference ID and caller-declared artifact SHA-256. These references
are never dereferenced, opened or trusted automatically. Duplicate references and unknown readers
are refused. An acquisition label does not upgrade `unverified_external_reference` to a verified
measurement. The registry starts with no supplied external reports; retained campaign artifacts
can be linked explicitly by the application without changing their trust level.

The bounded public source observations described in the source workflow documents remain their own
evidence. This registry neither repeats them nor upgrades them to continuous availability, source
truth, independent provider operation, scientific validity or clinical authority.

For each additional real provider, an application operator must review the service identity,
normalized contracts, source/transport versions, data class and unsupported actions; register a
concrete executor; and separately establish bounded runtime behavior, failure handling and actual
live evidence. Schema-only `inspect_request` conformance proves admission of that one normalized
request, not pagination, rate-limit handling, outage behavior or full provider support. Protected
accounts still require verified identity/scopes, protected token lifecycle and tenant/retention
policy. Durable quotas, broad authenticated onboarding and hundreds of verified integrations remain
required work; neither the registry's capacity nor these eighteen paths establishes that objective.
