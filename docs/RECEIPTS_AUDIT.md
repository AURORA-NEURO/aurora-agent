# The receipts audit: a depth battery for digest-sealed documents

Every receipt this workspace emits makes the same claim — *recompute the digest and you will catch
any later edit*. Until now each verifier backed that claim with a handful of happy-path tests and
one hand-written tamper case, which establishes something much weaker: that the verifier catches
the one edit whoever wrote the test thought of.

`bioprism-receipts-audit` replaces that with a measurement. It enumerates every position in a
well-formed document, generates a structure-aware mutation at each one, states in advance whether
that mutation is formatting-only or semantic, and checks the verifier against the statement. What
comes out is a number: how many positions, how many cases, and how many the verifier got wrong.

The crate is a library plus a test suite. The library is reusable — a new digest-sealed document
type gets a battery by writing one closure — and the test suite is the workspace's current
application of it.

## The two expectations

Every generated case carries exactly one of two claims, and there is deliberately no third:

| expectation | means | example |
|---|---|---|
| `Expect::VerdictUnchanged` | the canonical bytes are identical and so is the verdict | an object's keys written in a different order |
| `Expect::Rejected` | the canonical bytes differ and the document must be refused | an array's elements written in a different order |

The pair is the point. JSON objects are unordered and JSON arrays are ordered, so exactly one of
those two examples may move a digest. If key ordering changed a verdict, the digests in this
workspace would be artefacts of one serializer rather than names for content, and the
cross-language replay the certificates depend on would be a coincidence. If array ordering did
*not* change a verdict, the digest would not be naming the document.

A generator that could not decide which expectation it was producing would be testing nothing,
which is why no `Expect::Unknown` exists. Cases whose canonical bytes turn out to equal the
original's are dropped before execution rather than asserted on — a digest cannot distinguish a
document from itself, and claiming such a case was "rejected" would be claiming something untrue.

## The generators

Thirteen families, all pure functions of the document and a seeded SplitMix64. Where a family has
more candidates than it emits — which sibling pair to swap, which hex digit to substitute, how far
to rotate an array — the choice comes from that generator, so the seed printed with every reported
hole reproduces the exact case.

| family | what it does | expectation |
|---|---|---|
| `digest_byte_flip` | one different hex character at **every** offset of **every** digest | rejected |
| `digest_length_change` | a digest one character short, one long, two long, headless, or empty | rejected |
| `digest_case_change` | a digest fully uppercased, and one character uppercased | rejected |
| `sibling_swap` | two same-typed siblings exchanged inside one container | rejected |
| `required_key_deletion` | each key of each visited object removed in turn | rejected |
| `array_element_deletion` | each element of each visited array removed in turn | rejected |
| `unexpected_key` | a key the schema does not know, sorting before and after every existing key | rejected |
| `numeric_near_equal` | an integer as its equal-valued float, a float one ULP away, a zero with its sign flipped | rejected |
| `object_key_reordering` | the same entries reversed and rotated | **unchanged** |
| `array_reordering` | the same elements reversed and rotated | rejected |
| `unicode_confusable_string` | Cyrillic homoglyphs, a precomposed/decomposed accent pair, an appended zero-width space | rejected |
| `empty_or_null_substitution` | each visited value replaced by `""` and by `null` | rejected |
| `wire_duplicate_key` | one object written with a key twice, parsed back with the last occurrence winning | rejected |

`sibling_swap` is the family a field-by-field validator is most likely to miss: no key is added,
removed, or retyped, and every value in the container is one the producer really emitted — only the
binding between name and value moved.

`numeric_near_equal` exists because `1` and `1.0` print the same to a careless reader and encode
differently. A verifier that accepted both interchangeably would make its digest depend on how a
caller's JSON parser happened to type a literal.

## The properties asserted

Each is a claim-named test in `crates/receipts-audit/tests/receipt_battery.rs`.

- **Every single-byte digest mutation is caught at every offset of every digest field.** Digest
  coverage is never bounded. A digest that catches tampering at 63 of its 64 offsets is not a
  digest, and a battery that sampled offsets could not tell the difference.
- **Object key reordering never changes a verdict at any position**, and the canonical bytes are
  asserted identical as well — if they were not, the defect would be in canonicalisation, not in
  the verifier, and the battery reports those separately.
