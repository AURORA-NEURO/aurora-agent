# Findings

Measured results from `bioprism context compare`, including the ones that do not favour FIBER.
Every number here is asserted by a test — in
[`crates/baseline/tests/equal_engineering.rs`](../crates/baseline/tests/equal_engineering.rs),
[`crates/baseline/tests/embedding_retrieval.rs`](../crates/baseline/tests/embedding_retrieval.rs),
[`crates/baseline/tests/directed_walk.rs`](../crates/baseline/tests/directed_walk.rs),
[`crates/baseline/tests/sweep_grid.rs`](../crates/baseline/tests/sweep_grid.rs) and
[`crates/worldgen/tests/structural_families.rs`](../crates/worldgen/tests/structural_families.rs) —
so it fails loudly if it stops being true.

**Summary.** On the world the distribution ships, FIBER has no measurable advantage over a
correctly tuned graph walk, a lexical retriever, or a directed dependency walk: all four select
the identical eleven facts. That is partly a property of the benchmark, not of the methods. On a
world built to vary structure independently (43.39), the graph and retrieval families separate
cleanly and fail — but the **directed dependency walk does not**: it ties FIBER exactly, on that
world and in all 36 cells of the structural family sweep (§6). Within the swept families, FIBER's
selection behaviour is matched by an ordinary backward walk over directed factor edges; what the
walk does not reproduce is the temporal cut, the policy screen and the certificate, none of which
these worlds' verdicts exercise.

---

## 1. On the reference world, FIBER's compression is matched exactly by three baselines

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
| embedding-top-11 (hashed 3-gram) | 11 | 1.45% | **no** | 91% | **no** |
| embedding-top-50 (hashed 3-gram) | 50 | 6.57% | yes | 100% | yes |
| **directed-walk-full** | **11** | **1.45%** | yes | 100% | **yes** |
| **fiber** | **11** | **1.45%** | yes | 100% | **yes** |

*Sound* = the strategy's selection, fed to the same deterministic oracle, reproduces the
full-context verdict with the same four leakage witnesses. *Closure* = fraction of the query's
protected facts retained. *Admissible* = both.

`graph-5-hop`, `lexical-top-11` and `directed-walk-full` select **exactly the same eleven facts**
as FIBER — not the same count, the identical set. The cheapest admissible strategy on this world
is the graph walk, not the compiler. The fixed-basis embedding retriever is the one compact
strategy that *fails* here: at k=11 it drops `fact.label_source` (the temporal-leakage witness's
carrier) for a distractor and loses a witness, recovering only by widening to k=50.

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
| embedding-top-11 (hashed 3-gram) | 11 | 1.44% | **no** | **36%** | **no** |
| embedding-top-50 (hashed 3-gram) | 50 | 6.56% | **no** | **55%** | **no** |
| **directed-walk-full** | **11** | **1.44%** | **yes** | **100%** | **yes** |
| **fiber** | **11** | **1.44%** | **yes** | **100%** | **yes** |

Each strategy family fails in its own way — except one, which does not fail at all:

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

**The embedding retriever fails harder than its lexical proxy.** Camouflaged tags are the
protected vocabulary plus a suffix, so they share most of their character trigrams with the query
— exactly the similarity a hashed-trigram basis rewards. Closure falls to 36% at k=11 and is
still incomplete and unsound at k=50, where BM25 held 91%.

**The directed dependency walk does not fail.** Walking the *directed* factor edges backward from
the target — instead of the undirected incidence projection — never enters a distractor factor,
because distractors only consume decisive variables and a backward step never enters a consumer.
It selects **exactly FIBER's eleven facts**, identical set, full closure, admissible. The world
built to discriminate FIBER from adjacency and lexical similarity does not discriminate it from
directed dependency; see §6.

**FIBER is admissible at 11 facts with full closure — tied, not alone.** The compact admissible
set on this world is exactly {`fiber`, `directed-walk-full`}.

The lexical failure mode is why the comparison harness ranks on *admissibility* rather than on
verdict alone. Ranking on verdict would have crowned `lexical-top-11` here, a strategy that
violated the contract and got away with it.

## 4. What this does and does not establish

It establishes that the graph, lexical and embedding families are separable from FIBER, and that
under a structure where adjacency and character-level similarity both mislead, a dependency slice
with a mandatory closure satisfies the contract where they do not.

It does **not** establish that FIBER wins generally, and the two baselines this section once
recorded as missing now exist and sharpen that caveat considerably. The embedding retriever
(`crates/baseline/src/embedding.rs` — a fixed-basis hashed-trigram model, and it says so) turned
out *weaker* than its BM25 proxy on both worlds. But the directed dependency walk
(`crates/baseline/src/directed.rs`) — the fair competitor this section predicted "would recover
much of what backward slicing does" — recovers **all** of it on every world measured: the
identical fact set FIBER compiles, on the reference world, the discriminating world, and all 36
cells of the structural family sweep (§6). Within the swept families, FIBER's selection advantage
over a competently directed baseline is **zero**. What remains FIBER-only is outside these
worlds' verdicts: the temporal cut, the policy screen, and the certificate stating what was
omitted and whether it could have mattered.

