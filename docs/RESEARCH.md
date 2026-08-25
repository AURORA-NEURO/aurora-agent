# Research runner: a request in, a digested dossier and a rendered report out

`bioprism-research` executes an autonomous research protocol over **synthetic decision worlds**
— committed fixtures and seeded generators — and nothing else. The blueprint does not specify a
research protocol runner; the request document, protocol shape, dossier schema, finding rules,
and report layout are this crate's design, stated as such. What the steps *measure* is
specified, and each step calls the crate that owns it: 43.26 context certificates via
`bioprism-fiber`, the 43.38 equal-engineering comparison and the 43.39 structural families and
sweep via `bioprism-baseline`/`bioprism-worldgen`, and the 03.08/32 metamorphic suite via
`bioprism-mutation`. The runner adds orchestration and receipts, never measurement logic.

The CLI surface is `research template`, `research run`, and `research verify`.

What the word "research" here does NOT prove: no biology or medicine, no literature or
prior-work coverage, no external-world observation, and no release-level claims from fixture
evidence. Oracle review is a human gate. Negative findings are first-class results — the
repository's own headline finding is a tie, and this runner is built to keep reporting it.
Research and developer infrastructure: it does not diagnose an individual, recommend treatment,
triage care, enroll participants, or claim medical-device functionality.

## The request document

`bioprism --json research template` prints a bare request object, directly usable as
`--request` after editing. The schema (`deny_unknown_fields`; any unrecognised field is a parse
error, not an ignored knob):

- `research_id` — required. Names the run in every artifact: 1..=64 characters from
  `[A-Za-z0-9._-]`.
- `question` — required, at most 4096 bytes. Recorded **verbatim** in the dossier and report,
  and **never interpreted**: the runner executes the protocol the other fields declare; it does
  not understand the question, and no code path anywhere branches on its content. The field
  exists so the dossier can state what was asked next to what was measured, and the reader —
  not the runner — judges whether the measurements bear on it.
- `family` — required. One committed 43.39 world-family preset: `reference_like`,
  `discriminating`, `external_confirmation`, or `policy_restricted`. Only the seed and the
  world id are overridden on the preset; skeleton, events, protected set, decision time, and
  policy stay at the preset's committed values. That is a deliberate ceiling, not an oversight.
- `distractor_points` — required. 1 to 6 counts, each at most 2000, no duplicates (generation
  is deterministic, so a repeated point reruns an identical measurement and inflates the
  protocol). The first point is the base world for the mutation and minimization steps.
- `seed` — required. Seed for every generated world. The optional sweep is the one exception:
  it runs the committed default grid at the grid's own seed, because that grid is the
  benchmark.
- `run_sweep`, `run_mutation`, `run_minimize` — optional, default `false`.

An invalid request exits 3 with the rule that refused it, not just the field.

## The protocol

`plan_protocol` is a pure function of the request — no I/O, no clock, no randomness — so the
same request always plans the same protocol, and the dossier echoes the plan next to the
executed steps with nothing to reconcile. `research run --dry-run` prints exactly this plan,
no-dispatch: nothing runs and nothing is written.

Step 0 is always the anchor: the committed `fixtures/fiber-v0.1` pair (embedded at build time)
is compiled and its certificate digest is required to equal the pinned cross-language parity
value `c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4` — the digest CPython,
the eager Rust path, and the indexed store agree on. A mismatch aborts the run: a dossier whose
anchor is broken would be a lie from step 0.

Then, per declared distractor point: generate the preset world and query, compile it and
round-trip the certificate through verification, and run the full 43.38 default panel
(13 strategies) over the pair. Then the optional steps: the committed 43.39 structural sweep,
the standard metamorphic suite over the base world, and the 1-minimal reduction of the base
world, re-verified.

