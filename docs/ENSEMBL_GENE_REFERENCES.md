# Reviewed Ensembl gene references

`python/prism_sdk/reviewed_ensembl_gene_retrieval.py` implements one credentialless public
annotation reader, `ensembl.gene_references`. Its initial reviewed input is exactly **TP53** in
the human Ensembl core gene plane. It adds an executable service adapter without changing the
default workflow catalogue or the existing molecular bundles. This is a repository-defined
extension; no detailed source blueprint identifier is available in this checkout.

The adapter makes exactly two serial GET requests after explicit source approval:

1. `https://rest.ensembl.org/lookup/symbol/homo_sapiens/TP53?expand=0`
2. `https://rest.ensembl.org/xrefs/id/{returned_gene_id}?external_db=EntrezGene&object_type=gene&species=homo_sapiens&all_levels=0`

The [official symbol lookup contract](https://rest.ensembl.org/documentation/info/symbol_lookup)
defines the species/symbol route and feature expansion. The reader requires an exact `TP53`
display name, `homo_sapiens`, `Gene`, `core`, an unversioned `ENSG` identifier followed by eleven
digits, and a positive integer feature version. It never enables expansion. That checked returned
identifier fixes the second route; model-supplied URLs, database names and identifiers are absent.

The [official reference contract](https://rest.ensembl.org/documentation/info/xref_id) defines
gene-level filtering by external database. Every returned row must identify `EntrezGene` and a
positive decimal string accession. Distinct accessions all remain candidates; duplicate candidate
accessions or an out-of-contract row refuse the entire bundle. The reader never selects a preferred
candidate or merges NCBI and Ensembl evidence planes.

```python
from prism_sdk.reviewed_ensembl_gene_retrieval import (
    EnsemblGeneRetrievalConfig,
    retrieve_ensembl_gene_references,
    validate_ensembl_gene_bundle,
)

bundle = retrieve_ensembl_gene_references(
    EnsemblGeneRetrievalConfig(gene_symbol="TP53", timeout_ms=30_000),
    approve_source_dispatch=True,
)
checked = validate_ensembl_gene_bundle(bundle)
```

The public configuration accepts only `TP53` and a total budget from 100 through 30,000
milliseconds. Constructing configuration, importing the module, inspecting a catalogue, and
validating a retained bundle make no network requests. `approve_source_dispatch=True` must be a
literal boolean; registration grants no dispatch authority.

## Retained values and replay

The bundle contains a normalized `gene`, `entrez_candidates`, explicit `coverage`, whitelisted
native `source_inputs`, metadata-only acquisition `receipts`, transport posture and limitations.
Each source input binds its exact request route, successful status, response byte count, raw HTTP
body SHA-256 digest, retained projection digest and retrieval clock. Retrieval clocks are whole
seconds in UTC. No raw HTTP headers or response body bytes are retained.

Gene fields are `requested_symbol`, `ensembl_gene_id`, `display_name`, `species`, `object_type`,
`db_type`, `version`, nullable `assembly_name`, `biotype`, `source` and `description`. Candidate
fields are `ensembl_gene_id`, `entrez_gene_id`, `display_id`, and nullable `version`, `info_type`
and `description`. Other native fields are omitted. Gene coordinates, transcripts, exons, sequences,
variants, individual records, clinical actions and model-generated biological interpretations are
not requested or projected. Source text remains untrusted; escape it when rendering.

`validate_ensembl_gene_bundle()` returns a defensive dictionary copy. It reconstructs normalization
from retained native projections and verifies every derived row, count, route, receipt, posture and
bundle digest. Restamping only a changed derived summary cannot bypass that replay. Original HTTP
bytes are absent, so their recorded digests cannot be independently recomputed from the projected
bundle. These unkeyed digests are not provider signatures; a consistently rewritten source bundle
is not authenticated. Scientific identity validation remains `source_assertions_only`.

An empty successful reference array is `candidate_state="empty_success"`, with one observed gene
and zero returned candidates. HTTP refusals and malformed or missing responses throw a typed
failure instead. The source reports no total or pagination guarantee for these responses;
`reported_total_count` is null, `pagination="not_documented"`, and `exhaustiveness="unassessed"`.
No complete gene-identity crosswalk or comprehensive glioma coverage is claimed.

## Bounded transport and failures

The default transport fixes `rest.ensembl.org`, resolves at most 64 addresses, refuses nonpublic or
multicast addresses, pins the selected address and verifies TLS against the original hostname. It
does not read proxy settings, discover credentials, follow redirects or send authorization headers.
Both requests share the configured total deadline. DNS waiting uses at most four outstanding
daemon resolver jobs; a late DNS result cannot open an HTTP connection. Socket connect, TLS,
request send, each header/body/framing receive and normalization check the remaining budget.
Separate 65,536-byte header and 65,536-byte framing bounds apply to each response, alongside a
wire-byte bound equal to those two budgets plus the response-body budget. Timed-out DNS jobs
retain their original resolver slot until completion.

The default reader requires CRLF header lines and rejects duplicate or malformed headers,
ambiguous Content-Length/Transfer-Encoding, unsupported transfer or content encodings, and
declared Content-Length values that exceed the budget or differ from the received body. Chunked
responses require HTTP/1.1, bounded hexadecimal chunk sizes, exact CRLF delimiters and the final
empty trailer line. Chunk extensions, trailers and redirect headers are refused. Close-delimited
responses remain supported within the same receive deadline and byte limits.

Each response body is at most 1,000,000 bytes, the pair at most 2,000,000 bytes. JSON permits at
most depth 32 and 50,000 visited nodes per response. Duplicate keys, nonfinite numbers, unexpected
media types and malformed source structures refuse retrieval. At most 64 reference rows are
admitted; an over-bound array refuses rather than truncating or implying a unique candidate.

The [official response-code guide](https://github.com/Ensembl/ensembl-rest/wiki/HTTP-Response-Codes)
distinguishes an exceptional or missing-ID `400`, malformed-route `404`, rate-limit `429` and
unavailable-service `503`. All non-200 statuses remain failures. The
[official rate-limit guide](https://github.com/Ensembl/ensembl-rest/wiki/Rate-Limits) describes
rate headers and fractional-second `Retry-After`. A `429` retains only a finite numeric
`retry_after_seconds` from zero through 86,400; malformed, oversized or date-valued headers become
unknown. No automatic sleep or retry follows. Historical documented quotas do not grant a rate
reservation; repeated workflow scheduling must account for current provider limits.

`EnsemblGeneRetrievalError.to_dict()` contains only a failure class, optional HTTP status,
bounded retry delay and `automatic_retry=False`. Upstream payloads, raw headers, DNS answers and
application exception details are withheld.

Applications/tests may inject `fetcher(url, *, timeout_seconds, max_response_bytes)` returning
`EnsemblGeneHttpResponse(status, body, final_url, headers)`. An injected fetcher owns actual network
admission and preemption; this adapter detects late returns and prevents a second request after
deadline expiry. It cannot terminate arbitrary caller code. Injected results carry
`transport.mode="caller_supplied"` and never claim the default transport's deadline authority.

## Explicit workflow registration

```python
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations

catalog = register_ensembl_gene_operations(WorkflowOperationCatalog())
result = catalog.execute(
    "ensembl.gene_references", {"timeout_ms": 30_000},
    approve_source_dispatch=True,
)
```

The operation has a closed timeout-only input and a typed output schema. Its descriptor binds
the adapter and transport contract. Normal default registrations have stable catalogue identity;
each injected-callback registration receives an independent opaque session binding. Replacing a
callback registration therefore requires a new plan; a checkpoint cannot silently resume through
another injected executor. Capacity, fixtures and registered descriptors do not establish live
service count or availability. No `live_verified` flag is inferred from transport selection.
The stricter framing contract uses deterministic transport policy
`pinned_https_total_deadline/0.2` and a new binding digest. Existing default-policy registrations
and bundles from the previous binding do not silently acquire the new policy. Public bundle
schema and field names are unchanged.

## Observed verification

An approved bounded default-transport run on **2026-10-27 at 03:22:04 UTC** completed the two
reviewed requests and retained 644 response-body bytes. Ensembl returned TP53 feature
`ENSG00000141510` and one EntrezGene candidate, `7157`. Validator replay passed without another
request. This is one observed request pair, not a continuous availability measurement, an
independent scientific crosswalk validation or evidence of many connected services.
That initial observation used the earlier transport policy. The framing-policy implementation
was checked with offline wire fixtures. A subsequent bounded fixed gene-reference workflow used
the hardened reader for its two Ensembl requests plus one NCBI request, retained three citations,
resumed local stages and replayed its completed checkpoint without further dispatch. Neither
observation establishes a scientific crosswalk or continuous availability.

Focused local verification uses deterministic fixtures and mocked transport boundaries:

```powershell
$env:PYTHONPATH = 'python'
python -W error::ResourceWarning -m pytest python/tests/test_reviewed_ensembl_gene_retrieval.py -q
```

The tests cover scope/identity admission, candidate preservation, empty versus failed results,
response bounds, total-deadline behavior, public DNS/TLS routing, valid chunked/length/close
framing, truncated declared bodies, malformed chunks and encodings, aggregate framing bounds,
source replay, registration identity and checkpoint reuse. Actual fixture TLS bytes pass through
the HTTP parser; no live request is made. They are implementation checks, not independent
biological benchmarks or live service observations.
