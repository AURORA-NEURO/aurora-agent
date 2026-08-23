---
name: windows-test-truth
description: Diagnose Windows test runs that lie — os error 4551 means the binary never executed, and a workspace test total can silently lose every suite after a blocked one. Use when cargo test reports a failure with no failing test named, when a test count looks lower than expected on Windows, or when a freshly built binary refuses to start for no visible reason.
---

# Windows test truth (os error 4551)

Windows Application Control sometimes blocks a freshly linked binary. The
symptom in Rust is:

```
error: test failed ... (os error 4551)
```

**The binary never executed.** This is not a failing test — the suite NEVER
RAN. Two consequences:

1. A suite that never ran looks like a suite that failed.
2. `cargo test --workspace` keeps going and its final total silently loses
   every test in and after the blocked binary. One real run reported 344 tests
   where the true figure was 4,327.

## The recipe

Force a relink (a new binary usually passes) and prove the count is complete:

```bash
find crates -name '*.rs' -path '*/tests/*' -exec touch {} +
cargo test --workspace --offline --no-fail-fast 2>&1 | grep -c 'never executed'
cargo test --workspace --offline --no-fail-fast 2>&1 \
  | grep -E '^test result: ok' | awk '{s+=$4} END {print s}'
```

A non-zero first number invalidates the second. If touching is not enough,
delete the stale test executable under `target/` so the linker must produce a
fresh one.

## Related lies to watch for

- The same block can hit any freshly built executable, not just tests — a
  packaged app that "won't start" with an Application Control message needs a
  pristine known binary or a policy allowance, not a rebuild loop.
- Never sum per-crate results while other agents are concurrently editing
  crates: a crate mid-edit fails to compile and takes the whole invocation
  down for reasons unrelated to your change. Test per-crate (`-p`) in that
  situation.
