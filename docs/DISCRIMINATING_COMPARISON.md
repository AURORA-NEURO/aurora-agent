# Equal-engineering context comparisons on the generated worlds

Three reports from `Comparison::to_markdown()`.

- **§1** — `generated-discriminating-v1`, the world built to separate FIBER from adjacency and from
  lexical similarity. Panel: `bioprism_baseline::default_panel`. Verbatim, unchanged since it was
  first recorded, and the source of the table in [`FINDINGS.md`](FINDINGS.md) §3.
- **§2** — `generated-external-confirmation-v1` and **§3** — `generated-policy-restricted-v1`, the
  two worlds that sit in the region where `fiber` and `directed-walk-full` can differ at all.
  Panel: `bioprism_baseline::extended_panel`, which adds the three counter-baselines carrying
  FIBER's own passes. Recorded 2026-08-26; the source of [`FINDINGS.md`](FINDINGS.md) §7.4.

Every table cell in §2 and §3 is transcribed from that output unaltered. Two things are condensed
rather than reproduced: the per-strategy "not sound" bullets, where several strategies miss the
identical witness list and are listed together, and the Methods block, where only the three added
rows are restated because every other method line is identical to §1's. The command below prints
the uncondensed form of both.

§1 uses the narrower panel deliberately. Its numbers are pinned in three places and transcribed
into `FINDINGS.md` §3, and rewriting them in the same change that introduced the counter-baselines
would have left no before-and-after to check. Read §1 against §2 and §3 by row name: the extended
panel is a strict superset of the default one in the same order.

Reproduce §2 and §3 with:

```bash
cargo test -p bioprism-baseline --offline --test divergence_region -- --ignored --nocapture
```

---

## 1. generated-discriminating-v1 — neither pass fires

world `generated-discriminating-v1`, query `generated-discriminating-v1-split-integrity`, 762 facts total

Reference verdict (full-context): **invalid** with witnesses identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

| Strategy | Facts | % of world | Verdict | Sound? | Closure | Admissible |
|---|---:|---:|---|:-:|---:|:-:|
| full-context | 762 | 100.00% | invalid | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | valid | **no** | 0% | **no** |
| graph-5-hop | 750 | 98.43% | valid | **no** | 0% | **no** |
| graph-6-hop | 750 | 98.43% | valid | **no** | 0% | **no** |
| graph-7-hop | 750 | 98.43% | valid | **no** | 0% | **no** |
| hypergraph-component | 761 | 99.87% | invalid | yes | 100% | yes |
| query-graph | 0 | 0.00% | valid | **no** | 0% | **no** |
| lexical-top-11 | 11 | 1.44% | invalid | yes | 91% | **no** |
| lexical-top-50 | 50 | 6.56% | invalid | yes | 91% | **no** |
| embedding-top-11 | 11 | 1.44% | invalid | **no** | 36% | **no** |
| embedding-top-50 | 50 | 6.56% | invalid | **no** | 55% | **no** |
| directed-walk-full | 11 | 1.44% | invalid | yes | 100% | yes |
| fiber | 11 | 1.44% | invalid | yes | 100% | yes |

Cheapest admissible strategy (right verdict **and** full protected closure): **directed-walk-full** at 11 facts (1.44% of world).

- `lexical-top-11` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `lexical-top-50` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `graph-4-hop` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `graph-5-hop` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `graph-6-hop` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `graph-7-hop` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `query-graph` is **not sound**: missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `embedding-top-11` is **not sound**: missing identity_leakage, site_leakage, temporal_leakage

- `embedding-top-50` is **not sound**: missing identity_leakage, site_leakage, temporal_leakage

### Methods

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
  - 762 facts scored above zero; lexical proxy, not an embedding model
- **lexical-top-50** — BM25 (k1=1.2, b=0.75) over fact id, provided variable, tags and serialised value; top 50 by score, ties broken by fact id. A lexical proxy for embedding retrieval, not a neural model.
  - 762 facts scored above zero; lexical proxy, not an embedding model
- **embedding-top-11** — hashed character-3-gram embedding (FNV-1a into 512 fixed buckets) over fact id, provided variable, tags and serialised value; cosine similarity against the query's targets and protected tags; top 11 by score, ties broken by fact id. A fixed-basis lexical embedding, not a learned or neural model.
  - 762 facts scored above zero; fixed-basis lexical embedding, not a learned model
- **embedding-top-50** — hashed character-3-gram embedding (FNV-1a into 512 fixed buckets) over fact id, provided variable, tags and serialised value; cosine similarity against the query's targets and protected tags; top 50 by score, ties broken by fact id. A fixed-basis lexical embedding, not a learned or neural model.
  - 762 facts scored above zero; fixed-basis lexical embedding, not a learned model
