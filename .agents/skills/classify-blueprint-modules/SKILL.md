---
name: classify-blueprint-modules
description: Decide which blueprint modules are code before writing any, using the five verdicts this workspace discovered, and cite ids without inflating the coverage figure. Use when opening a blueprint section, when a section looks badly uncovered, when deciding what a crate should claim, or before writing any NN.MM token into crates/ or docs/.
---

# Classify blueprint modules

The blueprint ships 973 content modules. `docs/COVERAGE.md` excludes 214 of them as programme prose
before counting anything, which leaves 759 code-bearing. **That subtraction is not the end of the
work — it is the beginning of it.** The most valuable thing agents did in this workspace was keep
going: reading a section's remaining modules and deciding, module by module, which of them a crate
could honestly implement.

Do this **before** writing code, and report the classification as the headline. `crates/stewardship`
opened §14 expecting eighteen modules of work and found six. That is a result, not a shortfall.

## The test

One question, from `crates/stewardship`, reused by every crate since and quoted in
`crates/residue/src/verdict.rs`:

> **Is the detailed design a set of predicates over an artifact, or a description of what people do?**

A `Council::vote()` would assert only that a council met. Software cannot know that. A module whose
strongest sentence is of that kind is not under-specified — it is not code.

## The five verdicts

`crates/residue/src/verdict.rs::Classification` is the settled vocabulary. Reach for it rather than
inventing a sixth name for the same distinction; two crates disagreeing about a module for reasons
that are entirely about wording is a cost with no benefit.

| Verdict | What it means | Where it was found |
|---|---|---|
| `Process` | describes what people do | `crates/stewardship`, 12 of §14's 18 |
| `ForeignArtifact` | code-bearing, precise, testable — and not Rust and not in this repository | `crates/devplat`, 7 of 20 across §11 and §19 |
| `DischargedElsewhere` | implemented under a different section's id, by a crate that never names this one | `crates/worldfactory`, §27 |
| `BlockLevelSplit` | the division runs *inside* the module, not between modules | `crates/bioevalx` (§26/§07), `crates/megafactory` (§35) |
| `GenuinelyUncovered` | nobody has read it, or it is real work not yet done | the honest default |

Four of the five say a module will never move. Only `GenuinelyUncovered` says work remains, which is
why `Classification::is_work_remaining` is named for the question rather than for the variant — a
report that collapses the five into one percentage loses exactly that distinction.

**Process.** `crates/stewardship/src/lib.rs` lists all twelve of §14's process modules by title:
councils, RFC stages, maintainer promotion, recusal, sponsorship disclosure, budget publication,
review cadence. Two of the twelve carry one code-bearing clause each and *both were already
implemented elsewhere* — 14.07's "repeated queries reduce holdout status" is `bioprism_lab::Holdout`'s
exposure ledger, and 14.13's author-certification rule is `registry`'s reviewer-independence check.
Neither was reimplemented. Look for that before you build: a single clause inside a process module is
usually somebody else's already-shipped invariant.

**Foreign artifact.** `crates/devplat` found the bucket `crates/ops` had needed for §40. Seven of its
twenty are the Python and TypeScript SDKs, the REST/gRPC/event APIs, the webhook stream and both
GitHub Action modules. Its consequence is the model for how to report one: *the only two onboarding
documents §11 actually writes out are entirely outside this repository.* Calling these process would
be as wrong as implementing them, and the difference decides what a contributor works on next.

**Discharged elsewhere.** The verdict a token scan structurally cannot produce.
`crates/worldfactory/src/coverage.rs` is the worked example: a machine-checked table of all 22 §27
modules, of which 14 are `Owner::Sibling` — `scale`, `mutation`, `stress`, `registry`, `hub` and
`hubapi`, every one of which built the content while citing §10, §32, §34 or §35 and never named a
§27 id. Seven are implemented in the crate itself and exactly one is `Owner::Unclaimed`, carrying its
reason: *"there are no real executions in this workspace to mine"*. Copy that shape. A gap that is
stated is a limitation; one implied to be filled is a lie.

Across the whole register, eleven verdicts name **their own author** as the discharger — a crate that
built the capability without ever citing the module.

