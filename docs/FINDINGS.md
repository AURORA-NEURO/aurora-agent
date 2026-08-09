# Findings

Measured results from `bioprism context compare`, including the ones that do not favour FIBER.
Every number here is asserted by a test — in
[`crates/baseline/tests/equal_engineering.rs`](../crates/baseline/tests/equal_engineering.rs) and
[`crates/worldgen/tests/structural_families.rs`](../crates/worldgen/tests/structural_families.rs) —
so it fails loudly if it stops being true.

**Summary.** On the world the distribution ships, FIBER has no measurable advantage over a
correctly tuned graph walk or a lexical retriever: all three select the identical eleven facts.
That is a property of the benchmark, not of the methods. On a world built to vary structure
independently (43.39), the three separate cleanly and FIBER is the only admissible strategy.

---

## 1. On the reference world, FIBER's compression is matched exactly by two baselines

World `radiogenomic-integrity-demo-v1`, 761 facts. Full report:
[`BASELINE_COMPARISON.md`](BASELINE_COMPARISON.md).

| Strategy | Facts | % of world | Sound? | Closure | Admissible |
|---|---:|---:|:-:|---:|:-:|
| full-context | 761 | 100.00% | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | **no** | 0% | **no** |
| **graph-5-hop** | **11** | **1.45%** | yes | 100% | **yes** |
| graph-6-hop | 11 | 1.45% | yes | 100% | yes |
| graph-7-hop | 761 | 100.00% | yes | 100% | yes |
| hypergraph-component | 761 | 100.00% | yes | 100% | yes |
| **lexical-top-11 (BM25)** | **11** | **1.45%** | yes | 100% | **yes** |
| **fiber** | **11** | **1.45%** | yes | 100% | **yes** |

*Sound* = the strategy's selection, fed to the same deterministic oracle, reproduces the
full-context verdict with the same four leakage witnesses. *Closure* = fraction of the query's
protected facts retained. *Admissible* = both.

`graph-5-hop` and `lexical-top-11` select **exactly the same eleven facts** as FIBER — not the
same count, the identical set. The cheapest admissible strategy on this world is the graph walk,
not the compiler.

## 2. The distribution's own baseline script is a strawman

`reference/fiber_runtime/compare_baselines.py` reports the graph baseline at **depth 7 and
unbounded only** — the two settings where it returns all 761 facts. It never measures depths 5 or
6, where the same code returns 11.

The published comparison therefore shows a 69× advantage that disappears entirely under equal
tuning. Blueprint 43.38 requires "matched, equal-engineering comparisons"; 43.41 requires that
"if graph baselines remain compact under equal optimization, report that result".

## 3. A world that discriminates

The reference world sits at one corner of the structural space: distractors attached to a hub leaf,
no relay chain, and tags that name the answer. [`crates/worldgen`](../crates/worldgen) makes those
three properties parameters, holding the decisive skeleton and the oracle fixed.

Moving two knobs — distractors attached *near the target* instead of at the hub, decisive facts
placed behind a 3-step relay chain, and distractor tags camouflaged to tokenise into the protected
vocabulary (`identity_summary` rather than `exploratory`) — produces this. Full report:
[`DISCRIMINATING_COMPARISON.md`](DISCRIMINATING_COMPARISON.md).

| Strategy | Facts | % of world | Sound? | Closure | Admissible |
|---|---:|---:|:-:|---:|:-:|
| full-context | 762 | 100.00% | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | **no** | 0% | **no** |
| graph-5-hop | 750 | 98.43% | **no** | 0% | **no** |
| graph-6-hop | 750 | 98.43% | **no** | 0% | **no** |
| graph-7-hop | 750 | 98.43% | **no** | 0% | **no** |
| hypergraph-component | 761 | 99.87% | yes | 100% | yes |
| lexical-top-11 (BM25) | 11 | 1.44% | yes | **91%** | **no** |
| lexical-top-50 (BM25) | 50 | 6.56% | yes | **91%** | **no** |
| **fiber** | **11** | **1.44%** | **yes** | **100%** | **yes** |

Three things changed, and each is a distinct failure mode:

**The graph walk has no usable depth at all.** Depths 5–10 pull in all 750 distractors — 98% of the
world — *and still miss every decisive witness*. Depth 11 is the first sound setting, and by then it
has taken everything. The sound-and-compact window is empty, where on the reference world it was
`{5, 6}`. Near-total context with the wrong answer is the worst quadrant to land in.

**Lexical retrieval fails in the most dangerous way available.** BM25 still reaches the *correct
verdict* at k=11 — but from a 91% protected closure. It dropped a protected fact that happened not
to participate in any witness. It was right by luck. Raising the budget to k=50 does not recover
it: the dropped fact stays below rank 50. Under 43.13 the closure is mandatory *before* any
relevance step, precisely so a strategy cannot be credited for guessing correctly from an
incomplete basis.

**FIBER is the only admissible strategy**, at 11 facts with full closure.

That third failure mode is why the comparison harness ranks on *admissibility* rather than on
verdict alone. Ranking on verdict would have crowned `lexical-top-11` here, a strategy that
violated the contract and got away with it.

## 4. What this does and does not establish

It establishes that the three strategies are separable, and that under a structure where adjacency
and lexical similarity both mislead, a dependency slice with a mandatory closure is the only one of
the three that satisfies the contract.

It does **not** establish that FIBER wins generally. The discriminating world was built to expose
exactly these failure modes, in the same way the reference world was built to expose hub expansion.
Both are single points. A claim about the method needs the full family swept — attachment × relay
depth × tag style × distractor count — with the result reported wherever it lands. The knobs exist
now; the sweep does not.

Two further baselines are missing and would be fair competitors: an embedding retriever (the BM25
implementation is a lexical proxy and says so), and a graph walk over the *directed* dependency
edges rather than undirected incidence, which would recover much of what backward slicing does.

## 5. Defects found in the v0.6 distribution

1. **`machine/module_registry.jsonl` omits FIBER entirely.** 935 rows, **zero** from section 43,
   while `context_cards.jsonl` and `doc_graph.json` carry all 51 FIBER modules (994 rows each).
   `machine/README.md` claims one row per markdown module. An agent routing off the registry never
   sees the canonical runtime.
2. **Only 131 of 935 registered modules are `Build-Ready Specification`; 400 are `Planned`.**
   Sections 01–19 and 23–29 are 0% build-ready, including `03_CORE_SPECIFICATIONS` (the PRISM
   IRs), `05_EXECUTION_RUNTIME`, all 50 files of `23_AGENT_INTERWEAVE_FABRIC` and all 24 of
   `25_BIOLOGICAL_IR_AND_LANGUAGE`.
3. **The reference runtime hard-codes a radiogenomic goal string** into every Decision Section,
   regardless of query.
4. **The reference oracle compares label timestamps lexicographically as strings**, not as parsed
   instants. It agrees with instant ordering for the zero-offset `...Z` form used in the packs and
   silently disagrees under mixed offsets or differing sub-second precision.

Items 3 and 4 are reproduced exactly for byte parity and flagged at their call sites.

## Reproducing

```bash
cargo run -p bioprism-cli --offline -- context compare --world fixtures/fiber-v0.1/radiogenomic_world.json --query fixtures/fiber-v0.1/leakage_query.json --markdown
```

```bash
cargo run -p bioprism-cli --offline -- world generate --family discriminating --world-out /tmp/w.json --query-out /tmp/q.json
```

```bash
cargo run -p bioprism-cli --offline -- context compare --world /tmp/w.json --query /tmp/q.json --markdown
```
