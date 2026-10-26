# Reviewed research source integrations

`python/prism_sdk/reviewed_research_sources.py` supplies three concrete public HTTPS readers for
the executable workflow catalogue. Each reader performs one approved first-page query and returns
DOI or repository record identities, titles, partial dates, record types, citation URLs, provenance
digests, counts and explicit missingness. These are implemented transports and parsers, rather than
generated platform descriptors. Source availability and metadata quality remain separate questions.

This is a repository-defined SDK contract. It does not claim an unavailable blueprint module.

## Operations and request scope

All three operations accept the same closed argument object:

```json
{"query": "glioma", "limit": 5}
```

`query` defaults to `glioma` and permits 1–200 characters of plain topic text: letters, numbers,
spaces, underscores, periods, commas, apostrophes and hyphens. Empty or punctuation-only queries
are refused. `limit` is an integer from 1 to 25, default 5. The model cannot supply a URL, host,
header, credential, page number, cursor, file path, or authenticated access mode.

| Operation | Fixed endpoint | Constructed query | Retained identity |
| --- | --- | --- | --- |
| `openalex.search_works` | `https://api.openalex.org/works` | `search`, `page=1`, `per_page`, fixed `select` | OpenAlex work ID and optional DOI |
| `crossref.search_works` | `https://api.crossref.org/works` | `query.bibliographic`, `rows`, `offset=0`, fixed `select` | DOI |
| `zenodo.search_records` | `https://zenodo.org/api/records` | quoted literal `q`, `page=1`, `size` | Numeric record ID and optional version DOI |

