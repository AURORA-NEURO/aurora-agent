# Figures: drawing the artifacts you already have

`bioprism-figures` renders six deterministic SVG figures from serialized artifacts. Until now the
only way to reach them was `bioprism research run`, which draws seven figures of its own dossier
and nothing else. Every other command in this workspace prints JSON, and a user who compiled a
context, compared a panel, ran a sweep, drove an autopilot mission or exported an evidence bundle
had no way to draw any of it.

The `figure` command group is that way. It takes a JSON document, works out what is drawable
inside it, and writes SVG:

```
bioprism figure list   --input <path>
bioprism figure render --input <path> [--out-dir <dir>] [--kind <kind>] [--pointer <ptr>] [--dry-run]
bioprism figure batch  --input-dir <dir> [--out-dir <dir>] [--dry-run]
```

The renderers themselves are unchanged. What is new is the layer that finds their inputs.

## The six figures

| `--kind` | drawn from | shows |
|---|---|---|
| `baseline-panel` | a comparison | one bar per strategy, refused rows drawn as refused |
| `selection-ratio` | a context certificate | compiled facts and factors against their totals |
| `omission-accounting` | a context certificate | omitted facts by class, with the v0.1 summary's own caveat |
| `sweep-grid` | a sweep table | the structural family sweep, ties drawn as prominently as wins |
| `mutation-diversity` | a `Diversity` document | instances against independent equivalence classes |
| `autopilot-drive` | an autopilot report | the attempt sequence, in logical order and clock-free |

A certificate yields **two** figures at one location. The plan block and the omission block are
separate claims about one compile and neither can be derived from the other, so neither is a
default view of the certificate that the other has to be asked for.

## What is recognised, and what is only recognised

Detection classifies a document region into one of eight kinds:

| kind | how it is recognised | figures |
|---|---|---|
| `comparison` | `world_id`, `query_id`, `total_facts`, `reference`, `results` | 1 |
| `context-certificate` | `world_id`, `query_id`, `selected_facts`, a `plan` block and an `omissions` block | 2 |
| `sweep-table` | `seed` and a `cells` array | 1 |
| `mutation-diversity` | `instances`, `parents`, `families`, `signatures`, `equivalence_classes`, `inflation_ratio`, `caveat` | 1 |
| `mutation-family` | `parent_id`, `parent_sha256`, `accepted`, `rejected`, `duplicates` | 0 |
| `autopilot-report` | schema `bioprism-autopilot/report/0.1`, or `final_status`, `base_mission_id`, `attempts` and a `totals` block | 1 |
| `research-dossier` | schema `bioprism-research/dossier/0.1` | container |
| `cli-envelope` | a boolean `ok` | container |

`mutation-family` is in that table on purpose. It is a document this workspace produces, it is
recognised, and there is no renderer for it: the crate draws *effective diversity*, which is the
measurement over a family, not a family's membership list. A caller can therefore tell "I know
what this is and there is no figure for it" from "I have never seen this shape" — and the batch
manifest says which.

Both certificate profiles — `fiber-context-certificate/0.1` and the extended
`fiber-context-certificate/0.2-extended` — are recognised by the same required keys, because the
extended profile adds keys rather than replacing them.

## Detection is structural

A document is classified by the keys it carries and the schema strings it declares. Never by its
filename.

This is not a stylistic preference. A file called `comparison.json` that holds a certificate must
render as a certificate, and a certificate written to `out-3.json` must still be found — naming is
a habit, not evidence, and a builder that trusted the habit would draw the wrong figure exactly
when someone was careless. `bioprism context compile --certificate-out
nothing-in-this-name-says-certificate.json` produces a file the builder draws as a certificate;
that is a test in `crates/cli/tests/figure_contract.rs`, not an aspiration.

Where a shape publishes a schema string, that string is checked as a second and independent
statement rather than trusted as the first. A document declaring
`schema_version: "fiber-context-certificate/0.1"` while carrying no `plan` block makes two
statements that cannot both be true, and it is refused (`Inconsistent`) rather than repaired by
guesswork. The author is the only party who can say which statement to keep.

