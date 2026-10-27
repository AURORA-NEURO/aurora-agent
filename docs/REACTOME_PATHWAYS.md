# Reviewed Reactome pathway annotations

This repository-defined application extension has no detailed blueprint identifier. It adds one
opt-in reader path, `reactome.pathway_annotations`, through the shared reviewed public HTTPS
transport. It is not a bulk integration catalogue or an enrichment analysis.

The fixed GET is
`https://reactome.org/ContentService/data/mapping/UniProt/P04637/pathways?species=9606`.
The [official Content Service](https://reactome.org/dev/content-service/) and
[official client implementation](https://github.com/reactome/reactome2py/blob/main/reactome2py/content.py)
describe identifier-to-lower-level-pathway mapping. Source annotations describe human context;
they do not measure activity, alteration, enrichment, causal effect or glioma specificity.

`ReactomeRetrievalConfig` admits only P04637 and a 100–30,000 ms timeout. One approved request has
no retry, redirect, authentication, arbitrary URL, query, header or body surface. The shared transport
enforces public pinned DNS, hostname-verified TLS, framing/byte bounds and an absolute deadline.
The source parser additionally bounds response bytes, rows, tree depth/nodes, native keys, strings,
dates and identifiers. Malformed, nonhuman, duplicate or oversized native arrays refuse as a whole.

The bundle retains each native pathway object, stable identifier/version, display name, source
disease/inferred flags, exact source pointer, source clock, byte/payload digests and transport identity.
All derived fields replay from retained native inputs. An observed empty array is zero returned
annotations; missing acquisition and failed acquisition do not become empty success. The endpoint
does not supply a total or completeness guarantee in this contract. Hierarchy, relevance and
scientific identity remain unassessed.

An injected application callback receives `(fixed_url, *, http_config)` and must return the shared
`ReactomeHttpResponse` alias with matching byte/config/policy metadata. Its registration is
nonce-bound and labelled `caller_adapter`; its bundles are `caller_supplied`. The application owns
callback preemption, and the adapter rejects an elapsed deadline after callback return. Callback
exceptions are sanitized; typed shared HTTP failures retain bounded status/Retry-After metadata.
Replay and self-computed hashes establish internal consistency, not authenticated truth.

Registration is inert and leaves the eight default readers unchanged:

```python
from prism_sdk import WorkflowOperationCatalog, register_reactome_operations
catalog = register_reactome_operations(WorkflowOperationCatalog())
```

Offline tests use native-shape fixtures. The exploratory API-contract request observed 46 rows;
that request preceded the adapter and is not verification of this transport. A subsequent approved
fixed workflow on 2026-10-27 used one GET through this adapter and one UniProt GET. It retained
46 Reactome rows and 53 combined citations, paused after each source, resumed locally and replayed
the completed checkpoint without further dispatch. That bounded observation establishes neither
continuous availability, source completeness nor glioma pathway activity.
