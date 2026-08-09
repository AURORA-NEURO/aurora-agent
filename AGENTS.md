# AURORA Agent

AURORA Agent is a **context harness for biological research agents**. Its one idea is that context
assembly is a compiler pass, not a retrieval heuristic: a typed decision query compiles into the
smallest decision-sufficient evidence region, delivered as a **Decision Section** and accompanied by
a **Context Certificate** stating exactly what was omitted and whether the omission could have
changed the decision.

Everything else in the workspace is downstream of that. Storage exists to make compilation
output-sensitive. Evaluation exists to test whether the compiled context was any good. Mutation
exists to generate more decisions to test against. The MCP server exists so the compiled context
reaches something that can act on it.

The engine crate is `bioprism-fiber`; the workspace is prefixed `bioprism-` throughout, from the
BioPRISM blueprint it implements.

## What we can never compromise on

### 1. Honest labelling is the product

Every other system can tell you what it included. This one tells you what it *left out*, and
whether that could have mattered. If we lose that, we are a slower retrieval library.

Concretely, and these are non-negotiable:

- **Zero influence is not unknown influence.** "Provably cannot matter" and "nobody checked" are
  different states and must never share a representation. A single unknown-influence group voids a
  sufficiency claim.
- **Unmeasured is not zero.** A capability with no evidence is `Unmeasured`, categorically distinct
  from measured-and-poor. There is no `score_or_zero`.
- **A right answer from an incomplete basis is not a pass.** Protected closure is mandatory *before*
  any relevance step, so a strategy cannot be credited for guessing correctly from evidence it
  never had.
- **Instance count is not benchmark count.** Report independent equivalence classes. A million
  paraphrases are a robustness check, not a million benchmarks.

### 2. Invariants belong in the type system, not in comments

Where a rule can be made unrepresentable, make it unrepresentable. Existing examples to match:

| Rule | How it is enforced |
|---|---|
| A budget cannot be duplicated | `Budget` does not implement `Clone` — copying is a compile error |
| An unmeasured capability has no score | `Measurement` has private fields and one gated constructor |
| Provenance cannot be forged | `View` fields are private with a crate-internal seal; `Serialize`-only |
| Replay cannot fall through to live | `ReplayHost` has no source field, so no such branch exists |
| A cell needs human approval | `approve()` is the only path to a `DecisionCell` |
| Progression needs confirmation | The variant carries a token only the confirmation gate can mint |

A test that asserts a rule is good. A type that makes the rule unbreakable is better.

### 3. Determinism, byte for byte

Certificates hash canonical bytes. Cross-language parity is genuinely hard — matching CPython
required reproducing its `repr` float threshold, its exponent zero-padding, and its JSON object
iteration order, and serde_json needed `float_roundtrip` before its parser agreed with either
CPython or native Rust. Three implementations currently agree on the reference certificate
(`c0da17ff…7ea4`): CPython, the eager Rust path, and the indexed store. Do not break that without a
schema version bump.

### 4. Negative results ship

The comparison harness exists to make the central claim falsifiable, and it has come out against us
more than once. Both of these are in the repository, as tests:

- On the shipped reference world, a 5-hop graph walk and a BM25 retriever select **the identical
  eleven facts** FIBER does. The distribution's own baseline script only measured depths where
  graphs explode.
- The evidence router **captures 0% of available gain** under regime holdout and abstains on every
  task. Its own report says it did not beat the fixed default.

If a measurement disagrees with the thesis, that is the measurement we publish. Reporting it is
cheaper than someone else finding it.

## A small glossary

Use this language; the code does.

- **World** — facts, typed factors, and a causal event structure. Not a knowledge graph.
- **Fact** — a *local section*: a value valid inside one scope, not a globally true statement.
- **Factor** — a typed relation between variables. May be extensional, intensional, or rule-compiled.
- **Scope** — where a claim holds: identity, region, specimen, time, coordinate frame, ontology, policy.
- **Protected closure** — evidence that must be present regardless of relevance. Computed first.
- **Decision Section** — what the model actually sees, in layers L0–L4.
- **Context Certificate** — the receipt: what was included, omitted, and with what influence class.
- **Decision Cell** — a frozen decision state two architectures can resume from identically.
- **Witness** — a concrete checkable object ("alias ALT-77 spans train and test"), never a score.
- **Admissible** — right verdict *and* full protected closure. Cheapness alone is not a win.

## Navigating the workspace

Bottom of the dependency graph first.

```
ids scope                    foundation: canonical bytes, hashing, typed scopes
world store worldgen         the world, its index, and generated structural families
section fiber                the compiler and what it emits
weave                        the multi-agent microkernel (a TCB — keep it small)
prism baseline mutation      evaluation: cells, comparators, metamorphic families
registry packs atlas         packs, trust tiers, capability coverage
mcp cli trace                the surfaces an agent or a human touches
```

`section` deliberately depends on neither `world` nor `fiber`: a consumer must be able to *verify* a
compiled context without linking the engine that produced it.

## Working here

- **Read the blueprint module before implementing it**, and cite its id in the doc comment. Much of
  the spec is `Planned` rather than build-ready; where it under-specifies, say so in the docs rather
  than inventing detail and presenting it as spec.
- **Name what is not implemented.** Every crate's `lib.rs` carries an explicit list. A missing
  capability that is stated is a limitation; one that is implied to exist is a lie.
- **Tests state their claim in the name.** `a_budget_smaller_than_the_closure_fails_rather_than_truncating`,
  not `test_budget_2`. Smoke tests that exercise a path without asserting an invariant are worse
  than no test, because they inflate the count.
- **Doc comments explain *why*.** No `//` comments restating what the line does.
- **Green means green**: `cargo test -p <crate> --offline` passing and
  `cargo clippy -p <crate> --all-targets --offline` at zero warnings.

### Environment quirks worth knowing

- Builds are **offline** against pinned versions. You cannot add an external crate. Several things
  are hand-rolled for this reason: the CSV reader, the arg parser, JSON-RPC, log-gamma, RFC 3339.
- Windows Application Control sometimes blocks a freshly linked test binary with `os error 4551`
  ("never executed"). `cargo test` reports `error: test failed` and moves on, so **a suite that
  never ran looks like a suite that failed**, and a `--workspace` sum silently loses every test
  after it — one run reported 344 where the true figure was 4,327. Touch a test file to force a
  relink; `tools/status.sh --tests` does that and warns if any binary still refuses to run.

### Skills

`.agents/skills/` holds what agents working here have had to relearn. Read the relevant one before
starting, not after.

| Skill | For |
|---|---|
| `add-module` | starting a crate against a blueprint section |
| `classify-blueprint-modules` | deciding which modules are code at all, and citing ids without inflating coverage |
| `measure-section-boilerplate` | reporting how repetitive a section is, reproducibly |
| `keep-a-claim-honest` | the recurring defect: an error swallowed into a benign default |
| `prove-a-scanner-fires` | any test that greps the crate's own text |
| `verify-crate` | green means green, and the failures that are not failures |
| `check-parity` | anything touching canonical bytes, hashing or the store |

## Commit discipline

Small, semantic commits, one concern each. A commit that adds a module, a commit that fixes a
defect, a commit that records a measurement. The history should read as the argument for the
design, not as a series of "wip" saves.

## Boundary

Research and developer infrastructure. It does not diagnose an individual, recommend treatment,
triage care, enroll participants, or claim medical-device functionality. Compression or abstraction
never authorizes crossing a data-use, consent, privacy, or clinical boundary. `bioprism-onco`
carries a typed research boundary; do not route around it.