### Two kinds at one location is a refusal

If a value's keys satisfy two artifact shapes at once, detection refuses with `Inconsistent`
naming both. Choosing one would mean drawing a figure of something the document does not
unambiguously claim to be.

`cli-envelope` is deliberately outside that competition. An envelope is a wrapper whose
command-specific keys can *complete* an artifact's key set — `world sweep --json` emits `ok` and
`admissible_cells` alongside the sweep table's own `seed` and `cells`, so the envelope and the
artifact are one object. Envelope membership is a marker checked after the artifact shapes have
had their say, and it never makes a document ambiguous.

### Nothing drawable is an answer

A document this crate does not recognise yields an empty list, not an error. A world, a query, a
compile trace, a repair plan and an evidence bundle are all perfectly good documents that no
figure here draws, and reporting that as a failure would tell an operator their file is broken
when it is merely not a figure's input.

`figure list` reports the empty list and exits **0**: listing succeeded, and an empty list is the
answer. `figure render` exits **1**: it produced no artifact, which is a verdict about the input
rather than a failure of the command.

## Where the drawable regions are

Every detected region carries an RFC 6901 JSON pointer from the document root. `figure list`
prints it, `--pointer` selects on it, and the batch manifest records it.

| you have | pointer | note |
|---|---|---|
| `context compare --json > cmp.json` | `` (root) | the envelope is the comparison; there is no wrapper |
| `context compile --certificate-out cert.json` | `` (root) | two figures |
| `world sweep --json > sweep.json` | `` (root) | the envelope is a superset of the sweep table |
| `autopilot run --json > run.json` | `/report` | the envelope carries the report inline |
| `mutate family --json > family.json` | `/diversity` | the envelope root is a family, which draws nothing |
| `research run --out-dir out` → `out/dossier.json` | `/steps/<i>/outputs/<j>/artifact` | one per inlined drawable artifact |

Two entries in that table are worth stating plainly rather than leaving to be discovered:

- **`context compile --json` carries no certificate.** Its envelope summarises the compile and
  reports where the certificate was written. Point `figure render` at the `--certificate-out`
  file.
- **`research run --json` carries no dossier.** Same reason. Point `figure render` at the
  `dossier.json` in the `--out-dir`.

A dossier is walked at `steps[].outputs[].artifact`. An output record whose artifact was not
inlined carries a digest and no bytes; there is nothing to draw, and the dossier already states
the omission through its own `inlined: false`, so the walk passes over it rather than inventing an
entry.

The committed example dossier (`docs/research-example/dossier.json`) yields **13** figures — four
certificates at two figures each, three comparisons, one sweep table, one diversity document. The
report `research run` writes beside it draws seven: it draws the reference certificate and not the
certificate compiled at each distractor point. The builder reaches all of them.

### The scan is bounded

Detection reads the document root, the root's own members when the root is a `--json` envelope,
and a dossier's recorded artifacts. That is one container deep, plus the dossier walk. It is not a
recursive search of arbitrary JSON: an unbounded scan would start finding "artifacts" inside
fields that merely resemble them, and a figure of a coincidence is worse than no figure.

## The source digest, and what it does not prove

Every figure ends with `source sha256: <hex>`. It is `bioprism_ids::ContentHash::of_value` over
the exact value at the reported pointer — the workspace's single canonicalisation, the same
function that stamps `certificate_sha256`, `report_sha256` and `dossier_sha256`. It is computed
inside the renderer, never taken as a parameter, so a figure cannot mislabel its own source. The
same hex is reported as `source_sha256` by `figure render --json` and in the batch manifest.

**The digest identifies the artifact. It does not attest that the artifact is correct.**

Concretely, what it does and does not tell you:

- It **does** say which bytes were drawn, in canonical form, so whitespace and object-key order in
  your file do not change it, and it will match the digest any other component of this workspace
  computes for the same value. The 13 digests the builder reports for the committed dossier are
  the same 13 the dossier itself recorded for those artifacts.
