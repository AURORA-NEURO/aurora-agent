# Contributing to AURORA Agent

AURORA Agent is a Rust workspace of 79 crates implementing the FIBER decision-context
compiler. Before contributing, read [AGENTS.md](AGENTS.md) — it is the single source of
truth for how to work in this repository, and this document only distils it.

## Building

Builds are **offline** against pinned dependency versions:

```bash
cargo build --release --offline
```

On a fresh clone whose registry cache is empty, allow the first fetch explicitly and then
return to offline builds:

```bash
cargo build --release --config net.offline=false
```

You cannot add an external crate. Several components are hand-rolled for exactly this
reason (the CSV reader, the arg parser, JSON-RPC, log-gamma, RFC 3339 handling); follow
that pattern rather than proposing a new dependency.

## Testing

Prefer per-crate runs — they are much faster than a workspace run, and per-crate is what
CI gates on:

```bash
cargo test -p <crate> --offline
cargo clippy -p <crate> --all-targets --offline
```

Green means green: the tests pass **and** clippy reports zero warnings.

### Windows: `os error 4551`

Windows Application Control sometimes blocks a freshly linked test binary with
`os error 4551`. The binary **never executed** — but `cargo test` reports
`error: test failed` and moves on, so a suite that never ran looks like a suite that
failed, and a `--workspace` sum silently loses every test after it. Touch a test file to
force a relink; `tools/status.sh --tests` does that for you and warns if any binary still
refuses to run. Do not chase a "failure" until you have confirmed the binary actually ran.

## Honest labelling — the ground rules

Honest labelling is the product. Every system can tell you what it included; this one
tells you what it left out, and whether that could have mattered. Contributions must not
erode that. The non-negotiables:

- **Zero influence is not unknown influence.** "Provably cannot matter" and "nobody
  checked" are different states and must never share a representation. A single
  unknown-influence group voids a sufficiency claim.
- **Unmeasured is not zero.** A capability with no evidence is `Unmeasured`,
  categorically distinct from measured-and-poor. There is no `score_or_zero`.
- **A right answer from an incomplete basis is not a pass.** Protected closure is
  mandatory before any relevance step, so a strategy cannot be credited for guessing
  correctly from evidence it never had.
- **Declared is not enforced.** A limitation that is stated is a limitation; a capability
  that is implied to exist but does not is a lie. Every crate's `lib.rs` carries an
  explicit list of what is not implemented — keep it current.
- **Invariants belong in the type system, not in comments.** Where a rule can be made
  unrepresentable, make it unrepresentable (`Budget` does not implement `Clone`;
  `approve()` is the only path to a `DecisionCell`; `ReplayHost` has no live-source
  field). A test that asserts a rule is good; a type that makes the rule unbreakable is
  better.
- **Negative results ship.** If a measurement disagrees with the thesis, that is the
  measurement we publish.

Tests state their claim in the name
(`a_budget_smaller_than_the_closure_fails_rather_than_truncating`, not `test_budget_2`).
Smoke tests that exercise a path without asserting an invariant inflate the count and are
worse than no test. Doc comments explain *why*; no `//` comments restating what a line
does.

Anything touching canonical bytes, hashing, or the store must preserve cross-language
parity — three implementations currently agree on the reference certificate. Do not break
that without a schema version bump.

## Commits

Small, semantic commits, one concern each: a commit that adds a module, a commit that
fixes a defect, a commit that records a measurement. The history should read as the
argument for the design, not as a series of "wip" saves.

## Pull requests

CI runs `cargo fmt`, `cargo clippy`, and the test suites on every pull request. Before
opening one:

- run the per-crate tests for every crate you touched;
- run `cargo fmt` on the files you changed;
- make no claim in code, docs, or the PR description that is not backed by a measurement
  in the repository;
- update the documentation that your change affects.

## Boundary

Research and developer infrastructure: it does not diagnose an individual, recommend
treatment, triage care, enroll participants, or claim medical-device functionality.
Contributions that route around the typed research boundary will not be accepted.