- **Array reordering always changes a verdict at any position.**
- **A document whose sealing digest is absent is rejected distinctly from one whose digest is
  wrong.** Absent is `digest_absent`; wrong is `digest_mismatch`.
- **A shape-broken sealing digest is rejected as malformed and never as tampering.** The two
  answers accuse different parties: a mismatch says the body moved after the digest was taken, a
  shape defect says the claimed digest was never a digest. Reporting the second as the first would
  accuse a caller of tampering on the strength of a typo.
- **Deleting any field at any visited position is rejected and never silently accepted.**
- **A numeric near-equal substitution lands on one stable verdict and never between them** — each
  case is verified twice and both answers must agree.
- **A string replaced by a confusable form is rejected at every visited position.**
- **A swapped pair of same-typed siblings is rejected at every visited container.**
- **An unexpected key at any level is rejected**, except where a recorded gap says otherwise (see
  below).
- **An object written with a duplicate key resolves to a document that is rejected.**
- **A document fed to a verifier that does not own it is always rejected** — twenty cross-feeds,
  five verifiers against four foreign documents each.
- **Verifying the same document twice yields byte-identical projections**, for accepted documents
  and for rejected ones. A verifier that memoised its first answer would pass the accepted half
  alone.

### Differential checks

Three cross-implementation agreements, in the spirit of the repository's parity discipline:

- The certificate digest agrees across three paths: the value the producer embedded, a
  recomputation through `ContentHash::of_value`, and a direct `Sha256` over the canonical bytes
  with no wrapper in between. A pre-processing step inside the wrapper would show up here.
- Every inlined dossier artifact hashes to the digest its own record claims, and its recorded
  `canonical_bytes` is the length its content actually encodes to.
- The figure renderer and the dossier agree on every artifact digest: each figure's
  `source sha256:` footer, computed inside `bioprism-figures`, names an artifact the dossier
  records — and the digest the dossier verifier recomputes is the one the rendered report prints.

## What was measured

Six subjects, one seed (`0x9E3779B97F4A7C15`), 6,527 generated cases.

| subject | positions | digest fields | digest offsets | cases |
|---|---|---|---|---|
| context certificate | 39 of 39 | 4 | 256 | 498 |
| autopilot report | 93 of 93 | 11 | 704 | 1,307 |
| research dossier | **248 of 1,981** | 27 | 1,728 | 3,286 |
| mission evidence bundle | 34 of 34 | 2 | 128 | 335 |
| delivery receipt | 89 of 89 | 7 | 448 | 947 |
| delivery audit behind a fixed receipt | 39 of 39 | 0 | 0 | 154 |
| **total** | **542 of 2,275** | **51** | **3,264** | **6,527** |

Cases by family: 3,264 digest byte flips, 1,055 empty-or-null substitutions, 746 confusable
strings, 525 deletions, 357 digest shape changes, 190 unexpected keys, 114 key reorderings, 95
duplicate keys, 72 sibling swaps, 65 numeric substitutions, 44 array reorderings.

### The one bound, stated

The research dossier has 1,981 positions. The structural families visit **every eighth JSON
pointer in document order — 248 of them**. This is a bound, not a sample: it is a fixed stride over
the full traversal, so it is reproducible, spread across the whole document rather than
concentrated in a prefix, and reported with its step in `Coverage::bound_statement`, which every
assertion that touches the dossier prints. Digest coverage is *not* bounded by it: all 27 digest
fields are checked at all 64 offsets. Every other subject is exhaustive.

## What the battery found

Two holes, both fixed; one gap, recorded rather than closed.

### Fixed — the certificate verifier reported a typo as tampering

`ContextCertificate::verify` did not check the shape of `certificate_sha256` before comparing it.
A digest that was uppercase, truncated, or not hex at all recomputed to something different and was
reported as `DigestMismatch` — the answer that means *the body moved after the digest was taken*.
`verify_autopilot_report`, `verify_dossier`, and `verify_mission_evidence_bundle` all ship this
distinction and document it; the certificate, the oldest of the four, did not.

Fixed in `crates/section/src/certificate.rs`: a `certificate_sha256` that is not a 64-character
lowercase hex digest is now `Malformed` with a reason that names the field. The variant already
existed, so no consumer's match arm changed.

### Fixed — the delivery receipt verifier checked six fields out of twenty

