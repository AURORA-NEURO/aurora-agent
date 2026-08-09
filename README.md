# bioprism

**Query-compiled inference for executable biology.**

Implementation of the AURORA BioPRISM / OncoWorld / FIBER blueprint (v0.6, 935 registered spec
modules). A Rust workspace whose central idea is that **context assembly is a compiler pass**:
instead of retrieval plus summarisation plus vibes-based compaction, a typed decision query is
compiled into the smallest decision-sufficient evidence region, delivered as a **Decision
Section**, and accompanied by a **Context Certificate** that states exactly what was omitted and
whether the omission could have changed the decision.

> Compile the smallest decision-sufficient evidence region. Never traverse the whole knowledge
> structure by default.

## What the measurements actually say

The reference world ships 761 facts, 750 of them exploratory distractors that all consume the same
protected `cohort_id` hub. FIBER compiles the query down to **11 facts (1.45% of the world)** and
the deterministic oracle still returns the correct verdict with all four leakage witnesses.

**It is not alone in doing so.** Under equal tuning, a 5-hop incidence walk and a BM25 retriever at
k=11 select *exactly the same eleven facts*. The distribution's own `compare_baselines.py` measures
the graph baseline only at depth 7 and unbounded — the two settings where it returns everything —
and reports a 69× advantage that vanishes under equal tuning. That is a strawman comparison, and
correcting it is what 43.38 and 43.41 require.

So the reference world cannot tell these methods apart. [`crates/worldgen`](crates/worldgen) makes
the structure a parameter and builds one that can — distractors attached near the target instead of
at a hub leaf, decisive facts behind a relay chain, and distractor tags camouflaged to tokenise into
the protected vocabulary:

| Strategy | Facts | Sound? | Closure | Admissible |
|---|---:|:-:|---:|:-:|
| full-context | 762 | yes | 100% | yes |
| graph-5-hop | 750 | **no** | 0% | **no** |
| graph-7-hop | 750 | **no** | 0% | **no** |
| graph-11-hop | 761 | yes | 100% | yes |
| lexical-top-11 (BM25) | 11 | yes | **91%** | **no** |
| **fiber** | **11** | **yes** | **100%** | **yes** |

Three distinct failure modes appear. The graph walk has **no usable depth**: 5–10 pull in 98% of the
world *and still miss every decisive witness*; 11 is the first sound setting and by then it has
taken everything. BM25 reaches the *right verdict* from a **91% protected closure** — right by luck,
having dropped a protected fact that happened not to matter, and raising k to 50 never recovers it.
FIBER is the only admissible strategy: right verdict **and** full closure, at 11 facts.

That last failure is why the harness ranks on admissibility rather than verdict alone — ranking on
verdict would have crowned the strategy that violated the mandatory closure and got away with it.

This does not show FIBER wins generally: the discriminating world was built to expose these modes,
just as the reference world was built to expose hub expansion. Both are single points, the full
sweep is not done, and an embedding retriever and a *directed* dependency walk are still missing
from the panel. Full analysis: [docs/FINDINGS.md](docs/FINDINGS.md).

## Status

Twenty-three modules built, **820 tests passing, zero clippy warnings**. Byte-level parity with the CPython
reference runtime is achieved and enforced in CI-able tests — and now holds across *three*
implementations: CPython, the Rust eager path, and the Rust indexed store.