A step that cannot complete is a typed error that aborts the run — there is no "step skipped"
and no partial dossier. The sweep deliberately does not vary decision-defining knobs (skeleton,
events, protected set, decision time, policy): they change what the decision is, not the
structure around it, and a sweep that varied them would be comparing strategies across
different questions.

## The dossier contract

`research run` writes three things into `--out-dir`: `dossier.json`, `REPORT.md`, and
`figures/*.svg`. The dossier (`bioprism-research/dossier/0.1`) carries:

- the request **verbatim**, and its canonical content digest (`request_digest`);
- the planned protocol, echoed;
- one record per executed step: the typed step, input digests, and every output artifact's
  name, sha256, and canonical byte count. Artifacts at or below 131072 canonical bytes are
  inlined whole; larger ones are digest-only, never truncated — a truncated JSON copy would be
  a malformed artifact pretending to be real, and the worlds regenerate deterministically from
  the request;
- the findings, each derived by a fixed public rule from a cited measurement, each at level
  `observation` — a single-variant enum, so no other level is representable — and each citing
  the content digests of the artifacts it was derived from. A tie between the compiler and a
  baseline is a *required* finding, flagged `negative: true`, in the same shape as any positive
  result;
- the seven required limitations, verbatim;
- `dossier_sha256`, computed over the canonical document with the digest field removed.

Every finding cites artifacts the dossier itself carries; a finding whose support digests name
nothing in the dossier fails verification. Exit 0 reports a completed run whatever the findings
say — a run whose every finding is negative exits 0, because a measured tie is a result, not a
failure.

## Verification

`research verify --dossier <path>` recomputes the digest and checks the structural contract,
printing a projection rather than a bare boolean: digest shape and match (a malformed claimed
digest is reported as `digest_malformed`, distinctly from tampering), request digest match,
required limitations present, step outcomes known, findings present, finding levels valid, and
finding support digests resolving to carried artifacts. Exit 1 if the dossier does not verify;
a document that is not a research dossier at all — wrong shape, wrong schema — exits 3, because
there is nothing to verify.

Verification proves the dossier is the unaltered output of a run and that its findings cite
carried evidence. It does NOT prove the findings matter, that the question was answered, or
that any measurement generalises beyond the synthetic worlds it ran on.

## The report

`REPORT.md` is a pure, byte-stable function of the dossier: the question reproduced verbatim
(with the statement that the runner did not interpret it), the protocol table, the findings
table — negative findings tagged `negative observation` in the same table, same register, no
appendix, no smaller type — the figures, the limitations, and reproduction commands. Every
figure links as `./figures/<filename>`, its caption names the source artifact and sha256, and
the SVG's own footer carries the same digest, computed over the exact value rendered — figure,
caption, and dossier record can all be checked against each other.

## Worked example

[`research-example/`](research-example/) is a committed run of this request:

```json
{
  "research_id": "admissibility-under-distractor-pressure",
  "question": "Which context strategies remain admissible as distractor pressure and structural camouflage increase?",
  "family": "discriminating",
  "distractor_points": [50, 250, 750],
  "seed": 20260823,
  "run_sweep": true,
  "run_mutation": true,
  "run_minimize": false
}
```

See [`research-example/REPORT.md`](research-example/REPORT.md) and
[`research-example/dossier.json`](research-example/dossier.json)
(dossier sha256 `46a740c5396151064a075ae213acf50b2508e26e2cd72ec429c0b87beac02802`). Of its
nine findings, seven are negative — including a tie at every declared distractor point
(`directed-walk-full` matches fiber's 11-fact cost) and ties in 36 of 36 sweep cells. That is
the honest headline of this example, rendered in the same register as the positive findings.

To reproduce and check it:

```text
bioprism --json research template > request.json
  (edit request.json to the document above)
bioprism research run --request request.json --out-dir out --dry-run
bioprism research run --request request.json --out-dir out
bioprism research verify --dossier out/dossier.json
```

The run is deterministic: the same request produces the same dossier, byte for byte, figures
included.
