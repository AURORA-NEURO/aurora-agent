# Planning and checking an issue repair

How `bioprism-repair` turns the evidence region [PROJECT_MODELING](PROJECT_MODELING.md) compiles
for one issue into a typed **repair plan**, and later checks a claimed repair against that plan's
own declared criteria — three-valued, staleness-aware, and without ever claiming the issue is
fixed. Nothing in the crate edits a file, runs a test, or executes anything: it plans and it
checks.

**No blueprint module is cited, and no module id is written under `crates/repair/` at all.** Repair
planning over a scanned software project is beyond the BioPRISM blueprint's scope, exactly as
`bioprism-domain` and `bioprism-project` are, and the crate's `lib.rs` says so rather than
stretching a module id. The nearest neighbour is section 39's staleness and recomputation module —
named by title, because `tools/coverage.sh` and `tools/status.sh` count a dotted `NN.MM` token
anywhere under `crates/` and cannot tell a citation from a sentence disclaiming one, so writing the
id inside the disclaimer would put the section in the README's derived Blueprint column for this
crate. `bioprism-tokens` implements that module and holds the id;
`crates/repair/tests/citations.rs` reproduces the script's token rule over this crate's own files so
the claim is checked rather than asserted, and proves the scanner fires before trusting a clean
result. Every statement below is checked in `crates/repair/src/*`,
`crates/repair/tests/repair_plans.rs` or `crates/repair/tests/citations.rs`.

## What a repair plan is

A plan is a human's declaration of **what would count as having fixed this issue**, written down
before the work and checkable afterwards. It carries:

- `issue_id` and `goal` — the goal is the issue title **verbatim**. The plan never paraphrases it,
  because nothing in this crate reads prose and a restatement would be an inference wearing a
  human's authority.
- an `evidence_binding` — the world it was planned from (below);
- three lists of named, predicate-backed items — criteria, obligations, falsifiers (below);
- `limitations` — including one mandatory line, verbatim, that no plan may drop;
- a content-derived `plan_id`: `repair-<issue id>-<12 hex digits of the plan body's digest>`.

`RepairPlan` has private fields and `RepairPlan::admit` is its only constructor, so the
admissibility gate cannot be routed around by building the checked type directly — the same gating
`bioprism_foundation::contract::FalsifiableContract` uses. `RepairPlan::from_json` re-derives the
id and refuses a document whose declared id its body does not hash to, so a plan edited after
minting cannot keep its name (`a_plan_whose_body_was_edited_after_minting_fails_its_own_content_derived_id`).

## A plan is bound to the evidence it was made from

`EvidenceBinding` records four things: the `world_id`, the world's `world_sha256`, the compiled
region's `region_fact_ids` (ascending, deduplicated, so two callers holding the same region derive
the same plan id), and the `query_sha256` of the query that compiled it.

`verify()` checks that binding **first** and, when it does not hold, returns
`AcceptanceReport::Stale` having evaluated nothing at all. Not "evaluates and flags": a verdict
computed against a different world is not a verdict about this plan, and a report carrying both a
verdict and a staleness flag invites a reader to take the verdict and skip the flag. A stale
report's `outcome()` is `None` — it has no verdict rather than a neutral one — and its item list is
empty (`a_plan_binds_the_region_it_was_planned_from_and_refuses_to_verify_against_a_different_world`).

This bites immediately, because a project world id is derived from the file listing: **any edit
produces a different world**, so `verify()` reports `Stale` for exactly the situation the tool
exists for. That is correct, and it is why `verify_successor` exists. It takes a `Succession` — a
named person's assertion that this new world is the repaired successor of the planned one.
`Succession::declare` refuses an empty declarant or an empty statement, because "someone said so"
with nobody saying it is the shape of an unowned claim. The assertion rides on the report verbatim,
together with a limitation stating that it was asserted and never verified: nothing here can know
that two scanned trees are the same project before and after a change. There is deliberately no
path to a verdict against a different world without a name attached to the claim that it is the
right one. A succession declared for the planned world *itself* is recorded as declared and not
relied on, rather than as the world having differed: a report may not state the opposite of its own
`binding_matches` field
(`a_succession_declared_for_the_planned_world_itself_is_not_reported_as_a_different_world`).

The report also names, in `missing_region_facts`, the bound region's fact ids that no longer exist
in the verified world — the region binding is reported on, never used to blind the checker.

