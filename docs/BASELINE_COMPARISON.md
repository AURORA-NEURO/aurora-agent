# Equal-engineering context comparison: radiogenomic-integrity-demo-v1

world `radiogenomic-integrity-demo-v1`, query `audit-split-integrity-v1`, 761 facts total

Reference verdict (full-context): **invalid** with witnesses identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

| Strategy | Facts | % of world | Verdict | Sound? | Closure | Admissible |
|---|---:|---:|---|:-:|---:|:-:|
| full-context | 761 | 100.00% | invalid | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | valid | **no** | 0% | **no** |
| graph-5-hop | 11 | 1.45% | invalid | yes | 100% | yes |
| graph-6-hop | 11 | 1.45% | invalid | yes | 100% | yes |
| graph-7-hop | 761 | 100.00% | invalid | yes | 100% | yes |
| hypergraph-component | 761 | 100.00% | invalid | yes | 100% | yes |
| query-graph | 0 | 0.00% | valid | **no** | 0% | **no** |
| lexical-top-11 | 11 | 1.45% | invalid | yes | 100% | yes |
| lexical-top-50 | 50 | 6.57% | invalid | yes | 100% | yes |
| fiber | 11 | 1.45% | invalid | yes | 100% | yes |

Cheapest admissible strategy (right verdict **and** full protected closure): **graph-5-hop** at 11 facts (1.45% of world).

- `graph-4-hop` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `query-graph` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

## Methods

- **full-context** — expose every fact in the world
  - upper bound on decisive-evidence recall by construction
- **graph-4-hop** — breadth-first walk of the undirected factor/variable incidence graph from the query targets, to depth 4
  - undirected incidence projection; edges carry no direction, so hubs expand
- **graph-5-hop** — breadth-first walk of the undirected factor/variable incidence graph from the query targets, to depth 5
  - undirected incidence projection; edges carry no direction, so hubs expand
- **graph-6-hop** — breadth-first walk of the undirected factor/variable incidence graph from the query targets, to depth 6
  - undirected incidence projection; edges carry no direction, so hubs expand
- **graph-7-hop** — breadth-first walk of the undirected factor/variable incidence graph from the query targets, to depth 7
  - undirected incidence projection; edges carry no direction, so hubs expand
- **hypergraph-component** — unbounded breadth-first walk of the incidence graph from the query targets
  - no depth limit; returns the entire connected component
- **query-graph** — facts feeding any factor incident to a query target variable
  - one factor hop, undirected, restricted to factors touching a target
- **lexical-top-11** — BM25 (k1=1.2, b=0.75) over fact id, provided variable, tags and serialised value; top 11 by score, ties broken by fact id. A lexical proxy for embedding retrieval, not a neural model.
  - 761 facts scored above zero; lexical proxy, not an embedding model
- **lexical-top-50** — BM25 (k1=1.2, b=0.75) over fact id, provided variable, tags and serialised value; top 50 by score, ties broken by fact id. A lexical proxy for embedding retrieval, not a neural model.
  - 761 facts scored above zero; lexical proxy, not an embedding model
- **fiber** — protected closure, then backward dependency slice, then temporal cut

Facts exposed is a cost, not a score. It ranks only among verdict-preserving strategies. This world is constructed to expose hub expansion; it demonstrates compiler mechanics, not universal superiority.
