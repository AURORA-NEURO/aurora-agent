# Reviewed GDC gene mutation counts

`python/prism_sdk/reviewed_gdc_gene_mutation_retrieval.py` reads one public GDC aggregate query for
the fixed TP53 Ensembl ID `ENSG00000141510` and retains the TCGA-GBM and TCGA-LGG project counts.
This is a repository-defined research reader. It extends executable source coverage beyond
catalogue availability without expanding the existing GDC project or cBioPortal catalogue readers.
It supplies no individual records, clinical action, laboratory action, or preclinical-engine input.

The primary [GDC Data Analysis documentation](https://docs.gdc.cancer.gov/API/Users_Guide/Data_Analysis/#analysis-endpoints)
describes `GET /analysis/top_cases_counts_by_genes` as returning mutation-affected case counts by
requested gene and project. Its example contains an Elasticsearch-style aggregate response with
empty individual hits and nested project/gene count buckets. The same document identifies TP53 as
`ENSG00000141510`. This reader pins that one gene and exact endpoint/query; it adds no arbitrary
filters, fields, format, mutation occurrence, expression, survival, denominator, or record endpoint.

```python
from prism_sdk.reviewed_gdc_gene_mutation_retrieval import (
    ReviewedGdcGeneMutationRetrievalAdapter,
    ReviewedGdcGeneMutationRetrievalConfig,
    validate_reviewed_gdc_gene_mutation_bundle,
)

adapter = ReviewedGdcGeneMutationRetrievalAdapter(
    ReviewedGdcGeneMutationRetrievalConfig(timeout_ms=30_000)
)
plan = adapter.prepare()                 # No DNS lookup or source request.
result = adapter.execute(plan, approve_source_dispatch=True)
bundle = validate_reviewed_gdc_gene_mutation_bundle(result.bundle)
receipt = result.receipt
```

`ReviewedGdcGeneMutationRetrievalConfig` and `ReviewedGdcGeneMutationRetrievalPlan` are frozen,
canonical-digest-bound values with exact `to_dict()`/`from_dict()` round trips. Configuration
contains only the bounded timeout and declared transport identity; gene, retained projects, URL,
request limit, retention, and credential policy are fixed. Execution validates the plan, current
config, literal approval, and optional UTC timestamp before any transport call. A failed request
or parse raises `ReviewedGdcGeneMutationRetrievalError`; there is no retry or successful empty
fallback. Injected fixture/application transports require a distinct identity and own their stated
timeout, DNS, and redirect policy.

The built-in transport makes one credentialless HTTPS GET to:

```text
https://api.gdc.cancer.gov/analysis/top_cases_counts_by_genes?gene_ids=ENSG00000141510
```

It uses a 100–30,000 ms deadline, resolves at most 64 public IP answers, and connects directly to
the first checked address while authenticating the fixed hostname through TLS. DNS resolution is
bounded separately within the same deadline, with no connection permitted after expiry. The
transport follows no redirects, reads no proxy credentials, and requests no compressed response.
Non-200 responses, redirects, content-type/encoding drift, and inconsistent declared lengths fail.
Bytes are bounded before parsing to 1,000,000; JSON depth is bounded to 32, nodes to 50,000, and
returned project buckets to 128. Duplicate JSON fields, invalid Unicode, non-finite values,
unexpected fields, individual hits, incomplete shards, timeouts, approximate totals, and nonzero
aggregation error/omission bounds all fail closed. No request is made to discover missing metadata.

The returned bundle contains two project rows in fixed GBM/LGG order:

| Field | Meaning |
| --- | --- |
| `source_id`, `project_id`, `lane`, `gene_id`, `gene_symbol` | Exact source and retained selector identity |
| `state` | `observed`, `project_bucket_missing`, or `gene_bucket_missing` |
| `case_count_with_mutation` | Source gene-bucket count; null when the requested project/gene bucket is absent |
| `project_doc_count`, `nested_genes_doc_count`, `selected_genes_doc_count` | Distinct native aggregate fields, retained for review and consistency checks |
| `source_json_pointer` | Exact native count location for an observed row; null for a missing count |

An explicit zero is `observed` with count zero. A missing bucket remains null even when an ancestor
aggregate has a zero count; absence is not imputed as a measurement. Native nested-document counts
are not substituted for case counts. `observed_count` counts present requested count buckets;
`completeness` is `requested_counts_present` only when both are present. It is not a scientific
completeness claim.

The source query returns other projects too. They are structurally validated, then omitted from
retained records with an explicit bucket count and canonical digest. Their count values are not
pooled with glioma projects. The bundle retains aggregate status metadata, exact source URL,
timestamp, response/projection/source-set digests, and limitations. The acquisition receipt binds
the reviewed plan/config to the bundle and records one request, response bytes, source counts,
transport class, and its own digest. `result.to_dict()` is receipt-only; source values require
the explicit `.bundle` or `.to_transient_dict()` accessors. Returned dictionaries are defensive
copies.

`validate_reviewed_gdc_gene_mutation_bundle` replays the retained typed projection, count/state
relationships, source pointers, boundaries, and artifact digests. It returns a defensive normalized
dictionary. Omitted projects remain digest-only and cannot be reconstructed by that validator.
Self-computed digests provide internal consistency, not source authentication or independent
verification. Counts remain source observations under source annotation/release semantics.

No gene-specific assay denominator or prevalence is calculated. Cross-project counts are not
pooled; cohort overlap and independence remain unassessed. No Entrez/Ensembl crosswalk is inferred.
Scientific support remains false, human review remains required, and count availability cannot
authorize a mechanism, diagnosis, prognosis, treatment, or laboratory recommendation.

The optional `register_gdc_gene_mutation_operations(catalog)` function registers the executable
`gdc.gene_mutation_counts` operation with `integration_id="gdc"`, a detailed nullable output schema,
and a sole `timeout_ms` argument. It uses the existing GDC provider identity; it is not an additional
independent integration or evidence source. Registration makes no requests and does not change
`builtin_workflow_operation_catalog()`. Workflow composition and source-dispatch approval remain
caller-owned.

One separately approved live check on 2026-10-26 completed through this reader in a single request
of 14,364 bytes. It retained source TP53 mutation-affected case counts of 156 for TCGA-GBM and 247
for TCGA-LGG, with 58 other project buckets explicitly omitted. Both requested counts were present;
no denominator, rate, cross-source independence, or scientific conclusion was established. These
numbers describe that retrieval, are not a prevalence estimate, and may change with source updates.

Focused tests run offline with standard-library fixtures and mocked transport boundaries:

```powershell
$env:PYTHONPATH = (Join-Path (Get-Location) 'python')
python -m unittest discover -s python/tests -p test_reviewed_gdc_gene_mutation_retrieval.py -v
```