- It **does not** check the artifact's own claimed digest. Nothing in the figure path recomputes
  `certificate_sha256` against its body. `context verify`, `autopilot verify` and `research
  verify` are the surfaces that do that, and a figure must not be mistaken for one.
- It **does not** say the artifact is sound, that its oracle judged correctly, or that its
  omissions were harmless. A figure of a broken compile is a faithful figure of a broken compile.

For a superset envelope like `world sweep --json`, the digest names the *envelope*, because the
envelope is the value that was rendered. That is the honest reading of "the digest of exactly what
was drawn", and it means the same sweep saved bare and saved as an envelope carry different hex.

## The honesty encodings survive the builder

The rendering rules are the renderers', and the builder changes none of them. They are repeated
here because they are the reason a figure from this workspace is worth more than a chart:

- **A refused row is drawn as a refused state, never as a zero-length bar.** The oracle never
  judged it, so it gets no verdict-coloured geometry that could be misread as a measured zero. Its
  cost, which *was* measured, is stated in text.
- **A tie is drawn with at least the visual weight of a win.** The sweep grid's headline result in
  this repository is a tie, and a legend that washed ties out would hide the finding the sweep
  exists to produce.
- **A contradiction is refused, not rendered.** An `admissible` flag disagreeing with the verdict
  fields it is defined from, a `cells_total` that disagrees with its own array, an
  `inflation_ratio` that disagrees with its own counts, an `attempts_used` that disagrees with the
  attempts drawn below it — each is `Inconsistent`, because rendering such a document would lend
  it a coherence it does not have.
- **A missing field is an error naming the field.** Nothing silently defaults to zero.
- **A document caveat travels verbatim.** The v0.1 omission summary's caveat, the diversity
  caveat, and the sweep's unswept-knob caveat are reproduced as written, not paraphrased.
- **No wall-clock axis anywhere.** The autopilot figure's axis reads "attempt sequence (logical,
  clock-free)" because the kernel owns no clock, and drawing durations would fabricate
  measurements no artifact contains.

One refusal fails a whole render. `figure render` and each `figure batch` input render every
figure into memory before any file is opened, so a document whose fifth artifact is refused leaves
no directory holding its first four: a half-written figure directory looks exactly like a complete
one.

## Filenames

A suggested filename is `<figure-kind>-<label>.svg`, where the label is the artifact's **own**
identity — a comparison's or certificate's `world_id`, a sweep's `seed-<n>`, a report's
`base_mission_id` — falling back to the name a container filed it under when the artifact names
nothing. A dossier's `outputs[].name` is the dossier's bookkeeping ("comparison-d50"); the
artifact's `world_id` is what the artifact is about, and the second is the better name.

Labels are reduced to `[a-z0-9.-]`, which can map two distinct ids onto one label. Filenames are
therefore checked for collisions within one document, and *every* claimant of a colliding name is
qualified with its pointer — not the second onward — so adding an artifact to a document never
renames the figure another artifact already had.

Across documents, `figure batch` writes each input's figures into `--out-dir/<input file stem>/`,
because two files in one directory can carry the same artifact and their suggested filenames would
then collide. Uniqueness within a document is detection's job; uniqueness across documents is the
batch's.

`--out-dir` defaults to `./figures`, the layout `research run` already writes.

## The batch manifest

`figure batch` walks the `*.json` files **directly inside** `--input-dir`. Non-recursive by
decision, not by omission: a recursive walk would descend into the `figures/` directory a previous
run wrote and into store indexes, and an operator who wanted one directory drawn would have no way
to say "not that one". Files that are not `*.json` are left alone.

It writes `--out-dir/manifest.json`:

```json
{
  "inputs":  ["<every *.json file considered, sorted>"],
  "figures": [{ "input": "...", "kind": "...", "pointer": "...",
                "filename": "...", "source_sha256": "..." }],
  "skipped": [{ "input": "...", "reason": "..." }]
}
```

