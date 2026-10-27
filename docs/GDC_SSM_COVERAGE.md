# Reviewed GDC SSM-data coverage aggregates

`reviewed_gdc_ssm_coverage_retrieval.py` provides one opt-in credentialless public aggregate
reader. It retains source project populations and counts of cases with associated simple
somatic mutation (SSM) data, analyzing the fixed TCGA-GBM and TCGA-LGG project labels. These
availability observations do not establish a gene-specific assayed or callable denominator.
The existing GDC TP53 numerator reader and its schemas/transport identities are unchanged.

The primary [GDC Data Analysis API documentation](https://docs.gdc.cancer.gov/API/Users_Guide/Data_Analysis/#analysis-endpoints)
describes `/analysis/mutated_cases_count_by_project` and its nested
`case_summary.case_with_ssm.doc_count` values as associated-SSM case counts. Its documented
`size=0` query contains no individual hits. The
[GDC portal developer guide](https://docs.gdc.cancer.gov/Data_Portal/Users_Guide/Developers_Guide/#application-card-counts)
also distinguishes its SSM-available case count by a `cases.available_variation_data` filter.
Neither of those reviewed aggregate contracts establishes TP53 callability for every counted
case. No unsupported API availability, universal absence of denominator APIs, or equivalence
between availability and assay eligibility is claimed.

This is a repository-defined application extension. The available blueprint does not specify
this source-specific public aggregate contract; no new blueprint capability is claimed.

```python
from prism_sdk.reviewed_gdc_ssm_coverage_retrieval import (
    GdcSsmCoverageRetrievalConfig,
    retrieve_gdc_ssm_coverage,
    validate_gdc_ssm_coverage_bundle,
)

config = GdcSsmCoverageRetrievalConfig(timeout_ms=30_000)
bundle = retrieve_gdc_ssm_coverage(config, approve_source_dispatch=True)
checked = validate_gdc_ssm_coverage_bundle(bundle)  # Offline byte/native replay.
```

The installed exact GET route is:

```text
https://api.gdc.cancer.gov/analysis/mutated_cases_count_by_project?size=0
```

No other URL, filter, field selector, gene, case identifier, credential or pagination argument
is accepted. Configuration admits a typed integer timeout of 100–30,000 ms. Literal source
approval, timestamp and reconstructed configuration validation precede dispatch. The shared
[reviewed public HTTP transport](REVIEWED_PUBLIC_HTTP.md) enforces the public DNS/TLS,
absolute-deadline, no-redirect, strict-framing, identity-encoding and credentialless rules.
The one response has a 1,000,000-byte bound; JSON depth is 24, the node budget is 50,000,
and at most 256 source project buckets are admitted. There is no automatic retry.

The source shape admits only the reviewed aggregate fields. Individual `hits` must be empty,
the total relation exact, shards complete, and source terms error/omission counts explicit
integer zero. Duplicate JSON fields or project identities, malformed counts, extra fields,
nonfinite values, and unknown nested shapes refuse the entire acquisition. These source
accounting checks do not establish scientific completeness or reliable ascertainment.

Each project row retains three distinct quantities:

| Field | Native meaning retained |
| --- | --- |
| `project_doc_count` | Source project bucket `doc_count` |
| `case_summary_doc_count` | Nested source summary document count |
| `cases_with_ssm_data` | Source `case_with_ssm.doc_count`; associated SSM data |
| `gene_specific_assay_denominator` | Always null; not established by this contract |

The row state distinguishes `observed`, `project_bucket_missing`, `case_summary_missing`,
`ssm_bucket_missing` and `ssm_count_missing`. A present zero is observed; a missing field or
bucket remains null even when an ancestor count is zero. Explicit null/ill-typed count fields
refuse rather than being reinterpreted as a count or silently omitted. Source pointers refer
to exact native count locations when observed.

All bounded public project buckets remain in the retained native payload and exact UTF-8
response body. Projects outside the two fixed labels are unanalysed, with count/digest and
unknown influence recorded; they are not silently discarded or pooled into the glioma rows.
Raw response bytes/hash, canonical native payload hash, URL, timestamp, query, transport mode,
policy/config/binding digests and one-request receipt are retained. Replay parses the retained
response body again and rebuilds every derived field. A restamped outer digest cannot bless
changed projections, count states, receipt metadata or native pointers. Digests are integrity
checks, not proof of provider truth.

`fetcher=` is an application/fixture seam accepting the exact URL and typed `http_config` and
returning `GdcSsmCoverageHttpResponse` (the shared typed response). Such results are explicitly
`caller_supplied`, with a fresh binding nonce; the caller owns its transport behavior. The
adapter readmits typed response fields, checks exact route/status/config/policy/media and
elapsed deadline, sanitizes foreign failures, and never retries or substitutes an empty result.

`register_gdc_ssm_coverage_operations(catalog)` installs `gdc.ssm_project_coverage` under the
existing `gdc` integration identity. Registration makes no source calls and leaves the default
catalogue unchanged. This is an additional reader path, not an additional independent provider.
Workflow use is described in [denominator admissibility assessment](GLIOMA_DENOMINATOR_WORKFLOW.md).

Focused tests stay offline:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m pytest python/tests/test_reviewed_gdc_ssm_coverage_retrieval.py -q
```
