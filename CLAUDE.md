# Working in this repository

See [AGENTS.md](AGENTS.md). It is the single source of truth for how to work in this repository;
this file exists only so Claude Code finds it by its conventional name.

Two reminders that are easy to lose and expensive to relearn:

- `cargo test --workspace` is fine now, but per-crate (`-p`) is much faster and is what CI gates on.
- If a test suite reports `error: test failed` with `os error 4551`, the binary never executed —
  Windows Application Control blocked it. Touch a test file to force a relink.
