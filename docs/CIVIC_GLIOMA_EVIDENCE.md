# Reviewed CIViC glioblastoma-name evidence metadata

This repository-defined extension has no available detailed blueprint module. It adds one
optional reader, `civic.glioblastoma_evidence_metadata`, and the
`civic-glioblastoma-evidence` capability profile. It retrieves curation categories and source
publication identifiers for research review. It does not infer clinical support, recommend
treatment, retrieve patient records or establish a patient, cohort or assay denominator.

The [official CIViC API documentation](https://docs.civicdb.org/en/latest/api.html) and
[GraphQL schema documentation](https://griffithlab.github.io/civic-v2/#query-evidenceItems)
were reviewed on 2026-10-27. The fixed anonymous endpoint is
`https://civicdb.org/api/graphql`. The API documents an anonymous limit of three requests per
second over a five-minute window. This reader applies a stricter refusal-based one-start-per-second
policy per process. It never waits, retries, follows redirects or obtains credentials. Distributed
quota enforcement remains unimplemented.

The exact installed operation `AuroraGlioblastomaEvidenceMetadataV1` requests the first 25
`ACCEPTED` evidence items with `diseaseName: "glioblastoma"`, sorted by ascending native ID.
This is a substring filter, not all glioma terms or an ontology descendant expansion. Selected
fields are `totalCount`, `pageInfo { hasNextPage endCursor }` and each item's `id`, `status`,
`flagged`, `openRevisionCount`, `evidenceType`, `evidenceLevel`, `evidenceDirection`,
`evidenceRating`, `disease { id doid name }` and `source { id citationId displayType }`.
The query, operation name, empty variables, endpoint and request body are fixed in code; the
operation input exposes only a 100–30,000 ms timeout.

The [CIViC evidence model](https://docs.civicdb.org/en/latest/model/evidence.html) describes
curated statements linked to publications. A publication can supply several evidence items;
returned item counts therefore remain item counts. This reader retains each item's native
publication identifiers separately, without deduplicating them into an independent evidence count
or inventing a publication URL from a display label. It does not retrieve descriptions, therapies,
molecular profiles, variants/HGVS, sequences or source abstracts. Publication retraction status
and evidence quality are unassessed, not assumed favorable.

The separate `reviewed_public_graphql` transport binds an immutable application-installed UTF-8
JSON body to an installed route identifier. It sends one POST with exact Content-Length, fixed
JSON headers and no authentication, proxy inheritance or dynamic query argument. It reuses the
unchanged reviewed GET transport's public DNS pinning, hostname-verified TLS, strict bounded
HTTP framing and absolute deadline. Its POST policy/configuration identities are separate; the
GET policy, configuration bytes and existing reader descriptors are unchanged. The generic
transport's envelope check is not a GraphQL semantic validator: the CIViC adapter binds the
complete reviewed query and rejects any changed request contract.

Each response is bounded to 524,288 bytes, JSON depth 20 and 30,000 tree nodes, with at most 25
items. Duplicate/nonpositive/out-of-order IDs, unknown categorical enums, non-ACCEPTED statuses,
malformed counts, inconsistent first-page accounting, unknown fields and GraphQL `errors`
(including partial data with errors) refuse the whole response. There is no partial-success
bundle that silently drops refused items. HTTP failures preserve bounded status/Retry-After
metadata without retry. A returned nullable disease, rating, disease DOID or end cursor keeps
missing, explicit null and observed states distinct. Missing fields do not become zeros.

The bundle retains exact response bytes as canonical base64, their byte count/SHA256, the admitted
native JSON, the exact request body/digest, item-level JSON pointers, curation observations,
source identifiers, acquisition clock, and a one-request receipt. `validate_civic_evidence_bundle`
reconstructs the entire bundle from the bytes without network access; changing a projection and
restamping its digest cannot conceal disagreement. Self-computed hashes establish consistency,
not authenticated origin or scientific validity. A supplied acquisition clock is not an
independently authenticated source clock.

Only one page is requested. Native totalCount and pagination flags remain source assertions;
the cursor is retained as inert metadata and never followed. Completeness stays `unknown`, even
when the source reports no next page. Other statuses, other disease terms, later pages and
excluded fields remain explicit omissions with unknown influence. Release/snapshot identity and
deployed-schema parity remain unestablished.

Registration and profile compilation are inert:

```python
from prism_sdk import builtin_workflow_operation_catalog, with_workflow_capabilities
from prism_sdk import compile_workflow_capability

catalog = with_workflow_capabilities(
    builtin_workflow_operation_catalog(), ["civic-glioblastoma-evidence"])
plan = compile_workflow_capability(
    "Inspect the fixed CIViC glioblastoma-name metadata page", catalog,
    "civic-glioblastoma-evidence", enabled_profiles=["civic-glioblastoma-evidence"])
```

The fixed native plan has one step, one allowed source-operation call and an exact timeout
argument contract. Separate source approval is still required at execution. Direct callers may
use `register_civic_evidence_operations`, `retrieve_civic_glioblastoma_evidence` and
`validate_civic_evidence_bundle`. A caller fetcher receives `(fixed_url, *, http_config)` and must
return `CivicEvidenceHttpResponse`; its nonce binding and `caller_adapter`/`caller_supplied`
labels prevent it from inheriting default transport conformance. Callback timing is caller-owned,
and a late return is refused.

The original provider manifest `reviewed-public-readers/2026-10-27.3` counted 19 implemented reader paths in
16 declared API groups after fixture conformance. CIViC remains absent from the default catalogue.
These are code inventory units, not live availability, independent organizations or scientific
evidence counts. Earlier manifest `.2` and retained 18-path/15-group observations describe their
own source revisions; this increment does not rewrite or upgrade them. No live acquisition or
model inference is established by implementation or synthetic tests.

Manifest `.4` separately onboards the [Cellosaurus reader](CELLOSAURUS_CELL_LINE_METADATA.md)
and names twenty reader paths in seventeen declared API groups. This leaves the CIViC descriptor,
default catalogue and original `.3` acquisition/format evidence bound to their original revisions.

The optional [local curation metadata research review](GLIOMA_CIVIC_CURATION_WORKFLOW.md)
replays this bundle into item-level inspection and an explicit research handoff. It adds two
local operations and a three-step profile, without adding readers, following pages, joining
publications, or upgrading curation categories into scientific quality.
