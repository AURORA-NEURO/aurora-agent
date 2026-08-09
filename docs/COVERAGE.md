# Blueprint coverage

The blueprint ships 973 content modules across 44 sections (990 files, less 17 section indexes).
This file records which of them the workspace actually cites, so the next batch of crates is chosen
from evidence rather than from whichever section came to mind.

**Measured, not asserted.** A module counts as covered when its id appears in a doc comment, a test,
or a design note under `crates/` or `docs/`. That is a weak criterion on purpose — a citation is not
an implementation — so read the numbers as *"someone has read this and taken a position on it"*,
never as *"this is done"*. The stronger criterion would be a conformance test per module. It does
not exist and is not being claimed.

The table below is a snapshot; the script is the live source.

```bash
BLUEPRINT=/path/to/distribution/root tools/coverage.sh
```

## Where the 973 modules are

Ten sections are programme documents rather than specifications of behaviour: start-here material,
strategy, system-architecture narrative, the research and implementation programmes, growth and
community, the ADR log, sources, and templates. They total **214 modules and describe no code**.
Counting them in a coverage denominator would be flattering and meaningless.

§02 is the borderline call. It is architecture narrative the crate layout already reflects without
citing it, so excluding it slightly understates coverage rather than overstating it.

| | modules |
|---|---|
| total content modules | 973 |
| programme / prose modules | 214 |
| **code-bearing modules** | **759** |
| cited | 448 |
| **code-bearing coverage** | **59.0%** |

## Per section

Worst-covered code-bearing sections first. **This table is a snapshot from an earlier batch and is
now stale** — headline coverage has moved from 40.6% to 59.0% since it was taken. Regenerate with
`tools/coverage.sh` rather than trusting the rows below for anything load-bearing; they are kept
because the *shape* they show is still the argument, and the shape has not changed.

| § | section | cited | total | crate |
|---|---|---:|---:|---|
| 35 | MILLION_SCALE_BENCHMARK_FACTORY_AND_INFRASTRUCTURE | 0 | 18 | — |
| 04 | INGESTION_AND_INTEROP | 1 | 6 | `adapter` |
| 06 | BENCHMARK_COMPILER | 2 | 15 | — |
| 27 | BENCHMARK_FACTORY_AND_HUB | 2 | 22 | `factory` |
| 38 | REFERENCE_BIOWORLDS_AND_VERTICAL_SLICES | 2 | 16 | `examples` |
| 09 | INFERENCE_LAB | 3 | 11 | — |
| 12 | DATA_AND_INFRASTRUCTURE | 3 | 22 | `ledger` |
| 41 | GRAPH_FIRST_KNOWLEDGE_AND_NAVIGATION | 3 | 16 | — |
| 10 | REGISTRY_AND_HUB | 4 | 22 | `registry`, `hub` |
| 11 | DEVELOPER_PLATFORM | 4 | 25 | `sdk` |
| 19 | REFERENCE_EXAMPLES | 4 | 22 | `examples` |
| 28 | BIOLOGY_DATA_AND_STANDARDS | 4 | 21 | `standards` |
| 33 | BIOCAPABILITY_ATLAS_AND_METRICS | 4 | 19 | `atlas` |
| 42 | GRAPH_NATIVE_EVALUATION_HUB_AND_UI | 4 | 31 | — |
| 13 | SECURITY_PRIVACY_AND_SAFETY | 5 | 26 | — |
| 34 | BIOATLAS_PUBLIC_HUB_AND_ECOSYSTEM | 6 | 23 | `hub` |
| 14 | GOVERNANCE_AND_QUALITY | 7 | 25 | `governance` |
| 30 | NEURO_ONCOLOGY_ONCOWORLD | 7 | 30 | `onco` |
| 08 | ADAPTIVE_EVALUATION | 7 | 8 | `adaptive` |
| 31 | BIOLOGICAL_ORACLES_AND_REFERENCE_STANDARDS | 8 | 17 | `oracle` |
| 05 | EXECUTION_RUNTIME | 9 | 12 | `runtime` |
| 07 | EVALUATION_ENGINE | 9 | 13 | `evalengine` |
| 25 | BIOLOGICAL_IR_AND_LANGUAGE | 9 | 23 | `bioir` |
| 03 | CORE_SPECIFICATIONS | 10 | 12 | `section`, `fiber` |
| 32 | BIOLOGICAL_MUTATION_AND_STRESS_PROGRAM | 11 | 23 | `stress` |
| 23 | AGENT_INTERWEAVE_FABRIC | 12 | 50 | `weave` |
| 26 | BIO_EVALUATION_ENGINE | 12 | 24 | `bioeval` |
| 39 | TOKEN_EFFICIENT_BIOLOGICAL_INFERENCE | 14 | 25 | `fiber`, `section` |
| 36 | BIOLOGY_SECURITY_PRIVACY_ETHICS_AND_GOVERNANCE | 15 | 22 | `policy` |
| 24 | BIOPRISM_FOUNDATION | 17 | 17 | `foundation` |
| 40 | BUILD_READY_ENGINEERING_CONTRACTS | 21 | 45 | spread across all |
| 29 | BIOLOGY_CAPABILITY_AND_BENCHMARK_PACKS | 22 | 22 | `packs` |
| 15 | BENCHMARK_PACKS | 26 | 26 | `packs` |
| 43 | FIBER_QUERY_COMPILED_EPISTEMIC_CALCULUS | 39 | 50 | `fiber`, `section`, `ids` |

