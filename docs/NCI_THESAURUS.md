# Reviewed NCI Thesaurus glioma ontology

This repository-defined application extension has no configured detailed blueprint module.
It implements one opt-in public reader, `nci_thesaurus.glioma_ontology`, for NCI EVS terminology
metadata. Ontology assertions do not establish cohort prevalence, molecular measurements,
assay coverage, causal effects, diagnosis, treatment evidence or WHO classification equivalence.

The fixed route is:

```text
https://api-evsrest.nci.nih.gov/api/v1/concept/ncit/C3059?include=definitions,children,parents,roles,associations&limit=100
```

The [official EVS OpenAPI contract](https://api-evsrest.nci.nih.gov/v3/api-docs), reviewed on
2026-10-27 as API `2.5.0.RELEASE`, documents `getConcept`, its `include` parameter and a per-part
`limit` from 1 to 100. The [official include documentation](https://github.com/NCIEVS/evsrestapi-client-SDK/blob/main/doc/INCLUDE.md)
describes the selected concept parts and the minimal metadata always returned. The native
`Concept.name` is the preferred name, while `Definition` and `Role`/`Association` carry their
source-specific fields and qualifiers. These field descriptions define the reader's contract;
synthetic test fixtures are not captured NCI responses or independent scientific validation.

The root must return exact code `C3059`, preferred name `Glioma`, terminology `ncit` and a
nonempty source version. The official `Concept` schema does not require every nested metadata
field: direct children and parents can be compact records containing code, preferred name and
optional flags. If related terminology/version is explicitly present, it must match `ncit` and
the root version. Absent and null related metadata remain distinct `not_reported` observations
with unknown values; they never inherit root metadata. Explicit values are `source_reported`.
The reader preserves source strings and native type/code/source/group/qualifier records. Child
and parent edges retain their API field labels; it does not invent an `is_a` label or perform
transitive expansion. Related concepts are not fetched individually. Definition source and
attribution fields are retained when present; absent attribution is not assigned to NCI.

`NciThesaurusRetrievalConfig` admits only the fixed code and a 100–30,000 ms timeout. Each approved
invocation makes one request without retries, arbitrary queries, URLs, headers, credentials or
request bodies. It uses the unchanged shared reviewed public HTTPS transport: public pinned DNS,
hostname-verified TLS, bounded framing/headers/body and a total deadline. The reader additionally
bounds the response to 1,000,000 bytes, each requested part to 100 records, qualifier arrays to 32,
definition text to 16,384 UTF-8 bytes, and JSON depth/node counts.

The documented source limit signal is an extra record with a `ct` field. Every such marker
refuses the entire response. Unknown fields, paging envelopes, duplicate JSON keys, nonfinite
numbers, malformed lists, duplicate concept identities, version disagreement and contradictory
parent/child directions also refuse. No partial-success bundle silently drops inadmissible rows.
Absence of a marker does not independently establish exhaustive taxonomy coverage.

The bundle preserves exact raw UTF-8 response bytes as canonical base64, their byte count and
SHA256, the full admitted JSON payload, payload digest, derived records with JSON pointers,
source version, caller-supplied acquisition clock, transport identity and a one-request receipt.
`validate_nci_thesaurus_bundle` reparses those retained bytes and reconstructs the whole artifact,
so restamping a modified projection or parsed payload cannot conceal disagreement with the bytes.
Self-computed hashes establish consistency, not authenticated origin or scientific truth.

Each requested part reports one of:

| Native observation | State | Returned count |
|---|---|---|
| Field absent | `not_reported`, presence `absent` | unknown (`null`) |
| Field explicitly null | `not_reported`, presence `null` | unknown (`null`) |
| Explicit empty array | `empty_success` | observed zero |
| Admitted native records | `source_records` | observed array length |

Unrequested synonyms, properties, descendants, history, inverse relations, mappings and paths are
listed as omissions. Each omitted part carries an unknown omitted count (`null`) and influence
`unknown`; omission does not establish zero influence or decision sufficiency.
Counts concern returned ontology records; they are not patient, cohort or
assay denominators. The current `ncit` endpoint is not release-pinned. Its returned version is
preserved, and cross-acquisition compatibility remains unestablished.

Registration is inert. Applications explicitly opt into the operation:

```python
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.reviewed_nci_thesaurus_retrieval import register_nci_thesaurus_operations

catalog = register_nci_thesaurus_operations(WorkflowOperationCatalog())
```

The input schema exposes only `timeout_ms`; the code, include list and 100-record limit are fixed.
An application callback receives `(fixed_url, *, http_config)` and returns the shared
`NciThesaurusHttpResponse` alias with matching byte/config/policy metadata. Its registration uses
a nonce and `caller_adapter` label; returned bundles say `caller_supplied`. The application owns
callback preemption; the reader also refuses a callback that returns after its deadline. Shared
typed HTTP failures retain bounded status and Retry-After metadata. Other callback failures are
sanitized. Neither a callback nor default registration is evidence of live availability.

The initial default GET on 2026-10-27 returned an HTTP200 body (5,650 bytes) with root version
`26.09d`, but the original reader refused its compact child/parent records because it incorrectly
required their terminology/version fields. That failed attempt and its original bytes remain
preserved. Bundle/receipt schema `0.2` admits the documented compact shape, records the missing
related metadata explicitly, and changes the default transport binding to prevent old plans
silently adopting the changed admission policy. Offline reprocessing of those original bytes is
labelled separately and does not turn the original failed acquisition into a success.

A separate bounded default GET with schema `0.2` subsequently passed on 2026-10-27. It retained
two definitions and 50 direct relationship records (19 children, two parents, 13 roles and
16 associations), with root version `26.09d`. The 21 compact child/parent records kept their
unreported related terminology/version unknown. Exact raw bytes, native pointers and the
completed checkpoint were checked offline; completed replay made zero additional source calls.
This is one observed public response, not continuous availability, exhaustive taxonomy coverage
or scientific validation. Neither acquisition made a model call.
No patient, sequence, raw variant, controlled-access,
laboratory or clinical action surface is implemented.