`verify_delivery_receipt` recomputed the receipt from the delivery audit and then compared six
projections: the three digests, the targets, the evidence, and `release_candidate`. Nothing else.
`receipt_digest` is taken over the receipt's identity, digests, targets, evidence, and readiness
flag, so it does not cover the derived counts, `structurally_valid`, `verification`, `findings`, or
the guarantee and limitation text either. Those fourteen fields were sealed by nothing and compared
against nothing.

The battery generated 106 mutations of a stored receipt that verified as `valid: true`, including
emptying the receipt's own `limitations` array and setting `ready_target_count` to a number the
targets do not support. On a document whose purpose is to be a checkable handoff, unprotected
honesty text is the worst of the fourteen.

Fixed in `crates/devplat/src/delivery_receipt.rs`: every field the recomputation produces is now
compared against the stored receipt, under a `receipt_projection_mismatch` finding code that leaves
the six pre-existing codes separately identifiable. The same change gives the receipt the
malformed-versus-mismatch distinction the other verifiers have, as
`receipt_digest_malformed`.

### Recorded, not closed — an unrecognised key on a receipt is not checked

The projection comparison is one-directional: a stored receipt may carry fields the recomputation
does not, and those are ignored. This is not an oversight to fix later. The shipped MCP surface
returns the receipt with `ok`, `workflow`, `valid`, `receipt_ready`, and `delivery` written onto the
same object, so treating an unrecognised key as tampering would reject every receipt the server
hands out.

The battery records this as a `KnownGap` with that reason, and asserts it twice over: matching
cases are excused from the hole count, *and* the gap must still fire. Closing the underlying
behaviour without deleting the entry fails the battery, so the list cannot rot into a set of stale
excuses.

### Noted, not fixed — the certificate verifier checks a digest, not a schema

`ContextCertificate::verify` recomputes the embedded digest and nothing else. It does not check
that the document is a certificate. All twenty cross-document feeds in the battery are rejected,
because no other document type carries a `certificate_sha256` — but the rejection comes from the
absent field, not from a schema check, and a foreign document resealed with a self-consistent
`certificate_sha256` would verify.

This is reported rather than fixed. Adding a schema check changes the behaviour of a function
consumed by `crates/mcp/src/server.rs`, which this work could not modify, so the change could not be
validated end to end here.

## What this does not prove

The battery is a statement about verifiers. It is not any of the following, and a citation of the
numbers above should carry this paragraph with it.

- **Not a proof of collision resistance.** Nothing here tests SHA-256. Every case works by changing
  the canonical bytes and expecting the digest to notice; a battery cannot distinguish a strong hash
  from a weak one that happens to have no collisions among 6,527 mutations of one document.
- **Not a proof that the producing code is correct.** The battery starts from a document its
  verifier accepts and asks whether edits to it are caught. Whether the producer put the right
  values in that document in the first place is a different question, checked by each crate's own
  tests. The differential checks touch the edge of this and no more.
- **Not a security audit.** Digest verification says a document's canonical bytes are the ones its
  digest names. It says nothing about who produced it, whether they were entitled to, whether the
  document reached the verifier over a channel anyone controls, or whether a valid receipt means the
  thing it describes actually happened.
- **Not a proof about the wire form.** Verifiers receive a parsed `serde_json::Value`. The
  duplicate-key family checks that the *resolved* document — last occurrence winning — is rejected;
  it cannot check that duplication was detectable, because by the time a verifier runs, the
  information is gone.
- **Not a Unicode normalisation test.** The canonical encoder applies no normalisation, deliberately,
  because the CPython reference it must agree with byte for byte applies none either. The confusable
  family asserts that no verifier has quietly introduced normalisation or trimming that would erase a
  difference; it does not assert anything about normalisation forms themselves.
- **Not exhaustive over the research dossier's structure.** See the bound above.
- **Not a claim about documents other than the six built here.** A dossier from a different research
  request, or a receipt over a different delivery, has different positions. The library exists so
  that adding a subject is cheap; the numbers belong to the subjects that were run.

## Running it

```
cargo test -p bioprism-receipts-audit --offline
```

48 tests: 27 for the library's own generators and 21 for the battery. The battery's coverage
numbers are pinned equalities rather than lower bounds — a coverage number that can silently shrink
is not a coverage number, and a generator that stopped producing cases would otherwise turn the file
green by doing nothing.