- **directed-walk-full** — protected closure first (mandatory, as 43.13 orders it), then a walk of the directed factor graph backward from the query targets — needed variable to the factors that output it, to their input variables, transitively — unbounded (the full backward slice); facts providing any needed variable are selected
  - protected closure contributed 11 fact(s), the backward slice 11 (of which 0 beyond the closure); edges are directed, so factors that only consume a hub are never entered
- **fiber** — protected closure, then backward dependency slice, then temporal cut

Facts exposed is a cost, not a score. It ranks only among verdict-preserving strategies. This world is constructed to expose hub expansion; it demonstrates compiler mechanics, not universal superiority.

---

## 2. generated-external-confirmation-v1 — the temporal cut fires

`central_lab_confirmation` is an input to `factor.confirmation_check`, which feeds the target, and
the event releasing it becomes available months after the decision time. FIBER drops the fact that
provides it; a walk with no cut cannot. `local_lab_value` is the control — identically tagged,
identically event-managed, released *before* the cut — so the exclusion is demonstrably about the
release schedule rather than the tag vocabulary.

world `generated-external-confirmation-v1`, query `generated-external-confirmation-v1-split-integrity`, 764 facts total

Reference verdict (full-context): **invalid** with witnesses identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

| Strategy | Facts | % of world | Verdict | Sound? | Closure | Admissible |
|---|---:|---:|---|:-:|---:|:-:|
| full-context | 764 | 100.00% | invalid | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | valid | **no** | 0% | **no** |
| graph-5-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| graph-6-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| graph-7-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| hypergraph-component | 763 | 99.87% | invalid | yes | 100% | yes |
| query-graph | 0 | 0.00% | valid | **no** | 0% | **no** |
| lexical-top-11 | 11 | 1.44% | invalid | yes | 91% | **no** |
| lexical-top-50 | 50 | 6.54% | invalid | yes | 91% | **no** |
| embedding-top-11 | 11 | 1.44% | invalid | **no** | 36% | **no** |
| embedding-top-50 | 50 | 6.54% | invalid | **no** | 55% | **no** |
| directed-walk-full | 13 | 1.70% | invalid | yes | 100% | yes |
| directed-walk-cut | 12 | 1.57% | invalid | yes | 100% | yes |
| directed-walk-screened | 13 | 1.70% | invalid | yes | 100% | yes |
| directed-walk-compiled | 12 | 1.57% | invalid | yes | 100% | yes |
| fiber | 12 | 1.57% | invalid | yes | 100% | yes |

Cheapest admissible strategy (right verdict **and** full protected closure): **directed-walk-cut** at 12 facts (1.57% of world).

- `lexical-top-11` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `lexical-top-50` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `graph-4-hop`, `graph-5-hop`, `graph-6-hop`, `graph-7-hop` and `query-graph` are **not sound**: each missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `embedding-top-11` and `embedding-top-50` are **not sound**: each missing identity_leakage, site_leakage, temporal_leakage

### What the new rows say

`directed-walk-full` selects thirteen facts and is admissible. FIBER selects twelve — the same set
minus `fact.central_lab` — and is admissible. That single fact is the entire measured difference,
and it is the fact the temporal cut exists to remove.

`directed-walk-cut` is the same walk carrying `bioprism_fiber::temporal_cut`. It selects twelve:
**the identical set FIBER compiles**. `directed-walk-screened` selects thirteen, because this world
declares no policy requirement for a screen to act on — the two counter-baselines separate cleanly,
which is how a reader can tell each pass is doing its own work rather than both being one effect.

The cheapest admissible strategy on this world is `directed-walk-cut`, not `fiber`.

---

## 3. generated-policy-restricted-v1 — both passes fire

The same world, plus `local_lab_value` requiring the clause `consent-tier-2`. The corpus grants it
and the query accepts only `research-only`, so the screen withholds `fact.local_lab` and names it.
The variable is deliberately the one released *before* the cut, so the policy exclusion and the
temporal exclusion land on different facts and each can be read on its own.

world `generated-policy-restricted-v1`, query `generated-policy-restricted-v1-split-integrity`, 764 facts total

Reference verdict (full-context): **invalid** with witnesses identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