- `inputs` is every candidate the walk considered, whether or not it produced anything.
- `filename` is the path actually written, relative to the working directory — not a bare name,
  because figures land in a per-input subdirectory.
- **`skipped` is first-class.** An input that could not be read, that is not valid JSON, that
  holds nothing drawable, or whose artifact was refused is a manifest entry with the reason, never
  a silent omission. A batch that dropped its skips would report a directory as fully drawn when
  part of it was never read, which is the one thing the manifest exists to stop.

A skip never moves the exit code by itself: the code follows whether any figure was produced at
all, so a batch that drew nine files and skipped one exits 0 with the skip named in the manifest
and in the human output. The manifest is written even when nothing at all was drawable — under
exit 1 — because in that case the manifest *is* the answer.

## Exit codes

Only the codes already in the registry, and each carries its published 40.36 retry decision.

- `0` — figures were produced, or a listing completed (including a listing of nothing).
- `1` — the selection is empty: the document holds nothing drawable, or `--kind`/`--pointer`
  selected none of what it holds, or no file in the batch directory was drawable. A completed run
  whose verdict is negative, not an error. Nothing is written except the batch manifest.
- `2` — usage: a `--kind` outside the registry, a `--pointer` that is not an RFC 6901 pointer, a
  missing required flag. Both are validated at parse time, because a mistyped flag accepted and
  matched later would surface as "nothing selected" and send the caller to inspect a file that is
  fine.
- `3` — invalid input: a document that is not readable JSON, or a document from which a figure
  cannot be drawn (missing field, wrong type, empty collection, internal contradiction). All the
  figure refusals share this code: each is a defect in the file the operator named, and each is
  fixed by editing it. The distinction between them lives in the message, which is where the
  caller has to look anyway to learn *which* field.
- `5` — a declared dependency could not be read or written.

## Limitations

- **Static SVG only.** No raster output; PNG encoding belongs to whatever displays the figure.
- **No interactivity.** No scripts, links, tooltips, or animation. A figure is evidence, not an
  application.
- **No wall-clock axes.** Stated above; it is a property of the artifacts, not of the renderer.
- **No styling API.** Palette, fonts and layout are fixed at compile time so the same artifact
  always looks the same. The two dimensions that follow the input — figure height, and the sweep
  grid's width — are computed from what was drawn, never from a caller-supplied knob.
- **The batch walk is non-recursive**, and considers only `*.json`.
- **Six figures.** Worlds, queries, compile traces, repair plans, evidence bundles, workflow
  reconciliations and readiness audits are all documents this workspace produces, and none of them
  has a renderer here. `figure list` says so rather than drawing something adjacent.
- **No verification.** Nothing in this path recomputes a claimed digest, re-runs an oracle, or
  checks a certificate against its world. The `verify` commands exist for that.
- **No text shaping.** Labels are truncated and wrapped by character count, not measured against
  font metrics; layout constants leave slack instead.

## Worked example

```
$ bioprism figure list --input fixtures/fiber-v0.1/golden/reference_certificate.json
fixtures/fiber-v0.1/golden/reference_certificate.json — 2 drawable region(s)

  figure               pointer  suggested filename
  selection-ratio      (root)   selection-ratio-radiogenomic-integrity-demo-v1.svg
  omission-accounting  (root)   omission-accounting-radiogenomic-integrity-demo-v1.svg

$ bioprism figure batch --input-dir docs/research-example --out-dir target/figbuild
figure batch: completed
  input directory: docs/research-example (non-recursive)
  inputs considered: 1
  figures: 13
  skipped: 0
  ...
  wrote target/figbuild/manifest.json (4423 bytes)
```

The seven figures that overlap with the ones `research run` committed beside that dossier come out
**byte-identical** to the committed files. That is pinned as a test
(`the_builder_reproduces_the_committed_report_figures_byte_for_byte`), so the builder and the
report renderer cannot drift into two different renderings of one document.
