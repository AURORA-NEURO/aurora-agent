# Modeling a software project

How `bioprism-project` compiles an entire software project into a fiber-world, so the FIBER
pipeline can judge its release readiness and compile the minimal decision-sufficient context for
working its issues. This is the worked large-scale example of what
[GENERALIZATION](GENERALIZATION.md) describes in the small: the pipeline is domain-neutral, and a
project tree is just another world once an adapter has said honestly what it read and what it
skipped. The one blueprint module the crate implements against is **40.17, the data adapter
contract** — project modeling is beyond the biological blueprint's scope, and citing anything else
would inflate coverage. Every statement below is checked in `crates/project/src/*` or
`crates/project/tests/project_worlds.rs`.

## What project modeling is

A project becomes a fiber-world in three steps, each usable alone:

1. **Scan** (`ProjectScan::scan`) — a deterministic, std-only walk of the tree that produces a
   typed scan *and* a sealed `bioprism_adapter::Ingestion`: one fact per file, one per manifest,
   and the mandatory semantic-loss audit. `ProjectAdapter` implements
   `bioprism_adapter::Adapter` directly — a project root fits the `Source::directory` locator
   exactly as `InventoryAdapter`'s repositories do — and it walks via the adapter crate's own
   `probe::walk`, so the independent conformance probe and the scanner can never disagree about
   enumeration. `conformance::certify` verifies the sealed contract against that independent
   probe in the tests.
2. **Assemble** (`ProjectWorld::assemble`) — a `fiber-world/0.1` document of component
   inventories, aggregate decision inputs, per-issue facts and factors, together with the
   dimension document, the `project-release-readiness` pack and generated `fiber-query/0.2`
   queries.
3. **Audit** (`audit`) — assembles and runs `bioprism_fiber::compile_with_oracle` end to end,
   returning the verdict with its checkable witnesses.

The defining move is that **the loss ships into the world**. The scan's semantic-loss report is
not a side channel: its per-kind counts become the `scan_loss_summary` fact, tagged protected, and
the release oracle *requires* that variable before any check runs. A verdict computed without
seeing what the scan skipped would be a verdict about a tree nobody scanned.

## The scanner's honesty envelope

Everything the scanner reports is a **static textual proxy**, and everything it does not read is a
declared `LossEntry` naming its `SourceLocation` — never a silence.

**Read narrowly, with per-line loss declarations:**

- `Cargo.toml` — the `[workspace] members` array and, inside the four plain dependency sections
  (`dependencies`, `dev-dependencies`, `build-dependencies`, `workspace.dependencies`), the two
  common forms `name = "req"` and `name = { ... }` with a narrow `version = "..."` extraction.
  Every other line is a loss entry at its line number, `Degrading` when the line sits in a
  dependency-shaped section the reader cannot parse. An inline table whose version cannot be
  extracted records the requirement as absent with a declared gap, never a guess.
- `package.json` — via serde_json: `dependencies` and `devDependencies` (name → requirement
  string) and `scripts` key names. Every other top-level key is a declared loss.
- `pyproject.toml` — the `[project] dependencies` array only.

"Pinned" is defined narrowly and stated on `DependencyRecord`: `=`-prefix for Cargo, exact
three-part numeric semver for package.json (prerelease and build suffixes are not pinned), `==`
for pyproject. A declaration with no version requirement at all (a path or workspace dependency)
is *neither* pinned nor unpinned and never enters `unpinned_dependencies`. No lockfile is
consulted and no registry is contacted: a requirement string is a declaration, not a resolved
version.

**Inventoried, uninterpreted — each with its own declaration:**

- Files under `.github/workflows/` are inventoried as CI presence, and the loss entry says the
  quiet part: content is never interpreted, so a workflow that does nothing would still satisfy
  the presence check.
- Every other UTF-8 file is read for line and marker counts only, and its loss entry says the
  content received no semantic reading.
- Binary (non-UTF-8) files are hashed but declared uninterpreted; symlinks are recorded but never
  followed; files over the byte cap (`DEFAULT_MAX_FILE_BYTES`, 2 MiB) are named and sized but
  neither hashed nor read — and the entry states that every count that would have come from them
  is *missing, not zero*.
- The exclusion list `["target", "node_modules", ".git", "dist"]` applies at any path depth, and
  every excluded file is a per-file loss entry rather than a vanishing. The tests pin that an
  excluded file's only trace is its loss declaration, and that the independent conformance probe
  still accounts for it.
- A caller who supplies no upstream provenance gets a `ProvenanceUnavailable` loss: a filesystem
  mtime is not evidence about a project's origin.