It names one more loss on the way in. `World::from_json` does **not** refuse a world in which two
facts provide the same variable — shadowing is an error the separate reference validator
`bioprism_world::validate` raises, and the value map this crate builds reproduces the reference
runtime's behaviour of letting the last fact in document order win. So a criterion reading a
shadowed variable was checked against one of two candidate values rather than against the world,
and every evaluated report names the shadowed variables in its limitations instead of letting the
collapse pass as an ordinary check
(`a_world_providing_one_variable_from_two_facts_has_the_collapse_named_on_the_report`).

## Three kinds of item, and why they are three types

| kind | asks | where it lands |
|---|---|---|
| `AcceptanceCriterion` | did the change **achieve** the goal? | decides `Outcome` |
| `Obligation` | was the change **admissible to make**? | decides `Admissibility`, on its own axis |
| `Falsifier` | is this **the wrong plan**? | a met falsifier decides `Outcome` outright |

Collapsing criteria and obligations would force one of two lies: an unmet prerequisite reported as
a failure to achieve the goal (it is not — the goal may well have been reached by a change that
should not have been made), or a met prerequisite inflating the count of criteria that held. What
the crate genuinely cannot do is observe the "before" moment, so an obligation is checked
**retrospectively**, which is weaker than the plan asserted. That weakness is stated on every
report rather than papered over by merging the types
(`obligations_stay_out_of_the_outcome_and_are_reported_on_their_own_axis`).

Item names are unique across all three lists together, not within each list, because the report is
one flat list of named statuses and a duplicate would make a reader unable to say *which* item
could not run — the exact question this crate exists to answer.

### Why a plan with no falsifier is refused

`bioprism_foundation::contract::FalsifiableContract::admit` refuses a contract whose falsifier list
is empty, and its module documentation gives the blueprint's reason (24.07): such a contract is
"a benchmark that cannot be failed". A repair plan is the same object wearing different clothes. A
plan that declares only criteria declares only ways to succeed; nothing in it could ever come back
and say *this plan was the wrong plan*. `RepairPlan::admit` therefore returns
`RepairError::NoFalsifier`, whose message says so in full
(`a_plan_with_no_falsifier_is_refused_at_construction`). An empty criteria list is refused for the
adjacent reason: a plan that declares nothing to check would verify as met against any world.

Having a falsifier and having a falsifier that **could realistically fire** are still different
states, and the generator says which it produced — see the walkthrough.

## Origin: derived is not declared

Every item carries an `Origin`, `Derived` or `Declared`. A derived criterion is a proxy for
something the release pack could see; a declared one is a claim a person is accountable for. The
tool must never present its own inference as a human's assertion, and on the generation path that
is enforced by the types rather than by care: a caller hands `plan_for_issue` a `DeclaredItem`,
which has **no origin field at all**, so nothing a caller supplies can arrive pre-labelled and the
generator is the only thing that can stamp `Derived`.

The guarantee is exactly that wide and no wider. `RepairPlan::admit` and `RepairPlan::from_json`
accept whatever origin a hand-built draft or a parsed document carries, because a document that
already exists says what it says and this crate does not get to overrule its author.

A declared item reusing a derived item's name is refused as a duplicate rather than merged into or
over it
(`a_derived_criterion_is_marked_derived_and_a_declared_one_declared_and_neither_absorbs_the_other`).

## Three-valued acceptance

Each item's status is `Met`, `Unmet`, or `NotEvaluable(Obstruction)`, and the third is **never**
folded into either of the others. This is `bioprism_domain::Predicate`'s existing strong
three-valued `evaluate()`, reused rather than reimplemented: the same `Obstruction` that tells a
rule oracle a check did not run tells a reader here which criterion could not be checked and why —
it names the variable and the reason (`"absent from the compiled value map"`, or a wrong-type
message). There is no second predicate language in this crate and no `is_met_or_default`;
`ItemStatus` exposes `obstruction()` and nothing that supplies a default. A report document that
claims `not_evaluable` without naming an obstruction is refused by the reader, because the whole
point of the third value is that it says what blocked the check
(`a_report_claiming_not_evaluable_without_an_obstruction_is_refused`).

The aggregate is held to the items the way the plan id is held to the plan body. `outcome` and
`admissibility` are total functions of the item list, so `AcceptanceReport::from_json` rederives
both and refuses a document declaring one its own items do not produce. The document that refusal
exists for is a hand-edited report reading `"outcome": "met"` beside an item that never ran
(`a_report_whose_declared_outcome_its_own_items_do_not_produce_is_refused`).