| Crate | Blueprint | What it does | Tests |
|---|---|---|---|
| [`bioprism-ids`](crates/ids) | 40.05 | Canonical JSON, content hashing, typed identifiers | 8 |
| [`bioprism-scope`](crates/scope) | 43.03, 43.05, 43.06 | Typed scope base, refinement order, meet lattice, RFC 3339, mapping taxonomy | 18 |
| [`bioprism-world`](crates/world) | 43.02, 43.04, 43.07, 43.09 | Local evidence sections, typed factors, causal events, indices, diagnostics | 14 |
| [`bioprism-section`](crates/section) | 43.25, 43.26, 43.36 | Decision Section IR, Context Certificate, omission manifest, plan descriptor | 8 |
| [`bioprism-fiber`](crates/fiber) | 43.13, 43.16, 43.17, 43.41 | Protected closure, backward slice, temporal cut, oracle, compile pipeline | 15 |
| [`bioprism-baseline`](crates/baseline) | 43.38, 43.41 | Equal-engineering comparators: full-context, k-hop, component, query-graph, BM25 | 8 |
| [`bioprism-worldgen`](crates/worldgen) | 43.39 | Synthetic structural families: attachment, relay depth, tag camouflage, leakage injection | 8 |
| [`bioprism-store`](crates/store) | 43.34 | Content-addressed indexed storage; on-disk sorted maps, binary-search lookup | 8 |
| [`bioprism-mcp`](crates/mcp) | 11.11, 43.35 | MCP server: progressive disclosure, root confinement, side-effect preview | 13 |
| [`bioprism-prism`](crates/prism) | 03, 06, 07 | Decision Cells, matched forks, state minimization, attested bundles | 9 |
| [`bioprism-weave`](crates/weave) | 23.05–23.16, 23.49 | Microkernel: typed acts, hash-chained ledgers, attenuating authority, affine budgets, capsules, continuations | 15 |
| [`bioprism-mutation`](crates/mutation) | 03.08, 32 | Metamorphic mutations with executable postconditions, lineage, dedup, effective diversity | 14 |
| [`bioprism-runtime`](crates/runtime) | 05 | WorldTape, determinism seams, fork/suffix execution, effects broker, sandbox | 100 |
| [`bioprism-policy`](crates/policy) | 43.33, 13, 36 | Policy as a scope fiber, consent/purpose, information-flow lattice, redaction receipts | 100 |
| [`bioprism-adaptive`](crates/adaptive) | 08 | Capability posterior, parent-clustered uncertainty, information-gain selection | 90 |
| [`bioprism-routing`](crates/routing) | 09 | Task fingerprint, evidence router, oracle-bounded regret accounting | 81 |
| [`bioprism-obligation`](crates/obligation) | 39 | Decision obligation graph, action gating, token budget, sufficiency certificate | 54 |
| [`bioprism-atlas`](crates/atlas) | 33, 03.09, 03.10 | Capability ontology, failure taxonomy, unmeasured-vs-poor, claim ladder | 61 |
| [`bioprism-backends`](crates/backends) | 43.18–43.36 | Elimination order, induced width, variable elimination, portfolio and fallback | 53 |
| [`bioprism-packs`](crates/packs) | 15, 29, 03.06 | Pack IR, 46-pack portfolio, health gating, coverage gaps | 51 |
| [`bioprism-conformance`](crates/conformance) | 40.31–40.33 | Wire-level conformance cases, fixture drift, test pyramid, release gate | 41 |
| [`bioprism-graph`](crates/graph) | 41, 42, 43.01 | Graph/hypergraph/timeline/table projections with bound provenance | 39 |
| [`bioprism-cli`](crates/cli) | 40.13, 40.36 | `bioprism` binary: JSON mode, dry-run, exit-code matrix | 12 |

### Cross-language parity

Certificate hashes are taken over canonical bytes, so Rust and Python must agree exactly or a
certificate produced by one cannot be replayed by the other. Both the Decision Section and the
Certificate are byte-identical to `reference/fiber_runtime/fiber_compile.py`:

```
certificate_sha256      c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4
decision_section_sha256 7439b2262c52c1c794b59be86d922b723a2ea5646362d529f57fb11b5f7e93ce
world_sha256            b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5
```

Getting there required matching CPython in two places a naive port gets wrong: `repr` float
formatting (CPython switches to exponential at a different threshold than Rust and zero-pads the
exponent) and JSON object iteration order, which the reference relies on when building leakage
witnesses.

## Quickstart

```bash
cargo build --release --offline
```

```bash
./target/release/bioprism context explain --world fixtures/fiber-v0.1/radiogenomic_world.json --query fixtures/fiber-v0.1/leakage_query.json
```

That prints a database-style explain plan: which passes ran and what each retained, the backend,
selection ratios, omissions grouped by influence class, the oracle verdict with its witnesses,
and — importantly — **which passes did not run and why**.

```bash
./target/release/bioprism --json context compile --world fixtures/fiber-v0.1/radiogenomic_world.json --query fixtures/fiber-v0.1/leakage_query.json --certificate-out cert.json
```

```bash
./target/release/bioprism context verify --certificate cert.json
```

### Scale

Compiling from a JSON document parses the whole world on every query. Index it once instead:

```bash
./target/release/bioprism world index --world big-world.json --store big-world.bpw
```

`--world` then accepts the store directory anywhere it accepted a document, and the certificate is
identical. On a one-million-fact world this takes query time from **26.5 s to 41.6 ms (638×)**, and
compile cost becomes roughly logarithmic in corpus size rather than linear. The reasoning and the
full measurements are in [ADR-001](docs/ADR-001-language-strategy.md).

## Using it from an agent

```bash
./target/release/bioprism-mcp --root .
```

Speaks JSON-RPC 2.0 over stdio. `fiber_compile` returns the **L0 decision contract** — goal,
verdict, what was omitted, whether the sufficiency claim holds — plus a refinement handle, and
*not* the evidence. An agent descends with `fiber_refine` only when the contract is insufficient to
act on. On the reference world L0 is ~204 estimated tokens against ~1,900 for the full section.