- The sealed ingestion's `SourceManifest` measures the **scanned subset**, not the whole tree:
  both `byte_length` and `source_digest` skip excluded files and symlinks, so a build cache that
  grew by a gigabyte does not read as the project having grown and a `cargo build` does not read
  as the tree having drifted. `InventoryAdapter` sums every walked entry instead, so the two
  adapters' `byte_length` fields are not comparable. What is left out is not hidden — every
  excluded file carries its own loss entry.

**Not implemented, deliberately** (restating the crate's `lib.rs` list):

- **No execution.** A `#[test]` occurrence is a counted attribute in a `.rs` file, not an
  executed test. A counted test is not a passing test, and the `tests_absent` check's own
  description says it judges a substring count. For non-Rust files the count was never taken —
  `test_functions` is `None`, not zero, because "counted zero" and "could not count" must never
  share a representation.
- **No git history.** The working tree only; authorship, age and churn are absent, not zero.
- **No semantic code analysis.** `TODO`, `FIXME` and `unimplemented!` are case-sensitive
  substring counts: a `TODO` inside a string literal counts, a lowercase `todo!()` does not. The
  counts are proxies and every consumer is told so on the wire.
- **No network.** Nothing is resolved against a registry.
- **No semantic issue relevance.** An issue's evidence region comes from the components it
  *declares*, resolved syntactically; an unresolvable declaration is recorded on the issue fact,
  never guessed at.
- **No clocks.** The scan event and every generated query use one caller-supplied timestamp,
  defaulting to the fixed epoch `1970-01-01T00:00:00Z`, so two assemblies of the same scan are
  byte-identical — pinned by the determinism test.
- **No general TOML parser.** Cargo and pyproject manifests go through the narrow line readers
  described above, and every line they do not understand becomes a loss entry naming its line
  number. The loss report is the honesty valve, not a claim that the manifests were parsed.
- **No loss kind of its own.** Those unread manifest lines are declared under the borrowed
  `LossKind::UnmappedColumn`, because `LossKind` is a sealed vocabulary written for tabular
  sources and a manifest line is not a column; the reuse is argued on `ProjectAdapter::manifest`.
  The cost is on the record: a `losses_by_kind` total summed across adapters puts manifest lines
  and real unmapped columns in one bucket, and only each entry's `detail` and `location` tell
  them apart.
- **No collision guard on file variable names.** `component_<slug>_inventory` collisions fail
  assembly outright, but the ingestion's per-file `provides` names are slugged the same way
  (`path_slug` maps every non-alphanumeric to `_`), so `src/a-b.rs` and `src/a_b.rs` would both
  emit `file_src_a_b_rs`. Nothing in this crate consumes those names — the world is built from
  component, aggregate and issue facts — but a consumer loading the sealed ingestion into a world
  of its own must not assume they are unique.

## The assembled world

The world carries the *decision* layer, not the file layer: the per-file evidence stays in the
sealed ingestion, and component digests keep it addressable without copying ten thousand file
facts into every compile budget.

- **One `component_<slug>_inventory` fact per component.** A component is the nearest ancestor
  directory below the root that directly contains a recognized manifest, else the file's
  top-level directory, else the root — a stated syntactic rule, so `src/lib.rs` belongs to `src`
  even when `Cargo.toml` sits beside it at the root. Each inventory carries file, line, marker
  and test counts plus a content digest of the component's file listing. Two component
  directories whose slugs collide fail assembly rather than silently merging.
- **Ten aggregate facts**, scoped by `project` and by `scan` (the scanner ontology,
  `bioprism.project/0.1.0`): `dependency_declarations`, `unpinned_dependencies`,
  `test_function_total`, `todo_marker_total`, `ci_workflow_inventory`, `ci_workflow_count`,
  `source_file_total`, `uninterpreted_file_total`, `doc_inventory`, `scan_loss_summary`.
- **The protected set, chosen and stated**: the dependency declarations, the unpinned subset,
  the test inventory, the CI inventory, and the loss summary — the facts a project audit must
  never lose. Component inventories, marker totals and doc counts are colour: reachable through
  factors, droppable when irrelevant.
- **One fact and one factor per issue.** The issue fact records title, body, resolved components
  and — verbatim — the unresolved declarations. The `factor.issue_<id>_review` factor's inputs
  are the resolved components' inventories, the issue's own record, and the six aggregate
  decision inputs; its output is `issue_<id>_context_status`.
- **`factor.project_release_review`**, consuming the six decision inputs and producing
  `release_integrity_status`.
- **One scan event** producing all ten aggregates, at the caller-supplied time.
- **A content-derived world id**, `project-` plus 12 hex digits of the canonical file listing's
  digest: the same tree always assembles to the same id, a changed tree never reuses one.

The assembled document is re-validated through `bioprism_world::World::from_json` before it is
returned, and the dimension document classifies every dimension its scopes bind: `project` and
`issue` are identities, `component` is a region, `scan` is an ontology. No `manifest` dimension is
declared, because no fact is scoped by one — declaring a dimension nothing binds would be coverage
theatre.

## The pack: `project-release-readiness`

A `bioprism-domain/0.1` document (oracle kind `rule/project-release-readiness-v1`) whose four
checks are all violation detectors, and whose descriptions carry their caveats **on the wire**, so
a witness quoted out of context still says what it is a proxy for. `require` lists
`dependency_declarations`, `test_function_total`, `ci_workflow_inventory` and
`scan_loss_summary` — a compile that cannot deliver them abstains rather than judging.

| check | fires when | proxy for — and the stated gap |
|---|---|---|
| `unpinned_dependency` | `unpinned_dependencies` is nonempty | reproducible builds — but a declared requirement is not a resolved version, and no lockfile is consulted |
| `tests_absent` | `test_function_total` < 1 | a test suite existing at all — but zero counted means zero found by the `#[test]` substring proxy, not proof no test exists in another language |
| `no_ci` | `ci_workflow_inventory` is empty | continuous integration existing at all — content is never interpreted, so this fires only on total absence, and presence is not evidence of a working workflow |
| `todo_burden` | `todo_marker_total` ≥ threshold (default 50) | acknowledged unfinished work — an over- and under-counting substring proxy, and the threshold is a declared editorial default, not a measurement |

The `no_ci` check is `not` over `nonempty`: the predicate language as it exists can state
"empty", so nothing was extended and no shadow count was needed. A pack test asserts that every
check description declares itself a static proxy in some words.

## Issue-context compilation

Each issue's generated query targets `issue_<id>_context_status` and compiles, through the same
passes as any FIBER query, the **minimal decision-sufficient evidence region for that issue**.
Concretely that means: the inventories of the components the issue *declares*, the issue's own
record fact, and the aggregate decision inputs — protected closure included in full — and nothing
else. An issue naming `src/lib.rs` gets the `src` component inventory in its region and does not
get the `assets` inventory; an issue declaring no components compiles against the aggregates
alone. Both outcomes are pinned in `project_worlds.rs`, along with `dropped_protected` being
empty.

The limits are the declaration model's limits, and they are the point: relevance comes only from
what the issue declares, resolved syntactically (component directory, display name, slug, or a
path inside a component — longest directory prefix wins). There is no semantic search, no
similarity ranking, and no guessing: a declaration that resolves to nothing is carried on the
issue fact as `unresolved_components`, so a region that looks deliberately small can be told apart
from one starved by a typo.

## Worked walkthrough: demo-app

`fixtures/projects/demo-app/` is a small Rust project: a root `Cargo.toml` declaring
`exact-widget = "=1.0.0"` (pinned) and `loose-gadget = "1.0"` (not pinned), `src/lib.rs` and
`src/main.rs` (one `#[test]`), one workflow under `.github/workflows/`, a `README.md`, a non-UTF-8
`assets/blob.bin`, and an `issues.json` declaring `ISSUE-1` (components: `["src/lib.rs"]`) and
`ISSUE-2` (no components).

The actual audit outcome, pinned in `project_worlds.rs`:

- The verdict is **`Invalid`** under `rule/project-release-readiness-v1`, with exactly one fired
  check: `unpinned_dependency`. Its witness's observed bindings name `loose-gadget` with the
  requirement `1.0` and do **not** contain `exact-widget`; the detail carries the
  resolved-version caveat. The fixture has one counted test and one workflow, so `tests_absent`,
  `no_ci` and `todo_burden` stay silent.
- `ISSUE-1`'s compiled region contains `fact.component.src` and `fact.issue.ISSUE-1` and excludes
  `fact.component.assets`; `ISSUE-2`'s region contains no component inventory at all, only the
  aggregates. Neither drops a protected fact.
- Two scans of the tree produce byte-identical ingestions, and two assemblies produce
  byte-identical worlds with the same `project-…` id.
- Every file carries at least one loss declaration (a scan can never quietly claim a file was
  fully understood); the binary asset is declared not-UTF-8 at its path, and at least one
  unparsed `Cargo.toml` line is declared at its line number.

The second fixture, `fixtures/projects/bare-script/` (one Python script, a `pyproject.toml` with
`requests>=2.0`, no tests, no CI), pins that checks stack: `tests_absent` fires showing the
counted zero, `no_ci` fires showing the empty inventory `[]` rather than a fabricated count, and
`unpinned_dependency` fires alongside them.

## Using it

The crate API is the entry point:

```rust
use bioprism_project::{audit, AuditOptions};
let report = audit(std::path::Path::new("."), &AuditOptions::new("my-project"))?;
println!("{}", report.summary());
```

`AuditOptions` carries the scan policy (`ScanOptions`: project name, byte cap) and the assembly
context (`AssemblyOptions`: decision time, issues loaded via `Issue::load` from a strict JSON
issues file, thresholds). The report carries the status, the witnesses verbatim, fact and
selection counts, and the loss counts by kind. Issue parsing is strict — an undeclared key, a
malformed id or a duplicate id is refused, because ids are spliced into variable names.

### CLI

Two subcommands, as declared in `crates/cli/src/args.rs`:

```
project ingest    --root <dir> [--issues <path>] [--decision-time <rfc3339>]
                  --world-out <path> --pack-out <path> --dimensions-out <path>
                  [--queries-out <dir or .json path>] [--dry-run]
project audit     --root <dir> [--issues <path>] [--decision-time <rfc3339>]
```

`--queries-out` ending in `.json` writes one container document (`release` plus `issues` keyed by
issue id, each member a `fiber-query/0.2` document); any other path is a directory of
`release.json` plus `issue-<id>.json`. The flag chooses where the queries go, never which of them
survive. `--dry-run` reports every planned write, its byte count, and creates nothing — not even
the output directory. `--decision-time` is gated by the workspace's own RFC 3339 parser at the
flag, so a malformed timestamp is a usage error naming the flag rather than the emitted world
failing the reference validator.

`project audit` exits 1 when the verdict is `invalid` — the run completed and the property it
checked does not hold — and 5 when the root cannot be read. Both commands print the scan's loss
totals by kind; the audit also prints each witness with the bindings it read and each issue's
compiled region fact by fact.

### MCP

Two root-confined tools, `project_ingest` and `project_audit`. Both take `root` and optional
`issues` and `decision_time`; every path parameter goes through `Server::resolve`, which refuses
absolute paths, `..` on either separator, and symlinks leaving the root. `project_ingest` also
takes `out_dir` and follows the server's side-effect convention: without `confirm: true` it
previews the exact paths it would write and writes nothing. `project_audit` writes nothing at all
and returns the verdict, its witnesses verbatim, the loss summary by kind, and each issue's
compiled region with the declarations that produced it.

The two surfaces report the same audit, not the same JSON: `selected_facts` is a **count** in the
CLI and the **list of fact ids** in MCP, because each surface follows its own existing convention
(`context compile` counts; the MCP policy tools list). The CLI names an issue's region `region`
with `region_facts` beside it and carries the caller's raw `declared_components`; MCP names it
`selected_facts` and carries `resolved_components`. `unresolved_components` is on both, because
that is the field that tells a small region apart from a mistyped one.

## From an issue's region to a checkable repair plan

`bioprism-repair` takes the region compiled above for one issue and produces a typed repair plan
bound to it, then checks a claimed repair against that plan's own declared criteria — three-valued,
staleness-aware, and without ever claiming the issue is fixed. See
[ISSUE_REPAIR](ISSUE_REPAIR.md).

## Dogfood: this repository, judged by its own pack

The ignored test `dogfood_the_repository_scans_assembles_and_is_judged_by_its_own_pack` scans
this worktree itself (run:
`cargo test -p bioprism-project --offline -- --ignored --nocapture`).

**Two of the numbers it prints cannot be reproduced from this page, and saying which is the
point.** The world id is a digest of the file listing, and this file is in that listing, so
writing a world id down here changes it — no transcript pinned in a scanned document can survive
being written. The loss total moves with whatever happens to sit in `target/` and `.git/`: those
directories are excluded from *content*, but the walk still enumerates every file under them and
declares each one, so the total tracks the state of a build cache rather than the state of the
project. Treat both as observations of one run, not as expected output.

What *is* stable across runs, because it depends only on tracked non-excluded files:

- The verdict is **`Invalid`** under `rule/project-release-readiness-v1` with exactly one fired
  check, `unpinned_dependency`, and 109 world facts of which 6 are selected for the release query.
- The observed `unpinned_dependencies` binding names three declarations and no others: the two
  deliberately-unpinned fixture dependencies (`loose-gadget` in
  `fixtures/projects/demo-app/Cargo.toml`, `requests` in
  `fixtures/projects/bare-script/pyproject.toml`) and `typescript` (`^5.6.3`) in
  `typescript/package.json`. The workspace's own Rust dependencies are workspace-form
  declarations, which carry no version requirement and are therefore neither pinned nor unpinned.
- `unmapped_column` is 543 and `provenance_unavailable` is 1.

One observed run, with the two moving numbers in place:

```
DOGFOOD project-5e8630a2084d judged Invalid by rule/project-release-readiness-v1 with witnesses [unpinned_dependency]; 109 world facts, 6 selected; 8305 loss entries by kind {"content_uninterpreted": 7761, "provenance_unavailable": 1, "unmapped_column": 543}
```

The verdict was reached with the `scan_loss_summary` fact in the region: the oracle judged a tree
whose scan admitted, on the record, to thousands of things it did not read.
