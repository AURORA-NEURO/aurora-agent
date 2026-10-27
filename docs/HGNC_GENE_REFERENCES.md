# Reviewed HGNC gene references

`python/prism_sdk/reviewed_hgnc_gene_retrieval.py` supplies one public metadata reader,
`hgnc.gene_references`. The reviewed selector is exactly the human gene symbol **TP53**.
This is a repository-defined extension: the available blueprint does not specify this
source contract. Importing, registering or validating the reader does not acquire evidence.

The [primary HGNC REST documentation](https://www.genenames.org/help/rest/) defines the
`fetch/symbol/<term>` route, JSON negotiation, stored nomenclature/reference fields and a
recommendation of at most ten requests per second. A fetch can return multiple documents;
the reader retains them instead of choosing one. The sole reviewed request is:

```text
GET https://rest.genenames.org/fetch/symbol/TP53
Accept: application/json
```

```python
from prism_sdk.reviewed_hgnc_gene_retrieval import (
    HgncGeneRetrievalConfig,
    retrieve_hgnc_gene_references,
    validate_hgnc_gene_reference_bundle,
)

bundle = retrieve_hgnc_gene_references(
    HgncGeneRetrievalConfig(gene_symbol="TP53", timeout_ms=30_000),
    approve_source_dispatch=True,
)
checked = validate_hgnc_gene_reference_bundle(bundle)
```

Approval must be a literal boolean. Timeout must be an integer from 100 through 30,000 ms;
other symbols, URLs and selectors are refused. The registered operation accepts only the
optional `timeout_ms` argument. It uses the existing unchanged reviewed public HTTPS
transport: fixed hostname/route, public-address admission, pinned address with hostname TLS
verification, total deadline, bounded headers/framing/body, no redirects, proxies,
credentials or automatic retries. The response body is limited to 512,000 bytes. JSON has
a depth bound of 20 and 30,000 visited nodes; duplicate keys and nonfinite numbers fail.
At most eight documents are admitted; a larger response is refused in full.

The default path has a process-local admission gate: a start less than 100 ms after the
previous admitted start is refused, without sleeping or retrying. This is not a quota
reservation or cross-process coordinator. Caller-injected fetchers own network admission
and preemption, receive bounded timeout/body arguments, and are labeled `caller_supplied`.
Late returns are refused. Injected registration has a session binding; it cannot silently
replace the transport used by an existing checkpoint.

## Native evidence and missingness

The bundle retains the exact UTF-8 response text and its raw-byte SHA-256 digest, parsed
native payload and projection digest, source URL, byte count, whole-second UTC acquisition
clock, receipt, transport binding and deterministic bundle digest. JSON replay reconstructs
every derived field from those retained response bytes. Recomputing a digest over a changed
summary cannot bypass replay. Unkeyed digests establish internal consistency, not provider
authentication or scientific truth.

Each admitted document retains `hgnc_id`, `symbol`, `name`, `status`, `alias_symbol`,
`alias_name`, `prev_symbol`, `prev_name`, `entrez_id`, `ensembl_gene_id` and
`symbol_report_tag` as explicit field observations with native pointers. Missing, explicit
null, observed empty and observed nonempty values differ. Aliases and previous names keep
their array order and duplicates. No alias, numeric conversion or case folding supplies
an identifier. Other admitted stored metadata remains native and uninterpreted; annotation
links remain inert. Unknown fields, including out-of-contract sensitive or sequence
content, refuse the response.

The native `responseHeader` and `response` container are checked. `numFound`, `start`,
optional `numFoundExact` and all returned documents remain visible. The source selection
state distinguishes no records, multiple records, partial windows, explicitly approximate
totals, selector mismatch, missing identity/status, withdrawn status and unreviewed status.
`source_identity_ready` describes a sole approved source record only. When `numFoundExact`
is absent, this field can be true while count exactness remains unknown; it is not a
scientific identity or exhaustiveness claim.

`unreturned_reported_count` is arithmetic over the source-reported total and returned
document count. Its influence remains `unknown` when documents are unreturned or total
exactness is absent/false. No paging or search expansion follows. The reader records no
discarded admitted document; this does not establish that the source exposed every relevant
gene, alias or scientific assertion. Source release identity and scientific exhaustiveness
remain unassessed.

## Registration and verification scope

```python
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from prism_sdk.reviewed_hgnc_gene_retrieval import register_hgnc_gene_operations

catalog = register_hgnc_gene_operations(WorkflowOperationCatalog())
bundle = catalog.execute(
    "hgnc.gene_references", {"timeout_ms": 30_000},
    approve_source_dispatch=True,
)
```

The bundle/receipt schemas are `aurora-reviewed-hgnc-gene-references/0.1` and
`aurora-reviewed-hgnc-gene-receipt/0.1`. Offline tests cover source states, exact-byte replay,
restamped tampering, admission/budget refusal and callback binding. Fixtures are not a live
availability observation. No live HGNC acquisition has been asserted by this document.
Nomenclature/reference metadata does not establish glioma relevance, biological equivalence,
assay compatibility, clinical guidance or permission to join measurements. No sequences,
raw variants, individual records, controlled-access material or laboratory actions are
requested. See [the three-source composition](GLIOMA_GENE_IDENTITY_WORKFLOW.md).
