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
| cited | 287 |
| **code-bearing coverage** | **37.8%** |

## Per section

Worst-covered code-bearing sections first. `ledger`, `sdk`, `governance`, `standards` and `stress`
were mid-flight when this was taken, which is why §11, §12, §14, §28 and §32 read low.

| § | section | cited | total | crate |
|---|---|---:|---:|---|
| 35 | MILLION_SCALE_BENCHMARK_FACTORY_AND_INFRASTRUCTURE | 0 | 18 | — |
| 04 | INGESTION_AND_INTEROP | 1 | 6 | `adapter` |
| 11 | DEVELOPER_PLATFORM | 1 | 25 | `sdk` |
| 12 | DATA_AND_INFRASTRUCTURE | 1 | 22 | `ledger` |
| 06 | BENCHMARK_COMPILER | 2 | 15 | — |
| 27 | BENCHMARK_FACTORY_AND_HUB | 2 | 22 | `factory` |
| 28 | BIOLOGY_DATA_AND_STANDARDS | 2 | 21 | `standards` |
| 38 | REFERENCE_BIOWORLDS_AND_VERTICAL_SLICES | 2 | 16 | `examples` |
| 09 | INFERENCE_LAB | 3 | 11 | — |
| 32 | BIOLOGICAL_MUTATION_AND_STRESS_PROGRAM | 3 | 23 | `stress` |
| 41 | GRAPH_FIRST_KNOWLEDGE_AND_NAVIGATION | 3 | 16 | — |
| 10 | REGISTRY_AND_HUB | 4 | 22 | `registry`, `hub` |
| 14 | GOVERNANCE_AND_QUALITY | 4 | 25 | `governance` |
| 19 | REFERENCE_EXAMPLES | 4 | 22 | `examples` |
| 33 | BIOCAPABILITY_ATLAS_AND_METRICS | 4 | 19 | `atlas` |
| 42 | GRAPH_NATIVE_EVALUATION_HUB_AND_UI | 4 | 31 | — |
| 13 | SECURITY_PRIVACY_AND_SAFETY | 5 | 26 | — |
| 34 | BIOATLAS_PUBLIC_HUB_AND_ECOSYSTEM | 6 | 23 | `hub` |
| 30 | NEURO_ONCOLOGY_ONCOWORLD | 7 | 30 | `onco` |
| 08 | ADAPTIVE_EVALUATION | 7 | 8 | `adaptive` |
| 31 | BIOLOGICAL_ORACLES_AND_REFERENCE_STANDARDS | 8 | 17 | `oracle` |
| 05 | EXECUTION_RUNTIME | 9 | 12 | `runtime` |
| 07 | EVALUATION_ENGINE | 9 | 13 | `evalengine` |
| 25 | BIOLOGICAL_IR_AND_LANGUAGE | 9 | 23 | `bioir` |
| 03 | CORE_SPECIFICATIONS | 10 | 12 | `section`, `fiber` |
| 23 | AGENT_INTERWEAVE_FABRIC | 12 | 50 | `weave` |
| 26 | BIO_EVALUATION_ENGINE | 12 | 24 | `bioeval` |
| 39 | TOKEN_EFFICIENT_BIOLOGICAL_INFERENCE | 14 | 25 | `fiber`, `section` |
| 36 | BIOLOGY_SECURITY_PRIVACY_ETHICS_AND_GOVERNANCE | 15 | 22 | `policy` |
| 24 | BIOPRISM_FOUNDATION | 17 | 17 | `foundation` |
| 40 | BUILD_READY_ENGINEERING_CONTRACTS | 20 | 45 | spread across all |
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
than planned: frozen contracts, not design prose. 25 of its 45 modules are untouched. Every crate
that worked from a §40 module had an easier time than the ones working from `Planned` text, so at
equal size an uncovered §40 module should be preferred over an uncovered module anywhere else.

**Whole capability areas have no crate at all.** §13 security and safety (26), §42 graph-native
evaluation and UI (31), §35 million-scale infrastructure (18), §41 graph-first navigation (16), §06
benchmark compiler (15), §09 inference lab (11). That is 117 modules — 15% of the code-bearing
blueprint — with nothing standing in for them. It is where the batches after the current five
should go.

## The boilerplate correction

Six sections have now been measured independently, each by the agent that built against it, and each
reported the same thing: §07, §08, §31, §32, §33 and §34 carry roughly **15 lines of distinguishing
content per module** inside ~90% identical scaffolding. A section of 24 modules is therefore closer
to 360 lines of specification than to 24 specifications.

This matters for planning. Section size predicts *reading* cost but not *implementation* cost, so a
coverage percentage weighted by module count overstates how much real design surface remains. It is
recorded here rather than corrected away: the correction factor is not uniform across sections, and
inventing one would be worse than stating the observation.