**Block-level split.** `crates/bioevalx` found none of its sixteen §26/§07 modules was pure process:
every §26 file opens with a checkable purpose, target, numbered protocol and failure-mode set, and
closes with the same four process blocks ("Diagnostic outputs", "Required baselines", "Statistical
analysis", "Release gates"). The honest unit is the block. `crates/megafactory` then found §35 splits
a third way — per required *component*: each module's component list mixes checkable artifact
properties with things only an organisation can do, and the mixture differs every time. 35.07 is all
seven predicates because its taxonomy *is* a type; 35.06 has four of six as instrumentation a library
has no process to attach to. Both tables are in the crates' `lib.rs` and are worth reading before
classifying a section that resists a per-module answer.

**Contested is a state, not a tie to break.** Ten modules currently carry two readings: `atlasx` says
§33's remainder defines nothing once the shared blocks are stripped; `metrics` says the buildable
part is already built. `crates/residue` keeps both rather than adjudicating.

## The citation trap

This is the thing agents get wrong, and it is worth more attention than the classification itself.

`tools/coverage.sh` line 29 counts a module as covered when its `NN.MM` token appears **anywhere**
under `crates/` or `docs/`. Not in an implementation — anywhere. So:

> Writing "14.02 is a process module" into a crate raises the coverage figure by exactly as much as
> implementing 14.02 would have. Coverage then measures attention-while-reading and reports it as
> capability.

The fix is not to teach a scanner an exception the tool does not have. It is to **name the module by
its title in prose** and never in dotted form. `crates/stewardship` names all twelve; `crates/atlasx`
declined all ten of §33's remainder and cites zero §33 ids as a result; `crates/bioevalx` leaves the
BioCapability Atlas module uncited on purpose and says why.

Three mechanisms are available, in increasing strength:

1. **Prose discipline** — name by title. Fragile; nothing enforces it. `crates/interweave` stated
   exactly this as a fact with no mechanism, in the crate with the largest citation footprint in the
   workspace. Its claim turned out to be true, but nobody knew that until a test was written.
2. **A scan over the crate's own source** — reproduce the script's token rule and fail on an unbacked
   citation. `crates/devplat/src/citations.rs`, `crates/bioevalx/tests/citations.rs`,
   `crates/sweep/tests/citations.rs`, `crates/megafactory/tests/hygiene.rs`. Pair it with a proof it
   fires: see the `prove-a-scanner-fires` skill.
3. **A type that cannot hold the token** — the strongest, and the house preference. In
   `crates/devplat/src/classify.rs` only `Verdict::ImplementedHere` carries a `module_id`; the other
   three variants carry a `title` and nothing else, so recording "this is process" and writing the
   counted token are not simultaneously expressible. In `crates/residue/src/module.rs` a `ModuleKey`
   is a pair of integers with no string constructor and no `Display`, rendering a dotted id only at
   runtime, and it serialises as `{"section": 11, "index": 4}` for the same reason.

### Write figures to one decimal place

`crates/devplat/src/citations.rs` records the reason: the coverage script does not distinguish a
citation from a decimal number, so **a percentage written to two decimal places whose integer part
falls in the section range is a citation as far as the tool is concerned**. A workspace audit later
found four such numbers live in the tree. None was a real module id, so none inflated the count that
day, and each was one blueprint-numbering accident from doing so. Every measurement in `devplat` is
therefore written to one decimal place, and `tests/classification_and_citations.rs` asserts a rounded
figure scans clean.

The same rule catches version strings: `1.04.04` contains `04.04` and the script counts it. Two
crates' own scanners once disagreed with the script about this — see `prove-a-scanner-fires`.

### A guard must be protected from its own subject matter

Three times in this workspace a mechanism has cited the very ids it existed to keep uncited:

- **`docs/BACKLOG.md` emptied itself.** Its first run wrote 287 uncovered ids into `docs/`, which
  made every one of them match the citation scan, so the second run found zero remaining and emptied
  the file. `tools/coverage.sh` would have started reporting 100% for the same reason. Both scripts
  now carry `--exclude=BACKLOG.md` (`tools/coverage.sh` line 30) — *a metric that reads its own
  output is not a metric*.
- **`crates/sweep`'s citation test cited what it declared uncited.** The array asserting two module
  ids must never appear listed them as string literals, so the test proving they were absent was
  itself the citation. They are now assembled from digit pairs at run time
  (`the_scanner_sees_a_planted_violation`).
- **`crates/residue` caught its own author twice** — a literal in a test fixture, and a doc paragraph
  that spelled an id out *to illustrate what the serialized form must not look like*. Its scan
  caught both before they landed. That paragraph, and the incident, are still in
  `crates/residue/src/module.rs`.

Note the asymmetry: `residue` had a mechanism and survived; `BACKLOG.md` had none and destroyed
itself. Build the mechanism first.

**This file is safe.** Both scripts scan `crates/` and `docs/` only, so `.agents/` is outside the
walk and the dotted ids above cost nothing. Do not carry that licence into a crate or a doc.

## What to report

The classification, with counts, as the commit's headline — `feat(<crate>): twelve of §14's eighteen
modules are process, not code`. Then, in the crate's `lib.rs`:

- the test you applied and the answer per module, by title where the verdict is not
  `ImplementedHere`;
- for `DischargedElsewhere`, the crate that owns it and the id it cites instead;
- for anything declined, the sentence saying what would be duplicated if you built it anyway;
- for anything genuinely uncovered, either the blocker or the survey you actually read — `residue`'s
  `UncoveredStanding` makes "nobody has read this" unassertable without naming the crates searched.

Then add the verdict to `crates/residue`'s register, which reconciles against `docs/BACKLOG.md` by
test so the two cannot drift apart silently.

100% is not the target and never was. The honest end state is a backlog whose residue is explained
rather than empty — and it is deliberately kept off zero, because a register reporting no work
remaining while the workspace has no sandbox would be the flattering answer.