OpenAlex currently permits anonymous basic queries; a key is needed for cursor continuation after
the first cursor page. This reader uses only basic page 1, projects an explicit field list and
accepts no key. See [OpenAlex authentication](https://help.openalex.org/api/authentication/),
[search](https://help.openalex.org/api/searching/),
[paging](https://help.openalex.org/api/paging/) and
[field selection](https://help.openalex.org/api/selecting-fields/). Its search semantics are
source-owned; a matching work is not a validated scientific claim.

Crossref exposes public metadata without registration. This reader selects only `DOI`, `title`,
`issued` and `type`. It sends no invented contact identity or paid-service credential. Crossref's
issued date is labelled `publication_date_source: issued`, and an unavailable `[[null]]` date
remains unknown. See [access and authentication](https://www.crossref.org/documentation/retrieve-metadata/rest-api/access-and-authentication/),
[field selection and small query sizes](https://www.crossref.org/documentation/retrieve-metadata/rest-api/tips-for-using-the-crossref-rest-api/)
and [date meanings](https://community.crossref.org/t/date-types-in-the-rest-api/15118).

Zenodo's records endpoint searches published repository metadata. The query is quoted as one
literal phrase and no repository file is requested or followed. Anonymous search pages have a
25-record ceiling and a 30-request-per-minute limit. See the
[records API](https://developers.zenodo.org/#records) and
[search rate-limit announcement](https://blog.zenodo.org/2025/11/25/2025-11-14-search-api-updates/).
Its deposition/upload API is a different surface and is not implemented here.

## Registration, execution and output

```python
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.reviewed_research_sources import register_research_source_operations

catalog = register_research_source_operations(WorkflowOperationCatalog())
result = catalog.execute(
    "crossref.search_works",
    {"query": "glioma", "limit": 2},
    approve_source_dispatch=True,
)
public_metadata = result.value
metadata_receipt = result.receipt
```

Registration and `prepare_research_source_query()` are network-free. Every execution requires
literal `True` approval before transport entry. Supplying `1`, a string, or an omitted approval
does not dispatch. Timestamps are whole-second UTC; a caller may provide an exact
`YYYY-MM-DDTHH:MM:SSZ` value, or allow the adapter to record the current UTC time.

The result value is one source bundle with `records`, `returned_count`,
`reported_total_count`, `total_count_relation`, `omitted_count`, `truncated`, `coverage`,
`source_uri`, `query_digest`, `response_digest`, `source_digest`, `bundle_digest`, retrieval time
and limitations. A record contains `record_id`, optional `doi`, `title`, `publication_date`,
`publication_year`, `publication_date_source`, `record_type`, `citation_url` and `missing_fields`.
The first Crossref title is selected; valid partial dates retain their original precision.

`None` means unknown. A measured empty result with an exact total of zero is complete for that
query, while a missing total remains unknown even when the first page is empty. Zenodo's
`{value, relation: gte}` total is a lower bound, so `omitted_count` remains unknown. A lower bound
larger than the returned count proves truncation; otherwise truncation remains unknown. Exact
totals determine an exact omitted count. Completion does not establish literature completeness,
independent validation, or research sufficiency.

Receipts retain counts, fixed source identity, time and digests. They omit query text, record
titles, DOI values, response bodies and headers. Source text is untrusted metadata and must be
escaped for display. DOI URLs and public record pages are citations, not instructions or implicit
authorization to fetch more content. Public source bundles remain caller-owned transient values
until an application explicitly stores them.

## Transport and refusal boundaries

The built-in transport uses HTTPS GET, fixed JSON acceptance and application identification, a
20-second socket timeout with an elapsed-time check between read chunks, and a 2,000,000-byte
response ceiling. It refuses redirects,
non-JSON responses, malformed or duplicate JSON fields, non-finite numbers, more than 100,000 tree
nodes, depth above 32, identity mismatches, duplicate record identities and over-limit result
arrays. No redirect, next-page link, abstract, author contact, repository file or participant row
is followed. There are no automatic HTTP retries or synthetic fallback records.

Built-in calls are serial per source. Process-local pacing spaces OpenAlex starts by at least
0.15 seconds, Crossref starts by 1.1 seconds and Zenodo starts by 2.1 seconds. Crossref's public
list limit is one request per second, with a one-request concurrency limit; its
[July 2026 update](https://community.crossref.org/t/refining-rest-api-limits-for-improved-stability-and-reliability/16137)
distinguishes list queries from single-record lookups. Cross-process coordination, service quota
changes and deployment scheduling remain application responsibilities. HTTP 429 is an explicit
rate-limited failure and does not become an empty evidence bundle.

For deterministic testing or a deployment-owned transport, `fetchers` maps source IDs
`openalex`, `crossref`, or `zenodo` to:

```python
def fetch(url, *, timeout_seconds, max_response_bytes):
    return ResearchSourceHttpResponse(
        status=200,
        body=b"...bounded source JSON...",
        final_url=url,
        content_type="application/json",
    )
```

Injected transports own their timeout, redirect implementation and rate coordination. The adapter
still verifies response status, exact final URL, content type, byte/tree bounds and source parser
contracts, and labels their receipts `caller_transport`. Registration does not validate source
reachability or prove a custom transport's identity. Callback errors are reduced to a bounded
failure class without retaining exception bodies or headers.

## Verification evidence

The fixture suite exercises all three real request constructors and parsers, direct/default
transport seams, approval denial, exact and lower-bound totals, unknown dates, identity/response
drift, size/tree limits and secret-bearing unused fields that must not enter output. It makes no
public network requests:

```text
python -m pytest tests/test_reviewed_research_sources.py -q
```

Approved one-record `glioma` checks on 2026-10-26 observed a record from each actual service:

| Source | UTC retrieval | Returned | Source-reported total | Coverage |
| --- | --- | ---: | ---: | --- |
| OpenAlex | 22:20:05Z | 1 | 490256 | partial |
| Crossref | 22:21:18Z | 1 | 54904 | partial |
| Zenodo | 22:20:06Z | 1 | 1140 | partial |

These are changing source observations, not test fixtures or independent evidence-quality scores.
The Crossref record carried an explicitly unknown issued date, which remains unknown in output.
Live verification establishes only that these bounded credentialless request/parser contracts
worked from this environment at those times. The integration set adds three services; it is not a
claim of hundreds of live integrations.