The predicate language is `bioprism-domain`'s fifteen kinds unchanged — `exists`, `missing`,
`nonempty`, `equals`, `not_equals`, `contains`, `number_at_least`, `number_below`,
`string_before`, `string_after`, `has_key`, `count_at_least`, `not`, `all_of`, `any_of` — with
strong Kleene connectives: `all_of` returns `false` as soon as a limb is determinately false even
if another limb was obstructed, and only reports the obstruction when no limb settled it.
`exists` and `missing` are the two total predicates: they cannot be obstructed, which is why the
generator uses `exists` where it needs a determinate failure.

`bioprism-domain` shipped a strict reader for predicates and no writer, because packs are authored
by hand. A generated plan needs one, so `predicate_json` supplies it here — with the pairing
checked rather than asserted, every kind round-tripping through `bioprism-domain`'s own reader
(`every_predicate_kind_a_plan_can_carry_survives_the_domain_readers_round_trip`). One value is
refused rather than encoded: a non-finite numeric bound, which `serde_json` would write as `null`
and the reader would parse back as an absent threshold.

### The outcome ordering

`Falsified` > `Underdetermined` > `NotMet` > `Met`.

1. **`Falsified`** — some falsifier held. It outranks everything, including `Underdetermined`,
   because of an asymmetry in what the verdicts presuppose: `Falsified` is not a claim about the
   criterion set at all, but a single determinate observation that the plan was the wrong plan. A
   plan proven wrong does not become less wrong because one of its criteria could not be checked
   (`a_met_falsifier_decides_the_outcome_whatever_the_criteria_said`).
2. **`Underdetermined`** — some criterion or falsifier could not be evaluated. This outranking
   `NotMet` is the ordering worth arguing about, since the workspace's rule elsewhere is that a
   proven violation outranks a blind check. The difference is again what the verdict presupposes.
   `NotMet` says *the criteria were checked and not all held*, and a reader told that may
   reasonably conclude that clearing the failures is the whole remaining distance to `Met`. When a
   criterion never ran, that conclusion is false. `Underdetermined` refuses the inference, and
   nothing is lost by it: every determinate failure is still on its own item as `Unmet`, which is
   where a reader acts from. An unevaluable *falsifier* lands here too — nobody checked whether the
   plan is wrong, which is not the same as the plan being right
   (`a_falsifier_that_could_not_run_leaves_the_outcome_underdetermined_not_met`,
   `an_unevaluable_criterion_outranks_an_unmet_one`).
3. **`NotMet`** — everything was evaluated and some criterion did not hold.
4. **`Met`** — every declared criterion held, with nothing blind. It means exactly that and
   nothing more.

Obligations are **not** in this ordering
(`an_obligation_never_moves_the_achievement_outcome`). They report on `Admissibility`:
`Undeclared`, `Undetermined`, `Violated`, `Held`. `Undeclared` is a real state and not a synonym
for `Held` — a plan that declared no prerequisites has declared none, which is different from
having declared that none are needed
(`a_plan_with_no_obligation_reports_undeclared_rather_than_held`).

## What the generator derives, and what it cannot

`plan_for_issue(world, pack, issue_id, region_certificate, options)` takes the *certificate* of a
compiled issue query rather than the compiler's output, so the crate does not link the engine —
the same reason `bioprism-section` depends on neither `world` nor `fiber`. It refuses a
certificate that is not about this world: a plan bound to a region compiled from something else is
bound to nothing (`a_region_certificate_compiled_from_another_world_cannot_bind_a_plan`).

It derives three things and invents no fourth:

- **One criterion per fired release check that reads a variable in the issue's region.** Checks
  that did not fire produce nothing: asking a repair to keep clean something already clean would be
  a criterion nobody declared. The predicate is the check's **own predicate under
  `Predicate::Not`**, not a copy with an "expected outcome" flag flipped — the load-bearing choice.
  `Not` evaluates as `Ok(!inner?)`, so an unevaluable limb propagates and a check that could not
  run yields a criterion that could not run. An "expected outcome" encoding would have to decide
  what an unevaluable check means, and the tempting default — "did not fire" means "passed" — is
  precisely the lie this crate refuses. A check that was *already* unevaluable when the plan was
  made yields no criterion and a limitation naming the obstructed variable.