The sweep the earlier version of this section called for now exists —
`crates/baseline/src/sweep.rs`, measured in §6 — over attachment × relay depth × tag style ×
distractor count. Structural knobs it does not vary (skeleton, events, protected set, decision
time, policy) change what the decision *is*, and a world family that varies them comparably —
in particular one whose decisive evidence extends *beyond* the protected closure — is the
experiment that could still separate FIBER from the directed walk. On every swept world the
closure alone is already decision-sufficient, which overdetermines the tie (§6).

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

## 6. The structural family sweep (measured 2026-08-23)

The sweep §4 called for, run over the full default grid: attachment {Hub, NearTarget} ×
relay depth {0, 2, 4} × tag style {Distinct, Camouflaged} × distractors {50, 250, 750} — 36
cells, seed 20260823, one generated world and query per cell, the full panel of 11 strategies per
world. `crates/baseline/src/sweep.rs`; deterministic (same grid + seed ⇒ byte-identical table,
asserted). The admissible-cell counts below are asserted cell by cell in
[`crates/baseline/tests/sweep_grid.rs`](../crates/baseline/tests/sweep_grid.rs); the mean-facts
column and the closure and cost ranges quoted in the prose are *reproduced*, not pinned — the
ignored test `print_the_full_default_grid_markdown_for_the_findings_document` in the same file
reprints the complete 36-cell table this section was transcribed from.

| Strategy | Admissible cells | Mean facts when admissible |
|---|---:|---:|
| full-context | 36 / 36 | 362.0 |
| graph-4-hop | 0 / 36 | — |
| graph-5-hop | 12 / 36 | 186.0 |
| graph-6-hop | 12 / 36 | 186.0 |
| graph-7-hop | 12 / 36 | 361.0 |
| lexical-top-11 | 12 / 36 | 11.0 |
| lexical-top-50 | 18 / 36 | 12.0 |
| embedding-top-11 | 18 / 36 | 11.0 |
| embedding-top-50 | 24 / 36 | 50.0 |
| **directed-walk-full** | **36 / 36** | **11.0** |
| **fiber** | **36 / 36** | **11.0** |

What the sweep actually shows, per strategy, stated exactly:

- **fiber** is admissible in all 36 cells, always at 11 facts. No structural corner in this
  family defeats the compiler.
- **directed-walk-full is admissible in all 36 cells, always at exactly FIBER's fact count.**
  This is the headline negative result for FIBER: across the entire swept family, admissibility
  and cost cannot distinguish the compiler from a plain backward walk over directed factor edges
  with the mandatory closure taken first. The tie is overdetermined here — on every swept world
  the protected closure alone already carries all four decisive witnesses, so even a depth-0 walk
  is admissible (`crates/baseline/tests/directed_walk.rs` pins this). A family whose decisive
  evidence extends beyond the closure is needed before this comparison can say anything more.
- **graph-5/6/7-hop** (undirected) are admissible in exactly the 12 relay-free cells and in no
  cell with a relay chain: admissibility of the undirected walk is precisely the `relay_depth = 0`
  knob. Where admissible with NearTarget attachment it is admissible at near-full-world cost
  (61–761 facts); the compact-and-sound corner (11 facts) exists only at Hub attachment with no
  relays — the reference world's exact shape. graph-4-hop is admissible nowhere.
- **lexical-top-11** is admissible in 12 of 36 cells: distinct tags *and* ≥ 250 distractors. Two
  distinct failure modes bound it: camouflaged tags hold it at ≤ 91% closure at every measured
  budget, and small distinct corpora (50 distractors) break it too — with only 62 documents, IDF
  no longer isolates the protected tags and it goes unsound at 91% closure. The reference-world
  tie in §1 therefore also depends on the corpus being large.
- **lexical-top-50** is admissible in the 18 distinct-tag cells and in no camouflaged cell.
- **embedding-top-11** is admissible in the 18 distinct-tag cells and in no camouflaged cell
  (closure 45–55% there). On generated distinct worlds it is admissible at 11 facts where
  lexical-top-11 sometimes is not — the two proxies fail in different corners, which is exactly
  why both are in the panel.
- **embedding-top-50** adds the six camouflaged-50-distractor cells (24 / 36 total); at 250+
  camouflaged distractors it is unsound at every measured budget (closure 55–64%).
- **full-context** is admissible everywhere at full cost, as constructed.

No cell produced an oracle refusal; every one of the 396 rows was judged.

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

The full sweep table of §6:

```bash
cargo test -p bioprism-baseline --offline --test sweep_grid -- --ignored --nocapture
```
