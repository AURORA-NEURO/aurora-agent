---
name: verify-crate
description: (aurora-agent workspace only) Verify a bioprism crate is genuinely green — tests passing and clippy at zero warnings — and recognise the environment failures that masquerade as test failures. Use before claiming a crate is done, after landing an agent's work, when a test count looks wrong or lower than expected, or when `cargo test` reports a failure with no failing test named.
---

<!-- Mirrored from .agents/skills/verify-crate/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->

# Verify a crate

Green means both of these, per crate:

```bash
cargo test -p bioprism-<crate> --offline
cargo clippy -p bioprism-<crate> --all-targets --offline
```

Zero failures, zero warnings. `--offline` is not optional — this workspace builds against pinned
versions with no network, and omitting it produces a confusing registry error rather than a build.

## Do not trust a reported test count without running it

Agents and humans both miscount. Run the suite yourself and sum the `test result` lines:

```bash
cargo test -p bioprism-<crate> --offline 2>&1 | grep -E "^test result: ok" | awk '{s+=$4} END {print s}'
```

## The failure that is not a failure

Windows Application Control sometimes blocks a freshly linked test binary:

```
error: test failed, to rerun pass `-p bioprism-<crate> --test <suite>`
Caused by:
  could not execute process ... (never executed)
Caused by:
  An Application Control policy has blocked this file. (os error 4551)
```

**A suite that never ran looks exactly like a suite that failed.** Worse, `cargo test` continues to
the next binary, so a naive sum silently under-reports — this has already produced a wrong count
once (27 reported where the true figure was 75).

Force a relink to get a new binary hash the policy allows:

```bash
touch crates/<crate>/tests/*.rs
cargo test -p bioprism-<crate> --offline
```

If a per-crate count looks lower than expected, check for `never executed` before concluding
anything about the code.

### It bites the workspace sum harder than any single crate

This is the part that keeps costing time. **A blocked binary makes `cargo test --workspace` lose
every test after it, silently** — the run reports `error: test failed`, cargo moves on, and the sum
you take at the end is short by an amount nothing on screen tells you.

It has now hit five recorded times in this repository:

| Where | Reported | True |
|---|---:|---:|
| workspace, Batch I (`3a5bae2`) | 344 | 4,327 |
| workspace, `fiber` batch (`0f5b53b`) — three binaries blocked | short by 66 | 6,171 |
| `crates/fabric` (`18c5475`) | 130 | 176 |
| `crates/bioevalx` (`9a04c8b`) — one blocked, three later never ran | — | 113 |
| the original per-crate case in this skill | 27 | 75 |

The 344-against-4,327 run is the one to keep in mind: the shortfall is not a rounding error, and the
number looked plausible enough to be written down.

### Relink and recount

Touch every test source first, then run with `--no-fail-fast`, then **count the blocked binaries as
well as the tests**:

```bash
find crates -name '*.rs' -path '*/tests/*' -exec touch {} +
cargo test --workspace --offline --no-fail-fast 2>&1 | grep -c 'never executed'
cargo test --workspace --offline --no-fail-fast 2>&1 \
  | grep -E '^test result: ok' | awk '{s+=$4} END {print s}'
```

A non-zero first number invalidates the second. `tools/status.sh --tests` does exactly this and
prints a warning naming the shortfall, which is why the README's test count is generated rather than
typed.

**Do not trust the sum alone.** Check the per-binary `Running ...` lines against the crate list —
`ls crates | wc -l` is 79 — rather than accepting a total that has no way to tell you what is missing
from it. A crate whose binaries are all absent from the output looks identical to a crate with no
tests.

Do **not** run the workspace suite while agents are concurrently writing crates — a crate mid-edit
will not compile and will fail the whole invocation for reasons that have nothing to do with your
change. Use `-p` per crate in that situation.

## What a real test looks like here

A passing count is not evidence on its own. Before accepting a crate, read a few test names. They
should state the claim being made:

- good: `a_budget_smaller_than_the_closure_fails_rather_than_truncating`
- good: `an_unmeasured_capability_is_never_reported_as_a_low_score`
- bad: `test_budget_2`, `it_works`

A test that exercises a path without asserting an invariant inflates the count and protects
nothing. Prefer deleting it to keeping it.
