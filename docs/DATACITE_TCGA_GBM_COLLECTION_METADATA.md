# Fixed DataCite TCGA-GBM DOI metadata

This repository-defined reader has no available blueprint module identifier. Explicit SDK
registration adds `datacite.tcga_gbm_collection_metadata`, a read-only DataCite metadata
operation for DOI `10.7937/K9/TCIA.2016.RNYFUYE9`. Its service group is `datacite`. It does not
execute a TCIA imaging reader. The optional `datacite-tcga-gbm-metadata` capability profile
installs this one reader. Manifest `.5` now names twenty-one implemented reader paths in
eighteen declared API groups; default registration remains eight readers in seven groups,
with thirteen optional readers absent by default. Earlier `.4` twenty/seventeen observations
retain their original source binding. The core-only `c5e87` revision preceded this onboarding.
Descriptor `live_adapter`
labels describe implementation paths, not observed execution against a live endpoint.

The design uses official [TCIA API guidance](https://www.cancerimagingarchive.net/tcia-api-guides/),
the [TCGA-GBM collection page](https://www.cancerimagingarchive.net/collection/tcga-gbm/),
[DataCite public REST API documentation](https://support.datacite.org/docs/api), the
[singleton DOI example](https://support.datacite.org/docs/api-get-doi), and its
[reference route](https://support.datacite.org/reference/get_dois-id). The example is not a
complete schema guarantee. No actual TCGA-GBM DataCite response has been captured or admitted
by this reader in this revision. Tests use synthetic inputs and fixture TLS bytes only;
they do not establish live shape conformance, source availability or scientific validity.

## Fixed authority and local admission

The only input is optional exact integer `timeout_ms` from 100 through 30,000. Literal
source approval permits one anonymous GET to
`https://api.datacite.org/dois/10.7937/K9/TCIA.2016.RNYFUYE9`, capped at 524,288 response bytes.
The URL, DOI, host and empty query are installed authority. There is no caller endpoint,
enrichment/affiliation/publisher/include/history selector, authentication, redirect, retry,
fallback or link traversal. The existing reviewed HTTPS policy is unchanged. Only this
reader's configuration admits exact `application/json` and `application/vnd.api+json` media;
the actual admitted media type is retained separately from raw response bytes.

The explicit local policy requires one JSON:API `data` object with native `id`, exact type
`dois`, and `attributes.doi`. Both identity strings must correspond to the fixed DOI under
ASCII A–Z equivalence only, as specified by the [DOI Handbook](https://www.doi.org/doi-handbook/html/)
section 3.3.4. Exact native strings remain unchanged. Whitespace trimming, DOI-URL unwrapping,
Unicode casefolding/normalization and title-based disease or cohort matching are absent.
The [DataCite display guidance](https://support.datacite.org/docs/datacite-doi-display-guidelines)
also distinguishes DOI-name case insensitivity from case-sensitive URLs.

Root `links`, `meta`, `jsonapi` and resource `relationships`, `links`, `meta` may be bounded
objects or null and remain inert. Unknown envelope/resource keys, `errors`, `included`, a
list-valued `data`, unsupported selected shapes and excess bounds refuse the entire bundle.
All native attributes are retained, including unprojected attributes; their keys are listed
explicitly and their semantics remain unassessed.

The thirteen optional projected attributes are `titles`, `publisher`, `publicationYear`,
`types`, `version`, `metadataVersion`, `schemaVersion`, `state`, `isActive`, `created`,
`registered`, `updated`, and `url`. Every field retains missing, null or observed state;
admitted empty strings/objects/arrays remain observed input states. A missing field has no
native value pointer and cites its containing attributes object as absence context. Null
cites the actual native pointer/value. These optional shapes are local synthetic admission
policy, not observations of this DOI's actual metadata.

Publisher is either a complete bounded native string or object. Objects retain unknown keys
and nested canonical JSON types; the reader does not flatten them into `name` or assert
complete publisher-schema validation. Ordered title objects retain extra native keys. Titles
have at most 64 entries, each requiring a nonempty title string up to 32,768 UTF-8 bytes.
Publisher strings have the same byte cap. `types` remains an opaque bounded object.
`publicationYear` is a local exact-integer range 0–9999; `metadataVersion` is 0–2^63−1;
booleans and numeric-looking strings are not coerced to integers. `isActive` accepts native
booleans. State/date strings have 128-byte caps; version/schema/landing-URL strings 4,096.
Empty permitted strings are retained rather than inferred to be missing or invalid metadata.

Dataset/resource `version`, record-revision `metadataVersion`, and schema identifier
`schemaVersion` have separate named observations. Neither the schema URL nor the record
revision is a dataset release. Dates are retained strings without freshness arithmetic.
Inactive and nonfindable states remain explicit observations in the returned record;
they do not become an empty success, an absence claim or a scientific-quality decision.

## Raw replay, bounds and limits

The bundle retains complete raw bytes/base64/SHA-256, parsed native payload and digest,
typed transport metadata, fixed request, source clock, one native record, receipt, unknown
attribute keys, observations and citations. Citation pointers resolve relative to
`artifact_pointer_root=validated_datacite_bundle` under `/source_inputs/0/native_payload`.
Each citation binds its exact native value/digest, source URI, raw-response digest,
native DOI and retained source clock. There are at most eighteen field/context citations
in this version, with a defensive cap of 64; no citation or record prefix is returned.

`validate_datacite_collection_metadata_bundle` decodes the retained canonical base64,
reparses the raw body, rebuilds the full deterministic bundle from original admission inputs
and compares canonical values. Restamping an outer digest cannot hide projection, version,
citation, missingness, scope, omission or receipt changes. The retained content type, source
clock and transport mode/binding are declarations, not raw-body-authenticated evidence.
A coherent whole-source replacement can still be supplied; hashes and replay establish
local consistency rather than source truth, human provenance or independent scientific review.
Replay does not request a source or fall through to live acquisition.

Default reviewed transport has a fixed binding and built-in deadline. Caller-supplied
callbacks have a distinct nonce binding and caller-owned timing: callback completion cannot
be forcibly bounded by this adapter. Retained-byte callbacks remain caller provenance,
regardless of where their original bytes came from. An application retaining an unsuccessful
typed response must separately save its raw bytes and failure metadata; this reader returns
no partially admitted bundle after refusal.

Local tree bounds are depth 20, 30,000 JSON nodes and 256 UTF-8 bytes per key. Duplicate JSON
keys, nonfinite numbers, invalid UTF-8 and forbidden secret fields refuse. The complete
canonical bundle is capped at 8,000,000 bytes. Whole-artifact refusal is permitted when any
bound is exceeded; no claim is made that every otherwise source-valid payload fits. Synthetic
maximum-raw and lowered-policy boundary cases test retention/refusal rather than live API
limits. A shorter body is not evidence that the metadata record is scientifically complete.

Returned count one means one DataCite object, not one dataset, cohort, independent study,
publication, patient, subject, image or series denominator. Other DOI/collection records,
resource versions/history, current releases, metadata semantics and omitted influence remain
unknown. Native URLs, subject keywords, content/media links and related identifiers remain
inert; no subject/image/media access, source-link following, clinical or laboratory action is
authorized. Free-text retention cannot prove semantic absence of incidental context.

```python
from prism_sdk import (
    WorkflowOperationCatalog, register_datacite_collection_metadata_operations,
    validate_datacite_collection_metadata_bundle,
)

catalog = register_datacite_collection_metadata_operations(WorkflowOperationCatalog())
# Registration dispatches nothing. Execution has its separate source approval boundary.
```

The optional profile compiles one `collection_metadata` step with exactly
`{"timeout_ms": 30000}` and its native bundle as output. Its immutable requirements bind the
one operation, whole argument object, required output, one-step and one-call caps. Editing
the fixed timeout refuses rather than weakening the contract. CLI `workflow-template
--template datacite-tcga-gbm-metadata` and worker startup `--capability
datacite-tcga-gbm-metadata` use the same operator-selected profile; a request cannot enable it.
Task text labels this fixed source inspection and does not choose another DOI or endpoint.