The invariant that makes that safe: **omissions are reported at every layer**, so an agent that
stops at L0 still knows what it does not have. Layering hides volume, never the fact of an
omission. Paths are confined to `--root`; absolute paths and `..` are refused; `world_index`
previews its write unless called with `confirm: true`.

## Evaluating a context policy

```bash
./target/release/bioprism prism fork --world w.json --query q.json --bundle-out bundle.json
```

Freezes a Decision Cell from the full-context verdict, then runs every architecture from that
identical state — so a difference is attributable to the context policy and nothing else. On the
discriminating world:

| Architecture | Facts | Verdict | Closure | Cell |
|---|---:|---|---:|:-:|
| fiber | 11 | invalid | 100% | pass |
| full-context | 762 | invalid | 100% | pass |
| graph-5-hop | 750 | valid | 0% | **fail** |
| lexical-top-11 | 11 | invalid | 91% | **fail** |

Exit 1 when any architecture fails, so it gates CI. Acceptance is set-valued (03.07) and names its
failure mode: `graph-5-hop` fails on verdict, `lexical-top-11` on closure.

```bash
./target/release/bioprism prism minimize --world w.json
```

Reduces the world to a 1-minimal set preserving the oracle signature, then re-verifies it. On the
reference world: **761 facts → 6**, in 762 oracle evaluations.

That 6 is worth reading carefully against FIBER's 11. Only six facts are *causally* required for
the verdict; the other five are protected-closure facts that participate in no witness. FIBER is
deliberately **not** minimal — 43.13 makes identity, policy and negative-evidence closure mandatory
whether or not it moves this particular decision. Minimization measures what the verdict rests on;
closure decides what must be present regardless.

## Composing agents

`bioprism-weave` is the microkernel of §23 — a deliberately small trusted computing base that
enforces what cannot be delegated to untrusted participants and refuses to do anything else. Per
23.49 it "should not decide scientific truth, write patches, plan tasks, summarize evidence, or
choose a model", and it does not: it never inspects an act's payload for meaning.

What it does enforce, each with a conformance test named after it:

- **typed acts** — you cannot accept what was not proposed, challenge what was not claimed, or
  discharge what was not accepted, and a commitment cannot be discharged twice;
- **rejected acts never enter the ledger** — an unauthorised or unfunded move must not be able to
  write history;
- **attenuating authority** — delegation can only narrow, and revocation is transitive over the
  whole subtree;
- **affine budgets** — `Budget` does not implement `Clone`, so duplicating an allowance is a
  compile error rather than a runtime check; splitting *moves* it;
- **hash-chained ledgers** — claims and their challenges both survive; contradiction is preserved,
  not resolved into a score;
- **continuations** — a handle bound to a stale ledger head is refused rather than silently
  rebased; forking from a superseded point is the supported move.

Where Weave meets FIBER is the Context Capsule. A capsule is a recipient-specific projection of a
compiled Decision Section, so it inherits the certificate: a participant learns what the *compiler*
omitted from the world and, separately, what the *projection* withheld from it. A filtered capsule
reports `supports_sufficiency_claim: false` regardless of the compiler's own verdict — a
participant reasoning from a partial view cannot vouch for completeness it never observed.

## Generating a benchmark family

```bash
./target/release/bioprism mutate family --world w.json --out-dir family/
```

Applies eight metamorphic relations, each declaring what the oracle must do — four invariances
(rename, reorder, add distractors, camouflage tags) and one repair per leakage mechanism. **A
mutation does not get to mark its own homework**: the postcondition is checked by running the
oracle, and a mutation whose declared relation does not hold is rejected rather than shipped.

The headline number is deliberately not the instance count:

```
8 validated instances from 1 audited parent across 8 mutation families,
providing 8 independent equivalence classes (inflation ×1.00).
Instance count is not benchmark count.
```

An *equivalence class* is a distinct (parent, mutation family, oracle signature) triple — a counted
quantity, not a modelled one. Generate twenty reorderings of the same world and you get twenty
instances, **one** equivalence class, and an inflation ratio of ×20; the family is reported as a
robustness check rather than a benchmark. This is the executive summary's constraint made
operational: *a million paraphrases are not a million benchmarks*.

Deduplication hashes semantic content — facts, factors, events — and deliberately **not**
`world_id`, so a generator cannot defeat it by renaming.

## What is deliberately not implemented

The blueprint describes far more than exists here, and the gap is reported by the software rather
than buried in prose. Every compile returns `deferred_passes`, and `bioprism context explain`
prints them:

| Pass | Why it cannot run |
|---|---|
| Gluing and obstruction tests (43.06) | Requires a declared cover; `fiber-world/0.1` carries none |
| Abstract interpretation (43.11) | Requires an abstract-domain registry absent from the wire schema |
| Decision-equivalence quotient (43.10) | Defined relative to permitted actions and decision loss, neither of which `fiber-query/0.1` carries |
| Rate-distortion optimisation (43.12) | Optimises against a decision loss the query does not declare |

The backend portfolio of 43.19–43.24 (FAQ/InsideOut, worst-case-optimal joins, tensor networks,
decision diagrams, incremental view maintenance) is **not built**. `Backend` enumerates them so
the plan descriptor is honest about which one ran; only
`backward_factor_slice_reference` exists today.

Per 43.43, nothing here claims to have invented sheaves, factor graphs, semirings, tensor
networks, abstract interpretation, rate-distortion theory, or database query optimisation.

### Two honesty mechanisms worth knowing about

**Zero influence is not unknown influence.** The omission manifest classes every omitted group as
`zero`, `bounded`, `inaccessible_by_policy`, `deferred_acquisition` or `unknown`. Only `zero` and
`bounded` support a sufficiency claim; a single `unknown` group voids it. The reference v0.1
certificate has one `classification` *string* for all omissions and cannot express this, which is
why `--profile extended` exists.

**Zero-influence claims state their assumption.** Facts with no backward dependency path are
classed `zero` *conditional on the declared factor graph being complete* — the reason string says
so, because an incomplete factor graph turns a zero-influence claim into an unknown-influence one.

## Defects found in the v0.6 distribution

1. **`machine/module_registry.jsonl` is stale and omits FIBER entirely.** It carries 935 rows and
   **zero** from section 43, while `context_cards.jsonl` and `doc_graph.json` both carry all 51
   FIBER modules (994 rows each). `machine/README.md` claims one row per module. Any agent routing
   off the registry never sees the canonical runtime.
2. **The reference runtime hard-codes a radiogenomic goal string** into every Decision Section,
   and compares label timestamps **lexicographically as strings** rather than as parsed instants.
   Both are reproduced for parity, both are flagged: see `REFERENCE_GOAL` in
   [`qir.rs`](crates/fiber/src/qir.rs) and the note on `temporal_witnesses` in
   [`oracle.rs`](crates/fiber/src/oracle.rs).

Only 131 of the 935 registered modules are marked `Build-Ready Specification`; **400 are
`Planned`**. Sections 01–19 and 23–29 are 0% build-ready, including `03_CORE_SPECIFICATIONS`, all
50 files of `23_AGENT_INTERWEAVE_FABRIC` and all 24 of `25_BIOLOGICAL_IR_AND_LANGUAGE`. Those need
design work before implementation, not just coding.

## Repository layout

```
crates/           the workspace, bottom of the dependency DAG first
  ids/            canonical serialization + hashing + typed ids  (no internal deps)
  scope/          typed scope base                               (ids)
  world/          FIBER world model                              (ids, scope)
  section/        Decision Section + Context Certificate         (ids)
  fiber/          the query compiler                             (ids, scope, world, section)
  baseline/       equal-engineering comparators                  (ids, world, section, fiber)
  worldgen/       synthetic structural benchmark families        (world)
  store/          content-addressed indexed storage              (ids, scope, world)
  mcp/            Model Context Protocol server                  (fiber, section, store, world)
  prism/          decision-state evaluation                      (baseline, fiber, section, world)
  weave/          the multi-agent microkernel                    (ids, section)
  mutation/       metamorphic instance generation                (fiber, section, world)
  cli/            the bioprism binary                            (all)
docs/             FINDINGS.md and the generated baseline comparison
fixtures/         golden worlds, queries and reference artifacts
reference/        the CPython reference runtime, vendored as the parity oracle
schemas/          fiber-world / fiber-query / fiber-context-certificate JSON Schemas
tools/            golden regeneration and ground-truth generation
```

`section` deliberately depends on neither `world` nor `fiber`: a consumer — an MCP client, an
evaluator, a CI gate — must be able to read and verify a compiled context without linking the
engine that produced it.

## Development

```bash
cargo test --workspace --offline
```

```bash
cargo clippy --workspace --all-targets --offline
```

```bash
python tools/regenerate_golden.py
```

The last one re-derives the golden artifacts from the CPython reference. A diff there is a change
to the wire format and needs a schema version bump.

Builds are offline by default (`.cargo/config.toml`) against pinned dependency versions.

## Boundary

Research and developer infrastructure. It does not diagnose an individual, recommend treatment,
triage care, autonomously enroll participants, or claim medical-device functionality. Compression
or abstraction never authorizes crossing a data-use, consent, privacy, or clinical boundary.

## License

Apache-2.0