| Strategy | Facts | % of world | Verdict | Sound? | Closure | Admissible |
|---|---:|---:|---|:-:|---:|:-:|
| full-context | 764 | 100.00% | invalid | yes | 100% | yes |
| graph-4-hop | 0 | 0.00% | valid | **no** | 0% | **no** |
| graph-5-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| graph-6-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| graph-7-hop | 750 | 98.17% | valid | **no** | 0% | **no** |
| hypergraph-component | 763 | 99.87% | invalid | yes | 100% | yes |
| query-graph | 0 | 0.00% | valid | **no** | 0% | **no** |
| lexical-top-11 | 11 | 1.44% | invalid | yes | 91% | **no** |
| lexical-top-50 | 50 | 6.54% | invalid | yes | 91% | **no** |
| embedding-top-11 | 11 | 1.44% | invalid | **no** | 36% | **no** |
| embedding-top-50 | 50 | 6.54% | invalid | **no** | 55% | **no** |
| directed-walk-full | 13 | 1.70% | invalid | yes | 100% | yes |
| directed-walk-cut | 12 | 1.57% | invalid | yes | 100% | yes |
| directed-walk-screened | 12 | 1.57% | invalid | yes | 100% | yes |
| directed-walk-compiled | 11 | 1.44% | invalid | yes | 100% | yes |
| fiber | 11 | 1.44% | invalid | yes | 100% | yes |

Cheapest admissible strategy (right verdict **and** full protected closure): **directed-walk-compiled** at 11 facts (1.44% of world).

- `lexical-top-11` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `lexical-top-50` reached the correct verdict from an **incomplete protected closure** (91%). Under 43.13 the closure is mandatory before any relevance step, so this is a contract violation that guessed right, not a pass.

- `graph-4-hop`, `graph-5-hop`, `graph-6-hop`, `graph-7-hop` and `query-graph` are **not sound**: each missing identity_leakage, preprocessing_leakage, site_leakage, temporal_leakage

- `embedding-top-11` and `embedding-top-50` are **not sound**: each missing identity_leakage, site_leakage, temporal_leakage

### What the new rows say

The walk family reads as a ladder, one rung per pass: **13** facts with neither, **12** with either
one, **11** with both — and 11 is FIBER. `directed-walk-cut` drops `fact.central_lab`,
`directed-walk-screened` drops `fact.local_lab`, `directed-walk-compiled` drops both and lands on
the identical set FIBER compiles.

The cheapest admissible strategy on this world is `directed-walk-compiled`, not `fiber`.

### Methods for the added rows

- **directed-walk-cut** — protected closure first (mandatory, as 43.13 orders it), then the unbounded backward walk of the directed factor graph, then the temporal cut of 43.09 — the compiler's own pass code, run over the walk's selection; no certificate is produced
- **directed-walk-screened** — protected closure first (mandatory, as 43.13 orders it), then the unbounded backward walk of the directed factor graph, then the policy screen of 43.33 — the compiler's own pass code, run over the walk's selection; no certificate is produced
- **directed-walk-compiled** — protected closure first (mandatory, as 43.13 orders it), then the unbounded backward walk of the directed factor graph, then the policy screen of 43.33 and the temporal cut of 43.09 — the compiler's own pass code, run over the walk's selection; no certificate is produced

Every other row's method statement is identical to §1's and is not repeated.

---

## 4. Reading §2 and §3 together

**FIBER separates from the naive walk, and does not separate from an equally-engineered one.** On
both worlds FIBER selects strictly fewer facts than `directed-walk-full`, and on both worlds it
selects *exactly* what a walk carrying the same passes selects. The measured gap is the temporal
cut and the policy screen. It is not the compiler.

**Nothing separates on admissibility.** All five walk-family rows and FIBER reach the reference
verdict with a complete protected closure on both worlds: the withheld facts carry none of the four
leakage witnesses, so excluding them costs no soundness and keeping them costs no correctness. Cost
is the only column that moves here, and cost ranks only among admissible strategies. These worlds
show the passes have an effect; they do not show the effect changes an answer.

**A world where it would change an answer has not been built.** It needs decisive evidence — a fact
carrying a leakage witness — on the far side of a cut or a clause. `WorldSpec::protecting` turns a
temporal withholding into a closure violation and `crates/worldgen/tests/generator_knobs.rs`
exercises that, but no shipped spec places a *witness-carrying* fact behind either exclusion.

**What no row in any of these tables can see.** `directed-walk-compiled` discards the same facts
FIBER discards and says nothing about having done so. FIBER emits a Context Certificate naming each
omission, its influence class, and the refinement that would recover it — `POLICY_REFINEMENT_ACTION`
for the screened fact, a retrospective decision for the cut one. Whether that is worth anything is a
question about consumers of the certificate, and this harness does not ask it. It is stated here so
the tie above is not read as "the compiler does nothing".