- **One criterion per component the issue declares, asserting it still exists.** Deleting the file
  is the cheapest way to stop a static check firing over it. `Predicate::Exists` is total, so a
  vanished component is a determinate `Unmet` rather than a `NotEvaluable` a reader could shrug at.
- **One falsifier, `region_evidence_removed`**, over the decisive set: every variable the derived
  criteria read, watched with `any_of` over `missing`. If any of them is gone, the plan's premise —
  that this region is the evidence for this issue — is false about that world.

Nothing else. **No obligation is ever derived**: whether a change is admissible to make is a
judgement about process, and the scan sees none of it.

## Worked walkthrough: demo-app

Against `fixtures/projects/demo-app/` (world `project-2729a9712754`), whose audit fires exactly one
check, `unpinned_dependency`, over `loose-gadget = "1.0"`. These are values from an actual run of
`crates/repair/tests/`, stable as long as the fixture is unchanged.

### The generated plan for `ISSUE-1`

`ISSUE-1` declares `src/lib.rs`, so its region carries `fact.component.src`, `fact.issue.ISSUE-1`
and the six aggregate decision inputs. `plan_for_issue` with default options produces
`repair-ISSUE-1-d1729223b27c`, whose goal is the issue title verbatim — `"Fix the gadget seam"` —
and which is abridged here to its structure:

```json
{
  "schema_version": "bioprism-repair-plan/0.1",
  "issue_id": "ISSUE-1",
  "goal": "Fix the gadget seam",
  "evidence_binding": {
    "world_id": "project-2729a9712754",
    "world_sha256": "2948750afa89365b02483fe3eea68b99be727ecac1a72d908aaf11a166d90dd5",
    "region_fact_ids": [
      "fact.aggregate.ci_workflow_inventory", "fact.aggregate.dependency_declarations",
      "fact.aggregate.scan_loss_summary", "fact.aggregate.test_function_total",
      "fact.aggregate.todo_marker_total", "fact.aggregate.unpinned_dependencies",
      "fact.component.src", "fact.issue.ISSUE-1"
    ],
    "query_sha256": "0f1fb5f48cb761213e19034049baa962869fa723e89af3015a5de0639c236c05"
  },
  "criteria": [
    { "name": "check_cleared:unpinned_dependency", "origin": "derived",
      "predicate": { "kind": "not",
        "predicate": { "kind": "nonempty", "variable": "unpinned_dependencies" } } },
    { "name": "component_present:src", "origin": "derived",
      "predicate": { "kind": "exists", "variable": "component_src_inventory" } }
  ],
  "obligations": [],
  "falsifiers": [
    { "name": "region_evidence_removed", "origin": "derived",
      "predicate": { "kind": "any_of", "predicates": [
        { "kind": "missing", "variable": "component_src_inventory" },
        { "kind": "missing", "variable": "unpinned_dependencies" } ] } }
  ],
  "plan_id": "repair-ISSUE-1-d1729223b27c"
}
```

Each item also carries a `statement` in prose — the check's own description, caveats included —
so a status quoted alone still says what was checked, and each criterion carries a `rationale`
saying why it is in the plan. The seven `limitations` lines are omitted above; they are the
mandatory one, the five standing gaps, and "This plan declares no obligations."

Planning twice from the same world yields byte-identical documents and the same id
(`planning_twice_from_the_same_world_yields_byte_identical_documents`).

### Verifying the unchanged tree

`verify(plan, world)` against the tree the plan was made from — nothing repaired — reports:

```
repair-ISSUE-1-d1729223b27c not_met (admissibility undeclared) [criterion
check_cleared:unpinned_dependency=unmet; criterion component_present:src=met; falsifier
region_evidence_removed=unmet]
```

`binding_matches` is `true`, `missing_region_facts` is empty, and the report's `limitations` carry
the plan's seven verbatim plus three of its own, including that the report *does not state that the
issue is resolved*. The tool does not congratulate a tree nobody changed
(`verifying_the_unchanged_demo_app_against_its_own_plan_reports_not_met`).

### Verifying a tree that "cleared the check" by deleting the component

The tests copy the fixture with `src/` removed, assemble that tree, and verify it under a declared
`Succession`. All three statuses appear at once:

| item | status |
|---|---|
| `component_present:src` | `unmet` — `exists` is total, so a vanished component fails determinately |
| a declared `nonempty` criterion over `component_src_inventory` | `not_evaluable`, obstruction `component_src_inventory` / `absent from the compiled value map` |
| `region_evidence_removed` | `met` |

The outcome is `Falsified`: a met falsifier outranks both the unmet criterion and the unevaluable
one (`a_met_falsifier_outranks_unmet_and_unevaluable_criteria`). Even the one-line `summary()`
names `component_src_inventory` as the variable that stopped a check, and
`missing_region_facts` names `fact.component.src`
(`the_report_names_every_items_status_including_the_obstruction_that_stopped_it`).

### An issue with no component: a falsifier without teeth

`ISSUE-2` declares no components, so its region is the aggregates alone and the only derived
criterion is `check_cleared:unpinned_dependency`. Its decisive set is therefore
`{unpinned_dependencies}` — a variable the world assembler emits unconditionally, empty or not.
The derived falsifier can essentially never hold, and the plan says so in its own limitations
rather than passing the gate quietly:

> Every variable this plan's derived falsifier watches is one the world assembler emits
> unconditionally, so the derived falsifier is very unlikely ever to hold. Treat this plan as
> effectively carrying no derived falsifier and declare one with real teeth.

`ISSUE-1`'s plan does *not* carry that line, which is what keeps it a finding rather than
boilerplate (`a_derived_falsifier_that_watches_only_unconditional_aggregates_says_it_has_no_teeth`).

## What this deliberately does **not** do

Restating the crate's `lib.rs` list, because a missing capability that is stated is a limitation
and one that is implied to exist is a lie.

- **No editing.** Nothing writes a source file, applies a patch, or suggests a diff.
- **No execution.** Nothing is built, run or tested. A criterion about tests is a claim about the
  *scan* — `bioprism-project` counts `#[test]` substrings — and **a counted test is not a passing
  test**. A plan whose every criterion is met may sit on a tree that does not compile.
- **Meeting every criterion is not proof the issue is resolved.** This is the mandatory limitation
  every plan carries verbatim and `admit` refuses a plan without: the criteria are the author's
  declaration of what would count as evidence, and the gap between that declaration and the issue
  itself belongs to the author. No tool closes it. Verification reports which declared criteria
  held, and nothing more.
- **Derived criteria are proxies for what the pack could see**, not for what the issue means. The
  generator never reads the title or body as language — which is also why the goal is copied
  verbatim.
- **No criterion asserts the reported symptom is gone.** A repair satisfying every derived
  criterion may leave the behaviour exactly as it was.
- **No regression guard.** Checks the pack currently judges clean produce no criterion, so a plan
  cannot notice a repair that breaks one of them.
- **No obligation is ever derived**, and `Undeclared` is not `Held`.
- **No multi-issue interaction.** Two plans over one tree do not know about each other, and
  nothing detects that satisfying one would break the other.
- **No cost, effort, ordering or risk.** This is not a work plan and proposes no steps.
- **No recompiled region at verification time.** Criteria are evaluated against every fact in the
  world, so no budget is applied and the report is not evidence about what a region compiler would
  have delivered. The reason is argued in `verify()`: blinding the checker to the world would make
  "the compiler judged this irrelevant" and "the variable is gone" arrive as the same unevaluable
  status, and those are different states.
- **The succession is never verified.** `verify_successor` records a person's claim; it cannot
  check it.

## Using it

The crate API is the entry point; there is no `serde` derive anywhere in it, because
`bioprism_domain::Predicate`'s canonical form is defined by a hand-written strict parser.

```rust
use bioprism_repair::{plan_for_issue, verify, PlanOptions};

let plan = plan_for_issue(&world, &pack, "ISSUE-1", &certificate, &PlanOptions::default())?;
let report = verify(&plan, &world);
println!("{}", report.summary());
```

`PlanOptions` carries `declared_criteria`, `declared_obligations`, `declared_falsifiers` (each a
`DeclaredItem`, which cannot claim an origin) and extra `limitations`, appended after the
generator's own and never replacing them. Both documents have strict readers —
`RepairPlan::from_json` and `AcceptanceReport::from_json` — which refuse undeclared keys rather
than ignoring them (`a_plan_document_with_an_undeclared_key_is_refused_rather_than_ignored`).
Schema versions: `bioprism-repair-plan/0.1` and `bioprism-repair-report/0.1`.

