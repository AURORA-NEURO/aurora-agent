---
name: check-parity
description: (aurora-agent workspace only) Verify the CPython reference, the eager Rust path and the indexed store still agree byte for byte on a Context Certificate digest. Use after touching canonical serialization, hashing, compiler passes, the store, or any serde configuration, and before any release or wire-format change.
---

<!-- Mirrored from .agents/skills/check-parity/SKILL.md by tools/sync_plugin_skills.py.
     Edit the source and re-run the sync; do not edit this copy. -->

# Check cross-language parity

Certificates hash canonical bytes. If two implementations disagree, a certificate produced by one
cannot be replayed by the other, and the guarantee the whole scheme rests on is gone.

Three implementations must agree on the reference certificate:

```
c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4
```

## Run it

```bash
python tools/regenerate_golden.py
git diff --exit-code fixtures/
cargo test -p bioprism-ids -p bioprism-fiber -p bioprism-store --offline
```

A non-empty `git diff` means the CPython reference itself moved. `bioprism-store`'s suite is what
proves the eager and indexed backends agree. CI runs all of this as its own job.

## Why it is fragile, so you know what to suspect

Matching CPython required reproducing three behaviours a natural Rust port gets wrong.

**Float formatting.** CPython switches to exponential notation at a different threshold than Rust
and zero-pads the exponent to two digits. `python_repr_f64` in `crates/ids/src/canonical.rs`
reimplements CPython's rule; `tools/gen_python_ground_truth.py` regenerates the expectation table.

**Float parsing.** serde_json's default parser is not correctly rounded and disagrees with both
CPython and native Rust near the subnormal boundary and at 2^53 — `2.2250738585072011e-308` lands
one ULP high, `9007199254740993.0` rounds the wrong way. The workspace enables the
`float_roundtrip` feature to fix it. **If that feature is dropped, parity breaks silently and the
golden fixtures will not catch it**, because they contain no such values. The regression test
`float_parsing_agrees_with_native_rust_and_cpython` is the only guard.

**Object iteration order.** The reference builds leakage witnesses in document order, so serde_json
runs with `preserve_order`. Canonical output sorts keys regardless, so digests are unaffected — but
witness *list* order is not.

**Non-finite floats, which are a hole rather than a fixed behaviour.** `CanonicalError::NonFiniteNumber`
exists and cannot fire on the path every producer uses: `serde_json::to_value` maps `NaN`, `+inf`
and `-inf` to `Value::Null` before the encoder sees a number, so all three hash as a stated null.
They do *not* collide with an absent key — that distinction survives — but a corrupt number is
silently laundered into a stated null, which is the wrong direction to fail in here. CPython's
`json.dumps` meanwhile emits the bare token `NaN`, which is not JSON and which serde would refuse to
parse, so the two languages disagree about what a non-finite float is and agree today only because
no producer emits one. `crates/ids/tests/non_finite_floats.rs` pins all of it. **If you add a float
field to a hashed format, validate it at the producer** — `bioprism_biolang::BioState::validate` is
the worked example.

## If a digest changed

Do not update the expected constant to make the test pass. A changed digest means either a real bug
or a deliberate wire-format change, and a deliberate change needs a schema version bump
(`fiber-context-certificate/0.1` to the next), not a new literal.

To find which side moved, hash the same document from both languages:

```bash
python -c "import json,hashlib; o=json.load(open('fixtures/fiber-v0.1/radiogenomic_world.json')); print(hashlib.sha256(json.dumps(o,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest())"
```

and compare against `ContentHash::of_value` on the same file.