Excluded as prose: §00 (16), §01 (7), §02 (10), §16 (20), §17 (26), §18 (23), §20 (45), §21 (12),
§22 (26), §37 (29).

## What the shape of this table says

Three findings, none of them flattering.

**The deepest section is the best covered, and that is survivorship.** §43 (FIBER, 50 modules) sits
at 78% because it is the thesis and was built first. §23 (Agent Interweave Fabric, also 50 modules)
sits at 24% because `weave` deliberately stayed a microkernel. Those two numbers are not comparable
quality signals — one is depth, the other is restraint — and averaging them would hide both.

**§40 is the most valuable uncovered surface.** It is the only section marked build-ready rather
than planned: frozen contracts, not design prose. 24 of its 45 modules are untouched. Every crate
that worked from a §40 module had an easier time than the ones working from `Planned` text, so at
equal size an uncovered §40 module should be preferred over an uncovered module anywhere else.

**Whole capability areas had no crate at all, and now all six do.** §13 security and safety (26),
§42 graph-native evaluation and UI (31), §35 million-scale infrastructure (18), §41 graph-first
navigation (16), §06 benchmark compiler (15) and §09 inference lab (11) were 117 modules — 15% of
the code-bearing blueprint — with nothing standing in for them. `safety`, `lens`, `scale`,
`docgraph`, `benchcompiler` and `lab` closed that set. The remaining gaps are depth inside sections
that already have a crate, which is a different and easier problem than a blank area.

## Boilerplate, and why the numbers are not strictly comparable

Sixteen sections have now been measured, each by the agent that built against it. Most are heavily
repetitive; one is not, and the exception matters more than the average.

| § | boilerplate | distinguishing lines per module |
|---|---:|---|
| 42 | 93.6% | 5 — title, module id, H1, one outcome sentence, one diagram label |
| 35 | 82.3% | 14–16 |
| 32 | 79.3% | 19 median |
| 41 | 72.6% | 14.1 mean |
| 06 | 70.8% | 17–25 |
| 14 | 70% | 19.4 of ~65 non-blank |
| 09 | 68.8% | 17–31, median 19 |
| 13 | 67.5% | 19–32, median 21 |
| 28 | 52% | 34.5 of 71.5 non-blank |
| **23** | **16.2% verbatim / 51.2% rare-term** | **~54 median** |
| 12 | — | ~15 per 100-line file |
| 11 | — | 18 unique in a 93-line module, frontmatter and title included |

**§23 is the exception and it is a real one.** Measured three ways over all 50 modules: 16.2% of
lines appear in more than one module, 51.2% by the rare-term method used for §28, and only 11.6% of
802 headings recur verbatim. It is the most content-dense section in the blueprint, and its
repetition is *shape* — frontmatter, Purpose, a taxonomy list, a pseudo-code fence, evaluation hooks
— rather than text. Six of its modules yielded roughly 300 distinguishing lines.

**These figures were produced by different methods and are not a single scale.** Only §23 was
measured three ways, and its own two headline numbers differ by 35 points. A verbatim-duplication
count and a rare-term count answer different questions, and no agent was given a common definition.
Read the column as evidence that a section is repetitive or is not, and distrust small differences
between rows.

There is now a measured instance of exactly that hazard. Two agents independently measured §23's
verbatim duplication over the same 6,001 non-blank lines, both describing the metric as lines
appearing in another module, and got **16.2%** and **10.7%**. Recomputing it settles the discrepancy:
974 of 6,001 line *occurrences* sit in a string that appears in more than one module, so 16.2% is
right for that definition and 10.7% counts something narrower.

The recomputation also produces the sharper number. Only **2.1% of distinct line-strings** (108 of
5,078) are shared at all, and those 108 strings account for the whole 16.2% of occurrences. A section
is not repetitive because it has a lot of repeated content; it is repetitive because a small template
is stamped many times. That is the shape every high-boilerplate section in the table has, and it is
why "distinguishing lines per module" is the more useful column.

What survives every method: section size predicts *reading* cost and not *implementation* cost, so a
coverage percentage weighted by module count overstates how much real design surface remains.

Three findings are worth quoting exactly. In §14 one mitigation sentence appears **125 times** across
the section, under different failure modes. In §32 every one of the 23 modules carries an identical
transformation-contract YAML in which all four `changes:` flags are `false` — including modules that
obviously change observation — and `expected_relation.type` is a six-way pipe union that is never
resolved to a selection. In §42 every module's "Required API objects" list is identical, and none of
the seven objects is defined anywhere in the section.
