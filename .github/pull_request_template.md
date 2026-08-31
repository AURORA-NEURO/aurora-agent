# Summary

<!-- What this changes and why, in a few sentences. One concern per PR. -->

## Checklist

- [ ] Per-crate tests pass for every crate touched: `cargo test -p <crate> --offline`
- [ ] `cargo clippy -p <crate> --all-targets --offline` is at zero warnings
- [ ] `cargo fmt` run on the changed files
- [ ] No claim without a measurement — every number or capability stated in code, docs, or this description is backed by a test or measurement in the repository
- [ ] Docs updated (crate `lib.rs` not-implemented list, README, or docs/ as applicable)
- [ ] Changes touching canonical bytes, hashing, or the store preserve cross-language parity, or bump the schema version

<!-- On Windows: a suite reporting `error: test failed` with os error 4551 never ran.
     Touch a test file to force a relink, or use tools/status.sh --tests. -->