### CLI

Two subcommands, parsed in `crates/cli/src/args.rs` and dispatched in `crates/cli/src/main.rs`,
pinned by `crates/cli/tests/repair_contract.rs`.

```
project plan      --root <dir> --issues <path> --issue <id> [--decision-time <rfc3339>]
                  [--criteria <path>] --out <path> [--dry-run]
project verify    --root <dir> --plan <path> [--issues <path>] [--decision-time <rfc3339>]
```

`project plan --json` reports the bound region as `region_fact_ids` (the list) with `region_facts`
beside it (the count), which is the convention [PROJECT_MODELING](PROJECT_MODELING.md) records for
this surface and what `project audit` already emits — the list is under the plan document's own
field name, which is also what `repair_plan` returns.

`--issues` is *required* on `project plan` although `project ingest` and `project audit` both take
it optionally: those two have something to say about a tree with no declared issues, but a plan is
*for* one issue, so an invocation naming none has no subject. Defaulting to an empty issue list
would turn a missing flag into "issue not found in the world" — a diagnostic pointing at the tree
rather than at the operator's command.

`--criteria` loads a `bioprism-repair-declarations/0.1` document — a surface-level *authoring*
format, not one `bioprism-repair` defines — whose criteria, obligations and falsifiers are recorded
as `declared`; the generator's own items stay `derived` and the two never merge, and a declared name
colliding with a derived one is refused rather than absorbed. Undeclared keys are refused rather
than ignored: a misspelled `falsifier` that was silently dropped would produce a plan whose missing
falsifier the author would then be blamed for. A declared *criterion* must carry a `rationale`,
which obligations and falsifiers do not: `AcceptanceCriterion` is the one item type with the field,
and a criterion is what a plan marks `declared` in order to say somebody is accountable for it.
The reader is written twice — `read_declarations` in `crates/cli/src/main.rs` and
`Server::repair_declarations` in `crates/mcp/src/server.rs` — because the format belongs to neither
surface and `bioprism-repair` does not define it; the duplication is a real cost and is recorded
here rather than hidden.

The exit codes for `project verify` branch on the verdict: **1** when the outcome is
`not_met` or `falsified`, **8** (`Indeterminate`) when a criterion or falsifier could not be
evaluated, and **9** (`Stale`) when the plan is bound to a different world — a stale plan evaluated
nothing, so it is not a failed verification. Obligations never move the exit code; they are
reported on their own admissibility axis.

**`project verify` cannot give a repaired tree a verdict.** A project world id is derived from the
file listing, so any edit produces a different world and the command reports `stale` — correctly,
and unhelpfully for the case the feature exists for. `verify_successor` is what that case needs and
no flag mints a `Succession`, so this is a gap in the command rather than in the crate. It is
stated here and in `project_verify`'s own documentation rather than worked around by verifying
against the new world and calling the difference immaterial.

### MCP

`repair_plan` and `repair_verify`, both root-confined exactly as `project_ingest` and
`project_audit` are: every path parameter — `root`, `issues`, `criteria`, `plan`, `out` — resolves
through the server's root confinement, so planning a repair cannot become a way to read or write an
arbitrary file. The catalogue is 264 tools.

`repair_plan` mirrors `project plan`, including the `--criteria` document, and follows the server's
write-preview convention: with `out` but without `confirm: true` it names the exact path it would
create and writes nothing, and `preview.writes` is built from the same expression as `written` so a
caller cannot approve one effect and receive another
(`repair_plan_previews_exactly_the_path_confirming_writes_and_creates_none_of_it`). `performed` is
`null` when no `out` was named, `false` when one awaits confirmation, `true` when the file is on
disk — three states, because collapsing "nobody asked for a write" into "a write was declined"
reports a refusal that never happened.

`repair_verify` writes nothing and returns the crate's report verbatim under `report`, with
`stale`, `outcome` and `admissibility` lifted to the top level for a caller that only branches.
A stale report arrives as a **successful** call carrying a report that says `stale`, not as a
transport error: a finding routed through an error is a finding a caller discards with `?`. Its
`outcome` and `admissibility` are `null` and it carries no item list at all
(`repair_verify_reports_staleness_against_a_different_world_without_evaluating_anything`). The same
succession gap the CLI has applies here: no argument mints a `Succession`, so a repaired tree
reports `stale` rather than receiving a verdict.
