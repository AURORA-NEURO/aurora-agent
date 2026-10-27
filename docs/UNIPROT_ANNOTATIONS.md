# Reviewed UniProt TP53 name metadata

This repository-defined adapter adds one opt-in public service operation,
`uniprot.protein_annotation`. It requests the reviewed human entry with primary accession
`P04637`, projects name metadata, and retains a replayable research artifact. It has no available
blueprint module identifier. Registration, import, planning and replay perform no source requests.
The existing eight-source default catalogue and its digest are unchanged.

## Reviewed primary contract

Contract checked on 27 October 2026, using UniProt's own documentation rather than a research
entry lookup:

- The [UniProtKB OpenAPI specification](https://rest.uniprot.org/uniprotkb/api/docs) documents
  `GET /uniprotkb/{accession}`, its `fields` parameter and JSON entry schemas. The specification
  distinguishes recommended, alternative, submission and component protein names, and the four
  gene-name categories.
- [UniProtKB return fields](https://www.uniprot.org/help/return_fields), also available as
  [official help JSON](https://rest.uniprot.org/help/return_fields), and the
  [official field configuration](https://rest.uniprot.org/configure/uniprotkb/result-fields)
  identify the six selected fields below.
- [Individual-entry retrieval](https://www.uniprot.org/help/api_retrieve_entries) and
  [format and field selection](https://www.uniprot.org/help/api_queries) describe entry access
  and JSON field filtering. These help articles are also available through the official
  `rest.uniprot.org/help/` documentation endpoint.
- [Entry information](https://www.uniprot.org/help/entry_information_section) defines reviewed
  status; [accession documentation](https://www.uniprot.org/help/accession_numbers) distinguishes
  primary accessions from secondary accessions. The adapter requires the primary accession to
  remain exactly `P04637`.
- [Gene-name documentation](https://www.uniprot.org/help/gene_name) describes optional primary,
  synonym, ordered-locus and ORF names. A name can be missing, ambiguous or obsolete; the adapter
  does not infer an identifier-equivalence proof from it.

The installed route is exactly:

```text
https://rest.uniprot.org/uniprotkb/P04637?fields=accession,id,reviewed,protein_name,gene_names,organism_id&format=json
```

There is one GET, one response and no pagination, retry, sleep, search, batch retrieval or secondary
lookup. No application credentials are required. URLs, hostnames, accessions, query selectors and
headers are not model-controlled inputs.

## Public API

```python
from prism_sdk.reviewed_uniprot_retrieval import (
    UniProtRetrievalConfig,
    retrieve_uniprot_annotations,
    validate_uniprot_annotation_bundle,
    register_uniprot_operations,
)

config = UniProtRetrievalConfig(accession="P04637", timeout_ms=30_000)
# This explicit call authorizes one actual public request.
bundle = retrieve_uniprot_annotations(config, approve_source_dispatch=True)
checked_copy = validate_uniprot_annotation_bundle(bundle)  # no I/O

# Explicit opt-in; registration does not contact UniProt.
register_uniprot_operations(application_catalog)
```

The operation accepts only `timeout_ms`, an integer from 100 through 30,000; its default is 30,000.
The direct configuration also exposes the fixed accession, and refuses every other value. Only
literal `True` authorizes source dispatch. HTTP failures and malformed source data raise
`UniProtRetrievalError`; they never become a successful missing or empty observation. Sanitized
errors retain a status code and bounded numeric `Retry-After` delay when the shared transport
provides them, with `automatic_retry: false`.

An application-owned offline test callback may be installed explicitly:

```python
def fixture_fetcher(fixed_url, *, http_config):
    # Return UniProtHttpResponse, the ReviewedPublicHttpResponse public type.
    # Its body digest, config digest and policy digest must match the fixed contract.
    ...

register_uniprot_operations(application_catalog, fetcher=fixture_fetcher)
```

Injected callbacks own preemption. The source checks elapsed time and never retries. They are
labelled `caller_supplied` and `caller_adapter`, with an opaque per-registration nonce bound into
the catalogue descriptor, bundle and receipt. Replacing a callback registration changes the
catalogue digest and prevents silent checkpoint reuse. Direct calls with an injected callback
receive a fresh binding. Fixture bytes do not establish live service verification.

The built-in reader uses `ReviewedPublicHttpClient` with one exact installed route. Its public-DNS,
numeric-address pinning, TLS hostname verification, no-proxy/no-credential, strict framing, fixed
JSON media type and absolute transport deadline rules are shared with the other new readers. The
adapter binds the shared policy and configuration digests, checks response identity and byte
digests, and owns bounded JSON parsing and native projection.

## Artifact and exact coverage states

The bundle schema is `aurora-reviewed-uniprot-annotation/0.1`. Public keys are:

| Key | Retained information |
| --- | --- |
| `entry` | Requested and primary accession, optional entry name, taxon 9606, exact reviewed entry type and reviewed flag |
| `protein_names` | Every admitted name as `{category, value, source_pointer}` |
| `gene_name_candidates` | Every admitted gene name as `{gene_index, category, value, source_pointer}` |
| `field_observations` | Pointer, `not_supplied` / `empty_success` / `source_values`, and nullable immediate retained value count |
| `coverage` | Fixed requested/returned entry counts, nullable name/record counts, omission states and unassessed wider coverage |
| `source_inputs` | One fixed route, HTTP status, original byte count/digest, native whitelisted projection and projection digest |
| `receipts`, `transport` | Timestamp, request/response/projection digests, transport mode and binding, shared policy/configuration digests |
| `limitations`, `bundle_digest` | Explicit interpretation limits and canonical artifact integrity digest |

`primaryAccession == "P04637"`, `entryType == "UniProtKB reviewed (Swiss-Prot)"`, and the integer
`organism.taxonId == 9606` are admission gates. Source gene names are optional. Exact `TP53`, another
primary name, multiple gene records and differing aliases remain observations for later local
review. Case, whitespace, order and duplicate values are preserved; there is no alias collapse or
numeric/case coercion.

Native projections preserve source-present keys and array positions. Protein-name categories
include recommended full/short names; alternative full/short names; submission full names;
allergen, biotech, CD-antigen and INN names; and all admitted name categories within `includes`
and `contains` component sections. Gene categories are `geneName`, `synonyms`, `orderedLocusNames`
and `orfNames`, across all admitted gene rows. Every flattened value points exactly into the
retained projection, for example `/genes/0/geneName/value`. Component names remain component names.

An absent `genes` key means `gene_record_count` and `gene_name_count` are `null`, with
`gene_records_state: "not_supplied"`. A successful `genes: []` means measured counts of zero and
`empty_success`. A source gene row without a primary name retains that missing field explicitly in
the field observations. An absent protein description has a null name count; a successful empty
name projection has zero. Nested observation counts measure immediate retained object members or
array items, not biological prevalence or independently validated annotations.

Optional explicit nulls or malformed present name values are refused. A transport failure or
invalid response never becomes `not_supplied`, zero or `empty_success`. `reported_total_count`
is always `null`; `exhaustiveness` is `unassessed`; pagination is `not_applicable_single_entry`.
All these states concern the selected projection and fixed accession. They do not measure coverage
of proteins, glioma biology or all source annotations.

The whitelist deliberately omits evidence objects, EC numbers, validity flags, other organism
attributes, secondary accessions, audit/version fields, all unrequested annotations and unknown
fields. Original bytes are bounded and hashed, then discarded. The projection excludes sequences,
variants, features, structures, drug/laboratory actions and individual data. Unknown source fields
are still included in the initial JSON tree bound. No claim of zero scientific influence from
these omissions is made.

## Bounds, replay and remaining limits

- One response is limited to 1,000,000 bytes; JSON depth is at most 32 and nodes at most 50,000.
- At most 16 gene records, 64 items in each admitted name/component array, and 256 combined
  flattened protein/gene name rows and 2,048 field observations are retained. Each name is nonempty, contains no control
  characters, and is limited to 2,048 UTF-8 bytes. Exceeding a bound refuses the complete artifact;
  aliases are never truncated to fit.
- Duplicate JSON keys, nonfinite numbers, invalid Unicode, unexpected roots and type coercions are
  refused. Optional absent fields and successful empty arrays remain distinct.
- Replay defensively copies bounded canonical JSON, reprojects the exact retained native fields,
  reconstructs all rows/pointers/omission states/receipts/coverage, and checks the complete artifact.
  Re-stamping only the bundle digest cannot conceal inconsistent derived values.

Replay checks internal integrity. It cannot revalidate discarded HTTP bytes, independently
authenticate provider observations, detect consistently reauthored unkeyed artifacts or establish
scientific identity. The current-entry route does not pin a historical UniProt release. Source
text cannot authorize tool calls. No clinical inference, measurement/cohort join, treatment,
diagnosis, biological intervention or autonomous scientific sign-off is implemented.

## Verification

Focused offline tests exercise route and approval gates; every admitted name category and pointer;
candidate multiplicity; source omissions and empty results; schema/byte/tree/list/name bounds;
source and transport errors; rate-limit metadata without retries; deadlines; replay tampering;
stable default registrations; nonced callback identities; catalogue schema checking; and checkpoint
resume with no repeated source dispatch.

These fixtures are implementation evidence only. No live UniProt observation has been made for
this adapter at this documentation stage. Service-count claims require a separately recorded,
approved actual source execution; operation count and transport capacity are not service coverage.
